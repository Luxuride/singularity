export const VIRTUAL_DMS_ROOT_ID = "virtual:dms";
export const VIRTUAL_UNSPACED_ROOT_ID = "virtual:unspaced";

export function isVirtualRoomId(roomId: string): boolean {
  return roomId.startsWith("virtual:");
}
