<script lang="ts">
  import type { MatrixChatMessage } from "$lib/chats/types";
  import { toTime, decryptionLabel, verificationLabel } from "./helpers";

  interface TimelineMessage extends MatrixChatMessage {
    localId?: string;
    sendState?: "sending" | "failed";
  }

  interface Props {
    message: TimelineMessage;
    onRetry?: (message: TimelineMessage) => void;
    isSending?: boolean;
  }

  let { message, onRetry, isSending = false }: Props = $props();
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
        <figcaption class="text-sm whitespace-pre-wrap break-words text-surface-700-300">
          {message.body}
        </figcaption>
      {/if}
    </figure>
  {:else}
    <p class="text-sm whitespace-pre-wrap break-words">{message.body}</p>
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
