# HyperType

Instant text expansion for Windows. Type a short trigger anywhere and it is
replaced with the full text. `gm` becomes `Good morning`, `addr` becomes your
address, and so on. HyperType runs in the background from the system tray.

This is the Windows MVP: a lean Rust core that owns all keyboard logic, with a
small SolidJS tray UI for managing snippets.

## Measured performance (release build)

| Target | Goal | Measured |
| --- | --- | --- |
| Idle CPU | ~0% | 0 ms CPU over a 3s idle sample (fully event-driven) |
| Background RAM | < 20 MB | 16 MB working set, 2.2 MB private |
| WebView2 at idle | none resident | 0 processes (UI is only spawned when you open the window) |
| Binary size | small | 4.23 MB |

The background process never spawns a WebView. The UI window (and its WebView2
process) is created only when you open it from the tray and is destroyed when
you close it, so idle memory stays tiny.

## How it works

Two planes that never block each other:

- **Hot plane (pure Rust, always resident):** a `WH_KEYBOARD_LL` global hook on
  its own thread forwards keystrokes to an engine thread. The engine maintains a
  rolling buffer, matches triggers, and performs expansion. No polling, no
  timers; threads sleep until a key arrives.
- **Management plane (SolidJS in WebView2, on demand):** the tray window for
  viewing, adding, and removing snippets, and toggling the engine on/off. It
  talks to the core over Tauri IPC and is never in the keystroke path.

## Project layout

```
Hypertype/
  index.html, vite.config.ts, tsconfig.json   frontend build
  src/                  SolidJS UI ("ui")
    App.tsx, main.tsx, styles.css
    lib/ipc.ts          typed wrapper over the Rust commands
  src-tauri/
    tauri.conf.json     no startup window; tray-only background app
    capabilities/       IPC permissions
    src/
      main.rs           entry: state, threads, tray, lifecycle
      keyboard.rs       WH_KEYBOARD_LL hook + message pump
      engine.rs         buffer, modifier tracking, match -> expand
      snippets.rs       HashMap store + suffix matcher (unit-tested)
      storage.rs        atomic JSON persistence ("storage")
      expansion/        inject.rs (SendInput), clipboard.rs, mod.rs
      platform.rs       foreground window, layout, password-field check
      ipc.rs            Tauri commands ("ipc")
      app_state.rs      shared state
      logging.rs        minimal file logger
      consts.rs         virtual-key codes
```

## Prerequisites

- Windows 10/11 with WebView2 (preinstalled on Windows 11).
- Rust with the **MSVC** toolchain. The repo pins it via `rust-toolchain.toml`;
  installing it once: `rustup toolchain install stable-x86_64-pc-windows-msvc`.
  MSVC also requires the Visual Studio Build Tools (the "Desktop development with
  C++" workload).
- Node.js and pnpm (`npm i -g pnpm`).

## Build and run

Install dependencies once:

```sh
pnpm install
```

Development (hot-reloading UI, debug core, console logs):

```sh
pnpm tauri dev
```

Optimized release binary plus both installers:

```sh
pnpm dist
```

- **Main installer (NSIS, per-user):**
  `src-tauri/target/release/bundle/nsis/HyperType_0.1.0_x64-setup.exe`
- **Alternative installer (HyperType's own):**
  `installer/target/release/HyperTypeSetup.exe` — a single Rust exe that
  embeds the release binary and shows a small dark setup form matching the
  app's design. Install copies per-user (no admin) to
  `%LOCALAPPDATA%\Programs\HyperType`, creates a Start Menu shortcut and an
  "Installed apps" uninstall entry, and launches the app. It also copies
  itself into the install directory as `uninstall.exe` and handles
  `--uninstall`.

Run the background process headless (the autostart shape, no window):

```sh
src-tauri/target/release/hypertype.exe --minimized
```

## Using it

1. Launch the app. A tray icon appears. Launched normally, it also opens the
   manager window.
2. Type a trigger followed by what it should expand into, or use the defaults.
3. In any other app, type a trigger at a word boundary. It expands instantly.

Default snippets: `gm`, `addr`, `sig`, `brb`, `omw`.

Tray menu: toggle **Enabled**, **Open HyperType**, **Quit**.

### Verifying expansion

Type a trigger **physically** on your keyboard. HyperType deliberately ignores
synthetic/injected keystrokes (the `LLKHF_INJECTED` flag), which is what stops it
from matching its own output. A side effect: automated `SendInput`/SendKeys test
scripts will not trigger expansion. Real typing does.

## Data and logs

- Snippets: `%APPDATA%\HyperType\snippets.json` (atomic temp-file + rename writes).
- Log: `%APPDATA%\HyperType\hypertype.log` (errors and critical events only).
- Set `HYPERTYPE_DEBUG=1` for verbose stderr output in a console build.

## Start with Windows

The autostart plugin is wired in. Enabling it registers a per-user run entry
(no admin) that launches `hypertype.exe --minimized` at login. A UI toggle for
this is a small follow-up; the backend support is present.

## Known MVP limitations

- Password-field detection covers native Win32 `ES_PASSWORD` controls. Browser
  and Electron password fields need UI Automation (post-MVP).
- IME composition is best-effort: expansion stays out of the way during
  composition rather than tracking committed text.
- Dead-key composition on some non-US layouts can be affected by character
  decoding in the hook. Latin layouts are unaffected.
- Expansions are typed out key by key by default (Keysmith-style keystroke
  replay: works in every app, no clipboard involved). Clipboard paste is
  selectable per the Insert Method setting for very long snippets, with the
  previous clipboard restored after the target app consumes the paste.
