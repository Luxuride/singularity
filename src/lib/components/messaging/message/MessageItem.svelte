<script lang="ts">
  import { matrixCopyImageToClipboard } from "$lib/chats/api";
  import type { PickerCustomEmoji } from "$lib/emoji/picker";
  import type {
    JumpToMessageHandler,
    ReplyToMessageHandler,
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
    onReplyToMessage?: ReplyToMessageHandler;
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
    onReplyToMessage,
    onJumpToMessage,
    repliedMessage,
    currentUserId = null,
    pickerCustomEmoji = [],
    isSending = false,
  }: Props = $props();

  let optionsButtonElement: HTMLButtonElement | null = $state(null);
  let optionsMenuElement: HTMLDivElement | null = $state(null);
  let isOptionsOpen = $state(false);
  let contextMenuPosition = $state<{ x: number; y: number } | null>(null);
  let optionsMode = $state<"reply" | "copy-image">("reply");

  const primaryActionLabel = $derived(optionsMode === "copy-image" ? "Copy Image" : "Reply");

  $effect(() => {
    if (!isOptionsOpen) {
      return;
    }

    const onWindowPointerDown = (event: PointerEvent) => {
      const target = event.target;
      if (!(target instanceof Node)) {
        return;
      }

      if (optionsButtonElement?.contains(target) || optionsMenuElement?.contains(target)) {
        return;
      }

      closeOptionsMenu();
    };

    const onWindowKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") {
        return;
      }

      closeOptionsMenu();
    };

    window.addEventListener("pointerdown", onWindowPointerDown, true);
    window.addEventListener("keydown", onWindowKeyDown);

    return () => {
      window.removeEventListener("pointerdown", onWindowPointerDown, true);
      window.removeEventListener("keydown", onWindowKeyDown);
    };
  });

  function closeOptionsMenu() {
    isOptionsOpen = false;
    contextMenuPosition = null;
    optionsMode = "reply";
  }

  function toggleOptionsMenu(event: MouseEvent) {
    event.stopPropagation();
    optionsMode = "reply";
    contextMenuPosition = null;
    isOptionsOpen = !isOptionsOpen;
  }

  function openOptionsMenuFromContext(event: MouseEvent) {
    event.preventDefault();
    event.stopPropagation();
    optionsMode = "reply";
    contextMenuPosition = { x: event.clientX, y: event.clientY };
    isOptionsOpen = true;
  }

  function openImageOptionsMenuFromContext(event: MouseEvent) {
    optionsMode = "copy-image";
    contextMenuPosition = { x: event.clientX, y: event.clientY };
    isOptionsOpen = true;
  }

  async function copyImageToClipboard() {
    if (!message.imageUrl) {
      return;
    }

    try {
      await matrixCopyImageToClipboard({ imageUrl: message.imageUrl });
    } catch (error) {
      console.warn("Failed to copy image", error);
    }
  }

  function handlePrimaryAction() {
    const actionMode = optionsMode;
    closeOptionsMenu();

    if (actionMode === "copy-image") {
      void copyImageToClipboard();
      return;
    }

    onReplyToMessage?.(message);
  }
</script>

<li
  class="message-card card p-3 preset-outlined-surface-300-700 bg-surface-50-950 transition-colors duration-100 hover:bg-surface-100-900 group relative"
  data-message-event-id={message.eventId ?? undefined}
  oncontextmenu={openOptionsMenuFromContext}
>
  <button
    type="button"
    class="absolute right-2 top-2 z-10 rounded px-2 py-1 text-xs font-medium text-surface-700-300 transition hover:bg-surface-200-800 hover:text-surface-900-100 focus-visible:bg-surface-200-800 focus-visible:text-surface-900-100 focus-visible:opacity-100 focus-visible:outline-none opacity-0 group-hover:opacity-100"
    aria-label="Open message options"
    title="Message options"
    bind:this={optionsButtonElement}
    onclick={toggleOptionsMenu}
  >
    ...
  </button>

  {#if isOptionsOpen}
    <div
      class="z-20 min-w-28 overflow-hidden rounded border border-surface-300-700 bg-surface-50-950 shadow"
      class:fixed={Boolean(contextMenuPosition)}
      class:absolute={!contextMenuPosition}
      class:right-2={!contextMenuPosition}
      class:top-10={!contextMenuPosition}
      style={contextMenuPosition ? `left: ${contextMenuPosition.x}px; top: ${contextMenuPosition.y}px;` : undefined}
      role="menu"
      bind:this={optionsMenuElement}
    >
      <button
        type="button"
        class="block w-full px-3 py-2 text-left text-sm hover:bg-surface-200-800 disabled:cursor-not-allowed disabled:opacity-50"
        onclick={handlePrimaryAction}
        disabled={optionsMode === "copy-image" ? !message.imageUrl : !message.eventId}
        role="menuitem"
      >
        {primaryActionLabel}
      </button>
    </div>
  {/if}

  <MessageHeader {message} {roomId} {repliedMessage} {onJumpToMessage} />
  <MessageBody {message} onImageContextMenu={openImageOptionsMenuFromContext} />
  <MessageReactions {message} {currentUserId} {pickerCustomEmoji} onToggleReaction={onToggleReaction} />
  <MessageSendState {message} {isSending} onRetry={onRetry} />
  <MessageEncryptionState {message} />
</li>
