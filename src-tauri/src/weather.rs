//! The weather in your city, from Open-Meteo (free, no account). Only the city name goes there.
use serde::Serialize;
use serde_json::Value;
use std::{
    sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex},
    time::Duration,
};
use tauri::{AppHandle, Emitter};

use crate::core::Core;

pub static NOW: AtomicBool = AtomicBool::new(false);
const EVERY: Duration = Duration::from_secs(15 * 60);

#[derive(Serialize, Clone, Default)]
pub struct Weather {
    pub city: String,
    pub temp: Option<f64>,
    /// WMO weather code
    pub code: i64,
    pub day: bool,
    pub max: Option<f64>,
    pub min: Option<f64>,
    pub error: Option<String>,
}

static CURRENT: Mutex<Option<Weather>> = Mutex::new(None);

pub fn current() -> Option<Weather> {
    CURRENT.lock().unwrap().clone()
}

async fn fetch(client: &reqwest::Client, city: &str) -> Result<Weather, String> {
    let get = |url: &str, query: Vec<(&str, String)>| {
        client.get(url).query(&query).timeout(Duration::from_secs(15)).send()
    };
    let place: Value = get(
        "https://geocoding-api.open-meteo.com/v1/search",
        vec![("name", city.to_string()), ("count", "1".into()), ("language", "pt".into())],
    )
    .await
    .map_err(|_| "Sem conexão para buscar o clima.".to_string())?
    .json()
    .await
    .map_err(|_| "Resposta inesperada do serviço de clima.".to_string())?;
    let p = place["results"].get(0).ok_or_else(|| format!("Não achei a cidade “{city}”."))?;
    let (lat, lon) = (p["latitude"].as_f64().unwrap_or(0.0), p["longitude"].as_f64().unwrap_or(0.0));
    let name = p["name"].as_str().unwrap_or(city).to_string();
    let w: Value = get(
        "https://api.open-meteo.com/v1/forecast",
        vec![
            ("latitude", lat.to_string()),
            ("longitude", lon.to_string()),
            ("current", "temperature_2m,weather_code,is_day".into()),
            ("daily", "temperature_2m_max,temperature_2m_min".into()),
            ("forecast_days", "1".into()),
            ("timezone", "auto".into()),
        ],
    )
    .await
    .map_err(|_| "Sem conexão para buscar o clima.".to_string())?
    .json()
    .await
    .map_err(|_| "Resposta inesperada do serviço de clima.".to_string())?;
    Ok(Weather {
        city: name,
        temp: w["current"]["temperature_2m"].as_f64(),
        code: w["current"]["weather_code"].as_i64().unwrap_or(0),
        day: w["current"]["is_day"].as_i64().unwrap_or(1) == 1,
        max: w["daily"]["temperature_2m_max"][0].as_f64(),
        min: w["daily"]["temperature_2m_min"][0].as_f64(),
        error: None,
    })
}

pub fn spawn(app: AppHandle, core: Arc<Core>) {
    tauri::async_runtime::spawn(async move {
        let client = reqwest::Client::new();
        let mut last: Option<std::time::Instant> = None;
        let mut last_city = String::new();
        loop {
            let city = core.cfg().weather_city.trim().to_string();
            if city.is_empty() {
                if !last_city.is_empty() {
                    last_city.clear();
                    *CURRENT.lock().unwrap() = None;
                    let _ = app.emit("weather", Option::<Weather>::None);
                }
            } else if NOW.swap(false, Ordering::SeqCst) || city != last_city || last.map_or(true, |t| t.elapsed() >= EVERY) {
                last = Some(std::time::Instant::now());
                last_city = city.clone();
                let w = fetch(&client, &city).await.unwrap_or_else(|e| Weather { city: city.clone(), error: Some(e), ..Default::default() });
                *CURRENT.lock().unwrap() = Some(w.clone());
                let _ = app.emit("weather", Some(w));
            }
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
    });
}
