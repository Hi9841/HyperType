//! Editor canvas stub (S4 fills). Renders the canvas background only.

use gpui_kit::*;
use hypertype_core::commands::SnippetView;

use crate::ui::studio::Studio;
use crate::ui::tokens;

pub struct Editor {
    #[allow(dead_code)] // S4 reads Studio through this handle
    studio: WeakEntity<Studio>,
    _observe: Subscription,
}

impl Editor {
    pub fn new(studio: WeakEntity<Studio>, cx: &mut Context<Self>) -> Self {
        let _observe = studio
            .upgrade()
            .map(|entity| cx.observe(&entity, |_, _, cx| cx.notify()))
            .expect("Studio must exist when Editor is created");
        Self { studio, _observe }
    }

    /// App.tsx:225-234 — load an existing snippet into the form.
    /// S4: set mode/trigger/expansion from `snippet`, clear messages, stop recording.
    pub fn load(&mut self, _snippet: &SnippetView, _window: &mut Window, _cx: &mut Context<Self>) {
        // S4
    }

    /// App.tsx:236-246 — blank form for a new snippet; focuses the trigger input.
    /// S4: mode=text, empty trigger/expansion, clear messages, focus trigger.
    #[allow(dead_code)] // S4 blank-form path
    pub fn clear_for_new(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        // S4
    }

    /// App.tsx:437-452 — insert a token at the expansion caret (or append).
    /// S4: replace the selection, place caret after the token, focus textarea.
    #[allow(dead_code)] // S6 token insert
    pub fn insert_token(&mut self, _token: &str, _window: &mut Window, _cx: &mut Context<Self>) {
        // S4
    }

    /// App.tsx:419-426 — stop shortcut recording and drop key listeners.
    #[allow(dead_code)] // S4 recorder
    pub fn stop_recording(&mut self, _cx: &mut Context<Self>) {
        // S4
    }

    /// Surface a form error (delete failure, save failure).
    #[allow(dead_code)] // S4 form errors
    pub fn set_error(&mut self, _message: String, _cx: &mut Context<Self>) {
        // S4
    }

    /// S4 adds parity arms here. Signature matches `ui/parity.rs` dispatch.
    #[allow(dead_code)] // S4 parity arms
    pub fn parity_state(
        &mut self,
        _name: &str,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> bool {
        false
    }
}

impl Render for Editor {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        // .studio-canvas background — styles.css workspace right pane uses BG
        div().flex_1().h_full().min_w_0().bg(tokens::bg())
    }
}
