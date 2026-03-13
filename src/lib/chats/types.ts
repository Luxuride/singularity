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

export interface MatrixChatMessage {
  eventId: string | null;
  sender: string;
  timestamp: number | null;
  body: string;
  encrypted: boolean;
}

export interface MatrixGetChatMessagesResponse {
  roomId: string;
  nextFrom: string | null;
  messages: MatrixChatMessage[];
}
