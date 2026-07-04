// Chord capture and formatting for shortcut-kind snippets. The stored/wire
// format uses raw KeyboardEvent.code tokens (e.g. "Ctrl+Shift+KeyV") because
// that's exactly what the Rust side's chord parser (the global-hotkey
// crate's Code::from_str) accepts case-insensitively, with no translation
// needed. "Super" is the literal modifier token RegisterHotKey maps to the
// Windows key on Windows; it's only relabeled to "Win" for display.

const MODIFIER_CODES = new Set([
  "ControlLeft",
  "ControlRight",
  "AltLeft",
  "AltRight",
  "ShiftLeft",
  "ShiftRight",
  "MetaLeft",
  "MetaRight",
]);

export function isModifierEvent(e: KeyboardEvent): boolean {
  return MODIFIER_CODES.has(e.code);
}

/**
 * Builds the stored chord string from a keydown event, or null if the event
 * isn't a valid shortcut: a bare modifier press, or no modifier held at all
 * (every recorded shortcut requires at least one of Ctrl/Alt/Shift/Win).
 */
export function chordFromEvent(e: KeyboardEvent): string | null {
  if (isModifierEvent(e)) return null;
  const mods: string[] = [];
  if (e.ctrlKey) mods.push("Ctrl");
  if (e.altKey) mods.push("Alt");
  if (e.shiftKey) mods.push("Shift");
  if (e.metaKey) mods.push("Super");
  if (mods.length === 0) return null;
  return [...mods, e.code].join("+");
}

/** Display tokens for keycap rendering, e.g. "Ctrl+Shift+KeyV" -> ["Ctrl", "Shift", "V"]. */
export function chordKeys(chord: string): string[] {
  return chord.split("+").map((token) => {
    if (token === "Super") return "Win";
    if (token.startsWith("Key")) return token.slice(3);
    if (token.startsWith("Digit")) return token.slice(5);
    if (token.startsWith("Arrow")) return token.slice(5);
    return token;
  });
}

/** Friendlier label for flat contexts, e.g. "Ctrl+Shift+KeyV" -> "Ctrl+Shift+V". */
export function displayChord(chord: string): string {
  return chordKeys(chord).join("+");
}
