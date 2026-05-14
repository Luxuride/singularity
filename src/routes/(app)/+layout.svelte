<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { onMount } from "svelte";
  import { get } from "svelte/store";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrent, onOpenUrl } from "@tauri-apps/plugin-deep-link";

  import { matrixRecoveryStatus, matrixSessionStatus } from "$lib/auth/api";
  import {
    matrixGetChatNavigation,
    matrixGetChatNavigationWithUnjoined,
    matrixGetChats,
    matrixGetPickerAssets,
    matrixGetRoomPreview,
    matrixSetRootSpaceOrder,
    matrixTriggerRoomUpdate,
    matrixJoinRoom,
    matrixLeaveRoom,
    matrixLeaveRooms,
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
    shellRootScopedRoomsWithUnjoined,
    shellRootSpaces,
    shellSelectedRootSpaceId,
    shellSelectedRoomId,
  } from "$lib/chats/shell";
  import type {
    MatrixChatSummary,
    MatrixRoomRefreshCompleteEvent,
    MatrixRoomRemovedEvent,
    MatrixSelectedRoomMessagesEvent,
    MatrixRoomPreview,
  } from "$lib/chats/types";
  import { AppHeader, RootSpaceList, RoomList } from "$lib/components/navigation";
  import JoinConfirmDialog from "$lib/components/navigation/rooms/JoinConfirmDialog.svelte";

  let { children } = $props();

  let checkingAuth = $state(true);
  let loggingOut = $state(false);
  let deepLinkJoinOpen = $state(false);
  let deepLinkJoinRoom = $state<MatrixRoomPreview | null>(null);
  let deepLinkJoinTarget = $state<{ roomIdOrAlias: string; serverNames: string[] } | null>(null);
  let deepLinkJoining = $state(false);

  const ROOM_REFRESH_COMPLETE_EVENT = "matrix://rooms/refresh/complete";

  onMount(() => {
    let unlisten = () => {};
    let unlistenDeepLink = () => {};

    void (async () => {
      unlisten = await subscribeToRoomUpdates({
        onRoomAdded: applyRoomUpsert,
        onRoomUpdated: applyRoomUpsert,
        onRoomRemoved: applyRoomRemoval,
        onSelectedRoomMessages: (_payload: MatrixSelectedRoomMessagesEvent) => {},
        onChatMessagesStream: () => {},
        onChatMessageImageLoaded: () => {},
      });

      try {
        const currentUrls = await getCurrent();
        void handleDeepLinks(currentUrls);

        unlistenDeepLink = await onOpenUrl((urls) => {
          void handleDeepLinks(urls);
        });
      } catch (error) {
        console.error("Failed to initialize deep-link listener", error);
      }

      await loadShell();
      await requestRefresh();
      checkingAuth = false;
    })();

    return () => {
      unlisten();
      unlistenDeepLink();
    };
  });

  async function handleDeepLinks(urls: string[] | null) {
    if (!urls) return;

    for (const urlString of urls) {
      try {
        const parsed = new URL(urlString);
        
        if (parsed.protocol === "singularity:") {
          // singularity://join/#example:matrix.org
          if (parsed.hostname === "join" || parsed.hostname === "room" || parsed.hostname === "space") {
            const targetOrAlias = parsed.hash.length > 0 ? parsed.hash : parsed.pathname.slice(1);
            if (!targetOrAlias) continue;
            
            // Support `?via=example.com&via=another.com`
            const serverNames = parsed.searchParams.getAll("via");
            await requestDeepLinkJoin(targetOrAlias, serverNames);
          }
        }
      } catch {
        continue;
      }
    }
  }

  async function requestDeepLinkJoin(roomIdOrAlias: string, serverNames: string[]) {
    shellErrorMessage.set("");

    try {
      const preview = await matrixGetRoomPreview({ roomIdOrAlias, serverNames });
      deepLinkJoinRoom = preview;
      deepLinkJoinTarget = { roomIdOrAlias, serverNames };
      deepLinkJoinOpen = true;
    } catch (error) {
      shellErrorMessage.set(
        error instanceof Error ? error.message : `Failed to preview room: ${roomIdOrAlias}`,
      );
    }
  }

  function closeDeepLinkJoin() {
    deepLinkJoinOpen = false;
    deepLinkJoinRoom = null;
    deepLinkJoinTarget = null;
  }

  async function confirmDeepLinkJoin() {
    if (!deepLinkJoinTarget || deepLinkJoining) {
      return;
    }

    deepLinkJoining = true;
    shellErrorMessage.set("");

    try {
      const res = await matrixJoinRoom(
        deepLinkJoinTarget.roomIdOrAlias,
        deepLinkJoinTarget.serverNames,
      );
      await syncRoute("", res.roomId);
      closeDeepLinkJoin();
    } catch (error) {
      shellErrorMessage.set(
        error instanceof Error
          ? error.message
          : `Failed to join from link: ${deepLinkJoinTarget.roomIdOrAlias}`,
      );
    } finally {
      deepLinkJoining = false;
    }
  }

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

  async function waitForRoomRefreshComplete(): Promise<void> {
    await new Promise<void>((resolve) => {
      void (async () => {
        let unlisten: (() => void) | null = null;
        unlisten = await listen<MatrixRoomRefreshCompleteEvent>(
          ROOM_REFRESH_COMPLETE_EVENT,
          () => {
            unlisten?.();
            resolve();
          },
        );
      })();
    });
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
    const joinedResponse = await matrixGetChatNavigation(input);
    const nextRootSpaceId = joinedResponse.selectedRootSpaceId ?? "";

    shellRootSpaces.set(joinedResponse.rootSpaces);
    shellSelectedRootSpaceId.set(nextRootSpaceId);
    shellRootScopedRooms.set(joinedResponse.rootScopedRooms);

    const scopedResponse = await matrixGetChatNavigationWithUnjoined({
      rootSpaceId: nextRootSpaceId,
      selectedRoomId: input?.selectedRoomId,
    });

    shellRootScopedRoomsWithUnjoined.set(scopedResponse.rootScopedRooms);

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

  async function reorderRootSpaces(rootSpaceIds: string[]) {
    try {
      await matrixSetRootSpaceOrder({ rootSpaceIds });
    } catch (error) {
      shellErrorMessage.set(error instanceof Error ? error.message : "Failed to save root space order");
      throw error;
    }

    try {
      await refreshChatNavigationAndRoute();
    } catch (error) {
      shellErrorMessage.set(error instanceof Error ? error.message : "Failed to refresh root space order");
    }
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

  async function leaveRoom(roomId: string) {
    shellErrorMessage.set("");

    try {
      await matrixLeaveRoom({ roomId });
      if (get(shellSelectedRoomId) === roomId) {
        shellSelectedRoomId.set("");
      }

      if (get(shellSelectedRootSpaceId) === roomId) {
        shellSelectedRootSpaceId.set("");
      }

      await matrixTriggerRoomUpdate();
      await waitForRoomRefreshComplete();
      await refreshChatNavigationAndRoute();
    } catch (error) {
      shellErrorMessage.set(error instanceof Error ? error.message : "Failed to leave room");
      throw error;
    }
  }

  async function leaveRooms(roomIds: string[]) {
    if (roomIds.length === 0) {
      return;
    }

    shellErrorMessage.set("");

    try {
      await matrixLeaveRooms({ roomIds });

      if (roomIds.includes(get(shellSelectedRoomId))) {
        shellSelectedRoomId.set("");
      }

      if (roomIds.includes(get(shellSelectedRootSpaceId))) {
        shellSelectedRootSpaceId.set("");
      }

      await matrixTriggerRoomUpdate();
      await waitForRoomRefreshComplete();
      await refreshChatNavigationAndRoute();
    } catch (error) {
      shellErrorMessage.set(error instanceof Error ? error.message : "Failed to leave rooms");
      throw error;
    }
  }


  async function logout() {
    loggingOut = true;
    shellErrorMessage.set("");

    try {
      shellChats.set([]);
      shellRootSpaces.set([]);
      shellRootScopedRooms.set([]);
      shellRootScopedRoomsWithUnjoined.set([]);
      shellSelectedRootSpaceId.set("");
      shellSelectedRoomId.set("");
      shellCurrentUserId.set("");
      shellPickerCustomEmoji.set([]);
      await goto("/loading?mode=logout");
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
  <main class="h-screen overflow-x-hidden">
    <div class="grid gap-4 lg:grid-cols-[220px_280px_minmax(0,1fr)] p-4 md:p-6 min-h-0 h-full overflow-x-hidden">
      <div class="flex flex-col min-h-0 min-w-0 h-full gap-4">
        <AppHeader userId={$shellCurrentUserId} />
          <RootSpaceList spaces={$shellRootSpaces} selectedRootSpaceId={$shellSelectedRootSpaceId} onSelectRootSpace={selectRootSpace} onReorderRootSpaces={reorderRootSpaces} />
        <section class="card p-2 preset-outlined-surface-200-800 bg-surface-50-950">
          <a class="btn preset-tonal w-full justify-center" href="/settings">Settings</a>
        </section>
      </div>

      <div class="flex flex-col min-h-0 min-w-0 h-full gap-4">
        <RoomList
          rooms={$shellRootScopedRooms}
          selectedRoomId={$shellSelectedRoomId}
          onSelectRoom={selectRoom}
          enableManageMenu={true}
          onLeaveRoom={leaveRoom}
          onLeaveRooms={leaveRooms}
          emptyMessage={$shellSelectedRootSpaceId
            ? "No rooms or subspaces in this root space yet."
            : "Select a root space to browse rooms."}
        />
      </div>

      <section class="lg:min-h-[60vh] h-full min-w-0 overflow-x-hidden">
        {@render children()}
      </section>
    </div>

    <JoinConfirmDialog
      open={deepLinkJoinOpen}
      room={deepLinkJoinRoom}
      joinKind="single"
      joinCount={0}
      confirmDisabled={deepLinkJoining}
      onClose={closeDeepLinkJoin}
      onConfirm={confirmDeepLinkJoin}
    />
  </main>
{/if}
