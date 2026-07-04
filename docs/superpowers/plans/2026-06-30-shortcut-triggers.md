# Shortcut (hotkey) Triggers Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let a user record a real key combination (e.g. `Ctrl+Shift+V`) in the HyperType UI and have it paste a snippet anywhere in Windows, suppressing the combo's normal effect in whatever app is focused.

**Architecture:** Text triggers and the existing keyboard-hook engine (`engine.rs`) are untouched. Shortcut triggers are a parallel mechanism built on `tauri-plugin-global-shortcut` (which wraps Win32 `RegisterHotKey`), registered/unregistered directly from the IPC layer when snippets are added/removed, and from a startup loop. `snippets.json` moves from a flat `{trigger: expansion}` map to a list of `{trigger, expansion, kind}` entries, migrated transparently on first load.

**Tech Stack:** Rust (Tauri v2, `windows` crate, `tauri-plugin-global-shortcut` 2.x), SolidJS/TypeScript frontend, no new JS dependencies.

## Global Constraints

- Spec: `docs/superpowers/specs/2026-06-30-shortcut-triggers-design.md` — every requirement there must map to a task below.
- Shortcut firing must suppress the combo in the focused app (confirmed product decision — not "pass through + paste").
- Every recorded shortcut requires ≥1 modifier (Ctrl/Alt/Shift/Win) plus 1 non-modifier key; bare keys are rejected.
- Shortcuts respect the same master enabled/disabled toggle as text triggers; the password-field guard still applies.
- Storage migration from the old flat-map format must be transparent and lossless.
- No new npm dependency: the frontend never calls `tauri-plugin-global-shortcut`'s own JS bindings — it only sends the captured chord string to HyperType's own `add_snippet` command, which does the registration in Rust.
- API verified directly against `tauri-plugin-global-shortcut` v2.3.2 source (`github:tauri-apps/plugins-workspace`, `plugins/global-shortcut/src/lib.rs`) and `global-hotkey` v0.8.0 source (`github:tauri-apps/global-hotkey`, `src/hotkey.rs`) — the auto-generated docs had a stale/conflicting `on_shortcut` signature; the source is ground truth and is what every code sample below matches.

---

### Task 0: Initialize git

This project currently has no `.git` (confirmed: `git rev-parse --is-inside-work-tree` fails with "not a git repository"). The plan's commit steps need one.

**Files:** none (repo-level operation only)

- [ ] **Step 1: Initialize the repository and make a baseline commit**

```bash
cd "C:\Users\hi\desktop\Hypertype"
git init
git add -A
git commit -m "chore: baseline commit before shortcut-trigger feature"
```

Expected: `git log --oneline` shows one commit. If a `.gitignore` doesn't already exclude `node_modules/`, `dist/`, and `src-tauri/target/`, check `.gitignore` first — it should (the project already builds these directories).

---

### Task 1: Add the `tauri-plugin-global-shortcut` dependency

**Files:**
- Modify: `src-tauri/Cargo.toml:13-15`

**Interfaces:**
- Produces: the `tauri_plugin_global_shortcut` crate available to all later Rust tasks (`GlobalShortcutExt`, `Builder`, `ShortcutState`).

- [ ] **Step 1: Add the dependency**

In `src-tauri/Cargo.toml`, change:

```toml
tauri = { version = "2", features = ["tray-icon", "image-png"] }
tauri-plugin-autostart = "2"
tauri-plugin-single-instance = "2"
```

to:

```toml
tauri = { version = "2", features = ["tray-icon", "image-png"] }
tauri-plugin-autostart = "2"
tauri-plugin-global-shortcut = "2"
tauri-plugin-single-instance = "2"
```

- [ ] **Step 2: Verify it resolves and builds**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`
Expected: succeeds, pulling in `tauri-plugin-global-shortcut`, `global-hotkey`, and their transitive deps. No code uses the crate yet, so this just proves dependency resolution works.

- [ ] **Step 3: Commit**

```bash
cd "C:\Users\hi\desktop\Hypertype"
git add src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "build: add tauri-plugin-global-shortcut dependency"
```

---

### Task 2: Add `TriggerKind` to the snippet matcher

**Files:**
- Modify: `src-tauri/src/snippets.rs` (full rewrite, ~95 lines)

**Interfaces:**
- Consumes: nothing new.
- Produces: `pub enum TriggerKind { Text, Shortcut }` (Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, serialized as lowercase `"text"`/`"shortcut"`); `Snippets::from_entries(Vec<(String, String, TriggerKind)>) -> Self`; `Snippets::insert(&mut self, trigger: String, expansion: String, kind: TriggerKind)`; `Snippets::remove(&mut self, trigger: &str) -> Option<TriggerKind>`; `Snippets::get_kind(&self, trigger: &str) -> Option<TriggerKind>`; `Snippets::list(&self) -> Vec<(String, String, TriggerKind)>` (was `Vec<(String, String)>` — this is a breaking signature change consumed by Task 3 and Task 5). `Snippets::from_map` and `match_suffix` keep their existing signatures and behavior for `Text` entries; `match_suffix` now never returns a `Shortcut`-kind entry.

- [ ] **Step 1: Write the failing tests**

Add to the existing `#[cfg(test)] mod tests` block at the bottom of `src-tauri/src/snippets.rs` (after the existing `longest_trigger_wins` test):

```rust
    #[test]
    fn shortcut_kind_never_matches_as_text() {
        let mut s = Snippets::default();
        s.insert(
            "Ctrl+KeyG".to_string(),
            "secret".to_string(),
            TriggerKind::Shortcut,
        );
        let typed: Vec<char> = "Ctrl+KeyG".chars().collect();
        assert!(s.match_suffix(&typed).is_none());
    }

    #[test]
    fn list_reports_kind_per_entry() {
        let mut s = Snippets::default();
        s.insert("gm".to_string(), "Good morning".to_string(), TriggerKind::Text);
        s.insert(
            "Ctrl+KeyB".to_string(),
            "bold-snippet".to_string(),
            TriggerKind::Shortcut,
        );
        let mut entries = s.list();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(
            entries,
            vec![
                ("Ctrl+KeyB".to_string(), "bold-snippet".to_string(), TriggerKind::Shortcut),
                ("gm".to_string(), "Good morning".to_string(), TriggerKind::Text),
            ]
        );
    }

    #[test]
    fn remove_reports_the_removed_kind() {
        let mut s = Snippets::default();
        s.insert("gm".to_string(), "Good morning".to_string(), TriggerKind::Text);
        assert_eq!(s.remove("gm"), Some(TriggerKind::Text));
        assert_eq!(s.remove("gm"), None);
    }
```

- [ ] **Step 2: Run the tests to verify they fail to compile**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --quiet`
Expected: compile errors — `TriggerKind` doesn't exist yet, `insert`/`list` have the wrong arity.

- [ ] **Step 3: Rewrite `src-tauri/src/snippets.rs`**

Replace the entire file with:

```rust
//! Snippet store and the matcher.
//!
//! Lookup is a `HashMap<trigger, Entry>` (O(1) average). A snippet is one of
//! two kinds:
//! - `Text`: detected by suffix matching against the rolling input buffer —
//!   on each keystroke we test the buffer's trailing 1..=max_trigger_len
//!   characters, longest first, so the longest trigger wins. A trigger only
//!   fires when the character preceding it is a word boundary, which stops
//!   `gm` from firing inside `programming`.
//! - `Shortcut`: a key-chord string (e.g. "Ctrl+Shift+KeyV") registered as a
//!   real OS hotkey by `shortcuts.rs`. It is never matched here — `kind`
//!   filtering keeps a Shortcut entry from ever firing via typed text, and
//!   `max_len` only ever spans Text entries.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TriggerKind {
    Text,
    Shortcut,
}

#[derive(Clone)]
struct Entry {
    expansion: String,
    kind: TriggerKind,
}

#[derive(Default)]
pub struct Snippets {
    map: HashMap<String, Entry>,
    max_len: usize,
}

impl Snippets {
    pub fn from_entries(entries: Vec<(String, String, TriggerKind)>) -> Self {
        let mut map = HashMap::new();
        for (trigger, expansion, kind) in entries {
            map.insert(trigger, Entry { expansion, kind });
        }
        let mut s = Snippets { map, max_len: 0 };
        s.recompute();
        s
    }

    /// Convenience for callers that only ever deal in text triggers (the
    /// pre-shortcut-feature default snippets, and the storage migration path
    /// for the old flat-map file format).
    pub fn from_map(map: HashMap<String, String>) -> Self {
        Self::from_entries(
            map.into_iter()
                .map(|(trigger, expansion)| (trigger, expansion, TriggerKind::Text))
                .collect(),
        )
    }

    fn recompute(&mut self) {
        self.max_len = self
            .map
            .iter()
            .filter(|(_, e)| e.kind == TriggerKind::Text)
            .map(|(k, _)| k.chars().count())
            .max()
            .unwrap_or(0);
    }

    pub fn insert(&mut self, trigger: String, expansion: String, kind: TriggerKind) {
        self.map.insert(trigger, Entry { expansion, kind });
        self.recompute();
    }

    pub fn remove(&mut self, trigger: &str) -> Option<TriggerKind> {
        let removed = self.map.remove(trigger).map(|e| e.kind);
        self.recompute();
        removed
    }

    pub fn get_kind(&self, trigger: &str) -> Option<TriggerKind> {
        self.map.get(trigger).map(|e| e.kind)
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Snapshot sorted by trigger, for the UI list and for persistence.
    pub fn list(&self) -> Vec<(String, String, TriggerKind)> {
        let mut v: Vec<(String, String, TriggerKind)> = self
            .map
            .iter()
            .map(|(k, e)| (k.clone(), e.expansion.clone(), e.kind))
            .collect();
        v.sort_by(|a, b| a.0.cmp(&b.0));
        v
    }

    /// If a Text trigger ends the buffer at a word boundary, return its
    /// character length (how many chars to delete) and the expansion text.
    /// Shortcut-kind entries are never matched here.
    pub fn match_suffix(&self, buffer: &[char]) -> Option<(usize, String)> {
        if self.max_len == 0 || buffer.is_empty() {
            return None;
        }
        let max = self.max_len.min(buffer.len());
        for len in (1..=max).rev() {
            let start = buffer.len() - len;
            // The char before the trigger must be a boundary (or buffer start).
            if start > 0 && buffer[start - 1].is_alphanumeric() {
                continue;
            }
            let candidate: String = buffer[start..].iter().collect();
            if let Some(entry) = self.map.get(&candidate) {
                if entry.kind == TriggerKind::Text {
                    return Some((len, entry.expansion.clone()));
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Snippets {
        let mut m = HashMap::new();
        m.insert("gm".to_string(), "Good morning".to_string());
        m.insert("addr".to_string(), "123 Main Street".to_string());
        Snippets::from_map(m)
    }

    fn buf(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    #[test]
    fn matches_at_start() {
        let s = fixture();
        assert_eq!(s.match_suffix(&buf("gm")).unwrap().0, 2);
    }

    #[test]
    fn matches_after_space() {
        let s = fixture();
        let m = s.match_suffix(&buf("hello gm")).unwrap();
        assert_eq!(m.0, 2);
        assert_eq!(m.1, "Good morning");
    }

    #[test]
    fn does_not_match_inside_word() {
        let s = fixture();
        assert!(s.match_suffix(&buf("xgm")).is_none());
    }

    #[test]
    fn longest_trigger_wins() {
        let mut m = HashMap::new();
        m.insert("ad".to_string(), "short".to_string());
        m.insert("addr".to_string(), "long".to_string());
        let s = Snippets::from_map(m);
        assert_eq!(s.match_suffix(&buf("addr")).unwrap().1, "long");
    }

    #[test]
    fn shortcut_kind_never_matches_as_text() {
        let mut s = Snippets::default();
        s.insert(
            "Ctrl+KeyG".to_string(),
            "secret".to_string(),
            TriggerKind::Shortcut,
        );
        let typed: Vec<char> = "Ctrl+KeyG".chars().collect();
        assert!(s.match_suffix(&typed).is_none());
    }

    #[test]
    fn list_reports_kind_per_entry() {
        let mut s = Snippets::default();
        s.insert("gm".to_string(), "Good morning".to_string(), TriggerKind::Text);
        s.insert(
            "Ctrl+KeyB".to_string(),
            "bold-snippet".to_string(),
            TriggerKind::Shortcut,
        );
        let mut entries = s.list();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(
            entries,
            vec![
                ("Ctrl+KeyB".to_string(), "bold-snippet".to_string(), TriggerKind::Shortcut),
                ("gm".to_string(), "Good morning".to_string(), TriggerKind::Text),
            ]
        );
    }

    #[test]
    fn remove_reports_the_removed_kind() {
        let mut s = Snippets::default();
        s.insert("gm".to_string(), "Good morning".to_string(), TriggerKind::Text);
        assert_eq!(s.remove("gm"), Some(TriggerKind::Text));
        assert_eq!(s.remove("gm"), None);
    }
}
```

Note: this temporarily breaks the build — `storage.rs` and `ipc.rs` still call the old 2-arg `insert`/`to_map` API. That's expected; Task 3 and Task 5 fix them. Don't try to `cargo build` the whole workspace until then — just run the `snippets` unit tests in isolation:

- [ ] **Step 4: Run the snippets tests to verify they pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib snippets:: --quiet`
Expected: all 7 tests in `snippets::tests` pass (4 pre-existing + 3 new — `shortcut_kind_never_matches_as_text`, `list_reports_kind_per_entry`, `remove_reports_the_removed_kind`, and the pre-existing ones still compiling against the new API since `from_map`/`match_suffix` signatures didn't change).

This will actually fail to *build* at this point because `storage.rs`/`ipc.rs` reference the old API — that's fine, proceed straight to Task 3, then come back and run the full test suite at the end of Task 3.

- [ ] **Step 5: Commit**

```bash
cd "C:\Users\hi\desktop\Hypertype"
git add src-tauri/src/snippets.rs
git commit -m "feat: add TriggerKind to Snippets (text vs shortcut)"
```

(Commit even though the workspace doesn't fully build yet — Task 3 is the next step in the same logical change and lands within minutes. If you'd rather keep every commit green, squash Tasks 2+3 into one commit instead.)

---

### Task 3: Migrate snippet storage to the list format

**Files:**
- Modify: `src-tauri/src/storage.rs` (full rewrite, ~110 lines)
- Modify: `src-tauri/src/ipc.rs:90-95` (`persist` helper — fixed in Task 5, since `ipc.rs` needs broader changes anyway)

**Interfaces:**
- Consumes: `crate::snippets::{Snippets, TriggerKind}` from Task 2.
- Produces: `storage::save(path: &Path, snippets: &Snippets) -> std::io::Result<()>` (was `save(path, &HashMap<String,String>)` — breaking change, fixed in Task 5's `ipc.rs` rewrite). `storage::load_or_default` and `storage::data_file_path`/`data_dir`/`default_snippets` keep their existing signatures.

- [ ] **Step 1: Write the failing tests**

Add a `#[cfg(test)] mod tests` block at the bottom of `src-tauri/src/storage.rs`:

```rust
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
            vec![("gm".to_string(), "Good morning".to_string(), TriggerKind::Text)]
        );

        // Migration persists the new list format back to disk immediately.
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
    fn save_round_trips_through_load() {
        let path = temp_path("roundtrip");
        let snippets = Snippets::from_entries(vec![
            ("gm".to_string(), "Good morning".to_string(), TriggerKind::Text),
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
                ("Ctrl+Shift+KeyV".to_string(), "pasted".to_string(), TriggerKind::Shortcut),
                ("gm".to_string(), "Good morning".to_string(), TriggerKind::Text),
            ]
        );

        fs::remove_file(&path).ok();
    }
}
```

- [ ] **Step 2: Rewrite `src-tauri/src/storage.rs`**

Replace the entire file with:

```rust
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
        .map(|(trigger, expansion, kind)| StoredEntry { trigger, expansion, kind })
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
            vec![("gm".to_string(), "Good morning".to_string(), TriggerKind::Text)]
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
    fn save_round_trips_through_load() {
        let path = temp_path("roundtrip");
        let snippets = Snippets::from_entries(vec![
            ("gm".to_string(), "Good morning".to_string(), TriggerKind::Text),
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
                ("Ctrl+Shift+KeyV".to_string(), "pasted".to_string(), TriggerKind::Shortcut),
                ("gm".to_string(), "Good morning".to_string(), TriggerKind::Text),
            ]
        );

        fs::remove_file(&path).ok();
    }
}
```

- [ ] **Step 3: Fix the one remaining caller of the old `storage::save` signature**

`src-tauri/src/ipc.rs:90-95` still calls `storage::save(&state.data_path, &snapshot)` with a `HashMap` snapshot from the old `to_map()` method, which no longer exists. Task 5 rewrites all of `ipc.rs` anyway, but to get the workspace compiling *now* so the test suite can run, temporarily patch just the `persist` function:

```rust
fn persist(state: &Arc<AppState>) {
    let snippets = state.snippets.read().unwrap();
    if let Err(e) = storage::save(&state.data_path, &snippets) {
        crate::logging::error(&format!("failed to persist snippets: {e}"));
    }
}
```

This still leaves `ipc.rs`'s `get_snippets`/`add_snippet`/`remove_snippet` calling the old 2-arg `Snippets` API — that's fine, Task 5 finishes the job. The goal of this step is only to get `cargo build`/`cargo test` green again.

Also fix `SnippetView`'s construction in `get_snippets` (still using the old 2-tuple destructure) by changing:

```rust
.map(|(trigger, expansion)| SnippetView { trigger, expansion })
```

to:

```rust
.map(|(trigger, expansion, _kind)| SnippetView { trigger, expansion })
```

And fix `add_snippet`'s `snippets.insert(trigger, expansion);` call to:

```rust
snippets.insert(trigger, expansion, crate::snippets::TriggerKind::Text);
```

(Temporary — every snippet added through the UI is still treated as Text until Task 5 wires the real `kind` parameter through.)

- [ ] **Step 4: Run the full test suite**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --quiet`
Expected: `test result: ok. 21 passed; 0 failed` (15 pre-existing + 3 new `snippets` tests from Task 2 + 3 new `storage` tests from this task — adjust the exact count if it differs, but every test should pass and the crate should compile cleanly).

- [ ] **Step 5: Commit**

```bash
cd "C:\Users\hi\desktop\Hypertype"
git add src-tauri/src/storage.rs src-tauri/src/ipc.rs
git commit -m "feat: migrate snippet storage to a {trigger,expansion,kind} list format"
```

---

### Task 4: Build the shortcut-registration module and wire it into startup

**Files:**
- Create: `src-tauri/src/shortcuts.rs`
- Modify: `src-tauri/src/main.rs:4-13` (mod list), `:109-137` (builder chain and `setup`)

**Interfaces:**
- Consumes: `tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState}` (Task 1); `crate::snippets::TriggerKind`, `crate::app_state::AppState`, `crate::expansion::{expand, Mode}`, `crate::platform::is_password_field` (all pre-existing).
- Produces: `shortcuts::register_all(app: &AppHandle, state: &Arc<AppState>)`; `shortcuts::register_one(app: &AppHandle, state: &Arc<AppState>, trigger: &str, expansion: &str) -> Result<(), String>`; `shortcuts::unregister_one(app: &AppHandle, trigger: &str)`. These three are consumed by Task 5's `ipc.rs`.

- [ ] **Step 1: Create `src-tauri/src/shortcuts.rs`**

```rust
//! Global hotkey ("shortcut") triggers, layered on top of
//! `tauri-plugin-global-shortcut` (which wraps Win32 `RegisterHotKey`).
//! Unlike text triggers, a shortcut never touches the keyboard-hook engine:
//! `RegisterHotKey` makes the OS itself intercept the chord and suppress it
//! from the focused app, so registration and firing are handled entirely
//! here, independent of `engine.rs`.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::app_state::AppState;
use crate::expansion::{self, Mode};
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
    gs.on_shortcut(trigger, move |_app, _shortcut, event| {
        if event.state != ShortcutState::Pressed {
            return;
        }
        if !state.enabled.load(Ordering::Relaxed) {
            return;
        }
        if crate::platform::is_password_field() {
            return;
        }
        expansion::expand(0, &expansion, Mode::Clipboard);
        crate::logging::info("expanded shortcut trigger");
    })
    .map_err(|e| e.to_string())
}

/// Unregister a shortcut. Silently succeeds if it wasn't registered.
pub fn unregister_one(app: &AppHandle, trigger: &str) {
    let _ = app.global_shortcut().unregister(trigger);
}
```

- [ ] **Step 2: Wire the plugin and startup registration into `main.rs`**

In `src-tauri/src/main.rs`, add the module declaration. Change:

```rust
mod app_state;
mod consts;
mod engine;
mod expansion;
mod ipc;
mod keyboard;
mod logging;
mod platform;
mod snippets;
mod storage;
```

to:

```rust
mod app_state;
mod consts;
mod engine;
mod expansion;
mod ipc;
mod keyboard;
mod logging;
mod platform;
mod shortcuts;
mod snippets;
mod storage;
```

Then install the plugin and register shortcuts at startup. Change:

```rust
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .manage(state)
```

to:

```rust
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(state)
```

And change:

```rust
        .setup(move |app| {
            build_tray(app.handle(), setup_state.clone())?;
            // No window is created at startup (see tauri.conf.json windows: []).
            // Open one only for a manual launch; an autostart launch passes
            // --minimized and stays purely in the tray, so no WebView2 process
            // is ever spawned at idle.
            if !std::env::args().any(|a| a == "--minimized") {
                open_main_window(app.handle());
            }
            Ok(())
        })
```

to:

```rust
        .setup(move |app| {
            build_tray(app.handle(), setup_state.clone())?;
            shortcuts::register_all(app.handle(), &setup_state);
            // No window is created at startup (see tauri.conf.json windows: []).
            // Open one only for a manual launch; an autostart launch passes
            // --minimized and stays purely in the tray, so no WebView2 process
            // is ever spawned at idle.
            if !std::env::args().any(|a| a == "--minimized") {
                open_main_window(app.handle());
            }
            Ok(())
        })
```

- [ ] **Step 3: Verify it builds**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`
Expected: succeeds. (`ipc.rs` still has the Task-3 stopgap that treats every add as Text — that's fine, Task 5 finishes wiring `shortcuts::register_one`/`unregister_one` into the IPC commands.)

- [ ] **Step 4: Commit**

```bash
cd "C:\Users\hi\desktop\Hypertype"
git add src-tauri/src/shortcuts.rs src-tauri/src/main.rs
git commit -m "feat: register shortcut-kind snippets as OS hotkeys at startup"
```

---

### Task 5: Wire shortcut registration into the IPC layer

**Files:**
- Modify: `src-tauri/src/ipc.rs` (full rewrite, ~110 lines — supersedes the Task-3 stopgap edit)

**Interfaces:**
- Consumes: `shortcuts::{register_one, unregister_one}` (Task 4), `snippets::TriggerKind` (Task 2), `storage::save(path, &Snippets)` (Task 3).
- Produces: `SnippetView { trigger: String, expansion: String, kind: TriggerKind }` (was `{trigger, expansion}` — breaking change to the IPC wire shape, consumed by Task 6's frontend); `add_snippet(app, state, trigger, expansion, kind: TriggerKind) -> Result<(), String>` (was 3 args, no `app`/`kind` — breaking change to the command's call signature, consumed by Task 6); `remove_snippet(app, state, trigger) -> Result<(), String>` (gained the `app` param).

- [ ] **Step 1: Rewrite `src-tauri/src/ipc.rs`**

Replace the entire file with:

```rust
//! The command surface exposed to the UI. The UI reads/writes snippets and
//! flips the enabled flag. Text-kind snippets don't touch the keyboard path
//! beyond sharing `AppState`; Shortcut-kind snippets are registered as real
//! OS hotkeys via `shortcuts.rs` as part of the same add/remove call.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::app_state::AppState;
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
}

#[tauri::command]
pub fn get_status(state: State<Arc<AppState>>) -> Status {
    let count = state.snippets.read().unwrap().len();
    Status {
        enabled: state.enabled.load(Ordering::Relaxed),
        count,
        version: env!("CARGO_PKG_VERSION").to_string(),
    }
}

#[tauri::command]
pub fn get_snippets(state: State<Arc<AppState>>) -> Vec<SnippetView> {
    state
        .snippets
        .read()
        .unwrap()
        .list()
        .into_iter()
        .map(|(trigger, expansion, kind)| SnippetView { trigger, expansion, kind })
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
pub fn toggle_enabled(state: State<Arc<AppState>>) -> bool {
    let now = !state.enabled.load(Ordering::Relaxed);
    state.enabled.store(now, Ordering::Relaxed);
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
```

- [ ] **Step 2: Verify it builds and all tests pass**

Run: `cargo build --manifest-path src-tauri/Cargo.toml && cargo test --manifest-path src-tauri/Cargo.toml --quiet`
Expected: both succeed; same test count as the end of Task 3 (`ipc.rs` has no `#[cfg(test)]` of its own — IPC commands are exercised manually in Task 8, not unit-tested, since they need a live `AppHandle`).

- [ ] **Step 3: Commit**

```bash
cd "C:\Users\hi\desktop\Hypertype"
git add src-tauri/src/ipc.rs
git commit -m "feat: register/unregister OS hotkeys from add_snippet/remove_snippet"
```

---

### Task 6: Frontend types and chord-capture helpers

**Files:**
- Modify: `src/lib/ipc.ts` (full rewrite, ~26 lines)
- Create: `src/lib/shortcut.ts`

**Interfaces:**
- Consumes: nothing new (pure TS + the existing `@tauri-apps/api/core` `invoke`).
- Produces: `TriggerKind = "text" | "shortcut"`; `SnippetView { trigger, expansion, kind: TriggerKind }`; `api.addSnippet(trigger, expansion, kind: TriggerKind)`; `isModifierEvent(e: KeyboardEvent): boolean`; `chordFromEvent(e: KeyboardEvent): string | null`; `displayChord(chord: string): string`. All four consumed by Task 7's `App.tsx`.

- [ ] **Step 1: Rewrite `src/lib/ipc.ts`**

```typescript
import { invoke } from "@tauri-apps/api/core";

export type TriggerKind = "text" | "shortcut";

export interface SnippetView {
  trigger: string;
  expansion: string;
  kind: TriggerKind;
}

export interface Status {
  enabled: boolean;
  count: number;
  version: string;
}

// Thin typed wrapper over the Rust command surface. The frontend never
// touches the keyboard/engine path or the global-shortcut plugin directly;
// it only manages snippets and state, and Rust does all OS-level work.
export const api = {
  getStatus: () => invoke<Status>("get_status"),
  getSnippets: () => invoke<SnippetView[]>("get_snippets"),
  addSnippet: (trigger: string, expansion: string, kind: TriggerKind) =>
    invoke<void>("add_snippet", { trigger, expansion, kind }),
  removeSnippet: (trigger: string) => invoke<void>("remove_snippet", { trigger }),
  toggleEnabled: () => invoke<boolean>("toggle_enabled"),
  quit: () => invoke<void>("quit_app"),
};
```

- [ ] **Step 2: Create `src/lib/shortcut.ts`**

```typescript
// Chord capture and formatting for shortcut-kind snippets. The stored/wire
// format uses raw KeyboardEvent.code tokens (e.g. "Ctrl+Shift+KeyV") because
// that's exactly what the Rust side's chord parser (the global-hotkey
// crate's Code::from_str) accepts case-insensitively, with no translation
// needed. "Super" is the literal modifier token RegisterHotKey maps to the
// Windows key on Windows; it's only relabeled to "Win" for display.

const MODIFIER_CODES = new Set([
  "ControlLeft",
  "ControlRight",
  "AltLeft",
  "AltRight",
  "ShiftLeft",
  "ShiftRight",
  "MetaLeft",
  "MetaRight",
]);

export function isModifierEvent(e: KeyboardEvent): boolean {
  return MODIFIER_CODES.has(e.code);
}

/**
 * Builds the stored chord string from a keydown event, or null if the event
 * isn't a valid shortcut: a bare modifier press, or no modifier held at all
 * (every recorded shortcut requires at least one of Ctrl/Alt/Shift/Win).
 */
export function chordFromEvent(e: KeyboardEvent): string | null {
  if (isModifierEvent(e)) return null;
  const mods: string[] = [];
  if (e.ctrlKey) mods.push("Ctrl");
  if (e.altKey) mods.push("Alt");
  if (e.shiftKey) mods.push("Shift");
  if (e.metaKey) mods.push("Super");
  if (mods.length === 0) return null;
  return [...mods, e.code].join("+");
}

/** Friendlier label for the snippet-list badge, e.g. "Ctrl+Shift+KeyV" -> "Ctrl+Shift+V". */
export function displayChord(chord: string): string {
  return chord
    .split("+")
    .map((token) => {
      if (token === "Super") return "Win";
      if (token.startsWith("Key")) return token.slice(3);
      if (token.startsWith("Digit")) return token.slice(5);
      if (token.startsWith("Arrow")) return token.slice(5);
      return token;
    })
    .join("+");
}
```

- [ ] **Step 3: Verify the frontend still builds**

Run: `pnpm build`
Expected: succeeds (vite build → `dist/`). `App.tsx` doesn't import the new `kind` field or `shortcut.ts` yet, so TypeScript has nothing to complain about — this step just confirms `lib/ipc.ts`/`lib/shortcut.ts` are syntactically valid and type-check standalone.

- [ ] **Step 4: Commit**

```bash
cd "C:\Users\hi\desktop\Hypertype"
git add src/lib/ipc.ts src/lib/shortcut.ts
git commit -m "feat: add TriggerKind to the IPC types and a chord-capture helper"
```

---

### Task 7: Recorder UI and shortcut badges

**Files:**
- Modify: `src/App.tsx` (full rewrite, ~165 lines)
- Modify: `src/styles.css` (append ~30 lines)

**Interfaces:**
- Consumes: `TriggerKind`, `SnippetView`, `api` (Task 6's `lib/ipc.ts`); `isModifierEvent`, `chordFromEvent`, `displayChord` (Task 6's `lib/shortcut.ts`).
- Produces: the complete Add-snippet UI with a Text/Shortcut mode switch.

- [ ] **Step 1: Rewrite `src/App.tsx`**

```tsx
import { createResource, createSignal, For, Show } from "solid-js";
import { api, type SnippetView, type TriggerKind } from "./lib/ipc";
import { chordFromEvent, displayChord } from "./lib/shortcut";

export default function App() {
  const [status, { refetch: refetchStatus }] = createResource(api.getStatus);
  const [snippets, { refetch: refetchSnippets }] = createResource(api.getSnippets);
  const [mode, setMode] = createSignal<TriggerKind>("text");
  const [trigger, setTrigger] = createSignal("");
  const [expansion, setExpansion] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [recording, setRecording] = createSignal(false);
  const [error, setError] = createSignal("");

  const enabled = () => status()?.enabled ?? false;

  async function toggle() {
    setBusy(true);
    try {
      await api.toggleEnabled();
      await refetchStatus();
    } finally {
      setBusy(false);
    }
  }

  function switchMode(next: TriggerKind) {
    setMode(next);
    setTrigger("");
    setError("");
  }

  function startRecording() {
    if (recording()) return;
    setRecording(true);
    setTrigger("");
    setError("");

    const onKeyDown = (e: KeyboardEvent) => {
      e.preventDefault();
      if (e.code === "Escape") {
        stop();
        return;
      }
      const chord = chordFromEvent(e);
      if (chord) {
        setTrigger(chord);
        stop();
      }
    };
    const stop = () => {
      window.removeEventListener("keydown", onKeyDown, true);
      setRecording(false);
    };
    window.addEventListener("keydown", onKeyDown, true);
  }

  async function add(e: Event) {
    e.preventDefault();
    const t = trigger().trim();
    const x = expansion();
    if (!t || !x) return;
    setBusy(true);
    setError("");
    try {
      await api.addSnippet(t, x, mode());
      setTrigger("");
      setExpansion("");
      await refetchSnippets();
      await refetchStatus();
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  async function remove(t: string) {
    setBusy(true);
    try {
      await api.removeSnippet(t);
      await refetchSnippets();
      await refetchStatus();
    } finally {
      setBusy(false);
    }
  }

  return (
    <div class="app">
      <header class="topbar">
        <div class="brand">
          <span class="logo">H</span>
          <div class="brandtext">
            <h1>HyperType</h1>
            <p class="sub">v{status()?.version ?? "0.1.0"}</p>
          </div>
        </div>
        <button class="quit" onClick={() => api.quit()}>
          Quit
        </button>
      </header>

      <section class="statusrow">
        <div class="statuspill" classList={{ on: enabled(), off: !enabled() }}>
          <span class="dot" />
          {enabled() ? "Running" : "Disabled"}
        </div>
        <button class="toggle" disabled={busy()} onClick={toggle}>
          {enabled() ? "Disable" : "Enable"}
        </button>
        <span class="count">{status()?.count ?? 0} snippets</span>
      </section>

      <div class="moderow">
        <button
          type="button"
          class="modebtn"
          classList={{ active: mode() === "text" }}
          onClick={() => switchMode("text")}
        >
          Text
        </button>
        <button
          type="button"
          class="modebtn"
          classList={{ active: mode() === "shortcut" }}
          onClick={() => switchMode("shortcut")}
        >
          Shortcut
        </button>
      </div>

      <form class="addrow" onSubmit={add}>
        <Show
          when={mode() === "text"}
          fallback={
            <button
              type="button"
              class="in trig recorder"
              classList={{ listening: recording() }}
              onClick={startRecording}
            >
              {recording()
                ? "Press a key combination..."
                : trigger()
                  ? displayChord(trigger())
                  : "Record Shortcut"}
            </button>
          }
        >
          <input
            class="in trig"
            spellcheck={false}
            placeholder="trigger  (e.g. gm)"
            value={trigger()}
            onInput={(e) => setTrigger(e.currentTarget.value)}
          />
        </Show>
        <input
          class="in exp"
          spellcheck={false}
          placeholder="expansion  (e.g. Good morning)"
          value={expansion()}
          onInput={(e) => setExpansion(e.currentTarget.value)}
        />
        <button class="add" type="submit" disabled={busy()}>
          Add
        </button>
      </form>
      <Show when={error()}>
        <p class="formerror">{error()}</p>
      </Show>

      <ul class="list">
        <Show
          when={(snippets()?.length ?? 0) > 0}
          fallback={<li class="empty">No snippets yet. Add one above.</li>}
        >
          <For each={snippets()}>
            {(s: SnippetView) => (
              <li class="item">
                <code class="trigger" classList={{ shortcut: s.kind === "shortcut" }}>
                  {s.kind === "shortcut" ? displayChord(s.trigger) : s.trigger}
                </code>
                <span class="arrow">&#8594;</span>
                <span class="value" title={s.expansion}>
                  {s.expansion}
                </span>
                <button class="del" title="Delete" onClick={() => remove(s.trigger)}>
                  &#10005;
                </button>
              </li>
            )}
          </For>
        </Show>
      </ul>

      <footer class="foot">
        Type a trigger anywhere in Windows, or press a recorded shortcut, and it expands instantly.
      </footer>
    </div>
  );
}
```

- [ ] **Step 2: Append to `src/styles.css`**

Add at the end of the file (after the existing `.foot` rule):

```css

/* Mode switch */
.moderow {
  display: flex;
  gap: 6px;
}
.modebtn {
  background: var(--panel);
  color: var(--muted);
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 6px 14px;
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  transition: color 0.12s ease, border-color 0.12s ease, background 0.12s ease;
}
.modebtn:hover {
  color: var(--text);
}
.modebtn.active {
  background: var(--accent);
  color: #fff;
  border-color: var(--accent);
}

/* Shortcut recorder (reuses .in .trig sizing/font, see "Add row" above) */
.in.trig.recorder {
  text-align: left;
  cursor: pointer;
}
.in.trig.recorder.listening {
  border-color: var(--accent);
  color: var(--accent);
}

.trigger.shortcut {
  color: var(--on);
}

.formerror {
  color: var(--danger);
  font-size: 12px;
}
```

- [ ] **Step 3: Build and type-check**

Run: `pnpm build`
Expected: succeeds with no TypeScript errors.

- [ ] **Step 4: Commit**

```bash
cd "C:\Users\hi\desktop\Hypertype"
git add src/App.tsx src/styles.css
git commit -m "feat: add Text/Shortcut mode switch and live shortcut recorder to the UI"
```

---

### Task 8: Manual end-to-end verification

Unit tests cover chord parsing/formatting and storage migration, but OS-level hotkey suppression can only be confirmed by physically pressing a recorded combo — the same limitation today's text-trigger expansion has (the keyboard hook ignores synthetic/injected input by design, so this can't be scripted either). This task is for you, the user, not an automated step.

**Files:** none (verification only)

- [ ] **Step 1: Build and launch**

```bash
cd "C:\Users\hi\desktop\Hypertype"
cargo build --manifest-path src-tauri/Cargo.toml
pnpm tauri dev
```

- [ ] **Step 2: Record and test a shortcut**

In the HyperType window: click "Shortcut", click "Record Shortcut", press e.g. `Ctrl+Shift+0`, type some expansion text (e.g. "hello from a shortcut"), click Add. Open Notepad, click into it, press `Ctrl+Shift+0`.

Expected: the expansion text is pasted into Notepad, and nothing else happens (no Notepad menu/dialog triggered by that combo — confirms suppression).

- [ ] **Step 3: Test the conflict path**

Try recording a combo already reserved by Windows or another running app (e.g. `Ctrl+Alt+Delete` won't even be capturable by the browser layer, but something like a combo your browser/IDE already uses globally should trigger a registration failure).

Expected: an inline error appears under the Add form ("...already in use" or similar) and the snippet is not added to the list.

- [ ] **Step 4: Test disable + password guard + removal**

Click "Disable", press your recorded shortcut — expect nothing to happen. Click "Enable" again, confirm it pastes again. Delete the shortcut snippet from the list, press the old combo again — expect nothing to happen (confirms unregistration).

- [ ] **Step 5: Confirm the storage migration on a real file**

If you still have an old-format `snippets.json` from before this feature (flat `{trigger: expansion}` map) at `%APPDATA%\HyperType\snippets.json`, relaunch the app and check the file — it should now be the new `[{trigger, expansion, kind}, ...]` list shape, with all prior entries present and `"kind": "text"`.

- [ ] **Step 6: Report back**

Tell me what you saw at each step — particularly anything in Step 2/3 that *didn't* match expected, since suppression-vs-pass-through is the one behavior nothing in this plan can verify except a physical keypress.
