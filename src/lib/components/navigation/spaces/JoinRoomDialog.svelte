<script lang="ts">
  import { matrixJoinRoom } from "$lib/chats/api";
  import { shellSelectedRoomId, shellErrorMessage } from "$lib/chats/shell";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";

  interface Props {
    open: boolean;
    onClose: () => void;
  }

  let { open, onClose }: Props = $props();

  let dialogElement: HTMLDialogElement | undefined = $state();
  let loading = $state(false);
  let roomInput = $state("");

  $effect(() => {
    if (open && dialogElement) {
      if (!dialogElement.open) {
        dialogElement.showModal();
        roomInput = "";
      }
    } else if (!open && dialogElement && dialogElement.open) {
      dialogElement.close();
    }
  });

  function focusInput(node: HTMLInputElement) {
    setTimeout(() => node.focus(), 10);
  }

  function handleClose() {
    onClose();
  }

  async function joinRoom(event: Event) {
    event.preventDefault();
    if (!roomInput.trim() || loading) return;

    let targetIdOrAlias = roomInput.trim();
    if (!targetIdOrAlias.startsWith("#") && !targetIdOrAlias.startsWith("!")) {
      targetIdOrAlias = "#" + targetIdOrAlias;
    }

    loading = true;
    shellErrorMessage.set("");

    try {
      const { roomId } = await matrixJoinRoom(targetIdOrAlias);
      
      const searchParams = new URLSearchParams(page.url.searchParams);
      searchParams.set("roomId", roomId);
      shellSelectedRoomId.set(roomId);

      await goto(`/chats?${searchParams.toString()}`);
      onClose();
    } catch (e) {
      shellErrorMessage.set(e instanceof Error ? e.message : "Failed to join room");
    } finally {
      loading = false;
    }
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<dialog
  bind:this={dialogElement}
  onclose={handleClose}
  class="backdrop:bg-surface-900/50 card preset-outlined-surface-200-800 bg-surface-100-900 m-auto rounded-xl p-4 shadow-xl min-w-[300px]"
  onmousedown={(e) => {
      if (e.target === dialogElement) onClose();
  }}
>
  <div class="flex flex-col gap-4">
    <h2 class="text-lg font-semibold text-surface-900-50">Join Room or Space</h2>
    <form onsubmit={joinRoom} class="flex flex-col gap-4">
      <input
        type="text"
        placeholder="#room:server.com"
        bind:value={roomInput}
        class="input preset-outlined-surface-200-800 bg-surface-50-950 px-3 py-2 w-full"
        disabled={loading}
        use:focusInput
      />
      <div class="flex justify-end gap-2">
        <button
          type="button"
          class="btn preset-outlined-surface-200-800 px-4 py-2 hover:bg-surface-200-800"
          onclick={onClose}
          disabled={loading}
        >
          Cancel
        </button>
        <button
          type="submit"
          class="btn preset-filled-primary-500 px-4 py-2 opacity-90 hover:opacity-100 disabled:opacity-50"
          disabled={loading || !roomInput.trim()}
        >
          {loading ? 'Joining...' : 'Join'}
        </button>
      </div>
    </form>
  </div>
</dialog>
