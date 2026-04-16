<script lang="ts">
  import type { TimelineMessage } from "../shared";

  interface Props {
    message: TimelineMessage;
  }

  let {
    message,
  }: Props = $props();
</script>

{#if message.messageType === "m.image"}
  <figure class="space-y-2">
    {#if message.imageUrl}
      <img
        src={message.imageUrl}
        alt={message.body || "Image"}
        loading="lazy"
        class="max-h-[28rem] w-full rounded preset-outlined-surface-300-700 object-contain bg-surface-100-900"
      />
    {:else}
      <div class="rounded preset-outlined-surface-300-700 bg-surface-100-900 p-4 text-sm text-surface-700-300">
        Image unavailable
      </div>
    {/if}
    {#if message.body}
      <figcaption class="text-base whitespace-pre-wrap break-words text-surface-700-300">
        {message.body}
      </figcaption>
    {/if}
  </figure>
{:else if message.formattedBody}
  <div class="message-formatted-body whitespace-pre-wrap break-words text-base">
    {@html message.formattedBody}
  </div>
{:else}
  <p class="whitespace-pre-wrap break-words text-base">
    {message.body}
  </p>
{/if}

<style>
  .message-formatted-body :global(img[data-mx-emoticon]) {
    display: inline-block;
    vertical-align: text-bottom;
  }
</style>
