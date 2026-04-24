<script lang="ts">
  import { matrixJoinRoomsBatch } from "$lib/chats/api";
  import type {
    MatrixJoinBatchResult,
    MatrixJoinTargetPreview,
    MatrixRoomKind,
  } from "$lib/chats/types";
  import { shellSelectedRoomId, shellErrorMessage } from "$lib/chats/shell";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";

  interface Props {
    open: boolean;
    onClose: () => void;
    targets?: MatrixJoinTargetPreview[];
    title?: string;
    confirmLabel?: string;
    allowManualEntry?: boolean;
    onJoined?: (result: MatrixJoinBatchResult) => Promise<void> | void;
  }

  let {
    open,
    onClose,
    targets = [],
    title = "Join Room or Space",
    confirmLabel = "Join",
    allowManualEntry = true,
    onJoined,
  }: Props = $props();

  let dialogElement: HTMLDialogElement | undefined = $state();
  let loading = $state(false);
  let roomInput = $state("");

  const confirmMode = $derived(targets.length > 0);
  const manualMode = $derived(!confirmMode && allowManualEntry);
  const normalizedInput = $derived(normalizeRoomInput(roomInput));
  const manualPreviewTargets = $derived.by<MatrixJoinTargetPreview[]>(() => {
    if (!manualMode || !normalizedInput) {
      return [];
    }

    return [
      {
        roomIdOrAlias: normalizedInput,
        displayName: normalizedInput,
        kind: inferKind(normalizedInput),
        serverNames: deriveServerNames(normalizedInput),
      },
    ];
  });
  const displayTargets = $derived(confirmMode ? targets : manualPreviewTargets);

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

  function normalizeRoomInput(raw: string): string {
    const trimmed = raw.trim();
    if (!trimmed) {
      return "";
    }

    if (trimmed.startsWith("#") || trimmed.startsWith("!")) {
      return trimmed;
    }

    return `#${trimmed}`;
  }

  function inferKind(roomIdOrAlias: string): MatrixRoomKind {
    if (roomIdOrAlias.startsWith("#") || roomIdOrAlias.startsWith("!")) {
      return "room";
    }

    return "space";
  }

  function deriveServerNames(roomIdOrAlias: string): string[] {
    const target = roomIdOrAlias.trim();
    const serverName = target.includes(":") ? target.slice(target.indexOf(":") + 1) : "";

    return serverName ? [serverName] : [];
  }

  function handleClose() {
    onClose();
  }

  async function joinRoom(event: Event) {
    event.preventDefault();
    if (loading) return;

    const joinTargets = confirmMode
      ? targets
      : (manualMode && normalizedInput
        ? [{
            roomIdOrAlias: normalizedInput,
            displayName: normalizedInput,
            kind: inferKind(normalizedInput),
            serverNames: deriveServerNames(normalizedInput),
          }]
        : []);

    if (joinTargets.length === 0) {
      return;
    }

    loading = true;
    shellErrorMessage.set("");

    try {
      const result = await matrixJoinRoomsBatch(joinTargets);

      if (onJoined) {
        await onJoined(result);
      } else {
        const joinedRoomId = result.joinedRoomIds.at(-1);
        if (joinedRoomId) {
          const searchParams = new URLSearchParams(page.url.searchParams);
          searchParams.set("roomId", joinedRoomId);
          shellSelectedRoomId.set(joinedRoomId);
          await goto(`/chats?${searchParams.toString()}`);
        }
      }

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
    <h2 class="text-lg font-semibold text-surface-900-50">{title}</h2>
    <form onsubmit={joinRoom} class="flex flex-col gap-4">
      {#if confirmMode}
        <p class="text-sm text-surface-700-300">Review all rooms and spaces that will be joined.</p>
      {:else if manualMode}
        <input
          type="text"
          placeholder="#room:server.com"
          bind:value={roomInput}
          class="input preset-outlined-surface-200-800 bg-surface-50-950 px-3 py-2 w-full"
          disabled={loading}
          use:focusInput
        />
      {/if}

      {#if displayTargets.length > 0}
        <ul class="max-h-56 overflow-y-auto rounded-lg border border-surface-300-700 bg-surface-50-950/60 p-2 space-y-2">
          {#each displayTargets as target, index (target.roomIdOrAlias + ":" + index)}
            <li class="flex items-start justify-between gap-3 rounded-md border border-surface-300-700 px-2 py-1.5">
              <div class="min-w-0">
                <p class="truncate text-sm font-medium text-surface-900-50">{target.displayName}</p>
                <p class="truncate text-xs text-surface-700-300">{target.roomIdOrAlias}</p>
              </div>
              <span class="text-[10px] uppercase tracking-wide text-surface-700-300">
                {target.kind === "space" ? "Space" : "Room"}
              </span>
            </li>
          {/each}
        </ul>
      {/if}

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
          disabled={loading || (manualMode && !roomInput.trim()) || (!confirmMode && !manualMode)}
        >
          {loading ? 'Joining...' : confirmLabel}
        </button>
      </div>
    </form>
  </div>
</dialog>
