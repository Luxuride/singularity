<script lang="ts">
  import type { PickerCustomEmoji } from "$lib/emoji/picker";

  import MessageItem from "../message/MessageItem.svelte";
  import type { RetryMessageHandler, TimelineMessage, ToggleReactionHandler } from "../shared";

  interface Props {
    messages: TimelineMessage[];
    roomId: string;
    currentUserId: string | null;
    pickerCustomEmoji: PickerCustomEmoji[];
    isSending: boolean;
    onScroll?: (event: Event) => void;
    onRetryMessage?: RetryMessageHandler;
    onToggleReaction?: ToggleReactionHandler;
    onTimelineElementChange?: (element: HTMLElement | null) => void;
  }

  let {
    messages,
    roomId,
    currentUserId,
    pickerCustomEmoji,
    isSending,
    onScroll,
    onRetryMessage,
    onToggleReaction,
    onTimelineElementChange,
  }: Props = $props();

  let timelineElement: HTMLElement | null = $state(null);

  $effect(() => {
    onTimelineElementChange?.(timelineElement);
  });
</script>

<div
  class="timeline-scroll flex-1 overflow-y-auto px-4 pb-3 min-h-0"
  bind:this={timelineElement}
  onscroll={onScroll}
>
  {#if messages.length === 0}
    <p class="text-sm text-surface-700-300">No messages yet.</p>
  {:else}
    <ul class="space-y-2">
      {#each messages as message, index (`${message.eventId ?? message.localId ?? `${index}-${message.timestamp ?? 0}`}`)}
        <MessageItem
          {message}
          {roomId}
          onRetry={onRetryMessage}
          {currentUserId}
          {pickerCustomEmoji}
          onToggleReaction={onToggleReaction}
          isSending={isSending}
        />
      {/each}
    </ul>
  {/if}
</div>

<style>
  .timeline-scroll {
    overflow-anchor: auto;
  }
</style>
