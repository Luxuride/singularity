import { Node as PMNode, Schema } from "prosemirror-model";

import type { PickerCustomEmoji } from "$lib/emoji/picker";

const TOKEN_PATTERN = /:([A-Za-z0-9_+\-]+):/g;

export const composerSchema = new Schema({
  nodes: {
    doc: { content: "paragraph+" },
    paragraph: {
      content: "(text|customEmoji)*",
      parseDOM: [{ tag: "p" }],
      toDOM: () => ["p", 0],
    },
    text: {},
    customEmoji: {
      inline: true,
      group: "inline",
      atom: true,
      selectable: true,
      draggable: false,
      attrs: {
        token: {},
        url: { default: "" },
      },
      parseDOM: [
        {
          tag: "img[data-custom-emoji-token]",
          getAttrs: (dom) => {
            if (!(dom instanceof HTMLImageElement)) {
              return false;
            }

            const token = dom.dataset.customEmojiToken?.trim().toLowerCase();
            if (!token || !/^:[A-Za-z0-9_+\-]+:$/.test(token)) {
              return false;
            }

            return {
              token,
              url: dom.getAttribute("src") ?? "",
            };
          },
        },
        {
          tag: "img[alt]",
          getAttrs: (dom) => {
            if (!(dom instanceof HTMLImageElement)) {
              return false;
            }

            const token = dom.alt?.trim().toLowerCase();
            if (!token || !/^:[A-Za-z0-9_+\-]+:$/.test(token)) {
              return false;
            }

            return {
              token,
              url: dom.getAttribute("src") ?? "",
            };
          },
        },
      ],
      toDOM: (node) => [
        "img",
        {
          src: node.attrs.url,
          alt: node.attrs.token,
          title: String(node.attrs.token).replace(/^:+|:+$/g, ""),
          "aria-label": String(node.attrs.token).replace(/^:+|:+$/g, ""),
          "data-custom-emoji-token": node.attrs.token,
          contenteditable: "false",
          class: "composer-custom-emoji",
        },
      ],
      leafText: (node) => node.attrs.token as string,
    },
  },
});

function buildCustomEmojiByToken(customEmoji: PickerCustomEmoji[]): Map<string, PickerCustomEmoji> {
  const map = new Map<string, PickerCustomEmoji>();

  for (const emoji of customEmoji) {
    for (const shortcode of emoji.shortcodes ?? []) {
      const normalized = shortcode.trim().replace(/^:+|:+$/g, "").toLowerCase();
      if (!normalized) {
        continue;
      }

      const token = `:${normalized}:`;
      if (!map.has(token)) {
        map.set(token, emoji);
      }
    }
  }

  return map;
}

function buildInlineNodesFromText(value: string, customEmoji: PickerCustomEmoji[]): PMNode[] {
  const customByToken = buildCustomEmojiByToken(customEmoji);
  const nodes: PMNode[] = [];

  let lastIndex = 0;
  for (const match of value.matchAll(TOKEN_PATTERN)) {
    const fullMatch = match[0] ?? "";
    const start = match.index ?? -1;
    if (start < 0) {
      continue;
    }

    if (start > lastIndex) {
      nodes.push(composerSchema.text(value.slice(lastIndex, start)));
    }

    const token = fullMatch.toLowerCase();
    const custom = customByToken.get(token);
    if (custom?.url) {
      nodes.push(composerSchema.node("customEmoji", { token, url: custom.url }));
    } else {
      nodes.push(composerSchema.text(fullMatch));
    }

    lastIndex = start + fullMatch.length;
  }

  if (lastIndex < value.length) {
    nodes.push(composerSchema.text(value.slice(lastIndex)));
  }

  return nodes;
}

export function buildDocFromDraft(value: string, customEmoji: PickerCustomEmoji[]): PMNode {
  const lines = value.split("\n");
  const paragraphs = lines.map((line) => composerSchema.node("paragraph", null, buildInlineNodesFromText(line, customEmoji)));

  if (paragraphs.length === 0) {
    paragraphs.push(composerSchema.node("paragraph"));
  }

  return composerSchema.node("doc", null, paragraphs);
}
