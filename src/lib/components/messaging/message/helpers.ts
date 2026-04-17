import type { PickerCustomEmoji } from "$lib/emoji/picker";

import type { TimelineMessage } from "../shared";

export function shortcodeToken(value: string): string {
  const clean = value.trim().replace(/^:+|:+$/g, "");
  return clean ? `:${clean.toLowerCase()}:` : "";
}

export function emojiName(value: string): string {
  return value.trim().replace(/^:+|:+$/g, "");
}

export function buildPickerEmojiBySourceUrl(pickerCustomEmoji: PickerCustomEmoji[]): Map<string, string> {
  const map = new Map<string, string>();
  for (const emoji of pickerCustomEmoji) {
    const sourceUrl = emoji.sourceUrl?.trim();
    const imageUrl = emoji.url?.trim();

    if (sourceUrl && imageUrl) {
      map.set(sourceUrl, imageUrl);
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
      const imageUrl = emoji.url?.trim();
      if (imageUrl) {
        map.set(token, imageUrl);
      }
    }
  }

  for (const emoji of pickerCustomEmoji) {
    for (const shortcode of emoji.shortcodes ?? []) {
      const token = shortcodeToken(shortcode);
      if (token && !map.has(token)) {
        const imageUrl = emoji.url?.trim();
        if (imageUrl) {
          map.set(token, imageUrl);
        }
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

  if (
    trimmed.startsWith("mxc://") ||
    trimmed.startsWith("http://") ||
    trimmed.startsWith("https://") ||
    trimmed.startsWith("asset://")
  ) {
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
    const fromMessageEmoji = (message.customEmojis ?? []).find(
      (emoji) => emoji.url?.trim() === trimmed,
    );
    if (fromMessageEmoji?.shortcode) {
      return emojiName(fromMessageEmoji.shortcode);
    }

    const fromPickerEmoji = pickerCustomEmoji.find(
      (emoji) => emoji.sourceUrl?.trim() === trimmed,
    );
    const pickerShortcode = fromPickerEmoji?.shortcodes?.find((entry) => entry.trim().length > 0);
    if (pickerShortcode) {
      return emojiName(pickerShortcode);
    }
  }

  return emojiName(trimmed);
}


