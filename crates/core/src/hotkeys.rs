//! Global hotkey ("shortcut") triggers. Unlike text triggers, a shortcut
//! never touches the keyboard-hook engine: the OS (`RegisterHotKey`)
//! intercepts the chord and suppresses it from the focused app. The UI
//! adapter owns that registration through [`Hotkeys`] and calls [`fire`]
//! once per physical press; everything else lives here so every adapter
//! shares one policy.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::app_state::AppState;
use crate::snippets::TriggerKind;

/// OS hotkey registration, supplied by the UI adapter. `chord` is the stored
/// trigger string, e.g. `Ctrl+Shift+KeyV`.
pub trait Hotkeys {
    /// Register (or re-register) a chord. Errors when the OS refuses it,
    /// for example because another app already owns the chord.
    fn register(&self, chord: &str) -> Result<(), String>;
    /// Unregister a chord. Silently succeeds if it was not registered.
    fn unregister(&self, chord: &str);
}

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

fn shortcut_triggers(state: &AppState) -> Vec<String> {
    state
        .snippets
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .list()
        .into_iter()
        .filter(|(_, _, kind)| *kind == TriggerKind::Shortcut)
        .map(|(trigger, _, _)| trigger)
        .collect()
}

/// Register every persisted Shortcut-kind snippet as an OS hotkey. Called
/// once at startup, whether or not a window is ever opened.
pub fn register_all(hotkeys: &dyn Hotkeys, state: &AppState) {
    for trigger in shortcut_triggers(state) {
        if let Err(e) = hotkeys.register(&trigger) {
            crate::logging::error(&format!("failed to register shortcut: {e}"));
        }
    }
}

/// Unregister every persisted Shortcut-kind hotkey. Called when the engine is
/// paused: RegisterHotKey swallows the chord system-wide even if the handler
/// drops it, so a paused HyperType must hand the keys back to the focused app.
pub fn unregister_all(hotkeys: &dyn Hotkeys, state: &AppState) {
    for trigger in shortcut_triggers(state) {
        hotkeys.unregister(&trigger);
    }
}

/// Keep OS hotkey ownership in sync with the engine's enabled flag.
pub fn sync_enabled(hotkeys: &dyn Hotkeys, state: &AppState, enabled: bool) {
    if enabled {
        register_all(hotkeys, state);
    } else {
        unregister_all(hotkeys, state);
    }
}

/// Expand the shortcut snippet stored under `trigger`. The adapter calls this
/// once per physical press (it drops auto-repeat itself).
pub fn fire(state: &AppState, trigger: &str) {
    if !should_fire_shortcut() {
        return;
    }
    if !state.enabled.load(Ordering::Relaxed) {
        return;
    }
    if crate::platform::is_password_field() {
        return;
    }
    let found = state
        .snippets
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(trigger);
    let Some((expansion, TriggerKind::Shortcut)) = found else {
        return;
    };
    // Typed-out expansion takes visible time; run it off the caller's thread
    // so the UI event loop (tray, window) never stalls while it types.
    std::thread::spawn(move || {
        crate::expansion::expand(0, &expansion);
        crate::logging::info("expanded shortcut trigger");
    });
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

    // Indexed lookup: the previous `list()` cloned every trigger and every
    // expansion body on each Ctrl/Alt/Win keystroke.
    let matched = {
        let snippets = state
            .snippets
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        snippets.match_shortcut(&expected)
    };
    let Some((_trigger, expansion)) = matched else {
        return false;
    };

    if !should_fire_shortcut() {
        return true;
    }
    if !state.enabled.load(Ordering::Relaxed) {
        return true;
    }
    if crate::platform::is_password_field() {
        return true;
    }
    std::thread::spawn(move || {
        crate::expansion::expand(0, &expansion);
        crate::logging::info("expanded shortcut trigger (hook fallback)");
    });
    true
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
