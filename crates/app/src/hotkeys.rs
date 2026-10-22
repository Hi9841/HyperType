//! `Hotkeys` adapter over `global-hotkey`, the crate that
//! `tauri-plugin-global-shortcut` wraps, so stored chords such as
//! `Ctrl+Shift+KeyV` parse the same way. Firing policy lives in
//! `hypertype_core::hotkeys`.

use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use hypertype_core::app_state::AppState;
use hypertype_core::hotkeys::{self, Hotkeys};

/// Owns the OS registrations. Create it on the GPUI main thread: the manager's
/// hidden window receives `WM_HOTKEY` through that thread's message loop.
pub struct GlobalHotkeys {
    manager: GlobalHotKeyManager,
    /// Hotkey id -> stored chord string.
    chords: Arc<Mutex<HashMap<u32, String>>>,
}

impl GlobalHotkeys {
    pub fn new(state: Arc<AppState>) -> Result<Self, String> {
        let manager = GlobalHotKeyManager::new().map_err(|e| e.to_string())?;
        let chords = Arc::new(Mutex::new(HashMap::<u32, String>::new()));
        // Holding a chord auto-repeats Pressed events; fire once per physical
        // press by ignoring repeats until the matching Released arrives.
        let held = Mutex::new(HashSet::<u32>::new());
        let lookup = chords.clone();
        GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
            let mut held = held.lock().unwrap_or_else(|p| p.into_inner());
            match event.state() {
                HotKeyState::Released => {
                    held.remove(&event.id());
                }
                HotKeyState::Pressed => {
                    if !held.insert(event.id()) {
                        return;
                    }
                    let chord = lookup
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .get(&event.id())
                        .cloned();
                    if let Some(chord) = chord {
                        hotkeys::fire(&state, &chord);
                    }
                }
            }
        }));
        Ok(Self { manager, chords })
    }
}

impl Hotkeys for GlobalHotkeys {
    fn register(&self, chord: &str) -> Result<(), String> {
        let hotkey = HotKey::from_str(chord).map_err(|e| e.to_string())?;
        let _ = self.manager.unregister(hotkey);
        self.manager.register(hotkey).map_err(|e| match e {
            global_hotkey::Error::AlreadyRegistered(_)
            | global_hotkey::Error::FailedToRegister(_) => {
                "Another app already uses this shortcut.".to_string()
            }
            other => other.to_string(),
        })?;
        self.chords
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(hotkey.id(), chord.to_string());
        Ok(())
    }

    fn unregister(&self, chord: &str) {
        if let Ok(hotkey) = HotKey::from_str(chord) {
            let _ = self.manager.unregister(hotkey);
            self.chords
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .remove(&hotkey.id());
        }
    }
}
