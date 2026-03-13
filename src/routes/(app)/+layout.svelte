<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { onMount } from "svelte";
  import { get } from "svelte/store";

  import { matrixLogout, matrixRecoveryStatus, matrixSessionStatus } from "../../lib/auth/api";
  import { matrixGetChats, matrixTriggerRoomUpdate } from "../../lib/chats/api";
  import { subscribeToRoomUpdates } from "../../lib/chats/realtime";
  import {
    shellChats,
    shellCurrentUserId,
    shellErrorMessage,
    shellLoading,
    shellRecoveryState,
    shellRefreshing,
    shellSelectedRoomId,
  } from "../../lib/chats/shell";
  import type {
    MatrixChatSummary,
    MatrixRoomRemovedEvent,
    MatrixSelectedRoomMessagesEvent,
  } from "../../lib/chats/types";

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

    const selectedRoomId = get(shellSelectedRoomId);
    if (!selectedRoomId) {
      selectRoom(chat.roomId);
    }
  }

  function applyRoomRemoval(payload: MatrixRoomRemovedEvent) {
    shellChats.update((currentChats) =>
      currentChats.filter((chat) => chat.roomId !== payload.roomId),
    );

    if (get(shellSelectedRoomId) === payload.roomId) {
      const nextRoom = get(shellChats)[0]?.roomId ?? "";
      shellSelectedRoomId.set(nextRoom);
      void syncRoomQuery(nextRoom);
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

      const queryRoomId = page.url.searchParams.get("roomId") ?? "";
      const selectedRoomId = chooseInitialRoom(rooms, queryRoomId);
      shellSelectedRoomId.set(selectedRoomId);
      await syncRoomQuery(selectedRoomId);
    } catch (error) {
      shellErrorMessage.set(error instanceof Error ? error.message : "Failed to load chats");
    } finally {
      shellLoading.set(false);
    }
  }

  function chooseInitialRoom(rooms: MatrixChatSummary[], roomIdFromQuery: string): string {
    if (!rooms.length) {
      return "";
    }

    const hasQueryRoom = rooms.some((room) => room.roomId === roomIdFromQuery);
    if (hasQueryRoom) {
      return roomIdFromQuery;
    }

    return rooms[0].roomId;
  }

  async function requestRefresh() {
    if (get(shellRefreshing)) {
      return;
    }

    shellRefreshing.set(true);
    shellErrorMessage.set("");

    try {
      await matrixTriggerRoomUpdate({
        selectedRoomId: get(shellSelectedRoomId) || undefined,
      });
    } catch (error) {
      shellErrorMessage.set(
        error instanceof Error ? error.message : "Failed to trigger room refresh",
      );
    } finally {
      shellRefreshing.set(false);
    }
  }

  async function syncRoomQuery(roomId: string) {
    if (!roomId) {
      await goto("/chats", { replaceState: true, noScroll: true, keepFocus: true });
      return;
    }

    const params = new URLSearchParams(page.url.searchParams);
    params.set("roomId", roomId);
    const search = params.toString();
    await goto(`/chats?${search}`, { replaceState: true, noScroll: true, keepFocus: true });
  }

  function selectRoom(roomId: string) {
    shellSelectedRoomId.set(roomId);
    void syncRoomQuery(roomId);
  }

  async function logout() {
    loggingOut = true;
    shellErrorMessage.set("");

    try {
      await matrixLogout();
      shellChats.set([]);
      shellSelectedRoomId.set("");
      shellCurrentUserId.set("");
      await goto("/");
    } catch (error) {
      shellErrorMessage.set(error instanceof Error ? error.message : "Failed to log out");
    } finally {
      loggingOut = false;
    }
  }

  function recoveryLabel(state: string | null): string {
    if (state === "enabled") return "Recovery enabled";
    if (state === "incomplete") return "Recovery incomplete";
    if (state === "disabled") return "Recovery disabled";
    return "Recovery status unknown";
  }
</script>

{#if checkingAuth}
  <main class="min-h-screen grid place-items-center p-4">
    <p class="card p-3 text-sm bg-surface-100-900">Loading session...</p>
  </main>
{:else}
  <main class="min-h-screen p-4 md:p-8">
    <section class="card p-4 md:p-6 space-y-4 preset-outlined-surface-200-800 bg-surface-50-950">
      <header class="flex flex-wrap items-center justify-between gap-2">
        <div>
          <p class="text-xs font-bold uppercase tracking-[0.2em] text-primary-600-400">Singularity</p>
          <h1 class="h3">Chats</h1>
          <p class="text-xs text-surface-700-300">{$shellCurrentUserId}</p>
        </div>

        <div class="flex flex-wrap gap-2">
          <a class="btn preset-tonal text-xs" href="/settings/security">
            Verification / Recovery
          </a>
          <button
            type="button"
            class="btn preset-tonal"
            onclick={requestRefresh}
            disabled={$shellLoading || $shellRefreshing}
          >
            Refresh
          </button>
          <button
            type="button"
            class="btn preset-filled-error-500"
            onclick={logout}
            disabled={loggingOut}
          >
            {#if loggingOut}Logging out...{:else}Logout{/if}
          </button>
        </div>
      </header>

      {#if $shellErrorMessage}
        <p class="card p-3 text-sm preset-filled-error-500">{$shellErrorMessage}</p>
      {/if}

      <p class="text-xs text-surface-700-300">{recoveryLabel($shellRecoveryState)}</p>

      <div class="grid gap-4 lg:grid-cols-[280px_1fr]">
        <aside class="card p-2 preset-outlined-surface-200-800 bg-surface-100-900 max-h-[70vh] overflow-y-auto">
          {#if $shellChats.length === 0}
            <p class="p-2 text-sm text-surface-700-300">No joined rooms found.</p>
          {:else}
            <ul class="space-y-1">
              {#each $shellChats as chat (chat.roomId)}
                <li>
                  <button
                    type="button"
                    class="w-full text-left p-2 rounded hover:bg-surface-200-800 transition-colors"
                    class:bg-primary-100-900={chat.roomId === $shellSelectedRoomId}
                    onclick={() => selectRoom(chat.roomId)}
                  >
                    <p class="font-medium truncate">{chat.displayName}</p>
                    <p class="text-xs text-surface-700-300">
                      {chat.encrypted ? "Encrypted" : "Unencrypted"} • {chat.joinedMembers} members
                    </p>
                  </button>
                </li>
              {/each}
            </ul>
          {/if}
        </aside>

        <section class="min-h-[60vh]">
          {@render children()}
        </section>
      </div>
    </section>
  </main>
{/if}
