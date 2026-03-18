<script lang="ts">
  interface Props {
    userId: string;
    recoveryStateLabel: string;
    isLoading: boolean;
    isRefreshing: boolean;
    isLoggingOut: boolean;
    onRefresh?: () => void;
    onLogout?: () => void;
  }

  let { userId, recoveryStateLabel, isLoading, isRefreshing, isLoggingOut, onRefresh, onLogout }: Props = $props();
</script>

<section class="card p-4 md:p-6 space-y-4 preset-outlined-surface-200-800 bg-surface-50-950">
  <header class="flex flex-wrap items-center justify-between gap-2">
    <div>
      <p class="text-xs font-bold uppercase tracking-[0.2em] text-primary-600-400">Singularity</p>
      <h1 class="h3">Chats</h1>
      <p class="text-xs text-surface-700-300">{userId}</p>
    </div>

    <div class="flex flex-wrap gap-2">
      <a class="btn preset-tonal text-xs" href="/settings/security">
        Verification / Recovery
      </a>
      <button
        type="button"
        class="btn preset-tonal"
        onclick={onRefresh}
        disabled={isLoading || isRefreshing}
      >
        Refresh
      </button>
      <button
        type="button"
        class="btn preset-filled-error-500"
        onclick={onLogout}
        disabled={isLoggingOut}
      >
        {#if isLoggingOut}Logging out...{:else}Logout{/if}
      </button>
    </div>
  </header>

  {#if recoveryStateLabel}
    <p class="text-xs text-surface-700-300">{recoveryStateLabel}</p>
  {/if}
</section>
