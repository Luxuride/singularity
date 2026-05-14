<script lang="ts">
  import type { MatrixChatSummary } from "$lib/chats/types";

  interface Props {
    open: boolean;
    room: MatrixChatSummary | null;
    leaving?: boolean;
    onClose: () => void;
    onLeave: () => void;
  }

  let { open, room, leaving = false, onClose, onLeave }: Props = $props();

  let dialogElement: HTMLDialogElement | undefined = $state();
  let confirmElement: HTMLDialogElement | undefined = $state();
  let confirmOpen = $state(false);

  const roomName = $derived(room?.displayName ?? "Unknown");
  const roomId = $derived(room?.roomId ?? "Unknown");

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
      <p class="text-xs uppercase tracking-wide text-surface-700-300">Room settings</p>
      <h2 class="text-lg font-semibold text-surface-900-50">{roomName}</h2>
      <p class="text-xs text-surface-700-300 break-all">{roomId}</p>
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
          <p class="text-xs text-surface-700-300">Basic room settings and membership.</p>
        </div>

        <div class="flex items-center justify-between gap-3 rounded border border-surface-300-700 bg-surface-100-900 p-3">
          <div>
            <p class="text-sm text-surface-900-50">Leave room</p>
            <p class="text-xs text-surface-700-300">
              Leave this room and remove it from your sidebar.
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
      <p class="text-xs uppercase tracking-wide text-surface-700-300">Leave room</p>
      <h2 class="text-lg font-semibold text-surface-900-50">{roomName}</h2>
      <p class="text-xs text-surface-700-300 break-all">{roomId}</p>
    </header>

    <div class="rounded-lg border border-surface-200-800 bg-surface-50-950 p-3 space-y-2">
      <p class="text-sm text-surface-900-50">Leave this room?</p>
      <p class="text-xs text-surface-700-300">
        You can rejoin later, but you may lose access to history or encrypted keys.
      </p>
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
        onclick={onLeave}
        disabled={leaving || !room}
      >
        {leaving ? "Leaving..." : "Leave room"}
      </button>
    </div>
  </div>
</dialog>
