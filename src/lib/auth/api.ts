import { invokeMatrixCommand } from "../command-client";
import type {
  MatrixClearCacheExceptAuthResponse,
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
  return invokeMatrixCommand<MatrixStartOAuthResponse>("matrix_start_oauth", {
    request: input,
  });
}

export async function matrixCompleteOAuth(
  input: MatrixCompleteOAuthRequest,
): Promise<MatrixCompleteOAuthResponse> {
  return invokeMatrixCommand<MatrixCompleteOAuthResponse>("matrix_complete_oauth", {
    request: input,
  });
}

export async function matrixSessionStatus(): Promise<MatrixSessionStatusResponse> {
  return invokeMatrixCommand<MatrixSessionStatusResponse>("matrix_session_status");
}

export async function matrixLogout(): Promise<MatrixLogoutResponse> {
  return invokeMatrixCommand<MatrixLogoutResponse>("matrix_logout");
}

export async function matrixClearCacheExceptAuth(): Promise<MatrixClearCacheExceptAuthResponse> {
  return invokeMatrixCommand<MatrixClearCacheExceptAuthResponse>("matrix_clear_cache_except_auth");
}

export async function matrixRecoveryStatus(): Promise<MatrixRecoveryStatusResponse> {
  return invokeMatrixCommand<MatrixRecoveryStatusResponse>("matrix_recovery_status");
}

export async function matrixRecoverWithKey(
  input: MatrixRecoverWithKeyRequest,
): Promise<MatrixRecoverWithKeyResponse> {
  return invokeMatrixCommand<MatrixRecoverWithKeyResponse>("matrix_recover_with_key", {
    request: input,
  });
}
