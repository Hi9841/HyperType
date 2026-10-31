//! Root `Studio` entity: shared state + frozen API (RULEBOOK section 4).

use std::sync::Arc;

use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use hypertype_core::app_state::AppState;
use hypertype_core::commands::{self, SnippetView, Status};
use hypertype_core::snippets::TriggerKind;

use crate::autostart;
use crate::shell::{self, Shell};
use crate::ui::editor::Editor;
use crate::ui::settings_modal::SettingsModal;
use crate::ui::sidebar::Sidebar;
use crate::ui::titlebar;
use crate::ui::tokens;
use crate::ui::tokens_modal::TokensModal;
use crate::ui::update::{self, UpdateUi};

actions!(studio_focus, [FocusNext, FocusPrev]);

const STUDIO_FOCUS_CONTEXT: &str = "Studio";

/// Rule 21a: bind Studio-root Tab / Shift-Tab. Kit `Root` also binds Tab in
/// context `"Root"`, but an unfocused window dispatches from the tree root
/// (no context). Focusing the Studio handle + `"Studio"` context makes the
/// first Tab call `focus_next` → first DOM tab stop (title bar).
pub fn init_focus(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("tab", FocusNext, Some(STUDIO_FOCUS_CONTEXT)),
        KeyBinding::new("shift-tab", FocusPrev, Some(STUDIO_FOCUS_CONTEXT)),
    ]);
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Modal {
    Settings,
    Tokens,
}

pub struct Studio {
    pub state: Arc<AppState>,
    pub status: Status,
    pub snippets: Vec<SnippetView>,
    #[allow(dead_code)] // S5 Settings reads/writes this
    pub autostart: bool,
    pub selected: Option<String>,
    pub creating_new: bool,
    pub modal: Option<Modal>,
    pub update: UpdateUi,
    pub sidebar: Entity<Sidebar>,
    pub editor: Entity<Editor>,
    pub settings: Entity<SettingsModal>,
    pub tokens: Entity<TokensModal>,
    /// Parity: freeze infinite animations at t=0.
    pub freeze_animations: bool,
    /// Parity-only overlay for slider / value_edit proof (`widget-lab`).
    #[cfg(feature = "parity")]
    pub widget_lab: Option<Entity<crate::ui::widget_lab::WidgetLab>>,
    /// Rule 21a: focused on open (not a tab stop) so Kit `Root`'s Tab binding
    /// dispatches when nothing else is focused; first Tab → first DOM stop.
    root_focus: FocusHandle,
    did_initial_select: bool,
    _subscriptions: Vec<Subscription>,
}

impl Studio {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let state = cx.global::<Shell>().state.clone();
        let freeze_animations = cfg!(feature = "parity");

        let mut status = commands::status(&state);
        if freeze_animations {
            status = parity_mock_status(status);
        }

        let snippets = commands::snippets(&state);
        let autostart = autostart::is_enabled();

        let weak = cx.weak_entity();
        let sidebar = cx.new(|cx| Sidebar::new(weak.clone(), cx));
        let editor = cx.new(|cx| Editor::new(weak.clone(), cx));
        let settings = cx.new(|cx| SettingsModal::new(weak.clone(), cx));
        let tokens = cx.new(|cx| TokensModal::new(weak.clone(), cx));

        let _subscriptions = vec![
            // Tray toggle: copy ONLY status.enabled (App.tsx:191-194).
            cx.observe_global::<Shell>(|this, cx| {
                let enabled = cx.global::<Shell>().enabled();
                if this.status.enabled != enabled {
                    this.status.enabled = enabled;
                    cx.notify();
                }
            }),
        ];

        // Not a tab stop: holds focus so Root's Tab key context is active (A7/21a).
        let root_focus = cx.focus_handle().tab_stop(false);
        root_focus.focus(window, cx);

        let mut this = Self {
            state,
            status,
            snippets,
            autostart,
            selected: None,
            creating_new: false,
            modal: None,
            update: UpdateUi::default(),
            sidebar,
            editor,
            settings,
            tokens,
            freeze_animations,
            #[cfg(feature = "parity")]
            widget_lab: None,
            root_focus,
            did_initial_select: false,
            _subscriptions,
        };

        // Initial selection once when the first non-empty list arrives (248-257).
        this.maybe_initial_select(window, cx);

        // Silent update check at window open (App.tsx:205).
        let studio_entity = cx.entity();
        update::check(&studio_entity, false, cx);

        this
    }

    fn maybe_initial_select(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.did_initial_select || self.snippets.is_empty() {
            return;
        }
        if self.selected.is_some() || self.creating_new {
            return;
        }
        self.did_initial_select = true;
        let first = self.snippets[0].clone();
        self.select_snippet(&first, window, cx);
    }

    pub fn refetch_status(&mut self, cx: &mut Context<Self>) {
        let mut status = commands::status(&self.state);
        if self.freeze_animations {
            status = parity_mock_status(status);
        }
        self.status = status;
        cx.notify();
    }

    #[allow(dead_code)] // S2/S4 call after mutations; also drives initial select
    pub fn refetch_snippets(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.snippets = commands::snippets(&self.state);
        // Initial selection once when the first non-empty list arrives (App.tsx:248-257).
        self.maybe_initial_select(window, cx);
        cx.notify();
    }

    /// Optimistic status edit (Settings).
    #[allow(dead_code)] // S5 Settings patches
    pub fn patch_status(&mut self, f: impl FnOnce(&mut Status), cx: &mut Context<Self>) {
        f(&mut self.status);
        cx.notify();
    }

    /// App.tsx:225-234
    pub fn select_snippet(
        &mut self,
        snippet: &SnippetView,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selected = Some(snippet.trigger.clone());
        self.creating_new = false;
        self.editor.update(cx, |editor, cx| {
            editor.load(snippet, window, cx);
        });
        cx.notify();
    }

    /// App.tsx:236-246
    #[allow(dead_code)] // S2 new-snippet; also delete fallback
    pub fn start_new_snippet(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.creating_new = true;
        self.selected = None;
        self.editor.update(cx, |editor, cx| {
            editor.clear_for_new(window, cx);
        });
        cx.notify();
    }

    /// App.tsx:454-480 — Editor owns busy and messages. Trigger is trimmed (455).
    #[allow(dead_code)] // S4 save path
    pub fn save_snippet(
        &mut self,
        old: Option<&str>,
        trigger: &str,
        expansion: String,
        kind: TriggerKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let trigger = trigger.trim();
        let shell = cx.global::<Shell>();
        match old {
            None => commands::add_snippet(&shell.state, &shell.hotkeys, trigger, expansion, kind)?,
            Some(old_trigger) => commands::edit_snippet(
                &shell.state,
                &shell.hotkeys,
                old_trigger,
                trigger,
                expansion,
                kind,
            )?,
        }
        self.selected = Some(trigger.to_string());
        self.creating_new = false;
        self.refetch_snippets(window, cx);
        self.refetch_status(cx);
        Ok(())
    }

    /// App.tsx:482-499 — async confirm; OK deletes.
    #[allow(dead_code)] // S4 delete path
    pub fn delete_snippet(
        &mut self,
        trigger: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let message = format!("Delete snippet \"{trigger}\"?");
        let receiver = window.prompt(
            PromptLevel::Warning,
            &message,
            None,
            &["OK", "Cancel"],
            cx,
        );
        let this = cx.entity().downgrade();
        cx.spawn_in(window, async move |_, cx| {
            let Ok(answer) = receiver.await else {
                return;
            };
            if answer != 0 {
                return;
            }
            this.update_in(cx, |this, window, cx| {
                let shell = cx.global::<Shell>();
                if let Err(e) = commands::remove_snippet(&shell.state, &shell.hotkeys, &trigger) {
                    this.editor.update(cx, |editor, cx| {
                        editor.set_error(e, cx);
                    });
                    return;
                }
                this.refetch_snippets(window, cx);
                this.refetch_status(cx);
                if this.selected.as_deref() == Some(trigger.as_str()) {
                    match this.snippets.first().cloned() {
                        Some(first) => this.select_snippet(&first, window, cx),
                        None => this.start_new_snippet(window, cx),
                    }
                }
            })
            .ok();
        })
        .detach();
    }

    /// App.tsx:501-513 — optimistic reorder.
    #[allow(dead_code)] // S2 row reorder
    pub fn apply_reorder(&mut self, from: usize, to: usize, window: &mut Window, cx: &mut Context<Self>) {
        if from == to || from >= self.snippets.len() || to >= self.snippets.len() {
            return;
        }
        let moved = self.snippets.remove(from);
        self.snippets.insert(to, moved);
        let order: Vec<String> = self.snippets.iter().map(|s| s.trigger.clone()).collect();
        cx.notify();
        if commands::reorder_snippets(&self.state, order).is_err() {
            self.refetch_snippets(window, cx);
        }
    }

    /// App.tsx:275-285
    pub fn toggle_enabled(&mut self, cx: &mut Context<Self>) {
        shell::toggle_enabled(cx);
        self.refetch_status(cx);
    }

    /// App.tsx:381-389
    #[allow(dead_code)] // S5 Settings autostart
    pub fn set_autostart(&mut self, on: bool, cx: &mut Context<Self>) {
        let previous = self.autostart;
        self.autostart = on;
        cx.notify();
        if let Err(e) = autostart::set_enabled(on) {
            hypertype_core::logging::error(&format!("autostart: {e}"));
            self.autostart = previous;
            cx.notify();
        }
    }

    pub fn set_modal(&mut self, modal: Option<Modal>, cx: &mut Context<Self>) {
        self.modal = modal;
        cx.notify();
    }

    /// App.tsx:437-452
    #[allow(dead_code)] // S6 token insert
    pub fn insert_token(&mut self, token: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.editor.update(cx, |editor, cx| {
            editor.insert_token(token, window, cx);
        });
    }
}

fn parity_mock_status(mut status: Status) -> Status {
    status.version = "1.1.5".into();
    status.hook_events = 42;
    status.hook_active = true;
    status.hook_healthy = true;
    status.is_elevated = false;
    status
}

impl Studio {
    fn on_focus_next(&mut self, _: &FocusNext, window: &mut Window, cx: &mut Context<Self>) {
        window.focus_next(cx);
        // Prefer Studio's binding over Kit Root's Tab when both contexts match.
        cx.stop_propagation();
    }

    fn on_focus_prev(&mut self, _: &FocusPrev, window: &mut Window, cx: &mut Context<Self>) {
        window.focus_prev(cx);
        cx.stop_propagation();
    }
}

impl Render for Studio {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // .studio-app — styles.css:73-79
        // Rule 21a: zero-size focus anchor (not the full-size root). A full-size
        // track_focus steals every click via the auto mouse-down focus listener.
        // Key context + Tab actions live on the visible root; the anchor holds
        // initial focus so Tab dispatches without being a tab stop.
        v_flex()
            .id("studio-root")
            .key_context(STUDIO_FOCUS_CONTEXT)
            .on_action(cx.listener(Self::on_focus_next))
            .on_action(cx.listener(Self::on_focus_prev))
            .size_full()
            .bg(tokens::bg())
            .text_color(tokens::ink())
            .font_family(tokens::FONT_UI)
            .text_size(px(tokens::ROOT_SIZE))
            .line_height(relative(tokens::ROOT_LINE_HEIGHT))
            .font_weight(FontWeight::NORMAL)
            .child(
                div()
                    .id("studio-focus-anchor")
                    .absolute()
                    .size_0()
                    .track_focus(&self.root_focus)
                    .tab_index(0)
                    .tab_stop(false),
            )
            .child(titlebar::render(self, window, cx))
            // .studio-workspace — styles.css:205-210
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .child(self.sidebar.clone())
                    .child(self.editor.clone()),
            )
            .when(self.modal == Some(Modal::Settings), |this| {
                this.child(self.settings.clone())
            })
            .when(self.modal == Some(Modal::Tokens), |this| {
                this.child(self.tokens.clone())
            })
            .map(|this| {
                #[cfg(feature = "parity")]
                {
                    if let Some(lab) = self.widget_lab.clone() {
                        return this.child(
                            div()
                                .absolute()
                                .inset_0()
                                .bg(tokens::bg())
                                .child(lab),
                        );
                    }
                }
                this
            })
    }
}
