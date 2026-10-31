//! Asset sources: Kit icons, HyperType logo, and the 13 studio SVGs.

use std::borrow::Cow;
use std::sync::{Arc, OnceLock};

use gpui_kit::*;

/// Spec icons under `assets/icons/`. Kit Lucide icons stay available for
/// anything that still asks for them by path.
const STUDIO_ICONS: &[(&str, &[u8])] = &[
    (
        "icons/search.svg",
        include_bytes!("../assets/icons/search.svg"),
    ),
    (
        "icons/settings.svg",
        include_bytes!("../assets/icons/settings.svg"),
    ),
    (
        "icons/plus.svg",
        include_bytes!("../assets/icons/plus.svg"),
    ),
    (
        "icons/close.svg",
        include_bytes!("../assets/icons/close.svg"),
    ),
    (
        "icons/grip.svg",
        include_bytes!("../assets/icons/grip.svg"),
    ),
    (
        "icons/document.svg",
        include_bytes!("../assets/icons/document.svg"),
    ),
    (
        "icons/check.svg",
        include_bytes!("../assets/icons/check.svg"),
    ),
    (
        "icons/minimize.svg",
        include_bytes!("../assets/icons/minimize.svg"),
    ),
    (
        "icons/close-window.svg",
        include_bytes!("../assets/icons/close-window.svg"),
    ),
    (
        "icons/export.svg",
        include_bytes!("../assets/icons/export.svg"),
    ),
    (
        "icons/import.svg",
        include_bytes!("../assets/icons/import.svg"),
    ),
    (
        "icons/refresh.svg",
        include_bytes!("../assets/icons/refresh.svg"),
    ),
    (
        "icons/download.svg",
        include_bytes!("../assets/icons/download.svg"),
    ),
];

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some((_, bytes)) = STUDIO_ICONS.iter().find(|(p, _)| *p == path) {
            return Ok(Some(Cow::Borrowed(*bytes)));
        }
        gpui_kit::assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut paths = gpui_kit::assets::Assets.list(path)?;
        if path == "icons" || path.is_empty() {
            for (p, _) in STUDIO_ICONS {
                let name: SharedString = (*p).into();
                if !paths.iter().any(|existing| existing == &name) {
                    paths.push(name);
                }
            }
        }
        Ok(paths)
    }
}

/// Full 128×128 logo for the window / tray icon.
#[allow(dead_code)] // Window/tray icon consumers; brand mark uses logo_mark.
pub fn logo() -> Arc<Image> {
    static LOGO: OnceLock<Arc<Image>> = OnceLock::new();
    LOGO.get_or_init(|| {
        Arc::new(Image::from_bytes(
            ImageFormat::Png,
            include_bytes!("../assets/logo.png").to_vec(),
        ))
    })
    .clone()
}

/// Pre-scaled 18×18 brand mark (1x). Area (BOX) filter from the 128px source.
pub fn logo_18() -> Arc<Image> {
    static LOGO: OnceLock<Arc<Image>> = OnceLock::new();
    LOGO.get_or_init(|| {
        Arc::new(Image::from_bytes(
            ImageFormat::Png,
            include_bytes!("../assets/logo-18.png").to_vec(),
        ))
    })
    .clone()
}

/// Pre-scaled 23×23 brand mark (1.25x, 22.5 rounded). Area (BOX) filter.
pub fn logo_23() -> Arc<Image> {
    static LOGO: OnceLock<Arc<Image>> = OnceLock::new();
    LOGO.get_or_init(|| {
        Arc::new(Image::from_bytes(
            ImageFormat::Png,
            include_bytes!("../assets/logo-23.png").to_vec(),
        ))
    })
    .clone()
}

/// Pre-scaled 27×27 brand mark (1.5x). Area (BOX) filter from the 128px source.
pub fn logo_27() -> Arc<Image> {
    static LOGO: OnceLock<Arc<Image>> = OnceLock::new();
    LOGO.get_or_init(|| {
        Arc::new(Image::from_bytes(
            ImageFormat::Png,
            include_bytes!("../assets/logo-27.png").to_vec(),
        ))
    })
    .clone()
}

/// Pre-scaled 36×36 brand mark (2x). Area (BOX) filter from the 128px source.
pub fn logo_36() -> Arc<Image> {
    static LOGO: OnceLock<Arc<Image>> = OnceLock::new();
    LOGO.get_or_init(|| {
        Arc::new(Image::from_bytes(
            ImageFormat::Png,
            include_bytes!("../assets/logo-36.png").to_vec(),
        ))
    })
    .clone()
}

/// Brand mark at 1:1 device pixels for the window scale factor (RULEBOOK rule 15).
/// Picks the closest of 18 / 23 / 27 / 36.
pub fn logo_mark(scale_factor: f32) -> Arc<Image> {
    let target = 18.0 * scale_factor;
    let sizes = [18.0_f32, 23.0, 27.0, 36.0];
    let mut best = 0usize;
    let mut best_dist = (sizes[0] - target).abs();
    for (i, &s) in sizes.iter().enumerate().skip(1) {
        let d = (s - target).abs();
        if d < best_dist {
            best = i;
            best_dist = d;
        }
    }
    match best {
        0 => logo_18(),
        1 => logo_23(),
        2 => logo_27(),
        _ => logo_36(),
    }
}
