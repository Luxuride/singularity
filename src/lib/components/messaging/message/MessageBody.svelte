<script lang="ts">
  import type { PickerCustomEmoji } from "$lib/emoji/picker";

  import {
    buildEmojiByShortcodeToken,
    buildMessageBodyParts,
    emojiName,
    isEmojiOnlyMessage,
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
  const emojiOnlyBody = $derived.by(() => isEmojiOnlyMessage(messageBodyParts));
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
  <p class={`whitespace-pre-wrap break-words ${emojiOnlyBody ? "text-5xl leading-tight" : "text-base"}`}>
    {#each messageBodyParts as part, index (`${part.type}-${index}`)}
      {#if part.type === "emoji"}
        <img
          src={part.url}
          alt={part.shortcode}
          title={emojiName(part.shortcode)}
          class={emojiOnlyBody
            ? "inline-block h-24 w-24 align-text-bottom mx-0.5"
            : "inline-block h-8 w-8 align-text-bottom mx-0.5"}
          loading="lazy"
        />
      {:else}
        {part.value}
      {/if}
    {/each}
  </p>
{/if}
