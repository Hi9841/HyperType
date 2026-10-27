//! The snippet editor: trigger (text or recorded chord), expansion, and
//! save. Deletion goes to the manager, which owns the confirmation.

use std::time::Duration;

use gpui_kit::assets::IconName as Lucide;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState, Textarea, TextareaState};
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, Sizable as _, h_flex, v_flex,
};
use gpui_kit::component::StyledExt as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use hypertype_core::commands::{self, SnippetView};
use hypertype_core::snippets::{MAX_TEXT_TRIGGER_CHARS, TriggerKind};

use crate::shell::Shell;
use crate::widgets::{segmented, trigger_badge};
use crate::{chord, tokens};

const KINDS: &[(TriggerKind, &str)] = &[(TriggerKind::Text, "Text"), (TriggerKind::Shortcut, "Shortcut")];

pub enum EditorEvent {
    /// The snippet was stored under this trigger.
    Saved(String),
    DeleteRequested(String),
}

enum Notice {
    Success(SharedString),
    Error(SharedString),
}

pub struct Editor {
    /// The stored trigger of the snippet being edited; `None` for a new one.
    editing: Option<String>,
    kind: TriggerKind,
    trigger: Entity<InputState>,
    chord: Option<String>,
    recorder: FocusHandle,
    recording: Option<Subscription>,
    expansion: Entity<TextareaState>,
    notice: Option<Notice>,
    notice_timer: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<EditorEvent> for Editor {}

impl Editor {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let trigger = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("e.g. gm, sig, addr")
                .validate(|text, _| text.chars().count() <= MAX_TEXT_TRIGGER_CHARS)
        });
        let expansion = cx.new(|cx| {
            TextareaState::new(window, cx).placeholder(
                "Type or paste the text to insert. Paragraphs, templates, addresses, and code all work.",
            )
        });
        let _subscriptions = vec![
            cx.subscribe(&trigger, Self::on_input),
            cx.subscribe(&expansion, Self::on_input),
            // Leaving the window cancels recording, so keys are never
            // swallowed while the user is elsewhere.
            cx.observe_window_activation(window, |this, window, cx| {
                if !window.is_window_active() {
                    this.stop_recording(cx);
                }
            }),
        ];
        Self {
            editing: None,
            kind: TriggerKind::Text,
            trigger,
            chord: None,
            recorder: cx.focus_handle().tab_stop(true),
            recording: None,
            expansion,
            notice: None,
            notice_timer: None,
            _subscriptions,
        }
    }

    fn on_input<T>(&mut self, _: Entity<T>, event: &InputEvent, cx: &mut Context<Self>) {
        if matches!(event, InputEvent::Change) {
            if matches!(self.notice, Some(Notice::Error(_))) {
                self.notice = None;
            }
            cx.notify();
        }
    }

    /// Show a snippet, or an empty form for a new one.
    pub fn load(&mut self, snippet: Option<&SnippetView>, window: &mut Window, cx: &mut Context<Self>) {
        self.stop_recording(cx);
        self.editing = snippet.map(|s| s.trigger.clone());
        self.kind = snippet.map_or(TriggerKind::Text, |s| s.kind);
        let text_trigger = match snippet {
            Some(s) if s.kind == TriggerKind::Text => s.trigger.clone(),
            _ => String::new(),
        };
        self.chord = snippet
            .filter(|s| s.kind == TriggerKind::Shortcut)
            .map(|s| s.trigger.clone());
        self.trigger
            .update(cx, |input, cx| input.set_value(text_trigger, window, cx));
        let expansion = snippet.map(|s| s.expansion.clone()).unwrap_or_default();
        self.expansion
            .update(cx, |input, cx| input.set_value(expansion, window, cx));
        self.notice = None;
        cx.notify();
    }

    pub fn focus_trigger(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.kind {
            TriggerKind::Text => self.trigger.update(cx, |input, cx| input.focus(window, cx)),
            TriggerKind::Shortcut => self.recorder.focus(window, cx),
        }
    }

    pub fn insert_token(&mut self, token: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.expansion.update(cx, |input, cx| {
            input.insert(token.to_string(), window, cx);
            input.focus(window, cx);
        });
        cx.notify();
    }

    fn set_kind(&mut self, kind: TriggerKind, window: &mut Window, cx: &mut Context<Self>) {
        if self.kind == kind {
            return;
        }
        self.stop_recording(cx);
        self.kind = kind;
        self.chord = None;
        self.trigger
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.notice = None;
        self.focus_trigger(window, cx);
        cx.notify();
    }

    fn start_recording(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.recording.is_some() {
            return;
        }
        self.chord = None;
        self.notice = None;
        self.recorder.focus(window, cx);
        // An interceptor sees the key before any binding, so Ctrl+S records
        // as a chord instead of saving.
        let editor = cx.entity().downgrade();
        self.recording = Some(cx.intercept_keystrokes(move |event, window, cx| {
            cx.stop_propagation();
            let keystroke = event.keystroke.clone();
            editor
                .update(cx, |this, cx| this.record(&keystroke, window, cx))
                .ok();
        }));
        cx.notify();
    }

    fn record(&mut self, keystroke: &Keystroke, window: &mut Window, cx: &mut Context<Self>) {
        if keystroke.key == "escape" && !keystroke.modifiers.modified() {
            self.stop_recording(cx);
            return;
        }
        if let Some(chord) = chord::from_keystroke(keystroke) {
            self.chord = Some(chord);
            self.stop_recording(cx);
            self.expansion
                .update(cx, |input, cx| input.focus(window, cx));
        }
    }

    fn stop_recording(&mut self, cx: &mut Context<Self>) {
        if self.recording.take().is_some() {
            cx.notify();
        }
    }

    fn trigger_value(&self, cx: &App) -> String {
        match self.kind {
            TriggerKind::Text => self.trigger.read(cx).value().trim().to_string(),
            TriggerKind::Shortcut => self.chord.clone().unwrap_or_default(),
        }
    }

    pub fn save(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let trigger = self.trigger_value(cx);
        let expansion = self.expansion.read(cx).value().to_string();
        if trigger.is_empty() || expansion.is_empty() {
            return;
        }
        let shell = cx.global::<Shell>();
        let result = match &self.editing {
            None => commands::add_snippet(&shell.state, &shell.hotkeys, &trigger, expansion, self.kind),
            Some(old) => commands::edit_snippet(
                &shell.state,
                &shell.hotkeys,
                old,
                &trigger,
                expansion,
                self.kind,
            ),
        };
        match result {
            Ok(()) => {
                let message = if self.editing.is_some() { "Saved" } else { "Snippet created" };
                self.editing = Some(trigger.clone());
                self.show(Notice::Success(message.into()), cx);
                cx.emit(EditorEvent::Saved(trigger));
            }
            Err(e) => self.show(Notice::Error(e.into()), cx),
        }
    }

    pub fn request_delete(&mut self, cx: &mut Context<Self>) {
        if let Some(trigger) = self.editing.clone() {
            cx.emit(EditorEvent::DeleteRequested(trigger));
        }
    }

    pub fn show_error(&mut self, message: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.show(Notice::Error(message.into()), cx);
    }

    fn show(&mut self, notice: Notice, cx: &mut Context<Self>) {
        let clears = matches!(notice, Notice::Success(_));
        self.notice = Some(notice);
        self.notice_timer = clears.then(|| {
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(Duration::from_secs(3)).await;
                this.update(cx, |this, cx| {
                    this.notice = None;
                    cx.notify();
                })
                .ok();
            })
        });
        cx.notify();
    }

    fn render_recorder(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let recording = self.recording.is_some();
        let content: AnyElement = match (&self.chord, recording) {
            (_, true) => div()
                .text_color(theme.foreground)
                .child("Press the keys. Esc cancels.")
                .into_any_element(),
            (Some(chord), false) => trigger_badge(chord, TriggerKind::Shortcut, cx).into_any_element(),
            (None, false) => div()
                .text_color(theme.muted_foreground)
                .child("Click to record a shortcut")
                .into_any_element(),
        };
        let theme = cx.theme();
        h_flex()
            .id("chord-recorder")
            .track_focus(&self.recorder)
            .h_9()
            .px_3()
            .gap_2()
            .rounded(theme.radius)
            .border_1()
            .border_color(if recording { theme.ring } else { theme.input })
            .bg(theme.background)
            .text_sm()
            .cursor_pointer()
            .child(
                Icon::new(Lucide::Keyboard)
                    .small()
                    .text_color(theme.muted_foreground),
            )
            .child(content)
            .on_click(cx.listener(|this, _, window, cx| this.start_recording(window, cx)))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                let key = event.keystroke.key.as_str();
                if this.recording.is_none() && (key == "enter" || key == "space") {
                    this.start_recording(window, cx);
                    cx.stop_propagation();
                }
            }))
            .on_mouse_down_out(cx.listener(|this, _, _, cx| this.stop_recording(cx)))
    }
}

impl Render for Editor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let expansion = self.expansion.read(cx).value();
        let lines = if expansion.is_empty() { 0 } else { expansion.lines().count().max(1) };
        let words = expansion.split_whitespace().count();
        let chars = expansion.chars().count();
        let can_save = !expansion.is_empty() && !self.trigger_value(cx).is_empty();

        let title: SharedString = match &self.editing {
            None => "New snippet".into(),
            Some(t) if self.kind == TriggerKind::Shortcut => format!("Edit {}", chord::display(t)).into(),
            Some(t) => format!("Edit {t}").into(),
        };
        let hint = match self.kind {
            TriggerKind::Text => "Expands automatically at word boundaries.",
            TriggerKind::Shortcut => "Registered as a global Windows shortcut.",
        };

        let trigger_field: AnyElement = match self.kind {
            TriggerKind::Text => Input::new(&self.trigger).into_any_element(),
            TriggerKind::Shortcut => self.render_recorder(cx).into_any_element(),
        };

        let kind_control = segmented(
            "trigger-kind",
            KINDS,
            self.kind,
            {
                let editor = cx.entity().downgrade();
                move |kind, window, cx| {
                    editor.update(cx, |this, cx| this.set_kind(kind, window, cx)).ok();
                }
            },
            cx,
        );
        let editor = cx.entity().downgrade();
        let theme = cx.theme();
        let label = |text: &'static str| {
            div()
                .text_xs()
                .font_medium()
                .text_color(theme.muted_foreground)
                .child(text)
        };

        let token_chips = h_flex()
            .gap_1()
            .children(tokens::QUICK.iter().map(|token| {
                let token = *token;
                Button::new(token)
                    .ghost()
                    .xsmall()
                    .label(token)
                    .on_click(cx.listener(move |this, _, window, cx| this.insert_token(token, window, cx)))
            }))
            .child(
                Button::new("all-tokens")
                    .ghost()
                    .xsmall()
                    .label("All tokens...")
                    .on_click(move |_, window, cx| {
                        let editor = editor.clone();
                        tokens::open(window, cx, move |token, window, cx| {
                            editor
                                .update(cx, |this, cx| this.insert_token(token, window, cx))
                                .ok();
                        });
                    }),
            );

        let notice = self.notice.as_ref().map(|notice| match notice {
            Notice::Success(text) => div().text_color(theme.foreground).child(text.clone()),
            Notice::Error(text) => div().text_color(theme.danger).child(text.clone()),
        });

        v_flex()
            .size_full()
            .min_w_0()
            .bg(theme.background)
            .child(
                h_flex()
                    .justify_between()
                    .gap_4()
                    .px_6()
                    .pt_5()
                    .pb_4()
                    .child(
                        v_flex()
                            .min_w_0()
                            .gap_0p5()
                            .child(div().text_lg().font_semibold().truncate().child(title))
                            .child(div().text_sm().text_color(theme.muted_foreground).child(hint)),
                    )
                    .child(kind_control),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .px_6()
                    .pb_4()
                    .gap_4()
                    .child(
                        v_flex()
                            .gap_1p5()
                            .child(label(match self.kind {
                                TriggerKind::Text => "Trigger",
                                TriggerKind::Shortcut => "Shortcut",
                            }))
                            .child(trigger_field),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_h_0()
                            .gap_1p5()
                            .child(
                                h_flex()
                                    .justify_between()
                                    .child(label("Expansion"))
                                    .child(token_chips),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_h_0()
                                    .child(Textarea::new(&self.expansion).h(relative(1.))),
                            ),
                    ),
            )
            .child(
                h_flex()
                    .justify_between()
                    .gap_4()
                    .px_6()
                    .py_3()
                    .border_t_1()
                    .border_color(theme.border)
                    .child(
                        h_flex()
                            .min_w_0()
                            .gap_4()
                            .text_xs()
                            .child(
                                div()
                                    .text_color(theme.muted_foreground)
                                    .child(format!(
                                        "{lines} {} - {words} {} - {chars} {}",
                                        plural(lines, "line"),
                                        plural(words, "word"),
                                        plural(chars, "char"),
                                    )),
                            )
                            .children(notice.map(|n| n.truncate())),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .when(self.editing.is_some(), |row| {
                                row.child(
                                    Button::new("delete")
                                        .ghost()
                                        .small()
                                        .label("Delete...")
                                        .on_click(cx.listener(|this, _, _, cx| this.request_delete(cx))),
                                )
                            })
                            .child(
                                Button::new("save")
                                    .primary()
                                    .small()
                                    .label(if self.editing.is_some() { "Save" } else { "Create" })
                                    .disabled(!can_save)
                                    .on_click(cx.listener(|this, _, window, cx| this.save(window, cx))),
                            ),
                    ),
            )
    }
}

fn plural(n: usize, word: &str) -> String {
    if n == 1 { word.to_string() } else { format!("{word}s") }
}
