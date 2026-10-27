//! Shortcut chords. The stored format uses `KeyboardEvent.code` names, such
//! as `Ctrl+Shift+KeyV`, because v1.x wrote that format and the
//! `global-hotkey` parser reads it. `Super` is the Windows key; the UI shows
//! it as `Win`.

use gpui_kit::Keystroke;
use windows::Win32::UI::Input::KeyboardAndMouse::VkKeyScanW;

/// The chord for a key press, or `None` for a press without Ctrl, Alt,
/// Shift, or Win, and for keys that a hotkey cannot use.
pub fn from_keystroke(keystroke: &Keystroke) -> Option<String> {
    let mut shift = keystroke.modifiers.shift;
    let code = match named_code(&keystroke.key) {
        Some(code) => code.to_string(),
        None => {
            // GPUI names a printable key by its character and, for keys such
            // as Shift+1, drops Shift and reports "!". Map the character
            // back to the physical key in the active layout.
            let (code, shifted) = printable_code(&keystroke.key)?;
            shift |= shifted;
            code
        }
    };

    let modifiers = &keystroke.modifiers;
    let mut parts = Vec::new();
    if modifiers.control {
        parts.push("Ctrl");
    }
    if modifiers.alt {
        parts.push("Alt");
    }
    if shift {
        parts.push("Shift");
    }
    if modifiers.platform {
        parts.push("Super");
    }
    if parts.is_empty() {
        return None;
    }
    Some(format!("{}+{code}", parts.join("+")))
}

/// Keycap labels, for example `Ctrl+Shift+KeyV` gives `Ctrl`, `Shift`, `V`.
pub fn keys(chord: &str) -> Vec<String> {
    chord
        .split('+')
        .map(|token| {
            let label = match token {
                "Super" => "Win",
                _ => ["Key", "Digit", "Arrow"]
                    .iter()
                    .find_map(|prefix| token.strip_prefix(prefix))
                    .unwrap_or(token),
            };
            label.to_string()
        })
        .collect()
}

pub fn display(chord: &str) -> String {
    keys(chord).join("+")
}

fn named_code(key: &str) -> Option<&'static str> {
    Some(match key {
        "space" => "Space",
        "enter" => "Enter",
        "tab" => "Tab",
        "backspace" => "Backspace",
        "delete" => "Delete",
        "insert" => "Insert",
        "home" => "Home",
        "end" => "End",
        "pageup" => "PageUp",
        "pagedown" => "PageDown",
        "up" => "ArrowUp",
        "down" => "ArrowDown",
        "left" => "ArrowLeft",
        "right" => "ArrowRight",
        "f1" => "F1",
        "f2" => "F2",
        "f3" => "F3",
        "f4" => "F4",
        "f5" => "F5",
        "f6" => "F6",
        "f7" => "F7",
        "f8" => "F8",
        "f9" => "F9",
        "f10" => "F10",
        "f11" => "F11",
        "f12" => "F12",
        "f13" => "F13",
        "f14" => "F14",
        "f15" => "F15",
        "f16" => "F16",
        "f17" => "F17",
        "f18" => "F18",
        "f19" => "F19",
        "f20" => "F20",
        "f21" => "F21",
        "f22" => "F22",
        "f23" => "F23",
        "f24" => "F24",
        _ => return None,
    })
}

fn printable_code(key: &str) -> Option<(String, bool)> {
    let mut chars = key.chars();
    let ch = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    let mut units = [0u16; 2];
    let [unit] = ch.encode_utf16(&mut units) else {
        return None;
    };
    // Low byte: virtual-key code. High byte: bit 0 is Shift.
    let scan = unsafe { VkKeyScanW(*unit) };
    if scan == -1 {
        return None;
    }
    let vk = (scan & 0xFF) as u8;
    let shifted = (scan >> 8) & 1 == 1;
    let code = match vk {
        b'A'..=b'Z' => format!("Key{}", vk as char),
        b'0'..=b'9' => format!("Digit{}", vk as char),
        0xBA => "Semicolon".into(),
        0xBB => "Equal".into(),
        0xBC => "Comma".into(),
        0xBD => "Minus".into(),
        0xBE => "Period".into(),
        0xBF => "Slash".into(),
        0xC0 => "Backquote".into(),
        0xDB => "BracketLeft".into(),
        0xDC => "Backslash".into(),
        0xDD => "BracketRight".into(),
        0xDE => "Quote".into(),
        0xE2 => "IntlBackslash".into(),
        _ => return None,
    };
    Some((code, shifted))
}
