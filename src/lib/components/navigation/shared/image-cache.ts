type ImageValue = string | null;

type ImageLoader = () => Promise<ImageValue>;

/// A small cache that dedupes concurrent loads of the same key and memoizes
/// the resolved value. Used for room/space images and sender avatars.
export class ImageCache {
  private cache = new Map<string, ImageValue>();
  private inFlight = new Map<string, Promise<ImageValue>>();

  getCached(key: string): ImageValue | undefined {
    return this.cache.get(key);
  }

  prime(key: string, value: ImageValue) {
    this.cache.set(key, value);
  }

  async getOrLoad(key: string, loader: ImageLoader): Promise<ImageValue> {
    const cached = this.cache.get(key);
    if (cached !== undefined) {
      return cached;
    }

    const existing = this.inFlight.get(key);
    if (existing) {
      return existing;
    }

    const request = loader()
      .then((value) => {
        this.cache.set(key, value);
        return value;
      })
      .catch(() => null)
      .finally(() => {
        this.inFlight.delete(key);
      });

    this.inFlight.set(key, request);
    return request;
  }

  invalidate(key: string) {
    this.cache.delete(key);
    this.inFlight.delete(key);
  }

  keys(): IterableIterator<string> {
    return this.cache.keys();
  }

  clear() {
    this.cache.clear();
    this.inFlight.clear();
  }
}

export const roomImageCache = new ImageCache();

/// Sender avatars are shared across every MessageAvatar instance so a profile
/// change can invalidate the stale entry for the affected user.
export const senderAvatarCache = new ImageCache();

/// Bumped whenever a profile avatar change lands (from the
/// PROFILE_AVATAR_UPDATED handler after the cache has been invalidated);
/// message avatars read it so their load effects re-run and re-resolve.
export const avatarChangeEpoch = { current: 0 };

/// Drop cached avatar entries for a user. Pass a `roomId` to scope the
/// invalidation to one room's cache key, or omit it to clear every room.
export function invalidateSenderAvatar(userId: string, roomId?: string) {
  if (roomId === undefined) {
    for (const key of [...senderAvatarCache.keys()]) {
      if (key.endsWith(`\u0000${userId}`)) {
        senderAvatarCache.invalidate(key);
      }
    }
    avatarChangeEpoch.current += 1;
    return;
  }
  senderAvatarCache.invalidate(`${roomId}\u0000${userId}`);
  avatarChangeEpoch.current += 1;
}
