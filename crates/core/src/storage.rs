//! JSON persistence. A list of `{trigger, expansion, kind}` entries in
//! %APPDATA%\HyperType\snippets.json. Writes go through a temp file + rename
//! so a crash mid-write can never corrupt the live file. Missing file on
//! first run is seeded with a few example text snippets.
//!
//! Files written before the shortcut-trigger feature are a flat
//! `{trigger: expansion}` map of text triggers only. `load_or_default`
//! detects that old shape, converts it, and immediately persists the new
//! list format — a one-time, transparent migration.

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::expansion::{InsertMode, PasteCombo};
use crate::snippets::{Snippets, TriggerKind};

#[derive(Serialize, Deserialize)]
struct StoredEntry {
    trigger: String,
    expansion: String,
    kind: TriggerKind,
}

pub fn data_dir() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("HyperType")
}

pub fn data_file_path() -> PathBuf {
    data_dir().join("snippets.json")
}

pub fn settings_file_path() -> PathBuf {
    data_dir().join("settings.json")
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct AppSettings {
    /// How expansions are inserted (auto / paste / type).
    pub insert_mode: InsertMode,
    /// Typing speed for type-out insertion, words per minute.
    pub wpm: u32,
    /// The keystroke sent to make the target app paste.
    pub paste_combo: PasteCombo,
    /// How long the paste may take before the saved clipboard is restored.
    pub restore_delay_ms: u32,
    /// In Auto mode, expansions with more words than this threshold will paste;
    /// expansions with this many words or fewer will type out at configured WPM.
    pub auto_paste_words: u32,
    /// Whether the expansion engine is on. Persisted so a user who pauses
    /// HyperType does not silently get it re-enabled on the next launch.
    pub enabled: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            insert_mode: InsertMode::default(),
            wpm: 600,
            paste_combo: PasteCombo::default(),
            restore_delay_ms: 5_000,
            auto_paste_words: 15,
            enabled: true,
        }
    }
}

/// On-disk shape, every field optional: written by any past or future
/// version. `type_out` is the pre-InsertMode boolean; enum fields arrive as
/// raw JSON so one unknown value falls back alone instead of discarding the
/// whole file.
#[derive(Deserialize, Default)]
#[serde(default)]
struct StoredSettings {
    type_out: Option<bool>,
    insert_mode: Option<serde_json::Value>,
    wpm: Option<u32>,
    paste_combo: Option<serde_json::Value>,
    restore_delay_ms: Option<u32>,
    auto_paste_words: Option<u32>,
    enabled: Option<bool>,
}

/// Load settings; a missing or unreadable file means the defaults. A legacy
/// explicit `type_out` maps to Paste/Type (the user chose it); only a file
/// with neither field gets the new Auto default.
pub fn load_settings(path: &Path) -> AppSettings {
    let Some(stored) = fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<StoredSettings>(&text).ok())
    else {
        return AppSettings::default();
    };
    let defaults = AppSettings::default();
    let insert_mode = stored
        .insert_mode
        .and_then(|v| serde_json::from_value::<InsertMode>(v).ok())
        .unwrap_or(match stored.type_out {
            Some(true) => InsertMode::Type,
            Some(false) => InsertMode::Paste,
            None => defaults.insert_mode,
        });
    AppSettings {
        insert_mode,
        wpm: stored.wpm.unwrap_or(defaults.wpm),
        paste_combo: stored
            .paste_combo
            .and_then(|v| serde_json::from_value::<PasteCombo>(v).ok())
            .unwrap_or(defaults.paste_combo),
        restore_delay_ms: stored.restore_delay_ms.unwrap_or(defaults.restore_delay_ms),
        auto_paste_words: stored.auto_paste_words.unwrap_or(defaults.auto_paste_words),
        enabled: stored.enabled.unwrap_or(defaults.enabled),
    }
}

pub fn save_settings(path: &Path, settings: &AppSettings) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_string_pretty(settings).unwrap_or_else(|_| "{}".to_string());
    let tmp = path.with_extension("json.tmp");
    {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(json.as_bytes())?;
        file.flush()?;
    }
    fs::rename(&tmp, path)?;
    Ok(())
}

pub fn default_snippets() -> Snippets {
    let mut m = HashMap::new();
    m.insert("gm".to_string(), "Good morning".to_string());
    m.insert("addr".to_string(), "123 Main Street".to_string());
    m.insert("sig".to_string(), "Best regards, Dylan".to_string());
    m.insert("brb".to_string(), "be right back".to_string());
    m.insert("omw".to_string(), "on my way".to_string());
    Snippets::from_map(m)
}

pub fn load_or_default(path: &Path) -> Snippets {
    match fs::read_to_string(path) {
        Ok(text) => parse_or_migrate(path, &text),
        Err(_) => {
            // First run (or unreadable): seed defaults and persist them.
            let snippets = default_snippets();
            if let Err(e) = save(path, &snippets) {
                crate::logging::error(&format!("could not write initial snippets: {e}"));
            }
            snippets
        }
    }
}

pub fn parse_snippets(text: &str) -> Result<Snippets, String> {
    if let Ok(entries) = serde_json::from_str::<Vec<StoredEntry>>(text) {
        return Ok(Snippets::from_entries(
            entries
                .into_iter()
                .map(|e| (e.trigger, e.expansion, e.kind))
                .collect(),
        ));
    }
    if let Ok(map) = serde_json::from_str::<HashMap<String, String>>(text) {
        return Ok(Snippets::from_map(map));
    }
    Err("Import file is not a HyperType snippets export.".to_string())
}

fn parse_or_migrate(path: &Path, text: &str) -> Snippets {
    if let Ok(entries) = serde_json::from_str::<Vec<StoredEntry>>(text) {
        return Snippets::from_entries(
            entries
                .into_iter()
                .map(|e| (e.trigger, e.expansion, e.kind))
                .collect(),
        );
    }
    match serde_json::from_str::<HashMap<String, String>>(text) {
        Ok(map) => {
            let snippets = Snippets::from_map(map);
            if let Err(e) = save(path, &snippets) {
                crate::logging::error(&format!("could not persist migrated snippets: {e}"));
            }
            snippets
        }
        Err(e) => {
            crate::logging::error(&format!("snippets parse failed ({e}); using defaults"));
            default_snippets()
        }
    }
}

pub fn save(path: &Path, snippets: &Snippets) -> std::io::Result<()> {
    save_entries(path, snippets.list())
}

/// Persist an owned snapshot. Callers that share `Snippets` with the keyboard
/// engine can take this snapshot under the read lock, release it, and perform
/// JSON serialization and disk I/O without delaying keystroke matching.
pub fn save_entries(
    path: &Path,
    entries: Vec<(String, String, TriggerKind)>,
) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let entries: Vec<StoredEntry> = entries
        .into_iter()
        .map(|(trigger, expansion, kind)| StoredEntry {
            trigger,
            expansion,
            kind,
        })
        .collect();
    let json = serde_json::to_string_pretty(&entries).unwrap_or_else(|_| "[]".to_string());
    let tmp = path.with_extension("json.tmp");
    {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(json.as_bytes())?;
        file.flush()?;
    }
    // rename is atomic on the same volume and replaces the existing file on Windows.
    fs::rename(&tmp, path)?;
    Ok(())
}
