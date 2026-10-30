//! Shared studio widgets. Each item maps one CSS rule set; the doc comment
//! names the CSS lines.

#![allow(clippy::too_many_arguments)]

use std::rc::Rc;

use gpui_kit::base::transition;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use hypertype_core::snippets::TriggerKind;

use crate::chord;
use crate::ui::icons;
use crate::ui::tokens;

/// `.trigger-keycap` — styles.css:413-422
pub fn keycap(label: impl Into<SharedString>) -> Div {
    div()
        .font_family(tokens::FONT_MONO)
        .text_size(px(11.))
        .font_weight(FontWeight::BOLD)
        .text_color(tokens::accent_ink())
        .bg(tokens::bg())
        .border_1()
        .border_color(tokens::line())
        .px(px(6.))
        .py(px(1.))
        .rounded(px(4.))
        .child(label.into())
}

/// `.chord-key` — styles.css:428-437
pub fn chord_key(label: impl Into<SharedString>) -> Div {
    div()
        .font_family(tokens::FONT_MONO)
        .text_size(px(10.5))
        .font_weight(FontWeight::BOLD)
        .text_color(tokens::accent_ink())
        .bg(tokens::bg())
        .border_1()
        .border_color(tokens::line())
        .px(px(5.))
        .py(px(1.))
        .rounded(px(4.))
        .child(label.into())
}

/// `.chord-badge` — styles.css:424-427; uses `chord::keys`
pub fn chord_badge(trigger: &str) -> Div {
    h_flex()
        .gap(px(3.))
        .children(chord::keys(trigger).into_iter().map(chord_key))
}

/// Text trigger as one keycap, shortcut as a chord badge.
pub fn trigger_badge(trigger: &str, kind: TriggerKind) -> Div {
    match kind {
        TriggerKind::Text => h_flex().min_w_0().child(keycap(trigger.to_string()).truncate()),
        TriggerKind::Shortcut => chord_badge(trigger),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SegKind {
    Two,
    Three,
}

/// `.seg` / `.seg-kind` / `.seg-3` / `.seg-combo` — styles.css:601-657
/// Animated thumb uses SMOOTH + SPRING. `freeze` forces t=0 (parity).
pub fn segmented<T: Copy + PartialEq + 'static>(
    id: impl Into<ElementId>,
    options: &[(T, &str)],
    current: T,
    kind: SegKind,
    width: Pixels,
    freeze: bool,
    on_change: impl Fn(T, &mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let id = id.into();
    let selected = options
        .iter()
        .position(|(v, _)| *v == current)
        .unwrap_or(0);
    let track_inner = width - px(4.);
    let thumb_w = match kind {
        SegKind::Two => track_inner / 2.,
        SegKind::Three => track_inner / 3.,
    };
    let target_x = thumb_w * selected as f32;
    let policy = tokens::motion(tokens::transition_spring(), freeze);
    let x = transition(
        SharedString::from(format!("seg-thumb-{selected}")),
        target_x,
        policy,
        window,
        cx,
    );
    let on_change = Rc::new(on_change);

    div()
        .id(id)
        .relative()
        .flex()
        .w(width)
        .h(px(30.))
        .p(px(2.))
        .rounded(px(7.))
        .bg(tokens::surface_2())
        .border_1()
        .border_color(tokens::line())
        .child(
            div()
                .absolute()
                .top(px(2.))
                .left(px(2.) + x)
                .w(thumb_w)
                .h(px(26.))
                .rounded(px(5.))
                .bg(tokens::surface_3()),
        )
        .children(options.iter().enumerate().map(|(ix, (value, label))| {
            let active = *value == current;
            let value = *value;
            let on_change = on_change.clone();
            div()
                .id(("seg-opt", ix))
                .relative()
                .flex_1()
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(5.))
                .cursor(CursorStyle::PointingHand)
                .text_size(px(11.5))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(if active {
                    tokens::white()
                } else {
                    tokens::ink_2()
                })
                .hover(|s| s.text_color(tokens::white()))
                .child((*label).to_string())
                .on_click(move |_, window, cx| {
                    on_change(value, window, cx);
                })
        }))
}

/// `.switch` / `.knob` — styles.css:884-912. SMOOTH + SPRING on the knob.
pub fn switch(
    id: impl Into<ElementId>,
    checked: bool,
    freeze: bool,
    on_toggle: impl Fn(bool, &mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let id = id.into();
    let target = if checked { px(16.) } else { px(0.) };
    let policy = tokens::motion(tokens::transition_spring(), freeze);
    let motion_id: SharedString = "switch-knob".into();
    let x = transition(motion_id, target, policy, window, cx);

    div()
        .id(id)
        .relative()
        .flex_none()
        .w(px(38.))
        .h(px(22.))
        .rounded_full()
        .border_1()
        .border_color(if checked {
            tokens::accent()
        } else {
            tokens::control_line()
        })
        .bg(if checked {
            tokens::accent()
        } else {
            tokens::control()
        })
        .cursor(CursorStyle::PointingHand)
        .on_click(move |_, window, cx| on_toggle(!checked, window, cx))
        .child(
            div()
                .absolute()
                .top(px(2.))
                .left(px(2.) + x)
                .w(px(16.))
                .h(px(16.))
                .rounded_full()
                .bg(tokens::white())
                .shadow(vec![BoxShadow {
                    color: tokens::knob_shadow(),
                    offset: point(px(0.), px(1.)),
                    blur_radius: px(2.),
                    spread_radius: px(0.),
                    inset: false,
                }]),
        )
}

#[derive(Clone, Copy, Default)]
struct SliderDrag {
    active: bool,
    origin_x: Pixels,
    width: Pixels,
}

/// `.slider` — styles.css:914-932. 130px track × 4px, 14px white thumb.
/// Drag emits live value; release emits commit.
pub fn slider(
    id: impl Into<ElementId>,
    value: f32,
    min: f32,
    max: f32,
    on_live: impl Fn(f32, &mut Window, &mut App) + 'static,
    on_commit: impl Fn(f32, &mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let id = id.into();
    let range = (max - min).max(f32::EPSILON);
    let t = ((value - min) / range).clamp(0., 1.);
    let track_w = px(130.);
    let thumb = px(14.);
    let x = (track_w - thumb) * t;

    let drag = window.use_keyed_state("slider-drag", cx, |_, _| SliderDrag::default());
    let on_live = Rc::new(on_live);
    let on_commit = Rc::new(on_commit);

    let sample = move |pos_x: Pixels, origin_x: Pixels, width: Pixels| -> f32 {
        let local = pos_x - origin_x - thumb / 2.;
        let t = (local / (width - thumb)).clamp(0., 1.);
        min + t * (max - min)
    };

    div()
        .id(id)
        .relative()
        .w(track_w)
        .h(px(14.))
        .flex()
        .items_center()
        .cursor(CursorStyle::PointingHand)
        .child(
            div()
                .w_full()
                .h(px(4.))
                .rounded_full()
                .bg(tokens::surface_3()),
        )
        .child(
            div()
                .absolute()
                .top(px(0.))
                .left(x)
                .w(thumb)
                .h(thumb)
                .rounded_full()
                .bg(tokens::white())
                .shadow(vec![BoxShadow {
                    color: tokens::slider_thumb_shadow(),
                    offset: point(px(0.), px(1.)),
                    blur_radius: px(3.),
                    spread_radius: px(0.),
                    inset: false,
                }]),
        )
        .on_mouse_down(MouseButton::Left, {
            let on_live = on_live.clone();
            let drag = drag.clone();
            move |ev, window, cx| {
                // Approximate origin from position; refined on click bounds via move.
                drag.update(cx, |d, _| {
                    d.active = true;
                    d.origin_x = ev.position.x - x - thumb / 2.;
                    d.width = track_w;
                });
                let d = *drag.read(cx);
                let v = sample(ev.position.x, d.origin_x, d.width);
                on_live(v, window, cx);
                window.prevent_default();
                cx.stop_propagation();
            }
        })
        .on_mouse_move({
            let on_live = on_live.clone();
            let drag = drag.clone();
            move |ev, window, cx| {
                if !ev.dragging() && !drag.read(cx).active {
                    return;
                }
                if !drag.read(cx).active {
                    return;
                }
                let d = *drag.read(cx);
                let v = sample(ev.position.x, d.origin_x, d.width);
                on_live(v, window, cx);
            }
        })
        .on_mouse_up(MouseButton::Left, {
            let on_commit = on_commit.clone();
            let drag = drag.clone();
            move |ev, window, cx| {
                if !drag.read(cx).active {
                    return;
                }
                let d = *drag.read(cx);
                drag.update(cx, |d, _| d.active = false);
                let v = sample(ev.position.x, d.origin_x, d.width);
                on_commit(v, window, cx);
            }
        })
}

/// Wire a value-edit input: select-all on focus, Enter blurs, blur commits
/// clamped value, empty/NaN reverts. App.tsx:358-373.
///
/// Call from the owning entity's `new` with `cx.subscribe_in`.
pub fn on_value_edit_event(
    input: &Entity<InputState>,
    event: &InputEvent,
    min: f32,
    max: f32,
    current: f32,
    commit: impl FnOnce(f32, &mut Window, &mut App),
    window: &mut Window,
    cx: &mut App,
) {
    match event {
        InputEvent::Focus => {
            input.update(cx, |state, cx| {
                state.select_all(window, cx);
            });
        }
        InputEvent::Blur => {
            let text = input.read(cx).value().to_string();
            let trimmed = text.trim();
            if trimmed.is_empty() {
                input.update(cx, |state, cx| {
                    state.set_value(format!("{current}"), window, cx);
                });
                return;
            }
            let Ok(n) = trimmed.parse::<f32>() else {
                input.update(cx, |state, cx| {
                    state.set_value(format!("{current}"), window, cx);
                });
                return;
            };
            if !n.is_finite() {
                input.update(cx, |state, cx| {
                    state.set_value(format!("{current}"), window, cx);
                });
                return;
            }
            let clamped = n.round().clamp(min, max);
            input.update(cx, |state, cx| {
                state.set_value(format!("{clamped}"), window, cx);
            });
            if (clamped - current).abs() > f32::EPSILON {
                commit(clamped, window, cx);
            }
        }
        InputEvent::PressEnter { .. } => {
            window.blur(cx);
        }
        _ => {}
    }
}

/// `.value-edit` — styles.css:934-945. Kit Input with appearance(false).
pub fn value_edit(input: &Entity<InputState>, wide: bool) -> impl IntoElement {
    div()
        .w(if wide { px(36.) } else { px(32.) })
        .child(
            Input::new(input)
                .appearance(false)
                .text_size(px(12.))
                .font_family(tokens::FONT_MONO)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(tokens::ink())
                .border_b_1()
                .border_color(tokens::line_2()),
        )
}

/// `.studio-field` — styles.css:554-568. Focus ring = 3px RING_SOFT.
pub fn field_frame(focused: bool, child: impl IntoElement) -> impl IntoElement {
    div()
        .h(px(34.))
        .px(px(12.))
        .flex()
        .items_center()
        .bg(tokens::field())
        .border_1()
        .border_color(if focused {
            tokens::accent()
        } else {
            tokens::line()
        })
        .rounded(px(7.))
        .text_color(tokens::ink())
        .text_size(px(13.))
        .when(focused, |this| {
            this.shadow(vec![BoxShadow {
                color: tokens::ring_soft(),
                offset: point(px(0.), px(0.)),
                blur_radius: px(0.),
                spread_radius: px(3.),
                inset: false,
            }])
        })
        .child(child)
}

/// `.hook-status-badge` / `.hook-active` / `.hook-inactive` — styles.css:1161-1178
pub fn hook_badge(active: bool, label: impl Into<SharedString>) -> impl IntoElement {
    let (bg, ink) = if active {
        (tokens::hook_active_bg(), tokens::hook_active_ink())
    } else {
        (tokens::hook_inactive_bg(), tokens::hook_inactive_ink())
    };
    h_flex()
        .items_center()
        .gap(px(6.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_size(px(11.5))
        .px(px(6.))
        .py(px(1.))
        .rounded(px(4.))
        .bg(bg)
        .text_color(ink)
        .child(div().w(px(6.)).h(px(6.)).rounded_full().bg(ink))
        .child(label.into())
}

/// Modal backdrop + card + header. styles.css:799-849.
/// Fade in 150 ms (skipped when `freeze`). Backdrop click and Escape close.
pub fn modal_frame(
    id: impl Into<ElementId>,
    title: impl Into<SharedString>,
    freeze: bool,
    on_close: impl Fn(&mut Window, &mut App) + 'static,
    body: impl IntoElement,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let id = id.into();
    let on_close = Rc::new(on_close);
    let policy = tokens::motion(tokens::transition_modal_fade(), freeze);
    let opacity = transition("modal-fade", 1.0_f32, policy, window, cx);

    let close = on_close.clone();
    div()
        .id(id)
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(tokens::modal_backdrop())
        .opacity(opacity)
        .on_mouse_down(MouseButton::Left, {
            let close = on_close.clone();
            move |_, window, cx| close(window, cx)
        })
        .on_key_down({
            let close = on_close.clone();
            move |ev, window, cx| {
                if ev.keystroke.key == "escape" {
                    close(window, cx);
                    cx.stop_propagation();
                }
            }
        })
        .child(
            v_flex()
                .w(px(500.))
                .max_h(relative(0.85))
                .bg(tokens::surface_1())
                .border_1()
                .border_color(tokens::line_2())
                .rounded(px(12.))
                .shadow(vec![BoxShadow {
                    color: tokens::modal_shadow(),
                    offset: point(px(0.), px(20.)),
                    blur_radius: px(60.),
                    spread_radius: px(0.),
                    inset: false,
                }])
                .on_mouse_down(MouseButton::Left, |_, _, cx| {
                    cx.stop_propagation();
                })
                .child(
                    h_flex()
                        .px(px(20.))
                        .py(px(16.))
                        .border_b_1()
                        .border_color(tokens::line())
                        .justify_between()
                        .items_center()
                        .child(
                            div()
                                .text_size(px(14.))
                                .font_weight(FontWeight::BOLD)
                                .text_color(tokens::ink())
                                .child(title.into()),
                        )
                        .child(
                            div()
                                .id("modal-close")
                                .p(px(4.))
                                .rounded(px(4.))
                                .text_color(tokens::ink_3())
                                .cursor(CursorStyle::PointingHand)
                                .hover(|s| s.text_color(tokens::ink()))
                                .child(icons::close())
                                .tooltip(move |window, cx| {
                                    Tooltip::new("Close").build(window, cx)
                                })
                                .on_click(move |_, window, cx| close(window, cx)),
                        ),
                )
                .child(body),
        )
}

/// Shared button: accent fill (`.btn-save-snippet` base) — styles.css:772-793
pub fn btn_accent(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    enabled: bool,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .px(px(20.))
        .py(px(7.))
        .rounded(px(6.))
        .bg(tokens::accent())
        .text_color(tokens::white())
        .text_size(px(13.))
        .font_weight(FontWeight::SEMIBOLD)
        .when(enabled, |this| {
            this.cursor(CursorStyle::PointingHand)
                .hover(|s| s.bg(tokens::accent_hover()))
                .active(|s| s.bg(tokens::accent_down()))
                .on_click(move |_, window, cx| on_click(window, cx))
        })
        .when(!enabled, |this| this.opacity(0.4).cursor(CursorStyle::Arrow))
        .child(label.into())
}

/// Shared button: danger outline (`.btn-delete-active`) — styles.css:756-770
pub fn btn_danger_outline(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .px(px(14.))
        .py(px(7.))
        .rounded(px(6.))
        .border_1()
        .border_color(tokens::line())
        .bg(gpui_kit::hsla(0., 0., 0., 0.))
        .text_color(tokens::danger())
        .text_size(px(12.5))
        .font_weight(FontWeight::SEMIBOLD)
        .cursor(CursorStyle::PointingHand)
        .hover(|s| s.bg(tokens::danger_tint()).border_color(tokens::danger()))
        .on_click(move |_, window, cx| on_click(window, cx))
        .child(label.into())
}

/// Quiet surface button used by repair / check-updates style controls.
pub fn btn_surface(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    enabled: bool,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .px(px(12.))
        .py(px(6.))
        .rounded(px(6.))
        .bg(tokens::surface_3())
        .border_1()
        .border_color(tokens::line())
        .text_color(tokens::ink())
        .text_size(px(11.5))
        .font_weight(FontWeight::SEMIBOLD)
        .when(enabled, |this| {
            this.cursor(CursorStyle::PointingHand)
                .hover(|s| {
                    s.bg(tokens::surface_2())
                        .border_color(tokens::accent())
                        .text_color(tokens::white())
                })
                .on_click(move |_, window, cx| on_click(window, cx))
        })
        .when(!enabled, |this| this.opacity(0.6).cursor(CursorStyle::Arrow))
        .child(label.into())
}
