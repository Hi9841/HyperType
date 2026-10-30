# Feasibility report - HyperType UI: SolidJS/WebView2 -> GPUI Kit (1:1)

Date: 2026-10-30. Repo: `HyperType/HyperType`, commit 137ac57 (v1.1.8).
Method: code-migration kit, prompt 00. Evidence: [findings.md](findings.md).

## The six steps (index)
1. Map and rules: dependency order, gap inventory, rulebook.
2. Stress-test the rules before fan-out.
3. Translate every unit (implementer, reviewers, fixer).
4. Compile with one survey build.
5. Run it: hello world, then smoke.
6. Match behavior against the old code with a validated judge.

## Case for leaving
- WebView2 is a second runtime: `src-tauri/src/main.rs:35-67` destroys and
  rebuilds a WebView2 process per window open to keep idle RAM low.
- Two languages for one command surface: `src/lib/ipc.ts` (287 lines) mirrors
  `crates/core/src/commands.rs` (382 lines), including a hand-kept validator
  copy (`ipc.ts:15-22`) that can drift from core.
- Release needs Bun + Vite + Tauri CLI + NSIS (`package.json`, `tauri.conf.json`).
- The port is half done: `crates/app` (2,519 lines) runs the engine, tray,
  hotkeys, and single instance without Tauri (`migration/PROGRESS.md`).

Counter-evidence: the GPUI shell measured 46.6 MB idle vs about 16 MB for
v1.1.6 and 12.2 MB vs 6.6 MB exe (`migration/PROGRESS.md`). Leaving WebView2
does not make HyperType smaller today.

## Three calls
1. **Structure-preserving or redesign?** Engine: preserving (no change to
   `crates/core`). UI: redesign of the mechanism with a fixed visual target -
   DOM + CSS cascade + Solid signals become GPUI entities, elements, and
   explicit hover/focus/animation state. One 1,449-line component becomes
   about 6 views. -> **Redesign posture for the UI**, forced by `src/App.tsx`,
   `src/styles.css`. Consequence: the kit's Step 2 bakeoff is invalid; use
   adversarial review of the design doc instead, unit = feature slice.
2. **What does verification cost?** Baseline: `blaze check -p hypertype`
   failed on Windows (ra-jit has no proc-macro server; see port-progress.md).
   Retry with `RA_JIT_PROC_MACRO_SERVER=0` in progress. Assumption: an
   incremental check of `crates/app` alone is under 1 min once deps are
   built, so the type check can run inside every slice (Step 4 dissolves into
   Step 3). The expensive referee is the visual diff + real-app scenarios.
   -> forced by `crates/app/Cargo.toml` (gpui-kit 0.7.3 dependency tree).
3. **Do the tests survive?** 0 tests exist (`grep '#[test]'` = 0 in
   `crates/`, `src-tauri/src`). Nothing survives because nothing exists.
   -> **Build the judge first** (`prompts/00b-judge-setup.md`), forced by
   commit 6f78962 (tests removed) and `src/` (no frontend tests).

### Judge sketch (seed for 00b)
- Visual: run the SolidJS UI with its browser mock (`bun run dev`,
  `src/lib/ipc.ts:131-258`, 5 seed snippets) at 960x640, DPR 1. Capture each
  named state with Playwright. GPUI side: a parity-only cargo feature seeds
  the same 5 snippets in memory (never touches `%APPDATA%`), opens the same
  state, and saves a window capture. Diff per state; report % pixels over a
  color tolerance plus a diff image. Validate: identical reference vs itself
  = 0; deliberately broken variant (accent changed, sidebar 300px) must fail.
- States (first cut): default-selected, hover-row, new-snippet,
  shortcut-mode-empty, shortcut-recording, shortcut-recorded, search-filter,
  search-no-match, empty-library, success-message, error-message,
  settings-modal, settings-update-available, tokens-modal, update-pill.
- Scenarios (real release app, keyboard + mouse): create, edit, rename,
  delete with confirm OK/Cancel, search, reorder by drag and by keys,
  reorder lock, record chord + Esc + click-away, insert token at caret,
  64-char cap, every settings control incl. value-edit clamp/revert,
  autostart toggle, repair hook, run as admin, export/import summary text,
  tray toggle sync, close-to-tray, single instance, update check texts.

## Six steps instantiated
| Step | Here | Kit prompt |
| --- | --- | --- |
| Judge | `migration/parity/`: capture + diff scripts, states.md, scenarios.md | 00b |
| 1 | Depmap is trivial (1 component file); skip the script. RULEBOOK = design doc: token table, element map, state owners, motion, deviations. Gap inventory = the "easy to miss" list in findings.md | 01 (skip), 02 (light), RULEBOOK template |
| 2 | Adversarial review of RULEBOOK by a separate reviewer; no bakeoff | 03 (substituted) |
| 3 | 7 slices S1-S7 (task_plan.md), one Grok agent at a time, then one read-only reviewer, then parity run | 04 (manual queue: 7 units) |
| 4 | Dissolved: `blaze check` inside each slice | 05 (skip) |
| 5 | Release build, start from tray, open/close window, one expansion | - |
| 6 | All parity states + scenarios; owner sign-off on side-by-side images | 06 after the gate |

## Cost and duration
Drivers: 7 slices + 1 judge + 1 design doc = 9 units x (1 implementer +
1 reviewer + about 1 fix round) x referee (blaze check + parity run).
Band: **low tens of millions of tokens** (order 10^7). Wall clock: 2-4 days
of agent time, sequential (one agent at a time). Owner attention: about
30 min per gate x 5 gates, plus visual sign-off on the image set.

## Model plan (owner decides)
- Design doc and amendments: Claude Opus 5.5 (me) - every error replicates.
- Slice implementers: Grok (owner request). Note: grok-4.5 tops out at
  `high`; `xhigh` exists only on grok-4.6 / 4.7.
- Reviewer per slice: a model other than the implementer, read-only (Claude
  Sonnet 5.5 high, or me).
- Fixers: the same Grok agent with the confirmed findings.

## Verdict
**Migrate now**, judge first. The engine is already UI-free, the shell runs,
and every Kit API the port needs exists. The risk is fidelity, not
feasibility; the judge makes fidelity measurable.

Fact that would change it: the idle RAM budget. If the owner needs the
v1.1.x "16 MB background" figure, GPUI may never meet it (46.6 MB measured).
Check in under a day: start the release GPUI build with `--minimized`, read
its working set, then disable GPU init paths one by one (OpLoop).

## What I read and ran
Read: `src/App.tsx`, `src/styles.css`, `src/lib/ipc.ts`, `src/lib/shortcut.ts`,
part of `src/components/Icons.tsx`, `src-tauri/src/{main,ipc,shortcuts}.rs`,
`src-tauri/tauri.conf.json`, `crates/app/src/{main,shell,theme,widgets,chord}.rs`,
first 120 lines of `crates/app/src/manager.rs`, `migration/{PLAN,PROGRESS}.md`,
Kit 0.7.3 sources (grep). Ran: `git log`, `wc -l`, test grep,
`blaze check -p hypertype` (failed, see port-progress.md).
