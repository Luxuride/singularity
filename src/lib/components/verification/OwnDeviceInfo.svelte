<script lang="ts">
  import type { MatrixOwnVerificationStatus } from "$lib/chats/types";

  interface Props {
    ownVerification: MatrixOwnVerificationStatus | null;
    loading: boolean;
  }

  let { ownVerification, loading }: Props = $props();
</script>

{#if loading}
  <p class="text-sm text-surface-700-300">Loading device info...</p>
{:else if ownVerification}
  <div class="text-xs p-2 rounded bg-surface-100-900 space-y-0.5">
    <p>Own device: <span class="font-mono">{ownVerification.deviceId}</span></p>
    <p>
      Status:
      <span
        class="rounded px-1.5 py-0.5 ml-1"
        class:bg-success-200-800={ownVerification.deviceVerified}
        class:bg-warning-200-800={!ownVerification.deviceVerified}
      >
        {ownVerification.deviceVerified ? "Verified" : "Unverified"}
      </span>
      {#if !ownVerification.crossSigningSetup}
        <span class="ml-1 text-surface-500-500">(Cross-signing not set up)</span>
      {/if}
    </p>
  </div>
{/if}
