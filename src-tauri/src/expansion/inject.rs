//! Synthetic input via SendInput. Every event we generate carries the
//! injected flag, so our own keyboard hook ignores it (no self-matching loop).

use std::mem::size_of;

use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS,
    KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, VIRTUAL_KEY, VK_BACK, VK_CONTROL, VK_INSERT, VK_LCONTROL,
    VK_LMENU, VK_LSHIFT, VK_LWIN, VK_MENU, VK_RCONTROL, VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SHIFT,
};

use super::PasteCombo;

const VK_V: u16 = 0x56;
/// Unassigned virtual key, pressed between a restored Alt/Win down and the
/// user's eventual physical release so the OS "lone modifier tap" heuristic
/// never activates the menu bar or Start menu.
const VK_MASK: u16 = 0xE8;

fn key_vk(vk: VIRTUAL_KEY, up: bool) -> INPUT {
    let mut flags = KEYBD_EVENT_FLAGS(0);
    if up {
        flags |= KEYEVENTF_KEYUP;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: crate::consts::INJECT_SIGNATURE,
            },
        },
    }
}

fn key_unicode(unit: u16, up: bool) -> INPUT {
    let mut flags = KEYEVENTF_UNICODE;
    if up {
        flags |= KEYEVENTF_KEYUP;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(0),
                wScan: unit,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: crate::consts::INJECT_SIGNATURE,
            },
        },
    }
}

fn send(inputs: &[INPUT]) {
    if inputs.is_empty() {
        return;
    }
    unsafe {
        SendInput(inputs, size_of::<INPUT>() as i32);
    }
}

fn is_down(vk: VIRTUAL_KEY) -> bool {
    unsafe { (GetAsyncKeyState(vk.0 as i32) as u16 & 0x8000) != 0 }
}

/// Delete the typed trigger: one full press per character.
pub fn backspaces(n: usize) {
    if n == 0 {
        return;
    }
    let mut inputs = Vec::with_capacity(n * 2);
    for _ in 0..n {
        inputs.push(key_vk(VK_BACK, false));
        inputs.push(key_vk(VK_BACK, true));
    }
    send(&inputs);
}

/// Every modifier key we may need to lift before injecting input. A chord's
/// modifiers are still physically held when a shortcut trigger fires, and a
/// held Ctrl/Alt/Win would turn the injected characters into accelerator
/// presses in the target app.
const ALL_MODIFIERS: [VIRTUAL_KEY; 8] = [
    VK_LCONTROL,
    VK_RCONTROL,
    VK_LSHIFT,
    VK_RSHIFT,
    VK_LMENU,
    VK_RMENU,
    VK_LWIN,
    VK_RWIN,
];

fn held_of(candidates: &[VIRTUAL_KEY]) -> Vec<u16> {
    candidates
        .iter()
        .filter(|&&vk| is_down(vk))
        .map(|vk| vk.0)
        .collect()
}

fn needs_mask(held: &[u16]) -> bool {
    held.iter()
        .any(|&vk| vk == VK_LMENU.0 || vk == VK_RMENU.0 || vk == VK_LWIN.0 || vk == VK_RWIN.0)
}

fn still_held(initially_held: &[u16], currently_held: &[u16]) -> Vec<u16> {
    initially_held
        .iter()
        .copied()
        .filter(|vk| currently_held.contains(vk))
        .collect()
}

fn restore_modifiers(held: &[u16]) {
    let mut restores: Vec<INPUT> = held
        .iter()
        .map(|&vk| key_vk(VIRTUAL_KEY(vk), false))
        .collect();
    if needs_mask(held) {
        restores.push(key_vk(VIRTUAL_KEY(VK_MASK), false));
        restores.push(key_vk(VIRTUAL_KEY(VK_MASK), true));
    }
    send(&restores);
}

fn with_modifiers_lifted<T>(work: impl FnOnce() -> T) -> T {
    let held = held_of(&ALL_MODIFIERS);

    let lifts: Vec<INPUT> = held
        .iter()
        .map(|&vk| key_vk(VIRTUAL_KEY(vk), true))
        .collect();
    send(&lifts);

    let result = work();

    let current = held_of(&ALL_MODIFIERS);
    let restore = still_held(&held, &current);
    restore_modifiers(&restore);
    result
}

/// Delete the trigger while neutralizing any modifier that was part of the
/// trigger's final keypress, then restore only keys the user still holds.
pub fn delete_trigger(n: usize) {
    if n == 0 {
        return;
    }
    with_modifiers_lifted(|| backspaces(n));
}

/// Type text as Unicode (layout-independent; supports emoji via surrogate
/// pairs since we send each UTF-16 unit). Physically-held modifiers are
/// lifted first and restored at the end, mirroring the paste path, so the
/// logical key state always matches the user's hands. Each character goes
/// out as its own SendInput call at the user's configured WPM — Keysmith-
/// style keystroke replay — which slow apps and terminals handle far more
/// reliably than one giant batch.
pub fn type_unicode(text: &str) {
    let token = super::cancel_epoch();
    let foreground = crate::platform::foreground_window();
    let focus = crate::platform::focused_control();
    let completed =
        with_modifiers_lifted(|| type_units_cancellable(text, token, foreground, focus));
    if !completed {
        crate::logging::info("type-out expansion cancelled");
    }
}

fn typeout_cancelled(token: u64, foreground: isize, focus: isize) -> bool {
    super::cancel_epoch() != token
        || crate::platform::foreground_window() != foreground
        || crate::platform::focused_control() != focus
}

fn type_units_cancellable(text: &str, token: u64, foreground: isize, focus: isize) -> bool {
    use windows::Win32::Media::{timeBeginPeriod, timeEndPeriod};

    let delay = super::char_delay();
    unsafe {
        timeBeginPeriod(1);
    }
    let mut completed = true;
    for unit in text.encode_utf16() {
        if typeout_cancelled(token, foreground, focus) {
            completed = false;
            break;
        }
        send(&[key_unicode(unit, false), key_unicode(unit, true)]);
        std::thread::sleep(delay);
    }
    unsafe {
        timeEndPeriod(1);
    }
    completed
}

/// Native replacement in one modifier-neutral section. This matters for
/// triggers ending in shifted punctuation like `?`: backspacing under Shift
/// can be interpreted differently by target apps, and restoring Shift after
/// the user has already released it leaves Enter behaving like Shift+Enter.
pub fn replace_with_unicode(trigger_char_len: usize, text: &str) {
    let token = super::cancel_epoch();
    let foreground = crate::platform::foreground_window();
    let focus = crate::platform::focused_control();
    let completed = with_modifiers_lifted(|| {
        backspaces(trigger_char_len);
        type_units_cancellable(text, token, foreground, focus)
    });
    if !completed {
        crate::logging::info("type-out expansion cancelled");
    }
}

/// Map a specific left/right modifier VK to its generic class so a combo's
/// required modifiers (generic) can be matched against physically-held keys.
fn modifier_class(vk: u16) -> u16 {
    match vk {
        x if x == VK_LCONTROL.0 || x == VK_RCONTROL.0 => VK_CONTROL.0,
        x if x == VK_LSHIFT.0 || x == VK_RSHIFT.0 => VK_SHIFT.0,
        x if x == VK_LMENU.0 || x == VK_RMENU.0 => VK_MENU.0,
        x if x == VK_LWIN.0 || x == VK_RWIN.0 => VK_LWIN.0,
        other => other,
    }
}

/// The generic modifier classes and key a paste combo presses.
fn combo_parts(combo: PasteCombo) -> (&'static [u16], u16) {
    const CTRL: &[u16] = &[VK_CONTROL.0];
    const SHIFT: &[u16] = &[VK_SHIFT.0];
    const CTRL_SHIFT: &[u16] = &[VK_CONTROL.0, VK_SHIFT.0];
    match combo {
        PasteCombo::CtrlV => (CTRL, VK_V),
        PasteCombo::ShiftInsert => (SHIFT, VK_INSERT.0),
        PasteCombo::CtrlShiftV => (CTRL_SHIFT, VK_V),
    }
}

/// The (virtual key, is_key_up) sequence for a paste combo that may fire
/// while the user is still physically holding modifier keys — which is the
/// normal case for a shortcut trigger (the chord's modifiers are down when
/// the hotkey callback runs).
///
/// Rules, in order:
/// - Held modifiers the combo does *not* require would corrupt the paste
///   (the target app would see e.g. Ctrl+Shift+Insert), so they are lifted
///   first and re-pressed at the end. Re-pressing keeps the logical key
///   state in sync with the user's physically-held keys; without it, the OS
///   believes the modifier was released and the next press of the hotkey's
///   letter types plain text.
/// - A physically-held modifier the combo *requires* is reused rather than
///   released and re-pressed, so its logical state is never disturbed.
/// - Required modifiers not held are pressed around the key.
/// - Restoring Alt/Win appends a masking keystroke of an unassigned key so
///   the user's eventual physical release doesn't read as a lone modifier
///   tap (Start menu / menu-bar activation).
fn paste_steps(required: &[u16], key: u16, held: &[u16]) -> Vec<(u16, bool)> {
    let lifted: Vec<u16> = held
        .iter()
        .copied()
        .filter(|&vk| !required.contains(&modifier_class(vk)))
        .collect();
    let missing: Vec<u16> = required
        .iter()
        .copied()
        .filter(|&req| !held.iter().any(|&h| modifier_class(h) == req))
        .collect();

    let mut steps = Vec::new();
    for &vk in &lifted {
        steps.push((vk, true));
    }
    for &vk in &missing {
        steps.push((vk, false));
    }
    steps.push((key, false));
    steps.push((key, true));
    for &vk in missing.iter().rev() {
        steps.push((vk, true));
    }
    for &vk in &lifted {
        steps.push((vk, false));
    }
    if needs_mask(&lifted) {
        steps.push((VK_MASK, false));
        steps.push((VK_MASK, true));
    }
    steps
}

/// Send the configured paste combo to paste the current clipboard contents,
/// accounting for modifiers the user is still physically holding. Everything
/// goes out in a single SendInput batch, so the lift/restore around the
/// paste is atomic — there is no window where a physical release can
/// interleave.
pub fn send_paste(combo: PasteCombo) {
    let (required, key) = combo_parts(combo);
    let held = held_of(&ALL_MODIFIERS);
    let inputs: Vec<INPUT> = paste_steps(required, key, &held)
        .into_iter()
        .map(|(vk, up)| key_vk(VIRTUAL_KEY(vk), up))
        .collect();
    send(&inputs);
}

#[cfg(test)]
mod tests {
    use super::*;

    const CTRL: u16 = VK_CONTROL.0;
    const SHIFT: u16 = VK_SHIFT.0;
    const INS: u16 = 0x2D; // VK_INSERT
    const LCTRL: u16 = VK_LCONTROL.0;
    const LSHIFT: u16 = VK_LSHIFT.0;
    const LALT: u16 = VK_LMENU.0;
    const LWIN: u16 = VK_LWIN.0;

    #[test]
    fn plain_paste_when_nothing_held() {
        // Text-trigger case: no physical modifiers → the classic sequence.
        assert_eq!(
            paste_steps(&[CTRL], VK_V, &[]),
            vec![(CTRL, false), (VK_V, false), (VK_V, true), (CTRL, true)]
        );
    }

    #[test]
    fn reuses_physically_held_ctrl() {
        // Ctrl+N shortcut: the user's own Ctrl does the work; its logical
        // state is never touched, so holding it and pressing N again refires.
        assert_eq!(
            paste_steps(&[CTRL], VK_V, &[LCTRL]),
            vec![(VK_V, false), (VK_V, true)]
        );
    }

    #[test]
    fn lifts_and_restores_shift() {
        // Ctrl+Shift+V shortcut firing a Ctrl+V paste: Shift must not leak
        // into the paste, and must be logically down again afterwards to
        // match the physical hold.
        assert_eq!(
            paste_steps(&[CTRL], VK_V, &[LCTRL, LSHIFT]),
            vec![(LSHIFT, true), (VK_V, false), (VK_V, true), (LSHIFT, false)]
        );
    }

    #[test]
    fn masks_alt_and_win_restores() {
        let steps = paste_steps(&[CTRL], VK_V, &[LCTRL, LALT, LWIN]);
        assert_eq!(
            steps,
            vec![
                (LALT, true),
                (LWIN, true),
                (VK_V, false),
                (VK_V, true),
                (LALT, false),
                (LWIN, false),
                (VK_MASK, false),
                (VK_MASK, true),
            ]
        );
    }

    #[test]
    fn shift_restore_needs_no_mask() {
        let steps = paste_steps(&[CTRL], VK_V, &[LCTRL, LSHIFT]);
        assert!(!steps.iter().any(|&(vk, _)| vk == VK_MASK));
    }

    #[test]
    fn shift_insert_lifts_held_ctrl() {
        // Ctrl+Insert is *copy*; a held Ctrl must not leak into Shift+Insert.
        assert_eq!(
            paste_steps(&[SHIFT], INS, &[LCTRL]),
            vec![
                (LCTRL, true),
                (SHIFT, false),
                (INS, false),
                (INS, true),
                (SHIFT, true),
                (LCTRL, false),
            ]
        );
    }

    #[test]
    fn shift_insert_reuses_held_shift() {
        assert_eq!(
            paste_steps(&[SHIFT], INS, &[LSHIFT]),
            vec![(INS, false), (INS, true)]
        );
    }

    #[test]
    fn ctrl_shift_v_presses_missing_modifiers() {
        assert_eq!(
            paste_steps(&[CTRL, SHIFT], VK_V, &[]),
            vec![
                (CTRL, false),
                (SHIFT, false),
                (VK_V, false),
                (VK_V, true),
                (SHIFT, true),
                (CTRL, true),
            ]
        );
    }

    #[test]
    fn released_modifier_is_not_restored() {
        assert_eq!(still_held(&[LSHIFT], &[]), Vec::<u16>::new());
    }

    #[test]
    fn physically_held_modifier_is_restored() {
        assert_eq!(still_held(&[LSHIFT, LCTRL], &[LSHIFT]), vec![LSHIFT]);
    }
}
