# Findings - HyperType UI/IPC/persistence/shortcut-lifecycle audit

Read-only audit. Scope: `src-tauri/src/ipc.rs`, `storage.rs`, `snippets.rs`,
`src/App.tsx`, `src/lib/ipc.ts`, `src/lib/shortcut.ts`, `src-tauri/src/main.rs`
(+ cross-reads of `shortcuts.rs`, `engine.rs`, `expansion/mod.rs`, `tauri.conf.json`).
Ranked by severity. Each entry: severity | file:line | failure scenario | minimal fix.

## HIGH

### H1 - Drag reorder uses filtered-list indices against the full list
`src/App.tsx:478-531` (`applyReorder`, `startDrag`, `gripKeys`) with
`filteredSnippets` memo at `src/App.tsx:208`.
`<For each={filteredSnippets()}>` passes `i()` (index in the **search-filtered**
array) into `startDrag`/`gripKeys`, but `applyReorder` splices `snippets()` (the
**full** list) and persists `next.map(s => s.trigger)` to `reorder_snippets`.
`startDrag` also clamps `dropIdx` with `snippets()?.length ?? 0` (full count), so
the bounds are inconsistent too.
Failure: user types a search (e.g. "gm"), drags a row - a *different* snippet than
the one dragged gets moved in the real list, and the scrambled order is written to
`snippets.json` permanently.
Fix: pass trigger keys (not indices) through the reorder path, or compute
`applyReorder` against `filteredSnippets()` and map back to full-list positions;
simplest: disable drag/grip reorder while `search()` is non-empty.

### H2 - "Enabled" toggle is never persisted; every launch re-enables
`src-tauri/src/main.rs:148` (`enabled: AtomicBool::new(true)` hardcoded) and
`src-tauri/src/ipc.rs:369-374` (`toggle_enabled` only flips the atomic and syncs
the tray - no `settings.json` write; there is no `enabled` field in
`storage::AppSettings` / `StoredSettings`).
Failure: user turns HyperType off, quits or reboots (especially autostart
`--minimized`, where they never see the window) - the engine is silently back ON
and expands again, contradicting the state they left it in. Tray checkmark and
UI switch both rebuild as `true` (`build_tray` at `main.rs:80`).
Fix: add `enabled: bool` to `AppSettings`; persist it in `toggle_enabled`; load it
in `main()` before constructing `AppState` and pass into `build_tray`.

## MEDIUM

### M1 - Settings persist failures are swallowed; UI and disk diverge silently
`src-tauri/src/ipc.rs:102-109` (`persist_settings` only `logging::error`s) and
`set_insert_mode/set_wpm/set_paste_combo/set_restore_delay_ms/set_auto_paste_words`
at `ipc.rs:116-142` return `()`. Backend updates the in-memory atomic first, then
writes disk; on write failure the UI (`App.tsx` `changeInsertMode`/`commitWpm`/
etc.) keeps its optimistic value and never refetches, so the user sees "saved".
Failure: setting appears persisted but reverts to the previous value on restart;
no error is surfaced anywhere.
Fix: return `Result<(),String>` from the setters, surface it in the UI (or at
least `refetchStatus()` on failure); same for the snippet `persist()` error path
(`ipc.rs:344-350`) so a failed add/edit/delete write isn't silently lost on
restart.

### M2 - Disabling the engine does not release OS hotkeys; chords stay swallowed
`src-tauri/src/shortcuts.rs:73` (`if !state.enabled.load(..) { return; }` inside
the `on_shortcut` handler). `RegisterHotKey` still owns the chord, so Windows
consumes it and never delivers it to the focused app; the handler merely drops it.
Failure: user pauses HyperType to use a `Ctrl+Alt+…` chord in another app - the
chord is hijacked and does nothing, which reads as "the shortcut is broken."
Fix: on disable, unregister all Shortcut-kind hotkeys (and re-register on enable),
or document that Shortcut triggers are reserved while the app runs. Text triggers
are unaffected (engine checks `enabled`).

### M3 - Startup `register_all` is a fragile background-thread race; failures only logged
`src-tauri/src/main.rs:202-205` spawns `shortcuts::register_all` in `setup`, while
the inline comment itself says each `RegisterHotKey` marshals to the main thread
and blocks - but the spawn happens *before* `run()` starts pumping (there is no
ordering guarantee, only the assumption the main thread reaches `run()`).
`register_all` (`shortcuts.rs:34-43`) and the per-item `register_one` failure are
only logged.
Failure: a persisted shortcut whose chord is now taken by another app silently
never registers and the user gets no feedback; registration ordering vs. the
event loop is racy. (Ctrl/Alt/Win chords still get hook-fallback coverage, Shift
chords do not - see M4.)
Fix: perform registration from a `RunEvent::Ready` hook (or an explicit
`app.run_on_main_thread` handoff) instead of an uncoordinated spawn, and surface
a registration-failure summary to the UI.

## LOW

### L1 - Hook fallback coverage is partial: no Shift-only chords, limited key map
`src-tauri/src/shortcuts.rs:105` (`if !ctrl && !alt && !win { return false; }`) and
`vk_to_key_name` at `shortcuts.rs:158-171` (letters, digits, F1-F12, Space only).
`chordFromEvent` (`src/lib/shortcut.ts`) *does* allow Shift-only chords
(`Shift+KeyV`), and non-canonical/imported trigger strings (e.g. `KeyL+Ctrl`,
`Ctrl+ArrowLeft`, `Ctrl+Slash`) won't match the canonical
`ctrl+alt+shift+super+key` reconstruction.
Failure: a Shift-only or punctuation shortcut that fails `RegisterHotKey` at
startup is silently dead (no fallback), while Ctrl chords would still fire.
Fix: include `shift` in the fallback gate and map the remaining `KeyboardEvent.code`
tokens (arrows, punctuation, Backquote, etc.) in `vk_to_key_name`; normalize
trigger token order before comparing.

### L2 - Import overwrites existing snippets and reports them as "imported"
`src-tauri/src/ipc.rs:286-333` (`import_snippets`): an entry whose trigger already
exists is overwritten (expansion/kind replaced) and still increments
`imported_count`, so the UI toast "Imported N" overstates new content.
Failure: user imports a 100-snippet file where 90 triggers already exist - told
"Imported 100" when only 10 were new and 90 silently changed.
Fix: separate `overwritten` from `imported` in `ImportSummary` and the toast.

### L3 - Import size gate rejects overwrite-heavy files prematurely
`src-tauri/src/ipc.rs:272-276`: `if imported.len() > MAX_SNIPPETS` rejects the
whole file before considering that many entries may overwrite existing triggers.
Failure: a file with 10,001 entries (mostly updates to existing snippets) is
refused even though the per-item `projected_count` logic would accept it.
Fix: drop the coarse pre-check or apply it to the *new* trigger count.

### L4 - Shortcut recorder has no click-away cancel; global capture blocks typing
`src/App.tsx` `startShortcutRecording` (near line 330): once armed, the capture
`keydown` listener stays until a chord or `Escape`; clicking elsewhere (e.g. into
the expansion textarea) does not cancel it, and `e.preventDefault()` swallows
every keystroke while armed.
Failure: user clicks Record, changes their mind, clicks the expansion box and
types - nothing appears (each key intercepted), looks frozen until Esc.
Fix: add a window `pointerdown`/`blur`/`focusout` cancel, or cancel when the
textarea gains focus.

### L5 - Autostart `--minimized` window flashes before hiding
`src-tauri/src/main.rs:207-211` hides the window in `setup`, but
`tauri.conf.json` declares `"visible": true`, so the window is created and shown,
then hidden.
Failure: on boot the HyperType window flashes briefly; cosmetic.
Fix: set the config window `"visible": false` and show explicitly in
`open_main_window`, or hide in the `on_window_event`/`created` hook before paint.

### L6 - Updater relaunch may race single-instance
`src/App.tsx` `installDownloadedUpdate` → `downloadAndInstall` → `relaunch()`.
With the single-instance plugin holding the lock, if the old process hasn't fully
exited before the new one starts, the new process can route to the old instance's
focus callback and terminate, leaving the pre-update binary running.
Failure: user installs an update, app "restarts" but still shows the old version
until a manual quit.
Fix: `request_quit()`/`app.exit()` before `relaunch()`, or verify the relaunch
path exits first (already low confidence; verify against plugin-process semantics).

### L7 - Initial-selection interval can clobber an in-progress edit
`src/App.tsx:245-252`: a 100 ms `setInterval` selects `list[0]` whenever
`selectedTrigger() === null && !isCreatingNew()` and clears only after selecting.
Failure: negligible in practice (user can't type before load), but the poll never
stops while the list is empty (runs forever at 100 ms after a delete-all).
Fix: guard with a one-shot `createEffect` on first non-empty `snippets()`, and
don't poll when the list is empty.

## CLEAN (verified, no issue found)

- **Snippet store ordering** (`snippets.rs`): `insert` appends new / keeps
  position on edit, `update` keeps position on rename, `set_order` preserves
  missing/unknown triggers, `from_map` sorts deterministically. Backed by tests.
- **Shortcut kind-change hotkey cleanup** (`ipc.rs edit_snippet`): Text→Shortcut
  registers new; Shortcut→Text unregisters old; Shortcut→Shortcut rename/rebind
  unregisters old only after the new chord registers, with rollback on failure.
  Import handles all three kind transitions the same way. No stale OS hotkey.
- **MAX_SNIPPETS runtime enforcement**: `add_snippet` rejects at `len() >= MAX`;
  import uses `projected_count` correctly for new triggers (only the coarse
  whole-file pre-check in L3 is questionable).
- **`persist()` snapshot serialization** (`ipc.rs:334-351`): full snapshot taken
  under the read lock after the mutation, serialized outside the lock; writes are
  atomic temp+rename (`storage.rs save_entries`). No torn writes.
- **Frontend↔backend IPC validation parity** (`ipc.ts validateSnippet` vs
  `ipc.rs validate_snippet`): limits and control-char rules match.
- **Search filtering itself** (`App.tsx:208`): correct substring match on
  trigger/expansion; only the reorder-index interaction (H1) is broken.
- **Settings load/migration** (`storage.rs load_settings`): missing/unreadable →
  defaults; legacy `type_out` and unknown-enum fallback handled; round-trip tested.

## Cross-reference
- `findings.md` F1 (heartbeat watchdog false-positive in `keyboard.rs`) and F2
  (`highestAvailable` manifest in `build.rs`) are outside this worker's scope but
  remain the top "appears not to work" causes; nothing here contradicts them.
