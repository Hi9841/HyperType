//! Library pane stub (S2 fills). Renders the 340px SURFACE_1 pane + right border.

use gpui_kit::*;

use crate::ui::studio::Studio;
use crate::ui::tokens;

pub struct Sidebar {
    studio: WeakEntity<Studio>,
    _observe: Subscription,
}

impl Sidebar {
    pub fn new(studio: WeakEntity<Studio>, cx: &mut Context<Self>) -> Self {
        let _observe = studio
            .upgrade()
            .map(|entity| cx.observe(&entity, |_, _, cx| cx.notify()))
            .expect("Studio must exist when Sidebar is created");
        Self { studio, _observe }
    }

    /// S2 adds parity arms here. Signature matches `ui/parity.rs` dispatch.
    pub fn parity_state(
        &mut self,
        _name: &str,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> bool {
        false
    }
}

impl Render for Sidebar {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        // .studio-sidebar — styles.css:213-219
        div()
            .w(px(340.))
            .h_full()
            .flex_none()
            .bg(tokens::surface_1())
            .border_r_1()
            .border_color(tokens::line())
    }
}
