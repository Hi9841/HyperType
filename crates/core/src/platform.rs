//! Thin Windows context helpers used by the engine: which window is focused,
//! its keyboard layout (for correct character decoding), and whether the
//! focused control is a password field. All Win32, all confined to this file
//! plus `keyboard.rs` and `expansion/`, so a future macOS/Linux port only
//! reimplements these seams.

use std::path::Path;

use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, HWND};
use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, QueryFullProcessImageNameW,
    PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyboardLayout, ToUnicodeEx, HKL};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    GetClassNameW, GetForegroundWindow, GetGUIThreadInfo, GetWindowLongPtrW, GetWindowTextW,
    GetWindowThreadProcessId, GUITHREADINFO, GWL_STYLE, SW_SHOW,
};

/// Edit-control style bit. Defined locally to avoid depending on whether the
/// `windows` crate surfaces ES_* constants in this version.
const ES_PASSWORD: isize = 0x0020;

/// Ask `ToUnicodeEx` to translate without modifying the calling thread's
/// kernel keyboard buffer. This prevents a passive decoder from consuming or
/// composing dead keys before the foreground application sees them.
const TO_UNICODE_NO_STATE_CHANGE: u32 = 0x04;

/// Full result of translating one keyboard event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decoded {
    Text(String),
    Dead,
    None,
}

/// Raw handle of the foreground window, used to detect focus changes.
pub fn foreground_window() -> isize {
    unsafe { GetForegroundWindow().0 as isize }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ForegroundContext {
    pub hwnd: isize,
    pub title: String,
    pub class_name: String,
    pub process_name: String,
}

fn window_text(hwnd: windows::Win32::Foundation::HWND) -> String {
    let mut buf = [0u16; 512];
    let len = unsafe { GetWindowTextW(hwnd, &mut buf) };
    String::from_utf16_lossy(&buf[..len.max(0) as usize])
}

fn window_class_name(hwnd: windows::Win32::Foundation::HWND) -> String {
    let mut buf = [0u16; 256];
    let len = unsafe { GetClassNameW(hwnd, &mut buf) };
    String::from_utf16_lossy(&buf[..len.max(0) as usize])
}

fn process_basename(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn window_process_name(hwnd: HWND) -> String {
    unsafe {
        let mut process_id = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut process_id));
        if process_id == 0 {
            return String::new();
        }

        let Ok(process) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id) else {
            return String::new();
        };
        let mut buffer = vec![0u16; 1024];
        let mut len = buffer.len() as u32;
        let queried = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut len,
        )
        .is_ok();
        let _ = CloseHandle(process);

        if queried {
            process_basename(&String::from_utf16_lossy(&buffer[..len as usize]))
        } else {
            String::new()
        }
    }
}

pub fn foreground_context() -> ForegroundContext {
    unsafe {
        let fg = GetForegroundWindow();
        ForegroundContext {
            hwnd: fg.0 as isize,
            title: window_text(fg),
            class_name: window_class_name(fg),
            process_name: window_process_name(fg),
        }
    }
}

/// Raw handle of the focused child/control in the foreground GUI thread.
/// Best-effort: browser engines often expose a single renderer HWND, but
/// native controls and many desktop apps change this as the user tabs/clicks.
#[allow(dead_code)]
pub fn focused_control() -> isize {
    unsafe {
        let fg = GetForegroundWindow();
        if fg.0.is_null() {
            return 0;
        }
        let tid = GetWindowThreadProcessId(fg, None);
        let mut info = GUITHREADINFO {
            cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        if GetGUIThreadInfo(tid, &mut info).is_ok() && !info.hwndFocus.0.is_null() {
            info.hwndFocus.0 as isize
        } else {
            0
        }
    }
}

pub(crate) fn foreground_keyboard_layout() -> HKL {
    unsafe {
        let fg = GetForegroundWindow();
        let tid = GetWindowThreadProcessId(fg, None);
        GetKeyboardLayout(tid)
    }
}

fn decoded_from_utf16(result: i32, buffer: &[u16]) -> Decoded {
    match result {
        result if result < 0 => Decoded::Dead,
        0 => Decoded::None,
        result => {
            let produced = (result as usize).min(buffer.len());
            let text = String::from_utf16_lossy(&buffer[..produced]);
            if text.is_empty() {
                Decoded::None
            } else {
                Decoded::Text(text)
            }
        }
    }
}

/// Translate a virtual-key + scan-code into every Unicode scalar it would
/// produce in the foreground app's keyboard layout, honoring our tracked
/// modifier state and reporting dead keys distinctly.
pub(crate) fn decode_key(vk: u32, scan: u32, keystate: &[u8; 256]) -> Decoded {
    unsafe {
        let hkl = foreground_keyboard_layout();
        let mut buffer = [0u16; 16];
        let result = ToUnicodeEx(
            vk,
            scan,
            keystate,
            &mut buffer,
            TO_UNICODE_NO_STATE_CHANGE,
            hkl,
        );
        decoded_from_utf16(result, &buffer)
    }
}

/// Compatibility helper for callers that only accept one character. The
/// engine uses `decode_key` so it can handle dead keys and multi-scalar text.
#[allow(dead_code)]
pub fn decode_char(vk: u32, scan: u32, keystate: &[u8; 256]) -> Option<char> {
    match decode_key(vk, scan, keystate) {
        Decoded::Text(text) => text.chars().next(),
        Decoded::Dead | Decoded::None => None,
    }
}

/// Seed the engine's tracked keyboard state from the real physical state.
/// Without this, starting HyperType while a modifier or CapsLock is active
/// decodes the next keystroke as plain text (Ctrl+C read as "c" and firing a
/// snippet) or with the wrong case. Called at engine start and after any
/// context reset.
pub fn seed_modifier_state(keystate: &mut [u8; 256]) {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, GetKeyState, VK_CAPITAL, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN,
        VK_RCONTROL, VK_RMENU, VK_RSHIFT, VK_RWIN,
    };

    let modifiers = [
        VK_LSHIFT,
        VK_RSHIFT,
        VK_LCONTROL,
        VK_RCONTROL,
        VK_LMENU,
        VK_RMENU,
        VK_LWIN,
        VK_RWIN,
    ];
    for vk in modifiers {
        let down = unsafe { (GetAsyncKeyState(vk.0 as i32) as u16 & 0x8000) != 0 };
        if down {
            keystate[vk.0 as usize] |= 0x80;
        } else {
            keystate[vk.0 as usize] &= !0x80;
        }
    }

    let caps = unsafe { GetKeyState(VK_CAPITAL.0 as i32) & 0x0001 } != 0;
    if caps {
        keystate[VK_CAPITAL.0 as usize] |= 0x01;
    } else {
        keystate[VK_CAPITAL.0 as usize] &= !0x01;
    }
}

/// True when the focused control is a classic password edit (ES_PASSWORD).
/// Best-effort: catches native Win32 password fields. Browser/Electron fields
/// would need UI Automation, which is a post-MVP upgrade.
pub fn is_password_field() -> bool {
    unsafe {
        let fg = GetForegroundWindow();
        if fg.0.is_null() {
            return false;
        }
        let tid = GetWindowThreadProcessId(fg, None);
        let mut info = GUITHREADINFO {
            cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        if GetGUIThreadInfo(tid, &mut info).is_ok() {
            let focus = info.hwndFocus;
            if !focus.0.is_null() {
                let style = GetWindowLongPtrW(focus, GWL_STYLE);
                return (style & ES_PASSWORD) != 0;
            }
        }
        false
    }
}

/// Returns true if the current process is running with elevated privileges (Administrator).
pub fn is_elevated() -> bool {
    unsafe {
        let mut token = windows::Win32::Foundation::HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION::default();
        let mut return_len = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut _),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut return_len,
        );
        let _ = CloseHandle(token);
        ok.is_ok() && elevation.TokenIsElevated != 0
    }
}

/// Relaunches the HyperType executable with administrator privileges (UAC prompt)
/// and terminates the current process.
pub fn restart_elevated() -> Result<(), String> {
    let current_exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe_path_wide: Vec<u16> = current_exe
        .to_str()
        .ok_or_else(|| "invalid exe path".to_string())?
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let res = ShellExecuteW(
            HWND::default(),
            windows::core::w!("runas"),
            windows::core::PCWSTR(exe_path_wide.as_ptr()),
            windows::core::PCWSTR::null(),
            windows::core::PCWSTR::null(),
            SW_SHOW,
        );
        // ShellExecute returns an HINSTANCE > 32 on success
        if (res.0 as usize) > 32 {
            std::process::exit(0);
        } else {
            Err(format!(
                "ShellExecute runas failed with code {}",
                res.0 as usize
            ))
        }
    }
}
