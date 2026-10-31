//! Studio UI: tokens, icons, widgets, Studio root, and slice stubs.

mod editor;
mod icons;
mod parity;
mod settings_modal;
mod sidebar;
mod studio;
mod titlebar;
mod tokens;
mod tokens_modal;
mod update;
mod widgets;
#[cfg(feature = "parity")]
pub mod widget_lab;

#[allow(unused_imports)]
pub use studio::{Modal, Studio};
#[allow(unused_imports)]
pub use update::UpdateUi;

use gpui_kit::component::theme::{Theme, ThemeMode};
use gpui_kit::component::TitleBar;
use gpui_kit::*;

use crate::shell::Shell;

/// Install Kit theme: dark mode, system fonts, caret INK, selection RING.
/// Also binds Studio-root Tab (rule 21a).
pub fn init_theme(cx: &mut App) {
    Theme::change(ThemeMode::Dark, None, cx);
    Theme::update(cx, |theme| {
        theme.font_family = tokens::FONT_UI.into();
        theme.mono_font_family = tokens::FONT_MONO.into();
        theme.font_size = px(tokens::ROOT_SIZE);
        theme.caret = tokens::ink();
        theme.selection = tokens::ring();
        theme.background = tokens::bg();
        theme.foreground = tokens::ink();
        theme.border = tokens::line();
    });
    studio::init_focus(cx);
}

/// Open (or activate) the Studio window.
pub fn open_window(cx: &mut App) {
    if let Some(handle) = cx.global::<Shell>().window_handle_raw() {
        let shown = handle.update(cx, |_, window, _| window.activate_window());
        if shown.is_ok() {
            return;
        }
    }

    let bounds = Bounds::centered(None, size(px(960.), px(640.)), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        window_min_size: Some(size(px(780.), px(520.))),
        app_id: Some("HyperType".into()),
        window_background: WindowBackgroundAppearance::Opaque,
        // Frameless: transparent titlebar, no OS caption buttons.
        titlebar: Some(TitlebarOptions {
            title: Some("HyperType".into()),
            appears_transparent: true,
            traffic_light_position: None,
        }),
        is_resizable: true,
        is_minimizable: true,
        ..TitleBar::window_options()
    };

    match gpui_kit::open_window(options, cx, |window, cx| {
        cx.new(|cx| Studio::new(window, cx))
    }) {
        Ok((handle, studio)) => {
            let shell = cx.global_mut::<Shell>();
            shell.set_window(handle, studio);
        }
        Err(e) => hypertype_core::logging::error(&format!("failed to open window: {e:?}")),
    }
}

#[cfg(feature = "parity")]
pub use parity::apply as apply_parity;
