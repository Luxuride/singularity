<script lang="ts">
  import { getShortcodeSuggestions } from "$lib/emoji/picker";
  import type { EmojiShortcodeSuggestion, PickerCustomEmoji } from "$lib/emoji/picker";
  import type { TimelineMessage } from "../shared";
  import { ShortcodeSuggestions } from "../composer-emoji";
  import { ComposerActions, ComposerEditor, ComposerErrorBanner, ComposerToolbar } from ".";
  import type { ComposerEditorHandle, ShortcodeRange, SuggestionPosition } from ".";

  interface Props {
    draft: string;
    error: string;
    isSending: boolean;
    isDisabled: boolean;
    placeholder?: string;
    pickerCustomEmoji?: PickerCustomEmoji[];
    replyToMessage?: TimelineMessage | null;
    focusNonce?: number;
    onSubmit?: (draft: string) => void;
    onDraftChange?: (draft: string) => void;
    onClearReply?: () => void;
    onChooseAttachment?: () => void;
    onPasteAttachmentPath?: (filePath: string) => void;
  }

  let {
    draft,
    error,
    isSending,
    isDisabled,
    placeholder = "Write a message...",
    pickerCustomEmoji = [],
    replyToMessage = null,
    focusNonce = 0,
    onSubmit,
    onDraftChange,
    onClearReply,
    onChooseAttachment,
    onPasteAttachmentPath,
  }: Props = $props();

  let composerEditor = $state<ComposerEditorHandle | null>(null);
  let cursorPosition = $state(0);
  let draftShortcodeSuggestions = $state<EmojiShortcodeSuggestion[]>([]);
  let draftShortcodeRange = $state<ShortcodeRange | null>(null);
  let suggestionPopupPosition = $state<SuggestionPosition | null>(null);
  let lastAppliedFocusNonce = $state(0);

  function stripMxReplyBlock(html: string): string {
    return html.replace(/<mx-reply>[\s\S]*?<\/mx-reply>/i, "").trimStart();
  }

  const replyTextPreview = $derived.by(() => {
    if (!replyToMessage) {
      return "";
    }

    return replyToMessage.body.trim().split("\n")[0]?.slice(0, 72) ?? "";
  });

  const replyFormattedPreview = $derived.by(() => {
    if (!replyToMessage?.formattedBody || replyToMessage.messageType === "m.image") {
      return null;
    }

    return stripMxReplyBlock(replyToMessage.formattedBody);
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

  $effect(() => {
    if (focusNonce === lastAppliedFocusNonce) {
      return;
    }

    lastAppliedFocusNonce = focusNonce;
    composerEditor?.focusEditor();
  });
</script>

<form
  class="card p-3 mt-3 preset-outlined-surface-200-800 bg-surface-100-900 relative"
  onsubmit={handleSubmit}
>
  <ComposerErrorBanner {error} />

  {#if replyToMessage}
    <div class="mb-2 flex items-start justify-between gap-2 rounded border border-surface-300-700 bg-surface-100-900 px-3 py-2">
      <div class="min-w-0">
        <p class="text-xs font-semibold uppercase tracking-wide text-surface-600-400">
          Replying to {replyToMessage.sender}
        </p>

        {#if replyToMessage.messageType === "m.image" && replyToMessage.imageUrl}
          <div class="mt-1 flex items-center gap-2 text-sm text-surface-800-200">
            <img
              src={replyToMessage.imageUrl}
              alt={replyToMessage.body || "Reply image"}
              loading="lazy"
              class="h-8 w-8 rounded border border-surface-300-700 object-cover bg-surface-100-900"
            />
            <span class="truncate">Image</span>
          </div>
        {:else if replyFormattedPreview}
          <div class="reply-formatted-preview mt-1 text-sm text-surface-800-200">
            {@html replyFormattedPreview}
          </div>
        {:else if replyTextPreview}
          <p class="mt-1 truncate text-sm text-surface-800-200">{replyTextPreview}</p>
        {/if}
      </div>
      <button
        type="button"
        class="shrink-0 rounded px-2 py-1 text-xs font-medium text-surface-700-300 hover:bg-surface-200-800 hover:text-surface-900-100"
        onclick={() => onClearReply?.()}
      >
        Clear
      </button>
    </div>
  {/if}

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
      onPasteAttachmentPath={onPasteAttachmentPath}
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

  <ComposerActions {isSending} {isDisabled} {draft} {onChooseAttachment} />
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

  .reply-formatted-preview {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .reply-formatted-preview :global(p) {
    display: inline;
    margin: 0;
  }

  .reply-formatted-preview :global(img[data-mx-emoticon]),
  .reply-formatted-preview :global(img[data_mx_emoticon]) {
    display: inline-block;
    vertical-align: text-bottom;
    width: 1.05rem;
    height: 1.05rem;
    max-width: 100%;
    max-height: 1.35em;
    margin-inline: 0.02rem;
  }
</style>
