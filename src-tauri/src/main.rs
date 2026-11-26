#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod account_usage;
mod calendar;
mod claude;
mod clipboard;
mod codex;
mod codex_usage;
mod config;
mod focus;
mod core;
mod hover;
mod links;
mod lyrics;
mod media;
mod net;
mod server;
mod shelf;
mod system;
mod updater;
mod volume;
mod weather;

use std::{path::Path, sync::Arc};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, LogicalPosition, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};
use tauri_plugin_global_shortcut::ShortcutState;

use crate::{
    config::Config,
    core::{Core, Rect, Snapshot},
};

const ISLAND_W: f64 = 760.0; // room for the widest island (4 own apps) plus the ears
const ISLAND_H: f64 = 560.0;
// every webview of the app must get the same arguments (WebView2 shares one environment)
const BROWSER_ARGS: &str =
    "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --autoplay-policy=no-user-gesture-required";

pub fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

// ---------- windows ----------

fn position_island(win: &WebviewWindow) {
    if let Ok(Some(m)) = win.primary_monitor() {
        let scale = m.scale_factor();
        let (size, pos) = (m.size(), m.position());
        let x = pos.x as f64 / scale + (size.width as f64 / scale - ISLAND_W) / 2.0;
        let _ = win.set_position(LogicalPosition::new(x, pos.y as f64 / scale));
    }
}

fn create_island(app: &AppHandle) -> tauri::Result<()> {
    // Created visible and already in place: Tauri installs its file drop receiver while the webview is created,
    // and a window created hidden ended up without it (dropped files went to the webview and never reached the tray).
    let (x, y) = app
        .primary_monitor()
        .ok()
        .flatten()
        .map(|m| {
            let scale = m.scale_factor();
            let (size, pos) = (m.size(), m.position());
            (pos.x as f64 / scale + (size.width as f64 / scale - ISLAND_W) / 2.0, pos.y as f64 / scale)
        })
        .unwrap_or((0.0, 0.0));
    let win = WebviewWindowBuilder::new(app, "island", WebviewUrl::App("island.html".into()))
        .title("Ilha")
        .inner_size(ISLAND_W, ISLAND_H)
        .position(x, y)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(false)
        .focused(false)
        .visible(true)
        .additional_browser_args(BROWSER_ARGS)
        .build()?;
    let _ = win.set_ignore_cursor_events(true);
    // files dropped on the island go into the file tray
    let handle = app.clone();
    win.on_window_event(move |event| {
        if let tauri::WindowEvent::DragDrop(drop) = event {
            let enabled = handle.state::<Arc<Core>>().cfg().apps.shelf;
            match drop {
                tauri::DragDropEvent::Enter { .. } if enabled => {
                    let _ = handle.emit_to("island", "dragging", true);
                }
                tauri::DragDropEvent::Leave => {
                    let _ = handle.emit_to("island", "dragging", false);
                }
                tauri::DragDropEvent::Drop { paths, .. } => {
                    let _ = handle.emit_to("island", "dragging", false);
                    if enabled {
                        shelf::add(&handle, paths);
                    }
                }
                _ => {}
            }
        }
    });
    win.show()?;
    Ok(())
}

pub fn open_settings(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("settings") {
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();
        return;
    }
    let _ = WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("settings.html".into()))
        .title("Ilha · Configurações")
        .inner_size(1120.0, 780.0)
        .min_inner_size(880.0, 600.0)
        .theme(Some(tauri::Theme::Dark))
        .center()
        .additional_browser_args(BROWSER_ARGS)
        .build();
}

fn toggle_island(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("island") {
        if win.is_visible().unwrap_or(false) {
            let _ = win.hide();
        } else {
            position_island(&win);
            let _ = win.show();
        }
    }
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let settings = MenuItem::with_id(app, "settings", "Configurações", true, None::<&str>)?;
    let toggle = MenuItem::with_id(app, "toggle", "Mostrar / ocultar a ilha", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Sair da Ilha", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&settings, &toggle, &separator, &quit])?;
    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip("Ilha")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "settings" => open_settings(app),
            "toggle" => toggle_island(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                open_settings(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

// ---------- commands ----------

type Shared<'a> = State<'a, Arc<Core>>;

#[tauri::command]
fn get_state(core: Shared) -> Snapshot {
    core.snapshot()
}

#[tauri::command]
fn save_config(core: Shared, cfg: Config) -> Result<Snapshot, String> {
    core.save_config(cfg)?;
    Ok(core.snapshot())
}

#[tauri::command]
fn set_claude(core: Shared, on: bool) -> Result<Snapshot, String> {
    core.set_claude(on)?;
    Ok(core.snapshot())
}

#[tauri::command]
fn decide(core: Shared, session_id: String, behavior: String) -> Result<(), String> {
    core.decide(&session_id, &behavior)
}

#[tauri::command]
fn dismiss(core: Shared, session_id: String) {
    core.dismiss(&session_id)
}

#[tauri::command]
fn clear_history(core: Shared) {
    core.clear_history()
}

#[tauri::command]
fn set_hit_rects(core: Shared, rects: Vec<Rect>) {
    *core.hit.lock().unwrap() = rects;
}

/// A notice from the island itself (timer done, back to focus) or a test from Settings.
#[tauri::command]
fn notice(core: Shared, id: String, title: String, body: String) {
    core.notice(&id, &title, &body)
}

#[tauri::command]
fn get_note(core: Shared) -> String {
    std::fs::read_to_string(core.note_path()).unwrap_or_default()
}

#[tauri::command]
fn set_note(core: Shared, text: String) {
    let _ = write_atomic(&core.note_path(), &text);
}

#[tauri::command]
fn notify(core: Shared, title: String, body: String) {
    core.notify(&title, &body)
}

#[tauri::command]
fn test_notify(app: AppHandle, core: Shared) {
    core.notify("Ilha: teste", "É assim que a ilha avisa quando o Claude Code termina.");
    let _ = app.emit_to("island", "chime", "done");
    let _ = app.emit_to("island", "test", ());
}

// must be async: creating a window from a sync command deadlocks WebView2 on Windows (blank white window)
#[tauri::command]
async fn open_settings_cmd(app: AppHandle) {
    open_settings(&app)
}

#[tauri::command]
async fn media_control(action: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || media::control(&action))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
fn get_media() -> Option<media::Media> {
    media::current()
}

#[tauri::command]
fn get_sys() -> Option<system::Sys> {
    system::current()
}

#[tauri::command]
fn set_codex(core: Shared, on: bool) -> Result<Snapshot, String> {
    core.set_codex(on)?;
    Ok(core.snapshot())
}

// ---- clipboard ----
#[tauri::command]
fn get_clips() -> Vec<clipboard::Clip> {
    clipboard::list()
}

#[tauri::command]
fn clip_copy(app: AppHandle, id: u64) -> Result<(), String> {
    clipboard::copy(&app, id)
}

#[tauri::command]
fn clip_remove(app: AppHandle, id: Option<u64>) {
    clipboard::remove(&app, id)
}

// ---- file tray ----
#[tauri::command]
fn get_shelf() -> Vec<shelf::ShelfItem> {
    shelf::list()
}

#[tauri::command]
fn shelf_remove(app: AppHandle, id: Option<String>) {
    shelf::remove(&app, id.as_deref())
}

#[tauri::command]
fn shelf_open(id: String) -> Result<(), String> {
    shelf::open(&id)
}

#[tauri::command]
fn shelf_reveal(id: String) -> Result<(), String> {
    shelf::reveal(&id)
}

/// Drag files out of the island into another app. Sync on purpose: Windows' drag and drop must run on the main thread.
#[tauri::command]
fn shelf_drag(window: tauri::Window, ids: Vec<String>) -> Result<(), String> {
    let paths = shelf::paths(&ids);
    if paths.is_empty() {
        return Err("Esse arquivo não existe mais nesse lugar.".into());
    }
    let icon = include_bytes!("../icons/64x64.png").to_vec();
    drag::start_drag(&window, drag::DragItem::Files(paths), drag::Image::Raw(icon), |_, _| {}, drag::Options::default())
        .map_err(|e| e.to_string())
}

// ---- Wi‑Fi and Bluetooth ----
#[tauri::command]
fn get_net() -> Option<net::Net> {
    net::current()
}

#[tauri::command]
async fn net_set(kind: String, on: bool) -> Result<(), String> {
    let result = tauri::async_runtime::spawn_blocking(move || {
        let r = net::set(&kind, on);
        if r.is_err() {
            net::open_settings(&kind);
        }
        r
    })
    .await
    .map_err(|e| e.to_string())?;
    result
}

#[tauri::command]
fn net_settings(which: String) {
    net::open_settings(&which)
}

// ---- your own sites and programs ----
#[tauri::command]
fn open_link(target: String) -> Result<(), String> {
    links::open(&target)
}

#[tauri::command]
async fn link_icon(target: String) -> Option<String> {
    links::icon(&target).await
}

#[tauri::command]
async fn pick_program(app: AppHandle) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Escolha um programa")
            .add_filter("Programas e atalhos", &["exe", "lnk", "bat", "cmd", "url"])
            .blocking_pick_file()
            .and_then(|p| p.into_path().ok())
            .map(|p| p.to_string_lossy().to_string())
    })
    .await
    .ok()
    .flatten()
}

// ---- focus guard ----
#[tauri::command]
fn focus_timer(running: bool) {
    focus::TIMER_RUNNING.store(running, std::sync::atomic::Ordering::Relaxed);
}

#[tauri::command]
fn focus_snooze(minutes: i64) {
    focus::snooze(minutes)
}

#[tauri::command]
fn focus_leave() {
    focus::leave()
}

// ---- lyrics and calendar ----
#[tauri::command]
fn get_lyrics() -> Option<lyrics::Lyrics> {
    lyrics::current()
}

#[tauri::command]
fn get_calendar() -> Option<calendar::Calendar> {
    calendar::current()
}

// ---- updates ----
#[tauri::command]
fn update_get() -> updater::UpdateState {
    updater::current()
}

#[tauri::command]
fn update_check(app: AppHandle, core: Shared) {
    let core = core.inner().clone();
    tauri::async_runtime::spawn(updater::check(app, core, true));
}

#[tauri::command]
async fn update_install(app: AppHandle, core: Shared<'_>) -> Result<(), String> {
    let core = core.inner().clone();
    tauri::async_runtime::spawn_blocking(move || updater::install(&app, &core)).await.map_err(|e| e.to_string())
}

#[tauri::command]
fn get_weather() -> Option<weather::Weather> {
    weather::current()
}

#[tauri::command]
fn weather_refresh() {
    weather::NOW.store(true, std::sync::atomic::Ordering::SeqCst);
}

#[tauri::command]
fn calendar_refresh() {
    calendar::NOW.store(true, std::sync::atomic::Ordering::SeqCst);
}

#[tauri::command]
fn quit(app: AppHandle) {
    app.exit(0)
}

fn main() {
    if std::env::args().any(|a| a == "--probe") {
        println!("MÚSICA:\n{}\n\nSISTEMA:\n{}\n\nREDE:\n{}\n\nCODEX: encontrado={} conectado={}\n\nUSO DA CONTA:\n{}\n\nUSO DO CODEX:\n{}", media::probe(), system::probe(), net::probe(), codex::found(), codex::is_installed(), account_usage::probe(), codex_usage::probe());
        return;
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| open_settings(app)))
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        let _ = app.emit_to("island", "toggle", ());
                    }
                })
                .build(),
        )
        .setup(|app| {
            let handle = app.handle().clone();
            let data = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data)?;
            shelf::init(&data);
            let (core, first_run) = Core::new(handle.clone(), data);
            let core = Arc::new(core);
            app.manage(core.clone());
            create_island(&handle)?;
            build_tray(&handle)?;
            core.apply_shortcut();
            server::spawn(core.clone());
            hover::spawn(handle.clone(), core.clone());
            media::spawn(handle.clone());
            system::spawn(handle.clone(), core.clone());
            clipboard::spawn(handle.clone(), core.clone());
            net::spawn(handle.clone());
            account_usage::spawn(core.clone());
            codex_usage::spawn(core.clone());
            focus::spawn(handle.clone(), core.clone());
            lyrics::spawn(handle.clone(), core.clone());
            calendar::spawn(handle.clone(), core.clone());
            volume::spawn(handle.clone(), core.clone());
            weather::spawn(handle.clone(), core.clone());
            updater::spawn(handle.clone(), core.clone());
            if first_run {
                open_settings(&handle);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            save_config,
            set_claude,
            decide,
            dismiss,
            clear_history,
            set_hit_rects,
            notice,
            get_note,
            set_note,
            notify,
            test_notify,
            open_settings_cmd,
            media_control,
            get_media,
            get_sys,
            set_codex,
            get_clips,
            clip_copy,
            clip_remove,
            get_shelf,
            shelf_remove,
            shelf_open,
            shelf_reveal,
            shelf_drag,
            get_net,
            net_set,
            net_settings,
            open_link,
            link_icon,
            pick_program,
            focus_timer,
            focus_snooze,
            focus_leave,
            get_lyrics,
            get_calendar,
            calendar_refresh,
            get_weather,
            update_get,
            update_check,
            update_install,
            weather_refresh,
            quit
        ])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar a Ilha");
}
