//! Shared application state. A single `Arc<AppState>` is handed to both the
//! Tauri command layer (the UI) and the engine thread (the keyboard path),
//! so the two planes never desync.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::RwLock;

use crate::snippets::Snippets;

pub struct AppState {
    /// All snippets, indexed for fast suffix matching. Read on every matched
    /// keystroke; written only on UI edits, so contention is effectively nil.
    pub snippets: RwLock<Snippets>,
    /// Master on/off switch for expansion (tray + UI toggle this).
    pub enabled: AtomicBool,
    /// Path to the persisted JSON store.
    pub data_path: PathBuf,
}
