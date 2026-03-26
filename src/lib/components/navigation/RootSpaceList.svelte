<script lang="ts">
  import type { MatrixChatSummary } from "$lib/chats/types";

  interface Props {
    spaces: MatrixChatSummary[];
    selectedRootSpaceId: string | null;
    onSelectRootSpace?: (spaceId: string) => void;
  }

  let { spaces, selectedRootSpaceId, onSelectRootSpace }: Props = $props();

  const VIRTUAL_DMS_ROOT_ID = "virtual:dms";
  const VIRTUAL_UNSPACED_ROOT_ID = "virtual:unspaced";

  const sortedSpaces = $derived.by(() =>
    [...spaces].sort((a, b) => {
      const rank = (spaceId: string) => {
        if (spaceId === VIRTUAL_DMS_ROOT_ID) {
          return 0;
        }
        if (spaceId === VIRTUAL_UNSPACED_ROOT_ID) {
          return 1;
        }

        return 2;
      };

      const rankDiff = rank(a.roomId) - rank(b.roomId);
      if (rankDiff !== 0) {
        return rankDiff;
      }

      return a.displayName.localeCompare(b.displayName, undefined, { sensitivity: "base" });
    }),
  );

  function avatarLabelFor(name: string): string {
    return name.trim().charAt(0).toUpperCase() || "#";
  }
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
              <div class="flex items-start gap-2 min-w-0">
                <div
                  class="mt-0.5 h-8 w-8 shrink-0 rounded-full bg-surface-200-800 overflow-hidden grid place-items-center text-xs font-semibold text-surface-800-200"
                >
                  {#if space.imageUrl}
                    <img src={space.imageUrl} alt="" class="h-full w-full object-cover" loading="lazy" />
                  {:else}
                    <span>{avatarLabelFor(space.displayName)}</span>
                  {/if}
                </div>

                <div class="min-w-0">
                  <p class="font-medium truncate">{space.displayName}</p>
                  <p class="text-xs text-surface-700-300">{space.joined ? "Joined" : "Unjoined"}</p>
                </div>
              </div>
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </div>
</aside>
