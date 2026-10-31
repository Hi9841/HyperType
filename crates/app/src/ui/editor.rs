//! Editor canvas (S4). App.tsx:802-955, styles.css:504-793.

use std::time::Duration;

use gpui_kit::base::Easing;
use gpui_kit::component::h_flex;
use gpui_kit::component::input::{Input, InputEvent, InputState, Textarea, TextareaState};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::v_flex;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use hypertype_core::commands::SnippetView;
use hypertype_core::snippets::{TriggerKind, MAX_TEXT_TRIGGER_CHARS};

use crate::chord;
use crate::ui::icons;
use crate::ui::studio::{Modal, Studio};
use crate::ui::tokens;
use crate::ui::widgets::{self, SegKind};

const SUCCESS_CLEAR: Duration = Duration::from_secs(3);

const TRIGGER_PLACEHOLDER: &str = "e.g. gm, sig, addr, intro_pitch";
const EXPANSION_PLACEHOLDER: &str = "Type or paste your complete expansion here... (multiline paragraphs, email templates, addresses, code snippets, etc.)";

pub struct Editor {
    studio: WeakEntity<Studio>,
    mode: TriggerKind,
    /// Shortcut chord text, and a mirror of the text-trigger field for the header.
    trigger: String,
    busy: bool,
    recording: bool,
    form_error: String,
    form_success: String,
    trigger_input: Entity<InputState>,
    expansion: Entity<TextareaState>,
    /// Focus target for the shortcut recorder (App.tsx:433 focusout cancel).
    recorder_focus: FocusHandle,
    success_task: Option<Task<()>>,
    recording_subs: Vec<Subscription>,
    _observe: Subscription,
    _trigger_sub: Subscription,
    _expansion_sub: Subscription,
}

impl Editor {
    pub fn new(studio: WeakEntity<Studio>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let trigger_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(TRIGGER_PLACEHOLDER)
        });
        // Kit placeholder does not soft-wrap; empty Kit string + painted wrap (finding 2).
        let expansion = cx.new(|cx| TextareaState::new(window, cx).placeholder(""));
        let recorder_focus = cx.focus_handle().tab_stop(true);

        let _observe = studio
            .upgrade()
            .map(|entity| cx.observe(&entity, |_, _, cx| cx.notify()))
            .expect("Studio must exist when Editor is created");

        let _trigger_sub = cx.subscribe_in(&trigger_input, window, {
            move |this, input, event: &InputEvent, window, cx| {
                if !matches!(event, InputEvent::Change) {
                    return;
                }
                if this.mode != TriggerKind::Text {
                    return;
                }
                let value = input.read(cx).value().to_string();
                let limited: String = value.chars().take(MAX_TEXT_TRIGGER_CHARS).collect();
                if limited != value {
                    input.update(cx, |state, cx| {
                        state.set_value(limited.clone(), window, cx);
                    });
                }
                this.trigger = limited;
                cx.notify();
            }
        });

        let _expansion_sub = cx.subscribe_in(&expansion, window, {
            move |this, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    let _ = this;
                    cx.notify();
                }
            }
        });

        Self {
            studio,
            mode: TriggerKind::Text,
            trigger: String::new(),
            busy: false,
            recording: false,
            form_error: String::new(),
            form_success: String::new(),
            trigger_input,
            expansion,
            recorder_focus,
            success_task: None,
            recording_subs: Vec::new(),
            _observe,
            _trigger_sub,
            _expansion_sub,
        }
    }

    /// App.tsx:225-234 — load an existing snippet into the form.
    pub fn load(&mut self, snippet: &SnippetView, window: &mut Window, cx: &mut Context<Self>) {
        self.stop_recording(cx);
        self.mode = snippet.kind;
        self.trigger = snippet.trigger.clone();
        self.form_error.clear();
        self.form_success.clear();
        self.success_task = None;

        let trigger_value = if snippet.kind == TriggerKind::Text {
            snippet.trigger.as_str()
        } else {
            ""
        };
        self.trigger_input.update(cx, |state, cx| {
            state.set_value(trigger_value, window, cx);
        });
        self.expansion.update(cx, |state, cx| {
            state.set_value(snippet.expansion.as_str(), window, cx);
        });
        // App.tsx selectSnippet does not move focus (225-234); do not blur.
        cx.notify();
    }

    /// App.tsx:236-246 — blank form for a new snippet; focuses the trigger input.
    pub fn clear_for_new(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.stop_recording(cx);
        self.mode = TriggerKind::Text;
        self.trigger.clear();
        self.form_error.clear();
        self.form_success.clear();
        self.success_task = None;
        self.trigger_input.update(cx, |state, cx| {
            state.set_value("", window, cx);
        });
        self.expansion.update(cx, |state, cx| {
            state.set_value("", window, cx);
        });
        self.trigger_input.update(cx, |state, cx| {
            state.focus(window, cx);
        });
        cx.notify();
    }

    /// App.tsx:437-452 — insert a token at the expansion caret (or append).
    pub fn insert_token(&mut self, token: &str, window: &mut Window, cx: &mut Context<Self>) {
        let token = token.to_string();
        self.expansion.update(cx, |state, cx| {
            state.replace(token, window, cx);
            state.focus(window, cx);
        });
        cx.notify();
    }

    /// App.tsx:419-426 — stop shortcut recording and drop key listeners.
    pub fn stop_recording(&mut self, cx: &mut Context<Self>) {
        if !self.recording && self.recording_subs.is_empty() {
            return;
        }
        self.recording = false;
        self.recording_subs.clear();
        cx.notify();
    }

    /// Surface a form error (delete failure, save failure).
    pub fn set_error(&mut self, message: String, cx: &mut Context<Self>) {
        self.form_error = message;
        self.form_success.clear();
        cx.notify();
    }

    /// S4 parity arms. Signature matches `ui/parity.rs` dispatch.
    pub fn parity_state(
        &mut self,
        name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match name {
            "new-snippet" | "save-disabled" => {
                self.set_studio_creating(true, cx);
                self.clear_for_new(window, cx);
                true
            }
            "shortcut-mode-empty" => {
                self.set_studio_creating(true, cx);
                self.clear_for_new(window, cx);
                self.switch_mode(TriggerKind::Shortcut, window, cx);
                true
            }
            "shortcut-recording" => {
                self.set_studio_creating(true, cx);
                self.clear_for_new(window, cx);
                self.switch_mode(TriggerKind::Shortcut, window, cx);
                self.start_shortcut_recording(window, cx);
                true
            }
            // default-selected is applied by ui/parity.rs fallback (select first).
            _ => false,
        }
    }

    fn set_studio_creating(&self, creating: bool, cx: &mut Context<Self>) {
        // Parity dispatch already holds `studio.update`; defer to avoid re-entry.
        let Some(studio) = self.studio.upgrade() else {
            return;
        };
        cx.defer(move |cx| {
            studio.update(cx, |studio, cx| {
                studio.creating_new = creating;
                if creating {
                    studio.selected = None;
                }
                cx.notify();
            });
        });
    }

    fn freeze(&self, cx: &App) -> bool {
        self.studio
            .upgrade()
            .map(|s| s.read(cx).freeze_animations)
            .unwrap_or(false)
    }

    fn creating_new(&self, cx: &App) -> bool {
        self.studio
            .upgrade()
            .map(|s| s.read(cx).creating_new)
            .unwrap_or(false)
    }

    fn selected_trigger(&self, cx: &App) -> Option<String> {
        self.studio
            .upgrade()
            .and_then(|s| s.read(cx).selected.clone())
    }

    fn trigger_value(&self, cx: &App) -> String {
        match self.mode {
            TriggerKind::Text => self.trigger_input.read(cx).value().to_string(),
            TriggerKind::Shortcut => self.trigger.clone(),
        }
    }

    fn expansion_value(&self, cx: &App) -> String {
        self.expansion.read(cx).value().to_string()
    }

    fn can_save(&self, cx: &App) -> bool {
        !self.busy
            && !self.trigger_value(cx).trim().is_empty()
            && !self.expansion_value(cx).is_empty()
    }

    /// App.tsx:391-398
    fn switch_mode(&mut self, next: TriggerKind, window: &mut Window, cx: &mut Context<Self>) {
        if self.mode == next {
            return;
        }
        self.stop_recording(cx);
        self.mode = next;
        self.trigger.clear();
        self.form_error.clear();
        self.trigger_input.update(cx, |state, cx| {
            state.set_value("", window, cx);
        });
        if next == TriggerKind::Text {
            self.trigger_input.update(cx, |state, cx| {
                state.focus(window, cx);
            });
        }
        cx.notify();
    }

    /// App.tsx:400-435
    fn start_shortcut_recording(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.recording {
            return;
        }
        self.recording = true;
        self.trigger.clear();
        self.form_error.clear();
        self.recording_subs.clear();

        // Focus the recorder so it shows `.studio-field:focus` RING_SOFT (finding 6)
        // before blur listeners are armed (finding 7).
        self.recorder_focus.focus(window, cx);

        let weak = cx.weak_entity();
        // Every key is consumed while recording (App.tsx:407), including Tab.
        // Blur cancel still handles real focus moves (click elsewhere, deactivation).
        let key_sub = cx.intercept_keystrokes({
            let weak = weak.clone();
            move |event, window, cx| {
                let recording = weak
                    .upgrade()
                    .map(|e| e.read(cx).recording)
                    .unwrap_or(false);
                if !recording {
                    return;
                }
                window.prevent_default();
                cx.stop_propagation();
                let _ = weak.update(cx, |this, cx| {
                    if !this.recording {
                        return;
                    }
                    if event.keystroke.key == "escape" {
                        this.stop_recording(cx);
                        return;
                    }
                    if event.keystroke.key == "tab" {
                        // Spec consumes Tab; do not move focus or cancel via Tab.
                        return;
                    }
                    if let Some(chord) = chord::from_keystroke(&event.keystroke) {
                        this.trigger = chord;
                        this.stop_recording(cx);
                        this.expansion.update(cx, |state, cx| {
                            state.focus(window, cx);
                        });
                    }
                    // Bare modifiers and non-chord keys: swallowed, no action.
                });
            }
        });
        self.recording_subs.push(key_sub);

        // Tab / any focus move off the recorder cancels (App.tsx:433 focusout).
        let blur_sub = cx.on_blur(&self.recorder_focus, window, |this, _, cx| {
            if this.recording {
                this.stop_recording(cx);
            }
        });
        self.recording_subs.push(blur_sub);

        // OS window deactivation (App.tsx:433 blur).
        let act_sub = cx.observe_window_activation(window, |this, window, cx| {
            if this.recording && !window.is_window_active() {
                this.stop_recording(cx);
            }
        });
        self.recording_subs.push(act_sub);

        cx.notify();
    }

    /// App.tsx:454-480
    fn save_or_add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let trigger = self.trigger_value(cx);
        let expansion = self.expansion_value(cx);
        let t = trigger.trim();
        if t.is_empty() || expansion.is_empty() || self.busy {
            return;
        }

        let Some(studio) = self.studio.upgrade() else {
            return;
        };
        let creating = studio.read(cx).creating_new;
        let selected = studio.read(cx).selected.clone();
        let old = if creating || selected.is_none() {
            None
        } else {
            selected.clone()
        };
        let kind = self.mode;

        self.busy = true;
        self.form_error.clear();
        self.form_success.clear();
        cx.notify();

        let result = studio.update(cx, |studio, cx| {
            studio.save_snippet(old.as_deref(), t, expansion, kind, window, cx)
        });

        self.busy = false;
        match result {
            Ok(()) => {
                let msg = if creating || selected.is_none() {
                    "Snippet created!"
                } else {
                    "Saved!"
                };
                self.form_success = msg.into();
                self.success_task = Some(cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(SUCCESS_CLEAR).await;
                    this.update(cx, |this, cx| {
                        this.form_success.clear();
                        this.success_task = None;
                        cx.notify();
                    })
                    .ok();
                }));
            }
            Err(err) => {
                self.form_error = err;
            }
        }
        cx.notify();
    }

    fn delete_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(trigger) = self.selected_trigger(cx) else {
            return;
        };
        let Some(studio) = self.studio.upgrade() else {
            return;
        };
        studio.update(cx, |studio, cx| {
            studio.delete_snippet(trigger, window, cx);
        });
    }

    fn open_tokens_modal(&mut self, cx: &mut Context<Self>) {
        if let Some(studio) = self.studio.upgrade() {
            studio.update(cx, |studio, cx| {
                studio.set_modal(Some(Modal::Tokens), cx);
            });
        }
    }

    fn lines_words_chars(expansion: &str) -> (usize, usize, usize) {
        let lines = if expansion.is_empty() {
            0
        } else {
            expansion.split('\n').count()
        };
        let trimmed = expansion.trim();
        let words = if trimmed.is_empty() {
            0
        } else {
            trimmed.split_whitespace().count()
        };
        // Displayed length = JS `.length` = UTF-16 code units (RULEBOOK A5).
        let chars = expansion.encode_utf16().count();
        (lines, words, chars)
    }
}

impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let freeze = self.freeze(cx);
        let creating = self.creating_new(cx);
        let selected = self.selected_trigger(cx);
        let trigger_display = self.trigger_value(cx);
        let expansion_text = self.expansion_value(cx);
        let (lines, words, chars) = Self::lines_words_chars(&expansion_text);
        let can_save = self.can_save(cx);
        let mode = self.mode;
        let recording = self.recording;
        let busy = self.busy;
        let form_success = self.form_success.clone();
        let form_error = self.form_error.clone();

        let trigger_focused = self
            .trigger_input
            .read(cx)
            .focus_handle(cx)
            .is_focused(window);
        let expansion_focused = self
            .expansion
            .read(cx)
            .focus_handle(cx)
            .is_focused(window);

        let entity = cx.entity();
        let title = if creating {
            "New Snippet".to_string()
        } else {
            format!(
                "Editing: {}",
                if trigger_display.is_empty() {
                    "Snippet"
                } else {
                    trigger_display.as_str()
                }
            )
        };
        let sub_hint = if mode == TriggerKind::Text {
            "Triggers expand automatically at word boundaries"
        } else {
            "Registered as global Windows hotkey chord"
        };
        let field_label = if mode == TriggerKind::Text {
            "ABBREVIATION / TRIGGER"
        } else {
            "SHORTCUT CHORD"
        };
        let save_label = if creating {
            "Create Snippet"
        } else {
            "Save Changes"
        };
        let show_delete = !creating && selected.is_some();

        // .studio-canvas — styles.css:505-515
        v_flex()
            .id("studio-canvas")
            .relative()
            .flex_1()
            .h_full()
            .min_w_0()
            .min_h_0()
            .bg(tokens::bg())
            .px(px(24.))
            .py(px(20.))
            .gap(px(14.))
            .child(self.render_header(&title, sub_hint, freeze, window, cx))
            .child(self.render_trigger_row(field_label, trigger_focused, freeze, window, cx))
            .child(self.render_token_bar(cx))
            .child(self.render_textarea(expansion_focused, &expansion_text))
            .child(self.render_footer(
                lines,
                words,
                chars,
                &form_success,
                &form_error,
                show_delete,
                can_save && !busy,
                save_label,
                cx,
            ))
            .when(recording, {
                let entity = entity.clone();
                move |this| {
                    // Capture-phase window mouse-down cancel (App.tsx:432). No hitbox
                    // overlay — canvas only paints so the listener re-registers each
                    // frame (survives resize). Do not stop propagation.
                    this.child(
                        canvas(
                            |_, _, _| (),
                            move |_, (), window, _cx| {
                                let entity = entity.clone();
                                window.on_mouse_event(
                                    move |_ev: &MouseDownEvent, phase, _window, cx| {
                                        if phase != DispatchPhase::Capture {
                                            return;
                                        }
                                        entity.update(cx, |this, cx| {
                                            if this.recording {
                                                this.stop_recording(cx);
                                            }
                                        });
                                    },
                                );
                            },
                        )
                        .absolute()
                        .inset_0()
                        .w_full()
                        .h_full(),
                    )
                }
            })
    }
}

impl Editor {
    fn render_header(
        &self,
        title: &str,
        sub_hint: &str,
        freeze: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let entity = cx.entity();
        let mode = self.mode;
        // .canvas-header — styles.css:517-521
        h_flex()
            .id("canvas-header")
            .justify_between()
            .items_start()
            .child(
                // .canvas-header-title
                v_flex()
                    .gap(px(0.))
                    .child(
                        // h2 — styles.css:523-528 (letter-spacing omitted D2)
                        // Explicit 24px line box (16px × 1.5); rule 23.
                        div()
                            .text_size(px(16.))
                            .line_height(px(24.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(tokens::ink())
                            .child(title.to_string()),
                    )
                    .child(
                        // .canvas-sub-hint — styles.css:529-532
                        // Inline span in 13px/1.5 parent strut → 19.5px; GPUI rounds
                        // px(19.5) to 20 (gpui-pre style.rs:554-555). Pin 20 for the
                        // line box; mb(-1) cancels the 0.5px integer overshoot so the
                        // stack below stays within 1px of Chromium.
                        div()
                            .text_size(px(12.))
                            .line_height(px(20.))
                            .mb(px(-1.))
                            .text_color(tokens::ink_2())
                            .child(sub_hint.to_string()),
                    ),
            )
            .child(widgets::segmented(
                "editor-kind",
                &[
                    (TriggerKind::Text, "Text"),
                    (TriggerKind::Shortcut, "Shortcut"),
                ],
                mode,
                SegKind::Two,
                px(140.),
                freeze,
                move |next, window, cx| {
                    entity.update(cx, |this, cx| {
                        this.switch_mode(next, window, cx);
                    });
                },
                window,
                cx,
            ))
    }

    fn render_trigger_row(
        &self,
        field_label: &str,
        trigger_focused: bool,
        freeze: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        // .trigger-config-row — styles.css:534-538
        h_flex()
            .id("trigger-config-row")
            .gap(px(16.))
            .items_end()
            .child(
                // .trigger-input-wrapper — styles.css:540-545
                v_flex()
                    .flex_1()
                    .gap(px(4.))
                    .child(
                        // .field-label — styles.css:548-552 (letter-spacing omitted D2)
                        // Chromium keeps 15.75; GPUI rounds down — pin 16 so trigger y matches.
                        div()
                            .text_size(px(10.5))
                            .line_height(px(16.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(tokens::ink_3())
                            .child(field_label.to_string()),
                    )
                    .child(match self.mode {
                        TriggerKind::Text => self.render_trigger_input(trigger_focused),
                        TriggerKind::Shortcut => self
                            .render_recorder(self.recording, freeze, window, cx)
                            .into_any_element(),
                    }),
            )
    }

    fn render_trigger_input(&self, focused: bool) -> AnyElement {
        // .studio-field.trigger-text-input — styles.css:554-575
        // Kit Input padding/height zeroed (RULEBOOK rule 17); frame supplies chrome.
        widgets::field_frame(
            focused,
            Input::new(&self.trigger_input)
                .id("trigger-text-input")
                .appearance(false)
                .bordered(false)
                .w_full()
                .h_full()
                .px(px(0.))
                .py(px(0.))
                .text_size(px(13.))
                .font_family(tokens::FONT_MONO)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(tokens::accent_ink()),
        )
        .into_any_element()
    }

    fn render_recorder(
        &self,
        recording: bool,
        freeze: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let entity = cx.entity();
        let trigger = self.trigger.clone();
        let show_chord = !recording && !trigger.is_empty();
        let focused = self.recorder_focus.is_focused(window);
        let ring_soft_3 = BoxShadow {
            color: tokens::ring_soft(),
            offset: point(px(0.), px(0.)),
            blur_radius: px(0.),
            spread_radius: px(3.),
            inset: false,
        };

        // .studio-field.recorder-btn — styles.css:554-593
        let hint = if show_chord {
            // .chord-display — styles.css:595-598
            h_flex()
                .id("chord-display")
                .gap(px(4.))
                .children(chord::keys(&trigger).into_iter().map(widgets::chord_key))
                .into_any_element()
        } else {
            // .recorder-hint-text — styles.css:583-586
            div()
                .text_size(px(12.))
                .line_height(px(18.))
                .text_color(tokens::ink_2())
                .child(if recording {
                    "Press keys on keyboard… Esc cancels"
                } else {
                    "Click to Record Shortcut Chord"
                })
                .into_any_element()
        };

        let btn = div()
            .id("recorder-btn")
            .track_focus(&self.recorder_focus)
            .h(px(34.))
            .px(px(12.))
            .flex()
            .items_center()
            .w_full()
            .bg(tokens::field())
            .border_1()
            .border_color(if recording || focused {
                tokens::accent()
            } else {
                tokens::line()
            })
            .rounded(px(7.))
            .cursor(CursorStyle::PointingHand)
            // `.studio-field:focus` 3px RING_SOFT when focused; pulse overrides while live.
            .when(focused && !(recording && !freeze), {
                let ring = ring_soft_3.clone();
                move |this| this.shadow(vec![ring])
            })
            .when(recording && freeze && !focused, {
                let ring = ring_soft_3.clone();
                move |this| this.shadow(vec![ring])
            })
            .focus_visible({
                let ring = ring_soft_3.clone();
                move |s| {
                    let mut shadows = vec![ring];
                    shadows.extend(tokens::focus_visible_shadows(tokens::field()));
                    s.shadow(shadows)
                }
            })
            .on_click(move |_, window, cx| {
                entity.update(cx, |this, cx| {
                    this.start_shortcut_recording(window, cx);
                });
            })
            .child(hint);

        // listening pulse: 1.2s ease-in-out infinite; freeze at t=0 uses static ring above.
        if recording && !freeze {
            let ease = Easing::EaseInOut;
            btn.with_animation(
                "recorder-listening",
                Animation::new(tokens::LISTENING)
                    .repeat()
                    .with_easing(move |t| {
                        let phase = if t <= 0.5 { t * 2.0 } else { (1.0 - t) * 2.0 };
                        ease.sample(phase)
                    }),
                |this, pulse| {
                    this.shadow(vec![BoxShadow {
                        color: tokens::ring_soft(),
                        offset: point(px(0.), px(0.)),
                        blur_radius: px(0.),
                        spread_radius: px(4. * pulse),
                        inset: false,
                    }])
                },
            )
            .into_any_element()
        } else {
            btn.into_any_element()
        }
    }

    fn render_token_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        // .token-bar — styles.css:660-665
        h_flex()
            .id("token-bar")
            .items_center()
            .gap(px(6.))
            .mt(px(2.))
            .child(
                // .token-bar-label — styles.css:666-669
                // Explicit 16.5px line box (11 × 1.5); rule 23.
                div()
                    .text_size(px(11.))
                    .line_height(px(16.5))
                    .text_color(tokens::ink_3())
                    .child("Insert Token:"),
            )
            .child(token_btn("{date}", "Current Date (YYYY-MM-DD)", {
                let entity = entity.clone();
                move |window, cx| {
                    entity.update(cx, |this, cx| {
                        this.insert_token("{date}", window, cx);
                    });
                }
            }))
            .child(token_btn(
                "{time}",
                "12-Hour Time with AM/PM (e.g. 1:15 AM)",
                {
                    let entity = entity.clone();
                    move |window, cx| {
                        entity.update(cx, |this, cx| {
                            this.insert_token("{time}", window, cx);
                        });
                    }
                },
            ))
            .child(token_btn("{clipboard}", "Current Clipboard Text", {
                let entity = entity.clone();
                move |window, cx| {
                    entity.update(cx, |this, cx| {
                        this.insert_token("{clipboard}", window, cx);
                    });
                }
            }))
            .child(token_btn_all("View All Tokens & Modifiers", {
                let entity = entity.clone();
                move |_, cx| {
                    entity.update(cx, |this, cx| {
                        this.open_tokens_modal(cx);
                    });
                }
            }))
    }

    fn render_textarea(&self, focused: bool, expansion_text: &str) -> impl IntoElement {
        // .giant-editor-container — styles.css:688-693
        // .giant-editor-textarea — styles.css:695-714
        // Kit Medium editor paddings = 10/8 (sizing.rs). Style pads 6/6 → total inset
        // 16px L/R and 14px top (rule 17). Kit placeholder empty; wrap ourselves.
        let show_placeholder = expansion_text.is_empty();
        div()
            .id("giant-editor-container")
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(
                div()
                    .id("giant-editor-frame")
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .bg(tokens::field())
                    .border_1()
                    .border_color(if focused {
                        tokens::accent()
                    } else {
                        tokens::line()
                    })
                    .rounded(px(8.))
                    .when(focused, |this| {
                        this.shadow(vec![BoxShadow {
                            color: tokens::ring_soft(),
                            offset: point(px(0.), px(0.)),
                            blur_radius: px(0.),
                            spread_radius: px(3.),
                            inset: false,
                        }])
                    })
                    .child(
                        Textarea::new(&self.expansion)
                            .appearance(false)
                            .bordered(false)
                            .w_full()
                            .h_full()
                            // Kit Input defaults to items_center; CSS textarea is top-aligned.
                            .items_start()
                            .justify_start()
                            .px(px(6.))
                            .py(px(6.))
                            .text_size(px(13.5))
                            .line_height(relative(1.6))
                            .text_color(tokens::ink())
                            .font_family(tokens::FONT_UI),
                    )
                    .when(show_placeholder, |this| {
                        this.child(
                            // Soft-wrapping placeholder at the CSS content origin (14/16).
                            div()
                                .id("expansion-placeholder")
                                .absolute()
                                .top(px(14.))
                                .left(px(16.))
                                .right(px(16.))
                                .font_family(tokens::FONT_UI)
                                .text_size(px(13.5))
                                .line_height(relative(1.6))
                                .text_color(tokens::ink_3())
                                .child(EXPANSION_PLACEHOLDER.to_string()),
                        )
                    }),
            )
    }

    #[allow(clippy::too_many_arguments)]
    fn render_footer(
        &self,
        lines: usize,
        words: usize,
        chars: usize,
        form_success: &str,
        form_error: &str,
        show_delete: bool,
        can_save: bool,
        save_label: &str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let entity = cx.entity();
        let line_label = if lines == 1 { "line" } else { "lines" };
        let word_label = if words == 1 { "word" } else { "words" };

        // .canvas-footer — styles.css:717-722
        h_flex()
            .id("canvas-footer")
            .justify_between()
            .items_center()
            .pt(px(4.))
            .child(
                // .canvas-footer-stats — styles.css:724-731
                // flex_1 + min_w_0 so a long error truncates and Save stays on-canvas.
                h_flex()
                    .id("canvas-footer-stats")
                    .flex_1()
                    .min_w_0()
                    .items_center()
                    .gap(px(6.))
                    .font_family(tokens::FONT_MONO)
                    .text_size(px(11.5))
                    .text_color(tokens::ink_3())
                    .child(
                        h_flex()
                            .gap(px(4.))
                            .flex_shrink_0()
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .child(format!("{lines}")),
                            )
                            .child(line_label.to_string()),
                    )
                    .child(
                        // .stat-sep
                        div()
                            .flex_shrink_0()
                            .text_color(tokens::line_2())
                            .child("•"),
                    )
                    .child(
                        h_flex()
                            .gap(px(4.))
                            .flex_shrink_0()
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .child(format!("{words}")),
                            )
                            .child(word_label.to_string()),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_color(tokens::line_2())
                            .child("•"),
                    )
                    .child(
                        h_flex()
                            .gap(px(4.))
                            .flex_shrink_0()
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .child(format!("{chars}")),
                            )
                            .child("chars"),
                    )
                    .when(!form_success.is_empty(), |this| {
                        this.child(
                            // .success-message — styles.css:733-740
                            h_flex()
                                .id("success-message")
                                .items_center()
                                .gap(px(4.))
                                .ml(px(8.))
                                .min_w_0()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(tokens::success())
                                .child(icons::check())
                                .child(
                                    div()
                                        .min_w_0()
                                        .truncate()
                                        .child(form_success.to_string()),
                                ),
                        )
                    })
                    .when(!form_error.is_empty(), |this| {
                        this.child(
                            // .error-message — styles.css:741-748
                            h_flex()
                                .id("error-message")
                                .items_center()
                                .gap(px(4.))
                                .ml(px(8.))
                                .min_w_0()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(tokens::danger())
                                .child(icons::close())
                                .child(
                                    div()
                                        .min_w_0()
                                        .truncate()
                                        .child(form_error.to_string()),
                                ),
                        )
                    }),
            )
            .child(
                // .canvas-actions — styles.css:750-754
                h_flex()
                    .id("canvas-actions")
                    .items_center()
                    .gap(px(8.))
                    .when(show_delete, {
                        let entity = entity.clone();
                        move |this| {
                            this.child(widgets::btn_danger_outline("btn-delete", "Delete", {
                                move |window, cx| {
                                    entity.update(cx, |this, cx| {
                                        this.delete_selected(window, cx);
                                    });
                                }
                            }))
                        }
                    })
                    .child(widgets::btn_accent(
                        "btn-save-snippet",
                        save_label.to_string(),
                        can_save,
                        {
                            let entity = entity.clone();
                            move |window, cx| {
                                entity.update(cx, |this, cx| {
                                    this.save_or_add(window, cx);
                                });
                            }
                        },
                    )),
            )
    }
}

/// `.token-btn` — styles.css:670-685
fn token_btn(
    label: &'static str,
    tip: &'static str,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(SharedString::from(format!("token-btn-{label}")))
        .tab_index(0)
        .bg(tokens::surface_1())
        .border_1()
        .border_color(tokens::line())
        .text_color(tokens::ink_2())
        .font_family(tokens::FONT_MONO)
        .text_size(px(11.))
        .line_height(px(16.5))
        // Chromium border-box ≈ 24.5; reference box is 25px. Pin 25.
        .h(px(25.))
        .flex()
        .items_center()
        .px(px(8.))
        .rounded(px(5.))
        .cursor(CursorStyle::PointingHand)
        .hover(|s| {
            s.border_color(tokens::accent())
                .text_color(tokens::white())
                .bg(tokens::surface_2())
        })
        .focus_visible(|s| s.shadow(tokens::focus_visible_shadows(tokens::bg())))
        .tooltip(move |window, cx| Tooltip::new(tip).build(window, cx))
        .on_click(move |_, window, cx| on_click(window, cx))
        .child(label.to_string())
}

/// `.token-btn-all` — styles.css:979-993 (separate from mono token pills).
fn token_btn_all(
    tip: &'static str,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id("token-btn-all")
        .tab_index(0)
        .ml_auto()
        .bg(tokens::surface_2())
        .border_1()
        .border_color(tokens::line_2())
        .text_color(tokens::accent_ink())
        .font_family(tokens::FONT_UI)
        .text_size(px(11.))
        .line_height(px(16.5))
        .font_weight(FontWeight::SEMIBOLD)
        // Match token-btn height; reference box is 25px.
        .h(px(25.))
        .flex()
        .items_center()
        .px(px(10.))
        .rounded(px(5.))
        .cursor(CursorStyle::PointingHand)
        .hover(|s| {
            s.bg(tokens::surface_3())
                .text_color(tokens::white())
                .border_color(tokens::accent())
        })
        .focus_visible(|s| s.shadow(tokens::focus_visible_shadows(tokens::bg())))
        .tooltip(move |window, cx| Tooltip::new(tip).build(window, cx))
        .on_click(move |_, window, cx| on_click(window, cx))
        .child("All Tokens")
}
