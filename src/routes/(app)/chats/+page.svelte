<script lang="ts">
  import { onMount, tick } from "svelte";

  import { matrixGetChatMessages } from "../../../lib/chats/api";
  import { subscribeToRoomUpdates } from "../../../lib/chats/realtime";
  import { shellChats, shellSelectedRoomId } from "../../../lib/chats/shell";
  import type { MatrixChatMessage, MatrixSelectedRoomMessagesEvent } from "../../../lib/chats/types";

  let loadingMessages = $state(false);
  let errorMessage = $state("");

  let messages = $state<MatrixChatMessage[]>([]);
  let nextFrom = $state<string | null>(null);
  let timelineElement = $state<HTMLElement | null>(null);

  type RoomScrollState = {
    top: number;
    anchorEventId: string | null;
    anchorOffset: number;
  };

  const roomScrollStates = new Map<string, RoomScrollState>();

  let pendingRestoreRoomId = "";
  let pendingRestoreToBottom = false;
  let pendingRestoreAttempts = 0;
  let restoringScroll = false;

  const MAX_RESTORE_ATTEMPTS = 8;

  let previousSelectedRoomId = "";

  onMount(() => {
    let unlisten = () => {};

    void (async () => {
      unlisten = await subscribeToRoomUpdates({
        onRoomAdded: () => {},
        onRoomUpdated: () => {},
        onRoomRemoved: () => {},
        onSelectedRoomMessages: applySelectedRoomMessages,
      });
    })();

    return () => {
      unlisten();
    };
  });

  $effect(() => {
    const selectedRoomId = $shellSelectedRoomId;

    if (!selectedRoomId) {
      previousSelectedRoomId = "";
      pendingRestoreRoomId = "";
      pendingRestoreToBottom = false;
      pendingRestoreAttempts = 0;
      messages = [];
      nextFrom = null;
      return;
    }

    if (selectedRoomId === previousSelectedRoomId) {
      return;
    }

    pendingRestoreRoomId = selectedRoomId;
    pendingRestoreToBottom = !roomScrollStates.has(selectedRoomId);
    pendingRestoreAttempts = 0;
    previousSelectedRoomId = selectedRoomId;

    messages = [];
    nextFrom = null;

    void loadMessages(selectedRoomId);
  });

  $effect(() => {
    const selectedRoomId = $shellSelectedRoomId;

    if (
      !selectedRoomId ||
      !timelineElement ||
      loadingMessages ||
      pendingRestoreRoomId !== selectedRoomId
    ) {
      return;
    }

    const targetScrollState = roomScrollStates.get(selectedRoomId) ?? {
      top: 0,
      anchorEventId: null,
      anchorOffset: 0,
    };
    const restoreToBottom = pendingRestoreToBottom;

    void (async () => {
      await tick();

      if (!timelineElement || $shellSelectedRoomId !== selectedRoomId) {
        return;
      }

      // Wait an extra frame so li nodes have their final layout before restoring.
      await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));

      if (!timelineElement || $shellSelectedRoomId !== selectedRoomId) {
        return;
      }

      const maxScrollTop = Math.max(0, timelineElement.scrollHeight - timelineElement.clientHeight);
      let nextScrollTop = restoreToBottom ? maxScrollTop : Math.min(targetScrollState.top, maxScrollTop);

      const hasRenderableMessages =
        timelineElement.querySelector("[data-message-event-id]") !== null;
      const shouldRetryRestore =
        !restoreToBottom &&
        messages.length > 0 &&
        targetScrollState.top > 0 &&
        (!hasRenderableMessages || maxScrollTop === 0) &&
        pendingRestoreAttempts < MAX_RESTORE_ATTEMPTS;

      if (shouldRetryRestore) {
        pendingRestoreAttempts += 1;
        return;
      }

      if (!restoreToBottom && targetScrollState.anchorEventId) {
        const anchorElement = timelineElement.querySelector<HTMLElement>(
          `[data-message-event-id="${targetScrollState.anchorEventId}"]`
        );

        if (anchorElement) {
          nextScrollTop = Math.max(
            0,
            Math.min(anchorElement.offsetTop - targetScrollState.anchorOffset, maxScrollTop)
          );
        }
      }

      restoringScroll = true;
      timelineElement.scrollTop = nextScrollTop;
      saveRoomScrollState(selectedRoomId);
      restoringScroll = false;

      pendingRestoreRoomId = "";
      pendingRestoreToBottom = false;
      pendingRestoreAttempts = 0;
    })();
  });

  function handleTimelineScroll() {
    const selectedRoomId = $shellSelectedRoomId;

    if (!selectedRoomId || !timelineElement || restoringScroll) {
      return;
    }

    saveRoomScrollState(selectedRoomId);
  }

  function saveRoomScrollState(roomId: string) {
    if (!timelineElement) {
      return;
    }

    roomScrollStates.set(roomId, {
      top: timelineElement.scrollTop,
      ...findTopVisibleMessageAnchor(),
    });
  }

  function findTopVisibleMessageAnchor(): { anchorEventId: string | null; anchorOffset: number } {
    if (!timelineElement) {
      return { anchorEventId: null, anchorOffset: 0 };
    }

    const children = timelineElement.querySelectorAll<HTMLElement>("[data-message-event-id]");

    for (const child of children) {
      if (child.offsetTop + child.offsetHeight <= timelineElement.scrollTop) {
        continue;
      }

      const eventId = child.dataset.messageEventId;
      if (!eventId) {
        continue;
      }

      return {
        anchorEventId: eventId,
        anchorOffset: child.offsetTop - timelineElement.scrollTop,
      };
    }

    return { anchorEventId: null, anchorOffset: 0 };
  }

  function applySelectedRoomMessages(payload: MatrixSelectedRoomMessagesEvent) {
    if (payload.roomId !== $shellSelectedRoomId) {
      return;
    }

    messages = payload.messages;
    nextFrom = payload.nextFrom;
  }

  async function loadMessages(roomId: string) {
    loadingMessages = true;
    errorMessage = "";

    try {
      const response = await matrixGetChatMessages({
        roomId,
        limit: 50,
      });

      if ($shellSelectedRoomId !== roomId) {
        return;
      }

      messages = response.messages;
      nextFrom = response.nextFrom;
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : "Failed to load messages";
    } finally {
      loadingMessages = false;
    }
  }

  async function loadOlder() {
    const selectedRoomId = $shellSelectedRoomId;
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

  function decryptionLabel(status: MatrixChatMessage["decryptionStatus"]): string {
    if (status === "decrypted") {
      return "Decrypted";
    }

    if (status === "unableToDecrypt") {
      return "Unable to decrypt";
    }

    return "Plaintext";
  }

  function verificationLabel(status: MatrixChatMessage["verificationStatus"]): string {
    if (status === "verified") {
      return "Verified sender device";
    }

    if (status === "unverified") {
      return "Unverified sender device";
    }

    return "Verification unknown";
  }

  function selectedRoomName(): string {
    const selectedRoomId = $shellSelectedRoomId;
    const selectedRoom = $shellChats.find((chat) => chat.roomId === selectedRoomId);
    return selectedRoom?.displayName ?? "";
  }

  function selectedRoomEncrypted(): boolean {
    const selectedRoomId = $shellSelectedRoomId;
    const selectedRoom = $shellChats.find((chat) => chat.roomId === selectedRoomId);
    return selectedRoom?.encrypted ?? false;
  }
</script>

<section
  class="card p-4 preset-outlined-surface-200-800 bg-surface-100-900 max-h-[70vh] overflow-y-auto space-y-3"
  bind:this={timelineElement}
  onscroll={handleTimelineScroll}
>
  {#if errorMessage}
    <p class="card p-3 text-sm preset-filled-error-500">{errorMessage}</p>
  {/if}

  {#if !$shellSelectedRoomId}
    <p class="text-sm text-surface-700-300">Select a room to read messages.</p>
  {:else}
    <header class="flex items-center justify-between gap-2 sticky top-0 bg-surface-100-900 py-1">
      <div>
        <h2 class="h5">{selectedRoomName()}</h2>
        <p class="text-xs text-surface-700-300">
          {selectedRoomEncrypted() ? "Encrypted room" : "Unencrypted room"}
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
          <li
            class="card p-3 preset-outlined-surface-300-700 bg-surface-50-950"
            data-message-event-id={message.eventId ?? undefined}
          >
            <div class="flex items-center justify-between gap-2 text-xs text-surface-700-300 mb-1">
              <span>{message.sender}</span>
              <span>{toTime(message.timestamp)}</span>
            </div>
            <p class="text-sm whitespace-pre-wrap break-words">{message.body}</p>
            {#if message.encrypted}
              <div class="mt-1 flex flex-wrap items-center gap-2 text-xs">
                <span class="rounded px-2 py-0.5 bg-surface-200-800">{decryptionLabel(message.decryptionStatus)}</span>
                <span
                  class="rounded px-2 py-0.5"
                  class:bg-success-200-800={message.verificationStatus === "verified"}
                  class:bg-warning-200-800={message.verificationStatus === "unverified"}
                  class:bg-surface-200-800={message.verificationStatus === "unknown"}
                >
                  {verificationLabel(message.verificationStatus)}
                </span>
              </div>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  {/if}
</section>
