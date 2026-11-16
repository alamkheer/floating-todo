use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Dwm::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::Controls::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::state::AppState;

// ── Control IDs ──
pub const IDC_INPUT: i32 = 1001;
pub const IDC_TASKLIST: i32 = 1002;
pub const IDC_BTN_PIN: i32 = 1003;
pub const IDC_BTN_CLOSE: i32 = 1004;
pub const IDC_BTN_FILTER_ALL: i32 = 1005;
pub const IDC_BTN_FILTER_ACTIVE: i32 = 1006;
pub const IDC_BTN_FILTER_DONE: i32 = 1007;

// ── Layout constants (logical px at 96 DPI) ──
pub const HEADER_HEIGHT: i32 = 36;
const INPUT_HEIGHT: i32 = 30;
const FILTER_HEIGHT: i32 = 28;
const ITEM_HEIGHT: i32 = 40;
const MARGIN: i32 = 8;
const BTN_SIZE: i32 = 28;

// ── Colors ──
pub const CLR_BG_DARK: u32 = 0x001E1E1E;       // #1E1E1E
pub const CLR_HEADER_DARK: u32 = 0x00252526;    // #252526
pub const CLR_INPUT_BG: u32 = 0x002D2D2D;       // #2D2D2D
pub const CLR_TEXT: u32 = 0x00E0E0E0;           // #E0E0E0 (light gray)
pub const CLR_TEXT_DIM: u32 = 0x00808080;        // #808080 (dimmed)
pub const CLR_ACCENT: u32 = 0x00569CD6;         // #569CD6 (blue accent)
pub const CLR_CHECK: u32 = 0x006A9955;          // #6A9955 (green check)
pub const CLR_DELETE: u32 = 0x00D16969;          // #D16969 (red delete)
pub const CLR_HOVER: u32 = 0x002A2D2E;          // Subtle hover highlight
pub const CLR_FILTER_ACTIVE_BG: u32 = CLR_ACCENT; // Active filter button (blue accent)

/// RAII wrapper for an HFONT.
pub struct Font(pub HFONT);

impl Font {
    pub fn new(size: i32, bold: bool) -> Self {
        unsafe {
            let hfont = CreateFontW(
                size,
                0,
                0,
                0,
                if bold { 700 } else { 400 },
                0,
                0,
                0,
                DEFAULT_CHARSET,
                OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                CLEARTYPE_QUALITY,
                (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
                w!("Segoe UI"),
            );
            Self(hfont)
        }
    }

    pub fn handle(&self) -> HFONT {
        self.0
    }
}

impl Drop for Font {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            unsafe {
                let _ = DeleteObject(self.0.into());
            }
        }
    }
}

/// Apply dark mode to the window via DWM.
pub fn apply_dark_mode(hwnd: HWND, enabled: bool) {
    unsafe {
        let value: i32 = if enabled { 1 } else { 0 };
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &value as *const i32 as *const _,
            std::mem::size_of::<i32>() as u32,
        );
    }
}

/// Create all child controls inside the main window.
pub fn create_controls(hwnd: HWND, state: &mut AppState, hinstance: HINSTANCE, width: i32) {
    unsafe {
        let input_y = HEADER_HEIGHT + MARGIN;
        let input_w = width - MARGIN * 2;

        // ── Input field ──
        state.hwnd_input = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("EDIT"),
            w!(""),
            WS_CHILD | WS_VISIBLE | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
            MARGIN,
            input_y,
            input_w,
            INPUT_HEIGHT,
            Some(hwnd),
            Some(HMENU(IDC_INPUT as *mut _)),
            Some(hinstance),
            None,
        )
        .unwrap_or_default();

        // ── Filter buttons ──
        let filter_y = input_y + INPUT_HEIGHT + MARGIN;
        let filter_w = (input_w - 4) / 3; // 3 buttons with 2px gaps

        state.hwnd_filter_all = create_flat_button(
            hwnd, hinstance, "All", IDC_BTN_FILTER_ALL,
            MARGIN, filter_y, filter_w, FILTER_HEIGHT,
        );
        state.hwnd_filter_active = create_flat_button(
            hwnd, hinstance, "Active", IDC_BTN_FILTER_ACTIVE,
            MARGIN + filter_w + 2, filter_y, filter_w, FILTER_HEIGHT,
        );
        state.hwnd_filter_done = create_flat_button(
            hwnd, hinstance, "Done", IDC_BTN_FILTER_DONE,
            MARGIN + (filter_w + 2) * 2, filter_y, filter_w, FILTER_HEIGHT,
        );

        // ── Owner-drawn task list ──
        let list_y = filter_y + FILTER_HEIGHT + MARGIN;
        let list_h = 480 - list_y; // fill remaining height

        state.hwnd_list = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("LISTBOX"),
            w!(""),
            WS_CHILD
                | WS_VISIBLE
                | WS_VSCROLL
                | WINDOW_STYLE(
                    (LBS_OWNERDRAWFIXED
                        | LBS_HASSTRINGS
                        | LBS_NOINTEGRALHEIGHT
                        | LBS_NOTIFY) as u32,
                ),
            0,
            list_y,
            width,
            list_h,
            Some(hwnd),
            Some(HMENU(IDC_TASKLIST as *mut _)),
            Some(hinstance),
            None,
        )
        .unwrap_or_default();

        // ── Header buttons (Pin + Close) ──
        state.hwnd_pin_btn = create_flat_button(
            hwnd, hinstance, "PIN", IDC_BTN_PIN,
            width - BTN_SIZE * 2 - MARGIN, 4, BTN_SIZE, BTN_SIZE,
        );
        state.hwnd_close_btn = create_flat_button(
            hwnd, hinstance, "\u{2715}", IDC_BTN_CLOSE, // ✕
            width - BTN_SIZE - 4, 4, BTN_SIZE, BTN_SIZE,
        );

        // Subclass the EDIT control to capture Enter key
        let _ = SetWindowSubclass(
            state.hwnd_input,
            Some(edit_subclass_proc),
            1,
            0,
        );
    }
}

fn create_flat_button(
    parent: HWND,
    hinstance: HINSTANCE,
    text: &str,
    id: i32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) -> HWND {
    unsafe {
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("BUTTON"),
            PCWSTR(wide.as_ptr()),
            WS_CHILD | WS_VISIBLE | WINDOW_STYLE(BS_OWNERDRAW as u32),
            x,
            y,
            w,
            h,
            Some(parent),
            Some(HMENU(id as *mut _)),
            Some(hinstance),
            None,
        )
        .unwrap_or_default()
    }
}

/// Custom message: Enter pressed in input field.
pub const WM_ADD_TASK: u32 = WM_APP + 100;

/// Subclass proc for the EDIT control — captures Enter key.
unsafe extern "system" fn edit_subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _uid: usize,
    _data: usize,
) -> LRESULT {
    if msg == WM_KEYDOWN && wparam.0 == VK_RETURN.0 as usize {
        if let Ok(parent) = GetParent(hwnd) {
            let _ = PostMessageW(Some(parent), WM_ADD_TASK, WPARAM(0), LPARAM(0));
            return LRESULT(0);
        }
    }
    DefSubclassProc(hwnd, msg, wparam, lparam)
}

/// Apply a font to a window/control via WM_SETFONT.
pub fn set_font(hwnd: HWND, font: &Font) {
    unsafe {
        let _ = SendMessageW(
            hwnd,
            WM_SETFONT,
            Some(WPARAM(font.handle().0 as usize)),
            Some(LPARAM(1)), // redraw
        );
    }
}

/// Repopulate the listbox from the current filtered task state.
pub fn refresh_task_list(state: &AppState) {
    unsafe {
        // Clear existing items
        let _ = SendMessageW(state.hwnd_list, LB_RESETCONTENT, Some(WPARAM(0)), Some(LPARAM(0)));

        // Add visible tasks
        let visible = state.visible_tasks();
        for task in &visible {
            // Store task ID as item data; text is just for the string lookup
            let wide: Vec<u16> = task.text.encode_utf16().chain(std::iter::once(0)).collect();
            let idx = SendMessageW(
                state.hwnd_list,
                LB_ADDSTRING,
                Some(WPARAM(0)),
                Some(LPARAM(wide.as_ptr() as isize)),
            );
            let _ = SendMessageW(
                state.hwnd_list,
                LB_SETITEMDATA,
                Some(WPARAM(idx.0 as usize)),
                Some(LPARAM(task.id as isize)),
            );
        }
    }
}

/// Handle WM_DRAWITEM for the owner-drawn listbox.
pub fn draw_task_item(state: &AppState, dis: &DRAWITEMSTRUCT) {
    if dis.itemID == u32::MAX {
        return; // empty list focus rect
    }

    unsafe {
        let hdc = dis.hDC;
        let rc = dis.rcItem;
        let item_data = SendMessageW(
            state.hwnd_list,
            LB_GETITEMDATA,
            Some(WPARAM(dis.itemID as usize)),
            Some(LPARAM(0)),
        );
        let task_id = item_data.0 as u64;

        // Find the task
        let task = match state.tasks.iter().find(|t| t.id == task_id) {
            Some(t) => t,
            None => return,
        };

        // ── Background ──
        let is_hover = state.hover_item == dis.itemID as i32;
        let bg_color = if is_hover { CLR_HOVER } else { CLR_BG_DARK };
        let bg_brush = CreateSolidBrush(COLORREF(bg_color));
        FillRect(hdc, &rc, bg_brush);
        let _ = DeleteObject(bg_brush.into());

        // ── Checkbox area (left 36px) ──
        let check_x = rc.left + 10;
        let check_y = rc.top + (rc.bottom - rc.top) / 2 - 8;
        let check_rc = RECT {
            left: check_x,
            top: check_y,
            right: check_x + 16,
            bottom: check_y + 16,
        };

        if task.completed {
            // Filled green checkbox
            let check_brush = CreateSolidBrush(COLORREF(CLR_CHECK));
            FillRect(hdc, &check_rc, check_brush);
            let _ = DeleteObject(check_brush.into());
            // Draw checkmark "✓"
            SetBkMode(hdc, TRANSPARENT);
            SetTextColor(hdc, COLORREF(0x00FFFFFF));
            let mut text_rc = check_rc;
            let mut check_text: Vec<u16> = "✓".encode_utf16().collect();
            DrawTextW(
                hdc,
                &mut check_text,
                &mut text_rc,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );
        } else {
            // Empty bordered checkbox
            let border_brush = CreateSolidBrush(COLORREF(CLR_TEXT_DIM));
            FrameRect(hdc, &check_rc, border_brush);
            let _ = DeleteObject(border_brush.into());
        }

        // ── Task text ──
        let text_color = if task.completed {
            CLR_TEXT_DIM
        } else {
            CLR_TEXT
        };
        SetBkMode(hdc, TRANSPARENT);
        SetTextColor(hdc, COLORREF(text_color));

        let mut text_rc = RECT {
            left: rc.left + 36,
            top: rc.top,
            right: rc.right - 36,
            bottom: rc.bottom,
        };
        let mut wide_text: Vec<u16> = task.text.encode_utf16().collect();
        DrawTextW(
            hdc,
            &mut wide_text,
            &mut text_rc,
            DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS,
        );

        // Strikethrough for completed tasks
        if task.completed {
            let y_mid = (rc.top + rc.bottom) / 2;
            let mut sz = SIZE::default();
            let _ = GetTextExtentPoint32W(hdc, &wide_text, &mut sz);
            let line_end = (text_rc.left + sz.cx).min(text_rc.right);
            let pen = CreatePen(PS_SOLID, 1, COLORREF(CLR_TEXT_DIM));
            let old_pen = SelectObject(hdc, pen.into());
            let _ = MoveToEx(hdc, text_rc.left, y_mid, None);
            let _ = LineTo(hdc, line_end, y_mid);
            SelectObject(hdc, old_pen);
            let _ = DeleteObject(pen.into());
        }

        // ── Delete button area (right 36px) — only show on hover ──
        if is_hover {
            SetTextColor(hdc, COLORREF(CLR_DELETE));
            let mut del_rc = RECT {
                left: rc.right - 32,
                top: rc.top,
                right: rc.right - 4,
                bottom: rc.bottom,
            };
            let mut del_text: Vec<u16> = "✕".encode_utf16().collect();
            DrawTextW(
                hdc,
                &mut del_text,
                &mut del_rc,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );
        }
    }
}

/// Handle WM_DRAWITEM for owner-drawn buttons (header + filter buttons).
pub fn draw_button(state: &AppState, dis: &DRAWITEMSTRUCT) {
    unsafe {
        let hdc = dis.hDC;
        let rc = dis.rcItem;
        let ctrl_id = dis.CtlID as i32;

        // Determine background color
        let bg_color = match ctrl_id {
            IDC_BTN_FILTER_ALL if state.filter == crate::state::Filter::All => CLR_FILTER_ACTIVE_BG,
            IDC_BTN_FILTER_ACTIVE if state.filter == crate::state::Filter::Active => CLR_FILTER_ACTIVE_BG,
            IDC_BTN_FILTER_DONE if state.filter == crate::state::Filter::Completed => CLR_FILTER_ACTIVE_BG,
            IDC_BTN_PIN | IDC_BTN_CLOSE => CLR_HEADER_DARK,
            _ => CLR_BG_DARK,
        };

        let bg_brush = CreateSolidBrush(COLORREF(bg_color));
        FillRect(hdc, &rc, bg_brush);
        let _ = DeleteObject(bg_brush.into());

        // Draw text
        let text_color = if ctrl_id == IDC_BTN_CLOSE {
            CLR_DELETE
        } else if ctrl_id == IDC_BTN_PIN && !state.is_topmost {
            CLR_TEXT_DIM
        } else {
            CLR_TEXT
        };

        SetBkMode(hdc, TRANSPARENT);
        SetTextColor(hdc, COLORREF(text_color));

        // Get button text
        let mut buf = [0u16; 64];
        let len = GetWindowTextW(dis.hwndItem, &mut buf);
        if len > 0 {
            let mut text_rc = rc;
            DrawTextW(
                hdc,
                &mut buf[..len as usize],
                &mut text_rc,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );
        }
    }
}

/// Handle WM_MEASUREITEM for the owner-drawn listbox.
pub fn measure_item(mis: &mut MEASUREITEMSTRUCT) {
    mis.itemHeight = ITEM_HEIGHT as u32;
}

/// Get text from the input field.
pub fn get_input_text(hwnd_input: HWND) -> String {
    unsafe {
        let len = GetWindowTextLengthW(hwnd_input);
        if len == 0 {
            return String::new();
        }
        let mut buf = vec![0u16; (len + 1) as usize];
        GetWindowTextW(hwnd_input, &mut buf);
        String::from_utf16_lossy(&buf[..len as usize])
    }
}

/// Clear the input field.
pub fn clear_input(hwnd_input: HWND) {
    unsafe {
        let _ = SetWindowTextW(hwnd_input, w!(""));
    }
}
