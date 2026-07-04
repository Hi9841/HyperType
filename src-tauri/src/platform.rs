//! Thin Windows context helpers used by the engine: which window is focused,
//! its keyboard layout (for correct character decoding), and whether the
//! focused control is a password field. All Win32, all confined to this file
//! plus `keyboard.rs` and `expansion/`, so a future macOS/Linux port only
//! reimplements these seams.

use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyboardLayout, ToUnicodeEx, HKL};
use windows::Win32::UI::WindowsAndMessaging::{
    GetClassNameW, GetForegroundWindow, GetGUIThreadInfo, GetWindowLongPtrW, GetWindowTextW,
    GetWindowThreadProcessId, GUITHREADINFO, GWL_STYLE,
};

/// Edit-control style bit. Defined locally to avoid depending on whether the
/// `windows` crate surfaces ES_* constants in this version.
const ES_PASSWORD: isize = 0x0020;

/// Raw handle of the foreground window, used to detect focus changes.
pub fn foreground_window() -> isize {
    unsafe { GetForegroundWindow().0 as isize }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ForegroundContext {
    pub hwnd: isize,
    pub title: String,
    pub class_name: String,
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

pub fn foreground_context() -> ForegroundContext {
    unsafe {
        let fg = GetForegroundWindow();
        ForegroundContext {
            hwnd: fg.0 as isize,
            title: window_text(fg),
            class_name: window_class_name(fg),
        }
    }
}

/// Raw handle of the focused child/control in the foreground GUI thread.
/// Best-effort: browser engines often expose a single renderer HWND, but
/// native controls and many desktop apps change this as the user tabs/clicks.
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

fn foreground_keyboard_layout() -> HKL {
    unsafe {
        let fg = GetForegroundWindow();
        let tid = GetWindowThreadProcessId(fg, None);
        GetKeyboardLayout(tid)
    }
}

/// Translate a virtual-key + scan-code into the character it would produce in
/// the foreground app's keyboard layout, honoring our tracked modifier state.
/// Returns `None` for non-text keys and dead keys.
pub fn decode_char(vk: u32, scan: u32, keystate: &[u8; 256]) -> Option<char> {
    unsafe {
        let hkl = foreground_keyboard_layout();
        let mut buf = [0u16; 8];
        let n = ToUnicodeEx(vk, scan, keystate, &mut buf, 0, hkl);
        if n > 0 {
            String::from_utf16_lossy(&buf[..n as usize]).chars().next()
        } else {
            None
        }
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
