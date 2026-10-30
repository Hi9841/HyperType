# Progress - HyperType full GPUI port

Plan: [task_plan.md](task_plan.md). Evidence: [findings.md](findings.md).

## 2026-10-30 - Phase 0: feasibility and plan (complete)
- Read the spec (App.tsx, styles.css, ipc.ts, shortcut.ts, Tauri shell and
  config) and the existing GPUI crate. Found the earlier port follows a
  stack.menu redesign; the owner now wants a 1:1 copy, so the visual
  direction in `PLAN.md` is superseded.
- Wrote `FEASIBILITY.md` (verdict: migrate now, judge first),
  `task_plan.md`, `findings.md`, `port-progress.md` (named so because `PROGRESS.md` exists and Windows names are case-insensitive).
- T3 catalog: grok-4.5 has no `xhigh`; grok-4.6 / 4.7 do. Asked the owner.

### Build baseline (blaze)
| Command | Result |
| --- | --- |
| `blaze check -p hypertype` | FAIL - ra-jit: no `blaze-proc-macro-srv.exe` beside ra-jit (Windows releases do not ship it) |
| `RA_JIT_PROC_MACRO_SERVER=0 blaze check -p hypertype` | FAIL 21.5 s - build script output `serde_core .../out/private.rs` not found under `target/rust-analyzer-rustc` |
| `blaze build -p hypertype` | FAIL exit 127, 2m31s cold, no message; stops at `Compiling hypertype v1.1.8 (cratespp)`; no new exe (target/debug/hypertype.exe still 2026-10-22) |

Owner rule: no cargo fallback without approval. Reported to the owner.

### Gate
Phase 0 ends here. Phase 1 starts when the owner answers the open questions
in `task_plan.md`.

### Failed attempts / notes
- Writing `migration/progress.md` overwrote the tracked `migration/PROGRESS.md`
  (Windows names are case-insensitive). Restored with `git checkout`; no loss.
  Rule for agents: this task's log is `port-progress.md`; never create a
  file whose name differs from an existing one only by case.

## 2026-10-30 - Owner answers
- Implementers: grok-4.5 at `high`. Build tool: cargo, not blaze
  (blaze rows above are history only).
- `cargo check -p hypertype`: pass, 13 s.

## 2026-10-30 - Phase 1: parity judge (in progress)
- Delegated to grok / grok-4.5 / reasoningEffort high (verified in the
  delegate result). clientRequestId `hypertype-gpui-phase1-judge-r1`.
  taskId `node:delegated-task:command%3Amcp%3A7fb0d3ca-23ca-41c1-8ab5-be785a9a9b5c%3Adelegate-task%3Ahypertype-gpui-phase1-judge-r1`.
- Scope: states.md, scenarios.md, reference capture (system Edge, display
  scale), PrintWindow capture, `parity` cargo feature, diff, VALIDATION.md.
- Next: read-only review of the result, then the Phase 1 gate.
- Result 2026-10-31: task completed with no final message, but files exist:
  `migration/parity/` (states.md, scenarios.md, scripts, 11 reference PNGs,
  VALIDATION.md, README.md), `crates/app/src/parity.rs`, `parity` feature,
  `hypertype_core::load_state_only()`. Small clippy edits in settings.rs,
  tokens.rs (files to be replaced later anyway).
- Agent numbers: noise 0.000%, broken CSS 3.126%, current GPUI 6.449%,
  proposed threshold 0.50%. Display scale 1. Edge absent; Chrome 155 used.

### Phase 1 review (coordinator, read-only) - 3 defects
- R1 BLOCKER metric is blind to dark fills: diff/default-selected.png marks
  only text and edges. Sidebar #0c0c0f and row #141418 vs GPUI fills are not
  flagged (pixelmatch YIQ threshold 0.1). This palette is all dark grays,
  so wrong surfaces would pass. A full redesign scores only 6.4%.
- R2 BLOCKER stale reference: served `dist/` from 2026-10-19; `src/`
  changed 2026-10-27 (drag fixes, slide animation).
- R3 BLOCKER mid-animation captures: settings-modal.png shows the modal at
  partial opacity (fadeIn 150 ms). Hover/seg/switch transitions also at risk;
  the recorder pulse is infinite.
- Rust changes reviewed: OK (feature-gated, `start()` behavior unchanged).
- Fix round 1 delegated (grok-4.5 high, verified): clientRequestId
  `hypertype-gpui-phase1-judge-r2`, taskId suffix `...hypertype-gpui-phase1-judge-r2`.
  Scope: R1 fill metric + validation (e)(f)(g), R2 rebuild + commit in meta,
  R3 wait for animations / freeze infinite ones, table fix.
