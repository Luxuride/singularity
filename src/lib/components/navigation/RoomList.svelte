<script lang="ts">
  import type { MatrixChatSummary } from "$lib/chats/types";
  import RoomListItem from "./RoomListItem.svelte";

  interface Props {
    rooms: MatrixChatSummary[];
    selectedRoomId: string | null;
    onSelectRoom?: (roomId: string) => void;
    emptyMessage?: string;
  }

  let {
    rooms,
    selectedRoomId,
    onSelectRoom,
    emptyMessage = "No rooms in this space yet.",
  }: Props = $props();

  type FlatEntry = {
    key: string;
    room: MatrixChatSummary;
    depth: number;
    hasChildren: boolean;
  };

  let expandedSpaceIds = $state<Record<string, boolean>>({});

  $effect(() => {
    const nextExpanded = { ...expandedSpaceIds };
    let changed = false;

    for (const room of rooms) {
      if (room.kind !== "space" || nextExpanded[room.roomId] !== undefined) {
        continue;
      }

      nextExpanded[room.roomId] = true;
      changed = true;
    }

    if (changed) {
      expandedSpaceIds = nextExpanded;
    }
  });

  const flatEntries = $derived.by<FlatEntry[]>(() => {
    const roomsById = new Map(rooms.map((room) => [room.roomId, room]));
    const childrenByParent = new Map<string, MatrixChatSummary[]>();

    const rootKey = "__root__";

    for (const room of rooms) {
      const parentId = normalizeParentRoomId(room, roomsById);
      const key = parentId ?? rootKey;
      const siblings = childrenByParent.get(key) ?? [];
      siblings.push(room);
      childrenByParent.set(key, siblings);
    }

    for (const siblings of childrenByParent.values()) {
      siblings.sort(sortRooms);
    }

    const entries: FlatEntry[] = [];
    appendChildren(rootKey, 0, new Set<string>(), entries, childrenByParent);
    return entries;
  });

  function normalizeParentRoomId(
    room: MatrixChatSummary,
    roomsById: Map<string, MatrixChatSummary>,
  ): string | null {
    const candidateParentIds: string[] = [];

    if (room.parentRoomId) {
      candidateParentIds.push(room.parentRoomId);
    }

    for (const parentRoomId of room.parentRoomIds ?? []) {
      if (!candidateParentIds.includes(parentRoomId)) {
        candidateParentIds.push(parentRoomId);
      }
    }

    for (const parentRoomId of candidateParentIds) {
      if (!parentRoomId || parentRoomId === room.roomId) {
        continue;
      }

      if (!roomsById.has(parentRoomId)) {
        continue;
      }

      return parentRoomId;
    }

    return null;
  }

  function sortRooms(a: MatrixChatSummary, b: MatrixChatSummary): number {
    if (a.kind !== b.kind) {
      return a.kind === "space" ? -1 : 1;
    }

    return a.displayName.localeCompare(b.displayName, undefined, { sensitivity: "base" });
  }

  function appendChildren(
    parentKey: string,
    depth: number,
    ancestry: Set<string>,
    entries: FlatEntry[],
    childrenByParent: Map<string, MatrixChatSummary[]>,
  ) {
    const children = childrenByParent.get(parentKey) ?? [];
    let childIndex = 0;

    for (const room of children) {
      const hasChildren = (childrenByParent.get(room.roomId)?.length ?? 0) > 0;
      entries.push({
        key: `${parentKey}:${room.roomId}:${childIndex}`,
        room,
        depth,
        hasChildren,
      });
      childIndex += 1;

      if (ancestry.has(room.roomId)) {
        continue;
      }

      if (!hasChildren) {
        continue;
      }

      if (room.kind === "space" && expandedSpaceIds[room.roomId] === false) {
        continue;
      }

      const nextAncestry = new Set(ancestry);
      nextAncestry.add(room.roomId);
      appendChildren(room.roomId, depth + 1, nextAncestry, entries, childrenByParent);
    }
  }

  function toggleSpace(roomId: string) {
    expandedSpaceIds = {
      ...expandedSpaceIds,
      [roomId]: !expandedSpaceIds[roomId],
    };
  }
</script>

<aside class="card p-2 preset-outlined-surface-200-800 bg-surface-100-900 flex flex-col flex-1 min-h-0 gap-3">
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
          />
        {/each}
      </ul>
    {/if}
  </div>
</aside>
