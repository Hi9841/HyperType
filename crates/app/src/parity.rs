//! Parity judge entry. Built only with `--features parity`.
//!
//! Run: `hypertype --parity-state <name>` with `APPDATA` pointed at
//! `migration/parity/seed` (the runner sets this). Skips the instance mutex
//! and the keyboard hook. Opens the manager window at once (960x640).
//!
//! ## Adding a state
//! Add a match arm in [`apply`]. Drive [`crate::manager::Manager`] with a few
//! lines. Keep the name identical to `migration/parity/states.md`.
//!
//! TODO(port): freeze infinite animations at the same frame as the browser
//! reference (`currentTime = 0` for the shortcut recorder listening pulse).
//! See `migration/parity/states.md` "Animation settle".

use gpui_kit::*;
use hypertype_core::logging;

use crate::hotkeys::GlobalHotkeys;
use crate::shell::{self, Shell};

/// Parse `--parity-state <name>` from argv. Returns `None` when absent.
pub fn cli_state() -> Option<String> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--parity-state" {
            return args.next();
        }
    }
    None
}

/// Parity process entry. Never claims the instance mutex. Never starts the
/// engine or keyboard hook.
pub fn run(state_name: String) {
    logging::info(&format!("parity mode: state={state_name}"));
    let events = crate::events::init();
    let state = hypertype_core::load_state_only();

    application()
        .with_assets(crate::assets::Assets)
        .with_quit_mode(QuitMode::Explicit)
        .run(move |cx| {
            gpui_kit::init(cx);
            crate::theme::init(cx);
            crate::manager::bind_keys(cx);

            let hotkeys = match GlobalHotkeys::new(state.clone()) {
                Ok(hotkeys) => hotkeys,
                Err(e) => {
                    logging::error(&format!("parity: global hotkeys unavailable: {e}"));
                    cx.quit();
                    return;
                }
            };
            // Do not register_all: no OS chords while judging pixels.
            Shell::install(state.clone(), hotkeys, None, cx);

            cx.spawn(async move |cx| {
                while let Ok(event) = events.recv().await {
                    cx.update(|cx| shell::handle(event, cx));
                }
            })
            .detach();

            shell::open_window(cx);
            apply(&state_name, cx);
            logging::info(&format!("parity ready: {state_name}"));
        });
}

fn apply(name: &str, cx: &mut App) {
    let shell = cx.global::<Shell>();
    let Some(handle) = shell.window_handle() else {
        logging::error("parity: manager window missing after open_window");
        return;
    };
    let Some(manager) = shell.manager() else {
        logging::error("parity: manager entity missing after open_window");
        return;
    };
    let state_name = name.to_string();
    let result = cx.update_window(handle, |_, window, cx| {
        manager.update(cx, |manager, cx| {
            manager.apply_parity_state(&state_name, window, cx);
        });
    });
    if let Err(e) = result {
        logging::error(&format!("parity: failed to apply state '{name}': {e:?}"));
    }
}
