<script lang="ts">
  import type { MatrixChatSummary } from "$lib/chats/types";
  import type { FlatEntry } from "./types";
  import { buildRoomHierarchy } from "./hierarchy";
  import RoomListItem from "./RoomListItem.svelte";
  import JoinConfirmDialog from "./JoinConfirmDialog.svelte";
  import RoomManageDialog from "./RoomManageDialog.svelte";
  import SpaceManageDialog from "./SpaceManageDialog.svelte";
  import { isVirtualRoomId } from "../shared";

  interface Props {
    rooms: MatrixChatSummary[];
    selectedRoomId: string | null;
    onSelectRoom?: (roomId: string) => void;
    onJoinRoom?: (roomId: string) => void;
    showJoinAllRooms?: boolean;
    joinAllRoomContext?: MatrixChatSummary | null;
    joinAllRoomIds?: string[];
    onJoinAllRooms?: () => void;
    onJoinAllChildren?: (roomIds: string[]) => void;
    joinAllDisabled?: boolean;
    enableManageMenu?: boolean;
    onLeaveRoom?: (roomId: string) => Promise<void> | void;
    onLeaveRooms?: (roomIds: string[]) => Promise<void> | void;
    emptyMessage?: string;
  }

  let {
    rooms,
    selectedRoomId,
    onSelectRoom,
    onJoinRoom,
    showJoinAllRooms = false,
    joinAllRoomContext = null,
    joinAllRoomIds = [],
    onJoinAllRooms,
    onJoinAllChildren,
    joinAllDisabled = false,
    enableManageMenu = false,
    onLeaveRoom,
    onLeaveRooms,
    emptyMessage = "No rooms in this space yet.",
  }: Props = $props();

  let expandedSpaceIds = $state<Record<string, boolean>>({});
  let joinDialogOpen = $state(false);
  let pendingJoin = $state<{
    kind: "single" | "children";
    action: "single" | "children" | "all";
    room: MatrixChatSummary;
    roomIds: string[];
    rooms: MatrixChatSummary[];
  } | null>(null);
  let manageMenuOpen = $state(false);
  let manageMenuPosition = $state<{ x: number; y: number } | null>(null);
  let manageMenuElement = $state<HTMLDivElement | null>(null);
  let manageTarget = $state<MatrixChatSummary | null>(null);
  let manageRoomOpen = $state(false);
  let manageSpaceOpen = $state(false);
  let manageLeaveAllRoomIds = $state<string[]>([]);
  let leaving = $state(false);

  $effect(() => {
    const nextExpanded = { ...expandedSpaceIds };
    let changed = false;

    for (const room of rooms) {
      if (room.kind !== "space" || nextExpanded[room.roomId] !== undefined) {
        continue;
      }

      nextExpanded[room.roomId] = room.joined;
      changed = true;
    }

    if (changed) {
      expandedSpaceIds = nextExpanded;
    }
  });

  const flatEntries = $derived.by<FlatEntry[]>(() => buildRoomHierarchy(rooms, expandedSpaceIds));
  const roomsById = $derived.by(() => new Map(rooms.map((room) => [room.roomId, room])));
  const joinableRoomsBySpace = $derived.by<Map<string, string[]>>(() => {
    const roomsById = new Map(rooms.map((room) => [room.roomId, room]));
    const childrenByParent = new Map<string, string[]>();

    for (const room of rooms) {
      for (const childRoomId of room.childrenRoomIds ?? []) {
        if (!roomsById.has(childRoomId)) {
          continue;
        }

        const siblings = childrenByParent.get(room.roomId) ?? [];
        siblings.push(childRoomId);
        childrenByParent.set(room.roomId, siblings);
      }
    }

    const memo = new Map<string, string[]>();
    const visiting = new Set<string>();

    const collectJoinableTargets = (spaceId: string): string[] => {
      if (memo.has(spaceId)) {
        return memo.get(spaceId) ?? [];
      }

      if (visiting.has(spaceId)) {
        return [];
      }

      visiting.add(spaceId);

      const children = childrenByParent.get(spaceId) ?? [];
      const joinable: string[] = [];

      for (const childId of children) {
        const child = roomsById.get(childId);
        if (!child) {
          continue;
        }

        if (child.kind === "space") {
          if (!child.joined) {
            joinable.push(child.roomId);
          }
          joinable.push(...collectJoinableTargets(child.roomId));
          continue;
        }

        if (!child.joined) {
          joinable.push(child.roomId);
        }
      }

      visiting.delete(spaceId);
      memo.set(spaceId, joinable);
      return joinable;
    };

    const joinableBySpace = new Map<string, string[]>();
    for (const room of rooms) {
      if (room.kind !== "space") {
        continue;
      }

      const joinableRooms = collectJoinableTargets(room.roomId);
      if (joinableRooms.length > 0) {
        joinableBySpace.set(room.roomId, joinableRooms);
      }
    }

    return joinableBySpace;
  });

  function toggleSpace(roomId: string) {
    expandedSpaceIds = {
      ...expandedSpaceIds,
      [roomId]: !expandedSpaceIds[roomId],
    };
  }

  function requestJoin(room: MatrixChatSummary) {
    if (!onJoinRoom) {
      return;
    }

    pendingJoin = {
      kind: "single",
      action: "single",
      room,
      roomIds: [],
      rooms: [],
    };
    joinDialogOpen = true;
  }

  function requestJoinAllChildren(room: MatrixChatSummary, roomIds: string[]) {
    if (!onJoinAllChildren || roomIds.length === 0) {
      return;
    }

    const joinRooms = roomIds
      .map((roomId) => roomsById.get(roomId))
      .filter((childRoom): childRoom is MatrixChatSummary => !!childRoom)
      .sort((a, b) => a.displayName.localeCompare(b.displayName));

    pendingJoin = {
      kind: "children",
      action: "children",
      room,
      roomIds,
      rooms: joinRooms,
    };
    joinDialogOpen = true;
  }

  function requestJoinAllRooms() {
    if (!onJoinAllRooms || !joinAllRoomContext || joinAllRoomIds.length === 0) {
      onJoinAllRooms?.();
      return;
    }

    const joinRooms = joinAllRoomIds
      .map((roomId) => roomsById.get(roomId))
      .filter((childRoom): childRoom is MatrixChatSummary => !!childRoom)
      .sort((a, b) => a.displayName.localeCompare(b.displayName));

    pendingJoin = {
      kind: "children",
      action: "all",
      room: joinAllRoomContext,
      roomIds: joinAllRoomIds,
      rooms: joinRooms,
    };
    joinDialogOpen = true;
  }

  function closeJoinDialog() {
    joinDialogOpen = false;
    pendingJoin = null;
  }

  $effect(() => {
    if (!manageMenuOpen) {
      return;
    }

    const onWindowPointerDown = (event: PointerEvent) => {
      const target = event.target;
      if (!(target instanceof Node)) {
        return;
      }

      if (manageMenuElement?.contains(target)) {
        return;
      }

      closeManageMenu();
    };

    const onWindowKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") {
        return;
      }

      closeManageMenu();
    };

    window.addEventListener("pointerdown", onWindowPointerDown, true);
    window.addEventListener("keydown", onWindowKeyDown);

    return () => {
      window.removeEventListener("pointerdown", onWindowPointerDown, true);
      window.removeEventListener("keydown", onWindowKeyDown);
    };
  });

  function closeManageMenu() {
    manageMenuOpen = false;
    manageMenuPosition = null;
  }

  function openManageMenu(room: MatrixChatSummary, event: MouseEvent) {
    if (!enableManageMenu || !room.joined || isVirtualRoomId(room.roomId)) {
      return;
    }

    event.preventDefault();
    event.stopPropagation();
    manageTarget = room;
    manageLeaveAllRoomIds = room.kind === "space" ? collectJoinedRooms(room.roomId) : [];
    manageMenuPosition = { x: event.clientX, y: event.clientY };
    manageMenuOpen = true;
  }

  function openManageDialog() {
    if (!manageTarget) {
      return;
    }

    if (manageTarget.kind === "space") {
      manageSpaceOpen = true;
    } else {
      manageRoomOpen = true;
    }
    closeManageMenu();
  }

  function closeManageDialogs() {
    manageRoomOpen = false;
    manageSpaceOpen = false;
    manageTarget = null;
    manageLeaveAllRoomIds = [];
  }

  function collectJoinedRooms(spaceId: string): string[] {
    const visited = new Set<string>();
    const collected: string[] = [];

    const walk = (roomId: string) => {
      if (visited.has(roomId)) {
        return;
      }
      visited.add(roomId);

      const room = roomsById.get(roomId);
      if (!room) {
        return;
      }

      if (room.kind === "room") {
        if (room.joined && !isVirtualRoomId(room.roomId)) {
          collected.push(room.roomId);
        }
        return;
      }

      for (const childId of room.childrenRoomIds ?? []) {
        walk(childId);
      }
    };

    walk(spaceId);
    return collected;
  }

  async function confirmLeave() {
    if (!manageTarget || leaving) {
      return;
    }

    leaving = true;

    try {
      await onLeaveRoom?.(manageTarget.roomId);
      closeManageDialogs();
    } catch {
      // Leave errors are surfaced by the caller.
    } finally {
      leaving = false;
    }
  }

  async function confirmLeaveAll() {
    if (!manageTarget || leaving || manageLeaveAllRoomIds.length === 0) {
      return;
    }

    leaving = true;

    const leaveIds = manageTarget.joined
      ? [manageTarget.roomId, ...manageLeaveAllRoomIds]
      : [...manageLeaveAllRoomIds];

    const uniqueIds = Array.from(new Set(leaveIds));

    try {
      if (onLeaveRooms) {
        await onLeaveRooms(uniqueIds);
      } else {
        for (const roomId of uniqueIds) {
          await onLeaveRoom?.(roomId);
        }
      }
      closeManageDialogs();
    } catch {
      // Leave errors are surfaced by the caller.
    } finally {
      leaving = false;
    }
  }

  async function confirmJoin() {
    if (!pendingJoin) {
      return;
    }

    const join = pendingJoin;
    closeJoinDialog();

    if (join.action === "single") {
      await onJoinRoom?.(join.room.roomId);
      return;
    }

    if (join.action === "all") {
      onJoinAllRooms?.();
      return;
    }

    onJoinAllChildren?.(join.roomIds);
  }
</script>

<aside class="card p-2 preset-outlined-surface-200-800 bg-surface-100-900 flex flex-col flex-1 min-h-0 gap-3">
  {#if showJoinAllRooms && onJoinAllRooms}
    <button
      type="button"
      class="btn preset-tonal text-xs w-full"
      disabled={joinAllDisabled}
      onclick={requestJoinAllRooms}
    >
      Join all rooms
    </button>
  {/if}
  <div class="min-h-0 flex-1 overflow-y-auto">
    {#if rooms.length === 0}
      <p class="p-2 text-sm text-surface-700-300">{emptyMessage}</p>
    {:else}
      <ul class="space-y-1">
        {#each flatEntries as entry (entry.key)}
          <RoomListItem
            room={entry.room}
            depth={entry.depth}
            hasChildren={entry.hasChildren}
            isExpanded={expandedSpaceIds[entry.room.roomId] ?? true}
            isSelected={entry.room.roomId === selectedRoomId}
            onSelect={onSelectRoom}
            onToggleSpace={toggleSpace}
            onJoin={() => requestJoin(entry.room)}
            onContextMenu={openManageMenu}
            joinAllRoomIds={joinableRoomsBySpace.get(entry.room.roomId) ?? []}
            onJoinAllChildren={(_, roomIds) => requestJoinAllChildren(entry.room, roomIds)}
            joinAllDisabled={joinAllDisabled}
          />
        {/each}
      </ul>
    {/if}
  </div>
</aside>

{#if manageMenuOpen}
  <div
    class="z-30 min-w-32 overflow-hidden rounded border border-surface-300-700 bg-surface-50-950 shadow fixed"
    style={manageMenuPosition ? `left: ${manageMenuPosition.x}px; top: ${manageMenuPosition.y}px;` : undefined}
    role="menu"
    bind:this={manageMenuElement}
  >
    <button
      type="button"
      class="block w-full px-3 py-2 text-left text-sm hover:bg-surface-200-800"
      onclick={openManageDialog}
      role="menuitem"
    >
      {manageTarget?.kind === "space" ? "Manage space" : "Manage room"}
    </button>
  </div>
{/if}

<JoinConfirmDialog
  open={joinDialogOpen}
  room={pendingJoin?.room ?? null}
  joinKind={pendingJoin?.kind ?? "single"}
  joinCount={pendingJoin?.roomIds.length ?? 0}
  joinRooms={pendingJoin?.rooms ?? []}
  confirmDisabled={pendingJoin?.kind === "children" && joinAllDisabled}
  onClose={closeJoinDialog}
  onConfirm={confirmJoin}
/>

<RoomManageDialog
  open={manageRoomOpen}
  room={manageTarget}
  leaving={leaving}
  onClose={closeManageDialogs}
  onLeave={confirmLeave}
/>

<SpaceManageDialog
  open={manageSpaceOpen}
  room={manageTarget}
  leaveAllCount={manageLeaveAllRoomIds.length}
  leaving={leaving}
  onClose={closeManageDialogs}
  onLeave={confirmLeave}
  onLeaveAll={confirmLeaveAll}
/>
