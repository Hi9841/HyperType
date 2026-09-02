//! The command surface exposed to the UI. The UI reads/writes snippets and
//! flips the enabled flag. Text-kind snippets don't touch the keyboard path
//! beyond sharing `AppState`; Shortcut-kind snippets are registered as real
//! OS hotkeys via `shortcuts.rs` as part of the same add/remove call.

use std::io::Read;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::{fs, path::PathBuf};

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::app_state::AppState;
use crate::expansion::{self, InsertMode, PasteCombo};
use crate::shortcuts;
use crate::snippets::{TriggerKind, MAX_TEXT_TRIGGER_CHARS};
use crate::storage;

const MAX_EXPANSION_CHARS: usize = 1_000_000;
const MAX_IMPORT_BYTES: u64 = 16 * 1024 * 1024;
const MAX_SNIPPETS: usize = 10_000;
static PERSIST_LOCK: Mutex<()> = Mutex::new(());

fn validate_snippet(trigger: &str, expansion: &str, kind: TriggerKind) -> Result<(), String> {
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

#[derive(Serialize)]
pub struct SnippetView {
    pub trigger: String,
    pub expansion: String,
    pub kind: TriggerKind,
}

#[derive(Serialize)]
pub struct Status {
    pub enabled: bool,
    pub count: usize,
    pub version: String,
    pub insert_mode: InsertMode,
    pub wpm: u32,
    pub paste_combo: PasteCombo,
    pub restore_delay_ms: u32,
    pub auto_paste_words: u32,
}

#[derive(Serialize)]
pub struct ImportSummary {
    pub imported: usize,
    pub skipped: usize,
}

#[tauri::command]
pub fn get_status(state: State<Arc<AppState>>) -> Status {
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
    }
}

/// Settings live as expansion-module atomics; persistence just reads them
/// back out.
fn persist_settings() {
    let settings = storage::AppSettings {
        insert_mode: expansion::insert_mode(),
        wpm: expansion::wpm(),
        paste_combo: expansion::paste_combo(),
        restore_delay_ms: expansion::restore_delay_ms(),
        auto_paste_words: expansion::auto_paste_words(),
    };
    if let Err(e) = storage::save_settings(&storage::settings_file_path(), &settings) {
        crate::logging::error(&format!("failed to persist settings: {e}"));
    }
}

#[tauri::command]
pub fn set_insert_mode(mode: InsertMode) {
    expansion::set_insert_mode(mode);
    persist_settings();
}

#[tauri::command]
pub fn set_wpm(wpm: u32) {
    expansion::set_wpm(wpm);
    persist_settings();
}

#[tauri::command]
pub fn set_paste_combo(combo: PasteCombo) {
    expansion::set_paste_combo(combo);
    persist_settings();
}

#[tauri::command]
pub fn set_restore_delay_ms(delay_ms: u32) {
    expansion::set_restore_delay_ms(delay_ms);
    persist_settings();
}

#[tauri::command]
pub fn set_auto_paste_words(words: u32) {
    expansion::set_auto_paste_words(words);
    persist_settings();
}

#[tauri::command]
pub fn get_snippets(state: State<Arc<AppState>>) -> Vec<SnippetView> {
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

#[tauri::command]
pub fn add_snippet(
    app: AppHandle,
    state: State<Arc<AppState>>,
    trigger: String,
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
        shortcuts::register_one(&app, state.inner(), &trigger, &expansion)?;
    }

    {
        let mut snippets = state.snippets.write().unwrap();
        snippets.insert(trigger, expansion, kind);
    }
    persist(state.inner());
    Ok(())
}

#[tauri::command]
pub fn edit_snippet(
    app: AppHandle,
    state: State<Arc<AppState>>,
    old_trigger: String,
    trigger: String,
    expansion: String,
    kind: TriggerKind,
) -> Result<(), String> {
    let old_trigger = old_trigger.trim().to_string();
    let trigger = trigger.trim().to_string();
    if old_trigger.is_empty() {
        return Err("Trigger and expansion are required.".to_string());
    }
    validate_snippet(&trigger, &expansion, kind)?;

    let (old_expansion, old_kind) = {
        let snippets = state.snippets.read().unwrap();
        let current = snippets
            .get(&old_trigger)
            .ok_or_else(|| "Snippet not found.".to_string())?;
        if old_trigger != trigger && snippets.get(&trigger).is_some() {
            return Err("A snippet with that trigger already exists.".to_string());
        }
        current
    };

    if kind == TriggerKind::Shortcut {
        if let Err(e) = shortcuts::register_one(&app, state.inner(), &trigger, &expansion) {
            if old_kind == TriggerKind::Shortcut && old_trigger == trigger {
                let _ = shortcuts::register_one(&app, state.inner(), &old_trigger, &old_expansion);
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
            shortcuts::unregister_one(&app, &trigger);
            if old_kind == TriggerKind::Shortcut {
                let _ = shortcuts::register_one(&app, state.inner(), &old_trigger, &old_expansion);
            }
        }
        return Err(e);
    }

    if old_kind == TriggerKind::Shortcut
        && (kind != TriggerKind::Shortcut || old_trigger != trigger)
    {
        shortcuts::unregister_one(&app, &old_trigger);
    }

    persist(state.inner());
    Ok(())
}

#[tauri::command]
pub fn remove_snippet(
    app: AppHandle,
    state: State<Arc<AppState>>,
    trigger: String,
) -> Result<(), String> {
    let removed_kind = {
        let mut snippets = state.snippets.write().unwrap();
        snippets.remove(&trigger)
    };
    if removed_kind == Some(TriggerKind::Shortcut) {
        shortcuts::unregister_one(&app, &trigger);
    }
    persist(state.inner());
    Ok(())
}

#[tauri::command]
pub fn export_snippets(state: State<Arc<AppState>>, path: String) -> Result<(), String> {
    let path = PathBuf::from(path);
    let entries = state.snippets.read().unwrap().list();
    storage::save_entries(&path, entries).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn import_snippets(
    app: AppHandle,
    state: State<Arc<AppState>>,
    path: String,
) -> Result<ImportSummary, String> {
    let path = PathBuf::from(path);
    let mut text = String::new();
    fs::File::open(&path)
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
    if imported.len() > MAX_SNIPPETS {
        return Err(format!(
            "Import contains too many snippets (maximum {MAX_SNIPPETS})."
        ));
    }
    let mut imported_count = 0usize;
    let mut skipped = 0usize;
    let mut projected_count = state.snippets.read().unwrap().len();
    let mut accepted = Vec::with_capacity(imported.len());

    for (trigger, expansion, kind) in imported.list() {
        let trigger = trigger.trim().to_string();
        if validate_snippet(&trigger, &expansion, kind).is_err() {
            skipped += 1;
            continue;
        }

        let previous = {
            let snippets = state.snippets.read().unwrap();
            snippets.get(&trigger)
        };

        let is_new = previous.is_none();
        if is_new && projected_count >= MAX_SNIPPETS {
            skipped += 1;
            continue;
        }

        if kind == TriggerKind::Shortcut {
            if let Err(e) = shortcuts::register_one(&app, state.inner(), &trigger, &expansion) {
                if let Some((old_expansion, TriggerKind::Shortcut)) = previous {
                    let _ = shortcuts::register_one(&app, state.inner(), &trigger, &old_expansion);
                }
                crate::logging::error(&format!("skipping imported shortcut {trigger}: {e}"));
                skipped += 1;
                continue;
            }
        } else if matches!(previous, Some((_, TriggerKind::Shortcut))) {
            shortcuts::unregister_one(&app, &trigger);
        }

        if is_new {
            projected_count += 1;
        }
        accepted.push((trigger, expansion, kind));
        imported_count += 1;
    }

    if imported_count > 0 {
        state.snippets.write().unwrap().insert_many(accepted);
        persist(state.inner());
    }
    Ok(ImportSummary {
        imported: imported_count,
        skipped,
    })
}

#[tauri::command]
pub fn reorder_snippets(state: State<Arc<AppState>>, order: Vec<String>) {
    {
        let mut snippets = state.snippets.write().unwrap();
        snippets.set_order(order);
    }
    persist(state.inner());
}

#[tauri::command]
pub fn toggle_enabled(app: AppHandle, state: State<Arc<AppState>>) -> bool {
    let now = !state.enabled.fetch_xor(true, Ordering::Relaxed);
    crate::sync_tray_toggle(&app, now);
    now
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    crate::request_quit();
    app.exit(0);
}

fn persist(state: &Arc<AppState>) {
    // Order snapshots and writes without holding the engine's snippet lock
    // during JSON serialization or disk I/O. The last mutation therefore
    // always produces the last on-disk snapshot.
    let _persist = PERSIST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let entries = state.snippets.read().unwrap().list();
    if let Err(e) = storage::save_entries(&state.data_path, entries) {
        crate::logging::error(&format!("failed to persist snippets: {e}"));
    }
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
