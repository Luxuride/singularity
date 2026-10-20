<script lang="ts">
  import type { MatrixChatSummary } from "$lib/chats/types";
  import type { FlatEntry } from "./types";
  import { buildRoomHierarchy } from "./hierarchy";
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

  const flatEntries = $derived.by<FlatEntry[]>(() => buildRoomHierarchy(rooms, expandedSpaceIds));

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
