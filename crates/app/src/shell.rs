//! Application shell: owns the engine state, OS hotkeys, tray, and the
//! on-demand manager window. The window is created when needed and removed
//! on close, which releases its renderer; the process stays in the tray.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use gpui_kit::component::TitleBar;
use gpui_kit::*;
use hypertype_core::app_state::AppState;
use hypertype_core::commands;

use crate::events::AppEvent;
use crate::hotkeys::GlobalHotkeys;
use crate::manager::Manager;
use crate::tray::Tray;

pub struct Shell {
    pub state: Arc<AppState>,
    pub hotkeys: GlobalHotkeys,
    tray: Option<Tray>,
    window: Option<AnyWindowHandle>,
    _window_closed: Option<Subscription>,
}

impl Global for Shell {}

impl Shell {
    pub fn install(state: Arc<AppState>, hotkeys: GlobalHotkeys, tray: Option<Tray>, cx: &mut App) {
        let closed = cx.on_window_closed(|cx, closed_id| {
            let shell = cx.global_mut::<Shell>();
            if shell.window.is_some_and(|w| w.window_id() == closed_id) {
                shell.window = None;
            }
        });
        cx.set_global(Shell {
            state,
            hotkeys,
            tray,
            window: None,
            _window_closed: Some(closed),
        });
    }

    pub fn enabled(&self) -> bool {
        self.state.enabled.load(Ordering::Relaxed)
    }
}

pub fn handle(event: AppEvent, cx: &mut App) {
    match event {
        AppEvent::OpenWindow => open_window(cx),
        AppEvent::ToggleEnabled => {
            toggle_enabled(cx);
        }
        AppEvent::Quit => cx.quit(),
    }
}

/// Flip the engine and keep the tray in step. Observers of [`Shell`] (the
/// manager window) re-render through `update_global`.
pub fn toggle_enabled(cx: &mut App) -> bool {
    cx.update_global::<Shell, _>(|shell, _| {
        let now = commands::toggle_enabled(&shell.state, &shell.hotkeys);
        if let Some(tray) = &shell.tray {
            tray.set_enabled(now);
        }
        now
    })
}

pub fn open_window(cx: &mut App) {
    if let Some(handle) = cx.global::<Shell>().window {
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
        titlebar: Some(TitlebarOptions {
            title: Some("HyperType".into()),
            ..TitleBar::title_bar_options()
        }),
        ..TitleBar::window_options()
    };
    match gpui_kit::open_window(options, cx, |window, cx| {
        cx.new(|cx| Manager::new(window, cx))
    }) {
        Ok((handle, _)) => {
            cx.global_mut::<Shell>().window = Some(handle);
        }
        Err(e) => hypertype_core::logging::error(&format!("failed to open window: {e:?}")),
    }
}
