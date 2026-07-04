// Hide the console window in release; keep it in debug for logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app_state;
mod consts;
mod engine;
mod expansion;
mod ipc;
mod keyboard;
mod logging;
mod platform;
mod shortcuts;
mod snippets;
mod storage;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_autostart::MacosLauncher;

use app_state::AppState;

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
        let _ = window.set_focus();
    } else {
        let _ = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
            .title("HyperType")
            // Fixed phone-portrait ratio (9:16); the UI is designed for
            // exactly this footprint, so the window doesn't resize.
            .inner_size(480.0, 854.0)
            .resizable(false)
            .center()
            // Frameless: the UI draws its own titlebar (drag region + window
            // buttons). Pre-paint in the UI's background color so opening the
            // window never flashes white.
            .decorations(false)
            .theme(Some(tauri::Theme::Dark))
            .background_color(tauri::window::Color(0, 0, 0, 255))
            .build();
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
                let now = !state.enabled.load(Ordering::Relaxed);
                state.enabled.store(now, Ordering::Relaxed);
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

    let data_path = storage::data_file_path();
    let snippets = storage::load_or_default(&data_path);
    let settings = storage::load_settings(&storage::settings_file_path());
    expansion::set_insert_mode(settings.insert_mode);
    expansion::set_wpm(settings.wpm);
    expansion::set_paste_combo(settings.paste_combo);
    expansion::set_restore_delay_ms(settings.restore_delay_ms);
    let state = Arc::new(AppState {
        snippets: RwLock::new(snippets),
        enabled: AtomicBool::new(true),
        data_path,
    });

    // Engine + keyboard hook start before the UI: expansion runs regardless of
    // whether any window is ever opened.
    let tx = engine::start(state.clone());
    keyboard::start(tx);

    let setup_state = state.clone();

    tauri::Builder::default()
        // single-instance must be registered first.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            open_main_window(app);
        }))
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(state)
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
            ipc::quit_app
        ])
        .setup(move |app| {
            build_tray(app.handle(), setup_state.clone())?;
            // Registering shortcuts marshals each Win32 RegisterHotKey call
            // onto the main thread and blocks waiting for it to run. The
            // event loop isn't pumping yet during `setup`, so doing this
            // inline here deadlocks. Do it from a background thread instead,
            // after `run()` has started the loop.
            let handle = app.handle().clone();
            let register_state = setup_state.clone();
            std::thread::spawn(move || {
                shortcuts::register_all(&handle, &register_state);
            });
            // No window is created at startup (see tauri.conf.json windows: []).
            // Open one only for a manual launch; an autostart launch passes
            // --minimized and stays purely in the tray, so no WebView2 process
            // is ever spawned at idle.
            if !std::env::args().any(|a| a == "--minimized") {
                open_main_window(app.handle());
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("failed to build HyperType")
        .run(|_app, event| {
            if let RunEvent::ExitRequested { api, .. } = event {
                // Keep running in the tray when the last window closes, unless
                // the user actually chose Quit.
                if !QUIT.load(Ordering::SeqCst) {
                    api.prevent_exit();
                }
            }
        });
}
