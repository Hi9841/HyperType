//! HyperType engine. Owns the always-resident hot plane: the low-level
//! keyboard hook, trigger matching, expansion, persistence, and the command
//! logic every UI shares. It has no UI dependency; a UI adapter supplies OS
//! hotkey registration through [`hotkeys::Hotkeys`] and owns windows, tray,
//! dialogs, autostart, and updates.

pub mod app_state;
pub mod commands;
pub mod consts;
pub mod engine;
pub mod expansion;
pub mod hotkeys;
pub mod keyboard;
pub mod logging;
pub mod platform;
pub mod snippets;
pub mod storage;

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, RwLock};

use app_state::AppState;

/// Load snippets and settings into an [`AppState`] without starting the
/// engine thread or the keyboard hook. Used by the parity judge so a second
/// process never claims the instance mutex or installs a second hook.
pub fn load_state_only() -> Arc<AppState> {
    let data_path = storage::data_file_path();
    let snippets = storage::load_or_default(&data_path);
    let settings = storage::load_settings(&storage::settings_file_path());
    expansion::set_insert_mode(settings.insert_mode);
    expansion::set_wpm(settings.wpm);
    expansion::set_paste_combo(settings.paste_combo);
    expansion::set_restore_delay_ms(settings.restore_delay_ms);
    expansion::set_auto_paste_words(settings.auto_paste_words);
    Arc::new(AppState {
        snippets: RwLock::new(snippets),
        enabled: AtomicBool::new(settings.enabled),
        data_path,
    })
}

/// Load storage and settings, then start the engine thread and keyboard hook.
/// Call once, after the process has claimed single-instance ownership, so a
/// rejected second process never installs a second hook.
pub fn start() -> Arc<AppState> {
    let state = load_state_only();

    // The engine and hook start before any window is shown: expansion runs
    // whether or not a window is ever opened.
    let tx = engine::start(state.clone());
    keyboard::start(tx);
    state
}

/// Log panics to the HyperType log file. Call after [`logging::init`].
pub fn install_panic_logger() {
    std::panic::set_hook(Box::new(|info| {
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_default();
        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "unknown".to_string()
        };
        logging::error(&format!("PANIC at {location}: {payload}"));
    }));
}
