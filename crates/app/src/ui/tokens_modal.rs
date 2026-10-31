//! Token catalog modal (S6). Spec: App.tsx:1268-1446, styles.css:795-849 + 975-1073.
//!
//! Uses `widgets::modal_frame_sized(680px, 0.88)`. Fade via `transition_modal_fade`
//! (CSS keyword ease-out). No Escape-to-close (A4 / rule 21).

use std::rc::Rc;

use gpui_kit::component::scroll::{Scrollbar, ScrollbarMode};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::*;

use crate::ui::studio::{Modal, Studio};
use crate::ui::tokens;
use crate::ui::widgets;

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
    scroll: ScrollHandle,
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
            scroll: ScrollHandle::new(),
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
        let scroll = self.scroll.clone();
        let body_entity = cx.entity();

        let on_close = {
            let studio = studio.clone();
            Rc::new(move |_window: &mut Window, cx: &mut App| {
                TokensModal::close(&studio, cx);
            })
        };

        // Rule 21a: close (modal header) then pills, all tab_index(0), tree order.
        let this = body_entity.downgrade();
        let groups: Vec<_> = GROUPS
            .iter()
            .map(|group| category_group(group, cols, this.clone()))
            .collect();

        // Body — styles.css:1000-1006. Content sizes the card up to max_h; then
        // this flex_1 + overflow_y_scroll viewport scrolls. Kit scrollbar overlays
        // the reserved 15px right gutter (pr 35 = 20 + 15). No taffy scrollbar_width.
        let body = v_flex()
            .id("tokens-catalog-body")
            .relative()
            .flex_1()
            .min_h_0()
            .w_full()
            .pt(px(16.))
            .pb(px(24.))
            .pl(px(20.))
            .pr(px(35.))
            .gap(px(18.))
            .overflow_y_scroll()
            .track_scroll(&scroll)
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
            .children(groups)
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .child(
                        Scrollbar::vertical(&scroll)
                            .id("tokens-catalog-scrollbar")
                            .mode(ScrollbarMode::Always)
                            .viewport_from_layout(),
                    ),
            );

        widgets::modal_frame_sized(
            "tokens-modal",
            "Dynamic Insert Tokens & Modifiers",
            px(680.),
            0.88,
            freeze,
            {
                let on_close = on_close.clone();
                move |window, cx| on_close(window, cx)
            },
            body,
            window,
            cx,
        )
    }
}

/// `.tokens-catalog-intro` — styles.css:1009-1020.
///
/// Rule 22 / A7: each `<code>` is its own element (bg/padding/radius), not a
/// TextRun. Prose sits in one `flex_wrap` row of a few segments, split only
/// where the browser can break (line 1 ends "...or flags", line 2 starts
/// "like"). GPUI rounds each text element's width up to a whole pixel
/// (gpui-pre text.rs:772), so per-word elements would waste up to 1px each
/// and push "flags" to line 2.
fn catalog_intro() -> impl IntoElement {
    let prose = |text: &'static str| {
        div()
            .text_size(px(12.5))
            .line_height(px(19.))
            .text_color(tokens::ink_2())
            .flex_shrink_0()
            .child(text)
    };
    // Reference code boxes are h=16 (text line 14 + padding 1+1).
    let code = |token: &'static str| {
        div()
            .id(SharedString::from(format!("tokens-intro-code-{token}")))
            .font_family(tokens::FONT_MONO)
            .text_size(px(11.5))
            .line_height(px(14.))
            .text_color(tokens::accent_ink())
            .bg(tokens::surface_2())
            .rounded(px(4.))
            .px(px(5.))
            .py(px(1.))
            .flex_shrink_0()
            .child(token)
    };

    // Reference intro box is 38.5 (two 19.25 line boxes). GPUI integer
    // line boxes are 19+19=38; min_h 39 keeps the catalog stack within 1px.
    h_flex()
        .id("tokens-catalog-intro")
        .w_full()
        .min_h(px(39.))
        .flex_wrap()
        .items_center()
        .child(prose(
            "Click any token to insert it directly into your expansion canvas. You can also mix offsets like ",
        ))
        .child(code("{date+3d}"))
        .child(prose(" or flags "))
        .child(prose("like "))
        .child(code("{date:us}"))
        .child(prose("."))
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
        // Never shrink in the scroll body (rule 23b).
        .flex_shrink_0()
        .gap(px(8.))
        .child(
            // .token-cat-title — styles.css:1028-1033 (letter-spacing omitted: D2)
            // Explicit 16px line box: 1.5×10.5 = 15.75 → Chromium/GPUI match (rule 23).
            div()
                .text_size(px(10.5))
                .line_height(px(16.))
                .font_weight(FontWeight::BOLD)
                .text_color(tokens::ink_3())
                .child(group.title),
        )
        .children(rows.into_iter().enumerate().map(move |(row_ix, row)| {
            let this = this.clone();
            let missing = cols - row.len();
            // Subpixel row pitch 60.55 → alternating 52/53 pill heights + gap 8
            // yields starts 0, 60, 121, 181 (pitches 60, 61, 60) — rule 23.
            // Catalog labels are fixed one-line spec text; a wrapping label would need content-driven height.
            let pill_h = if row_ix % 2 == 0 { px(52.) } else { px(53.) };
            h_flex()
                .w_full()
                .gap(px(8.))
                .children(row.iter().map(move |entry| {
                    token_pill(entry, pill_h, this.clone())
                }))
                .children((0..missing).map(|_| {
                    // Keep last-row track widths equal to 1fr columns.
                    div().flex_1().min_w(px(190.)).h(pill_h)
                }))
        }))
}

fn token_pill(
    entry: &'static TokenEntry,
    height: Pixels,
    this: WeakEntity<TokensModal>,
) -> impl IntoElement {
    let token = entry.token;
    // Braces are fine in SharedString group names (div.rs group API).
    let group_name: SharedString = format!("token-pill-{token}").into();
    let pill_id: SharedString = format!("token-pill-{token}").into();
    let label_id: SharedString = format!("token-pill-label-{token}").into();
    // .token-pill-card — styles.css:1041-1072
    v_flex()
        .id(pill_id)
        .group(group_name.clone())
        .tab_index(0)
        .relative()
        .flex_1()
        .min_w(px(190.))
        .h(height)
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
        .focus_visible(|s| {
            s.shadow(tokens::focus_visible_shadows(tokens::surface_2()))
        })
        // Enter/Space (rule 21): GPUI fires on_click on KeyUp for the focused
        // pill; a separate key handler would insert twice.
        .on_click(move |_, window, cx| {
            if let Some(entity) = this.upgrade() {
                entity.update(cx, |modal, cx| {
                    modal.insert_and_close(token, window, cx);
                });
            }
        })
        .child(
            // code — styles.css:1059-1064
            // 11.5 × 1.5 = 17.25 → 17px line box (rule 23).
            div()
                .font_family(tokens::FONT_MONO)
                .text_size(px(11.5))
                .line_height(px(17.))
                .font_weight(FontWeight::BOLD)
                .text_color(tokens::accent_ink())
                .child(token),
        )
        .child(
            // span — styles.css:1065-1071
            // Rule 24: stable id + group_hover → INK_2 when the pill is hovered.
            div()
                .id(label_id)
                .text_size(px(11.))
                .text_color(tokens::ink_3())
                .line_height(px(14.))
                .group_hover(group_name, |s| s.text_color(tokens::ink_2()))
                .child(entry.label),
        )
}
