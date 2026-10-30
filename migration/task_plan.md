# Task plan - HyperType full GPUI port (1:1 with the SolidJS UI)

## Goal
Replace the Tauri + SolidJS + WebView2 manager with a native GPUI Kit window
that has the same functions, layout, colors, type, motion, and copy as the
v1.1.8 SolidJS UI, then remove the WebView stack.

## Current state
Phase 0 complete; owner signed off 2026-10-30 ("ok use 4.5 on high").
Phase 1 (parity judge) is in progress: one Grok 4.5 high agent.

## Next action
Wait for Phase 1 fix round 1 (R1-R3, see `port-progress.md`). Then review
its output read-only: run `migration/parity` validation (self-diff = 0,
broken variant fails), read `states.md` / `scenarios.md` against App.tsx.

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
- Subagents: Grok via T3 `delegate_task`, ONE at a time, each brief forbids
  sub-agents. No unit tests unless the owner asks; verify with the real app.
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
   Status: in_progress.
2. Design doc (`migration/RULEBOOK.md`): CSS token -> GPUI constant table,
   element map, state ownership, motion table, allowed deviations.
   Adversarial review (not a bakeoff: this is a redesign posture). Status: pending.
3. Translation slices, one Grok agent at a time, each with a separate
   read-only review and a parity run. Status: pending.
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
4. Build and run: clippy clean, release build, smoke from tray. Status: pending.
5. Parity: all states + scenarios green; owner visual sign-off. Status: pending.
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

## Open questions (owner)
1. Confirm Phase 6 (new installer + release script, delete Tauri/Solid/Bun)
   is in scope. It touches the release contract. Needed before Phase 6 only.

## Blockers
None.
