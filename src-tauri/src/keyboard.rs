//! Global keyboard capture via a WH_KEYBOARD_LL low-level hook.
//!
//! The hook lives on its own dedicated thread running a standard Win32 message
//! pump (`GetMessageW`).
//!
//! Critical OS rules enforced here:
//! 1. `low_level_proc` NEVER blocks, sleeps, or waits on condition variables.
//!    Windows enforces `LowLevelHooksTimeout`; if a hook callback delays, Windows
//!    silently and permanently removes it from the global hook chain.
//! 2. The hook procedure does only non-blocking work: filter our own injected
//!    events, bump event telemetry, and dispatch the key event to the engine
//!    channel before immediately calling `CallNextHookEx`.
//! 3. A thread supervisor and watchdog monitor thread health and automatically
//!    reinstall the hook if Windows drops it, or upon explicit request.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Mutex, RwLock};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use windows::Win32::Foundation::{CloseHandle, HINSTANCE, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{
    GetCurrentThreadId, GetExitCodeThread, OpenThread, THREAD_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, PostThreadMessageW, SetWindowsHookExW,
    TranslateMessage, UnhookWindowsHookEx, HC_ACTION, HHOOK, KBDLLHOOKSTRUCT, MSG,
    MSLLHOOKSTRUCT, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN, WM_LBUTTONDOWN, WM_MBUTTONDOWN,
    WM_QUIT, WM_RBUTTONDOWN, WM_SYSKEYDOWN, WM_XBUTTONDOWN,
};

/// One raw key transition forwarded from the hook to the engine.
#[derive(Clone, Copy, Debug)]
pub struct KeyEvent {
    pub message: u32,
    pub vk: u32,
    pub scan: u32,
}

static HOOK_TX: RwLock<Option<Sender<KeyEvent>>> = RwLock::new(None);
static HOOK_READY: AtomicBool = AtomicBool::new(false);
static HOOK_THREAD_ID: AtomicU32 = AtomicU32::new(0);
static HOOK_EVENTS: AtomicU64 = AtomicU64::new(0);
static LAST_EVENT_MS: AtomicU64 = AtomicU64::new(0);
static REINSTALL_LOCK: Mutex<()> = Mutex::new(());

/// True when the low-level hook thread is actively installed and running.
pub fn is_hook_installed() -> bool {
    HOOK_READY.load(Ordering::Acquire)
}

/// Total count of keyboard events intercepted since app launch.
pub fn get_event_count() -> u64 {
    HOOK_EVENTS.load(Ordering::Relaxed)
}

/// Millisecond timestamp of the last captured keyboard event.
#[allow(dead_code)]
pub fn last_event_ms() -> u64 {
    LAST_EVENT_MS.load(Ordering::Relaxed)
}

/// Install the hook on a dedicated thread and start pumping messages.
pub fn start(tx: Sender<KeyEvent>) {
    {
        let mut guard = HOOK_TX.write().unwrap();
        *guard = Some(tx);
    }
    spawn_hook_thread();
    start_watchdog();
}

/// Reinstall the low-level keyboard and mouse hooks cleanly.
/// Safely stops the old thread's message pump, unhooks stale handles,
/// and starts a fresh hook thread.
pub fn reinstall_hook() -> bool {
    let _guard = REINSTALL_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let old_tid = HOOK_THREAD_ID.swap(0, Ordering::SeqCst);
    if old_tid != 0 {
        crate::logging::info(&format!(
            "stopping previous hook thread (tid={old_tid})..."
        ));
        unsafe {
            let _ = PostThreadMessageW(old_tid, WM_QUIT, WPARAM(0), LPARAM(0));
        }
        // Give the old thread up to 100ms to exit its message loop and unhook
        let deadline = std::time::Instant::now() + Duration::from_millis(100);
        while HOOK_READY.load(Ordering::Acquire) && std::time::Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
    }

    spawn_hook_thread();
    wait_until_ready(Duration::from_secs(2))
}

fn spawn_hook_thread() {
    thread::spawn(|| unsafe {
        let tid = GetCurrentThreadId();
        HOOK_THREAD_ID.store(tid, Ordering::SeqCst);

        let hmod = GetModuleHandleW(None).unwrap_or_default();
        let kbd_hook = match SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(low_level_proc),
            HINSTANCE(hmod.0),
            0,
        ) {
            Ok(h) => {
                crate::logging::info("WH_KEYBOARD_LL hook registered successfully");
                Some(h)
            }
            Err(e) => {
                crate::logging::error(&format!("failed to register WH_KEYBOARD_LL hook: {e}"));
                None
            }
        };

        let mouse_hook = match SetWindowsHookExW(
            WH_MOUSE_LL,
            Some(low_level_mouse_proc),
            HINSTANCE(hmod.0),
            0,
        ) {
            Ok(h) => {
                crate::logging::info("WH_MOUSE_LL hook registered successfully");
                Some(h)
            }
            Err(e) => {
                crate::logging::error(&format!("failed to register WH_MOUSE_LL hook: {e}"));
                None
            }
        };

        if kbd_hook.is_some() {
            HOOK_READY.store(true, Ordering::Release);
        }

        let mut msg = MSG::default();
        // GetMessageW blocks until a message arrives. Returns <= 0 on WM_QUIT or error.
        while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        // Clean teardown upon exit
        HOOK_READY.store(false, Ordering::Release);
        if let Some(h) = kbd_hook {
            let _ = UnhookWindowsHookEx(h);
        }
        if let Some(h) = mouse_hook {
            let _ = UnhookWindowsHookEx(h);
        }
        crate::logging::info("hook thread cleanly unhooked and terminated");
    });
}

/// Background watchdog that checks hook thread health every 2.5 seconds.
/// If the thread terminated or becomes invalid, automatically recovers.
pub fn start_watchdog() {
    thread::spawn(|| loop {
        thread::sleep(Duration::from_millis(2500));
        let tid = HOOK_THREAD_ID.load(Ordering::SeqCst);
        let ready = HOOK_READY.load(Ordering::Acquire);

        let mut needs_healing = !ready || tid == 0;

        if !needs_healing && tid != 0 {
            unsafe {
                if let Ok(handle) = OpenThread(THREAD_QUERY_LIMITED_INFORMATION, false, tid) {
                    let mut exit_code = 0u32;
                    const STILL_ACTIVE: u32 = 259;
                    if GetExitCodeThread(handle, &mut exit_code).is_ok() && exit_code != STILL_ACTIVE {
                        crate::logging::error(&format!(
                            "watchdog detected hook thread {tid} died with code {exit_code}"
                        ));
                        needs_healing = true;
                    }
                    let _ = CloseHandle(handle);
                }
            }
        }

        if needs_healing {
            crate::logging::error("watchdog triggering self-healing keyboard hook reinstallation...");
            reinstall_hook();
        }
    });
}

/// The low-level keyboard hook callback procedure.
/// GUARANTEED NON-BLOCKING (< 5 microseconds).
unsafe extern "system" fn low_level_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let kb = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        // Ignore synthetic events generated by HyperType itself
        let ours = kb.dwExtraInfo == crate::consts::INJECT_SIGNATURE;
        if !ours {
            let message = wparam.0 as u32;
            if message == WM_KEYDOWN || message == WM_SYSKEYDOWN {
                crate::expansion::cancel_active_typeout();
            }

            HOOK_EVENTS.fetch_add(1, Ordering::Relaxed);
            let now_ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            LAST_EVENT_MS.store(now_ms, Ordering::Relaxed);

            if let Ok(guard) = HOOK_TX.read() {
                if let Some(ref tx) = *guard {
                    let _ = tx.send(KeyEvent {
                        message,
                        vk: kb.vkCode,
                        scan: kb.scanCode,
                    });
                }
            }
        }
    }
    CallNextHookEx(HHOOK::default(), code, wparam, lparam)
}

/// The low-level mouse hook callback procedure.
/// GUARANTEED NON-BLOCKING.
unsafe extern "system" fn low_level_mouse_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code == HC_ACTION as i32 {
        let ms = &*(lparam.0 as *const MSLLHOOKSTRUCT);
        let ours = ms.dwExtraInfo == crate::consts::INJECT_SIGNATURE;
        if !ours {
            let message = wparam.0 as u32;
            if matches!(
                message,
                WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_XBUTTONDOWN
            ) {
                crate::expansion::cancel_active_typeout();
            }
        }
    }
    CallNextHookEx(HHOOK::default(), code, wparam, lparam)
}

pub fn wait_until_ready(timeout: Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    while !HOOK_READY.load(Ordering::Acquire) && std::time::Instant::now() < deadline {
        thread::sleep(Duration::from_millis(5));
    }
    HOOK_READY.load(Ordering::Acquire)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_counter_and_last_event_telemetry() {
        let initial = get_event_count();
        HOOK_EVENTS.fetch_add(5, Ordering::Relaxed);
        assert_eq!(get_event_count(), initial + 5);

        let now_ms = 1234567890u64;
        LAST_EVENT_MS.store(now_ms, Ordering::Relaxed);
        assert_eq!(last_event_ms(), now_ms);
    }
}
