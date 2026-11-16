use windows::core::{w, PCWSTR, PWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Dwm::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::Controls::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::state::{get_recent_logs, AppState, LogLevel, SettingsSubPage, ViewMode};

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
pub const IDC_BTN_SETTINGS: i32 = 1010;
pub const IDC_BTN_TAB_SETTINGS: i32 = 1011;
pub const IDC_BTN_TAB_INFO: i32 = 1012;
pub const IDC_BTN_TAB_LOGS: i32 = 1013;
pub const IDC_BTN_BACK: i32 = 1014;
pub const IDC_BTN_REC_7: i32 = 1015;
pub const IDC_BTN_REC_15: i32 = 1016;
pub const IDC_BTN_REC_30: i32 = 1017;
pub const IDC_BTN_DEPTH_1: i32 = 1018;
pub const IDC_BTN_DEPTH_2: i32 = 1019;
pub const IDC_BTN_DEPTH_3: i32 = 1020;
pub const IDC_BTN_TEST_MIC: i32 = 1021;
pub const IDC_BTN_CLEAR_LOGS: i32 = 1022;

// ── Custom window messages ──
pub const WM_ADD_TASK: u32 = WM_APP + 100;
pub const WM_LIST_INDENT: u32 = WM_APP + 106;
pub const WM_LIST_OUTDENT: u32 = WM_APP + 107;

// ── Layout constants (logical px at 96 DPI) ──
pub const HEADER_HEIGHT: i32 = 36;
pub const INPUT_HEIGHT: i32 = 30;
pub const FILTER_HEIGHT: i32 = 28;
pub const ITEM_HEIGHT: i32 = 38;
pub const FOOTER_HEIGHT: i32 = 22;
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
pub const CLR_WARN: u32 = 0x0000D7FF;              // Yellow/Gold warning
pub const CLR_RECORDING: u32 = 0x002B2BD1;         // #D12B2B (red recording)
pub const CLR_HOVER: u32 = 0x002A2D2E;             // Subtle hover highlight
pub const CLR_CARD_BG: u32 = 0x00272728;           // Card background

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
            hwnd,
            hinstance,
            "All",
            IDC_BTN_FILTER_ALL,
            MARGIN,
            filter_y,
            filter_w,
            FILTER_HEIGHT,
        );
        state.hwnd_filter_active = create_flat_button(
            hwnd,
            hinstance,
            "Active",
            IDC_BTN_FILTER_ACTIVE,
            MARGIN + (filter_w + gap),
            filter_y,
            filter_w,
            FILTER_HEIGHT,
        );
        state.hwnd_filter_done = create_flat_button(
            hwnd,
            hinstance,
            "Done",
            IDC_BTN_FILTER_DONE,
            MARGIN + (filter_w + gap) * 2,
            filter_y,
            filter_w,
            FILTER_HEIGHT,
        );
        state.hwnd_add_sub_btn = create_flat_button(
            hwnd,
            hinstance,
            "＋ Sub",
            IDC_BTN_ADD_SUB,
            MARGIN + (filter_w + gap) * 3,
            filter_y,
            total_filter_w - (filter_w + gap) * 3,
            FILTER_HEIGHT,
        );

        // ── 4. Owner-drawn task list (with WS_TABSTOP) ──
        let list_y = filter_y + FILTER_HEIGHT + MARGIN;
        let list_h = 480 - list_y - FOOTER_HEIGHT;

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

        // ── 5. Header buttons: 📌 Pin | ⚙ Settings | ✕ Close (with WS_TABSTOP) ──
        state.hwnd_pin_btn = create_flat_button(
            hwnd,
            hinstance,
            "\u{1F4CC}", // 📌
            IDC_BTN_PIN,
            width - BTN_SIZE * 3 - 12,
            4,
            BTN_SIZE,
            BTN_SIZE,
        );
        state.hwnd_settings_btn = create_flat_button(
            hwnd,
            hinstance,
            "\u{2699}", // ⚙
            IDC_BTN_SETTINGS,
            width - BTN_SIZE * 2 - 8,
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

        // ── 6. Settings View Navigation Tabs (App Settings | Info | Logs) ──
        let tab_y = HEADER_HEIGHT + MARGIN;
        let total_tab_w = width - MARGIN * 2;
        let tab_w = (total_tab_w - 4) / 3;

        state.hwnd_tab_settings = create_flat_button(
            hwnd,
            hinstance,
            "Settings",
            IDC_BTN_TAB_SETTINGS,
            MARGIN,
            tab_y,
            tab_w,
            FILTER_HEIGHT,
        );
        state.hwnd_tab_info = create_flat_button(
            hwnd,
            hinstance,
            "Info",
            IDC_BTN_TAB_INFO,
            MARGIN + tab_w + 2,
            tab_y,
            tab_w,
            FILTER_HEIGHT,
        );
        state.hwnd_tab_logs = create_flat_button(
            hwnd,
            hinstance,
            "Logs",
            IDC_BTN_TAB_LOGS,
            MARGIN + (tab_w + 2) * 2,
            tab_y,
            total_tab_w - (tab_w + 2) * 2,
            FILTER_HEIGHT,
        );

        // ── 7. Settings View Option Buttons ──
        let btn_rec_w = (total_tab_w - 4) / 3;
        state.hwnd_btn_rec_7 = create_flat_button(
            hwnd,
            hinstance,
            "7s (Default)",
            IDC_BTN_REC_7,
            MARGIN,
            106,
            btn_rec_w,
            FILTER_HEIGHT,
        );
        state.hwnd_btn_rec_15 = create_flat_button(
            hwnd,
            hinstance,
            "15s",
            IDC_BTN_REC_15,
            MARGIN + btn_rec_w + 2,
            106,
            btn_rec_w,
            FILTER_HEIGHT,
        );
        state.hwnd_btn_rec_30 = create_flat_button(
            hwnd,
            hinstance,
            "30s",
            IDC_BTN_REC_30,
            MARGIN + (btn_rec_w + 2) * 2,
            106,
            total_tab_w - (btn_rec_w + 2) * 2,
            FILTER_HEIGHT,
        );

        let btn_depth_w = (total_tab_w - 4) / 3;
        state.hwnd_btn_depth_1 = create_flat_button(
            hwnd,
            hinstance,
            "Depth 1",
            IDC_BTN_DEPTH_1,
            MARGIN,
            174,
            btn_depth_w,
            FILTER_HEIGHT,
        );
        state.hwnd_btn_depth_2 = create_flat_button(
            hwnd,
            hinstance,
            "Depth 2",
            IDC_BTN_DEPTH_2,
            MARGIN + btn_depth_w + 2,
            174,
            btn_depth_w,
            FILTER_HEIGHT,
        );
        state.hwnd_btn_depth_3 = create_flat_button(
            hwnd,
            hinstance,
            "Depth 3 (Max)",
            IDC_BTN_DEPTH_3,
            MARGIN + (btn_depth_w + 2) * 2,
            174,
            total_tab_w - (btn_depth_w + 2) * 2,
            FILTER_HEIGHT,
        );

        // ── 8. Action & Back Buttons ──
        let bottom_action_y = 416;
        let action_w = (total_tab_w - 4) / 3;
        state.hwnd_btn_test_mic = create_flat_button(
            hwnd,
            hinstance,
            "🎙️ Test Mic",
            IDC_BTN_TEST_MIC,
            MARGIN,
            bottom_action_y,
            action_w,
            30,
        );
        state.hwnd_btn_clear_logs = create_flat_button(
            hwnd,
            hinstance,
            "Clear Logs",
            IDC_BTN_CLEAR_LOGS,
            MARGIN + action_w + 2,
            bottom_action_y,
            action_w,
            30,
        );
        state.hwnd_btn_back = create_flat_button(
            hwnd,
            hinstance,
            "← Back to Tasks",
            IDC_BTN_BACK,
            MARGIN + (action_w + 2) * 2,
            bottom_action_y,
            total_tab_w - (action_w + 2) * 2,
            30,
        );

        // Subclass EDIT control to capture Enter key
        let _ = SetWindowSubclass(state.hwnd_input, Some(edit_subclass_proc), 1, 0);

        // Placeholder cue banner text for EDIT input
        set_cue_banner(state.hwnd_input, "Add a task... (Press Enter)");

        // ── 9. Tooltips ──
        state.hwnd_tooltip = create_info_tooltips(hwnd, hinstance, state);

        // Set initial visibility
        update_view_visibility(state);
    }
}

/// Updates show/hide state of child controls according to ViewMode.
pub fn update_view_visibility(state: &AppState) {
    unsafe {
        match state.view_mode {
            ViewMode::Tasks => {
                let _ = ShowWindow(state.hwnd_input, SW_SHOW);
                let _ = ShowWindow(state.hwnd_voice_btn, SW_SHOW);
                let _ = ShowWindow(state.hwnd_filter_all, SW_SHOW);
                let _ = ShowWindow(state.hwnd_filter_active, SW_SHOW);
                let _ = ShowWindow(state.hwnd_filter_done, SW_SHOW);
                let _ = ShowWindow(state.hwnd_add_sub_btn, SW_SHOW);
                let _ = ShowWindow(state.hwnd_list, SW_SHOW);

                let _ = ShowWindow(state.hwnd_tab_settings, SW_HIDE);
                let _ = ShowWindow(state.hwnd_tab_info, SW_HIDE);
                let _ = ShowWindow(state.hwnd_tab_logs, SW_HIDE);
                let _ = ShowWindow(state.hwnd_btn_rec_7, SW_HIDE);
                let _ = ShowWindow(state.hwnd_btn_rec_15, SW_HIDE);
                let _ = ShowWindow(state.hwnd_btn_rec_30, SW_HIDE);
                let _ = ShowWindow(state.hwnd_btn_depth_1, SW_HIDE);
                let _ = ShowWindow(state.hwnd_btn_depth_2, SW_HIDE);
                let _ = ShowWindow(state.hwnd_btn_depth_3, SW_HIDE);
                let _ = ShowWindow(state.hwnd_btn_test_mic, SW_HIDE);
                let _ = ShowWindow(state.hwnd_btn_clear_logs, SW_HIDE);
                let _ = ShowWindow(state.hwnd_btn_back, SW_HIDE);
            }
            ViewMode::Settings(sub_page) => {
                let _ = ShowWindow(state.hwnd_input, SW_HIDE);
                let _ = ShowWindow(state.hwnd_voice_btn, SW_HIDE);
                let _ = ShowWindow(state.hwnd_filter_all, SW_HIDE);
                let _ = ShowWindow(state.hwnd_filter_active, SW_HIDE);
                let _ = ShowWindow(state.hwnd_filter_done, SW_HIDE);
                let _ = ShowWindow(state.hwnd_add_sub_btn, SW_HIDE);
                let _ = ShowWindow(state.hwnd_list, SW_HIDE);

                let _ = ShowWindow(state.hwnd_tab_settings, SW_SHOW);
                let _ = ShowWindow(state.hwnd_tab_info, SW_SHOW);
                let _ = ShowWindow(state.hwnd_tab_logs, SW_SHOW);
                let _ = ShowWindow(state.hwnd_btn_back, SW_SHOW);

                // Reposition Back button dynamically
                let total_tab_w = 320 - MARGIN * 2;
                match sub_page {
                    SettingsSubPage::AppSettings => {
                        let _ = ShowWindow(state.hwnd_btn_rec_7, SW_SHOW);
                        let _ = ShowWindow(state.hwnd_btn_rec_15, SW_SHOW);
                        let _ = ShowWindow(state.hwnd_btn_rec_30, SW_SHOW);
                        let _ = ShowWindow(state.hwnd_btn_depth_1, SW_SHOW);
                        let _ = ShowWindow(state.hwnd_btn_depth_2, SW_SHOW);
                        let _ = ShowWindow(state.hwnd_btn_depth_3, SW_SHOW);
                        let _ = ShowWindow(state.hwnd_btn_test_mic, SW_HIDE);
                        let _ = ShowWindow(state.hwnd_btn_clear_logs, SW_HIDE);

                        let _ = SetWindowPos(
                            state.hwnd_btn_back,
                            None,
                            MARGIN,
                            416,
                            total_tab_w,
                            30,
                            SWP_NOZORDER | SWP_NOACTIVATE,
                        );
                    }
                    SettingsSubPage::Info => {
                        let _ = ShowWindow(state.hwnd_btn_rec_7, SW_HIDE);
                        let _ = ShowWindow(state.hwnd_btn_rec_15, SW_HIDE);
                        let _ = ShowWindow(state.hwnd_btn_rec_30, SW_HIDE);
                        let _ = ShowWindow(state.hwnd_btn_depth_1, SW_HIDE);
                        let _ = ShowWindow(state.hwnd_btn_depth_2, SW_HIDE);
                        let _ = ShowWindow(state.hwnd_btn_depth_3, SW_HIDE);
                        let _ = ShowWindow(state.hwnd_btn_test_mic, SW_HIDE);
                        let _ = ShowWindow(state.hwnd_btn_clear_logs, SW_HIDE);

                        let _ = SetWindowPos(
                            state.hwnd_btn_back,
                            None,
                            MARGIN,
                            416,
                            total_tab_w,
                            30,
                            SWP_NOZORDER | SWP_NOACTIVATE,
                        );
                    }
                    SettingsSubPage::Logs => {
                        let _ = ShowWindow(state.hwnd_btn_rec_7, SW_HIDE);
                        let _ = ShowWindow(state.hwnd_btn_rec_15, SW_HIDE);
                        let _ = ShowWindow(state.hwnd_btn_rec_30, SW_HIDE);
                        let _ = ShowWindow(state.hwnd_btn_depth_1, SW_HIDE);
                        let _ = ShowWindow(state.hwnd_btn_depth_2, SW_HIDE);
                        let _ = ShowWindow(state.hwnd_btn_depth_3, SW_HIDE);
                        let _ = ShowWindow(state.hwnd_btn_test_mic, SW_SHOW);
                        let _ = ShowWindow(state.hwnd_btn_clear_logs, SW_SHOW);

                        let action_w = (total_tab_w - 4) / 3;
                        let _ = SetWindowPos(
                            state.hwnd_btn_back,
                            None,
                            MARGIN + (action_w + 2) * 2,
                            416,
                            total_tab_w - (action_w + 2) * 2,
                            30,
                            SWP_NOZORDER | SWP_NOACTIVATE,
                        );
                    }
                }
            }
        }
    }
}

pub fn create_flat_button(
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
            "Always-on-Top (📌)\nKeep window floating above other applications.",
        );
        add_tooltip_tool(
            hwnd_tip,
            parent,
            state.hwnd_settings_btn,
            "Settings (⚙)\nModify record time, sub-level depth, view app info & recent logs.",
        );
        add_tooltip_tool(
            hwnd_tip,
            parent,
            state.hwnd_voice_btn,
            "Voice Input (🎙️)\nRecord speech into task. Default: 7s, configurable up to 30s.",
        );
        add_tooltip_tool(
            hwnd_tip,
            parent,
            state.hwnd_add_sub_btn,
            "Add Sub-task (＋)\nSelect an item and press to nest under it (or press Tab).",
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
            text.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>().into_boxed_slice(),
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

/// Subclass proc for the EDIT control — captures Enter key and delegates Tab navigation.
unsafe extern "system" fn edit_subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _uid: usize,
    _data: usize,
) -> LRESULT {
    match msg {
        WM_GETDLGCODE => {
            let base = DefSubclassProc(hwnd, msg, wparam, lparam).0;
            if lparam.0 != 0 {
                let pmsg = &*(lparam.0 as *const MSG);
                if pmsg.message == WM_KEYDOWN && pmsg.wParam.0 == VK_RETURN.0 as usize {
                    return LRESULT(base | (DLGC_WANTMESSAGE as isize));
                }
            }
            if wparam.0 == VK_RETURN.0 as usize {
                return LRESULT(base | (DLGC_WANTMESSAGE as isize));
            }
            LRESULT(base)
        }
        WM_KEYDOWN => {
            if wparam.0 == VK_RETURN.0 as usize {
                if let Ok(parent) = GetParent(hwnd) {
                    let _ = PostMessageW(Some(parent), WM_ADD_TASK, WPARAM(0), LPARAM(0));
                    return LRESULT(0);
                }
            }
            DefSubclassProc(hwnd, msg, wparam, lparam)
        }
        _ => DefSubclassProc(hwnd, msg, wparam, lparam),
    }
}

pub fn set_font(hwnd: HWND, font: &Font) {
    unsafe {
        let _ = SendMessageW(
            hwnd,
            WM_SETFONT,
            Some(WPARAM(font.handle().0 as usize)),
            Some(LPARAM(1)),
        );
    }
}

/// Repopulate the listbox from current filtered task state.
pub fn refresh_task_list(state: &mut AppState) {
    unsafe {
        state.current_list = state.visible_items();

        let _ = SendMessageW(state.hwnd_list, LB_RESETCONTENT, Some(WPARAM(0)), Some(LPARAM(0)));

        for (idx, item_ref) in state.current_list.iter().enumerate() {
            let label = state
                .find_item_details(item_ref.id)
                .map(|(text, _, _, _, _)| text)
                .unwrap_or_default();

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

/// Handle WM_DRAWITEM for owner-drawn task rows (root and nested sub-tasks).
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

        let (text, completed, created_at, depth, has_children) =
            match state.find_item_details(item_ref.id) {
                Some(details) => details,
                None => return,
            };

        let is_hover = state.hover_item == dis.itemID as i32;
        let is_selected = (dis.itemState.0 & ODS_SELECTED.0) != 0;

        // Background
        let bg_color = if is_selected || is_hover {
            CLR_HOVER
        } else {
            CLR_BG_DARK
        };
        let bg_brush = CreateSolidBrush(COLORREF(bg_color));
        FillRect(hdc, &rc, bg_brush);
        let _ = DeleteObject(bg_brush.into());

        // Indentation calculation based on depth (0, 1, 2, 3)
        let indent_px = depth as i32 * 18;

        // Draw tree branch glyph for sub-tasks (depth > 0)
        if depth > 0 {
            SetBkMode(hdc, TRANSPARENT);
            SetTextColor(hdc, COLORREF(CLR_TEXT_DIM));
            let mut branch_rc = RECT {
                left: rc.left + 10 + (depth as i32 - 1) * 18,
                top: rc.top,
                right: rc.left + 10 + indent_px,
                bottom: rc.bottom,
            };
            let mut branch_text: Vec<u16> = "└─".encode_utf16().collect();
            DrawTextW(
                hdc,
                &mut branch_text,
                &mut branch_rc,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE,
            );
        }

        // Checkbox
        let check_x = rc.left + 10 + indent_px;
        let check_y = rc.top + (rc.bottom - rc.top) / 2 - 8;
        let check_rc = RECT {
            left: check_x,
            top: check_y,
            right: check_x + 16,
            bottom: check_y + 16,
        };

        if completed {
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

        // Delete button area (right 30px)
        let del_w = 26;
        let del_rc = RECT {
            left: rc.right - del_w - 4,
            top: rc.top,
            right: rc.right - 4,
            bottom: rc.bottom,
        };

        if completed {
            SetTextColor(hdc, COLORREF(CLR_DELETE));
            let mut del_text: Vec<u16> = "\u{1F5D1}".encode_utf16().collect(); // 🗑
            let mut r = del_rc;
            DrawTextW(hdc, &mut del_text, &mut r, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
        } else if is_hover {
            SetTextColor(hdc, COLORREF(CLR_DELETE));
            let mut del_text: Vec<u16> = "✕".encode_utf16().collect();
            let mut r = del_rc;
            DrawTextW(hdc, &mut del_text, &mut r, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
        }

        // Requirement 4: Small right-most placeholder text for datetime(3) when recorded
        let date_font = Font::new(9, false);
        let old_font = SelectObject(hdc, date_font.handle().into());
        SetBkMode(hdc, TRANSPARENT);
        SetTextColor(hdc, COLORREF(CLR_TEXT_DIM));

        let date_wide: Vec<u16> = created_at.encode_utf16().collect();
        let mut date_sz = SIZE::default();
        let _ = GetTextExtentPoint32W(hdc, &date_wide, &mut date_sz);

        let date_right = del_rc.left - 2;
        let date_left = date_right - date_sz.cx;
        let mut date_rc = RECT {
            left: date_left,
            top: rc.top,
            right: date_right,
            bottom: rc.bottom,
        };
        let mut date_mut_wide = date_wide;
        DrawTextW(
            hdc,
            &mut date_mut_wide,
            &mut date_rc,
            DT_RIGHT | DT_VCENTER | DT_SINGLELINE,
        );

        // Restore font for main item text
        SelectObject(hdc, old_font);

        // Task text
        let text_color = if completed { CLR_TEXT_DIM } else { CLR_TEXT };
        SetTextColor(hdc, COLORREF(text_color));

        let display_text = if has_children {
            format!("{} …", text)
        } else {
            text
        };

        let mut text_rc = RECT {
            left: check_x + 22,
            top: rc.top,
            right: date_left - 4,
            bottom: rc.bottom,
        };
        let mut wide_text: Vec<u16> = display_text.encode_utf16().collect();
        DrawTextW(
            hdc,
            &mut wide_text,
            &mut text_rc,
            DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS,
        );

        // Strikethrough for completed task text
        if completed {
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

        if (dis.itemState.0 & ODS_FOCUS.0) != 0 {
            let _ = DrawFocusRect(hdc, &rc);
        }
    }
}

/// Handle WM_DRAWITEM for owner-drawn buttons.
pub fn draw_button(state: &AppState, dis: &DRAWITEMSTRUCT) {
    unsafe {
        let hdc = dis.hDC;
        let rc = dis.rcItem;
        let ctrl_id = dis.CtlID as i32;

        let is_selected_tab = match (ctrl_id, state.view_mode) {
            (IDC_BTN_TAB_SETTINGS, ViewMode::Settings(SettingsSubPage::AppSettings)) => true,
            (IDC_BTN_TAB_INFO, ViewMode::Settings(SettingsSubPage::Info)) => true,
            (IDC_BTN_TAB_LOGS, ViewMode::Settings(SettingsSubPage::Logs)) => true,
            _ => false,
        };

        let is_active_rec_btn = match ctrl_id {
            IDC_BTN_REC_7 if state.config.record_time_seconds == 7 => true,
            IDC_BTN_REC_15 if state.config.record_time_seconds == 15 => true,
            IDC_BTN_REC_30 if state.config.record_time_seconds == 30 => true,
            _ => false,
        };

        let is_active_depth_btn = match ctrl_id {
            IDC_BTN_DEPTH_1 if state.config.max_depth == 1 => true,
            IDC_BTN_DEPTH_2 if state.config.max_depth == 2 => true,
            IDC_BTN_DEPTH_3 if state.config.max_depth == 3 => true,
            _ => false,
        };

        let bg_color = if is_selected_tab || is_active_rec_btn || is_active_depth_btn {
            CLR_ACCENT
        } else {
            match ctrl_id {
                IDC_BTN_FILTER_ALL if state.filter == crate::state::Filter::All => CLR_ACCENT,
                IDC_BTN_FILTER_ACTIVE if state.filter == crate::state::Filter::Active => CLR_ACCENT,
                IDC_BTN_FILTER_DONE if state.filter == crate::state::Filter::Completed => CLR_ACCENT,
                IDC_BTN_VOICE if state.is_recording_voice => CLR_RECORDING,
                IDC_BTN_PIN | IDC_BTN_SETTINGS | IDC_BTN_CLOSE => CLR_HEADER_DARK,
                IDC_BTN_BACK | IDC_BTN_TEST_MIC | IDC_BTN_CLEAR_LOGS => CLR_CARD_BG,
                _ => CLR_BG_DARK,
            }
        };

        let bg_brush = CreateSolidBrush(COLORREF(bg_color));
        FillRect(hdc, &rc, bg_brush);
        let _ = DeleteObject(bg_brush.into());

        // Text color
        let is_in_settings = matches!(state.view_mode, ViewMode::Settings(_));
        let text_color = if is_selected_tab || is_active_rec_btn || is_active_depth_btn {
            0x00FFFFFF
        } else {
            match ctrl_id {
                IDC_BTN_CLOSE => CLR_DELETE,
                IDC_BTN_PIN if state.config.is_topmost => CLR_ACCENT,
                IDC_BTN_SETTINGS if is_in_settings => CLR_ACCENT,
                IDC_BTN_VOICE if state.is_recording_voice => 0x00FFFFFF,
                IDC_BTN_ADD_SUB if state.selected_task_id.is_some() => CLR_ACCENT,
                IDC_BTN_TEST_MIC => CLR_ACCENT,
                IDC_BTN_CLEAR_LOGS => CLR_WARN,
                _ => CLR_TEXT,
            }
        };

        SetBkMode(hdc, TRANSPARENT);
        SetTextColor(hdc, COLORREF(text_color));

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

        if (dis.itemState.0 & ODS_FOCUS.0) != 0 {
            let _ = DrawFocusRect(hdc, &rc);
        }
    }
}

/// Draw content for Settings sub-pages (App Settings, Info, Logs).
pub fn draw_settings_content(state: &AppState, hdc: HDC, width: i32) {
    let sub_page = match state.view_mode {
        ViewMode::Settings(page) => page,
        _ => return,
    };

    unsafe {
        SetBkMode(hdc, TRANSPARENT);

        match sub_page {
            SettingsSubPage::AppSettings => {
                let font_bold = Font::new(12, true);
                let font_regular = Font::new(12, false);

                // Section 1: Record Time
                let old_font = SelectObject(hdc, font_bold.handle().into());
                SetTextColor(hdc, COLORREF(CLR_TEXT));
                let mut title1: Vec<u16> = "VOICE RECORDING DURATION:".encode_utf16().collect();
                let mut r1 = RECT {
                    left: MARGIN,
                    top: 86,
                    right: width - MARGIN,
                    bottom: 104,
                };
                DrawTextW(hdc, &mut title1, &mut r1, DT_LEFT | DT_VCENTER | DT_SINGLELINE);

                SelectObject(hdc, font_regular.handle().into());
                SetTextColor(hdc, COLORREF(CLR_TEXT_DIM));
                let mut desc1: Vec<u16> =
                    "Reduced to 7s by default. Update to 30s as needed.".encode_utf16().collect();
                let mut r1_desc = RECT {
                    left: MARGIN,
                    top: 138,
                    right: width - MARGIN,
                    bottom: 154,
                };
                DrawTextW(hdc, &mut desc1, &mut r1_desc, DT_LEFT | DT_VCENTER | DT_SINGLELINE);

                // Section 2: Max Depth
                SelectObject(hdc, font_bold.handle().into());
                SetTextColor(hdc, COLORREF(CLR_TEXT));
                let mut title2: Vec<u16> = "SUB-LEVEL MAX DEPTH:".encode_utf16().collect();
                let mut r2 = RECT {
                    left: MARGIN,
                    top: 156,
                    right: width - MARGIN,
                    bottom: 172,
                };
                DrawTextW(hdc, &mut title2, &mut r2, DT_LEFT | DT_VCENTER | DT_SINGLELINE);

                SelectObject(hdc, font_regular.handle().into());
                SetTextColor(hdc, COLORREF(CLR_TEXT_DIM));
                let mut desc2: Vec<u16> =
                    "Tab indents under preceding item (Allowed: 1 to 3).".encode_utf16().collect();
                let mut r2_desc = RECT {
                    left: MARGIN,
                    top: 206,
                    right: width - MARGIN,
                    bottom: 222,
                };
                DrawTextW(hdc, &mut desc2, &mut r2_desc, DT_LEFT | DT_VCENTER | DT_SINGLELINE);

                // Quick Tip box
                let card_rc = RECT {
                    left: MARGIN,
                    top: 236,
                    right: width - MARGIN,
                    bottom: 396,
                };
                let card_brush = CreateSolidBrush(COLORREF(CLR_CARD_BG));
                FillRect(hdc, &card_rc, card_brush);
                let _ = DeleteObject(card_brush.into());

                SetTextColor(hdc, COLORREF(CLR_ACCENT));
                SelectObject(hdc, font_bold.handle().into());
                let mut tip_title: Vec<u16> = "QUICK HIERARCHY TIP".encode_utf16().collect();
                let mut tip_tr = RECT {
                    left: MARGIN + 8,
                    top: 242,
                    right: width - MARGIN - 8,
                    bottom: 258,
                };
                DrawTextW(hdc, &mut tip_title, &mut tip_tr, DT_LEFT | DT_VCENTER | DT_SINGLELINE);

                SelectObject(hdc, font_regular.handle().into());
                SetTextColor(hdc, COLORREF(CLR_TEXT));
                let tip_body = "• Press Tab in task list to indent item.\n\
                                • Press Shift+Tab to outdent item.\n\
                                • Select item & click '+ Sub' to nest child.\n\
                                • Completing all children completes parent.\n\
                                • Uncompleting any child uncompletes parent.";
                let mut tip_body_w: Vec<u16> = tip_body.encode_utf16().collect();
                let mut tip_br = RECT {
                    left: MARGIN + 8,
                    top: 264,
                    right: width - MARGIN - 8,
                    bottom: 390,
                };
                DrawTextW(hdc, &mut tip_body_w, &mut tip_br, DT_LEFT | DT_WORDBREAK);

                SelectObject(hdc, old_font);
            }
            SettingsSubPage::Info => {
                let font_bold = Font::new(12, true);
                let font_regular = Font::new(12, false);
                let old_font = SelectObject(hdc, font_bold.handle().into());

                // Usage stats
                SetTextColor(hdc, COLORREF(CLR_TEXT));
                let mut title1: Vec<u16> = "APPLICATION USAGE:".encode_utf16().collect();
                let mut r1 = RECT {
                    left: MARGIN,
                    top: 82,
                    right: width - MARGIN,
                    bottom: 98,
                };
                DrawTextW(hdc, &mut title1, &mut r1, DT_LEFT | DT_VCENTER | DT_SINGLELINE);

                let total_tasks = state.tasks.len();
                let completed_tasks = state.tasks.iter().filter(|t| t.completed).count();
                let active_tasks = total_tasks - completed_tasks;

                SelectObject(hdc, font_regular.handle().into());
                SetTextColor(hdc, COLORREF(CLR_TEXT_DIM));
                let stats = format!(
                    "Total Tasks: {}  |  Active: {}  |  Done: {}\nSub-level Depth Setting: {} (Max: 3)\nVoice Record Timeout: {}s",
                    total_tasks, active_tasks, completed_tasks, state.config.max_depth, state.config.record_time_seconds
                );
                let mut stats_w: Vec<u16> = stats.encode_utf16().collect();
                let mut stats_rc = RECT {
                    left: MARGIN,
                    top: 102,
                    right: width - MARGIN,
                    bottom: 154,
                };
                DrawTextW(hdc, &mut stats_w, &mut stats_rc, DT_LEFT | DT_WORDBREAK);

                // Keyboard Shortcuts
                SelectObject(hdc, font_bold.handle().into());
                SetTextColor(hdc, COLORREF(CLR_TEXT));
                let mut title2: Vec<u16> = "KEYBOARD SHORTCUT BINDINGS:".encode_utf16().collect();
                let mut r2 = RECT {
                    left: MARGIN,
                    top: 162,
                    right: width - MARGIN,
                    bottom: 178,
                };
                DrawTextW(hdc, &mut title2, &mut r2, DT_LEFT | DT_VCENTER | DT_SINGLELINE);

                let shortcuts_card = RECT {
                    left: MARGIN,
                    top: 184,
                    right: width - MARGIN,
                    bottom: 396,
                };
                let card_brush = CreateSolidBrush(COLORREF(CLR_CARD_BG));
                FillRect(hdc, &shortcuts_card, card_brush);
                let _ = DeleteObject(card_brush.into());

                SelectObject(hdc, font_regular.handle().into());
                SetTextColor(hdc, COLORREF(CLR_TEXT));
                let shortcuts = "Ctrl+Shift+T  : Toggle window Show / Hide\n\
                                 Enter         : Add task or sub-task\n\
                                 Tab           : Indent task to sub-level\n\
                                 Shift+Tab     : Outdent task to parent\n\
                                 Space         : Toggle complete / active\n\
                                 Delete        : Remove selected task\n\
                                 Up / Down     : Navigate list items\n\
                                 Esc / ✕       : Minimize to tray";
                let mut shortcuts_w: Vec<u16> = shortcuts.encode_utf16().collect();
                let mut sc_rc = RECT {
                    left: MARGIN + 8,
                    top: 190,
                    right: width - MARGIN - 8,
                    bottom: 390,
                };
                DrawTextW(hdc, &mut shortcuts_w, &mut sc_rc, DT_LEFT | DT_WORDBREAK);

                SelectObject(hdc, old_font);
            }
            SettingsSubPage::Logs => {
                let font_bold = Font::new(12, true);
                let font_small = Font::new(11, false);
                let old_font = SelectObject(hdc, font_bold.handle().into());

                SetTextColor(hdc, COLORREF(CLR_TEXT));
                let mut title: Vec<u16> = "RECENT 5 DIAGNOSTIC LOGS:".encode_utf16().collect();
                let mut r = RECT {
                    left: MARGIN,
                    top: 80,
                    right: width - MARGIN,
                    bottom: 96,
                };
                DrawTextW(hdc, &mut title, &mut r, DT_LEFT | DT_VCENTER | DT_SINGLELINE);

                SelectObject(hdc, font_small.handle().into());
                SetTextColor(hdc, COLORREF(CLR_TEXT_DIM));
                let mut sub_title: Vec<u16> =
                    "Diagnostic trace for microphone input & WinRT Speech engine:".encode_utf16().collect();
                let mut r_sub = RECT {
                    left: MARGIN,
                    top: 98,
                    right: width - MARGIN,
                    bottom: 114,
                };
                DrawTextW(hdc, &mut sub_title, &mut r_sub, DT_LEFT | DT_VCENTER | DT_SINGLELINE);

                // Fetch recent 5 logs
                let logs = get_recent_logs(5);
                let mut start_y = 118;
                let card_h = 56;
                let card_gap = 4;

                if logs.is_empty() {
                    let mut no_logs: Vec<u16> =
                        "No warnings or errors logged yet. Mic input is clean.".encode_utf16().collect();
                    let mut r_empty = RECT {
                        left: MARGIN,
                        top: 140,
                        right: width - MARGIN,
                        bottom: 180,
                    };
                    DrawTextW(hdc, &mut no_logs, &mut r_empty, DT_CENTER | DT_WORDBREAK);
                } else {
                    for entry in &logs {
                        let log_rc = RECT {
                            left: MARGIN,
                            top: start_y,
                            right: width - MARGIN,
                            bottom: start_y + card_h,
                        };
                        let brush = CreateSolidBrush(COLORREF(CLR_CARD_BG));
                        FillRect(hdc, &log_rc, brush);
                        let _ = DeleteObject(brush.into());

                        // Badge color
                        let (badge_str, badge_color) = match entry.level {
                            LogLevel::Info => ("[INFO]", CLR_ACCENT),
                            LogLevel::Warn => ("[WARN]", CLR_WARN),
                            LogLevel::Error => ("[ERROR]", CLR_DELETE),
                        };

                        SelectObject(hdc, font_bold.handle().into());
                        SetTextColor(hdc, COLORREF(badge_color));
                        let mut badge_w: Vec<u16> = badge_str.encode_utf16().collect();
                        let mut b_rc = RECT {
                            left: MARGIN + 6,
                            top: start_y + 4,
                            right: MARGIN + 60,
                            bottom: start_y + 20,
                        };
                        DrawTextW(hdc, &mut badge_w, &mut b_rc, DT_LEFT | DT_VCENTER | DT_SINGLELINE);

                        SelectObject(hdc, font_small.handle().into());
                        SetTextColor(hdc, COLORREF(CLR_TEXT_DIM));
                        let mut time_w: Vec<u16> = entry.timestamp.encode_utf16().collect();
                        let mut t_rc = RECT {
                            left: width - MARGIN - 60,
                            top: start_y + 4,
                            right: width - MARGIN - 6,
                            bottom: start_y + 20,
                        };
                        DrawTextW(hdc, &mut time_w, &mut t_rc, DT_RIGHT | DT_VCENTER | DT_SINGLELINE);

                        // Message text
                        SetTextColor(hdc, COLORREF(CLR_TEXT));
                        let mut msg_w: Vec<u16> = entry.message.encode_utf16().collect();
                        let mut m_rc = RECT {
                            left: MARGIN + 6,
                            top: start_y + 22,
                            right: width - MARGIN - 6,
                            bottom: start_y + card_h - 2,
                        };
                        DrawTextW(hdc, &mut msg_w, &mut m_rc, DT_LEFT | DT_WORDBREAK);

                        start_y += card_h + card_gap;
                    }
                }

                SelectObject(hdc, old_font);
            }
        }
    }
}

pub fn measure_item(mis: &mut MEASUREITEMSTRUCT) {
    mis.itemHeight = ITEM_HEIGHT as u32;
}

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

pub fn clear_input(hwnd_input: HWND) {
    unsafe {
        let _ = SetWindowTextW(hwnd_input, w!(""));
    }
}

pub fn set_input_text(hwnd_input: HWND, text: &str) {
    unsafe {
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let _ = SetWindowTextW(hwnd_input, PCWSTR(wide.as_ptr()));
        let _ = SetFocus(Some(hwnd_input));
    }
}

pub fn set_cue_banner(hwnd_input: HWND, text: &str) {
    unsafe {
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let _ = SendMessageW(
            hwnd_input,
            0x1501, // EM_SETCUEBANNER
            Some(WPARAM(1)),
            Some(LPARAM(wide.as_ptr() as isize)),
        );
    }
}
