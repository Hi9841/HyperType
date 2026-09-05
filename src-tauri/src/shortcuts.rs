//! Global hotkey ("shortcut") triggers, layered on top of
//! `tauri-plugin-global-shortcut` (which wraps Win32 `RegisterHotKey`).
//! Unlike text triggers, a shortcut never touches the keyboard-hook engine:
//! `RegisterHotKey` makes the OS itself intercept the chord and suppress it
//! from the focused app, so registration and firing are handled entirely
//! here, independent of `engine.rs`.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::app_state::AppState;
use crate::expansion;
use crate::snippets::TriggerKind;

static LAST_SHORTCUT_MS: AtomicU64 = AtomicU64::new(0);

/// Debounces shortcut triggers across RegisterHotKey and hook fallback.
pub fn should_fire_shortcut() -> bool {
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let prev = LAST_SHORTCUT_MS.swap(now_ms, Ordering::SeqCst);
    now_ms.saturating_sub(prev) > 400
}

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
        if !should_fire_shortcut() {
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

/// Attempts to match a key chord against registered shortcut snippets as a fallback
/// when Win32 RegisterHotKey cannot capture or encountered a collision.
pub fn try_match_chord_fallback(
    vk: u32,
    ctrl: bool,
    alt: bool,
    shift: bool,
    win: bool,
    state: &AppState,
) -> bool {
    if !ctrl && !alt && !win {
        return false;
    }
    let Some(key_name) = vk_to_key_name(vk) else {
        return false;
    };

    let entries = state.snippets.read().unwrap().list();
    for (trigger, expansion, kind) in entries {
        if kind != TriggerKind::Shortcut {
            continue;
        }
        let trig_lower = trigger.to_lowercase().replace(' ', "");
        let mut expected = String::new();
        if ctrl {
            expected.push_str("ctrl+");
        }
        if alt {
            expected.push_str("alt+");
        }
        if shift {
            expected.push_str("shift+");
        }
        if win {
            expected.push_str("super+");
        }
        expected.push_str(&key_name.to_lowercase());

        let matches = trig_lower == expected
            || (trig_lower.starts_with("control+")
                && trig_lower == expected.replace("ctrl+", "control+"));

        if matches {
            if !should_fire_shortcut() {
                return true;
            }
            if !state.enabled.load(Ordering::Relaxed) {
                return true;
            }
            if crate::platform::is_password_field() {
                return true;
            }
            let exp = expansion.clone();
            std::thread::spawn(move || {
                crate::expansion::expand(0, &exp);
                crate::logging::info("expanded shortcut trigger (hook fallback)");
            });
            return true;
        }
    }
    false
}

fn vk_to_key_name(vk: u32) -> Option<String> {
    match vk {
        0x41..=0x5A => {
            let letter = (b'A' + (vk as u8 - 0x41)) as char;
            Some(format!("Key{letter}"))
        }
        0x30..=0x39 => {
            let digit = (b'0' + (vk as u8 - 0x30)) as char;
            Some(format!("Digit{digit}"))
        }
        0x70..=0x7B => {
            let num = vk - 0x70 + 1;
            Some(format!("F{num}"))
        }
        0x20 => Some("Space".to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicBool;
    use std::sync::RwLock;
    use crate::snippets::Snippets;

    #[test]
    fn test_vk_to_key_name() {
        assert_eq!(vk_to_key_name(0x4C), Some("KeyL".to_string()));
        assert_eq!(vk_to_key_name(0x41), Some("KeyA".to_string()));
        assert_eq!(vk_to_key_name(0x30), Some("Digit0".to_string()));
        assert_eq!(vk_to_key_name(0x70), Some("F1".to_string()));
        assert_eq!(vk_to_key_name(0x7B), Some("F12".to_string()));
        assert_eq!(vk_to_key_name(0x20), Some("Space".to_string()));
        assert_eq!(vk_to_key_name(0xFF), None);
    }

    #[test]
    fn test_try_match_chord_fallback_matches_ctrl_key_l() {
        let store = Snippets::from_entries(vec![(
            "Ctrl+KeyL".to_string(),
            "yeet mode".to_string(),
            TriggerKind::Shortcut,
        )]);
        let state = AppState {
            snippets: RwLock::new(store),
            enabled: AtomicBool::new(true),
            data_path: PathBuf::new(),
        };

        // 0x4C is virtual key 'L'
        let matched = try_match_chord_fallback(0x4C, true, false, false, false, &state);
        assert!(matched);

        // Wrong modifier (Alt instead of Ctrl) should not match
        let no_match = try_match_chord_fallback(0x4C, false, true, false, false, &state);
        assert!(!no_match);
    }
}
