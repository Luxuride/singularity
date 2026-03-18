<script lang="ts">
  import type { MatrixChatSummary } from "$lib/chats/types";

  interface Props {
    room: MatrixChatSummary;
    isSelected: boolean;
    onSelect?: (roomId: string) => void;
  }

  let { room, isSelected, onSelect }: Props = $props();
</script>

<li>
  <button
    type="button"
    class="w-full text-left p-2 rounded hover:bg-surface-200-800 transition-colors"
    class:bg-primary-100-900={isSelected}
    onclick={() => onSelect?.(room.roomId)}
  >
    <p class="font-medium truncate">{room.displayName}</p>
    <p class="text-xs text-surface-700-300">
      {room.encrypted ? "Encrypted" : "Unencrypted"} • {room.joinedMembers} members
    </p>
  </button>
</li>
