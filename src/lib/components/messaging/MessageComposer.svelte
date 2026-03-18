<script lang="ts">
  interface Props {
    draft: string;
    error: string;
    isSending: boolean;
    isDisabled: boolean;
    placeholder?: string;
    onSubmit?: (draft: string) => void;
    onDraftChange?: (draft: string) => void;
  }

  let { draft, error, isSending, isDisabled, placeholder = "Write a message...", onSubmit, onDraftChange }: Props = $props();

  function handleSubmit(event: SubmitEvent) {
    event.preventDefault();
    if (!isSending && draft.trim().length > 0) {
      onSubmit?.(draft);
    }
  }
</script>

<form class="card p-3 mt-3 preset-outlined-surface-200-800 bg-surface-100-900" onsubmit={handleSubmit}>
  {#if error}
    <p class="mb-2 text-sm preset-filled-error-500 card p-2">{error}</p>
  {/if}

  <label class="text-xs text-surface-700-300 mb-1" for="message-draft">Message</label>
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
