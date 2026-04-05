import type { PickerCustomEmoji } from "$lib/emoji/picker";

import type { TimelineMessage } from "../shared";

export type MessageBodyPart =
  | { type: "text"; value: string }
  | { type: "emoji"; shortcode: string; url: string };

const CUSTOM_EMOJI_PATTERN = /(:[A-Za-z0-9_+\-]+:)/g;
const SINGLE_UNICODE_EMOJI_PATTERN =
  /^\s*(?:\p{Regional_Indicator}{2}|(?:\p{Extended_Pictographic}|\p{Emoji_Presentation})(?:\p{Emoji_Modifier})?(?:\uFE0F|\uFE0E)?(?:\u200D(?:\p{Extended_Pictographic}|\p{Emoji_Presentation})(?:\p{Emoji_Modifier})?(?:\uFE0F|\uFE0E)?)*)\s*$/u;

export function shortcodeToken(value: string): string {
  const clean = value.trim().replace(/^:+|:+$/g, "");
  return clean ? `:${clean}:` : "";
}

export function emojiName(value: string): string {
  return value.trim().replace(/^:+|:+$/g, "");
}

export function buildPickerEmojiBySourceUrl(pickerCustomEmoji: PickerCustomEmoji[]): Map<string, string> {
  const map = new Map<string, string>();
  for (const emoji of pickerCustomEmoji) {
    if (emoji.sourceUrl?.trim()) {
      map.set(emoji.sourceUrl.trim(), emoji.url);
    }
  }

  return map;
}

export function buildEmojiByShortcodeToken(
  message: TimelineMessage,
  pickerCustomEmoji: PickerCustomEmoji[],
): Map<string, string> {
  const map = new Map<string, string>();

  for (const emoji of message.customEmojis ?? []) {
    const token = shortcodeToken(emoji.shortcode);
    if (token && !map.has(token)) {
      map.set(token, emoji.url);
    }
  }

  for (const emoji of pickerCustomEmoji) {
    for (const shortcode of emoji.shortcodes ?? []) {
      const token = shortcodeToken(shortcode);
      if (token && !map.has(token)) {
        map.set(token, emoji.url);
      }
    }
  }

  return map;
}

export function customEmojiUrlForToken(
  token: string,
  emojiByShortcodeToken: Map<string, string>,
  pickerEmojiBySourceUrl: Map<string, string>,
): string | null {
  const trimmed = token.trim();
  if (!trimmed) {
    return null;
  }

  if (trimmed.startsWith("mxc://") || trimmed.startsWith("http://") || trimmed.startsWith("https://")) {
    const bySource = pickerEmojiBySourceUrl.get(trimmed);
    if (bySource) {
      return bySource;
    }
  }

  const normalizedToken = shortcodeToken(trimmed);
  if (!normalizedToken) {
    return null;
  }

  return emojiByShortcodeToken.get(normalizedToken) ?? null;
}

export function reactionDisplayName(key: string, message: TimelineMessage, pickerCustomEmoji: PickerCustomEmoji[]): string {
  const trimmed = key.trim();
  if (!trimmed) {
    return "";
  }

  const isSourceKey =
    trimmed.startsWith("mxc://") ||
    trimmed.startsWith("http://") ||
    trimmed.startsWith("https://");

  if (isSourceKey) {
    const fromMessageEmoji = (message.customEmojis ?? []).find((emoji) => emoji.url?.trim() === trimmed);
    if (fromMessageEmoji?.shortcode) {
      return emojiName(fromMessageEmoji.shortcode);
    }

    const fromPickerEmoji = pickerCustomEmoji.find((emoji) => emoji.sourceUrl?.trim() === trimmed);
    const pickerShortcode = fromPickerEmoji?.shortcodes?.find((entry) => entry.trim().length > 0);
    if (pickerShortcode) {
      return emojiName(pickerShortcode);
    }
  }

  return emojiName(trimmed);
}

export function buildMessageBodyParts(
  message: TimelineMessage,
  pickerCustomEmoji: PickerCustomEmoji[],
  emojiByShortcodeToken: Map<string, string>,
): MessageBodyPart[] {
  if (!message.customEmojis?.length && pickerCustomEmoji.length === 0) {
    return [{ type: "text", value: message.body }];
  }

  const segments = message.body.split(CUSTOM_EMOJI_PATTERN);
  const parts: MessageBodyPart[] = [];

  for (const segment of segments) {
    if (!segment) {
      continue;
    }

    const emojiUrl = emojiByShortcodeToken.get(segment);
    if (emojiUrl) {
      parts.push({ type: "emoji", shortcode: segment, url: emojiUrl });
      continue;
    }

    parts.push({ type: "text", value: segment });
  }

  return parts.length ? parts : [{ type: "text", value: message.body }];
}

export function isEmojiOnlyMessage(parts: MessageBodyPart[]): boolean {
  const emojiParts = parts.filter((part) => part.type === "emoji");
  const hasOnlyWhitespaceText = parts.every(
    (part) => part.type === "emoji" || part.value.trim().length === 0,
  );

  if (emojiParts.length === 1 && hasOnlyWhitespaceText) {
    return true;
  }

  if (parts.length === 1 && parts[0].type === "text") {
    return SINGLE_UNICODE_EMOJI_PATTERN.test(parts[0].value);
  }

  return false;
}
