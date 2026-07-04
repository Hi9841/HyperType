# Design

Pure-black instrument panel for a Windows tray utility. OLED-black window,
flat hairline-separated sections (no card boxes — rejected as "weird stuff"),
one brand-purple accent. Quiet, precise, immediate.

## Theme

Dark only. The window is summoned over other work; it reads as a small
native settings panel, not a page. The window is frameless
(`decorations(false)`): the UI draws its own 44px titlebar with the brand
mark, a drag region, and Segoe Fluent window buttons (minimize / close, red
close hover per Windows convention). Fixed 480x854 — a 9:16 phone-portrait
ratio, non-resizable; the layout is designed for exactly this footprint,
sized so the settings, composer, and several snippet rows are all visible
at once.
No visible scrollbars: only the snippet list scrolls (wheel/touch,
phone-style). The webview background matches `--bg` so no white frame ever
flashes.

## Logo

Purple gradient lightning-bolt H, from the user-supplied brand pack at
`HyperType_Logo_Assets/` (approved mark, no wordmark). Gradient runs
#FFFFFF → #DDABFF → #7010FF; flat variants: brand-purple #7C2DFF,
electric-violet #A043FF, deep-purple #5718D2.

- App icon set: `04_App_Icons/hypertype-app-icon-dark-1024.png` copied to
  `src-tauri/icons/source.png`; regenerate with
  `pnpm tauri icon src-tauri/icons/source.png`.
- In-app titlebar mark: `03_PNG_Sizes/gradient-clean/…-128.png` copied to
  `src/assets/logo.png`, shown at 20px. Doubles as the ambient engine
  light: full color while running, grayscale-dimmed while paused (CSS
  filter on `.mark.paused`).

## Color

Neutral hex grays (no color tint — Apple's dark grays are effectively
colorless) plus one accent.

| Token | Value | Role |
| --- | --- | --- |
| `--bg` | `#000000` | window background (pure black) |
| `--raise` | `#161616` | row hover, keycaps, badges, titlebar hover |
| `--field` | `#131313` | inputs, segmented track |
| `--line` | `#1f1f1f` | field/seg borders |
| `--line-2` | `#2e2e2e` | keycap borders, seg thumb, hover borders |
| `--sep` | `#1a1a1a` | section and row hairlines |
| `--ink` | `#f2f2f2` | primary text |
| `--ink-2` | `#a3a3a3` | secondary text |
| `--ink-3` | `#7a7a7a` | tertiary hints, section labels |
| `--accent` | `#7c2dff` | brand purple: switch fills, Add button, field focus |
| `--accent-ink` | `#a043ff` | electric violet: accent text, chords, focus outlines (the lighter step hits 4.5:1 on black) |
| `--danger` | `#ff453a` | Apple systemRed: delete, quit-hover, errors |

Rules:

- The accent means "on / primary". It colors checked switches, the Add
  button, focus rings, the running status line, and shortcut chord keycaps.
  Never decoration.
- Engine paused ⇒ switch falls back to neutral; the subtitle ("Paused")
  carries the state, not color alone.
- `--danger` appears only on hover/focus of destructive controls and on
  error text. No resting red.

## Typography

System families only. No webfonts.

- **Headings/wordmark:** `"Segoe UI Variable Display"` (closest Windows
  analog to SF Pro Display).
- **UI:** `"Segoe UI Variable Text", "Segoe UI", system-ui`. Base 13px/1.5,
  weights 400/600.
- **Data:** Cascadia/Consolas mono for triggers, chords, keycaps, version.
- **Section labels:** sentence case only, 12px, 600, `--ink-2`. The one
  header is "Snippets" above the list. No uppercase tracked eyebrows —
  explicitly rejected.

## Shape & Space

- Radii: 5px keycaps, 6px small controls, 7px inputs/buttons/seg/rows,
  full pill for switches and the count badge.
- No card boxes and no shadows: sections sit directly on the black window
  and divide with full-width 1px `--sep` hairlines; snippet rows divide
  with inset (10px) `--sep` hairlines and light up `--raise` on hover.
- Spacing rhythm: 4/8/12/16. Window padding 22px horizontal; the snippet
  list is the flex-grow region and the only scroll container.

## Layout (flat groups, hairline-separated)

1. Titlebar: brand mark + wordmark left, minimize/close right, everything
   else drags the window. The mark fills accent while the engine runs — an
   ambient status light.
2. **Settings** group: "Text Expansion" row (pill switch + Running/Paused
   subtitle with live snippet count), "Launch at Login" row (autostart
   toggle, per-user run entry, no admin), "Insert Method" segmented row,
   and a "Typing Speed" slider row when Type is selected.
3. **Composer** group, stacked vertically for the narrow window: full-width
   sliding-thumb segmented control (Text | Shortcut), trigger field or chord
   recorder, expansion field, full-width blue Add button.
4. **Snippets** group: "Snippets" header + count badge, hairline-separated
   rows, hover-revealed controls (drag grip on the left, delete on the
   right). Chord triggers render as separate keycaps. Rows drag-reorder:
   the grip lifts the row (no transition, follows the pointer) while
   bystander rows glide aside; ArrowUp/Down on the focused grip moves the
   row for keyboard users. Order persists via `reorder_snippets`.
5. Footer: hint, version, Quit.

## Motion

State feedback only, CSS only.

- 130–180ms. Base curve `cubic-bezier(0.2, 0, 0, 1)` (ease-out).
- The one indulgence: switch knobs and the segmented thumb move on a
  slight spring (`cubic-bezier(0.32, 1.25, 0.32, 1)`) — the recognizable
  Apple overshoot, small enough to stay quiet.
- Newly added row: single 180ms fade + 3px rise, only for the row just
  added, never on initial load.
- Recorder in listening state: soft 1.2s accent pulse.
- `@media (prefers-reduced-motion: reduce)`: everything collapses to
  instant.

## Performance contract

- Zero UI dependencies beyond Solid itself; no motion or component
  libraries.
- No spinners for local IPC; optimistic mutation with quiet reconcile.
- `color-scheme: dark` + inline `<style>html{background:#000000}</style>`
  in `index.html` so first paint is already black (matches the native
  window's `background_color` in `main.rs`).
