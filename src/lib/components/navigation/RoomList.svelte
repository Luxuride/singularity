<script lang="ts">
  import type { MatrixChatSummary } from "$lib/chats/types";
  import RoomListItem from "./RoomListItem.svelte";

  interface Props {
    rooms: MatrixChatSummary[];
    selectedRoomId: string | null;
    onSelectRoom?: (roomId: string) => void;
  }

  let { rooms, selectedRoomId, onSelectRoom }: Props = $props();
</script>

<aside class="card p-2 preset-outlined-surface-200-800 bg-surface-100-900 max-h-[70vh] overflow-y-auto">
  {#if rooms.length === 0}
    <p class="p-2 text-sm text-surface-700-300">No joined rooms found.</p>
  {:else}
    <ul class="space-y-1">
      {#each rooms as room (room.roomId)}
        <RoomListItem
          {room}
          isSelected={room.roomId === selectedRoomId}
          onSelect={onSelectRoom}
        />
      {/each}
    </ul>
  {/if}
</aside>
