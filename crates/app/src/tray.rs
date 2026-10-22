//! Notification-area icon and menu: left click opens the manager; the menu
//! has Enabled (check), Open HyperType, and Quit, as in v1.x.

use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::events::{self, AppEvent};

/// Resource ordinal of the app icon in `assets/hypertype.rc`.
const ICON_RESOURCE: u16 = 1;

pub struct Tray {
    _icon: TrayIcon,
    enabled: CheckMenuItem,
}

impl Tray {
    /// Build on the GPUI main thread; the icon's window lives on its loop.
    pub fn new(enabled: bool) -> Result<Self, String> {
        let toggle = CheckMenuItem::with_id("toggle", "Enabled", true, enabled, None);
        let open = MenuItem::with_id("open", "Open HyperType", true, None);
        let quit = MenuItem::with_id("quit", "Quit", true, None);
        let menu = Menu::new();
        menu.append_items(&[&toggle, &PredefinedMenuItem::separator(), &open, &quit])
            .map_err(|e| e.to_string())?;

        MenuEvent::set_event_handler(Some(|event: MenuEvent| match event.id.0.as_str() {
            "toggle" => events::send(AppEvent::ToggleEnabled),
            "open" => events::send(AppEvent::OpenWindow),
            "quit" => events::send(AppEvent::Quit),
            _ => {}
        }));
        TrayIconEvent::set_event_handler(Some(|event: TrayIconEvent| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                events::send(AppEvent::OpenWindow);
            }
        }));

        let icon = Icon::from_resource(ICON_RESOURCE, None).map_err(|e| e.to_string())?;
        let tray = TrayIconBuilder::new()
            .with_id("main")
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .with_tooltip("HyperType")
            .with_icon(icon)
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            _icon: tray,
            enabled: toggle,
        })
    }

    /// Keep the menu checkmark in step when the engine is toggled elsewhere.
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.set_checked(enabled);
    }
}
