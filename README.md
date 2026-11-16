# Floating TODO

> **Ultra-lightweight, zero-bloat floating desktop TODO widget built with native Win32 API and Rust.**

Floating TODO is a high-performance Windows desktop companion designed for developers and power users who want an unobtrusive, always-accessible task list. Engineered in Rust using raw Win32 bindings and WinRT speech APIs (`windows-rs`), it runs without web runtimes, browser frameworks, or heavy abstraction layers.

- **Executable Size:** ~390 KB standalone binary (fully self-contained, no external DLL dependencies).
- **Memory Footprint:** Less than 12 MB RAM at runtime.
- **Framework Overhead:** 0% — No Electron, no WebView2, no Chromium.

---

## Table of Contents

1. [Features](#features)
2. [Keyboard Shortcuts & Controls](#keyboard-shortcuts--controls)
3. [Architecture & Project Layout](#architecture--project-layout)
4. [Prerequisites](#prerequisites)
5. [Building from Source](#building-from-source)
6. [Installation & Setup](#installation--setup)
7. [App Publish Versioning & Error Debugging](#app-publish-versioning--error-debugging)
   - [Why Versioning Matters for Debugging](#why-versioning-matters-for-debugging)
   - [How Versioning is Embedded (Windows PE Resource)](#how-versioning-is-embedded-windows-pe-resource)
   - [Checking the App Version](#checking-the-app-version)
   - [Publishing a New Release](#publishing-a-new-release)
   - [Debugging Errors Under Specific Versions](#debugging-errors-under-specific-versions)
8. [Storage & Data Persistence](#storage--data-persistence)
9. [Troubleshooting](#troubleshooting)
10. [License](#license)

---

## Features

- **Blazing Fast & Ultra-Lightweight:** Compiled with size-optimizing link-time optimization (`opt-level = "z"`, LTO, stripped binary), launching instantly.
- **Voice Dictation Input (WinRT Speech Recognition):** One-click 30-second voice dictation powered by `Windows.Media.SpeechRecognition` with speech topic dictation constraints. Features real-time countdown timer and automatically adds recognized speech as a new task entry.
- **Hierarchical Sub-Tasks & Cascading Completion:** Select any active task and click `＋ Sub` to attach sub-tasks. Indented with clean `└─` visual tree indicators. Automatically marks the parent task completed when all sub-tasks are done, and re-opens the parent if any sub-task is unchecked.
- **Quick-Delete for Done Tasks:** Completed tasks feature a small `🗑` emoji delete button for immediate, clutter-free task cleanup.
- **Modern Emoji Header Controls & Balloon Tooltips:** Native `📌` Always-on-Top toggle, `🎙️` Voice Dictation trigger, and rich balloon popovers (`tooltips_class32`) explaining each control.
- **Full Keyboard Accessibility:** Seamless `Tab` and `Shift + Tab` navigation through all buttons, inputs, and list items with native focus rectangles (`ODS_FOCUS`). Use `Space` to toggle completion and `Delete` to remove items directly in the list.
- **Native Dark Theme:** Owner-drawn controls using Win32 GDI with Desktop Window Manager (DWM) immersive dark titlebar integration (`DwmSetWindowAttribute`).
- **Per-Monitor V2 High-DPI Aware:** Configured prior to window initialization (`SetProcessDpiAwarenessContext`), ensuring crisp text and correct layouts on multi-monitor setups with mixed scaling (100%, 125%, 150%, 200%).
- **Global Hotkey:** Press `Ctrl + Shift + T` from anywhere in Windows to immediately toggle or focus the floating window.
- **Always-on-Top Pinning:** Pin the window to float above all other applications, or unpin it to behave like a standard window.
- **Frameless Draggable Window:** Clean borderless design; simply drag anywhere along the top header bar to reposition.
- **System Tray Companion:** Closing the window minimizes it directly to the Windows system tray. The tray context menu offers instant visibility toggle, pin status toggle, version inspection, and clean exit.
- **Smart Filtering:** Switch effortlessly between **All**, **Active**, and **Done** task views.
- **Crash-Resilient Atomic Persistence:** Tasks are stored in `%APPDATA%\FloatingTodo\tasks.json` using atomic temporary file swaps (`.tmp` write followed by atomic rename) with complete backward compatibility for legacy v1 format.

---

## Keyboard Shortcuts & Controls

| Shortcut / Action | Scope | Description |
|---|---|---|
| `Ctrl + Shift + T` | Global (System-wide) | Toggle floating window visibility (Show / Hide / Focus). |
| `Tab` / `Shift + Tab` | Window | Cycle keyboard focus forward and backward across all controls. |
| `Enter` | Input Box | Adds a new parent task (or adds a sub-task if sub-task input mode is active). |
| `Esc` | Input Box | Cancels sub-task input mode and returns to normal parent task entry. |
| `Space` | List View | Toggles selected task or sub-task between active and completed. |
| `Delete` | List View | Permanently deletes the selected task or sub-task. |
| Left Click on `[✓]` | Task Item | Toggles task between active and completed. |
| Left Click on `🗑` | Completed Task Item | Permanently deletes the completed entry. |
| Click `＋ Sub` Button | Control Bar | Adds a sub-task to the currently selected active task. |
| Click `🎙️` Button | Header Bar | Starts 30-second voice dictation (click again to cancel). |
| Click Filter Buttons | Header Bar | Filters view: `All`, `Active`, `Done`. |
| Click `📌` Pin Button | Header Bar | Toggles Always-on-Top mode (with hover balloon tooltip). |
| Click `✕` Close Button | Header Bar | Minimizes widget to the System Tray. |
| Right Click Tray Icon | System Tray | Opens context menu (Version, Show/Hide, Always-on-Top, Exit). |
| Left Click Tray Icon | System Tray | Toggles window visibility. |
| Drag Header Bar | Title Area | Repositions the floating window across monitors. |

---

## Architecture & Project Layout

```
windows-todo-app/
├── assets/                     # Application assets and icon resources
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
├── Dockerfile                  # Hermetic cross-compilation environment (rust + cargo-xwin + MSVC SDK)
├── Cargo.toml                  # Rust package manifest and aggressive size optimization flags
└── README.md                   # Project documentation
```

---

## Prerequisites

- **Operating System:** Windows 10 (Build 1809 or higher) or Windows 11 (64-bit).
- **For Building (Option 1 - Recommended):** [Docker Desktop](https://www.docker.com/products/docker-desktop/) (no Windows SDK or MSVC installation needed on host).
- **For Building (Option 2 - Native):**
  - [Rust toolchain](https://www.rust-lang.org/) (stable, 2021 edition).
  - Target: `rustup target add x86_64-pc-windows-msvc`.
  - Visual Studio C++ Build Tools (including Windows SDK).

---

## Building from Source

### Method A: Automated Docker Build (Recommended)

The project includes a hermetic Docker build environment using `cargo-xwin` and the Windows MSVC SDK. You do not need Rust or Visual Studio installed on your host machine.

Run the PowerShell build script:

```powershell
.\build.ps1
```

**What `build.ps1` does:**
1. Builds a Docker image containing the Rust toolchain, `cargo-xwin`, Clang/LLD, and Windows SDK headers.
2. Compiles the binary with size optimization (`opt-level = "z"`, LTO, strip).
3. Compiles the Windows PE version resource (`VERSIONINFO`) into the executable via `llvm-rc`.
4. Extracts `floating-todo.exe` directly into the project root directory.

### Method B: Native Windows Build (Cargo)

If you have Rust and Visual Studio C++ Build Tools installed on Windows:

```powershell
# Compile release binary
cargo build --release

# The resulting binary is located at:
# target\release\floating-todo.exe
```

### Binary Size Profile

The release build is configured in `Cargo.toml` for minimal binary size:

```toml
[profile.release]
opt-level = "z"        # Optimize aggressively for binary size
lto = true             # Full link-time optimization across all crates
codegen-units = 1      # Single codegen unit for maximum LTO compression
strip = true           # Strip debug symbols and symbol tables
panic = "abort"        # Abort on panic (removes unwinding machinery)
```

---

## Installation & Setup

Deploy the application using the automated `install.ps1` script:

```powershell
# 1. Standard install (installs to %LOCALAPPDATA%\FloatingTodo)
.\install.ps1

# 2. Install with automatic startup on Windows login
.\install.ps1 -AutoStart

# 3. Install with Start Menu shortcut
.\install.ps1 -Shortcut

# 4. Full installation (Start Menu shortcut + AutoStart on boot)
.\install.ps1 -AutoStart -Shortcut
```

### Installation File Paths

- **Application Binary:**  
  `%LOCALAPPDATA%\FloatingTodo\floating-todo.exe`
- **Task Storage File:**  
  `%APPDATA%\FloatingTodo\tasks.json`
- **Auto-Start Registry Entry:**  
  `HKCU:\Software\Microsoft\Windows\CurrentVersion\Run\FloatingTodo`
- **Start Menu Shortcut:**  
  `%APPDATA%\Microsoft\Windows\Start Menu\Programs\Floating TODO.lnk`

### Uninstallation

To remove Floating TODO completely, run the following PowerShell commands:

```powershell
# Stop running instance
Stop-Process -Name "floating-todo" -Force -ErrorAction SilentlyContinue

# Remove autostart registry entry
Remove-ItemProperty -Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run" -Name "FloatingTodo" -ErrorAction SilentlyContinue

# Remove Start Menu shortcut
Remove-Item "$env:APPDATA\Microsoft\Windows\Start Menu\Programs\Floating TODO.lnk" -ErrorAction SilentlyContinue

# Remove application directory
Remove-Item -Recurse -Force "$env:LOCALAPPDATA\FloatingTodo"
```

*(Note: If you wish to preserve your saved tasks, keep `%APPDATA%\FloatingTodo\tasks.json`)*.

---

## App Publish Versioning & Error Debugging

### Why Versioning Matters for Debugging

When publishing a native Windows desktop app, proper versioning is critical for debugging crashes and error reports:

1. **Windows Event Viewer & Error Reporting (WER):** When a crash occurs, Windows records an Application Error (`Event ID: 1000`). If the binary lacks embedded Windows PE version metadata, Event Viewer reports the application version as `0.0.0.0` or empty. With embedded metadata, Event Viewer records the exact version (e.g., `0.1.0.0`), allowing developers to pinpoint which release failed.
2. **Crash Dump Analysis:** User-generated minidumps (`.dmp`) store module version headers. Having exact version metadata ensures symbol servers or local `.pdb` files match the crashed executable.
3. **User Bug Reports:** Users can check their installed version directly via the CLI, Explorer properties, or System Tray without needing developer tools.

---

### How Versioning is Embedded (Windows PE Resource)

Floating TODO uses a dedicated `build.rs` script that hooks into the build pipeline:
1. It reads `version`, `name`, and `description` directly from `Cargo.toml` (`CARGO_PKG_VERSION`).
2. It dynamically synthesizes a Windows Resource file (`version.rc`) containing a complete `VERSIONINFO` structure:
   - `FILEVERSION` & `PRODUCTVERSION` (e.g. `0,1,0,0`)
   - `FileVersion` & `ProductVersion` strings
   - `FileDescription`, `InternalName`, and `OriginalFilename`
3. It compiles the resource using `llvm-rc` / `rc.exe` into an object (`.res`) and instructs the linker (`lld-link`) to embed it directly into the final Portable Executable (PE) headers.

---

### Checking the App Version

You can verify the published version through multiple methods:

#### 1. Command-Line Interface (`--version` / `-v`)
Run the executable with the version flag from PowerShell or Command Prompt:

```powershell
.\floating-todo.exe --version
```

Output:
```text
Floating TODO v0.1.0
Architecture: x86_64-pc-windows-msvc
Edition: 2021
Config: %APPDATA%\FloatingTodo\tasks.json
Hotkeys: Ctrl+Shift+T (Toggle show/hide)
```
*(If run from outside a console, e.g. via Windows Run dialog, it opens an informational dialog box).*

#### 2. PowerShell File Version Query
Query the embedded Windows PE header:

```powershell
(Get-Item .\floating-todo.exe).VersionInfo | Format-List FileVersion, ProductVersion, FileDescription
```

Output:
```text
FileVersion     : 0.1.0.0
ProductVersion  : 0.1.0
FileDescription : Ultra-lightweight floating TODO window for Windows
```

#### 3. Windows File Explorer GUI
- Right-click `floating-todo.exe` → Select **Properties**.
- Navigate to the **Details** tab.
- Inspect **File version**, **Product name**, and **Product version**.

#### 4. In-App System Tray
- **Hover:** Hover your mouse over the system tray icon to view the tooltip: `Floating TODO v0.1.0`.
- **Right-Click:** The top header of the context menu explicitly displays the active version.

---

### Publishing a New Release

To publish a new version of Floating TODO:

1. **Update `Cargo.toml`:**
   Increment the version number following [Semantic Versioning](https://semver.org/):
   ```toml
   [package]
   name = "floating-todo"
   version = "0.1.1" # Bump version here
   ```
2. **Rebuild the Executable:**
   ```powershell
   .\build.ps1
   ```
   `build.rs` automatically updates the embedded `VERSIONINFO` PE header and recompiles the binary with the new version stamp.
3. **Create a Git Tag:**
   ```powershell
   git add Cargo.toml
   git commit -m "Release v0.1.1"
   git tag -a v0.1.1 -m "Release v0.1.1"
   ```
4. **Publish:**
   Distribute `floating-todo.exe` via GitHub Releases or your deployment pipeline.

---

### Debugging Errors Under Specific Versions

When diagnosing errors or unexpected behavior reported on a specific version:

#### 1. Diagnosing Crashes via Windows Event Viewer
If the application closes unexpectedly:
1. Press `Win + R`, type `eventvwr.msc`, and press **Enter**.
2. Navigate to **Windows Logs** → **Application**.
3. In the right pane, click **Filter Current Log...** and enter `1000` in the Event IDs box.
4. Locate the error entry for `floating-todo.exe`:
   ```text
   Faulting application name: floating-todo.exe, version: 0.1.0.0
   Faulting module name: ntdll.dll, version: 10.0.22621.1
   Exception code: 0xc0000005
   Fault offset: 0x0000000000012345
   Faulting process id: 0x1a2b
   ```
5. Check the `version` field in the log to confirm whether the user is on the latest version or an older build.

#### 2. Inspecting Crash Dumps (WER)
Windows Error Reporting generates crash dump files in:
```powershell
%LOCALAPPDATA%\CrashDumps\floating-todo.exe.*.dmp
```
Open these dump files in **Visual Studio** or **WinDbg** to inspect the call stack at the time of failure.

#### 3. Building with Debug Symbols for Development
Because the release profile strips symbols (`strip = true`) and aborts on panic (`panic = "abort"`), reproduce tricky bugs with an unstripped build:

```powershell
# Build in debug mode (keeps symbols and stack unwinding)
cargo build

# Run with full backtrace enabled
$env:RUST_BACKTRACE=1
.\target\debug\floating-todo.exe
```

#### 4. Validating and Recovering Corrupted Tasks JSON
If tasks fail to load, check the persistence directory:
```powershell
Get-Content "$env:APPDATA\FloatingTodo\tasks.json" | ConvertFrom-Json
```
If the file is damaged, check if a temporary write was left behind:
```powershell
Test-Path "$env:APPDATA\FloatingTodo\tasks.json.tmp"
```

#### 5. Resolving Global Hotkey Conflicts
If `Ctrl + Shift + T` does not toggle the window:
- Another application (e.g. Google Chrome "Reopen closed tab", Microsoft PowerToys, or an IDE) may have registered `Ctrl + Shift + T` first.
- The app handles this gracefully without crashing; `RegisterHotKey` returns `None` and the app continues running via the system tray icon.
- You can change the registered hotkey combination in `src/hotkey.rs`:
  ```rust
  RegisterHotKey(Some(hwnd), HOTKEY_TOGGLE_ID, MOD_CONTROL | MOD_SHIFT, VK_T.0 as u32);
  ```

---

## Storage & Data Persistence

Tasks are persisted as JSON in `%APPDATA%\FloatingTodo\tasks.json`.

```json
{
  "next_id": 3,
  "tasks": [
    {
      "id": 1,
      "text": "Review pull request #42",
      "completed": false
    },
    {
      "id": 2,
      "text": "Deploy release v0.1.0",
      "completed": true
    }
  ]
}
```

- **Debounced Saving:** Edits to tasks trigger a 2-second debounce timer (`SAVE_DEBOUNCE_MS`), batching rapid keystrokes to minimize disk I/O.
- **Atomic File Swapping:** The app writes to `tasks.json.tmp`, flushes buffers to disk (`sync_all`), and atomically renames the temporary file over `tasks.json` to prevent partial writes if power is cut.

---

## Troubleshooting

| Issue | Cause | Solution |
|---|---|---|
| `Ctrl + Shift + T` does nothing | Hotkey registered by another program | Close conflicting apps or trigger visibility via the System Tray icon. |
| Window disappears | Minimized to tray | Check the Windows taskbar system tray area; click the Floating TODO icon. |
| Tasks not saving | File permissions in `%APPDATA%` | Ensure `%APPDATA%\FloatingTodo` is writable by the current user. |
| Docker build error | Docker Desktop daemon not running | Launch Docker Desktop and verify with `docker info` in PowerShell. |

---

## License

This project is open-source software licensed under the [MIT License](LICENSE).
