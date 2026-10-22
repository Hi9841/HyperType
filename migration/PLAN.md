# HyperType native rewrite - plan

Goal: replace the Tauri + SolidJS + WebView2 manager plane with a native
Rust UI (GPUI Kit). Keep the Rust engine. Keep every user-visible contract
below, so that v1.1.6 users update in place and keep their data.

Method: the stack.menu rewrite guide (https://stack.menu/rewrite/guide.md),
"staged transition". Each phase has a gate. The released v1.1.6 installer
and the Tauri adapter stay as the rollback until cutover.

## Decisions

| Topic | Choice | Reason |
| --- | --- | --- |
| UI framework | GPUI Kit `=0.7.3` (GPUI `gpui-pre 0.3.7`) | Owner choice. Kit has a multi-line Input, Switch, Slider, Dialog and theme tokens. 0.7.x matches the audited docs; 0.8.0 is 3 days old. |
| Visual direction | stack.menu tokens: Geist + JetBrains Mono, neutral canvas, hairline borders, one cyan accent, light and dark | Owner choice. |
| Boundary | `crates/core` owns engine, hook, expansion, storage, validation and mutations. UI adapters own windows, tray, hotkey registration, dialogs, autostart, updater. | Only `main.rs`, `ipc.rs` and `shortcuts.rs` touch Tauri today. |
| Hotkeys | Core defines a `Hotkeys` trait. Tauri adapter uses its plugin; GPUI app uses `global-hotkey` directly (the crate the plugin wraps). | Same chord parser, so stored chords stay valid. |
| Repo layout | Root Cargo workspace: `crates/core`, `crates/app`. `src-tauri` path-depends on core and keeps its own lockfile until cutover. | No broken tree on `main` at any commit. |

## Contract inventory (must not change)

| Contract | Value | Owner after rewrite |
| --- | --- | --- |
| Snippet store | `%APPDATA%\HyperType\snippets.json`, format in `storage.rs` | core |
| Settings | `%APPDATA%\HyperType\settings.json` (`AppSettings`) | core |
| Log | `%APPDATA%\HyperType\hypertype.log` | core |
| Chord string | `KeyboardEvent.code` tokens, e.g. `Ctrl+Shift+KeyV`, `Super` for Win | core (parse), app (record) |
| Autostart | `HKCU\...\CurrentVersion\Run`, value `HyperType` = `<install>\hypertype.exe --minimized` | app |
| Install | per-user NSIS, `%LOCALAPPDATA%\HyperType\hypertype.exe`, uninstall key `HyperType` | packaging |
| Updater feed | `releases/latest/download/latest.json`, minisign key in `tauri.conf.json` | app + release CI |
| Single instance | second launch focuses the first instance's window | app |
| Tray | left click opens; menu: Enabled (check), Open HyperType, Quit | app |
| Window lifetime | manager is created on demand and destroyed on close; tray keeps the process alive | app |
| Export/import | JSON file, `MAX_IMPORT_BYTES` 16 MB, summary imported/overwritten/skipped | core |

## Phases and gates

1. **Core extraction.** Move engine modules to `crates/core`; move command
   logic from `ipc.rs` to `core::commands`. Tauri app becomes a thin adapter.
   Gate: core tests pass; `src-tauri` builds; Tauri app runs and expands.
2. **App skeleton.** GPUI Kit binary: single instance, tray, engine start,
   on-demand window, hotkeys. Gate: release build runs from tray, expands text
   and chords, closes the window without exit. Idle RAM measured.
3. **Manager UI.** Library list (search, select, reorder), editor (text and
   shortcut kinds, recorder, tokens), settings, tokens catalog, import/export,
   autostart, hook repair, run as admin. Gate: each `App.tsx` flow done in the
   real app; keyboard-only path works; light and dark checked.
4. **Packaging and update.** NSIS per-user installer at the same path and
   uninstall key; updater reads the same feed and verifies minisign.
   Gate: install over v1.1.6 keeps data and autostart; v1.1.6 updater installs
   the new build; new build updates itself.
5. **Qualification and cutover.** Measure idle CPU/RAM, binary size and
   expansion latency against the README table. Remove `src/`, `src-tauri`,
   Bun files. Gate: README numbers come from measurement, not documentation.

## Checks (stack.menu Rust profile)

`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo test --locked`, `cargo build --release --locked`, then run the real
artifact. A check stays `not-run` until it has a result.

## Status

See `migration/PROGRESS.md`.
