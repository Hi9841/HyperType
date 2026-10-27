//! The catalog of dynamic tokens that `hypertype_core::expansion` resolves
//! at expansion time, and the dialog that inserts one into the editor.

use gpui_kit::component::{ActiveTheme as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::component::StyledExt as _;
use gpui_kit::*;

pub struct Token {
    pub code: &'static str,
    pub label: &'static str,
}

pub struct Group {
    pub title: &'static str,
    pub tokens: &'static [Token],
}

const fn t(code: &'static str, label: &'static str) -> Token {
    Token { code, label }
}

/// Shown as chips above the editor.
pub const QUICK: [&str; 3] = ["{date}", "{time}", "{clipboard}"];

pub const GROUPS: &[Group] = &[
    Group {
        title: "Date",
        tokens: &[
            t("{date}", "Today (YYYY-MM-DD)"),
            t("{date+1d}", "Tomorrow"),
            t("{date-1d}", "Yesterday"),
            t("{date+7d}", "Next week (+7 days)"),
            t("{date+1m}", "Next month (+30 days)"),
            t("{date+1y}", "Next year (+365 days)"),
            t("{date:text}", "Full date (Thursday, Aug 20...)"),
            t("{date:us}", "US format (MM/DD/YYYY)"),
            t("{date:eu}", "European format (DD/MM/YYYY)"),
            t("{day}", "Day name (e.g. Thursday)"),
            t("{month}", "Month name (e.g. August)"),
            t("{year}", "4-digit year (YYYY)"),
        ],
    },
    Group {
        title: "Time",
        tokens: &[
            t("{time}", "12-hour time with AM/PM"),
            t("{time:24}", "24-hour time (HH:MM)"),
            t("{time+1h}", "1 hour later"),
            t("{time+30m}", "30 minutes later"),
            t("{datetime}", "Date and time"),
            t("{timestamp}", "Unix epoch seconds"),
        ],
    },
    Group {
        title: "Clipboard",
        tokens: &[
            t("{clipboard}", "Raw clipboard text"),
            t("{clipboard:quote}", "Quote block (> line)"),
            t("{clipboard:code}", "Markdown code block"),
            t("{clipboard:bullets}", "Bullet list (- line)"),
            t("{clipboard:upper}", "UPPERCASE"),
            t("{clipboard:lower}", "lowercase"),
            t("{clipboard:trim}", "Trimmed whitespace"),
            t("{clipboard:oneline}", "One line (removes PDF line breaks)"),
            t("{clipboard:json}", "JSON-escaped string"),
        ],
    },
    Group {
        title: "Environment",
        tokens: &[
            t("{active_app}", "Active app executable (e.g. Code.exe)"),
            t("{window_title}", "Active window title"),
            t("{uuid}", "Random UUID v4"),
            t("{random_pin}", "Random 6-digit number"),
            t("{delimiter}", "AI prompt security barrier"),
            t("{username}", "Windows user name"),
            t("{computer}", "Computer name"),
        ],
    },
];

/// Open the catalog. `insert` runs with the chosen token; the dialog then
/// closes.
pub fn open(window: &mut Window, cx: &mut App, insert: impl Fn(&str, &mut Window, &mut App) + 'static) {
    let insert = std::rc::Rc::new(insert);
    window.open_dialog(cx, move |dialog, _, cx| {
        let insert = insert.clone();
        dialog
            .title("Tokens")
            .w(px(640.))
            .child(catalog(insert, cx))
    });
}

fn catalog(
    insert: std::rc::Rc<dyn Fn(&str, &mut Window, &mut App)>,
    cx: &App,
) -> impl IntoElement {
    let theme = cx.theme();
    let (fg, muted, hover, radius) = (
        theme.foreground,
        theme.muted_foreground,
        theme.secondary_hover,
        theme.radius,
    );
    let mono = theme.mono_font_family.clone();
    v_flex()
        .id("token-catalog")
        .gap_4()
        .max_h(px(460.))
        .overflow_y_scroll()
        .child(
            div()
                .text_sm()
                .text_color(muted)
                .child("Tokens are replaced when the snippet expands. Offsets and formats combine, for example {date+3d} or {date:us}."),
        )
        .children(GROUPS.iter().map(|group| {
            v_flex()
                .gap_1p5()
                .child(
                    div()
                        .text_xs()
                        .font_medium()
                        .text_color(muted)
                        .child(group.title),
                )
                .child(
                    div()
                        .grid()
                        .grid_cols(2)
                        .gap_1()
                        .children(group.tokens.iter().map(|token| {
                            let insert = insert.clone();
                            let code = token.code;
                            h_flex()
                                .id(code)
                                .gap_3()
                                .px_2()
                                .py_1p5()
                                .rounded(radius)
                                .cursor_pointer()
                                .hover(move |s| s.bg(hover))
                                .on_click(move |_, window, cx| {
                                    insert(code, window, cx);
                                    window.close_dialog(cx);
                                })
                                .child(
                                    div()
                                        .font_family(mono.clone())
                                        .text_xs()
                                        .text_color(fg)
                                        .child(code),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(muted)
                                        .truncate()
                                        .child(token.label),
                                )
                        })),
                )
        }))
}
