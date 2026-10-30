//! Application shell: owns the engine state, OS hotkeys, tray, and the
//! on-demand Studio window. The window is created when needed and removed
//! on close, which releases its renderer; the process stays in the tray.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use gpui_kit::*;
use hypertype_core::app_state::AppState;
use hypertype_core::commands;

use crate::events::AppEvent;
use crate::hotkeys::GlobalHotkeys;
use crate::tray::Tray;
use crate::ui::Studio;

pub struct Shell {
    pub state: Arc<AppState>,
    pub hotkeys: GlobalHotkeys,
    tray: Option<Tray>,
    window: Option<AnyWindowHandle>,
    studio: Option<Entity<Studio>>,
    _window_closed: Option<Subscription>,
}

impl Global for Shell {}

impl Shell {
    pub fn install(state: Arc<AppState>, hotkeys: GlobalHotkeys, tray: Option<Tray>, cx: &mut App) {
        let closed = cx.on_window_closed(|cx, closed_id| {
            let shell = cx.global_mut::<Shell>();
            if shell.window.is_some_and(|w| w.window_id() == closed_id) {
                shell.window = None;
                shell.studio = None;
            }
        });
        cx.set_global(Shell {
            state,
            hotkeys,
            tray,
            window: None,
            studio: None,
            _window_closed: Some(closed),
        });
    }

    pub fn enabled(&self) -> bool {
        self.state.enabled.load(Ordering::Relaxed)
    }

    pub fn window_handle_raw(&self) -> Option<AnyWindowHandle> {
        self.window
    }

    pub fn set_window(&mut self, handle: AnyWindowHandle, studio: Entity<Studio>) {
        self.window = Some(handle);
        self.studio = Some(studio);
    }

    #[cfg(feature = "parity")]
    pub fn window_handle(&self) -> Option<AnyWindowHandle> {
        self.window
    }

    #[cfg(feature = "parity")]
    pub fn studio(&self) -> Option<Entity<Studio>> {
        self.studio.clone()
    }
}

pub fn handle(event: AppEvent, cx: &mut App) {
    match event {
        AppEvent::OpenWindow => crate::ui::open_window(cx),
        AppEvent::ToggleEnabled => {
            toggle_enabled(cx);
        }
        AppEvent::Quit => cx.quit(),
    }
}

/// Flip the engine and keep the tray in step. Observers of [`Shell`] (the
/// Studio window) re-render through `observe_global`.
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
    crate::ui::open_window(cx);
}
