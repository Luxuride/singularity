<script lang="ts">
  import type { MatrixRecoveryState } from "$lib/auth/types";
  import { recoveryStateLabel } from "$lib/components/verification/helpers";

  interface Props {
    recoveryState: MatrixRecoveryState | null;
    pending: boolean;
    message: string;
    onImport?: (key: string) => void;
  }

  let { recoveryState, pending, message, onImport }: Props = $props();

  let recoveryKey = $state("");

  const handleImport = () => {
    if (!recoveryKey.trim()) {
      return;
    }
    onImport?.(recoveryKey);
  };
</script>

<section class="card p-3 preset-outlined-surface-300-700 bg-surface-50-950 space-y-2">
  <label class="label" for="recovery-key">Recovery Key</label>
  <textarea
    id="recovery-key"
    class="textarea"
    bind:value={recoveryKey}
    rows="3"
    placeholder="EsTc ..."
  ></textarea>
  <button
    type="button"
    class="btn preset-filled-primary-500 text-xs"
    onclick={handleImport}
    disabled={pending || !recoveryKey.trim()}
  >
    {#if pending}Recovering...{:else}Import Recovery Key{/if}
  </button>
  {#if message}
    <p class="text-sm {message.includes('completed') ? 'text-success-700-300' : 'text-warning-700-300'}">
      {message}
    </p>
  {/if}
</section>
