<script lang="ts">
  import type { MatrixChatMessage } from "$lib/chats/types";
  import type { PickerCustomEmoji } from "$lib/emoji/picker";
  import MessageItem from "./MessageItem.svelte";
  import { streamStatusLabel } from "./helpers";

  interface TimelineMessage extends MatrixChatMessage {
    localId?: string;
    sendState?: "sending" | "failed";
  }

  interface Props {
    messages: TimelineMessage[];
    roomId: string;
    selectedRoomId: string | null;
    currentUserId: string | null;
    pickerCustomEmoji: PickerCustomEmoji[];
    roomEncrypted: boolean;
    roomName: string;
    loadingMessages: boolean;
    activeLoadKind: string | null;
    streamMessageCount: number;
    error: string;
    nextFrom: string | null;
    isSending: boolean;
    onTimelineElementChange?: (element: HTMLElement | null) => void;
    onScroll?: (event: Event) => void;
    onLoadOlder?: () => void;
    onRetryMessage?: (message: TimelineMessage) => void;
    onToggleReaction?: (message: TimelineMessage, key: string) => void;
  }

  let { 
    messages, 
    roomId, 
    selectedRoomId, 
    currentUserId,
    pickerCustomEmoji,
    roomEncrypted, 
    roomName, 
    loadingMessages, 
    activeLoadKind, 
    streamMessageCount, 
    error,
    nextFrom,
    isSending,
    onTimelineElementChange,
    onScroll,
    onLoadOlder,
    onRetryMessage,
    onToggleReaction
  }: Props = $props();

  let timelineElement: HTMLElement | null = $state(null);

  $effect(() => {
    onTimelineElementChange?.(timelineElement);
  });
</script>

<section
  class="card preset-outlined-surface-200-800 bg-surface-100-900 flex flex-col flex-grow min-h-0 gap-3"
>
  {#if error}
    <p class="card p-3 text-sm preset-filled-error-500">{error}</p>
  {/if}

  {#if !selectedRoomId}
    <p class="text-sm text-surface-700-300">Select a room to read messages.</p>
  {:else}
    <header class="flex items-center justify-between gap-2 px-4 pt-3">
      <div>
        <h2 class="h5">{roomName}</h2>
        <p class="text-xs text-surface-700-300">
          {roomEncrypted ? "Encrypted room" : "Unencrypted room"}
        </p>
        {#if loadingMessages && activeLoadKind}
          <p class="text-xs text-primary-700-300">{streamStatusLabel(loadingMessages, activeLoadKind, streamMessageCount)}</p>
        {/if}
      </div>

      <button
        type="button"
        class="btn preset-tonal"
        onclick={onLoadOlder}
        disabled={!nextFrom || loadingMessages}
      >
        {#if loadingMessages}Loading...{:else}Load Older{/if}
      </button>
    </header>

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
  {/if}
</section>

<style>
  .timeline-scroll {
    overflow-anchor: auto;
  }
</style>
