<script lang="ts">
  import { onDestroy } from "svelte";

  import type { PickerCustomEmoji } from "$lib/emoji/picker";
  import "prosemirror-view/style/prosemirror.css";
  import { baseKeymap } from "prosemirror-commands";
  import { history, redo, undo } from "prosemirror-history";
  import { keymap } from "prosemirror-keymap";
  import { EditorState, TextSelection } from "prosemirror-state";
  import { EditorView } from "prosemirror-view";

  import { buildDocFromDraft, composerSchema } from "./editorSchema";
  import { docToDraft, getSelectionOffsetsFromState, textOffsetToPos } from "./editorHelpers";
  import type { ShortcodeRange, SuggestionPosition } from "./types";

  interface Props {
    draft: string;
    placeholder: string;
    isDisabled: boolean;
    isSending: boolean;
    pickerCustomEmoji: PickerCustomEmoji[];
    shortcodeRange: ShortcodeRange | null;
    shortcodeSuggestionCount: number;
    onDraftChange?: (draft: string) => void;
    onCursorChange?: (cursor: number) => void;
    onSubmit?: (draft: string) => void;
    onSuggestionPositionChange?: (position: SuggestionPosition | null) => void;
  }

  let {
    draft,
    placeholder,
    isDisabled,
    isSending,
    pickerCustomEmoji,
    shortcodeRange,
    shortcodeSuggestionCount,
    onDraftChange,
    onCursorChange,
    onSubmit,
    onSuggestionPositionChange,
  }: Props = $props();

  let editorMountElement = $state<HTMLDivElement | null>(null);
  let editorView = $state<EditorView | null>(null);
  let cursorPosition = $state(0);
  let suppressDraftSync = false;

  onDestroy(() => {
    editorView?.destroy();
  });

  $effect(() => {
    if (!editorMountElement || editorView) {
      return;
    }

    const initialDoc = buildDocFromDraft(draft, pickerCustomEmoji);

    const state = EditorState.create({
      schema: composerSchema,
      doc: initialDoc,
      plugins: [
        history(),
        keymap({
          "Mod-z": undo,
          "Shift-Mod-z": redo,
          "Mod-y": redo,
        }),
        keymap(baseKeymap),
      ],
    });

    const view = new EditorView(editorMountElement, {
      state,
      editable: () => !isDisabled && !isSending,
      clipboardTextSerializer: (slice) =>
        slice.content.textBetween(0, slice.content.size, "\n", (leafNode) => {
          if (leafNode.type.name === "customEmoji") {
            return leafNode.attrs.token ?? "";
          }

          return "";
        }),
      handleKeyDown: (view, event) => {
        if (event.key !== "Enter") {
          return false;
        }

        if (event.shiftKey) {
          event.preventDefault();
          const selection = getSelectionOffsetsFromState(view.state);
          const currentDraft = docToDraft(view.state.doc);
          const next = currentDraft.slice(0, selection.start) + "\n" + currentDraft.slice(selection.end);
          applyDraftToEditor(next, selection.start + 1);
          return true;
        }

        event.preventDefault();
        if (isDisabled || isSending) {
          return true;
        }

        const nextDraft = docToDraft(view.state.doc);
        if (nextDraft.trim().length === 0) {
          return true;
        }

        onSubmit?.(nextDraft);
        clearDraft();
        return true;
      },
      dispatchTransaction: (tr) => {
        const nextState = view.state.apply(tr);
        const selectionOffsets = getSelectionOffsetsFromState(nextState);

        const nextDraft = docToDraft(nextState.doc);
        const normalizedDoc = buildDocFromDraft(nextDraft, pickerCustomEmoji);

        if (normalizedDoc.eq(nextState.doc)) {
          view.updateState(nextState);
        } else {
          const normalizedState = EditorState.create({
            schema: composerSchema,
            doc: normalizedDoc,
            plugins: view.state.plugins,
          });

          const from = textOffsetToPos(normalizedDoc, selectionOffsets.start);
          const to = textOffsetToPos(normalizedDoc, selectionOffsets.end);
          const selection = TextSelection.create(normalizedDoc, from, to);
          view.updateState(normalizedState.apply(normalizedState.tr.setSelection(selection)));
        }

        const appliedState = view.state;
        const updatedDraft = docToDraft(appliedState.doc);
        if (draft !== updatedDraft) {
          suppressDraftSync = true;
          draft = updatedDraft;
          onDraftChange?.(updatedDraft);
          queueMicrotask(() => {
            suppressDraftSync = false;
          });
        }

        cursorPosition = getSelectionOffsetsFromState(appliedState).end;
        onCursorChange?.(cursorPosition);
      },
    });

    view.dom.setAttribute("role", "textbox");
    view.dom.setAttribute("aria-multiline", "true");
    view.dom.setAttribute("data-placeholder", placeholder);
    view.dom.classList.add("composer-editor", "input", "overflow-y-auto");

    cursorPosition = getSelectionOffsetsFromState(view.state).end;
    onCursorChange?.(cursorPosition);
    editorView = view;
  });

  $effect(() => {
    if (!editorView) {
      return;
    }

    editorView.setProps({
      ...editorView.props,
      editable: () => !isDisabled && !isSending,
    });

    if (isDisabled || isSending) {
      editorView.dom.setAttribute("tabindex", "-1");
    } else {
      editorView.dom.setAttribute("tabindex", "0");
    }
  });

  $effect(() => {
    if (!editorView) {
      return;
    }

    editorView.dom.setAttribute("data-placeholder", placeholder);
  });

  $effect(() => {
    if (!editorView || suppressDraftSync) {
      return;
    }

    const currentDraft = docToDraft(editorView.state.doc);
    if (currentDraft === draft) {
      return;
    }

    applyDraftToEditor(draft, Math.min(cursorPosition, draft.length));
  });

  $effect(() => {
    if (!editorView || !shortcodeRange || shortcodeSuggestionCount === 0) {
      onSuggestionPositionChange?.(null);
      return;
    }

    const updateSuggestionPopupPosition = () => {
      if (!editorView || !shortcodeRange) {
        onSuggestionPositionChange?.(null);
        return;
      }

      const formElement = editorView.dom.closest("form");
      if (!(formElement instanceof HTMLElement)) {
        onSuggestionPositionChange?.(null);
        return;
      }

      const anchorPos = textOffsetToPos(editorView.state.doc, shortcodeRange.start);
      const anchorCoords = editorView.coordsAtPos(anchorPos);
      const formRect = formElement.getBoundingClientRect();

      const left = Math.max(8, Math.min(anchorCoords.left - formRect.left, formRect.width - 220));
      const top = anchorCoords.top - formRect.top - 8;
      onSuggestionPositionChange?.({ top, left });
    };

    updateSuggestionPopupPosition();

    window.addEventListener("resize", updateSuggestionPopupPosition);
    const editorDom = editorView.dom;
    editorDom.addEventListener("scroll", updateSuggestionPopupPosition);

    return () => {
      window.removeEventListener("resize", updateSuggestionPopupPosition);
      editorDom.removeEventListener("scroll", updateSuggestionPopupPosition);
    };
  });

  function applyDraftToEditor(nextDraft: string, cursorOffset: number) {
    if (!editorView) {
      draft = nextDraft;
      onDraftChange?.(nextDraft);
      cursorPosition = cursorOffset;
      onCursorChange?.(cursorOffset);
      return;
    }

    const nextDoc = buildDocFromDraft(nextDraft, pickerCustomEmoji);
    const nextState = EditorState.create({
      schema: composerSchema,
      doc: nextDoc,
      plugins: editorView.state.plugins,
    });

    const clamped = Math.max(0, Math.min(cursorOffset, nextDraft.length));
    const pos = textOffsetToPos(nextDoc, clamped);
    const selection = TextSelection.create(nextDoc, pos, pos);

    editorView.updateState(nextState.apply(nextState.tr.setSelection(selection)));
    editorView.focus();

    suppressDraftSync = true;
    draft = nextDraft;
    onDraftChange?.(nextDraft);
    cursorPosition = clamped;
    onCursorChange?.(clamped);

    queueMicrotask(() => {
      suppressDraftSync = false;
    });
  }

  export function clearDraft() {
    applyDraftToEditor("", 0);
  }

  export function insertAtCursor(text: string) {
    const selection = editorView
      ? getSelectionOffsetsFromState(editorView.state)
      : { start: cursorPosition, end: cursorPosition };
    const currentDraft = editorView ? docToDraft(editorView.state.doc) : draft;
    const next = currentDraft.slice(0, selection.start) + text + currentDraft.slice(selection.end);
    const nextPosition = selection.start + text.length;
    applyDraftToEditor(next, nextPosition);
  }

  export function replaceRange(start: number, end: number, replacement: string) {
    const currentDraft = editorView ? docToDraft(editorView.state.doc) : draft;
    const next = currentDraft.slice(0, start) + replacement + currentDraft.slice(end);
    applyDraftToEditor(next, start + replacement.length);
  }
</script>

<div bind:this={editorMountElement}></div>
