export type MatrixRoomKind = "room" | "space";

export interface MatrixChatSummary {
  roomId: string;
  displayName: string;
  imageUrl: string | null;
  encrypted: boolean;
  joinedMembers: number;
  kind: MatrixRoomKind;
  joined: boolean;
  isDirect: boolean;
  childrenRoomIds: string[];
}

export interface MatrixGetChatsResponse {
  chats: MatrixChatSummary[];
}

export interface MatrixGetRoomImageRequest {
  roomId: string;
}

export interface MatrixGetRoomImageResponse {
  roomId: string;
  imageUrl: string | null;
}

export interface MatrixGetChatNavigationRequest {
  rootSpaceId?: string;
  selectedRoomId?: string;
}

export interface MatrixGetChatNavigationResponse {
  selectedRootSpaceId: string | null;
  rootSpaces: MatrixChatSummary[];
  rootScopedRooms: MatrixChatSummary[];
}

export interface MatrixSetRootSpaceOrderRequest {
  rootSpaceIds: string[];
}

export interface MatrixSetRootSpaceOrderResponse {
  rootSpaceIds: string[];
}

export interface MatrixTriggerRoomUpdateRequest {
  selectedRoomId?: string;
  includeSelectedMessages?: boolean;
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

export type MatrixMessageLoadKind = "initial" | "older";

export interface MatrixStreamChatMessagesRequest {
  roomId: string;
  from?: string;
  limit?: number;
  streamId: string;
  loadKind: MatrixMessageLoadKind;
}

export interface MatrixSendChatMessageRequest {
  roomId: string;
  body: string;
}

export interface MatrixToggleReactionRequest {
  roomId: string;
  targetEventId: string;
  key: string;
}

export type MatrixMessageDecryptionStatus = "plaintext" | "decrypted" | "unableToDecrypt";

export type MatrixMessageVerificationStatus = "unknown" | "verified" | "unverified";

export interface MatrixCustomEmoji {
  shortcode: string;
  url: string;
}

export interface MatrixPickerCustomEmoji {
  name: string;
  shortcodes: string[];
  url: string;
  sourceUrl: string;
  category?: string;
}

export interface MatrixGetEmojiPacksResponse {
  customEmoji: MatrixPickerCustomEmoji[];
}

export interface MatrixGetUserAvatarRequest {
  roomId: string;
  userId: string;
}

export interface MatrixGetUserAvatarResponse {
  userId: string;
  imageUrl: string | null;
}

export interface MatrixReactionSummary {
  key: string;
  count: number;
  senders: string[];
}

export interface MatrixChatMessage {
  eventId: string | null;
  inReplyToEventId: string | null;
  sender: string;
  timestamp: number | null;
  body: string;
  formattedBody: string | null;
  messageType: string | null;
  imageUrl: string | null;
  customEmojis: MatrixCustomEmoji[];
  reactions: MatrixReactionSummary[];
  encrypted: boolean;
  decryptionStatus: MatrixMessageDecryptionStatus;
  verificationStatus: MatrixMessageVerificationStatus;
}

export interface MatrixGetChatMessagesResponse {
  roomId: string;
  nextFrom: string | null;
  messages: MatrixChatMessage[];
}

export interface MatrixStreamChatMessagesResponse {
  streamId: string;
  started: boolean;
}

export interface MatrixSendChatMessageResponse {
  eventId: string;
  formattedBody?: string;
}

export interface MatrixToggleReactionResponse {
  added: boolean;
  eventId: string | null;
}

export interface MatrixChatMessageStreamEvent {
  roomId: string;
  streamId: string;
  loadKind: MatrixMessageLoadKind;
  sequence: number;
  message: MatrixChatMessage | null;
  nextFrom: string | null;
  done: boolean;
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

