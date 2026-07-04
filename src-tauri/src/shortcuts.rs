//! Global hotkey ("shortcut") triggers, layered on top of
//! `tauri-plugin-global-shortcut` (which wraps Win32 `RegisterHotKey`).
//! Unlike text triggers, a shortcut never touches the keyboard-hook engine:
//! `RegisterHotKey` makes the OS itself intercept the chord and suppress it
//! from the focused app, so registration and firing are handled entirely
//! here, independent of `engine.rs`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::app_state::AppState;
use crate::expansion;
use crate::snippets::TriggerKind;

/// Register every persisted Shortcut-kind snippet as an OS hotkey. Called
/// once at startup, after the plugin is installed, regardless of whether a
/// window is ever opened — shortcuts work the same way text triggers do.
pub fn register_all(app: &AppHandle, state: &Arc<AppState>) {
    let entries = state.snippets.read().unwrap().list();
    for (trigger, expansion, kind) in entries {
        if kind == TriggerKind::Shortcut {
            if let Err(e) = register_one(app, state, &trigger, &expansion) {
                crate::logging::error(&format!("failed to register shortcut: {e}"));
            }
        }
    }
}

/// Register (or re-register) a single shortcut. Unregistering first makes
/// this safe to call both for a brand new chord and for rebinding an
/// existing one to new expansion text.
pub fn register_one(
    app: &AppHandle,
    state: &Arc<AppState>,
    trigger: &str,
    expansion: &str,
) -> Result<(), String> {
    let gs = app.global_shortcut();
    let _ = gs.unregister(trigger);

    let state = state.clone();
    let expansion = expansion.to_string();
    // Holding the chord auto-repeats Pressed events; fire once per physical
    // press by ignoring repeats until the matching Released arrives.
    let held_down = AtomicBool::new(false);
    gs.on_shortcut(trigger, move |_app, _shortcut, event| {
        if event.state != ShortcutState::Pressed {
            held_down.store(false, Ordering::Relaxed);
            return;
        }
        if held_down.swap(true, Ordering::Relaxed) {
            return;
        }
        if !state.enabled.load(Ordering::Relaxed) {
            return;
        }
        if crate::platform::is_password_field() {
            return;
        }
        // Typed-out expansion takes visible time; run it off the main thread
        // so the event loop (tray, window) never stalls while it types.
        let expansion = expansion.clone();
        std::thread::spawn(move || {
            expansion::expand(0, &expansion);
            crate::logging::info("expanded shortcut trigger");
        });
    })
    .map_err(|e| e.to_string())
}

/// Unregister a shortcut. Silently succeeds if it wasn't registered.
pub fn unregister_one(app: &AppHandle, trigger: &str) {
    let _ = app.global_shortcut().unregister(trigger);
}
