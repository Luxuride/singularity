<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { onMount, tick } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { open } from "@tauri-apps/plugin-dialog";

  import {
    matrixCancelMediaTranscode,
    matrixGetSpaceBrowse,
    matrixSendChatMessage,
    matrixSendMediaFile,
    matrixStreamChatMessages,
    matrixTriggerRoomUpdate,
    matrixToggleReaction,
  } from "$lib/chats/api";
  import { subscribeToRoomUpdates } from "$lib/chats/realtime";
  import {
    shellChats,
    shellCurrentUserId,
    shellPickerCustomEmoji,
    shellRootBrowseRooms,
    shellRootScopedRooms,
    shellRootSpaces,
    shellSelectedRootSpaceId,
    shellSelectedRoomId,
  } from "$lib/chats/shell";
  import type {
    MatrixChatSummary,
    MatrixChatMessage,
    MatrixChatMessageStreamEvent,
    MatrixJoinBatchResult,
    MatrixJoinTargetPreview,
    MatrixMediaTranscodeProgressEvent,
    MatrixSelectedRoomMessagesEvent,
    MatrixSendChatMessageRequest,
    MatrixMessageLoadKind,
    MatrixReactionSummary,
  } from "$lib/chats/types";
  import {
    normalizeReactionKey,
    normalizeShortcodesToEmoji,
  } from "$lib/emoji/picker";
  import { RootSpaceBrowseList } from "$lib/components/navigation";
  import JoinRoomDialog from "$lib/components/navigation/spaces/JoinRoomDialog.svelte";
  import { MessageComposer } from "$lib/components/messaging/composer";
  import { MessageTimeline } from "$lib/components/messaging/timeline";
  import type { TimelineMessage } from "$lib/components/messaging/shared";

  let loadingMessages = $state(false);
  let errorMessage = $state("");
  let composerErrorMessage = $state("");
  let messageDraft = $state("");
  let sendingMessage = $state(false);
  let replyToMessage = $state<TimelineMessage | null>(null);
  let composerFocusNonce = $state(0);
  let pendingMedia = $state<{
    filePath: string;
    fileName: string;
    messageType: "m.image" | "m.video" | "m.file";
    compressMedia: boolean;
  } | null>(null);
  let sendingMedia = $state(false);
  let mediaErrorMessage = $state("");
  let activeMediaFilePath = $state("");
  let mediaTranscodeProgress = $state<MatrixMediaTranscodeProgressEvent | null>(null);

  const EVENT_MEDIA_TRANSCODE_PROGRESS = "matrix://media/transcode/progress";

  let messages = $state<TimelineMessage[]>([]);
  let nextFrom = $state<string | null>(null);
  let timelineElement = $state<HTMLElement | null>(null);
  let activeStreamId = $state("");
  let activeLoadKind = $state<MatrixMessageLoadKind | null>(null);
  let streamMessageCount = $state(0);
  let joinDialogOpen = $state(false);
  let joinDialogTitle = $state("Confirm join");
  let joinDialogConfirmLabel = $state("Join");
  let joinDialogTargets = $state<MatrixJoinTargetPreview[]>([]);
  let expandedSpacePollTargets = $state<string[]>([]);
  let hydratingBrowseSpaceIds = $state<string[]>([]);

  const expandedSpacePollInFlight = new Set<string>();
  let expandedSpaceRefreshQueued = false;

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
    let unlistenTranscode = () => {};

    void (async () => {
      unlisten = await subscribeToRoomUpdates({
        onRoomAdded: queueExpandedSpaceRefresh,
        onRoomUpdated: queueExpandedSpaceRefresh,
        onRoomRemoved: () => {
          queueExpandedSpaceRefresh();
        },
        onSelectedRoomMessages: applySelectedRoomMessages,
        onChatMessagesStream: applyChatMessageStream,
      });

      unlistenTranscode = await listen<MatrixMediaTranscodeProgressEvent>(
        EVENT_MEDIA_TRANSCODE_PROGRESS,
        (event) => {
          if (event.payload.filePath !== activeMediaFilePath) {
            return;
          }

          mediaTranscodeProgress = event.payload;
        },
      );
    })();

    return () => {
      unlisten();
      unlistenTranscode();
    };
  });

  $effect(() => {
    const rootSpaceId = $shellSelectedRootSpaceId;
    if (!rootSpaceId || rootSpaceId.startsWith("virtual:")) {
      expandedSpacePollTargets = [];
      expandedSpacePollInFlight.clear();
      return;
    }

    expandedSpacePollTargets = expandedSpacePollTargets.filter(
      (spaceId) => spaceId === rootSpaceId || $shellRootBrowseRooms.some((room) => room.roomId === spaceId),
    );
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
      pendingMedia = null;
      mediaErrorMessage = "";
      activeMediaFilePath = "";
      mediaTranscodeProgress = null;
      replyToMessage = null;
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
    pendingMedia = null;
    mediaErrorMessage = "";
    activeMediaFilePath = "";
    mediaTranscodeProgress = null;
    replyToMessage = null;

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
          olderCandidates.push(message);
          continue;
        }

        if (index > lastSeenIndex) {
          newerCandidates.push(message);
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
    for (const message of newerCandidates) {
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

    messages = [...prependOlder, ...messages, ...appendNewer];
    if (appendNewer.length) {
      queuePinTimelineToBottom(payload.roomId);
    }
  }

  function buildOptimisticMessage(body: string, inReplyToEventId: string | null): TimelineMessage {
    const encryptedRoom = selectedRoomEncrypted();

    return {
      eventId: null,
      inReplyToEventId,
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

  function buildOptimisticMediaMessage(): TimelineMessage {
    const encryptedRoom = selectedRoomEncrypted();

    if (!pendingMedia) {
      return buildOptimisticMessage("Attachment", null);
    }

    return {
      eventId: null,
      inReplyToEventId: null,
      sender: $shellCurrentUserId || "You",
      timestamp: Date.now(),
      body: pendingMedia.fileName,
      formattedBody: null,
      messageType: pendingMedia.messageType,
      imageUrl:
        pendingMedia.messageType === "m.file"
          ? null
          : convertFileSrc(pendingMedia.filePath),
      customEmojis: [],
      reactions: [],
      encrypted: encryptedRoom,
      decryptionStatus: encryptedRoom ? "decrypted" : "plaintext",
      verificationStatus: "unknown",
      localId: createStreamId(),
      sendState: "sending",
    };
  }

  function detectMediaKind(filePath: string): "m.image" | "m.video" | "m.file" {
    const extension = filePath.split(".").pop()?.toLowerCase() ?? "";

    if (["png", "jpg", "jpeg", "gif", "webp", "bmp"].includes(extension)) {
      return "m.image";
    }

    if (["mp4", "mkv", "mov", "webm", "avi"].includes(extension)) {
      return "m.video";
    }

    return "m.file";
  }

  function createPendingMedia(filePath: string) {
    pendingMedia = {
      filePath,
      fileName: filePath.split(/[\\/]/).pop() || "attachment",
      messageType: detectMediaKind(filePath),
      compressMedia: true,
    };
    mediaErrorMessage = "";
  }

  async function chooseAttachment() {
    const selected = await open({
      multiple: false,
      directory: false,
      title: "Choose a file to send",
    });

    if (typeof selected === "string" && selected.length > 0) {
      createPendingMedia(selected);
    }
  }

  function handlePendingMediaEscape(event: KeyboardEvent) {
    if (event.key !== "Escape" || !pendingMedia) {
      return;
    }

    event.preventDefault();
    void cancelPendingMedia();
  }

  function handlePendingMediaBackdropClick(event: MouseEvent) {
    if (!pendingMedia || event.currentTarget !== event.target) {
      return;
    }

    void cancelPendingMedia();
  }

  function handlePendingMediaBackdropKeydown(event: KeyboardEvent) {
    if (!pendingMedia) {
      return;
    }

    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      void cancelPendingMedia();
    }
  }

  onMount(() => {
    window.addEventListener("keydown", handlePendingMediaEscape);

    return () => {
      window.removeEventListener("keydown", handlePendingMediaEscape);
    };
  });

  async function sendPendingMedia() {
    if (!pendingMedia || sendingMedia || !$shellSelectedRoomId) {
      return;
    }

    const mediaDraft = pendingMedia;
    activeMediaFilePath = mediaDraft.filePath;
    mediaTranscodeProgress = {
      roomId: $shellSelectedRoomId,
      filePath: mediaDraft.filePath,
      stage: mediaDraft.compressMedia ? "preparing" : "uploading",
      progress: 0,
      hardwareMode: "",
    };
    const optimistic = buildOptimisticMediaMessage();
    const localId = optimistic.localId;

    if (!localId) {
      return;
    }

    messages = [...messages, optimistic];
    queuePinTimelineToBottom($shellSelectedRoomId);

    sendingMedia = true;
    mediaErrorMessage = "";

    try {
      await matrixSendMediaFile({
        roomId: $shellSelectedRoomId,
        filePath: mediaDraft.filePath,
        compressMedia: mediaDraft.compressMedia,
      });

      pendingMedia = null;
      mediaTranscodeProgress = null;

      messages = messages.map((message) => {
        if (message.localId !== localId) {
          return message;
        }

        return {
          ...message,
          sendState: undefined,
          localId: undefined,
        };
      });
    } catch (error) {
      const errorText = error instanceof Error ? error.message : "Failed to send media";
      const cancelled = errorText.toLowerCase().includes("cancelled by user");
      mediaTranscodeProgress = null;

      if (cancelled) {
        mediaErrorMessage = "";
        messages = messages.filter((message) => message.localId !== localId);
      } else {
        mediaErrorMessage = errorText;
        messages = messages.map((message) => {
          if (message.localId !== localId) {
            return message;
          }

          return {
            ...message,
            sendState: "failed",
          };
        });
      }
    } finally {
      sendingMedia = false;
      activeMediaFilePath = "";
    }
  }

  async function cancelPendingMedia() {
    if (!pendingMedia) {
      return;
    }

    const roomId = $shellSelectedRoomId;
    const mediaDraft = pendingMedia;

    if (sendingMedia && roomId && mediaDraft.compressMedia && mediaDraft.messageType !== "m.file") {
      try {
        await matrixCancelMediaTranscode({
          roomId,
          filePath: mediaDraft.filePath,
        });
      } catch {
        // Ignore cancellation command errors and close preview anyway.
      }
    }

    mediaErrorMessage = "";
    mediaTranscodeProgress = null;
    activeMediaFilePath = "";
    pendingMedia = null;
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
          formattedBody: response.formattedBody || message.formattedBody,
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

  async function sendDraftMessage(rawDraft?: string) {
    const roomId = $shellSelectedRoomId;
    if (!roomId) {
      return;
    }

    const rawBody = (rawDraft ?? messageDraft).trim();
    if (!rawBody) {
      composerErrorMessage = "Message cannot be empty";
      return;
    }

    const body = rawBody;
    const inReplyToEventId = replyToMessage?.eventId ?? null;

    composerErrorMessage = "";
    messageDraft = "";
    replyToMessage = null;

    const optimistic = buildOptimisticMessage(body, inReplyToEventId);
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
        inReplyToEventId,
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
        inReplyToEventId: message.inReplyToEventId,
      },
      message.localId,
    );
  }

  function handleReplyToMessage(message: TimelineMessage) {
    if (!message.eventId) {
      return;
    }

    replyToMessage = message;
    composerFocusNonce += 1;
  }

  function clearReplyToMessage() {
    replyToMessage = null;
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
    $shellRootSpaces.find((room) => room.roomId === $shellSelectedRootSpaceId)?.displayName ?? "Server",
  );

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

  function openJoinDialog(request: {
    title: string;
    confirmLabel?: string;
    targets: MatrixJoinTargetPreview[];
  }) {
    if (request.targets.length === 0) {
      return;
    }

    joinDialogTitle = request.title;
    joinDialogConfirmLabel = request.confirmLabel ?? "Join";
    joinDialogTargets = request.targets;
    joinDialogOpen = true;
  }

  async function handleJoinDialogJoined(result: MatrixJoinBatchResult) {
    const plannedTargets = joinDialogTargets;
    const joinedRoomTargets = plannedTargets.filter((target) => target.kind === "room");
    const includesSpaceTargets = plannedTargets.some((target) => target.kind === "space");
    const shouldOpenJoinedRoom = !includesSpaceTargets && joinedRoomTargets.length === 1;
    const joinedRoomId = shouldOpenJoinedRoom ? (result.joinedRoomIds.at(-1) ?? "") : "";

    joinDialogOpen = false;
    joinDialogTargets = [];

    await matrixTriggerRoomUpdate();

    if (joinedRoomId) {
      await selectRoomFromOverview(joinedRoomId);
    }
  }

  async function hydrateSpaceBrowse(spaceId: string) {
    if (!spaceId || spaceId.startsWith("virtual:")) {
      return;
    }

    const selectedRootSpaceId = $shellSelectedRootSpaceId;
    if (!selectedRootSpaceId || selectedRootSpaceId.startsWith("virtual:")) {
      return;
    }

    const inFlightKey = `root:${selectedRootSpaceId}`;

    if (expandedSpacePollInFlight.has(inFlightKey)) {
      return;
    }

    expandedSpacePollInFlight.add(inFlightKey);
    if (!hydratingBrowseSpaceIds.includes(spaceId)) {
      hydratingBrowseSpaceIds = [...hydratingBrowseSpaceIds, spaceId];
    }

    try {
      const browse = await matrixGetSpaceBrowse(selectedRootSpaceId);
      if ($shellSelectedRootSpaceId !== selectedRootSpaceId) {
        return;
      }

      shellRootBrowseRooms.set(browse.rooms);
    } catch {
      // Ignore lazy hydrate failures; the existing hierarchy remains usable.
    } finally {
      expandedSpacePollInFlight.delete(inFlightKey);
      hydratingBrowseSpaceIds = hydratingBrowseSpaceIds.filter((candidate) => candidate !== spaceId);
    }
  }

  function queueExpandedSpaceRefresh() {
    if (expandedSpaceRefreshQueued || expandedSpacePollTargets.length === 0) {
      return;
    }

    expandedSpaceRefreshQueued = true;
    queueMicrotask(() => {
      expandedSpaceRefreshQueued = false;
      for (const spaceId of expandedSpacePollTargets) {
        void hydrateSpaceBrowse(spaceId);
      }
    });
  }

  async function hydrateExpandedSpace(spaceId: string, expanded: boolean) {
    if (!expanded) {
      expandedSpacePollTargets = expandedSpacePollTargets.filter((candidate) => candidate !== spaceId);
      return;
    }

    if (!expandedSpacePollTargets.includes(spaceId)) {
      expandedSpacePollTargets = [...expandedSpacePollTargets, spaceId];
    }

    await hydrateSpaceBrowse(spaceId);
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
        {#if $shellRootBrowseRooms.length === 0}
          <p class="text-sm text-surface-700-300">No spaces or rooms found under this root space.</p>
        {:else}
          <RootSpaceBrowseList
            rooms={$shellRootBrowseRooms}
            selectedRoomId={$shellSelectedRoomId}
            loadingSpaceIds={hydratingBrowseSpaceIds}
            onSelectRoom={selectRoomFromOverview}
            onJoinTargets={openJoinDialog}
            onExpandSpace={hydrateExpandedSpace}
            emptyMessage="No spaces or rooms found under this root space."
          />
        {/if}
      </section>
    {/if}

    <JoinRoomDialog
      open={joinDialogOpen}
      onClose={() => {
        joinDialogOpen = false;
        joinDialogTargets = [];
      }}
      targets={joinDialogTargets}
      title={joinDialogTitle}
      confirmLabel={joinDialogConfirmLabel}
      allowManualEntry={false}
      onJoined={handleJoinDialogJoined}
    />
  </section>
{:else}
  <div class="flex flex-col h-full min-w-0 overflow-x-hidden">
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
      onReplyToMessage={handleReplyToMessage}
    />

    <MessageComposer
      draft={messageDraft}
      error={composerErrorMessage}
      isSending={sendingMessage}
      isDisabled={!$shellSelectedRoomId}
      pickerCustomEmoji={$shellPickerCustomEmoji}
      {replyToMessage}
      focusNonce={composerFocusNonce}
      placeholder={$shellSelectedRoomId ? "Write a message..." : "Select a room to compose"}
      onSubmit={sendDraftMessage}
      onDraftChange={(d) => messageDraft = d}
      onChooseAttachment={chooseAttachment}
      onPasteAttachmentPath={createPendingMedia}
      onClearReply={clearReplyToMessage}
    />

    {#if pendingMedia}
      <div
        class="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4 backdrop-blur-sm"
        role="button"
        tabindex="0"
        onclick={handlePendingMediaBackdropClick}
        onkeydown={handlePendingMediaBackdropKeydown}
      >
        <div class="w-full max-w-5xl rounded-2xl border border-surface-300-700 bg-surface-100-900 shadow-2xl">
          <div class="flex items-center justify-between border-b border-surface-300-700 px-5 py-4">
            <div>
              <p class="text-lg font-semibold text-surface-900-100">Send file</p>
              <p class="text-sm text-surface-600-400">{pendingMedia.fileName}</p>
            </div>
            <button class="btn preset-outlined" type="button" onclick={cancelPendingMedia}>
              Cancel
            </button>
          </div>

          <div class="grid gap-4 p-5 lg:grid-cols-[minmax(0,2fr)_minmax(18rem,1fr)]">
            <div class="flex min-h-[24rem] items-center justify-center rounded-xl bg-surface-200-800 p-4">
              {#if pendingMedia.messageType === "m.image"}
                <img
                  src={convertFileSrc(pendingMedia.filePath)}
                  alt={pendingMedia.fileName}
                  class="max-h-[70vh] w-full rounded-lg object-contain"
                />
              {:else if pendingMedia.messageType === "m.video"}
                <!-- svelte-ignore a11y_media_has_caption -->
                <video
                  src={convertFileSrc(pendingMedia.filePath)}
                  controls
                  playsinline
                  class="max-h-[70vh] w-full rounded-lg bg-black"
                ></video>
              {:else}
                <div class="text-center">
                  <div class="text-5xl">📎</div>
                  <p class="mt-3 text-lg font-medium">{pendingMedia.fileName}</p>
                </div>
              {/if}
            </div>

            <div class="space-y-4 rounded-xl border border-surface-300-700 bg-surface-100-900 p-4">
              {#if pendingMedia.messageType !== "m.file"}
                <label class="flex items-start gap-3 rounded-lg border border-surface-300-700 p-3">
                  <input type="checkbox" bind:checked={pendingMedia.compressMedia} class="mt-1" />
                  <span>
                    <span class="block font-medium">Compress before sending</span>
                    <span class="block text-sm text-surface-600-400">
                      Images become WebP. Videos become VP9 WebM.
                    </span>
                  </span>
                </label>
              {/if}

              {#if sendingMedia}
                <div class="space-y-2 rounded-lg border border-surface-300-700 p-3">
                  <div class="flex items-center justify-between gap-3 text-xs uppercase tracking-wide text-surface-600-400">
                    <span>
                      {#if mediaTranscodeProgress?.stage === "uploading"}
                        Upload
                      {:else if mediaTranscodeProgress?.hardwareMode === "vaapi"}
                        VA
                      {:else if mediaTranscodeProgress?.hardwareMode === "cuda"}
                        CUDA
                      {:else if mediaTranscodeProgress?.hardwareMode === "software"}
                        Software
                      {:else}
                        Detecting...
                      {/if}
                    </span>
                    <span>{Math.round(mediaTranscodeProgress?.progress ?? 0)}%</span>
                  </div>
                  <div class="h-2 overflow-hidden rounded-full bg-surface-300-700">
                    <div
                      class="h-full rounded-full bg-primary-500 transition-[width] duration-200"
                      style={`width: ${Math.max(4, mediaTranscodeProgress?.progress ?? 0)}%`}
                    ></div>
                  </div>
                  <p class="text-sm text-surface-600-400">
                    {#if mediaTranscodeProgress?.stage === "transcoding"}
                      {pendingMedia.messageType === "m.video" ? "Transcoding video..." : "Transcoding image..."}
                    {:else if mediaTranscodeProgress?.stage === "uploading"}
                      Uploading media...
                    {:else if mediaTranscodeProgress?.stage === "finalizing"}
                      Finalizing the output...
                    {:else if mediaTranscodeProgress?.stage === "cancelled"}
                      Cancelling transcode...
                    {:else}
                      Preparing media...
                    {/if}
                  </p>
                </div>
              {/if}

              {#if mediaErrorMessage}
                <div class="rounded-lg border border-red-500/40 bg-red-500/10 p-3 text-sm text-red-700">
                  {mediaErrorMessage}
                </div>
              {/if}

              <button
                type="button"
                class="btn preset-filled w-full"
                disabled={sendingMedia || !$shellSelectedRoomId}
                onclick={sendPendingMedia}
              >
                {#if sendingMedia}Sending...{:else}Send file{/if}
              </button>
            </div>
          </div>
        </div>
      </div>
    {/if}
  </div>
{/if}
