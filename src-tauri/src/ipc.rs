//! The command surface exposed to the UI. The UI reads/writes snippets and
//! flips the enabled flag. Text-kind snippets don't touch the keyboard path
//! beyond sharing `AppState`; Shortcut-kind snippets are registered as real
//! OS hotkeys via `shortcuts.rs` as part of the same add/remove call.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::{fs, path::PathBuf};

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::app_state::AppState;
use crate::expansion::{self, InsertMode, PasteCombo};
use crate::shortcuts;
use crate::snippets::TriggerKind;
use crate::storage;

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
    if trigger.is_empty() || expansion.is_empty() {
        return Err("Both trigger and expansion are required.".to_string());
    }

    if kind == TriggerKind::Shortcut {
        // Register with the OS first: if the chord is already taken, nothing
        // is saved and the UI can show why.
        shortcuts::register_one(&app, state.inner(), &trigger, &expansion)?;
    } else {
        // If this exact trigger string was previously bound as a shortcut,
        // drop the stale OS-level registration before it becomes plain text.
        let previous_kind = state.snippets.read().unwrap().get_kind(&trigger);
        if previous_kind == Some(TriggerKind::Shortcut) {
            shortcuts::unregister_one(&app, &trigger);
        }
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
    if old_trigger.is_empty() || trigger.is_empty() || expansion.is_empty() {
        return Err("Trigger and expansion are required.".to_string());
    }

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
    let snippets = state.snippets.read().unwrap();
    storage::save(&path, &snippets).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn import_snippets(
    app: AppHandle,
    state: State<Arc<AppState>>,
    path: String,
) -> Result<ImportSummary, String> {
    let text = fs::read_to_string(PathBuf::from(path)).map_err(|e| e.to_string())?;
    let imported = storage::parse_snippets(&text)?;
    let mut imported_count = 0usize;
    let mut skipped = 0usize;

    for (trigger, expansion, kind) in imported.list() {
        let trigger = trigger.trim().to_string();
        if trigger.is_empty() || expansion.is_empty() {
            skipped += 1;
            continue;
        }

        let previous = {
            let snippets = state.snippets.read().unwrap();
            snippets.get(&trigger)
        };

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

        {
            let mut snippets = state.snippets.write().unwrap();
            snippets.insert(trigger, expansion, kind);
        }
        imported_count += 1;
    }

    if imported_count > 0 {
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
    let now = !state.enabled.load(Ordering::Relaxed);
    state.enabled.store(now, Ordering::Relaxed);
    crate::sync_tray_toggle(&app, now);
    now
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    crate::request_quit();
    app.exit(0);
}

fn persist(state: &Arc<AppState>) {
    let snippets = state.snippets.read().unwrap();
    if let Err(e) = storage::save(&state.data_path, &snippets) {
        crate::logging::error(&format!("failed to persist snippets: {e}"));
    }
}
