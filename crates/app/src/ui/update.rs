//! Updater UI state. S1 stubs `check` / `install`; S7b fills them.

use gpui_kit::*;

use crate::ui::studio::Studio;

/// Mirrors App.tsx updater signals (54-58).
#[derive(Clone, Default)]
pub struct UpdateUi {
    pub info: Option<UpdateInfo>,
    pub checking: bool,
    pub status_text: String,
    pub downloading: bool,
    pub progress: u32,
}

#[derive(Clone, Debug)]
pub struct UpdateInfo {
    pub version: String,
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
pub fn install(studio: &Entity<Studio>, cx: &mut App) {
    let _ = (studio, cx);
}
