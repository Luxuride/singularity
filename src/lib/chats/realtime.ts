import { listen } from "@tauri-apps/api/event";

import type {
  MatrixChatSummary,
  MatrixChatMessageStreamEvent,
  MatrixRoomRemovedEvent,
  MatrixSelectedRoomMessagesEvent,
  MatrixVerificationStateChangedEvent,
} from "./types";

const EVENT_ROOM_ADDED = "matrix://rooms/added";
const EVENT_ROOM_UPDATED = "matrix://rooms/updated";
const EVENT_ROOM_REMOVED = "matrix://rooms/removed";
const EVENT_SELECTED_ROOM_MESSAGES = "matrix://rooms/selected/messages";
const EVENT_CHAT_MESSAGES_STREAM = "matrix://rooms/messages/stream";
const EVENT_VERIFICATION_STATE_CHANGED = "matrix://verification/state";

export interface RoomUpdateHandlers {
  onRoomAdded: (room: MatrixChatSummary) => void;
  onRoomUpdated: (room: MatrixChatSummary) => void;
  onRoomRemoved: (payload: MatrixRoomRemovedEvent) => void;
  onSelectedRoomMessages: (payload: MatrixSelectedRoomMessagesEvent) => void;
  onChatMessagesStream: (payload: MatrixChatMessageStreamEvent) => void;
  onVerificationStateChanged?: (payload: MatrixVerificationStateChangedEvent) => void;
}

export async function subscribeToRoomUpdates(handlers: RoomUpdateHandlers): Promise<() => void> {
  const unlisteners = await Promise.all([
    listen<MatrixChatSummary>(EVENT_ROOM_ADDED, (event) => handlers.onRoomAdded(event.payload)),
    listen<MatrixChatSummary>(EVENT_ROOM_UPDATED, (event) => handlers.onRoomUpdated(event.payload)),
    listen<MatrixRoomRemovedEvent>(EVENT_ROOM_REMOVED, (event) => handlers.onRoomRemoved(event.payload)),
    listen<MatrixSelectedRoomMessagesEvent>(EVENT_SELECTED_ROOM_MESSAGES, (event) =>
      handlers.onSelectedRoomMessages(event.payload),
    ),
    listen<MatrixChatMessageStreamEvent>(EVENT_CHAT_MESSAGES_STREAM, (event) =>
      handlers.onChatMessagesStream(event.payload),
    ),
    listen<MatrixVerificationStateChangedEvent>(EVENT_VERIFICATION_STATE_CHANGED, (event) => {
      handlers.onVerificationStateChanged?.(event.payload);
    }),
  ]);

  return () => {
    for (const unlisten of unlisteners) {
      unlisten();
    }
  };
}
