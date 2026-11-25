//! What is playing (Spotify, browsers, any app that uses Windows' media controls) and play/pause/next/previous.
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde::Serialize;
use std::{sync::Mutex, time::Duration};
use tauri::{AppHandle, Emitter};
use windows::{
    core::Result,
    Media::Control::{
        GlobalSystemMediaTransportControlsSession as Session,
        GlobalSystemMediaTransportControlsSessionManager as Manager,
        GlobalSystemMediaTransportControlsSessionMediaProperties as Props,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
    },
    Storage::Streams::DataReader,
    Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED},
};

#[derive(Serialize, Clone, Default, PartialEq)]
pub struct Media {
    pub title: String,
    pub artist: String,
    pub app: String,
    pub playing: bool,
    pub position_ms: i64,
    /// unix ms at which `position_ms` was true
    pub position_at: i64,
    pub duration_ms: i64,
    /// album art as a data: URL
    pub art: String,
    pub can_next: bool,
    pub can_prev: bool,
}

static CURRENT: Mutex<Option<Media>> = Mutex::new(None);

pub fn current() -> Option<Media> {
    CURRENT.lock().unwrap().clone()
}

fn init_thread() {
    // WinRT needs the thread in an apartment; "already initialized" is fine
    unsafe {
        let _ = RoInitialize(RO_INIT_MULTITHREADED);
    }
}

fn session() -> Result<Session> {
    Manager::RequestAsync()?.join()?.GetCurrentSession()
}

fn app_name(id: &str) -> String {
    let id = id.rsplit('!').next().unwrap_or(id);
    let id = id.trim_end_matches(".exe").trim_end_matches(".EXE");
    match id.to_ascii_lowercase().as_str() {
        "spotify" => "Spotify".into(),
        "chrome" => "Chrome".into(),
        "msedge" => "Edge".into(),
        "firefox" => "Firefox".into(),
        "vivaldi" => "Vivaldi".into(),
        "brave" => "Brave".into(),
        _ if id.contains("ZuneMusic") => "Media Player".into(),
        _ => id.split('.').last().unwrap_or(id).to_string(),
    }
}

fn thumbnail(props: &Props) -> Result<String> {
    let stream = props.Thumbnail()?.OpenReadAsync()?.join()?;
    let size = stream.Size()? as u32;
    if size == 0 || size > 4_000_000 {
        return Ok(String::new());
    }
    let kind = stream.ContentType().map(|c| c.to_string()).unwrap_or_default();
    let reader = DataReader::CreateDataReader(&stream)?;
    reader.LoadAsync(size)?.join()?;
    let mut bytes = vec![0u8; size as usize];
    reader.ReadBytes(&mut bytes)?;
    let kind = if kind.starts_with("image/") { kind } else { "image/png".into() };
    Ok(format!("data:{kind};base64,{}", B64.encode(bytes)))
}

fn read(session: &Session, last: &Option<Media>) -> Result<Media> {
    let props = session.TryGetMediaPropertiesAsync()?.join()?;
    let title = props.Title()?.to_string();
    let artist = props.Artist()?.to_string();
    let info = session.GetPlaybackInfo()?;
    let playing = info.PlaybackStatus()? == Status::Playing;
    let controls = info.Controls()?;
    let timeline = session.GetTimelineProperties()?;
    let now = crate::core::now();
    let ticks = timeline.LastUpdatedTime()?.UniversalTime;
    let mut position_at = (ticks - 116_444_736_000_000_000) / 10_000;
    if position_at <= 0 || position_at > now + 5_000 || position_at < now - 86_400_000 {
        position_at = now;
    }
    let start = timeline.StartTime()?.Duration / 10_000;
    let art = match last {
        Some(m) if m.title == title && m.artist == artist && !m.art.is_empty() => m.art.clone(),
        _ => thumbnail(&props).unwrap_or_default(),
    };
    Ok(Media {
        title,
        artist,
        app: app_name(&session.SourceAppUserModelId()?.to_string()),
        playing,
        position_ms: timeline.Position()?.Duration / 10_000 - start,
        position_at,
        duration_ms: timeline.EndTime()?.Duration / 10_000 - start,
        art,
        can_next: controls.IsNextEnabled().unwrap_or(true),
        can_prev: controls.IsPreviousEnabled().unwrap_or(true),
    })
}

/// Only a seek, a new song or play/pause is news; the position moving on by itself is not.
fn changed(old: &Option<Media>, new: &Option<Media>) -> bool {
    match (old, new) {
        (Some(a), Some(b)) => {
            if (a.title.as_str(), a.artist.as_str(), a.playing, a.duration_ms, a.art.len(), a.app.as_str(), a.can_next, a.can_prev)
                != (b.title.as_str(), b.artist.as_str(), b.playing, b.duration_ms, b.art.len(), b.app.as_str(), b.can_next, b.can_prev)
            {
                return true;
            }
            let expected = a.position_ms + if a.playing { b.position_at - a.position_at } else { 0 };
            (b.position_ms - expected).abs() > 2_500
        }
        (None, None) => false,
        _ => true,
    }
}

pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        init_thread();
        loop {
            let last = current();
            let media = session()
                .and_then(|s| read(&s, &last))
                .ok()
                .filter(|m| !m.title.is_empty());
            if changed(&last, &media) {
                *CURRENT.lock().unwrap() = media.clone();
                let _ = app.emit("media", media);
            }
            std::thread::sleep(Duration::from_millis(900));
        }
    });
}

/// For `ilha --probe`: what Windows says is playing right now.
pub fn probe() -> String {
    init_thread();
    match session().and_then(|s| read(&s, &None)) {
        Ok(mut m) => {
            let art = m.art.len();
            m.art = format!("<{art} bytes>");
            serde_json::to_string_pretty(&m).unwrap_or_default()
        }
        Err(e) => format!("nada tocando ({e})"),
    }
}

pub fn control(action: &str) -> std::result::Result<(), String> {
    init_thread();
    let run = || -> Result<bool> {
        let s = session()?;
        match action {
            "toggle" => s.TryTogglePlayPauseAsync()?.join(),
            "next" => s.TrySkipNextAsync()?.join(),
            "prev" => s.TrySkipPreviousAsync()?.join(),
            _ => Ok(false),
        }
    };
    match run() {
        Ok(true) => Ok(()),
        Ok(false) => Err("O player não aceitou o comando.".into()),
        Err(_) => Err("Nada tocando agora.".into()),
    }
}
