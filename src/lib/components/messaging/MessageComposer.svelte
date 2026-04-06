<script lang="ts">
  import { getShortcodeSuggestions } from "$lib/emoji/picker";
  import type { EmojiShortcodeSuggestion, PickerCustomEmoji } from "$lib/emoji/picker";
  import { ShortcodeSuggestions } from "./composer-emoji";
  import { ComposerActions, ComposerEditor, ComposerErrorBanner, ComposerToolbar } from "./composer";
  import type { ComposerEditorHandle, ShortcodeRange, SuggestionPosition } from "./composer";

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

  let composerEditor = $state<ComposerEditorHandle | null>(null);
  let cursorPosition = $state(0);
  let draftShortcodeSuggestions = $state<EmojiShortcodeSuggestion[]>([]);
  let draftShortcodeRange = $state<ShortcodeRange | null>(null);
  let suggestionPopupPosition = $state<SuggestionPosition | null>(null);

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

  function handleSubmit(event: SubmitEvent) {
    event.preventDefault();
    if (!isSending && draft.trim().length > 0) {
      onSubmit?.(draft);
      clearDraft();
    }
  }

  function clearDraft() {
    composerEditor?.clearDraft();
    draftShortcodeSuggestions = [];
    draftShortcodeRange = null;
    suggestionPopupPosition = null;
  }

  function insertEmoji(emoji: string) {
    composerEditor?.insertAtCursor(emoji);
  }

  function applyShortcodeSuggestion(suggestion: EmojiShortcodeSuggestion) {
    const range = draftShortcodeRange;
    if (!range) {
      return;
    }

    draftShortcodeSuggestions = [];
    draftShortcodeRange = null;
    suggestionPopupPosition = null;

    composerEditor?.replaceRange(range.start, range.end, suggestion.replacement);
  }

  function handleEditorDraftChange(nextDraft: string) {
    draft = nextDraft;
    onDraftChange?.(nextDraft);
  }

  function handleEditorSubmit(nextDraft: string) {
    onSubmit?.(nextDraft);
    draftShortcodeSuggestions = [];
    draftShortcodeRange = null;
    suggestionPopupPosition = null;
  }
</script>

<form
  class="card p-3 mt-3 preset-outlined-surface-200-800 bg-surface-100-900 relative"
  onsubmit={handleSubmit}
>
  <ComposerErrorBanner {error} />

  <ComposerToolbar
    {isDisabled}
    {isSending}
    {pickerCustomEmoji}
    onInsertEmoji={insertEmoji}
  />

  <div id="message-draft">
    <ComposerEditor
      bind:this={composerEditor}
      {draft}
      {placeholder}
      {isDisabled}
      {isSending}
      {pickerCustomEmoji}
      shortcodeRange={draftShortcodeRange}
      shortcodeSuggestionCount={draftShortcodeSuggestions.length}
      onDraftChange={handleEditorDraftChange}
      onCursorChange={(position) => {
        cursorPosition = position;
      }}
      onSubmit={handleEditorSubmit}
      onSuggestionPositionChange={(position) => {
        suggestionPopupPosition = position;
      }}
    />
  </div>

  {#if draftShortcodeSuggestions.length > 0 && suggestionPopupPosition}
    <ShortcodeSuggestions
      suggestions={draftShortcodeSuggestions}
      position={suggestionPopupPosition}
      disabled={isDisabled || isSending}
      onApply={applyShortcodeSuggestion}
    />
  {/if}

  <ComposerActions {isSending} {isDisabled} {draft} />
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
