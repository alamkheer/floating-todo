use windows::core::{w, PCWSTR, PWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Dwm::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::Controls::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::state::{AppState, ListItemRef};

// ── Control IDs ──
pub const IDC_INPUT: i32 = 1001;
pub const IDC_TASKLIST: i32 = 1002;
pub const IDC_BTN_PIN: i32 = 1003;
pub const IDC_BTN_CLOSE: i32 = 1004;
pub const IDC_BTN_FILTER_ALL: i32 = 1005;
pub const IDC_BTN_FILTER_ACTIVE: i32 = 1006;
pub const IDC_BTN_FILTER_DONE: i32 = 1007;
pub const IDC_BTN_VOICE: i32 = 1008;
pub const IDC_BTN_ADD_SUB: i32 = 1009;

// ── Layout constants (logical px at 96 DPI) ──
pub const HEADER_HEIGHT: i32 = 36;
pub const INPUT_HEIGHT: i32 = 30;
pub const FILTER_HEIGHT: i32 = 28;
pub const ITEM_HEIGHT: i32 = 38;
pub const MARGIN: i32 = 8;
pub const BTN_SIZE: i32 = 28;

// ── Colors ──
pub const CLR_BG_DARK: u32 = 0x001E1E1E;          // #1E1E1E
pub const CLR_HEADER_DARK: u32 = 0x00252526;       // #252526
pub const CLR_INPUT_BG: u32 = 0x002D2D2D;          // #2D2D2D
pub const CLR_TEXT: u32 = 0x00E0E0E0;              // #E0E0E0 (light gray)
pub const CLR_TEXT_DIM: u32 = 0x00808080;           // #808080 (dimmed)
pub const CLR_ACCENT: u32 = 0x00569CD6;            // #569CD6 (blue accent)
pub const CLR_CHECK: u32 = 0x006A9955;             // #6A9955 (green check)
pub const CLR_DELETE: u32 = 0x00D16969;            // #D16969 (red delete)
pub const CLR_RECORDING: u32 = 0x002B2BD1;         // #D12B2B (red recording)
pub const CLR_HOVER: u32 = 0x002A2D2E;             // Subtle hover highlight
pub const CLR_FILTER_ACTIVE_BG: u32 = CLR_ACCENT;  // Active filter button (blue accent)

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

    pub fn emoji_font(size: i32) -> Self {
        unsafe {
            let hfont = CreateFontW(
                size,
                0,
                0,
                0,
                400,
                0,
                0,
                0,
                DEFAULT_CHARSET,
                OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                CLEARTYPE_QUALITY,
                (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
                w!("Segoe UI Emoji"),
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

/// Create all child controls inside the main window with keyboard accessibility (WS_TABSTOP).
pub fn create_controls(hwnd: HWND, state: &mut AppState, hinstance: HINSTANCE, width: i32) {
    unsafe {
        let input_y = HEADER_HEIGHT + MARGIN;
        let voice_btn_w = 34;
        let input_w = width - MARGIN * 2 - voice_btn_w - 4;

        // ── 1. Input field (with WS_TABSTOP) ──
        state.hwnd_input = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("EDIT"),
            w!(""),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
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

        // ── 2. Voice Input Button 🎙️ (with WS_TABSTOP) ──
        state.hwnd_voice_btn = create_flat_button(
            hwnd,
            hinstance,
            "\u{1F399}", // 🎙
            IDC_BTN_VOICE,
            MARGIN + input_w + 4,
            input_y,
            voice_btn_w,
            INPUT_HEIGHT,
        );

        // ── 3. Filter & Add Sub Buttons ──
        let filter_y = input_y + INPUT_HEIGHT + MARGIN;
        let total_filter_w = width - MARGIN * 2;
        let gap = 2;
        let filter_w = (total_filter_w - gap * 3) / 4;

        state.hwnd_filter_all = create_flat_button(
            hwnd, hinstance, "All", IDC_BTN_FILTER_ALL,
            MARGIN, filter_y, filter_w, FILTER_HEIGHT,
        );
        state.hwnd_filter_active = create_flat_button(
            hwnd, hinstance, "Active", IDC_BTN_FILTER_ACTIVE,
            MARGIN + (filter_w + gap), filter_y, filter_w, FILTER_HEIGHT,
        );
        state.hwnd_filter_done = create_flat_button(
            hwnd, hinstance, "Done", IDC_BTN_FILTER_DONE,
            MARGIN + (filter_w + gap) * 2, filter_y, filter_w, FILTER_HEIGHT,
        );
        state.hwnd_add_sub_btn = create_flat_button(
            hwnd, hinstance, "＋ Sub", IDC_BTN_ADD_SUB,
            MARGIN + (filter_w + gap) * 3, filter_y, total_filter_w - (filter_w + gap) * 3, FILTER_HEIGHT,
        );

        // ── 4. Owner-drawn task list (with WS_TABSTOP) ──
        let list_y = filter_y + FILTER_HEIGHT + MARGIN;
        let list_h = 480 - list_y;

        state.hwnd_list = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("LISTBOX"),
            w!(""),
            WS_CHILD
                | WS_VISIBLE
                | WS_TABSTOP
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

        // ── 5. Header buttons (📌 Pin + ✕ Close) with WS_TABSTOP ──
        state.hwnd_pin_btn = create_flat_button(
            hwnd,
            hinstance,
            "\u{1F4CC}", // 📌
            IDC_BTN_PIN,
            width - BTN_SIZE * 2 - MARGIN,
            4,
            BTN_SIZE,
            BTN_SIZE,
        );
        state.hwnd_close_btn = create_flat_button(
            hwnd,
            hinstance,
            "\u{2715}", // ✕
            IDC_BTN_CLOSE,
            width - BTN_SIZE - 4,
            4,
            BTN_SIZE,
            BTN_SIZE,
        );

        // Subclass the EDIT control to capture Enter key
        let _ = SetWindowSubclass(
            state.hwnd_input,
            Some(edit_subclass_proc),
            1,
            0,
        );

        // ── 6. Info Popovers / Balloon Tooltips ──
        state.hwnd_tooltip = create_info_tooltips(hwnd, hinstance, state);
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
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_OWNERDRAW as u32),
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

/// Create native Windows balloon tooltips for buttons.
fn create_info_tooltips(parent: HWND, hinstance: HINSTANCE, state: &AppState) -> HWND {
    unsafe {
        let hwnd_tip = CreateWindowExW(
            WS_EX_TOPMOST,
            w!("tooltips_class32"),
            PCWSTR::null(),
            WS_POPUP | WINDOW_STYLE((TTS_NOPREFIX | TTS_ALWAYSTIP | TTS_BALLOON) as u32),
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            Some(parent),
            None,
            Some(hinstance),
            None,
        )
        .unwrap_or_default();

        let _ = SendMessageW(
            hwnd_tip,
            TTM_SETMAXTIPWIDTH,
            Some(WPARAM(0)),
            Some(LPARAM(260)),
        );

        add_tooltip_tool(
            hwnd_tip,
            parent,
            state.hwnd_pin_btn,
            "Always-on-Top (📌)\nClick or press Space to keep window visible above other apps.",
        );
        add_tooltip_tool(
            hwnd_tip,
            parent,
            state.hwnd_voice_btn,
            "Voice Input (🎙️)\nClick to record speech for up to 30s. Dictated text is added as a task.",
        );
        add_tooltip_tool(
            hwnd_tip,
            parent,
            state.hwnd_add_sub_btn,
            "Add Sub-task (＋)\nSelect an active task and click to nest a sub-todo under it.",
        );
        add_tooltip_tool(
            hwnd_tip,
            parent,
            state.hwnd_close_btn,
            "Minimize to Tray (✕)\nHides window to the Windows notification tray.",
        );

        hwnd_tip
    }
}

fn add_tooltip_tool(hwnd_tip: HWND, parent: HWND, target_hwnd: HWND, text: &str) {
    unsafe {
        let wide: &'static mut [u16] = Box::leak(
            text.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>().into_boxed_slice()
        );
        let mut ti = TTTOOLINFOW {
            cbSize: std::mem::size_of::<TTTOOLINFOW>() as u32,
            uFlags: TTF_SUBCLASS | TTF_IDISHWND,
            hwnd: parent,
            uId: target_hwnd.0 as usize,
            lpszText: PWSTR(wide.as_mut_ptr()),
            ..Default::default()
        };
        let _ = SendMessageW(
            hwnd_tip,
            TTM_ADDTOOLW,
            Some(WPARAM(0)),
            Some(LPARAM(&mut ti as *mut _ as isize)),
        );
    }
}

/// Custom message: Enter pressed in input field.
pub const WM_ADD_TASK: u32 = WM_APP + 100;

/// Subclass proc for the EDIT control — captures Enter key and Escape key.
unsafe extern "system" fn edit_subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _uid: usize,
    _data: usize,
) -> LRESULT {
    if msg == WM_KEYDOWN {
        if wparam.0 == VK_RETURN.0 as usize {
            if let Ok(parent) = GetParent(hwnd) {
                let _ = PostMessageW(Some(parent), WM_ADD_TASK, WPARAM(0), LPARAM(0));
                return LRESULT(0);
            }
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

/// Repopulate the listbox from the current filtered task and sub-task state.
pub fn refresh_task_list(state: &mut AppState) {
    unsafe {
        state.current_list = state.visible_items();

        // Clear existing items
        let _ = SendMessageW(state.hwnd_list, LB_RESETCONTENT, Some(WPARAM(0)), Some(LPARAM(0)));

        for (idx, item_ref) in state.current_list.iter().enumerate() {
            let label = match item_ref {
                ListItemRef::Parent(parent_id) => {
                    state.find_task(*parent_id).map(|t| t.text.clone()).unwrap_or_default()
                }
                ListItemRef::SubTask { parent_id, sub_id } => {
                    state.find_task(*parent_id)
                        .and_then(|t| t.sub_tasks.iter().find(|s| s.id == *sub_id))
                        .map(|s| s.text.clone())
                        .unwrap_or_default()
                }
            };
            let wide: Vec<u16> = label.encode_utf16().chain(std::iter::once(0)).collect();
            let list_idx = SendMessageW(
                state.hwnd_list,
                LB_ADDSTRING,
                Some(WPARAM(0)),
                Some(LPARAM(wide.as_ptr() as isize)),
            );
            let _ = SendMessageW(
                state.hwnd_list,
                LB_SETITEMDATA,
                Some(WPARAM(list_idx.0 as usize)),
                Some(LPARAM(idx as isize)),
            );
        }
    }
}

/// Handle WM_DRAWITEM for the owner-drawn listbox (tasks and sub-tasks).
pub fn draw_task_item(state: &AppState, dis: &DRAWITEMSTRUCT) {
    if dis.itemID == u32::MAX {
        return;
    }

    unsafe {
        let hdc = dis.hDC;
        let rc = dis.rcItem;
        let idx = dis.itemID as usize;

        let item_ref = match state.current_list.get(idx) {
            Some(r) => *r,
            None => return,
        };

        let is_hover = state.hover_item == dis.itemID as i32;
        let is_selected = (dis.itemState.0 & ODS_SELECTED.0) != 0;

        // Background
        let bg_color = if is_selected {
            CLR_HOVER
        } else if is_hover {
            CLR_HOVER
        } else {
            CLR_BG_DARK
        };
        let bg_brush = CreateSolidBrush(COLORREF(bg_color));
        FillRect(hdc, &rc, bg_brush);
        let _ = DeleteObject(bg_brush.into());

        match item_ref {
            ListItemRef::Parent(task_id) => {
                let task = match state.find_task(task_id) {
                    Some(t) => t,
                    None => return,
                };

                // Checkbox
                let check_x = rc.left + 10;
                let check_y = rc.top + (rc.bottom - rc.top) / 2 - 8;
                let check_rc = RECT {
                    left: check_x,
                    top: check_y,
                    right: check_x + 16,
                    bottom: check_y + 16,
                };

                if task.completed {
                    let check_brush = CreateSolidBrush(COLORREF(CLR_CHECK));
                    FillRect(hdc, &check_rc, check_brush);
                    let _ = DeleteObject(check_brush.into());

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
                    let border_brush = CreateSolidBrush(COLORREF(CLR_TEXT_DIM));
                    FrameRect(hdc, &check_rc, border_brush);
                    let _ = DeleteObject(border_brush.into());
                }

                // Text & Subtask Count Badge
                let text_color = if task.completed { CLR_TEXT_DIM } else { CLR_TEXT };
                SetBkMode(hdc, TRANSPARENT);
                SetTextColor(hdc, COLORREF(text_color));

                let display_text = if !task.sub_tasks.is_empty() {
                    let completed_count = task.sub_tasks.iter().filter(|s| s.completed).count();
                    format!("{} ({}/{})", task.text, completed_count, task.sub_tasks.len())
                } else {
                    task.text.clone()
                };

                let mut text_rc = RECT {
                    left: rc.left + 34,
                    top: rc.top,
                    right: rc.right - 36,
                    bottom: rc.bottom,
                };
                let mut wide_text: Vec<u16> = display_text.encode_utf16().collect();
                DrawTextW(
                    hdc,
                    &mut wide_text,
                    &mut text_rc,
                    DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS,
                );

                // Strikethrough for completed task
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

                // Delete button area (right 36px)
                // Requirement 4: if one entry reached done. one small delete emoji to remove that entry.
                if task.completed {
                    // Always show small delete emoji on done entries
                    SetTextColor(hdc, COLORREF(CLR_DELETE));
                    let mut del_rc = RECT {
                        left: rc.right - 34,
                        top: rc.top,
                        right: rc.right - 4,
                        bottom: rc.bottom,
                    };
                    let mut del_text: Vec<u16> = "\u{1F5D1}".encode_utf16().collect(); // 🗑
                    DrawTextW(
                        hdc,
                        &mut del_text,
                        &mut del_rc,
                        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                    );
                } else if is_hover {
                    SetTextColor(hdc, COLORREF(CLR_DELETE));
                    let mut del_rc = RECT {
                        left: rc.right - 34,
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
            ListItemRef::SubTask { parent_id, sub_id } => {
                let task = match state.find_task(parent_id) {
                    Some(t) => t,
                    None => return,
                };
                let sub = match task.sub_tasks.iter().find(|s| s.id == sub_id) {
                    Some(s) => s,
                    None => return,
                };

                // Indentation branch glyph "└─"
                SetBkMode(hdc, TRANSPARENT);
                SetTextColor(hdc, COLORREF(CLR_TEXT_DIM));
                let mut branch_rc = RECT {
                    left: rc.left + 16,
                    top: rc.top,
                    right: rc.left + 36,
                    bottom: rc.bottom,
                };
                let mut branch_text: Vec<u16> = "└─".encode_utf16().collect();
                DrawTextW(
                    hdc,
                    &mut branch_text,
                    &mut branch_rc,
                    DT_LEFT | DT_VCENTER | DT_SINGLELINE,
                );

                // Sub-task Checkbox (at rc.left + 36)
                let check_x = rc.left + 38;
                let check_y = rc.top + (rc.bottom - rc.top) / 2 - 7;
                let check_rc = RECT {
                    left: check_x,
                    top: check_y,
                    right: check_x + 14,
                    bottom: check_y + 14,
                };

                if sub.completed {
                    let check_brush = CreateSolidBrush(COLORREF(CLR_CHECK));
                    FillRect(hdc, &check_rc, check_brush);
                    let _ = DeleteObject(check_brush.into());

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
                    let border_brush = CreateSolidBrush(COLORREF(CLR_TEXT_DIM));
                    FrameRect(hdc, &check_rc, border_brush);
                    let _ = DeleteObject(border_brush.into());
                }

                // Sub-task Text
                let text_color = if sub.completed { CLR_TEXT_DIM } else { CLR_TEXT };
                SetTextColor(hdc, COLORREF(text_color));
                let mut text_rc = RECT {
                    left: rc.left + 58,
                    top: rc.top,
                    right: rc.right - 36,
                    bottom: rc.bottom,
                };
                let mut wide_text: Vec<u16> = sub.text.encode_utf16().collect();
                DrawTextW(
                    hdc,
                    &mut wide_text,
                    &mut text_rc,
                    DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS,
                );

                // Strikethrough for completed sub-task
                if sub.completed {
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

                // Delete button for sub-task
                if sub.completed {
                    SetTextColor(hdc, COLORREF(CLR_DELETE));
                    let mut del_rc = RECT {
                        left: rc.right - 34,
                        top: rc.top,
                        right: rc.right - 4,
                        bottom: rc.bottom,
                    };
                    let mut del_text: Vec<u16> = "\u{1F5D1}".encode_utf16().collect(); // 🗑
                    DrawTextW(
                        hdc,
                        &mut del_text,
                        &mut del_rc,
                        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                    );
                } else if is_hover {
                    SetTextColor(hdc, COLORREF(CLR_DELETE));
                    let mut del_rc = RECT {
                        left: rc.right - 34,
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

        // Accessibility Focus Ring
        if (dis.itemState.0 & ODS_FOCUS.0) != 0 {
            let _ = DrawFocusRect(hdc, &rc);
        }
    }
}

/// Handle WM_DRAWITEM for owner-drawn buttons (Header, Filters, Voice, Add Sub).
pub fn draw_button(state: &AppState, dis: &DRAWITEMSTRUCT) {
    unsafe {
        let hdc = dis.hDC;
        let rc = dis.rcItem;
        let ctrl_id = dis.CtlID as i32;

        let bg_color = match ctrl_id {
            IDC_BTN_FILTER_ALL if state.filter == crate::state::Filter::All => CLR_FILTER_ACTIVE_BG,
            IDC_BTN_FILTER_ACTIVE if state.filter == crate::state::Filter::Active => CLR_FILTER_ACTIVE_BG,
            IDC_BTN_FILTER_DONE if state.filter == crate::state::Filter::Completed => CLR_FILTER_ACTIVE_BG,
            IDC_BTN_VOICE if state.is_recording_voice => CLR_RECORDING,
            IDC_BTN_PIN if state.is_topmost => CLR_HEADER_DARK,
            IDC_BTN_PIN | IDC_BTN_CLOSE => CLR_HEADER_DARK,
            _ => CLR_BG_DARK,
        };

        let bg_brush = CreateSolidBrush(COLORREF(bg_color));
        FillRect(hdc, &rc, bg_brush);
        let _ = DeleteObject(bg_brush.into());

        // Determine text color
        let is_active_sub_selected = state.selected_task_id
            .and_then(|id| state.find_task(id))
            .map(|t| !t.completed)
            .unwrap_or(false);

        let text_color = match ctrl_id {
            IDC_BTN_CLOSE => CLR_DELETE,
            IDC_BTN_PIN if state.is_topmost => CLR_ACCENT,
            IDC_BTN_PIN => CLR_TEXT_DIM,
            IDC_BTN_VOICE if state.is_recording_voice => 0x00FFFFFF,
            IDC_BTN_ADD_SUB if is_active_sub_selected => CLR_ACCENT,
            IDC_BTN_ADD_SUB => CLR_TEXT_DIM,
            _ => CLR_TEXT,
        };

        SetBkMode(hdc, TRANSPARENT);
        SetTextColor(hdc, COLORREF(text_color));

        // Get button text (or dynamic recording seconds)
        let display_text = if ctrl_id == IDC_BTN_VOICE && state.is_recording_voice {
            format!("{}s", state.voice_seconds_remaining)
        } else {
            let mut buf = [0u16; 64];
            let len = GetWindowTextW(dis.hwndItem, &mut buf);
            if len > 0 {
                String::from_utf16_lossy(&buf[..len as usize])
            } else {
                String::new()
            }
        };

        if !display_text.is_empty() {
            let mut wide_buf: Vec<u16> = display_text.encode_utf16().collect();
            let mut text_rc = rc;
            DrawTextW(
                hdc,
                &mut wide_buf,
                &mut text_rc,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );
        }

        // Accessibility Focus Rect
        if (dis.itemState.0 & ODS_FOCUS.0) != 0 {
            let _ = DrawFocusRect(hdc, &rc);
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

/// Set text and focus to input field.
pub fn set_input_text(hwnd_input: HWND, text: &str) {
    unsafe {
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let _ = SetWindowTextW(hwnd_input, PCWSTR(wide.as_ptr()));
        let _ = SetFocus(Some(hwnd_input));
    }
}
