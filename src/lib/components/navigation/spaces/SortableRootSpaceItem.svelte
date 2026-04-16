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

  const buttonClasses = (isSelected: boolean) =>
    `w-full flex items-center gap-2 rounded p-2 text-left cursor-grab active:cursor-grabbing transition-all ${
      isSelected ? "bg-primary-100-900" : "hover:bg-surface-200-800"
    }`;
</script>

<li class="list-none">
  <button
    type="button"
    class={buttonClasses(selected)}
    onclick={() => onSelectRootSpace?.(space.roomId)}
  >
    <RoomAvatar imageUrl={imageUrl} displayName={space.displayName} />

    <div class="min-w-0">
      <p class="font-medium truncate">{space.displayName}</p>
      <p class="text-xs text-surface-700-300">{space.joined ? "Joined" : "Unjoined"}</p>
    </div>
  </button>
</li>