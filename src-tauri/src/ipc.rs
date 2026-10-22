//! Tauri command layer: maps each `invoke` onto `hypertype_core::commands`.

use std::path::PathBuf;
use std::sync::Arc;

use hypertype_core::app_state::AppState;
use hypertype_core::commands::{self, ImportSummary, SnippetView, Status};
use hypertype_core::expansion::{InsertMode, PasteCombo};
use hypertype_core::snippets::TriggerKind;
use tauri::{AppHandle, State};

use crate::shortcuts::TauriHotkeys;

#[tauri::command]
pub fn get_status(state: State<Arc<AppState>>) -> Status {
    commands::status(&state)
}

#[tauri::command]
pub fn restart_elevated() -> Result<(), String> {
    hypertype_core::platform::restart_elevated()
}

#[tauri::command]
pub fn set_insert_mode(state: State<Arc<AppState>>, mode: InsertMode) -> Result<(), String> {
    commands::set_insert_mode(&state, mode)
}

#[tauri::command]
pub fn set_wpm(state: State<Arc<AppState>>, wpm: u32) -> Result<(), String> {
    commands::set_wpm(&state, wpm)
}

#[tauri::command]
pub fn set_paste_combo(state: State<Arc<AppState>>, combo: PasteCombo) -> Result<(), String> {
    commands::set_paste_combo(&state, combo)
}

#[tauri::command]
pub fn set_restore_delay_ms(state: State<Arc<AppState>>, delay_ms: u32) -> Result<(), String> {
    commands::set_restore_delay_ms(&state, delay_ms)
}

#[tauri::command]
pub fn set_auto_paste_words(state: State<Arc<AppState>>, words: u32) -> Result<(), String> {
    commands::set_auto_paste_words(&state, words)
}

#[tauri::command]
pub fn get_snippets(state: State<Arc<AppState>>) -> Vec<SnippetView> {
    commands::snippets(&state)
}

#[tauri::command]
pub fn add_snippet(
    app: AppHandle,
    state: State<Arc<AppState>>,
    trigger: String,
    expansion: String,
    kind: TriggerKind,
) -> Result<(), String> {
    commands::add_snippet(&state, &TauriHotkeys(&app), &trigger, expansion, kind)
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
    commands::edit_snippet(
        &state,
        &TauriHotkeys(&app),
        &old_trigger,
        &trigger,
        expansion,
        kind,
    )
}

#[tauri::command]
pub fn remove_snippet(
    app: AppHandle,
    state: State<Arc<AppState>>,
    trigger: String,
) -> Result<(), String> {
    commands::remove_snippet(&state, &TauriHotkeys(&app), &trigger)
}

#[tauri::command]
pub fn export_snippets(state: State<Arc<AppState>>, path: String) -> Result<(), String> {
    commands::export_snippets(&state, &PathBuf::from(path))
}

#[tauri::command]
pub fn import_snippets(
    app: AppHandle,
    state: State<Arc<AppState>>,
    path: String,
) -> Result<ImportSummary, String> {
    commands::import_snippets(&state, &TauriHotkeys(&app), &PathBuf::from(path))
}

#[tauri::command]
pub fn reorder_snippets(state: State<Arc<AppState>>, order: Vec<String>) -> Result<(), String> {
    commands::reorder_snippets(&state, order)
}

#[tauri::command]
pub fn toggle_enabled(app: AppHandle, state: State<Arc<AppState>>) -> bool {
    let now = commands::toggle_enabled(&state, &TauriHotkeys(&app));
    crate::sync_tray_toggle(&app, now);
    now
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    crate::request_quit();
    app.exit(0);
}

#[tauri::command]
pub fn reinstall_hook() -> Result<bool, String> {
    commands::reinstall_hook()
}
