<script lang="ts">
  interface Props {
    imageUrl: string | null;
    displayName: string;
  }

  let { imageUrl, displayName }: Props = $props();

  const avatarLabel = $derived(displayName.trim().charAt(0).toUpperCase() || "#");
</script>

<div
  class="mt-0.5 h-8 w-8 shrink-0 rounded-full bg-surface-200-800 overflow-hidden grid place-items-center text-xs font-semibold text-surface-800-200"
>
  {#if imageUrl}
    <img src={imageUrl} alt="" class="h-full w-full object-cover" />
  {:else}
    <span>{avatarLabel}</span>
  {/if}
</div>
