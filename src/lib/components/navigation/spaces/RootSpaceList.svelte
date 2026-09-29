<script lang="ts">
  import { dndzone, SHADOW_ITEM_MARKER_PROPERTY_NAME, type DndEvent } from "svelte-dnd-action";
  import { matrixGetRoomImage } from "$lib/chats/api";
  import type { MatrixChatSummary } from "$lib/chats/types";
  import { isVirtualRoomId, roomImageCache } from "../shared";
  import FixedRootSpaceItem from "./FixedRootSpaceItem.svelte";
  import SortableRootSpaceItem from "./SortableRootSpaceItem.svelte";
  import JoinRoomDialog from "./JoinRoomDialog.svelte";

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
  let joinRoomDialogOpen = $state(false);
  let isDragging = $state(false);
  let persistQueue: Promise<void> = Promise.resolve();

  $effect(() => {
    const nextSpaces = spaces.filter((space) => !isVirtualRoomId(space.roomId));
    const nextById = new Map(nextSpaces.map((space) => [space.roomId, space]));

    // A realtime navigation refresh lands mid-drag and would otherwise snap the
    // list back under the pointer, so the local order wins until the drag ends.
    if (isDragging) {
      draggableItems = draggableItems
        .map((item) => {
          const space = nextById.get(item.id);
          return space ? { id: item.id, space } : item;
        })
        .filter((item) => nextById.has(item.id) || item[SHADOW_ITEM_MARKER_PROPERTY_NAME] !== undefined);
      for (const space of nextSpaces) {
        if (!draggableItems.some((item) => item.id === space.roomId)) {
          draggableItems = [...draggableItems, { id: space.roomId, space }];
        }
      }
      return;
    }

    draggableItems = toDndItems(nextSpaces);
    lastPersistedOrder = nextSpaces.map((space) => space.roomId);
  });

  $effect(() => {
    const known: Record<string, string | null> = {};

    for (const space of spaces) {
      if (space.imageUrl) {
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

  /// Reorders run one at a time and in the order the user made them: two
  /// overlapping saves that resolve out of order leave the sidebar showing the
  /// older of the two orders while the stored one is the newer.
  function persistRootSpaceOrder(nextIds: string[], previousIds: string[]): Promise<void> {
    persistQueue = persistQueue
      .then(() => onReorderRootSpaces?.(nextIds))
      .then(() => {
        lastPersistedOrder = nextIds;
      })
      .catch((error) => {
        console.error("Failed to reorder root spaces:", error);
        rollbackRootSpaceOrder(previousIds);
      });

    return persistQueue;
  }

  /// Restores the pre-drag order, keeping any space that only the local list
  /// knew about. Dropping those would silently remove a space from the sidebar
  /// until the next full refresh.
  function rollbackRootSpaceOrder(previousIds: string[]): void {
    const byId = new Map(
      draggableItems
        .filter((item) => item.space !== null)
        .map((item) => [item.id, item.space as MatrixChatSummary]),
    );
    const restored = previousIds
      .map((roomId) => byId.get(roomId))
      .filter((space): space is MatrixChatSummary => space != null)
      .map((space) => ({ id: space.roomId, space }));

    for (const item of draggableItems) {
      if (item.space !== null && !previousIds.includes(item.id)) {
        restored.push({ id: item.id, space: item.space });
      }
    }

    draggableItems = restored;
  }

  function handleDndConsider(event: CustomEvent<DndEvent<RootSpaceDndItem>>): void {
    isDragging = true;
    draggableItems = event.detail.items;
  }

  function handleDndFinalize(event: CustomEvent<DndEvent<RootSpaceDndItem>>): void {
    draggableItems = event.detail.items;

    const nextIds = getPersistedOrder(event.detail.items);
    const previousIds = [...lastPersistedOrder];
    isDragging = false;

    if (nextIds.length === previousIds.length && nextIds.every((id, index) => id === previousIds[index])) {
      return;
    }

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
      
      <button
        class="mt-2 mx-[1px] flex h-12 w-[calc(100%-2px)] items-center justify-center rounded border border-dashed border-surface-400-600 text-surface-500 transition-colors hover:bg-surface-200-800 hover:text-surface-900-50"
        title="Join room or space"
        onclick={() => (joinRoomDialogOpen = true)}
        aria-label="Join Room"
      >
        <span class="text-xl">+</span>
      </button>
    {/if}
  </div>
</aside>

<JoinRoomDialog
  open={joinRoomDialogOpen}
  onClose={() => (joinRoomDialogOpen = false)}
/>
