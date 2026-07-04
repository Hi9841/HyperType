//! Expansion: delete the trigger, then insert the replacement. Three modes:
//!
//! - `Auto` (default): per-expansion choice — type short, single-sentence
//!   snippets; paste anything long, multiline, or multi-sentence.
//! - `Paste`: save the clipboard (every byte-copyable format), set it to the
//!   expansion, send the configured paste combo, restore. Fast and exact for
//!   any length or content.
//! - `Type`: type the expansion as Unicode keystrokes at the configured WPM.
//!   Does not touch the clipboard.
//!
//! All insertion policy (mode, WPM, paste combo, restore delay) lives here
//! as atomics: set from persisted settings at startup, live from the UI via
//! IPC, and read on every expansion.

mod clipboard;
#[cfg(test)]
mod e2e_tests;
mod inject;

use std::sync::atomic::{AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::time::Duration;

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

/// How expansions are inserted. Auto picks per-expansion: type short,
/// single-sentence snippets; paste anything long, multiline, or
/// multi-sentence.
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
const AUTO_TYPE_MAX_CHARS: usize = 110;

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
}

fn is_simple_sentence(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty()
        || trimmed.contains(['\r', '\n', '\t'])
        || trimmed.chars().count() > AUTO_TYPE_MAX_CHARS
    {
        return false;
    }

    trimmed
        .chars()
        .filter(|ch| matches!(ch, '.' | '!' | '?'))
        .count()
        <= 1
}

/// Pure mode decision, separated from the global so it can be unit-tested.
#[cfg(test)]
fn resolve(mode: InsertMode, text: &str) -> Mode {
    resolve_for_target(mode, text, false)
}

fn resolve_for_target(mode: InsertMode, text: &str, codex_target: bool) -> Mode {
    match mode {
        InsertMode::Paste => Mode::Clipboard,
        InsertMode::Type => Mode::Native,
        InsertMode::Auto => {
            if codex_target || is_simple_sentence(text) {
                Mode::Native
            } else {
                Mode::Clipboard
            }
        }
    }
}

pub fn expand(trigger_char_len: usize, text: &str) {
    let target = crate::platform::foreground_context();
    match resolve_for_target(insert_mode(), text, is_codex_target(&target)) {
        Mode::Clipboard => {
            inject::delete_trigger(trigger_char_len);
            paste_via_clipboard(text, &target);
        }
        Mode::Native => inject::replace_with_unicode(trigger_char_len, text),
    }
}

fn paste_via_clipboard(text: &str, target: &crate::platform::ForegroundContext) {
    // Snapshot everything byte-copyable (text, HTML, images, file lists) so
    // the user's clipboard survives the expansion intact.
    let saved = clipboard::snapshot();
    if !clipboard::set_unicode_text(text) {
        // Clipboard was unavailable; fall back to typing so the user still
        // gets their expansion rather than nothing.
        crate::logging::error("clipboard unavailable; expanding via direct typing");
        inject::type_unicode(text);
        return;
    }
    let seq = clipboard::sequence_number();
    let configured_combo = paste_combo();
    let sensitive_target = prefers_shift_insert(target);
    let combo = effective_paste_combo(configured_combo, sensitive_target);
    if combo != configured_combo {
        crate::logging::info(&format!(
            "using {:?} paste for target {:?} / {:?}",
            combo, target.title, target.class_name
        ));
    }
    inject::send_paste(combo);
    // Restore on a detached thread so expansion never blocks. The sequence
    // number moving means someone (a user copy, another app) claimed the
    // clipboard after us — then it is no longer ours to restore.
    let restore_delay = restore_delay_for(
        saved.contains_non_plain_text(),
        text.chars().count(),
        sensitive_target,
    );
    let delay = Duration::from_millis(restore_delay as u64);
    std::thread::spawn(move || {
        std::thread::sleep(delay);
        if clipboard::sequence_number() == seq {
            clipboard::restore(&saved);
        }
    });
}

fn effective_paste_combo(configured: PasteCombo, sensitive_target: bool) -> PasteCombo {
    if sensitive_target && configured == PasteCombo::CtrlV {
        PasteCombo::ShiftInsert
    } else {
        configured
    }
}

fn prefers_shift_insert(target: &crate::platform::ForegroundContext) -> bool {
    prefers_shift_insert_text(&target.title, &target.class_name)
}

fn is_codex_target(target: &crate::platform::ForegroundContext) -> bool {
    is_codex_target_text(&target.title)
}

fn is_codex_target_text(title: &str) -> bool {
    title.to_ascii_lowercase().contains("codex")
}

fn prefers_shift_insert_text(title: &str, class_name: &str) -> bool {
    let title = title.to_ascii_lowercase();
    let class_name = class_name.to_ascii_lowercase();

    is_codex_target_text(&title)
        || title.contains("windows terminal")
        || title.contains("command prompt")
        || title.contains("powershell")
        || title.contains("pwsh")
        || title.contains("terminal")
        || class_name.contains("cascadia_hosting_window_class")
        || class_name.contains("consolewindowclass")
        || class_name.contains("mintty")
        || class_name.contains("virtualconsoleclass")
}

fn long_text_restore_extra_ms(text_char_len: usize) -> u32 {
    let extra_chars = text_char_len.saturating_sub(AUTO_TYPE_MAX_CHARS);
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
    }

    #[test]
    fn auto_types_short_single_sentence() {
        assert_eq!(resolve(InsertMode::Auto, "Good morning"), Mode::Native);
        assert_eq!(
            resolve(InsertMode::Auto, "This is one sentence."),
            Mode::Native
        );
    }

    #[test]
    fn auto_pastes_long_single_line() {
        assert_eq!(resolve(InsertMode::Auto, &"x".repeat(111)), Mode::Clipboard);
    }

    #[test]
    fn auto_pastes_multiline() {
        assert_eq!(resolve(InsertMode::Auto, "Best,\nDylan"), Mode::Clipboard);
    }

    #[test]
    fn auto_pastes_multi_sentence() {
        assert_eq!(
            resolve(InsertMode::Auto, "First sentence. Second sentence."),
            Mode::Clipboard
        );
    }

    #[test]
    fn auto_types_in_codex_targets_even_for_long_text() {
        assert_eq!(
            resolve_for_target(InsertMode::Auto, &"x".repeat(500), true),
            Mode::Native
        );
        assert_eq!(
            resolve_for_target(InsertMode::Paste, &"x".repeat(500), true),
            Mode::Clipboard
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
    fn codex_like_targets_use_shift_insert_instead_of_ctrl_v() {
        assert!(prefers_shift_insert_text("Codex", "Chrome_WidgetWin_1"));
        assert_eq!(
            effective_paste_combo(PasteCombo::CtrlV, true),
            PasteCombo::ShiftInsert
        );
        assert_eq!(
            effective_paste_combo(PasteCombo::CtrlShiftV, true),
            PasteCombo::CtrlShiftV
        );
    }

    #[test]
    fn terminal_like_targets_use_longer_restore_delay() {
        assert!(prefers_shift_insert_text(
            "PowerShell",
            "CASCADIA_HOSTING_WINDOW_CLASS"
        ));
        assert_eq!(
            restore_delay_from(3_000, false, 20, true),
            SENSITIVE_TARGET_RESTORE_MIN_MS
        );
    }

    #[test]
    fn long_pastes_keep_text_clipboard_alive_longer() {
        assert_eq!(restore_delay_from(12_000, false, 1_200, true), 15_000);
    }
}
