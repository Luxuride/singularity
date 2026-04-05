<script lang="ts">
  import { matrixGetUserAvatar } from "$lib/chats/api";

  const avatarCache = new Map<string, string | null>();
  const avatarInFlight = new Map<string, Promise<string | null>>();

  interface Props {
    roomId: string;
    sender: string;
  }

  let { roomId, sender }: Props = $props();

  let lazyImageUrl = $state<string | null>(null);

  const avatarLabel = $derived.by(() => {
    const localPart = sender.includes(":") ? sender.slice(0, sender.indexOf(":")) : sender;
    const normalized = localPart.replace(/^@/, "").trim();
    return normalized.charAt(0).toUpperCase() || "#";
  });

  $effect(() => {
    lazyImageUrl = null;

    const cachedImage = avatarCache.get(sender);
    if (cachedImage !== undefined) {
      lazyImageUrl = cachedImage;
      return;
    }

    const inFlight = avatarInFlight.get(sender);
    if (inFlight) {
      const senderId = sender;
      void inFlight.then((imageUrl) => {
        avatarCache.set(senderId, imageUrl);
        if (sender === senderId) {
          lazyImageUrl = imageUrl;
        }
      });
      return;
    }

    const senderId = sender;
    const request = matrixGetUserAvatar(roomId, senderId)
      .then((imageUrl) => {
        avatarCache.set(senderId, imageUrl);
        return imageUrl;
      })
      .catch(() => null)
      .finally(() => {
        avatarInFlight.delete(senderId);
      });

    avatarInFlight.set(senderId, request);

    void request.then((imageUrl) => {
      if (sender === senderId) {
        lazyImageUrl = imageUrl;
      }
    });
  });
</script>

<div
  class="h-8 w-8 shrink-0 overflow-hidden rounded-full bg-surface-200-800 grid place-items-center text-xs font-semibold text-surface-800-200"
  title={sender}
>
  {#if lazyImageUrl}
    <img src={lazyImageUrl} alt="" class="h-full w-full object-cover" loading="lazy" decoding="async" />
  {:else}
    <span>{avatarLabel}</span>
  {/if}
</div>
