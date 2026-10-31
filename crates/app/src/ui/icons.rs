//! Spec icons from `src/components/Icons.tsx`. Paths resolve through [`crate::assets`].

use gpui_kit::component::Icon;
use gpui_kit::*;

fn icon(path: &'static str, size: f32) -> Icon {
    Icon::default()
        .path(path)
        .w(px(size))
        .h(px(size))
}

/// Icons.tsx IconSearch — 14×14
#[allow(dead_code)] // S2 search field
pub fn search() -> Icon {
    icon("icons/search.svg", 14.)
}
/// Icons.tsx IconSettings — 14×14
pub fn settings() -> Icon {
    icon("icons/settings.svg", 14.)
}
/// Icons.tsx IconPlus — 13×13
#[allow(dead_code)] // S2 new-snippet button
pub fn plus() -> Icon {
    icon("icons/plus.svg", 13.)
}
/// Icons.tsx IconClose — 12×12
#[allow(dead_code)] // S5/S6 modal close via widgets::modal_frame
pub fn close() -> Icon {
    icon("icons/close.svg", 12.)
}
/// Icons.tsx IconGrip — 12×12
#[allow(dead_code)] // S2 row drag handle
pub fn grip() -> Icon {
    icon("icons/grip.svg", 12.)
}
/// Icons.tsx IconDocument — 24×24
#[allow(dead_code)] // S2 empty state
pub fn document() -> Icon {
    icon("icons/document.svg", 24.)
}
/// Icons.tsx IconCheck — 13×13
#[allow(dead_code)] // S4 save success
pub fn check() -> Icon {
    icon("icons/check.svg", 13.)
}
/// Icons.tsx IconMinimize — 10×10
pub fn minimize() -> Icon {
    icon("icons/minimize.svg", 10.)
}
/// Icons.tsx IconCloseWindow — 10×10
pub fn close_window() -> Icon {
    icon("icons/close-window.svg", 10.)
}
/// Icons.tsx IconExport — 12×12
#[allow(dead_code)] // S3 export
pub fn export() -> Icon {
    icon("icons/export.svg", 12.)
}
/// Icons.tsx IconImport — 12×12
#[allow(dead_code)] // S3 import
pub fn import() -> Icon {
    icon("icons/import.svg", 12.)
}
/// Icons.tsx IconRefresh — 12×12
#[allow(dead_code)] // S5 repair / check-updates
pub fn refresh() -> Icon {
    icon("icons/refresh.svg", 12.)
}
/// Icons.tsx IconDownload — 12×12
pub fn download() -> Icon {
    icon("icons/download.svg", 12.)
}
