//! Global keyboard capture via a WH_KEYBOARD_LL low-level hook.
//!
//! The hook lives on its own thread running a Windows message pump (required
//! for low-level hooks). The callback does the absolute minimum: drop injected
//! events (our own SendInput output), then forward the key to the engine over
//! a channel. Each callback waits only for the previous event to finish, which
//! preserves input ordering without doing expansion work on the hook thread.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Condvar, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, SetWindowsHookExW, TranslateMessage, HC_ACTION,
    HHOOK, KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN, WM_LBUTTONDOWN,
    WM_MBUTTONDOWN, WM_RBUTTONDOWN, WM_SYSKEYDOWN, WM_XBUTTONDOWN,
};

/// One raw key transition forwarded from the hook to the engine.
pub struct KeyEvent {
    pub sequence: u64,
    pub message: u32,
    pub vk: u32,
    pub scan: u32,
}

static HOOK_TX: OnceLock<Sender<KeyEvent>> = OnceLock::new();
static NEXT_SEQUENCE: AtomicU64 = AtomicU64::new(1);
static HOOK_READY: AtomicBool = AtomicBool::new(false);
static EVENT_PROGRESS: EventProgress = EventProgress::new();
const ENGINE_ORDER_TIMEOUT: Duration = Duration::from_millis(100);

struct EventProgress {
    processed: Mutex<u64>,
    changed: Condvar,
}

impl EventProgress {
    const fn new() -> Self {
        Self {
            processed: Mutex::new(0),
            changed: Condvar::new(),
        }
    }

    fn wait_for(&self, sequence: u64, timeout: Duration) -> bool {
        let processed = self
            .processed
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let (processed, _) = self
            .changed
            .wait_timeout_while(processed, timeout, |value| *value < sequence)
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *processed >= sequence
    }

    fn mark(&self, sequence: u64) {
        let mut processed = self
            .processed
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if sequence > *processed {
            *processed = sequence;
            self.changed.notify_all();
        }
    }
}

pub(crate) fn mark_processed(sequence: u64) {
    EVENT_PROGRESS.mark(sequence);
}

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
        HOOK_READY.store(true, Ordering::Release);
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
            let message = wparam.0 as u32;
            if message == WM_KEYDOWN || message == WM_SYSKEYDOWN {
                crate::expansion::cancel_active_typeout();
            }
            let sequence = NEXT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            if sequence > 1 {
                let _ = EVENT_PROGRESS.wait_for(sequence - 1, ENGINE_ORDER_TIMEOUT);
            }
            if let Some(tx) = HOOK_TX.get() {
                if tx
                    .send(KeyEvent {
                        sequence,
                        message,
                        vk: kb.vkCode,
                        scan: kb.scanCode,
                    })
                    .is_err()
                {
                    mark_processed(sequence);
                }
            } else {
                mark_processed(sequence);
            }
        }
    }
    CallNextHookEx(HHOOK::default(), code, wparam, lparam)
}

#[cfg(test)]
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
    use std::sync::Arc;
    use std::time::Instant;

    #[test]
    fn event_progress_waits_for_engine_acknowledgement() {
        let progress = Arc::new(EventProgress::new());
        let worker = progress.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(10));
            worker.mark(7);
        });

        let started = Instant::now();
        assert!(progress.wait_for(7, Duration::from_millis(100)));
        assert!(started.elapsed() >= Duration::from_millis(5));
    }
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
