<script lang="ts">
  import "../app.css";
  import type { Snippet } from "svelte";
  import { Toast } from "@skeletonlabs/skeleton-svelte";
  import { toaster } from "$lib/toaster";

  interface Props {
    children?: Snippet;
  }

  let { children }: Props = $props();
</script>

{@render children?.()}

<Toast.Group {toaster}>
  {#snippet children(toast)}
    <Toast toast={toast}>
      <Toast.Message>
        <Toast.Title>{toast.title}</Toast.Title>
        <Toast.Description>{toast.description}</Toast.Description>
      </Toast.Message>
      <Toast.CloseTrigger />
    </Toast>
  {/snippet}
</Toast.Group>
