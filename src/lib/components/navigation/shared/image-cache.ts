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
}

export const roomImageCache = new ImageCache();
