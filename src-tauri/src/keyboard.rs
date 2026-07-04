//! Global keyboard capture via a WH_KEYBOARD_LL low-level hook.
//!
//! The hook lives on its own thread running a Windows message pump (required
//! for low-level hooks). The callback does the absolute minimum: drop injected
//! events (our own SendInput output), then forward the key to the engine over
//! a channel and return immediately. Doing real work here would risk Windows
//! silently removing a slow hook (~300ms budget). Zero polling: the OS calls
//! us only when a key is pressed, so idle CPU is 0%.

use std::sync::mpsc::Sender;
use std::sync::OnceLock;
use std::thread;

use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, SetWindowsHookExW, TranslateMessage, HC_ACTION,
    HHOOK, KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN, WM_LBUTTONDOWN,
    WM_MBUTTONDOWN, WM_RBUTTONDOWN, WM_SYSKEYDOWN, WM_XBUTTONDOWN,
};

/// One raw key transition forwarded from the hook to the engine.
pub struct KeyEvent {
    pub message: u32,
    pub vk: u32,
    pub scan: u32,
}

static HOOK_TX: OnceLock<Sender<KeyEvent>> = OnceLock::new();

/// Install the hook on a dedicated thread and start pumping messages.
pub fn start(tx: Sender<KeyEvent>) {
    let _ = HOOK_TX.set(tx);
    thread::spawn(|| unsafe {
        let hmod = GetModuleHandleW(None).unwrap_or_default();
        let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(low_level_proc), HINSTANCE(hmod.0), 0);
        if hook.is_err() {
            crate::logging::error("failed to install WH_KEYBOARD_LL hook");
            return;
        }
        crate::logging::info("keyboard hook installed");

        let mouse_hook = SetWindowsHookExW(
            WH_MOUSE_LL,
            Some(low_level_mouse_proc),
            HINSTANCE(hmod.0),
            0,
        );
        if mouse_hook.is_err() {
            crate::logging::error("failed to install WH_MOUSE_LL hook");
        } else {
            crate::logging::info("mouse hook installed");
        }

        let mut msg = MSG::default();
        // GetMessageW blocks until a message arrives; this thread is otherwise
        // asleep. Returns <= 0 on WM_QUIT or error.
        while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    });
}

unsafe extern "system" fn low_level_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let kb = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        // Ignore only the events we synthesized ourselves (tagged in
        // dwExtraInfo). Everything else, physical or otherwise, is processed.
        let ours = kb.dwExtraInfo == crate::consts::INJECT_SIGNATURE;
        if !ours {
            if wparam.0 as u32 == WM_KEYDOWN || wparam.0 as u32 == WM_SYSKEYDOWN {
                crate::expansion::cancel_active_typeout();
            }
            if let Some(tx) = HOOK_TX.get() {
                let _ = tx.send(KeyEvent {
                    message: wparam.0 as u32,
                    vk: kb.vkCode,
                    scan: kb.scanCode,
                });
            }
        }
    }
    CallNextHookEx(HHOOK::default(), code, wparam, lparam)
}

unsafe extern "system" fn low_level_mouse_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code == HC_ACTION as i32 {
        let message = wparam.0 as u32;
        if matches!(
            message,
            WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_XBUTTONDOWN
        ) {
            crate::expansion::cancel_active_typeout();
        }
    }
    CallNextHookEx(HHOOK::default(), code, wparam, lparam)
}
