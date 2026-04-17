import { convertFileSrc } from "@tauri-apps/api/core";

import type {
  MatrixChatSummary,
  MatrixChatMessage,
  MatrixChatMessageStreamEvent,
  MatrixSelectedRoomMessagesEvent,
} from "./types";

export function normalizeImageUrl(imageUrl: string | null): string | null {
  if (!imageUrl) {
    return null;
  }

  if (
    imageUrl.startsWith("data:") ||
    imageUrl.startsWith("http://") ||
    imageUrl.startsWith("https://") ||
    imageUrl.startsWith("matrix-media://") ||
    imageUrl.startsWith("tauri://") ||
    imageUrl.startsWith("asset://")
  ) {
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
      url: emoji.url,
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

type VitestContext = {
  describe: any;
  it: any;
  expect: any;
  vi: any;
};

const vitest = (import.meta as unknown as { vitest?: VitestContext }).vitest;

if (vitest) {
  const { describe, it, expect, vi } = vitest;

  vi.mock("@tauri-apps/api/core", () => ({
    convertFileSrc: (input: string) => `converted:${input}`,
  }));

  describe("normalizeImageUrl", () => {
    it("preserves asset path URLs from the backend", () => {
      const raw = "asset://localhost/home/lux/.cache/eu.luxuride.singularity/media-cache/img%2D123.png";
      expect(normalizeImageUrl(raw)).toBe(raw);
    });

    it("leaves matrix-media URLs unchanged", () => {
      const raw = "matrix-media://localhost/img-123.png";
      expect(normalizeImageUrl(raw)).toBe(raw);
    });
  });
}
