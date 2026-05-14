<script lang="ts">
  import type { MatrixChatSummary } from "$lib/chats/types";

  interface Props {
    open: boolean;
    room: MatrixChatSummary | null;
    leaveAllCount?: number;
    leaving?: boolean;
    onClose: () => void;
    onLeave: () => void;
    onLeaveAll: () => void;
  }

  let {
    open,
    room,
    leaveAllCount = 0,
    leaving = false,
    onClose,
    onLeave,
    onLeaveAll,
  }: Props = $props();

  let dialogElement: HTMLDialogElement | undefined = $state();
  let confirmElement: HTMLDialogElement | undefined = $state();
  let confirmOpen = $state(false);
  let confirmMode = $state<"space" | "all">("space");

  const spaceName = $derived(room?.displayName ?? "Unknown");
  const spaceId = $derived(room?.roomId ?? "Unknown");

  $effect(() => {
    if (open && dialogElement) {
      if (!dialogElement.open) {
        dialogElement.showModal();
      }
    } else if (!open && dialogElement && dialogElement.open) {
      dialogElement.close();
    }
  });

  $effect(() => {
    if (confirmOpen && confirmElement) {
      if (!confirmElement.open) {
        confirmElement.showModal();
      }
    } else if (!confirmOpen && confirmElement && confirmElement.open) {
      confirmElement.close();
    }
  });

  function handleClose() {
    onClose();
  }

  function openLeaveConfirm() {
    confirmMode = "space";
    confirmOpen = true;
  }

  function openLeaveAllConfirm() {
    confirmMode = "all";
    confirmOpen = true;
  }

  function closeLeaveConfirm() {
    confirmOpen = false;
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<dialog
  bind:this={dialogElement}
  onclose={handleClose}
  class="backdrop:bg-surface-900/50 card preset-outlined-surface-200-800 bg-surface-100-900 m-auto rounded-xl p-4 shadow-xl min-w-[320px]"
  onmousedown={(e) => {
      if (e.target === dialogElement) onClose();
  }}
>
  <div class="flex flex-col gap-4">
    <header class="space-y-1">
      <p class="text-xs uppercase tracking-wide text-surface-700-300">Space settings</p>
      <h2 class="text-lg font-semibold text-surface-900-50">{spaceName}</h2>
      <p class="text-xs text-surface-700-300 break-all">{spaceId}</p>
    </header>

    <div class="grid gap-4 md:grid-cols-[140px_1fr]">
      <nav class="rounded-lg border border-surface-200-800 bg-surface-50-950 p-2">
        <button
          type="button"
          class="w-full rounded px-3 py-2 text-left text-sm bg-surface-200-800 text-surface-900-50"
          disabled
        >
          General
        </button>
      </nav>

      <section class="rounded-lg border border-surface-200-800 bg-surface-50-950 p-3 space-y-3">
        <div>
          <p class="text-sm font-semibold text-surface-900-50">General</p>
          <p class="text-xs text-surface-700-300">Basic space settings and membership.</p>
        </div>

        <div class="flex items-center justify-between gap-3 rounded border border-surface-300-700 bg-surface-100-900 p-3">
          <div>
            <p class="text-sm text-surface-900-50">Leave space</p>
            <p class="text-xs text-surface-700-300">
              Leave this space and remove it from your navigation.
            </p>
          </div>
          <button
            type="button"
            class="btn preset-filled-error-500 px-3 py-2 text-xs opacity-90 hover:opacity-100 disabled:opacity-50"
            onclick={openLeaveConfirm}
            disabled={!room}
          >
            Leave
          </button>
        </div>

        <div class="flex items-center justify-between gap-3 rounded border border-surface-300-700 bg-surface-100-900 p-3">
          <div>
            <p class="text-sm text-surface-900-50">Leave all rooms</p>
            <p class="text-xs text-surface-700-300">
              Leave this space and {leaveAllCount} joined room{leaveAllCount === 1 ? "" : "s"}.
            </p>
          </div>
          <button
            type="button"
            class="btn preset-filled-error-500 px-3 py-2 text-xs opacity-90 hover:opacity-100 disabled:opacity-50"
            onclick={openLeaveAllConfirm}
            disabled={!room || leaveAllCount === 0}
          >
            Leave all
          </button>
        </div>
      </section>
    </div>

    <div class="flex justify-end gap-2">
      <button
        type="button"
        class="btn preset-outlined-surface-200-800 px-4 py-2 hover:bg-surface-200-800"
        onclick={onClose}
      >
        Close
      </button>
    </div>
  </div>
</dialog>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<dialog
  bind:this={confirmElement}
  onclose={closeLeaveConfirm}
  class="backdrop:bg-surface-900/50 card preset-outlined-surface-200-800 bg-surface-100-900 m-auto rounded-xl p-4 shadow-xl min-w-[320px]"
  onmousedown={(e) => {
      if (e.target === confirmElement) closeLeaveConfirm();
  }}
>
  <div class="flex flex-col gap-4">
    <header class="space-y-1">
      <p class="text-xs uppercase tracking-wide text-surface-700-300">
        {confirmMode === "all" ? "Leave all" : "Leave space"}
      </p>
      <h2 class="text-lg font-semibold text-surface-900-50">{spaceName}</h2>
      <p class="text-xs text-surface-700-300 break-all">{spaceId}</p>
    </header>

    <div class="rounded-lg border border-surface-200-800 bg-surface-50-950 p-3 space-y-2">
      {#if confirmMode === "all"}
        <p class="text-sm text-surface-900-50">Leave this space and all joined rooms?</p>
        <p class="text-xs text-surface-700-300">
          This removes the space and {leaveAllCount} room{leaveAllCount === 1 ? "" : "s"} from your
          account. You can rejoin later.
        </p>
      {:else}
        <p class="text-sm text-surface-900-50">Leave this space?</p>
        <p class="text-xs text-surface-700-300">
          The space will be removed from your navigation. You can rejoin later.
        </p>
      {/if}
    </div>

    <div class="flex justify-between gap-2">
      <button
        type="button"
        class="btn preset-outlined-surface-200-800 px-4 py-2 hover:bg-surface-200-800"
        onclick={closeLeaveConfirm}
        disabled={leaving}
      >
        Cancel
      </button>
      <button
        type="button"
        class="btn preset-filled-error-500 px-4 py-2 opacity-90 hover:opacity-100 disabled:opacity-50"
        onclick={confirmMode === "all" ? onLeaveAll : onLeave}
        disabled={leaving || !room}
      >
        {leaving ? "Leaving..." : confirmMode === "all" ? "Leave all" : "Leave space"}
      </button>
    </div>
  </div>
</dialog>
