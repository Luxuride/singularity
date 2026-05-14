<script lang="ts">
  import type { MatrixChatSummary, MatrixRoomPreview } from "$lib/chats/types";

  type JoinPreviewRoom = MatrixChatSummary | MatrixRoomPreview;

  interface Props {
    open: boolean;
    room: JoinPreviewRoom | null;
    joinKind: "single" | "children";
    joinCount: number;
    joinRooms?: MatrixChatSummary[];
    confirmDisabled?: boolean;
    onClose: () => void;
    onConfirm: () => void;
  }

  let {
    open,
    room,
    joinKind,
    joinCount,
    joinRooms = [],
    confirmDisabled = false,
    onClose,
    onConfirm,
  }: Props = $props();

  let dialogElement: HTMLDialogElement | undefined = $state();

  const roomName = $derived(room?.displayName ?? "Unknown");
  const roomId = $derived(room?.roomId ?? "Unknown");
  const roomKindLabel = $derived(room?.kind === "space" ? "Space" : "Room");
  const titleLabel = $derived(
    joinKind === "children" ? "Join all rooms" : `Join ${roomKindLabel}`,
  );
  const confirmLabel = $derived(joinKind === "children" ? "Join all" : "Join");
  const iconKind = $derived(
    joinKind === "children" ? "rooms" : room?.kind === "space" ? "space" : "room",
  );
  const description = $derived.by(() => {
    const raw = room?.description ?? "";
    const normalized = raw.trim();
    return normalized.length > 0 ? normalized : "No description available.";
  });
  const joinedMembersLabel = $derived.by(() => {
    const count = room?.joinedMembers ?? 0;
    return `${count} member${count === 1 ? "" : "s"}`;
  });
  const joinHint = $derived(
    joinKind === "children"
      ? `This will join ${joinCount} room${joinCount === 1 ? "" : "s"} in this space.`
      : `You're about to join this ${roomKindLabel.toLowerCase()}.`,
  );

  $effect(() => {
    if (open && dialogElement) {
      if (!dialogElement.open) {
        dialogElement.showModal();
      }
    } else if (!open && dialogElement && dialogElement.open) {
      dialogElement.close();
    }
  });

  function handleClose() {
    onClose();
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
    <div class="flex items-start justify-between gap-3">
      <div class="space-y-1">
        <div class="flex items-center gap-2">
          {#if iconKind === "room"}
            <svg
              class="h-4 w-4 text-surface-700-200"
              viewBox="0 0 16 16"
              fill="none"
              stroke="currentColor"
              stroke-width="1.5"
              aria-hidden="true"
            >
              <rect x="2" y="2" width="12" height="12" rx="2" />
              <path d="M6 9h4" />
            </svg>
          {:else if iconKind === "space"}
            <svg
              class="h-4 w-4 text-surface-700-200"
              viewBox="0 0 16 16"
              fill="none"
              stroke="currentColor"
              stroke-width="1.5"
              aria-hidden="true"
            >
              <rect x="3" y="3" width="9" height="9" rx="2" />
              <rect x="6" y="6" width="7" height="7" rx="2" />
            </svg>
          {:else}
            <svg
              class="h-4 w-4 text-surface-700-200"
              viewBox="0 0 16 16"
              fill="none"
              stroke="currentColor"
              stroke-width="1.5"
              aria-hidden="true"
            >
              <path d="M3 4h10" />
              <path d="M3 8h10" />
              <path d="M3 12h10" />
            </svg>
          {/if}

          <h2 class="text-lg font-semibold text-surface-900-50">{titleLabel}</h2>
        </div>
        <p class="text-xs text-surface-700-300">{joinHint}</p>
      </div>
      <span class="text-[10px] uppercase tracking-wide text-surface-700-300">{roomKindLabel}</span>
    </div>

    <div class="rounded-lg border border-surface-200-800 bg-surface-50-950 p-3 space-y-3">
      <div>
        <p class="text-[10px] uppercase tracking-wide text-surface-700-300">Name</p>
        <p class="text-sm font-medium text-surface-900-50">{roomName}</p>
      </div>
      <div>
        <p class="text-[10px] uppercase tracking-wide text-surface-700-300">Room ID</p>
        <p class="text-xs text-surface-900-50 break-all">{roomId}</p>
      </div>
      <div>
        <p class="text-[10px] uppercase tracking-wide text-surface-700-300">Description</p>
        <p class="text-sm text-surface-700-100">{description}</p>
      </div>
      <div class="flex items-center justify-between">
        <p class="text-[10px] uppercase tracking-wide text-surface-700-300">Members</p>
        <p class="text-sm text-surface-900-50">{joinedMembersLabel}</p>
      </div>
    </div>

    {#if joinKind === "children"}
      <div class="rounded-lg border border-surface-200-800 bg-surface-50-950 p-3 space-y-2">
        <p class="text-[10px] uppercase tracking-wide text-surface-700-300">Rooms to join</p>
        {#if joinRooms.length === 0}
          <p class="text-sm text-surface-700-100">Room list unavailable.</p>
        {:else}
          <ul class="max-h-40 overflow-y-auto space-y-1">
            {#each joinRooms as joinRoom (joinRoom.roomId)}
              <li class="text-sm text-surface-900-50">
                {joinRoom.displayName}
              </li>
            {/each}
          </ul>
        {/if}
      </div>
    {/if}

    <div class="flex justify-end gap-2">
      <button
        type="button"
        class="btn preset-outlined-surface-200-800 px-4 py-2 hover:bg-surface-200-800"
        onclick={onClose}
      >
        Cancel
      </button>
      <button
        type="button"
        class="btn preset-filled-primary-500 px-4 py-2 opacity-90 hover:opacity-100 disabled:opacity-50"
        disabled={!room || confirmDisabled}
        onclick={onConfirm}
      >
        {confirmLabel}
      </button>
    </div>
  </div>
</dialog>
