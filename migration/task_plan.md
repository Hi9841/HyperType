# Task plan - HyperType full GPUI port (1:1 with the SolidJS UI)

## Goal
Replace the Tauri + SolidJS + WebView2 manager with a native GPUI Kit window
that has the same functions, layout, colors, type, motion, and copy as the
v1.1.8 SolidJS UI, then remove the WebView stack.

## Current state
All UI slices are merged on main: S1 foundation, S2+S3 sidebar, S4 editor,
S5 settings, S6 tokens, S7a updater backend, S7b update UI (b4832d1).
Phase 5 full-frame sweep recorded (port-progress.md); geometry matched to the
reference boxes within 1px per slice; remaining differences are D1/D2/D4/D5.
Grok balance is exhausted; the coordinator works directly (owner choice).

## Next action
1. Owner visual sign-off on the side-by-side page (shown in chat).
2. Real-app checks need the installed HyperType stopped (it now runs
   elevated, PID 19256, so only the owner can quit it): then run the GPUI
   release build in its place (scripts/dev-run.ps1) and check expansion,
   tray, close-to-tray, single instance, hotkeys, autostart, RAM, size.
3. Phase 6 packaging plan (NSIS compatible with the v1.1.8 updater flags
   /P /UPDATE /R /ARGS, same install path + uninstall key, minisign-signed
   latest.json, local release per owner memory) - confirm before any
   release or outward step.

## Scope and constraints
- Spec = `src/App.tsx`, `src/styles.css`, `src/components/Icons.tsx`,
  `src/lib/ipc.ts`, `src/lib/shortcut.ts`, `src-tauri/tauri.conf.json`,
  `src-tauri/src/main.rs` at commit 137ac57 (v1.1.8). The old code stays
  runnable until Phase 6.
- Stack: GPUI Kit `=0.7.3` (already chosen, in `crates/app/Cargo.toml`).
- Visual direction CHANGED: the stack.menu redesign (Geist, JetBrains Mono,
  cyan, light+dark, `assets/theme.json`) in `migration/PLAN.md` is superseded.
  New target: the OLED purple dark theme in `src/styles.css`, dark only.
- Engine: `crates/core` does not change, except for bugs that the port finds.
- Contracts in `migration/PLAN.md` §"Contract inventory" stay frozen
  (data paths, chord format, autostart key, install path, updater feed).
- Rust builds use cargo (owner, 2026-10-30: "dont use blaze"; global AGENTS.md updated).
- Subagents: Grok 4.5 high via T3 `delegate_task`; owner allows several at
  once (2026-10-31). Each gets its own git worktree under `../wt/`; each brief
  forbids sub-agents. T3 `isolation: worktree` fails here (thread root is not
  the repo), so worktrees are made by hand.
- Review (owner, 2026-10-31): every implementer result gets a read-only
  review by a separate grok-4.7 `xhigh` agent; the coordinator rules on its
  findings (adversarial, default not-confirmed), then a fix round runs. No unit tests unless the owner asks; verify with the real app.
- Commits: small, straight to `main`, no agent co-author lines.

## Completion criteria
- Every parity state in `migration/parity/states.md` diffs under the agreed
  threshold against the SolidJS reference, and the owner signs off on the
  side-by-side images.
- Every scenario in `migration/parity/scenarios.md` passes in the real
  release build.
- `cargo clippy -p hypertype --all-targets -- -D warnings` and `cargo build --release -p hypertype` pass.
- The installer installs over v1.1.8, keeps data and autostart; the v1.1.8
  updater can install the new build; the new build updates itself.
- `src/`, `src-tauri/`, `index.html`, `vite.config.ts`, `package.json`,
  `bun.lock`, `tsconfig.json` are removed; README is true.

## Phases
0. Feasibility report and plan. Status: complete. (`FEASIBILITY.md`, signed off)
1. Parity judge: reference capture, GPUI capture hook, diff, scenarios;
   validated against the SolidJS UI AND a deliberately broken variant.
   Status: complete (commits 6fcaebe, 2dcf293).
2. Design doc (`migration/RULEBOOK.md`): CSS token -> GPUI constant table,
   element map, state ownership, motion table, allowed deviations.
   Adversarial review (not a bakeoff: this is a redesign posture). Status: complete (A1-A9).
3. Translation slices, one Grok agent at a time, each with a separate
   read-only review and a parity run. Status: complete (all merged).
   - S1 Tokens, system fonts, 13 SVG icons, frameless window, title bar.
   - S2 Sidebar: search, New/Export/Import, toast, list, rows, empty state,
     native confirm + file dialogs.
   - S3 Reorder: pointer drag with slide animation, grip ArrowUp/Down,
     lock while searching, missed-release end.
   - S4 Editor canvas: header, kind segmented control, trigger input,
     shortcut recorder, token bar, textarea, metrics, save/create/delete.
   - S5 Settings modal: all 8 rows, switches, sliders, value edits,
     hook badge + 1.5 s poll, repair, run as admin, version, quit.
   - S6 Token catalog modal.
   - S7 Updater without WebView: latest.json, minisign verify, progress,
     passive NSIS install, relaunch, startup check, title-bar pill.
4. Build and run: clippy clean, release build, smoke from tray. Status: in_progress (needs the installed app stopped).
5. Parity: all states + scenarios green; owner visual sign-off. Status: in_progress (sweep done; sign-off pending).
6. Packaging and cutover: NSIS per-user installer (same path, uninstall key,
   feed), release script, remove WebView stack, README. Status: pending.

## Decisions
- Kit 0.7.3 stays. Reason: established choice; it has every API the port
  needs (`findings.md` §Kit capabilities).
- Unit of work = feature slice, not file. Reason: 1 TSX file holds all UI;
  the target splits by view. Kit Step 2 bakeoff replaced by design-doc review.
- Judge = pixel diff vs the SolidJS UI running on its browser mock, plus a
  scenario list. Reason: the repo has 0 tests (`findings.md` §Tests).
- Reuse from `crates/app`: `instance.rs`, `tray.rs`, `hotkeys.rs`,
  `autostart.rs`, `events.rs`, `chord.rs` (after review). Replace:
  `theme.rs`, `widgets.rs`, `manager.rs`, `editor.rs`, `settings.rs`,
  `tokens.rs`, `assets/theme.json`, `assets/fonts/*`.

## Owner answers (2026-10-30)
- Implementers: grok-4.5, reasoningEffort `high` (no xhigh on 4.5).
- Build tool: cargo, not blaze.
- Phase 0 signed off.

## Owner answers (2026-10-31, later)
- Grok balance exhausted (HTTP 402). Owner: "You (Opus) do it directly":
  the coordinator implements and verifies the remaining work in this thread,
  with no subagents. Review = coordinator side-by-side + boxes + live checks.
- Phase 6 (NSIS installer, release script, remove Tauri/SolidJS/Bun) is IN
  SCOPE. Confirm each release-affecting or outward step with the owner first.

## Open questions (owner)
(none)

## Blockers
None.
