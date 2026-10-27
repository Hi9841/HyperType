//! The settings panel, shown in a dialog. It is built each time the dialog
//! opens and dropped when it closes, which also stops its status refresh.

use std::time::Duration;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::slider::{Slider, SliderEvent, SliderState};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::component::StyledExt as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use hypertype_core::app_state::AppState;
use hypertype_core::commands::{self, Status};
use hypertype_core::expansion::{InsertMode, PasteCombo};

use crate::shell::{self, Shell};
use crate::widgets::segmented;
use crate::autostart;

const INSERT_MODES: &[(InsertMode, &str)] = &[
    (InsertMode::Auto, "Auto"),
    (InsertMode::Paste, "Paste"),
    (InsertMode::Type, "Type"),
];

const PASTE_COMBOS: &[(PasteCombo, &str)] = &[
    (PasteCombo::CtrlV, "Ctrl+V"),
    (PasteCombo::ShiftInsert, "Shift+Ins"),
    (PasteCombo::CtrlShiftV, "Ctrl+Shift+V"),
];

/// The hook counter moves while the panel is open; v1.x refreshed it at
/// the same rate.
const REFRESH: Duration = Duration::from_millis(1500);

type Commit = fn(&AppState, u32) -> Result<(), String>;

/// A bounded number with a slider and an editable value.
struct NumberSetting {
    slider: Entity<SliderState>,
    input: Entity<InputState>,
    min: u32,
    max: u32,
}

pub struct SettingsPanel {
    status: Status,
    autostart: bool,
    auto_words: NumberSetting,
    wpm: NumberSetting,
    restore_ms: NumberSetting,
    hook_message: Option<SharedString>,
    error: Option<SharedString>,
    _refresh: Task<()>,
    _subscriptions: Vec<Subscription>,
}

impl SettingsPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let status = commands::status(&cx.global::<Shell>().state);
        let mut subscriptions = vec![cx.observe_global::<Shell>(|this, cx| this.refresh(cx))];
        let auto_words = Self::number(
            (1, 100, 1),
            status.auto_paste_words,
            commands::set_auto_paste_words,
            &mut subscriptions,
            window,
            cx,
        );
        let wpm = Self::number((100, 1500, 50), status.wpm, commands::set_wpm, &mut subscriptions, window, cx);
        let restore_ms = Self::number(
            (3000, 15000, 100),
            status.restore_delay_ms,
            commands::set_restore_delay_ms,
            &mut subscriptions,
            window,
            cx,
        );
        let refresh = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(REFRESH).await;
                if this.update(cx, |this, cx| this.refresh(cx)).is_err() {
                    break;
                }
            }
        });
        Self {
            status,
            autostart: autostart::is_enabled(),
            auto_words,
            wpm,
            restore_ms,
            hook_message: None,
            error: None,
            _refresh: refresh,
            _subscriptions: subscriptions,
        }
    }

    fn number(
        (min, max, step): (u32, u32, u32),
        value: u32,
        commit: Commit,
        subscriptions: &mut Vec<Subscription>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> NumberSetting {
        let slider = cx.new(|_| {
            SliderState::new()
                .min(min as f32)
                .max(max as f32)
                .step(step as f32)
                .default_value(value as f32)
        });
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(value.to_string())
                .validate(|text, _| text.len() <= 5 && text.chars().all(|c| c.is_ascii_digit()))
        });

        let field = input.clone();
        subscriptions.push(cx.subscribe_in(&slider, window, move |this, _, event, window, cx| {
            let (SliderEvent::Change(value) | SliderEvent::Release(value)) = event;
            let value = value.start().round() as u32;
            field.update(cx, |input, cx| input.set_value(value.to_string(), window, cx));
            if matches!(event, SliderEvent::Release(_)) {
                this.apply(commit, value, cx);
            }
        }));

        let bar = slider.clone();
        subscriptions.push(cx.subscribe_in(&input, window, move |this, input, event, window, cx| {
            if !matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                return;
            }
            let current = bar.read(cx).value().start().round() as u32;
            let value = input
                .read(cx)
                .value()
                .parse::<u32>()
                .map_or(current, |v| v.clamp(min, max));
            input.update(cx, |input, cx| input.set_value(value.to_string(), window, cx));
            bar.update(cx, |bar, cx| bar.set_value(value as f32, window, cx));
            if value != current {
                this.apply(commit, value, cx);
            }
        }));

        NumberSetting {
            slider,
            input,
            min,
            max,
        }
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        self.status = commands::status(&cx.global::<Shell>().state);
        cx.notify();
    }

    fn apply(&mut self, commit: Commit, value: u32, cx: &mut Context<Self>) {
        let result = commit(&cx.global::<Shell>().state, value);
        self.report(result, cx);
    }

    fn report(&mut self, result: Result<(), String>, cx: &mut Context<Self>) {
        self.error = result.err().map(Into::into);
        self.refresh(cx);
    }

    fn set_autostart(&mut self, enabled: bool, cx: &mut Context<Self>) {
        let result = autostart::set_enabled(enabled);
        self.autostart = autostart::is_enabled();
        self.report(result, cx);
    }

    fn repair_hook(&mut self, cx: &mut Context<Self>) {
        self.hook_message = Some(match commands::reinstall_hook() {
            Ok(true) => "Hook reconnected".into(),
            Ok(false) | Err(_) => "Could not reinstall the hook".into(),
        });
        self.refresh(cx);
    }

    fn run_as_admin(&mut self, cx: &mut Context<Self>) {
        // On success the process exits and the elevated copy takes over.
        if let Err(e) = hypertype_core::platform::restart_elevated() {
            self.error = Some(e.into());
            cx.notify();
        }
    }
}

impl Render for SettingsPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let status = &self.status;
        let entity = cx.entity().downgrade();
        let update = move |f: fn(&mut Self, &mut Context<Self>)| {
            let entity = entity.clone();
            move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                entity.update(cx, |this, cx| f(this, cx)).ok();
            }
        };

        let insert_desc: SharedString = match status.insert_mode {
            InsertMode::Auto => format!(
                "Types up to {} words at the set speed and pastes longer text.",
                status.auto_paste_words
            )
            .into(),
            InsertMode::Paste => "Always pastes through the clipboard.".into(),
            InsertMode::Type => "Always types at the set speed.".into(),
        };
        let (hook_label, hook_ok) = match (status.hook_active, status.hook_healthy) {
            (true, true) => ("Active", true),
            (true, false) => ("Active, verifying", true),
            (false, _) => ("Detached", false),
        };
        let mut hook_desc = format!("{hook_label} - {} events seen", group_digits(status.hook_events));
        if let Some(message) = &self.hook_message {
            hook_desc.push_str(&format!(" - {message}"));
        }
        let (privilege, privilege_desc) = if status.is_elevated {
            ("Administrator", "Can type into elevated terminals and apps.")
        } else {
            ("Standard user", "Cannot type into apps that run as administrator.")
        };

        let entity = cx.entity().downgrade();
        let insert_mode = segmented("insert-mode", INSERT_MODES, status.insert_mode, {
            let entity = entity.clone();
            move |mode, _, cx| {
                entity
                    .update(cx, |this, cx| {
                        let result = commands::set_insert_mode(&cx.global::<Shell>().state, mode);
                        this.report(result, cx);
                    })
                    .ok();
            }
        }, cx);
        let paste_combo = segmented("paste-combo", PASTE_COMBOS, status.paste_combo, {
            let entity = entity.clone();
            move |combo, _, cx| {
                entity
                    .update(cx, |this, cx| {
                        let result = commands::set_paste_combo(&cx.global::<Shell>().state, combo);
                        this.report(result, cx);
                    })
                    .ok();
            }
        }, cx);

        let enabled = status.enabled;
        let autostart = self.autostart;
        let theme = cx.theme();
        v_flex()
            .gap_5()
            .child(section(
                "General",
                vec![
                    row(
                        "Expansion engine",
                        "Listens for triggers in every app.",
                        Switch::new("engine")
                            .checked(enabled)
                            .on_click(|_, _, cx| {
                                shell::toggle_enabled(cx);
                            }),
                        cx,
                    ),
                    row(
                        "Launch at login",
                        "Starts HyperType in the tray when you sign in.",
                        Switch::new("autostart").checked(autostart).on_click({
                            let entity = entity.clone();
                            move |checked, _, cx| {
                                let checked = *checked;
                                entity.update(cx, |this, cx| this.set_autostart(checked, cx)).ok();
                            }
                        }),
                        cx,
                    ),
                ],
                cx,
            ))
            .child(section(
                "Insertion",
                vec![
                    row("Method", insert_desc, insert_mode, cx),
                    number_row("Auto-paste threshold", "words", &self.auto_words, cx),
                    number_row("Typing speed", "words per minute", &self.wpm, cx),
                    row("Paste shortcut", "Sent to the app when text is pasted.", paste_combo, cx),
                    number_row(
                        "Clipboard restore delay",
                        "ms until the previous clipboard comes back",
                        &self.restore_ms,
                        cx,
                    ),
                ],
                cx,
            ))
            .child(section(
                "System",
                vec![
                    row_with_dot(
                        "Keyboard hook",
                        hook_desc,
                        hook_ok,
                        Button::new("repair-hook")
                            .outline()
                            .small()
                            .label("Repair hook")
                            .on_click(update(Self::repair_hook)),
                        cx,
                    ),
                    row_with_dot(
                        privilege,
                        privilege_desc,
                        status.is_elevated,
                        Button::new("run-as-admin")
                            .outline()
                            .small()
                            .label("Run as administrator")
                            .disabled(status.is_elevated)
                            .on_click(update(Self::run_as_admin)),
                        cx,
                    ),
                ],
                cx,
            ))
            .children(
                self.error
                    .clone()
                    .map(|e| div().text_sm().text_color(theme.danger).child(e)),
            )
            .child(
                h_flex()
                    .justify_between()
                    .pt_3()
                    .border_t_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child(format!("HyperType v{}", status.version)),
                    )
                    .child(
                        Button::new("quit")
                            .ghost()
                            .small()
                            .label("Quit HyperType")
                            .on_click(|_, _, cx| cx.quit()),
                    ),
            )
    }
}

fn section(title: &'static str, rows: Vec<AnyElement>, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    v_flex()
        .gap_1()
        .child(
            div()
                .text_xs()
                .font_medium()
                .text_color(theme.muted_foreground)
                .child(title),
        )
        .child(
            v_flex()
                .rounded(theme.radius_lg)
                .border_1()
                .border_color(theme.border)
                .children(rows.into_iter().enumerate().map(|(ix, row)| {
                    div()
                        .when(ix > 0, |d| d.border_t_1().border_color(theme.border))
                        .child(row)
                })),
        )
}

fn row(
    title: impl Into<SharedString>,
    desc: impl Into<SharedString>,
    control: impl IntoElement,
    cx: &App,
) -> AnyElement {
    let theme = cx.theme();
    h_flex()
        .justify_between()
        .gap_4()
        .px_4()
        .py_3()
        .child(
            v_flex()
                .min_w_0()
                .gap_0p5()
                .child(div().text_sm().font_medium().child(title.into()))
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(desc.into()),
                ),
        )
        .child(div().flex_none().child(control))
        .into_any_element()
}

/// A row whose description starts with a status dot: cyan when good.
fn row_with_dot(
    title: impl Into<SharedString>,
    desc: impl Into<SharedString>,
    ok: bool,
    control: impl IntoElement,
    cx: &App,
) -> AnyElement {
    let theme = cx.theme();
    let dot = div()
        .size(px(6.))
        .rounded_full()
        .bg(if ok { theme.success } else { theme.muted_foreground });
    h_flex()
        .justify_between()
        .gap_4()
        .px_4()
        .py_3()
        .child(
            v_flex()
                .min_w_0()
                .gap_0p5()
                .child(div().text_sm().font_medium().child(title.into()))
                .child(
                    h_flex()
                        .gap_1p5()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(dot)
                        .child(desc.into()),
                ),
        )
        .child(div().flex_none().child(control))
        .into_any_element()
}

fn number_row(title: &'static str, unit: &'static str, setting: &NumberSetting, cx: &App) -> AnyElement {
    let theme = cx.theme();
    h_flex()
        .gap_4()
        .px_4()
        .py_3()
        .child(
            v_flex()
                .w(px(170.))
                .flex_none()
                .gap_0p5()
                .child(div().text_sm().font_medium().child(title))
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(format!("{}-{} {unit}", setting.min, setting.max)),
                ),
        )
        .child(div().flex_1().child(Slider::new(&setting.slider)))
        .child(div().w(px(72.)).flex_none().child(Input::new(&setting.input).small()))
        .into_any_element()
}

fn group_digits(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (ix, ch) in digits.chars().enumerate() {
        if ix > 0 && (digits.len() - ix) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}
