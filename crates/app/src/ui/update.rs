//! Updater UI state. S1 stubs `check` / `install`; S7b fills them.

use gpui_kit::*;

use crate::ui::studio::Studio;

/// Mirrors App.tsx updater signals (54-58).
#[derive(Clone, Default)]
pub struct UpdateUi {
    pub info: Option<UpdateInfo>,
    #[allow(dead_code)] // S7b checking spinner
    pub checking: bool,
    #[allow(dead_code)] // S7b status label
    pub status_text: String,
    #[allow(dead_code)] // S7b download progress
    pub downloading: bool,
    #[allow(dead_code)] // S7b download percent
    pub progress: u32,
}

#[derive(Clone, Debug)]
pub struct UpdateInfo {
    pub version: String,
    #[allow(dead_code)] // S7b release notes
    pub body: Option<String>,
}

/// Silent or manual update check. App.tsx:61-90.
/// S1 no-op stub; S7b talks to the updater backend.
pub fn check(studio: &Entity<Studio>, manual: bool, cx: &mut App) {
    let _ = (studio, manual, cx);
    // S7b: set checking, call updater, fill `studio.update`.
}

/// Download and install the available update. App.tsx:92-120.
/// S1 no-op stub; S7b fills.
#[allow(dead_code)] // S7b install path
pub fn install(studio: &Entity<Studio>, cx: &mut App) {
    let _ = (studio, cx);
}

/// S7b adds `update-available` here so `ui/parity.rs` stays untouched.
#[allow(dead_code)] // Dispatched from ui/parity; arms land in S7b
pub fn parity_state(
    _studio: &mut Studio,
    _name: &str,
    _window: &mut Window,
    _cx: &mut Context<Studio>,
) -> bool {
    false
}
