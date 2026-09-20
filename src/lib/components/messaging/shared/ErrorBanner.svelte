<script lang="ts">
  interface Props {
    error: string;
    class?: string;
  }

  let { error, class: className = "card p-3 text-sm preset-filled-error-500" }: Props = $props();
</script>

{#if error}
  <p class={className}>{error}</p>
{/if}