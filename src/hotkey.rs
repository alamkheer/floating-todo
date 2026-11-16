use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

const HOTKEY_TOGGLE_ID: i32 = 1;

/// RAII wrapper for a registered global hotkey.
pub struct GlobalHotkey {
    hwnd: HWND,
    id: i32,
}

impl GlobalHotkey {
    /// Register Ctrl+Shift+T as a global hotkey.
    pub fn register(hwnd: HWND) -> Option<Self> {
        unsafe {
            let result = RegisterHotKey(
                Some(hwnd),
                HOTKEY_TOGGLE_ID,
                MOD_CONTROL | MOD_SHIFT,
                VK_T.0 as u32,
            );
            if result.is_ok() {
                Some(Self {
                    hwnd,
                    id: HOTKEY_TOGGLE_ID,
                })
            } else {
                None // Hotkey already in use by another app
            }
        }
    }
}

impl Drop for GlobalHotkey {
    fn drop(&mut self) {
        unsafe {
            let _ = UnregisterHotKey(Some(self.hwnd), self.id);
        }
    }
}

/// Handle WM_HOTKEY in the window procedure.
pub fn handle_hotkey(hwnd: HWND, hotkey_id: usize) {
    if hotkey_id == HOTKEY_TOGGLE_ID as usize {
        toggle_visibility(hwnd);
    }
}

/// Toggle window visibility: if visible → hide; if hidden → show & focus.
pub fn toggle_visibility(hwnd: HWND) {
    unsafe {
        if IsWindowVisible(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_HIDE);
        } else {
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = SetForegroundWindow(hwnd);
        }
    }
}
