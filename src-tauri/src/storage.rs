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

#[derive(Serialize, Clone, Copy)]
pub struct AppSettings {
    /// How expansions are inserted (auto / paste / type).
    pub insert_mode: InsertMode,
    /// Typing speed for type-out insertion, words per minute.
    pub wpm: u32,
    /// The keystroke sent to make the target app paste.
    pub paste_combo: PasteCombo,
    /// How long the paste may take before the saved clipboard is restored.
    pub restore_delay_ms: u32,
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            insert_mode: InsertMode::default(),
            wpm: 600,
            paste_combo: PasteCombo::default(),
            restore_delay_ms: 5_000,
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
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let entries: Vec<StoredEntry> = snippets
        .list()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snippets::TriggerKind;

    fn temp_path(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("hypertype_test_{name}_{}.json", std::process::id()));
        p
    }

    #[test]
    fn migrates_old_flat_map_format() {
        let path = temp_path("migrate");
        fs::write(&path, r#"{"gm": "Good morning"}"#).unwrap();

        let snippets = load_or_default(&path);
        assert_eq!(snippets.len(), 1);
        assert_eq!(
            snippets.list(),
            vec![(
                "gm".to_string(),
                "Good morning".to_string(),
                TriggerKind::Text
            )]
        );

        let on_disk = fs::read_to_string(&path).unwrap();
        assert!(on_disk.contains("\"kind\""));
        assert!(serde_json::from_str::<Vec<StoredEntry>>(&on_disk).is_ok());

        fs::remove_file(&path).ok();
    }

    #[test]
    fn loads_new_list_format_with_shortcut_kind() {
        let path = temp_path("newformat");
        fs::write(
            &path,
            r#"[{"trigger":"Ctrl+KeyB","expansion":"bold-snippet","kind":"shortcut"}]"#,
        )
        .unwrap();

        let snippets = load_or_default(&path);
        assert_eq!(
            snippets.list(),
            vec![(
                "Ctrl+KeyB".to_string(),
                "bold-snippet".to_string(),
                TriggerKind::Shortcut
            )]
        );

        fs::remove_file(&path).ok();
    }

    #[test]
    fn settings_migrate_legacy_type_out_true() {
        let path = temp_path("settings_legacy_true");
        fs::write(&path, r#"{"type_out": true, "wpm": 900}"#).unwrap();
        let s = load_settings(&path);
        assert_eq!(s.insert_mode, InsertMode::Type);
        assert_eq!(s.wpm, 900);
        assert_eq!(s.paste_combo, PasteCombo::CtrlV);
        assert_eq!(s.restore_delay_ms, 5_000);
        fs::remove_file(&path).ok();
    }

    #[test]
    fn settings_migrate_legacy_type_out_false() {
        let path = temp_path("settings_legacy_false");
        fs::write(&path, r#"{"type_out": false, "wpm": 600}"#).unwrap();
        assert_eq!(load_settings(&path).insert_mode, InsertMode::Paste);
        fs::remove_file(&path).ok();
    }

    #[test]
    fn settings_missing_file_defaults_to_auto() {
        let s = load_settings(Path::new("Z:/nonexistent/settings.json"));
        assert_eq!(s.insert_mode, InsertMode::Auto);
        assert_eq!(s.wpm, 600);
        assert_eq!(s.restore_delay_ms, 5_000);
    }

    #[test]
    fn settings_unknown_enum_falls_back_without_losing_others() {
        let path = temp_path("settings_unknown_enum");
        fs::write(&path, r#"{"insert_mode": "telepathy", "wpm": 1200}"#).unwrap();
        let s = load_settings(&path);
        assert_eq!(s.insert_mode, InsertMode::Auto);
        assert_eq!(s.wpm, 1200);
        fs::remove_file(&path).ok();
    }

    #[test]
    fn settings_round_trip() {
        let path = temp_path("settings_roundtrip");
        let out = AppSettings {
            insert_mode: InsertMode::Paste,
            wpm: 700,
            paste_combo: PasteCombo::ShiftInsert,
            restore_delay_ms: 400,
        };
        save_settings(&path, &out).unwrap();
        let s = load_settings(&path);
        assert_eq!(s.insert_mode, InsertMode::Paste);
        assert_eq!(s.wpm, 700);
        assert_eq!(s.paste_combo, PasteCombo::ShiftInsert);
        assert_eq!(s.restore_delay_ms, 400);
        fs::remove_file(&path).ok();
    }

    #[test]
    fn save_round_trips_through_load() {
        let path = temp_path("roundtrip");
        let snippets = Snippets::from_entries(vec![
            (
                "gm".to_string(),
                "Good morning".to_string(),
                TriggerKind::Text,
            ),
            (
                "Ctrl+Shift+KeyV".to_string(),
                "pasted".to_string(),
                TriggerKind::Shortcut,
            ),
        ]);
        save(&path, &snippets).unwrap();

        let reloaded = load_or_default(&path);
        let mut entries = reloaded.list();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(
            entries,
            vec![
                (
                    "Ctrl+Shift+KeyV".to_string(),
                    "pasted".to_string(),
                    TriggerKind::Shortcut
                ),
                (
                    "gm".to_string(),
                    "Good morning".to_string(),
                    TriggerKind::Text
                ),
            ]
        );

        fs::remove_file(&path).ok();
    }
}
