export type PickerCustomEmoji = {
  name: string;
  shortcodes: string[];
  url: string;
  sourceUrl: string;
  category?: string;
};

// If/when custom categories are added, this defines their display priority.
export const CUSTOM_PICKER_CATEGORY_ORDER: string[] = [];

type EmojiClickDetail = {
  unicode?: string;
  name?: string;
  emoji?: {
    unicode?: string;
    shortcodes?: string[];
    name?: string;
  };
};

const SHORTCODE_PATTERN = /:([A-Za-z0-9_+\-]+):/g;

function normalizeShortcode(value: string): string {
  return value.trim().replace(/^:+|:+$/g, "").toLowerCase();
}

function buildSourceByShortcode(customEmoji: PickerCustomEmoji[]): Map<string, string> {
  const sourceByShortcode = new Map<string, string>();

  for (const emoji of customEmoji) {
    for (const shortcode of emoji.shortcodes ?? []) {
      const normalized = normalizeShortcode(shortcode);
      if (!normalized || sourceByShortcode.has(normalized)) {
        continue;
      }

      if (emoji.sourceUrl?.trim()) {
        sourceByShortcode.set(normalized, emoji.sourceUrl.trim());
      }
    }
  }

  return sourceByShortcode;
}

type PickerDatabaseEmoji = {
  name: string;
  shortcodes?: string[];
  url: string;
  category?: string;
};

let emojiDatabasePromise: Promise<{
  getEmojiByShortcode: (shortcode: string) => Promise<{ unicode?: string } | null>;
  customEmoji?: PickerDatabaseEmoji[];
}> | null = null;

async function getEmojiDatabase(customEmoji: PickerCustomEmoji[]) {
  const dbCustomEmoji: PickerDatabaseEmoji[] = customEmoji.map((emoji) => ({
    name: emoji.name,
    shortcodes: emoji.shortcodes,
    url: emoji.url,
    category: emoji.category,
  }));

  if (!emojiDatabasePromise) {
    emojiDatabasePromise = import("emoji-picker-element").then(({ Database }) => {
      return new Database({ customEmoji: dbCustomEmoji }) as {
        getEmojiByShortcode: (shortcode: string) => Promise<{ unicode?: string } | null>;
        customEmoji?: PickerDatabaseEmoji[];
      };
    });
  }

  const database = await emojiDatabasePromise;
  database.customEmoji = dbCustomEmoji;
  return database;
}

export async function ensureEmojiPickerLoaded(): Promise<void> {
  if (typeof window === "undefined") {
    return;
  }

  await import("emoji-picker-element");
}

export function applyCustomEmojiConfig(
  picker: {
    customEmoji?: PickerCustomEmoji[];
    customCategorySorting?: (a?: string, b?: string) => number;
  } | null,
  customEmoji: PickerCustomEmoji[] = [],
): void {
  if (!picker) {
    return;
  }

  picker.customEmoji = customEmoji;
  picker.customCategorySorting = (a?: string, b?: string): number => {
    const indexOf = (category?: string): number => {
      if (!category) {
        return -1;
      }

      const index = CUSTOM_PICKER_CATEGORY_ORDER.findIndex(
        (entry) => entry.toLowerCase() === category.toLowerCase(),
      );

      return index;
    };

    const left = indexOf(a);
    const right = indexOf(b);

    if (left >= 0 && right >= 0) {
      return left - right;
    }

    if (left >= 0) {
      return -1;
    }

    if (right >= 0) {
      return 1;
    }

    if (!a && !b) {
      return 0;
    }

    if (!a) {
      return -1;
    }

    if (!b) {
      return 1;
    }

    return a.localeCompare(b, undefined, { sensitivity: "base" });
  };
}

export function selectedEmojiToken(detail: EmojiClickDetail): string | null {
  const unicode = detail.unicode ?? detail.emoji?.unicode;
  if (unicode) {
    return unicode;
  }

  const shortcode =
    detail.emoji?.shortcodes?.find((entry) => entry.trim().length > 0) ??
    detail.name ??
    detail.emoji?.name;

  if (!shortcode) {
    return null;
  }

  const clean = shortcode.trim().replace(/^:+|:+$/g, "");
  if (!clean) {
    return null;
  }

  return `:${clean}:`;
}

export async function normalizeShortcodesToEmoji(
  input: string,
  customEmoji: PickerCustomEmoji[] = [],
): Promise<string> {
  if (!input.includes(":")) {
    return input;
  }

  if (typeof window === "undefined") {
    return input;
  }

  const shortcodes = new Set<string>();
  for (const match of input.matchAll(SHORTCODE_PATTERN)) {
    const shortcode = match[1]?.trim();
    if (shortcode) {
      shortcodes.add(shortcode.toLowerCase());
    }
  }

  if (shortcodes.size === 0) {
    return input;
  }

  const database = await getEmojiDatabase(customEmoji);
  const replacements = new Map<string, string>();

  for (const shortcode of shortcodes) {
    const emoji = await database.getEmojiByShortcode(shortcode);
    if (emoji?.unicode) {
      replacements.set(shortcode, emoji.unicode);
    }
  }

  if (replacements.size === 0) {
    return input;
  }

  return input.replace(SHORTCODE_PATTERN, (_fullMatch, shortcodeRaw: string) => {
    const replacement = replacements.get(shortcodeRaw.toLowerCase());
    return replacement ?? `:${shortcodeRaw}:`;
  });
}

export async function normalizeReactionKey(
  input: string,
  customEmoji: PickerCustomEmoji[] = [],
): Promise<string> {
  const trimmed = input.trim();
  if (!trimmed) {
    return "";
  }

  const shortcodeMatch = /^:([A-Za-z0-9_+\-]+):$/.exec(trimmed);
  if (shortcodeMatch) {
    const sourceByShortcode = buildSourceByShortcode(customEmoji);
    const sourceUrl = sourceByShortcode.get(shortcodeMatch[1].toLowerCase());
    if (sourceUrl) {
      return sourceUrl;
    }
  }

  const normalized = await normalizeShortcodesToEmoji(trimmed, customEmoji);
  return normalized.trim();
}

function escapeHtml(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}

export async function buildMessageForSend(
  input: string,
  customEmoji: PickerCustomEmoji[] = [],
): Promise<{ body: string; formattedBody?: string }> {
  const body = await normalizeShortcodesToEmoji(input, customEmoji);

  if (!body.includes(":")) {
    return { body };
  }

  const sourceByShortcode = buildSourceByShortcode(customEmoji);

  if (sourceByShortcode.size === 0) {
    return { body };
  }

  let hasCustomEmoji = false;
  const formattedBody = body.replace(SHORTCODE_PATTERN, (_fullMatch, shortcodeRaw: string) => {
    const normalized = shortcodeRaw.trim().toLowerCase();
    const sourceUrl = sourceByShortcode.get(normalized);
    if (!sourceUrl) {
      return `:${escapeHtml(shortcodeRaw)}:`;
    }

    hasCustomEmoji = true;
    const token = `:${shortcodeRaw}:`;
    return `<img data-mx-emoticon src="${escapeHtml(sourceUrl)}" alt="${escapeHtml(token)}" title="${escapeHtml(token)}" height="32" />`;
  });

  if (!hasCustomEmoji) {
    return { body };
  }

  return {
    body,
    formattedBody: `<p>${formattedBody}</p>`,
  };
}
