<script lang="ts">
  import { matrixGetRoomImage } from "$lib/chats/api";
  import type { MatrixChatSummary, MatrixJoinTargetPreview } from "$lib/chats/types";
  import type { FlatEntry } from "./types";
  import { buildRoomHierarchy } from "./hierarchy";
  import { RoomAvatar, isVirtualRoomId, roomImageCache } from "../shared";

  interface JoinTargetsRequest {
    title: string;
    confirmLabel?: string;
    targets: MatrixJoinTargetPreview[];
  }

  interface Props {
    rooms: MatrixChatSummary[];
    selectedRoomId: string | null;
    loadingSpaceIds?: string[];
    onSelectRoom?: (roomId: string) => void;
    onJoinTargets?: (request: JoinTargetsRequest) => void;
    onExpandSpace?: (spaceId: string, expanded: boolean) => Promise<void> | void;
    emptyMessage?: string;
  }

  let {
    rooms,
    selectedRoomId,
    loadingSpaceIds = [],
    onSelectRoom,
    onJoinTargets,
    onExpandSpace,
    emptyMessage = "No spaces or rooms found under this root space.",
  }: Props = $props();

  let expandedSpaceIds = $state<Record<string, boolean>>({});
  let lazyImageUrlsByRoomId = $state<Record<string, string | null>>({});
  const roomsById = $derived.by(() => new Map(rooms.map((room) => [room.roomId, room])));

  $effect(() => {
    const nextExpanded = { ...expandedSpaceIds };
    let changed = false;

    if (rooms.length > 0) {
      const rootRoom = rooms[0];
      if (rootRoom.kind === "space" && nextExpanded[rootRoom.roomId] === undefined) {
        nextExpanded[rootRoom.roomId] = true;
        changed = true;
      }
    }

    for (const room of rooms) {
      if (room.kind !== "space" || nextExpanded[room.roomId] !== undefined) {
        continue;
      }

      nextExpanded[room.roomId] = false;
      changed = true;
    }

    if (changed) {
      expandedSpaceIds = nextExpanded;
    }
  });

  $effect(() => {
    const nextImages: Record<string, string | null> = { ...lazyImageUrlsByRoomId };
    let changed = false;

    for (const room of rooms) {
      if (room.imageUrl !== null) {
        if (nextImages[room.roomId] !== room.imageUrl) {
          nextImages[room.roomId] = room.imageUrl;
          changed = true;
        }
        roomImageCache.prime(room.roomId, room.imageUrl);
        continue;
      }

      if (nextImages[room.roomId] !== undefined) {
        continue;
      }

      const cachedImage = roomImageCache.getCached(room.roomId);
      if (cachedImage !== undefined) {
        if (nextImages[room.roomId] !== cachedImage) {
          nextImages[room.roomId] = cachedImage;
          changed = true;
        }
        continue;
      }

      if (isVirtualRoomId(room.roomId)) {
        if (nextImages[room.roomId] !== null) {
          nextImages[room.roomId] = null;
          changed = true;
        }
        continue;
      }

      const roomId = room.roomId;
      void roomImageCache
        .getOrLoad(room.roomId, () => matrixGetRoomImage(room.roomId))
        .then((imageUrl) => {
          if (!roomsById.has(roomId)) {
            return;
          }

          if (lazyImageUrlsByRoomId[roomId] === imageUrl) {
            return;
          }

          lazyImageUrlsByRoomId = {
            ...lazyImageUrlsByRoomId,
            [roomId]: imageUrl,
          };
        });
    }

    if (changed) {
      lazyImageUrlsByRoomId = nextImages;
    }
  });

  const flatEntries = $derived.by<FlatEntry[]>(() => buildRoomHierarchy(rooms, expandedSpaceIds));

  function requestJoin(room: MatrixChatSummary) {
    onJoinTargets?.({
      title: `Join ${room.displayName}`,
      confirmLabel: "Join",
      targets: [{
        roomIdOrAlias: room.roomId,
        displayName: room.displayName,
        kind: room.kind,
        serverNames: deriveServerNames(room.roomId),
      }],
    });
  }

  function collectJoinAllTargets(spaceId: string): MatrixJoinTargetPreview[] {
    const targets = new Map<string, MatrixJoinTargetPreview>();
    const seen = new Set<string>();
    const stack = [spaceId];

    while (stack.length > 0) {
      const roomId = stack.pop();
      if (!roomId || seen.has(roomId)) {
        continue;
      }

      seen.add(roomId);

      const room = roomsById.get(roomId);
      if (!room) {
        continue;
      }

      if (!room.joined) {
        targets.set(room.roomId, {
          roomIdOrAlias: room.roomId,
          displayName: room.displayName,
          kind: room.kind,
          serverNames: deriveServerNames(room.roomId),
        });
      }

      for (const childRoomId of room.childrenRoomIds ?? []) {
        stack.push(childRoomId);
      }
    }

    return [...targets.values()];
  }

  function joinAccessLabel(room: MatrixChatSummary): string | null {
    if (room.joined) {
      return null;
    }

    if (room.kind === "space" && room.joinRule === "restricted") {
      return "Join access · Space members";
    }

    switch (room.joinRule) {
      case "public":
        return "Join access · Public";
      case "invite":
        return "Join access · Invite only";
      case "knock":
      case "knock_restricted":
        return "Join access · Knock";
      case "private":
        return "Join access · Private";
      case "restricted":
        return room.kind === "space" ? "Join access · Space members" : "Join access · Restricted";
      default:
        return "Join access · Unknown";
    }
  }

  function toggleSpace(spaceId: string) {
    const expanded = !(expandedSpaceIds[spaceId] ?? false);
    expandedSpaceIds = {
      ...expandedSpaceIds,
      [spaceId]: expanded,
    };

    void onExpandSpace?.(spaceId, expanded);
  }

  function handleSelect(entry: FlatEntry) {
    const room = entry.room;
    if (room.kind === "space") {
      if (!room.joined) {
        requestJoin(room);
        return;
      }

      if (entry.hasChildren) {
        toggleSpace(room.roomId);
      }
      return;
    }

    if (room.joined) {
      onSelectRoom?.(room.roomId);
      return;
    }

    requestJoin(room);
  }

  function isLoadingSpace(spaceId: string): boolean {
    return loadingSpaceIds.includes(spaceId);
  }

  function getRoomImage(roomId: string): string | null {
    return lazyImageUrlsByRoomId[roomId] ?? null;
  }

  function deriveServerNames(roomIdOrAlias: string): string[] {
    const target = roomIdOrAlias.trim();
    const serverName = target.includes(":") ? target.slice(target.indexOf(":") + 1) : "";

    return serverName ? [serverName] : [];
  }
</script>

<aside class="card p-2 preset-outlined-surface-200-800 bg-surface-100-900 flex flex-col flex-1 min-h-0 gap-3">
  <div class="min-h-0 flex-1 overflow-y-auto">
    {#if rooms.length === 0}
      <p class="p-2 text-sm text-surface-700-300">{emptyMessage}</p>
    {:else}
      <ul class="space-y-1">
        {#each flatEntries as entry (entry.key)}
          {@const room = entry.room}
          {@const isSpace = room.kind === "space"}
          {@const isExpanded = expandedSpaceIds[room.roomId] ?? false}
          {@const indentation = `${Math.max(0, entry.depth) * 0.9}rem`}

          <li>
            <div class="flex items-stretch gap-2">
              <button
                type="button"
                class="w-full text-left p-2 rounded transition-colors"
                class:hover:bg-surface-200-800={!isSpace || entry.hasChildren || !room.joined}
                class:opacity-90={isSpace}
                class:cursor-pointer={!isSpace || entry.hasChildren || !room.joined}
                class:cursor-default={isSpace && !entry.hasChildren && room.joined}
                class:bg-primary-100-900={room.roomId === selectedRoomId}
                style={`padding-left: calc(0.5rem + ${indentation});`}
                onclick={() => handleSelect(entry)}
              >
                <div class="flex items-start justify-between gap-2">
                  <div class="flex items-start gap-2 min-w-0">
                    <RoomAvatar imageUrl={getRoomImage(room.roomId)} displayName={room.displayName} />

                    <p class="font-medium truncate">
                      {#if isSpace}
                        {isExpanded ? "▼" : "▶"}
                      {/if}
                      {room.displayName}
                    </p>
                  </div>

                  <div class="flex items-center gap-2 text-[10px] uppercase tracking-wide text-surface-700-300">
                    {#if isSpace && isLoadingSpace(room.roomId)}
                      <span>Loading</span>
                    {/if}
                    <span>{isSpace ? "Space" : "Room"}</span>
                  </div>
                </div>

                {#if room.joined}
                  <p class="text-xs text-surface-700-300">
                    {room.encrypted ? "Encrypted" : "Unencrypted"} • {room.joinedMembers} members
                  </p>
                {:else}
                  <p class="text-xs text-surface-700-300">
                    {joinAccessLabel(room) ?? "Join access · Unknown"}
                  </p>
                {/if}
              </button>

              {#if !room.joined}
                <button
                  type="button"
                  class="btn preset-filled-primary-500 px-3 py-1 text-xs whitespace-nowrap"
                  onclick={() =>
                    isSpace
                      ? requestJoin(room)
                      : onJoinTargets?.({
                          title: `Join ${room.displayName}`,
                          confirmLabel: "Join",
                          targets: [{
                            roomIdOrAlias: room.roomId,
                            displayName: room.displayName,
                            kind: room.kind,
                            serverNames: deriveServerNames(room.roomId),
                          }],
                        })}
                >
                  {isSpace ? "Join space" : "Join"}
                </button>
              {:else if isSpace}
                {@const joinAllTargets = collectJoinAllTargets(room.roomId)}
                {#if joinAllTargets.length > 0}
                  <button
                    type="button"
                    class="btn preset-filled-primary-500 px-3 py-1 text-xs whitespace-nowrap"
                    onclick={() =>
                      onJoinTargets?.({
                        title: `Join all under ${room.displayName}`,
                        confirmLabel: "Join all",
                        targets: joinAllTargets,
                      })}
                  >
                    Join all
                  </button>
                {/if}
              {/if}
            </div>
          </li>
        {/each}
      </ul>
    {/if}
  </div>
</aside>