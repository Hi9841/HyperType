//! Launch at login through the per-user Run key. The value name and data
//! match what v1.x (tauri-plugin-autostart) wrote, so the setting carries
//! over: `HyperType` = `<exe path> --minimized`.

use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, WIN32_ERROR};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SAM_FLAGS, REG_SZ, RegCloseKey,
    RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
};
use windows::core::w;

const RUN_KEY: windows::core::PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const VALUE: windows::core::PCWSTR = w!("HyperType");

pub fn is_enabled() -> bool {
    let Ok(key) = open(KEY_QUERY_VALUE) else {
        return false;
    };
    let found = unsafe { RegQueryValueExW(key, VALUE, None, None, None, None) }.is_ok();
    close(key);
    found
}

#[allow(dead_code)] // S5 Settings autostart switch
pub fn set_enabled(enabled: bool) -> Result<(), String> {
    let key = open(KEY_SET_VALUE)?;
    let result = if enabled {
        command_line().and_then(|command| {
            let data: Vec<u8> = command
                .encode_utf16()
                .chain(std::iter::once(0))
                .flat_map(u16::to_le_bytes)
                .collect();
            check(unsafe { RegSetValueExW(key, VALUE, 0, REG_SZ, Some(&data)) })
        })
    } else {
        match unsafe { RegDeleteValueW(key, VALUE) } {
            ERROR_FILE_NOT_FOUND => Ok(()),
            status => check(status),
        }
    };
    close(key);
    result
}

fn command_line() -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    Ok(format!("{} --minimized", exe.display()))
}

fn open(access: REG_SAM_FLAGS) -> Result<HKEY, String> {
    let mut key = HKEY::default();
    check(unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, RUN_KEY, 0, access, &mut key) })?;
    Ok(key)
}

fn close(key: HKEY) {
    let _ = unsafe { RegCloseKey(key) };
}

fn check(status: WIN32_ERROR) -> Result<(), String> {
    status
        .ok()
        .map_err(|e| format!("Could not update the Run key: {e}"))
}
