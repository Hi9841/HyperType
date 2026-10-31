//! Parity-only scratch UI for proving slider capture and value_edit padding
//! (S1 F1/F2). Open with `--features parity -- --parity-state widget-lab`.

use std::cell::Cell;
use std::rc::Rc;

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::*;

use crate::ui::tokens;
use crate::ui::widgets::{self, on_value_edit_event};

pub struct WidgetLab {
    /// Shared with slider callbacks so mouse handlers can set values without
    /// re-entering `Entity<WidgetLab>::update` mid-dispatch.
    slider_live: Rc<Cell<f32>>,
    slider_committed: Rc<Cell<f32>>,
    /// Extra notifies after commit so DWM/BitBlt see the new swapchain image.
    repaint_burst: Rc<Cell<u32>>,
    /// Included in label text so GPUI cannot keep a stale shaped run.
    slider_gen: Rc<Cell<u32>>,
    edit: Entity<InputState>,
    edit_committed: f32,
    checking: bool,
    _edit_sub: Subscription,
}

impl WidgetLab {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let edit = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("")
                .default_value("15")
        });
        let _edit_sub = cx.subscribe_in(&edit, window, {
            let edit = edit.clone();
            move |lab, _input, event: &InputEvent, window, cx| {
                // Already inside a WidgetLab update (subscribe_in). Do not
                // entity.read/update the same entity or GPUI panics.
                let current = lab.edit_committed;
                let mut committed = None;
                on_value_edit_event(
                    &edit,
                    event,
                    1.,
                    100.,
                    current,
                    |v, _window, _cx| {
                        committed = Some(v);
                    },
                    window,
                    cx,
                );
                if let Some(v) = committed {
                    lab.edit_committed = v;
                    cx.notify();
                }
            }
        });
        Self {
            slider_live: Rc::new(Cell::new(50.)),
            slider_committed: Rc::new(Cell::new(50.)),
            repaint_burst: Rc::new(Cell::new(0)),
            slider_gen: Rc::new(Cell::new(0)),
            edit,
            edit_committed: 15.,
            checking: false,
            _edit_sub,
        }
    }
}

impl Render for WidgetLab {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let live = self.slider_live.get();
        let committed = self.slider_committed.get();
        let gen_n = self.slider_gen.get();
        // Parity drive sync: BitBlt must wait until this frame is built.
        let _ = std::fs::write(
            std::env::temp_dir().join("ht-lab-render.txt"),
            format!("live={live:.0} committed={committed:.0} gen={gen_n}"),
        );
        let burst = self.repaint_burst.get();
        if burst > 0 {
            self.repaint_burst.set(burst - 1);
            let entity_id = cx.entity().entity_id();
            // Schedule the next burst frame after this one finishes painting.
            window.on_next_frame(move |_, cx| {
                cx.notify(entity_id);
            });
        }
        let edit_committed = self.edit_committed;
        let entity = cx.entity();
        let slider_live = self.slider_live.clone();
        let slider_committed = self.slider_committed.clone();
        let slider_gen = self.slider_gen.clone();
        let repaint_burst = self.repaint_burst.clone();
        let entity_id = entity.entity_id();

        v_flex()
            .id("widget-lab")
            .size_full()
            .bg(tokens::bg())
            .text_color(tokens::ink())
            .font_family(tokens::FONT_UI)
            .text_size(px(tokens::ROOT_SIZE))
            .p(px(24.))
            .gap(px(20.))
            .child(
                div()
                    .text_size(px(14.))
                    .font_weight(FontWeight::BOLD)
                    .child("widget-lab (S1 F1/F2)"),
            )
            .child(
                v_flex()
                    .gap(px(8.))
                    .child(div().child("Slider (1–100, step 1)"))
                    .child(
                        h_flex()
                            .items_center()
                            .gap(px(16.))
                            .child(widgets::slider(
                                "lab-slider",
                                live,
                                1.,
                                100.,
                                1.,
                                {
                                    let slider_live = slider_live.clone();
                                    let slider_gen = slider_gen.clone();
                                    move |v, _window, cx| {
                                        slider_live.set(v);
                                        slider_gen.set(slider_gen.get() + 1);
                                        cx.notify(entity_id);
                                    }
                                },
                                {
                                    let slider_live = slider_live.clone();
                                    let slider_committed = slider_committed.clone();
                                    let slider_gen = slider_gen.clone();
                                    move |v, _window, cx| {
                                        slider_live.set(v);
                                        slider_committed.set(v);
                                        slider_gen.set(slider_gen.get() + 1);
                                        repaint_burst.set(8);
                                        // Parity drive sync: BitBlt can read a stale
                                        // swapchain image before this frame presents.
                                        let _ = std::fs::write(
                                            std::env::temp_dir().join("ht-lab-commit.txt"),
                                            format!("{v:.0}"),
                                        );
                                        cx.notify(entity_id);
                                    }
                                },
                                window,
                                cx,
                            ))
                            .child(
                                div()
                                    .font_family(tokens::FONT_MONO)
                                    .child(format!("live={live:.0} g{gen_n}")),
                            )
                            .child(
                                div()
                                    .font_family(tokens::FONT_MONO)
                                    .child(format!("committed={committed:.0} g{gen_n}")),
                            ),
                    ),
            )
            .child(
                v_flex()
                    .gap(px(8.))
                    .child(div().child("value_edit (1–100)"))
                    .child(
                        h_flex()
                            .items_center()
                            .gap(px(16.))
                            .child(
                                div()
                                    .id("lab-value-edit")
                                    .child(widgets::value_edit(&self.edit, false, window)),
                            )
                            .child(
                                div()
                                    .id("lab-edit-committed")
                                    .font_family(tokens::FONT_MONO)
                                    .child(format!("committed={edit_committed:.0}")),
                            ),
                    ),
            )
            .child(
                h_flex()
                    .gap(px(12.))
                    .child(widgets::btn_create_first("lab-create", "Create Snippet", |_, _| {}))
                    .child(widgets::btn_check_updates(
                        "lab-check",
                        if self.checking {
                            "Checking..."
                        } else {
                            "Check for updates"
                        },
                        true,
                        self.checking,
                        true, // parity freeze: spin frozen at t=0
                        {
                            let entity = entity.clone();
                            move |_, cx| {
                                entity.update(cx, |lab, cx| {
                                    lab.checking = !lab.checking;
                                    cx.notify();
                                });
                            }
                        },
                    )),
            )
    }
}
