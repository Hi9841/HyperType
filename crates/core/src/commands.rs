//! The command surface every UI shares. Each operation validates input,
//! mutates `AppState`, keeps OS hotkeys in step through [`Hotkeys`], and
//! persists. A UI adapter only maps its own events onto these calls, so the
//! rules live in one place.

use std::io::Read;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::{fs, path::Path};

use serde::Serialize;

use crate::app_state::AppState;
use crate::expansion::{self, InsertMode, PasteCombo};
use crate::hotkeys::{self, Hotkeys};
use crate::snippets::{TriggerKind, MAX_TEXT_TRIGGER_CHARS};
use crate::storage;

pub const MAX_EXPANSION_CHARS: usize = 1_000_000;
const MAX_IMPORT_BYTES: u64 = 16 * 1024 * 1024;
const MAX_SNIPPETS: usize = 10_000;
static PERSIST_LOCK: Mutex<()> = Mutex::new(());

pub fn validate_snippet(trigger: &str, expansion: &str, kind: TriggerKind) -> Result<(), String> {
    if trigger.is_empty() || expansion.is_empty() {
        return Err("Both trigger and expansion are required.".to_string());
    }
    if expansion.chars().count() > MAX_EXPANSION_CHARS {
        return Err(format!(
            "Expansion is too long (maximum {MAX_EXPANSION_CHARS} characters)."
        ));
    }
    if kind == TriggerKind::Text {
        if trigger.chars().count() > MAX_TEXT_TRIGGER_CHARS {
            return Err(format!(
                "Text trigger is too long (maximum {MAX_TEXT_TRIGGER_CHARS} characters)."
            ));
        }
        if trigger.chars().any(char::is_control) {
            return Err("Text triggers cannot contain control characters.".to_string());
        }
    }
    Ok(())
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct SnippetView {
    pub trigger: String,
    pub expansion: String,
    pub kind: TriggerKind,
}

#[derive(Serialize, Clone, Debug)]
pub struct Status {
    pub enabled: bool,
    pub count: usize,
    pub version: String,
    pub insert_mode: InsertMode,
    pub wpm: u32,
    pub paste_combo: PasteCombo,
    pub restore_delay_ms: u32,
    pub auto_paste_words: u32,
    pub hook_active: bool,
    pub hook_events: u64,
    pub hook_healthy: bool,
    pub is_elevated: bool,
}

#[derive(Serialize, Clone, Copy, Debug)]
pub struct ImportSummary {
    /// Brand-new triggers added during the import.
    pub imported: usize,
    /// Existing triggers whose expansion/kind were replaced.
    pub overwritten: usize,
    pub skipped: usize,
}

pub fn status(state: &AppState) -> Status {
    let count = state.snippets.read().unwrap().len();
    Status {
        enabled: state.enabled.load(Ordering::Relaxed),
        count,
        version: env!("CARGO_PKG_VERSION").to_string(),
        insert_mode: expansion::insert_mode(),
        wpm: expansion::wpm(),
        paste_combo: expansion::paste_combo(),
        restore_delay_ms: expansion::restore_delay_ms(),
        auto_paste_words: expansion::auto_paste_words(),
        hook_active: crate::keyboard::is_hook_installed(),
        hook_events: crate::keyboard::get_event_count(),
        hook_healthy: crate::keyboard::is_hook_healthy(),
        is_elevated: crate::platform::is_elevated(),
    }
}

/// Settings live as expansion-module atomics; persistence just reads them
/// back out. Returns the write error so the UI can surface it instead of
/// claiming a change was saved when only the in-memory atomics moved.
pub fn persist_settings(state: &AppState) -> Result<(), String> {
    let settings = storage::AppSettings {
        insert_mode: expansion::insert_mode(),
        wpm: expansion::wpm(),
        paste_combo: expansion::paste_combo(),
        restore_delay_ms: expansion::restore_delay_ms(),
        auto_paste_words: expansion::auto_paste_words(),
        enabled: state.enabled.load(Ordering::Relaxed),
    };
    storage::save_settings(&storage::settings_file_path(), &settings).map_err(|e| {
        let message = format!("failed to persist settings: {e}");
        crate::logging::error(&message);
        message
    })
}

pub fn set_insert_mode(state: &AppState, mode: InsertMode) -> Result<(), String> {
    expansion::set_insert_mode(mode);
    persist_settings(state)
}

pub fn set_wpm(state: &AppState, wpm: u32) -> Result<(), String> {
    expansion::set_wpm(wpm);
    persist_settings(state)
}

pub fn set_paste_combo(state: &AppState, combo: PasteCombo) -> Result<(), String> {
    expansion::set_paste_combo(combo);
    persist_settings(state)
}

pub fn set_restore_delay_ms(state: &AppState, delay_ms: u32) -> Result<(), String> {
    expansion::set_restore_delay_ms(delay_ms);
    persist_settings(state)
}

pub fn set_auto_paste_words(state: &AppState, words: u32) -> Result<(), String> {
    expansion::set_auto_paste_words(words);
    persist_settings(state)
}

pub fn snippets(state: &AppState) -> Vec<SnippetView> {
    state
        .snippets
        .read()
        .unwrap()
        .list()
        .into_iter()
        .map(|(trigger, expansion, kind)| SnippetView {
            trigger,
            expansion,
            kind,
        })
        .collect()
}

pub fn add_snippet(
    state: &AppState,
    hotkeys: &dyn Hotkeys,
    trigger: &str,
    expansion: String,
    kind: TriggerKind,
) -> Result<(), String> {
    let trigger = trigger.trim().to_string();
    validate_snippet(&trigger, &expansion, kind)?;

    {
        let snippets = state.snippets.read().unwrap();
        if snippets.get_kind(&trigger).is_some() {
            return Err("A snippet with that trigger already exists.".to_string());
        }
        if snippets.len() >= MAX_SNIPPETS {
            return Err(format!("Snippet limit reached ({MAX_SNIPPETS})."));
        }
    }

    if kind == TriggerKind::Shortcut {
        // Register with the OS first: if the chord is already taken, nothing
        // is saved and the UI can show why.
        hotkeys.register(&trigger)?;
    }

    {
        let mut snippets = state.snippets.write().unwrap();
        snippets.insert(trigger, expansion, kind);
    }
    persist(state)
}

pub fn edit_snippet(
    state: &AppState,
    hotkeys: &dyn Hotkeys,
    old_trigger: &str,
    trigger: &str,
    expansion: String,
    kind: TriggerKind,
) -> Result<(), String> {
    let old_trigger = old_trigger.trim().to_string();
    let trigger = trigger.trim().to_string();
    if old_trigger.is_empty() {
        return Err("Trigger and expansion are required.".to_string());
    }
    validate_snippet(&trigger, &expansion, kind)?;

    let old_kind = {
        let snippets = state.snippets.read().unwrap();
        let (_, current_kind) = snippets
            .get(&old_trigger)
            .ok_or_else(|| "Snippet not found.".to_string())?;
        if old_trigger != trigger && snippets.get(&trigger).is_some() {
            return Err("A snippet with that trigger already exists.".to_string());
        }
        current_kind
    };

    if kind == TriggerKind::Shortcut {
        if let Err(e) = hotkeys.register(&trigger) {
            if old_kind == TriggerKind::Shortcut && old_trigger == trigger {
                let _ = hotkeys.register(&old_trigger);
            }
            return Err(e);
        }
    }

    let update_result = {
        let mut snippets = state.snippets.write().unwrap();
        snippets.update(&old_trigger, trigger.clone(), expansion, kind)
    };

    if let Err(e) = update_result {
        if kind == TriggerKind::Shortcut {
            hotkeys.unregister(&trigger);
            if old_kind == TriggerKind::Shortcut {
                let _ = hotkeys.register(&old_trigger);
            }
        }
        return Err(e);
    }

    if old_kind == TriggerKind::Shortcut
        && (kind != TriggerKind::Shortcut || old_trigger != trigger)
    {
        hotkeys.unregister(&old_trigger);
    }

    persist(state)
}

pub fn remove_snippet(
    state: &AppState,
    hotkeys: &dyn Hotkeys,
    trigger: &str,
) -> Result<(), String> {
    let removed_kind = {
        let mut snippets = state.snippets.write().unwrap();
        snippets.remove(trigger)
    };
    if removed_kind == Some(TriggerKind::Shortcut) {
        hotkeys.unregister(trigger);
    }
    persist(state)
}

pub fn export_snippets(state: &AppState, path: &Path) -> Result<(), String> {
    let entries = state.snippets.read().unwrap().list();
    storage::save_entries(path, entries).map_err(|e| e.to_string())
}

pub fn import_snippets(
    state: &AppState,
    hotkeys: &dyn Hotkeys,
    path: &Path,
) -> Result<ImportSummary, String> {
    let mut text = String::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(MAX_IMPORT_BYTES + 1)
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    if text.len() as u64 > MAX_IMPORT_BYTES {
        return Err(format!(
            "Import file is too large (maximum {} MB).",
            MAX_IMPORT_BYTES / (1024 * 1024)
        ));
    }
    let imported = storage::parse_snippets(&text)?;
    let mut imported_count = 0usize;
    let mut overwritten_count = 0usize;
    let mut skipped = 0usize;
    let mut projected_count = state.snippets.read().unwrap().len();
    let mut accepted = Vec::with_capacity(imported.len());

    for (trigger, expansion, kind) in imported.list() {
        let trigger = trigger.trim().to_string();
        if validate_snippet(&trigger, &expansion, kind).is_err() {
            skipped += 1;
            continue;
        }

        let previous_kind = state.snippets.read().unwrap().get_kind(&trigger);

        let is_new = previous_kind.is_none();
        if is_new && projected_count >= MAX_SNIPPETS {
            skipped += 1;
            continue;
        }

        if kind == TriggerKind::Shortcut {
            if let Err(e) = hotkeys.register(&trigger) {
                if previous_kind == Some(TriggerKind::Shortcut) {
                    let _ = hotkeys.register(&trigger);
                }
                crate::logging::error(&format!("skipping imported shortcut {trigger}: {e}"));
                skipped += 1;
                continue;
            }
        } else if previous_kind == Some(TriggerKind::Shortcut) {
            hotkeys.unregister(&trigger);
        }

        if is_new {
            projected_count += 1;
            imported_count += 1;
        } else {
            overwritten_count += 1;
        }
        accepted.push((trigger, expansion, kind));
    }

    if !accepted.is_empty() {
        state.snippets.write().unwrap().insert_many(accepted);
        persist(state)?;
    }
    Ok(ImportSummary {
        imported: imported_count,
        overwritten: overwritten_count,
        skipped,
    })
}

pub fn reorder_snippets(state: &AppState, order: Vec<String>) -> Result<(), String> {
    {
        let mut snippets = state.snippets.write().unwrap();
        snippets.set_order(order);
    }
    persist(state)
}

/// Flip the engine on or off and return the new value. A paused engine must
/// release OS hotkeys; otherwise RegisterHotKey keeps swallowing those chords
/// from the focused app. The adapter keeps its tray checkmark in step.
pub fn toggle_enabled(state: &Arc<AppState>, hotkeys: &dyn Hotkeys) -> bool {
    let now = !state.enabled.fetch_xor(true, Ordering::Relaxed);
    hotkeys::sync_enabled(hotkeys, state, now);
    // The choice must survive a restart because the tray is the only UI
    // when the window is closed.
    if let Err(e) = persist_settings(state) {
        crate::logging::error(&format!("failed to persist enabled toggle: {e}"));
    }
    now
}

pub fn reinstall_hook() -> Result<bool, String> {
    if crate::keyboard::reinstall_hook() {
        Ok(true)
    } else {
        Err("Failed to reinstall keyboard hook".to_string())
    }
}

fn persist(state: &AppState) -> Result<(), String> {
    // Order snapshots and writes without holding the engine's snippet lock
    // during JSON serialization or disk I/O. The last mutation therefore
    // always produces the last on-disk snapshot.
    let _persist = PERSIST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let entries = state.snippets.read().unwrap().list();
    storage::save_entries(&state.data_path, entries).map_err(|e| {
        let message = format!("failed to persist snippets: {e}");
        crate::logging::error(&message);
        message
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unreachable_or_control_character_text_triggers() {
        let too_long = "x".repeat(MAX_TEXT_TRIGGER_CHARS + 1);
        assert!(validate_snippet(&too_long, "value", TriggerKind::Text).is_err());
        assert!(validate_snippet("line\nbreak", "value", TriggerKind::Text).is_err());
    }

    #[test]
    fn allows_unicode_text_trigger_at_character_limit() {
        let trigger = "\u{05d0}".repeat(MAX_TEXT_TRIGGER_CHARS);
        assert!(validate_snippet(&trigger, "value", TriggerKind::Text).is_ok());
    }

    #[test]
    fn caps_expansion_size() {
        let expansion = "x".repeat(MAX_EXPANSION_CHARS + 1);
        assert!(validate_snippet("ok", &expansion, TriggerKind::Text).is_err());
    }
}
