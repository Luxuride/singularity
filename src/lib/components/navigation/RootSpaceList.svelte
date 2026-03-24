<script lang="ts">
  import type { MatrixChatSummary } from "$lib/chats/types";

  interface Props {
    spaces: MatrixChatSummary[];
    selectedRootSpaceId: string | null;
    onSelectRootSpace?: (spaceId: string) => void;
  }

  let { spaces, selectedRootSpaceId, onSelectRootSpace }: Props = $props();

  const sortedSpaces = $derived.by(() =>
    [...spaces].sort((a, b) => {
      if (a.roomId === "virtual:dms") {
        return -1;
      }
      if (b.roomId === "virtual:dms") {
        return 1;
      }

      return a.displayName.localeCompare(b.displayName, undefined, { sensitivity: "base" });
    }),
  );
</script>

<aside class="card p-2 preset-outlined-surface-200-800 bg-surface-100-900 flex flex-col flex-1 min-h-0 gap-3">
  <div class="min-h-0 flex-1 overflow-y-auto">
    {#if sortedSpaces.length === 0}
      <p class="px-2 text-xs text-surface-700-300">No known root spaces.</p>
    {:else}
      <ul class="space-y-1">
        {#each sortedSpaces as space (space.roomId)}
          <li>
            <button
              type="button"
              class="w-full text-left p-2 rounded transition-colors hover:bg-surface-200-800"
              class:bg-primary-100-900={space.roomId === selectedRootSpaceId}
              onclick={() => onSelectRootSpace?.(space.roomId)}
            >
              <p class="font-medium truncate">{space.displayName}</p>
              <p class="text-xs text-surface-700-300">{space.joined ? "Joined" : "Unjoined"}</p>
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </div>
</aside>
