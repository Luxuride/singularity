<script lang="ts">
  import { matrixGetRoomImage } from "$lib/chats/api";
  import type { MatrixChatSummary } from "$lib/chats/types";
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

  const settingsAvailable = $derived(!isVirtualRoomId(room.roomId));

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

  function handleClick() {
    if (isSpace) {
      if (hasChildren) {
        onToggleSpace?.(room.roomId);
      }
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
    if (!settingsAvailable) {
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
  <button
    type="button"
    class="w-full text-left p-2 rounded transition-colors"
    class:hover:bg-surface-200-800={!isSpace}
    class:opacity-90={isSpace}
    class:cursor-pointer={!isSpace || hasChildren}
    class:cursor-default={isSpace && !hasChildren}
    class:bg-primary-100-900={isSelected}
    style={`padding-left: calc(0.5rem + ${indentation});`}
    onclick={handleClick}
  >
    <div class="flex items-start justify-between gap-2">
      <div class="flex items-start gap-2 min-w-0">
        <RoomAvatar imageUrl={lazyImageUrl} displayName={room.displayName} />

        <p class="font-medium truncate">
          {#if isSpace}
            {isExpanded ? "▼" : "▶"}
          {/if}
          {room.displayName}
        </p>
      </div>

      {#if room.kind === "space"}
        <span class="text-[10px] uppercase tracking-wide text-surface-700-300">Space</span>
      {/if}
    </div>

    {#if room.kind === "space"}
      <p class="text-xs text-surface-700-300">
        {hasChildren ? "Contains rooms" : "No child rooms"}
      </p>
    {:else}
      <p class="text-xs text-surface-700-300">
        {room.encrypted ? "Encrypted" : "Unencrypted"} • {room.joinedMembers} members
      </p>
    {/if}
  </button>

  {#if isContextMenuOpen && contextMenuPosition}
    <div
      class="fixed z-30 min-w-28 overflow-hidden rounded border border-surface-300-700 bg-surface-50-950 shadow"
      style={`left: ${contextMenuPosition.x}px; top: ${contextMenuPosition.y}px;`}
      role="menu"
      bind:this={contextMenuElement}
    >
      <button
        type="button"
        class="block w-full px-3 py-2 text-left text-sm hover:bg-surface-200-800"
        role="menuitem"
        onclick={openSettingsFromContextMenu}
      >
        Settings
      </button>
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
