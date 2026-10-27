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

/// Cap the buffer so a long typing session never grows memory. IPC validation
/// uses the same limit, so an accepted text trigger is always reachable.
const MAX_BUFFER: usize = crate::snippets::MAX_TEXT_TRIGGER_CHARS;

const WM_KEYDOWN: u32 = 0x0100;
const WM_KEYUP: u32 = 0x0101;
const WM_SYSKEYDOWN: u32 = 0x0104;
const WM_SYSKEYUP: u32 = 0x0105;

/// The side-effecting operations the engine needs from the platform. The real
/// implementation is `WinHost`; tests supply a mock.
pub trait EngineHost {
    fn foreground_window(&self) -> isize;
    fn focused_control(&self) -> isize;
    fn seed_modifier_state(&self, keystate: &mut [u8; 256]);
    fn decode_key(&self, vk: u32, scan: u32, keystate: &[u8; 256]) -> platform::Decoded;
    fn is_password_field(&self) -> bool;
    fn expand(&self, trigger_char_len: usize, text: &str);
}

/// Production host: real Win32 calls.
pub struct WinHost;

impl EngineHost for WinHost {
    fn foreground_window(&self) -> isize {
        platform::foreground_window()
    }
    fn focused_control(&self) -> isize {
        platform::focused_control()
    }
    fn seed_modifier_state(&self, keystate: &mut [u8; 256]) {
        platform::seed_modifier_state(keystate);
    }
    fn decode_key(&self, vk: u32, scan: u32, keystate: &[u8; 256]) -> platform::Decoded {
        platform::decode_key(vk, scan, keystate)
    }
    fn is_password_field(&self) -> bool {
        platform::is_password_field()
    }
    fn expand(&self, trigger_char_len: usize, text: &str) {
        // The hook has returned, but Windows may still be dispatching the final
        // trigger key. Give it one scheduler tick before sending backspaces.
        thread::sleep(std::time::Duration::from_millis(1));
        expansion::expand(trigger_char_len, text);
    }
}

struct Engine {
    buffer: Vec<char>,
    keystate: [u8; 256],
    last_foreground: isize,
    last_focused_control: isize,
    last_context_epoch: u64,
}

impl Engine {
    fn new() -> Self {
        Engine {
            buffer: Vec::with_capacity(MAX_BUFFER),
            keystate: [0u8; 256],
            last_foreground: 0,
            last_focused_control: 0,
            last_context_epoch: crate::keyboard::context_reset_epoch(),
        }
    }

    fn handle(&mut self, host: &dyn EngineHost, state: &AppState, ev: KeyEvent) {
        // Reset the buffer when focus moves to another window: the caret context
        // is gone, so anything typed before no longer precedes the cursor.
        let foreground = host.foreground_window();
        let focused_control = host.focused_control();
        if foreground != self.last_foreground || focused_control != self.last_focused_control {
            self.buffer.clear();
            self.last_foreground = foreground;
            self.last_focused_control = focused_control;
            host.seed_modifier_state(&mut self.keystate);
        }

        // A click or drag-select can move the caret inside the same window
        // without changing the foreground HWND. The mouse hook bumps this
        // epoch, so a stale prefix is never completed against unrelated text.
        let context_epoch = crate::keyboard::context_reset_epoch();
        if context_epoch != self.last_context_epoch {
            self.buffer.clear();
            self.last_context_epoch = context_epoch;
            host.seed_modifier_state(&mut self.keystate);
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

        let ctrl = self.keystate[consts::VK_CONTROL as usize] & 0x80 != 0;
        let alt = self.keystate[consts::VK_MENU as usize] & 0x80 != 0;
        let win = (self.keystate[consts::VK_LWIN as usize]
            | self.keystate[consts::VK_RWIN as usize])
            & 0x80
            != 0;
        let shift = self.keystate[consts::VK_SHIFT as usize] & 0x80 != 0;

        // Editing / navigation keys.
        if ev.vk == consts::VK_BACK {
            // Ctrl/Alt/Win+Backspace deletes a word (or is an undo), so the
            // target is no longer just "one character shorter".
            if ctrl || alt || win {
                self.buffer.clear();
            } else {
                self.buffer.pop();
            }
            return;
        }
        if ev.vk == consts::VK_INSERT {
            // Shift+Insert pastes and Ctrl+Insert copies: both change context.
            self.buffer.clear();
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
        if crate::hotkeys::try_match_chord_fallback(ev.vk, ctrl, alt, shift, win, state) {
            self.buffer.clear();
            return;
        }

        if (ctrl && !alt) || win {
            self.buffer.clear();
            return;
        }

        // Decode text without disturbing the foreground app's dead-key state.
        match host.decode_key(ev.vk, ev.scan, &self.keystate) {
            platform::Decoded::Text(text) => {
                self.buffer.extend(text.chars());
                if self.buffer.len() > MAX_BUFFER {
                    let overflow = self.buffer.len() - MAX_BUFFER;
                    self.buffer.drain(0..overflow);
                }
                self.try_expand(host, state);
            }
            platform::Decoded::Dead => {
                // We cannot mirror composition without mutating the foreground
                // app's keyboard buffer, so never match across a dead key.
                self.buffer.clear();
            }
            platform::Decoded::None => {}
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
            let snippets = state
                .snippets
                .read()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
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
    host.seed_modifier_state(&mut engine.keystate);
    // recv() blocks until the hook sends; the thread sleeps otherwise (0% CPU).
    while let Ok(ev) = rx.recv() {
        engine.handle(&host, &state, ev);
    }
}
