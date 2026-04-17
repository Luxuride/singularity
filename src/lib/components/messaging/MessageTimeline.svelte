<script lang="ts">
  import type { PickerCustomEmoji } from "$lib/emoji/picker";
  import type {
    ReplyToMessageHandler,
    RetryMessageHandler,
    TimelineMessage,
    ToggleReactionHandler,
  } from "./shared";
  import {
    TimelineContent,
    TimelineEmptySelection,
    TimelineErrorBanner,
    TimelineHeader,
  } from "./timeline";

  interface Props {
    messages: TimelineMessage[];
    roomId: string;
    selectedRoomId: string | null;
    currentUserId: string | null;
    pickerCustomEmoji: PickerCustomEmoji[];
    roomEncrypted: boolean;
    roomName: string;
    loadingMessages: boolean;
    activeLoadKind: string | null;
    streamMessageCount: number;
    error: string;
    nextFrom: string | null;
    isSending: boolean;
    onTimelineElementChange?: (element: HTMLElement | null) => void;
    onScroll?: (event: Event) => void;
    onLoadOlder?: () => void;
    onRetryMessage?: RetryMessageHandler;
    onToggleReaction?: ToggleReactionHandler;
    onReplyToMessage?: ReplyToMessageHandler;
  }

  let { 
    messages, 
    roomId, 
    selectedRoomId, 
    currentUserId,
    pickerCustomEmoji,
    roomEncrypted, 
    roomName, 
    loadingMessages, 
    activeLoadKind, 
    streamMessageCount, 
    error,
    nextFrom,
    isSending,
    onTimelineElementChange,
    onScroll,
    onLoadOlder,
    onRetryMessage,
    onToggleReaction,
    onReplyToMessage,
  }: Props = $props();

</script>

<section
  class="card preset-outlined-surface-200-800 bg-surface-100-900 flex flex-col flex-grow min-h-0 gap-3"
>
  <TimelineErrorBanner {error} />

  {#if !selectedRoomId}
    <TimelineEmptySelection />
  {:else}
    <TimelineHeader
      {roomName}
      {roomEncrypted}
      {loadingMessages}
      {activeLoadKind}
      {streamMessageCount}
      {nextFrom}
      onLoadOlder={onLoadOlder}
    />

    <TimelineContent
      {messages}
      {roomId}
      {currentUserId}
      {pickerCustomEmoji}
      {isSending}
      {onScroll}
      {onRetryMessage}
      {onToggleReaction}
      {onReplyToMessage}
      {onTimelineElementChange}
    />
  {/if}
</section>
