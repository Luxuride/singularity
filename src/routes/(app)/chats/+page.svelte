<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { onMount, tick } from "svelte";

  import {
    matrixSendChatMessage,
    matrixStreamChatMessages,
    matrixToggleReaction,
  } from "$lib/chats/api";
  import { subscribeToRoomUpdates } from "$lib/chats/realtime";
  import {
    shellChats,
    shellCurrentUserId,
    shellPickerCustomEmoji,
    shellSelectedRootSpaceId,
    shellSelectedRoomId,
  } from "$lib/chats/shell";
  import type {
    MatrixChatMessage,
    MatrixChatMessageStreamEvent,
    MatrixSelectedRoomMessagesEvent,
    MatrixSendChatMessageRequest,
    MatrixMessageLoadKind,
    MatrixReactionSummary,
  } from "$lib/chats/types";
  import {
    buildMessageForSend,
    normalizeReactionKey,
    normalizeShortcodesToEmoji,
  } from "$lib/emoji/picker";
  import MessageTimeline from "$lib/components/messaging/MessageTimeline.svelte";
  import MessageComposer from "$lib/components/messaging/MessageComposer.svelte";
  import RoomList from "$lib/components/navigation/RoomList.svelte";

  const VIRTUAL_DMS_ROOT_ID = "virtual:dms";

  let loadingMessages = $state(false);
  let errorMessage = $state("");
  let composerErrorMessage = $state("");
  let messageDraft = $state("");
  let sendingMessage = $state(false);

  type MessageSendState = "sending" | "failed";
  type TimelineMessage = MatrixChatMessage & {
    localId?: string;
    sendState?: MessageSendState;
  };

  let messages = $state<TimelineMessage[]>([]);
  let nextFrom = $state<string | null>(null);
  let timelineElement = $state<HTMLElement | null>(null);
  let activeStreamId = $state("");
  let activeLoadKind = $state<MatrixMessageLoadKind | null>(null);
  let streamMessageCount = $state(0);

  const seenEventIds = new Set<string>();

  type RoomScrollState = {
    bottomOffset: number;
    anchorEventId: string | null;
    anchorOffset: number;
  };

  const roomScrollStates = new Map<string, RoomScrollState>();
  const AUTO_LOAD_TOP_THRESHOLD_PX = 96;
  const AUTO_LOAD_OLDER_COOLDOWN_MS = 400;

  let pendingRestoreRoomId = "";
  let pendingRestoreToBottom = false;
  let pendingRestoreAttempts = 0;
  let restoringScroll = false;
  let pendingPinToBottomRoomId = "";

  const MAX_RESTORE_ATTEMPTS = 8;

  let previousSelectedRoomId = "";
  let lastAutoLoadOlderAt = 0;

  onMount(() => {
    let unlisten = () => {};

    void (async () => {
      unlisten = await subscribeToRoomUpdates({
        onRoomAdded: () => {},
        onRoomUpdated: () => {},
        onRoomRemoved: () => {},
        onSelectedRoomMessages: applySelectedRoomMessages,
        onChatMessagesStream: applyChatMessageStream,
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
      lastAutoLoadOlderAt = 0;
      pendingRestoreRoomId = "";
      pendingRestoreToBottom = false;
      pendingRestoreAttempts = 0;
      activeStreamId = "";
      activeLoadKind = null;
      streamMessageCount = 0;
      seenEventIds.clear();
      loadingMessages = false;
      messageDraft = "";
      sendingMessage = false;
      composerErrorMessage = "";
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
    lastAutoLoadOlderAt = 0;
    activeStreamId = "";
    activeLoadKind = null;
    streamMessageCount = 0;
    seenEventIds.clear();
    messageDraft = "";
    sendingMessage = false;
    composerErrorMessage = "";

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
      bottomOffset: 0,
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
      let nextScrollTop = restoreToBottom
        ? maxScrollTop
        : Math.max(0, Math.min(maxScrollTop - targetScrollState.bottomOffset, maxScrollTop));

      const hasRenderableMessages =
        timelineElement.querySelector("[data-message-event-id]") !== null;
      const shouldRetryRestore =
        !restoreToBottom &&
        messages.length > 0 &&
        targetScrollState.bottomOffset > 0 &&
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

    if (timelineElement.scrollTop > AUTO_LOAD_TOP_THRESHOLD_PX || loadingMessages || !nextFrom) {
      return;
    }

    const now = Date.now();
    if (now - lastAutoLoadOlderAt < AUTO_LOAD_OLDER_COOLDOWN_MS) {
      return;
    }

    lastAutoLoadOlderAt = now;
    void loadOlder();
  }

  function saveRoomScrollState(roomId: string) {
    if (!timelineElement) {
      return;
    }

    roomScrollStates.set(roomId, {
      bottomOffset: Math.max(0, timelineElement.scrollHeight - timelineElement.clientHeight - timelineElement.scrollTop),
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

  function createStreamId(): string {
    if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
      return crypto.randomUUID();
    }

    return `${Date.now()}-${Math.random().toString(16).slice(2)}`;
  }

  function queuePinTimelineToBottom(roomId: string) {
    if (pendingPinToBottomRoomId === roomId) {
      return;
    }

    pendingPinToBottomRoomId = roomId;

    void (async () => {
      await tick();
      await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));

      if (!timelineElement || $shellSelectedRoomId !== roomId) {
        pendingPinToBottomRoomId = "";
        return;
      }

      const maxScrollTop = Math.max(0, timelineElement.scrollHeight - timelineElement.clientHeight);

      restoringScroll = true;
      timelineElement.scrollTop = maxScrollTop;
      saveRoomScrollState(roomId);
      restoringScroll = false;

      pendingPinToBottomRoomId = "";
    })();
  }

  function isDuplicateMessage(message: MatrixChatMessage): boolean {
    if (!message.eventId) {
      return false;
    }

    if (seenEventIds.has(message.eventId)) {
      return true;
    }

    seenEventIds.add(message.eventId);
    return false;
  }

  function tryReplaceOptimisticWithRemote(message: MatrixChatMessage): boolean {
    if (message.timestamp == null) {
      return false;
    }

    const incomingTimestamp = message.timestamp;

    const optimisticIndex = messages.findIndex((candidate) => {
      if (candidate.sendState !== "sending") {
        return false;
      }

      if (candidate.body !== message.body || candidate.sender !== message.sender) {
        return false;
      }

      if (!candidate.timestamp) {
        return false;
      }

      return Math.abs(candidate.timestamp - incomingTimestamp) <= 120_000;
    });

    if (optimisticIndex < 0) {
      return false;
    }

    const updated = [...messages];
    updated[optimisticIndex] = message;
    messages = updated;
    return true;
  }

  function applyChatMessageStream(payload: MatrixChatMessageStreamEvent) {
    if (payload.roomId !== $shellSelectedRoomId) {
      return;
    }

    if (!activeStreamId || payload.streamId !== activeStreamId || payload.loadKind !== activeLoadKind) {
      return;
    }

    if (payload.done) {
      nextFrom = payload.nextFrom;
      loadingMessages = false;
      activeStreamId = "";
      activeLoadKind = null;
      streamMessageCount = 0;
      return;
    }

    if (!payload.message || isDuplicateMessage(payload.message)) {
      return;
    }

    if (tryReplaceOptimisticWithRemote(payload.message)) {
      if (payload.message.eventId) {
        seenEventIds.add(payload.message.eventId);
      }
      return;
    }

    streamMessageCount = payload.sequence + 1;

    if (payload.loadKind === "older") {
      // Older pagination keeps newest->older stream order; prepend preserves chronology.
      messages = [payload.message, ...messages];
    } else {
      // Initial streaming emits oldest->newest so appending keeps timeline ascending.
      messages = [...messages, payload.message];
    }

    if (payload.loadKind === "initial") {
      queuePinTimelineToBottom(payload.roomId);
    }
  }

  function applySelectedRoomMessages(payload: MatrixSelectedRoomMessagesEvent) {
    if (payload.roomId !== $shellSelectedRoomId) {
      return;
    }

    if (loadingMessages) {
      return;
    }

    const indexed = payload.messages
      .map((message, index) => ({ message, index }))
      .filter(({ message }) => Boolean(message.eventId));

    if (!indexed.length) {
      return;
    }

    for (const { message } of indexed) {
      if (!message.eventId || !seenEventIds.has(message.eventId)) {
        continue;
      }

      messages = messages.map((candidate) => {
        if (candidate.eventId !== message.eventId) {
          return candidate;
        }

        return {
          ...candidate,
          ...message,
          localId: candidate.localId,
          sendState: candidate.sendState,
        };
      });
    }

    const seenIndexes = indexed
      .filter(({ message }) => Boolean(message.eventId && seenEventIds.has(message.eventId)))
      .map(({ index }) => index);

    const newerCandidates: MatrixChatMessage[] = [];
    const olderCandidates: MatrixChatMessage[] = [];

    if (!seenIndexes.length) {
      for (const { message } of indexed) {
        if (!message.eventId || seenEventIds.has(message.eventId)) {
          continue;
        }
        newerCandidates.push(message);
      }
    } else {
      const firstSeenIndex = Math.min(...seenIndexes);
      const lastSeenIndex = Math.max(...seenIndexes);

      for (const { message, index } of indexed) {
        if (!message.eventId || seenEventIds.has(message.eventId)) {
          continue;
        }

        if (index < firstSeenIndex) {
          newerCandidates.push(message);
          continue;
        }

        if (index > lastSeenIndex) {
          olderCandidates.push(message);
        }
      }
    }

    const prependOlder: TimelineMessage[] = [];
    for (const message of olderCandidates) {
      if (tryReplaceOptimisticWithRemote(message)) {
        if (message.eventId) {
          seenEventIds.add(message.eventId);
        }
        continue;
      }

      if (message.eventId) {
        seenEventIds.add(message.eventId);
      }

      prependOlder.push(message);
    }

    const appendNewer: TimelineMessage[] = [];
    for (const message of [...newerCandidates].reverse()) {
      if (tryReplaceOptimisticWithRemote(message)) {
        if (message.eventId) {
          seenEventIds.add(message.eventId);
        }
        continue;
      }

      if (message.eventId) {
        seenEventIds.add(message.eventId);
      }

      appendNewer.push(message);
    }

    if (!prependOlder.length && !appendNewer.length) {
      return;
    }

    messages = [...prependOlder.reverse(), ...messages, ...appendNewer];
    if (appendNewer.length) {
      queuePinTimelineToBottom(payload.roomId);
    }
  }

  function buildOptimisticMessage(body: string): TimelineMessage {
    const encryptedRoom = selectedRoomEncrypted();

    return {
      eventId: null,
      sender: $shellCurrentUserId || "You",
      timestamp: Date.now(),
      body,
      formattedBody: null,
      messageType: "m.text",
      imageUrl: null,
      customEmojis: [],
      reactions: [],
      encrypted: encryptedRoom,
      decryptionStatus: encryptedRoom ? "decrypted" : "plaintext",
      verificationStatus: "unknown",
      localId: createStreamId(),
      sendState: "sending",
    };
  }

  async function sendOptimisticMessage(
    roomId: string,
    request: MatrixSendChatMessageRequest,
    localId: string,
  ) {
    sendingMessage = true;

    try {
      const response = await matrixSendChatMessage(request);

      if (response.eventId) {
        seenEventIds.add(response.eventId);
      }

      messages = messages.map((message) => {
        if (message.localId !== localId) {
          return message;
        }

        return {
          ...message,
          eventId: response.eventId,
          sendState: undefined,
          localId: undefined,
        };
      });
    } catch (error) {
      const message = error instanceof Error ? error.message : "Failed to send message";
      composerErrorMessage = message;

      messages = messages.map((timelineMessage) => {
        if (timelineMessage.localId !== localId) {
          return timelineMessage;
        }

        return {
          ...timelineMessage,
          sendState: "failed",
        };
      });
    } finally {
      sendingMessage = false;
      queuePinTimelineToBottom(roomId);
    }
  }

  async function sendDraftMessage() {
    const roomId = $shellSelectedRoomId;
    if (!roomId) {
      return;
    }

    const rawBody = messageDraft.trim();
    if (!rawBody) {
      composerErrorMessage = "Message cannot be empty";
      return;
    }

    const messageForSend = await buildMessageForSend(rawBody, $shellPickerCustomEmoji);
    const body = messageForSend.body;

    composerErrorMessage = "";
    messageDraft = "";

    const optimistic = buildOptimisticMessage(body);
    const localId = optimistic.localId;

    if (!localId) {
      return;
    }

    messages = [...messages, optimistic];
    queuePinTimelineToBottom(roomId);

    await sendOptimisticMessage(
      roomId,
      {
        roomId,
        body,
        formattedBody: messageForSend.formattedBody,
      },
      localId,
    );
  }

  function updateReactionSummary(
    existing: MatrixReactionSummary[],
    key: string,
    userId: string,
    added: boolean,
  ): MatrixReactionSummary[] {
    const next = existing.map((reaction) => ({
      ...reaction,
      senders: [...reaction.senders],
    }));

    const index = next.findIndex((reaction) => reaction.key === key);
    if (index < 0) {
      if (!added) {
        return next;
      }

      next.push({ key, count: 1, senders: [userId] });
      return next;
    }

    const reaction = next[index];
    const senderSet = new Set(reaction.senders);
    if (added) {
      senderSet.add(userId);
    } else {
      senderSet.delete(userId);
    }

    const senders = [...senderSet];
    if (senders.length === 0) {
      next.splice(index, 1);
      return next;
    }

    next[index] = {
      ...reaction,
      senders,
      count: senders.length,
    };

    return next;
  }

  async function handleToggleReaction(message: TimelineMessage, key: string) {
    const roomId = $shellSelectedRoomId;
    if (!roomId || !message.eventId) {
      return;
    }

    const normalizedKey = await normalizeReactionKey(key, $shellPickerCustomEmoji);
    const reactionKey = normalizedKey.trim();
    if (!reactionKey) {
      return;
    }

    const userId = $shellCurrentUserId;
    const targetEventId = message.eventId;
    let snapshot: TimelineMessage | null = null;

    if (userId) {
      const current = messages.find((candidate) => candidate.eventId === targetEventId);
      if (current) {
        snapshot = {
          ...current,
          reactions: current.reactions.map((reaction) => ({
            ...reaction,
            senders: [...reaction.senders],
          })),
        };

        const alreadyReacted = current.reactions.some(
          (reaction) => reaction.key === reactionKey && reaction.senders.includes(userId),
        );

        messages = messages.map((candidate) => {
          if (candidate.eventId !== targetEventId) {
            return candidate;
          }

          return {
            ...candidate,
            reactions: updateReactionSummary(
              candidate.reactions,
              reactionKey,
              userId,
              !alreadyReacted,
            ),
          };
        });
      }
    }

    try {
      const response = await matrixToggleReaction({
        roomId,
        targetEventId,
        key: reactionKey,
      });

      if (!userId) {
        return;
      }

      messages = messages.map((candidate) => {
        if (candidate.eventId !== targetEventId) {
          return candidate;
        }

        return {
          ...candidate,
          reactions: updateReactionSummary(
            candidate.reactions,
            reactionKey,
            userId,
            response.added,
          ),
        };
      });
    } catch (error) {
      if (snapshot) {
        messages = messages.map((candidate) => {
          if (candidate.eventId !== targetEventId) {
            return candidate;
          }

          return snapshot as TimelineMessage;
        });
      }

      composerErrorMessage = error instanceof Error ? error.message : "Failed to toggle reaction";
    }
  }

  async function retryMessage(message: TimelineMessage) {
    const roomId = $shellSelectedRoomId;
    if (!roomId || !message.localId || message.sendState !== "failed") {
      return;
    }

    composerErrorMessage = "";

    messages = messages.map((timelineMessage) => {
      if (timelineMessage.localId !== message.localId) {
        return timelineMessage;
      }

      return {
        ...timelineMessage,
        sendState: "sending",
      };
    });

    await sendOptimisticMessage(
      roomId,
      {
        roomId,
        body: message.body,
      },
      message.localId,
    );
  }

  function handleComposerSubmit(event: SubmitEvent) {
    event.preventDefault();

    if (sendingMessage) {
      return;
    }

    void sendDraftMessage();
  }

  async function loadMessages(roomId: string) {
    loadingMessages = true;
    errorMessage = "";
    seenEventIds.clear();

    const streamId = createStreamId();
    activeStreamId = streamId;
    activeLoadKind = "initial";
    streamMessageCount = 0;

    try {
      await matrixStreamChatMessages({
        roomId,
        limit: 50,
        streamId,
        loadKind: "initial",
      });

      if ($shellSelectedRoomId !== roomId || activeStreamId !== streamId) {
        return;
      }
    } catch (error) {
      if (activeStreamId !== streamId) {
        return;
      }

      loadingMessages = false;
      activeStreamId = "";
      activeLoadKind = null;
      streamMessageCount = 0;
      errorMessage = error instanceof Error ? error.message : "Failed to stream messages";
    }
  }

  async function loadOlder() {
    const selectedRoomId = $shellSelectedRoomId;
    if (!selectedRoomId || !nextFrom) {
      return;
    }

    loadingMessages = true;
    errorMessage = "";

    const streamId = createStreamId();
    activeStreamId = streamId;
    activeLoadKind = "older";
    streamMessageCount = 0;

    try {
      await matrixStreamChatMessages({
        roomId: selectedRoomId,
        from: nextFrom,
        limit: 10,
        streamId,
        loadKind: "older",
      });
    } catch (error) {
      if (activeStreamId !== streamId) {
        return;
      }

      loadingMessages = false;
      activeStreamId = "";
      activeLoadKind = null;
      streamMessageCount = 0;
      errorMessage = error instanceof Error ? error.message : "Failed to stream older messages";
    }
  }

  function selectedRoomEncrypted(): boolean {
    const selectedRoomId = $shellSelectedRoomId;
    const selectedRoom = $shellChats.find((chat) => chat.roomId === selectedRoomId);
    return selectedRoom?.encrypted ?? false;
  }

  const selectedRootSpaceName = $derived(
    $shellSelectedRootSpaceId === VIRTUAL_DMS_ROOT_ID
      ? "DMs"
      : $shellChats.find((room) => room.roomId === $shellSelectedRootSpaceId)?.displayName ?? "Server",
  );

  const selectedRootScopedRooms = $derived.by(() => {
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

    const childrenByParent = new Map<string, typeof rooms>();

    for (const room of rooms) {
      if (!room.parentRoomId || room.parentRoomId === room.roomId) {
        continue;
      }

      const siblings = childrenByParent.get(room.parentRoomId) ?? [];
      siblings.push(room);
      childrenByParent.set(room.parentRoomId, siblings);
    }

    const descendants: typeof rooms = [];
    const seen = new Set<string>();
    const stack = [...(childrenByParent.get(rootSpaceId) ?? [])];

    while (stack.length > 0) {
      const candidate = stack.pop();
      if (!candidate || seen.has(candidate.roomId)) {
        continue;
      }

      seen.add(candidate.roomId);
      descendants.push(candidate);

      for (const child of childrenByParent.get(candidate.roomId) ?? []) {
        stack.push(child);
      }
    }

    return descendants;
  });

  async function selectRoomFromOverview(roomId: string) {
    const room = $shellChats.find((candidate) => candidate.roomId === roomId);
    if (!room || room.kind !== "room") {
      return;
    }

    shellSelectedRoomId.set(roomId);

    const params = new URLSearchParams(page.url.searchParams);
    if ($shellSelectedRootSpaceId) {
      params.set("rootSpaceId", $shellSelectedRootSpaceId);
    }
    params.set("roomId", roomId);

    const search = params.toString();
    await goto(search ? `/chats?${search}` : "/chats", {
      replaceState: true,
      noScroll: true,
      keepFocus: true,
    });
  }
</script>

{#if !$shellSelectedRoomId}
  <section class="card p-6 preset-outlined-surface-200-800 bg-surface-100-900 h-full overflow-y-auto space-y-5">
    <header>
      <p class="text-xs uppercase tracking-wide text-surface-700-300">Server</p>
      <h1 class="text-2xl font-semibold">{selectedRootSpaceName}</h1>
      <p class="text-sm text-surface-700-300 mt-1">
        Spaces and rooms are shown as one mixed tree. Expand sub-spaces and select a room to open chat.
      </p>
    </header>

    {#if !$shellSelectedRootSpaceId}
      <p class="text-sm text-surface-700-300">Select a root space from the left column to browse its hierarchy.</p>
    {:else}
      <section class="space-y-2">
        <h2 class="text-sm font-medium">{selectedRootSpaceName} Hierarchy</h2>
        {#if selectedRootScopedRooms.length === 0}
          <p class="text-sm text-surface-700-300">No spaces or rooms found under this root space.</p>
        {:else}
          <RoomList
            rooms={selectedRootScopedRooms}
            selectedRoomId={$shellSelectedRoomId}
            onSelectRoom={selectRoomFromOverview}
            emptyMessage="No spaces or rooms found under this root space."
          />
        {/if}
      </section>
    {/if}
  </section>
{:else}
  <div class="flex flex-col h-full">
    <MessageTimeline
      {messages}
      roomId={$shellSelectedRoomId || ""}
      selectedRoomId={$shellSelectedRoomId}
      currentUserId={$shellCurrentUserId || null}
      pickerCustomEmoji={$shellPickerCustomEmoji}
      roomEncrypted={selectedRoomEncrypted()}
      roomName={$shellChats.find((chat) => chat.roomId === $shellSelectedRoomId)?.displayName ?? ""}
      {loadingMessages}
      {activeLoadKind}
      {streamMessageCount}
      error={errorMessage}
      {nextFrom}
      isSending={sendingMessage}
      onTimelineElementChange={(element) => timelineElement = element}
      onScroll={handleTimelineScroll}
      onLoadOlder={loadOlder}
      onRetryMessage={retryMessage}
      onToggleReaction={handleToggleReaction}
    />

    <MessageComposer
      draft={messageDraft}
      error={composerErrorMessage}
      isSending={sendingMessage}
      isDisabled={!$shellSelectedRoomId}
      pickerCustomEmoji={$shellPickerCustomEmoji}
      placeholder={$shellSelectedRoomId ? "Write a message..." : "Select a room to compose"}
      onSubmit={sendDraftMessage}
      onDraftChange={(d) => messageDraft = d}
    />
  </div>
{/if}
