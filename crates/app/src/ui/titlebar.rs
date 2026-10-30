//! Frameless title bar. App.tsx:618-661, styles.css:81-199.

use gpui_kit::component::h_flex;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::assets;
use crate::ui::icons;
use crate::ui::studio::{Modal, Studio};
use crate::ui::tokens;
use crate::ui::update::UpdateInfo;

/// Title bar row. Drag area except interactive buttons.
pub fn render(
    studio: &Studio,
    _window: &mut Window,
    cx: &mut Context<Studio>,
) -> impl IntoElement {
    let enabled = studio.status.enabled;
    let update_info = studio.update.info.clone();

    // .studio-titlebar — styles.css:82-92
    h_flex()
        .id("studio-titlebar")
        .flex_none()
        .w_full()
        .h(px(42.))
        .bg(tokens::surface_1())
        .border_b_1()
        .border_color(tokens::line())
        .items_center()
        .justify_between()
        .px(px(16.))
        .child(brand(enabled, cx))
        // Flexible drag region between brand and window controls.
        .child(
            div()
                .id("titlebar-drag")
                .flex_1()
                .h_full()
                .window_control_area(WindowControlArea::Drag),
        )
        .child(win_controls(update_info, cx))
}

fn brand(enabled: bool, cx: &mut Context<Studio>) -> impl IntoElement {
    // .studio-brand — styles.css:94-98
    h_flex()
        .id("studio-brand")
        .items_center()
        .gap(px(10.))
        .child(brand_icon(enabled, cx))
        .child(
            // .brand-name — styles.css:133-140 (letter-spacing omitted D2)
            h_flex()
                .id("brand-name")
                .items_center()
                .gap(px(6.))
                .text_size(px(13.))
                .font_weight(FontWeight::BOLD)
                .text_color(tokens::ink())
                .window_control_area(WindowControlArea::Drag)
                .child("HyperType")
                .child(studio_pill()),
        )
}

fn brand_icon(enabled: bool, cx: &mut Context<Studio>) -> impl IntoElement {
    let tip = if enabled {
        "Engine Active (click to pause)"
    } else {
        "Engine Paused (click to resume)"
    };
    // .brand-icon-wrap — styles.css:100-106; sized to the 18×18 mark for absolute dot.
    div()
        .id("brand-icon-wrap")
        .relative()
        .flex()
        .items_center()
        .justify_center()
        .w(px(18.))
        .h(px(18.))
        .flex_shrink_0()
        .cursor(CursorStyle::PointingHand)
        .tooltip(move |window, cx| Tooltip::new(tip).build(window, cx))
        .on_click(cx.listener(|this, _, _, cx| {
            this.toggle_enabled(cx);
        }))
        .child(
            // .brand-mark — styles.css:108-115; 18×18 logo
            img(assets::logo())
                .w(px(18.))
                .h(px(18.))
                .object_fit(ObjectFit::Contain)
                .grayscale(!enabled)
                .opacity(if enabled {
                    1.0
                } else {
                    tokens::BRAND_PAUSED_OPACITY
                }),
        )
        .child(status_dot(enabled))
}

fn status_dot(enabled: bool) -> impl IntoElement {
    // .brand-status-dot — styles.css:117-131
    div()
        .absolute()
        .bottom(px(-2.))
        .right(px(-2.))
        .w(px(6.))
        .h(px(6.))
        .rounded_full()
        .border_1()
        .border_color(tokens::surface_1())
        .when(enabled, |this| {
            this.bg(tokens::success()).shadow(vec![BoxShadow {
                color: tokens::status_dot_glow(),
                offset: point(px(0.), px(0.)),
                blur_radius: px(6.),
                spread_radius: px(0.),
                inset: false,
            }])
        })
        .when(!enabled, |this| this.bg(tokens::ink_3()))
}

fn studio_pill() -> impl IntoElement {
    // .studio-pill — styles.css:142-151 (letter-spacing omitted D2)
    // font 10px/700, padding 1px 6px, line-height unitless 1.5 from root, radius 99px.
    div()
        .bg(linear_gradient(
            135.,
            linear_color_stop(tokens::accent(), 0.),
            linear_color_stop(tokens::pill_end(), 1.),
        ))
        .text_color(tokens::white())
        .text_size(px(10.))
        .line_height(relative(1.5))
        .font_weight(FontWeight::BOLD)
        .px(px(6.))
        .py(px(1.))
        .rounded(px(99.))
        .child("STUDIO")
}

fn win_controls(
    update_info: Option<UpdateInfo>,
    cx: &mut Context<Studio>,
) -> impl IntoElement {
    // .studio-win-controls — styles.css:153-157
    h_flex()
        .id("studio-win-controls")
        .items_center()
        .gap(px(4.))
        .children(update_info.map(|info| update_pill(info, cx)))
        .child(settings_btn(cx))
        .child(winbtn_minimize())
        .child(winbtn_close())
}

fn update_pill(info: UpdateInfo, cx: &mut Context<Studio>) -> impl IntoElement {
    let version = info.version.clone();
    let tip = format!("HyperType v{version} available. Click to install.");
    // .update-available-pill — styles.css:1078-1096
    h_flex()
        .id("update-available-pill")
        .items_center()
        .gap(px(5.))
        .bg(tokens::update_pill_bg())
        .border_1()
        .border_color(tokens::accent())
        .text_color(tokens::accent_ink())
        .text_size(px(11.))
        .font_weight(FontWeight::SEMIBOLD)
        .px(px(8.))
        .py(px(3.))
        .rounded(px(6.))
        .cursor(CursorStyle::PointingHand)
        .mr(px(4.))
        .hover(|s| s.bg(tokens::accent()).text_color(tokens::white()))
        .tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
        .on_click(cx.listener(|this, _, _, cx| {
            this.set_modal(Some(Modal::Settings), cx);
        }))
        .child(icons::download())
        .child(format!("Update v{version}"))
}

fn settings_btn(cx: &mut Context<Studio>) -> impl IntoElement {
    // .settings-icon-btn — styles.css:159-177
    div()
        .id("settings-icon-btn")
        .w(px(28.))
        .h(px(28.))
        .border_1()
        .border_color(tokens::line())
        .bg(tokens::surface_2())
        .rounded(px(6.))
        .text_color(tokens::ink_2())
        .flex()
        .items_center()
        .justify_center()
        .cursor(CursorStyle::PointingHand)
        .mr(px(4.))
        .hover(|s| {
            s.bg(tokens::surface_3())
                .text_color(tokens::ink())
                .border_color(tokens::accent())
        })
        .focus_visible(|s| {
            s.shadow(tokens::focus_visible_shadows(tokens::surface_1()))
        })
        .tooltip(move |window, cx| {
            Tooltip::new("Settings & Preferences").build(window, cx)
        })
        .on_click(cx.listener(|this, _, _, cx| {
            this.set_modal(Some(Modal::Settings), cx);
        }))
        .child(icons::settings().text_color(tokens::ink_2()))
}

fn winbtn_minimize() -> impl IntoElement {
    // .winbtn — styles.css:179-195
    div()
        .id("winbtn-minimize")
        .w(px(28.))
        .h(px(28.))
        .rounded(px(6.))
        .text_color(tokens::ink_2())
        .flex()
        .items_center()
        .justify_center()
        .cursor(CursorStyle::PointingHand)
        .hover(|s| s.bg(tokens::surface_2()).text_color(tokens::ink()))
        .on_click(|_, window, _| {
            window.minimize_window();
        })
        .child(icons::minimize().text_color(tokens::ink_2()))
}

fn winbtn_close() -> impl IntoElement {
    // .winbtn.close — styles.css:179-199
    div()
        .id("winbtn-close")
        .w(px(28.))
        .h(px(28.))
        .rounded(px(6.))
        .text_color(tokens::ink_2())
        .flex()
        .items_center()
        .justify_center()
        .cursor(CursorStyle::PointingHand)
        .hover(|s| s.bg(tokens::danger()).text_color(tokens::white()))
        .on_click(|_, window, _| {
            window.remove_window();
        })
        .child(icons::close_window().text_color(tokens::ink_2()))
}

/// S7b may add update-available parity; S1 has no arms yet.
pub fn parity_state(
    _studio: &mut Studio,
    _name: &str,
    _window: &mut Window,
    _cx: &mut Context<Studio>,
) -> bool {
    false
}
