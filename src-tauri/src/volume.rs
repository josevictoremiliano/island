//! The master volume: when it changes (volume keys, the mixer, any app) the island shows a volume bar for a moment.
use serde::Serialize;
use std::{sync::Arc, time::Duration};
use tauri::{AppHandle, Emitter};
use windows::Win32::{
    Media::Audio::{eConsole, eRender, Endpoints::IAudioEndpointVolume, IMMDeviceEnumerator, MMDeviceEnumerator},
    System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED},
};

use crate::core::Core;

#[derive(Serialize, Clone, PartialEq)]
pub struct Volume {
    pub level: u32,
    pub muted: bool,
}

fn endpoint() -> windows::core::Result<IAudioEndpointVolume> {
    unsafe {
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
        device.Activate(CLSCTX_ALL, None)
    }
}

fn read(ep: &IAudioEndpointVolume) -> windows::core::Result<Volume> {
    unsafe {
        let level = (ep.GetMasterVolumeLevelScalar()? * 100.0).round() as u32;
        let muted = ep.GetMute()?.as_bool();
        Ok(Volume { level, muted })
    }
}

pub fn spawn(app: AppHandle, core: Arc<Core>) {
    std::thread::spawn(move || {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
        let mut ep: Option<IAudioEndpointVolume> = None;
        let mut last: Option<Volume> = None;
        let mut tick = 0u32;
        loop {
            std::thread::sleep(Duration::from_millis(150));
            tick = tick.wrapping_add(1);
            if !core.cfg().volume_bar {
                last = None;
                continue;
            }
            // the default device can change (headphones plugged in): look again every few seconds
            if ep.is_none() || tick % 30 == 0 {
                ep = endpoint().ok();
            }
            let Some(v) = ep.as_ref().and_then(|e| read(e).ok()) else {
                ep = None;
                continue;
            };
            if last.as_ref() != Some(&v) {
                // the first reading is not a change
                if last.is_some() {
                    let _ = app.emit_to("island", "volume", v.clone());
                }
                last = Some(v);
            }
        }
    });
}
