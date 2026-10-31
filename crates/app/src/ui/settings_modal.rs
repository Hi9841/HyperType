//! Settings modal (S5). Spec: App.tsx:957-1266, styles.css:795-973 + 1074-1219.
//!
//! Renders when `studio.modal == Some(Modal::Settings)` via `widgets::modal_frame`
//! (500px / 85vh). Backdrop click closes; Escape must not (A4 — shared frame
//! still has Escape; see Requests).

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::scroll::{Scrollbar, ScrollbarMode};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use hypertype_core::commands::{self, Status};
use hypertype_core::expansion::{InsertMode, PasteCombo};

use crate::ui::icons;
use crate::ui::studio::{Modal, Studio};
use crate::ui::tokens;
use crate::ui::update;
use crate::ui::widgets::{self, on_value_edit_event, SegKind};

const POLL_MS: u64 = 1500;
const HOOK_MSG_MS: u64 = 3500;

const INSERT_OPTIONS: [(InsertMode, &str); 3] = [
    (InsertMode::Auto, "Auto"),
    (InsertMode::Paste, "Paste"),
    (InsertMode::Type, "Type"),
];

const PASTE_OPTIONS: [(PasteCombo, &str); 3] = [
    (PasteCombo::CtrlV, "Ctrl+V"),
    (PasteCombo::ShiftInsert, "Shift+Ins"),
    (PasteCombo::CtrlShiftV, "Ctrl+⇧+V"),
];

fn insert_sub(mode: InsertMode, words: u32) -> SharedString {
    match mode {
        InsertMode::Paste => "Always pastes via clipboard".into(),
        InsertMode::Type => "Always types at configured WPM".into(),
        InsertMode::Auto => format!(
            "Types out ≤ {words} words at set WPM; pastes longer text"
        )
        .into(),
    }
}

fn format_thousands(n: u64) -> String {
    let digits: Vec<char> = n.to_string().chars().collect();
    let mut out = String::new();
    for (i, ch) in digits.iter().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(*ch);
    }
    out
}

fn hook_label(active: bool, healthy: bool) -> SharedString {
    if active {
        if healthy {
            "Active & Healthy".into()
        } else {
            "Active (Verifying)".into()
        }
    } else {
        "Detached".into()
    }
}

pub struct SettingsModal {
    studio: WeakEntity<Studio>,
    _observe: Subscription,
    /// Live drag values (None = read from status). Cell so slider handlers
    /// can write without re-entering SettingsModal::update mid-gesture.
    auto_words_drag: Rc<Cell<Option<f32>>>,
    wpm_drag: Rc<Cell<Option<f32>>>,
    restore_drag: Rc<Cell<Option<f32>>>,
    /// `.value-edit` inputs (RULEBOOK section 5: created once in `new`).
    words_edit: Entity<InputState>,
    wpm_edit: Entity<InputState>,
    restore_edit: Entity<InputState>,
    _edit_subs: [Subscription; 3],
    /// Last status value written into each `.value-edit` (words, wpm,
    /// restore). Solid's `value={signal()}` writes the input only when the
    /// signal changes; writing on every render would undo typing before the
    /// Enter -> blur -> commit path reads it.
    synced: [Cell<Option<u32>>; 3],
    settings_error: String,
    repairing_hook: bool,
    elevating: bool,
    hook_message: String,
    poll_task: Option<Task<()>>,
    hook_msg_task: Option<Task<()>>,
    scroll: ScrollHandle,
}

impl SettingsModal {
    pub fn new(
        studio: WeakEntity<Studio>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let _observe = studio
            .upgrade()
            .map(|entity| cx.observe(&entity, |_, _, cx| cx.notify()))
            .expect("Studio must exist when SettingsModal is created");

        let words_edit = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("")
                .default_value("15")
        });
        let wpm_edit = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("")
                .default_value("600")
        });
        let restore_edit = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("")
                .default_value("5000")
        });

        let auto_words_drag: Rc<Cell<Option<f32>>> = Rc::new(Cell::new(None));
        let wpm_drag: Rc<Cell<Option<f32>>> = Rc::new(Cell::new(None));
        let restore_drag: Rc<Cell<Option<f32>>> = Rc::new(Cell::new(None));

        let words_sub = cx.subscribe_in(&words_edit, window, {
            let edit = words_edit.clone();
            let drag = auto_words_drag.clone();
            move |this, _input, event: &InputEvent, window, cx| {
                let current = this.current_auto_words(cx) as f32;
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
                    drag.set(None);
                    this.commit_auto_words(v.round() as u32, cx);
                }
            }
        });

        let wpm_sub = cx.subscribe_in(&wpm_edit, window, {
            let edit = wpm_edit.clone();
            let drag = wpm_drag.clone();
            move |this, _input, event: &InputEvent, window, cx| {
                let current = this.current_wpm(cx) as f32;
                let mut committed = None;
                on_value_edit_event(
                    &edit,
                    event,
                    100.,
                    1500.,
                    current,
                    |v, _window, _cx| {
                        committed = Some(v);
                    },
                    window,
                    cx,
                );
                if let Some(v) = committed {
                    drag.set(None);
                    this.commit_wpm(v.round() as u32, cx);
                }
            }
        });

        let restore_sub = cx.subscribe_in(&restore_edit, window, {
            let edit = restore_edit.clone();
            let drag = restore_drag.clone();
            move |this, _input, event: &InputEvent, window, cx| {
                let current = this.current_restore_ms(cx) as f32;
                let mut committed = None;
                on_value_edit_event(
                    &edit,
                    event,
                    3000.,
                    15000.,
                    current,
                    |v, _window, _cx| {
                        committed = Some(v);
                    },
                    window,
                    cx,
                );
                if let Some(v) = committed {
                    drag.set(None);
                    this.commit_restore_ms(v.round() as u32, cx);
                }
            }
        });


        Self {
            studio,
            _observe,
            auto_words_drag,
            wpm_drag,
            restore_drag,
            words_edit,
            wpm_edit,
            restore_edit,
            _edit_subs: [words_sub, wpm_sub, restore_sub],
            synced: [Cell::new(None), Cell::new(None), Cell::new(None)],
            settings_error: String::new(),
            repairing_hook: false,
            elevating: false,
            hook_message: String::new(),
            poll_task: None,
            hook_msg_task: None,
            scroll: ScrollHandle::new(),
        }
    }

    fn status(&self, cx: &App) -> Option<Status> {
        self.studio.upgrade().map(|e| e.read(cx).status.clone())
    }

    fn current_auto_words(&self, cx: &App) -> u32 {
        self.auto_words_drag
            .get()
            .map(|v| v.round() as u32)
            .or_else(|| self.status(cx).map(|s| s.auto_paste_words))
            .unwrap_or(15)
    }

    fn current_wpm(&self, cx: &App) -> u32 {
        self.wpm_drag
            .get()
            .map(|v| v.round() as u32)
            .or_else(|| self.status(cx).map(|s| s.wpm))
            .unwrap_or(600)
    }

    fn current_restore_ms(&self, cx: &App) -> u32 {
        self.restore_drag
            .get()
            .map(|v| v.round() as u32)
            .or_else(|| self.status(cx).map(|s| s.restore_delay_ms))
            .unwrap_or(5000)
    }

    fn ensure_poll(&mut self, cx: &mut Context<Self>) {
        if self.poll_task.is_some() {
            return;
        }
        let studio = self.studio.clone();
        self.poll_task = Some(cx.spawn(async move |_this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(POLL_MS))
                    .await;
                let Some(entity) = studio.upgrade() else {
                    break;
                };
                let still_open = entity.update(cx, |studio, cx| {
                    if studio.modal == Some(Modal::Settings) {
                        studio.refetch_status(cx);
                        true
                    } else {
                        false
                    }
                });
                if !still_open {
                    break;
                }
            }
        }));
    }

    fn clear_hook_message_later(&mut self, cx: &mut Context<Self>) {
        self.hook_msg_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(HOOK_MSG_MS))
                .await;
            this.update(cx, |this, cx| {
                this.hook_message.clear();
                cx.notify();
            })
            .ok();
        }));
    }

    fn close(studio: &WeakEntity<Studio>, this: &Entity<Self>, cx: &mut App) {
        this.update(cx, |this, _cx| {
            this.poll_task = None;
            this.hook_msg_task = None;
        });
        if let Some(entity) = studio.upgrade() {
            entity.update(cx, |studio, cx| {
                studio.set_modal(None, cx);
            });
        }
    }

    fn set_settings_error(&mut self, err: String, cx: &mut Context<Self>) {
        self.settings_error = err;
        cx.notify();
    }

    fn clear_settings_error(&mut self, cx: &mut Context<Self>) {
        if !self.settings_error.is_empty() {
            self.settings_error.clear();
            cx.notify();
        }
    }

    fn with_studio<R>(
        &self,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut Studio, &mut Context<Studio>) -> R,
    ) -> Option<R> {
        let entity = self.studio.upgrade()?;
        Some(entity.update(cx, f))
    }

    fn commit_auto_words(&mut self, value: u32, cx: &mut Context<Self>) {
        self.clear_settings_error(cx);
        let state = self.with_studio(cx, |studio, cx| {
            studio.patch_status(
                |s| {
                    s.auto_paste_words = value;
                },
                cx,
            );
            studio.state.clone()
        });
        let Some(state) = state else {
            return;
        };
        if let Err(err) = commands::set_auto_paste_words(&state, value) {
            self.set_settings_error(err, cx);
            self.with_studio(cx, |studio, cx| studio.refetch_status(cx));
        }
        cx.notify();
    }

    fn commit_wpm(&mut self, value: u32, cx: &mut Context<Self>) {
        self.clear_settings_error(cx);
        let state = self.with_studio(cx, |studio, cx| {
            studio.patch_status(
                |s| {
                    s.wpm = value;
                },
                cx,
            );
            studio.state.clone()
        });
        let Some(state) = state else {
            return;
        };
        if let Err(err) = commands::set_wpm(&state, value) {
            self.set_settings_error(err, cx);
            self.with_studio(cx, |studio, cx| studio.refetch_status(cx));
        }
        cx.notify();
    }

    fn commit_restore_ms(&mut self, value: u32, cx: &mut Context<Self>) {
        self.clear_settings_error(cx);
        let state = self.with_studio(cx, |studio, cx| {
            studio.patch_status(
                |s| {
                    s.restore_delay_ms = value;
                },
                cx,
            );
            studio.state.clone()
        });
        let Some(state) = state else {
            return;
        };
        if let Err(err) = commands::set_restore_delay_ms(&state, value) {
            self.set_settings_error(err, cx);
            self.with_studio(cx, |studio, cx| studio.refetch_status(cx));
        }
        cx.notify();
    }

    fn change_insert_mode(&mut self, next: InsertMode, cx: &mut Context<Self>) {
        self.clear_settings_error(cx);
        let state = self.with_studio(cx, |studio, cx| {
            studio.patch_status(
                |s| {
                    s.insert_mode = next;
                },
                cx,
            );
            studio.state.clone()
        });
        let Some(state) = state else {
            return;
        };
        if let Err(err) = commands::set_insert_mode(&state, next) {
            self.set_settings_error(err, cx);
            self.with_studio(cx, |studio, cx| studio.refetch_status(cx));
        }
        cx.notify();
    }

    fn change_paste_combo(&mut self, next: PasteCombo, cx: &mut Context<Self>) {
        self.clear_settings_error(cx);
        let state = self.with_studio(cx, |studio, cx| {
            studio.patch_status(
                |s| {
                    s.paste_combo = next;
                },
                cx,
            );
            studio.state.clone()
        });
        let Some(state) = state else {
            return;
        };
        if let Err(err) = commands::set_paste_combo(&state, next) {
            self.set_settings_error(err, cx);
            self.with_studio(cx, |studio, cx| studio.refetch_status(cx));
        }
        cx.notify();
    }

    fn sync_edit_value(
        input: &Entity<InputState>,
        synced: &Cell<Option<u32>>,
        value: u32,
        window: &mut Window,
        cx: &mut App,
    ) {
        if synced.get() == Some(value) {
            return;
        }
        synced.set(Some(value));
        let text = value.to_string();
        if input.read(cx).value().as_ref() == text {
            return;
        }
        input.update(cx, |state, cx| {
            state.set_value(text, window, cx);
        });
    }

    fn repair_hook(&mut self, cx: &mut Context<Self>) {
        if self.repairing_hook {
            return;
        }
        self.repairing_hook = true;
        self.hook_message = "Reinstalling hook...".into();
        cx.notify();
        let studio = self.studio.clone();
        self.hook_msg_task = None;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { commands::reinstall_hook() })
                .await;
            this.update(cx, |this, cx| {
                this.repairing_hook = false;
                match result {
                    Ok(_) => {
                        this.hook_message = "Hook active & reconnected!".into();
                        if let Some(entity) = studio.upgrade() {
                            entity.update(cx, |studio, cx| studio.refetch_status(cx));
                        }
                    }
                    Err(_) => {
                        this.hook_message = "Failed to reinstall hook".into();
                    }
                }
                this.clear_hook_message_later(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn run_as_admin(&mut self, cx: &mut Context<Self>) {
        if self.elevating {
            return;
        }
        self.elevating = true;
        self.hook_message = "Requesting administrator privileges...".into();
        cx.notify();
        self.hook_msg_task = None;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { hypertype_core::platform::restart_elevated() })
                .await;
            this.update(cx, |this, cx| {
                // Success path exits the process inside restart_elevated.
                if let Err(e) = result {
                    this.hook_message = e;
                    this.elevating = false;
                    this.clear_hook_message_later(cx);
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// S5 parity arm: open Settings for `settings-modal`.
    pub fn parity_state(
        &mut self,
        name: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match name {
            "settings-modal" => {
                let studio = self.studio.clone();
                cx.defer(move |cx| {
                    if let Some(entity) = studio.upgrade() {
                        entity.update(cx, |studio, cx| {
                            // Do not mirror the real HKCU Run key in parity captures.
                            studio.autostart = false;
                            studio.set_modal(Some(Modal::Settings), cx);
                        });
                    }
                });
                true
            }
            _ => false,
        }
    }
}

impl Render for SettingsModal {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_poll(cx);

        let Some(studio_entity) = self.studio.upgrade() else {
            return Empty.into_any_element();
        };
        let (status, autostart, freeze, update_ui) = {
            let s = studio_entity.read(cx);
            (
                s.status.clone(),
                s.autostart,
                s.freeze_animations,
                s.update.clone(),
            )
        };

        let words_edit = self.words_edit.clone();
        let wpm_edit = self.wpm_edit.clone();
        let restore_edit = self.restore_edit.clone();

        let auto_words = self
            .auto_words_drag
            .get()
            .map(|v| v.round() as u32)
            .unwrap_or(status.auto_paste_words);
        let wpm = self
            .wpm_drag
            .get()
            .map(|v| v.round() as u32)
            .unwrap_or(status.wpm);
        let restore_ms = self
            .restore_drag
            .get()
            .map(|v| v.round() as u32)
            .unwrap_or(status.restore_delay_ms);

        Self::sync_edit_value(&words_edit, &self.synced[0], auto_words, window, cx);
        Self::sync_edit_value(&wpm_edit, &self.synced[1], wpm, window, cx);
        Self::sync_edit_value(&restore_edit, &self.synced[2], restore_ms, window, cx);

        let entity = cx.entity();
        let entity_id = entity.entity_id();
        let studio = self.studio.clone();
        let scroll = self.scroll.clone();
        let settings_error = self.settings_error.clone();
        let hook_message = self.hook_message.clone();
        let repairing_hook = self.repairing_hook;
        let elevating = self.elevating;

        let on_close = {
            let studio = studio.clone();
            let this = entity.clone();
            move |_window: &mut Window, cx: &mut App| {
                SettingsModal::close(&studio, &this, cx);
            }
        };

        // Body — styles.css:851-857. pr 35 = 20 + 15px classic scrollbar gutter.
        let body_rows = v_flex()
            .id("settings-modal-body")
            .relative()
            .flex_1()
            .min_h_0()
            .w_full()
            .pt(px(12.))
            .pb(px(12.))
            .pl(px(20.))
            .pr(px(35.))
            .gap(px(14.))
            .overflow_y_scroll()
            .track_scroll(&scroll)
            .when(!settings_error.is_empty(), |this| {
                this.child(
                    // .error-message — styles.css:741-748
                    h_flex()
                        .id("settings-error")
                        .items_center()
                        .gap(px(4.))
                        .ml(px(8.))
                        .text_color(tokens::danger())
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(icons::close())
                        .child(settings_error),
                )
            })
            .child(pref_row(
                ROW_HEIGHTS[0],
                true,
                v_flex()
                    .gap(px(2.))
                    .child(pref_title("Text Expansion Engine"))
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(tokens::ink_2())
                            .child("Global background keyboard hook listener"),
                    ),
                widgets::switch(
                    "settings-engine",
                    status.enabled,
                    freeze,
                    {
                        let studio = studio.clone();
                        move |_on, _window, cx| {
                            if let Some(entity) = studio.upgrade() {
                                entity.update(cx, |studio, cx| studio.toggle_enabled(cx));
                            }
                        }
                    },
                    window,
                    cx,
                ),
            ))
            .child(pref_row(
                ROW_HEIGHTS[1],
                true,
                v_flex()
                    .gap(px(2.))
                    .child(pref_title("Launch at Login"))
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(tokens::ink_2())
                            .child("Start HyperType minimized in tray on Windows boot"),
                    ),
                widgets::switch(
                    "settings-autostart",
                    autostart,
                    freeze,
                    {
                        let studio = studio.clone();
                        move |on, _window, cx| {
                            if let Some(entity) = studio.upgrade() {
                                entity.update(cx, |studio, cx| studio.set_autostart(on, cx));
                            }
                        }
                    },
                    window,
                    cx,
                ),
            ))
            .child(pref_row(
                ROW_HEIGHTS[2],
                true,
                v_flex()
                    .gap(px(2.))
                    .child(pref_title("Default Insertion Method"))
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(tokens::ink_2())
                            .child(insert_sub(status.insert_mode, auto_words)),
                    ),
                widgets::segmented(
                    "settings-insert-mode",
                    &INSERT_OPTIONS,
                    status.insert_mode,
                    SegKind::Three,
                    px(156.),
                    freeze,
                    {
                        let this = entity.downgrade();
                        move |mode, _window, cx| {
                            this.update(cx, |this, cx| this.change_insert_mode(mode, cx))
                                .ok();
                        }
                    },
                    window,
                    cx,
                ),
            ))
            .child(pref_row(
                ROW_HEIGHTS[3],
                true,
                v_flex()
                    .gap(px(2.))
                    .min_w_0()
                    .child(pref_title("Auto-Paste Word Threshold"))
                    .child(
                        h_flex()
                            .items_center()
                            .gap(px(4.))
                            .text_size(px(12.))
                            .text_color(tokens::ink_2())
                            .child(widgets::value_edit(&words_edit, false, window))
                            .child(format!(
                                "words (pastes if > {auto_words}, types if ≤ {auto_words})"
                            )),
                    ),
                widgets::slider(
                    "settings-auto-words",
                    auto_words as f32,
                    1.,
                    100.,
                    1.,
                    {
                        let drag = self.auto_words_drag.clone();
                        move |v, _window, cx| {
                            drag.set(Some(v));
                            cx.notify(entity_id);
                        }
                    },
                    {
                        let drag = self.auto_words_drag.clone();
                        let this = entity.downgrade();
                        move |v, _window, cx| {
                            drag.set(None);
                            this.update(cx, |this, cx| {
                                this.commit_auto_words(v.round() as u32, cx);
                            })
                            .ok();
                        }
                    },
                    window,
                    cx,
                ),
            ))
            .child(pref_row(
                ROW_HEIGHTS[4],
                true,
                v_flex()
                    .gap(px(2.))
                    .min_w_0()
                    .child(pref_title("Typing Replay Speed"))
                    .child(
                        h_flex()
                            .items_center()
                            .gap(px(4.))
                            .text_size(px(12.))
                            .text_color(tokens::ink_2())
                            .child(widgets::value_edit(&wpm_edit, false, window))
                            .child("words per minute"),
                    ),
                widgets::slider(
                    "settings-wpm",
                    wpm as f32,
                    100.,
                    1500.,
                    50.,
                    {
                        let drag = self.wpm_drag.clone();
                        move |v, _window, cx| {
                            drag.set(Some(v));
                            cx.notify(entity_id);
                        }
                    },
                    {
                        let drag = self.wpm_drag.clone();
                        let this = entity.downgrade();
                        move |v, _window, cx| {
                            drag.set(None);
                            this.update(cx, |this, cx| {
                                this.commit_wpm(v.round() as u32, cx);
                            })
                            .ok();
                        }
                    },
                    window,
                    cx,
                ),
            ))
            .child(pref_row(
                ROW_HEIGHTS[5],
                true,
                v_flex()
                    .gap(px(2.))
                    .child(pref_title("Paste Shortcut"))
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(tokens::ink_2())
                            .child("Sent when inserting via clipboard paste"),
                    ),
                widgets::segmented(
                    "settings-paste-combo",
                    &PASTE_OPTIONS,
                    status.paste_combo,
                    SegKind::Three,
                    px(196.),
                    freeze,
                    {
                        let this = entity.downgrade();
                        move |combo, _window, cx| {
                            this.update(cx, |this, cx| this.change_paste_combo(combo, cx))
                                .ok();
                        }
                    },
                    window,
                    cx,
                ),
            ))
            .child(pref_row(
                ROW_HEIGHTS[6],
                true,
                v_flex()
                    .gap(px(2.))
                    .min_w_0()
                    .child(pref_title("Clipboard Restore Delay"))
                    .child(
                        h_flex()
                            .items_center()
                            .gap(px(4.))
                            .text_size(px(12.))
                            .text_color(tokens::ink_2())
                            .child(widgets::value_edit(&restore_edit, true, window))
                            .child("ms until previous clipboard restores"),
                    ),
                widgets::slider(
                    "settings-restore",
                    restore_ms as f32,
                    3000.,
                    15000.,
                    100.,
                    {
                        let drag = self.restore_drag.clone();
                        move |v, _window, cx| {
                            drag.set(Some(v));
                            cx.notify(entity_id);
                        }
                    },
                    {
                        let drag = self.restore_drag.clone();
                        let this = entity.downgrade();
                        move |v, _window, cx| {
                            drag.set(None);
                            this.update(cx, |this, cx| {
                                this.commit_restore_ms(v.round() as u32, cx);
                            })
                            .ok();
                        }
                    },
                    window,
                    cx,
                ),
            ))
            .child(pref_row(
                ROW_HEIGHTS[7],
                true,
                v_flex()
                    .gap(px(2.))
                    .min_w_0()
                    .child(pref_title("Keyboard Capture & Hook Health"))
                    .child(
                        h_flex()
                            .items_center()
                            .gap(px(4.))
                            .flex_wrap()
                            .text_size(px(12.))
                            .text_color(tokens::ink_2())
                            .child(widgets::hook_badge(
                                status.hook_active && status.hook_healthy,
                                hook_label(status.hook_active, status.hook_healthy),
                            ))
                            .child(format!(
                                " · {} events captured",
                                format_thousands(status.hook_events)
                            ))
                            .when(!hook_message.is_empty(), |this| {
                                this.child(
                                    // .hook-msg — styles.css:1188-1191
                                    div()
                                        .text_color(tokens::accent_ink())
                                        .font_weight(FontWeight::MEDIUM)
                                        .child(format!(" — {hook_message}")),
                                )
                            }),
                    ),
                widgets::btn_surface(
                    "settings-repair-hook",
                    if repairing_hook {
                        "Repairing..."
                    } else {
                        "Repair Hook"
                    },
                    !repairing_hook,
                    {
                        let this = entity.downgrade();
                        move |_window, cx| {
                            this.update(cx, |this, cx| this.repair_hook(cx)).ok();
                        }
                    },
                ),
            ))
            .child(pref_row(
                ROW_HEIGHTS[8],
                false, // last row: no border
                v_flex()
                    .gap(px(2.))
                    .min_w_0()
                    .child(pref_title("Process Privilege (UIPI Status)"))
                    .child(
                        h_flex()
                            .items_center()
                            .gap(px(4.))
                            .flex_wrap()
                            .text_size(px(12.))
                            .text_color(tokens::ink_2())
                            .child(widgets::hook_badge(
                                status.is_elevated,
                                if status.is_elevated {
                                    "Administrator (Elevated)"
                                } else {
                                    "Standard User"
                                },
                            ))
                            .child(if status.is_elevated {
                                " · Can inject into elevated terminals (WezTerm, CMD, PowerShell)"
                            } else {
                                " · Cannot type into administrator terminals without elevation"
                            }),
                    ),
                if status.is_elevated {
                    div().into_any_element()
                } else {
                    widgets::btn_surface(
                        "settings-run-admin",
                        if elevating {
                            "Elevating..."
                        } else {
                            "Run as Admin"
                        },
                        !elevating,
                        {
                            let this = entity.downgrade();
                            move |_window, cx| {
                                this.update(cx, |this, cx| this.run_as_admin(cx)).ok();
                            }
                        },
                    )
                    .into_any_element()
                },
            ))
            .child(
                div().absolute().inset_0().child(
                    Scrollbar::vertical(&scroll)
                        .id("settings-modal-scrollbar")
                        .mode(ScrollbarMode::Always)
                        .viewport_from_layout(),
                ),
            );

        let version_label = if status.version.is_empty() {
            "HyperType".to_string()
        } else {
            format!("HyperType v{}", status.version)
        };

        let check_label = if !update_ui.status_text.is_empty() {
            update_ui.status_text.clone()
        } else if update_ui.checking {
            "Checking...".into()
        } else {
            "Check for updates".into()
        };

        // Footer — styles.css:947-955
        let footer = h_flex()
            .id("settings-modal-footer")
            .flex_none()
            .w_full()
            .px(px(20.))
            .py(px(14.))
            .bg(tokens::surface_2())
            .border_t_1()
            .border_color(tokens::line())
            .rounded_b(px(12.))
            .justify_between()
            .items_center()
            .child(
                // .footer-version-group — styles.css:1098-1102
                h_flex()
                    .id("footer-version-group")
                    .items_center()
                    .gap(px(10.))
                    .child(
                        // .app-version-tag — styles.css:956-960
                        div()
                            .id("app-version-tag")
                            .text_size(px(11.))
                            .text_color(tokens::ink_3())
                            .font_family(tokens::FONT_MONO)
                            .child(version_label),
                    )
                    .when(update_ui.info.is_none(), |this| {
                        this.child(widgets::btn_check_updates(
                            "settings-check-updates",
                            check_label,
                            !update_ui.checking,
                            update_ui.checking,
                            freeze,
                            {
                                let studio = studio.clone();
                                move |_window, cx| {
                                    if let Some(entity) = studio.upgrade() {
                                        update::check(&entity, true, cx);
                                        entity.update(cx, |_, cx| cx.notify());
                                    }
                                }
                            },
                        ))
                    }),
            )
            .when_some(update_ui.info.clone(), |this, info| {
                this.child(
                    // .btn-install-update — styles.css:1128-1148
                    h_flex()
                        .id("settings-install-update")
                        .items_center()
                        .gap(px(6.))
                        .bg(tokens::accent())
                        .text_color(tokens::white())
                        .text_size(px(11.5))
                        .font_weight(FontWeight::SEMIBOLD)
                        .px(px(12.))
                        .py(px(5.))
                        .rounded(px(6.))
                        .when(!update_ui.downloading, |this| {
                            this.cursor(CursorStyle::PointingHand)
                                .tab_index(0)
                                .focus_visible(|s| {
                                    s.shadow(tokens::focus_visible_shadows(tokens::accent()))
                                })
                                .hover(|s| s.bg(tokens::accent_hover()))
                                .on_click({
                                    let studio = studio.clone();
                                    move |_ev, _window, cx| {
                                        if let Some(entity) = studio.upgrade() {
                                            update::install(&entity, cx);
                                            entity.update(cx, |_, cx| cx.notify());
                                        }
                                    }
                                })
                        })
                        .when(update_ui.downloading, |this| {
                            this.opacity(0.7).cursor(CursorStyle::Arrow)
                        })
                        .child(icons::download())
                        .child(if update_ui.downloading {
                            format!("Downloading ({}%)", update_ui.progress)
                        } else {
                            format!("Update to v{}", info.version)
                        }),
                )
            })
            .child(
                // .btn-quit-app — styles.css:961-973
                div()
                    .id("settings-quit-app")
                    .px(px(10.))
                    .py(px(4.))
                    .rounded(px(5.))
                    .border_1()
                    .border_color(tokens::danger_tint())
                    .bg(gpui_kit::hsla(0., 0., 0., 0.))
                    .text_color(tokens::danger())
                    .text_size(px(11.5))
                    .font_weight(FontWeight::SEMIBOLD)
                    .cursor(CursorStyle::PointingHand)
                    .tab_index(0)
                    .focus_visible(|s| s.shadow(tokens::focus_visible_shadows(tokens::surface_2())))
                    .hover(|s| s.bg(tokens::danger_tint()))
                    .on_click(|_ev, _window, cx| {
                        cx.quit();
                    })
                    .child("Quit HyperType"),
            );

        let frame_body = v_flex()
            .id("settings-modal-frame-body")
            .flex_1()
            .min_h_0()
            .w_full()
            .child(body_rows)
            .child(footer);

        widgets::modal_frame(
            "settings-modal",
            "Preferences & Engine Settings",
            freeze,
            on_close,
            frame_body,
            window,
            cx,
        )
        .into_any_element()
    }
}

fn pref_title(text: impl Into<SharedString>) -> impl IntoElement {
    // .pref-title — styles.css:874-877
    div()
        .font_weight(FontWeight::SEMIBOLD)
        .text_size(px(13.))
        .text_color(tokens::ink())
        .child(text.into())
}

/// Border-box heights of the nine `.pref-row`s on the reference pixel grid
/// (rule 23a). Box tops 115, 185.5, 256, 344.5, 416.8, 489.09, 559.59,
/// 631.89, 705.02 round half up to 115, 186, 256, 345, 417, 489, 560, 632,
/// 705 (end 781); minus the 14px body gap. Rows with an inline `.value-edit`
/// or hook badge are taller than 56.5 because the inline box grows the
/// description line (19.8 / 20.63px).
const ROW_HEIGHTS: [f32; 9] = [57., 56., 75., 58., 58., 57., 58., 59., 76.];

fn pref_row(
    height: f32,
    bordered: bool,
    info: impl IntoElement,
    control: impl IntoElement,
) -> impl IntoElement {
    // .pref-row — styles.css:859-867
    h_flex()
        .w_full()
        .items_center()
        .justify_between()
        .gap(px(16.))
        // CSS flex items keep min-height:auto and never shrink below their
        // content; Taffy shrinks them to fit the scroll body unless told not to.
        .flex_shrink_0()
        .h(px(height))
        .py(px(8.))
        .when(bordered, |this| {
            this.border_b_1().border_color(tokens::sep())
        })
        .child(
            // .pref-info — styles.css:869-873
            div().flex_1().min_w_0().child(info),
        )
        .child(control)
}

