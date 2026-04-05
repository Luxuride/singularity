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
    buttonText?: string;
    buttonClass?: string;
    popoverClass?: string;
    pickerStyle?: string;
    disabled?: boolean;
    pickerCustomEmoji?: PickerCustomEmoji[];
    onSelect?: (token: string) => void;
  }

  let {
    buttonText = "Emoji",
    buttonClass = "btn btn-sm preset-tonal",
    popoverClass = "absolute bottom-full left-0 mb-2 z-20 card p-2 preset-outlined-surface-300-700 bg-surface-50-950 w-[30rem] max-w-[94vw]",
    pickerStyle = "width: 100%; height: 22rem; --emoji-size: 2.5rem; --category-emoji-size: 1.5rem; --num-columns: 7;",
    disabled = false,
    pickerCustomEmoji = [],
    onSelect,
  }: Props = $props();

  let isOpen = $state(false);
  let pickerElement = $state<HTMLElement | null>(null);
  let pickerPopoverElement = $state<HTMLElement | null>(null);

  onMount(() => {
    void ensureEmojiPickerLoaded();
  });

  $effect(() => {
    if (!isOpen || !pickerElement) {
      return;
    }

    applyCustomEmojiConfig(pickerElement as never, pickerCustomEmoji);
  });

  $effect(() => {
    if (!isOpen) {
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

      isOpen = false;
    };

    window.addEventListener("pointerdown", handleOutsidePointerDown, true);

    return () => {
      window.removeEventListener("pointerdown", handleOutsidePointerDown, true);
    };
  });

  function handleEmojiClick(event: CustomEvent<EmojiClickDetail>) {
    const selected = selectedEmojiToken(event.detail);
    if (!selected) {
      return;
    }

    onSelect?.(selected);
    isOpen = false;
  }
</script>

<div class="relative" bind:this={pickerPopoverElement}>
  <button
    type="button"
    class={buttonClass}
    onclick={() => {
      isOpen = !isOpen;
    }}
    {disabled}
  >
    {buttonText}
  </button>

  {#if isOpen}
    <div class={popoverClass}>
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <emoji-picker
        bind:this={pickerElement}
        style={pickerStyle}
        onemoji-click={handleEmojiClick}
      ></emoji-picker>
    </div>
  {/if}
</div>
