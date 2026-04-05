export type ShortcodeRange = { start: number; end: number };

export type SuggestionPosition = { top: number; left: number };

export type ComposerEditorHandle = {
  clearDraft: () => void;
  insertAtCursor: (text: string) => void;
  replaceRange: (start: number, end: number, replacement: string) => void;
};
