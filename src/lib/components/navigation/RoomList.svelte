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
    const childRoomIds = new Set<string>();

    for (const room of rooms) {
      for (const childRoomId of room.childrenRoomIds ?? []) {
        childRoomIds.add(childRoomId);

        const childRoom = roomsById.get(childRoomId);
        if (!childRoom) {
          continue;
        }

        const siblings = childrenByParent.get(room.roomId) ?? [];
        siblings.push(childRoom);
        childrenByParent.set(room.roomId, siblings);
      }
    }

    for (const siblings of childrenByParent.values()) {
      siblings.sort(sortRooms);
    }

    const entries: FlatEntry[] = [];
    const rootRooms = rooms
      .filter((room) => !childRoomIds.has(room.roomId))
      .sort(sortRooms);

    for (const room of rootRooms) {
      appendRoom(room, 0, new Set<string>(), entries, childrenByParent, room.roomId);
    }

    return entries;
  });

  function sortRooms(a: MatrixChatSummary, b: MatrixChatSummary): number {
    if (a.kind !== b.kind) {
      return a.kind === "space" ? -1 : 1;
    }

    return a.displayName.localeCompare(b.displayName, undefined, { sensitivity: "base" });
  }

  function appendRoom(
    room: MatrixChatSummary,
    depth: number,
    ancestry: Set<string>,
    entries: FlatEntry[],
    childrenByParent: Map<string, MatrixChatSummary[]>,
    keySeed: string,
  ) {
    const children = childrenByParent.get(room.roomId) ?? [];
    const hasChildren = children.length > 0;

    entries.push({
      key: `${keySeed}:${room.roomId}`,
      room,
      depth,
      hasChildren,
    });

    if (!hasChildren) {
      return;
    }

    if (ancestry.has(room.roomId)) {
      return;
    }

    if (room.kind === "space" && expandedSpaceIds[room.roomId] === false) {
      return;
    }

    const nextAncestry = new Set(ancestry);
    nextAncestry.add(room.roomId);

    for (let index = 0; index < children.length; index += 1) {
      const child = children[index];
      appendRoom(
        child,
        depth + 1,
        nextAncestry,
        entries,
        childrenByParent,
        `${keySeed}:${room.roomId}:${index}`,
      );
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
