# Installation & Deployment

This guide explains how to install, configure, update, and uninstall **Floating TODO** using the automated deployment script `install.ps1`.

---

## Table of Contents

- [Overview](#overview)
- [Quick Start](#quick-start)
- [Interactive Setup Wizard](#interactive-setup-wizard)
- [Unattended / Automated Deployment](#unattended--automated-deployment)
- [Installation File Paths](#installation-file-paths)
- [Uninstallation](#uninstallation)

---

## Overview

Floating TODO provides a self-contained PowerShell installation script (`install.ps1`). It handles:
- Detecting existing versions and file locks.
- Optional automatic Docker builds with patch version increments.
- Safe process termination and file deployment.
- Windows user registry auto-start configuration.
- Start Menu and Desktop shortcut creation.
- Graceful, clean uninstallation with task data protection.

---

## Quick Start

To launch the interactive installer:

```powershell
.\install.ps1
```

If an existing installation is detected, the script identifies the installed version and prompts you before proceeding with upgrades or reinstallation.

---

## Interactive Setup Wizard

Running `install.ps1` without flags runs the interactive wizard:

```powershell
# Interactive install / update wizard
.\install.ps1
```

### Wizard Actions:
1. **Docker Build Option:** Prompts if you would like to rebuild a fresh executable from source via Docker, automatically bumping the patch version in `Cargo.toml`.
2. **Existing Version Detection:** Inspects `%LOCALAPPDATA%\FloatingTodo\floating-todo.exe` and displays its embedded version.
3. **Graceful Replacement:** Prompts to safely close any currently running instance of Floating TODO before updating files.
4. **Auto-Start Prompt:** Asks if Floating TODO should launch on Windows user login.
5. **Shortcut Configuration:** Asks if Start Menu and Desktop shortcuts should be generated.
6. **Immediate Launch:** Asks whether to start the application upon completion.

---

## Unattended / Automated Deployment

For headless environments, scripted setups, or automated testing, `install.ps1` accepts specific parameter switches:

```powershell
# 1. Build a new Docker version (bumps patch in Cargo.toml) and install interactively
.\install.ps1 -Build

# 2. Automated unattended install with specific flags
.\install.ps1 -AutoStart -Shortcut -DesktopShortcut -Launch -NonInteractive

# 3. Unattended Docker build + full setup + launch
.\install.ps1 -Build -AutoStart -Shortcut -Launch -NonInteractive
```

### Command-Line Parameters Reference

| Switch | Type | Description |
|---|---|---|
| `-Build` / `-DockerBuild` | Switch | Automatically builds a new executable using Docker and increments patch version in `Cargo.toml`. |
| `-AutoStart` | Switch | Adds Floating TODO to `HKCU:\Software\Microsoft\Windows\CurrentVersion\Run`. |
| `-Shortcut` | Switch | Creates a Start Menu shortcut under `%APPDATA%\Microsoft\Windows\Start Menu\Programs`. |
| `-DesktopShortcut` | Switch | Creates a Desktop shortcut under `%USERPROFILE%\Desktop`. |
| `-Launch` | Switch | Automatically launches the installed application after installation finishes. |
| `-NonInteractive` | Switch | Suppresses all prompts; uses provided switches or defaults. |
| `-Interactive` | Switch | Enforces interactive prompts even if some switches are supplied. |
| `-Uninstall` | Switch | Initiates the uninstallation sequence. |

---

## Installation File Paths

Floating TODO strictly abides by standard Windows application conventions:

| Component | Filesystem / Registry Location |
|---|---|
| **Application Binary** | `%LOCALAPPDATA%\FloatingTodo\floating-todo.exe` |
| **Task Storage Database** | `%APPDATA%\FloatingTodo\tasks.json` |
| **Auto-Start Registry Key** | `HKCU:\Software\Microsoft\Windows\CurrentVersion\Run\FloatingTodo` |
| **Start Menu Shortcut** | `%APPDATA%\Microsoft\Windows\Start Menu\Programs\Floating TODO.lnk` |
| **Desktop Shortcut (Optional)** | `%USERPROFILE%\Desktop\Floating TODO.lnk` |

---

## Uninstallation

Floating TODO includes a graceful uninstaller directly within `install.ps1` that never performs silent or destructive file deletions:

```powershell
# Interactive uninstallation
.\install.ps1 -Uninstall
```

### Uninstallation Workflow:
1. **Process Safety:** Checks if `floating-todo.exe` is currently running and prompts for permission to shut it down gracefully.
2. **Registry Cleanup:** Removes the `FloatingTodo` value from `HKCU:\Software\Microsoft\Windows\CurrentVersion\Run`.
3. **Shortcut Cleanup:** Deletes Start Menu and Desktop `.lnk` shortcuts.
4. **Binary Deletion:** Removes `%LOCALAPPDATA%\FloatingTodo\floating-todo.exe` and cleans up the directory.
5. **User Data Safeguard:** Prompts whether you want to **preserve** or **delete** your saved task history located at `%APPDATA%\FloatingTodo\tasks.json` (Defaults to preserving data).
