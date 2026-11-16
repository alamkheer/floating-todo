use std::future::{Future, IntoFuture};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::thread;
use std::time::{Duration, Instant};

use windows::core::HSTRING;
use windows::Foundation::TimeSpan;
use windows::Media::SpeechRecognition::{
    SpeechRecognitionResultStatus, SpeechRecognitionScenario, SpeechRecognitionTopicConstraint,
    SpeechRecognizer,
};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};
use windows::Win32::UI::WindowsAndMessaging::PostMessageW;

use crate::state::{log_event, LogLevel};

pub const WM_VOICE_RESULT: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 101;
pub const WM_VOICE_STOPPED: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 102;
pub const WM_VOICE_PERMISSION_REQUIRED: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 103;

/// Global atomic cancellation handle for an in-flight voice recording session.
static RECORDING_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Standard, memory-safe thread waker for evaluating async WinRT operations on background threads.
struct ThreadWaker(thread::Thread);

impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

/// Helper to synchronously evaluate a standard Rust future with timeout and cancellation checks.
fn await_future<F: Future>(mut future: F, timeout: Duration) -> Option<F::Output> {
    let waker = Waker::from(Arc::new(ThreadWaker(thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut pin = unsafe { Pin::new_unchecked(&mut future) };

    let start = Instant::now();
    loop {
        match pin.as_mut().poll(&mut cx) {
            Poll::Ready(val) => return Some(val),
            Poll::Pending => {
                if !RECORDING_ACTIVE.load(Ordering::SeqCst) {
                    return None;
                }
                let elapsed = start.elapsed();
                if elapsed >= timeout {
                    return None;
                }
                let tick = Duration::from_millis(250).min(timeout - elapsed);
                thread::park_timeout(tick);
            }
        }
    }
}

/// Checks if an error is due to missing Windows speech privacy policy or microphone permission.
fn is_permission_error(err: &windows::core::Error) -> bool {
    let code = err.code().0 as u32;
    // 0x80045509: SPP_E_POLICY_NOT_ACCEPTED (speech privacy policy not accepted)
    // 0x80070005: E_ACCESSDENIED (microphone permission denied)
    if code == 0x80045509 || code == 0x80070005 {
        return true;
    }
    let msg = err.message().to_lowercase();
    msg.contains("privacy policy") || msg.contains("permission") || msg.contains("access")
}

/// Outcome of a voice recognition attempt.
enum VoiceOutcome {
    Success(String),
    PermissionRequired,
    Stopped,
}

/// Checks if a voice recording session is currently running.
#[allow(dead_code)]
pub fn is_recording() -> bool {
    RECORDING_ACTIVE.load(Ordering::SeqCst)
}

/// Requests an active voice recording session to cancel or stop early.
pub fn cancel_recording() {
    RECORDING_ACTIVE.store(false, Ordering::SeqCst);
}

/// Prompt the user with a dialog to open Windows Settings and enable Speech & Microphone permissions.
pub fn prompt_speech_permission(hwnd: HWND) {
    unsafe {
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::{
            MessageBoxW, IDYES, MB_ICONINFORMATION, MB_YESNO, SW_SHOWNORMAL,
        };

        let title = windows::core::w!("Microphone & Speech Permission Required");
        let msg = windows::core::w!(
            "Voice dictation requires Windows Online Speech Recognition and Microphone access.\n\n\
            To enable speech dictation:\n\
            1. Turn on \"Online speech recognition\" in Settings.\n\
            2. Ensure Microphone access is allowed for apps.\n\n\
            Would you like to open Windows Settings now?"
        );

        let res = MessageBoxW(Some(hwnd), msg, title, MB_YESNO | MB_ICONINFORMATION);
        if res == IDYES {
            let _ = ShellExecuteW(
                Some(hwnd),
                windows::core::w!("open"),
                windows::core::w!("ms-settings:privacy-speech"),
                None,
                None,
                SW_SHOWNORMAL,
            );
        }
    }
}

/// Spawns a background thread to record voice for the configured duration using
/// WinRT SpeechRecognizer with a Dictation topic constraint.
pub fn start_voice_recognition(hwnd: HWND, duration_secs: u32) {
    if RECORDING_ACTIVE.swap(true, Ordering::SeqCst) {
        log_event(LogLevel::Warn, "Voice recognition session is already active.");
        return;
    }

    let hwnd_raw = hwnd.0 as usize;
    log_event(
        LogLevel::Info,
        &format!("Starting voice recognition (timeout: {}s)...", duration_secs),
    );

    thread::spawn(move || {
        let outcome = run_recognition_session(duration_secs);

        RECORDING_ACTIVE.store(false, Ordering::SeqCst);

        let target_hwnd = HWND(hwnd_raw as *mut _);

        match outcome {
            VoiceOutcome::Success(text) => {
                log_event(
                    LogLevel::Info,
                    &format!("Speech recognized successfully: \"{}\"", text),
                );
                let text_box = Box::new(text);
                unsafe {
                    let _ = PostMessageW(
                        Some(target_hwnd),
                        WM_VOICE_RESULT,
                        WPARAM(Box::into_raw(text_box) as usize),
                        LPARAM(0),
                    );
                }
            }
            VoiceOutcome::PermissionRequired => {
                log_event(
                    LogLevel::Error,
                    "Microphone or Windows Speech Recognition permission required.",
                );
                unsafe {
                    let _ = PostMessageW(
                        Some(target_hwnd),
                        WM_VOICE_PERMISSION_REQUIRED,
                        WPARAM(0),
                        LPARAM(0),
                    );
                }
            }
            VoiceOutcome::Stopped => {
                unsafe {
                    let _ = PostMessageW(Some(target_hwnd), WM_VOICE_STOPPED, WPARAM(0), LPARAM(0));
                }
            }
        }
    });
}

fn run_recognition_session(duration_secs: u32) -> VoiceOutcome {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }

    let outcome = (|| -> windows::core::Result<VoiceOutcome> {
        // 1. Create SpeechRecognizer
        let recognizer = match SpeechRecognizer::new() {
            Ok(r) => r,
            Err(e) => {
                log_event(
                    LogLevel::Error,
                    &format!("Failed to create SpeechRecognizer: {}", e),
                );
                if is_permission_error(&e) {
                    return Ok(VoiceOutcome::PermissionRequired);
                }
                return Err(e);
            }
        };

        // 2. Define custom recognition constraints (Dictation topic constraint)
        let dictation_constraint = match SpeechRecognitionTopicConstraint::Create(
            SpeechRecognitionScenario::Dictation,
            &HSTRING::from("dictation"),
        ) {
            Ok(c) => c,
            Err(e) => {
                log_event(
                    LogLevel::Warn,
                    &format!("Speech dictation topic constraint failed: {}", e),
                );
                if is_permission_error(&e) {
                    return Ok(VoiceOutcome::PermissionRequired);
                }
                return Err(e);
            }
        };

        if let Ok(constraints) = recognizer.Constraints() {
            let _ = constraints.Append(&dictation_constraint);
        }

        // 3. Compile constraints
        let compile_op = match recognizer.CompileConstraintsAsync() {
            Ok(op) => op,
            Err(e) => {
                log_event(
                    LogLevel::Error,
                    &format!("CompileConstraintsAsync failed: {}", e),
                );
                if is_permission_error(&e) {
                    return Ok(VoiceOutcome::PermissionRequired);
                }
                return Err(e);
            }
        };

        let compile_res = match await_future(compile_op.into_future(), Duration::from_secs(10)) {
            Some(Ok(res)) => res,
            Some(Err(e)) => {
                log_event(
                    LogLevel::Error,
                    &format!("CompileConstraintsAsync returned error: {}", e),
                );
                if is_permission_error(&e) {
                    return Ok(VoiceOutcome::PermissionRequired);
                }
                return Ok(VoiceOutcome::Stopped);
            }
            None => {
                log_event(LogLevel::Warn, "Constraint compilation timed out.");
                return Ok(VoiceOutcome::Stopped);
            }
        };

        let compile_status = compile_res.Status()?;
        if compile_status != SpeechRecognitionResultStatus::Success {
            log_event(
                LogLevel::Error,
                &format!("Constraint compilation failed with status: {:?}", compile_status),
            );
            return Ok(VoiceOutcome::Stopped);
        }

        // 4. Configure timeouts according to user-selected duration
        let duration_100ns = (duration_secs as i64) * 10_000_000;
        if let Ok(timeouts) = recognizer.Timeouts() {
            let _ = timeouts.SetInitialSilenceTimeout(TimeSpan {
                Duration: duration_100ns,
            });
            let _ = timeouts.SetBabbleTimeout(TimeSpan {
                Duration: duration_100ns,
            });
            let _ = timeouts.SetEndSilenceTimeout(TimeSpan {
                Duration: 5 * 10_000_000,
            });
        }

        // 5. Start recognition
        let recognize_op = match recognizer.RecognizeAsync() {
            Ok(op) => op,
            Err(e) => {
                log_event(
                    LogLevel::Error,
                    &format!("RecognizeAsync initialization failed: {}", e),
                );
                if is_permission_error(&e) {
                    return Ok(VoiceOutcome::PermissionRequired);
                }
                return Err(e);
            }
        };

        let recognize_res = match await_future(
            recognize_op.into_future(),
            Duration::from_secs((duration_secs + 1) as u64),
        ) {
            Some(Ok(res)) => res,
            Some(Err(e)) => {
                log_event(
                    LogLevel::Error,
                    &format!("Speech recognition evaluation error: {}", e),
                );
                if is_permission_error(&e) {
                    return Ok(VoiceOutcome::PermissionRequired);
                }
                return Ok(VoiceOutcome::Stopped);
            }
            None => {
                log_event(
                    LogLevel::Warn,
                    &format!("Speech recognition session timed out after {}s with no audio captured.", duration_secs),
                );
                return Ok(VoiceOutcome::Stopped);
            }
        };

        let status = recognize_res.Status()?;
        if status == SpeechRecognitionResultStatus::Success {
            let text = recognize_res.Text()?.to_string_lossy();
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return Ok(VoiceOutcome::Success(trimmed.to_string()));
            } else {
                log_event(
                    LogLevel::Warn,
                    "Speech recognizer reported Success, but transcribed 0 characters (inaudible/silence).",
                );
            }
        } else {
            log_event(
                LogLevel::Warn,
                &format!("Speech recognition finished with non-success status: {:?}. Ensure microphone is unmuted and set as default recording device.", status),
            );
        }

        Ok(VoiceOutcome::Stopped)
    })()
    .unwrap_or_else(|e| {
        log_event(LogLevel::Error, &format!("Voice recognition exception: {}", e));
        if is_permission_error(&e) {
            VoiceOutcome::PermissionRequired
        } else {
            VoiceOutcome::Stopped
        }
    });

    unsafe {
        CoUninitialize();
    }

    outcome
}
