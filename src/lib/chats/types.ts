export interface MatrixChatSummary {
  roomId: string;
  displayName: string;
  encrypted: boolean;
  joinedMembers: number;
}

export interface MatrixGetChatsResponse {
  chats: MatrixChatSummary[];
}

export interface MatrixTriggerRoomUpdateRequest {
  selectedRoomId?: string;
}

export interface MatrixTriggerRoomUpdateResponse {
  queued: boolean;
}

export interface MatrixRoomRemovedEvent {
  roomId: string;
}

export interface MatrixSelectedRoomMessagesEvent {
  roomId: string;
  nextFrom: string | null;
  messages: MatrixChatMessage[];
}

export interface MatrixGetChatMessagesRequest {
  roomId: string;
  from?: string;
  limit?: number;
}

export type MatrixMessageDecryptionStatus = "plaintext" | "decrypted" | "unableToDecrypt";

export type MatrixMessageVerificationStatus = "unknown" | "verified" | "unverified";

export interface MatrixChatMessage {
  eventId: string | null;
  sender: string;
  timestamp: number | null;
  body: string;
  encrypted: boolean;
  decryptionStatus: MatrixMessageDecryptionStatus;
  verificationStatus: MatrixMessageVerificationStatus;
}

export interface MatrixGetChatMessagesResponse {
  roomId: string;
  nextFrom: string | null;
  messages: MatrixChatMessage[];
}

// ----- Verification types -----

export type MatrixDeviceTrust = "crossSigned" | "locallyVerified" | "notVerified" | "ownDevice";

export interface MatrixDeviceInfo {
  userId: string;
  deviceId: string;
  displayName: string | null;
  trust: MatrixDeviceTrust;
  ed25519Fingerprint: string | null;
}

export interface MatrixGetUserDevicesResponse {
  userId: string;
  identityVerified: boolean;
  devices: MatrixDeviceInfo[];
}

export interface MatrixOwnVerificationStatus {
  userId: string;
  deviceId: string;
  deviceVerified: boolean;
  crossSigningSetup: boolean;
}

export interface MatrixRequestVerificationResponse {
  flowId: string;
  userId: string;
  deviceId: string;
}

export interface MatrixVerificationStateChangedEvent {
  verified: boolean;
}

export type MatrixVerificationRequestState =
  | "notFound"
  | "created"
  | "requested"
  | "ready"
  | "transitioned"
  | "done"
  | "cancelled";

export type MatrixSasVerificationState =
  | "created"
  | "started"
  | "accepted"
  | "keysExchanged"
  | "confirmed"
  | "done"
  | "cancelled";

export interface MatrixVerificationEmoji {
  symbol: string;
  description: string;
}

export interface MatrixVerificationFlowResponse {
  flowId: string;
  userId: string;
  requestState: MatrixVerificationRequestState;
  sasState: MatrixSasVerificationState | null;
  canAcceptRequest: boolean;
  canStartSas: boolean;
  canAcceptSas: boolean;
  canConfirmSas: boolean;
  isDone: boolean;
  isCancelled: boolean;
  decimals: [number, number, number] | null;
  emojis: MatrixVerificationEmoji[];
  message: string | null;
}

