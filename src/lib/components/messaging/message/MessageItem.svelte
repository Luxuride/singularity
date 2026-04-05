<script lang="ts">
  import type { PickerCustomEmoji } from "$lib/emoji/picker";
  import type { RetryMessageHandler, TimelineMessage, ToggleReactionHandler } from "../shared";
  import {
    MessageBody,
    MessageEncryptionState,
    MessageHeader,
    MessageReactions,
    MessageSendState,
  } from ".";

  interface Props {
    message: TimelineMessage;
    roomId: string;
    onRetry?: RetryMessageHandler;
    onToggleReaction?: ToggleReactionHandler;
    currentUserId?: string | null;
    pickerCustomEmoji?: PickerCustomEmoji[];
    isSending?: boolean;
  }

  let {
    message,
    roomId,
    onRetry,
    onToggleReaction,
    currentUserId = null,
    pickerCustomEmoji = [],
    isSending = false,
  }: Props = $props();
</script>

<li class="card p-3 preset-outlined-surface-300-700 bg-surface-50-950" data-message-event-id={message.eventId ?? undefined}>
  <MessageHeader {message} {roomId} />
  <MessageBody {message} {pickerCustomEmoji} />
  <MessageReactions {message} {currentUserId} {pickerCustomEmoji} onToggleReaction={onToggleReaction} />
  <MessageSendState {message} {isSending} onRetry={onRetry} />
  <MessageEncryptionState {message} />
</li>
