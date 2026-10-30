# Findings - HyperType full GPUI port

Plan: [task_plan.md](task_plan.md). Report: [FEASIBILITY.md](FEASIBILITY.md).

## Repo state (2026-10-30, commit 137ac57, v1.1.8)
- Shipped product: Tauri v2 + SolidJS 1.9 + WebView2 (`package.json`,
  `src-tauri/tauri.conf.json`). Release CI exists (`.github/workflows/release.yml`)
  but the owner releases locally (memory: "Release without CI").
- A GPUI port started on 2026-10-22 (`migration/PLAN.md`, `PROGRESS.md`):
  Phase 1 core extraction done, Phase 2 native shell done, Phase 3 manager UI
  partial (commit 1ea2ade). It follows a stack.menu REDESIGN, not the SolidJS look.
- Sibling dirs `../baseline`, `../work`, `../oploop` are worktrees from a perf
  OpLoop session (2026-10-27). Not part of this task. Do not touch.
- Root `task_plan.md` / `findings.md` / `progress.md` belong to the finished
  reliability audit. Kept unchanged.

## Spec size (lines, `wc -l`)
| File | Lines | Role |
| --- | --- | --- |
| `src/App.tsx` | 1449 | All UI: title bar, sidebar, editor, 2 modals, updater |
| `src/styles.css` | 1219 | All tokens and component styles |
| `src/lib/ipc.ts` | 287 | Command surface + browser mock (5 seed snippets) |
| `src/components/Icons.tsx` | 249 | 13 stroke SVG icons |
| `src/lib/shortcut.ts` | 53 | Chord capture/format |
| `src-tauri/src/*.rs` | 382 | Tray, single instance, window, commands |

Existing GPUI crate `crates/app/src`: 2,519 lines in 16 files.

## Design tokens (from `src/styles.css` :root)
- bg #000000; surface-1 #0c0c0f; surface-2 #141418; surface-3 #1e1e26;
  field #111115; line #1c1c22; line-2 #282832; sep #18181f; control #202028;
  control-line #30303c; ink #f4f4f7; ink-2 #9e9ea8; ink-3 #6b6b78.
- accent #7c2dff; hover #8f47ff; down #681ee0; accent-ink #b580ff;
  ring rgba(124,45,255,.35); ring-soft .2; danger #ff453a; danger-tint
  rgba(255,69,58,.12); success #30d158.
- Fonts: "Segoe UI Variable Text" -> "Segoe UI"; mono "Cascadia Code" ->
  "Cascadia Mono" -> Consolas. Base 400 13px/1.5. Windows system fonts: no
  bundling needed. Geist/JetBrains Mono assets become unused.
- Motion: ease-out cubic-bezier(.2,0,0,1); spring cubic-bezier(.32,1.25,.32,1);
  fast 130 ms; smooth 180 ms; modal fadeIn 150 ms; listening pulse 1.2 s;
  spin 1 s linear.
- Window: 960x640, min 780x520, centered, frameless, dark, black pre-paint,
  hidden at start, shown unless `--minimized`. Layout grid 340px | 1fr.

## Behaviors that are easy to miss (App.tsx line refs)
- Initial selection: first snippet once, only when list non-empty (248-257).
- Text trigger capped at 64 chars by code points (375-379).
- Recorder: Esc cancels; pointerdown / blur / focusout cancel; chord needs a
  modifier; bare modifier ignored; focus moves to textarea (400-435).
- insertToken replaces the selection, then puts the caret after the token (437-452).
- Delete uses `confirm("Delete snippet \"<t>\"?")` -> needs a native OK/Cancel box (484).
- Reorder: rows slide by row height; dragged row follows pointer with z-index;
  ends on `buttons === 0`, pointercancel, window blur; locked while search
  non-empty; grip ArrowUp/ArrowDown moves one step (501-578).
- Success text clears after 3 s; transfer toast 3 s; hook message 3.5 s;
  update status 4 s.
- Settings: status refetch every 1.5 s while the modal is open (197-203).
- Value edits: select-all on focus, Enter blurs, blur commits clamped value,
  empty/NaN reverts (358-373). Ranges: words 1-100, WPM 100-1500 step 50,
  restore 3000-15000 step 100.
- Hook events use `toLocaleString()` (thousands separators).
- Updater check at mount (silent) and manual; pill in title bar opens Settings.
- Footer stats: lines = split("\n").length (0 when empty); words = trimmed
  split on whitespace; "line"/"lines", "word"/"words" plurals.
- Row stats badge: "<n> lines" if > 1 line else "<n> chars"; excerpt
  replaces "\n" with " ↵ ".

## Kit 0.7.3 capabilities (verified in `~/.cargo/registry/src/*/`)
- Input/Textarea: `appearance(false)`, `bordered(false)` -> fully custom chrome
  (`gpui-component-0.7.3/src/input/{input,textarea}.rs`).
- InputBaseState: `insert`, `replace`, `set_value`, `cursor`,
  `selected_range`, `set_selected_range`, `select_all`, `set_cursor_position`
  (`gpui-base-0.7.3/src/input/base/state.rs:1038-3150`). Enough for insertToken.
- Frameless: `div().window_control_area(WindowControlArea::Drag)` and
  `window.start_window_move()` (`gpui-pre-0.3.9/src/elements/div.rs:1257`,
  `window.rs:3006`).
- Icons: `svg().path(..)` from the app asset source; tinted by text color.
- Slider and Switch exist (`gpui-component-0.7.3/src/{slider,switch}.rs`) but
  their look differs; custom elements are likely simpler for 1:1.

## Known fidelity limits (must be listed as allowed deviations)
- `backdrop-filter: blur(4px)` on the modal backdrop: GPUI has no backdrop
  blur. Use the rgba(0,0,0,.75) fill only. Visual difference is small.
- Text raster: Chromium vs DirectWrite through GPUI. Glyph edges will differ
  by a few pixels; the diff threshold must allow this.
- `title=` tooltips: map to Kit `Tooltip`; delay and style differ from native
  WebView tooltips.
- `:focus-visible` outline 2px accent-ink offset 2px: draw by hand.
- `cursor: grab / wait / default`: GPUI `CursorStyle` has pointer, grab
  (OpenHand/ClosedHand); "wait" may not exist -> check `CursorStyle` enum.

## Tests
- 0 `#[test]` and 0 `#[gpui::test]` in `crates/` and `src-tauri/src`
  (unit tests removed in 6f78962). No frontend tests.
- So no inherited suite can be the judge. A parity judge must be built
  (Phase 1). Owner rule: no unit tests unless asked.

## Model catalog (T3 `orchestrator_capabilities`, 2026-10-30)
- grok-4.5: reasoningEffort high / medium / low. NO xhigh.
- grok-4.6, grok-4.7, grok-4.7-build-fast: xhigh / high / medium / low.

## Brief template (for every Grok slice)
Each brief contains: goal; the exact spec lines (App.tsx / styles.css ranges);
the RULEBOOK sections that apply; files the agent may edit (and may not);
the parity states and scenarios the slice must pass; the commands to run
(`cargo check -p hypertype`, `cargo clippy -p hypertype --all-targets -- -D warnings`,
parity script); "Do not start subagents or parallel agents; do all the work
yourself."; "Do not commit; do not write unit tests"; report format:
files changed, commands + results, open TODO(port) markers.

## Runtime environment (2026-10-30)
- `cargo check -p hypertype`: pass, 13 s incremental.
- Installed v1.x runs: `%LOCALAPPDATA%\HyperType\hypertype.exe` PID 2376.
  It owns the single-instance mutex and a keyboard hook. A parity run must
  skip `instance::claim()` and `hypertype_core::start()` hook install, or it
  exits at once (Secondary) / double-expands.
- Core data dir = env `APPDATA` + `HyperType` (`crates/core/src/storage.rs:28`).
  A parity run can set `APPDATA` to a seed dir.
- Fonts installed: `SegUIVar.ttf`, `CascadiaCode.ttf`, `CascadiaMono.ttf`.
- Browser mock status: version "1.1.5", hook_events 42, hook_active and
  healthy true, not elevated, auto / 600 WPM / ctrl_v / 5000 ms / 15 words.
  Seed: gm, addr, sig (2 lines), brb, omw (`src/lib/ipc.ts:131-160`).
