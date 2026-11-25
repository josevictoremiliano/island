//! Your last copies (text). Kept in memory only, never written to disk, and anything a password manager marks
//! as private is skipped (the same formats Windows' own clipboard history respects).
use serde::Serialize;
use std::{sync::Arc, sync::Mutex, time::Duration};
use tauri::{AppHandle, Emitter};
use windows::{
    core::w,
    Win32::System::DataExchange::{GetClipboardSequenceNumber, IsClipboardFormatAvailable, RegisterClipboardFormatW},
};

use crate::core::Core;

const MAX_ITEMS: usize = 30;
const MAX_LEN: usize = 200_000;

#[derive(Serialize, Clone)]
pub struct Clip {
    pub id: u64,
    pub text: String,
    pub at: i64,
}

static ITEMS: Mutex<Vec<Clip>> = Mutex::new(Vec::new());
static NEXT: Mutex<u64> = Mutex::new(1);

pub fn list() -> Vec<Clip> {
    ITEMS.lock().unwrap().clone()
}

fn private_content() -> bool {
    unsafe {
        let exclude = RegisterClipboardFormatW(w!("ExcludeClipboardContentFromMonitorProcessing"));
        let history = RegisterClipboardFormatW(w!("CanIncludeInClipboardHistory"));
        (exclude != 0 && IsClipboardFormatAvailable(exclude).is_ok())
            || (history != 0 && IsClipboardFormatAvailable(history).is_ok())
    }
}

fn push(text: String) -> bool {
    if text.trim().is_empty() || text.len() > MAX_LEN {
        return false;
    }
    let mut items = ITEMS.lock().unwrap();
    if items.first().map_or(false, |c| c.text == text) {
        return false;
    }
    items.retain(|c| c.text != text);
    let mut next = NEXT.lock().unwrap();
    items.insert(0, Clip { id: *next, text, at: crate::core::now() });
    *next += 1;
    items.truncate(MAX_ITEMS);
    true
}

fn emit(app: &AppHandle) {
    let _ = app.emit("clips", list());
}

pub fn spawn(app: AppHandle, core: Arc<Core>) {
    std::thread::spawn(move || {
        let mut last = 0u32;
        loop {
            std::thread::sleep(Duration::from_millis(500));
            if !core.cfg().apps.clipboard {
                if !ITEMS.lock().unwrap().is_empty() {
                    ITEMS.lock().unwrap().clear();
                    emit(&app);
                }
                continue;
            }
            let seq = unsafe { GetClipboardSequenceNumber() };
            if seq == last {
                continue;
            }
            last = seq;
            if private_content() {
                continue;
            }
            // another app may still hold the clipboard for a moment
            let text = (0..3).find_map(|i| {
                if i > 0 {
                    std::thread::sleep(Duration::from_millis(80));
                }
                arboard::Clipboard::new().ok()?.get_text().ok()
            });
            if let Some(text) = text {
                if push(text) {
                    emit(&app);
                }
            }
        }
    });
}

pub fn copy(app: &AppHandle, id: u64) -> Result<(), String> {
    let text = ITEMS.lock().unwrap().iter().find(|c| c.id == id).map(|c| c.text.clone()).ok_or("Essa cópia não existe mais.")?;
    arboard::Clipboard::new()
        .and_then(|mut c| c.set_text(text.clone()))
        .map_err(|e| format!("Não consegui copiar: {e}"))?;
    // move it to the top right away (the watcher sees the same text and keeps it there)
    {
        let mut items = ITEMS.lock().unwrap();
        if let Some(pos) = items.iter().position(|c| c.id == id) {
            let mut clip = items.remove(pos);
            clip.at = crate::core::now();
            items.insert(0, clip);
        }
    }
    emit(app);
    Ok(())
}

pub fn remove(app: &AppHandle, id: Option<u64>) {
    match id {
        Some(id) => ITEMS.lock().unwrap().retain(|c| c.id != id),
        None => ITEMS.lock().unwrap().clear(),
    }
    emit(app);
}
