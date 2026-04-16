<script lang="ts">
  import type { PickerCustomEmoji } from "$lib/emoji/picker";
  import type {
    JumpToMessageHandler,
    RetryMessageHandler,
    TimelineMessage,
    ToggleReactionHandler,
  } from "../shared";
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
    onJumpToMessage?: JumpToMessageHandler;
    repliedMessage?: TimelineMessage;
    currentUserId?: string | null;
    pickerCustomEmoji?: PickerCustomEmoji[];
    isSending?: boolean;
  }

  let {
    message,
    roomId,
    onRetry,
    onToggleReaction,
    onJumpToMessage,
    repliedMessage,
    currentUserId = null,
    pickerCustomEmoji = [],
    isSending = false,
  }: Props = $props();
</script>

<li
  class="message-card card p-3 preset-outlined-surface-300-700 bg-surface-50-950 transition-colors duration-100 hover:bg-surface-100-900"
  data-message-event-id={message.eventId ?? undefined}
>
  <MessageHeader {message} {roomId} {repliedMessage} {onJumpToMessage} />
  <MessageBody {message} />
  <MessageReactions {message} {currentUserId} {pickerCustomEmoji} onToggleReaction={onToggleReaction} />
  <MessageSendState {message} {isSending} onRetry={onRetry} />
  <MessageEncryptionState {message} />
</li>
