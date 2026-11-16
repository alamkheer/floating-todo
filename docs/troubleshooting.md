# Debugging & Troubleshooting

This guide provides procedures for diagnosing crashes, inspecting Windows Error Reporting (WER) logs, resolving hotkey conflicts, recovering corrupted data, and troubleshooting common issues in **Floating TODO**.

---

## Table of Contents

- [Diagnosing Crashes by Version](#diagnosing-crashes-by-version)
  - [Windows Event Viewer (Event ID: 1000)](#windows-event-viewer-event-id-1000)
  - [Inspecting Crash Dumps (WER)](#inspecting-crash-dumps-wer)
- [Development Debugging & Stack Traces](#development-debugging--stack-traces)
- [Validating and Recovering Corrupted Tasks JSON](#validating-and-recovering-corrupted-tasks-json)
- [Resolving Global Hotkey Conflicts](#resolving-global-hotkey-conflicts)
- [Troubleshooting Common Issues](#troubleshooting-common-issues)

---

## Diagnosing Crashes by Version

### Windows Event Viewer (Event ID: 1000)

If Floating TODO terminates unexpectedly:

1. Press `Win + R`, type `eventvwr.msc`, and press **Enter**.
2. Navigate to **Windows Logs** → **Application**.
3. In the right-hand Actions pane, click **Filter Current Log...**.
4. Enter `1000` into the Event IDs filter box and click **OK**.
5. Locate the error entry for `floating-todo.exe`:
   ```text
   Faulting application name: floating-todo.exe, version: 0.2.3.0
   Faulting module name: ntdll.dll, version: 10.0.22621.1
   Exception code: 0xc0000005
   Fault offset: 0x0000000000012345
   Faulting process id: 0x1a2b
   ```
6. Check the `version` field in the log. Because Floating TODO embeds Windows PE `VERSIONINFO` metadata, the exact version of the failing binary is logged, identifying whether the user is on an outdated build.

---

### Inspecting Crash Dumps (WER)

Windows Error Reporting generates user-mode crash dump files in:

```powershell
%LOCALAPPDATA%\CrashDumps\floating-todo.exe.*.dmp
```

To enable full user-mode dumps in Windows if not already active:
- Key: `HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\Windows\Windows Error Reporting\LocalDumps\floating-todo.exe`
- Value: `DumpType` (DWORD) = `2` (Full dump)

Open the resulting `.dmp` file in **Visual Studio** or **WinDbg** to analyze the register state, faulting instruction, and thread call stack.

---

## Development Debugging & Stack Traces

Because the production release profile strips debug symbols (`strip = true`) and aborts on panic (`panic = "abort"`), reproducing difficult edge cases should be done with a native debug build:

```powershell
# 1. Compile in debug mode (retains full symbols and unwinding machinery)
cargo build

# 2. Enable verbose Rust backtraces in the environment
$env:RUST_BACKTRACE=1

# 3. Launch debug binary
.\target\debug\floating-todo.exe
```

---

## Validating and Recovering Corrupted Tasks JSON

If tasks appear missing or fail to load on startup, check the persistence directory:

```powershell
# 1. Check if the task file exists and contains valid JSON
Get-Content "$env:APPDATA\FloatingTodo\tasks.json" | ConvertFrom-Json
```

### Checking for Atomic Write Leftovers:
Floating TODO writes to a temporary file before atomic renaming. If power was lost during disk flush, a `.tmp` file may remain:

```powershell
# Check for temporary file
Test-Path "$env:APPDATA\FloatingTodo\tasks.json.tmp"
```

If `tasks.json.tmp` exists and contains valid JSON while `tasks.json` is missing or corrupted, restore it:
```powershell
Copy-Item "$env:APPDATA\FloatingTodo\tasks.json.tmp" "$env:APPDATA\FloatingTodo\tasks.json" -Force
```

---

## Resolving Global Hotkey Conflicts

Floating TODO registers `Ctrl + Shift + T` as a global hotkey via `RegisterHotKey`:

### Symptoms:
- Pressing `Ctrl + Shift + T` does not toggle the window.

### Root Cause:
- Another application running on the system (e.g. Google Chrome / Edge "Reopen Closed Tab", Microsoft PowerToys, or a development IDE) registered `Ctrl + Shift + T` prior to Floating TODO.
- The app handles this gracefully without crashing; `RegisterHotKey` returns a failure code and Floating TODO falls back to operation via the System Tray icon.

### Solutions:
1. Trigger window visibility by left-clicking the Floating TODO system tray icon.
2. If desired, customize the registered hotkey combination in `src/hotkey.rs`:
   ```rust
   RegisterHotKey(Some(hwnd), HOTKEY_TOGGLE_ID, MOD_CONTROL | MOD_SHIFT, VK_T.0 as u32);
   ```
   For example, changing `VK_T` to `VK_SPACE` or adding `MOD_ALT`.

---

## Troubleshooting Common Issues

| Issue | Cause | Solution |
|---|---|---|
| `Ctrl + Shift + T` does nothing | Hotkey registered by another program | Close conflicting apps or toggle visibility via the System Tray icon. |
| Window disappears completely | Minimized to system tray | Check the taskbar notification area (overflow tray); click the Floating TODO icon. |
| Tasks not saving | File permissions in `%APPDATA%` | Ensure `%APPDATA%\FloatingTodo` is writable by the current user profile. |
| Voice dictation fails to start | Microphone permission or service disabled | Ensure Windows Speech Recognition and microphone access are enabled in *Windows Settings > Privacy & security*. |
| High-DPI blurriness on secondary monitor | Process DPI scaling override active | Ensure no compatibility mode DPI scaling override is enabled on `floating-todo.exe`. The app handles Per-Monitor V2 scaling natively. |
| Docker build fails | Docker Desktop daemon not running | Launch Docker Desktop and verify with `docker info` in PowerShell. |
