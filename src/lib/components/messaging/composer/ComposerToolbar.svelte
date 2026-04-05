<script lang="ts">
  import type { PickerCustomEmoji } from "$lib/emoji/picker";

  import { EmojiPickerPopover } from "../composer-emoji";

  interface Props {
    isDisabled: boolean;
    isSending: boolean;
    pickerCustomEmoji: PickerCustomEmoji[];
    onInsertEmoji?: (emoji: string) => void;
  }

  let {
    isDisabled,
    isSending,
    pickerCustomEmoji,
    onInsertEmoji,
  }: Props = $props();
</script>

<label class="text-xs text-surface-700-300 mb-1" for="message-draft">Message</label>
<div class="mb-2 flex items-center gap-2">
  <EmojiPickerPopover
    disabled={isDisabled || isSending}
    {pickerCustomEmoji}
    onSelect={onInsertEmoji}
  />
</div>
