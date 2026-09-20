/// Shared shortcode parsing/normalization for emoji. The shortcode charset is
/// protocol-sensitive (matches the Matrix custom-emoji convention).

/// Matches `:shortcode:` tokens anywhere in a string.
export const TOKEN_PATTERN = /:([A-Za-z0-9_+\-]+):/g;

/// Matches a shortcode token at the end of a string (for the active-composer
/// suggestion popup), optionally preceded by whitespace.
export const ACTIVE_SHORTCODE_PATTERN = /(^|\s):([A-Za-z0-9_+\-]{1,64})$/;

/// Strip surrounding colons and lowercase a shortcode value.
export function normalizeShortcode(value: string): string {
  return value.trim().replace(/^:+|:+$/g, "").toLowerCase();
}

/// Build a `:shortcode:` token from a raw value, or "" if empty after cleaning.
export function shortcodeToken(value: string): string {
  const clean = normalizeShortcode(value);
  return clean ? `:${clean}:` : "";
}

/// Strip surrounding colons (without lowercasing) — used for display labels.
export function emojiName(value: string): string {
  return value.trim().replace(/^:+|:+$/g, "");
}