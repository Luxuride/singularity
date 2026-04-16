<script lang="ts">
  import type { MatrixChatSummary } from "$lib/chats/types";
  import { RoomAvatar } from "../shared";

  interface Props {
    space: MatrixChatSummary;
    selected: boolean;
    imageUrl: string | null;
    onSelectRootSpace?: (spaceId: string) => void;
  }

  let { space, selected, imageUrl, onSelectRootSpace }: Props = $props();
</script>

<li class="list-none">
  <div
    class={`flex items-stretch gap-2 rounded transition-colors ${selected ? "bg-primary-100-900" : "hover:bg-surface-200-800"}`}
  >
    <button
      type="button"
      class="flex min-w-0 flex-1 items-center gap-2 rounded p-2 text-left"
      onclick={() => onSelectRootSpace?.(space.roomId)}
    >
      <RoomAvatar imageUrl={imageUrl} displayName={space.displayName} />

      <div class="min-w-0">
        <p class="font-medium truncate">{space.displayName}</p>
        <p class="text-xs text-surface-700-300">{space.joined ? "Joined" : "Unjoined"}</p>
      </div>
    </button>
  </div>
</li>