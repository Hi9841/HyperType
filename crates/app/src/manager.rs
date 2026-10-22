//! The manager window: snippet library, editor, and settings.

use gpui_kit::component::{ActiveTheme as _, TitleBar};
use gpui_kit::*;

use crate::shell::Shell;

pub struct Manager {
    _subscriptions: Vec<Subscription>,
}

impl Manager {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            _subscriptions: vec![
                cx.observe_global::<Shell>(|_, cx| cx.notify()),
                crate::theme::follow_system(window),
            ],
        }
    }
}

impl Render for Manager {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let enabled = cx.global::<Shell>().enabled();
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(TitleBar::new().child("HyperType"))
            .child(if enabled {
                "Engine on"
            } else {
                "Engine paused"
            })
    }
}
