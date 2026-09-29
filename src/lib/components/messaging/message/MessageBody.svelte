<script lang="ts">
  import { matrixDownloadFile, matrixResolveVideoUrl } from "$lib/chats/api";
  import { stripMxReplyBlock } from "../shared";
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

  let videoElement = $state<HTMLVideoElement | null>(null);
  let videoUrl = $state<string | null>(null);
  let videoError = $state(false);
  let videoLoading = $state(false);
  let videoWaitingForData = $state(false);

  let imageError = $state(false);

  let fileSaved = $state(false);
  let fileError = $state(false);
  let fileLoading = $state(false);

  const renderedFormattedBody = $derived(
    message.formattedBody ? stripMxReplyBlock(message.formattedBody) : null,
  );

  /// Resolve the video URL on demand when the user starts playback. The video
  /// element is always rendered (with the thumbnail as poster); the API is only
  /// hit once the user presses play.
  ///
  /// Playback is started explicitly once there is something to play. The first
  /// click starts from an element with no `src`, so the browser has nothing to
  /// run and the click is spent on resolving the URL instead.
  async function handleVideoPlay() {
    if (videoUrl || videoLoading || videoError || !message.eventId) {
      return;
    }

    videoLoading = true;
    try {
      const { videoUrl: resolved } = await matrixResolveVideoUrl({
        roomId,
        eventId: message.eventId,
      });
      if (!resolved) {
        videoError = true;
        return;
      }

      videoUrl = resolved;
    } catch {
      videoError = true;
    } finally {
      videoLoading = false;
    }

    await startPlaybackWhenReady();
  }

  /// Play once the element has metadata. Resolves when playback has started or
  /// has failed on its own terms, so a caller never waits on a stalled video.
  async function startPlaybackWhenReady() {
    const element = videoElement;
    if (!element || videoError) {
      return;
    }

    if (element.readyState < HTMLMediaElement.HAVE_METADATA) {
      try {
        await new Promise<void>((resolve, reject) => {
          const cleanup = () => {
            element.removeEventListener("loadedmetadata", handleMetadata);
            element.removeEventListener("canplay", handleCanPlay);
            element.removeEventListener("error", handleError);
          };
          const handleMetadata = () => {
            cleanup();
            resolve();
          };
          const handleCanPlay = () => {
            cleanup();
            resolve();
          };
          const handleError = () => {
            cleanup();
            reject(new Error("Video metadata could not be loaded"));
          };

          element.addEventListener("loadedmetadata", handleMetadata, { once: true });
          element.addEventListener("canplay", handleCanPlay, { once: true });
          element.addEventListener("error", handleError, { once: true });
        });
      } catch (error) {
        if (error instanceof Error && error.message.includes("metadata")) {
          videoError = true;
        }
        return;
      }
    }

    try {
      await element.play();
    } catch {
      // A rejected `play()` is not a broken video: autoplay policy, or a pause
      // that raced the call. The element is left on its first frame with the
      // controls live, so a second press starts it.
    }
  }

  function handleVideoElementError() {
    videoError = true;
    videoLoading = false;
  }

  /// The element has a source and is filling its buffer. Distinct from
  /// `videoLoading`, which covers resolving and caching the file.
  function handleVideoWaiting() {
    videoWaitingForData = true;
  }

  function handleVideoPlaying() {
    videoWaitingForData = false;
  }

  async function loadFile() {
    if (fileSaved || fileLoading || !message.eventId) {
      return;
    }

    fileLoading = true;
    fileError = false;
    try {
      const { saved } = await matrixDownloadFile({
        roomId,
        eventId: message.eventId,
      });
      if (saved) {
        fileSaved = true;
      } else {
        fileError = true;
      }
    } catch {
      fileError = true;
    } finally {
      fileLoading = false;
    }
  }
</script>

{#if message.messageType === "m.image"}
  <figure class="space-y-2">
    {#if message.imageUrl && !imageError}
      <img
        src={message.imageUrl}
        alt={message.body || "Image"}
        loading="lazy"
        class="max-h-[28rem] w-full rounded preset-outlined-surface-300-700 object-contain bg-surface-100-900"
        onerror={() => (imageError = true)}
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
    {#if videoError}
      <div class="rounded preset-outlined-surface-300-700 bg-surface-100-900 p-4 text-sm text-surface-700-300">
        Video unavailable
      </div>
    {:else}
      <div class="relative">
        <!-- svelte-ignore a11y_media_has_caption -->
        <video
          bind:this={videoElement}
          src={videoUrl ?? undefined}
          poster={message.thumbnailUrl ?? undefined}
          controls
          playsinline
          preload="metadata"
          onclick={handleVideoPlay}
          onerror={handleVideoElementError}
          onwaiting={handleVideoWaiting}
          onplaying={handleVideoPlaying}
          oncanplay={handleVideoPlaying}
          onpause={handleVideoPlaying}
          class="max-h-[28rem] w-full rounded preset-outlined-surface-300-700 bg-surface-100-900"
        ></video>
        {#if videoLoading || videoWaitingForData}
          <div
            class="pointer-events-none absolute inset-0 flex items-center justify-center"
            aria-live="polite"
          >
            <span
              class="size-10 animate-spin rounded-full border-4 border-surface-700-300 border-t-transparent"
              role="status"
              aria-label="Loading video"
            ></span>
          </div>
        {/if}
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
    {#if fileSaved}
      <span class="text-surface-700-300">File saved</span>
    {:else if fileError}
      <span class="text-surface-700-300">File unavailable</span>
    {:else}
      <button
        type="button"
        class="underline"
        onclick={loadFile}
        aria-label="Download file"
        title="Download file"
      >
        {fileLoading ? "Loading…" : "Download file"}
      </button>
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
