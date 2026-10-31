//! Token catalog modal (S6). Spec: App.tsx:1268-1446, styles.css:795-849 + 975-1073.
//!
//! `widgets::modal_frame` hardcodes the Settings card at 500px / 85vh. The catalog
//! needs 680px / 88vh and a scroll body, so this file builds the frame locally with
//! the same backdrop/header/fade pattern. Request: extend `modal_frame` with
//! `card_width` + `card_max_h` so S6 can call it instead of duplicating.

use std::rc::Rc;

use gpui_kit::base::Presence;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::*;

use crate::ui::icons;
use crate::ui::studio::{Modal, Studio};
use crate::ui::tokens;

/// One catalog entry: token string + label (App.tsx verbatim).
struct TokenEntry {
    token: &'static str,
    label: &'static str,
}

struct TokenGroup {
    title: &'static str,
    entries: &'static [TokenEntry],
}

/// CSS `repeat(auto-fill, minmax(190px, 1fr))` with gap 8.
fn auto_fill_columns(width: f32) -> usize {
    const MIN: f32 = 190.;
    const GAP: f32 = 8.;
    if width <= 0. {
        return 1;
    }
    ((width + GAP) / (MIN + GAP)).floor().max(1.) as usize
}

const GROUPS: &[TokenGroup] = &[
    TokenGroup {
        title: "DATE & RELATIVE MATH",
        entries: &[
            TokenEntry {
                token: "{date}",
                label: "Today (YYYY-MM-DD)",
            },
            TokenEntry {
                token: "{date+1d}",
                label: "Tomorrow",
            },
            TokenEntry {
                token: "{date-1d}",
                label: "Yesterday",
            },
            TokenEntry {
                token: "{date+7d}",
                label: "Next week (+7 days)",
            },
            TokenEntry {
                token: "{date+1m}",
                label: "Next month (+30 days)",
            },
            TokenEntry {
                token: "{date+1y}",
                label: "Next year (+365 days)",
            },
            TokenEntry {
                token: "{date:text}",
                label: "Full text date (Thursday, Aug 20...)",
            },
            TokenEntry {
                token: "{date:us}",
                label: "US format (MM/DD/YYYY)",
            },
            TokenEntry {
                token: "{date:eu}",
                label: "European format (DD/MM/YYYY)",
            },
            TokenEntry {
                token: "{day}",
                label: "Day name (e.g. Thursday)",
            },
            TokenEntry {
                token: "{month}",
                label: "Month name (e.g. August)",
            },
            TokenEntry {
                token: "{year}",
                label: "4-digit year (YYYY)",
            },
        ],
    },
    TokenGroup {
        title: "TIME & OFFSETS",
        entries: &[
            TokenEntry {
                token: "{time}",
                label: "12h Time with AM/PM",
            },
            TokenEntry {
                token: "{time:24}",
                label: "24h Time (HH:MM)",
            },
            TokenEntry {
                token: "{time+1h}",
                label: "1 Hour Later",
            },
            TokenEntry {
                token: "{time+30m}",
                label: "30 Minutes Later",
            },
            TokenEntry {
                token: "{datetime}",
                label: "Date & Time combined",
            },
            TokenEntry {
                token: "{timestamp}",
                label: "Unix epoch seconds",
            },
        ],
    },
    TokenGroup {
        title: "CLIPBOARD & AI PROMPT MODIFIERS",
        entries: &[
            TokenEntry {
                token: "{clipboard}",
                label: "Raw clipboard text",
            },
            TokenEntry {
                token: "{clipboard:quote}",
                label: "Quote block (> line)",
            },
            TokenEntry {
                token: "{clipboard:code}",
                label: "Markdown code block (```)",
            },
            TokenEntry {
                token: "{clipboard:bullets}",
                label: "Bullet list (- line)",
            },
            TokenEntry {
                token: "{clipboard:upper}",
                label: "UPPERCASE",
            },
            TokenEntry {
                token: "{clipboard:lower}",
                label: "lowercase",
            },
            TokenEntry {
                token: "{clipboard:trim}",
                label: "Trimmed whitespace",
            },
            TokenEntry {
                token: "{clipboard:oneline}",
                label: "Single line (removes PDF enters)",
            },
            TokenEntry {
                token: "{clipboard:json}",
                label: "JSON escaped string",
            },
        ],
    },
    TokenGroup {
        title: "ENVIRONMENT & GENERATORS",
        entries: &[
            TokenEntry {
                token: "{active_app}",
                label: "Active app executable (e.g. Code.exe)",
            },
            TokenEntry {
                token: "{window_title}",
                label: "Active window title",
            },
            TokenEntry {
                token: "{uuid}",
                label: "Random UUID v4",
            },
            TokenEntry {
                token: "{random_pin}",
                label: "Random 6-digit number",
            },
            TokenEntry {
                token: "{delimiter}",
                label: "AI prompt security barrier",
            },
            TokenEntry {
                token: "{username}",
                label: "Windows username",
            },
            TokenEntry {
                token: "{computer}",
                label: "PC Hostname",
            },
        ],
    },
];

pub struct TokensModal {
    studio: WeakEntity<Studio>,
    /// Content width of `.tokens-catalog-body` (padding excluded). Default = 680-40.
    body_width: f32,
    _observe: Subscription,
}

impl TokensModal {
    pub fn new(studio: WeakEntity<Studio>, cx: &mut Context<Self>) -> Self {
        let _observe = studio
            .upgrade()
            .map(|entity| cx.observe(&entity, |_, _, cx| cx.notify()))
            .expect("Studio must exist when TokensModal is created");
        Self {
            studio,
            // Card 680px, body padding 20px L/R, Chromium scrollbar ~15px → ~605-625.
            // Seed assumes the gutter so the first frame already uses 3 columns.
            body_width: 625.,
            _observe,
        }
    }

    fn close(studio: &WeakEntity<Studio>, cx: &mut App) {
        if let Some(entity) = studio.upgrade() {
            entity.update(cx, |studio, cx| {
                studio.set_modal(None, cx);
            });
        }
    }

    fn insert_and_close(&self, token: &str, window: &mut Window, cx: &mut App) {
        if let Some(entity) = self.studio.upgrade() {
            let token = token.to_string();
            entity.update(cx, |studio, cx| {
                studio.insert_token(&token, window, cx);
                studio.set_modal(None, cx);
            });
        }
    }

    /// Parity arms live only in this file.
    ///
    /// Called from `ui/parity.rs` while `Studio` is already updating, so Studio
    /// mutations go through `cx.defer` (no nested `studio.update`).
    pub fn parity_state(
        &mut self,
        name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let _ = window;
        match name {
            "tokens-modal" => {
                let studio = self.studio.clone();
                cx.defer(move |cx| {
                    if let Some(entity) = studio.upgrade() {
                        entity.update(cx, |studio, cx| {
                            studio.set_modal(Some(Modal::Tokens), cx);
                        });
                    }
                });
                true
            }
            // Parity-only behavioral checks (no screenshot). Exercise the same
            // Studio paths as pill click / backdrop close.
            "tokens-modal-insert-check" => {
                let studio = self.studio.clone();
                cx.defer_in(window, move |modal, window, cx| {
                    modal.insert_and_close("{date}", window, cx);
                    let closed = studio
                        .upgrade()
                        .is_some_and(|e| e.read(cx).modal.is_none());
                    hypertype_core::logging::info(&format!(
                        "parity tokens-modal-insert-check: insert_token+close ok={closed}"
                    ));
                });
                true
            }
            "tokens-modal-close-check" => {
                let studio = self.studio.clone();
                cx.defer(move |cx| {
                    if let Some(entity) = studio.upgrade() {
                        entity.update(cx, |studio, cx| {
                            studio.set_modal(Some(Modal::Tokens), cx);
                            studio.set_modal(None, cx);
                        });
                        let closed = entity.read(cx).modal.is_none();
                        hypertype_core::logging::info(&format!(
                            "parity tokens-modal-close-check: set_modal None ok={closed}"
                        ));
                    }
                });
                true
            }
            _ => false,
        }
    }
}

impl Render for TokensModal {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let freeze = self
            .studio
            .upgrade()
            .is_some_and(|e| e.read(cx).freeze_animations);
        let studio = self.studio.clone();
        let cols = auto_fill_columns(self.body_width);

        let on_close = {
            let studio = studio.clone();
            Rc::new(move |_window: &mut Window, cx: &mut App| {
                TokensModal::close(&studio, cx);
            })
        };

        // Local catalog frame (modal_frame is 500/85; need 680/88). styles.css:799-849 + 995-998.
        let id: ElementId = "tokens-modal".into();
        let policy = tokens::motion(tokens::transition_modal_fade(), freeze);
        let opacity = Presence::new((id.clone(), "fade"), true)
            .transition(policy)
            .sample(window, cx)
            .progress;

        let close = on_close.clone();
        let body_entity = cx.entity();

        div()
            .id(id)
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(tokens::modal_backdrop())
            .opacity(opacity)
            .on_mouse_down(MouseButton::Left, {
                let close = on_close.clone();
                move |_, window, cx| close(window, cx)
            })
            .on_key_down({
                let close = on_close.clone();
                move |ev, window, cx| {
                    if ev.keystroke.key == "escape" {
                        close(window, cx);
                        cx.stop_propagation();
                    }
                }
            })
            .child(
                // .tokens-catalog-card — styles.css:816-825 + 995-998
                v_flex()
                    .id("tokens-catalog-card")
                    .w(px(680.))
                    .max_h(relative(0.88))
                    .bg(tokens::surface_1())
                    .border_1()
                    .border_color(tokens::line_2())
                    .rounded(px(12.))
                    .shadow(vec![BoxShadow {
                        color: tokens::modal_shadow(),
                        offset: point(px(0.), px(20.)),
                        blur_radius: tokens::css_blur(60.),
                        spread_radius: px(0.),
                        inset: false,
                    }])
                    .overflow_hidden()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                        cx.stop_propagation();
                    })
                    .child(
                        // .settings-modal-header — styles.css:827-837
                        h_flex()
                            .flex_none()
                            .px(px(20.))
                            .py(px(16.))
                            .border_b_1()
                            .border_color(tokens::line())
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(tokens::ink())
                                    .child("Dynamic Insert Tokens & Modifiers"),
                            )
                            .child(
                                // .modal-close-btn — styles.css:838-849
                                div()
                                    .id("tokens-modal-close")
                                    .p(px(4.))
                                    .rounded(px(4.))
                                    .text_color(tokens::ink_3())
                                    .cursor(CursorStyle::PointingHand)
                                    .hover(|s| s.text_color(tokens::ink()))
                                    .child(icons::close())
                                    .tooltip(move |window, cx| {
                                        Tooltip::new("Close").build(window, cx)
                                    })
                                    .on_click(move |_, window, cx| close(window, cx)),
                            ),
                    )
                    .child(
                        // .tokens-catalog-body — styles.css:1000-1006
                        v_flex()
                            .id("tokens-catalog-body")
                            .relative()
                            .flex_1()
                            .min_h_0()
                            .w_full()
                            .pt(px(16.))
                            .pb(px(24.))
                            // Left 20 matches CSS. Right 20+15 reserves Chromium scrollbar
                            // gutter so column widths match the reference capture (D5 look
                            // still differs; width is geometry).
                            .pl(px(20.))
                            .pr(px(35.))
                            .gap(px(18.))
                            .overflow_y_scroll()
                            .child(
                                // Width probe only — absolute so it does not consume flex gap.
                                canvas(
                                    {
                                        let entity = body_entity.clone();
                                        move |bounds, _, cx| {
                                            let w = f32::from(bounds.size.width);
                                            // Ignore empty first-layout probes.
                                            if w < 200. {
                                                return;
                                            }
                                            entity.update(cx, |this, cx| {
                                                if (this.body_width - w).abs() > 0.5 {
                                                    this.body_width = w;
                                                    cx.notify();
                                                }
                                            });
                                        }
                                    },
                                    |_, _, _, _| {},
                                )
                                .absolute()
                                .top(px(0.))
                                .left(px(0.))
                                .w_full()
                                .h(px(0.)),
                            )
                            .child(catalog_intro())
                            .children(GROUPS.iter().map(|group| {
                                category_group(group, cols, body_entity.downgrade())
                            })),
                    ),
            )
    }
}

/// `.tokens-catalog-intro` — styles.css:1009-1020
///
/// One StyledText paragraph so wrapping matches the spec. Code spans use mono +
/// ACCENT_INK + SURFACE_2 background (padding/radius on inline runs omitted).
fn catalog_intro() -> impl IntoElement {
    const TEXT: &str = "Click any token to insert it directly into your expansion canvas. You can also mix offsets like {date+3d} or flags like {date:us}.";
    const CODE1: &str = "{date+3d}";
    const CODE2: &str = "{date:us}";
    let i1 = TEXT.find(CODE1).expect("intro code1");
    let i2 = TEXT.find(CODE2).expect("intro code2");

    let ui_font = Font {
        family: tokens::FONT_UI.into(),
        features: FontFeatures::default(),
        fallbacks: Some(FontFallbacks::from_fonts(vec![
            tokens::FONT_UI_FALLBACK.to_string(),
        ])),
        weight: FontWeight::NORMAL,
        style: FontStyle::Normal,
    };
    let mono_font = Font {
        family: tokens::FONT_MONO.into(),
        features: FontFeatures::default(),
        fallbacks: Some(FontFallbacks::from_fonts(
            tokens::FONT_MONO_FALLBACKS
                .iter()
                .map(|f| (*f).to_string())
                .collect(),
        )),
        weight: FontWeight::NORMAL,
        style: FontStyle::Normal,
    };

    let ink = tokens::ink_2();
    let accent = tokens::accent_ink();
    let code_bg = Some(tokens::surface_2());

    let run = |len: usize, font: Font, color: Hsla, bg: Option<Hsla>| TextRun {
        len,
        font,
        color,
        background_color: bg,
        underline: None,
        strikethrough: None,
    };

    div()
        .id("tokens-catalog-intro")
        .w_full()
        .text_size(px(12.5))
        .line_height(relative(1.5))
        .child(
            StyledText::new(TEXT).with_runs(vec![
                run(i1, ui_font.clone(), ink, None),
                run(CODE1.len(), mono_font.clone(), accent, code_bg),
                run(i2 - (i1 + CODE1.len()), ui_font.clone(), ink, None),
                run(CODE2.len(), mono_font, accent, code_bg),
                run(TEXT.len() - (i2 + CODE2.len()), ui_font, ink, None),
            ]),
        )
}

fn category_group(
    group: &'static TokenGroup,
    cols: usize,
    this: WeakEntity<TokensModal>,
) -> impl IntoElement {
    // .token-category-group — styles.css:1022-1026
    // .token-pills-grid — styles.css:1035-1038: auto-fill rows of `cols` × 1fr
    let cols = cols.max(1);
    let rows: Vec<&[TokenEntry]> = group.entries.chunks(cols).collect();

    v_flex()
        .w_full()
        .gap(px(8.))
        .child(
            // .token-cat-title — styles.css:1028-1033 (letter-spacing omitted: D2)
            div()
                .text_size(px(10.5))
                .font_weight(FontWeight::BOLD)
                .text_color(tokens::ink_3())
                .child(group.title),
        )
        .children(rows.into_iter().map(move |row| {
            let this = this.clone();
            let missing = cols - row.len();
            h_flex()
                .w_full()
                .gap(px(8.))
                .children(row.iter().map(move |entry| {
                    token_pill(entry, this.clone())
                }))
                .children((0..missing).map(|_| {
                    // Keep last-row track widths equal to 1fr columns.
                    div().flex_1().min_w(px(190.))
                }))
        }))
}

fn token_pill(
    entry: &'static TokenEntry,
    this: WeakEntity<TokensModal>,
) -> impl IntoElement {
    let token = entry.token;
    let group_name: SharedString = format!("token-pill-{token}").into();
    let pill_id: SharedString = format!("token-pill-{token}").into();
    // .token-pill-card — styles.css:1041-1072
    v_flex()
        .id(pill_id)
        .group(group_name.clone())
        .relative()
        .flex_1()
        .min_w(px(190.))
        .items_start()
        .gap(px(3.))
        .px(px(10.))
        .py(px(8.))
        .bg(tokens::surface_2())
        .border_1()
        .border_color(tokens::line())
        .rounded(px(7.))
        .cursor(CursorStyle::PointingHand)
        .hover(|s| {
            // Instant hover (D3). Positional: translateY(-1px) → top(-1) (rule 7.2).
            s.bg(tokens::surface_3())
                .border_color(tokens::accent())
                .top(px(-1.))
        })
        .on_click(move |_, window, cx| {
            if let Some(entity) = this.upgrade() {
                entity.update(cx, |modal, cx| {
                    modal.insert_and_close(token, window, cx);
                });
            }
        })
        .child(
            // code — styles.css:1059-1064
            div()
                .font_family(tokens::FONT_MONO)
                .text_size(px(11.5))
                .font_weight(FontWeight::BOLD)
                .text_color(tokens::accent_ink())
                .child(token),
        )
        .child(
            // span — styles.css:1065-1071
            div()
                .text_size(px(11.))
                .text_color(tokens::ink_3())
                .line_height(relative(1.3))
                .group_hover(group_name, |s| s.text_color(tokens::ink_2()))
                .child(entry.label),
        )
}
