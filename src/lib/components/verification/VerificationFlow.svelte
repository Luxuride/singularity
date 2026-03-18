<script lang="ts">
  import type { MatrixVerificationFlowResponse } from "$lib/chats/types";
  import { flowStateLabel } from "./helpers";

  interface Props {
    flow: MatrixVerificationFlowResponse | null;
    pending: boolean;
    error: string;
    onRefresh?: () => void;
    onAcceptRequest?: () => void;
    onStartSas?: () => void;
    onAcceptSas?: () => void;
    onConfirmSas?: () => void;
  }

  let {
    flow,
    pending,
    error,
    onRefresh,
    onAcceptRequest,
    onStartSas,
    onAcceptSas,
    onConfirmSas,
  }: Props = $props();
</script>

{#if flow}
  <section class="card p-3 preset-outlined-surface-300-700 bg-surface-100-900 space-y-3">
    <div class="flex items-center justify-between gap-2">
      <div>
        <p class="text-xs font-bold uppercase tracking-[0.18em] text-surface-600-400">Active Flow</p>
        <p class="text-sm font-medium">{flowStateLabel(flow)}</p>
      </div>
      <button
        type="button"
        class="btn preset-tonal text-xs"
        onclick={onRefresh}
        disabled={pending}
      >
        Refresh Flow
      </button>
    </div>

    <p class="text-xs font-mono break-all text-surface-600-400">{flow.flowId}</p>

    {#if error}
      <p class="text-xs text-warning-700-300">{error}</p>
    {/if}

    {#if flow.message}
      <p class="text-xs text-warning-700-300">{flow.message}</p>
    {/if}

    {#if flow.decimals}
      <p class="text-sm font-mono tracking-[0.2em]">
        {flow.decimals[0]} {flow.decimals[1]} {flow.decimals[2]}
      </p>
    {/if}

    {#if flow.emojis.length > 0}
      <div class="grid grid-cols-4 gap-2 text-center sm:grid-cols-7">
        {#each flow.emojis as emoji (`${emoji.symbol}-${emoji.description}`)}
          <div class="rounded bg-surface-200-800 p-2">
            <p class="text-lg">{emoji.symbol}</p>
            <p class="text-[10px] leading-tight">{emoji.description}</p>
          </div>
        {/each}
      </div>
    {/if}

    <div class="flex flex-wrap gap-2">
      {#if flow.canAcceptRequest}
        <button
          type="button"
          class="btn preset-tonal text-xs"
          onclick={onAcceptRequest}
          disabled={pending}
        >
          Accept Request
        </button>
      {/if}

      {#if flow.canStartSas}
        <button
          type="button"
          class="btn preset-tonal text-xs"
          onclick={onStartSas}
          disabled={pending}
        >
          Start SAS
        </button>
      {/if}

      {#if flow.canAcceptSas}
        <button
          type="button"
          class="btn preset-tonal text-xs"
          onclick={onAcceptSas}
          disabled={pending}
        >
          Accept SAS
        </button>
      {/if}

      {#if flow.canConfirmSas}
        <button
          type="button"
          class="btn preset-filled-primary text-xs"
          onclick={onConfirmSas}
          disabled={pending}
        >
          Confirm Match
        </button>
      {/if}
    </div>
  </section>
{/if}
