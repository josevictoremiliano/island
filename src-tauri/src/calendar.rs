//! Your next events, from a calendar's secret iCal address (Google Calendar: Settings → your calendar →
//! "Secret address in iCal format"; Outlook: Publish calendar → ICS). No login: the address itself is the key.
//! Repeating events are expanded for the next two weeks. A notice comes 10 minutes before an event.
use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, TimeZone, Utc};
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex},
    time::Duration,
};
use tauri::{AppHandle, Emitter};

use crate::core::Core;

pub static NOW: AtomicBool = AtomicBool::new(false);
const EVERY: Duration = Duration::from_secs(10 * 60);
const DAYS: i64 = 14;

#[derive(Serialize, Clone)]
pub struct Event {
    pub id: String,
    pub title: String,
    pub start: i64,
    pub end: i64,
    pub all_day: bool,
    pub location: String,
}

#[derive(Serialize, Clone, Default)]
pub struct Calendar {
    pub events: Vec<Event>,
    pub error: Option<String>,
    pub updated_at: i64,
}

static CURRENT: Mutex<Option<Calendar>> = Mutex::new(None);

pub fn current() -> Option<Calendar> {
    CURRENT.lock().unwrap().clone()
}

struct Prop {
    name: String,
    params: Vec<(String, String)>,
    value: String,
}

impl Prop {
    fn param(&self, key: &str) -> Option<&str> {
        self.params.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v.as_str())
    }
}

fn unfold(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for raw in text.split('\n') {
        let raw = raw.trim_end_matches('\r');
        if (raw.starts_with(' ') || raw.starts_with('\t')) && !lines.is_empty() {
            lines.last_mut().unwrap().push_str(&raw[1..]);
        } else {
            lines.push(raw.to_string());
        }
    }
    lines
}

fn parse_prop(line: &str) -> Option<Prop> {
    // the value starts at the first ':' that is not inside quotes
    let mut quoted = false;
    let split = line.char_indices().find(|(_, c)| {
        if *c == '"' {
            quoted = !quoted;
        }
        *c == ':' && !quoted
    })?.0;
    let (left, value) = (&line[..split], &line[split + 1..]);
    let mut parts = left.split(';');
    let name = parts.next()?.to_ascii_uppercase();
    let params = parts
        .filter_map(|p| p.split_once('=').map(|(k, v)| (k.to_string(), v.trim_matches('"').to_string())))
        .collect();
    Some(Prop { name, params, value: value.to_string() })
}

fn find<'a>(ev: &'a [Prop], name: &str) -> Option<&'a Prop> {
    ev.iter().find(|p| p.name == name)
}

fn unescape(s: &str) -> String {
    s.replace("\\n", " ").replace("\\N", " ").replace("\\,", ",").replace("\\;", ";").replace("\\\\", "\\")
}

/// (moment in UTC, all day)
fn parse_time(p: &Prop) -> Option<(DateTime<Utc>, bool)> {
    let v = p.value.trim();
    if p.param("VALUE").map_or(false, |x| x.eq_ignore_ascii_case("DATE")) || v.len() == 8 {
        let d = NaiveDate::parse_from_str(&v[..8], "%Y%m%d").ok()?;
        let local = Local.from_local_datetime(&d.and_hms_opt(0, 0, 0)?).earliest()?;
        return Some((local.with_timezone(&Utc), true));
    }
    if let Some(utc) = v.strip_suffix('Z') {
        let n = NaiveDateTime::parse_from_str(utc, "%Y%m%dT%H%M%S").ok()?;
        return Some((Utc.from_utc_datetime(&n), false));
    }
    let n = NaiveDateTime::parse_from_str(v, "%Y%m%dT%H%M%S").ok()?;
    if let Some(tz) = p.param("TZID").and_then(|t| t.parse::<chrono_tz::Tz>().ok()) {
        return Some((tz.from_local_datetime(&n).earliest()?.with_timezone(&Utc), false));
    }
    // floating time, or a time zone name only Windows knows: take it as this PC's time
    Some((Local.from_local_datetime(&n).earliest()?.with_timezone(&Utc), false))
}

/// rrule wants UNTIL in UTC when the start is in UTC.
fn rrule_for_utc(rule: &str) -> String {
    rule.split(';')
        .map(|part| match part.split_once('=') {
            Some((k, v)) if k.eq_ignore_ascii_case("UNTIL") => {
                let v = if v.len() == 8 { format!("{v}T235959Z") } else if v.ends_with('Z') { v.to_string() } else { format!("{v}Z") };
                format!("UNTIL={v}")
            }
            _ => part.to_string(),
        })
        .collect::<Vec<_>>()
        .join(";")
}

fn parse(text: &str) -> Vec<Event> {
    let now = Utc::now();
    let (from, to) = (now - chrono::Duration::days(1), now + chrono::Duration::days(DAYS));
    let mut raw_events: Vec<Vec<Prop>> = Vec::new();
    let mut current: Option<Vec<Prop>> = None;
    for line in unfold(text) {
        match line.as_str() {
            "BEGIN:VEVENT" => current = Some(Vec::new()),
            "END:VEVENT" => {
                if let Some(ev) = current.take() {
                    raw_events.push(ev);
                }
            }
            _ => {
                if let (Some(ev), Some(p)) = (current.as_mut(), parse_prop(&line)) {
                    ev.push(p);
                }
            }
        }
    }
    // moved / changed single occurrences of a repeating event
    let mut overridden: HashMap<String, HashSet<i64>> = HashMap::new();
    for ev in &raw_events {
        if let (Some(uid), Some(rid)) = (find(ev, "UID"), find(ev, "RECURRENCE-ID")) {
            if let Some((t, _)) = parse_time(rid) {
                overridden.entry(uid.value.clone()).or_default().insert(t.timestamp());
            }
        }
    }
    let mut events = Vec::new();
    for ev in &raw_events {
        let prop = |name: &str| find(ev, name);
        if prop("STATUS").map_or(false, |s| s.value.eq_ignore_ascii_case("CANCELLED")) {
            continue;
        }
        let Some((start, all_day)) = prop("DTSTART").and_then(parse_time) else { continue };
        let end = prop("DTEND").and_then(parse_time).map(|(t, _)| t);
        let length = end.map(|e| e - start).unwrap_or_else(|| if all_day { chrono::Duration::days(1) } else { chrono::Duration::hours(1) });
        let uid = prop("UID").map(|p| p.value.clone()).unwrap_or_default();
        let title = prop("SUMMARY").map(|p| unescape(&p.value)).filter(|t| !t.trim().is_empty()).unwrap_or_else(|| "(sem título)".into());
        let location = prop("LOCATION").map(|p| unescape(&p.value)).unwrap_or_default();
        let mut starts = vec![start];
        if let Some(rule) = prop("RRULE") {
            if prop("RECURRENCE-ID").is_none() {
                let spec = format!("DTSTART:{}\nRRULE:{}", start.format("%Y%m%dT%H%M%SZ"), rrule_for_utc(&rule.value));
                let excluded: HashSet<i64> = ev
                    .iter()
                    .filter(|p| p.name == "EXDATE")
                    .flat_map(|p| {
                        p.value.split(',').filter_map(|v| parse_time(&Prop { name: p.name.clone(), params: p.params.clone(), value: v.to_string() })).map(|(t, _)| t.timestamp()).collect::<Vec<_>>()
                    })
                    .collect();
                let moved = overridden.get(&uid);
                if let Ok(set) = spec.parse::<rrule::RRuleSet>() {
                    let tz = rrule::Tz::UTC;
                    let result = set.after((from - length).with_timezone(&tz)).before(to.with_timezone(&tz)).all(200);
                    starts = result
                        .dates
                        .into_iter()
                        .map(|d| d.with_timezone(&Utc))
                        .filter(|d| !excluded.contains(&d.timestamp()) && !moved.map_or(false, |m| m.contains(&d.timestamp())))
                        .collect();
                }
            }
        }
        for s in starts {
            let e = s + length;
            if e <= now || s >= to {
                continue;
            }
            events.push(Event {
                id: format!("{uid}@{}", s.timestamp()),
                title: title.clone(),
                start: s.timestamp_millis(),
                end: e.timestamp_millis(),
                all_day,
                location: location.clone(),
            });
        }
    }
    events.sort_by_key(|e| e.start);
    events.truncate(60);
    events
}

async fn fetch(client: &reqwest::Client, url: &str) -> Result<Vec<Event>, String> {
    let url = url.trim();
    let url = if let Some(rest) = url.strip_prefix("webcal://") { format!("https://{rest}") } else { url.to_string() };
    let response = client
        .get(&url)
        .header("user-agent", concat!("ilha/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .map_err(|_| "Não consegui baixar a agenda. Está online?".to_string())?;
    if !response.status().is_success() {
        return Err(format!("O endereço da agenda respondeu {}. Confira o link.", response.status()));
    }
    let text = response.text().await.map_err(|e| e.to_string())?;
    if !text.contains("BEGIN:VCALENDAR") {
        return Err("Esse link não é uma agenda iCal (.ics).".into());
    }
    Ok(parse(&text))
}

pub fn spawn(app: AppHandle, core: Arc<Core>) {
    tauri::async_runtime::spawn(async move {
        let client = reqwest::Client::new();
        let mut last: Option<std::time::Instant> = None;
        let mut last_url = String::new();
        let mut warned: HashSet<String> = HashSet::new();
        loop {
            let cfg = core.cfg();
            let url = cfg.calendar_url.trim().to_string();
            if cfg.apps.calendar && !url.is_empty() {
                let due = NOW.swap(false, Ordering::SeqCst) || url != last_url || last.map_or(true, |t| t.elapsed() >= EVERY);
                if due {
                    last = Some(std::time::Instant::now());
                    last_url = url.clone();
                    let cal = match fetch(&client, &url).await {
                        Ok(events) => Calendar { events, error: None, updated_at: crate::core::now() },
                        Err(e) => Calendar { events: current().map(|c| c.events).unwrap_or_default(), error: Some(e), updated_at: crate::core::now() },
                    };
                    *CURRENT.lock().unwrap() = Some(cal.clone());
                    let _ = app.emit("calendar", cal);
                }
                // a notice 10 minutes before
                {
                    let now = crate::core::now();
                    for ev in current().map(|c| c.events).unwrap_or_default() {
                        if !ev.all_day && ev.start > now && ev.start - now <= 10 * 60_000 && warned.insert(ev.id.clone()) {
                            let at = Local.timestamp_millis_opt(ev.start).single().map(|d| d.format("%H:%M").to_string()).unwrap_or_default();
                            let mins = ((ev.start - now) as f64 / 60_000.0).round() as i64;
                            let body = if ev.location.is_empty() { format!("Às {at}") } else { format!("Às {at} · {}", ev.location) };
                            core.notice("calendar", &format!("Em {mins} min: {}", ev.title), &body);
                            let _ = app.emit_to("island", "chime", "question");
                        }
                    }
                }
            } else if !last_url.is_empty() {
                last_url.clear();
                *CURRENT.lock().unwrap() = None;
                let _ = app.emit("calendar", Calendar::default());
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    });
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_simple_repeating_and_all_day_events() {
        let now = Utc::now();
        let fmt = |d: DateTime<Utc>| d.format("%Y%m%dT%H%M%SZ").to_string();
        let soon = now + chrono::Duration::hours(2);
        let weekly = now - chrono::Duration::days(7) + chrono::Duration::hours(3);
        let skipped = weekly + chrono::Duration::days(7);
        let tomorrow = (Local::now() + chrono::Duration::days(1)).format("%Y%m%d").to_string();
        let ics = format!(
            "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:a\r\nDTSTART:{}\r\nDTEND:{}\r\nSUMMARY:Reuni\r\n ão\\, time\r\nEND:VEVENT\r\n\
             BEGIN:VEVENT\r\nUID:b\r\nDTSTART:{}\r\nDTEND:{}\r\nRRULE:FREQ=WEEKLY;COUNT=5\r\nEXDATE:{}\r\nSUMMARY:Semanal\r\nEND:VEVENT\r\n\
             BEGIN:VEVENT\r\nUID:c\r\nDTSTART;VALUE=DATE:{tomorrow}\r\nSUMMARY:Feriado\r\nEND:VEVENT\r\n\
             BEGIN:VEVENT\r\nUID:d\r\nSTATUS:CANCELLED\r\nDTSTART:{}\r\nSUMMARY:Cancelado\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
            fmt(soon), fmt(soon + chrono::Duration::hours(1)), fmt(weekly), fmt(weekly + chrono::Duration::hours(1)), fmt(skipped), fmt(soon)
        );
        let events = parse(&ics);
        let titles: Vec<&str> = events.iter().map(|e| e.title.as_str()).collect();
        assert!(titles.contains(&"Reunião, time"), "{titles:?}");
        assert!(titles.contains(&"Feriado"), "{titles:?}");
        assert!(!titles.contains(&"Cancelado"), "{titles:?}");
        // weekly: last week's is over, today's is excluded (EXDATE), next week's is in the 14-day window
        let weekly_left: Vec<_> = events.iter().filter(|e| e.title == "Semanal").collect();
        assert_eq!(weekly_left.len(), 1, "{titles:?}");
        assert_eq!(weekly_left[0].start / 1000, (weekly + chrono::Duration::days(14)).timestamp());
        assert!(events.iter().find(|e| e.title == "Feriado").unwrap().all_day);
    }
}