<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { onMount } from "svelte";
  import { get } from "svelte/store";

  import { matrixLogout, matrixRecoveryStatus, matrixSessionStatus } from "$lib/auth/api";
  import { matrixGetChats, matrixTriggerRoomUpdate } from "$lib/chats/api";
  import { subscribeToRoomUpdates } from "$lib/chats/realtime";
  import {
    shellChats,
    shellCurrentUserId,
    shellErrorMessage,
    shellLoading,
    shellRecoveryState,
    shellRefreshing,
    shellSelectedRootSpaceId,
    shellSelectedRoomId,
  } from "$lib/chats/shell";
  import type {
    MatrixChatSummary,
    MatrixRoomRemovedEvent,
    MatrixSelectedRoomMessagesEvent,
  } from "$lib/chats/types";
  import AppHeader from "$lib/components/navigation/AppHeader.svelte";
  import RootSpaceList from "$lib/components/navigation/RootSpaceList.svelte";
  import RoomList from "$lib/components/navigation/RoomList.svelte";

  const VIRTUAL_DMS_ROOT_ID = "virtual:dms";

  let { children } = $props();

  let checkingAuth = $state(true);
  let loggingOut = $state(false);

  onMount(() => {
    let unlisten = () => {};

    void (async () => {
      unlisten = await subscribeToRoomUpdates({
        onRoomAdded: applyRoomUpsert,
        onRoomUpdated: applyRoomUpsert,
        onRoomRemoved: applyRoomRemoval,
        onSelectedRoomMessages: (_payload: MatrixSelectedRoomMessagesEvent) => {},
        onChatMessagesStream: () => {},
      });

      await loadShell();
      await requestRefresh();
      checkingAuth = false;
    })();

    return () => {
      unlisten();
    };
  });

  function applyRoomUpsert(chat: MatrixChatSummary) {
    shellChats.update((currentChats) => {
      const index = currentChats.findIndex((candidate) => candidate.roomId === chat.roomId);
      if (index >= 0) {
        return currentChats.map((candidate, candidateIndex) =>
          candidateIndex === index ? chat : candidate,
        );
      }

      return [...currentChats, chat];
    });

    const rootSpaceId = get(shellSelectedRootSpaceId);
    if (rootSpaceId && !isRootSpace(rootSpaceId, get(shellChats))) {
      shellSelectedRootSpaceId.set("");
      void syncRoute("", get(shellSelectedRoomId));
    }
  }

  function applyRoomRemoval(payload: MatrixRoomRemovedEvent) {
    shellChats.update((currentChats) =>
      currentChats.filter((chat) => chat.roomId !== payload.roomId),
    );

    if (get(shellSelectedRoomId) === payload.roomId) {
      shellSelectedRoomId.set("");
      void syncRoute(get(shellSelectedRootSpaceId), "");
    }
  }

  async function loadShell() {
    shellLoading.set(true);
    shellErrorMessage.set("");

    try {
      const session = await matrixSessionStatus();
      if (!session.authenticated) {
        await goto("/");
        return;
      }

      shellCurrentUserId.set(session.userId ?? "");

      try {
        const recovery = await matrixRecoveryStatus();
        shellRecoveryState.set(recovery.state);
      } catch {
        shellRecoveryState.set(null);
      }

      const rooms = await matrixGetChats();
      shellChats.set(rooms);

      const queryRootSpaceId = page.url.searchParams.get("rootSpaceId") ?? "";
      const queryRoomId = page.url.searchParams.get("roomId") ?? "";
      const selectedRoomId = chooseInitialRoom(rooms, queryRoomId);
      const selectedRootSpaceId = chooseInitialRootSpace(rooms, queryRootSpaceId, selectedRoomId);

      shellSelectedRootSpaceId.set(selectedRootSpaceId);
      shellSelectedRoomId.set(selectedRoomId);
      await syncRoute(selectedRootSpaceId, selectedRoomId);
    } catch (error) {
      shellErrorMessage.set(error instanceof Error ? error.message : "Failed to load chats");
    } finally {
      shellLoading.set(false);
    }
  }

  function chooseInitialRoom(rooms: MatrixChatSummary[], roomIdFromQuery: string): string {
    const hasQueryRoom = rooms.some(
      (room) => room.roomId === roomIdFromQuery && room.kind === "room",
    );
    if (hasQueryRoom) {
      return roomIdFromQuery;
    }

    return "";
  }

  function chooseInitialRootSpace(
    rooms: MatrixChatSummary[],
    rootSpaceIdFromQuery: string,
    selectedRoomId: string,
  ): string {
    if (rootSpaceIdFromQuery && isRootSpace(rootSpaceIdFromQuery, rooms)) {
      return rootSpaceIdFromQuery;
    }

    if (selectedRoomId) {
      return deriveRootSpaceIdForRoom(selectedRoomId, rooms) ?? "";
    }

    return "";
  }

  async function requestRefresh() {
    if (get(shellRefreshing)) {
      return;
    }

    shellRefreshing.set(true);
    shellErrorMessage.set("");

    try {
      await matrixTriggerRoomUpdate();
    } catch (error) {
      shellErrorMessage.set(
        error instanceof Error ? error.message : "Failed to trigger room refresh",
      );
    } finally {
      shellRefreshing.set(false);
    }
  }

  async function syncRoute(rootSpaceId: string, roomId: string) {
    const params = new URLSearchParams(page.url.searchParams);
    if (rootSpaceId) {
      params.set("rootSpaceId", rootSpaceId);
    } else {
      params.delete("rootSpaceId");
    }

    if (roomId) {
      params.set("roomId", roomId);
    } else {
      params.delete("roomId");
    }

    const search = params.toString();
    const path = search ? `/chats?${search}` : "/chats";
    await goto(path, { replaceState: true, noScroll: true, keepFocus: true });
  }

  function selectRootSpace(rootSpaceId: string) {
    if (!isRootSpace(rootSpaceId, get(shellChats))) {
      return;
    }

    shellSelectedRootSpaceId.set(rootSpaceId);
    shellSelectedRoomId.set("");
    void syncRoute(rootSpaceId, "");
  }

  function selectRoom(roomId: string) {
    const chats = get(shellChats);
    const room = chats.find((candidate) => candidate.roomId === roomId);
    if (!room || room.kind !== "room") {
      return;
    }

    const nextRootSpaceId = deriveRootSpaceIdForRoom(roomId, chats) ?? get(shellSelectedRootSpaceId);
    if (nextRootSpaceId) {
      shellSelectedRootSpaceId.set(nextRootSpaceId);
    }

    shellSelectedRoomId.set(roomId);
    void syncRoute(get(shellSelectedRootSpaceId), roomId);
  }

  const rootSpaces = $derived.by<MatrixChatSummary[]>(() => {
    const rooms = $shellChats;
    const roomById = new Map(rooms.map((room) => [room.roomId, room]));
    const directRooms = rooms.filter((room) => room.kind === "room" && room.isDirect);

    const dmsRoot: MatrixChatSummary = {
      roomId: VIRTUAL_DMS_ROOT_ID,
      displayName: "DMs",
      imageUrl: null,
      encrypted: false,
      joinedMembers: directRooms.length,
      kind: "space",
      joined: true,
      isDirect: false,
      parentRoomId: null,
    };

    const matrixRootSpaces = rooms
      .filter((room) => room.kind === "space")
      .filter((space) => !space.parentRoomId || !roomById.has(space.parentRoomId));

    matrixRootSpaces.sort((a, b) =>
      a.displayName.localeCompare(b.displayName, undefined, { sensitivity: "base" }),
    );

    return [dmsRoot, ...matrixRootSpaces];
  });

  const rootScopedRooms = $derived.by<MatrixChatSummary[]>(() => {
    const rootSpaceId = $shellSelectedRootSpaceId;
    if (!rootSpaceId) {
      return [];
    }

    const rooms = $shellChats;
    if (rootSpaceId === VIRTUAL_DMS_ROOT_ID) {
      return rooms
        .filter((room) => room.kind === "room" && room.isDirect)
        .sort((a, b) => a.displayName.localeCompare(b.displayName, undefined, { sensitivity: "base" }));
    }

    const roomById = new Map(rooms.map((room) => [room.roomId, room]));
    const childrenByParent = new Map<string, MatrixChatSummary[]>();

    for (const room of rooms) {
      if (!room.parentRoomId || room.parentRoomId === room.roomId) {
        continue;
      }

      const siblings = childrenByParent.get(room.parentRoomId) ?? [];
      siblings.push(room);
      childrenByParent.set(room.parentRoomId, siblings);
    }

    const descendants: MatrixChatSummary[] = [];
    const visited = new Set<string>();
    const stack = [...(childrenByParent.get(rootSpaceId) ?? [])];

    while (stack.length > 0) {
      const candidate = stack.pop();
      if (!candidate || visited.has(candidate.roomId)) {
        continue;
      }

      visited.add(candidate.roomId);
      descendants.push(candidate);

      const children = childrenByParent.get(candidate.roomId) ?? [];
      for (const child of children) {
        stack.push(child);
      }
    }

    descendants.sort((a, b) => a.displayName.localeCompare(b.displayName, undefined, { sensitivity: "base" }));

    return descendants.filter((room) => {
      if (room.kind === "space") {
        return true;
      }

      const parentId = room.parentRoomId;
      if (!parentId) {
        return true;
      }

      return parentId === rootSpaceId || roomById.has(parentId);
    });
  });

  function isRootSpace(roomId: string, rooms: MatrixChatSummary[]): boolean {
    if (roomId === VIRTUAL_DMS_ROOT_ID) {
      return true;
    }

    const roomById = new Map(rooms.map((room) => [room.roomId, room]));
    const room = roomById.get(roomId);
    if (!room || room.kind !== "space") {
      return false;
    }

    return !room.parentRoomId || !roomById.has(room.parentRoomId);
  }

  function deriveRootSpaceIdForRoom(roomId: string, rooms: MatrixChatSummary[]): string | null {
    const roomById = new Map(rooms.map((room) => [room.roomId, room]));
    const seen = new Set<string>();
    let current = roomById.get(roomId);

    if (current?.kind === "room" && current.isDirect) {
      return VIRTUAL_DMS_ROOT_ID;
    }

    while (current) {
      if (seen.has(current.roomId)) {
        break;
      }
      seen.add(current.roomId);

      const parentId = current.parentRoomId;
      if (!parentId) {
        return current.kind === "space" ? current.roomId : null;
      }

      const parent = roomById.get(parentId);
      if (!parent) {
        return parentId;
      }

      if (!parent.parentRoomId) {
        return parent.roomId;
      }

      current = parent;
    }

    return null;
  }

  async function logout() {
    loggingOut = true;
    shellErrorMessage.set("");

    try {
      await matrixLogout();
      shellChats.set([]);
      shellSelectedRootSpaceId.set("");
      shellSelectedRoomId.set("");
      shellCurrentUserId.set("");
      await goto("/");
    } catch (error) {
      shellErrorMessage.set(error instanceof Error ? error.message : "Failed to log out");
    } finally {
      loggingOut = false;
    }
  }
</script>

{#if false}
  <main class="min-h-screen grid place-items-center p-4">
    <p class="card p-3 text-sm bg-surface-100-900">Loading session...</p>
  </main>
{:else}
  <main class="h-screen">
    {#if $shellErrorMessage}
      <p class="card p-3 text-sm preset-filled-error-500 mx-4 md:mx-6 mt-4">{$shellErrorMessage}</p>
    {/if}

    <div class="grid gap-4 lg:grid-cols-[220px_280px_1fr] p-4 md:p-6 min-h-0 h-full">
      <div class="flex flex-col min-h-0 h-full gap-4">
        <AppHeader userId={$shellCurrentUserId} />
        <RootSpaceList
          spaces={rootSpaces}
          selectedRootSpaceId={$shellSelectedRootSpaceId}
          onSelectRootSpace={selectRootSpace}
        />
        <section class="card p-2 preset-outlined-surface-200-800 bg-surface-50-950">
          <a class="btn preset-tonal w-full justify-center" href="/settings">Settings</a>
        </section>
      </div>

      <div class="flex flex-col min-h-0 h-full gap-4">
        <RoomList
          rooms={rootScopedRooms}
          selectedRoomId={$shellSelectedRoomId}
          onSelectRoom={selectRoom}
          emptyMessage={$shellSelectedRootSpaceId
            ? "No rooms or subspaces in this root space yet."
            : "Select a root space to browse rooms."}
        />
      </div>

      <section class="lg:min-h-[60vh] h-full">
        {@render children()}
      </section>
    </div>
  </main>
{/if}
