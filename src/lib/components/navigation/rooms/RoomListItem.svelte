<script lang="ts">
  import { matrixGetRoomImage, matrixJoinRoom, matrixTriggerRoomUpdate } from "$lib/chats/api";
  import type { MatrixChatSummary } from "$lib/chats/types";
  import { shellErrorMessage } from "$lib/chats/shell";
  import { RoomAvatar, RoomSettingsDialog, isVirtualRoomId, roomImageCache } from "../shared";

  interface Props {
    room: MatrixChatSummary;
    depth: number;
    isSelected: boolean;
    hasChildren: boolean;
    isExpanded: boolean;
    onSelect?: (roomId: string) => void;
    onToggleSpace?: (roomId: string) => void;
  }

  let {
    room,
    depth,
    isSelected,
    hasChildren,
    isExpanded,
    onSelect,
    onToggleSpace,
  }: Props = $props();

  const isSpace = $derived(room.kind === "space");
  let lazyImageUrl = $state<string | null | undefined>(null);
  let settingsDialogOpen = $state(false);
  let isContextMenuOpen = $state(false);
  let contextMenuPosition = $state<{ x: number; y: number } | null>(null);
  let contextMenuElement: HTMLDivElement | undefined = $state();
  let isJoining = $state(false);

  const contextMenuAvailable = $derived(!isVirtualRoomId(room.roomId));
  const settingsAvailable = $derived(!isVirtualRoomId(room.roomId) && room.joined);

  const indentation = $derived(`${Math.max(0, depth) * 0.9}rem`);

  $effect(() => {
    lazyImageUrl = room.imageUrl;

    if (room.imageUrl) {
      roomImageCache.prime(room.roomId, room.imageUrl);
    }
  });

  $effect(() => {
    if (lazyImageUrl || isVirtualRoomId(room.roomId)) {
      return;
    }

    const cachedImage = roomImageCache.getCached(room.roomId);
    if (cachedImage !== undefined) {
      lazyImageUrl = cachedImage;
      return;
    }

    const roomId = room.roomId;
    void roomImageCache
      .getOrLoad(room.roomId, () => matrixGetRoomImage(room.roomId))
      .then((imageUrl) => {
        if (room.roomId === roomId) {
          lazyImageUrl = imageUrl;
        }
      });
  });

  async function handleJoin(event?: MouseEvent) {
    if (event) {
      event.stopPropagation();
    }
    if (isJoining || room.joined) {
      return;
    }

    isJoining = true;
    shellErrorMessage.set("");

    try {
      const res = await matrixJoinRoom(room.roomId);
      await matrixTriggerRoomUpdate();
      onSelect?.(res.roomId || room.roomId);
    } catch (error) {
      shellErrorMessage.set(error instanceof Error ? error.message : "Failed to join room");
    } finally {
      isJoining = false;
    }
  }

  function handleClick() {
    if (isSpace) {
      if (hasChildren) {
        onToggleSpace?.(room.roomId);
      }
      return;
    }

    if (!room.joined) {
      void handleJoin();
      return;
    }

    onSelect?.(room.roomId);
  }

  $effect(() => {
    if (!isContextMenuOpen) {
      return;
    }

    const onWindowPointerDown = (event: PointerEvent) => {
      if (!(event.target instanceof Node)) {
        return;
      }

      if (contextMenuElement?.contains(event.target)) {
        return;
      }

      closeContextMenu();
    };

    const onWindowKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") {
        return;
      }

      closeContextMenu();
    };

    window.addEventListener("pointerdown", onWindowPointerDown, true);
    window.addEventListener("keydown", onWindowKeyDown);

    return () => {
      window.removeEventListener("pointerdown", onWindowPointerDown, true);
      window.removeEventListener("keydown", onWindowKeyDown);
    };
  });

  function openContextMenu(event: MouseEvent) {
    if (!contextMenuAvailable) {
      return;
    }

    event.preventDefault();
    event.stopPropagation();
    contextMenuPosition = { x: event.clientX, y: event.clientY };
    isContextMenuOpen = true;
  }

  function closeContextMenu() {
    isContextMenuOpen = false;
    contextMenuPosition = null;
  }

  function openSettingsFromContextMenu() {
    closeContextMenu();
    settingsDialogOpen = true;
  }
</script>

<li oncontextmenu={openContextMenu}>
  <div
    class="flex items-center justify-between w-full p-2 rounded transition-colors"
    class:hover:bg-surface-200-800={!isSpace || hasChildren}
    class:opacity-90={isSpace}
    class:bg-primary-100-900={isSelected}
    style={`padding-left: calc(0.5rem + ${indentation});`}
  >
    <button
      type="button"
      class="flex-1 text-left min-w-0"
      class:cursor-pointer={!isSpace || hasChildren}
      class:cursor-default={isSpace && !hasChildren}
      onclick={handleClick}
    >
      <div class="flex items-center gap-2 min-w-0">
        <RoomAvatar imageUrl={lazyImageUrl} displayName={room.displayName} />

        <p class="font-medium truncate">
          {#if isSpace}
            {isExpanded ? "▼" : "▶"}
          {/if}
          {room.displayName}
        </p>
      </div>

      {#if room.kind === "space"}
        <p class="text-xs text-surface-700-300">
          {#if !room.joined}
            <span class="text-warning-500 font-medium">Not joined</span> • {hasChildren ? "Contains rooms" : "No child rooms"}
          {:else}
            {hasChildren ? "Contains rooms" : "No child rooms"}
          {/if}
        </p>
      {:else}
        <p class="text-xs text-surface-700-300">
          {#if !room.joined}
            <span class="text-warning-500 font-medium">Not joined</span> • {room.joinedMembers} members
          {:else}
            {room.encrypted ? "Encrypted" : "Unencrypted"} • {room.joinedMembers} members
          {/if}
        </p>
      {/if}
    </button>

    <div class="flex items-center gap-1.5 shrink-0 ml-2">
      {#if room.kind === "space"}
        <span class="text-[10px] uppercase tracking-wide text-surface-700-300">Space</span>
      {/if}
      {#if !room.joined}
        <button
          type="button"
          class="btn preset-filled-primary-500 text-xs px-2.5 py-1"
          disabled={isJoining}
          onclick={handleJoin}
        >
          {isJoining ? "Joining..." : "Join"}
        </button>
      {/if}
    </div>
  </div>

  {#if isContextMenuOpen && contextMenuPosition}
    <div
      class="fixed z-30 min-w-28 overflow-hidden rounded border border-surface-300-700 bg-surface-50-950 shadow"
      style={`left: ${contextMenuPosition.x}px; top: ${contextMenuPosition.y}px;`}
      role="menu"
      bind:this={contextMenuElement}
    >
      {#if !room.joined}
        <button
          type="button"
          class="block w-full px-3 py-2 text-left text-sm hover:bg-surface-200-800 text-primary-500 font-medium"
          role="menuitem"
          disabled={isJoining}
          onclick={() => {
            closeContextMenu();
            void handleJoin();
          }}
        >
          {isJoining ? "Joining..." : "Join"}
        </button>
      {:else if settingsAvailable}
        <button
          type="button"
          class="block w-full px-3 py-2 text-left text-sm hover:bg-surface-200-800"
          role="menuitem"
          onclick={openSettingsFromContextMenu}
        >
          Settings
        </button>
      {/if}
    </div>
  {/if}

  {#if settingsAvailable}
    <RoomSettingsDialog
      open={settingsDialogOpen}
      roomId={room.roomId}
      roomName={room.displayName}
      isSpace={isSpace}
      onClose={() => (settingsDialogOpen = false)}
    />
  {/if}
</li>
