<script lang="ts">
  interface Props {
    isSending: boolean;
    isDisabled: boolean;
    draft: string;
  }

  let {
    isSending,
    isDisabled,
    draft,
  }: Props = $props();
</script>

<div class="mt-2 flex justify-end">
  <button
    type="submit"
    class="btn preset-filled"
    disabled={isDisabled || isSending || draft.trim().length === 0}
  >
    {#if isSending}Sending...{:else}Send{/if}
  </button>
</div>
