<script lang="ts">
  import { dndzone, SHADOW_ITEM_MARKER_PROPERTY_NAME, type DndEvent } from "svelte-dnd-action";
  import { matrixGetRoomImage } from "$lib/chats/api";
  import type { MatrixChatSummary } from "$lib/chats/types";
  import { isVirtualRoomId, roomImageCache } from "../shared";
  import FixedRootSpaceItem from "./FixedRootSpaceItem.svelte";
  import SortableRootSpaceItem from "./SortableRootSpaceItem.svelte";

  interface Props {
    spaces: MatrixChatSummary[];
    selectedRootSpaceId: string | null;
    onSelectRootSpace?: (spaceId: string) => void;
    onReorderRootSpaces?: (rootSpaceIds: string[]) => Promise<void> | void;
  }

  interface RootSpaceDndItem {
    id: string;
    space: MatrixChatSummary | null;
    [SHADOW_ITEM_MARKER_PROPERTY_NAME]?: string;
  }

  let { spaces, selectedRootSpaceId, onSelectRootSpace, onReorderRootSpaces }: Props = $props();

  let draggableItems = $state<RootSpaceDndItem[]>([]);
  let lastPersistedOrder = $state<string[]>([]);
  let lazyImageUrlsByRoomId = $state<Record<string, string | null>>({});

  $effect(() => {
    const nextSpaces = spaces.filter((space) => !isVirtualRoomId(space.roomId));
    draggableItems = toDndItems(nextSpaces);
    lastPersistedOrder = nextSpaces.map((space) => space.roomId);
  });

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

  function toDndItems(items: MatrixChatSummary[]): RootSpaceDndItem[] {
    return items.map((space) => ({ id: space.roomId, space }));
  }

  function getPersistedOrder(items: RootSpaceDndItem[]): string[] {
    return items
      .filter((item) => !item[SHADOW_ITEM_MARKER_PROPERTY_NAME] && item.space !== null)
      .map((item) => item.id);
  }

  function getItemKey(item: RootSpaceDndItem): string {
    if (item[SHADOW_ITEM_MARKER_PROPERTY_NAME]) {
      return `${item.id}_${item[SHADOW_ITEM_MARKER_PROPERTY_NAME]}`;
    }
    return item.id;
  }

  async function persistRootSpaceOrder(nextIds: string[], previousIds: string[]): Promise<void> {
    try {
      await onReorderRootSpaces?.(nextIds);
      lastPersistedOrder = nextIds;
    } catch (error) {
      console.error("Failed to reorder root spaces:", error);
      
      // Rollback to previous order
      const byId = new Map(
        draggableItems
          .filter((item) => item.space !== null)
          .map((item) => [item.id, item.space as MatrixChatSummary]),
      );
      draggableItems = previousIds
        .map((roomId) => byId.get(roomId))
        .filter((space): space is MatrixChatSummary => space != null)
        .map((space) => ({ id: space.roomId, space }));
    }
  }

  function handleDndConsider(event: CustomEvent<DndEvent<RootSpaceDndItem>>): void {
    draggableItems = event.detail.items;
  }

  function handleDndFinalize(event: CustomEvent<DndEvent<RootSpaceDndItem>>): void {
    draggableItems = event.detail.items;

    const nextIds = getPersistedOrder(event.detail.items);
    const previousIds = [...lastPersistedOrder];

    void persistRootSpaceOrder(nextIds, previousIds);
  }
</script>

<aside class="card p-2 preset-outlined-surface-200-800 bg-surface-100-900 flex flex-col flex-1 min-h-0 gap-3">
  <div class="min-h-0 flex-1 overflow-y-auto">
    {#if spaces.length === 0}
      <p class="px-2 text-xs text-surface-700-300">No known root spaces.</p>
    {:else}
      <ul class="space-y-1">
        {#each spaces.filter((space) => isVirtualRoomId(space.roomId)) as space (space.roomId)}
          <FixedRootSpaceItem
            space={space}
            selected={space.roomId === selectedRootSpaceId}
            onSelectRootSpace={onSelectRootSpace}
            imageUrl={lazyImageUrlsByRoomId[space.roomId] ?? null}
          />
        {/each}
      </ul>

      <ul
        class="space-y-1"
        use:dndzone={{
          items: draggableItems,
          flipDurationMs: 150,
          dropFromOthersDisabled: true,
        }}
        onconsider={handleDndConsider}
        onfinalize={handleDndFinalize}
      >
        {#each draggableItems as item (getItemKey(item))}
          {#if item[SHADOW_ITEM_MARKER_PROPERTY_NAME]}
            <li class="list-none h-12 rounded border border-dashed border-surface-400-600 opacity-40"></li>
          {:else if item.space}
            <SortableRootSpaceItem
              space={item.space}
              selected={item.space.roomId === selectedRootSpaceId}
              onSelectRootSpace={onSelectRootSpace}
              imageUrl={lazyImageUrlsByRoomId[item.space.roomId] ?? null}
            />
          {/if}
        {/each}
      </ul>
    {/if}
  </div>
</aside>
