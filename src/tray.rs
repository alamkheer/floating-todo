use std::mem;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

/// Custom message ID for tray icon callbacks.
pub const WM_TRAYICON: u32 = WM_APP + 1;
const TRAY_UID: u32 = 1;

// Context menu command IDs
pub const IDM_SHOW_HIDE: u16 = 100;
pub const IDM_TOGGLE_PIN: u16 = 101;
pub const IDM_EXIT: u16 = 102;

/// RAII wrapper for the system tray icon.
pub struct TrayIcon {
    nid: NOTIFYICONDATAW,
}

impl TrayIcon {
    pub fn new(hwnd: HWND) -> windows::core::Result<Self> {
        unsafe {
            let mut nid: NOTIFYICONDATAW = mem::zeroed();
            nid.cbSize = mem::size_of::<NOTIFYICONDATAW>() as u32;
            nid.hWnd = hwnd;
            nid.uID = TRAY_UID;
            nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
            nid.uCallbackMessage = WM_TRAYICON;
            nid.hIcon = LoadIconW(None, IDI_APPLICATION)?;

            // Set tooltip text with version
            let tip = format!("Floating TODO v{}\0", env!("CARGO_PKG_VERSION"));
            let utf16: Vec<u16> = tip.encode_utf16().collect();
            let len = utf16.len().min(nid.szTip.len());
            nid.szTip[..len].copy_from_slice(&utf16[..len]);

            let _ = Shell_NotifyIconW(NIM_ADD, &nid);

            Ok(Self { nid })
        }
    }
}

impl Drop for TrayIcon {
    fn drop(&mut self) {
        unsafe {
            let _ = Shell_NotifyIconW(NIM_DELETE, &self.nid);
        }
    }
}

/// Handle tray icon messages (called from WndProc for WM_TRAYICON).
pub fn handle_tray_message(hwnd: HWND, lparam: LPARAM) {
    let event = lparam.0 as u32 & 0xFFFF;
    match event {
        WM_LBUTTONUP => {
            crate::hotkey::toggle_visibility(hwnd);
        }
        WM_RBUTTONUP => {
            show_context_menu(hwnd);
        }
        _ => {}
    }
}

fn show_context_menu(hwnd: HWND) {
    unsafe {
        let menu = CreatePopupMenu().unwrap();
        let version_hdr = format!("Floating TODO v{}\0", env!("CARGO_PKG_VERSION"));
        let version_u16: Vec<u16> = version_hdr.encode_utf16().collect();
        let _ = AppendMenuW(
            menu,
            MF_GRAYED | MF_DISABLED,
            0,
            PCWSTR(version_u16.as_ptr()),
        );
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
        let _ = AppendMenuW(menu, MF_STRING, IDM_SHOW_HIDE as usize, w!("Show / Hide"));
        let _ = AppendMenuW(
            menu,
            MF_STRING,
            IDM_TOGGLE_PIN as usize,
            w!("Toggle Always-on-Top"),
        );
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
        let _ = AppendMenuW(menu, MF_STRING, IDM_EXIT as usize, w!("Exit"));

        // Required for popup menu to dismiss properly
        let _ = SetForegroundWindow(hwnd);

        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        let _ = TrackPopupMenu(menu, TPM_RIGHTALIGN | TPM_BOTTOMALIGN, pt.x, pt.y, Some(0), hwnd, None);

        let _ = DestroyMenu(menu);
    }
}

/// Handle WM_COMMAND for tray menu items (called from WndProc).
pub fn handle_menu_command(hwnd: HWND, cmd_id: u16) {
    match cmd_id {
        IDM_SHOW_HIDE => {
            crate::hotkey::toggle_visibility(hwnd);
        }
        IDM_TOGGLE_PIN => {
            if let Some(state) = unsafe { crate::window::get_state(hwnd) } {
                state.is_topmost = !state.is_topmost;
                let pos = if state.is_topmost {
                    HWND_TOPMOST
                } else {
                    HWND_NOTOPMOST
                };
                unsafe {
                    let _ = SetWindowPos(hwnd, Some(pos), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
                }
            }
        }
        IDM_EXIT => unsafe {
            let _ = DestroyWindow(hwnd);
        },
        _ => {}
    }
}
