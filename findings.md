# Findings - HyperType reliability audit

Status legend: FIXED (code + test), DEFERRED (documented, needs larger change).

## Confirmed from production evidence

### F1 - CRITICAL: heartbeat watchdog false-positives → hook reinstalled every ~2.5s - FIXED (probe removed)
Evidence: `%APPDATA%\HyperType\hypertype.log` had **120** cycles of
`watchdog detected dead hook chain (ping=<T>, pong=<T-1>)` and only 1 logged
expansion. Cause: `send_heartbeat_ping` stored the ping timestamp *after*
`SendInput` returned, while the hook stored its pong while processing the event,
so `pong < ping` was always true while idle. Every idle pause tore the hook down
and back up; keystrokes in the teardown window were lost.

First fix attempt (capture timestamp before `SendInput`; require 3 consecutive
misses + 30s cooldown) reduced but did not eliminate the problem. Runtime testing
of the **non-elevated** build showed the SendInput probe is not reliably
delivered back to our own low-level hook at all: `hook_calls=0`, `pong=0`, even
with a full F24 down/up pair and VK_F24 matching - while real input and
`expanded trigger` still worked. The probe is therefore unusable in the shipping
(non-elevated) configuration.

Final fix (`keyboard.rs`): removed the SendInput heartbeat entirely. The watchdog
now reinstalls only on definitive failure - the hook thread exited or the hook is
not installed - with a 30s cooldown, plus the user-facing "Repair Hook" button.
Verified live: 40s idle → **0** reinstalls; typing `brb` still expanded twice.

### F2 - HIGH: `highestAvailable` manifest forces elevation - FIXED
`src-tauri/build.rs` requested `highestAvailable`: UAC prompt on every launch for
admins (manual, autostart, updater relaunch), unreliable per-user autostart,
single-instance IPC blocked across integrity levels, and local `cargo test`
failed with os error 740. The app's own Settings UI already offers on-demand
"Run as Admin" (`platform::restart_elevated`), which is the intended design.
Fix: reverted `build.rs` to `tauri_build::build()` (asInvoker + Common-Controls).
`cargo test` now runs unelevated: 76 passed, 0 failed.

## Expansion / injection (worker B report, `%TEMP%\hypertype_expansion_audit.md`)

| ID | Sev | Finding | Status |
|----|-----|---------|--------|
| H1 | HIGH | Password detection misses browser/Electron/UWP (no UIA). Masked-field expansion, incl. `{clipboard}` | DEFERRED - needs IUIAutomation; note as known limitation |
| H2 | HIGH | Atomic (Auto short-text) mode drops `\n`/`\t`: Windows emits no WM_CHAR for control code points | FIXED - `unicode_content_steps` maps to VK_RETURN/VK_TAB, collapses CRLF; 2 tests |
| H3 | HIGH | Failed clipboard snapshot (`open()` busy) restores empty → wipes user clipboard | FIXED - `Snapshot::acquired`; skip restore and fall back to typing |
| M1 | MED | Non-BMP/emoji surrogate handling + backspace count in chars vs UTF-16 units | DEFERRED - control units already pair; backspace semantics need app testing |
| M2 | MED | Modifier lift/restore desync via post-lift `GetAsyncKeyState` | DEFERRED - correct fix needs physical-state tracking; affects shifted triggers |
| M3 | MED | `SendInput` failures silently swallowed under UIPI | FIXED - `send()` checks return + logs (rate-limited 5s) |
| M4 | MED | Window-title heuristics falsely classify "Terminal"/"codex" → wrong paste chord | FIXED - dropped bare "terminal"; Codex now process-name only; 1 test |
| M5 | MED | Unmappable chars in terminal replay fall back to ignored Unicode input | DEFERRED |
| L1 | LOW | `{date+1M}` parsed as minutes | FIXED - `parse_offset` matches `M` before lowercasing; 1 test |
| L2 | LOW | `get_unicode_text` can read past HGLOBAL | FIXED - bounded by `GlobalSize` |
| L3-L6 | LOW | Clipboard hijack window, partial cancel, snapshot format caps, ANSI `?` fallback | DEFERRED |

Clean: clipboard sequence/generation guards, restore worker, WPM math,
timeBeginPeriod pairing, template-token panic paths, atomic batch ordering.

## UI / IPC / persistence (worker C report, `findings-ui-ipc.md`)

| ID | Sev | Finding | Status |
|----|-----|---------|--------|
| H1 | HIGH | Drag reorder uses search-filtered indices against the full list → moves wrong row and persists scrambled order | FIXED - reorder locked while search is active (grip disabled, keyboard blocked) |
| H2 | HIGH | Enabled toggle never persisted → every launch re-enables | FIXED - `enabled` in settings.json; loaded at startup; persisted from window and tray |
| M1 | MED | Settings/snippet persist failures swallowed; UI diverges from disk | FIXED - setters/persist return `Result`; App.tsx surfaces `settingsError` |
| M2 | MED | Disabling engine keeps OS hotkeys registered (chords still swallowed) | FIXED - `shortcuts::sync_enabled` unregisters on pause, re-registers on resume |
| M3 | MED | Startup `register_all` background-thread race; failures only logged | PARTIAL - storage/state/engine/hook moved into `.setup()` after single-instance claims primary; `register_all` still spawned and failures logged |
| L1-L7 | LOW | Fallback chord coverage, import accounting, recorder click-away, autostart flash, updater relaunch race, initial-selection poll | PARTIAL - import accounting, recorder click-away, and one-shot initial selection FIXED; fallback coverage / updater race deferred |

Clean: snippet store ordering, shortcut kind-change cleanup/rollback,
MAX_SNIPPETS runtime enforcement, persist snapshot atomicity, IPC validation
parity, search filtering, settings migration.

## Hot path (worker A report, msg_ea694fcda4a6)

| ID | Sev | Finding | Status |
|----|-----|---------|--------|
| H1 | HIGH | Dead-key layouts: `ToUnicodeEx` with flags=0 mutates kernel keyboard buffer; multi-char/ligature output truncated | FIXED - `platform::decode_key` uses flag 0x04, returns `Decoded{Text,Dead,None}`, appends all scalars; engine clears on Dead; 4 tests |
| H2 | HIGH | IME composition: raw VK matching fires mid-composition and backspaces into candidates | DEFERRED - needs composition detection |
| H3 | HIGH | Mouse click/drag in the same window does not reset the buffer → stale prefix fires and backspaces unrelated text | FIXED - mouse hook bumps a context-reset epoch; engine clears; test |
| H4 | HIGH | Fallback shortcut is not consumed, so the focused app also acts | DEFERRED - needs hook-level matching/consumption |
| H5 | HIGH | Keystate not seeded at startup (CapsLock/modifiers) → Ctrl+C decoded as `c`, wrong case | FIXED - `platform::seed_modifier_state` at engine start and after context reset |
| H6 | HIGH | Fallback cloned the entire snippet list on every Ctrl/Alt/Win keydown | FIXED - indexed `Snippets::match_shortcut`; clones one entry; test |
| M1 | MED | Foreground identity is only the top-level HWND; intra-window focus changes missed | FIXED - focused control added to the engine context key; test |
| M2 | MED | Ctrl/Alt+Backspace treated as one-char pop; Shift+Insert not an invalidator | FIXED - modified Backspace clears; Insert clears; test |
| M3 | MED | Buffer always cleared after expansion, even on failure; boundary state lost | DEFERRED |
| M4 | MED | Unbounded mpsc backlog can replay stale keys against the wrong context | DEFERRED |
| M5 | MED | Permanent install failure retried every 2.5s (cooldown only applied to heartbeat) | FIXED - cooldown applies to every automatic reinstall cause |
| M6 | MED | Any physical/remapped F24 is consumed as a heartbeat | FIXED - consume only `HEARTBEAT_SIGNATURE` |
| M7 | MED | Heartbeat proof goes stale under UIPI; cannot distinguish healthy from unprobeable | PARTIAL - failed send no longer arms the probe; explicit health enum deferred |
| M8 | MED | Single global 400ms shortcut debounce suppresses two different chords pressed quickly | DEFERRED |
| L1-L4 | LOW | Poisoned snippet lock panics engine; health state only flips late; wall-clock cooldown; mouse-hook readiness untracked | FIXED (poison-tolerant locks, one-shot `SendError` health, `GetTickCount64` monotonic clock); mouse-hook readiness still deferred |

Clean: no self-injection leak, no panic path inside the hook callbacks, mpsc
`send` is non-blocking, NumLock/ScrollLock bits irrelevant to the decoder.

## Build / release (worker D report, msg build-elevation)

Recommendation (adopted): `asInvoker` + `uiAccess=false`, keep elevation explicit.

| ID | Sev | Finding | Status |
|----|-----|---------|--------|
| H1 | HIGH | `highestAvailable` breaks admin-user autostart/manual launch (UAC every time) and forces the WebView/updater high | FIXED - reverted to `tauri_build::build()` |
| H2 | HIGH | Published installer is not Authenticode-signed (updater sig only); SmartScreen/unverified publisher | DEFERRED - needs a Windows code-signing cert (out of code scope) |
| H3 | HIGH | `restart_elevated` runs as the credential-supplying admin's identity → different APPDATA, "missing" snippets; can update the wrong profile | DEFERRED - recommends an elevated injection broker; document the caveat |
| M1 | MED | `restart_elevated` races single-instance (child can self-classify as secondary and exit; high instance blocks WM_COPYDATA) | DEFERRED |
| M2 | MED | `cargo test` os error 740 for non-elevated developers/CI | FIXED by the manifest revert; verified 79 tests run unelevated |
| M3 | MED | UIPI SendInput failure is silent; no user-facing "cannot type into an admin app" signal | PARTIAL - rate-limited error log added in `inject::send`; toast deferred |
| M4 | MED | Storage/engine/hook start before the single-instance plugin rejects a secondary process → brief double engine/hook | FIXED - startup moved into `.setup()` after the plugin claims primary |
| L1-L4 | LOW | Heartbeat infers UIPI from GetLastError; window flash at logon; stale version fallback; hard-coded release notes | FIXED (flash, version fallback); rest deferred |

Verified good: version consistency (package/Cargo/tauri all 1.1.5 + tag check),
updater pubkey/endpoints, icons, currentUser NSIS, CSP, capabilities, relaunch token.

## Known limitation to document
Non-elevated HyperType cannot inject into elevated windows (UIPI). Users who
need that should use Settings → "Run as Admin". Browser/Electron password fields
are not detected (H1) until UI Automation is added.
