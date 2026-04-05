<script lang="ts">
  import MessageAvatar from "./MessageAvatar.svelte";
  import { toTime } from "../shared";
  import type { TimelineMessage } from "../shared";

  interface Props {
    message: TimelineMessage;
    roomId: string;
  }

  let { message, roomId }: Props = $props();
</script>

<div class="mb-1 flex items-center justify-between gap-2 text-xs text-surface-700-300">
  <div class="flex min-w-0 items-center gap-2">
    <MessageAvatar {roomId} sender={message.sender} />
    <span class="truncate">{message.sender}</span>
  </div>
  <span>{toTime(message.timestamp)}</span>
</div>
