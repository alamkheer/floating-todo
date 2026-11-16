# Architecture & System Design

This document details the architectural principles, source structure, native Win32/WinRT integrations, persistence layer, and performance characteristics of **Floating TODO**.

---

## Table of Contents

- [Core Design Philosophy](#core-design-philosophy)
- [Project Directory Layout](#project-directory-layout)
- [Native Windows Integrations](#native-windows-integrations)
  - [Raw Win32 API & windows-rs](#raw-win32-api--windows-rs)
  - [Owner-Drawn GDI & DWM Dark Theme](#owner-drawn-gdi--dwm-dark-theme)
  - [Per-Monitor V2 DPI Awareness](#per-monitor-v2-dpi-awareness)
  - [WinRT Speech Recognition](#winrt-speech-recognition)
- [Storage & Data Persistence](#storage--data-persistence)
  - [Storage Location & Schema](#storage-location--schema)
  - [Debounced Writes](#debounced-writes)
  - [Crash-Resilient Atomic Swapping](#crash-resilient-atomic-swapping)
- [Performance Profile](#performance-profile)

---

## Core Design Philosophy

Floating TODO was engineered from the ground up to challenge modern desktop app bloat:

1. **Zero Browser Framework Overhead:** No Chromium, Electron, CEF, or WebView2 runtime.
2. **Deterministic Resource Consumption:** Stable memory usage under 12 MB RAM, near 0% idle CPU.
3. **Pure Native Win32 Subsystem:** Direct calls to `user32.dll`, `gdi32.dll`, `dwmapi.dll`, and `shell32.dll`.
4. **Instant Startup Time:** Zero JIT compilation, zero JS parsing, launch latencies below 20ms.

---

## Project Directory Layout

```
windows-todo-app/
├── assets/                     # Application assets and icon resources
├── docs/                       # Modular documentation guides
│   ├── architecture.md         # System design and architecture (this file)
│   ├── building.md             # Compilation & build instructions
│   ├── installation.md         # Installer and deployment guide
│   ├── user-guide.md           # Features, shortcuts, and controls
│   ├── versioning-and-publishing.md # Versioning and CI/CD release workflow
│   └── troubleshooting.md      # Debugging, crash analysis, and common issues
├── src/
│   ├── main.rs                 # Entry point, CLI argument parser (--version), DPI awareness
│   ├── window.rs               # Window class registration, WndProc, drag handling, timers, Tab nav
│   ├── ui.rs                   # Win32 GDI owner-draw painting, dark palette, fonts, tooltips, controls
│   ├── state.rs                # Central AppState, Task/SubTask structs, filter modes, cascade logic
│   ├── storage.rs              # Atomic JSON persistence (%APPDATA%\FloatingTodo\tasks.json)
│   ├── voice.rs                # WinRT SpeechRecognizer dictation constraints & async worker
│   ├── tray.rs                 # System tray icon (Shell_NotifyIconW) & context menu
│   └── hotkey.rs               # Global keyboard hotkey hook (Ctrl + Shift + T)
├── build.rs                    # Auto-generates Windows PE VERSIONINFO resource from Cargo.toml
├── build.ps1                   # Dockerized build script (produces optimized floating-todo.exe)
├── install.ps1                 # Deployment script (AutoStart registry & Start Menu shortcut)
├── test.ps1                    # Architectural spec & unit verification test suite
├── tests.ps1                   # Trampoline wrapper for test.ps1
├── Dockerfile                  # Hermetic cross-compilation environment (rust + cargo-xwin + MSVC SDK)
├── Cargo.toml                  # Rust package manifest and aggressive size optimization flags
└── README.md                   # Primary project overview and documentation index
```

---

## Native Windows Integrations

### Raw Win32 API & windows-rs

Floating TODO communicates directly with the Windows kernel and user subsystems using the official Microsoft `windows-rs` (v0.62) crate. Modules target exact subsystem primitives:
- `Win32_UI_WindowsAndMessaging`: Custom `WndProc`, message loop dispatch, window subclassing.
- `Win32_Graphics_Gdi`: Device contexts (`HDC`), double-buffered painting, GDI fonts, solid brushes.
- `Win32_UI_Controls`: Standard controls (`EDIT`, `BUTTON`, `LISTBOX`) customized via owner-drawn hooks.
- `Win32_UI_Shell`: System tray icon lifecycle using `Shell_NotifyIconW` and `NOTIFYICONDATAW`.
- `Win32_UI_Input_KeyboardAndMouse`: System-wide hotkey registration (`RegisterHotKey`, `UnregisterHotKey`).

### Owner-Drawn GDI & DWM Dark Theme

To achieve a modern appearance without adding UI framework libraries:
- **DWM Dark Titlebar:** The window signals the Desktop Window Manager to apply Windows 11 / Windows 10 dark frame styling using `DwmSetWindowAttribute` with `DWMWA_USE_IMMERSIVE_DARK_MODE`.
- **Owner-Drawn Controls (`WM_DRAWITEM`):** Standard Win32 buttons and listbox entries intercept the `WM_DRAWITEM` message in `ui.rs`, painting custom dark background brushes, accent highlights, and clean typography with zero UI toolkit overhead.
- **Focus Rectangles (`ODS_FOCUS`):** Full keyboard navigation displays crisp native focus indicators for accessibility.

### Per-Monitor V2 DPI Awareness

Before any window handle is created, the application invokes:
```rust
SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
```
This guarantees crisp text rendering and appropriate scaling when the window is dragged across monitors with differing scaling factors (e.g., transitioning between a 100% 1080p display and a 150% 4K display).

### WinRT Speech Recognition

Voice dictation does not depend on cloud APIs or third-party web services:
- Initializes the Windows Runtime COM apartment.
- Leverages `Windows.Media.SpeechRecognition.SpeechRecognizer` with a `SpeechRecognitionTopicConstraint` configured for short dictation.
- Runs speech processing on a detached background worker thread communicating back to the Win32 message loop via custom Windows messages (`WM_USER`).

---

## Storage & Data Persistence

### Storage Location & Schema

Task records are persisted in the user's roaming application directory:
```
%APPDATA%\FloatingTodo\tasks.json
```

Example JSON structure:
```json
{
  "next_id": 3,
  "tasks": [
    {
      "id": 1,
      "text": "Review pull request #42",
      "completed": false,
      "sub_tasks": [
        {
          "id": 101,
          "text": "Check unit test coverage",
          "completed": true
        }
      ]
    },
    {
      "id": 2,
      "text": "Deploy release v0.2.3",
      "completed": false,
      "sub_tasks": []
    }
  ]
}
```

The storage module includes backward compatibility handlers capable of seamlessly migrating legacy v1 schema files lacking the `sub_tasks` field.

### Debounced Writes

To prevent excessive disk thrashing during rapid user input (e.g. rapid typing or multi-item reordering), task updates trigger a 2-second debounce timer (`SAVE_DEBOUNCE_MS = 2000`). Once the timer elapses without further modifications, the persistence layer commits the state to disk.

### Crash-Resilient Atomic Swapping

To prevent file corruption during sudden system shutdowns, power interruptions, or OS crashes, `src/storage.rs` utilizes atomic replacement semantics:
1. Serialize the state to a sibling temporary file:
   `%APPDATA%\FloatingTodo\tasks.json.tmp`
2. Force disk caches to flush using file handle synchronization (`file.sync_all()`).
3. Atomically rename/replace `tasks.json.tmp` over `tasks.json` using standard OS filesystem replacement primitives.

---

## Performance Profile

| Metric | Target Specification | Measured Result |
|---|---|---|
| **Binary Size** | < 1 MB | **~231 KB** |
| **Idle Working Set RAM** | < 15 MB | **< 12 MB** |
| **Idle CPU Utilization** | < 0.1% | **~0.0%** |
| **Cold Startup Latency** | < 100 ms | **< 25 ms** |
| **External Dependencies** | 0 DLLs | Fully statically linked to native OS system DLLs |
