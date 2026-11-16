# Floating TODO

> **Ultra-lightweight, zero-bloat floating desktop TODO widget built with native Win32 API and Rust.**

Floating TODO is a high-performance Windows desktop companion designed for developers and power users who want an unobtrusive, always-accessible task list. Engineered in Rust using raw Win32 bindings and WinRT speech APIs (`windows-rs`), it runs without web runtimes, browser frameworks, or heavy abstraction layers.

- **Executable Size:** ~231 KB standalone binary (fully self-contained, no external DLL dependencies).
- **Memory Footprint:** Less than 12 MB RAM at runtime.
- **Framework Overhead:** 0% — No Electron, no WebView2, no Chromium.

---

## Quick Start

### 1. Run the Executable Directly
If you already have `floating-todo.exe`:
```powershell
.\floating-todo.exe
```

### 2. Build with Docker (One Command)
No Rust or Visual Studio installation required on host:
```powershell
.\build.ps1
```

### 3. Install or Update (Interactive Wizard)
Deploys to `%LOCALAPPDATA%\FloatingTodo`, registers shortcuts, and configures auto-start:
```powershell
.\install.ps1
```

---

## Release Executable Version & Publishing Information

### Current Executable Specifications

| Attribute | Specification |
|---|---|
| **Current Release Version** | `v0.2.3` |
| **Target Architecture** | `x86_64-pc-windows-msvc` (64-bit AMD64) |
| **Subsystem** | Windows GUI (`IMAGE_SUBSYSTEM_WINDOWS_GUI`) |
| **Binary Footprint** | ~231 KB (LTO stripped, `opt-level = "z"`) |
| **Dependencies** | 0 external runtime DLLs (statically bound to Windows OS subsystem) |

### Embedded Windows PE Version Metadata

Every release build of Floating TODO automatically generates and embeds a native Windows Portable Executable (PE) `VERSIONINFO` resource into the binary headers via `build.rs`.

This ensures that:
- **Windows Event Viewer** records the exact application version on crashes (`Event ID: 1000`) instead of `0.0.0.0`.
- **Windows Error Reporting (WER)** crash dumps and minidumps contain accurate build headers.
- **System Tray & File Explorer** expose official version metadata to users without developer tools.

### Checking the Release Version

You can verify the release version through any of the following methods:

```powershell
# 1. Command-Line Interface flag
.\floating-todo.exe --version

# 2. PowerShell file version query
(Get-Item .\floating-todo.exe).VersionInfo | Format-List FileVersion, ProductVersion, FileDescription
```

- **File Explorer:** Right-click `floating-todo.exe` → **Properties** → **Details** tab.
- **System Tray:** Hover over the system tray icon, or right-click to view the version header in the context menu.

### Publishing a New Release

Floating TODO supports both automated CI/CD publishing and local release creation:

#### Method A: Automated GitHub Actions CI/CD (Recommended)
The repository includes an automated release workflow (`.github/workflows/release.yml`):
1. Bump the version in `Cargo.toml`:
   ```toml
   [package]
   version = "0.2.4"
   ```
2. Commit and push a git release tag:
   ```powershell
   git commit -am "Release v0.2.4"
   git tag -a v0.2.4 -m "Release v0.2.4"
   git push origin main --tags
   ```
3. GitHub Actions builds the release executable, generates cryptographic SHA-256 checksums (`floating-todo-v0.2.4.exe.sha256`), and publishes a new GitHub Release with release notes.

#### Method B: Local Automated Build & Version Bump
Run the installer with the `-Build` flag to increment the patch version, compile via Docker, and update `Cargo.toml`:
```powershell
.\install.ps1 -Build
```

For full details on versioning architecture and release workflows, see [docs/versioning-and-publishing.md](docs/versioning-and-publishing.md).

---

## Primary Controls & Shortcuts

| Shortcut / Action | Scope | Description |
|---|---|---|
| `Ctrl + Shift + T` | Global (System-wide) | Toggle floating window visibility (Show / Hide / Focus). |
| `Tab` / `Shift + Tab` | Window | Cycle keyboard focus forward and backward across all controls. |
| `Enter` | Input Box | Adds a new parent task (or adds a sub-task if sub-task mode is active). |
| `Esc` | Input Box | Cancels sub-task input mode. |
| `Space` | List View | Toggles selected task between active and completed. |
| `Delete` | List View | Permanently deletes the selected task. |
| Click `＋ Sub` | Control Bar | Adds a sub-task to the currently selected active task. |
| Click `🎙️` | Header Bar | Starts 30-second WinRT voice dictation. |
| Click `📌` | Header Bar | Toggles Always-on-Top pinning mode. |
| Click `✕` | Header Bar | Minimizes widget directly to the Windows System Tray. |

---

## Documentation Index

Detailed technical guides and documentation are organized in the [docs/](docs/) directory:

| Document | Description |
|---|---|
| [User Guide & Controls](docs/user-guide.md) | Comprehensive feature tour, keyboard navigation, voice dictation, and sub-task cascading logic. |
| [Architecture & System Design](docs/architecture.md) | Project layout, Win32 raw API bindings, atomic JSON persistence, and performance benchmarks. |
| [Building from Source](docs/building.md) | Hermetic Docker build pipeline, native Windows Cargo compilation, and compiler optimization flags. |
| [Installation & Deployment](docs/installation.md) | Interactive installer, unattended switches, file paths, and graceful uninstallation. |
| [App Versioning & Publishing](docs/versioning-and-publishing.md) | PE `VERSIONINFO` embedding, version verification, CI/CD pipeline, and release checksums. |
| [Debugging & Troubleshooting](docs/troubleshooting.md) | Windows Event Viewer diagnostics, crash dump analysis, corrupted storage recovery, and hotkey conflict resolution. |

---

## License

This project is open-source software licensed under the [MIT License](LICENSE).
