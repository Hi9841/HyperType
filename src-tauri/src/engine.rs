//! The engine thread: the single owner of the typed-text buffer and keyboard
//! state. It consumes raw key events from the hook over a channel and is the
//! only place expansion is triggered. Single-threaded ownership means the hot
//! path takes no locks except a brief read of the snippet map on a match.
//!
//! The OS-touching operations (foreground window, character decoding,
//! password-field check, and the actual expansion) sit behind `EngineHost` so
//! the matching logic can be driven deterministically in tests with a mock.

use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread;

use crate::app_state::AppState;
use crate::consts;
use crate::expansion;
use crate::keyboard::KeyEvent;
use crate::platform;

/// Cap the buffer so a long typing session never grows memory. Longer than any
/// reasonable trigger.
const MAX_BUFFER: usize = 64;

const WM_KEYDOWN: u32 = 0x0100;
const WM_KEYUP: u32 = 0x0101;
const WM_SYSKEYDOWN: u32 = 0x0104;
const WM_SYSKEYUP: u32 = 0x0105;

/// The side-effecting operations the engine needs from the platform. The real
/// implementation is `WinHost`; tests supply a mock.
pub trait EngineHost {
    fn foreground_window(&self) -> isize;
    fn decode_char(&self, vk: u32, scan: u32, keystate: &[u8; 256]) -> Option<char>;
    fn is_password_field(&self) -> bool;
    fn expand(&self, trigger_char_len: usize, text: &str);
}

/// Production host: real Win32 calls.
pub struct WinHost;

impl EngineHost for WinHost {
    fn foreground_window(&self) -> isize {
        platform::foreground_window()
    }
    fn decode_char(&self, vk: u32, scan: u32, keystate: &[u8; 256]) -> Option<char> {
        platform::decode_char(vk, scan, keystate)
    }
    fn is_password_field(&self) -> bool {
        platform::is_password_field()
    }
    fn expand(&self, trigger_char_len: usize, text: &str) {
        expansion::expand(trigger_char_len, text);
    }
}

struct Engine {
    buffer: Vec<char>,
    keystate: [u8; 256],
    last_foreground: isize,
}

impl Engine {
    fn new() -> Self {
        Engine {
            buffer: Vec::with_capacity(MAX_BUFFER),
            keystate: [0u8; 256],
            last_foreground: 0,
        }
    }

    fn handle(&mut self, host: &dyn EngineHost, state: &AppState, ev: KeyEvent) {
        // Reset the buffer when focus moves to another window: the caret context
        // is gone, so anything typed before no longer precedes the cursor.
        let fg = host.foreground_window();
        if fg != self.last_foreground {
            self.buffer.clear();
            self.last_foreground = fg;
        }

        let down = ev.message == WM_KEYDOWN || ev.message == WM_SYSKEYDOWN;
        let up = ev.message == WM_KEYUP || ev.message == WM_SYSKEYUP;

        // Track raw key state for character decoding (shift/ctrl/alt/caps).
        let vk = ev.vk as usize;
        if vk < 256 {
            if down {
                self.keystate[vk] |= 0x80;
            }
            if up {
                self.keystate[vk] &= !0x80;
            }
        }
        if down && ev.vk == consts::VK_CAPITAL {
            self.keystate[consts::VK_CAPITAL as usize] ^= 0x01; // toggle bit
        }
        self.normalize_modifiers();

        if !down {
            return;
        }

        // When disabled, observe nothing and keep the buffer empty.
        if !state.enabled.load(Ordering::Relaxed) {
            if !self.buffer.is_empty() {
                self.buffer.clear();
            }
            return;
        }

        // Editing / navigation keys.
        if ev.vk == consts::VK_BACK {
            self.buffer.pop();
            return;
        }
        if consts::is_navigation(ev.vk)
            || ev.vk == consts::VK_RETURN
            || ev.vk == consts::VK_TAB
            || ev.vk == consts::VK_ESCAPE
        {
            self.buffer.clear();
            return;
        }
        if consts::is_modifier(ev.vk) {
            return;
        }

        // Shortcut combos (Ctrl+x, Win+x) are commands, not text. AltGr is
        // Ctrl+Alt and DOES produce characters, so only bail when Ctrl is held
        // without Alt.
        let ctrl = self.keystate[consts::VK_CONTROL as usize] & 0x80 != 0;
        let alt = self.keystate[consts::VK_MENU as usize] & 0x80 != 0;
        let win = (self.keystate[consts::VK_LWIN as usize]
            | self.keystate[consts::VK_RWIN as usize])
            & 0x80
            != 0;
        if (ctrl && !alt) || win {
            self.buffer.clear();
            return;
        }

        // Decode to a character in the foreground app's layout.
        if let Some(ch) = host.decode_char(ev.vk, ev.scan, &self.keystate) {
            if ch.is_control() {
                return;
            }
            self.buffer.push(ch);
            if self.buffer.len() > MAX_BUFFER {
                let overflow = self.buffer.len() - MAX_BUFFER;
                self.buffer.drain(0..overflow);
            }
            self.try_expand(host, state);
        }
    }

    /// Keep the generic Shift/Ctrl/Alt bits in sync with the left/right keys
    /// the low-level hook actually reports, so decoding sees them.
    fn normalize_modifiers(&mut self) {
        let pair = |a: u32, b: u32, ks: &[u8; 256]| (ks[a as usize] | ks[b as usize]) & 0x80;
        self.keystate[consts::VK_SHIFT as usize] =
            pair(consts::VK_LSHIFT, consts::VK_RSHIFT, &self.keystate);
        self.keystate[consts::VK_CONTROL as usize] =
            pair(consts::VK_LCONTROL, consts::VK_RCONTROL, &self.keystate);
        self.keystate[consts::VK_MENU as usize] =
            pair(consts::VK_LMENU, consts::VK_RMENU, &self.keystate);
    }

    fn try_expand(&mut self, host: &dyn EngineHost, state: &AppState) {
        let matched = {
            let snippets = state.snippets.read().unwrap();
            snippets.match_suffix(&self.buffer)
        };
        if let Some((trigger_len, expansion)) = matched {
            // Never expand into a password field.
            if host.is_password_field() {
                return;
            }
            host.expand(trigger_len, &expansion);
            // Non-sensitive signal that an expansion fired (no snippet content).
            crate::logging::info(&format!("expanded trigger of {trigger_len} chars"));
            self.buffer.clear();
        }
    }
}

/// Spawn the engine thread and return the sender the hook uses to feed it.
pub fn start(state: Arc<AppState>) -> Sender<KeyEvent> {
    let (tx, rx) = mpsc::channel::<KeyEvent>();
    thread::spawn(move || run(state, rx));
    tx
}

fn run(state: Arc<AppState>, rx: Receiver<KeyEvent>) {
    let host = WinHost;
    let mut engine = Engine::new();
    // recv() blocks until the hook sends; the thread sleeps otherwise (0% CPU).
    while let Ok(ev) = rx.recv() {
        engine.handle(&host, &state, ev);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snippets::Snippets;
    use std::cell::{Cell, RefCell};
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicBool;
    use std::sync::RwLock;

    /// Mock host: stable, scriptable foreground; trivial vk->char decode;
    /// toggleable password flag; records every expansion request.
    struct MockHost {
        foreground: Cell<isize>,
        password: Cell<bool>,
        expansions: RefCell<Vec<(usize, String)>>,
    }

    impl MockHost {
        fn new() -> Self {
            MockHost {
                foreground: Cell::new(1),
                password: Cell::new(false),
                expansions: RefCell::new(Vec::new()),
            }
        }
    }

    impl EngineHost for MockHost {
        fn foreground_window(&self) -> isize {
            self.foreground.get()
        }
        fn decode_char(&self, vk: u32, _scan: u32, _keystate: &[u8; 256]) -> Option<char> {
            if vk == 0x20 {
                Some(' ')
            } else if (0x41..=0x5A).contains(&vk) {
                Some((b'a' + (vk as u8 - 0x41)) as char) // letters as lowercase
            } else if (0x30..=0x39).contains(&vk) {
                Some((b'0' + (vk as u8 - 0x30)) as char)
            } else {
                None
            }
        }
        fn is_password_field(&self) -> bool {
            self.password.get()
        }
        fn expand(&self, trigger_char_len: usize, text: &str) {
            self.expansions
                .borrow_mut()
                .push((trigger_char_len, text.to_string()));
        }
    }

    fn state_with(snippets: &[(&str, &str)]) -> AppState {
        let mut m = HashMap::new();
        for (t, e) in snippets {
            m.insert(t.to_string(), e.to_string());
        }
        AppState {
            snippets: RwLock::new(Snippets::from_map(m)),
            enabled: AtomicBool::new(true),
            data_path: PathBuf::from("test.json"),
        }
    }

    const VK_BACK: u32 = 0x08;
    const VK_LEFT: u32 = 0x25;

    fn vk_of(c: char) -> u32 {
        if c == ' ' {
            0x20
        } else {
            (c.to_ascii_uppercase() as u32) & 0xFF
        }
    }

    fn down(engine: &mut Engine, host: &dyn EngineHost, state: &AppState, vk: u32) {
        engine.handle(
            host,
            state,
            KeyEvent {
                message: WM_KEYDOWN,
                vk,
                scan: 0,
            },
        );
    }

    /// Type a literal string as individual key-down events.
    fn type_str(engine: &mut Engine, host: &dyn EngineHost, state: &AppState, s: &str) {
        for c in s.chars() {
            down(engine, host, state, vk_of(c));
        }
    }

    #[test]
    fn basic_expansion() {
        let host = MockHost::new();
        let state = state_with(&[("gm", "Good morning")]);
        let mut engine = Engine::new();
        type_str(&mut engine, &host, &state, "gm");
        assert_eq!(
            *host.expansions.borrow(),
            vec![(2, "Good morning".to_string())]
        );
    }

    #[test]
    fn multichar_trigger() {
        let host = MockHost::new();
        let state = state_with(&[("addr", "123 Main Street")]);
        let mut engine = Engine::new();
        type_str(&mut engine, &host, &state, "addr");
        assert_eq!(
            *host.expansions.borrow(),
            vec![(4, "123 Main Street".to_string())]
        );
    }

    #[test]
    fn no_match_inside_word() {
        let host = MockHost::new();
        let state = state_with(&[("gm", "Good morning")]);
        let mut engine = Engine::new();
        type_str(&mut engine, &host, &state, "xgm"); // 'g' preceded by alnum 'x'
        assert!(host.expansions.borrow().is_empty());
    }

    #[test]
    fn fires_after_space_boundary() {
        let host = MockHost::new();
        let state = state_with(&[("gm", "Good morning")]);
        let mut engine = Engine::new();
        type_str(&mut engine, &host, &state, "hi gm");
        assert_eq!(
            *host.expansions.borrow(),
            vec![(2, "Good morning".to_string())]
        );
    }

    #[test]
    fn backspace_correction() {
        let host = MockHost::new();
        let state = state_with(&[("gm", "Good morning")]);
        let mut engine = Engine::new();
        down(&mut engine, &host, &state, vk_of('g'));
        down(&mut engine, &host, &state, vk_of('x'));
        down(&mut engine, &host, &state, VK_BACK); // delete the 'x'
        down(&mut engine, &host, &state, vk_of('m'));
        assert_eq!(
            *host.expansions.borrow(),
            vec![(2, "Good morning".to_string())]
        );
    }

    #[test]
    fn navigation_resets_buffer() {
        let host = MockHost::new();
        let state = state_with(&[("gm", "Good morning")]);
        let mut engine = Engine::new();
        down(&mut engine, &host, &state, vk_of('g'));
        down(&mut engine, &host, &state, VK_LEFT); // caret moved
        down(&mut engine, &host, &state, vk_of('m'));
        assert!(host.expansions.borrow().is_empty());
    }

    #[test]
    fn focus_change_resets_buffer() {
        let host = MockHost::new();
        let state = state_with(&[("gm", "Good morning")]);
        let mut engine = Engine::new();
        down(&mut engine, &host, &state, vk_of('g'));
        host.foreground.set(2); // user switched windows
        down(&mut engine, &host, &state, vk_of('m'));
        assert!(host.expansions.borrow().is_empty());
    }

    #[test]
    fn password_field_blocks_expansion() {
        let host = MockHost::new();
        host.password.set(true);
        let state = state_with(&[("gm", "Good morning")]);
        let mut engine = Engine::new();
        type_str(&mut engine, &host, &state, "gm");
        assert!(host.expansions.borrow().is_empty());
    }

    #[test]
    fn disabled_blocks_expansion() {
        let host = MockHost::new();
        let state = state_with(&[("gm", "Good morning")]);
        state.enabled.store(false, Ordering::Relaxed);
        let mut engine = Engine::new();
        type_str(&mut engine, &host, &state, "gm");
        assert!(host.expansions.borrow().is_empty());
    }

    #[test]
    fn prefix_trigger_fires_immediately() {
        // Instant (no-terminator) expansion: a shorter trigger that is a prefix
        // of a longer one fires the moment it completes. With both "ad" and
        // "addr" defined, typing "addr" expands "ad" first. This is the
        // documented tradeoff of instant expansion, verified here so the
        // behavior can't regress silently.
        let host = MockHost::new();
        let state = state_with(&[("ad", "short"), ("addr", "123 Main Street")]);
        let mut engine = Engine::new();
        type_str(&mut engine, &host, &state, "addr");
        assert_eq!(host.expansions.borrow()[0], (2, "short".to_string()));
    }

    #[test]
    fn two_expansions_in_sequence() {
        let host = MockHost::new();
        let state = state_with(&[("gm", "Good morning"), ("brb", "be right back")]);
        let mut engine = Engine::new();
        type_str(&mut engine, &host, &state, "gm");
        type_str(&mut engine, &host, &state, " brb");
        assert_eq!(
            *host.expansions.borrow(),
            vec![
                (2, "Good morning".to_string()),
                (3, "be right back".to_string())
            ]
        );
    }
}
