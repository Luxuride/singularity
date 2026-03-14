import { invoke } from "@tauri-apps/api/core";
import { toMessage } from "../errors";
import type {
  MatrixCompleteOAuthRequest,
  MatrixCompleteOAuthResponse,
  MatrixLogoutResponse,
  MatrixRecoveryStatusResponse,
  MatrixRecoverWithKeyRequest,
  MatrixRecoverWithKeyResponse,
  MatrixSessionStatusResponse,
  MatrixStartOAuthRequest,
  MatrixStartOAuthResponse,
} from "./types";

export async function matrixStartOAuth(
  input: MatrixStartOAuthRequest,
): Promise<MatrixStartOAuthResponse> {
  try {
    return await invoke<MatrixStartOAuthResponse>("matrix_start_oauth", {
      request: input,
    });
  } catch (error) {
    throw new Error(toMessage(error));
  }
}

export async function matrixCompleteOAuth(
  input: MatrixCompleteOAuthRequest,
): Promise<MatrixCompleteOAuthResponse> {
  try {
    return await invoke<MatrixCompleteOAuthResponse>("matrix_complete_oauth", {
      request: input,
    });
  } catch (error) {
    throw new Error(toMessage(error));
  }
}

export async function matrixSessionStatus(): Promise<MatrixSessionStatusResponse> {
  try {
    return await invoke<MatrixSessionStatusResponse>("matrix_session_status");
  } catch (error) {
    throw new Error(toMessage(error));
  }
}

export async function matrixLogout(): Promise<MatrixLogoutResponse> {
  try {
    return await invoke<MatrixLogoutResponse>("matrix_logout");
  } catch (error) {
    throw new Error(toMessage(error));
  }
}

export async function matrixRecoveryStatus(): Promise<MatrixRecoveryStatusResponse> {
  try {
    return await invoke<MatrixRecoveryStatusResponse>("matrix_recovery_status");
  } catch (error) {
    throw new Error(toMessage(error));
  }
}

export async function matrixRecoverWithKey(
  input: MatrixRecoverWithKeyRequest,
): Promise<MatrixRecoverWithKeyResponse> {
  try {
    return await invoke<MatrixRecoverWithKeyResponse>("matrix_recover_with_key", {
      request: input,
    });
  } catch (error) {
    throw new Error(toMessage(error));
  }
}
