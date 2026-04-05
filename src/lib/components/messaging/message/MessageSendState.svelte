<script lang="ts">
  import type { RetryMessageHandler, TimelineMessage } from "../shared";

  interface Props {
    message: TimelineMessage;
    isSending?: boolean;
    onRetry?: RetryMessageHandler;
  }

  let {
    message,
    isSending = false,
    onRetry,
  }: Props = $props();
</script>

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
