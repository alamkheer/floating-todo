use std::future::{Future, IntoFuture};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
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

pub const WM_VOICE_RESULT: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 101;
pub const WM_VOICE_STOPPED: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 102;

/// Global atomic cancellation handle for an in-flight voice recording session.
static RECORDING_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Helper to synchronously evaluate a standard Rust future with timeout and cancellation checks.
fn await_future<F: Future>(mut future: F, timeout: Duration) -> Option<F::Output> {
    let thread = thread::current();

    fn raw_waker(thread: &thread::Thread) -> RawWaker {
        let ptr = thread as *const thread::Thread as *const ();
        RawWaker::new(ptr, &VTABLE)
    }

    static VTABLE: RawWakerVTable = RawWakerVTable::new(
        |ptr| RawWaker::new(ptr, &VTABLE),
        |ptr| unsafe { (*(ptr as *const thread::Thread)).unpark() },
        |ptr| unsafe { (*(ptr as *const thread::Thread)).unpark() },
        |_| {},
    );

    let waker = unsafe { Waker::from_raw(raw_waker(&thread)) };
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
                let tick = Duration::from_millis(500).min(timeout - elapsed);
                thread::park_timeout(tick);
            }
        }
    }
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

/// Spawns a background thread to record voice for up to 30 seconds using
/// WinRT SpeechRecognizer with a Dictation topic constraint.
///
/// On completion, posts WM_VOICE_RESULT with `*mut String` as WPARAM to the window.
pub fn start_voice_recognition(hwnd: HWND) {
    if RECORDING_ACTIVE.swap(true, Ordering::SeqCst) {
        // Already recording; return early
        return;
    }

    let hwnd_raw = hwnd.0 as usize;

    thread::spawn(move || {
        let result_text = run_recognition_session();

        RECORDING_ACTIVE.store(false, Ordering::SeqCst);

        let target_hwnd = HWND(hwnd_raw as *mut _);

        // Notify window on completion
        if let Some(text) = result_text {
            let text_box = Box::new(text);
            unsafe {
                let _ = PostMessageW(
                    Some(target_hwnd),
                    WM_VOICE_RESULT,
                    WPARAM(Box::into_raw(text_box) as usize),
                    LPARAM(0),
                );
            }
        } else {
            unsafe {
                let _ = PostMessageW(Some(target_hwnd), WM_VOICE_STOPPED, WPARAM(0), LPARAM(0));
            }
        }
    });
}

fn run_recognition_session() -> Option<String> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }

    let run_inner = || -> windows::core::Result<Option<String>> {
        // 1. Create SpeechRecognizer
        let recognizer = SpeechRecognizer::new()?;

        // 2. Define custom recognition constraints (Microsoft Learn guideline: Dictation topic constraint)
        let dictation_constraint = SpeechRecognitionTopicConstraint::Create(
            SpeechRecognitionScenario::Dictation,
            &HSTRING::from("dictation"),
        )?;

        let constraints = recognizer.Constraints()?;
        constraints.Append(&dictation_constraint)?;

        // 3. Compile constraints
        let compile_op = recognizer.CompileConstraintsAsync()?;
        let compile_res = match await_future(compile_op.into_future(), Duration::from_secs(10)) {
            Some(Ok(res)) => res,
            _ => return Ok(None),
        };

        if compile_res.Status()? != SpeechRecognitionResultStatus::Success {
            return Ok(None);
        }

        // 4. Configure 30-second timeouts
        if let Ok(timeouts) = recognizer.Timeouts() {
            let _ = timeouts.SetInitialSilenceTimeout(TimeSpan {
                Duration: 30 * 10_000_000,
            });
            let _ = timeouts.SetBabbleTimeout(TimeSpan {
                Duration: 30 * 10_000_000,
            });
            let _ = timeouts.SetEndSilenceTimeout(TimeSpan {
                Duration: 5 * 10_000_000,
            });
        }

        // 5. Start recognition (up to 30 seconds)
        let recognize_op = recognizer.RecognizeAsync()?;
        let recognize_res = match await_future(recognize_op.into_future(), Duration::from_secs(31)) {
            Some(Ok(res)) => res,
            _ => return Ok(None),
        };

        if recognize_res.Status()? == SpeechRecognitionResultStatus::Success {
            let text = recognize_res.Text()?.to_string_lossy();
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return Ok(Some(trimmed.to_string()));
            }
        }

        Ok(None)
    };

    let result = run_inner().unwrap_or(None);

    unsafe {
        CoUninitialize();
    }

    result
}
