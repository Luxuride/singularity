import type { MatrixChatMessage } from "$lib/chats/types";

export interface TimelineMessage extends MatrixChatMessage {
  localId?: string;
  sendState?: "sending" | "failed";
}

export type RetryMessageHandler = (message: TimelineMessage) => void;

export type ToggleReactionHandler = (message: TimelineMessage, key: string) => void;

export type JumpToMessageHandler = (eventId: string) => void;
