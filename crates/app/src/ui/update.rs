//! Updater UI state and glue to the native updater backend (`crate::updater`).
//! Mirrors App.tsx:53-120. Blocking updater calls run on the background
//! executor; results are applied on the foreground.

use std::time::Duration;

use gpui_kit::*;

use crate::ui::studio::Studio;
use crate::updater::{self, Progress};

/// Status texts that clear themselves (App.tsx:79, 85) stay this long.
const STATUS_CLEAR: Duration = Duration::from_millis(4000);

/// Mirrors App.tsx updater signals (54-58).
#[derive(Clone, Default)]
pub struct UpdateUi {
    pub info: Option<UpdateInfo>,
    pub checking: bool,
    pub status_text: String,
    pub downloading: bool,
    pub progress: u32,
    /// The backend release behind `info` (App.tsx `availableUpdateObj`).
    available: Option<updater::UpdateInfo>,
    /// Bumped on every status text change, so a pending clear timer only
    /// clears the text it was started for.
    status_gen: u64,
}

#[derive(Clone, Debug)]
pub struct UpdateInfo {
    pub version: String,
    #[allow(dead_code)] // Release notes; the spec UI does not show them.
    pub body: Option<String>,
}

impl UpdateUi {
    fn set_status(&mut self, text: impl Into<String>) -> u64 {
        self.status_text = text.into();
        self.status_gen += 1;
        self.status_gen
    }
}

/// Silent (`manual == false`, at window open) or manual update check.
/// App.tsx:61-90.
pub fn check(studio: &Entity<Studio>, manual: bool, cx: &mut App) {
    // Parity captures must not depend on the network.
    if cfg!(feature = "parity") && !manual {
        return;
    }
    let started = studio.update(cx, |studio, cx| {
        let ui = &mut studio.update;
        if ui.checking || ui.downloading {
            return false;
        }
        ui.checking = true;
        if manual {
            ui.set_status("Checking for updates...");
        }
        cx.notify();
        true
    });
    if !started {
        return;
    }

    let studio = studio.downgrade();
    cx.spawn(async move |cx| {
        let result = cx.background_executor().spawn(async { updater::check() }).await;
        let clear_gen = studio
            .update(cx, |studio, cx| {
                let ui = &mut studio.update;
                ui.checking = false;
                let clear_gen = match result {
                    Ok(Some(update)) => {
                        ui.info = Some(UpdateInfo {
                            version: update.version.clone(),
                            body: update.body.clone(),
                        });
                        ui.set_status(format!("v{} available", update.version));
                        ui.available = Some(update);
                        None
                    }
                    Ok(None) => {
                        ui.available = None;
                        ui.info = None;
                        manual.then(|| ui.set_status("HyperType is up to date"))
                    }
                    Err(_) => manual.then(|| ui.set_status("Update check failed")),
                };
                cx.notify();
                clear_gen
            })
            .ok()
            .flatten();
        if let Some(clear_gen) = clear_gen {
            clear_status_later(studio, clear_gen, cx).await;
        }
    })
    .detach();
}

/// Download, verify, and launch the available installer. App.tsx:92-120.
/// On success the backend starts the passive NSIS installer and ends this
/// process (like tauri-plugin-updater on Windows), so the "Restarting..."
/// texts of the spec are never visible there either.
pub fn install(studio: &Entity<Studio>, cx: &mut App) {
    let available = studio.update(cx, |studio, cx| {
        let ui = &mut studio.update;
        if ui.downloading {
            return None;
        }
        let available = ui.available.clone()?;
        ui.downloading = true;
        ui.progress = 0;
        cx.notify();
        Some(available)
    });
    let Some(available) = available else {
        return;
    };

    let studio = studio.downgrade();
    let (tx, rx) = async_channel::unbounded::<u32>();
    cx.spawn(async move |cx| {
        let download = cx.background_executor().spawn(async move {
            let mut total = 0u64;
            let mut downloaded = 0u64;
            updater::download_and_install(&available, |event| match event {
                Progress::Started { content_length } => total = content_length.unwrap_or(0),
                Progress::Chunk(len) => {
                    downloaded += len as u64;
                    if total > 0 {
                        let percent = (downloaded as f64 / total as f64 * 100.).round() as u32;
                        let _ = tx.send_blocking(percent);
                    }
                }
                Progress::Finished => {}
            })
        });

        // The sender drops when the download ends, which ends this loop.
        while let Ok(percent) = rx.recv().await {
            let _ = studio.update(cx, |studio, cx| {
                studio.update.progress = percent;
                cx.notify();
            });
        }

        // Only an error returns: success exits the process.
        if let Err(message) = download.await {
            let _ = studio.update(cx, |studio, cx| {
                let short: String = message.chars().take(45).collect();
                studio.update.set_status(format!("Update failed: {short}"));
                studio.update.downloading = false;
                cx.notify();
            });
        }
    })
    .detach();
}

async fn clear_status_later(studio: WeakEntity<Studio>, clear_gen: u64, cx: &mut AsyncApp) {
    cx.background_executor().timer(STATUS_CLEAR).await;
    let _ = studio.update(cx, |studio, cx| {
        if studio.update.status_gen == clear_gen {
            studio.update.status_text.clear();
            cx.notify();
        }
    });
}

/// `update-available`: the title-bar pill and the settings footer install
/// button, with a fixed fake release (no network).
pub fn parity_state(
    studio: &mut Studio,
    name: &str,
    _window: &mut Window,
    cx: &mut Context<Studio>,
) -> bool {
    if name != "update-available" {
        return false;
    }
    studio.update.info = Some(UpdateInfo {
        version: "1.1.9".into(),
        body: None,
    });
    studio.update.set_status("v1.1.9 available");
    cx.notify();
    true
}
