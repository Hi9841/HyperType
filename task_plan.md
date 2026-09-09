# Task plan — HyperType reliability audit & fix

## Goal
Find and fix the root causes of "sometimes it just doesn't work" in HyperType
(text expansion intermittently fails), verified by build + tests + production log.

## Current state
Phase 7 complete. Live runtime testing found the round-2 heartbeat fix was still
acting on an unreliable probe; the SendInput heartbeat was removed entirely. The
release build is running and verified: 40s idle with 0 reinstalls, expansions work.

## Next action
Continue the manual regression matrix from the previous message (insertion modes,
clipboard restore, pause/release hotkeys, import accounting). Then commit.

## Phase 6 — implementation round 2 (subagents) — complete
Run `run_04d984a6b883`:
- task_336d41b717e2 → codex: dead-key-safe decoding, monotonic clock, poison/SendError,
  focused-control context. Files: platform.rs, engine.rs, keyboard.rs. +5 tests.
- task_a05cceac983a → pi: persist-failure surfacing, import accounting, recorder cancel,
  one-shot initial selection, startup ordering. Files: ipc.rs, storage.rs, App.tsx,
  lib/ipc.ts, main.rs.
Both released/acked; coordinator fixed one leftover warning and re-verified
(84 tests, tsc clean, release built, manifest asInvoker).

## Scope and constraints
- Windows-only Tauri v2 + Rust app. Per-user NSIS install.
- User asked for: audit + fix, planning skill, "my agents" (Orca supervised workers).
- Workers restricted to codex + pi (DeepSeek) per user instruction.
- Fixes keep existing behavior/UX and are verifiable offline where possible.

## Completion criteria
- `bunx tsc --noEmit` clean, `bun run build` succeeds. DONE
- Rust unit tests pass unelevated. DONE (79 passed, 0 failed, 4 ignored e2e)
- No new "dead hook chain" false positives in a fresh runtime log during idle. PENDING runtime
- Each confirmed finding fixed with a test, or deferred with a documented reason. DONE

## Phases
1. Baseline + evidence from source and production log. Status: complete.
2. Parallel Orca worker audit (hot path, expansion, UI/IPC, build/elevation). Status: complete.
3. Implement confirmed fixes. Status: complete.
4. Verify: typecheck, build, Rust tests, runtime log check. Status: complete (runtime pending).
5. Record PruneADE memory + handoff summary. Status: complete.

## Decisions
- Treat the production log as primary evidence; source claims verified against it.
- Heartbeat watchdog false-positive was the primary cause and was fixed first.
- Audit workers read-only; coordinator owned edits to avoid worktree conflicts.
- Elevation: adopt worker D's recommendation — `asInvoker` + on-demand "Run as Admin".
- Deferred the changes that need a larger design (UIA password detection, dead-key/IME,
  elevated broker, Authenticode, single-instance startup ordering) with reasons in findings.md.

## Blockers
None. The only remaining check needs the new build running, which requires quitting
the currently running elevated v1.1.5 (cannot be killed from a non-elevated shell).
