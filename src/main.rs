#![windows_subsystem = "windows"]

mod hotkey;
mod state;
mod storage;
mod tray;
mod ui;
mod window;

use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};

fn main() -> windows::core::Result<()> {
    // 0. Check for CLI version query
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--version" || a == "-v" || a == "/v" || a == "version") {
        let ver = env!("CARGO_PKG_VERSION");
        let info = format!(
            "Floating TODO v{}\nArchitecture: x86_64-pc-windows-msvc\nEdition: 2021\nConfig: %APPDATA%\\FloatingTodo\\tasks.json\nHotkeys: Ctrl+Shift+T (Toggle show/hide)\n",
            ver
        );
        unsafe {
            use windows::Win32::System::Console::{
                AttachConsole, GetStdHandle, WriteConsoleW, ATTACH_PARENT_PROCESS, STD_OUTPUT_HANDLE,
            };
            if AttachConsole(ATTACH_PARENT_PROCESS).is_ok() {
                if let Ok(handle) = GetStdHandle(STD_OUTPUT_HANDLE) {
                    let u16_chars: Vec<u16> = info.encode_utf16().collect();
                    let _ = WriteConsoleW(handle, &u16_chars, None, None);
                }
            } else {
                use windows::core::PCWSTR;
                use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONINFORMATION, MB_OK};
                let title = windows::core::w!("Floating TODO");
                let msg_u16: Vec<u16> = format!("{}\0", info).encode_utf16().collect();
                let _ = MessageBoxW(None, PCWSTR(msg_u16.as_ptr()), title, MB_OK | MB_ICONINFORMATION);
            }
        }
        return Ok(());
    }

    // 1. Set Per-Monitor V2 DPI awareness BEFORE any window creation
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }

    // 2. Load persisted tasks
    let (tasks, next_id) = storage::load_tasks();
    let mut state = Box::new(state::AppState::new());
    state.tasks = tasks;
    state.next_id = next_id;

    // 3. Create the main window (passes state ownership into GWLP_USERDATA)
    let hwnd = window::create_main_window(state)?;

    // 4. Register global hotkey (RAII — auto-unregisters on drop)
    let _hotkey = hotkey::GlobalHotkey::register(hwnd);

    // 5. Create system tray icon (RAII — auto-removes on drop)
    let _tray = tray::TrayIcon::new(hwnd)?;

    // 6. Enter the message loop (blocks until WM_QUIT)
    window::run_message_loop();

    // _tray and _hotkey Drop here → cleanup is automatic
    Ok(())
}
