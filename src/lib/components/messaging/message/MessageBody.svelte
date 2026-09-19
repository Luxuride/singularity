<script lang="ts">
  import { matrixResolveVideoUrl } from "$lib/chats/api";
  import type { TimelineMessage } from "../shared";

  interface Props {
    message: TimelineMessage;
    roomId: string;
    onImageContextMenu?: (event: MouseEvent) => void;
  }

  let {
    message,
    roomId,
    onImageContextMenu,
  }: Props = $props();

  let videoUrl = $state<string | null>(null);
  let videoError = $state(false);
  let videoLoading = $state(false);

  function stripMxReplyBlock(html: string): string {
    return html.replace(/<mx-reply>[\s\S]*?<\/mx-reply>/i, "").trimStart();
  }

  const renderedFormattedBody = $derived(
    message.formattedBody ? stripMxReplyBlock(message.formattedBody) : null,
  );

  async function loadVideo() {
    if (videoUrl || videoLoading || !message.eventId) {
      return;
    }

    videoLoading = true;
    videoError = false;
    try {
      const { videoUrl: resolved } = await matrixResolveVideoUrl({
        roomId,
        eventId: message.eventId,
      });
      if (resolved) {
        videoUrl = resolved;
      } else {
        videoError = true;
      }
    } catch {
      videoError = true;
    } finally {
      videoLoading = false;
    }
  }
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
{:else if message.messageType === "m.video"}
  <figure class="space-y-2">
    {#if videoUrl}
      <!-- svelte-ignore a11y_media_has_caption -->
      <video
        src={videoUrl}
        controls
        playsinline
        class="max-h-[28rem] w-full rounded preset-outlined-surface-300-700 bg-surface-100-900"
      ></video>
    {:else if videoError}
      <div class="rounded preset-outlined-surface-300-700 bg-surface-100-900 p-4 text-sm text-surface-700-300">
        Video unavailable
      </div>
    {:else}
      <button
        type="button"
        class="relative block w-full max-h-[28rem] rounded preset-outlined-surface-300-700 bg-surface-100-900 overflow-hidden"
        onclick={loadVideo}
        aria-label="Play video"
        title="Play video"
      >
        {#if message.thumbnailUrl}
          <img
            src={message.thumbnailUrl}
            alt={message.body || "Video"}
            loading="lazy"
            class="w-full object-contain"
          />
        {:else}
          <div class="flex aspect-video w-full items-center justify-center text-sm text-surface-700-300">
            Video
          </div>
        {/if}
        <div
          class="absolute inset-0 flex items-center justify-center bg-black/40"
          class:opacity-0={videoLoading}
        >
          <span class="flex h-14 w-14 items-center justify-center rounded-full bg-black/60 text-2xl text-white">
            {videoLoading ? "…" : "▶"}
          </span>
        </div>
      </button>
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
