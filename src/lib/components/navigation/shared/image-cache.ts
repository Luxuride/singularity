type RoomImageValue = string | null;

type RoomImageLoader = () => Promise<RoomImageValue>;

class RoomImageCache {
  private cache = new Map<string, RoomImageValue>();
  private inFlight = new Map<string, Promise<RoomImageValue>>();

  getCached(roomId: string): RoomImageValue | undefined {
    return this.cache.get(roomId);
  }

  prime(roomId: string, value: RoomImageValue) {
    this.cache.set(roomId, value);
  }

  async getOrLoad(roomId: string, loader: RoomImageLoader): Promise<RoomImageValue> {
    const cached = this.cache.get(roomId);
    if (cached !== undefined) {
      return cached;
    }

    const existing = this.inFlight.get(roomId);
    if (existing) {
      return existing;
    }

    const request = loader()
      .then((value) => {
        this.cache.set(roomId, value);
        return value;
      })
      .catch(() => null)
      .finally(() => {
        this.inFlight.delete(roomId);
      });

    this.inFlight.set(roomId, request);
    return request;
  }
}

export const roomImageCache = new RoomImageCache();
