export function toMessage(error: unknown): string {
  if (typeof error === "string") {
    return error;
  }

  if (error && typeof error === "object") {
    const candidate = (error as Record<string, unknown>).message;
    if (typeof candidate === "string") {
      return candidate;
    }
  }

  return "Unexpected error";
}
