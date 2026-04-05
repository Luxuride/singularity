<script lang="ts">
  import type { PickerCustomEmoji } from "$lib/emoji/picker";

  import EmojiPickerPopover from "../composer-emoji/EmojiPickerPopover.svelte";
  import {
    buildEmojiByShortcodeToken,
    buildPickerEmojiBySourceUrl,
    customEmojiUrlForToken,
    reactionDisplayName,
  } from "./helpers";
  import type { TimelineMessage, ToggleReactionHandler } from "../shared";

  interface Props {
    message: TimelineMessage;
    currentUserId?: string | null;
    pickerCustomEmoji?: PickerCustomEmoji[];
    onToggleReaction?: ToggleReactionHandler;
  }

  let {
    message,
    currentUserId = null,
    pickerCustomEmoji = [],
    onToggleReaction,
  }: Props = $props();

  const pickerEmojiBySourceUrl = $derived.by(() => buildPickerEmojiBySourceUrl(pickerCustomEmoji));
  const emojiByShortcodeToken = $derived.by(() => buildEmojiByShortcodeToken(message, pickerCustomEmoji));

  function ownReaction(reaction: { senders: string[] }): boolean {
    return Boolean(currentUserId && reaction.senders.includes(currentUserId));
  }

  function handleReactionClick(key: string) {
    if (!message.eventId) {
      return;
    }

    onToggleReaction?.(message, key);
  }
</script>

{#if message.eventId}
  <div class="mt-2 flex flex-wrap items-center gap-1">
    {#each message.reactions as reaction (`${reaction.key}-${reaction.senders.join("|")}`)}
      {@const reactionEmojiUrl = customEmojiUrlForToken(reaction.key, emojiByShortcodeToken, pickerEmojiBySourceUrl)}
      <button
        type="button"
        class="rounded-full px-2 py-0.5 text-xs border"
        title={reactionDisplayName(reaction.key, message, pickerCustomEmoji)}
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
            title={reactionDisplayName(reaction.key, message, pickerCustomEmoji)}
            class="inline-block h-5 w-5 align-text-bottom"
            loading="lazy"
          />
        {:else}
          <span class="inline-block text-2xl leading-none align-text-bottom" title={reactionDisplayName(reaction.key, message, pickerCustomEmoji)}>{reaction.key}</span>
        {/if}
        {reaction.count}
      </button>
    {/each}

    <EmojiPickerPopover
      buttonText="+"
      buttonClass="rounded-full px-2 py-0.5 text-xs border border-surface-400-600 bg-surface-100-900"
      popoverClass="absolute bottom-full left-0 mb-1 z-10 card p-2 preset-outlined-surface-300-700 bg-surface-50-950 w-[22rem] max-w-[80vw]"
      pickerStyle="width: 100%; height: 22rem; --emoji-size: 2rem; --category-emoji-size: 1.15rem;"
      {pickerCustomEmoji}
      onSelect={handleReactionClick}
    />
  </div>
{/if}
