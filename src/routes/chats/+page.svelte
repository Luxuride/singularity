<script lang="ts">
  import { onMount } from "svelte";
  import { matrixSessionStatus } from "../../lib/auth/api";
  import { matrixGetChatMessages, matrixGetChats, matrixTriggerRoomUpdate } from "../../lib/chats/api";
  import { subscribeToRoomUpdates } from "../../lib/chats/realtime";
  import type {
    MatrixChatMessage,
    MatrixChatSummary,
    MatrixRoomRemovedEvent,
    MatrixSelectedRoomMessagesEvent,
  } from "../../lib/chats/types";

  let loading = $state(true);
  let loadingMessages = $state(false);
  let refreshing = $state(false);
  let errorMessage = $state("");

  let chats = $state<MatrixChatSummary[]>([]);
  let selectedRoomId = $state("");
  let selectedRoomName = $state("");
  let selectedEncrypted = $state(false);

  let messages = $state<MatrixChatMessage[]>([]);
  let nextFrom = $state<string | null>(null);

  onMount(() => {
    let unlisten = () => {};

    void (async () => {
      await loadChats();
      unlisten = await subscribeToRoomUpdates({
        onRoomAdded: applyRoomUpsert,
        onRoomUpdated: applyRoomUpsert,
        onRoomRemoved: applyRoomRemoval,
        onSelectedRoomMessages: applySelectedRoomMessages,
      });
    })();

    return () => {
      unlisten();
    };
  });

  function applyRoomUpsert(chat: MatrixChatSummary) {
    const index = chats.findIndex((candidate) => candidate.roomId === chat.roomId);
    if (index >= 0) {
      chats = chats.map((candidate, candidateIndex) => (candidateIndex === index ? chat : candidate));
    } else {
      chats = [...chats, chat];
    }

    if (selectedRoomId === chat.roomId) {
      selectedRoomName = chat.displayName;
      selectedEncrypted = chat.encrypted;
    }

    if (!selectedRoomId && chats.length > 0) {
      openChat(chats[0]);
    }
  }

  function applyRoomRemoval(payload: MatrixRoomRemovedEvent) {
    chats = chats.filter((chat) => chat.roomId !== payload.roomId);

    if (selectedRoomId === payload.roomId) {
      selectedRoomId = "";
      selectedRoomName = "";
      selectedEncrypted = false;
      messages = [];
      nextFrom = null;
    }
  }

  function applySelectedRoomMessages(payload: MatrixSelectedRoomMessagesEvent) {
    if (payload.roomId !== selectedRoomId) {
      return;
    }

    messages = payload.messages;
    nextFrom = payload.nextFrom;
  }

  async function loadChats() {
    loading = true;
    errorMessage = "";

    try {
      const session = await matrixSessionStatus();
      if (!session.authenticated) {
        errorMessage = "You are not logged in. Use the login page first.";
        chats = [];
        return;
      }

      chats = await matrixGetChats();

      if (chats.length > 0 && !selectedRoomId) {
        openChat(chats[0]);
      }
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : "Failed to load chats";
    } finally {
      loading = false;
    }
  }

  async function requestRefresh() {
    refreshing = true;
    errorMessage = "";

    try {
      await matrixTriggerRoomUpdate({
        selectedRoomId: selectedRoomId || undefined,
      });
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : "Failed to trigger room refresh";
    } finally {
      refreshing = false;
    }
  }

  function openChat(chat: MatrixChatSummary) {
    selectedRoomId = chat.roomId;
    selectedRoomName = chat.displayName;
    selectedEncrypted = chat.encrypted;
    messages = [];
    nextFrom = null;

    void loadMessages();
  }

  async function loadMessages() {
    if (!selectedRoomId) {
      return;
    }

    loadingMessages = true;
    errorMessage = "";

    try {
      const response = await matrixGetChatMessages({
        roomId: selectedRoomId,
        limit: 50,
      });

      messages = response.messages;
      nextFrom = response.nextFrom;
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : "Failed to load messages";
    } finally {
      loadingMessages = false;
    }
  }

  async function loadOlder() {
    if (!selectedRoomId || !nextFrom) {
      return;
    }

    loadingMessages = true;
    errorMessage = "";

    try {
      const response = await matrixGetChatMessages({
        roomId: selectedRoomId,
        from: nextFrom,
        limit: 50,
      });

      messages = [...response.messages, ...messages];
      nextFrom = response.nextFrom;
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : "Failed to load older messages";
    } finally {
      loadingMessages = false;
    }
  }

  function toTime(timestamp: number | null): string {
    if (!timestamp) {
      return "";
    }

    return new Date(timestamp).toLocaleString();
  }
</script>

<main class="min-h-screen p-4 md:p-8">
  <section class="card p-4 md:p-6 space-y-4 preset-outlined-surface-200-800 bg-surface-50-950">
    <header class="flex flex-wrap items-center justify-between gap-2">
      <div>
        <p class="text-xs font-bold uppercase tracking-[0.2em] text-primary-600-400">Singularity</p>
        <h1 class="h3">Chats</h1>
      </div>

      <button type="button" class="btn preset-tonal" onclick={requestRefresh} disabled={loading || loadingMessages || refreshing}>
        Refresh
      </button>
    </header>

    {#if errorMessage}
      <p class="card p-3 text-sm preset-filled-error-500">{errorMessage}</p>
    {/if}

    {#if loading}
      <p class="card p-3 text-sm bg-surface-100-900">Loading chats...</p>
    {:else}
      <div class="grid gap-4 lg:grid-cols-[280px_1fr]">
        <aside class="card p-2 preset-outlined-surface-200-800 bg-surface-100-900 max-h-[70vh] overflow-y-auto">
          {#if chats.length === 0}
            <p class="p-2 text-sm text-surface-700-300">No joined rooms found.</p>
          {:else}
            <ul class="space-y-1">
              {#each chats as chat (chat.roomId)}
                <li>
                  <button
                    type="button"
                    class="w-full text-left p-2 rounded hover:bg-surface-200-800 transition-colors"
                    class:bg-primary-100-900={chat.roomId === selectedRoomId}
                    onclick={() => openChat(chat)}
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

        <section class="card p-4 preset-outlined-surface-200-800 bg-surface-100-900 max-h-[70vh] overflow-y-auto space-y-3">
          {#if !selectedRoomId}
            <p class="text-sm text-surface-700-300">Select a room to read messages.</p>
          {:else}
            <header class="flex items-center justify-between gap-2 sticky top-0 bg-surface-100-900 py-1">
              <div>
                <h2 class="h5">{selectedRoomName}</h2>
                <p class="text-xs text-surface-700-300">
                  {selectedEncrypted ? "Encrypted room" : "Unencrypted room"}
                </p>
              </div>

              <button
                type="button"
                class="btn preset-tonal"
                onclick={loadOlder}
                disabled={!nextFrom || loadingMessages}
              >
                {#if loadingMessages}Loading...{:else}Load Older{/if}
              </button>
            </header>

            {#if messages.length === 0}
              <p class="text-sm text-surface-700-300">No messages yet.</p>
            {:else}
              <ul class="space-y-2">
                {#each messages as message, index (`${message.eventId ?? index}-${message.timestamp ?? 0}`)}
                  <li class="card p-3 preset-outlined-surface-300-700 bg-surface-50-950">
                    <div class="flex items-center justify-between gap-2 text-xs text-surface-700-300 mb-1">
                      <span>{message.sender}</span>
                      <span>{toTime(message.timestamp)}</span>
                    </div>
                    <p class="text-sm whitespace-pre-wrap break-words">{message.body}</p>
                    {#if message.encrypted}
                      <p class="text-xs mt-1 text-primary-700-300">Encrypted-origin event</p>
                    {/if}
                  </li>
                {/each}
              </ul>
            {/if}
          {/if}
        </section>
      </div>
    {/if}
  </section>
</main>
