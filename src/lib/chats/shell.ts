import { writable } from "svelte/store";

import type { MatrixRecoveryState } from "../auth/types";
import type {
	MatrixChatSummary,
	MatrixPickerCustomEmoji,
} from "./types";

export const shellLoading = writable(true);
export const shellRefreshing = writable(false);
export const shellErrorMessage = writable("");
export const shellCurrentUserId = writable("");
export const shellRecoveryState = writable<MatrixRecoveryState | null>(null);

export const shellChats = writable<MatrixChatSummary[]>([]);
export const shellRootSpaces = writable<MatrixChatSummary[]>([]);
export const shellRootScopedRooms = writable<MatrixChatSummary[]>([]);
export const shellSelectedRootSpaceId = writable("");
export const shellSelectedRoomId = writable("");
export const shellPickerCustomEmoji = writable<MatrixPickerCustomEmoji[]>([]);
