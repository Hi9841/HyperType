# Shortcut (hotkey) triggers — design

## Problem

HyperType currently only supports **text triggers**: a typed string like `gm`
matched by suffix-scanning the rolling keystroke buffer. The engine
explicitly treats any Ctrl/Win-held combo as a command, not text, and clears
the buffer on it (`engine.rs::handle`, the `(ctrl && !alt) || win` branch).
That means a literal trigger like `"ctrl + b"` typed into the existing UI
text field can never fire — pressing Ctrl+B physically clears the buffer
before any match is attempted, and typing the *string* "ctrl + b" character
by character is not the same action as pressing the chord.

Goal: let a user record an actual key combination (e.g. `Ctrl+Shift+V`) and
have HyperType paste a snippet when that combination is pressed anywhere in
Windows, suppressing the combo's normal effect in the focused app.

## Approach

Use `tauri-plugin-global-shortcut`, which wraps Windows `RegisterHotKey`.
`RegisterHotKey` is the OS-native mechanism for "global hotkey that's
suppressed everywhere" — the focused app's window never receives the
keystroke, so e.g. Ctrl+B won't also bold text in Word. This matches the
plugin pattern already used in `main.rs` for autostart/single-instance, so it
fits the existing architecture with minimal new code, versus hand-rolling
`RegisterHotKey`/`UnregisterHotKey` on a dedicated thread (more control, but
reinvents chord parsing/ID bookkeeping the plugin already provides).

Text triggers and the keyboard-hook engine are **unchanged**. Shortcut
triggers are a fully separate mechanism that happens to share the snippet
list and the clipboard-paste expansion path.

## Data model

`snippets.json` changes from a flat `{trigger: expansion}` map to a list of
entries:

```json
[
  { "trigger": "gm", "expansion": "Good morning", "kind": "text" },
  { "trigger": "Ctrl+Shift+V", "expansion": "you@example.com", "kind": "shortcut" }
]
```

- `kind: "text"` — matched exactly as today (suffix scan, word-boundary
  rule, backspace-and-paste).
- `kind: "shortcut"` — `trigger` is a normalized chord string (fixed
  modifier order `Ctrl+Alt+Shift+Win+<Key>`, only the modifiers actually
  held appear), registered as an OS hotkey.

**Migration**: on load, if the file parses as the old flat-map shape, every
entry is converted to `kind: "text"` and the file is rewritten in the new
list format. One-time, transparent, no data loss. If it already parses as
the new list shape, it's used as-is.

## Backend

- `Snippets` gains a `kind` per entry; `match_suffix` only ever considers
  `Text` entries (unchanged behavior for the hook/engine path).
- A new module owns shortcut registration: on startup, every `Shortcut`
  entry is registered with `tauri-plugin-global-shortcut`. The IPC layer's
  `add_snippet`/`remove_snippet` register/unregister the OS hotkey
  immediately as part of the same call, instead of waiting for a restart.
- When a registered shortcut fires, the plugin's callback runs in-process
  (no typed prefix to match) and calls the same `expansion::expand(...)`
  used by text triggers, with `trigger_char_len = 0` (nothing to backspace —
  the OS never delivered the keystroke to any text field).
- Password-field guard (`is_password_field`) still applies before pasting.

## Conflicts

If `register()` fails (already taken by another HyperType snippet, another
app, or reserved by Windows), `add_snippet` returns an error. The UI shows
it inline ("This shortcut is already in use") and does not save the entry.

## Frontend

The Add form gets a Text/Shortcut mode switch:

- **Text** — today's two free-text inputs, unchanged.
- **Shortcut** — a single "Record Shortcut" button. Clicking it enters a
  listening state ("Press a key combination…"), captures the next chord via
  a `keydown` listener, requires at least one modifier (Ctrl/Alt/Shift/Win)
  plus one non-modifier key (bare keys are rejected so normal typing is
  never hijacked), shows the chord live as modifiers are held, and locks it
  in once the non-modifier key lands. Esc cancels and returns to idle.

The snippet list shows both kinds together; shortcut entries render as a
keycap-style badge (e.g. `Ctrl+Shift+V`) instead of plain trigger text, so
the two kinds are visually distinguishable at a glance.

## Testing

- Rust unit tests: chord parsing/formatting (string ↔ modifiers+key,
  including the fixed ordering and rejecting modifier-only chords) and the
  storage migration (old flat-map fixture → new list shape), following the
  existing test style in `snippets.rs`/`storage.rs`.
- What unit tests **cannot** cover: actual OS-level hotkey suppression. As
  with text-trigger expansion today (synthetic input is intentionally
  ignored by the keyboard hook, so it can't be scripted either), this
  requires physically pressing the recorded combo once the feature is
  built. The user will be asked to do this by hand as the final
  verification step.

## Out of scope (for this pass)

- Modifier-less single-key shortcuts (e.g. bare `F13`) — every recorded
  shortcut requires ≥1 modifier.
- Per-shortcut enable/disable independent of the global enabled toggle —
  shortcuts respect the same master on/off switch as text triggers.
