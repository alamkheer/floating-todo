use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::hotkey;
use crate::state::{AppState, ListItemRef};
use crate::storage;
use crate::tray;
use crate::ui;
use crate::voice;

/// Window class name.
const CLASS_NAME: PCWSTR = w!("FloatingTodoClass");

/// Window dimensions (logical pixels at 96 DPI).
pub const WIN_WIDTH: i32 = 320;
pub const WIN_HEIGHT: i32 = 480;

/// Timer IDs.
const SAVE_TIMER_ID: usize = 1;
const SAVE_DEBOUNCE_MS: u32 = 2000;
const VOICE_COUNTDOWN_TIMER_ID: usize = 2;

/// Custom message IDs for keyboard listbox actions.
pub const WM_LIST_TOGGLE: u32 = WM_APP + 104;
pub const WM_LIST_DELETE: u32 = WM_APP + 105;

/// Retrieve a mutable reference to AppState from GWLP_USERDATA.
///
/// # Safety
/// Must only be called after WM_NCCREATE has stored the pointer.
pub unsafe fn get_state(hwnd: HWND) -> Option<&'static mut AppState> {
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppState;
    if ptr.is_null() {
        None
    } else {
        Some(&mut *ptr)
    }
}

/// Schedule a debounced save.
fn schedule_save(hwnd: HWND) {
    unsafe {
        let _ = SetTimer(Some(hwnd), SAVE_TIMER_ID, SAVE_DEBOUNCE_MS, None);
    }
}

/// Subclass proc for the LISTBOX control to support keyboard shortcuts (Space to toggle, Delete to delete).
unsafe extern "system" fn list_subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _uid: usize,
    _data: usize,
) -> LRESULT {
    if msg == WM_KEYDOWN {
        if let Ok(parent) = GetParent(hwnd) {
            if wparam.0 == VK_SPACE.0 as usize {
                let _ = PostMessageW(Some(parent), WM_LIST_TOGGLE, WPARAM(0), LPARAM(0));
                return LRESULT(0);
            } else if wparam.0 == VK_DELETE.0 as usize {
                let _ = PostMessageW(Some(parent), WM_LIST_DELETE, WPARAM(0), LPARAM(0));
                return LRESULT(0);
            }
        }
    }
    DefSubclassProc(hwnd, msg, wparam, lparam)
}

/// The main window procedure.
unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        // ── Store AppState pointer during window creation ──
        WM_NCCREATE => {
            let cs = &*(lparam.0 as *const CREATESTRUCTW);
            let state_ptr = cs.lpCreateParams as *mut AppState;
            if !state_ptr.is_null() {
                (*state_ptr).hwnd = hwnd;
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr as isize);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }

        // ── Create child controls ──
        WM_CREATE => {
            let cs = &*(lparam.0 as *const CREATESTRUCTW);
            let hinstance: HINSTANCE = cs.hInstance;

            ui::apply_dark_mode(hwnd, true);

            if let Some(state) = get_state(hwnd) {
                ui::create_controls(hwnd, state, hinstance, WIN_WIDTH);

                // Create fonts and apply to controls
                let font_normal = Box::leak(Box::new(ui::Font::new(16, false)));
                let font_small = Box::leak(Box::new(ui::Font::new(13, false)));
                let font_emoji = Box::leak(Box::new(ui::Font::emoji_font(15)));

                ui::set_font(state.hwnd_input, font_normal);
                ui::set_font(state.hwnd_voice_btn, font_emoji);
                ui::set_font(state.hwnd_list, font_normal);
                ui::set_font(state.hwnd_filter_all, font_small);
                ui::set_font(state.hwnd_filter_active, font_small);
                ui::set_font(state.hwnd_filter_done, font_small);
                ui::set_font(state.hwnd_add_sub_btn, font_small);
                ui::set_font(state.hwnd_pin_btn, font_emoji);
                ui::set_font(state.hwnd_close_btn, font_normal);

                // Subclass listbox for Space (toggle) and Delete (remove)
                let _ = SetWindowSubclass(
                    state.hwnd_list,
                    Some(list_subclass_proc),
                    2,
                    0,
                );

                // Populate the list from loaded tasks
                ui::refresh_task_list(state);
            }

            LRESULT(0)
        }

        // ── Make the frameless window draggable by header area ──
        WM_NCHITTEST => {
            let result = DefWindowProcW(hwnd, msg, wparam, lparam);
            if result == LRESULT(HTCLIENT as isize) {
                let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
                let mut rect = RECT::default();
                let _ = GetWindowRect(hwnd, &mut rect);
                if y - rect.top < ui::HEADER_HEIGHT {
                    return LRESULT(HTCAPTION as isize);
                }
            }
            result
        }

        // ── Paint header background ──
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);

            // Header background
            let header_rc = RECT {
                left: 0,
                top: 0,
                right: WIN_WIDTH,
                bottom: ui::HEADER_HEIGHT,
            };
            let header_brush = CreateSolidBrush(COLORREF(ui::CLR_HEADER_DARK));
            FillRect(hdc, &header_rc, header_brush);
            let _ = DeleteObject(header_brush.into());

            // Title text
            SetBkMode(hdc, TRANSPARENT);
            SetTextColor(hdc, COLORREF(ui::CLR_TEXT));

            let title_font = ui::Font::new(15, true);
            let old_font = SelectObject(hdc, title_font.handle().into());

            let mut title_rc = RECT {
                left: 10,
                top: 0,
                right: WIN_WIDTH - 70,
                bottom: ui::HEADER_HEIGHT,
            };
            let mut title: Vec<u16> = "Floating TODO".encode_utf16().collect();
            DrawTextW(
                hdc,
                &mut title,
                &mut title_rc,
                DT_SINGLELINE | DT_VCENTER,
            );

            SelectObject(hdc, old_font);

            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }

        // ── Erase background ──
        WM_ERASEBKGND => {
            let hdc = HDC(wparam.0 as *mut _);
            let mut rc = RECT::default();
            let _ = GetClientRect(hwnd, &mut rc);
            let brush = CreateSolidBrush(COLORREF(ui::CLR_BG_DARK));
            FillRect(hdc, &rc, brush);
            let _ = DeleteObject(brush.into());
            LRESULT(1)
        }

        // ── Color the EDIT control (dark theme) ──
        WM_CTLCOLOREDIT => {
            let hdc = HDC(wparam.0 as *mut _);
            SetTextColor(hdc, COLORREF(ui::CLR_TEXT));
            SetBkColor(hdc, COLORREF(ui::CLR_INPUT_BG));
            if let Some(state) = get_state(hwnd) {
                return LRESULT(state.input_brush.0 as isize);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }

        // ── Color the LISTBOX control (dark theme) ──
        WM_CTLCOLORLISTBOX => {
            let hdc = HDC(wparam.0 as *mut _);
            SetTextColor(hdc, COLORREF(ui::CLR_TEXT));
            SetBkColor(hdc, COLORREF(ui::CLR_BG_DARK));
            if let Some(state) = get_state(hwnd) {
                return LRESULT(state.list_brush.0 as isize);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }

        // ── Owner-draw: measure list item height ──
        WM_MEASUREITEM => {
            let mis = &mut *(lparam.0 as *mut MEASUREITEMSTRUCT);
            if mis.CtlID == ui::IDC_TASKLIST as u32 {
                ui::measure_item(mis);
            }
            LRESULT(1)
        }

        // ── Owner-draw: paint list items and buttons ──
        WM_DRAWITEM => {
            let dis = &*(lparam.0 as *const DRAWITEMSTRUCT);
            if let Some(state) = get_state(hwnd) {
                match dis.CtlID as i32 {
                    ui::IDC_TASKLIST => {
                        ui::draw_task_item(state, dis);
                    }
                    ui::IDC_BTN_PIN
                    | ui::IDC_BTN_CLOSE
                    | ui::IDC_BTN_VOICE
                    | ui::IDC_BTN_ADD_SUB
                    | ui::IDC_BTN_FILTER_ALL
                    | ui::IDC_BTN_FILTER_ACTIVE
                    | ui::IDC_BTN_FILTER_DONE => {
                        ui::draw_button(state, dis);
                    }
                    _ => {}
                }
            }
            LRESULT(1)
        }

        // ── Custom message: Enter pressed in input ──
        ui::WM_ADD_TASK => {
            if let Some(state) = get_state(hwnd) {
                let text = ui::get_input_text(state.hwnd_input);
                if !text.is_empty() {
                    if state.sub_task_input_mode {
                        if let Some(parent_id) = state.selected_task_id {
                            state.add_sub_task(parent_id, text);
                        }
                        state.sub_task_input_mode = false;
                    } else {
                        state.add_task(text);
                    }
                    ui::clear_input(state.hwnd_input);
                    ui::refresh_task_list(state);
                    schedule_save(hwnd);
                } else if state.sub_task_input_mode {
                    state.sub_task_input_mode = false;
                }
            }
            LRESULT(0)
        }

        // ── Custom message: Space pressed in listbox (Toggle item) ──
        WM_LIST_TOGGLE => {
            if let Some(state) = get_state(hwnd) {
                let sel = SendMessageW(state.hwnd_list, LB_GETCURSEL, Some(WPARAM(0)), Some(LPARAM(0)));
                if sel.0 >= 0 {
                    let idx = sel.0 as usize;
                    if let Some(item_ref) = state.current_list.get(idx).copied() {
                        match item_ref {
                            ListItemRef::Parent(id) => state.toggle_task(id),
                            ListItemRef::SubTask { parent_id, sub_id } => {
                                state.toggle_sub_task(parent_id, sub_id)
                            }
                        }
                        ui::refresh_task_list(state);
                        schedule_save(hwnd);
                    }
                }
            }
            LRESULT(0)
        }

        // ── Custom message: Delete pressed in listbox (Remove item) ──
        WM_LIST_DELETE => {
            if let Some(state) = get_state(hwnd) {
                let sel = SendMessageW(state.hwnd_list, LB_GETCURSEL, Some(WPARAM(0)), Some(LPARAM(0)));
                if sel.0 >= 0 {
                    let idx = sel.0 as usize;
                    if let Some(item_ref) = state.current_list.get(idx).copied() {
                        match item_ref {
                            ListItemRef::Parent(id) => state.delete_task(id),
                            ListItemRef::SubTask { parent_id, sub_id } => {
                                state.delete_sub_task(parent_id, sub_id)
                            }
                        }
                        ui::refresh_task_list(state);
                        schedule_save(hwnd);
                    }
                }
            }
            LRESULT(0)
        }

        // ── Custom message: Voice speech recognized ──
        voice::WM_VOICE_RESULT => {
            let ptr = wparam.0 as *mut String;
            if !ptr.is_null() {
                let text = *Box::from_raw(ptr);
                if let Some(state) = get_state(hwnd) {
                    state.is_recording_voice = false;
                    let _ = KillTimer(Some(hwnd), VOICE_COUNTDOWN_TIMER_ID);
                    if !text.is_empty() {
                        state.add_task(text);
                        ui::refresh_task_list(state);
                        schedule_save(hwnd);
                    }
                    let _ = InvalidateRect(Some(state.hwnd_voice_btn), None, true);
                }
            }
            LRESULT(0)
        }

        // ── Custom message: Voice session ended or timed out ──
        voice::WM_VOICE_STOPPED => {
            if let Some(state) = get_state(hwnd) {
                state.is_recording_voice = false;
                let _ = KillTimer(Some(hwnd), VOICE_COUNTDOWN_TIMER_ID);
                let _ = InvalidateRect(Some(state.hwnd_voice_btn), None, true);
            }
            LRESULT(0)
        }

        // ── Button clicks and list selection ──
        WM_COMMAND => {
            let ctrl_id = (wparam.0 & 0xFFFF) as i32;
            let notify_code = ((wparam.0 >> 16) & 0xFFFF) as u16;

            match ctrl_id {
                // ── Header buttons ──
                ui::IDC_BTN_PIN => {
                    if let Some(state) = get_state(hwnd) {
                        state.is_topmost = !state.is_topmost;
                        let pos = if state.is_topmost {
                            HWND_TOPMOST
                        } else {
                            HWND_NOTOPMOST
                        };
                        let _ = SetWindowPos(
                            hwnd,
                            Some(pos),
                            0,
                            0,
                            0,
                            0,
                            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                        );
                        let _ = InvalidateRect(Some(state.hwnd_pin_btn), None, true);
                    }
                }

                ui::IDC_BTN_CLOSE => {
                    let _ = ShowWindow(hwnd, SW_HIDE);
                }

                // ── Voice Input Button 🎙️ ──
                ui::IDC_BTN_VOICE => {
                    if let Some(state) = get_state(hwnd) {
                        if state.is_recording_voice {
                            // User clicked to cancel/stop early
                            voice::cancel_recording();
                            state.is_recording_voice = false;
                            let _ = KillTimer(Some(hwnd), VOICE_COUNTDOWN_TIMER_ID);
                            let _ = InvalidateRect(Some(state.hwnd_voice_btn), None, true);
                        } else {
                            // Start 30-second recording session
                            state.is_recording_voice = true;
                            state.voice_seconds_remaining = 30;
                            let _ = SetTimer(Some(hwnd), VOICE_COUNTDOWN_TIMER_ID, 1000, None);
                            voice::start_voice_recognition(hwnd);
                            let _ = InvalidateRect(Some(state.hwnd_voice_btn), None, true);
                        }
                    }
                }

                // ── Add Sub-task Button ＋ Sub ──
                ui::IDC_BTN_ADD_SUB => {
                    if let Some(state) = get_state(hwnd) {
                        if let Some(parent_id) = state.selected_task_id {
                            if let Some(task) = state.find_task(parent_id) {
                                if !task.completed {
                                    state.sub_task_input_mode = true;
                                    ui::set_input_text(state.hwnd_input, "");
                                    let _ = InvalidateRect(Some(state.hwnd_add_sub_btn), None, true);
                                }
                            }
                        }
                    }
                }

                // ── Filter buttons ──
                ui::IDC_BTN_FILTER_ALL => {
                    if let Some(state) = get_state(hwnd) {
                        state.filter = crate::state::Filter::All;
                        state.selected_task_id = None;
                        state.sub_task_input_mode = false;
                        ui::refresh_task_list(state);
                        let _ = InvalidateRect(Some(state.hwnd_filter_all), None, true);
                        let _ = InvalidateRect(Some(state.hwnd_filter_active), None, true);
                        let _ = InvalidateRect(Some(state.hwnd_filter_done), None, true);
                        let _ = InvalidateRect(Some(state.hwnd_add_sub_btn), None, true);
                    }
                }
                ui::IDC_BTN_FILTER_ACTIVE => {
                    if let Some(state) = get_state(hwnd) {
                        state.filter = crate::state::Filter::Active;
                        state.selected_task_id = None;
                        state.sub_task_input_mode = false;
                        ui::refresh_task_list(state);
                        let _ = InvalidateRect(Some(state.hwnd_filter_all), None, true);
                        let _ = InvalidateRect(Some(state.hwnd_filter_active), None, true);
                        let _ = InvalidateRect(Some(state.hwnd_filter_done), None, true);
                        let _ = InvalidateRect(Some(state.hwnd_add_sub_btn), None, true);
                    }
                }
                ui::IDC_BTN_FILTER_DONE => {
                    if let Some(state) = get_state(hwnd) {
                        state.filter = crate::state::Filter::Completed;
                        state.selected_task_id = None;
                        state.sub_task_input_mode = false;
                        ui::refresh_task_list(state);
                        let _ = InvalidateRect(Some(state.hwnd_filter_all), None, true);
                        let _ = InvalidateRect(Some(state.hwnd_filter_active), None, true);
                        let _ = InvalidateRect(Some(state.hwnd_filter_done), None, true);
                        let _ = InvalidateRect(Some(state.hwnd_add_sub_btn), None, true);
                    }
                }

                // ── Listbox click: determine checkbox, delete, or selection ──
                ui::IDC_TASKLIST if notify_code == LBN_SELCHANGE as u16 => {
                    if let Some(state) = get_state(hwnd) {
                        let sel = SendMessageW(
                            state.hwnd_list,
                            LB_GETCURSEL,
                            Some(WPARAM(0)),
                            Some(LPARAM(0)),
                        );
                        if sel.0 >= 0 {
                            let idx = sel.0 as usize;
                            if let Some(item_ref) = state.current_list.get(idx).copied() {
                                let mut pt = POINT::default();
                                let _ = GetCursorPos(&mut pt);
                                let _ = ScreenToClient(state.hwnd_list, &mut pt);

                                let mut list_rc = RECT::default();
                                let _ = GetClientRect(state.hwnd_list, &mut list_rc);

                                if pt.x > list_rc.right - 36 {
                                    // Delete area (right 36px)
                                    match item_ref {
                                        ListItemRef::Parent(id) => state.delete_task(id),
                                        ListItemRef::SubTask { parent_id, sub_id } => {
                                            state.delete_sub_task(parent_id, sub_id)
                                        }
                                    }
                                    ui::refresh_task_list(state);
                                    schedule_save(hwnd);
                                } else if pt.x < 34 || (matches!(item_ref, ListItemRef::SubTask { .. }) && pt.x < 56) {
                                    // Checkbox area
                                    match item_ref {
                                        ListItemRef::Parent(id) => state.toggle_task(id),
                                        ListItemRef::SubTask { parent_id, sub_id } => {
                                            state.toggle_sub_task(parent_id, sub_id)
                                        }
                                    }
                                    ui::refresh_task_list(state);
                                    schedule_save(hwnd);
                                } else {
                                    // Selection for adding sub-todos
                                    let parent_id = match item_ref {
                                        ListItemRef::Parent(id) => id,
                                        ListItemRef::SubTask { parent_id, .. } => parent_id,
                                    };
                                    state.selected_task_id = Some(parent_id);
                                    let _ = InvalidateRect(Some(state.hwnd_add_sub_btn), None, true);
                                }
                            }
                        }
                    }
                }

                // ── Tray menu commands ──
                _ if notify_code == 0 => {
                    tray::handle_menu_command(hwnd, ctrl_id as u16);
                }

                _ => {}
            }

            LRESULT(0)
        }

        // ── Timers: Save debounce & Voice countdown ──
        WM_TIMER => {
            if wparam.0 == SAVE_TIMER_ID {
                let _ = KillTimer(Some(hwnd), SAVE_TIMER_ID);
                if let Some(state) = get_state(hwnd) {
                    if state.dirty {
                        let _ = storage::save_tasks(&state.tasks, state.next_id);
                        state.dirty = false;
                    }
                }
            } else if wparam.0 == VOICE_COUNTDOWN_TIMER_ID {
                if let Some(state) = get_state(hwnd) {
                    if state.voice_seconds_remaining > 0 {
                        state.voice_seconds_remaining -= 1;
                        let _ = InvalidateRect(Some(state.hwnd_voice_btn), None, true);
                    }
                    if state.voice_seconds_remaining == 0 {
                        let _ = KillTimer(Some(hwnd), VOICE_COUNTDOWN_TIMER_ID);
                        state.is_recording_voice = false;
                        voice::cancel_recording();
                        let _ = InvalidateRect(Some(state.hwnd_voice_btn), None, true);
                    }
                }
            }
            LRESULT(0)
        }

        // ── Global hotkey ──
        WM_HOTKEY => {
            hotkey::handle_hotkey(hwnd, wparam.0);
            LRESULT(0)
        }

        // ── System tray icon messages ──
        tray::WM_TRAYICON => {
            tray::handle_tray_message(hwnd, lparam);
            LRESULT(0)
        }

        // ── Clean up and quit ──
        WM_DESTROY => {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppState;
            if !ptr.is_null() {
                let state = &*ptr;
                if state.dirty {
                    let _ = storage::save_tasks(&state.tasks, state.next_id);
                }
                let _ = Box::from_raw(ptr);
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            }
            PostQuitMessage(0);
            LRESULT(0)
        }

        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Register the window class and create the main floating window.
pub fn create_main_window(state: Box<AppState>) -> windows::core::Result<HWND> {
    unsafe {
        let hinstance = GetModuleHandleW(None)?;

        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinstance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            lpszClassName: CLASS_NAME,
            ..Default::default()
        };
        let atom = RegisterClassExW(&wc);
        debug_assert!(atom != 0, "RegisterClassExW failed");

        // Leak the Box to get a stable raw pointer
        let state_ptr = Box::into_raw(state);

        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            CLASS_NAME,
            w!("Floating TODO"),
            WS_POPUP | WS_VISIBLE | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            WIN_WIDTH,
            WIN_HEIGHT,
            None,
            None,
            Some(hinstance.into()),
            Some(state_ptr as *const std::ffi::c_void),
        )?;

        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        let _ = UpdateWindow(hwnd);

        Ok(hwnd)
    }
}

/// Standard Win32 message loop with keyboard Tab navigation support via IsDialogMessageW.
pub fn run_message_loop(hwnd: HWND) {
    unsafe {
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            // IsDialogMessageW translates Tab / Shift+Tab and Space/Enter navigation
            if !IsDialogMessageW(hwnd, &msg).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
}
