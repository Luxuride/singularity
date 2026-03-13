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

export interface MatrixLogoutResponse {
  loggedOut: boolean;
}
