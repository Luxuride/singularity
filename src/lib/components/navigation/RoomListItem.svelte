<script lang="ts">
  import { matrixGetRoomImage } from "$lib/chats/api";
  import type { MatrixChatSummary } from "$lib/chats/types";

  const roomImageCache = new Map<string, string | null>();
  const roomImageInFlight = new Map<string, Promise<string | null>>();

  interface Props {
    room: MatrixChatSummary;
    depth: number;
    isSelected: boolean;
    hasChildren: boolean;
    isExpanded: boolean;
    onSelect?: (roomId: string) => void;
    onToggleSpace?: (roomId: string) => void;
  }

  let {
    room,
    depth,
    isSelected,
    hasChildren,
    isExpanded,
    onSelect,
    onToggleSpace,
  }: Props = $props();

  const isSpace = $derived(room.kind === "space");
  let lazyImageUrl = $state<string | null>(null);

  const indentation = $derived(`${Math.max(0, depth) * 0.9}rem`);

  const avatarLabel = $derived(room.displayName.trim().charAt(0).toUpperCase() || "#");

  $effect(() => {
    lazyImageUrl = room.imageUrl;

    if (room.imageUrl !== null) {
      roomImageCache.set(room.roomId, room.imageUrl);
    }
  });

  $effect(() => {
    if (lazyImageUrl || room.roomId.startsWith("virtual:")) {
      return;
    }

    const cachedImage = roomImageCache.get(room.roomId);
    if (cachedImage !== undefined) {
      lazyImageUrl = cachedImage;
      return;
    }

    const inFlight = roomImageInFlight.get(room.roomId);
    if (inFlight) {
      const roomId = room.roomId;
      void inFlight.then((imageUrl) => {
        roomImageCache.set(roomId, imageUrl);
        if (room.roomId === roomId) {
          lazyImageUrl = imageUrl;
        }
      });
      return;
    }

    const roomId = room.roomId;
    const request = matrixGetRoomImage(room.roomId)
      .then((imageUrl) => {
        roomImageCache.set(roomId, imageUrl);
        return imageUrl;
      })
      .catch(() => null)
      .finally(() => {
        roomImageInFlight.delete(roomId);
      });

    roomImageInFlight.set(roomId, request);

    void request.then((imageUrl) => {
      if (room.roomId === roomId) {
        lazyImageUrl = imageUrl;
      }
    });
  });

  function handleClick() {
    if (isSpace) {
      if (hasChildren) {
        onToggleSpace?.(room.roomId);
      }
      return;
    }

    onSelect?.(room.roomId);
  }
</script>

<li>
  <button
    type="button"
    class="w-full text-left p-2 rounded transition-colors"
    class:hover:bg-surface-200-800={!isSpace}
    class:opacity-90={isSpace}
    class:cursor-pointer={!isSpace || hasChildren}
    class:cursor-default={isSpace && !hasChildren}
    class:bg-primary-100-900={isSelected}
    style={`padding-left: calc(0.5rem + ${indentation});`}
    onclick={handleClick}
  >
    <div class="flex items-start justify-between gap-2">
      <div class="flex items-start gap-2 min-w-0">
        <div
          class="mt-0.5 h-8 w-8 shrink-0 rounded-full bg-surface-200-800 overflow-hidden grid place-items-center text-xs font-semibold text-surface-800-200"
        >
          {#if lazyImageUrl}
            <img src={lazyImageUrl} alt="" class="h-full w-full object-cover" />
          {:else}
            <span>{avatarLabel}</span>
          {/if}
        </div>

        <p class="font-medium truncate">
          {#if isSpace}
            {isExpanded ? "▼" : "▶"}
          {/if}
          {room.displayName}
        </p>
      </div>

      {#if room.kind === "space"}
        <span class="text-[10px] uppercase tracking-wide text-surface-700-300">Space</span>
      {/if}
    </div>

    {#if room.kind === "space"}
      <p class="text-xs text-surface-700-300">
        {hasChildren ? "Contains rooms" : "No child rooms"}
      </p>
    {:else}
      <p class="text-xs text-surface-700-300">
        {room.encrypted ? "Encrypted" : "Unencrypted"} • {room.joinedMembers} members
      </p>
    {/if}
  </button>
</li>
