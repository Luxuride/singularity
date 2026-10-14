import { invokeMatrixCommand } from "../command-client";
import type {
	MatrixAddKeywordRequest,
	MatrixAddKeywordResponse,
	MatrixAddRoomKeywordRequest,
	MatrixAddRoomKeywordResponse,
	MatrixChangePasswordRequest,
	MatrixChangePasswordResponse,
	MatrixGetRoomKeywordsRequest,
	MatrixGetRoomSettingsRequest,
	MatrixKeywordsResponse,
	MatrixLeaveRoomRequest,
	MatrixLeaveRoomResponse,
	MatrixNotificationDefaultsResponse,
	MatrixProfileResponse,
	MatrixRemoveAvatarRequest,
	MatrixRemoveAvatarResponse,
	MatrixRemoveKeywordRequest,
	MatrixRemoveKeywordResponse,
	MatrixRemoveRoomKeywordRequest,
	MatrixRemoveRoomKeywordResponse,
	MatrixRemoveRoomTagRequest,
	MatrixRemoveRoomTagResponse,
	MatrixRoomKeywordsResponse,
	MatrixRoomSettingsResponse,
	MatrixSetAvatarRequest,
	MatrixSetAvatarResponse,
	MatrixSetDisplayNameRequest,
	MatrixSetDisplayNameResponse,
	MatrixSetNotificationDefaultRequest,
	MatrixSetNotificationDefaultResponse,
	MatrixSetRoomFlagsRequest,
	MatrixSetRoomFlagsResponse,
	MatrixSetRoomNotificationModeRequest,
	MatrixSetRoomNotificationModeResponse,
	MatrixSetRoomTagRequest,
	MatrixSetRoomTagResponse,
} from "./types";

export async function matrixGetProfile(): Promise<MatrixProfileResponse> {
	return invokeMatrixCommand<MatrixProfileResponse>("matrix_get_profile");
}

export async function matrixSetDisplayName(
	input: MatrixSetDisplayNameRequest,
): Promise<MatrixSetDisplayNameResponse> {
	return invokeMatrixCommand<MatrixSetDisplayNameResponse>("matrix_set_display_name", {
		request: input,
	});
}

export async function matrixSetAvatar(
	input: MatrixSetAvatarRequest,
): Promise<MatrixSetAvatarResponse> {
	return invokeMatrixCommand<MatrixSetAvatarResponse>("matrix_set_avatar", {
		request: input,
	});
}

export async function matrixRemoveAvatar(
	input: MatrixRemoveAvatarRequest = {},
): Promise<MatrixRemoveAvatarResponse> {
	return invokeMatrixCommand<MatrixRemoveAvatarResponse>("matrix_remove_avatar", {
		request: input,
	});
}

export async function matrixChangePassword(
	input: MatrixChangePasswordRequest,
): Promise<MatrixChangePasswordResponse> {
	return invokeMatrixCommand<MatrixChangePasswordResponse>("matrix_change_password", {
		request: input,
	});
}

export async function matrixRoomGetSettings(
	input: MatrixGetRoomSettingsRequest,
): Promise<MatrixRoomSettingsResponse> {
	return invokeMatrixCommand<MatrixRoomSettingsResponse>("matrix_room_get_settings", {
		request: input,
	});
}

export async function matrixRoomSetNotificationMode(
	input: MatrixSetRoomNotificationModeRequest,
): Promise<MatrixSetRoomNotificationModeResponse> {
	return invokeMatrixCommand<MatrixSetRoomNotificationModeResponse>(
		"matrix_room_set_notification_mode",
		{ request: input },
	);
}

export async function matrixRoomSetTag(
	input: MatrixSetRoomTagRequest,
): Promise<MatrixSetRoomTagResponse> {
	return invokeMatrixCommand<MatrixSetRoomTagResponse>("matrix_room_set_tag", {
		request: input,
	});
}

export async function matrixRoomRemoveTag(
	input: MatrixRemoveRoomTagRequest,
): Promise<MatrixRemoveRoomTagResponse> {
	return invokeMatrixCommand<MatrixRemoveRoomTagResponse>("matrix_room_remove_tag", {
		request: input,
	});
}

export async function matrixRoomSetFlags(
	input: MatrixSetRoomFlagsRequest,
): Promise<MatrixSetRoomFlagsResponse> {
	return invokeMatrixCommand<MatrixSetRoomFlagsResponse>("matrix_room_set_flags", {
		request: input,
	});
}

export async function matrixRoomLeave(
	input: MatrixLeaveRoomRequest,
): Promise<MatrixLeaveRoomResponse> {
	return invokeMatrixCommand<MatrixLeaveRoomResponse>("matrix_room_leave", {
		request: input,
	});
}

export async function matrixGetKeywords(): Promise<MatrixKeywordsResponse> {
	return invokeMatrixCommand<MatrixKeywordsResponse>("matrix_get_keywords");
}

export async function matrixAddKeyword(
	input: MatrixAddKeywordRequest,
): Promise<MatrixAddKeywordResponse> {
	return invokeMatrixCommand<MatrixAddKeywordResponse>("matrix_add_keyword", {
		request: input,
	});
}

export async function matrixRemoveKeyword(
	input: MatrixRemoveKeywordRequest,
): Promise<MatrixRemoveKeywordResponse> {
	return invokeMatrixCommand<MatrixRemoveKeywordResponse>("matrix_remove_keyword", {
		request: input,
	});
}

export async function matrixRoomGetKeywords(
	input: MatrixGetRoomKeywordsRequest,
): Promise<MatrixRoomKeywordsResponse> {
	return invokeMatrixCommand<MatrixRoomKeywordsResponse>("matrix_room_get_keywords", {
		request: input,
	});
}

export async function matrixRoomAddKeyword(
	input: MatrixAddRoomKeywordRequest,
): Promise<MatrixAddRoomKeywordResponse> {
	return invokeMatrixCommand<MatrixAddRoomKeywordResponse>("matrix_room_add_keyword", {
		request: input,
	});
}

export async function matrixRoomRemoveKeyword(
	input: MatrixRemoveRoomKeywordRequest,
): Promise<MatrixRemoveRoomKeywordResponse> {
	return invokeMatrixCommand<MatrixRemoveRoomKeywordResponse>("matrix_room_remove_keyword", {
		request: input,
	});
}

export async function matrixGetNotificationDefaults(): Promise<MatrixNotificationDefaultsResponse> {
	return invokeMatrixCommand<MatrixNotificationDefaultsResponse>("matrix_get_notification_defaults");
}

export async function matrixSetNotificationDefault(
	input: MatrixSetNotificationDefaultRequest,
): Promise<MatrixSetNotificationDefaultResponse> {
	return invokeMatrixCommand<MatrixSetNotificationDefaultResponse>("matrix_set_notification_default", {
		request: input,
	});
}
