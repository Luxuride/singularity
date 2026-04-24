<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { onMount } from "svelte";
  import { get } from "svelte/store";
  import { getCurrent, onOpenUrl } from "@tauri-apps/plugin-deep-link";

  import { matrixLogout, matrixRecoveryStatus, matrixSessionStatus } from "$lib/auth/api";
  import {
    matrixGetChatNavigation,
    matrixGetSpaceBrowse,
    matrixGetChats,
    matrixGetPickerAssets,
    matrixSetRootSpaceOrder,
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
    shellRootBrowseRooms,
    shellRootSpaces,
    shellSelectedRootSpaceId,
    shellSelectedRoomId,
  } from "$lib/chats/shell";
  import type {
    MatrixJoinBatchResult,
    MatrixJoinTargetPreview,
    MatrixChatSummary,
    MatrixRoomRemovedEvent,
    MatrixSelectedRoomMessagesEvent,
  } from "$lib/chats/types";
  import { AppHeader, RootSpaceList, RoomList } from "$lib/components/navigation";
  import JoinRoomDialog from "$lib/components/navigation/spaces/JoinRoomDialog.svelte";

  let { children } = $props();

  let checkingAuth = $state(true);
  let loggingOut = $state(false);
  let deepLinkJoinDialogOpen = $state(false);
  let deepLinkJoinTargets = $state<MatrixJoinTargetPreview[]>([]);
  let deepLinkJoinTitle = $state("Confirm link join");
  let deepLinkJoinQueue = $state<
    { title: string; targets: MatrixJoinTargetPreview[] }[]
  >([]);

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

            const serverNames = parsed.searchParams.getAll("via");
            const joinKind = parsed.hostname === "space" ? "space" : "room";

            enqueueDeepLinkJoin({
              title: `Confirm link join: ${targetOrAlias}`,
              targets: [{
                roomIdOrAlias: targetOrAlias,
                displayName: targetOrAlias,
                kind: joinKind,
                serverNames,
              }],
            });
          }
        }
      } catch {
        continue;
      }
    }

    openNextDeepLinkJoin();
  }

  function enqueueDeepLinkJoin(item: {
    title: string;
    targets: MatrixJoinTargetPreview[];
  }) {
    deepLinkJoinQueue = [...deepLinkJoinQueue, item];
  }

  function openNextDeepLinkJoin() {
    if (deepLinkJoinDialogOpen || deepLinkJoinQueue.length === 0) {
      return;
    }

    const [next, ...remaining] = deepLinkJoinQueue;
    deepLinkJoinQueue = remaining;
    deepLinkJoinTitle = next.title;
    deepLinkJoinTargets = next.targets;
    deepLinkJoinDialogOpen = true;
  }

  function closeDeepLinkJoinDialog() {
    deepLinkJoinDialogOpen = false;
    deepLinkJoinTargets = [];
    openNextDeepLinkJoin();
  }

  async function handleDeepLinkJoinConfirmed(result: MatrixJoinBatchResult) {
    const plannedTargets = deepLinkJoinTargets;
    const shouldFocusRoom = plannedTargets.some((target) => target.kind === "room");

    closeDeepLinkJoinDialog();

    await matrixTriggerRoomUpdate();

    if (shouldFocusRoom) {
      const joinedRoomId = result.joinedRoomIds.at(-1) ?? "";
      if (joinedRoomId) {
        shellSelectedRoomId.set(joinedRoomId);
      }
    }

    await refreshChatNavigationAndRoute();
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

    if (!nextRootSpaceId) {
      shellRootBrowseRooms.set([]);
      return nextRootSpaceId;
    }

    if (nextRootSpaceId.startsWith("virtual:")) {
      shellRootBrowseRooms.set(response.rootScopedRooms);
      return nextRootSpaceId;
    }

    try {
      const browse = await matrixGetSpaceBrowse(nextRootSpaceId);
      if (get(shellSelectedRootSpaceId) !== nextRootSpaceId) {
        return nextRootSpaceId;
      }
      shellRootBrowseRooms.set(browse.rooms);
    } catch {
      // Fallback keeps root view functional if hierarchy discovery is unavailable.
      if (get(shellSelectedRootSpaceId) !== nextRootSpaceId) {
        return nextRootSpaceId;
      }
      shellRootBrowseRooms.set(response.rootScopedRooms);
    }

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
    shellSelectedRootSpaceId.set(rootSpaceId);
    shellRootBrowseRooms.set([]);
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

  async function logout() {
    loggingOut = true;
    shellErrorMessage.set("");

    try {
      await matrixLogout();
      shellChats.set([]);
      shellRootSpaces.set([]);
      shellRootScopedRooms.set([]);
      shellRootBrowseRooms.set([]);
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
  <main class="h-screen overflow-x-hidden">
    {#if $shellErrorMessage}
      <p class="card p-3 text-sm preset-filled-error-500 mx-4 md:mx-6 mt-4">{$shellErrorMessage}</p>
    {/if}

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
          emptyMessage={$shellSelectedRootSpaceId
            ? "No rooms or subspaces in this root space yet."
            : "Select a root space to browse rooms."}
        />
      </div>

      <section class="lg:min-h-[60vh] h-full min-w-0 overflow-x-hidden">
        {@render children()}
      </section>
    </div>

    <JoinRoomDialog
      open={deepLinkJoinDialogOpen}
      onClose={closeDeepLinkJoinDialog}
      targets={deepLinkJoinTargets}
      title={deepLinkJoinTitle}
      confirmLabel="Join"
      onJoined={handleDeepLinkJoinConfirmed}
    />
  </main>
{/if}
