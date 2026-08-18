//! Expansion: delete the trigger, then insert the replacement. Three modes:
//!
//! - `Auto` (default): type snippets of 15 words or fewer; paste anything
//!   longer.
//! - `Paste`: save the clipboard (every byte-copyable format), set it to the
//!   expansion, send the configured paste combo, restore. Fast and exact for
//!   any length or content.
//! - `Type`: type every expansion at the configured WPM. Terminal hosts replay
//!   layout-aware virtual keys instead of Unicode input for compatibility.
//!
//! All insertion policy (mode, WPM, paste combo, restore delay) lives here
//! as atomics: set from persisted settings at startup, live from the UI via
//! IPC, and read on every expansion.

mod clipboard;
#[cfg(test)]
mod e2e_tests;
mod inject;

use std::sync::atomic::{AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// Typing speed for type-out insertion, in words per minute (standard
/// 5-character word). Read by `inject::type_unicode` on every character.
static WPM: AtomicU32 = AtomicU32::new(600);

pub const WPM_MIN: u32 = 100;
pub const WPM_MAX: u32 = 1500;

pub fn set_wpm(wpm: u32) {
    WPM.store(wpm.clamp(WPM_MIN, WPM_MAX), Ordering::Relaxed);
}

pub fn wpm() -> u32 {
    WPM.load(Ordering::Relaxed)
}

/// Per-character pause: 60s / (5 chars * wpm).
pub(crate) fn char_delay() -> Duration {
    Duration::from_micros(12_000_000 / wpm() as u64)
}

/// How expansions are inserted. Auto types snippets of 15 words or fewer and
/// pastes anything longer.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InsertMode {
    #[default]
    Auto,
    Paste,
    Type,
}

/// The keystroke sent to make the target app paste. Terminals often bind
/// Ctrl+V to something else and paste on Shift+Insert or Ctrl+Shift+V.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PasteCombo {
    #[default]
    CtrlV,
    ShiftInsert,
    CtrlShiftV,
}

static INSERT_MODE: AtomicU8 = AtomicU8::new(0);
static PASTE_COMBO: AtomicU8 = AtomicU8::new(0);
static RESTORE_DELAY: AtomicU32 = AtomicU32::new(5_000);
static CANCEL_EPOCH: AtomicU64 = AtomicU64::new(0);
static CLIPBOARD_GENERATION: AtomicU64 = AtomicU64::new(0);
static EXPANSION_LOCK: Mutex<()> = Mutex::new(());
static ACTIVE_CLIPBOARD: Mutex<Option<ClipboardRestore>> = Mutex::new(None);
static RESTORE_TX: OnceLock<Option<Sender<RestoreRequest>>> = OnceLock::new();

pub const RESTORE_DELAY_MIN_MS: u32 = 3_000;
pub const RESTORE_DELAY_MAX_MS: u32 = 15_000;

/// Extra delay when restoring a clipboard that had image/file/HTML-like
/// formats. Some apps consume paste asynchronously; restoring too early can
/// make them process the old clipboard instead of HyperType's text.
const RICH_CLIPBOARD_RESTORE_MIN_MS: u32 = 5_000;
const SENSITIVE_TARGET_RESTORE_MIN_MS: u32 = 12_000;
const LONG_TEXT_RESTORE_STEP_CHARS: usize = 500;
const LONG_TEXT_RESTORE_STEP_MS: u32 = 1_000;
const LONG_TEXT_RESTORE_MAX_EXTRA_MS: u32 = 3_000;
const AUTO_TYPE_MAX_WORDS: usize = 15;
const LONG_TEXT_RESTORE_BASE_CHARS: usize = 110;

pub fn set_insert_mode(mode: InsertMode) {
    INSERT_MODE.store(mode as u8, Ordering::Relaxed);
}

pub fn insert_mode() -> InsertMode {
    match INSERT_MODE.load(Ordering::Relaxed) {
        1 => InsertMode::Paste,
        2 => InsertMode::Type,
        _ => InsertMode::Auto,
    }
}

pub fn set_paste_combo(combo: PasteCombo) {
    PASTE_COMBO.store(combo as u8, Ordering::Relaxed);
}

pub fn paste_combo() -> PasteCombo {
    match PASTE_COMBO.load(Ordering::Relaxed) {
        1 => PasteCombo::ShiftInsert,
        2 => PasteCombo::CtrlShiftV,
        _ => PasteCombo::CtrlV,
    }
}

pub fn set_restore_delay_ms(ms: u32) {
    RESTORE_DELAY.store(
        ms.clamp(RESTORE_DELAY_MIN_MS, RESTORE_DELAY_MAX_MS),
        Ordering::Relaxed,
    );
}

pub fn restore_delay_ms() -> u32 {
    RESTORE_DELAY.load(Ordering::Relaxed)
}

pub fn cancel_active_typeout() {
    CANCEL_EPOCH.fetch_add(1, Ordering::Relaxed);
}

pub(crate) fn cancel_epoch() -> u64 {
    CANCEL_EPOCH.load(Ordering::Relaxed)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    Clipboard,
    Native,
    KeyReplay,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum TargetKind {
    Standard,
    Codex,
    Terminal(PasteCombo),
}

struct ClipboardRestore {
    original: clipboard::Snapshot,
    sequence: u32,
    generation: u64,
}

#[derive(Clone, Copy)]
struct RestoreRequest {
    generation: u64,
    sequence: u32,
    delay: Duration,
}

fn word_count(text: &str) -> usize {
    text.split_whitespace().count()
}

/// Pure mode decision, separated from the global so it can be unit-tested.
#[cfg(test)]
fn resolve(mode: InsertMode, text: &str) -> Mode {
    resolve_for_target(mode, text, TargetKind::Standard)
}

fn resolve_for_target(mode: InsertMode, text: &str, target: TargetKind) -> Mode {
    match mode {
        InsertMode::Paste => Mode::Clipboard,
        InsertMode::Type => typing_mode_for_target(target),
        InsertMode::Auto if word_count(text) <= AUTO_TYPE_MAX_WORDS => {
            typing_mode_for_target(target)
        }
        InsertMode::Auto => Mode::Clipboard,
    }
}

fn typing_mode_for_target(target: TargetKind) -> Mode {
    if matches!(target, TargetKind::Terminal(_)) {
        Mode::KeyReplay
    } else {
        Mode::Native
    }
}

pub fn expand(trigger_char_len: usize, text: &str) {
    // Shortcut callbacks and the keyboard engine run on different threads.
    // Serialize SendInput sequences so simultaneous triggers cannot interleave
    // backspaces, modifiers, or replacement text in the foreground app.
    let _expansion = EXPANSION_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let target = crate::platform::foreground_context();
    let target_kind = classify_target(&target);
    match resolve_for_target(insert_mode(), text, target_kind) {
        Mode::Clipboard => {
            inject::delete_trigger(trigger_char_len);
            paste_via_clipboard(text, &target, target_kind);
        }
        Mode::Native => inject::replace_with_unicode(trigger_char_len, text),
        Mode::KeyReplay => inject::replace_with_virtual_keys(trigger_char_len, text),
    }
}

fn paste_via_clipboard(
    text: &str,
    target: &crate::platform::ForegroundContext,
    target_kind: TargetKind,
) {
    let (generation, seq, saved_contains_non_plain_text) = {
        let mut active = ACTIVE_CLIPBOARD
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let current_sequence = clipboard::sequence_number();

        // A second expansion may happen before the first delayed restore. If
        // the clipboard is still ours, carry the original user snapshot
        // forward instead of snapshotting the previous expansion text.
        let saved = match active.take() {
            Some(previous) if previous.sequence == current_sequence => previous.original,
            _ => clipboard::snapshot(),
        };

        if !clipboard::set_unicode_text(text) {
            // EmptyClipboard may have succeeded before allocation failed, so
            // restore immediately before falling back to native typing.
            clipboard::restore(&saved);
            drop(active);
            crate::logging::error("clipboard unavailable; expanding via direct typing");
            inject::type_unicode(text);
            return;
        }

        let seq = clipboard::sequence_number();
        let generation = CLIPBOARD_GENERATION
            .fetch_add(1, Ordering::Relaxed)
            .wrapping_add(1);
        let saved_contains_non_plain_text = saved.contains_non_plain_text();
        *active = Some(ClipboardRestore {
            original: saved,
            sequence: seq,
            generation,
        });
        (generation, seq, saved_contains_non_plain_text)
    };

    let configured_combo = paste_combo();
    let combo = effective_paste_combo(configured_combo, target_kind);
    if combo != configured_combo {
        crate::logging::info(&format!(
            "using {:?} paste for target {:?} / {:?} / {:?}",
            combo, target.title, target.class_name, target.process_name
        ));
    }
    inject::send_paste(combo);
    // The dedicated restore worker keeps expansion non-blocking. A changed
    // sequence number means someone else claimed the clipboard after us, so
    // it is no longer ours to restore.
    let restore_delay = restore_delay_for(
        saved_contains_non_plain_text,
        text.chars().count(),
        target_kind != TargetKind::Standard,
    );
    schedule_restore(RestoreRequest {
        generation,
        sequence: seq,
        delay: Duration::from_millis(restore_delay as u64),
    });
}

fn schedule_restore(request: RestoreRequest) {
    let tx = RESTORE_TX.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        match std::thread::Builder::new()
            .name("hypertype-clipboard-restore".to_string())
            .spawn(move || restore_worker(rx))
        {
            Ok(_) => Some(tx),
            Err(e) => {
                crate::logging::error(&format!("failed to start clipboard restore worker: {e}"));
                None
            }
        }
    });

    if let Some(tx) = tx {
        match tx.send(request) {
            Ok(()) => (),
            Err(e) => {
                crate::logging::error("clipboard restore worker stopped unexpectedly");
                schedule_fallback_restore(e.0);
            }
        }
    } else {
        schedule_fallback_restore(request);
    }
}

fn schedule_fallback_restore(request: RestoreRequest) {
    let queued = request;
    if let Err(e) = std::thread::Builder::new()
        .name("hypertype-clipboard-restore-fallback".to_string())
        .spawn(move || {
            std::thread::sleep(queued.delay);
            restore_clipboard(queued);
        })
    {
        crate::logging::error(&format!("failed to schedule clipboard restore: {e}"));
        restore_clipboard(request);
    }
}

fn restore_worker(rx: Receiver<RestoreRequest>) {
    let mut pending: Option<(RestoreRequest, Instant)> = None;
    loop {
        if let Some((request, deadline)) = pending.take() {
            match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                Ok(next) => {
                    let deadline = Instant::now() + next.delay;
                    pending = Some((next, deadline));
                }
                Err(RecvTimeoutError::Timeout) => restore_clipboard(request),
                Err(RecvTimeoutError::Disconnected) => break,
            }
        } else {
            match rx.recv() {
                Ok(request) => {
                    let deadline = Instant::now() + request.delay;
                    pending = Some((request, deadline));
                }
                Err(_) => break,
            }
        }
    }
}

fn restore_clipboard(request: RestoreRequest) {
    let mut active = ACTIVE_CLIPBOARD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if active.as_ref().map(|pending| pending.generation) != Some(request.generation) {
        return;
    }
    let Some(pending) = active.take() else {
        return;
    };
    if clipboard::sequence_number() == request.sequence {
        clipboard::restore(&pending.original);
    }
}

fn effective_paste_combo(configured: PasteCombo, target: TargetKind) -> PasteCombo {
    if configured != PasteCombo::CtrlV {
        return configured;
    }
    match target {
        TargetKind::Terminal(preferred) => preferred,
        // Codex reserves Ctrl+V for image attachment. Ctrl+Shift+V remains a
        // text paste in browser/WebView input controls.
        TargetKind::Codex => PasteCombo::CtrlShiftV,
        TargetKind::Standard => PasteCombo::CtrlV,
    }
}

fn classify_target(target: &crate::platform::ForegroundContext) -> TargetKind {
    classify_target_parts(&target.title, &target.class_name, &target.process_name)
}

fn classify_target_parts(title: &str, class_name: &str, process_name: &str) -> TargetKind {
    let title = title.to_ascii_lowercase();
    let class_name = class_name.to_ascii_lowercase();
    let process_name = process_name.to_ascii_lowercase();

    let terminal_process = matches!(
        process_name.as_str(),
        "windowsterminal.exe"
            | "wezterm-gui.exe"
            | "wezterm.exe"
            | "alacritty.exe"
            | "kitty.exe"
            | "kitty_portable.exe"
            | "mintty.exe"
            | "conhost.exe"
            | "conemu.exe"
            | "conemu64.exe"
            | "cmder.exe"
            | "tabby.exe"
            | "hyper.exe"
            | "waveterm.exe"
            | "wave.exe"
            | "warp.exe"
            | "rio.exe"
            | "contour.exe"
            | "fluentterminal.exe"
            | "extraterm.exe"
    );
    let terminal_class = class_name.contains("cascadia_hosting_window_class")
        || class_name.contains("consolewindowclass")
        || class_name.contains("mintty")
        || class_name.contains("virtualconsoleclass")
        || class_name.contains("org.wezfurlong.wezterm")
        || class_name.contains("alacritty")
        || class_name.contains("kitty")
        || class_name.contains("conemu");
    let terminal_title = title.contains("windows terminal")
        || title.contains("command prompt")
        || title.contains("powershell")
        || title.contains("pwsh")
        || title.contains("terminal");

    if terminal_process || terminal_class || terminal_title {
        let classic_console = process_name == "mintty.exe"
            || process_name == "conhost.exe"
            || class_name.contains("consolewindowclass")
            || class_name.contains("mintty");
        return TargetKind::Terminal(if classic_console {
            PasteCombo::ShiftInsert
        } else {
            PasteCombo::CtrlShiftV
        });
    }

    if process_name == "codex.exe" || title.contains("codex") {
        TargetKind::Codex
    } else {
        TargetKind::Standard
    }
}

fn long_text_restore_extra_ms(text_char_len: usize) -> u32 {
    let extra_chars = text_char_len.saturating_sub(LONG_TEXT_RESTORE_BASE_CHARS);
    let steps = extra_chars.div_ceil(LONG_TEXT_RESTORE_STEP_CHARS) as u32;
    steps
        .saturating_mul(LONG_TEXT_RESTORE_STEP_MS)
        .min(LONG_TEXT_RESTORE_MAX_EXTRA_MS)
}

fn restore_delay_for(
    saved_contains_non_plain_text: bool,
    text_char_len: usize,
    sensitive_target: bool,
) -> u32 {
    restore_delay_from(
        restore_delay_ms(),
        saved_contains_non_plain_text,
        text_char_len,
        sensitive_target,
    )
}

fn restore_delay_from(
    configured_delay_ms: u32,
    saved_contains_non_plain_text: bool,
    text_char_len: usize,
    sensitive_target: bool,
) -> u32 {
    let mut base = configured_delay_ms;
    if saved_contains_non_plain_text {
        base = base.max(RICH_CLIPBOARD_RESTORE_MIN_MS);
    }
    if sensitive_target {
        base = base.max(SENSITIVE_TARGET_RESTORE_MIN_MS);
    }
    base.saturating_add(long_text_restore_extra_ms(text_char_len))
        .min(RESTORE_DELAY_MAX_MS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_modes_ignore_content() {
        assert_eq!(resolve(InsertMode::Paste, "hi"), Mode::Clipboard);
        assert_eq!(resolve(InsertMode::Type, &"x".repeat(500)), Mode::Native);
        assert_eq!(
            resolve(
                InsertMode::Paste,
                "one two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen sixteen"
            ),
            Mode::Clipboard
        );
    }

    #[test]
    fn auto_types_fifteen_words_or_fewer() {
        assert_eq!(resolve(InsertMode::Auto, "Good morning"), Mode::Native);
        assert_eq!(
            resolve(
                InsertMode::Auto,
                "one two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen"
            ),
            Mode::Native
        );
    }

    #[test]
    fn auto_pastes_more_than_fifteen_words() {
        assert_eq!(
            resolve(
                InsertMode::Auto,
                "one two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen sixteen"
            ),
            Mode::Clipboard
        );
    }

    #[test]
    fn auto_counts_words_across_whitespace_and_punctuation() {
        assert_eq!(
            resolve(InsertMode::Auto, "First sentence.\nSecond sentence!"),
            Mode::Native
        );
        assert_eq!(word_count("  hello\tworld\r\nagain  "), 3);
    }

    #[test]
    fn auto_uses_word_count_not_character_count() {
        assert_eq!(resolve(InsertMode::Auto, &"x".repeat(5_000)), Mode::Native);
    }

    #[test]
    fn explicit_type_types_structured_text() {
        assert_eq!(
            resolve(InsertMode::Type, "First line.\nSecond line.\tTabbed"),
            Mode::Native
        );
    }

    #[test]
    fn codex_ui_avoids_its_ctrl_v_image_shortcut() {
        let target = classify_target_parts("Codex", "Chrome_WidgetWin_1", "Codex.exe");
        assert_eq!(target, TargetKind::Codex);
        assert_eq!(
            resolve_for_target(
                InsertMode::Auto,
                "one two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen sixteen",
                target
            ),
            Mode::Clipboard
        );
        assert_eq!(
            resolve_for_target(InsertMode::Paste, &"x".repeat(500), target),
            Mode::Clipboard
        );
        assert_eq!(
            effective_paste_combo(PasteCombo::CtrlV, target),
            PasteCombo::CtrlShiftV
        );
    }

    #[test]
    fn restore_delay_is_clamped() {
        set_restore_delay_ms(1);
        assert_eq!(restore_delay_ms(), RESTORE_DELAY_MIN_MS);
        set_restore_delay_ms(99_999);
        assert_eq!(restore_delay_ms(), RESTORE_DELAY_MAX_MS);
        set_restore_delay_ms(5_000);
    }

    #[test]
    fn rich_clipboard_uses_longer_restore_delay() {
        assert_eq!(restore_delay_from(3_000, false, 20, false), 3_000);
        assert_eq!(
            restore_delay_from(3_000, true, 20, false),
            RICH_CLIPBOARD_RESTORE_MIN_MS
        );
    }

    #[test]
    fn wezterm_respects_all_three_insert_modes() {
        let target = classify_target_parts(
            "[2/2] accountmanagement",
            "org.wezfurlong.wezterm",
            "wezterm-gui.exe",
        );
        assert_eq!(target, TargetKind::Terminal(PasteCombo::CtrlShiftV));
        assert_eq!(
            resolve_for_target(InsertMode::Auto, "short text", target),
            Mode::KeyReplay
        );
        assert_eq!(
            resolve_for_target(InsertMode::Type, "short text", target),
            Mode::KeyReplay
        );
        assert_eq!(
            resolve_for_target(InsertMode::Type, "first line\nsecond line", target),
            Mode::KeyReplay
        );
        assert_eq!(
            resolve_for_target(InsertMode::Paste, "short text", target),
            Mode::Clipboard
        );
        assert_eq!(
            effective_paste_combo(PasteCombo::CtrlV, target),
            PasteCombo::CtrlShiftV
        );
        assert_eq!(
            effective_paste_combo(PasteCombo::ShiftInsert, target),
            PasteCombo::ShiftInsert
        );
    }

    #[test]
    fn modern_and_classic_terminals_get_compatible_default_chords() {
        let windows_terminal = classify_target_parts(
            "PowerShell",
            "CASCADIA_HOSTING_WINDOW_CLASS",
            "WindowsTerminal.exe",
        );
        assert_eq!(
            windows_terminal,
            TargetKind::Terminal(PasteCombo::CtrlShiftV)
        );
        assert_eq!(
            effective_paste_combo(PasteCombo::CtrlV, windows_terminal),
            PasteCombo::CtrlShiftV
        );

        let classic = classify_target_parts("Command Prompt", "ConsoleWindowClass", "conhost.exe");
        assert_eq!(classic, TargetKind::Terminal(PasteCombo::ShiftInsert));
        assert_eq!(
            effective_paste_combo(PasteCombo::CtrlV, classic),
            PasteCombo::ShiftInsert
        );
        assert_eq!(
            restore_delay_from(3_000, false, 20, true),
            SENSITIVE_TARGET_RESTORE_MIN_MS
        );
    }

    #[test]
    fn normal_apps_keep_existing_auto_and_ctrl_v_behavior() {
        let target = classify_target_parts("Notes", "Notepad", "notepad.exe");
        assert_eq!(target, TargetKind::Standard);
        assert_eq!(
            resolve_for_target(InsertMode::Auto, "Good morning", target),
            Mode::Native
        );
        assert_eq!(
            effective_paste_combo(PasteCombo::CtrlV, target),
            PasteCombo::CtrlV
        );
    }

    #[test]
    fn long_pastes_keep_text_clipboard_alive_longer() {
        assert_eq!(restore_delay_from(12_000, false, 1_200, true), 15_000);
    }
}
