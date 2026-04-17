<script lang="ts">
  interface Props {
    isSending: boolean;
    isDisabled: boolean;
    draft: string;
    onChooseAttachment?: () => void;
  }

  let {
    isSending,
    isDisabled,
    draft,
    onChooseAttachment,
  }: Props = $props();
</script>

<div class="mt-2 flex items-center justify-between gap-2">
  <button
    type="button"
    class="btn preset-outlined"
    disabled={isDisabled || isSending}
    onclick={() => onChooseAttachment?.()}
  >
    Attach file
  </button>
  <button
    type="submit"
    class="btn preset-filled"
    disabled={isDisabled || isSending || draft.trim().length === 0}
  >
    {#if isSending}Sending...{:else}Send{/if}
  </button>
</div>
