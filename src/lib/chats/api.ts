import { invokeMatrixCommand } from "../command-client";
import { normalizeChatSummaryImageUrl, normalizeImageUrl, normalizeMessageImageUrl } from "./media";
import type {
  MatrixChatSummary,
  MatrixGetEmojiPacksResponse,
  MatrixGetChatNavigationRequest,
  MatrixGetChatNavigationResponse,
  MatrixGetChatsResponse,
  MatrixGetRoomImageResponse,
  MatrixGetUserAvatarResponse,
  MatrixGetChatMessagesRequest,
  MatrixGetChatMessagesResponse,
  MatrixPickerCustomEmoji,
  MatrixSendChatMessageRequest,
  MatrixSendChatMessageResponse,
  MatrixCancelMediaTranscodeRequest,
  MatrixCancelMediaTranscodeResponse,
  MatrixSendMediaFileRequest,
  MatrixSendMediaFileResponse,
  MatrixStreamChatMessagesRequest,
  MatrixStreamChatMessagesResponse,
  MatrixToggleReactionRequest,
  MatrixToggleReactionResponse,
  MatrixSetRootSpaceOrderRequest,
  MatrixSetRootSpaceOrderResponse,
  MatrixCopyImageToClipboardRequest,
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
  const response = await invokeMatrixCommand<MatrixGetEmojiPacksResponse>("matrix_get_emoji_packs");

  const customEmoji = response.customEmoji.flatMap((emoji: MatrixPickerCustomEmoji) => {
    const normalizedUrl = normalizeImageUrl(emoji.url);
    if (!normalizedUrl) {
      return [];
    }

    return [{
      ...emoji,
      url: normalizedUrl,
      category: emoji.category ?? undefined,
    }];
  });

  return {
    customEmoji,
  };
}

export async function matrixGetChats(): Promise<MatrixChatSummary[]> {
  const response = await invokeMatrixCommand<MatrixGetChatsResponse>("matrix_get_chats");
  return response.chats.map(normalizeChatSummaryImageUrl);
}

export async function matrixGetRoomImage(roomId: string): Promise<string | null> {
  const response = await invokeMatrixCommand<MatrixGetRoomImageResponse>("matrix_get_room_image", {
    request: { roomId },
  });

  return normalizeImageUrl(response.imageUrl);
}

export async function matrixGetUserAvatar(roomId: string, userId: string): Promise<string | null> {
  const response = await invokeMatrixCommand<MatrixGetUserAvatarResponse>("matrix_get_user_avatar", {
    request: { roomId, userId },
  });

  return normalizeImageUrl(response.imageUrl);
}

export async function matrixGetChatNavigation(
  input?: MatrixGetChatNavigationRequest,
): Promise<MatrixGetChatNavigationResponse> {
  const response = await invokeMatrixCommand<MatrixGetChatNavigationResponse>("matrix_get_chat_navigation", {
    request: input ?? null,
  });

  return {
    ...response,
    rootSpaces: response.rootSpaces.map(normalizeChatSummaryImageUrl),
    rootScopedRooms: response.rootScopedRooms.map(normalizeChatSummaryImageUrl),
  };
}

export async function matrixSetRootSpaceOrder(
  input: MatrixSetRootSpaceOrderRequest,
): Promise<MatrixSetRootSpaceOrderResponse> {
  return invokeMatrixCommand<MatrixSetRootSpaceOrderResponse>("matrix_set_root_space_order", {
    request: input,
  });
}

export async function matrixGetChatMessages(
  input: MatrixGetChatMessagesRequest,
): Promise<MatrixGetChatMessagesResponse> {
  const response = await invokeMatrixCommand<MatrixGetChatMessagesResponse>("matrix_get_chat_messages", {
    request: input,
  });

  return {
    ...response,
    messages: response.messages.map(normalizeMessageImageUrl),
  };
}

export async function matrixStreamChatMessages(
  input: MatrixStreamChatMessagesRequest,
): Promise<MatrixStreamChatMessagesResponse> {
  return invokeMatrixCommand<MatrixStreamChatMessagesResponse>("matrix_stream_chat_messages", {
    request: input,
  });
}

export async function matrixSendChatMessage(
  input: MatrixSendChatMessageRequest,
): Promise<MatrixSendChatMessageResponse> {
  return invokeMatrixCommand<MatrixSendChatMessageResponse>("matrix_send_chat_message", {
    request: input,
  });
}

export async function matrixSendMediaFile(
  input: MatrixSendMediaFileRequest,
): Promise<MatrixSendMediaFileResponse> {
  return invokeMatrixCommand<MatrixSendMediaFileResponse>("matrix_send_media_file", {
    request: input,
  });
}

export async function matrixCancelMediaTranscode(
  input: MatrixCancelMediaTranscodeRequest,
): Promise<MatrixCancelMediaTranscodeResponse> {
  return invokeMatrixCommand<MatrixCancelMediaTranscodeResponse>("matrix_cancel_media_transcode", {
    request: input,
  });
}

export async function matrixToggleReaction(
  input: MatrixToggleReactionRequest,
): Promise<MatrixToggleReactionResponse> {
  return invokeMatrixCommand<MatrixToggleReactionResponse>("matrix_toggle_reaction", {
    request: input,
  });
}

export async function matrixCopyImageToClipboard(
  input: MatrixCopyImageToClipboardRequest,
): Promise<void> {
  return invokeMatrixCommand<void>("matrix_copy_image_to_clipboard", {
    request: input,
  });
}

export async function matrixReadClipboardText(): Promise<string> {
  return invokeMatrixCommand<string>("matrix_read_clipboard_text");
}

export async function matrixTriggerRoomUpdate(
  input?: MatrixTriggerRoomUpdateRequest,
): Promise<MatrixTriggerRoomUpdateResponse> {
  return invokeMatrixCommand<MatrixTriggerRoomUpdateResponse>("matrix_trigger_room_update", {
    request: input ?? null,
  });
}

export async function matrixOwnVerificationStatus(): Promise<MatrixOwnVerificationStatus> {
  return invokeMatrixCommand<MatrixOwnVerificationStatus>("matrix_own_verification_status");
}

export async function matrixGetUserDevices(userId: string): Promise<MatrixGetUserDevicesResponse> {
  return invokeMatrixCommand<MatrixGetUserDevicesResponse>("matrix_get_user_devices", {
    userIdRaw: userId,
  });
}

export async function matrixRequestDeviceVerification(
  userId: string,
  deviceId: string,
): Promise<MatrixRequestVerificationResponse> {
  return invokeMatrixCommand<MatrixRequestVerificationResponse>(
    "matrix_request_device_verification",
    { userIdRaw: userId, deviceIdRaw: deviceId },
  );
}

export async function matrixGetVerificationFlow(
  userId: string,
  flowId: string,
): Promise<MatrixVerificationFlowResponse> {
  return invokeMatrixCommand<MatrixVerificationFlowResponse>("matrix_get_verification_flow", {
    userIdRaw: userId,
    flowId,
  });
}

export async function matrixAcceptVerificationRequest(
  userId: string,
  flowId: string,
): Promise<MatrixVerificationFlowResponse> {
  return invokeMatrixCommand<MatrixVerificationFlowResponse>("matrix_accept_verification_request", {
    userIdRaw: userId,
    flowId,
  });
}

export async function matrixStartSasVerification(
  userId: string,
  flowId: string,
): Promise<MatrixVerificationFlowResponse> {
  return invokeMatrixCommand<MatrixVerificationFlowResponse>("matrix_start_sas_verification", {
    userIdRaw: userId,
    flowId,
  });
}

export async function matrixAcceptSasVerification(
  userId: string,
  flowId: string,
): Promise<MatrixVerificationFlowResponse> {
  return invokeMatrixCommand<MatrixVerificationFlowResponse>("matrix_accept_sas_verification", {
    userIdRaw: userId,
    flowId,
  });
}

export async function matrixConfirmSasVerification(
  userId: string,
  flowId: string,
): Promise<MatrixVerificationFlowResponse> {
  return invokeMatrixCommand<MatrixVerificationFlowResponse>("matrix_confirm_sas_verification", {
    userIdRaw: userId,
    flowId,
  });
}
