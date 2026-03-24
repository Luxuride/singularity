<script lang="ts">
  import { onMount } from "svelte";

  import type { MatrixChatMessage } from "$lib/chats/types";
  import {
    applyCustomEmojiConfig,
    ensureEmojiPickerLoaded,
    selectedEmojiToken,
  } from "$lib/emoji/picker";
  import type { PickerCustomEmoji } from "$lib/emoji/picker";
  import { toTime, decryptionLabel, verificationLabel } from "./helpers";

  type EmojiClickDetail = {
    unicode?: string;
    name?: string;
    emoji?: {
      shortcodes?: string[];
      name?: string;
    };
  };

  type MessageBodyPart =
    | { type: "text"; value: string }
    | { type: "emoji"; shortcode: string; url: string };

  interface TimelineMessage extends MatrixChatMessage {
    localId?: string;
    sendState?: "sending" | "failed";
  }

  interface Props {
    message: TimelineMessage;
    onRetry?: (message: TimelineMessage) => void;
    onToggleReaction?: (message: TimelineMessage, key: string) => void;
    currentUserId?: string | null;
    pickerCustomEmoji?: PickerCustomEmoji[];
    isSending?: boolean;
  }

  let {
    message,
    onRetry,
    onToggleReaction,
    currentUserId = null,
    pickerCustomEmoji = [],
    isSending = false,
  }: Props = $props();
  let showReactionPicker = $state(false);
  let reactionPickerElement = $state<HTMLElement | null>(null);
  let reactionPickerPopoverElement = $state<HTMLElement | null>(null);

  onMount(() => {
    void ensureEmojiPickerLoaded();
  });

  $effect(() => {
    if (!showReactionPicker || !reactionPickerElement) {
      return;
    }

    applyCustomEmojiConfig(reactionPickerElement as never, pickerCustomEmoji);
  });

  $effect(() => {
    if (!showReactionPicker) {
      return;
    }

    const handleOutsidePointerDown = (event: PointerEvent) => {
      const target = event.target;
      if (!(target instanceof Node)) {
        return;
      }

      if (reactionPickerPopoverElement?.contains(target)) {
        return;
      }

      showReactionPicker = false;
    };

    window.addEventListener("pointerdown", handleOutsidePointerDown, true);

    return () => {
      window.removeEventListener("pointerdown", handleOutsidePointerDown, true);
    };
  });

  const CUSTOM_EMOJI_PATTERN = /(:[A-Za-z0-9_+\-]+:)/g;
  const SINGLE_UNICODE_EMOJI_PATTERN =
    /^\s*(?:\p{Regional_Indicator}{2}|(?:\p{Extended_Pictographic}|\p{Emoji_Presentation})(?:\p{Emoji_Modifier})?(?:\uFE0F|\uFE0E)?(?:\u200D(?:\p{Extended_Pictographic}|\p{Emoji_Presentation})(?:\p{Emoji_Modifier})?(?:\uFE0F|\uFE0E)?)*)\s*$/u;

  function shortcodeToken(value: string): string {
    const clean = value.trim().replace(/^:+|:+$/g, "");
    return clean ? `:${clean}:` : "";
  }

  const pickerEmojiBySourceUrl = $derived.by(() => {
    const map = new Map<string, string>();
    for (const emoji of pickerCustomEmoji) {
      if (emoji.sourceUrl?.trim()) {
        map.set(emoji.sourceUrl.trim(), emoji.url);
      }
    }
    return map;
  });

  const emojiByShortcodeToken = $derived.by(() => {
    const map = new Map<string, string>();

    for (const emoji of message.customEmojis ?? []) {
      const token = shortcodeToken(emoji.shortcode);
      if (token && !map.has(token)) {
        map.set(token, emoji.url);
      }
    }

    for (const emoji of pickerCustomEmoji) {
      for (const shortcode of emoji.shortcodes ?? []) {
        const token = shortcodeToken(shortcode);
        if (token && !map.has(token)) {
          map.set(token, emoji.url);
        }
      }
    }

    return map;
  });

  function customEmojiUrlForToken(token: string): string | null {
    const trimmed = token.trim();
    if (!trimmed) {
      return null;
    }

    if (trimmed.startsWith("mxc://") || trimmed.startsWith("http://") || trimmed.startsWith("https://")) {
      const bySource = pickerEmojiBySourceUrl.get(trimmed);
      if (bySource) {
        return bySource;
      }
    }

    const normalizedToken = shortcodeToken(trimmed);
    if (!normalizedToken) {
      return null;
    }

    return emojiByShortcodeToken.get(normalizedToken) ?? null;
  }

  function buildMessageBodyParts(message: TimelineMessage): MessageBodyPart[] {
    if (!message.customEmojis?.length && pickerCustomEmoji.length === 0) {
      return [{ type: "text", value: message.body }];
    }

    const segments = message.body.split(CUSTOM_EMOJI_PATTERN);
    const parts: MessageBodyPart[] = [];

    for (const segment of segments) {
      if (!segment) {
        continue;
      }

      const emojiUrl = emojiByShortcodeToken.get(segment);
      if (emojiUrl) {
        parts.push({ type: "emoji", shortcode: segment, url: emojiUrl });
        continue;
      }

      parts.push({ type: "text", value: segment });
    }

    return parts.length ? parts : [{ type: "text", value: message.body }];
  }

  function isEmojiOnlyMessage(parts: MessageBodyPart[]): boolean {
    const emojiParts = parts.filter((part) => part.type === "emoji");
    const hasOnlyWhitespaceText = parts.every(
      (part) => part.type === "emoji" || part.value.trim().length === 0,
    );

    if (emojiParts.length === 1 && hasOnlyWhitespaceText) {
      return true;
    }

    if (parts.length === 1 && parts[0].type === "text") {
      return SINGLE_UNICODE_EMOJI_PATTERN.test(parts[0].value);
    }

    return false;
  }

  const messageBodyParts = $derived.by(() => buildMessageBodyParts(message));
  const emojiOnlyBody = $derived.by(() => isEmojiOnlyMessage(messageBodyParts));

  function ownReaction(reaction: { senders: string[] }): boolean {
    return Boolean(currentUserId && reaction.senders.includes(currentUserId));
  }

  function handleReactionClick(key: string) {
    if (!message.eventId) {
      return;
    }

    onToggleReaction?.(message, key);
    showReactionPicker = false;
  }

  function handleReactionEmojiClick(event: CustomEvent<EmojiClickDetail>) {
    const selected = selectedEmojiToken(event.detail);
    if (!selected) {
      return;
    }

    handleReactionClick(selected);
  }
</script>

<li class="card p-3 preset-outlined-surface-300-700 bg-surface-50-950" data-message-event-id={message.eventId ?? undefined}>
  <div class="flex items-center justify-between gap-2 text-xs text-surface-700-300 mb-1">
    <span>{message.sender}</span>
    <span>{toTime(message.timestamp)}</span>
  </div>
  {#if message.messageType === "m.image"}
    <figure class="space-y-2">
      <img
        src={message.imageUrl}
        alt={message.body || "Image"}
        loading="lazy"
        class="max-h-[28rem] w-full rounded preset-outlined-surface-300-700 object-contain bg-surface-100-900"
      />
      {#if message.body}
        <figcaption class="text-base whitespace-pre-wrap break-words text-surface-700-300">
          {message.body}
        </figcaption>
      {/if}
    </figure>
  {:else}
    <p class={`whitespace-pre-wrap break-words ${emojiOnlyBody ? "text-5xl leading-tight" : "text-base"}`}>
      {#each messageBodyParts as part, index (`${part.type}-${index}`)}
        {#if part.type === "emoji"}
          <img
            src={part.url}
            alt={part.shortcode}
            class={emojiOnlyBody
              ? "inline-block h-20 w-20 align-text-bottom mx-0.5"
              : "inline-block h-8 w-8 align-text-bottom mx-0.5"}
            loading="lazy"
          />
        {:else}
          {part.value}
        {/if}
      {/each}
    </p>
  {/if}
  {#if message.eventId}
    <div class="mt-2 flex flex-wrap items-center gap-1">
      {#each message.reactions as reaction (`${reaction.key}-${reaction.senders.join("|")}`)}
        {@const reactionEmojiUrl = customEmojiUrlForToken(reaction.key)}
        <button
          type="button"
          class="rounded-full px-2 py-0.5 text-xs border"
          class:bg-primary-200-800={ownReaction(reaction)}
          class:text-primary-900-100={ownReaction(reaction)}
          class:bg-surface-200-800={!ownReaction(reaction)}
          class:text-surface-900-100={!ownReaction(reaction)}
          class:border-primary-500={ownReaction(reaction)}
          class:border-surface-400-600={!ownReaction(reaction)}
          onclick={() => handleReactionClick(reaction.key)}
        >
          {#if reactionEmojiUrl}
            <img
              src={reactionEmojiUrl}
              alt={reaction.key}
              class="inline-block h-5 w-5 align-text-bottom"
              loading="lazy"
            />
          {:else}
            <span class="inline-block text-2xl leading-none align-text-bottom">{reaction.key}</span>
          {/if}
          {reaction.count}
        </button>
      {/each}

      <div class="relative" bind:this={reactionPickerPopoverElement}>
        <button type="button" class="rounded-full px-2 py-0.5 text-xs border border-surface-400-600 bg-surface-100-900" onclick={() => showReactionPicker = !showReactionPicker}>
          +
        </button>
        {#if showReactionPicker}
          <div class="absolute bottom-full left-0 mb-1 z-10 card p-2 preset-outlined-surface-300-700 bg-surface-50-950 w-[22rem] max-w-[80vw]">
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <emoji-picker
              bind:this={reactionPickerElement}
              style="width: 100%; height: 22rem; --emoji-size: 2rem; --category-emoji-size: 1.15rem;"
              onemoji-click={handleReactionEmojiClick}
            ></emoji-picker>
          </div>
        {/if}
      </div>
    </div>
  {/if}
  {#if message.sendState}
    <div class="mt-2 flex items-center gap-2 text-xs">
      {#if message.sendState === "sending"}
        <span class="rounded px-2 py-0.5 bg-primary-200-800 text-primary-900-100">Sending...</span>
      {:else}
        <span class="rounded px-2 py-0.5 bg-error-200-800 text-error-900-100">Failed to send</span>
        <button
          type="button"
          class="btn btn-xs preset-tonal"
          onclick={() => onRetry?.(message)}
          disabled={isSending}
        >
          Retry
        </button>
      {/if}
    </div>
  {/if}
  {#if message.encrypted}
    <div class="mt-1 flex flex-wrap items-center gap-2 text-xs">
      <span class="rounded px-2 py-0.5 bg-surface-200-800">{decryptionLabel(message.decryptionStatus)}</span>
      <span
        class="rounded px-2 py-0.5"
        class:bg-success-200-800={message.verificationStatus === "verified"}
        class:bg-warning-200-800={message.verificationStatus === "unverified"}
        class:bg-surface-200-800={message.verificationStatus === "unknown"}
      >
        {verificationLabel(message.verificationStatus)}
      </span>
    </div>
  {/if}
</li>
