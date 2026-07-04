# Paste Optimization + Configurable Insertion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix HyperType's paste path (full-format clipboard preservation, sequence-number restore, configurable paste combo) and add an Auto insert mode, with every knob exposed in the Settings card.

**Architecture:** All insertion policy (insert mode, WPM, paste combo, restore delay) becomes atomics in `expansion/mod.rs`, set at startup from `settings.json` and live via IPC — the existing `WPM` pattern. `AppState.type_out` disappears; `expansion::expand` resolves the mode itself. `clipboard.rs` gains snapshot/restore of every HGLOBAL-backed format; `inject.rs` generalizes `paste_steps` from hardcoded Ctrl+V to any (required-modifiers, key) combo.

**Tech Stack:** Rust (windows crate 0.58, Tauri v2), SolidJS + TypeScript frontend.

**Spec:** `docs/superpowers/specs/2026-07-02-paste-optimization-design.md`

## Global Constraints

- Windows-only; `windows` crate 0.58; MSVC toolchain (pinned in rust-toolchain.toml).
- **Do NOT `git commit`** — the repo owner keeps all work uncommitted on master. Skip every commit step; leave changes in the working tree.
- Run Rust tests from `src-tauri/`: `cargo test`. Frontend build: `pnpm build` from repo root.
- Settings enums serialize snake_case: `"auto" | "paste" | "type"`, `"ctrl_v" | "shift_insert" | "ctrl_shift_v"`.
- Clamps: WPM 100–1500 (existing), restore delay 100–2000 ms. Auto-mode paste threshold: newline present OR > 40 chars.
- Synthetic input cannot trigger the keyboard hook (by design) — end-to-end expansion is physical-typing-only; do not attempt scripted e2e.

---

### Task 1: Generalize `paste_steps` in inject.rs

**Files:**
- Modify: `src-tauri/src/expansion/inject.rs`

**Interfaces:**
- Produces: `fn paste_steps(required: &[u16], key: u16, held: &[u16]) -> Vec<(u16, bool)>` (private, tested); `pub fn send_paste(required: &[u16], key: u16)` replacing `pub fn ctrl_v()`.
- `held` = specific physically-held modifier VKs (VK_LCONTROL etc.); `required` = generic modifier classes (VK_CONTROL.0, VK_SHIFT.0).

- [ ] **Step 1: Rewrite the existing tests for the new signature and add combo cases**

Replace the `tests` module bodies (imports gain `VK_SHIFT`, `VK_INSERT`):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const CTRL: u16 = VK_CONTROL.0;
    const SHIFT: u16 = VK_SHIFT.0;
    const INS: u16 = VK_INSERT.0;
    const LCTRL: u16 = VK_LCONTROL.0;
    const LSHIFT: u16 = VK_LSHIFT.0;
    const LALT: u16 = VK_LMENU.0;
    const LWIN: u16 = VK_LWIN.0;

    #[test]
    fn plain_paste_when_nothing_held() {
        assert_eq!(
            paste_steps(&[CTRL], VK_V, &[]),
            vec![(CTRL, false), (VK_V, false), (VK_V, true), (CTRL, true)]
        );
    }

    #[test]
    fn reuses_physically_held_ctrl() {
        assert_eq!(paste_steps(&[CTRL], VK_V, &[LCTRL]), vec![(VK_V, false), (VK_V, true)]);
    }

    #[test]
    fn lifts_and_restores_shift() {
        assert_eq!(
            paste_steps(&[CTRL], VK_V, &[LCTRL, LSHIFT]),
            vec![(LSHIFT, true), (VK_V, false), (VK_V, true), (LSHIFT, false)]
        );
    }

    #[test]
    fn masks_alt_and_win_restores() {
        assert_eq!(
            paste_steps(&[CTRL], VK_V, &[LCTRL, LALT, LWIN]),
            vec![
                (LALT, true),
                (LWIN, true),
                (VK_V, false),
                (VK_V, true),
                (LALT, false),
                (LWIN, false),
                (VK_MASK, false),
                (VK_MASK, true),
            ]
        );
    }

    #[test]
    fn shift_restore_needs_no_mask() {
        let steps = paste_steps(&[CTRL], VK_V, &[LCTRL, LSHIFT]);
        assert!(!steps.iter().any(|&(vk, _)| vk == VK_MASK));
    }

    #[test]
    fn shift_insert_lifts_held_ctrl() {
        // Ctrl+Insert is *copy*; a held Ctrl must not leak into Shift+Insert.
        assert_eq!(
            paste_steps(&[SHIFT], INS, &[LCTRL]),
            vec![
                (LCTRL, true),
                (SHIFT, false),
                (INS, false),
                (INS, true),
                (SHIFT, true),
                (LCTRL, false),
            ]
        );
    }

    #[test]
    fn shift_insert_reuses_held_shift() {
        assert_eq!(paste_steps(&[SHIFT], INS, &[LSHIFT]), vec![(INS, false), (INS, true)]);
    }

    #[test]
    fn ctrl_shift_v_presses_missing_modifiers() {
        assert_eq!(
            paste_steps(&[CTRL, SHIFT], VK_V, &[]),
            vec![
                (CTRL, false),
                (SHIFT, false),
                (VK_V, false),
                (VK_V, true),
                (SHIFT, true),
                (CTRL, true),
            ]
        );
    }
}
```

- [ ] **Step 2: Run tests to verify they fail to compile** (`cargo test` in `src-tauri/`) — expected: compile error, `paste_steps` has the old signature.

- [ ] **Step 3: Implement the generalized builder**

Replace `paste_steps` and `ctrl_v` (imports gain `VK_MENU`, `VK_SHIFT`, `VK_INSERT`; doc comment carries over the lift/reuse/mask rules):

```rust
/// Map a specific left/right modifier VK to its generic class so a combo's
/// required modifiers (generic) can be matched against physically-held keys.
fn modifier_class(vk: u16) -> u16 {
    match vk {
        x if x == VK_LCONTROL.0 || x == VK_RCONTROL.0 => VK_CONTROL.0,
        x if x == VK_LSHIFT.0 || x == VK_RSHIFT.0 => VK_SHIFT.0,
        x if x == VK_LMENU.0 || x == VK_RMENU.0 => VK_MENU.0,
        x if x == VK_LWIN.0 || x == VK_RWIN.0 => VK_LWIN.0,
        other => other,
    }
}

/// The (virtual key, is_key_up) sequence for a paste combo that may fire
/// while the user physically holds modifier keys. Held modifiers the combo
/// *requires* are reused as-is (their logical state is never disturbed);
/// held modifiers it does *not* require are lifted first and re-pressed at
/// the end; required modifiers not held are pressed around the key. A
/// restored Alt/Win appends a masking keystroke so the user's eventual
/// physical release doesn't read as a lone modifier tap.
fn paste_steps(required: &[u16], key: u16, held: &[u16]) -> Vec<(u16, bool)> {
    let lifted: Vec<u16> = held
        .iter()
        .copied()
        .filter(|&vk| !required.contains(&modifier_class(vk)))
        .collect();
    let missing: Vec<u16> = required
        .iter()
        .copied()
        .filter(|&req| !held.iter().any(|&h| modifier_class(h) == req))
        .collect();

    let mut steps = Vec::new();
    for &vk in &lifted {
        steps.push((vk, true));
    }
    for &vk in &missing {
        steps.push((vk, false));
    }
    steps.push((key, false));
    steps.push((key, true));
    for &vk in missing.iter().rev() {
        steps.push((vk, true));
    }
    for &vk in &lifted {
        steps.push((vk, false));
    }
    if needs_mask(&lifted) {
        steps.push((VK_MASK, false));
        steps.push((VK_MASK, true));
    }
    steps
}

/// Send the given paste combo, accounting for physically-held modifiers.
/// One SendInput batch: the lift/restore around the paste is atomic.
pub fn send_paste(required: &[u16], key: u16) {
    let held = held_of(&ALL_MODIFIERS);
    let inputs: Vec<INPUT> = paste_steps(required, key, &held)
        .into_iter()
        .map(|(vk, up)| key_vk(VIRTUAL_KEY(vk), up))
        .collect();
    send(&inputs);
}
```

Delete `ctrl_v()` and the now-unused `is_down` call sites it owned (`is_down` itself stays — `held_of` uses it). `VK_V` const stays. Note `mod.rs` still calls `ctrl_v()` until Task 3; to keep the tree compiling per-task, add a temporary shim in this task:

```rust
/// Transitional alias until expansion::paste_via_clipboard passes a combo.
pub fn ctrl_v() {
    send_paste(&[VK_CONTROL.0], VK_V);
}
```

- [ ] **Step 4: Run `cargo test`** — expected: all pass (engine, snippets, storage, inject).

---

### Task 2: Clipboard snapshot / restore / sequence number

**Files:**
- Modify: `src-tauri/src/expansion/clipboard.rs`

**Interfaces:**
- Produces: `pub struct Snapshot` (opaque, `Send`), `pub fn snapshot() -> Snapshot`, `pub fn restore(snapshot: &Snapshot)`, `pub fn sequence_number() -> u32`. Existing `get_unicode_text`/`set_unicode_text` unchanged (`get_unicode_text` becomes test/fallback-only — keep it, `set_unicode_text` still sets the expansion).

- [ ] **Step 1: Implement**

Add imports: `EnumClipboardFormats`, `GetClipboardSequenceNumber` (DataExchange); `GlobalFree`, `GlobalSize` (Memory). Append:

```rust
/// A saved copy of every byte-copyable clipboard format, so an expansion can
/// put back exactly what the user had — text, HTML, RTF, images (CF_DIB),
/// copied files (CF_HDROP) — not just plain text.
pub struct Snapshot {
    formats: Vec<(u32, Vec<u8>)>,
}

/// System-wide clipboard change counter (no clipboard open needed). If it
/// hasn't moved since we set the expansion text, the clipboard is still ours
/// to restore.
pub fn sequence_number() -> u32 {
    unsafe { GetClipboardSequenceNumber() }
}

/// Cap the total snapshot size; a multi-hundred-MB copied video frame is not
/// worth stalling expansion for.
const SNAPSHOT_CAP_BYTES: usize = 32 * 1024 * 1024;

/// GDI-handle and owner-drawn formats can't be byte-copied; the private and
/// GDI-object ranges are only meaningful to their owner process.
fn is_copyable_format(fmt: u32) -> bool {
    const CF_BITMAP: u32 = 2;
    const CF_METAFILEPICT: u32 = 3;
    const CF_PALETTE: u32 = 9;
    const CF_ENHMETAFILE: u32 = 14;
    const CF_OWNERDISPLAY: u32 = 0x0080;
    const CF_DSPBITMAP: u32 = 0x0082;
    const CF_DSPMETAFILEPICT: u32 = 0x0083;
    const CF_DSPENHMETAFILE: u32 = 0x008E;
    const CF_PRIVATEFIRST: u32 = 0x0200;
    const CF_GDIOBJLAST: u32 = 0x03FF;
    !matches!(
        fmt,
        CF_BITMAP
            | CF_METAFILEPICT
            | CF_PALETTE
            | CF_ENHMETAFILE
            | CF_OWNERDISPLAY
            | CF_DSPBITMAP
            | CF_DSPMETAFILEPICT
            | CF_DSPENHMETAFILE
    ) && !(CF_PRIVATEFIRST..=CF_GDIOBJLAST).contains(&fmt)
}

pub fn snapshot() -> Snapshot {
    let mut formats = Vec::new();
    if !open() {
        return Snapshot { formats };
    }
    let mut total = 0usize;
    unsafe {
        let mut fmt = EnumClipboardFormats(0);
        while fmt != 0 {
            if is_copyable_format(fmt) {
                // Delayed-render entries whose owner is gone return Err; skip.
                if let Ok(handle) = GetClipboardData(fmt) {
                    let hglobal = HGLOBAL(handle.0);
                    let size = GlobalSize(hglobal);
                    if size > 0 && total + size <= SNAPSHOT_CAP_BYTES {
                        let ptr = GlobalLock(hglobal) as *const u8;
                        if !ptr.is_null() {
                            let bytes = std::slice::from_raw_parts(ptr, size).to_vec();
                            let _ = GlobalUnlock(hglobal);
                            total += bytes.len();
                            formats.push((fmt, bytes));
                        }
                    } else if size > 0 {
                        crate::logging::info(&format!(
                            "clipboard snapshot cap reached; dropping format {fmt}"
                        ));
                    }
                }
            }
            fmt = EnumClipboardFormats(fmt);
        }
        let _ = CloseClipboard();
    }
    Snapshot { formats }
}

/// Put a snapshot back. An empty snapshot restores an empty clipboard —
/// never an empty *string* over whatever synthesized content remained.
pub fn restore(snapshot: &Snapshot) {
    if !open() {
        return;
    }
    unsafe {
        if EmptyClipboard().is_err() {
            let _ = CloseClipboard();
            return;
        }
        for (fmt, bytes) in &snapshot.formats {
            if let Ok(hglobal) = GlobalAlloc(GMEM_MOVEABLE, bytes.len()) {
                let dst = GlobalLock(hglobal) as *mut u8;
                if !dst.is_null() {
                    std::ptr::copy_nonoverlapping(bytes.as_ptr(), dst, bytes.len());
                    let _ = GlobalUnlock(hglobal);
                    if SetClipboardData(*fmt, HANDLE(hglobal.0)).is_err() {
                        // System didn't take ownership; don't leak.
                        let _ = GlobalFree(hglobal);
                    }
                }
            }
        }
        let _ = CloseClipboard();
    }
}
```

API-signature note: if `GlobalFree` in windows 0.58 wants `Option<HGLOBAL>` or `HGLOBAL` differs, follow the compiler — the intent is "free on failed SetClipboardData".

- [ ] **Step 2: `cargo check`** — expected: clean (these functions are unused until Task 3; if an unused warning appears, it disappears in Task 3 — do not add `#[allow]`).

---

### Task 3: Insertion policy in expansion/mod.rs

**Files:**
- Modify: `src-tauri/src/expansion/mod.rs`
- Modify: `src-tauri/src/expansion/inject.rs` (delete the `ctrl_v` shim)

**Interfaces:**
- Produces (all `pub`, used by storage/ipc/main):
  - `enum InsertMode { Auto, Paste, Type }` and `enum PasteCombo { CtrlV, ShiftInsert, CtrlShiftV }`, both `#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]` + `#[serde(rename_all = "snake_case")]`; `PasteCombo` also `impl Default` (CtrlV), `InsertMode` also `impl Default` (Auto).
  - `set_insert_mode(InsertMode)` / `insert_mode() -> InsertMode`
  - `set_paste_combo(PasteCombo)` / `paste_combo() -> PasteCombo`
  - `set_restore_delay_ms(u32)` / `restore_delay_ms() -> u32` (clamped 100–2000)
  - `RESTORE_DELAY_MIN_MS: u32 = 100`, `RESTORE_DELAY_MAX_MS: u32 = 2000`
  - `expand(trigger_char_len: usize, text: &str)` — mode parameter removed.
- Consumes: `inject::send_paste(&[u16], u16)`, `clipboard::{snapshot, restore, sequence_number}`.

- [ ] **Step 1: Write failing tests for `resolve` (pure) at the bottom of mod.rs**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_modes_ignore_content() {
        assert_eq!(resolve(InsertMode::Paste, "hi"), Mode::Clipboard);
        assert_eq!(resolve(InsertMode::Type, &"x".repeat(500)), Mode::Native);
    }

    #[test]
    fn auto_types_short_single_line() {
        assert_eq!(resolve(InsertMode::Auto, "Good morning"), Mode::Native);
        assert_eq!(resolve(InsertMode::Auto, &"x".repeat(40)), Mode::Native);
    }

    #[test]
    fn auto_pastes_long_text() {
        assert_eq!(resolve(InsertMode::Auto, &"x".repeat(41)), Mode::Clipboard);
    }

    #[test]
    fn auto_pastes_multiline() {
        // A typed '\n' presses Enter — in chat apps that *sends* the message.
        assert_eq!(resolve(InsertMode::Auto, "Best,\nDylan"), Mode::Clipboard);
    }

    #[test]
    fn restore_delay_is_clamped() {
        set_restore_delay_ms(1);
        assert_eq!(restore_delay_ms(), RESTORE_DELAY_MIN_MS);
        set_restore_delay_ms(99_999);
        assert_eq!(restore_delay_ms(), RESTORE_DELAY_MAX_MS);
        set_restore_delay_ms(800);
    }
}
```

- [ ] **Step 2: `cargo test`** — expected: compile failure (`resolve`, enums missing).

- [ ] **Step 3: Implement**

Module docs update (three modes incl. Auto). `Mode` gains `PartialEq, Eq, Debug`, loses `#[allow(dead_code)]`. Add after the WPM block:

```rust
use serde::{Deserialize, Serialize};

/// How expansions are inserted. Auto picks per-expansion: paste for long or
/// multiline text (a typed '\n' presses Enter — dangerous in chat apps),
/// type-out for short everyday snippets (no clipboard touched, works in apps
/// where the paste chord does something else).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InsertMode {
    #[default]
    Auto,
    Paste,
    Type,
}

/// The keystroke sent to make the target app paste. Terminals often bind
/// Ctrl+V to something else and paste on Shift+Insert or Ctrl+Shift+V.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PasteCombo {
    #[default]
    CtrlV,
    ShiftInsert,
    CtrlShiftV,
}

static INSERT_MODE: AtomicU8 = AtomicU8::new(0);
static PASTE_COMBO: AtomicU8 = AtomicU8::new(0);
static RESTORE_DELAY: AtomicU32 = AtomicU32::new(800);

pub const RESTORE_DELAY_MIN_MS: u32 = 100;
pub const RESTORE_DELAY_MAX_MS: u32 = 2000;

/// Auto mode pastes anything longer than this (typing 100 chars at 600 WPM
/// takes two visible seconds; paste is instant).
const AUTO_PASTE_THRESHOLD_CHARS: usize = 40;

pub fn set_insert_mode(mode: InsertMode) {
    INSERT_MODE.store(mode as u8, Ordering::Relaxed);
}

pub fn insert_mode() -> InsertMode {
    match INSERT_MODE.load(Ordering::Relaxed) {
        1 => InsertMode::Paste,
        2 => InsertMode::Type,
        _ => InsertMode::Auto,
    }
}

pub fn set_paste_combo(combo: PasteCombo) {
    PASTE_COMBO.store(combo as u8, Ordering::Relaxed);
}

pub fn paste_combo() -> PasteCombo {
    match PASTE_COMBO.load(Ordering::Relaxed) {
        1 => PasteCombo::ShiftInsert,
        2 => PasteCombo::CtrlShiftV,
        _ => PasteCombo::CtrlV,
    }
}

pub fn set_restore_delay_ms(ms: u32) {
    RESTORE_DELAY.store(ms.clamp(RESTORE_DELAY_MIN_MS, RESTORE_DELAY_MAX_MS), Ordering::Relaxed);
}

pub fn restore_delay_ms() -> u32 {
    RESTORE_DELAY.load(Ordering::Relaxed)
}

/// Pure mode decision, separated from the global so it can be unit-tested.
fn resolve(mode: InsertMode, text: &str) -> Mode {
    match mode {
        InsertMode::Paste => Mode::Clipboard,
        InsertMode::Type => Mode::Native,
        InsertMode::Auto => {
            if text.contains('\n') || text.chars().count() > AUTO_PASTE_THRESHOLD_CHARS {
                Mode::Clipboard
            } else {
                Mode::Native
            }
        }
    }
}
```

(`AtomicU8` joins the existing `AtomicU32` import.) Replace `expand` + `paste_via_clipboard`, and delete the `PASTE_SETTLE` const:

```rust
pub fn expand(trigger_char_len: usize, text: &str) {
    inject::backspaces(trigger_char_len);
    match resolve(insert_mode(), text) {
        Mode::Clipboard => paste_via_clipboard(text),
        Mode::Native => inject::type_unicode(text),
    }
}

fn paste_via_clipboard(text: &str) {
    // Snapshot everything byte-copyable (text, HTML, images, file lists) so
    // the user's clipboard survives the expansion intact.
    let saved = clipboard::snapshot();
    if !clipboard::set_unicode_text(text) {
        // Clipboard was unavailable; fall back to typing so the user still
        // gets their expansion rather than nothing.
        crate::logging::error("clipboard unavailable; expanding via direct typing");
        inject::type_unicode(text);
        return;
    }
    let seq = clipboard::sequence_number();
    let (required, key): (&[u16], u16) = match paste_combo() {
        PasteCombo::CtrlV => (&[inject::VK_CONTROL_U16], inject::VK_V),
        PasteCombo::ShiftInsert => (&[inject::VK_SHIFT_U16], inject::VK_INSERT_U16),
        PasteCombo::CtrlShiftV => (&[inject::VK_CONTROL_U16, inject::VK_SHIFT_U16], inject::VK_V),
    };
    inject::send_paste(required, key);
    // Restore on a detached thread so expansion never blocks. The sequence
    // number moving means someone (user copy, another app) claimed the
    // clipboard after us — then it is no longer ours to restore.
    let delay = Duration::from_millis(restore_delay_ms() as u64);
    std::thread::spawn(move || {
        std::thread::sleep(delay);
        if clipboard::sequence_number() == seq {
            clipboard::restore(&saved);
        }
    });
}
```

In `inject.rs`: delete the `ctrl_v` shim; export what the match needs —

```rust
pub const VK_V: u16 = 0x56;
pub const VK_CONTROL_U16: u16 = VK_CONTROL.0;
pub const VK_SHIFT_U16: u16 = VK_SHIFT.0;
pub const VK_INSERT_U16: u16 = VK_INSERT.0;
```

(If re-exporting consts this way reads noisy, an equally good shape is `pub fn combo_parts(combo: PasteCombo) -> (&'static [u16], u16)` inside inject.rs — pick one, keep the match total.)

- [ ] **Step 4: `cargo test`** — expected: new tests pass; engine/shortcuts/ipc/main do NOT compile yet (they call the old `expand(_, _, Mode)` / `set_type_out`). That is Task 4's job — run `cargo test -p hypertype --lib` only if the workspace fails hard, otherwise proceed straight to Task 4 and gate on its full `cargo test`.

---

### Task 4: Rewire callers (app_state, engine, shortcuts, ipc, main, storage)

**Files:**
- Modify: `src-tauri/src/app_state.rs` — delete the `type_out` field + its doc comment.
- Modify: `src-tauri/src/engine.rs`
- Modify: `src-tauri/src/shortcuts.rs`
- Modify: `src-tauri/src/storage.rs`
- Modify: `src-tauri/src/ipc.rs`
- Modify: `src-tauri/src/main.rs`

**Interfaces:**
- Consumes: everything Task 3 produced.
- Produces: `EngineHost::expand(&self, trigger_char_len: usize, text: &str)` (no flag); `AppSettings { insert_mode: InsertMode, wpm: u32, paste_combo: PasteCombo, restore_delay_ms: u32 }`; IPC commands `set_insert_mode(mode: InsertMode)`, `set_paste_combo(combo: PasteCombo)`, `set_restore_delay_ms(delay_ms: u32)`; `Status` gains `insert_mode`, `paste_combo`, `restore_delay_ms`, drops `type_out`.

- [ ] **Step 1: Write failing storage migration tests** (append to `storage.rs` tests)

```rust
#[test]
fn settings_migrate_legacy_type_out_true() {
    let path = temp_path("settings_legacy_true");
    fs::write(&path, r#"{"type_out": true, "wpm": 900}"#).unwrap();
    let s = load_settings(&path);
    assert_eq!(s.insert_mode, InsertMode::Type);
    assert_eq!(s.wpm, 900);
    assert_eq!(s.paste_combo, PasteCombo::CtrlV);
    assert_eq!(s.restore_delay_ms, 800);
    fs::remove_file(&path).ok();
}

#[test]
fn settings_migrate_legacy_type_out_false() {
    let path = temp_path("settings_legacy_false");
    fs::write(&path, r#"{"type_out": false, "wpm": 600}"#).unwrap();
    assert_eq!(load_settings(&path).insert_mode, InsertMode::Paste);
    fs::remove_file(&path).ok();
}

#[test]
fn settings_missing_file_defaults_to_auto() {
    let s = load_settings(Path::new("Z:/nonexistent/settings.json"));
    assert_eq!(s.insert_mode, InsertMode::Auto);
    assert_eq!(s.wpm, 600);
    assert_eq!(s.restore_delay_ms, 800);
}

#[test]
fn settings_unknown_enum_falls_back_without_losing_others() {
    let path = temp_path("settings_unknown_enum");
    fs::write(&path, r#"{"insert_mode": "telepathy", "wpm": 1200}"#).unwrap();
    let s = load_settings(&path);
    assert_eq!(s.insert_mode, InsertMode::Auto);
    assert_eq!(s.wpm, 1200);
    fs::remove_file(&path).ok();
}

#[test]
fn settings_round_trip() {
    let path = temp_path("settings_roundtrip");
    let out = AppSettings {
        insert_mode: InsertMode::Paste,
        wpm: 700,
        paste_combo: PasteCombo::ShiftInsert,
        restore_delay_ms: 400,
    };
    save_settings(&path, &out).unwrap();
    let s = load_settings(&path);
    assert_eq!(s.insert_mode, InsertMode::Paste);
    assert_eq!(s.wpm, 700);
    assert_eq!(s.paste_combo, PasteCombo::ShiftInsert);
    assert_eq!(s.restore_delay_ms, 400);
    fs::remove_file(&path).ok();
}
```

- [ ] **Step 2: Implement `storage.rs`**

```rust
use crate::expansion::{InsertMode, PasteCombo};

#[derive(Serialize, Clone, Copy)]
pub struct AppSettings {
    pub insert_mode: InsertMode,
    pub wpm: u32,
    pub paste_combo: PasteCombo,
    pub restore_delay_ms: u32,
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            insert_mode: InsertMode::default(),
            wpm: 600,
            paste_combo: PasteCombo::default(),
            restore_delay_ms: 800,
        }
    }
}

/// On-disk shape, every field optional: written by any past or future
/// version. `type_out` is the pre-InsertMode boolean; enum fields arrive as
/// raw JSON so one unknown value falls back alone instead of discarding the
/// whole file.
#[derive(Deserialize, Default)]
#[serde(default)]
struct StoredSettings {
    type_out: Option<bool>,
    insert_mode: Option<serde_json::Value>,
    wpm: Option<u32>,
    paste_combo: Option<serde_json::Value>,
    restore_delay_ms: Option<u32>,
}

/// Load settings; a missing or unreadable file means the defaults. A legacy
/// explicit `type_out` maps to Paste/Type (the user chose it); only a file
/// with neither field gets the new Auto default.
pub fn load_settings(path: &Path) -> AppSettings {
    let Some(stored) = fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<StoredSettings>(&text).ok())
    else {
        return AppSettings::default();
    };
    let defaults = AppSettings::default();
    let insert_mode = stored
        .insert_mode
        .and_then(|v| serde_json::from_value::<InsertMode>(v).ok())
        .unwrap_or(match stored.type_out {
            Some(true) => InsertMode::Type,
            Some(false) => InsertMode::Paste,
            None => defaults.insert_mode,
        });
    AppSettings {
        insert_mode,
        wpm: stored.wpm.unwrap_or(defaults.wpm),
        paste_combo: stored
            .paste_combo
            .and_then(|v| serde_json::from_value::<PasteCombo>(v).ok())
            .unwrap_or(defaults.paste_combo),
        restore_delay_ms: stored.restore_delay_ms.unwrap_or(defaults.restore_delay_ms),
    }
}
```

Delete `default_type_out`/`default_wpm` fns (fold values into `Default`). `save_settings` unchanged.

- [ ] **Step 3: Rewire engine.rs, shortcuts.rs, ipc.rs, main.rs**

engine.rs — trait + host + call site + tests:

```rust
fn expand(&self, trigger_char_len: usize, text: &str);
// WinHost:
fn expand(&self, trigger_char_len: usize, text: &str) {
    expansion::expand(trigger_char_len, text);
}
// try_expand:
host.expand(trigger_len, &expansion);
```

Drop the `expansion::{self, Mode}` import down to `expansion`; in tests, MockHost's `expand` drops the flag and `state_with` drops `type_out`.

shortcuts.rs — the handler body between the password check and the spawn shrinks to:

```rust
// Typed-out expansion takes visible time; run it off the main thread
// so the event loop (tray, window) never stalls while it types.
let expansion = expansion.clone();
std::thread::spawn(move || {
    expansion::expand(0, &expansion);
    crate::logging::info("expanded shortcut trigger");
});
```

(import shrinks to `use crate::expansion;`).

ipc.rs — Status + commands:

```rust
use crate::expansion::{self, InsertMode, PasteCombo};

#[derive(Serialize)]
pub struct Status {
    pub enabled: bool,
    pub count: usize,
    pub version: String,
    pub insert_mode: InsertMode,
    pub wpm: u32,
    pub paste_combo: PasteCombo,
    pub restore_delay_ms: u32,
}

#[tauri::command]
pub fn get_status(state: State<Arc<AppState>>) -> Status {
    let count = state.snippets.read().unwrap().len();
    Status {
        enabled: state.enabled.load(Ordering::Relaxed),
        count,
        version: env!("CARGO_PKG_VERSION").to_string(),
        insert_mode: expansion::insert_mode(),
        wpm: expansion::wpm(),
        paste_combo: expansion::paste_combo(),
        restore_delay_ms: expansion::restore_delay_ms(),
    }
}

/// Settings live as expansion-module atomics; persistence just reads them
/// back. No AppState involved.
fn persist_settings() {
    let settings = storage::AppSettings {
        insert_mode: expansion::insert_mode(),
        wpm: expansion::wpm(),
        paste_combo: expansion::paste_combo(),
        restore_delay_ms: expansion::restore_delay_ms(),
    };
    if let Err(e) = storage::save_settings(&storage::settings_file_path(), &settings) {
        crate::logging::error(&format!("failed to persist settings: {e}"));
    }
}

#[tauri::command]
pub fn set_insert_mode(mode: InsertMode) {
    expansion::set_insert_mode(mode);
    persist_settings();
}

#[tauri::command]
pub fn set_wpm(wpm: u32) {
    expansion::set_wpm(wpm);
    persist_settings();
}

#[tauri::command]
pub fn set_paste_combo(combo: PasteCombo) {
    expansion::set_paste_combo(combo);
    persist_settings();
}

#[tauri::command]
pub fn set_restore_delay_ms(delay_ms: u32) {
    expansion::set_restore_delay_ms(delay_ms);
    persist_settings();
}
```

Delete `set_type_out`.

main.rs — startup + handlers:

```rust
let settings = storage::load_settings(&storage::settings_file_path());
expansion::set_insert_mode(settings.insert_mode);
expansion::set_wpm(settings.wpm);
expansion::set_paste_combo(settings.paste_combo);
expansion::set_restore_delay_ms(settings.restore_delay_ms);
let state = Arc::new(AppState {
    snippets: RwLock::new(snippets),
    enabled: AtomicBool::new(true),
    data_path,
});
```

(add `use expansion;`-path calls via the existing `mod expansion;`), and in `generate_handler!`: replace `ipc::set_type_out` with `ipc::set_insert_mode, ipc::set_paste_combo, ipc::set_restore_delay_ms`.

- [ ] **Step 4: `cargo test`** — expected: everything compiles; all tests pass (inject 8, expansion 5, storage 8, engine 11, snippets suite).

---

### Task 5: Frontend — settings UI, IPC types, 3-way segment CSS

**Files:**
- Modify: `src/lib/ipc.ts`
- Modify: `src/App.tsx`
- Modify: `src/styles.css`

**Interfaces:**
- Consumes: IPC commands from Task 4 (`set_insert_mode {mode}`, `set_paste_combo {combo}`, `set_restore_delay_ms {delayMs}` — Tauri camelCases `delay_ms`).

- [ ] **Step 1: ipc.ts**

```ts
export type InsertMode = "auto" | "paste" | "type";
export type PasteCombo = "ctrl_v" | "shift_insert" | "ctrl_shift_v";

export interface Status {
  enabled: boolean;
  count: number;
  version: string;
  insert_mode: InsertMode;
  wpm: number;
  paste_combo: PasteCombo;
  restore_delay_ms: number;
}
```

Api gains `setInsertMode(mode: InsertMode)`, `setPasteCombo(combo: PasteCombo)`, `setRestoreDelay(delayMs: number)`; drops `setTypeOut`. Tauri impl:

```ts
setInsertMode: (mode) => invoke<void>("set_insert_mode", { mode }),
setPasteCombo: (combo) => invoke<void>("set_paste_combo", { combo }),
setRestoreDelay: (delayMs) => invoke<void>("set_restore_delay_ms", { delayMs }),
```

browserMock: replace `typeOut` with `insertMode: InsertMode = "auto"`, add `pasteCombo: PasteCombo = "ctrl_v"`, `restoreDelayMs = 800`; mirror all three setters and the new Status fields.

- [ ] **Step 2: App.tsx**

Replace `setInsert` with (note: the composer's `mode` signal keeps its name; the new helpers are insert-prefixed):

```tsx
const insertMode = () => status()?.insert_mode;

async function changeInsertMode(next: InsertMode) {
  const s = status();
  if (s) mutateStatus({ ...s, insert_mode: next });
  try {
    await api.setInsertMode(next);
  } catch {
    refetchStatus();
  }
}

async function changePasteCombo(next: PasteCombo) {
  const s = status();
  if (s) mutateStatus({ ...s, paste_combo: next });
  try {
    await api.setPasteCombo(next);
  } catch {
    refetchStatus();
  }
}

const [restoreDrag, setRestoreDrag] = createSignal<number | null>(null);
const restoreMs = () => restoreDrag() ?? status()?.restore_delay_ms ?? 800;

async function commitRestoreDelay(value: number) {
  setRestoreDrag(null);
  const s = status();
  if (s) mutateStatus({ ...s, restore_delay_ms: value });
  try {
    await api.setRestoreDelay(value);
  } catch {
    refetchStatus();
  }
}

const INSERT_MODES: InsertMode[] = ["auto", "paste", "type"];
const PASTE_COMBOS: PasteCombo[] = ["ctrl_v", "shift_insert", "ctrl_shift_v"];
const INSERT_SUB: Record<InsertMode, string> = {
  auto: "Short snippets are typed out, long ones pasted",
  paste: "Expansions are pasted instantly",
  type: "Expansions are typed out key by key",
};
```

Insert Method row becomes a 3-way segment (`data-pos` drives the thumb):

```tsx
<div class="setting-row">
  <div class="setting-text">
    <span class="setting-title">Insert Method</span>
    <span class="setting-sub">{INSERT_SUB[insertMode() ?? "auto"]}</span>
  </div>
  <div
    class="seg seg-mini seg-3"
    role="group"
    aria-label="Insert method"
    data-pos={INSERT_MODES.indexOf(insertMode() ?? "auto")}
  >
    <span class="seg-thumb" aria-hidden="true" />
    <For each={INSERT_MODES}>
      {(m) => (
        <button
          type="button"
          classList={{ active: insertMode() === m }}
          onClick={() => changeInsertMode(m)}
        >
          {m === "auto" ? "Auto" : m === "paste" ? "Paste" : "Type"}
        </button>
      )}
    </For>
  </div>
</div>
```

Typing Speed row: `<Show when={insertMode() && insertMode() !== "paste"}>` (was `type_out`). After it, two new rows:

```tsx
<Show when={insertMode() && insertMode() !== "type"}>
  <div class="setting-row">
    <div class="setting-text">
      <span class="setting-title">Paste Shortcut</span>
      <span class="setting-sub">What HyperType presses to paste — terminals often use Shift+Ins</span>
    </div>
    <div
      class="seg seg-mini seg-3 seg-combo"
      role="group"
      aria-label="Paste shortcut"
      data-pos={PASTE_COMBOS.indexOf(status()?.paste_combo ?? "ctrl_v")}
    >
      <span class="seg-thumb" aria-hidden="true" />
      <For each={PASTE_COMBOS}>
        {(c) => (
          <button
            type="button"
            classList={{ active: status()?.paste_combo === c }}
            onClick={() => changePasteCombo(c)}
          >
            {c === "ctrl_v" ? "Ctrl+V" : c === "shift_insert" ? "Shift+Ins" : "Ctrl+⇧+V"}
          </button>
        )}
      </For>
    </div>
  </div>
  <div class="setting-row">
    <div class="setting-text">
      <span class="setting-title">Clipboard Restore</span>
      <span class="setting-sub">
        {restoreMs()} ms until your old clipboard comes back — raise this if slow apps paste the wrong thing
      </span>
    </div>
    <input
      class="slider"
      type="range"
      min="100"
      max="2000"
      step="100"
      value={restoreMs()}
      aria-label="Clipboard restore delay in milliseconds"
      onInput={(e) => setRestoreDrag(Number(e.currentTarget.value))}
      onChange={(e) => commitRestoreDelay(Number(e.currentTarget.value))}
    />
  </div>
</Show>
```

Import `InsertMode`, `PasteCombo` types from `./lib/ipc`.

- [ ] **Step 3: styles.css — 3-column segment**

After the `.seg-mini button` rule:

```css
.seg-3 {
  grid-template-columns: 1fr 1fr 1fr;
  width: 180px;
}
.seg-3 .seg-thumb {
  width: calc(33.333% - 2px);
  transform: translateX(0);
}
.seg-3[data-pos="1"] .seg-thumb {
  transform: translateX(100%);
}
.seg-3[data-pos="2"] .seg-thumb {
  transform: translateX(200%);
}
.seg-combo {
  width: 224px;
}
```

- [ ] **Step 4: `pnpm build`** — expected: clean TypeScript build, no unused-symbol errors.

---

### Task 6: Full verification + project memory

- [ ] **Step 1:** `cargo test` (src-tauri) — all pass.
- [ ] **Step 2:** `pnpm build` — clean.
- [ ] **Step 3:** `pnpm tauri dev` compiles and launches; open the window and eyeball the Settings card: Auto/Paste/Type segment, conditional Typing Speed / Paste Shortcut / Clipboard Restore rows, thumb animation on all three segments. (Physical-typing e2e is the user's step; synthetic input is ignored by design.)
- [ ] **Step 4:** `memory_write` a handoff note: what changed, verification status, that physical typing + terminal paste-combo checks remain.
