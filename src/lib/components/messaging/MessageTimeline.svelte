<script lang="ts">
  import type { MatrixChatMessage } from "$lib/chats/types";
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
    roomEncrypted: boolean;
    roomName: string;
    loadingMessages: boolean;
    activeLoadKind: string | null;
    streamMessageCount: number;
    error: string;
    nextFrom: string | null;
    isSending: boolean;
    onScroll?: (event: Event) => void;
    onLoadOlder?: () => void;
    onRetryMessage?: (message: TimelineMessage) => void;
  }

  let { 
    messages, 
    roomId, 
    selectedRoomId, 
    roomEncrypted, 
    roomName, 
    loadingMessages, 
    activeLoadKind, 
    streamMessageCount, 
    error,
    nextFrom,
    isSending,
    onScroll,
    onLoadOlder,
    onRetryMessage 
  }: Props = $props();

  let timelineElement: HTMLElement | null = $state(null);
</script>

<section
  class="card preset-outlined-surface-200-800 bg-surface-100-900 space-y-3 max-h-[70vh] flex flex-col flex-grow"
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
      class="flex-grow overflow-y-auto px-4 pb-3"
      bind:this={timelineElement}
      onscroll={onScroll}
    >
      {#if messages.length === 0}
        <p class="text-sm text-surface-700-300">No messages yet.</p>
      {:else}
        <ul class="space-y-2">
          {#each messages as message, index (`${message.eventId ?? index}-${message.timestamp ?? 0}`)}
            <MessageItem
              {message}
              onRetry={onRetryMessage}
              isSending={isSending}
            />
          {/each}
        </ul>
      {/if}
    </div>
  {/if}
</section>
