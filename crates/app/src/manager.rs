//! The manager window: title bar, snippet library, and editor. The library
//! lives here because selection, reorder, and deletion all change the list.

use std::path::PathBuf;

use gpui_kit::assets::IconName as Lucide;
use gpui_kit::component::button::{Button, ButtonVariant, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{ContextMenuExt as _, DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, TitleBar, WindowExt as _, h_flex, v_flex,
};
use gpui_kit::component::StyledExt as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use hypertype_core::commands::{self, SnippetView};
use hypertype_core::snippets::TriggerKind;

use crate::editor::{Editor, EditorEvent};
use crate::settings::SettingsPanel;
use crate::shell::{self, Shell};
use crate::widgets::trigger_badge;
use crate::{assets, chord};

actions!(
    hypertype,
    [
        NewSnippet,
        FocusSearch,
        SaveSnippet,
        OpenSettings,
        SelectPrevious,
        SelectNext,
        MoveUp,
        MoveDown,
        DeleteSelected,
    ]
);

const CONTEXT: &str = "HyperType";
const LIST_CONTEXT: &str = "SnippetList";
const ROW_HEIGHT: f32 = 56.;
const EXPORT_NAME: &str = "hypertype-snippets.json";

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("ctrl-n", NewSnippet, Some(CONTEXT)),
        KeyBinding::new("ctrl-f", FocusSearch, Some(CONTEXT)),
        KeyBinding::new("ctrl-s", SaveSnippet, Some(CONTEXT)),
        KeyBinding::new("ctrl-,", OpenSettings, Some(CONTEXT)),
        KeyBinding::new("up", SelectPrevious, Some(LIST_CONTEXT)),
        KeyBinding::new("down", SelectNext, Some(LIST_CONTEXT)),
        KeyBinding::new("alt-up", MoveUp, Some(LIST_CONTEXT)),
        KeyBinding::new("alt-down", MoveDown, Some(LIST_CONTEXT)),
        KeyBinding::new("delete", DeleteSelected, Some(LIST_CONTEXT)),
    ]);
}

/// The payload of a row drag.
#[derive(Clone)]
struct DraggedSnippet {
    from: usize,
    trigger: String,
    kind: TriggerKind,
}

struct DragPreview(DraggedSnippet);

impl Render for DragPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        h_flex()
            .px_2()
            .py_1p5()
            .rounded(theme.radius)
            .border_1()
            .border_color(theme.border)
            .bg(theme.popover)
            .child(trigger_badge(&self.0.trigger, self.0.kind, cx))
    }
}

pub struct Manager {
    snippets: Vec<SnippetView>,
    /// Trigger of the snippet in the editor; `None` while creating one.
    selected: Option<String>,
    search: Entity<InputState>,
    editor: Entity<Editor>,
    list_focus: FocusHandle,
    scroll: UniformListScrollHandle,
    _subscriptions: Vec<Subscription>,
}

impl Manager {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Search triggers and text")
                .clean_on_escape()
        });
        let editor = cx.new(|cx| Editor::new(window, cx));
        let _subscriptions = vec![
            cx.observe_global::<Shell>(|_, cx| cx.notify()),
            crate::theme::follow_system(window),
            cx.subscribe(&search, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            }),
            cx.subscribe_in(&editor, window, |this, _, event, window, cx| match event {
                EditorEvent::Saved(trigger) => {
                    this.reload(cx);
                    this.selected = Some(trigger.clone());
                    this.reveal_selected(cx);
                    cx.notify();
                }
                EditorEvent::DeleteRequested(trigger) => this.confirm_delete(trigger.clone(), window, cx),
            }),
        ];

        let list_focus = cx.focus_handle().tab_stop(true);
        list_focus.focus(window, cx);
        let mut this = Self {
            snippets: commands::snippets(&cx.global::<Shell>().state),
            selected: None,
            search,
            editor,
            list_focus,
            scroll: UniformListScrollHandle::new(),
            _subscriptions,
        };
        match this.snippets.first().cloned() {
            Some(first) => this.select(&first, window, cx),
            None => this.start_new(window, cx),
        }
        this
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        self.snippets = commands::snippets(&cx.global::<Shell>().state);
        if let Some(selected) = &self.selected
            && !self.snippets.iter().any(|s| &s.trigger == selected)
        {
            self.selected = None;
        }
    }

    fn query(&self, cx: &App) -> String {
        self.search.read(cx).value().trim().to_lowercase()
    }

    /// Indices into `snippets` that match the search, in library order.
    fn visible(&self, cx: &App) -> Vec<usize> {
        let query = self.query(cx);
        self.snippets
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                query.is_empty()
                    || s.trigger.to_lowercase().contains(&query)
                    || s.expansion.to_lowercase().contains(&query)
            })
            .map(|(ix, _)| ix)
            .collect()
    }

    fn selected_index(&self) -> Option<usize> {
        let selected = self.selected.as_ref()?;
        self.snippets.iter().position(|s| &s.trigger == selected)
    }

    fn select(&mut self, snippet: &SnippetView, window: &mut Window, cx: &mut Context<Self>) {
        self.selected = Some(snippet.trigger.clone());
        self.editor
            .update(cx, |editor, cx| editor.load(Some(snippet), window, cx));
        cx.notify();
    }

    fn start_new(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.selected = None;
        self.editor.update(cx, |editor, cx| {
            editor.load(None, window, cx);
            editor.focus_trigger(window, cx);
        });
        cx.notify();
    }

    fn reveal_selected(&self, cx: &App) {
        let Some(ix) = self.selected_index() else {
            return;
        };
        if let Some(pos) = self.visible(cx).iter().position(|v| *v == ix) {
            self.scroll.scroll_to_item(pos, ScrollStrategy::Nearest);
        }
    }

    fn reorder_locked(&self, cx: &App) -> bool {
        !self.query(cx).is_empty()
    }

    fn move_snippet(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        if from == to || from >= self.snippets.len() || to >= self.snippets.len() {
            return;
        }
        let moved = self.snippets.remove(from);
        self.snippets.insert(to, moved);
        let order = self.snippets.iter().map(|s| s.trigger.clone()).collect();
        if commands::reorder_snippets(&cx.global::<Shell>().state, order).is_err() {
            self.reload(cx);
        }
        self.reveal_selected(cx);
        cx.notify();
    }

    fn confirm_delete(&mut self, trigger: String, window: &mut Window, cx: &mut Context<Self>) {
        let label = match self.snippets.iter().find(|s| s.trigger == trigger) {
            Some(s) if s.kind == TriggerKind::Shortcut => chord::display(&s.trigger),
            _ => trigger.clone(),
        };
        let this = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let this = this.clone();
            let trigger = trigger.clone();
            alert
                .title(format!("Delete \"{label}\"?"))
                .description("The snippet is removed from your library. You cannot undo this.")
                .show_cancel(true)
                .ok_text("Delete")
                .ok_variant(ButtonVariant::Danger)
                .on_ok(move |_, window, cx| {
                    this.update(cx, |this, cx| this.delete(&trigger, window, cx)).ok();
                    true
                })
        });
    }

    fn delete(&mut self, trigger: &str, window: &mut Window, cx: &mut Context<Self>) {
        let shell = cx.global::<Shell>();
        if let Err(e) = commands::remove_snippet(&shell.state, &shell.hotkeys, trigger) {
            self.editor.update(cx, |editor, cx| editor.show_error(e, cx));
            return;
        }
        let was_selected = self.selected.as_deref() == Some(trigger);
        self.reload(cx);
        if was_selected {
            match self.snippets.first().cloned() {
                Some(first) => self.select(&first, window, cx),
                None => self.start_new(window, cx),
            }
        }
        self.list_focus.focus(window, cx);
        cx.notify();
    }

    fn import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Import".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            this.update_in(cx, |this, window, cx| {
                let shell = cx.global::<Shell>();
                let note = match commands::import_snippets(&shell.state, &shell.hotkeys, &path) {
                    Ok(summary) => {
                        let mut parts = vec![format!("Imported {}", summary.imported)];
                        if summary.overwritten > 0 {
                            parts.push(format!("replaced {}", summary.overwritten));
                        }
                        if summary.skipped > 0 {
                            parts.push(format!("skipped {}", summary.skipped));
                        }
                        Notification::success(parts.join(", "))
                    }
                    Err(e) => Notification::error(e),
                };
                window.push_notification(note, cx);
                this.reload(cx);
                if let Some(ix) = this.selected_index() {
                    let snippet = this.snippets[ix].clone();
                    this.select(&snippet, window, cx);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn export(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let dir = std::env::var_os("USERPROFILE")
            .map(|home| PathBuf::from(home).join("Documents"))
            .filter(|dir| dir.is_dir())
            .unwrap_or_default();
        let path = cx.prompt_for_new_path(&dir, Some(EXPORT_NAME));
        let count = self.snippets.len();
        cx.spawn_in(window, async move |_, cx| {
            let Ok(Ok(Some(path))) = path.await else {
                return;
            };
            cx.update(|window, cx| {
                let note = match commands::export_snippets(&cx.global::<Shell>().state, &path) {
                    Ok(()) => Notification::success(format!(
                        "Exported {count} {}",
                        if count == 1 { "snippet" } else { "snippets" }
                    )),
                    Err(e) => Notification::error(e),
                };
                window.push_notification(note, cx);
            })
            .ok();
        })
        .detach();
    }

    fn open_settings(&mut self, _: &OpenSettings, window: &mut Window, cx: &mut Context<Self>) {
        if window.has_active_dialog(cx) {
            return;
        }
        let panel = cx.new(|cx| SettingsPanel::new(window, cx));
        window.open_dialog(cx, move |dialog, window, _| {
            dialog.title("Settings").w(px(620.)).child(
                div()
                    .id("settings-scroll")
                    .max_h(window.viewport_size().height * 0.72)
                    .overflow_y_scroll()
                    .pr_1()
                    .child(panel.clone()),
            )
        });
    }

    fn on_new(&mut self, _: &NewSnippet, window: &mut Window, cx: &mut Context<Self>) {
        self.start_new(window, cx);
    }

    fn on_focus_search(&mut self, _: &FocusSearch, window: &mut Window, cx: &mut Context<Self>) {
        self.search.update(cx, |input, cx| {
            input.focus(window, cx);
            input.select_all(window, cx);
        });
    }

    fn on_save(&mut self, _: &SaveSnippet, window: &mut Window, cx: &mut Context<Self>) {
        self.editor.update(cx, |editor, cx| editor.save(window, cx));
    }

    fn step_selection(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        let visible = self.visible(cx);
        if visible.is_empty() {
            return;
        }
        let current = self
            .selected_index()
            .and_then(|ix| visible.iter().position(|v| *v == ix));
        let next = match current {
            Some(pos) => (pos as isize + delta).clamp(0, visible.len() as isize - 1) as usize,
            None => 0,
        };
        let snippet = self.snippets[visible[next]].clone();
        self.select(&snippet, window, cx);
        self.scroll.scroll_to_item(next, ScrollStrategy::Nearest);
    }

    fn step_order(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.reorder_locked(cx) {
            return;
        }
        if let Some(from) = self.selected_index() {
            let to = from as isize + delta;
            if to >= 0 {
                self.move_snippet(from, to as usize, cx);
            }
        }
    }

    fn render_title_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let enabled = cx.global::<Shell>().enabled();
        let theme = cx.theme();
        let dot = div()
            .size(px(6.))
            .rounded_full()
            .bg(if enabled { theme.success } else { theme.muted_foreground });
        TitleBar::new().child(
            h_flex()
                .flex_1()
                .justify_between()
                .pr_2()
                .child(
                    h_flex()
                        .gap_2()
                        .child(img(assets::logo()).size_4())
                        .child(div().text_sm().font_medium().child("HyperType"))
                        .child(
                            Button::new("engine")
                                .ghost()
                                .xsmall()
                                .child(
                                    h_flex()
                                        .gap_1p5()
                                        .child(dot)
                                        .child(if enabled { "Active" } else { "Paused" }),
                                )
                                .tooltip(if enabled { "Pause expansion" } else { "Resume expansion" })
                                .on_click(|_, _, cx| {
                                    shell::toggle_enabled(cx);
                                }),
                        ),
                )
                .child(
                    Button::new("settings")
                        .ghost()
                        .xsmall()
                        .icon(IconName::Settings)
                        .tooltip("Settings (Ctrl+,)")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.open_settings(&OpenSettings, window, cx)
                        })),
                ),
        )
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let visible = self.visible(cx);
        let locked = self.reorder_locked(cx);
        let query = self.query(cx);
        let total = self.snippets.len();
        let theme = cx.theme();

        let toolbar = h_flex()
            .gap_2()
            .child(
                Button::new("new-snippet")
                    .primary()
                    .small()
                    .flex_1()
                    .icon(IconName::Plus)
                    .label("New snippet")
                    .tooltip("Ctrl+N")
                    .on_click(cx.listener(|this, _, window, cx| this.start_new(window, cx))),
            )
            .child({
                let this = cx.entity().downgrade();
                Button::new("library-menu")
                    .outline()
                    .small()
                    .icon(IconName::Ellipsis)
                    .tooltip("Import and export")
                    .dropdown_menu(move |menu, _, _| {
                        let import = this.clone();
                        let export = this.clone();
                        menu.item(PopupMenuItem::new("Import...").icon(Lucide::Upload).on_click(
                            move |_, window, cx| {
                                import.update(cx, |this, cx| this.import(window, cx)).ok();
                            },
                        ))
                        .item(PopupMenuItem::new("Export...").icon(Lucide::Download).on_click(
                            move |_, window, cx| {
                                export.update(cx, |this, cx| this.export(window, cx)).ok();
                            },
                        ))
                    })
            });

        let header = h_flex()
            .justify_between()
            .px_4()
            .pt_1()
            .pb_2()
            .text_xs()
            .text_color(theme.muted_foreground)
            .child(div().font_medium().child("Snippets"))
            .child(if query.is_empty() {
                total.to_string()
            } else {
                format!("{} of {total}", visible.len())
            });

        let body: AnyElement = if visible.is_empty() {
            let (title, detail): (SharedString, SharedString) = if total == 0 {
                ("No snippets yet".into(), "Create one to start expanding text.".into())
            } else {
                ("No matches".into(), format!("Nothing matches \"{query}\".").into())
            };
            v_flex()
                .flex_1()
                .items_center()
                .justify_center()
                .gap_1()
                .px_6()
                .text_center()
                .child(div().text_sm().font_medium().child(title))
                .child(div().text_xs().text_color(theme.muted_foreground).child(detail))
                .into_any_element()
        } else {
            let selected = self.selected_index();
            uniform_list(
                "snippets",
                visible.len(),
                cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                    range
                        .map(|pos| this.render_row(visible[pos], selected == Some(visible[pos]), locked, cx))
                        .collect::<Vec<_>>()
                }),
            )
            .track_scroll(&self.scroll)
            .flex_1()
            .pb_2()
            .into_any_element()
        };

        v_flex()
            .w(px(300.))
            .flex_none()
            .h_full()
            .bg(theme.sidebar)
            .border_r_1()
            .border_color(theme.sidebar_border)
            .child(
                v_flex()
                    .p_3()
                    .gap_2()
                    .child(
                        Input::new(&self.search)
                            .small()
                            .cleanable(true)
                            .prefix(Icon::new(IconName::Search).small()),
                    )
                    .child(toolbar),
            )
            .child(header)
            .child(
                v_flex()
                    .id("snippet-list")
                    .key_context(LIST_CONTEXT)
                    .track_focus(&self.list_focus)
                    .flex_1()
                    .min_h_0()
                    .on_action(cx.listener(|this, _: &SelectPrevious, window, cx| {
                        this.step_selection(-1, window, cx)
                    }))
                    .on_action(cx.listener(|this, _: &SelectNext, window, cx| {
                        this.step_selection(1, window, cx)
                    }))
                    .on_action(cx.listener(|this, _: &MoveUp, _, cx| this.step_order(-1, cx)))
                    .on_action(cx.listener(|this, _: &MoveDown, _, cx| this.step_order(1, cx)))
                    .on_action(cx.listener(|this, _: &DeleteSelected, window, cx| {
                        if let Some(trigger) = this.selected.clone() {
                            this.confirm_delete(trigger, window, cx);
                        }
                    }))
                    .child(body),
            )
    }

    fn render_row(&self, ix: usize, selected: bool, locked: bool, cx: &mut Context<Self>) -> AnyElement {
        let snippet = &self.snippets[ix];
        let lines = snippet.expansion.lines().count();
        let stats = if lines > 1 {
            format!("{lines} lines")
        } else {
            format!("{} chars", snippet.expansion.chars().count())
        };
        let excerpt: String = snippet
            .expansion
            .chars()
            .take(160)
            .collect::<String>()
            .replace("\r\n", " \u{21b5} ")
            .replace('\n', " \u{21b5} ");
        let dragged = DraggedSnippet {
            from: ix,
            trigger: snippet.trigger.clone(),
            kind: snippet.kind,
        };
        let trigger = snippet.trigger.clone();
        let this = cx.entity().downgrade();
        let theme = cx.theme();
        let (active, hover, ring, radius) = (theme.list_active, theme.list_hover, theme.ring, theme.radius);

        let row = h_flex()
            .id(("snippet", ix))
            .w_full()
            .h(px(ROW_HEIGHT))
            .pl_1()
            .pr_3()
            .gap_1()
            .rounded(radius)
            .cursor_pointer()
            .when(selected, |row| row.bg(active))
            .when(!selected, |row| row.hover(move |s| s.bg(hover)))
            .on_click(cx.listener(move |this, _, window, cx| {
                if let Some(snippet) = this.snippets.get(ix).cloned() {
                    this.select(&snippet, window, cx);
                }
                this.list_focus.focus(window, cx);
            }))
            .when(!locked, |row| {
                row.on_drag(dragged, |dragged, _, _, cx| cx.new(|_| DragPreview(dragged.clone())))
                    .drag_over::<DraggedSnippet>(move |style, _, _, _| style.border_t_2().border_color(ring))
                    .on_drop(cx.listener(move |this, dragged: &DraggedSnippet, _, cx| {
                        this.move_snippet(dragged.from, ix, cx);
                    }))
            })
            .child(
                div()
                    .flex_none()
                    .w_4()
                    .text_color(theme.muted_foreground)
                    .when(!locked, |grip| grip.child(Icon::new(Lucide::GripVertical).xsmall())),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1()
                    .child(
                        h_flex()
                            .justify_between()
                            .gap_2()
                            .child(trigger_badge(&snippet.trigger, snippet.kind, cx))
                            .child(
                                div()
                                    .flex_none()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(stats),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .truncate()
                            .child(excerpt),
                    ),
            )
            .context_menu(move |menu, _, _| {
                let this = this.clone();
                let trigger = trigger.clone();
                menu.item(PopupMenuItem::new("Delete...").on_click(move |_, window, cx| {
                    this.update(cx, |this, cx| this.confirm_delete(trigger.clone(), window, cx))
                        .ok();
                }))
            });
        div().w_full().px_2().child(row).into_any_element()
    }
}

impl Render for Manager {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        v_flex()
            .key_context(CONTEXT)
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .font_family(theme.font_family.clone())
            .on_action(cx.listener(Self::on_new))
            .on_action(cx.listener(Self::on_focus_search))
            .on_action(cx.listener(Self::on_save))
            .on_action(cx.listener(Self::open_settings))
            .child(self.render_title_bar(cx))
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.render_sidebar(cx))
                    .child(div().flex_1().min_w_0().h_full().child(self.editor.clone())),
            )
    }
}
