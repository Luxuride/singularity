import type {
  MatrixChatSummary,
  MatrixChatMessage,
  MatrixChatMessageStreamEvent,
  MatrixSelectedRoomMessagesEvent,
} from "./types";

/// Whether a string is a media URL (mxc, http(s), or the local asset scheme)
/// rather than a shortcode token.
export function isMediaUrl(value: string): boolean {
  return (
    value.startsWith("mxc://") ||
    value.startsWith("http://") ||
    value.startsWith("https://") ||
    value.startsWith("asset://")
  );
}

/// The backend resolves all media to `asset://` URLs (or leaves http(s) URLs
/// untouched). In a webkit2gtk dev session the page origin is the Vite dev
/// server, which is not allowed to load the `asset://` custom scheme; Vite's
/// `/@fs` route serves the same file from the http origin instead. Production
/// builds load from the packaged origin, where `asset://` works.
export function normalizeImageUrl(imageUrl: string | null | undefined): string | null {
  if (!imageUrl) {
    return null;
  }

  if (import.meta.env.DEV && imageUrl.startsWith("asset://localhost/")) {
    return `/@fs${decodeURIComponent(imageUrl.slice("asset://localhost/".length))}`;
  }

  return imageUrl;
}

export function normalizeMessageImageUrl(message: MatrixChatMessage): MatrixChatMessage {
  return {
    ...message,
    imageUrl: normalizeImageUrl(message.imageUrl),
    thumbnailUrl: normalizeImageUrl(message.thumbnailUrl),
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

type VitestContext = {
  describe: any;
  it: any;
  expect: any;
  vi: any;
};

// SAFETY: vitest injects `import.meta.vitest` at transform time; the runtime
// object has no static type and is only present in test builds.
const vitest = (import.meta as unknown as { vitest?: VitestContext }).vitest;

if (vitest) {
  const { describe, it, expect, vi } = vitest;

  vi.mock("@tauri-apps/api/core", () => ({
    convertFileSrc: (input: string) => `converted:${input}`,
  }));

  describe("normalizeImageUrl", () => {
    it("preserves asset path URLs from the backend", () => {
      const raw = "asset://localhost/home/lux/.cache/eu.luxuride.singularity/media-cache/img%2D123.png";
      vi.stubEnv("DEV", false);
      expect(normalizeImageUrl(raw)).toBe(raw);
      vi.unstubAllEnvs();
    });

    it("rewrites asset URLs to the Vite /@fs route in dev", () => {
      const raw = "asset://localhost/home/lux/.cache/eu.luxuride.singularity/media-cache/img%2D123.png";
      vi.stubEnv("DEV", true);
      expect(normalizeImageUrl(raw)).toBe("/@fs/home/lux/.cache/eu.luxuride.singularity/media-cache/img-123.png");
      vi.unstubAllEnvs();
    });

    it("normalizes custom emoji URLs in messages", () => {
      const message: MatrixChatMessage = {
        eventId: "$event",
        inReplyToEventId: null,
        sender: "@alice:example.org",
        timestamp: 1,
        body: "body",
        formattedBody: null,
        messageType: "m.text",
        imageUrl: null,
        thumbnailUrl: null,
        customEmojis: [{ shortcode: "wave", url: "asset://localhost/img.png" }],
        reactions: [],
        encrypted: false,
        decryptionStatus: "plaintext",
        verificationStatus: "unknown",
      };
      const normalized = normalizeMessageImageUrl(message);
      expect(normalized.customEmojis[0].url).toBe("asset://localhost/img.png");
    });
  });
}
