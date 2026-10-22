// Hide the console window in release; keep it in debug for logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod events;
mod hotkeys;
mod instance;
mod manager;
mod shell;
mod theme;
mod tray;

use gpui_kit::*;
use hypertype_core::logging;

use crate::hotkeys::GlobalHotkeys;
use crate::instance::Claim;
use crate::shell::Shell;
use crate::tray::Tray;

fn main() {
    logging::init();
    hypertype_core::install_panic_logger();

    let events = events::init();
    // Claim ownership before the engine starts, so a rejected second
    // process never installs a second keyboard hook.
    let _instance = match instance::claim() {
        Ok(Claim::Primary(guard)) => guard,
        Ok(Claim::Secondary) => return,
        Err(e) => {
            logging::error(&format!("single-instance claim failed: {e}"));
            return;
        }
    };
    let state = hypertype_core::start();
    let minimized = std::env::args().any(|a| a == "--minimized");

    application()
        .with_assets(assets::Assets)
        // The tray keeps HyperType alive; closing the window must not quit.
        .with_quit_mode(QuitMode::Explicit)
        .run(move |cx| {
            gpui_kit::init(cx);
            theme::init(cx);

            // The tray and hotkey windows must live on this thread's message loop.
            let hotkeys = match GlobalHotkeys::new(state.clone()) {
                Ok(hotkeys) => hotkeys,
                Err(e) => {
                    logging::error(&format!("global hotkeys unavailable: {e}"));
                    cx.quit();
                    return;
                }
            };
            hypertype_core::hotkeys::register_all(&hotkeys, &state);
            let tray = Tray::new(state.enabled.load(std::sync::atomic::Ordering::Relaxed))
                .inspect_err(|e| logging::error(&format!("tray unavailable: {e}")))
                .ok();
            Shell::install(state.clone(), hotkeys, tray, cx);

            cx.spawn(async move |cx| {
                while let Ok(event) = events.recv().await {
                    cx.update(|cx| shell::handle(event, cx));
                }
            })
            .detach();

            if !minimized {
                shell::open_window(cx);
            }
            logging::info("HyperType ready");
        });
}
