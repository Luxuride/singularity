<script lang="ts">
  import type { EmojiShortcodeSuggestion } from "$lib/emoji/picker";

  interface Props {
    suggestions: EmojiShortcodeSuggestion[];
    position: { top: number; left: number };
    disabled?: boolean;
    onApply?: (suggestion: EmojiShortcodeSuggestion) => void;
  }

  let {
    suggestions,
    position,
    disabled = false,
    onApply,
  }: Props = $props();
</script>

<div
  class="absolute z-30 card preset-outlined-surface-300-700 bg-surface-50-950 p-1 w-56"
  style={`left: ${position.left}px; top: ${position.top}px; transform: translateY(-100%);`}
>
  <div class="flex flex-col gap-1">
    {#each suggestions as suggestion (suggestion.token)}
      <button
        type="button"
        class="btn btn-xs preset-tonal justify-start"
        onclick={() => onApply?.(suggestion)}
        {disabled}
        title={`Insert ${suggestion.token}`}
      >
        {#if suggestion.kind === "custom" && suggestion.previewUrl}
          <img src={suggestion.previewUrl} alt={suggestion.token} class="inline-block h-4 w-4 rounded-sm" />
          {suggestion.token}
        {:else}
          {suggestion.replacement} {suggestion.token}
        {/if}
      </button>
    {/each}
  </div>
</div>
