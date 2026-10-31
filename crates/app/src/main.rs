// Hide the console window in release; keep it in debug for logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod assets;
mod autostart;
mod chord;
mod events;
mod hotkeys;
mod instance;
#[cfg(feature = "parity")]
mod parity;
mod shell;
mod tray;
mod ui;
mod updater;

use gpui_kit::*;
use hypertype_core::logging;

use crate::hotkeys::GlobalHotkeys;
use crate::instance::Claim;
use crate::shell::Shell;
use crate::tray::Tray;

fn main() {
    logging::init();
    hypertype_core::install_panic_logger();

    #[cfg(feature = "parity")]
    if let Some(state_name) = parity::cli_state() {
        parity::run(state_name);
        return;
    }

    #[cfg(feature = "update-probe")]
    if update_probe_cli() {
        return;
    }

    let events = events::init();
    // Claim ownership before the engine starts, so a rejected second
    // process never installs a second keyboard hook.
    let _instance = match instance::claim() {
        Ok(Claim::Primary(guard)) => guard,
        Ok(Claim::Secondary) => return,
        Err(e) => {
            logging::error(&format!("single-instance claim failed: {e}"));
            return;
        }
    };
    let state = hypertype_core::start();
    let minimized = std::env::args().any(|a| a == "--minimized");

    application()
        .with_assets(crate::assets::Assets)
        // The tray keeps HyperType alive; closing the window must not quit.
        .with_quit_mode(QuitMode::Explicit)
        .run(move |cx| {
            gpui_kit::init(cx);
            ui::init_theme(cx);

            // The tray and hotkey windows must live on this thread's message loop.
            let hotkeys = match GlobalHotkeys::new(state.clone()) {
                Ok(hotkeys) => hotkeys,
                Err(e) => {
                    logging::error(&format!("global hotkeys unavailable: {e}"));
                    cx.quit();
                    return;
                }
            };
            hypertype_core::hotkeys::register_all(&hotkeys, &state);
            let tray = Tray::new(state.enabled.load(std::sync::atomic::Ordering::Relaxed))
                .inspect_err(|e| logging::error(&format!("tray unavailable: {e}")))
                .ok();
            Shell::install(state.clone(), hotkeys, tray, cx);

            cx.spawn(async move |cx| {
                while let Ok(event) = events.recv().await {
                    cx.update(|cx| shell::handle(event, cx));
                }
            })
            .detach();

            if !minimized {
                ui::open_window(cx);
            }
            logging::info("HyperType ready");
        });
}

/// Live-feed probe: `hypertype --update-probe <version>` or
/// `hypertype --update-probe-tamper <version>`. Never launches the installer.
#[cfg(feature = "update-probe")]
fn update_probe_cli() -> bool {
    let mut args = std::env::args().skip(1);
    let Some(flag) = args.next() else {
        return false;
    };
    let tamper = match flag.as_str() {
        "--update-probe" => false,
        "--update-probe-tamper" => true,
        _ => return false,
    };
    let version = args
        .next()
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());

    println!("update-probe: current={version} tamper={tamper}");
    match updater::check_with(&version) {
        Ok(None) => {
            println!("result: no update");
            true
        }
        Ok(Some(info)) => {
            println!("result: update available");
            println!("version: {}", info.version);
            if let Some(body) = &info.body {
                let preview: String = body.chars().take(120).collect();
                println!("body: {preview}…");
            }

            let mut downloaded = 0usize;
            let mut total = 0u64;
            match updater::download_verified_to_temp(&info, |p| match p {
                updater::Progress::Started { content_length } => {
                    total = content_length.unwrap_or(0);
                    println!("download: started content_length={content_length:?}");
                }
                updater::Progress::Chunk(n) => {
                    downloaded += n;
                    if let Some(pct) = (downloaded as u64).checked_mul(100).and_then(|v| v.checked_div(total)) {
                        print!("\rdownload: {downloaded}/{total} ({pct}%)");
                    } else {
                        print!("\rdownload: {downloaded} bytes");
                    }
                    let _ = std::io::Write::flush(&mut std::io::stdout());
                }
                updater::Progress::Finished => {
                    println!("\ndownload: finished");
                }
            }) {
                Ok(path) => {
                    let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                    println!("installer: {} ({size} bytes)", path.display());
                    println!("signature: OK");

                    if tamper {
                        let mut bytes =
                            std::fs::read(&path).expect("read installer for tamper");
                        if let Some(b) = bytes.first_mut() {
                            *b ^= 0xff;
                        }
                        match updater::verify_signature(&bytes, info.signature()) {
                            Ok(()) => {
                                eprintln!("tamper: unexpected OK");
                                std::process::exit(2);
                            }
                            Err(e) => {
                                println!("tamper: rejected as expected: {e}");
                            }
                        }
                    }

                    println!("probe: done (installer NOT launched)");
                }
                Err(e) => {
                    eprintln!("download/verify failed: {e}");
                    std::process::exit(1);
                }
            }
            true
        }
        Err(e) => {
            eprintln!("check failed: {e}");
            std::process::exit(1);
        }
    }
}
