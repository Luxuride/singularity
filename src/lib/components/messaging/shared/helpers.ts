import type { TimelineMessage } from "./types";

/// Strip a Matrix `<mx-reply>` block from formatted HTML, leaving the actual
/// message body. Used when rendering replies and reply previews.
export function stripMxReplyBlock(html: string): string {
  return html.replace(/<mx-reply>[\s\S]*?<\/mx-reply>/i, "").trimStart();
}

/// Derive a short text preview of a message for reply headers. Images render
/// as a fixed "Image" label; text is truncated to the first line (72 chars).
export function replyTextPreview(message: TimelineMessage | null | undefined): string {
  if (!message) {
    return "";
  }

  if (message.messageType === "m.image") {
    return "Image";
  }

  return message.body.trim().split("\n")[0]?.slice(0, 72) ?? "";
}