# Paste optimization + configurable insertion — design

## Problem

Expansion insertion has two modes today (clipboard paste and typed-out
Unicode), with a Paste/Type toggle and a WPM slider already in the UI. But
the paste path itself is rigid and has real defects:

1. **Clipboard clobbering.** `paste_via_clipboard` saves only
   `CF_UNICODETEXT`. If the user had an image, files, or rich text on the
   clipboard, the save comes back `None`, and after the expansion the
   "restore" writes an **empty string** over whatever synthesized text
   remained — the user's copied image/files are silently destroyed.
2. **Fixed 800 ms restore delay.** `PASTE_SETTLE` is a hardcoded constant.
   Fast machines wait longer than needed to get their clipboard back; very
   slow targets (cold Electron apps, RDP) may still consume the paste *after*
   the restore and paste the old clipboard.
3. **Stale-clipboard check by text compare.** The restore thread re-reads the
   clipboard and string-compares against the expansion. This requires opening
   the clipboard (contention) and can't see non-text claims.
4. **Hardcoded Ctrl+V.** Terminals and some apps don't paste on Ctrl+V
   (Windows conhost uses Shift+Insert or Ctrl+Shift+V; vim/mintty likewise).
   Paste mode is simply broken there today.
5. **Mode is all-or-nothing.** Type mode is maximally compatible but slow for
   long snippets; paste mode is instant but touches the clipboard and fails
   in Ctrl+V-less apps. The user must pick one global behavior.

Goal: make the paste path fast, safe for any clipboard content, and reliable
in more apps — and expose the speed/method knobs in the Settings card.

## Approaches considered

1. **Knobs only.** Expose restore delay + paste combo as settings; leave the
   save/restore logic as-is. Cheapest, but the clipboard-clobbering defect
   and the all-or-nothing mode remain.
2. **Optimized paste core + focused settings (chosen).** Fix the paste path
   (full-format clipboard snapshot, sequence-number ownership check), add an
   Auto insert mode, and expose exactly the knobs that map to real decisions:
   insert method, typing speed, paste shortcut, restore delay.
3. **Per-app / per-snippet profiles.** Window-class detection, per-snippet
   overrides. The most powerful, but a much larger surface (rules UI, app
   matching) for value that global settings + Auto mode mostly deliver.
   Deferred; the settings model below leaves room for it.

## Design

### Insert mode: Auto | Paste | Type

`InsertMode` replaces the boolean `type_out`:

- **Type** — typed-out Unicode keystrokes at the configured WPM (unchanged).
- **Paste** — clipboard paste (unchanged semantics, improved internals).
- **Auto (new, default for fresh installs)** — per-expansion choice:
  - paste when the expansion contains a newline (typed `\n` presses Enter,
    which *sends* the message in Slack/Discord/WhatsApp — never type it), or
  - paste when the expansion is longer than 40 chars (typing 100 chars at
    600 WPM takes 2 s; paste is instant),
  - otherwise type it out (no clipboard touched for short everyday snippets,
    and maximal app compatibility).

The decision lives in `expansion::resolve_mode(text) -> Mode`, unit-tested.

All insertion policy (mode, WPM, paste combo, restore delay) moves into the
`expansion` module as atomics, set at startup from persisted settings and
live from IPC — the existing `WPM` pattern. `AppState.type_out` is removed;
`EngineHost::expand` and the shortcut handler no longer thread a flag
through (`expansion::expand(trigger_char_len, text)` resolves internally).

### Paste path internals

`paste_via_clipboard` becomes:

1. `clipboard::snapshot()` — enumerate **all** clipboard formats and save
   every HGLOBAL-backed one as raw bytes: `Vec<(u32, Vec<u8>)>`. This covers
   plain text, HTML, RTF, images (CF_DIB/CF_DIBV5 — synthesized from
   CF_BITMAP, and re-synthesized back on restore), and copied files
   (CF_HDROP is an HGLOBAL DROPFILES block). Skipped: GDI-handle formats
   (CF_BITMAP, CF_PALETTE, CF_ENHMETAFILE, CF_METAFILEPICT, their DSP
   variants), CF_OWNERDISPLAY, and the private ranges 0x0200–0x03FF, which
   can't be byte-copied. Delayed-render entries whose owner fails to render
   return null and are skipped. Total snapshot is capped at 32 MiB;
   formats beyond the cap are dropped with a log line.
2. Set the expansion text; on failure, fall back to typing (unchanged).
3. Record `GetClipboardSequenceNumber()`.
4. Send the configured paste combo.
5. On a detached thread, sleep the configured restore delay, then restore
   the snapshot **only if the sequence number is unchanged** (nobody —
   user or app — has claimed the clipboard since our set). The sequence
   check needs no clipboard open, so it can't contend; an empty snapshot
   restores to an empty clipboard rather than writing an empty string.

### Paste shortcut

`PasteCombo`: `CtrlV` (default) | `ShiftInsert` | `CtrlShiftV`.

`inject::paste_steps` generalizes from hardcoded Ctrl+V to (required
modifiers, key): physically-held modifiers *required* by the combo are
reused (matched by class — a held LCTRL satisfies a required Ctrl);
held modifiers *not* required are lifted first and restored after; the
Alt/Win mask-key rule is unchanged. Existing unit tests keep passing with
`CtrlV`; new cases cover Shift+Insert (held Ctrl must be lifted — Ctrl+
Insert is *copy*) and Ctrl+Shift+V.

### Settings model

`settings.json` grows; old files migrate transparently (same pattern as the
snippets migration):

```json
{
  "insert_mode": "auto",        // "auto" | "paste" | "type"
  "wpm": 600,                    // 100–1500 (existing)
  "paste_combo": "ctrl_v",      // "ctrl_v" | "shift_insert" | "ctrl_shift_v"
  "restore_delay_ms": 800        // 100–2000, clamped
}
```

Migration: deserialize with every field optional plus the legacy `type_out`.
If `insert_mode` is absent but `type_out` is present, map `true → "type"`,
`false → "paste"` (the user chose it deliberately; don't switch them to
Auto). A missing file or absent both → `"auto"`. Unknown enum strings fall
back to defaults rather than failing the whole file.

### IPC + UI

`Status` gains `insert_mode`, `paste_combo`, `restore_delay_ms` (keeps
`wpm`; drops `type_out`). Commands: `set_insert_mode`, `set_paste_combo`,
`set_restore_delay_ms` (each persists settings, like `set_wpm`);
`set_type_out` is removed. The browser mock mirrors all of it.

Settings card rows (existing style: `.setting-row`, `.seg`, `.slider`):

- **Insert Method** — 3-way segmented control `Auto | Paste | Type`. The
  `.seg` CSS extends to a 3-column variant (thumb width ⅓, translate by
  index via `data-mode`). Subtitle explains the active mode ("Short
  snippets typed, long ones pasted" / "Pasted instantly" / "Typed out key
  by key").
- **Typing Speed** — existing slider, plus the numeric value becomes an
  inline-editable field (user request 2026-07-02): clicking it selects the
  whole number — the "Ctrl+A and retype" feel — so an exact WPM can be
  typed. Enter or blur commits (clamped 100–1500), Esc reverts. Shown when
  mode is Type or Auto.
- **Paste Shortcut** — 3-way segmented control `Ctrl+V | Shift+Ins |
  Ctrl+⇧+V`; shown when mode is Paste or Auto. Subtitle: "What HyperType
  presses to paste — terminals often use Shift+Ins".
- **Clipboard Restore** — slider 100–2000 ms, step 100, with the same
  inline-editable numeric value as Typing Speed; shown when mode is Paste
  or Auto. Subtitle: "{n} ms until your old clipboard comes back — raise
  this if slow apps paste the wrong thing".

### Error handling

- Clipboard open failures: snapshot returns an empty snapshot and set-text
  falls back to type-out (both already logged).
- Restore never runs if the sequence number moved; a failed restore write
  is logged and abandoned (never retried into a user's active copy).
- All numeric settings are clamped in Rust (`set_*`), not trusted from the
  UI; enum strings parse with a default fallback.

### Testing

- `cargo test` unit coverage: `paste_steps` combo matrix, `resolve_mode`
  (short/long/newline × three modes), settings migration (legacy
  `type_out` true/false, missing file, unknown enum string), WPM/delay
  clamping. Clipboard snapshot/restore is Win32-global and stays manual.
- `pnpm build` + `cargo check` clean.
- Physical verification (synthetic input is intentionally ignored by the
  hook, so expansion can't be script-tested): type a short trigger (should
  type out in Auto), a long/multiline one (should paste), toggle combos in
  a terminal, and confirm a copied image survives an expansion.

## Out of scope

- Per-app rules and per-snippet insert-method overrides (approach 3).
- Configurable Auto threshold (fixed at 40 chars until someone needs it).
- Preserving GDI-only clipboard formats (CF_BITMAP without DIB, palettes,
  metafiles) — Windows synthesizes DIB for real-world image copies.
