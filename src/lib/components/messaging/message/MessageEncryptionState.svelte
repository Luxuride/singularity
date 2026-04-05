<script lang="ts">
  import { decryptionLabel, verificationLabel } from "../shared";
  import type { TimelineMessage } from "../shared";

  interface Props {
    message: TimelineMessage;
  }

  let { message }: Props = $props();
</script>

{#if message.encrypted}
  <div class="mt-1 flex flex-wrap items-center gap-2 text-xs">
    <span class="rounded px-2 py-0.5 bg-surface-200-800">{decryptionLabel(message.decryptionStatus)}</span>
    <span
      class="rounded px-2 py-0.5"
      class:bg-success-200-800={message.verificationStatus === "verified"}
      class:bg-warning-200-800={message.verificationStatus === "unverified"}
      class:bg-surface-200-800={message.verificationStatus === "unknown"}
    >
      {verificationLabel(message.verificationStatus)}
    </span>
  </div>
{/if}
