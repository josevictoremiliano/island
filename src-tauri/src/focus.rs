//! Focus guard: when the window in front matches a word on your list (YouTube, Instagram…) the island stops you
//! for a moment. By default only while the focus timer runs. Looks only at the window title and program name, locally.
use serde::Serialize;
use std::{
    sync::{
        atomic::{AtomicBool, AtomicI64, AtomicIsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tauri::{AppHandle, Emitter};
use windows::{
    core::PWSTR,
    Win32::{
        Foundation::{CloseHandle, HWND},
        System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION},
        UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId, ShowWindow, SW_MINIMIZE},
    },
};

use crate::core::Core;

pub static TIMER_RUNNING: AtomicBool = AtomicBool::new(false);
static SNOOZE_UNTIL: AtomicI64 = AtomicI64::new(0);
static LAST: AtomicIsize = AtomicIsize::new(0);

#[derive(Serialize, Clone)]
pub struct Distraction {
    pub word: String,
    pub title: String,
}

fn window_info(hwnd: HWND) -> Option<(String, String, u32)> {
    unsafe {
        let mut buf = [0u16; 512];
        let n = GetWindowTextW(hwnd, &mut buf) as usize;
        let title = String::from_utf16_lossy(&buf[..n]);
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        let mut exe = String::new();
        if let Ok(process) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            let mut path = [0u16; 1024];
            let mut len = path.len() as u32;
            if QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, PWSTR(path.as_mut_ptr()), &mut len).is_ok() {
                let full = String::from_utf16_lossy(&path[..len as usize]);
                exe = full.rsplit('\\').next().unwrap_or("").to_string();
            }
            let _ = CloseHandle(process);
        }
        Some((title, exe, pid))
    }
}

pub fn spawn(app: AppHandle, core: Arc<Core>) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(1500));
        let cfg = core.cfg();
        let active = cfg.apps.focus
            && (!cfg.focus.only_timer || TIMER_RUNNING.load(Ordering::Relaxed))
            && crate::core::now() > SNOOZE_UNTIL.load(Ordering::Relaxed);
        if !active {
            LAST.store(0, Ordering::Relaxed);
            continue;
        }
        let hwnd = unsafe { GetForegroundWindow() };
        if hwnd.is_invalid() {
            continue;
        }
        let Some((title, exe, pid)) = window_info(hwnd) else { continue };
        if pid == std::process::id() {
            continue; // the island itself
        }
        let haystack = format!("{} {}", title, exe).to_lowercase();
        let hit = cfg.focus.words.iter().map(|w| w.trim()).filter(|w| !w.is_empty()).find(|w| haystack.contains(&w.to_lowercase()));
        match hit {
            Some(word) => {
                // warn once per window; switching away and back warns again
                if LAST.swap(hwnd.0 as isize, Ordering::Relaxed) != hwnd.0 as isize {
                    let _ = app.emit_to("island", "distraction", Distraction { word: word.to_string(), title });
                }
            }
            None => LAST.store(0, Ordering::Relaxed),
        }
    });
}

/// Leave it alone for a few minutes.
pub fn snooze(minutes: i64) {
    SNOOZE_UNTIL.store(crate::core::now() + minutes * 60_000, Ordering::Relaxed);
}

/// "Back to focus": minimize the distracting window.
pub fn leave() {
    let hwnd = LAST.swap(0, Ordering::Relaxed);
    if hwnd != 0 {
        unsafe {
            let _ = ShowWindow(HWND(hwnd as *mut _), SW_MINIMIZE);
        }
    }
}
