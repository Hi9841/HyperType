# Migration progress

See [PLAN.md](PLAN.md) for the goal, contracts, and phase gates.

## Phase 1 - core extraction: done

- The engine is in `crates/core` (`hypertype-core`). `src-tauri` is a thin adapter over `hypertype_core::commands`.
- Core tests: 81 passed, 4 ignored (live-desktop e2e). Clippy is clean on core and on `src-tauri`.
- Not yet checked: a runtime run of the Tauri adapter after the extraction.

## Phase 2 - native shell: done

`crates/app` (`hypertype`) runs the engine without Tauri or WebView2.

| Item | Result |
| --- | --- |
| Single instance (Tauri protocol, same mutex and window names) | A second launch shows the window and exits |
| Close to tray | The window closes, the process stays (`QuitMode::Explicit`) |
| Keyboard hook | Installed at start |
| Tray | Created; clicks and menu not yet checked by hand |
| Hotkeys | Registered through `global-hotkey`; firing not yet checked by hand |
| Theme | Geist, JetBrains Mono, stack.menu light and dark palette |

Release measurements (2026-10-22), compared with installed v1.1.6:

| Metric | Native build | v1.1.6 |
| --- | --- | --- |
| Exe size | 12.2 MB | 6.6 MB |
| Working set, started with `--minimized` | 46.6 MB | about 16 MB |
| Working set after the window opened and closed | 61.5 MB | - |
| CPU in 20 s idle after close | about 0.1 s | - |

Risk: idle memory is about three times v1.1.6. Phase 5 must find the cause (GPUI start-up, fonts, GPU device) and set a budget.

## Phase 3 - manager UI: in progress

## Phase 4 - installer and updater: not started

## Phase 5 - qualification and cutover: not started
