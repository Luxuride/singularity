<script lang="ts">
  import { streamStatusLabel } from "../shared";

  interface Props {
    roomName: string;
    roomEncrypted: boolean;
    loadingMessages: boolean;
    activeLoadKind: string | null;
    streamMessageCount: number;
    nextFrom: string | null;
    onLoadOlder?: () => void;
  }

  let {
    roomName,
    roomEncrypted,
    loadingMessages,
    activeLoadKind,
    streamMessageCount,
    nextFrom,
    onLoadOlder,
  }: Props = $props();
</script>

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
