import { convertFileSrc } from "@tauri-apps/api/core";

import type {
  MatrixChatSummary,
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

export function normalizeImageUrl(imageUrl: string | null): string | null {
  if (!imageUrl) {
    return imageUrl;
  }

  if (
    imageUrl.startsWith("data:") ||
    imageUrl.startsWith("http://") ||
    imageUrl.startsWith("https://") ||
    imageUrl.startsWith("matrix-media://") ||
    imageUrl.startsWith("asset:") ||
    imageUrl.startsWith("tauri://")
  ) {
    return imageUrl;
  }

  if (!isLikelyAbsoluteFilePath(imageUrl)) {
    return imageUrl;
  }

  return convertFileSrc(imageUrl);
}

export function normalizeMessageImageUrl(message: MatrixChatMessage): MatrixChatMessage {
  return {
    ...message,
    imageUrl: normalizeImageUrl(message.imageUrl),
    customEmojis: message.customEmojis.map((emoji) => ({
      ...emoji,
      url: normalizeImageUrl(emoji.url) ?? emoji.url,
    })),
  };
}

export function normalizeChatSummaryImageUrl(chat: MatrixChatSummary): MatrixChatSummary {
  return {
    ...chat,
    imageUrl: normalizeImageUrl(chat.imageUrl),
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
