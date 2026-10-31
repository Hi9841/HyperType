//! Library pane (S2) + drag/keyboard reorder (S3). Spec: App.tsx 122-127,
//! 208-222, 501-614, 665-800; styles.css 212-502.

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use gpui_kit::base::transition;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::scroll::{Scrollbar, ScrollbarMode};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use hypertype_core::commands::{self, SnippetView};

use crate::shell::Shell;
use crate::ui::icons;
use crate::ui::studio::Studio;
use crate::ui::tokens;
use crate::ui::widgets::{self, trigger_badge};

/// `.studio-item` is 59.5px (padding 8+8, border 1+1, content 41.5) with a
/// 4px gap, so the reference pitch is 63.5px. Chromium rounds the fractional
/// edges half up (row tops 178, 241, 305, 368, 432); GPUI snaps toward zero
/// and would drift 0.5px per row. Rows alternate 59/60px to land on the
/// reference pixel grid.
fn row_height(index: usize) -> f32 {
    if index.is_multiple_of(2) { 59. } else { 60. }
}

/// Drag pitch: the spec reads `offsetHeight` (59.5 rounded to 60).
const DRAG_ROW_HEIGHT: f32 = 60.;
const TRANSFER_CLEAR: Duration = Duration::from_secs(3);
const SEARCH_PLACEHOLDER: &str = "Search triggers or content...";

#[derive(Clone, Copy)]
struct DragState {
    from: usize,
    start_y: f32,
    drag_y: f32,
    drop_idx: usize,
    row_height: f32,
}

pub struct Sidebar {
    studio: WeakEntity<Studio>,
    /// Mirrored search text (App.tsx `search` signal).
    search: String,
    search_input: Entity<InputState>,
    transfer_busy: bool,
    transfer_message: String,
    transfer_clear_task: Option<Task<()>>,
    /// Live drag (Rc so window mouse handlers can update without re-entry).
    drag: Rc<Cell<Option<DragState>>>,
    /// Pointer-down on grip before the first move (click focuses without drag).
    pending_drag: Rc<Cell<Option<(usize, f32, f32)>>>,
    scroll: ScrollHandle,
    /// Parity-only forced hover row index (row-hover state).
    parity_hover_row: Option<usize>,
    _observe: Subscription,
    _subs: Vec<Subscription>,
}

impl Sidebar {
    pub fn new(studio: WeakEntity<Studio>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let _observe = studio
            .upgrade()
            .map(|entity| cx.observe(&entity, |_, _, cx| cx.notify()))
            .expect("Studio must exist when Sidebar is created");
        let search_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(SEARCH_PLACEHOLDER));
        let search_sub = cx.subscribe_in(&search_input, window, {
            move |this, input, event: &InputEvent, _window, cx| {
                if matches!(
                    event,
                    InputEvent::Change | InputEvent::Focus | InputEvent::Blur
                ) {
                    this.search = input.read(cx).value().to_string();
                    cx.notify();
                }
            }
        });
        let activation_sub = cx.observe_window_activation(window, |this, window, cx| {
            if !window.is_window_active() && this.drag.get().is_some() {
                // Blur / deactivate: cancel without commit (App.tsx onCancel).
                this.drag.set(None);
                cx.notify();
            }
        });
        Self {
            studio,
            search: String::new(),
            search_input,
            transfer_busy: false,
            transfer_message: String::new(),
            transfer_clear_task: None,
            drag: Rc::new(Cell::new(None)),
            pending_drag: Rc::new(Cell::new(None)),
            scroll: ScrollHandle::new(),
            parity_hover_row: None,
            _observe,
            _subs: vec![search_sub, activation_sub],
        }
    }

    fn reorder_locked(&self) -> bool {
        !self.search.trim().is_empty()
    }

    fn filtered<'a>(&self, snippets: &'a [SnippetView]) -> Vec<&'a SnippetView> {
        let q = self.search.trim().to_lowercase();
        if q.is_empty() {
            return snippets.iter().collect();
        }
        snippets
            .iter()
            .filter(|s| {
                s.trigger.to_lowercase().contains(&q) || s.expansion.to_lowercase().contains(&q)
            })
            .collect()
    }

    fn set_search_text(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.search = text.to_string();
        self.search_input.update(cx, |state, cx| {
            state.set_value(text.to_string(), window, cx);
        });
        cx.notify();
    }

    /// Match reference captures: typed search leaves the field focused (accent
    /// border + ring). Empty search stays unfocused.
    fn set_search_text_parity(
        &mut self,
        text: &str,
        focus: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_search_text(text, window, cx);
        let focus_handle = self.search_input.focus_handle(cx);
        if focus {
            focus_handle.focus(window, cx);
        } else if focus_handle.is_focused(window) {
            window.blur(cx);
        }
        cx.notify();
    }

    fn clear_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.set_search_text("", window, cx);
    }

    fn arm_drag(&self, index: usize, start_y: f32, row_height: f32, cx: &mut Context<Self>) {
        if self.reorder_locked() || self.drag.get().is_some() {
            return;
        }
        self.pending_drag.set(Some((index, start_y, row_height)));
        cx.notify();
    }

    fn start_drag(&self, index: usize, start_y: f32, row_height: f32, cx: &mut Context<Self>) {
        if self.reorder_locked() || self.drag.get().is_some() {
            return;
        }
        self.pending_drag.set(None);
        self.drag.set(Some(DragState {
            from: index,
            start_y,
            drag_y: 0.,
            drop_idx: index,
            row_height,
        }));
        cx.notify();
    }

    fn on_drag_move(&self, client_y: f32, count: usize, cx: &mut Context<Self>) -> bool {
        if self.drag.get().is_none() {
            if let Some((index, start_y, row_height)) = self.pending_drag.get() {
                // Promote pending grip press to a live drag on first move.
                if (client_y - start_y).abs() >= 1. {
                    self.start_drag(index, start_y, row_height, cx);
                } else {
                    return false;
                }
            } else {
                return false;
            }
        }
        let mut state = match self.drag.get() {
            Some(s) => s,
            None => return false,
        };
        let dy = client_y - state.start_y;
        let rh = state.row_height.max(1.);
        let drop = ((state.from as f32) + (dy / rh).round())
            .clamp(0., (count.saturating_sub(1)) as f32) as usize;
        state.drag_y = dy;
        state.drop_idx = drop;
        self.drag.set(Some(state));
        cx.notify();
        true
    }

    fn clear_pending_drag(&self, cx: &mut Context<Self>) {
        if self.pending_drag.get().is_some() {
            self.pending_drag.set(None);
            cx.notify();
        }
    }

    fn row_shift(&self, index: usize, freeze: bool, window: &mut Window, cx: &mut App) -> Pixels {
        let Some(state) = self.drag.get() else {
            return px(0.);
        };
        let target = if index == state.from {
            state.drag_y
        } else if state.from < state.drop_idx && index > state.from && index <= state.drop_idx {
            -state.row_height
        } else if state.from > state.drop_idx && index >= state.drop_idx && index < state.from {
            state.row_height
        } else {
            0.
        };
        // Dragged row tracks the pointer with no ease; others slide FAST ease-out.
        if index == state.from {
            return px(target);
        }
        let policy = tokens::motion(tokens::transition_fast(), freeze);
        transition(
            (("row-shift", index), "top"),
            px(target),
            policy,
            window,
            cx,
        )
    }

    fn set_transfer_message(&mut self, message: String, clear_after: bool, cx: &mut Context<Self>) {
        self.transfer_message = message;
        self.transfer_clear_task = None;
        if clear_after {
            let expected = self.transfer_message.clone();
            self.transfer_clear_task = Some(cx.spawn(async move |this, cx| {
                cx.background_executor().timer(TRANSFER_CLEAR).await;
                this.update(cx, |this, cx| {
                    if this.transfer_message == expected {
                        this.transfer_message.clear();
                        this.transfer_clear_task = None;
                        cx.notify();
                    }
                })
                .ok();
            }));
        }
        cx.notify();
    }

    fn export_snippets(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.transfer_busy {
            return;
        }
        self.transfer_busy = true;
        self.transfer_message.clear();
        self.transfer_clear_task = None;
        cx.notify();

        let (tx, rx) = async_channel::bounded::<Option<PathBuf>>(1);
        std::thread::spawn(move || {
            let path = rfd::FileDialog::new()
                .set_file_name("HyperType-snippets.json")
                .add_filter("HyperType snippets", &["json"])
                .save_file();
            let _ = tx.send_blocking(path);
        });

        let studio = self.studio.clone();
        cx.spawn_in(window, async move |this, cx| {
            let path = rx.recv().await.ok().flatten();
            this.update_in(cx, |this, window, cx| {
                this.transfer_busy = false;
                match path {
                    None => {
                        cx.notify();
                    }
                    Some(path) => {
                        let result = match studio.upgrade() {
                            Some(entity) => Some(entity.update(cx, |studio, _cx| {
                                commands::export_snippets(&studio.state, &path)
                            })),
                            None => None,
                        };
                        match result {
                            Some(Ok(())) => {
                                this.set_transfer_message("Exported to JSON".into(), true, cx);
                            }
                            Some(Err(err)) => {
                                this.set_transfer_message(err, false, cx);
                            }
                            None => {
                                this.set_transfer_message(
                                    "Studio unavailable".into(),
                                    false,
                                    cx,
                                );
                            }
                        }
                        let _ = window;
                    }
                }
            })
            .ok();
        })
        .detach();
    }

    fn import_snippets(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.transfer_busy {
            return;
        }
        self.transfer_busy = true;
        self.transfer_message.clear();
        self.transfer_clear_task = None;
        cx.notify();

        let (tx, rx) = async_channel::bounded::<Option<PathBuf>>(1);
        std::thread::spawn(move || {
            let path = rfd::FileDialog::new()
                .add_filter("HyperType snippets", &["json"])
                .pick_file();
            let _ = tx.send_blocking(path);
        });

        let studio = self.studio.clone();
        cx.spawn_in(window, async move |this, cx| {
            let path = rx.recv().await.ok().flatten();
            this.update_in(cx, |this, window, cx| {
                this.transfer_busy = false;
                let Some(path) = path else {
                    cx.notify();
                    return;
                };
                let Some(entity) = studio.upgrade() else {
                    this.set_transfer_message("Studio unavailable".into(), false, cx);
                    return;
                };
                let result = entity.update(cx, |_studio, cx| {
                    let shell = cx.global::<Shell>();
                    commands::import_snippets(&shell.state, &shell.hotkeys, &path)
                });
                match result {
                    Ok(summary) => {
                        let mut parts = vec![format!("Imported {}", summary.imported)];
                        if summary.overwritten > 0 {
                            parts.push(format!("overwrote {}", summary.overwritten));
                        }
                        if summary.skipped > 0 {
                            parts.push(format!("skipped {}", summary.skipped));
                        }
                        this.set_transfer_message(parts.join(", "), true, cx);
                        entity.update(cx, |studio, cx| {
                            studio.refetch_snippets(window, cx);
                            studio.refetch_status(cx);
                        });
                    }
                    Err(err) => {
                        this.set_transfer_message(err, false, cx);
                    }
                }
            })
            .ok();
        })
        .detach();
    }

    fn select_row(&self, snippet: &SnippetView, window: &mut Window, cx: &mut App) {
        if let Some(studio) = self.studio.upgrade() {
            studio.update(cx, |studio, cx| {
                studio.select_snippet(snippet, window, cx);
            });
        }
    }

    fn start_new(&self, window: &mut Window, cx: &mut App) {
        if let Some(studio) = self.studio.upgrade() {
            studio.update(cx, |studio, cx| {
                studio.start_new_snippet(window, cx);
            });
        }
    }

    fn delete_row(&self, trigger: String, window: &mut Window, cx: &mut App) {
        if let Some(studio) = self.studio.upgrade() {
            studio.update(cx, |studio, cx| {
                studio.delete_snippet(trigger, window, cx);
            });
        }
    }

    fn grip_keys(
        &self,
        key: &str,
        index: usize,
        count: usize,
        window: &mut Window,
        cx: &mut App,
    ) {
        if self.reorder_locked() {
            return;
        }
        let Some(studio) = self.studio.upgrade() else {
            return;
        };
        if key == "up" && index > 0 {
            studio.update(cx, |studio, cx| {
                studio.apply_reorder(index, index - 1, window, cx);
            });
        } else if key == "down" && index + 1 < count {
            studio.update(cx, |studio, cx| {
                studio.apply_reorder(index, index + 1, window, cx);
            });
        }
    }

    /// Parity arms live only in this file (RULEBOOK §2 / §10).
    pub fn parity_state(
        &mut self,
        name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.parity_hover_row = None;
        match name {
            "default-selected" => {
                // Studio::new already selects the first snippet; ensure empty search.
                self.set_search_text_parity("", false, window, cx);
                true
            }
            "row-hover" => {
                self.set_search_text_parity("", false, window, cx);
                self.parity_hover_row = Some(1);
                cx.notify();
                true
            }
            "search-filter" => {
                // Reference capture leaves the search field focused after typing.
                self.set_search_text_parity("o", true, window, cx);
                true
            }
            "search-no-match" => {
                self.set_search_text_parity("zzzzz", true, window, cx);
                true
            }
            "empty-library" => {
                self.set_search_text_parity("", false, window, cx);
                let studio = self.studio.clone();
                cx.defer_in(window, move |_, window, cx| {
                    if let Some(entity) = studio.upgrade() {
                        entity.update(cx, |studio, cx| {
                            let triggers: Vec<String> =
                                studio.snippets.iter().map(|s| s.trigger.clone()).collect();
                            let shell = cx.global::<Shell>();
                            for trigger in triggers {
                                let _ = commands::remove_snippet(
                                    &shell.state,
                                    &shell.hotkeys,
                                    &trigger,
                                );
                            }
                            studio.refetch_snippets(window, cx);
                            studio.refetch_status(cx);
                            studio.start_new_snippet(window, cx);
                        });
                    }
                });
                true
            }
            _ => false,
        }
    }
}

/// Take drag state and optionally commit `apply_reorder` with a live `Window`.
fn commit_drag(
    entity: &WeakEntity<Sidebar>,
    commit: bool,
    window: &mut Window,
    cx: &mut App,
) {
    let Some((from, to, studio)) = entity
        .update(cx, |this, cx| {
            let state = this.drag.take();
            cx.notify();
            state.map(|s| (s.from, s.drop_idx, this.studio.clone()))
        })
        .ok()
        .flatten()
    else {
        return;
    };
    if commit && let Some(studio) = studio.upgrade() {
        studio.update(cx, |studio, cx| {
            studio.apply_reorder(from, to, window, cx);
        });
    }
}

fn expansion_stats(expansion: &str) -> SharedString {
    // JS `"".split("\n")` → `[""]` length 1; same as Rust split count on empty.
    let lines = expansion.split('\n').count();
    let chars = expansion.encode_utf16().count();
    if lines > 1 {
        format!("{lines} lines").into()
    } else {
        format!("{chars} chars").into()
    }
}

fn excerpt_text(expansion: &str) -> SharedString {
    expansion.replace('\n', " ↵ ").into()
}

impl Render for Sidebar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {

        let studio = self.studio.upgrade();
        let snippets = studio
            .as_ref()
            .map(|e| e.read(cx).snippets.clone())
            .unwrap_or_default();
        let selected = studio.as_ref().and_then(|e| e.read(cx).selected.clone());
        let creating_new = studio.as_ref().is_some_and(|e| e.read(cx).creating_new);
        let freeze = studio.as_ref().is_some_and(|e| e.read(cx).freeze_animations);

        let filtered = self.filtered(&snippets);
        let filtered_count = filtered.len();
        let reorder_locked = self.reorder_locked();
        let drag = self.drag.get();
        let transfer_busy = self.transfer_busy;
        let transfer_message = self.transfer_message.clone();
        let parity_hover = self.parity_hover_row;
        let search_nonempty = !self.search.is_empty();
        let has_query = !self.search.trim().is_empty();

        let search_input = self.search_input.clone();
        let search_focused = search_input.focus_handle(cx).is_focused(window);

        let entity = cx.entity().downgrade();
        let scroll = self.scroll.clone();
        let drag_cell = self.drag.clone();
        let pending_cell = self.pending_drag.clone();
        let drag_listening = drag.is_some() || pending_cell.get().is_some();

        // .studio-sidebar — styles.css:213-219
        v_flex()
            .id("studio-sidebar")
            .w(px(340.))
            .h_full()
            .flex_none()
            .bg(tokens::surface_1())
            .border_r_1()
            .border_color(tokens::line())
            .min_h_0()
            // Window-level drag listeners while a reorder is armed or active (rule 14).
            .when(drag_listening, {
                let entity = entity.clone();
                let drag_cell = drag_cell.clone();
                let pending_cell = pending_cell.clone();
                let count = snippets.len();
                move |this| {
                    this.child(
                        canvas(
                            |_, _, _| {},
                            move |_, _, window, _cx| {
                                window.on_mouse_event({
                                    let entity = entity.clone();
                                    let drag_cell = drag_cell.clone();
                                    let pending_cell = pending_cell.clone();
                                    move |ev: &MouseMoveEvent, phase, window, cx| {
                                        if phase != DispatchPhase::Bubble {
                                            return;
                                        }
                                        if drag_cell.get().is_none() && pending_cell.get().is_none()
                                        {
                                            return;
                                        }
                                        // Missed release: buttons == 0 → commit (App.tsx:531-533).
                                        if ev.pressed_button != Some(MouseButton::Left) {
                                            if drag_cell.get().is_some() {
                                                commit_drag(&entity, true, window, cx);
                                            } else {
                                                entity
                                                    .update(cx, |this, cx| {
                                                        this.clear_pending_drag(cx);
                                                    })
                                                    .ok();
                                            }
                                            return;
                                        }
                                        let y = f32::from(ev.position.y);
                                        entity
                                            .update(cx, |this, cx| {
                                                this.on_drag_move(y, count, cx);
                                            })
                                            .ok();
                                    }
                                });
                                window.on_mouse_event({
                                    let entity = entity.clone();
                                    let drag_cell = drag_cell.clone();
                                    move |ev: &MouseUpEvent, phase, window, cx| {
                                        if phase != DispatchPhase::Bubble
                                            || ev.button != MouseButton::Left
                                        {
                                            return;
                                        }
                                        if drag_cell.get().is_some() {
                                            commit_drag(&entity, true, window, cx);
                                        } else {
                                            entity
                                                .update(cx, |this, cx| {
                                                    this.clear_pending_drag(cx);
                                                })
                                                .ok();
                                        }
                                    }
                                });
                            },
                        )
                        .absolute()
                        .size_0(),
                    )
                }
            })
            .child(self.render_top_section(
                search_input,
                search_focused,
                search_nonempty,
                transfer_busy,
                transfer_message,
                entity.clone(),
            ))
            .child(self.render_list_header(filtered_count))
            .child(self.render_list(
                &filtered,
                &selected,
                creating_new,
                reorder_locked,
                has_query,
                freeze,
                parity_hover,
                drag,
                scroll,
                entity,
                window,
                cx,
            ))
    }
}

impl Sidebar {
    fn render_top_section(
        &self,
        search_input: Entity<InputState>,
        search_focused: bool,
        search_nonempty: bool,
        transfer_busy: bool,
        transfer_message: String,
        entity: WeakEntity<Self>,
    ) -> impl IntoElement {
        // .sidebar-top-section — styles.css:221-227
        v_flex()
            .id("sidebar-top-section")
            .p(px(14.))
            .gap(px(10.))
            .border_b_1()
            .border_color(tokens::line())
            .child(
                // .search-bar-wrap — styles.css:229-233
                div()
                    .id("search-bar-wrap")
                    .relative()
                    .w_full()
                    .child(
                        // .studio-search-input — styles.css:245-260
                        // Kit Input still applies size padding/height (rule 17); zero py and
                        // force the style height so the frame stays 32px.
                        div()
                            .id("studio-search-input")
                            .w_full()
                            .h(px(32.))
                            .flex()
                            .items_center()
                            .overflow_hidden()
                            .bg(tokens::field())
                            .border_1()
                            .border_color(if search_focused {
                                tokens::accent()
                            } else {
                                tokens::line()
                            })
                            .rounded(px(7.))
                            .when(search_focused, |this| {
                                this.shadow(vec![BoxShadow {
                                    color: tokens::ring_soft(),
                                    offset: point(px(0.), px(0.)),
                                    blur_radius: px(0.),
                                    spread_radius: px(2.),
                                    inset: false,
                                }])
                            })
                            .child(
                                Input::new(&search_input)
                                    .id("studio-search-field")
                                    .appearance(false)
                                    .bordered(false)
                                    .tab_index(0)
                                    .w_full()
                                    .pl(px(30.))
                                    .pr(px(28.))
                                    .py(px(0.))
                                    .text_size(px(12.5))
                                    .line_height(px(32.))
                                    .text_color(tokens::ink())
                                    .font_family(tokens::FONT_UI)
                                    .map(|mut this| {
                                        this.style().size.height = Some(px(32.).into());
                                        this
                                    }),
                            ),
                    )
                    .child(
                        // .search-icon — styles.css:235-243. After the input frame: CSS
                        // absolute paints above static siblings; GPUI paints in tree order.
                        div()
                            .id("search-icon")
                            .absolute()
                            .left(px(10.))
                            .top(px(0.))
                            .h(px(32.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(tokens::ink_3())
                            .child(icons::search()),
                    )
                    .when(search_nonempty, {
                        let entity = entity.clone();
                        move |this| {
                            this.child(
                                // .search-clear-btn — styles.css:262-275
                                div()
                                    .id("search-clear-btn")
                                    .absolute()
                                    .right(px(8.))
                                    .top(px(0.))
                                    .h(px(32.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .p(px(4.))
                                    .rounded(px(4.))
                                    .text_color(tokens::ink_3())
                                    .cursor(CursorStyle::PointingHand)
                                    .tab_index(0)
                                    .focus_visible(|s| {
                                        s.shadow(tokens::focus_visible_shadows(tokens::field()))
                                    })
                                    .hover(|s| s.text_color(tokens::ink()))
                                    .tooltip(move |window, cx| {
                                        Tooltip::new("Clear search").build(window, cx)
                                    })
                                    .on_key_down({
                                        let entity = entity.clone();
                                        move |ev, window, cx| {
                                            if ev.keystroke.modifiers.modified() {
                                                return;
                                            }
                                            match ev.keystroke.key.as_str() {
                                                "enter" | "space" => {
                                                    entity
                                                        .update(cx, |this, cx| {
                                                            this.clear_search(window, cx);
                                                        })
                                                        .ok();
                                                    cx.stop_propagation();
                                                }
                                                _ => {}
                                            }
                                        }
                                    })
                                    .on_click({
                                        let entity = entity.clone();
                                        move |_, window, cx| {
                                            entity
                                                .update(cx, |this, cx| {
                                                    this.clear_search(window, cx);
                                                })
                                                .ok();
                                        }
                                    })
                                    .child(icons::close()),
                            )
                        }
                    }),
            )
            .child(
                // .sidebar-action-bar — styles.css:277-281
                h_flex()
                    .id("sidebar-action-bar")
                    .gap(px(8.))
                    .items_center()
                    .w_full()
                    .child({
                        let entity = entity.clone();
                        // .new-snippet-btn — styles.css:283-301
                        h_flex()
                            .id("new-snippet-btn")
                            .flex_1()
                            .min_w_0()
                            .h(px(28.))
                            .bg(tokens::accent())
                            .rounded(px(6.))
                            .text_color(tokens::white())
                            .text_size(px(12.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .items_center()
                            .justify_center()
                            .gap(px(6.))
                            .whitespace_nowrap()
                            .cursor(CursorStyle::PointingHand)
                            .tab_index(0)
                            .focus_visible(|s| {
                                s.shadow(tokens::focus_visible_shadows(tokens::surface_1()))
                            })
                            .hover(|s| s.bg(tokens::accent_hover()))
                            .on_key_down({
                                let entity = entity.clone();
                                move |ev, window, cx| {
                                    if ev.keystroke.modifiers.modified() {
                                        return;
                                    }
                                    match ev.keystroke.key.as_str() {
                                        "enter" | "space" => {
                                            entity
                                                .update(cx, |this, cx| {
                                                    this.start_new(window, cx);
                                                })
                                                .ok();
                                            cx.stop_propagation();
                                        }
                                        _ => {}
                                    }
                                }
                            })
                            .on_click({
                                let entity = entity.clone();
                                move |_, window, cx| {
                                    entity
                                        .update(cx, |this, cx| {
                                            this.start_new(window, cx);
                                        })
                                        .ok();
                                }
                            })
                            .child(icons::plus())
                            .child(
                                div()
                                    .whitespace_nowrap()
                                    .child("New Snippet"),
                            )
                    })
                    .child(
                        // .library-io-btns — styles.css:303-326
                        h_flex()
                            .id("library-io-btns")
                            .gap(px(4.))
                            .child(io_button(
                                "export-btn",
                                "Export",
                                "Export snippets to JSON",
                                icons::export(),
                                !transfer_busy,
                                {
                                    let entity = entity.clone();
                                    move |window, cx| {
                                        entity
                                            .update(cx, |this, cx| {
                                                this.export_snippets(window, cx);
                                            })
                                            .ok();
                                    }
                                },
                            ))
                            .child(io_button(
                                "import-btn",
                                "Import",
                                "Import snippets from JSON",
                                icons::import(),
                                !transfer_busy,
                                {
                                    let entity = entity.clone();
                                    move |window, cx| {
                                        entity
                                            .update(cx, |this, cx| {
                                                this.import_snippets(window, cx);
                                            })
                                            .ok();
                                    }
                                },
                            )),
                    ),
            )
            .when(!transfer_message.is_empty(), move |this| {
                this.child(
                    // .transfer-toast — styles.css:328-335
                    div()
                        .id("transfer-toast")
                        .text_size(px(11.5))
                        .text_color(tokens::accent_ink())
                        .bg(tokens::transfer_toast_bg())
                        .px(px(8.))
                        .py(px(4.))
                        .rounded(px(5.))
                        .border_1()
                        .border_color(tokens::transfer_toast_border())
                        .child(transfer_message),
                )
            })
    }

    fn render_list_header(&self, filtered_count: usize) -> impl IntoElement {
        // .sidebar-list-header — styles.css:337-342
        // Padding 10px 14px 4px around one 11px/1.5 line: box 30.5, text line
        // at +10 (reference title line y=151 for the header at 141).
        h_flex()
            .id("sidebar-list-header")
            // 30.5 in the reference; 31 keeps the list on Chromium's rounded grid
            // (list top 172, first row 178).
            .h(px(31.))
            .pt(px(10.))
            .px(px(14.))
            .justify_between()
            .items_start()
            .child(
                // .list-title — styles.css:343-348 (letter-spacing D2)
                div()
                    .id("list-title")
                    .text_size(px(11.))
                    .line_height(px(16.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(tokens::ink_3())
                    .child("SNIPPETS"),
            )
            .child(
                // .list-count — styles.css:349-353
                div()
                    .id("list-count")
                    .text_size(px(11.))
                    .line_height(px(16.))
                    .text_color(tokens::ink_3())
                    .font_family(tokens::FONT_MONO)
                    .child(format!("{filtered_count} total")),
            )
    }

    #[allow(clippy::too_many_arguments)]
    fn render_list(
        &self,
        filtered: &[&SnippetView],
        selected: &Option<String>,
        creating_new: bool,
        reorder_locked: bool,
        has_query: bool,
        freeze: bool,
        parity_hover: Option<usize>,
        drag: Option<DragState>,
        scroll: ScrollHandle,
        entity: WeakEntity<Self>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let count = filtered.len();
        // .studio-snippets-list — styles.css:355-362
        // Ref: header ends ~171.5 + pad-top 6 → first row 177.5.
        // Integer GPUI: header h31 ends 172; pt 6 → row at 178 (matches raster).
        // Chromium's classic scrollbar takes 15px of layout width while the
        // list overflows (rule 25); the Kit scrollbar is an overlay, so reserve
        // that gutter on the right. max_offset is from the previous layout.
        let overflows = scroll.max_offset().y > px(0.);
        let list = v_flex()
            .id("studio-snippets-list")
            .relative()
            .flex_1()
            .min_h_0()
            .pt(px(6.))
            .pl(px(10.))
            .pr(px(if overflows { 25. } else { 10. }))
            .pb(px(14.))
            .gap(px(4.))
            .overflow_y_scroll()
            .track_scroll(&scroll);

        if filtered.is_empty() {
            return list
                .child(self.render_empty(has_query, entity))
                .into_any_element();
        }

        let shifts: Vec<Pixels> = (0..filtered.len())
            .map(|i| self.row_shift(i, freeze, window, cx))
            .collect();
        let mut rows = Vec::with_capacity(filtered.len());
        for (i, snippet) in filtered.iter().enumerate() {
            let is_active =
                !creating_new && selected.as_deref() == Some(snippet.trigger.as_str());
            let force_hover = parity_hover == Some(i);
            let is_dragging = drag.is_some_and(|d| d.from == i);
            rows.push(render_row(
                snippet,
                i,
                count,
                is_active,
                force_hover,
                is_dragging,
                shifts[i],
                reorder_locked,
                entity.clone(),
                window,
                cx,
            ));
        }

        list.children(rows)
            .when(overflows, |list| {
                list.child(
                    div().absolute().inset_0().child(
                        Scrollbar::vertical(&scroll)
                            .id("studio-snippets-scrollbar")
                            .mode(ScrollbarMode::Always)
                            .viewport_from_layout(),
                    ),
                )
            })
            .into_any_element()
    }

    fn render_empty(&self, has_query: bool, entity: WeakEntity<Self>) -> impl IntoElement {
        // .sidebar-empty — styles.css:473-481
        v_flex()
            .id("sidebar-empty")
            .items_center()
            .justify_center()
            .p(px(40.))
            .px(px(16.))
            .gap(px(10.))
            .child(
                // .empty-icon — styles.css:482-488
                div()
                    .id("empty-icon")
                    .text_color(tokens::ink_3())
                    .opacity(0.6)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icons::document()),
            )
            .child(
                // .empty-text — styles.css:489
                div()
                    .id("empty-text")
                    .text_size(px(12.5))
                    .text_color(tokens::ink_2())
                    .child(if has_query {
                        "No matching snippets found"
                    } else {
                        "No snippets in library"
                    }),
            )
            .child(widgets::btn_create_first("btn-create-first", "Create Snippet", {
                move |window, cx| {
                    entity
                        .update(cx, |this, cx| {
                            this.start_new(window, cx);
                        })
                        .ok();
                }
            }))
    }
}

fn io_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    tip: impl Into<SharedString>,
    icon: impl IntoElement,
    enabled: bool,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let tip: SharedString = tip.into();
    let on_click = Rc::new(on_click);
    h_flex()
        .id(id)
        .h(px(28.))
        .px(px(8.))
        .bg(tokens::surface_2())
        .border_1()
        .border_color(tokens::line())
        .rounded(px(6.))
        .text_color(tokens::ink_2())
        .text_size(px(11.5))
        .font_weight(FontWeight::SEMIBOLD)
        .items_center()
        .gap(px(5.))
        .when(enabled, |this| {
            this.cursor(CursorStyle::PointingHand)
                .tab_index(0)
                .focus_visible(|s| {
                    s.shadow(tokens::focus_visible_shadows(tokens::surface_1()))
                })
                .hover(|s| {
                    s.bg(tokens::surface_3())
                        .text_color(tokens::ink())
                        .border_color(tokens::line_2())
                })
                .tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
                .on_key_down({
                    let on_click = on_click.clone();
                    move |ev, window, cx| {
                        if ev.keystroke.modifiers.modified() {
                            return;
                        }
                        match ev.keystroke.key.as_str() {
                            "enter" | "space" => {
                                on_click(window, cx);
                                cx.stop_propagation();
                            }
                            _ => {}
                        }
                    }
                })
                .on_click({
                    let on_click = on_click.clone();
                    move |_, window, cx| on_click(window, cx)
                })
        })
        .when(!enabled, |this| this.cursor(CursorStyle::Arrow))
        .child(icon)
        .child(label.into())
}

#[allow(clippy::too_many_arguments)]
fn render_row(
    snippet: &SnippetView,
    index: usize,
    count: usize,
    is_active: bool,
    force_hover: bool,
    is_dragging: bool,
    shift: Pixels,
    reorder_locked: bool,
    entity: WeakEntity<Sidebar>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let trigger = snippet.trigger.clone();
    let expansion = snippet.expansion.clone();
    let kind = snippet.kind;
    let stats = expansion_stats(&expansion);
    let excerpt = excerpt_text(&expansion);
    let tip_full = expansion.clone();
    let group_name: SharedString = format!("snippet-row-{trigger}").into();
    let row_id = SharedString::from(format!("studio-item-{trigger}"));
    let grip_id = SharedString::from(format!("drag-grip-{trigger}"));
    let delete_id = SharedString::from(format!("item-delete-{trigger}"));
    let grip_focus = window
        .use_keyed_state(
            SharedString::from(format!("drag-grip-focus-{trigger}")),
            cx,
            |_, cx| cx.focus_handle(),
        )
        .read(cx)
        .clone()
        .tab_index(0)
        .tab_stop(true);
    let grip_focus_mouse = grip_focus.clone();

    let delete_focus = window
        .use_keyed_state(
            SharedString::from(format!("item-delete-focus-{trigger}")),
            cx,
            |_, cx| cx.focus_handle(),
        )
        .read(cx)
        .clone()
        .tab_index(0)
        .tab_stop(true);
    let delete_focus_mouse = delete_focus.clone();

    let hovered_bg = force_hover || is_dragging;
    // .drag-grip — styles.css:385-397
    let grip_tip: SharedString = if reorder_locked {
        "Clear the search to reorder".into()
    } else {
        "Drag to reorder".into()
    };
    let grip = div()
        .id(grip_id)
        .p(px(2.))
        .flex()
        .items_center()
        .justify_center()
        .text_color(tokens::ink_3())
        .opacity(if force_hover || is_dragging {
            1.
        } else {
            0.4
        })
        .when(!reorder_locked, |this| {
            this.cursor(CursorStyle::OpenHand)
                .track_focus(&grip_focus)
                .tab_index(0)
                .tab_stop(true)
                .focus_visible(|s| s.shadow(tokens::focus_visible_shadows(tokens::surface_2())))
                .group_hover(group_name.clone(), |s| s.opacity(1.))
                .tooltip({
                    let tip = grip_tip.clone();
                    move |window, cx| Tooltip::new(tip.clone()).build(window, cx)
                })
                .on_mouse_down(MouseButton::Left, {
                    let entity = entity.clone();
                    move |ev, window, cx| {
                        // Focus so ArrowUp/ArrowDown work after a plain click.
                        // Arm pending drag; first move promotes it (keeps click
                        // from committing a no-op reorder).
                        grip_focus_mouse.focus(window, cx);
                        let y = f32::from(ev.position.y);
                        entity
                            .update(cx, |this, cx| {
                                this.arm_drag(index, y, DRAG_ROW_HEIGHT, cx);
                            })
                            .ok();
                        cx.stop_propagation();
                    }
                })
                .on_key_down({
                    let entity = entity.clone();
                    move |ev, window, cx| {
                        if ev.keystroke.modifiers.modified() {
                            return;
                        }
                        let key = ev.keystroke.key.as_str();
                        if key == "up" || key == "down" {
                            entity
                                .update(cx, |this, cx| {
                                    this.grip_keys(key, index, count, window, cx);
                                })
                                .ok();
                            cx.stop_propagation();
                        }
                    }
                })
                .on_click(|_, _, cx| cx.stop_propagation())
        })
        .when(reorder_locked, |this| {
            this.cursor(CursorStyle::Arrow)
                .tooltip(move |window, cx| Tooltip::new(grip_tip.clone()).build(window, cx))
                .on_click(|_, _, cx| cx.stop_propagation())
        })
        .child(icons::grip());

    // .item-content-body — styles.css:399-405
    let body = v_flex()
        .id(SharedString::from(format!("item-content-{trigger}")))
        .flex_1()
        .min_w_0()
        .gap(px(3.))
        .child(
            // .item-header-row — styles.css:407-411
            h_flex()
                .id(SharedString::from(format!("item-header-{trigger}")))
                .justify_between()
                .items_center()
                .w_full()
                .min_w_0()
                .child(trigger_badge(&trigger, kind))
                .child(
                    // .item-stats-badge — styles.css:439-443
                    div()
                        .id(SharedString::from(format!("item-stats-{trigger}")))
                        .text_size(px(10.5))
                        .text_color(tokens::ink_3())
                        .font_family(tokens::FONT_MONO)
                        .child(stats),
                ),
        )
        .child(
            // .item-excerpt — styles.css:445-451
            div()
                .id(SharedString::from(format!("item-excerpt-{trigger}")))
                .text_size(px(12.))
                .text_color(tokens::ink_2())
                .truncate()
                .tooltip(move |window, cx| Tooltip::new(tip_full.clone()).build(window, cx))
                .child(excerpt),
        );

    // .item-delete-btn — styles.css:453-471
    let delete = div()
        .id(delete_id)
        .w(px(22.))
        .h(px(22.))
        .rounded(px(4.))
        .flex()
        .items_center()
        .justify_center()
        .text_color(tokens::ink_3())
        // Keep a tiny opacity when idle so the hit target stays live; hover
        // / group_hover still ramp to 1 for the visual (styles.css:453-471).
        .opacity(if force_hover { 1. } else { 0. })
        .cursor(CursorStyle::PointingHand)
        .track_focus(&delete_focus)
        .tab_index(0)
        .tab_stop(true)
        .focus_visible(|s| s.shadow(tokens::focus_visible_shadows(tokens::surface_2())))
        .group_hover(group_name.clone(), |s| s.opacity(1.))
        .hover(|s| s.bg(tokens::danger_tint()).text_color(tokens::danger()).opacity(1.))
        .tooltip(move |window, cx| Tooltip::new("Delete snippet").build(window, cx))
        .on_key_down({
            let entity = entity.clone();
            let trigger = trigger.clone();
            move |ev, window, cx| {
                if ev.keystroke.modifiers.modified() {
                    return;
                }
                match ev.keystroke.key.as_str() {
                    "enter" | "space" => {
                        entity
                            .update(cx, |this, cx| {
                                this.delete_row(trigger.clone(), window, cx);
                            })
                            .ok();
                        cx.stop_propagation();
                    }
                    _ => {}
                }
            }
        })
        .on_click({
            let entity = entity.clone();
            let trigger = trigger.clone();
            let delete_focus_mouse = delete_focus_mouse.clone();
            move |_, window, cx| {
                delete_focus_mouse.focus(window, cx);
                entity
                    .update(cx, |this, cx| {
                        this.delete_row(trigger.clone(), window, cx);
                    })
                    .ok();
                cx.stop_propagation();
            }
        })
        .child(icons::close());

    // .studio-item — styles.css:364-383
    h_flex()
        .id(row_id)
        .group(group_name)
        .relative()
        .top(shift)
        .when(is_dragging, |this| this.occlude())
        .items_center()
        .gap(px(8.))
        // Never shrink in the scroll list (rule 23b).
        .flex_shrink_0()
        .h(px(row_height(index)))
        .px(px(10.))
        .py(px(8.))
        .bg(if is_active {
            tokens::active_row_bg()
        } else if hovered_bg {
            tokens::surface_3()
        } else {
            tokens::surface_2()
        })
        .border_1()
        .border_color(if is_active {
            tokens::accent()
        } else if hovered_bg {
            tokens::line_2()
        } else {
            transparent_black()
        })
        .rounded(px(8.))
        .cursor(CursorStyle::PointingHand)
        .when(!is_active && !force_hover, |this| {
            this.hover(|s| s.bg(tokens::surface_3()).border_color(tokens::line_2()))
        })
        .on_click({
            let entity = entity.clone();
            let snippet = SnippetView {
                trigger: trigger.clone(),
                expansion: expansion.clone(),
                kind,
            };
            move |_, window, cx| {
                entity
                    .update(cx, |this, cx| {
                        this.select_row(&snippet, window, cx);
                    })
                    .ok();
            }
        })
        .child(grip)
        .child(body)
        .child(delete)
        .into_any_element()
}
