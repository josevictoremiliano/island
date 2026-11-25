//! Codex's own use of your ChatGPT plan limits (5-hour and weekly window). Codex writes them into its session
//! files (~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl), so the island only has to read the newest one:
//! no login, no internet, nothing to connect.
use serde_json::Value;
use std::{
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use crate::core::{Core, Limit};

const TAIL: u64 = 512 * 1024;

fn sessions_dir() -> Option<PathBuf> {
    std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|h| h.join(".codex")))
        .map(|h| h.join("sessions"))
}

/// The newest session file: walk the latest year → month → day, falling back a day if one is empty.
fn newest_file() -> Option<PathBuf> {
    let root = sessions_dir()?;
    let mut days: Vec<PathBuf> = Vec::new();
    let mut years: Vec<PathBuf> = std::fs::read_dir(&root).ok()?.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    years.sort();
    for year in years.iter().rev().take(2) {
        let mut months: Vec<PathBuf> = std::fs::read_dir(year).ok()?.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
        months.sort();
        for month in months.iter().rev().take(2) {
            let mut d: Vec<PathBuf> = std::fs::read_dir(month).ok()?.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
            d.sort();
            days.extend(d.into_iter().rev().take(3));
        }
    }
    days.iter()
        .filter_map(|day| std::fs::read_dir(day).ok())
        .flat_map(|entries| entries.flatten())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".jsonl"))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .max_by_key(|(t, _)| *t)
        .map(|(_, p)| p)
}

/// The last `"rate_limits": {...}` object in the end of the file.
fn last_rate_limits(path: &Path) -> Option<Value> {
    let mut file = std::fs::File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    file.seek(SeekFrom::Start(len.saturating_sub(TAIL))).ok()?;
    let mut text = String::new();
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    text.push_str(&String::from_utf8_lossy(&bytes));
    // read whole lines from the end: each line is one JSON event
    for line in text.lines().rev() {
        if !line.contains("\"rate_limits\"") {
            continue;
        }
        if let Ok(event) = serde_json::from_str::<Value>(line) {
            if let Some(limits) = find(&event, "rate_limits") {
                if limits.is_object() {
                    return Some(limits.clone());
                }
            }
        }
    }
    None
}

fn find<'a>(v: &'a Value, key: &str) -> Option<&'a Value> {
    match v {
        Value::Object(o) => o.get(key).or_else(|| o.values().find_map(|x| find(x, key))),
        Value::Array(a) => a.iter().find_map(|x| find(x, key)),
        _ => None,
    }
}

fn window(v: &Value) -> Option<(i64, Limit)> {
    let used = v["used_percent"].as_f64()?;
    let minutes = v["window_minutes"].as_i64().unwrap_or(0);
    let resets = v["resets_at"].as_i64().map(Value::from).unwrap_or_else(|| {
        // older Codex versions: seconds from now
        v["resets_in_seconds"].as_i64().map(|s| Value::from(chrono::Utc::now().timestamp() + s)).unwrap_or(Value::Null)
    });
    Some((minutes, Limit { used: used.round() as i64, resets_at: resets }))
}

/// (5-hour, weekly) from Codex's newest session file.
pub fn read() -> Option<(Option<Limit>, Option<Limit>)> {
    let limits = last_rate_limits(&newest_file()?)?;
    let mut five = None;
    let mut week = None;
    for key in ["primary", "secondary"] {
        if let Some((minutes, limit)) = window(&limits[key]) {
            // the short window (5 h) and the long one (a week), whatever order Codex puts them in
            if minutes > 0 && minutes <= 24 * 60 {
                five = Some(limit);
            } else {
                week = Some(limit);
            }
        }
    }
    (five.is_some() || week.is_some()).then_some((five, week))
}

pub fn spawn(core: Arc<Core>) {
    std::thread::spawn(move || loop {
        if core.cfg().apps.usage {
            if let Some((five, week)) = read() {
                core.apply_codex_usage(five, week);
            }
        }
        std::thread::sleep(Duration::from_secs(20));
    });
}

/// For `ilha --probe`.
pub fn probe() -> String {
    match read() {
        Some((five, week)) => format!(
            "5h: {:?}% · semana: {:?}% (arquivo: {})",
            five.map(|l| l.used),
            week.map(|l| l.used),
            newest_file().map(|p| p.file_name().unwrap_or_default().to_string_lossy().to_string()).unwrap_or_default()
        ),
        None => "nenhum limite encontrado nas sessões do Codex".into(),
    }
}
