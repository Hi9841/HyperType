//! Parity judge entry. Built only with `--features parity`.
//!
//! Run: `hypertype --parity-state <name>` with `APPDATA` pointed at
//! `migration/parity/seed` (the runner sets this). Skips the instance mutex
//! and the keyboard hook. Opens the Studio window at once (960x640).
//!
//! Infinite animations are frozen at t=0 via `Studio.freeze_animations`
//! (see `ui/studio.rs` and `migration/parity/states.md`).

use gpui_kit::*;
use hypertype_core::logging;

use crate::hotkeys::GlobalHotkeys;
use crate::shell::{self, Shell};
use crate::ui;

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
            ui::init_theme(cx);

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

            ui::open_window(cx);
            apply(&state_name, cx);
            logging::info(&format!("parity ready: {state_name}"));
        });
}

fn apply(name: &str, cx: &mut App) {
    let shell = cx.global::<Shell>();
    let Some(handle) = shell.window_handle() else {
        logging::error("parity: studio window missing after open_window");
        return;
    };
    let Some(studio) = shell.studio() else {
        logging::error("parity: studio entity missing after open_window");
        return;
    };
    let state_name = name.to_string();
    let result = cx.update_window(handle, |_, window, cx| {
        if !ui::apply_parity(&state_name, &studio, window, cx) {
            logging::error(&format!(
                "parity: unhandled state '{state_name}' (add an arm in a slice parity_state)"
            ));
        }
    });
    if let Err(e) = result {
        logging::error(&format!("parity: failed to apply state '{name}': {e:?}"));
    }
}
