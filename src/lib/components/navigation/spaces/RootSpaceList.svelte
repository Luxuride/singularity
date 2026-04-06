<script lang="ts">
  import { matrixGetRoomImage } from "$lib/chats/api";
  import type { MatrixChatSummary } from "$lib/chats/types";
  import { sortRootSpaces } from "./sorting";
  import { RoomAvatar, isVirtualRoomId, roomImageCache } from "../shared";

  interface Props {
    spaces: MatrixChatSummary[];
    selectedRootSpaceId: string | null;
    onSelectRootSpace?: (spaceId: string) => void;
  }

  let { spaces, selectedRootSpaceId, onSelectRootSpace }: Props = $props();

  let lazyImageUrlsByRoomId = $state<Record<string, string | null>>({});

  const sortedSpaces = $derived.by(() => sortRootSpaces(spaces));

  $effect(() => {
    const known: Record<string, string | null> = {};

    for (const space of spaces) {
      if (space.imageUrl !== null) {
        known[space.roomId] = space.imageUrl;
        roomImageCache.prime(space.roomId, space.imageUrl);
        continue;
      }

      const cached = roomImageCache.getCached(space.roomId);
      if (cached !== undefined) {
        known[space.roomId] = cached;
      }
    }

    lazyImageUrlsByRoomId = known;
  });

  $effect(() => {
    for (const space of spaces) {
      if (space.imageUrl || isVirtualRoomId(space.roomId)) {
        continue;
      }

      const roomId = space.roomId;
      void roomImageCache
        .getOrLoad(roomId, () => matrixGetRoomImage(roomId))
        .then((imageUrl) => {
          if (!spaces.some((candidate) => candidate.roomId === roomId)) {
            return;
          }

          lazyImageUrlsByRoomId = {
            ...lazyImageUrlsByRoomId,
            [roomId]: imageUrl,
          };
        });
    }
  });
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
                <RoomAvatar
                  imageUrl={lazyImageUrlsByRoomId[space.roomId] ?? null}
                  displayName={space.displayName}
                />

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
