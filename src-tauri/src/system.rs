//! CPU, memory and battery for the island, and hiding the island while a fullscreen app (game, video) is in front.
use serde::Serialize;
use std::{sync::Arc, sync::Mutex, time::Duration};
use tauri::{AppHandle, Emitter, Manager};
use windows::Win32::{
    Foundation::{FILETIME, RECT},
    Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST},
    System::{
        Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS},
        SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX},
        Threading::GetSystemTimes,
    },
    UI::{
        Shell::SHQueryUserNotificationState,
        WindowsAndMessaging::{GetClassNameW, GetForegroundWindow, GetWindowRect, IsZoomed},
    },
};

use crate::core::Core;

#[derive(Serialize, Clone, Default)]
pub struct Sys {
    pub cpu: u32,
    pub ram: u32,
    pub battery: Option<u32>,
    pub charging: bool,
}

static CURRENT: Mutex<Option<Sys>> = Mutex::new(None);

pub fn current() -> Option<Sys> {
    CURRENT.lock().unwrap().clone()
}

fn ticks(f: &FILETIME) -> u64 {
    ((f.dwHighDateTime as u64) << 32) | f.dwLowDateTime as u64
}

fn cpu_times() -> Option<(u64, u64)> {
    let (mut idle, mut kernel, mut user) = (FILETIME::default(), FILETIME::default(), FILETIME::default());
    unsafe { GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)).ok()? };
    // kernel time includes idle time
    Some((ticks(&idle), ticks(&kernel) + ticks(&user)))
}

fn sample(prev: &mut Option<(u64, u64)>) -> Sys {
    let mut sys = Sys::default();
    if let Some((idle, total)) = cpu_times() {
        if let Some((pi, pt)) = *prev {
            let (di, dt) = (idle.saturating_sub(pi), total.saturating_sub(pt));
            if dt > 0 {
                sys.cpu = (100.0 * (1.0 - di as f64 / dt as f64)).round().clamp(0.0, 100.0) as u32;
            }
        }
        *prev = Some((idle, total));
    }
    let mut mem = MEMORYSTATUSEX { dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32, ..Default::default() };
    if unsafe { GlobalMemoryStatusEx(&mut mem) }.is_ok() {
        sys.ram = mem.dwMemoryLoad;
    }
    let mut power = SYSTEM_POWER_STATUS::default();
    if unsafe { GetSystemPowerStatus(&mut power) }.is_ok() {
        let no_battery = power.BatteryFlag & 128 != 0 || power.BatteryLifePercent == 255;
        sys.battery = (!no_battery).then_some(power.BatteryLifePercent as u32);
        sys.charging = power.ACLineStatus == 1;
    }
    sys
}

/// What Windows itself says (the same signal it uses to hold back notifications):
/// a fullscreen app, a game in exclusive fullscreen or presentation mode.
fn windows_says_busy() -> Option<i32> {
    unsafe { SHQueryUserNotificationState().ok().map(|s| s.0) }
}

/// A borderless window (not just maximized) covering the whole primary screen, e.g. a video in fullscreen.
fn borderless_fullscreen() -> bool {
    unsafe {
        let fg = GetForegroundWindow();
        if fg.is_invalid() || IsZoomed(fg).as_bool() {
            return false;
        }
        let mut class = [0u16; 64];
        let n = GetClassNameW(fg, &mut class) as usize;
        let class = String::from_utf16_lossy(&class[..n]);
        if matches!(class.as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd" | "Windows.UI.Core.CoreWindow") {
            return false;
        }
        let mut r = RECT::default();
        if GetWindowRect(fg, &mut r).is_err() {
            return false;
        }
        let monitor = MonitorFromWindow(fg, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        if !GetMonitorInfoW(monitor, &mut info).as_bool() {
            return false;
        }
        let m = info.rcMonitor;
        let primary = m.left == 0 && m.top == 0; // the island lives on the primary screen
        primary && r.left <= m.left && r.top <= m.top && r.right >= m.right && r.bottom >= m.bottom
    }
}

fn fullscreen_in_front() -> bool {
    // 2 = busy (fullscreen app), 3 = Direct3D fullscreen game, 4 = presentation mode
    matches!(windows_says_busy(), Some(2..=4)) || borderless_fullscreen()
}
/// For `ilha --probe`.
pub fn probe() -> String {
    let mut prev = None;
    sample(&mut prev);
    std::thread::sleep(Duration::from_millis(700));
    let sys = sample(&mut prev);
    format!("{}\nestado do Windows: {:?} · janela sem borda em tela cheia: {} · esconder: {}", serde_json::to_string(&sys).unwrap_or_default(), windows_says_busy(), borderless_fullscreen(), fullscreen_in_front())
}

pub fn spawn(app: AppHandle, core: Arc<Core>) {
    std::thread::spawn(move || {
        let mut prev = None;
        let mut tick = 0u32;
        let mut hidden_by_us = false;
        let mut battery_warned: Option<u32> = None;
        loop {
            // fullscreen check every second
            if let Some(win) = app.get_webview_window("island") {
                let full = core.cfg().hide_fullscreen && fullscreen_in_front();
                if full && !hidden_by_us && win.is_visible().unwrap_or(false) {
                    let _ = win.hide();
                    hidden_by_us = true;
                } else if !full && hidden_by_us {
                    let _ = win.show();
                    hidden_by_us = false;
                }
            }
            // system numbers every 2 seconds
            if tick % 2 == 0 {
                let sys = sample(&mut prev);
                // battery low: once at 20% and once at 10%, again after charging
                match (sys.battery, sys.charging) {
                    (Some(_), true) | (None, _) => battery_warned = None,
                    (Some(level), false) => {
                        for mark in [20u32, 10] {
                            if level <= mark && battery_warned.map_or(true, |w| w > mark) {
                                battery_warned = Some(mark);
                                core.notice("battery", &format!("Bateria em {level}%"), "Conecte o carregador.");
                                break;
                            }
                        }
                    }
                }
                *CURRENT.lock().unwrap() = Some(sys.clone());
                let _ = app.emit("sys", sys);
            }
            tick = tick.wrapping_add(1);
            std::thread::sleep(Duration::from_secs(1));
        }
    });
}
