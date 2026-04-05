<script lang="ts">
  interface Props {
    error: string;
  }

  let { error }: Props = $props();
</script>

{#if error}
  <p class="mb-2 text-sm preset-filled-error-500 card p-2">{error}</p>
{/if}
