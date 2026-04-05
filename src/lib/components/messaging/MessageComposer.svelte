<script lang="ts">
  import { onDestroy, onMount } from "svelte";

  import {
    applyCustomEmojiConfig,
    ensureEmojiPickerLoaded,
    getShortcodeSuggestions,
    selectedEmojiToken,
  } from "$lib/emoji/picker";
  import type { EmojiShortcodeSuggestion, PickerCustomEmoji } from "$lib/emoji/picker";
  import "prosemirror-view/style/prosemirror.css";
  import { baseKeymap } from "prosemirror-commands";
  import { history, redo, undo } from "prosemirror-history";
  import { keymap } from "prosemirror-keymap";
  import { Node as PMNode, Schema } from "prosemirror-model";
  import { EditorState, TextSelection } from "prosemirror-state";
  import { EditorView } from "prosemirror-view";

  type EmojiClickDetail = {
    unicode?: string;
    name?: string;
    emoji?: {
      shortcodes?: string[];
      name?: string;
    };
  };

  interface Props {
    draft: string;
    error: string;
    isSending: boolean;
    isDisabled: boolean;
    placeholder?: string;
    pickerCustomEmoji?: PickerCustomEmoji[];
    onSubmit?: (draft: string) => void;
    onDraftChange?: (draft: string) => void;
  }

  const TOKEN_PATTERN = /:([A-Za-z0-9_+\-]+):/g;

  const composerSchema = new Schema({
    nodes: {
      doc: { content: "paragraph+" },
      paragraph: {
        content: "(text|customEmoji)*",
        parseDOM: [{ tag: "p" }],
        toDOM: () => ["p", 0],
      },
      text: {},
      customEmoji: {
        inline: true,
        group: "inline",
        atom: true,
        selectable: true,
        draggable: false,
        attrs: {
          token: {},
          url: { default: "" },
        },
        parseDOM: [
          {
            tag: "img[data-custom-emoji-token]",
            getAttrs: (dom) => {
              if (!(dom instanceof HTMLImageElement)) {
                return false;
              }

              const token = dom.dataset.customEmojiToken?.trim().toLowerCase();
              if (!token || !/^:[A-Za-z0-9_+\-]+:$/.test(token)) {
                return false;
              }

              return {
                token,
                url: dom.getAttribute("src") ?? "",
              };
            },
          },
          {
            tag: "img[alt]",
            getAttrs: (dom) => {
              if (!(dom instanceof HTMLImageElement)) {
                return false;
              }

              const token = dom.alt?.trim().toLowerCase();
              if (!token || !/^:[A-Za-z0-9_+\-]+:$/.test(token)) {
                return false;
              }

              return {
                token,
                url: dom.getAttribute("src") ?? "",
              };
            },
          },
        ],
        toDOM: (node) => [
          "img",
          {
            src: node.attrs.url,
            alt: node.attrs.token,
            title: String(node.attrs.token).replace(/^:+|:+$/g, ""),
            "aria-label": String(node.attrs.token).replace(/^:+|:+$/g, ""),
            "data-custom-emoji-token": node.attrs.token,
            contenteditable: "false",
            class: "composer-custom-emoji",
          },
        ],
        leafText: (node) => node.attrs.token as string,
      },
    },
  });

  let {
    draft,
    error,
    isSending,
    isDisabled,
    placeholder = "Write a message...",
    pickerCustomEmoji = [],
    onSubmit,
    onDraftChange,
  }: Props = $props();

  let showEmojiPicker = $state(false);
  let pickerElement = $state<HTMLElement | null>(null);
  let pickerPopoverElement = $state<HTMLElement | null>(null);
  let composerFormElement = $state<HTMLFormElement | null>(null);
  let editorMountElement = $state<HTMLDivElement | null>(null);
  let editorView = $state<EditorView | null>(null);
  let cursorPosition = $state(0);
  let draftShortcodeSuggestions = $state<EmojiShortcodeSuggestion[]>([]);
  let draftShortcodeRange = $state<{ start: number; end: number } | null>(null);
  let suggestionPopupPosition = $state<{ top: number; left: number } | null>(null);
  let suppressDraftSync = false;

  onMount(() => {
    void ensureEmojiPickerLoaded();
  });

  onDestroy(() => {
    editorView?.destroy();
  });

  $effect(() => {
    if (!showEmojiPicker || !pickerElement) {
      return;
    }

    applyCustomEmojiConfig(pickerElement as never, pickerCustomEmoji);
  });

  $effect(() => {
    if (!showEmojiPicker) {
      return;
    }

    const handleOutsidePointerDown = (event: PointerEvent) => {
      const target = event.target;
      if (!(target instanceof Node)) {
        return;
      }

      if (pickerPopoverElement?.contains(target)) {
        return;
      }

      showEmojiPicker = false;
    };

    window.addEventListener("pointerdown", handleOutsidePointerDown, true);

    return () => {
      window.removeEventListener("pointerdown", handleOutsidePointerDown, true);
    };
  });

  $effect(() => {
    let cancelled = false;

    void (async () => {
      const suggestionResult = await getShortcodeSuggestions(draft, cursorPosition, pickerCustomEmoji);
      if (cancelled) {
        return;
      }

      draftShortcodeSuggestions = suggestionResult?.suggestions ?? [];
      draftShortcodeRange = suggestionResult
        ? { start: suggestionResult.start, end: suggestionResult.end }
        : null;
    })();

    return () => {
      cancelled = true;
    };
  });

  $effect(() => {
    if (!editorView || !draftShortcodeRange || draftShortcodeSuggestions.length === 0 || !composerFormElement) {
      suggestionPopupPosition = null;
      return;
    }

    const updateSuggestionPopupPosition = () => {
      if (!editorView || !draftShortcodeRange || !composerFormElement) {
        suggestionPopupPosition = null;
        return;
      }

      const anchorPos = textOffsetToPos(editorView.state.doc, draftShortcodeRange.start);
      const anchorCoords = editorView.coordsAtPos(anchorPos);
      const formRect = composerFormElement.getBoundingClientRect();

      const left = Math.max(8, Math.min(anchorCoords.left - formRect.left, formRect.width - 220));
      const top = anchorCoords.top - formRect.top - 8;
      suggestionPopupPosition = { top, left };
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
      },
    });

    view.dom.setAttribute("role", "textbox");
    view.dom.setAttribute("aria-multiline", "true");
    view.dom.setAttribute("data-placeholder", placeholder);
    view.dom.classList.add("composer-editor", "input", "overflow-y-auto");

    cursorPosition = getSelectionOffsetsFromState(view.state).end;
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

  function handleSubmit(event: SubmitEvent) {
    event.preventDefault();
    if (!isSending && draft.trim().length > 0) {
      onSubmit?.(draft);
      clearDraft();
    }
  }

  function clearDraft() {
    applyDraftToEditor("", 0);
    draftShortcodeSuggestions = [];
    draftShortcodeRange = null;
  }

  function insertEmoji(emoji: string) {
    const selection = editorView
      ? getSelectionOffsetsFromState(editorView.state)
      : { start: cursorPosition, end: cursorPosition };
    const currentDraft = editorView ? docToDraft(editorView.state.doc) : draft;
    const next = currentDraft.slice(0, selection.start) + emoji + currentDraft.slice(selection.end);
    const nextPosition = selection.start + emoji.length;
    applyDraftToEditor(next, nextPosition);
  }

  function applyShortcodeSuggestion(suggestion: EmojiShortcodeSuggestion) {
    const range = draftShortcodeRange;
    if (!range) {
      return;
    }

    const next =
      draft.slice(0, range.start) +
      suggestion.replacement +
      draft.slice(range.end);

    draftShortcodeSuggestions = [];
    draftShortcodeRange = null;

    applyDraftToEditor(next, range.start + suggestion.replacement.length);
  }

  function handleEmojiClick(event: CustomEvent<EmojiClickDetail>) {
    const selected = selectedEmojiToken(event.detail);
    if (!selected) {
      return;
    }

    insertEmoji(selected);
    showEmojiPicker = false;
  }

  function applyDraftToEditor(nextDraft: string, cursorOffset: number) {
    if (!editorView) {
      draft = nextDraft;
      onDraftChange?.(nextDraft);
      cursorPosition = cursorOffset;
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

    queueMicrotask(() => {
      suppressDraftSync = false;
    });
  }

  function buildCustomEmojiByToken(customEmoji: PickerCustomEmoji[]): Map<string, PickerCustomEmoji> {
    const map = new Map<string, PickerCustomEmoji>();

    for (const emoji of customEmoji) {
      for (const shortcode of emoji.shortcodes ?? []) {
        const normalized = shortcode.trim().replace(/^:+|:+$/g, "").toLowerCase();
        if (!normalized) {
          continue;
        }

        const token = `:${normalized}:`;
        if (!map.has(token)) {
          map.set(token, emoji);
        }
      }
    }

    return map;
  }

  function buildInlineNodesFromText(value: string, customEmoji: PickerCustomEmoji[]) {
    const customByToken = buildCustomEmojiByToken(customEmoji);
    const nodes: PMNode[] = [];

    let lastIndex = 0;
    for (const match of value.matchAll(TOKEN_PATTERN)) {
      const fullMatch = match[0] ?? "";
      const start = match.index ?? -1;
      if (start < 0) {
        continue;
      }

      if (start > lastIndex) {
        nodes.push(composerSchema.text(value.slice(lastIndex, start)));
      }

      const token = fullMatch.toLowerCase();
      const custom = customByToken.get(token);
      if (custom?.url) {
        nodes.push(composerSchema.node("customEmoji", { token, url: custom.url }));
      } else {
        nodes.push(composerSchema.text(fullMatch));
      }

      lastIndex = start + fullMatch.length;
    }

    if (lastIndex < value.length) {
      nodes.push(composerSchema.text(value.slice(lastIndex)));
    }

    return nodes;
  }

  function buildDocFromDraft(value: string, customEmoji: PickerCustomEmoji[]) {
    const lines = value.split("\n");
    const paragraphs = lines.map((line) => composerSchema.node("paragraph", null, buildInlineNodesFromText(line, customEmoji)));

    if (paragraphs.length === 0) {
      paragraphs.push(composerSchema.node("paragraph"));
    }

    return composerSchema.node("doc", null, paragraphs);
  }

  function docToDraft(doc: PMNode): string {
    const paragraphs: string[] = [];
    doc.forEach((node) => {
      paragraphs.push(draftBetween(node, 0, node.content.size));
    });

    return paragraphs.join("\n");
  }

  function draftBetween(docNode: PMNode, from: number, to: number): string {
    return docNode.textBetween(from, to, "\n", (leafNode) => {
      if (leafNode.type.name === "customEmoji") {
        return leafNode.attrs.token ?? "";
      }

      return "";
    });
  }

  function getSelectionOffsetsFromState(state: EditorState): { start: number; end: number } {
    const { from, to } = state.selection;

    return {
      start: draftBetween(state.doc, 0, from).length,
      end: draftBetween(state.doc, 0, to).length,
    };
  }

  function inlineNodeTextLength(node: { type: { name: string }; text?: string; attrs: { token?: string } }): number {
    if (node.type.name === "customEmoji") {
      return (node.attrs.token ?? "").length;
    }

    return node.text?.length ?? 0;
  }

  function textOffsetToPos(doc: PMNode, textOffset: number): number {
    let remaining = Math.max(0, textOffset);
    let resultPos = doc.content.size;

    doc.forEach((paragraph, paragraphOffset, index) => {
      if (remaining < 0 || paragraph.type.name !== "paragraph") {
        return;
      }

      const paragraphStart = paragraphOffset + 1;
      let found = false;

      paragraph.forEach((child, childOffset) => {
        if (found) {
          return;
        }

        const childPos = paragraphStart + childOffset;
        const childLength = inlineNodeTextLength(child);

        if (remaining <= childLength) {
          if (child.type.name === "text") {
            resultPos = childPos + remaining;
          } else {
            resultPos = remaining === 0 ? childPos : childPos + 1;
          }
          remaining = -1;
          found = true;
          return;
        }

        remaining -= childLength;
      });

      if (found) {
        return;
      }

      const paragraphEnd = paragraphStart + paragraph.content.size;
      if (remaining === 0) {
        resultPos = paragraphEnd;
        remaining = -1;
        return;
      }

      if (index < doc.childCount - 1) {
        if (remaining === 1) {
          const nextParagraphOffset = paragraphOffset + paragraph.nodeSize;
          resultPos = Math.min(nextParagraphOffset + 1, doc.content.size);
          remaining = -1;
          return;
        }

        remaining -= 1;
      }
    });

    return Math.max(0, Math.min(resultPos, doc.content.size));
  }
</script>

<form
  class="card p-3 mt-3 preset-outlined-surface-200-800 bg-surface-100-900 relative"
  onsubmit={handleSubmit}
  bind:this={composerFormElement}
>
  {#if error}
    <p class="mb-2 text-sm preset-filled-error-500 card p-2">{error}</p>
  {/if}

  <label class="text-xs text-surface-700-300 mb-1" for="message-draft">Message</label>
  <div class="mb-2 flex items-center gap-2">
    <div class="relative" bind:this={pickerPopoverElement}>
      <button
        type="button"
        class="btn btn-sm preset-tonal"
        onclick={() => {
          showEmojiPicker = !showEmojiPicker;
        }}
        disabled={isDisabled || isSending}
      >
        Emoji
      </button>
      {#if showEmojiPicker}
        <div class="absolute bottom-full left-0 mb-2 z-20 card p-2 preset-outlined-surface-300-700 bg-surface-50-950 w-[30rem] max-w-[94vw]">
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <emoji-picker
            bind:this={pickerElement}
            style="width: 100%; height: 22rem; --emoji-size: 2.5rem; --category-emoji-size: 1.5rem; --num-columns: 7;"
            onemoji-click={handleEmojiClick}
          ></emoji-picker>
        </div>
      {/if}
    </div>

  </div>

  <div id="message-draft" bind:this={editorMountElement}></div>

  {#if draftShortcodeSuggestions.length > 0 && suggestionPopupPosition}
    <div
      class="absolute z-30 card preset-outlined-surface-300-700 bg-surface-50-950 p-1 w-56"
      style={`left: ${suggestionPopupPosition.left}px; top: ${suggestionPopupPosition.top}px; transform: translateY(-100%);`}
    >
      <div class="flex flex-col gap-1">
        {#each draftShortcodeSuggestions as suggestion (suggestion.token)}
          <button
            type="button"
            class="btn btn-xs preset-tonal justify-start"
            onclick={() => applyShortcodeSuggestion(suggestion)}
            disabled={isDisabled || isSending}
            title={`Insert ${suggestion.token}`}
          >
            {#if suggestion.kind === "custom" && suggestion.previewUrl}
              <img src={suggestion.previewUrl} alt={suggestion.token} class="inline-block h-4 w-4 rounded-sm" />
              {suggestion.token}
            {:else}
              {suggestion.replacement} {suggestion.token}
            {/if}
          </button>
        {/each}
      </div>
    </div>
  {/if}

  <div class="mt-2 flex justify-end">
    <button
      type="submit"
      class="btn preset-filled"
      disabled={isDisabled || isSending || draft.trim().length === 0}
    >
      {#if isSending}Sending...{:else}Send{/if}
    </button>
  </div>
</form>

<style>
  #message-draft :global(.composer-editor) {
    font-size: 1.2rem;
    line-height: 1.45;
    outline: none;
    min-height: 2.5rem;
    max-height: 10rem;
    padding-top: 0.25rem;
    padding-bottom: 0.25rem;
    height: auto;
  }
  #message-draft :global(.composer-editor p) {
    margin: 0;
  }

  #message-draft :global(.composer-editor p + p) {
    margin-top: 0.12rem;
  }

  #message-draft :global(.composer-editor:empty::before),
  #message-draft :global(.composer-editor p:only-child:empty::before) {
    content: attr(data-placeholder);
    color: color-mix(in oklab, currentColor 55%, transparent);
    pointer-events: none;
  }

  #message-draft :global(.composer-custom-emoji) {
    display: inline-block;
    vertical-align: text-bottom;
    width: 1.4rem;
    height: 1.4rem;
    max-width: 100%;
    max-height: 1.8em;
    margin-inline: 0.04rem;
  }
</style>
