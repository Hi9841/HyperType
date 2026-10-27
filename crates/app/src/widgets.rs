//! Small presentational pieces shared by the library and the editor.

use std::rc::Rc;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _, Selectable as _, Sizable as _, Theme, h_flex};
use gpui_kit::*;
use hypertype_core::snippets::TriggerKind;

use crate::chord;

/// One key in mono type on a hairline cap.
pub fn keycap(label: impl Into<SharedString>, theme: &Theme) -> Div {
    div()
        .px_1p5()
        .h(px(20.))
        .flex()
        .items_center()
        .rounded(px(4.))
        .border_1()
        .border_color(theme.border)
        .bg(theme.background)
        .font_family(theme.mono_font_family.clone())
        .text_xs()
        .text_color(theme.foreground)
        .child(label.into())
}

/// A text trigger as one cap, a chord as one cap per key.
pub fn trigger_badge(trigger: &str, kind: TriggerKind, cx: &App) -> Div {
    let theme = cx.theme();
    match kind {
        TriggerKind::Text => h_flex().min_w_0().child(keycap(trigger.to_string(), theme).truncate()),
        TriggerKind::Shortcut => h_flex()
            .gap_0p5()
            .children(chord::keys(trigger).into_iter().map(|key| keycap(key, theme))),
    }
}

/// A single-choice segmented control: ghost buttons in a hairline track,
/// the chosen one filled.
pub fn segmented<T: Copy + PartialEq + 'static>(
    id: &'static str,
    options: &'static [(T, &'static str)],
    current: T,
    on_change: impl Fn(T, &mut Window, &mut App) + 'static,
    cx: &App,
) -> Div {
    let theme = cx.theme();
    let on_change = Rc::new(on_change);
    h_flex()
        .flex_none()
        .p_0p5()
        .gap_0p5()
        .rounded(theme.radius)
        .border_1()
        .border_color(theme.border)
        .bg(theme.muted)
        .children(options.iter().enumerate().map(|(ix, (value, label))| {
            let (value, on_change) = (*value, on_change.clone());
            Button::new((id, ix))
                .ghost()
                .xsmall()
                .label(*label)
                .selected(value == current)
                .on_click(move |_, window, cx| {
                    if value != current {
                        on_change(value, window, cx);
                    }
                })
        }))
}
