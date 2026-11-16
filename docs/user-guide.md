# User Guide & Controls

This guide provides a comprehensive overview of the features, keyboard shortcuts, voice dictation, and user controls available in **Floating TODO**.

---

## Table of Contents

- [Features](#features)
- [Keyboard Shortcuts & Controls](#keyboard-shortcuts--controls)
- [Voice Dictation Input](#voice-dictation-input)
- [Hierarchical Sub-Tasks & Cascading Logic](#hierarchical-sub-tasks--cascading-logic)
- [System Tray Integration](#system-tray-integration)
- [Always-on-Top & Window Management](#always-on-top--window-management)

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

## Voice Dictation Input

Floating TODO features built-in speech recognition utilizing the Windows Runtime (`Windows.Media.SpeechRecognition` API) through `windows-rs`:

1. Click the microphone button (`🎙️`) in the header bar.
2. A 30-second countdown timer will display on the microphone button.
3. Speak your task clearly into your default Windows microphone.
4. When finished, either click the microphone button again to finalize immediately, or wait for the 30-second timer to expire.
5. The transcribed text will be automatically inserted as a new task.

> [!NOTE]
> Voice dictation requires Windows 10 (Build 1809+) or Windows 11 with Windows Speech Recognition services enabled in Windows Settings (*Settings > Privacy & security > Speech*).

---

## Hierarchical Sub-Tasks & Cascading Logic

Floating TODO supports single-level nested sub-tasks for decomposing complex items:

- **Adding a Sub-Task:**
  1. Click to select an active parent task in the list.
  2. Click the `＋ Sub` button.
  3. The input box will switch to sub-task mode (prefixed with `[Sub-Task]`).
  4. Type your sub-task text and press `Enter`.
  5. Press `Esc` or enter an empty string to exit sub-task input mode.
- **Tree Visualization:** Sub-tasks are rendered indented with clear `└─` tree glyphs.
- **Cascading Completion:**
  - Completing all sub-tasks of a parent automatically marks the parent task as completed.
  - Unchecking any sub-task automatically restores the parent task to active status.

---

## System Tray Integration

Floating TODO operates as a persistent desktop companion:

- **Minimizing to Tray:** Clicking the `✕` close button in the header hides the window to the Windows System Tray rather than quitting the process.
- **Tray Icon Tooltip:** Hovering over the tray icon reveals the current application version (e.g. `Floating TODO v0.2.3`).
- **Tray Menu Actions:** Right-clicking the tray icon reveals a context menu:
  - **Version Header:** Non-clickable label showing current active version.
  - **Show / Hide:** Instantly toggles window visibility.
  - **Always on Top:** Toggles window pinning state.
  - **Exit:** Shuts down the process and unregisters global hooks.

---

## Always-on-Top & Window Management

- **Pinning (`📌`):** Toggle the pin button in the header bar or in the tray context menu to keep Floating TODO floating over full-screen browser sessions, IDEs, and documents.
- **Borderless Dragging:** The top title header acts as an intuitive drag handle; click and drag anywhere in the upper area to relocate the window across multi-monitor setups.
- **Filter Bar:** Switch views between **All** tasks, **Active** only, or **Done** only. The active filter is highlighted with an owner-drawn accent color.
