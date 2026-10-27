//! Snippet store and the matcher.
//!
//! Lookup is a `HashMap<trigger, Entry>` (O(1) average). A snippet is one of
//! two kinds:
//! - `Text`: detected by walking a reverse-character index from the end of
//!   the rolling input buffer. This avoids allocating candidate strings on
//!   every keystroke while still selecting the longest match. A trigger only
//!   fires when the character preceding it is a word boundary, which stops
//!   `gm` from firing inside `programming`.
//! - `Shortcut`: a key-chord string (e.g. "Ctrl+Shift+KeyV") registered as a
//!   real OS hotkey by `shortcuts.rs`. It is never matched here — `kind`
//!   filtering keeps a Shortcut entry from ever firing via typed text.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// The engine's rolling buffer is intentionally bounded. Enforce the same
/// limit at IPC boundaries so every accepted text trigger can actually fire.
pub const MAX_TEXT_TRIGGER_CHARS: usize = 64;

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
struct SuffixNode {
    children: HashMap<char, SuffixNode>,
    /// Trigger key in `Snippets::map`. Kept only at terminal nodes, so lookup
    /// remains allocation-free until an expansion is cloned for delivery.
    trigger: Option<String>,
}

#[derive(Default)]
pub struct Snippets {
    map: HashMap<String, Entry>,
    /// User-visible order of triggers. Matching never touches this; it only
    /// drives `list()` (the UI and the persisted file order).
    order: Vec<String>,
    suffixes: SuffixNode,
    /// Normalized chord -> trigger. Lets the hook fallback match a shortcut in
    /// O(1) without cloning the whole snippet list on every Ctrl/Alt/Win key.
    shortcut_index: HashMap<String, String>,
}

/// Canonical form for shortcut chords: lowercase, no spaces, `control` folded
/// into `ctrl`. Must stay in sync with how `hotkeys::try_match_chord_fallback`
/// builds its expected chord.
pub fn normalize_chord(trigger: &str) -> String {
    trigger
        .to_lowercase()
        .replace(' ', "")
        .replace("control+", "ctrl+")
}

impl Snippets {
    pub fn from_entries(entries: Vec<(String, String, TriggerKind)>) -> Self {
        let mut map = HashMap::new();
        let mut order = Vec::new();
        for (trigger, expansion, kind) in entries {
            if map
                .insert(trigger.clone(), Entry { expansion, kind })
                .is_none()
            {
                order.push(trigger);
            }
        }
        let mut s = Snippets {
            map,
            order,
            suffixes: SuffixNode::default(),
            shortcut_index: HashMap::new(),
        };
        s.rebuild_suffixes();
        s
    }

    /// Convenience for callers that only ever deal in text triggers (the
    /// pre-shortcut-feature default snippets, and the storage migration path
    /// for the old flat-map file format). Sorted so the resulting order is
    /// deterministic despite the map.
    pub fn from_map(map: HashMap<String, String>) -> Self {
        let mut entries: Vec<(String, String, TriggerKind)> = map
            .into_iter()
            .map(|(trigger, expansion)| (trigger, expansion, TriggerKind::Text))
            .collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        Self::from_entries(entries)
    }

    fn rebuild_suffixes(&mut self) {
        let mut root = SuffixNode::default();
        let mut shortcut_index = HashMap::new();
        for (trigger, entry) in &self.map {
            if entry.kind == TriggerKind::Text {
                let mut node = &mut root;
                for ch in trigger.chars().rev() {
                    node = node.children.entry(ch).or_default();
                }
                node.trigger = Some(trigger.clone());
            } else {
                shortcut_index.insert(normalize_chord(trigger), trigger.clone());
            }
        }
        self.suffixes = root;
        self.shortcut_index = shortcut_index;
    }

    fn upsert(&mut self, trigger: String, expansion: String, kind: TriggerKind) {
        if self
            .map
            .insert(trigger.clone(), Entry { expansion, kind })
            .is_none()
        {
            self.order.push(trigger);
        }
    }

    pub fn insert(&mut self, trigger: String, expansion: String, kind: TriggerKind) {
        // A brand-new trigger appends to the list; editing an existing one
        // keeps its position.
        self.upsert(trigger, expansion, kind);
        self.rebuild_suffixes();
    }

    /// Insert a batch while rebuilding the suffix index only once. Imports can
    /// contain thousands of entries; rebuilding after each one is quadratic.
    pub fn insert_many(&mut self, entries: Vec<(String, String, TriggerKind)>) {
        for (trigger, expansion, kind) in entries {
            self.upsert(trigger, expansion, kind);
        }
        self.rebuild_suffixes();
    }

    pub fn update(
        &mut self,
        old_trigger: &str,
        new_trigger: String,
        expansion: String,
        kind: TriggerKind,
    ) -> Result<TriggerKind, String> {
        if old_trigger != new_trigger && self.map.contains_key(&new_trigger) {
            return Err("A snippet with that trigger already exists.".to_string());
        }

        let old = self
            .map
            .remove(old_trigger)
            .ok_or_else(|| "Snippet not found.".to_string())?;
        let old_kind = old.kind;
        self.map
            .insert(new_trigger.clone(), Entry { expansion, kind });

        if let Some(slot) = self.order.iter_mut().find(|t| t.as_str() == old_trigger) {
            *slot = new_trigger;
        } else {
            self.order.push(new_trigger);
        }

        self.rebuild_suffixes();
        Ok(old_kind)
    }

    pub fn remove(&mut self, trigger: &str) -> Option<TriggerKind> {
        let removed = self.map.remove(trigger).map(|e| e.kind);
        if removed.is_some() {
            self.order.retain(|t| t != trigger);
        }
        self.rebuild_suffixes();
        removed
    }

    /// Apply a user-chosen order (drag-reorder in the UI). Unknown triggers
    /// are dropped, duplicates keep their first occurrence, and any snippet
    /// missing from `order` retains its old relative position at the end —
    /// so a stale or partial list can never lose snippets.
    pub fn set_order(&mut self, order: Vec<String>) {
        let mut seen = std::collections::HashSet::new();
        let mut next: Vec<String> = order
            .into_iter()
            .filter(|t| self.map.contains_key(t) && seen.insert(t.clone()))
            .collect();
        for t in &self.order {
            if !seen.contains(t) {
                next.push(t.clone());
            }
        }
        self.order = next;
    }

    pub fn get_kind(&self, trigger: &str) -> Option<TriggerKind> {
        self.map.get(trigger).map(|e| e.kind)
    }

    pub fn get(&self, trigger: &str) -> Option<(String, TriggerKind)> {
        self.map.get(trigger).map(|e| (e.expansion.clone(), e.kind))
    }

    /// Match a normalized shortcut chord. Only the matched trigger and
    /// expansion are cloned, so a chord keypress never allocates the whole
    /// snippet library.
    pub fn match_shortcut(&self, normalized: &str) -> Option<(String, String)> {
        let trigger = self.shortcut_index.get(normalized)?;
        let entry = self.map.get(trigger)?;
        Some((trigger.clone(), entry.expansion.clone()))
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Snapshot in user order, for the UI list and for persistence.
    pub fn list(&self) -> Vec<(String, String, TriggerKind)> {
        self.order
            .iter()
            .filter_map(|t| {
                self.map
                    .get(t)
                    .map(|e| (t.clone(), e.expansion.clone(), e.kind))
            })
            .collect()
    }

    /// If a Text trigger ends the buffer at a word boundary, return its
    /// character length (how many chars to delete) and the expansion text.
    /// Shortcut-kind entries are never matched here.
    pub fn match_suffix(&self, buffer: &[char]) -> Option<(usize, String)> {
        if buffer.is_empty() {
            return None;
        }

        let mut node = &self.suffixes;
        let mut matched = None;
        for (offset, ch) in buffer.iter().rev().enumerate() {
            let Some(next) = node.children.get(ch) else {
                break;
            };
            node = next;
            let len = offset + 1;
            let start = buffer.len() - len;
            if start == 0 || !buffer[start - 1].is_alphanumeric() {
                if let Some(entry) = node
                    .trigger
                    .as_deref()
                    .and_then(|trigger| self.map.get(trigger))
                {
                    matched = Some((len, entry.expansion.clone()));
                }
            }
        }
        matched
    }
}
