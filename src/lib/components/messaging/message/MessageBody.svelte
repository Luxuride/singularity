<script lang="ts">
  import type { PickerCustomEmoji } from "$lib/emoji/picker";

  import {
    buildEmojiByShortcodeToken,
    buildMessageBodyParts,
    emojiName,
  } from "./helpers";
  import type { TimelineMessage } from "../shared";

  interface Props {
    message: TimelineMessage;
    pickerCustomEmoji?: PickerCustomEmoji[];
  }

  let {
    message,
    pickerCustomEmoji = [],
  }: Props = $props();

  const emojiByShortcodeToken = $derived.by(() => buildEmojiByShortcodeToken(message, pickerCustomEmoji));
  const messageBodyParts = $derived.by(() => buildMessageBodyParts(message, pickerCustomEmoji, emojiByShortcodeToken));
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
{:else}
  <p class="whitespace-pre-wrap break-words text-base">
    {#each messageBodyParts as part, index (`${part.type}-${index}`)}
      {#if part.type === "emoji"}
        <img
          src={part.url}
          alt={part.shortcode}
          title={emojiName(part.shortcode)}
          height="32"
          width="32"
          class="inline-block align-text-bottom mx-0.5"
          loading="lazy"
        />
      {:else}
        {part.value}
      {/if}
    {/each}
  </p>
{/if}
