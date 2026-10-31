//! Settings modal stub (S5 fills). Renders nothing yet.

use gpui_kit::*;

use crate::ui::studio::Studio;

pub struct SettingsModal {
    #[allow(dead_code)] // S5 reads Studio through this handle
    studio: WeakEntity<Studio>,
    _observe: Subscription,
}

impl SettingsModal {
    pub fn new(
        studio: WeakEntity<Studio>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let _observe = studio
            .upgrade()
            .map(|entity| cx.observe(&entity, |_, _, cx| cx.notify()))
            .expect("Studio must exist when SettingsModal is created");
        Self { studio, _observe }
    }

    /// S5 adds parity arms here.
    #[allow(dead_code)] // S5 parity arms
    pub fn parity_state(
        &mut self,
        _name: &str,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> bool {
        false
    }
}

impl Render for SettingsModal {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}
