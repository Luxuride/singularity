<script lang="ts">
  import { Progress } from "@skeletonlabs/skeleton-svelte";
  import type { TimelineMessage } from "../shared";

  interface Props {
    message: TimelineMessage;
    onImageContextMenu?: (event: MouseEvent) => void;
  }

  let {
    message,
    onImageContextMenu,
  }: Props = $props();

  let imageLoaded = $state(false);
  let imageLoadFailed = $state(false);
  let currentImageSrc = $state<string | null>(null);

  $effect(() => {
    if (message.messageType !== "m.image" || !message.imageUrl) {
      imageLoaded = false;
      imageLoadFailed = false;
      currentImageSrc = null;
      return;
    }

    if (message.imageUrl === currentImageSrc) {
      return;
    }

    currentImageSrc = message.imageUrl;
    imageLoaded = false;
    imageLoadFailed = false;
  });

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
      <div class="relative rounded preset-outlined-surface-300-700 bg-surface-100-900">
        {#if !imageLoaded && !imageLoadFailed}
          <div class="absolute inset-0 grid place-items-center rounded bg-surface-100-900/85" aria-hidden="true">
            <Progress value={null} class="size-8 text-primary-500">
              <Progress.Circle>
                <Progress.CircleTrack class="stroke-surface-400-600" />
                <Progress.CircleRange class="stroke-primary-500" />
              </Progress.Circle>
            </Progress>
          </div>
        {/if}

        {#if !imageLoadFailed}
          <img
            src={message.imageUrl}
            alt={message.body || "Image"}
            loading="lazy"
            decoding="async"
            class={`max-h-[28rem] min-h-40 w-full rounded object-contain bg-surface-100-900 transition-opacity duration-200 ${imageLoaded ? "opacity-100" : "opacity-0"}`}
            onload={() => {
              imageLoaded = true;
            }}
            onerror={() => {
              imageLoadFailed = true;
            }}
            oncontextmenu={(event) => {
              event.preventDefault();
              event.stopPropagation();
              onImageContextMenu?.(event);
            }}
          />
        {:else}
          <div class="rounded bg-surface-100-900 p-4 text-sm text-surface-700-300">
            Image unavailable
          </div>
        {/if}
      </div>
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
{:else if message.messageType === "m.video"}
  <figure class="space-y-2">
    {#if message.imageUrl}
      <!-- svelte-ignore a11y_media_has_caption -->
      <video
        src={message.imageUrl}
        controls
        playsinline
        class="max-h-[28rem] w-full rounded preset-outlined-surface-300-700 bg-surface-100-900"
      ></video>
    {:else}
      <div class="rounded preset-outlined-surface-300-700 bg-surface-100-900 p-4 text-sm text-surface-700-300">
        Video unavailable
      </div>
    {/if}
    {#if message.body}
      <figcaption class="text-base whitespace-pre-wrap break-words text-surface-700-300">
        {message.body}
      </figcaption>
    {/if}
  </figure>
{:else if message.messageType === "m.file"}
  <div class="rounded preset-outlined-surface-300-700 bg-surface-100-900 p-4 text-sm text-surface-700-300">
    {#if message.imageUrl}
      <a class="underline" href={message.imageUrl} target="_blank" rel="noreferrer">Open file</a>
    {:else}
      File attachment
    {/if}
    {#if message.body}
      <div class="mt-2 whitespace-pre-wrap break-words">{message.body}</div>
    {/if}
  </div>
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
