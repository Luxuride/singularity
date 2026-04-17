<script lang="ts">
  import type { PickerCustomEmoji } from "$lib/emoji/picker";

  import MessageItem from "../message/MessageItem.svelte";
  import type {
    ReplyToMessageHandler,
    RetryMessageHandler,
    TimelineMessage,
    ToggleReactionHandler,
  } from "../shared";

  interface Props {
    messages: TimelineMessage[];
    roomId: string;
    currentUserId: string | null;
    pickerCustomEmoji: PickerCustomEmoji[];
    isSending: boolean;
    onScroll?: (event: Event) => void;
    onRetryMessage?: RetryMessageHandler;
    onToggleReaction?: ToggleReactionHandler;
    onReplyToMessage?: ReplyToMessageHandler;
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
    onReplyToMessage,
    onTimelineElementChange,
  }: Props = $props();

  let timelineElement: HTMLElement | null = $state(null);
  let highlightedMessageElement: HTMLElement | null = $state(null);
  let highlightTimeout: ReturnType<typeof setTimeout> | null = $state(null);
  let ignoreNextGlobalPointerDown = $state(false);

  const messagesByEventId = $derived.by(() => {
    const indexed = new Map<string, TimelineMessage>();
    for (const message of messages) {
      if (!message.eventId) {
        continue;
      }

      indexed.set(message.eventId, message);
    }

    return indexed;
  });

  function jumpToMessage(eventId: string) {
    if (!timelineElement) {
      return;
    }

    const target = timelineElement.querySelector<HTMLElement>(
      `[data-message-event-id="${eventId.replaceAll('"', '\\"')}"]`,
    );

    if (!target) {
      return;
    }

    target.scrollIntoView({ behavior: "smooth", block: "center" });

    if (highlightedMessageElement && highlightedMessageElement !== target) {
      highlightedMessageElement.classList.remove("bg-surface-100-900");
      highlightedMessageElement.classList.remove("border-primary-500");
    }

    target.classList.add("bg-surface-100-900");
    target.classList.add("border-primary-500");
    highlightedMessageElement = target;
    ignoreNextGlobalPointerDown = true;
    queueMicrotask(() => {
      ignoreNextGlobalPointerDown = false;
    });

    if (highlightTimeout) {
      clearTimeout(highlightTimeout);
    }

    highlightTimeout = setTimeout(() => {
      highlightedMessageElement?.classList.remove("border-primary-500");
      highlightTimeout = null;
    }, 750);
  }

  $effect(() => {
    onTimelineElementChange?.(timelineElement);
  });

  $effect(() => {
    const onGlobalPointerDown = () => {
      if (ignoreNextGlobalPointerDown) {
        return;
      }

      highlightedMessageElement?.classList.remove("bg-surface-100-900");
      highlightedMessageElement = null;
    };

    window.addEventListener("pointerdown", onGlobalPointerDown, true);

    return () => {
      window.removeEventListener("pointerdown", onGlobalPointerDown, true);
    };
  });

  $effect(() => {
    return () => {
      if (highlightTimeout) {
        clearTimeout(highlightTimeout);
      }
    };
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
          repliedMessage={
            message.inReplyToEventId ? messagesByEventId.get(message.inReplyToEventId) : undefined
          }
          onJumpToMessage={jumpToMessage}
          onRetry={onRetryMessage}
          {currentUserId}
          {pickerCustomEmoji}
          onToggleReaction={onToggleReaction}
          onReplyToMessage={onReplyToMessage}
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
