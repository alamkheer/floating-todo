use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::hotkey;
use crate::state::{AppState, SettingsSubPage, ViewMode};
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

/// Subclass proc for the LISTBOX control to support keyboard shortcuts.
unsafe extern "system" fn list_subclass_proc(
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
            // Tell Windows dialog manager this listbox wants Tab and Arrow keys directly
            LRESULT(base | (DLGC_WANTTAB as isize) | (DLGC_WANTALLKEYS as isize))
        }
        WM_KEYDOWN => {
            if let Ok(parent) = GetParent(hwnd) {
                if wparam.0 == VK_SPACE.0 as usize {
                    let _ = PostMessageW(Some(parent), WM_LIST_TOGGLE, WPARAM(0), LPARAM(0));
                    return LRESULT(0);
                } else if wparam.0 == VK_DELETE.0 as usize {
                    let _ = PostMessageW(Some(parent), WM_LIST_DELETE, WPARAM(0), LPARAM(0));
                    return LRESULT(0);
                } else if wparam.0 == VK_TAB.0 as usize {
                    let is_shift = (GetKeyState(VK_SHIFT.0 as i32) as u16 & 0x8000) != 0;
                    let target_msg = if is_shift {
                        ui::WM_LIST_OUTDENT
                    } else {
                        ui::WM_LIST_INDENT
                    };
                    let _ = PostMessageW(Some(parent), target_msg, WPARAM(0), LPARAM(0));
                    return LRESULT(0);
                }
            }
            DefSubclassProc(hwnd, msg, wparam, lparam)
        }
        _ => DefSubclassProc(hwnd, msg, wparam, lparam),
    }
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
                let font_small = Box::leak(Box::new(ui::Font::new(12, false)));
                let font_emoji = Box::leak(Box::new(ui::Font::emoji_font(15)));

                ui::set_font(state.hwnd_input, font_normal);
                ui::set_font(state.hwnd_voice_btn, font_emoji);
                ui::set_font(state.hwnd_list, font_normal);
                ui::set_font(state.hwnd_filter_all, font_small);
                ui::set_font(state.hwnd_filter_active, font_small);
                ui::set_font(state.hwnd_filter_done, font_small);
                ui::set_font(state.hwnd_add_sub_btn, font_small);
                ui::set_font(state.hwnd_pin_btn, font_emoji);
                ui::set_font(state.hwnd_settings_btn, font_emoji);
                ui::set_font(state.hwnd_close_btn, font_normal);

                ui::set_font(state.hwnd_tab_settings, font_small);
                ui::set_font(state.hwnd_tab_info, font_small);
                ui::set_font(state.hwnd_tab_logs, font_small);
                ui::set_font(state.hwnd_btn_rec_7, font_small);
                ui::set_font(state.hwnd_btn_rec_15, font_small);
                ui::set_font(state.hwnd_btn_rec_30, font_small);
                ui::set_font(state.hwnd_btn_depth_1, font_small);
                ui::set_font(state.hwnd_btn_depth_2, font_small);
                ui::set_font(state.hwnd_btn_depth_3, font_small);
                ui::set_font(state.hwnd_btn_test_mic, font_small);
                ui::set_font(state.hwnd_btn_clear_logs, font_small);
                ui::set_font(state.hwnd_btn_back, font_small);

                // Subclass listbox for Space (toggle), Delete (remove), and Tab (indent/outdent)
                let _ = SetWindowSubclass(state.hwnd_list, Some(list_subclass_proc), 2, 0);

                // Populate list from loaded tasks
                ui::refresh_task_list(state);
            }

            LRESULT(0)
        }

        // ── Make frameless window draggable by header area ──
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

        // ── Paint header background, title, settings view content & footer ──
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
                right: WIN_WIDTH - 100, // Leave room for 📌, ⚙, ✕
                bottom: ui::HEADER_HEIGHT,
            };

            let title_str = if let Some(state) = get_state(hwnd) {
                match state.view_mode {
                    ViewMode::Tasks => "Floating TODO",
                    ViewMode::Settings(_) => "Settings",
                }
            } else {
                "Floating TODO"
            };

            let mut title: Vec<u16> = title_str.encode_utf16().collect();
            DrawTextW(hdc, &mut title, &mut title_rc, DT_SINGLELINE | DT_VCENTER);

            SelectObject(hdc, old_font);

            // If in Settings view, draw settings sub-page content in client area
            if let Some(state) = get_state(hwnd) {
                if matches!(state.view_mode, ViewMode::Settings(_)) {
                    ui::draw_settings_content(state, hdc, WIN_WIDTH);
                }
            }

            // ── Footer placeholder text denoting version ──
            let footer_y = WIN_HEIGHT - ui::FOOTER_HEIGHT;
            let footer_rc = RECT {
                left: 0,
                top: footer_y,
                right: WIN_WIDTH,
                bottom: WIN_HEIGHT,
            };
            let footer_brush = CreateSolidBrush(COLORREF(ui::CLR_HEADER_DARK));
            FillRect(hdc, &footer_rc, footer_brush);
            let _ = DeleteObject(footer_brush.into());

            // 1px top border for subtle separation
            let sep_rc = RECT {
                left: 0,
                top: footer_y,
                right: WIN_WIDTH,
                bottom: footer_y + 1,
            };
            let sep_brush = CreateSolidBrush(COLORREF(0x00333333));
            FillRect(hdc, &sep_rc, sep_brush);
            let _ = DeleteObject(sep_brush.into());

            // Version text
            let ver_font = ui::Font::new(11, false);
            let old_ver_font = SelectObject(hdc, ver_font.handle().into());
            SetTextColor(hdc, COLORREF(ui::CLR_TEXT_DIM));

            let ver_text = format!("Floating TODO v{}", env!("CARGO_PKG_VERSION"));
            let mut ver_u16: Vec<u16> = ver_text.encode_utf16().collect();
            let mut text_rc = RECT {
                left: 10,
                top: footer_y,
                right: WIN_WIDTH - 10,
                bottom: WIN_HEIGHT,
            };
            DrawTextW(
                hdc,
                &mut ver_u16,
                &mut text_rc,
                DT_SINGLELINE | DT_CENTER | DT_VCENTER,
            );
            SelectObject(hdc, old_ver_font);

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

        // ── Color EDIT control (dark theme) ──
        WM_CTLCOLOREDIT => {
            let hdc = HDC(wparam.0 as *mut _);
            SetTextColor(hdc, COLORREF(ui::CLR_TEXT));
            SetBkColor(hdc, COLORREF(ui::CLR_INPUT_BG));
            if let Some(state) = get_state(hwnd) {
                return LRESULT(state.input_brush.0 as isize);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }

        // ── Color LISTBOX control (dark theme) ──
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
                    | ui::IDC_BTN_SETTINGS
                    | ui::IDC_BTN_CLOSE
                    | ui::IDC_BTN_VOICE
                    | ui::IDC_BTN_ADD_SUB
                    | ui::IDC_BTN_FILTER_ALL
                    | ui::IDC_BTN_FILTER_ACTIVE
                    | ui::IDC_BTN_FILTER_DONE
                    | ui::IDC_BTN_TAB_SETTINGS
                    | ui::IDC_BTN_TAB_INFO
                    | ui::IDC_BTN_TAB_LOGS
                    | ui::IDC_BTN_REC_7
                    | ui::IDC_BTN_REC_15
                    | ui::IDC_BTN_REC_30
                    | ui::IDC_BTN_DEPTH_1
                    | ui::IDC_BTN_DEPTH_2
                    | ui::IDC_BTN_DEPTH_3
                    | ui::IDC_BTN_TEST_MIC
                    | ui::IDC_BTN_CLEAR_LOGS
                    | ui::IDC_BTN_BACK => {
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
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    if state.sub_task_input_mode {
                        if let Some(parent_id) = state.selected_task_id {
                            state.add_sub_task(parent_id, trimmed.to_string());
                        }
                        state.sub_task_input_mode = false;
                        ui::set_cue_banner(state.hwnd_input, "Add a task... (Press Enter)");
                    } else {
                        state.add_task(trimmed.to_string());
                    }
                    ui::clear_input(state.hwnd_input);
                    ui::refresh_task_list(state);
                    let _ = storage::save_tasks(&state.tasks, state.next_id, &state.config);
                    state.dirty = false;
                } else if state.sub_task_input_mode {
                    state.sub_task_input_mode = false;
                    ui::clear_input(state.hwnd_input);
                    ui::set_cue_banner(state.hwnd_input, "Add a task... (Press Enter)");
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
                        state.toggle_item(item_ref.id);
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
                        state.delete_item(item_ref.id);
                        ui::refresh_task_list(state);
                        schedule_save(hwnd);
                    }
                }
            }
            LRESULT(0)
        }

        // ── Custom message: Tab pressed in listbox (Indent sub-level) ──
        ui::WM_LIST_INDENT => {
            if let Some(state) = get_state(hwnd) {
                let sel = SendMessageW(state.hwnd_list, LB_GETCURSEL, Some(WPARAM(0)), Some(LPARAM(0)));
                if sel.0 >= 0 {
                    let idx = sel.0 as usize;
                    if let Some(item_ref) = state.current_list.get(idx).copied() {
                        if state.indent_item(item_ref.id) {
                            ui::refresh_task_list(state);
                            // Reselect the indented item
                            if let Some(new_idx) =
                                state.current_list.iter().position(|r| r.id == item_ref.id)
                            {
                                let _ = SendMessageW(
                                    state.hwnd_list,
                                    LB_SETCURSEL,
                                    Some(WPARAM(new_idx)),
                                    Some(LPARAM(0)),
                                );
                            }
                            schedule_save(hwnd);
                        }
                    }
                }
            }
            LRESULT(0)
        }

        // ── Custom message: Shift+Tab pressed in listbox (Outdent sub-level) ──
        ui::WM_LIST_OUTDENT => {
            if let Some(state) = get_state(hwnd) {
                let sel = SendMessageW(state.hwnd_list, LB_GETCURSEL, Some(WPARAM(0)), Some(LPARAM(0)));
                if sel.0 >= 0 {
                    let idx = sel.0 as usize;
                    if let Some(item_ref) = state.current_list.get(idx).copied() {
                        if state.outdent_item(item_ref.id) {
                            ui::refresh_task_list(state);
                            // Reselect the outdented item
                            if let Some(new_idx) =
                                state.current_list.iter().position(|r| r.id == item_ref.id)
                            {
                                let _ = SendMessageW(
                                    state.hwnd_list,
                                    LB_SETCURSEL,
                                    Some(WPARAM(new_idx)),
                                    Some(LPARAM(0)),
                                );
                            }
                            schedule_save(hwnd);
                        }
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
                        let _ = storage::save_tasks(&state.tasks, state.next_id, &state.config);
                        state.dirty = false;
                    }
                    let _ = InvalidateRect(Some(state.hwnd_voice_btn), None, true);
                    let _ = InvalidateRect(Some(hwnd), None, true);
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
                let _ = InvalidateRect(Some(hwnd), None, true);
            }
            LRESULT(0)
        }

        // ── Custom message: Speech privacy policy or mic permission required ──
        voice::WM_VOICE_PERMISSION_REQUIRED => {
            if let Some(state) = get_state(hwnd) {
                state.is_recording_voice = false;
                let _ = KillTimer(Some(hwnd), VOICE_COUNTDOWN_TIMER_ID);
                let _ = InvalidateRect(Some(state.hwnd_voice_btn), None, true);
                let _ = InvalidateRect(Some(hwnd), None, true);
                voice::prompt_speech_permission(hwnd);
            }
            LRESULT(0)
        }

        // ── Button clicks, list selection, and commands ──
        WM_COMMAND => {
            let ctrl_id = (wparam.0 & 0xFFFF) as i32;
            let notify_code = ((wparam.0 >> 16) & 0xFFFF) as u16;

            match ctrl_id {
                1 => {
                    let focus = GetFocus();
                    if let Some(state) = get_state(hwnd) {
                        if focus == state.hwnd_input {
                            let _ = PostMessageW(Some(hwnd), ui::WM_ADD_TASK, WPARAM(0), LPARAM(0));
                        }
                    }
                }

                // ── Header buttons ──
                ui::IDC_BTN_PIN => {
                    if let Some(state) = get_state(hwnd) {
                        state.config.is_topmost = !state.config.is_topmost;
                        let pos = if state.config.is_topmost {
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
                        schedule_save(hwnd);
                    }
                }

                ui::IDC_BTN_SETTINGS => {
                    if let Some(state) = get_state(hwnd) {
                        state.view_mode = match state.view_mode {
                            ViewMode::Tasks => ViewMode::Settings(SettingsSubPage::AppSettings),
                            ViewMode::Settings(_) => ViewMode::Tasks,
                        };
                        ui::update_view_visibility(state);
                        let _ = InvalidateRect(Some(hwnd), None, true);
                    }
                }

                ui::IDC_BTN_CLOSE => {
                    let _ = ShowWindow(hwnd, SW_HIDE);
                }

                // ── Voice Input Button 🎙️ ──
                ui::IDC_BTN_VOICE => {
                    if let Some(state) = get_state(hwnd) {
                        if state.is_recording_voice {
                            voice::cancel_recording();
                            state.is_recording_voice = false;
                            let _ = KillTimer(Some(hwnd), VOICE_COUNTDOWN_TIMER_ID);
                            let _ = InvalidateRect(Some(state.hwnd_voice_btn), None, true);
                        } else {
                            state.is_recording_voice = true;
                            state.voice_seconds_remaining = state.config.record_time_seconds;
                            let _ = SetTimer(Some(hwnd), VOICE_COUNTDOWN_TIMER_ID, 1000, None);
                            voice::start_voice_recognition(hwnd, state.config.record_time_seconds);
                            let _ = InvalidateRect(Some(state.hwnd_voice_btn), None, true);
                        }
                    }
                }

                // ── Add Sub-task Button ＋ Sub ──
                ui::IDC_BTN_ADD_SUB => {
                    if let Some(state) = get_state(hwnd) {
                        if let Some(parent_id) = state.selected_task_id {
                            if let Some((_, completed, _, depth, _)) =
                                state.find_item_details(parent_id)
                            {
                                if !completed && depth < state.config.max_depth {
                                    state.sub_task_input_mode = true;
                                    ui::set_input_text(state.hwnd_input, "");
                                    ui::set_cue_banner(
                                        state.hwnd_input,
                                        "Add sub-task... (Press Enter)",
                                    );
                                    let _ = InvalidateRect(
                                        Some(state.hwnd_add_sub_btn),
                                        None,
                                        true,
                                    );
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

                // ── Settings Sub-page Navigation Tabs ──
                ui::IDC_BTN_TAB_SETTINGS => {
                    if let Some(state) = get_state(hwnd) {
                        state.view_mode = ViewMode::Settings(SettingsSubPage::AppSettings);
                        ui::update_view_visibility(state);
                        let _ = InvalidateRect(Some(hwnd), None, true);
                    }
                }
                ui::IDC_BTN_TAB_INFO => {
                    if let Some(state) = get_state(hwnd) {
                        state.view_mode = ViewMode::Settings(SettingsSubPage::Info);
                        ui::update_view_visibility(state);
                        let _ = InvalidateRect(Some(hwnd), None, true);
                    }
                }
                ui::IDC_BTN_TAB_LOGS => {
                    if let Some(state) = get_state(hwnd) {
                        state.view_mode = ViewMode::Settings(SettingsSubPage::Logs);
                        ui::update_view_visibility(state);
                        let _ = InvalidateRect(Some(hwnd), None, true);
                    }
                }
                ui::IDC_BTN_BACK => {
                    if let Some(state) = get_state(hwnd) {
                        state.view_mode = ViewMode::Tasks;
                        ui::update_view_visibility(state);
                        let _ = InvalidateRect(Some(hwnd), None, true);
                    }
                }

                // ── App Settings: Record time options (7s, 15s, 30s) ──
                ui::IDC_BTN_REC_7 => {
                    if let Some(state) = get_state(hwnd) {
                        state.config.record_time_seconds = 7;
                        crate::state::log_event(
                            crate::state::LogLevel::Info,
                            "Record time updated to 7s (Default)",
                        );
                        schedule_save(hwnd);
                        let _ = InvalidateRect(Some(hwnd), None, true);
                    }
                }
                ui::IDC_BTN_REC_15 => {
                    if let Some(state) = get_state(hwnd) {
                        state.config.record_time_seconds = 15;
                        crate::state::log_event(
                            crate::state::LogLevel::Info,
                            "Record time updated to 15s",
                        );
                        schedule_save(hwnd);
                        let _ = InvalidateRect(Some(hwnd), None, true);
                    }
                }
                ui::IDC_BTN_REC_30 => {
                    if let Some(state) = get_state(hwnd) {
                        state.config.record_time_seconds = 30;
                        crate::state::log_event(
                            crate::state::LogLevel::Info,
                            "Record time updated to 30s",
                        );
                        schedule_save(hwnd);
                        let _ = InvalidateRect(Some(hwnd), None, true);
                    }
                }

                // ── App Settings: Sub-level max depth options (1, 2, 3) ──
                ui::IDC_BTN_DEPTH_1 => {
                    if let Some(state) = get_state(hwnd) {
                        state.config.max_depth = 1;
                        crate::state::log_event(
                            crate::state::LogLevel::Info,
                            "Sub-level max depth set to 1",
                        );
                        ui::refresh_task_list(state);
                        schedule_save(hwnd);
                        let _ = InvalidateRect(Some(hwnd), None, true);
                    }
                }
                ui::IDC_BTN_DEPTH_2 => {
                    if let Some(state) = get_state(hwnd) {
                        state.config.max_depth = 2;
                        crate::state::log_event(
                            crate::state::LogLevel::Info,
                            "Sub-level max depth set to 2",
                        );
                        ui::refresh_task_list(state);
                        schedule_save(hwnd);
                        let _ = InvalidateRect(Some(hwnd), None, true);
                    }
                }
                ui::IDC_BTN_DEPTH_3 => {
                    if let Some(state) = get_state(hwnd) {
                        state.config.max_depth = 3;
                        crate::state::log_event(
                            crate::state::LogLevel::Info,
                            "Sub-level max depth set to 3 (Max)",
                        );
                        ui::refresh_task_list(state);
                        schedule_save(hwnd);
                        let _ = InvalidateRect(Some(hwnd), None, true);
                    }
                }

                // ── Logs View Actions: Test Mic & Clear Logs ──
                ui::IDC_BTN_TEST_MIC => {
                    if let Some(state) = get_state(hwnd) {
                        if !state.is_recording_voice {
                            state.is_recording_voice = true;
                            state.voice_seconds_remaining = state.config.record_time_seconds;
                            let _ = SetTimer(Some(hwnd), VOICE_COUNTDOWN_TIMER_ID, 1000, None);
                            voice::start_voice_recognition(hwnd, state.config.record_time_seconds);
                            let _ = InvalidateRect(Some(hwnd), None, true);
                        }
                    }
                }
                ui::IDC_BTN_CLEAR_LOGS => {
                    crate::state::clear_logs();
                    let _ = InvalidateRect(Some(hwnd), None, true);
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

                                if pt.x > list_rc.right - 34 {
                                    // Delete area (right 34px)
                                    state.delete_item(item_ref.id);
                                    ui::refresh_task_list(state);
                                    schedule_save(hwnd);
                                } else {
                                    let indent_px = item_ref.depth as i32 * 18;
                                    let check_start = 10 + indent_px;
                                    let check_end = check_start + 20;

                                    if pt.x >= check_start && pt.x <= check_end {
                                        // Checkbox area
                                        state.toggle_item(item_ref.id);
                                        ui::refresh_task_list(state);
                                        schedule_save(hwnd);
                                    } else {
                                        // Select item for nesting sub-tasks
                                        state.selected_task_id = Some(item_ref.id);
                                        let _ = InvalidateRect(
                                            Some(state.hwnd_add_sub_btn),
                                            None,
                                            true,
                                        );
                                    }
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
                        let _ = storage::save_tasks(&state.tasks, state.next_id, &state.config);
                        state.dirty = false;
                    }
                }
            } else if wparam.0 == VOICE_COUNTDOWN_TIMER_ID {
                if let Some(state) = get_state(hwnd) {
                    if state.voice_seconds_remaining > 0 {
                        state.voice_seconds_remaining -= 1;
                        let _ = InvalidateRect(Some(state.hwnd_voice_btn), None, true);
                        if matches!(state.view_mode, ViewMode::Settings(SettingsSubPage::Logs)) {
                            let _ = InvalidateRect(Some(hwnd), None, true);
                        }
                    }
                    if state.voice_seconds_remaining == 0 {
                        let _ = KillTimer(Some(hwnd), VOICE_COUNTDOWN_TIMER_ID);
                        state.is_recording_voice = false;
                        voice::cancel_recording();
                        let _ = InvalidateRect(Some(state.hwnd_voice_btn), None, true);
                        let _ = InvalidateRect(Some(hwnd), None, true);
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
                    let _ = storage::save_tasks(&state.tasks, state.next_id, &state.config);
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

/// Standard Win32 message loop with keyboard Tab navigation support.
pub fn run_message_loop(hwnd: HWND) {
    unsafe {
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            // Enter key dispatch for EDIT control
            if msg.message == WM_KEYDOWN && msg.wParam.0 == VK_RETURN.0 as usize {
                if let Some(state) = get_state(hwnd) {
                    if GetFocus() == state.hwnd_input {
                        let _ = TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                        continue;
                    }
                }
            }

            // Tab / Shift+Tab interception for task list indentation
            if msg.message == WM_KEYDOWN && msg.wParam.0 == VK_TAB.0 as usize {
                if let Some(state) = get_state(hwnd) {
                    if GetFocus() == state.hwnd_list {
                        let is_shift = (GetKeyState(VK_SHIFT.0 as i32) as u16 & 0x8000) != 0;
                        let target_msg = if is_shift {
                            ui::WM_LIST_OUTDENT
                        } else {
                            ui::WM_LIST_INDENT
                        };
                        let _ = PostMessageW(Some(hwnd), target_msg, WPARAM(0), LPARAM(0));
                        continue;
                    }
                }
            }

            if !IsDialogMessageW(hwnd, &msg).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
}
