<script lang="ts">
  import type { TimelineMessage } from "../shared";

  interface Props {
    message: TimelineMessage;
    onImageContextMenu?: (event: MouseEvent) => void;
  }

  let {
    message,
    onImageContextMenu,
  }: Props = $props();

  function stripMxReplyBlock(html: string): string {
    return html.replace(/<mx-reply>[\s\S]*?<\/mx-reply>/i, "").trimStart();
  }

  const renderedFormattedBody = $derived(
    message.formattedBody ? stripMxReplyBlock(message.formattedBody) : null,
  );
</script>

{#if message.messageType === "m.image"}
  <figure class="space-y-2">
    {#if message.imageUrl}
      <img
        src={message.imageUrl}
        alt={message.body || "Image"}
        loading="lazy"
        class="max-h-[28rem] w-full rounded preset-outlined-surface-300-700 object-contain bg-surface-100-900"
        oncontextmenu={(event) => {
          event.preventDefault();
          event.stopPropagation();
          onImageContextMenu?.(event);
        }}
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
{:else if renderedFormattedBody}
  <div class="message-formatted-body whitespace-pre-wrap break-words text-base">
    {@html renderedFormattedBody}
  </div>
{:else}
  <p class="whitespace-pre-wrap break-words text-base">
    {message.body}
  </p>
{/if}

<style>
  /* Handle both data-mx-emoticon (hyphens) and data_mx_emoticon (underscores) formats */
  .message-formatted-body :global(img[data-mx-emoticon]),
  .message-formatted-body :global(img[data_mx_emoticon]) {
    display: inline-block;
    vertical-align: text-bottom;
  }
</style>
