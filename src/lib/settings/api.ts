import { invokeMatrixCommand } from "../command-client";
import type {
	MatrixChangePasswordRequest,
	MatrixChangePasswordResponse,
	MatrixProfileResponse,
	MatrixRemoveAvatarRequest,
	MatrixRemoveAvatarResponse,
	MatrixSetAvatarRequest,
	MatrixSetAvatarResponse,
	MatrixSetDisplayNameRequest,
	MatrixSetDisplayNameResponse,
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
