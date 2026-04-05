<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { onMount } from "svelte";
  import { get } from "svelte/store";

  import { matrixLogout, matrixRecoveryStatus, matrixSessionStatus } from "$lib/auth/api";
  import {
    matrixGetChatNavigation,
    matrixGetChats,
    matrixGetPickerAssets,
    matrixTriggerRoomUpdate,
  } from "$lib/chats/api";
  import { subscribeToRoomUpdates } from "$lib/chats/realtime";
  import {
    shellChats,
    shellCurrentUserId,
    shellErrorMessage,
    shellLoading,
    shellRecoveryState,
    shellRefreshing,
    shellPickerCustomEmoji,
    shellRootScopedRooms,
    shellRootSpaces,
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

    void refreshChatNavigationAndRoute();
  }

  function applyRoomRemoval(payload: MatrixRoomRemovedEvent) {
    shellChats.update((currentChats) =>
      currentChats.filter((chat) => chat.roomId !== payload.roomId),
    );

    if (get(shellSelectedRoomId) === payload.roomId) {
      shellSelectedRoomId.set("");
    }

    void refreshChatNavigationAndRoute();
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

      const rooms = await matrixGetChats();
      shellChats.set(rooms);

      const queryRootSpaceId = page.url.searchParams.get("rootSpaceId") ?? "";
      const queryRoomId = page.url.searchParams.get("roomId") ?? "";
      const selectedRoomId = chooseInitialRoom(rooms, queryRoomId);
      shellSelectedRoomId.set(selectedRoomId);

      const selectedRootSpaceId = await refreshChatNavigation({
        rootSpaceId: queryRootSpaceId,
        selectedRoomId,
      });

      await syncRoute(selectedRootSpaceId, selectedRoomId);

      void loadShellMetadata();
    } catch (error) {
      shellErrorMessage.set(error instanceof Error ? error.message : "Failed to load chats");
    } finally {
      shellLoading.set(false);
    }
  }

  async function loadShellMetadata() {
    try {
      const recovery = await matrixRecoveryStatus();
      shellRecoveryState.set(recovery.state);
    } catch {
      shellRecoveryState.set(null);
    }

    try {
      const { customEmoji: pickerCustomEmoji } = await matrixGetPickerAssets();
      shellPickerCustomEmoji.set(pickerCustomEmoji);
    } catch {
      shellPickerCustomEmoji.set([]);
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

  async function refreshChatNavigation(input?: { rootSpaceId?: string; selectedRoomId?: string }) {
    const response = await matrixGetChatNavigation(input);
    const nextRootSpaceId = response.selectedRootSpaceId ?? "";

    shellRootSpaces.set(response.rootSpaces);
    shellRootScopedRooms.set(response.rootScopedRooms);
    shellSelectedRootSpaceId.set(nextRootSpaceId);

    return nextRootSpaceId;
  }

  async function refreshChatNavigationAndRoute() {
    const roomId = get(shellSelectedRoomId);
    const rootSpaceId = await refreshChatNavigation({
      rootSpaceId: get(shellSelectedRootSpaceId),
      selectedRoomId: roomId,
    });

    await syncRoute(rootSpaceId, roomId);
  }

  async function selectRootSpace(rootSpaceId: string) {
    shellSelectedRoomId.set("");
    const nextRootSpaceId = await refreshChatNavigation({ rootSpaceId });
    await syncRoute(nextRootSpaceId, "");
  }

  async function selectRoom(roomId: string) {
    const chats = get(shellChats);
    const room = chats.find((candidate) => candidate.roomId === roomId);
    if (!room || room.kind !== "room") {
      return;
    }

    shellSelectedRoomId.set(roomId);
    const nextRootSpaceId = await refreshChatNavigation({
      rootSpaceId: get(shellSelectedRootSpaceId),
      selectedRoomId: roomId,
    });

    await syncRoute(nextRootSpaceId, roomId);
  }

  async function logout() {
    loggingOut = true;
    shellErrorMessage.set("");

    try {
      await matrixLogout();
      shellChats.set([]);
      shellRootSpaces.set([]);
      shellRootScopedRooms.set([]);
      shellSelectedRootSpaceId.set("");
      shellSelectedRoomId.set("");
      shellCurrentUserId.set("");
      shellPickerCustomEmoji.set([]);
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
          spaces={$shellRootSpaces}
          selectedRootSpaceId={$shellSelectedRootSpaceId}
          onSelectRootSpace={selectRootSpace}
        />
        <section class="card p-2 preset-outlined-surface-200-800 bg-surface-50-950">
          <a class="btn preset-tonal w-full justify-center" href="/settings">Settings</a>
        </section>
      </div>

      <div class="flex flex-col min-h-0 h-full gap-4">
        <RoomList
          rooms={$shellRootScopedRooms}
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
