import { Node as PMNode } from "prosemirror-model";
import type { EditorState } from "prosemirror-state";

export function draftBetween(docNode: PMNode, from: number, to: number): string {
  return docNode.textBetween(from, to, "\n", (leafNode) => {
    if (leafNode.type.name === "customEmoji") {
      return leafNode.attrs.token ?? "";
    }

    return "";
  });
}

export function docToDraft(doc: PMNode): string {
  const paragraphs: string[] = [];
  doc.forEach((node) => {
    paragraphs.push(draftBetween(node, 0, node.content.size));
  });

  return paragraphs.join("\n");
}

export function getSelectionOffsetsFromState(state: EditorState): { start: number; end: number } {
  const { from, to } = state.selection;

  return {
    start: draftBetween(state.doc, 0, from).length,
    end: draftBetween(state.doc, 0, to).length,
  };
}

function inlineNodeTextLength(node: { type: { name: string }; text?: string; attrs: { token?: string } }): number {
  if (node.type.name === "customEmoji") {
    return (node.attrs.token ?? "").length;
  }

  return node.text?.length ?? 0;
}

export function textOffsetToPos(doc: PMNode, textOffset: number): number {
  let remaining = Math.max(0, textOffset);
  let resultPos = doc.content.size;

  doc.forEach((paragraph, paragraphOffset, index) => {
    if (remaining < 0 || paragraph.type.name !== "paragraph") {
      return;
    }

    const paragraphStart = paragraphOffset + 1;
    let found = false;

    paragraph.forEach((child, childOffset) => {
      if (found) {
        return;
      }

      const childPos = paragraphStart + childOffset;
      const childLength = inlineNodeTextLength(child);

      if (remaining <= childLength) {
        if (child.type.name === "text") {
          resultPos = childPos + remaining;
        } else {
          resultPos = remaining === 0 ? childPos : childPos + 1;
        }
        remaining = -1;
        found = true;
        return;
      }

      remaining -= childLength;
    });

    if (found) {
      return;
    }

    const paragraphEnd = paragraphStart + paragraph.content.size;
    if (remaining === 0) {
      resultPos = paragraphEnd;
      remaining = -1;
      return;
    }

    if (index < doc.childCount - 1) {
      if (remaining === 1) {
        const nextParagraphOffset = paragraphOffset + paragraph.nodeSize;
        resultPos = Math.min(nextParagraphOffset + 1, doc.content.size);
        remaining = -1;
        return;
      }

      remaining -= 1;
    }
  });

  return Math.max(0, Math.min(resultPos, doc.content.size));
}
