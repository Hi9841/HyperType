# Slice briefs (Phase 3 fan-out)

Launch after S1 merges into main. One worktree per slice under `../wt/`,
branch `port/<slice>`, from main. Implementer: grok-4.5 `high`. Reviewer
after each result: grok-4.7 `xhigh` (read-only). See [task_plan.md](task_plan.md).

## Common block (put in every brief)

- Do not start subagents or parallel agents; do all the work yourself. Do not
  push. No unit tests. Do not ask confirmation questions; the brief is the
  authorization. End with the full written report as the LAST message.
- Work only in `C:\Users\hi\Desktop\Work\HyperType\wt\<slice>`. One commit,
  `feat(app): <slice title> [skip ci]`, no co-author lines. cargo, not blaze.
- Binding: `migration/RULEBOOK.md` (sections 2-9; section 4 frozen Studio and
  Editor API; 4.1 behavioral contracts; section 7 rules 1-16; section 9 the
  only allowed deviations). Edit ONLY the files your slice owns (section 2).
  Need a change in another file? Put it under "Requests" in the report.
- Shared widgets live in `ui/widgets.rs` (S1). Use them; do not copy them.
- Parity: add arms only in your file's `parity_state`. Run
  `migration/parity` (`bun run run -- <state> --region x,y,w,h`) with APPDATA
  on a temp copy of `migration/parity/seed`. Never stop the installed
  HyperType; never touch the real `%APPDATA%\HyperType`.
- Captures and outputs stay in YOUR worktree (`wt/<slice>/migration/parity`)
  or %TEMP%; run only the `boxes` CLI from the main checkout (read-only).
- Geometry: get exact targets with `bun run boxes -- <state> --grep <class>`
  (reference element and text line boxes); match within 1px.
- Checks: `cargo clippy -p hypertype --all-targets -- -D warnings` and with
  `--features parity`.
- Report: files changed; per spec line range what was built; parity numbers
  per state and region; each remaining difference classified (defect / D-n /
  raster) with coordinates; scenarios run and results; requests; commit hash.

## S2+S3 - sidebar (`ui/sidebar.rs`)

- Spec: App.tsx 122-127 (search state), 208-222 (filter, reorder lock),
  580-614 (export/import + toast), 665-800 (sidebar markup), 501-578
  (reorder: drag with slide animation, missed-release end, grip ArrowUp/Down,
  lock while searching). styles.css 212-502.
- Delete button on a row calls `studio.delete_snippet` (confirm is in Studio).
  "Create Snippet" in the empty state calls `studio.start_new_snippet`.
  Export/import with `rfd` (RULEBOOK section 8): default name
  `HyperType-snippets.json`, filter "HyperType snippets" / json.
- Parity states: default-selected region `0,42,340,598`, row-hover,
  search-filter, search-no-match, empty-library (all sidebar region).
- Scenarios (`migration/parity/scenarios.md`): Search, Drag reorder, Keyboard
  reorder, Reorder locked while searching, Delete with confirm OK / Cancel
  (from the row X), Export / import summary text.

## S4 - editor (`ui/editor.rs`)

- Spec: App.tsx 129-136, 185-187, 224-246, 259-273, 375-379, 391-480
  (mode switch, recorder, insertToken, save), 802-955 (markup). styles.css
  504-793. shortcut.ts (chord via existing `chord.rs`).
- Implements the frozen Editor API (load, clear_for_new, insert_token,
  stop_recording, set_error). Save calls `studio.save_snippet`; Delete button
  calls `studio.delete_snippet`; "All Tokens" calls
  `studio.set_modal(Some(Modal::Tokens))`.
- Parity states: default-selected region `340,42,620,598`, new-snippet,
  save-disabled, shortcut-mode-empty, shortcut-recording (pulse frozen at t=0
  in parity mode).
- Scenarios: Create, Edit, Rename, Chord record, Chord Esc cancel, Chord
  click-away cancel, Insert token at caret, 64-char trigger cap.

## S5+S6 - settings and token modals (`ui/settings_modal.rs`, `ui/tokens_modal.rs`)

- Spec settings: App.tsx 146-183 (hook repair, run as admin), 197-203 (1.5 s
  poll while open), 287-373 (setters, value-edit commit), 957-1266 (markup,
  footer: version, check for updates, install button, quit). styles.css
  795-973, 1074-1219.
- Footer update UI reads `studio.update` (`UpdateUi`) and calls
  `update::check` / `update::install`; texts from RULEBOOK 4.1. S7b fills
  `update.rs` later; build against the stub.
- Spec tokens: App.tsx 1268-1446 (4 groups, every token and label verbatim,
  click inserts via `studio.insert_token` then closes). styles.css 975-1073.
- Parity states: settings-modal, tokens-modal (full frame).
- Scenarios: every "Settings: ..." scenario except tray; Update check texts.

## S7b - update UI glue (`ui/update.rs`) - after S5 or in parallel

- Fill `UpdateUi`, `check(studio, manual, cx)`, `install(studio, cx)` with
  `crate::updater` (merged in 737ca73). All updater calls on a background
  executor, never the UI thread; results applied on the foreground; stale
  results dropped. Texts and timers from RULEBOOK 4.1 (App.tsx 53-120).
- Installer launch exits the process (OS releases the mutex and hook);
  `relaunch` only after the instance guard is dropped.
- Parity: `update-available` state in `update.rs` `parity_state` (fake
  `UpdateInfo` 1.1.9) so the title-bar pill and the settings footer render.
