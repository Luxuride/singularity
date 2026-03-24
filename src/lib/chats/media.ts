import { convertFileSrc } from "@tauri-apps/api/core";

import type {
  MatrixChatMessage,
  MatrixChatMessageStreamEvent,
  MatrixSelectedRoomMessagesEvent,
} from "./types";

function isLikelyAbsoluteFilePath(value: string): boolean {
  if (!value) {
    return false;
  }

  if (value.startsWith("/")) {
    return true;
  }

  return /^[a-zA-Z]:[\\/]/.test(value);
}

export function normalizeMessageImageUrl(message: MatrixChatMessage): MatrixChatMessage {
  const imageUrl = message.imageUrl;
  if (!imageUrl) {
    return message;
  }

  if (
    imageUrl.startsWith("data:") ||
    imageUrl.startsWith("http://") ||
    imageUrl.startsWith("https://") ||
    imageUrl.startsWith("asset:") ||
    imageUrl.startsWith("tauri://")
  ) {
    return message;
  }

  if (!isLikelyAbsoluteFilePath(imageUrl)) {
    return message;
  }

  return {
    ...message,
    imageUrl: convertFileSrc(imageUrl),
  };
}

export function normalizeSelectedRoomMessagesEvent(
  payload: MatrixSelectedRoomMessagesEvent,
): MatrixSelectedRoomMessagesEvent {
  return {
    ...payload,
    messages: payload.messages.map(normalizeMessageImageUrl),
  };
}

export function normalizeChatMessageStreamEvent(
  payload: MatrixChatMessageStreamEvent,
): MatrixChatMessageStreamEvent {
  if (!payload.message) {
    return payload;
  }

  return {
    ...payload,
    message: normalizeMessageImageUrl(payload.message),
  };
}
