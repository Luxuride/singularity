<script lang="ts">
  import { matrixGetRoomPreview, matrixJoinRoom, matrixTriggerRoomUpdate } from "$lib/chats/api";
  import { shellSelectedRootSpaceId, shellSelectedRoomId, shellErrorMessage } from "$lib/chats/shell";
  import type { MatrixRoomPreview } from "$lib/chats/types";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import JoinConfirmDialog from "../rooms/JoinConfirmDialog.svelte";

  interface Props {
    open: boolean;
    onClose: () => void;
  }

  let { open, onClose }: Props = $props();

  let dialogElement: HTMLDialogElement | undefined = $state();
  let previewing = $state(false);
  let joining = $state(false);
  let roomInput = $state("");
  let confirmOpen = $state(false);
  let previewRoom = $state<MatrixRoomPreview | null>(null);
  let previewTarget = $state<{ roomIdOrAlias: string; serverNames: string[] } | null>(null);

  $effect(() => {
    if (open && dialogElement) {
      if (!dialogElement.open) {
        dialogElement.showModal();
        roomInput = "";
        previewRoom = null;
        previewTarget = null;
        confirmOpen = false;
      }
    } else if (!open && dialogElement && dialogElement.open) {
      dialogElement.close();
    }
  });

  function focusInput(node: HTMLInputElement) {
    setTimeout(() => node.focus(), 10);
  }

  function handleClose() {
    confirmOpen = false;
    previewRoom = null;
    previewTarget = null;
    onClose();
  }

  function normalizeRoomTarget(raw: string): { roomIdOrAlias: string; serverNames: string[] } {
    const trimmed = raw.trim();
    if (!trimmed) {
      return { roomIdOrAlias: "", serverNames: [] };
    }

    if (trimmed.startsWith("http://") || trimmed.startsWith("https://")) {
      try {
        const parsed = new URL(trimmed);
        if (parsed.hostname === "matrix.to") {
          const hash = parsed.hash.startsWith("#/") ? parsed.hash.slice(2) : parsed.hash.slice(1);
          if (hash) {
            const [target, query] = hash.split("?");
            const params = new URLSearchParams(query ?? "");
            return { roomIdOrAlias: target, serverNames: params.getAll("via") };
          }
        }
      } catch {
        // Fall through to raw input parsing.
      }
    }

    let target = trimmed;
    if (!target.startsWith("#") && !target.startsWith("!")) {
      target = "#" + target;
    }

    return { roomIdOrAlias: target, serverNames: [] };
  }

  async function joinRoom(event: Event) {
    event.preventDefault();
    if (!roomInput.trim() || previewing || joining) return;

    const { roomIdOrAlias, serverNames } = normalizeRoomTarget(roomInput);
    if (!roomIdOrAlias) return;

    previewing = true;
    shellErrorMessage.set("");

    try {
      const preview = await matrixGetRoomPreview({ roomIdOrAlias, serverNames });
      previewRoom = preview;
      previewTarget = { roomIdOrAlias, serverNames };
      confirmOpen = true;
    } catch (e) {
      shellErrorMessage.set(e instanceof Error ? e.message : "Failed to preview room");
    } finally {
      previewing = false;
    }
  }

  function closeConfirm() {
    confirmOpen = false;
    previewRoom = null;
    previewTarget = null;
  }

  async function confirmJoin() {
    if (!previewTarget || joining) return;

    joining = true;
    shellErrorMessage.set("");

    try {
      const { roomId } = await matrixJoinRoom(
        previewTarget.roomIdOrAlias,
        previewTarget.serverNames,
      );

      await matrixTriggerRoomUpdate();

      const searchParams = new URLSearchParams(page.url.searchParams);

      if (previewRoom?.kind === "space") {
        shellSelectedRootSpaceId.set(roomId);
        shellSelectedRoomId.set("");
        searchParams.set("rootSpaceId", roomId);
        searchParams.delete("roomId");
      } else {
        shellSelectedRoomId.set(roomId);
        searchParams.set("roomId", roomId);
      }

      const search = searchParams.toString();
      await goto(search ? `/chats?${search}` : "/chats", {
        replaceState: true,
        noScroll: true,
        keepFocus: true,
      });
      handleClose();
    } catch (e) {
      shellErrorMessage.set(e instanceof Error ? e.message : "Failed to join room");
    } finally {
      joining = false;
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
        disabled={previewing || joining}
        use:focusInput
      />
      <div class="flex justify-end gap-2">
        <button
          type="button"
          class="btn preset-outlined-surface-200-800 px-4 py-2 hover:bg-surface-200-800"
          onclick={onClose}
          disabled={previewing || joining}
        >
          Cancel
        </button>
        <button
          type="submit"
          class="btn preset-filled-primary-500 px-4 py-2 opacity-90 hover:opacity-100 disabled:opacity-50"
          disabled={previewing || joining || !roomInput.trim()}
        >
          {previewing ? "Checking..." : "Continue"}
        </button>
      </div>
    </form>
  </div>
</dialog>

<JoinConfirmDialog
  open={confirmOpen}
  room={previewRoom}
  joinKind="single"
  joinCount={0}
  confirmDisabled={joining}
  onClose={closeConfirm}
  onConfirm={confirmJoin}
/>
