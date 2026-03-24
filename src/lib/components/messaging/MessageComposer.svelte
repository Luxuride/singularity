<script lang="ts">
  import { onMount } from "svelte";

  import {
    applyCustomEmojiConfig,
    ensureEmojiPickerLoaded,
    selectedEmojiToken,
  } from "$lib/emoji/picker";
  import type { PickerCustomEmoji } from "$lib/emoji/picker";

  type EmojiClickDetail = {
    unicode?: string;
    name?: string;
    emoji?: {
      shortcodes?: string[];
      name?: string;
    };
  };

  interface Props {
    draft: string;
    error: string;
    isSending: boolean;
    isDisabled: boolean;
    placeholder?: string;
    pickerCustomEmoji?: PickerCustomEmoji[];
    onSubmit?: (draft: string) => void;
    onDraftChange?: (draft: string) => void;
  }

  let {
    draft,
    error,
    isSending,
    isDisabled,
    placeholder = "Write a message...",
    pickerCustomEmoji = [],
    onSubmit,
    onDraftChange,
  }: Props = $props();
  let showEmojiPicker = $state(false);
  let pickerElement = $state<HTMLElement | null>(null);
  let pickerPopoverElement = $state<HTMLElement | null>(null);

  onMount(() => {
    void ensureEmojiPickerLoaded();
  });

  $effect(() => {
    if (!showEmojiPicker || !pickerElement) {
      return;
    }

    applyCustomEmojiConfig(pickerElement as never, pickerCustomEmoji);
  });

  $effect(() => {
    if (!showEmojiPicker) {
      return;
    }

    const handleOutsidePointerDown = (event: PointerEvent) => {
      const target = event.target;
      if (!(target instanceof Node)) {
        return;
      }

      if (pickerPopoverElement?.contains(target)) {
        return;
      }

      showEmojiPicker = false;
    };

    window.addEventListener("pointerdown", handleOutsidePointerDown, true);

    return () => {
      window.removeEventListener("pointerdown", handleOutsidePointerDown, true);
    };
  });

  function handleSubmit(event: SubmitEvent) {
    event.preventDefault();
    if (!isSending && draft.trim().length > 0) {
      onSubmit?.(draft);
    }
  }

  function insertEmoji(emoji: string) {
    const next = `${draft}${emoji}`;
    draft = next;
    onDraftChange?.(next);
  }

  function handleEmojiClick(event: CustomEvent<EmojiClickDetail>) {
    const selected = selectedEmojiToken(event.detail);
    if (!selected) {
      return;
    }

    insertEmoji(selected);
    showEmojiPicker = false;
  }
</script>

<form class="card p-3 mt-3 preset-outlined-surface-200-800 bg-surface-100-900 relative" onsubmit={handleSubmit}>
  {#if error}
    <p class="mb-2 text-sm preset-filled-error-500 card p-2">{error}</p>
  {/if}

  <label class="text-xs text-surface-700-300 mb-1" for="message-draft">Message</label>
  <div class="mb-2 flex items-center gap-2">
    <div class="relative" bind:this={pickerPopoverElement}>
      <button
        type="button"
        class="btn btn-sm preset-tonal"
        onclick={() => {
          showEmojiPicker = !showEmojiPicker;
        }}
        disabled={isDisabled || isSending}
      >
        Emoji
      </button>
      {#if showEmojiPicker}
        <div class="absolute bottom-full left-0 mb-2 z-20 card p-2 preset-outlined-surface-300-700 bg-surface-50-950 w-[30rem] max-w-[94vw]">
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <emoji-picker
            bind:this={pickerElement}
            style="width: 100%; height: 22rem; --emoji-size: 2.5rem; --category-emoji-size: 1.5rem; --num-columns: 7;"
            onemoji-click={handleEmojiClick}
          ></emoji-picker>
        </div>
      {/if}
    </div>

  </div>

  <textarea
    id="message-draft"
    class="input h-24"
    {placeholder}
    bind:value={draft}
    onchange={() => onDraftChange?.(draft)}
    disabled={isDisabled || isSending}
  ></textarea>

  <div class="mt-2 flex justify-end">
    <button
      type="submit"
      class="btn preset-filled"
      disabled={isDisabled || isSending || draft.trim().length === 0}
    >
      {#if isSending}Sending...{:else}Send{/if}
    </button>
  </div>
</form>
