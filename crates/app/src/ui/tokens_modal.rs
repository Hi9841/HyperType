//! Token catalog modal stub (S6 fills). Renders nothing yet.

use gpui_kit::*;

use crate::ui::studio::Studio;

pub struct TokensModal {
    studio: WeakEntity<Studio>,
    _observe: Subscription,
}

impl TokensModal {
    pub fn new(studio: WeakEntity<Studio>, cx: &mut Context<Self>) -> Self {
        let _observe = studio
            .upgrade()
            .map(|entity| cx.observe(&entity, |_, _, cx| cx.notify()))
            .expect("Studio must exist when TokensModal is created");
        Self { studio, _observe }
    }

    /// S6 adds parity arms here.
    pub fn parity_state(
        &mut self,
        _name: &str,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> bool {
        false
    }
}

impl Render for TokensModal {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}
