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
fallback. `capture-reference` runs `bun run build` in the repo root, then
serves `dist/` on :1420. `reference/meta.json` records git HEAD and whether
`src/` is dirty.

## Commands

```bash
bun run capture-reference
bun run capture-gpui -- --pid <pid> --state default-selected
bun run diff
bun run run -- default-selected
bun run validate
```

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
