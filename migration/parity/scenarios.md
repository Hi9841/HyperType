# Behavior scenarios (GPUI app)

Run these on the real GPUI release build after each slice. Each item lists
steps, the expected result, and `src/App.tsx` line refs for the SolidJS
spec.

## Create snippet

1. Click New Snippet.
2. Enter trigger `hello` and expansion `Hello world`.
3. Click Create Snippet.

Expected: list shows `hello`; editor selects it; success text
"Snippet created!" for 3 s.

Refs: `startNewSnippet` 236-246, `saveOrAdd` 454-480, success clear 474.

## Edit snippet

1. Select `gm`.
2. Change expansion to `Good morning!`.
3. Click Save Changes.

Expected: success "Saved!"; list excerpt updates; selection stays on `gm`.

Refs: `selectSnippet` 225-234, `saveOrAdd` 467-470.

## Rename snippet

1. Select `brb`.
2. Change trigger to `brb2`.
3. Save.

Expected: list shows `brb2` instead of `brb`; selection is `brb2`.

Refs: `editSnippet` path in `saveOrAdd` 467-472; ipc edit 165-178.

## Delete with confirm OK

1. Select a snippet.
2. Click Delete (row or canvas).
3. Accept the confirm dialog.

Expected: snippet gone; if it was selected, first remaining is selected, or
New Snippet starts when the list is empty.

Refs: `deleteSnippet` 482-499; confirm string 484.

## Delete with confirm Cancel

1. Click Delete.
2. Dismiss the confirm dialog.

Expected: list and selection unchanged.

Refs: early return on cancel 484.

## Search

1. Type `sig` in the search field.
2. Clear with the clear button.

Expected: only matching rows while filtered; full list after clear.

Refs: `filteredSnippets` 209-218; clear UI 679-682.

## Drag reorder

1. With empty search, drag the grip of row 0 onto row 2.
2. Release.

Expected: order persists; rows animate by row height; dragged row follows
the pointer.

Refs: `startDrag` 515-557, `applyReorder` 501-513; lock note 220-222.

## Keyboard reorder

1. Focus a grip.
2. Press ArrowDown (or ArrowUp).

Expected: snippet moves one step in the full list.

Refs: `gripKeys` 559-568.

## Reorder locked while searching

1. Type any non-empty search.
2. Try to drag a grip.

Expected: grips disabled; title "Clear the search to reorder".

Refs: `reorderLocked` 222; disabled grip 756.

## Chord record

1. New Snippet -> Shortcut -> click recorder.
2. Press Ctrl+Shift+K (or any chord with a modifier).

Expected: chord shows on the button; focus moves to the expansion textarea;
recording ends.

Refs: `startShortcutRecording` 400-435; `chordFromEvent` via shortcut.ts.

## Chord Esc cancel

1. Enter recording.
2. Press Escape.

Expected: recording ends; trigger stays empty.

Refs: Escape branch 408-411.

## Chord click-away cancel

1. Enter recording.
2. Pointer-down outside the recorder (or blur the window).

Expected: recording ends without a chord.

Refs: cancel listeners 430-434.

## Insert token at caret

1. Select a snippet. Put the caret in the middle of the expansion.
2. Click `{date}` on the token bar.

Expected: token replaces the selection; caret sits after the token.

Refs: `insertToken` 437-452.

## 64-char trigger cap

1. New Snippet (text mode).
2. Paste or type more than 64 code points into the trigger field.

Expected: field keeps only the first 64 code points.

Refs: `updateTextTrigger` 375-379; `MAX_TEXT_TRIGGER_CHARS` in ipc.ts.

## Settings: engine toggle

1. Open Settings.
2. Toggle Text Expansion Engine.

Expected: switch and brand status dot stay in sync; tray would match in a
full run.

Refs: `toggle` 275+; settings switch 979-986.

## Settings: launch at login

1. Toggle Launch at Login.

Expected: switch flips; failure reverts the signal.

Refs: `toggleAutostart` 381-389; UI 989-1001.

## Settings: insert mode

1. Click Paste, Type, then Auto on the mini segmented control.

Expected: active segment and description text update; value persists.

Refs: insert mode UI 1004-1027; `changeInsertMode`.

## Settings: auto-paste words clamp / revert / Enter

1. Focus the words value-edit; select-all on focus.
2. Type `999`, blur -> clamps to 100.
3. Type `abc`, blur -> reverts to prior value.
4. Type `20`, press Enter -> blur commits 20.

Expected: ranges 1-100; Enter blurs; empty/NaN reverts.

Refs: `commitValueText` 358-373; words UI 1029-1065; findings 58-60.

## Settings: WPM clamp

1. Edit WPM value to `50` -> clamps to 100.
2. Edit to `2000` -> clamps to 1500.
3. Slider uses step 50.

Refs: WPM UI 1067-1097; range 100-1500 step 50.

## Settings: paste combo

1. Select Shift+Ins, then Ctrl+⇧+V, then Ctrl+V.

Expected: segment position follows `paste_combo`.

Refs: 1099-1122.

## Settings: restore delay clamp

1. Value-edit outside 3000-15000 clamps; step 100 on the slider.

Refs: 1124-1160.

## Settings: repair hook

1. Click Repair Hook.

Expected: temporary "Reinstalling hook..." then success or failure text;
message clears after 3.5 s.

Refs: `handleRepairHook` 155-170.

## Settings: run as admin

1. When not elevated, click Run as Admin.

Expected: requests elevation; on failure shows error text for 3.5 s.

Refs: `handleRestartElevated` 172-183; UI 1193-1222.

## Export / import summary text

1. Export snippets -> toast or path feedback.
2. Import a file -> toast like `Imported N, overwrote M, skipped K`.
3. Toast clears after 3 s.

Refs: import summary 600-608; transfer toast 710-712.

## Tray toggle sync

1. Toggle enabled from the tray.
2. Watch the manager window.

Expected: brand status and Settings switch update via `enabled-changed`.

Refs: `onEnabledChanged` 191-195; ipc 284-287.

## Close to tray

1. Click the window close button.

Expected: window hides; process stays; tray remains.

Refs: `win.close` 657; shell quit mode Explicit in GPUI main.

## Single instance

1. With HyperType running, start a second process.

Expected: second process exits (Secondary claim); no second hook.

Refs: `instance::claim` in crates/app; findings Runtime environment.

## Update check texts

1. Settings -> Check for updates (no update): "HyperType is up to date"
   then clear after 4 s.
2. Failure: "Update check failed" then clear after 4 s.
3. Available: title-bar pill and install button with version.

Refs: `checkForUpdate` 61-90; footer 1231-1240; pill 638-646.
