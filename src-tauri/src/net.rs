//! Wi‑Fi and Bluetooth: on/off, the network you are on, the Bluetooth devices that are connected.
use serde::Serialize;
use std::{sync::Mutex, time::Duration};
use tauri::{AppHandle, Emitter};
use windows::{
    core::Result,
    Devices::{
        Bluetooth::{BluetoothConnectionStatus, BluetoothDevice, BluetoothLEDevice},
        Enumeration::DeviceInformation,
        Radios::{Radio, RadioAccessStatus, RadioKind, RadioState},
    },
    Networking::Connectivity::NetworkInformation,
    Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED},
};

#[derive(Serialize, Clone, Default, PartialEq)]
pub struct Net {
    /// None: this PC has no Wi‑Fi
    pub wifi: Option<bool>,
    pub network: String,
    /// connected by cable (or mobile data) instead of Wi‑Fi
    pub wired: bool,
    pub online: bool,
    /// None: this PC has no Bluetooth
    pub bluetooth: Option<bool>,
    pub devices: Vec<String>,
}

static CURRENT: Mutex<Option<Net>> = Mutex::new(None);

pub fn current() -> Option<Net> {
    CURRENT.lock().unwrap().clone()
}

fn init_thread() {
    unsafe {
        let _ = RoInitialize(RO_INIT_MULTITHREADED);
    }
}

fn radios() -> Result<Vec<Radio>> {
    Ok(Radio::GetRadiosAsync()?.join()?.into_iter().collect())
}

fn connected_devices() -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    let selectors = [
        BluetoothDevice::GetDeviceSelectorFromConnectionStatus(BluetoothConnectionStatus::Connected),
        BluetoothLEDevice::GetDeviceSelectorFromConnectionStatus(BluetoothConnectionStatus::Connected),
    ];
    for selector in selectors.into_iter().flatten() {
        if let Ok(found) = DeviceInformation::FindAllAsyncAqsFilter(&selector).and_then(|op| op.join()) {
            for device in found {
                if let Ok(name) = device.Name() {
                    let name = name.to_string();
                    if !name.is_empty() && !names.contains(&name) {
                        names.push(name);
                    }
                }
            }
        }
    }
    names
}

fn read(with_devices: bool, last: &Option<Net>) -> Net {
    let mut net = Net::default();
    for radio in radios().unwrap_or_default() {
        let on = radio.State().map(|s| s == RadioState::On).unwrap_or(false);
        match radio.Kind() {
            Ok(RadioKind::WiFi) => net.wifi = Some(net.wifi.unwrap_or(false) || on),
            Ok(RadioKind::Bluetooth) => net.bluetooth = Some(net.bluetooth.unwrap_or(false) || on),
            _ => {}
        }
    }
    if let Ok(profile) = NetworkInformation::GetInternetConnectionProfile() {
        net.online = true;
        let name = profile.ProfileName().map(|n| n.to_string()).unwrap_or_default();
        if profile.IsWlanConnectionProfile().unwrap_or(false) {
            net.network = name;
        } else {
            net.wired = true;
            net.network = if profile.IsWwanConnectionProfile().unwrap_or(false) { name } else { "Cabo (Ethernet)".into() };
        }
    }
    net.devices = if net.bluetooth == Some(true) {
        if with_devices { connected_devices() } else { last.as_ref().map(|n| n.devices.clone()).unwrap_or_default() }
    } else {
        Vec::new()
    };
    net
}

pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        init_thread();
        let mut tick = 0u32;
        loop {
            let last = current();
            // Bluetooth devices are slower to list: every 6 seconds is plenty
            let net = read(tick % 3 == 0, &last);
            if last.as_ref() != Some(&net) {
                *CURRENT.lock().unwrap() = Some(net.clone());
                let _ = app.emit("net", net);
            }
            tick = tick.wrapping_add(1);
            std::thread::sleep(Duration::from_secs(2));
        }
    });
}

/// Switch Wi‑Fi or Bluetooth on or off. Windows may refuse (policy, airplane mode): then the caller opens Settings.
pub fn set(kind: &str, on: bool) -> std::result::Result<(), String> {
    init_thread();
    let want = if kind == "wifi" { RadioKind::WiFi } else { RadioKind::Bluetooth };
    let access = Radio::RequestAccessAsync().and_then(|op| op.join()).map_err(|e| e.to_string())?;
    if access != RadioAccessStatus::Allowed {
        return Err("O Windows não deixou a Ilha mudar isso.".into());
    }
    let radio = radios()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|r| r.Kind().ok() == Some(want))
        .ok_or("Não achei esse rádio neste PC.")?;
    let state = if on { RadioState::On } else { RadioState::Off };
    match radio.SetStateAsync(state).and_then(|op| op.join()) {
        Ok(RadioAccessStatus::Allowed) => Ok(()),
        _ => Err("O Windows não deixou a Ilha mudar isso.".into()),
    }
}

pub fn open_settings(which: &str) {
    let uri = match which {
        "wifi" => "ms-settings:network-wifi",
        "bluetooth" => "ms-settings:bluetooth",
        _ => "ms-settings:network",
    };
    let _ = std::process::Command::new("explorer.exe").arg(uri).spawn();
}

/// For `ilha --probe`.
pub fn probe() -> String {
    init_thread();
    serde_json::to_string(&read(true, &None)).unwrap_or_default()
}
