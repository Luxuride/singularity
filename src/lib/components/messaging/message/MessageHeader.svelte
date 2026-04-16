<script lang="ts">
  import MessageAvatar from "./MessageAvatar.svelte";
  import { toTime } from "../shared";
  import type { JumpToMessageHandler, TimelineMessage } from "../shared";

  interface Props {
    message: TimelineMessage;
    roomId: string;
    repliedMessage?: TimelineMessage;
    onJumpToMessage?: JumpToMessageHandler;
  }

  let {
    message,
    roomId,
    repliedMessage,
    onJumpToMessage,
  }: Props = $props();

  const replySenderLabel = $derived(repliedMessage?.sender ?? "message");
  const replyTextPreview = $derived(
    repliedMessage?.messageType === "m.image"
      ? "Image"
      : (repliedMessage?.body ?? "").trim().split("\n")[0]?.slice(0, 72) ?? "",
  );

  function jumpToReply() {
    if (!message.inReplyToEventId) {
      return;
    }

    onJumpToMessage?.(message.inReplyToEventId);
  }
</script>

<div class="mb-1 flex items-center justify-between gap-2 text-xs text-surface-700-300">
  <div class="flex min-w-0 items-center gap-2">
    <MessageAvatar {roomId} sender={message.sender} />
    <span class="truncate">{message.sender}</span>
  </div>
  <span>{toTime(message.timestamp)}</span>
</div>

{#if message.inReplyToEventId}
  <button
    type="button"
    class="mb-2 w-full rounded border border-surface-300-700 bg-surface-100-900 p-2 text-left text-xs text-surface-700-300 transition hover:bg-surface-200-800"
    onclick={jumpToReply}
  >
    <p class="font-medium text-surface-600-400">In reply to {replySenderLabel}</p>

    {#if repliedMessage?.messageType === "m.image" && repliedMessage.imageUrl}
      <div class="mt-1 flex items-center gap-2">
        <img
          src={repliedMessage.imageUrl}
          alt={repliedMessage.body || "Reply image"}
          loading="lazy"
          class="h-10 w-10 rounded border border-surface-300-700 object-cover bg-surface-100-900"
        />
        <span class="truncate">Image</span>
      </div>
    {:else if replyTextPreview}
      <p class="mt-1 truncate">{replyTextPreview}</p>
    {/if}
  </button>
{/if}
