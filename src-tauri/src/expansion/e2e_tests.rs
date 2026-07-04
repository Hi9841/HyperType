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

use std::time::{Duration, Instant};

use windows::core::PCWSTR;
use windows::Win32::Foundation::{HINSTANCE, HWND};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, DispatchMessageW, GetWindowTextW, PeekMessageW,
    SetForegroundWindow, TranslateMessage, MSG, PM_REMOVE, WINDOW_EX_STYLE, WINDOW_STYLE,
    WS_OVERLAPPEDWINDOW, WS_VISIBLE,
};

use super::{
    clipboard, expand, set_insert_mode, set_paste_combo, set_restore_delay_ms, InsertMode,
    PasteCombo,
};

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
fn edit_window() -> HWND {
    const ES_MULTILINE: u32 = 0x0004;
    let class = wide("EDIT");
    let title = wide("hypertype-e2e");
    unsafe {
        let hmod = GetModuleHandleW(None).unwrap_or_default();
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
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

/// Run one paste-mode expansion against a real edit window.
///
/// `stall_ms` simulates a busy app: the window's thread does not pump its
/// message queue for that long after the expansion fires (an Electron app
/// under load behaves exactly like this), so the injected Ctrl+V sits
/// unprocessed in its queue.
fn run_paste(expansion_text: &str, stall_ms: u64, restore_ms: u32) -> String {
    // Preserve whatever the user had on the clipboard across the test.
    let user_clipboard = clipboard::snapshot();

    let hwnd = edit_window();
    unsafe {
        let _ = SetForegroundWindow(hwnd);
        let _ = SetFocus(hwnd);
    }
    pump_for(250); // let focus settle

    assert!(
        clipboard::set_unicode_text("OLD_CLIPBOARD"),
        "could not seed the clipboard"
    );
    set_insert_mode(InsertMode::Paste);
    set_paste_combo(PasteCombo::CtrlV);
    set_restore_delay_ms(restore_ms);

    let text = expansion_text.to_string();
    std::thread::spawn(move || expand(0, &text))
        .join()
        .expect("expand panicked");

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
