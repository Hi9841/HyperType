# Parity UI states

Named frames that the judge captures from the SolidJS reference and from the
GPUI app. Viewport is always 960x640 CSS px. Device scale factor equals the
Windows display scale.

## Animation settle (required before every screenshot)

After each state's actions, the reference capture:

1. Freezes infinite CSS animations at `currentTime = 0` (start of the cycle).
   The shortcut recorder `@keyframes listening` (1.2 s infinite pulse) uses this
   freeze frame.
2. Waits until no finite animation or transition is running
   (`document.getAnimations()`).
3. Waits two animation frames.
4. Writes `reference/<state>.boxes.json` (element boxes and text line rects).
5. Takes the screenshot.

GPUI parity mode must show the same frozen frame for infinite animations
(recorder pulse at t=0). See `TODO(port)` in `crates/app/src/parity.rs`.

GPUI capture waits for two consecutive identical window bitmaps (with a
timeout) instead of a fixed sleep, so mid-transition frames are not saved.

Use `bun run boxes` to get exact target geometry from the SolidJS reference.
Match element y and height within 1 px. The boxes file records
`getBoundingClientRect()` for every visible element and
`Range.getClientRects()` for each text line box, so CSS line-box height is a
number and not a guess.

## Browser mock status (parity must match)

The Vite browser mock (`src/lib/ipc.ts`) reports:

- version `"1.1.5"`
- `hook_events` 42
- hook active and healthy
- not elevated
- insert auto / 600 WPM / ctrl_v / 5000 ms / 15 words
- seed snippets: gm, addr, sig, brb, omw (same as `seed/HyperType/snippets.json`)

Parity mode must show the same values in Settings. Today the GPUI build
reads real hook counters and `CARGO_PKG_VERSION` (1.1.8).

TODO(port): when `--features parity` runs, force mock status values
(version 1.1.5, hook_events 42, hook active+healthy, not elevated).

## How to add a GPUI state (later slices)

1. Add a match arm in `crates/app/src/parity.rs` `apply()`.
2. Drive `Manager` with a few lines (search set, mode switch, modal open).
3. Keep the state name identical to the table below.
4. Capture with `bun run capture-gpui -- --pid <pid> --state <name>`.

`default-selected` needs no extra drive: `Manager::new` selects the first
snippet.

## States

### default-selected

- Reference: open app, wait for first row `.studio-item.active` (trigger gm).
- Playwright: `goto` -> wait `.studio-item.active` -> screenshot.
- GPUI: `default-selected` (first snippet selected on open).

### row-hover

- Reference: default-selected, then hover the 2nd `.studio-item` (addr).
- Playwright: `locator('.studio-item').nth(1).hover()` -> screenshot.
- GPUI: `row-hover` (pointer over 2nd row).

### new-snippet

- Reference: click `.new-snippet-btn`.
- Playwright: wait heading "New Snippet", empty trigger and expansion.
- GPUI: `new-snippet`.

### shortcut-mode-empty

- Reference: New Snippet, click Shortcut in `.seg-kind`.
- Playwright: wait recorder hint "Click to Record Shortcut Chord".
- GPUI: `shortcut-mode-empty`.

### shortcut-recording

- Reference: from shortcut-mode-empty, click `.recorder-btn`.
- Playwright: wait `.recorder-btn.listening` and "Press keys on keyboard".
- GPUI: `shortcut-recording`.

### search-filter

- Reference: type `o` into `.studio-search-input`.
- Playwright: wait filtered list (rows that contain "o").
- GPUI: `search-filter`.

### search-no-match

- Reference: type `zzzzz` into `.studio-search-input`.
- Playwright: wait text "No matching snippets found".
- GPUI: `search-no-match`.

### empty-library

- Reference: delete every snippet (accept each `confirm` dialog), land on
  empty library copy "No snippets in library".
- Playwright: for each row delete button, accept dialog; wait empty state.
- GPUI: `empty-library` (seed with zero snippets, or clear then reload).

### settings-modal

- Reference: click `.settings-icon-btn`.
- Playwright: wait `.settings-modal-card` and title Preferences.
- GPUI: `settings-modal`.

### tokens-modal

- Reference: click `.token-btn-all` ("All Tokens").
- Playwright: wait `.tokens-catalog-card`.
- GPUI: `tokens-modal`.

### save-disabled

- Reference: click New Snippet; leave trigger and expansion empty.
- Playwright: wait `.btn-save-snippet[disabled]` with label "Create Snippet".
- GPUI: `save-disabled`.

### update-available (manual)

- Reason: needs the Tauri updater (`check()` + real feed). The browser mock
  has no `__TAURI_INTERNALS__`, so the title-bar update pill never appears.
- Optional cheap path: inject a fake updater via `addInitScript` that sets
  Solid signals. Not done in Phase 1.
- GPUI: `update-available` (manual until the native updater lands).
