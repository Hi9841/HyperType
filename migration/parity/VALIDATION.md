# Parity judge validation

Date: 2026-10-30T22:26:43.835Z
Display scale: 1
DETAIL pixelmatch threshold: 0.1
FILL block: 4x4, tolerance: 3/255
DETAIL pass bar: 1%
FILL pass bar: 3%

## Results

| check | detail % | fill % | pass | criterion |
| --- | ---: | ---: | --- | --- |
| (a) Noise floor (ref vs ref) | 0.0000% | 0.0000% | PASS | both ~0% |
| (b) Broken CSS variant | 3.126% | 11.708% | FAIL | clearly fails |
| (c) Current GPUI default-selected | 6.449% | 96.010% | FAIL | must fail vs SolidJS |
| (e) Surface variant | 0.000% | 38.594% | FAIL | MUST FAIL fill |
| (f) 1-step border (--line +4) | 0.000% | 0.000% | PASS | report numbers; see note below |
| (g) Raster-noise (--disable-lcd-text) | 0.657% | 2.560% | PASS | MUST PASS both (glyph AA only) |

| state | size | detail % | detail px | fill % | fill blocks | pass | note |
| --- | --- | ---: | ---: | ---: | ---: | --- | --- |
| noise-floor | 960x640 | 0.000% | 0 | 0.000% | 0/38400 | PASS |  |
| broken-variant | 960x640 | 3.126% | 19206 | 11.708% | 4496/38400 | FAIL |  |
| surface-variant | 960x640 | 0.000% | 0 | 38.594% | 14820/38400 | FAIL |  |
| onestep-border | 960x640 | 0.000% | 0 | 0.000% | 0/38400 | PASS |  |
| raster-noise | 960x640 | 0.657% | 4039 | 2.560% | 983/38400 | PASS |  |
| default-selected | 960x640 | 6.449% | 39623 | 96.010% | 36868/38400 | FAIL |  |

## Chosen pass thresholds

- **DETAIL**: <= 1% mismatched pixels (pixelmatch threshold 0.1)
- **FILL**: <= 3% mismatched 4x4 blocks (tolerance 3/255)
- A state **PASSES** only when both metrics are under their bars.

Noise floor detail 0.0000% fill 0.0000%. Broken detail 3.126% fill 11.708%. Surface fill 38.594% (must fail fill). 1-step border detail 0.000% fill 0.000%. Raster-noise detail 0.657% fill 2.560%. GPUI detail 6.449% fill 96.010%. Chosen: DETAIL_PASS_PERCENT=1% (pixelmatch 0.1), FILL_PASS_PERCENT=3% (block 4x4, tolerance 3/255). A state PASSES only when both metrics are under their bars. Fill catches near-black surface swaps that YIQ pixelmatch misses; detail still flags glyph/edge breaks. Raster-noise stayed under both thresholds (glyph AA change only).

## Case (f) observation

--line #1c1c22 -> #202026 (delta 4). Observed detail 0.000% fill 0.000%. Fill stayed at 0%: 1px borders diluted inside 4x4 averages stay under tolerance 3/255.

## Notes

- Edge channel `msedge` is preferred; this host falls back to Chrome CDP when Edge is absent.
- Scripts run under Node (`node --experimental-strip-types`) because Bun's Playwright WebSocket hangs on CDP connect.
- `capture-reference` rebuilds `dist/` from current `src/` before serving, and records git HEAD + src dirty in `reference/meta.json`.
- After each state action the capture waits for finite animations to finish, freezes infinite animations at t=0, then waits two animation frames.
- GPUI capture waits for two consecutive identical frames instead of a fixed sleep.
- Installed HyperType must stay running; parity skips the instance mutex and the hook.
