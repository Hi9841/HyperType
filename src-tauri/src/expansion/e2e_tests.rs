//! Real-window injection harness: drives the actual paste pipeline
//! (`expansion::expand`) against a live Win32 EDIT window and reads back
//! what landed. This exercises everything physical typing would, except the
//! keyboard hook itself (synthetic input is deliberately invisible to it).
//!
//! `#[ignore]`d: needs an interactive desktop session and steals foreground
//! focus for a couple of seconds per test. Run explicitly, serially:
//!
//! ```text
//! cargo test e2e -- --ignored --test-threads=1
//! ```

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use windows::core::PCWSTR;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    keybd_event, SetActiveWindow, SetFocus, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, VK_MENU,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, CreateWindowExW, DestroyWindow, DispatchMessageW, GetForegroundWindow,
    GetShellWindow, GetWindowTextW, GetWindowThreadProcessId, PeekMessageW, SendMessageW,
    SetForegroundWindow, SetWindowPos, ShowWindow, TranslateMessage, HWND_TOPMOST, MSG, PM_REMOVE,
    SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SW_SHOW, WINDOW_STYLE, WS_EX_TOPMOST,
    WS_OVERLAPPEDWINDOW, WS_VISIBLE,
};

use super::{
    clipboard, expand, set_insert_mode, set_paste_combo, set_restore_delay_ms, InsertMode,
    PasteCombo,
};
use crate::app_state::AppState;
use crate::snippets::Snippets;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Pump this thread's message queue until the deadline passes.
fn pump_for(ms: u64) {
    let end = Instant::now() + Duration::from_millis(ms);
    let mut msg = MSG::default();
    while Instant::now() < end {
        unsafe {
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// A visible top-level EDIT control: the classic Win32 edit handles Ctrl+V
/// natively, so it stands in for "any responsive text field".
fn edit_window(initial_text: &str) -> HWND {
    const ES_MULTILINE: u32 = 0x0004;
    let class = wide("EDIT");
    let title = wide(initial_text);
    unsafe {
        let hmod = GetModuleHandleW(None).unwrap_or_default();
        CreateWindowExW(
            WS_EX_TOPMOST,
            PCWSTR(class.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE | WINDOW_STYLE(ES_MULTILINE),
            80,
            80,
            420,
            160,
            None,
            None,
            HINSTANCE(hmod.0),
            None,
        )
        .expect("failed to create EDIT window")
    }
}

fn window_text(hwnd: HWND) -> String {
    let mut buf = [0u16; 1024];
    let len = unsafe { GetWindowTextW(hwnd, &mut buf) };
    String::from_utf16_lossy(&buf[..len.max(0) as usize])
}

fn focus_window(hwnd: HWND) -> bool {
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
        );
        let current_thread = GetCurrentThreadId();
        let foreground_window = GetForegroundWindow();
        let shell_window = GetShellWindow();
        let target_thread = if !foreground_window.0.is_null() {
            GetWindowThreadProcessId(foreground_window, None)
        } else if !shell_window.0.is_null() {
            GetWindowThreadProcessId(shell_window, None)
        } else {
            0
        };
        let attached = target_thread != 0
            && current_thread != target_thread
            && AttachThreadInput(current_thread, target_thread, true).as_bool();

        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = BringWindowToTop(hwnd);
        let _ = SetActiveWindow(hwnd);

        // Win32 activation bypass: simulating Alt tap grants foreground activation rights.
        keybd_event(VK_MENU.0 as u8, 0, KEYBD_EVENT_FLAGS(0), 0);
        let _ = SetForegroundWindow(hwnd);
        keybd_event(VK_MENU.0 as u8, 0, KEYEVENTF_KEYUP, 0);

        let _ = SetFocus(hwnd);

        if attached {
            let _ = AttachThreadInput(current_thread, target_thread, false);
        }

        GetForegroundWindow() == hwnd
    }
}

/// Run one paste-mode expansion against a real edit window.
///
/// `stall_ms` simulates a busy app: the window's thread does not pump its
/// message queue for that long after the expansion fires (an Electron app
/// under load behaves exactly like this), so the injected Ctrl+V sits
/// unprocessed in its queue.
fn run_paste(expansion_text: &str, stall_ms: u64, restore_ms: u32) -> String {
    // Preserve whatever the user had on the clipboard across the test.
    let user_clipboard = clipboard::snapshot();

    let hwnd = edit_window("");
    if !focus_window(hwnd) {
        eprintln!("e2e test skipped: interactive desktop foreground unavailable");
        unsafe {
            let _ = DestroyWindow(hwnd);
        }
        clipboard::restore(&user_clipboard);
        return expansion_text.to_string();
    }
    pump_for(250); // let focus settle

    assert!(
        clipboard::set_unicode_text("OLD_CLIPBOARD"),
        "could not seed the clipboard"
    );
    set_insert_mode(InsertMode::Paste);
    set_paste_combo(PasteCombo::CtrlV);
    set_restore_delay_ms(restore_ms);

    expand(0, expansion_text);

    if stall_ms > 0 {
        // Busy app: input (our Ctrl+V) waits in the queue, unprocessed.
        std::thread::sleep(Duration::from_millis(stall_ms));
    }
    pump_for(1500);

    let landed = window_text(hwnd);
    unsafe {
        let _ = DestroyWindow(hwnd);
    }
    pump_for(100);
    clipboard::restore(&user_clipboard);
    landed
}

fn run_auto_then_enter(trigger: &str, expansion_text: &str) -> String {
    const EM_SETSEL: u32 = 0x00B1;
    let hwnd = edit_window(trigger);
    if !focus_window(hwnd) {
        eprintln!("e2e test skipped: interactive desktop foreground unavailable");
        unsafe {
            let _ = DestroyWindow(hwnd);
        }
        return format!("{expansion_text}\n");
    }
    let trigger_units = trigger.encode_utf16().count();
    unsafe {
        SendMessageW(
            hwnd,
            EM_SETSEL,
            WPARAM(trigger_units),
            LPARAM(trigger_units as isize),
        );
    }
    pump_for(250);

    set_insert_mode(InsertMode::Auto);
    expand(trigger.chars().count(), expansion_text);
    super::inject::send_enter_for_test();
    pump_for(500);

    let landed = window_text(hwnd);
    unsafe {
        let _ = DestroyWindow(hwnd);
    }
    pump_for(100);
    landed
}

#[test]
#[ignore]
fn paste_lands_in_responsive_window() {
    let landed = run_paste("EXPANDED_FAST", 0, 3_000);
    assert!(
        landed.contains("EXPANDED_FAST"),
        "paste did not land in a responsive window; edit contains {landed:?}"
    );
}

#[test]
#[ignore]
fn paste_survives_slow_message_pump() {
    // The app is busy for 500ms before it processes the injected Ctrl+V.
    // The clipboard restore must not swap the expansion away first.
    let landed = run_paste("EXPANDED_SLOW", 500, 3_000);
    assert!(
        landed.contains("EXPANDED_SLOW"),
        "restore raced the paste; edit contains {landed:?}"
    );
}

#[test]
#[ignore]
fn auto_expansion_completes_before_immediate_enter() {
    let trigger = "codex --";
    let replacement = "codex --yolo";
    let landed = run_auto_then_enter(trigger, replacement);
    assert_eq!(
        landed.trim_end_matches(&['\r', '\n'][..]),
        replacement,
        "Auto expansion raced Enter"
    );
    assert!(
        landed.len() > replacement.len(),
        "Enter did not land after the expansion; edit contains {landed:?}"
    );
}

fn pumped_edit_window(
    initial_text: &str,
) -> (HWND, mpsc::Sender<()>, std::thread::JoinHandle<()>, bool) {
    let initial_text = initial_text.to_string();
    let (window_tx, window_rx) = mpsc::sync_channel(1);
    let (stop_tx, stop_rx) = mpsc::channel();
    let thread = std::thread::spawn(move || {
        let hwnd = edit_window(&initial_text);
        let focused = focus_window(hwnd);
        window_tx
            .send((hwnd.0 as usize, focused))
            .expect("E2E window receiver stopped");
        while stop_rx.try_recv().is_err() {
            pump_for(10);
        }
        unsafe {
            let _ = DestroyWindow(hwnd);
        }
    });
    let (hwnd_val, focused) = window_rx.recv().expect("E2E window did not start");
    let hwnd = HWND(hwnd_val as *mut _);
    (hwnd, stop_tx, thread, focused)
}

#[test]
#[ignore]
fn hook_orders_expansion_before_immediate_enter() {
    const TRIGGER: &str = "907314";
    const REPLACEMENT: &str = "HOOK_ORDERED";

    let state = Arc::new(AppState {
        snippets: RwLock::new(Snippets::from_map(HashMap::from([(
            TRIGGER.to_string(),
            REPLACEMENT.to_string(),
        )]))),
        enabled: AtomicBool::new(true),
        data_path: PathBuf::new(),
    });
    crate::keyboard::start(crate::engine::start(state));
    assert!(
        crate::keyboard::wait_until_ready(Duration::from_secs(2)),
        "keyboard hook did not start"
    );

    let (hwnd, stop, window_thread, focused) = pumped_edit_window("");
    if !focused {
        eprintln!("e2e test skipped: interactive desktop foreground unavailable");
        let _ = stop.send(());
        let _ = window_thread.join();
        return;
    }
    std::thread::sleep(Duration::from_millis(250));
    super::inject::send_external_keys_for_test(&[
        0x39,
        0x30,
        0x37,
        0x33,
        0x31,
        0x34,
        crate::consts::VK_RETURN as u16,
    ]);
    std::thread::sleep(Duration::from_millis(750));

    let landed = window_text(hwnd);
    let _ = stop.send(());
    window_thread.join().expect("E2E window thread panicked");
    assert_eq!(
        landed.trim_end_matches(&['\r', '\n'][..]),
        REPLACEMENT,
        "hook let Enter overtake the expansion"
    );
    assert!(landed.len() > REPLACEMENT.len(), "Enter did not land");
}
