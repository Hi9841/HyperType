//! HyperType's own installer. No NSIS, no WiX: the release `hypertype.exe`
//! is embedded in this binary at compile time, and the setup UI is a single
//! small dark form drawn with raw Win32 (matching the app's design). The
//! same binary copies itself into the install directory as `uninstall.exe`
//! and handles `--uninstall` silently.
//!
//! Install layout:
//! - %LOCALAPPDATA%\Programs\HyperType\hypertype.exe (+ uninstall.exe)
//! - Start Menu shortcut: %APPDATA%\...\Start Menu\Programs\HyperType.lnk
//! - HKCU uninstall registry entry (shows up in Settings > Installed apps)

#![windows_subsystem = "windows"]

use std::io;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicIsize, Ordering};

use winreg::enums::HKEY_CURRENT_USER;
use winreg::RegKey;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    CreateFontW, CreatePen, CreateSolidBrush, DrawTextW, RoundRect, SelectObject, SetBkMode,
    SetTextColor, CLEARTYPE_QUALITY, DT_CENTER, DT_SINGLELINE, DT_VCENTER, HDC, HFONT,
    PS_SOLID, TRANSPARENT,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Controls::{DRAWITEMSTRUCT, ODS_SELECTED};
use windows_sys::Win32::UI::HiDpi::GetDpiForSystem;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AdjustWindowRectEx, CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW,
    GetSystemMetrics, LoadCursorW, MessageBoxW, PostQuitMessage, RegisterClassW, SendMessageW,
    ShowWindow, TranslateMessage, BS_OWNERDRAW, CS_HREDRAW, CS_VREDRAW, IDC_ARROW,
    MB_ICONERROR, MB_ICONINFORMATION, MB_OK, MSG, SM_CXSCREEN, SM_CYSCREEN, SW_SHOW,
    WM_COMMAND, WM_CTLCOLORSTATIC, WM_DESTROY, WM_DRAWITEM, WM_SETFONT, WNDCLASSW, WS_CAPTION,
    WS_CHILD, WS_EX_APPWINDOW, WS_SYSMENU, WS_VISIBLE,
};

/// The app binary, baked in at build time. Building the installer therefore
/// requires a prior release build of the app itself (`pnpm dist` does both).
const APP_EXE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../src-tauri/target/release/hypertype.exe"
));

const VERSION: &str = env!("CARGO_PKG_VERSION");
const UNINSTALL_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\HyperType";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

const ID_INSTALL: usize = 1;
const ID_CLOSE: usize = 2;

// UI palette, COLORREF (0x00BBGGRR). Mirrors the app's tokens.
const BG: u32 = 0x000E0E0E;
const INK: u32 = 0x00EDEDED;
const INK_2: u32 = 0x009D9D9D;
const ACCENT: u32 = 0x00FF840A; // #0a84ff
const ACCENT_DOWN: u32 = 0x00E87407; // #0774e8
const BTN_2: u32 = 0x002A2A2A;
const BTN_2_LINE: u32 = 0x003D3D3D;

static HWND_TITLE: AtomicIsize = AtomicIsize::new(0);
static HWND_INSTALL: AtomicIsize = AtomicIsize::new(0);
static HWND_CLOSE: AtomicIsize = AtomicIsize::new(0);
static BG_BRUSH: AtomicIsize = AtomicIsize::new(0);

fn main() {
    if std::env::args().any(|a| a == "--uninstall") {
        if let Err(e) = uninstall() {
            message_box("HyperType could not be removed", &e.to_string(), MB_ICONERROR);
            std::process::exit(1);
        }
        return;
    }
    run_setup_form();
}

// ---- Install / uninstall logic ----

fn install_dir() -> io::Result<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")
        .ok_or_else(|| io::Error::other("LOCALAPPDATA is not set"))?;
    Ok(PathBuf::from(base).join("Programs").join("HyperType"))
}

fn shortcut_path() -> io::Result<PathBuf> {
    let appdata = std::env::var_os("APPDATA")
        .ok_or_else(|| io::Error::other("APPDATA is not set"))?;
    Ok(PathBuf::from(appdata)
        .join(r"Microsoft\Windows\Start Menu\Programs")
        .join("HyperType.lnk"))
}

fn kill_running_app() {
    // Best-effort: a running instance would hold the exe file locked.
    let _ = Command::new("taskkill")
        .args(["/F", "/IM", "hypertype.exe"])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
}

fn install() -> io::Result<()> {
    let dir = install_dir()?;
    let app_path = dir.join("hypertype.exe");
    let uninstaller_path = dir.join("uninstall.exe");

    kill_running_app();
    std::fs::create_dir_all(&dir)?;

    // The old exe can stay locked for a moment after taskkill; retry briefly.
    let mut written = std::fs::write(&app_path, APP_EXE);
    for _ in 0..10 {
        if written.is_ok() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
        written = std::fs::write(&app_path, APP_EXE);
    }
    written?;

    // This same binary becomes the uninstaller. Skip the copy when running
    // *from* the install dir (e.g. a repair run of uninstall.exe itself).
    let me = std::env::current_exe()?;
    if me != uninstaller_path {
        std::fs::copy(&me, &uninstaller_path)?;
    }

    create_shortcut(&app_path)?;
    register_uninstall(&dir, &app_path, &uninstaller_path)?;

    // Launching the app (which opens its manager window) is the completion
    // feedback; the form closes right after.
    Command::new(&app_path).spawn()?;
    Ok(())
}

fn create_shortcut(app_path: &Path) -> io::Result<()> {
    let link = mslnk::ShellLink::new(app_path)
        .map_err(|e| io::Error::other(format!("shortcut: {e}")))?;
    link.create_lnk(shortcut_path()?)
        .map_err(|e| io::Error::other(format!("shortcut: {e}")))
}

fn register_uninstall(dir: &Path, app_path: &Path, uninstaller: &Path) -> io::Result<()> {
    let (key, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(UNINSTALL_KEY)?;
    key.set_value("DisplayName", &"HyperType")?;
    key.set_value("DisplayVersion", &VERSION)?;
    key.set_value("Publisher", &"HyperType")?;
    key.set_value("DisplayIcon", &app_path.to_string_lossy().as_ref())?;
    key.set_value("InstallLocation", &dir.to_string_lossy().as_ref())?;
    key.set_value(
        "UninstallString",
        &format!("\"{}\" --uninstall", uninstaller.display()),
    )?;
    key.set_value("EstimatedSize", &((APP_EXE.len() / 1024) as u32))?;
    key.set_value("NoModify", &1u32)?;
    key.set_value("NoRepair", &1u32)?;
    Ok(())
}

fn uninstall() -> io::Result<()> {
    let dir = install_dir()?;
    kill_running_app();

    let _ = std::fs::remove_file(shortcut_path()?);
    let _ = RegKey::predef(HKEY_CURRENT_USER).delete_subkey_all(UNINSTALL_KEY);
    let _ = std::fs::remove_file(dir.join("hypertype.exe"));

    message_box("HyperType", "HyperType has been removed.", MB_ICONINFORMATION);

    // uninstall.exe cannot delete itself while running; hand the final
    // cleanup to a detached cmd that waits for this process to exit.
    let cleanup = format!(
        "ping -n 2 127.0.0.1 > nul & rmdir /S /Q \"{}\"",
        dir.display()
    );
    Command::new("cmd")
        .args(["/C", &cleanup])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()?;
    Ok(())
}

fn message_box(title: &str, text: &str, icon: u32) {
    let (t, m) = (wide(title), wide(text));
    unsafe {
        MessageBoxW(std::ptr::null_mut(), m.as_ptr(), t.as_ptr(), MB_OK | icon);
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

// ---- The setup form: one small dark window, two buttons ----

fn run_setup_form() {
    unsafe {
        let hinstance = GetModuleHandleW(std::ptr::null());
        let class_name = wide("HyperTypeSetup");
        let bg_brush = CreateSolidBrush(BG);
        BG_BRUSH.store(bg_brush as isize, Ordering::Relaxed);

        // The app icon is embedded in this exe as a resource; pull the first
        // icon group so the caption and taskbar match the app everywhere.
        let mut app_icon: windows_sys::Win32::UI::WindowsAndMessaging::HICON =
            std::ptr::null_mut();
        if let Ok(me) = std::env::current_exe() {
            let path = wide(&me.to_string_lossy());
            windows_sys::Win32::UI::Shell::ExtractIconExW(
                path.as_ptr(),
                0,
                &mut app_icon,
                std::ptr::null_mut(),
                1,
            );
        }

        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: app_icon,
            hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
            hbrBackground: bg_brush,
            lpszMenuName: std::ptr::null(),
            lpszClassName: class_name.as_ptr(),
        };
        RegisterClassW(&wc);

        let s = GetDpiForSystem() as f32 / 96.0;
        let px = |v: i32| (v as f32 * s) as i32;

        let (client_w, client_h) = (px(400), px(176));
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: client_w,
            bottom: client_h,
        };
        let style = WS_CAPTION | WS_SYSMENU;
        AdjustWindowRectEx(&mut rect, style, 0, WS_EX_APPWINDOW);
        let (w, h) = (rect.right - rect.left, rect.bottom - rect.top);
        let x = (GetSystemMetrics(SM_CXSCREEN) - w) / 2;
        let y = (GetSystemMetrics(SM_CYSCREEN) - h) / 2;

        let title = wide("HyperType Setup");
        let hwnd = CreateWindowExW(
            WS_EX_APPWINDOW,
            class_name.as_ptr(),
            title.as_ptr(),
            style,
            x,
            y,
            w,
            h,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            hinstance,
            std::ptr::null(),
        );

        let title_font = CreateFontW(
            -px(20), 0, 0, 0, 600, 0, 0, 0, 0, 0, 0, CLEARTYPE_QUALITY as u32, 0,
            wide("Segoe UI").as_ptr(),
        );
        let body_font = CreateFontW(
            -px(13), 0, 0, 0, 400, 0, 0, 0, 0, 0, 0, CLEARTYPE_QUALITY as u32, 0,
            wide("Segoe UI").as_ptr(),
        );

        let make_static = |text: &str, x: i32, y: i32, w: i32, h: i32, font: HFONT| {
            let cls = wide("STATIC");
            let txt = wide(text);
            let hw = CreateWindowExW(
                0, cls.as_ptr(), txt.as_ptr(), WS_CHILD | WS_VISIBLE,
                x, y, w, h, hwnd, std::ptr::null_mut(), hinstance, std::ptr::null(),
            );
            SendMessageW(hw, WM_SETFONT, font as WPARAM, 1);
            hw
        };
        let make_button = |text: &str, id: usize, x: i32, y: i32, w: i32, h: i32| {
            let cls = wide("BUTTON");
            let txt = wide(text);
            let hw = CreateWindowExW(
                0, cls.as_ptr(), txt.as_ptr(), WS_CHILD | WS_VISIBLE | BS_OWNERDRAW as u32,
                x, y, w, h, hwnd, id as _, hinstance, std::ptr::null(),
            );
            SendMessageW(hw, WM_SETFONT, body_font as WPARAM, 1);
            hw
        };

        let title_hw = make_static("HyperType", px(24), px(24), px(352), px(28), title_font);
        HWND_TITLE.store(title_hw as isize, Ordering::Relaxed);
        make_static(
            "Instant text expansion for Windows.",
            px(24), px(56), px(352), px(20), body_font,
        );
        make_static(
            "Installs for the current user. No admin needed.",
            px(24), px(76), px(352), px(20), body_font,
        );

        let close_hw = make_button("Close", ID_CLOSE, px(196), px(124), px(88), px(32));
        HWND_CLOSE.store(close_hw as isize, Ordering::Relaxed);
        let install_hw = make_button("Install", ID_INSTALL, px(292), px(124), px(88), px(32));
        HWND_INSTALL.store(install_hw as isize, Ordering::Relaxed);

        ShowWindow(hwnd, SW_SHOW);

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_CTLCOLORSTATIC => {
            let hdc = wparam as HDC;
            let is_title = lparam == HWND_TITLE.load(Ordering::Relaxed);
            SetTextColor(hdc, if is_title { INK } else { INK_2 });
            SetBkMode(hdc, TRANSPARENT as i32);
            BG_BRUSH.load(Ordering::Relaxed) as LRESULT
        }
        WM_DRAWITEM => {
            let dis = &*(lparam as *const DRAWITEMSTRUCT);
            let primary = dis.CtlID as usize == ID_INSTALL;
            let pressed = dis.itemState & ODS_SELECTED != 0;
            let (fill, line, text_color) = if primary {
                (
                    if pressed { ACCENT_DOWN } else { ACCENT },
                    if pressed { ACCENT_DOWN } else { ACCENT },
                    0x00FFFFFF,
                )
            } else {
                (BTN_2, BTN_2_LINE, INK)
            };
            let brush = CreateSolidBrush(fill);
            let pen = CreatePen(PS_SOLID, 1, line);
            let old_brush = SelectObject(dis.hDC, brush as _);
            let old_pen = SelectObject(dis.hDC, pen as _);
            let radius = (dis.rcItem.bottom - dis.rcItem.top) / 4;
            RoundRect(
                dis.hDC,
                dis.rcItem.left,
                dis.rcItem.top,
                dis.rcItem.right,
                dis.rcItem.bottom,
                radius,
                radius,
            );
            SetTextColor(dis.hDC, text_color);
            SetBkMode(dis.hDC, TRANSPARENT as i32);
            let label = if primary { wide("Install") } else { wide("Close") };
            let mut rc = dis.rcItem;
            DrawTextW(
                dis.hDC,
                label.as_ptr(),
                -1,
                &mut rc,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );
            SelectObject(dis.hDC, old_brush);
            SelectObject(dis.hDC, old_pen);
            windows_sys::Win32::Graphics::Gdi::DeleteObject(brush as _);
            windows_sys::Win32::Graphics::Gdi::DeleteObject(pen as _);
            1
        }
        WM_COMMAND => {
            match wparam & 0xFFFF {
                ID_INSTALL => {
                    EnableWindow(HWND_INSTALL.load(Ordering::Relaxed) as HWND, 0);
                    EnableWindow(HWND_CLOSE.load(Ordering::Relaxed) as HWND, 0);
                    match install() {
                        Ok(()) => PostQuitMessage(0),
                        Err(e) => {
                            message_box(
                                "HyperType could not be installed",
                                &e.to_string(),
                                MB_ICONERROR,
                            );
                            EnableWindow(HWND_INSTALL.load(Ordering::Relaxed) as HWND, 1);
                            EnableWindow(HWND_CLOSE.load(Ordering::Relaxed) as HWND, 1);
                        }
                    }
                }
                ID_CLOSE => PostQuitMessage(0),
                _ => {}
            }
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
