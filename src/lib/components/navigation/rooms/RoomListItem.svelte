<script lang="ts">
  import { matrixGetRoomImage } from "$lib/chats/api";
  import type { MatrixChatSummary } from "$lib/chats/types";
  import { RoomAvatar, isVirtualRoomId, roomImageCache } from "../shared";

  interface Props {
    room: MatrixChatSummary;
    depth: number;
    isSelected: boolean;
    hasChildren: boolean;
    isExpanded: boolean;
    onSelect?: (roomId: string) => void;
    onToggleSpace?: (roomId: string) => void;
    onJoin?: (roomId: string) => void;
    onContextMenu?: (room: MatrixChatSummary, event: MouseEvent) => void;
    joinAllRoomIds?: string[];
    onJoinAllChildren?: (roomId: string, roomIds: string[]) => void;
    joinAllDisabled?: boolean;
  }

  let {
    room,
    depth,
    isSelected,
    hasChildren,
    isExpanded,
    onSelect,
    onToggleSpace,
    onJoin,
    onContextMenu,
    joinAllRoomIds = [],
    onJoinAllChildren,
    joinAllDisabled = false,
  }: Props = $props();

  const isSpace = $derived(room.kind === "space");
  const canJoin = $derived(
    !!onJoin && !room.joined && !isVirtualRoomId(room.roomId),
  );
  const canJoinAllChildren = $derived(
    !!onJoinAllChildren &&
      isSpace &&
      room.joined &&
      joinAllRoomIds.length > 0 &&
      !isVirtualRoomId(room.roomId),
  );
  let lazyImageUrl = $state<string | null>(null);

  const indentation = $derived(`${Math.max(0, depth) * 0.9}rem`);

  $effect(() => {
    lazyImageUrl = room.imageUrl;

    if (room.imageUrl !== null) {
      roomImageCache.prime(room.roomId, room.imageUrl);
    }
  });

  $effect(() => {
    if (lazyImageUrl || isVirtualRoomId(room.roomId)) {
      return;
    }

    const cachedImage = roomImageCache.getCached(room.roomId);
    if (cachedImage !== undefined) {
      lazyImageUrl = cachedImage;
      return;
    }

    const roomId = room.roomId;
    void roomImageCache
      .getOrLoad(room.roomId, () => matrixGetRoomImage(room.roomId))
      .then((imageUrl) => {
        if (room.roomId === roomId) {
          lazyImageUrl = imageUrl;
        }
      });
  });

  function handleClick() {
    if (isSpace) {
      if (!room.joined) {
        onJoin?.(room.roomId);
        return;
      }

      if (hasChildren) {
        onToggleSpace?.(room.roomId);
      }
      return;
    }

    if (!room.joined) {
      return;
    }

    onSelect?.(room.roomId);
  }

  function handleJoin(event: MouseEvent) {
    event.stopPropagation();
    onJoin?.(room.roomId);
  }

  function handleJoinAllChildren(event: MouseEvent) {
    event.stopPropagation();
    if (joinAllRoomIds.length === 0) {
      return;
    }

    onJoinAllChildren?.(room.roomId, joinAllRoomIds);
  }

  function handleContextMenu(event: MouseEvent) {
    onContextMenu?.(room, event);
  }
</script>

<li>
  <div class="flex items-stretch gap-2">
    <button
      type="button"
      class="flex-1 min-w-0 text-left p-2 rounded transition-colors hover:bg-surface-200-800"
      class:opacity-90={isSpace}
      class:cursor-pointer={(!isSpace && room.joined) || (isSpace && (hasChildren || !room.joined))}
      class:cursor-default={!((!isSpace && room.joined) || (isSpace && (hasChildren || !room.joined)))}
      class:bg-primary-100-900={isSelected}
      style={`padding-left: calc(0.5rem + ${indentation});`}
      onclick={handleClick}
      oncontextmenu={handleContextMenu}
    >
      <div class="flex items-start justify-between gap-2">
        <div class="flex items-start gap-2 min-w-0">
          <RoomAvatar imageUrl={lazyImageUrl} displayName={room.displayName} />

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

    <div class="flex flex-col gap-1 self-center">
      {#if canJoinAllChildren}
        <button
          type="button"
          class="btn preset-tonal text-[11px]"
          disabled={joinAllDisabled}
          onclick={handleJoinAllChildren}
        >
          Join all
        </button>
      {/if}

      {#if canJoin}
        <button
          type="button"
          class="btn preset-tonal text-xs"
          onclick={handleJoin}
        >
          Join
        </button>
      {/if}
    </div>
  </div>
</li>
