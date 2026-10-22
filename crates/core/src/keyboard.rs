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
use std::time::Duration;

use windows::Win32::Foundation::{CloseHandle, HINSTANCE, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::SystemInformation::GetTickCount64;
use windows::Win32::System::Threading::{
    GetCurrentThreadId, GetExitCodeThread, OpenThread, THREAD_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, PostThreadMessageW, SetWindowsHookExW,
    TranslateMessage, UnhookWindowsHookEx, HC_ACTION, HHOOK, KBDLLHOOKSTRUCT, MSG, MSLLHOOKSTRUCT,
    WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN, WM_LBUTTONDOWN, WM_MBUTTONDOWN, WM_QUIT,
    WM_RBUTTONDOWN, WM_SYSKEYDOWN, WM_XBUTTONDOWN,
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
static ENGINE_HEALTHY: AtomicBool = AtomicBool::new(true);
static HOOK_THREAD_ID: AtomicU32 = AtomicU32::new(0);
static HOOK_EVENTS: AtomicU64 = AtomicU64::new(0);
static LAST_EVENT_MS: AtomicU64 = AtomicU64::new(0);
static CONTEXT_RESET_EPOCH: AtomicU64 = AtomicU64::new(0);
static LAST_REINSTALL_MS: AtomicU64 = AtomicU64::new(0);
static REINSTALL_LOCK: Mutex<()> = Mutex::new(());

/// Hard floor between automatic reinstalls. Only definitive failures (the
/// hook thread exited, or the hook is no longer installed) trigger a
/// reinstall, and never more often than this.
const REINSTALL_COOLDOWN_MS: u64 = 30_000;

fn now_ms() -> u64 {
    unsafe { GetTickCount64() }
}

/// True when the low-level hook thread is actively installed and running.
pub fn is_hook_installed() -> bool {
    HOOK_READY.load(Ordering::Acquire)
}

/// Total count of keyboard events intercepted since app launch.
pub fn get_event_count() -> u64 {
    HOOK_EVENTS.load(Ordering::Relaxed)
}

/// Monotonic millisecond tick count of the last captured keyboard event.
#[allow(dead_code)]
pub fn last_event_ms() -> u64 {
    LAST_EVENT_MS.load(Ordering::Relaxed)
}

/// Bump when the caret context is invalidated without a foreground change
/// (mouse click or drag-select). The engine clears its typed-text buffer when
/// it sees a new epoch, so a stale prefix can never complete against text at a
/// different caret position in the same window.
pub fn request_context_reset() {
    CONTEXT_RESET_EPOCH.fetch_add(1, Ordering::Relaxed);
}

pub fn context_reset_epoch() -> u64 {
    CONTEXT_RESET_EPOCH.load(Ordering::Relaxed)
}

/// Install the hook on a dedicated thread and start pumping messages.
pub fn start(tx: Sender<KeyEvent>) {
    {
        let mut guard = HOOK_TX
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *guard = Some(tx);
    }
    ENGINE_HEALTHY.store(true, Ordering::Release);
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
        crate::logging::info(&format!("stopping previous hook thread (tid={old_tid})..."));
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
        let kbd_hook =
            match SetWindowsHookExW(WH_KEYBOARD_LL, Some(low_level_proc), HINSTANCE(hmod.0), 0) {
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

/// True when the hook is installed and the engine event channel is alive.
pub fn is_hook_healthy() -> bool {
    ENGINE_HEALTHY.load(Ordering::Acquire) && HOOK_READY.load(Ordering::Acquire)
}

/// Pure decision for one watchdog tick: reinstall only when the hook is
/// definitively gone (thread exited or not installed), and never more often
/// than the cooldown.
///
/// The former SendInput heartbeat probe was removed. Runtime testing showed a
/// non-elevated process cannot reliably get its own injected probe delivered
/// back to its low-level hook, so misses were false positives that tore the
/// hook down and dropped keystrokes. Recovery is now limited to definitive
/// failure plus the user-facing "Repair Hook" action.
fn should_reinstall(ready: bool, tid: u32, now: u64, last_reinstall: u64) -> bool {
    let cooldown_elapsed =
        last_reinstall == 0 || now.saturating_sub(last_reinstall) >= REINSTALL_COOLDOWN_MS;
    (!ready || tid == 0) && cooldown_elapsed
}

/// Background watchdog that checks hook thread health every 2.5 seconds.
/// If the thread dies or the hook is no longer installed, it reinstalls it
/// (with a cooldown). It never tears down a working hook on a soft signal.
pub fn start_watchdog() {
    thread::spawn(|| loop {
        thread::sleep(Duration::from_millis(2500));
        let tid = HOOK_THREAD_ID.load(Ordering::SeqCst);
        let ready = HOOK_READY.load(Ordering::Acquire);
        let now = now_ms();
        let last_reinstall = LAST_REINSTALL_MS.load(Ordering::SeqCst);

        let mut needs_healing = should_reinstall(ready, tid, now, last_reinstall);

        if !needs_healing && tid != 0 {
            unsafe {
                if let Ok(handle) = OpenThread(THREAD_QUERY_LIMITED_INFORMATION, false, tid) {
                    let mut exit_code = 0u32;
                    const STILL_ACTIVE: u32 = 259;
                    if GetExitCodeThread(handle, &mut exit_code).is_ok()
                        && exit_code != STILL_ACTIVE
                    {
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
            crate::logging::error(
                "watchdog triggering self-healing keyboard hook reinstallation...",
            );
            LAST_REINSTALL_MS.store(now_ms(), Ordering::SeqCst);
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
            LAST_EVENT_MS.store(now_ms(), Ordering::Relaxed);

            let guard = HOOK_TX
                .read()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if let Some(ref tx) = *guard {
                if tx
                    .send(KeyEvent {
                        message,
                        vk: kb.vkCode,
                        scan: kb.scanCode,
                    })
                    .is_err()
                    && ENGINE_HEALTHY.swap(false, Ordering::AcqRel)
                {
                    crate::logging::error(
                        "engine event channel disconnected; text expansion is unhealthy until restart",
                    );
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
                // Clicks and drag-selects move the caret without changing the
                // foreground HWND, so the engine must drop its typed buffer.
                request_context_reset();
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
    fn reinstall_requires_definitive_failure() {
        assert!(should_reinstall(false, 42, 10_000, 0));
        assert!(should_reinstall(true, 0, 10_000, 0));
        assert!(!should_reinstall(true, 42, 10_000, 0));
    }

    #[test]
    fn reinstall_cooldown_blocks_storm() {
        // Thread is gone, but we reinstalled 5s ago: wait, do not storm.
        assert!(!should_reinstall(false, 42, 15_000, 10_000));
        // Cooldown elapsed: allowed again.
        assert!(should_reinstall(false, 42, 45_000, 10_000));
    }

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
