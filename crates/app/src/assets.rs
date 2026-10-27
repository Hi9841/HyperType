//! Asset sources: the Kit component icons plus the few Lucide icons the
//! manager needs on top of them, and the HyperType logo.

use std::borrow::Cow;
use std::sync::{Arc, OnceLock};

use gpui_kit::assets::icon_assets;
use gpui_kit::*;

icon_assets!(ExtraIcons, [GripVertical, Keyboard, Download, Upload, Zap]);

/// Kit icons first, then the extra icons.
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        match gpui_kit::assets::Assets.load(path)? {
            Some(bytes) => Ok(Some(bytes)),
            None => ExtraIcons.load(path),
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut paths = gpui_kit::assets::Assets.list(path)?;
        paths.extend(ExtraIcons.list(path)?);
        Ok(paths)
    }
}

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
