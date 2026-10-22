//! `Hotkeys` adapter over `tauri-plugin-global-shortcut` (which wraps Win32
//! `RegisterHotKey`). Firing policy lives in `hypertype_core::hotkeys`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use hypertype_core::app_state::AppState;
use hypertype_core::hotkeys::{self, Hotkeys};
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

pub struct TauriHotkeys<'a>(pub &'a AppHandle);

impl Hotkeys for TauriHotkeys<'_> {
    fn register(&self, chord: &str) -> Result<(), String> {
        let gs = self.0.global_shortcut();
        let _ = gs.unregister(chord);

        let state = self.0.state::<Arc<AppState>>().inner().clone();
        let trigger = chord.to_string();
        // Holding the chord auto-repeats Pressed events; fire once per physical
        // press by ignoring repeats until the matching Released arrives.
        let held_down = AtomicBool::new(false);
        gs.on_shortcut(chord, move |_app, _shortcut, event| {
            if event.state != ShortcutState::Pressed {
                held_down.store(false, Ordering::Relaxed);
                return;
            }
            if held_down.swap(true, Ordering::Relaxed) {
                return;
            }
            hotkeys::fire(&state, &trigger);
        })
        .map_err(|e| e.to_string())
    }

    fn unregister(&self, chord: &str) {
        let _ = self.0.global_shortcut().unregister(chord);
    }
}
