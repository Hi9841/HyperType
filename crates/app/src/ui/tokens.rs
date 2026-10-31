//! Design tokens from `src/styles.css:4-36`. Views use only these names.

// Later slices use the full token set; S1 only paints the shell.
#![allow(dead_code)]

use std::time::Duration;

use gpui_kit::base::{Easing, Transition};
use gpui_kit::*;

/// #000000
pub fn bg() -> Hsla {
    rgb(0x000000).into()
}
/// #0c0c0f
pub fn surface_1() -> Hsla {
    rgb(0x0c0c0f).into()
}
/// #141418
pub fn surface_2() -> Hsla {
    rgb(0x141418).into()
}
/// #1e1e26
pub fn surface_3() -> Hsla {
    rgb(0x1e1e26).into()
}
/// #111115
pub fn field() -> Hsla {
    rgb(0x111115).into()
}
/// #1c1c22
pub fn line() -> Hsla {
    rgb(0x1c1c22).into()
}
/// #282832
pub fn line_2() -> Hsla {
    rgb(0x282832).into()
}
/// #18181f
pub fn sep() -> Hsla {
    rgb(0x18181f).into()
}
/// #202028
pub fn control() -> Hsla {
    rgb(0x202028).into()
}
/// #30303c
pub fn control_line() -> Hsla {
    rgb(0x30303c).into()
}
/// #f4f4f7
pub fn ink() -> Hsla {
    rgb(0xf4f4f7).into()
}
/// #9e9ea8
pub fn ink_2() -> Hsla {
    rgb(0x9e9ea8).into()
}
/// #6b6b78
pub fn ink_3() -> Hsla {
    rgb(0x6b6b78).into()
}
/// #7c2dff
pub fn accent() -> Hsla {
    rgb(0x7c2dff).into()
}
/// #8f47ff
pub fn accent_hover() -> Hsla {
    rgb(0x8f47ff).into()
}
/// #681ee0
pub fn accent_down() -> Hsla {
    rgb(0x681ee0).into()
}
/// #b580ff
pub fn accent_ink() -> Hsla {
    rgb(0xb580ff).into()
}
/// rgba(124,45,255,.35)
pub fn ring() -> Hsla {
    let mut c: Hsla = rgb(0x7c2dff).into();
    c.a = 0.35;
    c
}
/// rgba(124,45,255,.20)
pub fn ring_soft() -> Hsla {
    let mut c: Hsla = rgb(0x7c2dff).into();
    c.a = 0.20;
    c
}
/// #ff453a
pub fn danger() -> Hsla {
    rgb(0xff453a).into()
}
/// rgba(255,69,58,.12)
pub fn danger_tint() -> Hsla {
    let mut c: Hsla = rgb(0xff453a).into();
    c.a = 0.12;
    c
}
/// #30d158
pub fn success() -> Hsla {
    rgb(0x30d158).into()
}
/// #ffffff
pub fn white() -> Hsla {
    rgb(0xffffff).into()
}
/// #a855f7 (studio pill gradient end)
pub fn pill_end() -> Hsla {
    rgb(0xa855f7).into()
}

pub const FONT_UI: &str = "Segoe UI Variable Text";
pub const FONT_UI_FALLBACK: &str = "Segoe UI";
pub const FONT_MONO: &str = "Cascadia Code";
pub const FONT_MONO_FALLBACKS: &[&str] = &["Cascadia Mono", "Consolas"];

pub const ROOT_SIZE: f32 = 13.0;
pub const ROOT_LINE_HEIGHT: f32 = 1.5;

/// styles.css:112-115 — paused brand mark opacity
pub const BRAND_PAUSED_OPACITY: f32 = 0.45;

pub const FAST: Duration = Duration::from_millis(130);
pub const SMOOTH: Duration = Duration::from_millis(180);
pub const MODAL_FADE: Duration = Duration::from_millis(150);
pub const LISTENING: Duration = Duration::from_millis(1200);
pub const SPIN: Duration = Duration::from_secs(1);

/// cubic-bezier(0.2, 0, 0, 1)
pub fn ease_out() -> Easing {
    Easing::cubic_bezier(0.2, 0.0, 0.0, 1.0).expect("EASE_OUT")
}

/// cubic-bezier(0.32, 1.25, 0.32, 1)
pub fn spring() -> Easing {
    Easing::cubic_bezier(0.32, 1.25, 0.32, 1.0).expect("SPRING")
}

pub fn transition_fast() -> Transition {
    Transition::new(FAST).easing(ease_out())
}

pub fn transition_smooth() -> Transition {
    Transition::new(SMOOTH).easing(ease_out())
}

pub fn transition_spring() -> Transition {
    Transition::new(SMOOTH).easing(spring())
}

pub fn transition_modal_fade() -> Transition {
    Transition::new(MODAL_FADE).easing(ease_out())
}

/// Motion policy that finishes at once when parity freezes animations.
pub fn motion(policy: Transition, freeze: bool) -> Transition {
    if freeze {
        Transition::new(Duration::ZERO)
    } else {
        policy
    }
}

/// CSS `box-shadow` blur radius → GPUI Gaussian sigma (RULEBOOK rule 11).
/// Chromium: `sigma = 0.57735 * r + 0.5`.
pub fn css_blur(r: f32) -> Pixels {
    px(0.57735 * r + 0.5)
}

/// Global `:focus-visible` two-shadow ring (D10). `local_bg` fills the gap.
pub fn focus_visible_shadows(local_bg: Hsla) -> Vec<BoxShadow> {
    vec![
        BoxShadow {
            color: local_bg,
            offset: point(px(0.), px(0.)),
            blur_radius: px(0.),
            spread_radius: px(2.),
            inset: false,
        },
        BoxShadow {
            color: accent_ink(),
            offset: point(px(0.), px(0.)),
            blur_radius: px(0.),
            spread_radius: px(4.),
            inset: false,
        },
    ]
}

// One-off colors (CSS line in comment at use site).
/// rgba(124,45,255,.14) — active row bg
pub fn active_row_bg() -> Hsla {
    let mut c: Hsla = rgb(0x7c2dff).into();
    c.a = 0.14;
    c
}
/// rgba(124,45,255,.10) — transfer toast bg
pub fn transfer_toast_bg() -> Hsla {
    let mut c: Hsla = rgb(0x7c2dff).into();
    c.a = 0.10;
    c
}
/// rgba(124,45,255,.20) — transfer toast border
pub fn transfer_toast_border() -> Hsla {
    let mut c: Hsla = rgb(0x7c2dff).into();
    c.a = 0.20;
    c
}
/// rgba(124,45,255,.18) — update pill bg
pub fn update_pill_bg() -> Hsla {
    let mut c: Hsla = rgb(0x7c2dff).into();
    c.a = 0.18;
    c
}
/// rgba(34,197,94,.12) — hook active bg
pub fn hook_active_bg() -> Hsla {
    let mut c: Hsla = rgb(0x22c55e).into();
    c.a = 0.12;
    c
}
/// #4ade80 — hook active ink
pub fn hook_active_ink() -> Hsla {
    rgb(0x4ade80).into()
}
/// rgba(239,68,68,.12) — hook inactive bg
pub fn hook_inactive_bg() -> Hsla {
    let mut c: Hsla = rgb(0xef4444).into();
    c.a = 0.12;
    c
}
/// #f87171 — hook inactive ink
pub fn hook_inactive_ink() -> Hsla {
    rgb(0xf87171).into()
}
/// rgba(0,0,0,.75) — modal backdrop
pub fn modal_backdrop() -> Hsla {
    let mut c: Hsla = rgb(0x000000).into();
    c.a = 0.75;
    c
}
/// rgba(0,0,0,.9) — modal shadow
pub fn modal_shadow() -> Hsla {
    let mut c: Hsla = rgb(0x000000).into();
    c.a = 0.9;
    c
}
/// rgba(48,209,88,.8) — status-dot glow
pub fn status_dot_glow() -> Hsla {
    let mut c: Hsla = rgb(0x30d158).into();
    c.a = 0.8;
    c
}
/// rgb(0 0 0 / .4) — knob shadow
pub fn knob_shadow() -> Hsla {
    let mut c: Hsla = rgb(0x000000).into();
    c.a = 0.4;
    c
}
/// rgb(0 0 0 / .5) — slider thumb shadow
pub fn slider_thumb_shadow() -> Hsla {
    let mut c: Hsla = rgb(0x000000).into();
    c.a = 0.5;
    c
}
