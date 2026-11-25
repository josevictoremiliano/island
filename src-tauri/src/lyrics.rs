//! Song lyrics from lrclib.net (free and open, no account). Only the song's title, artist and length are sent.
//! Synced lyrics ("[01:02.30] line") follow the song; otherwise the plain text is shown.
use serde::Serialize;
use serde_json::Value;
use std::{sync::Arc, sync::Mutex, time::Duration};
use tauri::{AppHandle, Emitter};

use crate::core::Core;

#[derive(Serialize, Clone)]
pub struct Line {
    pub t: i64,
    pub text: String,
}

#[derive(Serialize, Clone, Default)]
pub struct Lyrics {
    /// which song these lyrics belong to ("title|artist")
    pub key: String,
    pub synced: bool,
    pub lines: Vec<Line>,
    pub plain: String,
    pub found: bool,
}

static CURRENT: Mutex<Option<Lyrics>> = Mutex::new(None);

pub fn current() -> Option<Lyrics> {
    CURRENT.lock().unwrap().clone()
}

fn parse_synced(text: &str) -> Vec<Line> {
    let mut lines = Vec::new();
    for raw in text.lines() {
        let mut rest = raw.trim();
        let mut stamps = Vec::new();
        // a line may carry several time stamps: [00:12.30][01:40.00] text
        while let Some(end) = rest.strip_prefix('[').and_then(|r| r.find(']')) {
            let stamp = &rest[1..end + 1];
            let mut parts = stamp.split(':');
            if let (Some(m), Some(s)) = (parts.next(), parts.next()) {
                if let (Ok(m), Ok(s)) = (m.parse::<f64>(), s.parse::<f64>()) {
                    stamps.push(((m * 60.0 + s) * 1000.0) as i64);
                }
            }
            rest = rest[end + 2..].trim_start();
        }
        for t in stamps {
            lines.push(Line { t, text: rest.to_string() });
        }
    }
    lines.sort_by_key(|l| l.t);
    lines
}

/// YouTube titles carry extras: "Song (Official Video) [4K]" → "Song".
fn clean_title(title: &str) -> String {
    let mut t = title.to_string();
    for (open, close) in [('(', ')'), ('[', ']')] {
        while let (Some(a), Some(b)) = (t.find(open), t.find(close)) {
            if b > a {
                t.replace_range(a..=b, "");
            } else {
                break;
            }
        }
    }
    t.split(" | ").next().unwrap_or(&t).trim().to_string()
}

async fn fetch(client: &reqwest::Client, title: &str, artist: &str, duration_s: i64) -> Option<Value> {
    let get = |url: String, query: Vec<(&'static str, String)>| {
        client
            .get(url)
            .query(&query)
            .header("user-agent", concat!("ilha/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(10))
            .send()
    };
    let mut title = clean_title(title);
    let mut artist = artist.to_string();
    // videos often say "Artist - Song" in the title and have no artist
    if artist.is_empty() {
        if let Some((a, t)) = title.split_once(" - ") {
            artist = a.trim().to_string();
            title = t.trim().to_string();
        }
    }
    let artist = artist.as_str();
    if !artist.is_empty() {
        let mut q = vec![("track_name", title.clone()), ("artist_name", artist.to_string())];
        if duration_s > 0 {
            q.push(("duration", duration_s.to_string()));
        }
        if let Ok(r) = get("https://lrclib.net/api/get".into(), q).await {
            if r.status().is_success() {
                if let Ok(v) = r.json::<Value>().await {
                    return Some(v);
                }
            }
        }
    }
    // no exact match: search, but only accept a result whose song (and artist, if known) really matches:
    // a radio stream or a random video must not get some other song's lyrics
    if artist.is_empty() {
        return None;
    }
    let simple = |s: &str| s.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect::<String>();
    let (want_title, want_artist) = (simple(&title), simple(artist));
    if want_title.len() < 2 {
        return None;
    }
    let q = vec![("track_name", title.clone()), ("artist_name", artist.to_string())];
    let r = get("https://lrclib.net/api/search".into(), q).await.ok()?;
    let list: Vec<Value> = r.json().await.ok()?;
    list.into_iter().find(|v| {
        let t = simple(v["trackName"].as_str().unwrap_or(""));
        let a = simple(v["artistName"].as_str().unwrap_or(""));
        let title_ok = !t.is_empty() && (t == want_title || t.contains(&want_title) || want_title.contains(&t));
        let artist_ok = !a.is_empty() && (a.contains(&want_artist) || want_artist.contains(&a));
        title_ok && artist_ok && (v["syncedLyrics"].is_string() || v["plainLyrics"].is_string())
    })
}

pub fn spawn(app: AppHandle, core: Arc<Core>) {
    tauri::async_runtime::spawn(async move {
        let client = reqwest::Client::new();
        let mut last_key = String::new();
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            let cfg = core.cfg();
            if !(cfg.apps.lyrics && cfg.apps.music) {
                continue;
            }
            let Some(m) = crate::media::current() else { continue };
            let key = format!("{}|{}", m.title, m.artist);
            if key == last_key {
                continue;
            }
            last_key = key.clone();
            let mut lyrics = Lyrics { key: key.clone(), ..Default::default() };
            if let Some(v) = fetch(&client, &m.title, &m.artist, m.duration_ms / 1000).await {
                if let Some(s) = v["syncedLyrics"].as_str() {
                    lyrics.lines = parse_synced(s);
                    lyrics.synced = !lyrics.lines.is_empty();
                }
                lyrics.plain = v["plainLyrics"].as_str().unwrap_or("").to_string();
                lyrics.found = lyrics.synced || !lyrics.plain.trim().is_empty();
            }
            *CURRENT.lock().unwrap() = Some(lyrics.clone());
            let _ = app.emit_to("island", "lyrics", lyrics);
        }
    });
}
