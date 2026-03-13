import { invoke } from "@tauri-apps/api/core";
import type {
  MatrixChatSummary,
  MatrixGetChatMessagesRequest,
  MatrixGetChatMessagesResponse,
  MatrixGetChatsResponse,
  MatrixTriggerRoomUpdateRequest,
  MatrixTriggerRoomUpdateResponse,
} from "./types";

function toMessage(error: unknown): string {
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

export async function matrixGetChats(): Promise<MatrixChatSummary[]> {
  try {
    const response = await invoke<MatrixGetChatsResponse>("matrix_get_chats");
    return response.chats;
  } catch (error) {
    throw new Error(toMessage(error));
  }
}

export async function matrixGetChatMessages(
  input: MatrixGetChatMessagesRequest,
): Promise<MatrixGetChatMessagesResponse> {
  try {
    return await invoke<MatrixGetChatMessagesResponse>("matrix_get_chat_messages", {
      request: input,
    });
  } catch (error) {
    throw new Error(toMessage(error));
  }
}

export async function matrixTriggerRoomUpdate(
  input?: MatrixTriggerRoomUpdateRequest,
): Promise<MatrixTriggerRoomUpdateResponse> {
  try {
    return await invoke<MatrixTriggerRoomUpdateResponse>("matrix_trigger_room_update", {
      request: input ?? null,
    });
  } catch (error) {
    throw new Error(toMessage(error));
  }
}
