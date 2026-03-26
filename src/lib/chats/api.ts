import { invoke } from "@tauri-apps/api/core";
import { toMessage } from "../errors";
import { normalizeChatSummaryImageUrl, normalizeImageUrl, normalizeMessageImageUrl } from "./media";
import type {
  MatrixChatSummary,
  MatrixGetEmojiPacksResponse,
  MatrixGetChatNavigationRequest,
  MatrixGetChatNavigationResponse,
  MatrixGetChatsResponse,
  MatrixGetChatMessagesRequest,
  MatrixGetChatMessagesResponse,
  MatrixPickerCustomEmoji,
  MatrixSendChatMessageRequest,
  MatrixSendChatMessageResponse,
  MatrixStreamChatMessagesRequest,
  MatrixStreamChatMessagesResponse,
  MatrixToggleReactionRequest,
  MatrixToggleReactionResponse,
  MatrixGetUserDevicesResponse,
  MatrixOwnVerificationStatus,
  MatrixRequestVerificationResponse,
  MatrixVerificationFlowResponse,
  MatrixTriggerRoomUpdateRequest,
  MatrixTriggerRoomUpdateResponse,
} from "./types";

type PickerAssets = {
  customEmoji: MatrixPickerCustomEmoji[];
};

export async function matrixGetPickerAssets(): Promise<PickerAssets> {
  const response = await invoke<MatrixGetEmojiPacksResponse>("matrix_get_emoji_packs");

  return {
    customEmoji: response.customEmoji.map((emoji) => ({
      ...emoji,
      url: normalizeImageUrl(emoji.url) ?? emoji.url,
      category: emoji.category ?? undefined,
    })),
  };
}

export async function matrixGetChats(): Promise<MatrixChatSummary[]> {
  try {
    const response = await invoke<MatrixGetChatsResponse>("matrix_get_chats");
    return response.chats.map(normalizeChatSummaryImageUrl);
  } catch (error) {
    throw new Error(toMessage(error));
  }
}

export async function matrixGetChatNavigation(
  input?: MatrixGetChatNavigationRequest,
): Promise<MatrixGetChatNavigationResponse> {
  try {
    const response = await invoke<MatrixGetChatNavigationResponse>("matrix_get_chat_navigation", {
      request: input ?? null,
    });

    return {
      ...response,
      rootSpaces: response.rootSpaces.map(normalizeChatSummaryImageUrl),
      rootScopedRooms: response.rootScopedRooms.map(normalizeChatSummaryImageUrl),
    };
  } catch (error) {
    throw new Error(toMessage(error));
  }
}

export async function matrixGetChatMessages(
  input: MatrixGetChatMessagesRequest,
): Promise<MatrixGetChatMessagesResponse> {
  try {
    const response = await invoke<MatrixGetChatMessagesResponse>("matrix_get_chat_messages", {
      request: input,
    });

    return {
      ...response,
      messages: response.messages.map(normalizeMessageImageUrl),
    };
  } catch (error) {
    throw new Error(toMessage(error));
  }
}

export async function matrixStreamChatMessages(
  input: MatrixStreamChatMessagesRequest,
): Promise<MatrixStreamChatMessagesResponse> {
  try {
    return await invoke<MatrixStreamChatMessagesResponse>("matrix_stream_chat_messages", {
      request: input,
    });
  } catch (error) {
    throw new Error(toMessage(error));
  }
}

export async function matrixSendChatMessage(
  input: MatrixSendChatMessageRequest,
): Promise<MatrixSendChatMessageResponse> {
  try {
    return await invoke<MatrixSendChatMessageResponse>("matrix_send_chat_message", {
      request: input,
    });
  } catch (error) {
    throw new Error(toMessage(error));
  }
}

export async function matrixToggleReaction(
  input: MatrixToggleReactionRequest,
): Promise<MatrixToggleReactionResponse> {
  try {
    return await invoke<MatrixToggleReactionResponse>("matrix_toggle_reaction", {
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

export async function matrixOwnVerificationStatus(): Promise<MatrixOwnVerificationStatus> {
  try {
    return await invoke<MatrixOwnVerificationStatus>("matrix_own_verification_status");
  } catch (error) {
    throw new Error(toMessage(error));
  }
}

export async function matrixGetUserDevices(userId: string): Promise<MatrixGetUserDevicesResponse> {
  try {
    return await invoke<MatrixGetUserDevicesResponse>("matrix_get_user_devices", {
      userIdRaw: userId,
    });
  } catch (error) {
    throw new Error(toMessage(error));
  }
}

export async function matrixRequestDeviceVerification(
  userId: string,
  deviceId: string,
): Promise<MatrixRequestVerificationResponse> {
  try {
    return await invoke<MatrixRequestVerificationResponse>(
      "matrix_request_device_verification",
      { userIdRaw: userId, deviceIdRaw: deviceId },
    );
  } catch (error) {
    throw new Error(toMessage(error));
  }
}

export async function matrixGetVerificationFlow(
  userId: string,
  flowId: string,
): Promise<MatrixVerificationFlowResponse> {
  try {
    return await invoke<MatrixVerificationFlowResponse>("matrix_get_verification_flow", {
      userIdRaw: userId,
      flowId,
    });
  } catch (error) {
    throw new Error(toMessage(error));
  }
}

export async function matrixAcceptVerificationRequest(
  userId: string,
  flowId: string,
): Promise<MatrixVerificationFlowResponse> {
  try {
    return await invoke<MatrixVerificationFlowResponse>("matrix_accept_verification_request", {
      userIdRaw: userId,
      flowId,
    });
  } catch (error) {
    throw new Error(toMessage(error));
  }
}

export async function matrixStartSasVerification(
  userId: string,
  flowId: string,
): Promise<MatrixVerificationFlowResponse> {
  try {
    return await invoke<MatrixVerificationFlowResponse>("matrix_start_sas_verification", {
      userIdRaw: userId,
      flowId,
    });
  } catch (error) {
    throw new Error(toMessage(error));
  }
}

export async function matrixAcceptSasVerification(
  userId: string,
  flowId: string,
): Promise<MatrixVerificationFlowResponse> {
  try {
    return await invoke<MatrixVerificationFlowResponse>("matrix_accept_sas_verification", {
      userIdRaw: userId,
      flowId,
    });
  } catch (error) {
    throw new Error(toMessage(error));
  }
}

export async function matrixConfirmSasVerification(
  userId: string,
  flowId: string,
): Promise<MatrixVerificationFlowResponse> {
  try {
    return await invoke<MatrixVerificationFlowResponse>("matrix_confirm_sas_verification", {
      userIdRaw: userId,
      flowId,
    });
  } catch (error) {
    throw new Error(toMessage(error));
  }
}
