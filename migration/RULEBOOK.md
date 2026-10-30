# RULEBOOK - HyperType GPUI 1:1 port (design document)

Posture: REDESIGN of the UI mechanism with a FIXED visual and behavioral
target. The spec is the v1.1.8 SolidJS UI at commit 137ac57:
`src/App.tsx`, `src/styles.css`, `src/components/Icons.tsx`,
`src/lib/ipc.ts`, `src/lib/shortcut.ts`, `src-tauri/tauri.conf.json`,
`src-tauri/src/main.rs`. When this file and the spec disagree, the spec wins;
record the disagreement in "Amendment queue" below.

Rules for agents: this file is read-only for implementers, reviewers, and
fixers. Only the coordinator edits it, between batches.

## 1. Stack and build

- GPUI Kit `=0.7.3` (`gpui-pre 0.3.9`). Import with `use gpui_kit::*;`,
  components from `gpui_kit::component`, behavior from `gpui_kit::base`.
- Verify every API in `~/.cargo/registry/src/*/gpui-{pre-0.3.9,base-0.7.3,component-0.7.3}`.
  Never invent methods from React, CSS, or other GPUI versions.
- Build with cargo (not blaze). Gate per slice:
  `cargo clippy -p hypertype --all-targets -- -D warnings` and the same with
  `--features parity`.
- No unit tests. Proof = parity judge + scenarios on the real app.
- Commits: one per slice, `feat(app): ... [skip ci]`, no co-author lines.
- Review: every implementer result gets a read-only review by a separate
  Grok 4.7 `xhigh` agent; the coordinator rules on its findings, then a fix
  round runs. Owner rule, 2026-10-31.

## 2. Module layout and ownership

All new UI code lives in `crates/app/src/ui/`. One slice owns each file.
A slice edits ONLY its own files. A needed change in a file it does not own
goes in its report as a request; the coordinator applies it.

| File | Owner slice | Purpose |
| --- | --- | --- |
| `ui/mod.rs` | S1 | module list, `ui::open_window`, root `Studio` render |
| `ui/tokens.rs` | S1 | color, type, radius, motion constants (section 3) |
| `ui/icons.rs` + `assets/icons/*.svg` | S1 | the 13 spec icons (section 6) |
| `ui/widgets.rs` | S1 | shared primitives (section 5) |
| `ui/studio.rs` | S1 | `Studio` entity: shared state + methods (section 4) |
| `ui/titlebar.rs` | S1 | frameless title bar incl. update pill |
| `ui/sidebar.rs` | S2 (+S3 reorder) | library pane |
| `ui/editor.rs` | S4 | editor canvas |
| `ui/settings_modal.rs` | S5 | settings modal |
| `ui/tokens_modal.rs` | S6 | token catalog modal |
| `ui/update.rs` | S1 stub, S7b fills | `UpdateUi` state + `check` / `install` glue |
| `ui/parity.rs` | S1 (dispatch), each slice adds arms in its own file | parity states |
| `updater.rs` | S7a | updater backend (no UI) |
| `shell.rs`, `main.rs`, `parity.rs` | S1 | window open, startup |
| `instance.rs`, `tray.rs`, `hotkeys.rs`, `autostart.rs`, `events.rs`, `chord.rs` | keep; edit only with coordinator approval | OS integration |

S1 deletes the superseded files: `manager.rs`, `editor.rs` (old),
`settings.rs`, `tokens.rs` (old), `widgets.rs` (old), `theme.rs` (replaced by
`ui/tokens.rs` + a small Kit theme setup), `assets/theme.json`,
`assets/fonts/*`, and their font registration.

Parity dispatch: `ui/parity.rs` holds `pub fn apply(name, studio, window, cx) -> bool`.
It calls `sidebar::parity_state`, `editor::parity_state`,
`settings_modal::parity_state`, `tokens_modal::parity_state`,
`titlebar::parity_state` in turn; each returns `true` if it handled the name.
Each slice adds arms ONLY in its own file's `parity_state` fn.

## 3. Tokens (exact values from `src/styles.css:4-36`)

`ui/tokens.rs` defines these as `Hsla` constants built from hex (`rgb(0x...)`
/ `rgba(...)`), plus `px` constants. Views use ONLY these names, never Kit
theme colors and never raw hex.

| Name | Value | Name | Value |
| --- | --- | --- | --- |
| BG | #000000 | ACCENT | #7c2dff |
| SURFACE_1 | #0c0c0f | ACCENT_HOVER | #8f47ff |
| SURFACE_2 | #141418 | ACCENT_DOWN | #681ee0 |
| SURFACE_3 | #1e1e26 | ACCENT_INK | #b580ff |
| FIELD | #111115 | RING | rgba(124,45,255,.35) |
| LINE | #1c1c22 | RING_SOFT | rgba(124,45,255,.20) |
| LINE_2 | #282832 | DANGER | #ff453a |
| SEP | #18181f | DANGER_TINT | rgba(255,69,58,.12) |
| CONTROL | #202028 | SUCCESS | #30d158 |
| CONTROL_LINE | #30303c | WHITE | #ffffff |
| INK | #f4f4f7 | INK_2 | #9e9ea8 |
| INK_3 | #6b6b78 | PILL_END | #a855f7 |

One-off colors used by single rules (keep them as named constants in the
owning file, with the CSS line in a comment): active row bg
rgba(124,45,255,.14); transfer toast bg .10 / border .20; update pill bg .18;
hook active bg rgba(34,197,94,.12) ink #4ade80; hook inactive bg
rgba(239,68,68,.12) ink #f87171; modal backdrop rgba(0,0,0,.75); modal
shadow 0 20px 60px rgba(0,0,0,.9); status-dot glow rgba(48,209,88,.8);
knob shadow 0 1px 2px rgb(0 0 0/.4); slider thumb shadow 0 1px 3px /.5;
BRAND_PAUSED_OPACITY 0.45 + grayscale (styles.css:112-115).

Type:
- UI font family "Segoe UI Variable Text"; fallback "Segoe UI". Mono font
  "Cascadia Code"; fallbacks "Cascadia Mono", "Consolas". All are Windows
  system fonts. Do not bundle fonts. S1 must prove in a capture that the
  family name resolves (DirectWrite) and not a fallback.
- Root text: 13px, weight 400, line height 1.5, color INK, bg BG.
- Use the exact CSS px sizes (12.5, 11.5, 10.5 are valid `px()` values) and
  weights (400, 500, 600, 700 -> `FontWeight::NORMAL/MEDIUM/SEMIBOLD/BOLD`).
- `text-transform: uppercase` -> write the uppercase string ("STUDIO").

Motion (Kit `gpui_kit::base` `Transition` + `Easing::cubic_bezier`):
- EASE_OUT = cubic-bezier(0.2, 0, 0, 1); SPRING = cubic-bezier(0.32, 1.25, 0.32, 1).
- FAST = 130 ms; SMOOTH = 180 ms.
- MODAL_FADE = 150 ms ease-out (`@keyframes fadeIn`, styles.css:808-814).
- LISTENING = 1.2 s ease-in-out, infinite: box-shadow 0 0 0 4px RING_SOFT at
  50% (styles.css:587-593). SPIN = 1 s linear, infinite (refresh icon,
  styles.css:1150-1155; `svg().with_transformation(Transformation::rotate)`).
- Exact construction (`Easing::cubic_bezier` returns Result):
  `Transition::new(FAST).easing(Easing::cubic_bezier(0.2, 0., 0., 1.).expect("EASE_OUT"))`;
  SPRING the same with (0.32, 1.25, 0.32, 1.). Use `.easing(..)`, not `.ease(..)`.

## 4. State model (`ui/studio.rs`)

One `Studio` entity is the root view and owns all shared state. It maps the
Solid signals in `App.tsx:47-153` one to one:

```rust
pub struct Studio {
    pub state: Arc<AppState>,          // hypertype_core
    pub status: Status,                // commands::status, refreshed by refetch_status()
    pub snippets: Vec<SnippetView>,    // commands::snippets
    pub autostart: bool,
    pub selected: Option<String>,      // selectedTrigger
    pub creating_new: bool,            // isCreatingNew
    pub modal: Option<Modal>,          // Settings | Tokens
    pub update: UpdateUi,              // info, checking, status_text, downloading, progress (S7b fills)
    pub sidebar: Entity<Sidebar>, pub editor: Entity<Editor>,
    pub settings: Entity<SettingsModal>, pub tokens: Entity<TokensModal>,
}
```

Children get a `WeakEntity<Studio>` in their `new` and `cx.observe` it, so
they re-render when Studio notifies. They read Studio fields in render and
call Studio methods; they never hold copies of shared fields.

Frozen Studio API (S1 implements all of it; later slices only call it):

| Method | Spec | Notes |
| --- | --- | --- |
| `refetch_status(cx)`, `refetch_snippets(cx)` | createResource refetch | commands::status / snippets |
| `patch_status(f, cx)` | mutateStatus | optimistic edits by Settings |
| `select_snippet(&SnippetView, window, cx)` | App.tsx:225-234 | selected=Some, creating_new=false, calls `editor.load` |
| `start_new_snippet(window, cx)` | App.tsx:236-246 | creating_new=true, selected=None, `editor.clear_for_new` |
| `save_snippet(old: Option<&str>, trigger, expansion, kind, cx) -> Result<(), String>` | App.tsx:454-480 | add or edit via commands; on Ok selected=trigger, creating_new=false, refetch both; Editor owns busy and messages |
| `delete_snippet(trigger, window, cx)` | App.tsx:482-499 | async confirm (section 8); on OK remove, refetch, reselect first or start new; error -> `editor.set_error` |
| `apply_reorder(from, to, cx)` | App.tsx:501-513 | optimistic; on Err refetch |
| `toggle_enabled(cx)` | App.tsx:275-285 | also syncs tray + hotkeys via `shell::toggle_enabled` |
| `set_autostart(on, cx)` | App.tsx:381-389 | optimistic, revert on Err |
| `set_modal(Option<Modal>, cx)` | showSettings / showTokensModal | |
| `insert_token(&str, window, cx)` | App.tsx:437-452 | forwards to `editor.insert_token` |

Settings setters (insert mode, paste combo, WPM, restore delay, auto-paste
words), repair hook, run as admin, quit, export and import stay in the file
of the only slice that uses them (S5, S2). They call `hypertype_core::commands`
plus `patch_status` / `refetch_*`.

Frozen Editor API (S1 stubs with doc comments, S4 implements):
`load(&SnippetView, window, cx)`, `clear_for_new(window, cx)` (focuses the
trigger input), `insert_token(&str, window, cx)` (replace the selection,
caret after the token), `stop_recording(cx)`, `set_error(String, cx)`.

Update glue (`ui/update.rs`, S1 stub, S7b fills): `UpdateUi { info, checking,
status_text, downloading, progress }`, `check(studio, manual, cx)`,
`install(studio, cx)`. S1 calls `check(.., false)` once at window open
(App.tsx:205). The title-bar pill (S1) shows when `info` is Some and opens
Settings. The Settings footer (S5) reads `UpdateUi` and calls `check` / `install`.

Enabled sync: the tray toggle runs `shell::toggle_enabled`; Studio observes
`Shell` and refetches `status.enabled` (App.tsx:191-195). The title-bar dot,
the brand mark, and the Settings switch all read `studio.status.enabled`.

### 4.1 Behavioral contracts (copy strings verbatim)

- Text trigger: at most 64 code points; extra input is cut (App.tsx:375-379).
- `canSave` = not busy, trimmed trigger non-empty, expansion non-empty (260-261).
- Save success text "Snippet created!" (new) / "Saved!" (edit), cleared after 3 s (466-474).
- Recorder (400-435, shortcut.ts:28-37): click starts; Esc cancels; pointer
  down anywhere, window blur, or focus loss cancels; bare modifier ignored;
  a chord needs Ctrl/Alt/Shift/Win; on a chord: set trigger, stop, focus the
  expansion textarea. Hint texts "Click to Record Shortcut Chord" and
  "Press keys on keyboard… Esc cancels".
- Mode switch to the other kind clears the trigger and error; switching to
  text focuses the trigger input (391-398).
- Footer stats: lines = split on "\n" (0 if empty), words = trimmed split on
  whitespace (0 if empty), chars = length; "line"/"lines", "word"/"words",
  always "chars" (264-269, 910-919).
- Row badge "N lines" when lines > 1 else "N chars"; excerpt replaces "\n"
  with " ↵ " (740-783). List count "N total" uses the filtered list (717).
- Search filter: trimmed, lowercase, matches trigger or expansion (209-218).
  Reorder is locked while the trimmed search is non-empty (222).
- Transfer toast: "Exported to JSON"; "Imported N" + ", overwrote N" if > 0
  + ", skipped N" if > 0; cleared after 3 s; errors stay (580-614).
- Hook messages: "Reinstalling hook...", "Hook active & reconnected!",
  "Failed to reinstall hook", "Requesting administrator privileges...";
  cleared after 3.5 s (155-183). Hook events use thousands separators (1177).
- Settings status poll every 1.5 s while Settings is open (197-203).
- Slider ranges: words 1-100 step 1; WPM 100-1500 step 50; restore
  3000-15000 step 100 (1055-1159). Drag shows the live value; release commits.
- Update texts: "Checking for updates...", "v<x> available",
  "HyperType is up to date" (4 s), "Update check failed" (4 s),
  "Downloading (<p>%)", "Update installed. Restarting...",
  "Update installed. Restart HyperType.", "Update failed: <first 45 chars>" (61-120).
- Silent update check at window open (205).

Parity mode (`--features parity`): `Studio` shows the browser-mock status
values (version "1.1.5", hook_events 42, hook active and healthy, not
elevated) and freezes infinite animations at t = 0.

## 5. Shared widgets (`ui/widgets.rs`, S1)

Each widget reproduces one CSS rule set; the doc comment names the CSS
lines. Required: `keycap` (.trigger-keycap), `chord_key` / `chord_badge`
(.chord-key, .chord-badge, uses `chord::keys`), `segmented` (.seg, .seg-kind,
.seg-3, .seg-combo; animated thumb, SMOOTH + SPRING), `switch` (.switch,
.knob; SMOOTH + SPRING), `slider` (.slider; 130px track 4px, 14px white
thumb; drag emits live value, release emits commit), `value_edit`
(.value-edit; select-all on focus, Enter blurs, blur commits clamped value,
empty or NaN reverts, App.tsx:358-373), `field` frame (.studio-field with the
3px RING_SOFT focus ring), `hook_badge` (.hook-status-badge), `modal_frame`
(.settings-modal-backdrop/.settings-modal-card/.settings-modal-header with
close button; fade in 150 ms; click on backdrop closes; Escape closes),
button styles used in more than one file.

Text inputs: Kit `Input` / `Textarea` with `appearance(false)` inside our
own frame. `Input` binds `Entity<InputState>`; `Textarea` binds
`Entity<TextareaState>`. Create them once in the owning entity's `new`,
never in render. Kit theme is set in S1 so that the caret is
INK and the selection is RING.

## 6. Icons

Copy the 13 SVGs from `src/components/Icons.tsx` verbatim into
`crates/app/assets/icons/<name>.svg` (keep viewBox, stroke widths, caps,
joins; `currentColor` becomes the element's text color). Render size = the
`width`/`height` in Icons.tsx. Names: search, settings, plus, close, grip,
document, check, minimize, close-window, export, import, refresh, download.

Brand mark: keep `assets/logo.png` (`assets::logo()`), 18x18. Paused =
`img(..).grayscale(true)` + opacity BRAND_PAUSED_OPACITY (styles.css:108-115).

## 7. CSS-to-GPUI translation rules

1. Every px, color, radius, border, padding, gap, and size comes from the
   CSS rule for that element. Put the CSS selector and line in a short
   comment above each element builder that is not obvious.
2. `:hover` -> `.hover(...)` on an element with a stable `id`. `:enabled`
   hover rules apply only when not disabled. Hover color/border changes are
   INSTANT in every slice (D3, mandatory, for consistency). Positional hover
   (`.token-pill-card:hover translateY(-1px)`) -> `.hover(|s| s.top(px(-1.)))`
   on a `.relative()` element.
3. `:focus` rings -> read the `FocusHandle` in render and draw the border +
   `BoxShadow { offset: point(px(0.), px(0.)), blur_radius: px(0.),
   spread_radius: px(3.) /* search: 2 */, color: RING_SOFT, inset: false }`.
   Global `:focus-visible` (2px ACCENT_INK outline, offset 2px,
   styles.css:64-67) -> `.focus_visible(..)` with two shadows: spread 2px in
   the local background color, then spread 4px ACCENT_INK (D10).
4. `disabled` -> opacity and cursor from the CSS; no hover; no click.
5. `cursor: pointer` -> `CursorStyle::PointingHand`; `grab` -> `OpenHand`
   (`ClosedHand` while dragging); `default` -> `Arrow`; `wait` -> `Arrow`
   (no Wait in 0.3.9, D11).
6. `title="..."` -> Kit `Tooltip` with the same text.
7. `white-space: nowrap; overflow: hidden; text-overflow: ellipsis` ->
   `.truncate()` (or `.text_ellipsis()` + `.overflow_hidden()` + `.whitespace_nowrap()`).
8. Repeated rows use domain IDs (`("row", trigger)`), never indexes alone.
9. Layout: `.studio-workspace` is a 340px + flex-1 row; `.studio-canvas`
   padding 20px 24px, gap 14px; follow each flex/grid rule literally.
   CSS `grid repeat(auto-fill, minmax(190px, 1fr))` -> compute the column
   count from the container width (S6).
10. Scroll areas (`overflow-y: auto`) -> `div().id(..).overflow_y_scroll()`
   (track a `ScrollHandle` where needed). Do not invent a `scrollable()`
   helper. Scrollbar look follows D5.

## 8. Window and native behavior

- Window: 960x640 logical, min 780x520, centered, frameless (no OS title bar,
  no OS buttons), BG painted from the first frame, title "HyperType",
  dark. The title bar row (42px) is a drag area
  (`WindowControlArea::Drag`) except its buttons. Minimize button ->
  `window.minimize_window()`. Close button -> close the window (process
  stays in the tray, `QuitMode::Explicit`). Resize must work on all four
  edges and corners (gpui-pre-windows events.rs:987+ does a custom hit test
  for client-decorated windows); S1 verifies by hand and reports.
- Confirm delete: `window.prompt(PromptLevel::Warning, &format!("Delete snippet \"{t}\"?"), None, &["OK", "Cancel"], cx)`
  returns `oneshot::Receiver<usize>`; await it in a spawned task; continue
  only on `Ok(0)` (OK). Cancel or a dropped receiver does nothing.
- Export: save dialog, default name `HyperType-snippets.json`, filter
  name "HyperType snippets", extension `json`. Import: open dialog, same
  filter, single file. Use the `rfd` crate (Tauri's dialog plugin uses it);
  GPUI `PathPromptOptions` has no filters, so it is not allowed.
- Native integration already present and kept: single instance, tray
  (Enabled check / Open HyperType / Quit), hotkeys, autostart, `--minimized`.

## 9. Allowed deviations (only these)

- D1 No `backdrop-filter: blur(4px)` behind modals (GPUI has no backdrop blur).
- D2 No `letter-spacing` (GPUI 0.3.9 has no letter spacing). Affects
  .brand-name, .studio-pill, .list-title, .field-label, .token-cat-title, h2.
- D3 Hover color/border fades may be instant instead of 130 ms.
- D4 Glyph rasterization differs (DirectWrite vs Chromium).
- D5 Scrollbar look (Kit scrollbar instead of the Chromium dark scrollbar).
- D6 `transform: scale(.98)` on the pressed save button may be omitted.
- D7 Tooltip look and delay follow Kit `Tooltip`.
- D8 Confirm and file dialogs are native Win32 instead of WebView2 dialogs.
- D4 also covers `-webkit-font-smoothing: antialiased` (styles.css:51).
- D10 The focus-visible outline gap uses the local background color.
- D11 `cursor: wait` shows the arrow cursor.
Anything else that differs is a defect.

## 10. Slice plan and parity states

| Slice | Files | Parity states it must pass |
| --- | --- | --- |
| S1 foundation | section 2 S1 files | layout frame of `default-selected` (title bar, pane split, backgrounds) |
| S2 sidebar | `ui/sidebar.rs` | default-selected (left pane), row-hover, search-filter, search-no-match, empty-library |
| S3 reorder | `ui/sidebar.rs` | scenarios: drag, keyboard, lock |
| S4 editor | `ui/editor.rs` | default-selected (right pane), new-snippet, save-disabled, shortcut-mode-empty, shortcut-recording |
| S5 settings | `ui/settings_modal.rs` | settings-modal |
| S6 tokens | `ui/tokens_modal.rs` | tokens-modal |
| S7a updater backend | `updater.rs` | probe against the real feed |
| S7b updater UI | `ui/update.rs` (pill in titlebar.rs and footer in settings_modal.rs already read it) | update-available (manual) |

Order: S1 first (alone). Then S2+S3, S4, S5+S6 in parallel worktrees.
S7a runs now. S7b after S5 and S7a merge.

## Amendment queue

(empty)

## Amendment log

- 2026-10-31 A1: applied review r1 (Grok 4.5 high), findings 1-15: frozen
  Studio/Editor/update API, 4.1 behavioral contracts, motion tokens,
  BoxShadow/prompt/Textarea API shapes, rfd mandatory, logo,
  overflow_y_scroll, focus-visible, hover rules, resize check, D10-D11.
  Owner rule added: Grok 4.7 xhigh review of every implementer result.

## Deviation log

(empty)
