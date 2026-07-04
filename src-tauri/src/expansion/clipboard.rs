//! Native clipboard access for the paste-based expansion mode: set the
//! expansion as CF_UNICODETEXT, and snapshot/restore *every* byte-copyable
//! format so the user's clipboard — text, HTML, RTF, images, copied files —
//! survives an expansion intact.

use windows::core::w;
use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, EnumClipboardFormats, GetClipboardData,
    GetClipboardSequenceNumber, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
};
use windows::Win32::System::Memory::{
    GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE,
};

const CF_TEXT: u32 = 1;
const CF_OEMTEXT: u32 = 7;
const CF_UNICODETEXT: u32 = 13;
const CF_LOCALE: u32 = 16;

/// OpenClipboard can transiently fail if another process holds it; retry briefly.
fn open() -> bool {
    for _ in 0..10 {
        unsafe {
            if OpenClipboard(HWND::default()).is_ok() {
                return true;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    false
}

/// A saved copy of every byte-copyable clipboard format, so an expansion can
/// put back exactly what the user had — text, HTML, RTF, images (CF_DIB),
/// copied files (CF_HDROP) — not just plain text.
pub struct Snapshot {
    formats: Vec<(u32, Vec<u8>)>,
}

impl Snapshot {
    pub fn contains_non_plain_text(&self) -> bool {
        self.formats
            .iter()
            .any(|(fmt, _)| !matches!(*fmt, CF_TEXT | CF_OEMTEXT | CF_UNICODETEXT | CF_LOCALE))
    }
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
/// GDI-object ranges are only meaningful to their owner process. Everything
/// else on the clipboard is an HGLOBAL whose bytes round-trip as-is —
/// including CF_DIB (which Windows synthesizes from CF_BITMAP and back).
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
                // Delayed-render entries whose owner is gone fail here; skip.
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
/// never an empty *string* over whatever content remained.
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
                        // The system didn't take ownership; don't leak.
                        let _ = GlobalFree(hglobal);
                    }
                }
            }
        }
        let _ = CloseClipboard();
    }
}

fn global_from_bytes(bytes: &[u8]) -> Option<HGLOBAL> {
    unsafe {
        let hglobal = GlobalAlloc(GMEM_MOVEABLE, bytes.len()).ok()?;
        let dst = GlobalLock(hglobal) as *mut u8;
        if dst.is_null() {
            let _ = GlobalFree(hglobal);
            return None;
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), dst, bytes.len());
        let _ = GlobalUnlock(hglobal);
        Some(hglobal)
    }
}

fn set_hglobal_format(format: u32, bytes: &[u8]) -> bool {
    let Some(hglobal) = global_from_bytes(bytes) else {
        return false;
    };
    unsafe {
        if SetClipboardData(format, HANDLE(hglobal.0)).is_err() {
            let _ = GlobalFree(hglobal);
            return false;
        }
    }
    true
}

fn ascii_fallback_bytes(text: &str) -> Vec<u8> {
    let mut bytes: Vec<u8> = text
        .chars()
        .map(|ch| if ch.is_ascii() { ch as u8 } else { b'?' })
        .collect();
    bytes.push(0);
    bytes
}

fn utf16_clipboard_bytes(text: &str) -> Vec<u8> {
    let mut units: Vec<u16> = text.encode_utf16().collect();
    units.push(0);
    units
        .into_iter()
        .flat_map(|unit| unit.to_le_bytes())
        .collect()
}

fn utf8_clipboard_bytes(text: &str) -> Vec<u8> {
    let mut bytes = text.as_bytes().to_vec();
    bytes.push(0);
    bytes
}

pub fn set_unicode_text(text: &str) -> bool {
    if !open() {
        return false;
    }
    let ok = unsafe {
        if EmptyClipboard().is_err() {
            let _ = CloseClipboard();
            return false;
        }

        let unicode_ok = set_hglobal_format(CF_UNICODETEXT, &utf16_clipboard_bytes(text));

        // Web/Electron paste handlers do not all query the same Windows
        // format. Advertise plain text redundantly so async readers see a
        // text paste, not an image/file paste fallback.
        let legacy = ascii_fallback_bytes(text);
        let _ = set_hglobal_format(CF_TEXT, &legacy);
        let _ = set_hglobal_format(CF_OEMTEXT, &legacy);

        let utf8 = utf8_clipboard_bytes(text);
        let text_plain = RegisterClipboardFormatW(w!("text/plain"));
        if text_plain != 0 {
            let _ = set_hglobal_format(text_plain, &utf8);
        }
        let utf8_plain = RegisterClipboardFormatW(w!("text/plain;charset=utf-8"));
        if utf8_plain != 0 {
            let _ = set_hglobal_format(utf8_plain, &utf8);
        }
        unicode_ok
    };
    unsafe {
        let _ = CloseClipboard();
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_clipboard_bytes_are_nul_terminated() {
        assert_eq!(utf16_clipboard_bytes("A"), vec![65, 0, 0, 0]);
    }

    #[test]
    fn utf8_clipboard_bytes_are_nul_terminated() {
        assert_eq!(utf8_clipboard_bytes("hi"), b"hi\0".to_vec());
    }

    #[test]
    fn legacy_bytes_keep_ascii_and_replace_unicode() {
        assert_eq!(ascii_fallback_bytes("hi\u{2026}"), b"hi?\0".to_vec());
    }
}
