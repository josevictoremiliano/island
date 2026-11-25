//! Your Claude plan's use (5-hour and weekly limits), asked from Anthropic with the login Claude Code already saved
//! on this PC: the same question Claude Code's /usage asks. It is not a model call, so it uses no tokens.
//! The login is only read, never renewed or changed (Claude Code renews it itself), and only sent to api.anthropic.com.
//! Off unless you switch it on (Settings → Connections).
use serde_json::Value;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use crate::core::{Core, Limit};

const URL: &str = "https://api.anthropic.com/api/oauth/usage";
const EVERY: Duration = Duration::from_secs(5 * 60);

/// Ask again right away (the switch was just turned on).
pub static NOW: AtomicBool = AtomicBool::new(false);

fn limit(v: &Value) -> Option<Limit> {
    let used = v["utilization"].as_f64().or(v["used_percentage"].as_f64())?;
    Some(Limit { used: used.round() as i64, resets_at: v["resets_at"].clone() })
}

/// Claude's own login only. The same file also keeps logins of MCP connectors (other services): those must never
/// be sent anywhere, so only a top-level entry named like Claude's own login is accepted.
fn find_login(v: &Value) -> Option<&Value> {
    let obj = v.as_object()?;
    obj.iter()
        .filter(|(k, _)| {
            let k = k.to_ascii_lowercase();
            k.contains("claude") && !k.contains("mcp")
        })
        .map(|(_, v)| v)
        .find(|v| v.get("accessToken").or(v.get("access_token")).map_or(false, Value::is_string))
}

/// For diagnosis: the shape of the login file, names only, never values.
fn shape(v: &Value, depth: usize) -> String {
    match v.as_object() {
        Some(o) if depth < 2 => o
            .iter()
            .map(|(k, v)| if v.is_object() { format!("{k}{{{}}}", shape(v, depth + 1)) } else { k.clone() })
            .collect::<Vec<_>>()
            .join(", "),
        Some(_) => "…".into(),
        None => String::new(),
    }
}

async fn fetch(client: &reqwest::Client) -> Result<(Option<Limit>, Option<Limit>), String> {
    let path = dirs::home_dir().ok_or("Pasta do usuário não encontrada.")?.join(".claude").join(".credentials.json");
    let text = tokio::fs::read_to_string(&path)
        .await
        .map_err(|_| "Não encontrei o login do Claude Code. Entre uma vez com: claude".to_string())?;
    let login: Value = serde_json::from_str(&text).map_err(|_| "O arquivo de login do Claude Code não pôde ser lido.".to_string())?;
    let Some(oauth) = find_login(&login) else {
        // names only, never values: enough to see what kind of login this is
        let _ = shape; // kept for diagnosis while developing
        return Err("Não achei o login do Claude Code neste PC (o app Claude guarda o dele de outro jeito). Para ativar: no terminal, rode claude e faça /login uma vez.".into());
    };
    let token = oauth["accessToken"].as_str().or(oauth["access_token"].as_str()).unwrap_or_default();
    if oauth["expiresAt"].as_i64().map_or(false, |exp| exp < crate::core::now()) {
        return Err("O login do Claude Code expirou. Abra o Claude Code uma vez para ele renovar.".into());
    }
    let response = client
        .get(URL)
        .bearer_auth(token)
        .header("anthropic-beta", "oauth-2025-04-20")
        .header("user-agent", concat!("ilha/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .map_err(|_| "Sem conexão com a Anthropic agora.".to_string())?;
    let status = response.status();
    if status.as_u16() == 401 || status.as_u16() == 403 {
        return Err("A Anthropic recusou o login. Abra o Claude Code uma vez para ele renovar.".into());
    }
    if !status.is_success() {
        return Err(format!("A Anthropic respondeu {status}. Tento de novo em 5 minutos."));
    }
    let body: Value = response.json().await.map_err(|_| "Resposta inesperada da Anthropic.".to_string())?;
    let five = limit(&body["five_hour"]);
    let seven = limit(&body["seven_day"]);
    if five.is_none() && seven.is_none() {
        return Err("Sua conta não informou limites (o uso por limite só existe nos planos Pro e Max).".into());
    }
    Ok((five, seven))
}

pub fn spawn(core: Arc<Core>) {
    tauri::async_runtime::spawn(async move {
        let client = reqwest::Client::new();
        let mut last: Option<std::time::Instant> = None;
        loop {
            let due = NOW.swap(false, Ordering::SeqCst) || last.map_or(true, |t| t.elapsed() >= EVERY);
            if core.cfg().usage_from_account && due {
                last = Some(std::time::Instant::now());
                match fetch(&client).await {
                    Ok((five, seven)) => core.apply_usage(five, seven, None),
                    Err(e) => core.set_usage_error(Some(e)),
                }
            }
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
    });
}

/// For `ilha --probe`: only the numbers, never the login.
pub fn probe() -> String {
    tauri::async_runtime::block_on(async {
        match fetch(&reqwest::Client::new()).await {
            Ok((five, seven)) => format!(
                "5h: {:?}% (reinicia {}) · semana: {:?}%",
                five.as_ref().map(|l| l.used),
                five.as_ref().map(|l| l.resets_at.to_string()).unwrap_or_default(),
                seven.as_ref().map(|l| l.used)
            ),
            Err(e) => format!("erro: {e}"),
        }
    })
}
