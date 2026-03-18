<script lang="ts">
  import { onMount, tick } from "svelte";

  import {
    matrixSendChatMessage,
    matrixStreamChatMessages,
    matrixTriggerRoomUpdate,
  } from "$lib/chats/api";
  import { subscribeToRoomUpdates } from "$lib/chats/realtime";
  import { shellChats, shellCurrentUserId, shellSelectedRoomId } from "$lib/chats/shell";
  import type {
    MatrixChatMessage,
    MatrixChatMessageStreamEvent,
    MatrixSelectedRoomMessagesEvent,
    MatrixSendChatMessageRequest,
    MatrixMessageLoadKind,
  } from "$lib/chats/types";
  import MessageTimeline from "$lib/components/messaging/MessageTimeline.svelte";
  import MessageComposer from "$lib/components/messaging/MessageComposer.svelte";

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
    if (!selectedRoomId) {
      return;
    }

    void matrixTriggerRoomUpdate({ selectedRoomId });
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

    // Backend streams newest -> older for immediate delivery; prepend keeps timeline ordered.
    messages = [payload.message, ...messages];

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
      messageType: "m.text",
      imageUrl: null,
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

    const body = messageDraft.trim();
    if (!body) {
      composerErrorMessage = "Message cannot be empty";
      return;
    }

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
      },
      localId,
    );
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
        limit: 20,
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
</script>

<div class="flex flex-col">
  <MessageTimeline
    {messages}
    roomId={$shellSelectedRoomId || ""}
    selectedRoomId={$shellSelectedRoomId}
    roomEncrypted={selectedRoomEncrypted()}
    roomName={$shellChats.find((chat) => chat.roomId === $shellSelectedRoomId)?.displayName ?? ""}
    {loadingMessages}
    {activeLoadKind}
    {streamMessageCount}
    error={errorMessage}
    {nextFrom}
    isSending={sendingMessage}
    onScroll={handleTimelineScroll}
    onLoadOlder={loadOlder}
    onRetryMessage={retryMessage}
  />

  <MessageComposer
    draft={messageDraft}
    error={composerErrorMessage}
    isSending={sendingMessage}
    isDisabled={!$shellSelectedRoomId}
    placeholder={$shellSelectedRoomId ? "Write a message..." : "Select a room to compose"}
    onSubmit={sendDraftMessage}
    onDraftChange={(d) => messageDraft = d}
  />
</div>
