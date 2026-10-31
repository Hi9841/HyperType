# Parity judge

Compares the SolidJS reference UI to the GPUI port, state by state.

Two metrics per state (both must pass):

- **DETAIL** - pixelmatch on full resolution (glyphs / edges)
- **FILL** - NxN box-average RGB blocks (surfaces / layout color)

## Setup

```bash
cd migration/parity
bun install
```

Scripts run under Node (Playwright CDP). Edge is preferred; Chrome is the
fallback. The SolidJS UI left the repo in v1.2.0, so `capture-reference`
checks out tag `v1.1.8` once into `.web-ref/` (a git worktree), runs
`bun run build` there, and serves its `dist/` on :1420 (`scripts/web-ref.ts`).
`reference/meta.json` records the reference commit.

## Commands

```bash
bun run capture-reference
bun run capture-gpui -- --pid <pid> --state default-selected
bun run boxes -- <state> [--grep <class-or-text>] [--region x,y,w,h]
bun run diff
bun run run -- default-selected
bun run validate
```

`capture-reference` also writes `reference/<state>.boxes.json` with exact
element boxes and text line rects (CSS px). Use `bun run boxes` to print a
compact table for a state. Match GPUI element y and height within 1 px.

## Parity binary

```bash
set APPDATA=migration\parity\seed
cargo run -p hypertype --features parity -- --parity-state default-selected
```

Skips the instance mutex and the keyboard hook. Opens 960x640 at once.

## Docs

- `states.md` - named frames, animation settle, how to add GPUI arms
- `scenarios.md` - behavior checks for later slices
- `VALIDATION.md` - judge proof numbers and pass thresholds
