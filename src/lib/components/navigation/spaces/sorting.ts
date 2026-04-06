import type { MatrixChatSummary } from "$lib/chats/types";
import { VIRTUAL_DMS_ROOT_ID, VIRTUAL_UNSPACED_ROOT_ID } from "../shared";

function rankRootSpace(spaceId: string): number {
  if (spaceId === VIRTUAL_DMS_ROOT_ID) {
    return 0;
  }

  if (spaceId === VIRTUAL_UNSPACED_ROOT_ID) {
    return 1;
  }

  return 2;
}

export function sortRootSpaces(spaces: MatrixChatSummary[]): MatrixChatSummary[] {
  return [...spaces].sort((a, b) => {
    const rankDiff = rankRootSpace(a.roomId) - rankRootSpace(b.roomId);
    if (rankDiff !== 0) {
      return rankDiff;
    }

    return a.displayName.localeCompare(b.displayName, undefined, { sensitivity: "base" });
  });
}
