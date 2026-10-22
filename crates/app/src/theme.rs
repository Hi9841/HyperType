//! HyperType visual system: Geist for interface text, JetBrains Mono for
//! triggers, chords, and tokens, and the neutral stack.menu palette in
//! `assets/theme.json`. Colors are defined only there; views read them from
//! `cx.theme()` by role.

use std::borrow::Cow;
use std::rc::Rc;

use gpui_kit::component::theme::{Theme, ThemeSet};
use gpui_kit::*;

const THEME: &str = include_str!("../assets/theme.json");

const FONTS: [&[u8]; 4] = [
    include_bytes!("../assets/fonts/Geist-Regular.ttf"),
    include_bytes!("../assets/fonts/Geist-Medium.ttf"),
    include_bytes!("../assets/fonts/Geist-SemiBold.ttf"),
    include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf"),
];

/// Register fonts and install the light and dark themes. Call after
/// `gpui_kit::init` and before any window measures text.
pub fn init(cx: &mut App) {
    if let Err(e) = cx
        .text_system()
        .add_fonts(FONTS.iter().map(|f| Cow::Borrowed(*f)).collect())
    {
        hypertype_core::logging::error(&format!("failed to load fonts: {e}"));
    }

    match serde_json::from_str::<ThemeSet>(THEME) {
        Ok(set) => Theme::update(cx, |theme| {
            for config in set.themes {
                theme.apply_config(&Rc::new(config));
            }
        }),
        Err(e) => hypertype_core::logging::error(&format!("invalid theme.json: {e}")),
    }
    Theme::sync_system_appearance(None, cx);
}

/// Follow the system light/dark setting while a window is open.
pub fn follow_system(window: &Window) -> Subscription {
    window.observe_window_appearance(|window, cx| {
        Theme::sync_system_appearance(Some(window), cx);
    })
}
