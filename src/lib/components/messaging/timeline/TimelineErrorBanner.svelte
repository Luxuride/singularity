<script lang="ts">
  interface Props {
    error: string;
  }

  let { error }: Props = $props();
</script>

{#if error}
  <p class="card p-3 text-sm preset-filled-error-500">{error}</p>
{/if}
