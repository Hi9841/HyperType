//! Single-instance ownership. This uses the same Win32 protocol as
//! `tauri-plugin-single-instance` with the Tauri app identifier, so this
//! build and an installed v1.x build recognize each other: a second launch of
//! either one asks the running instance to show its window, then exits.

use windows::Win32::Foundation::{
    CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, HWND, LPARAM, LRESULT, WPARAM,
};
use windows::Win32::System::DataExchange::COPYDATASTRUCT;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{CreateMutexW, ReleaseMutex};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, FindWindowW, RegisterClassExW, SendMessageW,
    WM_COPYDATA, WNDCLASSEXW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP,
};
use windows::core::{HSTRING, PCWSTR};

use crate::events::{self, AppEvent};

/// `tauri.conf.json` identifier of v1.x. Changing it breaks the handshake
/// with installed v1.x builds.
const APP_ID: &str = "com.hypertype.desktop";
/// `dwData` tag the Tauri plugin puts on its `WM_COPYDATA` message.
const COPYDATA_TAG: usize = 1542;

pub enum Claim {
    /// This process owns HyperType. Keep the guard alive until exit.
    Primary(InstanceGuard),
    /// Another instance runs and was asked to show its window.
    Secondary,
}

pub struct InstanceGuard {
    mutex: HANDLE,
    window: HWND,
}

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.window);
            let _ = ReleaseMutex(self.mutex);
            let _ = CloseHandle(self.mutex);
        }
    }
}

/// Claim single-instance ownership. Call on the thread that runs the GPUI
/// message loop: the target window created here receives its messages there.
pub fn claim() -> Result<Claim, String> {
    let class_name = HSTRING::from(format!("{APP_ID}-sic"));
    let window_name = HSTRING::from(format!("{APP_ID}-siw"));
    let mutex_name = HSTRING::from(format!("{APP_ID}-sim"));

    unsafe {
        let mutex = CreateMutexW(None, true, &mutex_name).map_err(|e| e.to_string())?;
        // If the owner is still starting (no window yet), run anyway, as
        // v1.x does, rather than leave the user with no HyperType.
        if GetLastError() == ERROR_ALREADY_EXISTS
            && let Ok(target) = FindWindowW(&class_name, &window_name)
        {
            let args = std::env::args().collect::<Vec<_>>().join("|");
            let cwd = std::env::current_dir().unwrap_or_default();
            let data = format!("{}|{args}\0", cwd.display());
            let copy = COPYDATASTRUCT {
                dwData: COPYDATA_TAG,
                cbData: data.len() as u32,
                lpData: data.as_ptr() as *mut _,
            };
            SendMessageW(
                target,
                WM_COPYDATA,
                WPARAM(0),
                LPARAM(&copy as *const _ as isize),
            );
            let _ = CloseHandle(mutex);
            return Ok(Claim::Secondary);
        }

        let instance = GetModuleHandleW(None).map_err(|e| e.to_string())?;
        let class = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(window_proc),
            hInstance: instance.into(),
            lpszClassName: PCWSTR(class_name.as_ptr()),
            ..Default::default()
        };
        RegisterClassExW(&class);
        // A hidden top-level window, not a message-only one: FindWindowW does
        // not search message-only windows.
        let window = CreateWindowExW(
            WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
            &class_name,
            &window_name,
            WS_POPUP,
            0,
            0,
            0,
            0,
            None,
            None,
            instance,
            None,
        )
        .map_err(|e| e.to_string())?;
        Ok(Claim::Primary(InstanceGuard { mutex, window }))
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_COPYDATA {
        let copy = unsafe { &*(lparam.0 as *const COPYDATASTRUCT) };
        if copy.dwData == COPYDATA_TAG {
            hypertype_core::logging::info("second launch: showing the window");
            events::send(AppEvent::OpenWindow);
            return LRESULT(1);
        }
    }
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}
