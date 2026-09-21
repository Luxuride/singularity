import { writable } from "svelte/store";

import { matrixLogout, matrixRecoveryStatus } from "../auth/api";
import type { MatrixRecoveryState } from "../auth/types";
import type {
	MatrixChatSummary,
	MatrixPickerCustomEmoji,
} from "./types";

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

/// Reset all shell state to its signed-out default.
export function resetShellState() {
	shellChats.set([]);
	shellRootSpaces.set([]);
	shellRootScopedRooms.set([]);
	shellSelectedRootSpaceId.set("");
	shellSelectedRoomId.set("");
	shellCurrentUserId.set("");
	shellRecoveryState.set(null);
	shellPickerCustomEmoji.set([]);
}

/// Refresh the recovery status into `shellRecoveryState`, clearing it on error.
export async function refreshRecoveryState() {
	try {
		const recovery = await matrixRecoveryStatus();
		shellRecoveryState.set(recovery.state);
	} catch {
		shellRecoveryState.set(null);
	}
}

/// Sign out: clear the server session, reset shell state, and navigate to the
/// signed-out landing page. Returns the error message on failure, or null.
export async function logout(): Promise<string | null> {
	try {
		await matrixLogout();
		resetShellState();
		return null;
	} catch (error) {
		return error instanceof Error ? error.message : "Failed to log out";
	}
}
