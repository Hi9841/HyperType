// Hide the console window in release; keep it in debug for logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ipc;
mod shortcuts;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use hypertype_core::app_state::AppState;
use hypertype_core::{commands, hotkeys, logging};
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_autostart::MacosLauncher;

use shortcuts::TauriHotkeys;

/// Set when the user explicitly quits, so the tray-keep-alive guard knows to
/// let the process exit.
static QUIT: AtomicBool = AtomicBool::new(false);

pub fn request_quit() {
    QUIT.store(true, Ordering::SeqCst);
}

/// Handle to the tray menu's "Enabled" check item, so the IPC layer can keep
/// its checkmark in sync when the engine is toggled from the window.
pub struct TrayToggle(pub CheckMenuItem<tauri::Wry>);

pub fn sync_tray_toggle(app: &AppHandle, enabled: bool) {
    if let Some(tray) = app.try_state::<TrayToggle>() {
        let _ = tray.0.set_checked(enabled);
    }
}

/// Show the main window, recreating it if it was closed (which destroys the
/// WebView2 process to keep idle memory low).
fn open_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    } else {
        match WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
            .title("HyperType")
            // Studio split-pane layout: wide canvas for large multi-line expansions
            // and comfortable snippet library browsing.
            .inner_size(960.0, 640.0)
            .min_inner_size(780.0, 520.0)
            .resizable(true)
            .center()
            // Frameless: the UI draws its own titlebar (drag region + window
            // buttons). Pre-paint in the UI's background color so opening the
            // window never flashes white.
            .decorations(false)
            .theme(Some(tauri::Theme::Dark))
            .background_color(tauri::window::Color(0, 0, 0, 255))
            .build()
        {
            Ok(w) => {
                let _ = w.show();
                let _ = w.set_focus();
            }
            Err(e) => {
                eprintln!("[ERROR] open_main_window build failed: {e:?}");
            }
        }
    }
}

fn build_tray(app: &AppHandle, state: Arc<AppState>) -> tauri::Result<()> {
    let enabled = state.enabled.load(Ordering::Relaxed);
    let toggle = CheckMenuItem::with_id(app, "toggle", "Enabled", true, enabled, None::<&str>)?;
    let open = MenuItem::with_id(app, "open", "Open HyperType", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&toggle, &separator, &open, &quit])?;

    app.manage(TrayToggle(toggle.clone()));

    let toggle_item = toggle.clone();
    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip("HyperType")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "toggle" => {
                let state = app.state::<Arc<AppState>>();
                let now = commands::toggle_enabled(state.inner(), &TauriHotkeys(app));
                let _ = toggle_item.set_checked(now);
                // If the manager window is open, keep its switch in sync.
                let _ = app.emit("enabled-changed", now);
            }
            "open" => open_main_window(app),
            "quit" => {
                request_quit();
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                open_main_window(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

fn main() {
    logging::init();
    hypertype_core::install_panic_logger();

    tauri::Builder::default()
        // single-instance must be registered first.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            logging::info("single-instance triggered: focus requested from second process");
            open_main_window(app);
        }))
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            ipc::get_status,
            ipc::get_snippets,
            ipc::add_snippet,
            ipc::edit_snippet,
            ipc::remove_snippet,
            ipc::export_snippets,
            ipc::import_snippets,
            ipc::reorder_snippets,
            ipc::toggle_enabled,
            ipc::set_insert_mode,
            ipc::set_wpm,
            ipc::set_paste_combo,
            ipc::set_restore_delay_ms,
            ipc::set_auto_paste_words,
            ipc::quit_app,
            ipc::reinstall_hook,
            ipc::restart_elevated
        ])
        .setup(move |app| {
            logging::info("Tauri setup callback invoked");

            // Storage, state, engine, and hook start here — after the
            // single-instance plugin has claimed primary. A rejected secondary
            // process exits inside the plugin's own setup, before this runs,
            // so it never starts a second engine or keyboard hook.
            let state = hypertype_core::start();
            app.manage(state.clone());

            build_tray(app.handle(), state.clone())?;
            logging::info("Tray built successfully");
            // Registering shortcuts marshals each Win32 RegisterHotKey call
            // onto the main thread and blocks waiting for it to run. The
            // event loop isn't pumping yet during `setup`, so doing this
            // inline here deadlocks. Do it from a background thread instead,
            // after `run()` has started the loop.
            let handle = app.handle().clone();
            let register_state = state.clone();
            std::thread::spawn(move || {
                hotkeys::register_all(&TauriHotkeys(&handle), &register_state);
            });
            if std::env::args().any(|a| a == "--minimized") {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.hide();
                }
            } else {
                // The window is created hidden to avoid a flash at logon;
                // a normal launch shows it explicitly.
                open_main_window(app.handle());
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("failed to build HyperType")
        .run(|_app, event| match event {
            RunEvent::ExitRequested { api, .. } => {
                logging::info("RunEvent::ExitRequested received");
                // Keep running in the tray when the last window closes, unless
                // the user actually chose Quit.
                if !QUIT.load(Ordering::SeqCst) {
                    api.prevent_exit();
                    logging::info("prevented exit, running in tray");
                }
            }
            RunEvent::Exit => {
                logging::info("RunEvent::Exit received");
            }
            RunEvent::Ready => {
                logging::info("RunEvent::Ready received");
            }
            RunEvent::Resumed => {
                logging::info("RunEvent::Resumed received");
            }
            RunEvent::WindowEvent { label, event, .. } => {
                logging::info(&format!("RunEvent::WindowEvent [{label}]: {event:?}"));
            }
            _ => {}
        });
    logging::info("main() exited");
}
