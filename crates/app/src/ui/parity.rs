//! Parity state dispatch. Each slice adds arms only in its own `parity_state`.

use gpui_kit::*;

use crate::ui::studio::Studio;
use crate::ui::titlebar;
use crate::ui::update;

/// Apply a named parity state. Returns true when a slice handled it.
#[cfg_attr(not(feature = "parity"), allow(dead_code))] // Wired only under --features parity
pub fn apply(
    name: &str,
    studio: &Entity<Studio>,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    studio.update(cx, |studio, cx| {
        if studio
            .sidebar
            .update(cx, |sidebar, cx| sidebar.parity_state(name, window, cx))
        {
            return true;
        }
        if studio
            .editor
            .update(cx, |editor, cx| editor.parity_state(name, window, cx))
        {
            return true;
        }
        if studio
            .settings
            .update(cx, |settings, cx| settings.parity_state(name, window, cx))
        {
            return true;
        }
        if studio
            .tokens
            .update(cx, |tokens, cx| tokens.parity_state(name, window, cx))
        {
            return true;
        }
        if titlebar::parity_state(studio, name, window, cx) {
            return true;
        }
        // S7b adds `update-available` in update.rs without editing this file.
        if update::parity_state(studio, name, window, cx) {
            return true;
        }
        // default-selected: Studio::new already selects the first snippet.
        if name == "default-selected" {
            if let Some(first) = studio.snippets.first().cloned() {
                studio.select_snippet(&first, window, cx);
            }
            return true;
        }
        // S1 F1/F2 proof surface: slider + value_edit.
        if name == "widget-lab" {
            #[cfg(feature = "parity")]
            {
                let lab = cx.new(|cx| crate::ui::widget_lab::WidgetLab::new(window, cx));
                studio.widget_lab = Some(lab);
                cx.notify();
                return true;
            }
            #[cfg(not(feature = "parity"))]
            {
                return false;
            }
        }
        false
    })
}
