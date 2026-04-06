import type { MatrixChatSummary } from "$lib/chats/types";
import type { FlatEntry } from "./types";

export function sortRooms(a: MatrixChatSummary, b: MatrixChatSummary): number {
  if (a.kind !== b.kind) {
    return a.kind === "space" ? -1 : 1;
  }

  return a.displayName.localeCompare(b.displayName, undefined, { sensitivity: "base" });
}

export function buildRoomHierarchy(
  rooms: MatrixChatSummary[],
  expandedSpaceIds: Record<string, boolean>,
): FlatEntry[] {
  const roomsById = new Map(rooms.map((room) => [room.roomId, room]));
  const childrenByParent = new Map<string, MatrixChatSummary[]>();
  const childRoomIds = new Set<string>();

  for (const room of rooms) {
    for (const childRoomId of room.childrenRoomIds ?? []) {
      childRoomIds.add(childRoomId);

      const childRoom = roomsById.get(childRoomId);
      if (!childRoom) {
        continue;
      }

      const siblings = childrenByParent.get(room.roomId) ?? [];
      siblings.push(childRoom);
      childrenByParent.set(room.roomId, siblings);
    }
  }

  for (const siblings of childrenByParent.values()) {
    siblings.sort(sortRooms);
  }

  const entries: FlatEntry[] = [];
  const rootRooms = rooms
    .filter((room) => !childRoomIds.has(room.roomId))
    .sort(sortRooms);

  for (const room of rootRooms) {
    appendRoom(room, 0, new Set<string>(), entries, childrenByParent, room.roomId, expandedSpaceIds);
  }

  return entries;
}

function appendRoom(
  room: MatrixChatSummary,
  depth: number,
  ancestry: Set<string>,
  entries: FlatEntry[],
  childrenByParent: Map<string, MatrixChatSummary[]>,
  keySeed: string,
  expandedSpaceIds: Record<string, boolean>,
) {
  const children = childrenByParent.get(room.roomId) ?? [];
  const hasChildren = children.length > 0;

  entries.push({
    key: `${keySeed}:${room.roomId}`,
    room,
    depth,
    hasChildren,
  });

  if (!hasChildren) {
    return;
  }

  if (ancestry.has(room.roomId)) {
    return;
  }

  if (room.kind === "space" && expandedSpaceIds[room.roomId] === false) {
    return;
  }

  const nextAncestry = new Set(ancestry);
  nextAncestry.add(room.roomId);

  for (let index = 0; index < children.length; index += 1) {
    const child = children[index];
    appendRoom(
      child,
      depth + 1,
      nextAncestry,
      entries,
      childrenByParent,
      `${keySeed}:${room.roomId}:${index}`,
      expandedSpaceIds,
    );
  }
}
