//! The island window is bigger than the visible pill and clicks must pass through the empty part.
//! This loop watches the mouse: over the island (or a notice card) the window takes clicks and drops,
//! elsewhere it lets them through.
//!
//! It talks to Windows directly (cursor, window rect, the click-through style) instead of asking Tauri's main thread:
//! during a drag-and-drop the main thread is busy with the drag, the loop waited for it, and the island never became
//! a drop target, so files dropped on it fell through to whatever was behind.
use std::{sync::Arc, time::Duration};
use tauri::{AppHandle, Emitter, Manager};
use windows::Win32::{
    Foundation::{HWND, POINT, RECT},
    UI::{
        HiDpi::GetDpiForWindow,
        WindowsAndMessaging::{
            GetCursorPos, GetWindowLongPtrW, GetWindowRect, IsWindowVisible, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_LAYERED,
            WS_EX_TRANSPARENT,
        },
    },
};

use crate::core::Core;

/// Let clicks pass through the window (or not), like Tauri's set_ignore_cursor_events, but callable from any thread.
fn click_through(hwnd: HWND, on: bool) {
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let transparent = WS_EX_TRANSPARENT.0 as isize;
        let layered = WS_EX_LAYERED.0 as isize;
        let new = if on { style | transparent | layered } else { (style & !transparent) | layered };
        if new != style {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new);
        }
    }
}

pub fn spawn(app: AppHandle, core: Arc<Core>) {
    std::thread::spawn(move || {
        // the island window's handle, once it exists
        let hwnd = loop {
            if let Some(h) = app.get_webview_window("island").and_then(|w| w.hwnd().ok()) {
                break HWND(h.0 as _);
            }
            std::thread::sleep(Duration::from_millis(100));
        };
        let mut last: Option<bool> = None;
        let mut last_island: Option<bool> = None;
        loop {
            std::thread::sleep(Duration::from_millis(40));
            let (mut cursor, mut rect) = (POINT::default(), RECT::default());
            unsafe {
                if !IsWindowVisible(hwnd).as_bool() || GetCursorPos(&mut cursor).is_err() || GetWindowRect(hwnd, &mut rect).is_err() {
                    continue;
                }
            }
            let scale = unsafe { GetDpiForWindow(hwnd) }.max(96) as f64 / 96.0;
            let x = (cursor.x - rect.left) as f64 / scale;
            let y = (cursor.y - rect.top) as f64 / scale;
            let rects = core.hit.lock().unwrap().clone();
            let over = |r: &crate::core::Rect| r.w > 0.0 && x >= r.x && x <= r.x + r.w && y >= r.y - 4.0 && y <= r.y + r.h;
            // the first rect is the island (resting on it opens it); the others are notice cards (only clickable)
            let on_island = rects.first().map_or(false, over);
            let inside = rects.iter().any(over);
            if last != Some(inside) {
                last = Some(inside);
                click_through(hwnd, !inside);
            }
            if last_island != Some(on_island) {
                last_island = Some(on_island);
                let _ = app.emit_to("island", "hover", on_island);
            }
        }
    });
}
