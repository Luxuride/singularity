import type { MatrixChatSummary } from "$lib/chats/types";

export type FlatEntry = {
  key: string;
  room: MatrixChatSummary;
  depth: number;
  hasChildren: boolean;
};
