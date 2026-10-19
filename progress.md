# Progress - HyperType reliability audit & fix

## Baseline
- Read all Rust core + frontend + build/release files.
- Production log `%APPDATA%\HyperType\hypertype.log`: 723 lines, **120** false
  "dead hook chain" reinstall cycles, only 1 logged expansion → F1.
- `cargo test` failed locally with os error 740 (elevation manifest) → F2.
- Installed binary: `%LOCALAPPDATA%\HyperType\hypertype.exe`, v1.1.5, running
  elevated (PID 8112), cannot be killed from this non-elevated shell.

## Phase 1 - evidence (complete)
- Settings: auto mode, 600 WPM, ctrl_v, 3000ms restore, 15-word threshold.
- Snippets: 7 entries incl. a `Ctrl+KeyL` shortcut.

## Phase 2 - parallel audit (complete)
Run `run_40ca8874a627`. Four read-only workers (codex + pi/DeepSeek only):
| Worker | Agent | Dispatch | Task | Result |
|--------|-------|----------|------|--------|
| A hot path | codex | ctx_83196560bd54 | task_5d50faae2327 | 6 HIGH, 8 MED, 4 LOW |
| B expansion | pi | ctx_1097f261760f | task_d3aa7c59784b | 3 HIGH, 5 MED, 6 LOW |
| C UI/IPC | pi | ctx_70c8cacd8742 | task_13a14838a0a0 | 2 HIGH, 3 MED, 7 LOW |
| D build/elev | codex | ctx_a8a3049fc639 | task_ee62c967ee8b | 3 HIGH, 4 MED, 4 LOW |

All four `worker_done` reports processed, deliveries acknowledged, terminals released.
(The first B attempt used `--agent claude` before the user's instruction; it was
interrupted, marked failed, and retried with `--agent pi`.)

## Phase 3 - fixes (complete)
All edits are coordinator-owned. Summary:

- `keyboard.rs` - heartbeat timestamp captured before `SendInput`; pure
  `heartbeat_verdict` (3 consecutive misses + 30s cooldown); context-reset epoch;
  mouse hook bumps it; F24 passthrough; cooldown applies to all auto-reinstalls;
  poison-tolerant health. +5 tests.
- `build.rs` - reverted `highestAvailable` → default `asInvoker` (+Common-Controls).
- `engine.rs` - seed modifier/CapsLock state at start and after reset; clear buffer
  on context epoch; Ctrl/Alt+Backspace clears; VK_INSERT clears; poison-recovered
  snippet read. +2 tests.
- `platform.rs` - `seed_modifier_state`.
- `expansion/inject.rs` - atomic Unicode maps `\n`/`\t` to Return/Tab, collapses
  CRLF; `send()` checks SendInput count and logs rate-limited. +2 tests.
- `expansion/clipboard.rs` - `Snapshot::acquired`; no empty-clipboard wipe;
  `get_unicode_text` bounded by `GlobalSize`.
- `expansion/mod.rs` - failed snapshot falls back to typing; `{date+1M}` fixed;
  title-based target misclassification removed. +2 tests.
- `snippets.rs` - `shortcut_index` + `match_shortcut` + `normalize_chord`. +1 test.
- `shortcuts.rs` - fallback uses the index (no full-list clone); `sync_enabled`
  unregisters/re-registers OS hotkeys on pause/resume.
- `ipc.rs` - settings persistence includes `enabled`; toggle persists + syncs hotkeys.
- `storage.rs` - `enabled` persisted/loaded/migrated. +2 tests.
- `main.rs` - restore enabled at startup; tray toggle syncs hotkeys + persists;
  window shown explicitly unless `--minimized`.
- `tauri.conf.json` - `visible: false` (no logon flash).
- `App.tsx` - reorder locked during search; version fallback no longer stale.

## Phase 4 - verification (complete)
- `bunx tsc --noEmit` → clean.
- `bun run build` → success (dist rebuilt).
- `cargo test --manifest-path src-tauri/Cargo.toml` → **79 passed, 0 failed, 4 ignored**.
  (Runs unelevated now - proves the manifest fix.)
- `cargo build` → success.
- Runtime log check: PENDING - needs the new build running; the old elevated
  instance cannot be stopped from a non-elevated shell.
- `cargo build --release` → success; `src-tauri/target/release/hypertype.exe` (6.85 MB).
- Extracted the release exe manifest with `mt.exe`: it contains only the
  Common-Controls dependency and **no `requestedExecutionLevel`**, i.e. the
  Windows default `asInvoker`. Elevation is no longer forced.

## Phase 5 - memory/handoff (complete)
- PruneADE note appended to `.prunade/memory.jsonl` (`829647e4-500e-4e48-864a-aff99b630efc`).
- Planning files: `task_plan.md`, `findings.md`, `progress.md`.
- Worker C's report kept at `findings-ui-ipc.md`; worker B's at
  `%TEMP%\hypertype_expansion_audit.md`.

## Phase 6 - implementation round 2 (subagents) (complete)
Run `run_04d984a6b883`. Two implementation workers on disjoint files, both released and acked.

- **HOT (codex, ctx_c851bc5d6017, task_336d41b717e2)** - `platform.rs`, `engine.rs`, `keyboard.rs`:
  - Dead keys: `platform::decode_key` calls `ToUnicodeEx` with flag 0x04 (no kernel state
    mutation), returns `Decoded::{Text,Dead,None}`, appends every scalar; engine clears on Dead.
  - Monotonic timing: heartbeat/cooldown use `GetTickCount64` instead of `SystemTime`.
  - Robustness: poison-tolerant `HOOK_TX`/snippet locks; one-shot `SendError` → `ENGINE_HEALTHY=false`.
  - Focus context: `focused_control()` added to the engine reset key.
  - +5 tests. `cargo test` 84 passed / 0 failed / 4 ignored.
- **UI (pi/DeepSeek, ctx_9e272fad73b6, task_a05cceac983a)** - `ipc.rs`, `main.rs`, `App.tsx`, `lib/ipc.ts`:
  - Persist failures now return `Result<(),String>` and surface in the Settings modal.
  - Import summary separates `imported` / `overwritten`; coarse whole-file limit dropped.
  - Shortcut recorder cancels on pointerdown/blur/focusout.
  - One-shot initial selection (no 100ms poll of an empty list).
  - Startup ordering (D-M4): storage/state/engine/hook start in `.setup()` after the
    single-instance plugin claims primary.
  - Verification green: tsc clean, frontend build ok, `cargo test` 84 passed.

Coordinator integration: fixed the one leftover warning in `main.rs` (unused `Result`
from the tray-toggle `persist_settings`), then re-verified.

## Phase 6 verification (complete)
- `bunx tsc --noEmit` clean.
- `bun run build` success.
- `cargo test --manifest-path src-tauri/Cargo.toml` → **84 passed, 0 failed, 4 ignored**.
- `cargo build` → no warnings/errors.
- `cargo build --release` → `src-tauri/target/release/hypertype.exe` (6.85 MB, 09:25).
- Release manifest extracted with `mt.exe`: still no `requestedExecutionLevel` (asInvoker).

## Phase 7 - live runtime verification + heartbeat removal (complete)
Ran the rebuilt release exe and tested on the real desktop:
- First launch of the round-2 build still logged `heartbeat missed (ping=…, pong=0)` and
  reinstalled once. Root cause investigation: `hook_calls=0` - the injected probe never
  reached our own low-level hook in the non-elevated process.
- Tried a full F24 down/up pair and VK_F24 matching: still no pong. Real input
  (`SendKeys 'brb'`) **did** reach the hook and logged `expanded trigger of 3 chars`,
  proving the hook itself is healthy and the probe is the broken part.
- Final fix: removed the SendInput heartbeat and all its statics/tests. Watchdog now
  reinstalls only on definitive failure (thread exited / hook not installed) with the 30s
  cooldown; `is_heartbeat_healthy` renamed to `is_hook_healthy` (ready + engine alive).
- Removed `HEARTBEAT_SIGNATURE` / `VK_F24` from `consts.rs`.

Verification of the final build (`src-tauri/target/release/hypertype.exe`, running PID 27660):
- 40s idle → **0** `heartbeat` / `dead hook chain` / `reinstallation` lines.
- `brb` expanded twice via synthetic input (`expanded trigger of 3 chars`).
- `bunx tsc --noEmit` clean; `cargo test` **81 passed, 0 failed, 4 ignored**;
  `cargo build` 0 warnings; `bun run build` success.

## Failed attempts / notes
- `cargo test` before the manifest fix: os error 740.
- First worker B dispatch with `--agent claude` aborted per user instruction.
- PruneADE MCP tool not exposed in this harness; appended the JSONL entry directly
  in the documented schema.
