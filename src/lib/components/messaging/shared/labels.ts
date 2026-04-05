import type { MatrixChatMessage } from "$lib/chats/types";

export function toTime(timestamp: number | null): string {
  if (!timestamp) {
    return "";
  }

  return new Date(timestamp).toLocaleString();
}

export function decryptionLabel(status: MatrixChatMessage["decryptionStatus"]): string {
  if (status === "decrypted") {
    return "Decrypted";
  }

  if (status === "unableToDecrypt") {
    return "Unable to decrypt";
  }

  return "Plaintext";
}

export function verificationLabel(status: MatrixChatMessage["verificationStatus"]): string {
  if (status === "verified") {
    return "Verified sender device";
  }

  if (status === "unverified") {
    return "Unverified sender device";
  }

  return "Verification unknown";
}

export function streamStatusLabel(
  loadingMessages: boolean,
  activeLoadKind: string | null,
  streamMessageCount: number,
): string {
  if (!loadingMessages || !activeLoadKind) {
    return "";
  }

  const noun = streamMessageCount === 1 ? "message" : "messages";
  if (activeLoadKind === "older") {
    return `Loading older ${noun}: ${streamMessageCount}`;
  }

  return `Streaming ${noun}: ${streamMessageCount}`;
}
