export interface MatrixStartOAuthRequest {
  homeserverUrl: string;
}

export interface MatrixStartOAuthResponse {
  authorizationUrl: string;
  redirectUri: string;
}

export interface MatrixCompleteOAuthRequest {
  callbackUrl: string;
}

export interface MatrixCompleteOAuthResponse {
  authenticated: boolean;
  homeserverUrl: string;
  userId: string;
  deviceId: string;
}

export interface MatrixSessionStatusResponse {
  authenticated: boolean;
  homeserverUrl: string | null;
  userId: string | null;
  deviceId: string | null;
}

export type MatrixRecoveryState = "unknown" | "enabled" | "disabled" | "incomplete";

export interface MatrixRecoveryStatusResponse {
  state: MatrixRecoveryState;
}

export interface MatrixRecoverWithKeyRequest {
  recoveryKey: string;
}

export interface MatrixRecoverWithKeyResponse {
  recovered: boolean;
  state: MatrixRecoveryState;
}

export interface MatrixLogoutResponse {
  loggedOut: boolean;
}

export interface MatrixClearCacheExceptAuthResponse {
  cleared: boolean;
}
