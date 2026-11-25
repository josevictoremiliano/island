//! The island window is bigger than the visible pill and clicks must pass through the empty part.
//! This loop watches the mouse: over the pill the window takes clicks, elsewhere it lets them through.
use std::{sync::Arc, time::Duration};
use tauri::{AppHandle, Emitter, Manager};

use crate::core::Core;

pub fn spawn(app: AppHandle, core: Arc<Core>) {
    std::thread::spawn(move || {
        let mut last: Option<bool> = None;
        let mut last_island: Option<bool> = None;
        loop {
            std::thread::sleep(Duration::from_millis(40));
            let Some(win) = app.get_webview_window("island") else { continue };
            if !win.is_visible().unwrap_or(false) {
                continue;
            }
            let (Ok(cursor), Ok(pos), Ok(scale)) = (win.cursor_position(), win.outer_position(), win.scale_factor()) else {
                continue;
            };
            let x = (cursor.x - pos.x as f64) / scale;
            let y = (cursor.y - pos.y as f64) / scale;
            // the island and the notice cards under it; a little slack on top so the very edge of the screen counts too
            let rects = core.hit.lock().unwrap().clone();
            let over = |r: &crate::core::Rect| r.w > 0.0 && x >= r.x && x <= r.x + r.w && y >= r.y - 4.0 && y <= r.y + r.h;
            // the first rect is the island (resting on it opens it); the others are notice cards (only clickable)
            let on_island = rects.first().map_or(false, over);
            let inside = rects.iter().any(over);
            if last != Some(inside) {
                last = Some(inside);
                let _ = win.set_ignore_cursor_events(!inside);
            }
            if last_island != Some(on_island) {
                last_island = Some(on_island);
                let _ = app.emit_to("island", "hover", on_island);
            }
        }
    });
}
