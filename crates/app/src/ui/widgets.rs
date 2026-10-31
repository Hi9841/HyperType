//! Shared studio widgets. Each item maps one CSS rule set; the doc comment
//! names the CSS lines.

#![allow(clippy::too_many_arguments)]

use std::cell::Cell;
use std::rc::Rc;

use gpui_kit::base::{transition, Presence};
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
#[allow(dead_code)] // S2/S4 trigger badge
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
#[allow(dead_code)] // S2/S4 chord badge
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
#[allow(dead_code)] // S2/S4 chord badge
pub fn chord_badge(trigger: &str) -> Div {
    h_flex()
        .gap(px(3.))
        .children(chord::keys(trigger).into_iter().map(chord_key))
}

/// Text trigger as one keycap, shortcut as a chord badge.
#[allow(dead_code)] // S2/S4 row badge
pub fn trigger_badge(trigger: &str, kind: TriggerKind) -> Div {
    match kind {
        TriggerKind::Text => h_flex().min_w_0().child(keycap(trigger.to_string()).truncate()),
        TriggerKind::Shortcut => chord_badge(trigger),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // S4/S5 segmented
pub enum SegKind {
    Two,
    Three,
}

/// `.seg` / `.seg-kind` / `.seg-3` / `.seg-combo` — styles.css:601-657
/// Animated thumb uses SMOOTH + SPRING. `freeze` forces t=0 (parity).
///
/// Keys (rule 21): each option is a tab stop; Enter/Space selects (via `on_click`).
#[allow(dead_code)] // S4/S5 segmented
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
    let id: ElementId = id.into();
    let n = match kind {
        SegKind::Two => 2.0_f32,
        SegKind::Three => 3.0,
    };
    // Thumb: (border-box − 2px border) / n − 2px; height 24 (30 − 2 border − 4 pad).
    let thumb_w = (width - px(2.)) / n - px(2.);
    let selected = options
        .iter()
        .position(|(v, _)| *v == current)
        .unwrap_or(0);
    let target_x = thumb_w * selected as f32;
    // Linear time + tokens::spring(t) via transition_spring (rule 18).
    let policy = tokens::motion(tokens::transition_spring(), freeze);
    // Stable channel per control (RULEBOOK rule 12).
    let x = transition((id.clone(), "thumb"), target_x, policy, window, cx);
    let on_change = Rc::new(on_change);

    div()
        .id(id.clone())
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
                .h(px(24.))
                .rounded(px(5.))
                .bg(tokens::surface_3()),
        )
        .children(options.iter().enumerate().map(|(ix, (value, label))| {
            let active = *value == current;
            let value = *value;
            let on_change = on_change.clone();
            // Option ids include the parent element id (finding 18).
            let opt_id = (id.clone(), format!("opt-{ix}"));
            // Explicit handle so Enter/Space dispatch after Tab/click focus.
            let focus = window
                .use_keyed_state((id.clone(), format!("opt-{ix}-focus")), cx, |_, cx| {
                    cx.focus_handle()
                })
                .read(cx)
                .clone()
                .tab_index(0)
                .tab_stop(true);
            let focus_for_mouse = focus.clone();
            div()
                .id(opt_id)
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
                .track_focus(&focus)
                .tab_index(0)
                .tab_stop(true)
                .occlude()
                .focus_visible(|s| {
                    s.shadow(tokens::focus_visible_shadows(tokens::surface_2()))
                })
                .child((*label).to_string())
                .on_mouse_down(MouseButton::Left, {
                    let on_change = on_change.clone();
                    move |_, window, cx| {
                        focus_for_mouse.focus(window, cx);
                        on_change(value, window, cx);
                        cx.stop_propagation();
                    }
                })
                // Enter/Space: GPUI fires `on_click` on KeyUp for focused elements
                // (pending_keyboard_down). Do not also handle keys here.
                .on_click(move |_, window, cx| {
                    on_change(value, window, cx);
                })
        }))
}

/// `.switch` / `.knob` — styles.css:884-912.
/// Track bg/border: SMOOTH + EASE_OUT. Knob: SMOOTH + SPRING.
///
/// Keys (rule 21): tab stop; Enter/Space toggles (via `on_click`).
#[allow(dead_code)] // S5 settings switch
pub fn switch(
    id: impl Into<ElementId>,
    checked: bool,
    freeze: bool,
    on_toggle: impl Fn(bool, &mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let id: ElementId = id.into();
    let target = if checked { px(16.) } else { px(0.) };
    // Linear time + tokens::spring(t) via transition_spring (rule 18).
    let knob_policy = tokens::motion(tokens::transition_spring(), freeze);
    let track_policy = tokens::motion(tokens::transition_smooth(), freeze);
    let x = transition((id.clone(), "knob"), target, knob_policy, window, cx);
    let track_bg = transition(
        (id.clone(), "track-bg"),
        if checked {
            tokens::accent()
        } else {
            tokens::control()
        },
        track_policy.clone(),
        window,
        cx,
    );
    let track_border = transition(
        (id.clone(), "track-border"),
        if checked {
            tokens::accent()
        } else {
            tokens::control_line()
        },
        track_policy,
        window,
        cx,
    );
    let ring_bg = if checked {
        tokens::accent()
    } else {
        tokens::control()
    };
    let on_toggle = Rc::new(on_toggle);
    // Explicit handle so Enter/Space dispatch on this node after click-focus.
    let focus = window
        .use_keyed_state((id.clone(), "focus"), cx, |_, cx| cx.focus_handle())
        .read(cx)
        .clone()
        .tab_index(0)
        .tab_stop(true);
    let focus_for_mouse = focus.clone();

    div()
        .id(id)
        .relative()
        .flex_none()
        .w(px(38.))
        .h(px(22.))
        .rounded_full()
        .border_1()
        .border_color(track_border)
        .bg(track_bg)
        .cursor(CursorStyle::PointingHand)
        .track_focus(&focus)
        .tab_index(0)
        .tab_stop(true)
        .focus_visible(|s| s.shadow(tokens::focus_visible_shadows(ring_bg)))
        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
            focus_for_mouse.focus(window, cx);
        })
        // Enter/Space: GPUI fires `on_click` on KeyUp for focused elements
        // (pending_keyboard_down). Do not also handle keys here.
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
                    blur_radius: tokens::css_blur(2.),
                    spread_radius: px(0.),
                    inset: false,
                }]),
        )
}

#[derive(Clone, Copy, Default)]
#[allow(dead_code)] // slider drag state
struct SliderDrag {
    active: bool,
    origin_x: Pixels,
    width: Pixels,
    last: f32,
}

#[allow(dead_code)] // slider step snap
fn snap_slider(value: f32, min: f32, max: f32, step: f32) -> f32 {
    let clamped = value.clamp(min, max);
    if step <= 0. {
        return clamped;
    }
    let n = ((clamped - min) / step).round();
    (min + n * step).clamp(min, max)
}

/// `.slider` — styles.css:914-932. 130×4 track; 14px thumb paints over, centered.
/// Click jumps to pointer (track left origin); drag captures the pointer so
/// moves/release outside the 4px box keep updating (rules 14 + F1).
///
/// Drag flags live in an `Rc<Cell<_>>` keyed by slider id. Updating an
/// `Entity<SliderDrag>` would notify the parent view mid-gesture and replace
/// mouse listeners before mouse-up, dropping the click `change`/`commit`.
///
/// Keys (rule 21): tab stop; Arrow Left/Down −step, Right/Up +step, Home/End,
/// PageUp/PageDown ±10 steps (Chromium `<input type=range>`); each key lives + commits.
#[allow(dead_code)] // S5 settings + parity widget-lab
pub fn slider(
    id: impl Into<ElementId>,
    value: f32,
    min: f32,
    max: f32,
    step: f32,
    on_live: impl Fn(f32, &mut Window, &mut App) + 'static,
    on_commit: impl Fn(f32, &mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let id: ElementId = id.into();
    let range = (max - min).max(f32::EPSILON);
    let t = ((value - min) / range).clamp(0., 1.);
    let track_w = px(130.);
    let thumb = px(14.);
    let track_h = px(4.);
    let x = (track_w - thumb) * t;
    let step = if step > 0. { step } else { 1. };
    let page = step * 10.;

    // Keyed Rc<Cell>: same cell across frames; never entity.update mid-gesture.
    let drag = window
        .use_keyed_state((id.clone(), "drag"), cx, |_, _| {
            Rc::new(Cell::new(SliderDrag::default()))
        })
        .read(cx)
        .clone();
    // Explicit focus handle so key dispatch hits this node. TabStopMap reads
    // `FocusHandle.tab_stop` (not the element flag) — set both on the handle
    // (carousel pattern) and on the element.
    let focus = window
        .use_keyed_state((id.clone(), "focus"), cx, |_, cx| cx.focus_handle())
        .read(cx)
        .clone()
        .tab_index(0)
        .tab_stop(true);
    let on_live = Rc::new(on_live);
    let on_commit = Rc::new(on_commit);

    let sample = move |pos_x: Pixels, origin_x: Pixels, width: Pixels| -> f32 {
        let width = width.max(px(1.));
        let t = ((pos_x - origin_x) / width).clamp(0., 1.);
        snap_slider(min + t * (max - min), min, max, step)
    };

    let on_live_key = on_live.clone();
    let on_commit_key = on_commit.clone();
    let focus_for_mouse = focus.clone();

    div()
        .id(id)
        .relative()
        .w(track_w)
        .h(track_h)
        .cursor(CursorStyle::PointingHand)
        .track_focus(&focus)
        .tab_index(0)
        .tab_stop(true)
        .focus_visible(|s| s.shadow(tokens::focus_visible_shadows(tokens::bg())))
        .on_key_down(move |ev, window, cx| {
            if ev.keystroke.modifiers.modified() {
                return;
            }
            let next = match ev.keystroke.key.as_str() {
                "left" | "down" => snap_slider(value - step, min, max, step),
                "right" | "up" => snap_slider(value + step, min, max, step),
                "home" => min,
                "end" => max,
                "pageup" => snap_slider(value + page, min, max, step),
                "pagedown" => snap_slider(value - page, min, max, step),
                _ => return,
            };
            if (next - value).abs() > f32::EPSILON {
                on_live_key(next, window, cx);
                on_commit_key(next, window, cx);
            }
            cx.stop_propagation();
        })
        .child(
            div()
                .w_full()
                .h_full()
                .rounded_full()
                .bg(tokens::surface_3()),
        )
        .child(
            div()
                .absolute()
                // Center 14px thumb on 4px track: (14-4)/2 = 5 above.
                .top(px(-5.))
                .left(x)
                .w(thumb)
                .h(thumb)
                .rounded_full()
                .bg(tokens::white())
                .shadow(vec![BoxShadow {
                    color: tokens::slider_thumb_shadow(),
                    offset: point(px(0.), px(1.)),
                    blur_radius: tokens::css_blur(3.),
                    spread_radius: px(0.),
                    inset: false,
                }]),
        )
        // Hit area covers the 14px thumb (shifted top -5) while layout stays 130×4.
        .child(
            canvas(
                |bounds, window, _| {
                    let hit_bounds = Bounds {
                        origin: point(bounds.origin.x, bounds.origin.y - px(5.)),
                        size: size(bounds.size.width, px(14.)),
                    };
                    (bounds, window.insert_hitbox(hit_bounds, HitboxBehavior::Normal))
                },
                move |_bounds, (track_bounds, hitbox), window, _cx| {
                    let on_live = on_live.clone();
                    let on_commit = on_commit.clone();
                    let drag = drag.clone();
                    let hitbox = hitbox.clone();
                    let focus_for_mouse = focus_for_mouse.clone();

                    window.on_mouse_event({
                        let on_live = on_live.clone();
                        let drag = drag.clone();
                        let hitbox = hitbox.clone();
                        let focus_for_mouse = focus_for_mouse.clone();
                        move |ev: &MouseDownEvent, phase, window, cx| {
                            if phase != DispatchPhase::Bubble
                                || ev.button != MouseButton::Left
                                || !hitbox.is_hovered(window)
                            {
                                return;
                            }
                            // Canvas hitbox sits above the focusable div; focus explicitly
                            // so arrow keys dispatch along this control's path.
                            focus_for_mouse.focus(window, cx);
                            window.capture_pointer(hitbox.id);
                            let origin_x = track_bounds.origin.x;
                            let width = track_bounds.size.width;
                            let v = sample(ev.position.x, origin_x, width);
                            drag.set(SliderDrag {
                                active: true,
                                origin_x,
                                width,
                                last: v,
                            });
                            on_live(v, window, cx);
                            window.prevent_default();
                            cx.stop_propagation();
                        }
                    });

                    window.on_mouse_event({
                        let on_live = on_live.clone();
                        let drag = drag.clone();
                        move |ev: &MouseMoveEvent, phase, window, cx| {
                            if phase != DispatchPhase::Bubble {
                                return;
                            }
                            let d = drag.get();
                            // While dragging, sample every move. capture_pointer stores a
                            // HitboxId from the press frame; ids are not reused, so
                            // hitbox.is_hovered is false on later frames (F1 / rule 14).
                            if !d.active {
                                return;
                            }
                            let v = sample(ev.position.x, d.origin_x, d.width);
                            drag.set(SliderDrag {
                                last: v,
                                ..d
                            });
                            on_live(v, window, cx);
                        }
                    });

                    window.on_mouse_event({
                        let on_commit = on_commit.clone();
                        let drag = drag.clone();
                        move |ev: &MouseUpEvent, phase, window, cx| {
                            if phase != DispatchPhase::Bubble || ev.button != MouseButton::Left {
                                return;
                            }
                            let d = drag.get();
                            if !d.active {
                                return;
                            }
                            let v = sample(ev.position.x, d.origin_x, d.width);
                            drag.set(SliderDrag {
                                active: false,
                                last: v,
                                ..d
                            });
                            // capture_pointer auto-releases on mouse up.
                            on_commit(v, window, cx);
                        }
                    });
                },
            )
            .absolute()
            .inset_0(),
        )
}

/// Wire a value-edit input: select-all on focus, Enter blurs, blur commits
/// clamped value, empty/NaN reverts. App.tsx:358-373.
///
/// Call from the owning entity's `new` with `cx.subscribe_in`.
#[allow(dead_code)] // S5 settings + parity widget-lab
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

/// Advance of mono `"0"` at 12px (Cascadia Code / fallbacks).
#[allow(dead_code)] // value_edit width
fn mono_zero_advance(window: &Window) -> Pixels {
    let text_system = window.text_system();
    let font = Font {
        family: tokens::FONT_MONO.into(),
        features: tokens::font_features_ui(),
        fallbacks: Some(FontFallbacks::from_fonts(
            tokens::FONT_MONO_FALLBACKS
                .iter()
                .map(|f| (*f).to_string())
                .collect(),
        )),
        weight: FontWeight::SEMIBOLD,
        style: FontStyle::Normal,
    };
    let shaped = text_system.shape_line(
        "0".into(),
        px(12.),
        &[TextRun {
            len: 1,
            font,
            color: tokens::ink(),
            background_color: None,
            underline: None,
            strikethrough: None,
        }],
        None,
    );
    shaped.width
}

/// `.value-edit` — styles.css:934-945. Kit Input with appearance(false).
/// Rule 17: set px/py/height explicitly so Kit Medium (10/8, h_8) does not remain.
///
/// Keys (rule 21): Kit `Input` is already a tab stop (`tab_index(0)`); type digits,
/// Enter blurs, blur commits (see `on_value_edit_event`).
#[allow(dead_code)] // S5 settings + parity widget-lab
pub fn value_edit(
    input: &Entity<InputState>,
    wide: bool,
    window: &Window,
) -> impl IntoElement {
    let ch = mono_zero_advance(window);
    let width = if wide { ch * 4.5 } else { ch * 4.0 };
    // font 12px / line-height 1.4 → content height 16.8; CSS padding: 0.
    let height = px(16.8);
    div()
        .w(width)
        .child(
            Input::new(input)
                .appearance(false)
                .tab_index(0)
                .text_size(px(12.))
                .font_family(tokens::FONT_MONO)
                .font_weight(FontWeight::SEMIBOLD)
                .line_height(relative(1.4))
                .text_align(TextAlign::Right)
                .text_color(tokens::ink())
                .border_b_1()
                .border_dashed()
                .border_color(tokens::line_2())
                .px(px(0.))
                .py(px(0.))
                .map(|mut this| {
                    // Input::h sets multi-line height; refine the single-line box (rule 17).
                    this.style().size.height = Some(height.into());
                    this
                }),
        )
}

/// `.studio-field` — styles.css:554-568. Focus ring = 3px RING_SOFT.
#[allow(dead_code)] // S4 editor fields
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
#[allow(dead_code)] // S5 hook status
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
/// Settings size: 500px / 85vh. Fade in 150 ms from 0 to 1 on open (RULEBOOK
/// rule 12). Backdrop click closes. Escape does NOT close (A4 / rule 21).
///
/// Rule 21a: no focus trap - Tab continues through page stops behind the
/// backdrop. Close control is `tab_index(0)` (tree order).
#[allow(dead_code)] // S5/S6 modals
pub fn modal_frame(
    id: impl Into<ElementId>,
    title: impl Into<SharedString>,
    freeze: bool,
    on_close: impl Fn(&mut Window, &mut App) + 'static,
    body: impl IntoElement,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    modal_frame_sized(id, title, px(500.), 0.85, freeze, on_close, body, window, cx)
}

/// Sized modal frame. Catalog uses 680px / 0.88 (styles.css:995-998).
/// `overflow_hidden` on the card. No Escape-to-close (A4).
#[allow(dead_code)] // S5/S6 modals
pub fn modal_frame_sized(
    id: impl Into<ElementId>,
    title: impl Into<SharedString>,
    width: Pixels,
    max_height_frac: f32,
    freeze: bool,
    on_close: impl Fn(&mut Window, &mut App) + 'static,
    body: impl IntoElement,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let id: ElementId = id.into();
    let on_close = Rc::new(on_close);
    let policy = tokens::motion(tokens::transition_modal_fade(), freeze);
    // Presence starts from 0 on mount, then animates to 1.
    let opacity = Presence::new((id.clone(), "fade"), true)
        .transition(policy)
        .sample(window, cx)
        .progress;

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
        .child(
            v_flex()
                .id("modal-card")
                .tab_group()
                .tab_stop(false)
                .w(width)
                .max_h(relative(max_height_frac))
                .bg(tokens::surface_1())
                .border_1()
                .border_color(tokens::line_2())
                .rounded(px(12.))
                .shadow(vec![BoxShadow {
                    color: tokens::modal_shadow(),
                    offset: point(px(0.), px(20.)),
                    blur_radius: tokens::css_blur(60.),
                    spread_radius: px(0.),
                    inset: false,
                }])
                .overflow_hidden()
                .on_mouse_down(MouseButton::Left, |_, _, cx| {
                    cx.stop_propagation();
                })
                .child(
                    h_flex()
                        .flex_none()
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
                            // Close is first tab stop in the modal tab_group (rule 21).
                            div()
                                .id("modal-close")
                                .tab_index(0)
                                .p(px(4.))
                                .rounded(px(4.))
                                .text_color(tokens::ink_3())
                                .cursor(CursorStyle::PointingHand)
                                .hover(|s| s.text_color(tokens::ink()))
                                .focus_visible(|s| {
                                    s.shadow(tokens::focus_visible_shadows(tokens::surface_1()))
                                })
                                .child(icons::close())
                                .tooltip(move |window, cx| {
                                    Tooltip::new("Close").build(window, cx)
                                })
                                // Enter/Space: GPUI fires on_click on KeyUp for the
                                // focused element; no separate key handler.
                                .on_click({
                                    let close = close.clone();
                                    move |_, window, cx| close(window, cx)
                                }),
                        ),
                )
                .child(body),
        )
}

/// Shared button: accent fill (`.btn-save-snippet` base) — styles.css:772-793
///
/// Keys (rule 21): tab stop when enabled; Enter/Space activates (via `on_click`).
#[allow(dead_code)] // S4 save button
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
                .tab_index(0)
                .focus_visible(|s| s.shadow(tokens::focus_visible_shadows(tokens::accent())))
                .hover(|s| s.bg(tokens::accent_hover()))
                .active(|s| s.bg(tokens::accent_down()))
                .on_click(move |_, window, cx| on_click(window, cx))
        })
        .when(!enabled, |this| this.opacity(0.4).cursor(CursorStyle::Arrow))
        .child(label.into())
}

/// Shared button: danger outline (`.btn-delete-active`) — styles.css:756-770
///
/// Keys (rule 21): tab stop; Enter/Space activates (via `on_click`).
#[allow(dead_code)] // S4 delete button
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
        .tab_index(0)
        .focus_visible(|s| s.shadow(tokens::focus_visible_shadows(tokens::bg())))
        .hover(|s| s.bg(tokens::danger_tint()).border_color(tokens::danger()))
        .on_click(move |_, window, cx| on_click(window, cx))
        .child(label.into())
}

/// `.btn-repair-hook` — styles.css:1193-1217. Quiet surface control.
///
/// Keys (rule 21): tab stop when enabled; Enter/Space activates (via `on_click`).
#[allow(dead_code)] // S5 repair hook
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
        .whitespace_nowrap()
        .when(enabled, |this| {
            this.cursor(CursorStyle::PointingHand)
                .tab_index(0)
                .focus_visible(|s| {
                    s.shadow(tokens::focus_visible_shadows(tokens::surface_3()))
                })
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

/// `.btn-check-updates` — styles.css:1104-1126. IconRefresh before label (gap 5).
/// Spins while checking (SPIN 1s linear infinite); frozen at t=0 in parity mode.
///
/// Keys (rule 21): tab stop when enabled; Enter/Space activates (via `on_click`).
#[allow(dead_code)] // S5 settings + parity widget-lab
pub fn btn_check_updates(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    enabled: bool,
    spinning: bool,
    freeze: bool,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let icon = if spinning && !freeze {
        icons::refresh()
            .with_animation(
                "btn-check-updates-spin",
                Animation::new(tokens::SPIN).repeat(),
                |icon, delta| icon.rotate(percentage(delta)),
            )
            .into_any_element()
    } else {
        // freeze or idle: IconRefresh at t=0 (no rotation).
        icons::refresh().into_any_element()
    };

    h_flex()
        .id(id)
        .items_center()
        .gap(px(5.))
        .bg(tokens::surface_3())
        .border_1()
        .border_color(tokens::line())
        .text_color(tokens::ink_2())
        .text_size(px(11.))
        .font_weight(FontWeight::SEMIBOLD)
        .px(px(8.))
        .py(px(3.))
        .rounded(px(5.))
        .when(enabled, |this| {
            this.cursor(CursorStyle::PointingHand)
                .tab_index(0)
                .focus_visible(|s| {
                    s.shadow(tokens::focus_visible_shadows(tokens::surface_3()))
                })
                .hover(|s| {
                    s.bg(tokens::field())
                        .text_color(tokens::ink())
                        .border_color(tokens::line_2())
                })
                .on_click(move |_, window, cx| on_click(window, cx))
        })
        .when(!enabled, |this| this.opacity(0.6).cursor(CursorStyle::Arrow))
        .child(icon)
        .child(label.into())
}

/// `.btn-create-first` — styles.css:490-502. IconPlus before label (gap 6).
///
/// Keys (rule 21): tab stop; Enter/Space activates (via `on_click`).
#[allow(dead_code)] // S2 empty library + parity widget-lab
pub fn btn_create_first(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    h_flex()
        .id(id)
        .items_center()
        .gap(px(6.))
        .bg(tokens::surface_3())
        .border_1()
        .border_color(tokens::line())
        .text_color(tokens::ink())
        .text_size(px(12.))
        .font_weight(FontWeight::SEMIBOLD)
        .px(px(14.))
        .py(px(6.))
        .rounded(px(6.))
        .cursor(CursorStyle::PointingHand)
        .tab_index(0)
        .focus_visible(|s| s.shadow(tokens::focus_visible_shadows(tokens::surface_3())))
        .on_click(move |_, window, cx| on_click(window, cx))
        .child(icons::plus())
        .child(label.into())
}
