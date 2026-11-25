//! Connects the island to Claude Code by adding HTTP hooks (and a status line) to ~/.claude/settings.json.
//! Only entries that point at our own local server are ever touched, so the user's own hooks stay as they are.
use serde_json::{json, Value};
use std::path::PathBuf;

use crate::server::PORT;

const EVENTS: [&str; 9] = [
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "PostToolUseFailure",
    "PermissionRequest",
    "Notification",
    "Stop",
    "StopFailure",
    "SessionEnd",
];

/// Seconds Claude Code waits for an answer from the island on a permission request.
pub const PERMISSION_TIMEOUT_S: u64 = 600;

fn mark() -> String {
    format!("127.0.0.1:{PORT}/")
}

fn claude_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude"))
}

fn settings_path() -> Option<PathBuf> {
    claude_dir().map(|d| d.join("settings.json"))
}

/// Claude Code has been used on this PC (its folder exists).
pub fn found() -> bool {
    claude_dir().map(|d| d.exists()).unwrap_or(false)
}

fn read() -> Result<Value, String> {
    let path = settings_path().ok_or("Pasta do usuário não encontrada.")?;
    match std::fs::read_to_string(&path) {
        Ok(text) if text.trim().is_empty() => Ok(json!({})),
        Ok(text) => serde_json::from_str(&text)
            .map_err(|_| "~/.claude/settings.json não é um JSON válido. Nada foi alterado.".to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(json!({})),
        Err(e) => Err(format!("Não consegui ler ~/.claude/settings.json: {e}")),
    }
}

fn write(value: &Value) -> Result<(), String> {
    let path = settings_path().ok_or("Pasta do usuário não encontrada.")?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    // one-time backup of the user's own file, before we ever change it
    let backup = path.with_file_name("settings.json.ilha-backup");
    if path.exists() && !backup.exists() {
        let _ = std::fs::copy(&path, &backup);
    }
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())? + "\n";
    crate::write_atomic(&path, &text).map_err(|e| format!("Não consegui salvar ~/.claude/settings.json: {e}"))
}

fn is_ours(hook: &Value) -> bool {
    let m = mark();
    ["url", "command"]
        .iter()
        .any(|k| hook.get(*k).and_then(Value::as_str).map_or(false, |s| s.contains(&m)))
}

fn strip(settings: &mut Value) {
    let Some(obj) = settings.as_object_mut() else { return };
    let mut hooks_empty = false;
    if let Some(hooks) = obj.get_mut("hooks").and_then(Value::as_object_mut) {
        for (_, groups) in hooks.iter_mut() {
            if let Some(groups) = groups.as_array_mut() {
                for group in groups.iter_mut() {
                    if let Some(list) = group.get_mut("hooks").and_then(Value::as_array_mut) {
                        list.retain(|h| !is_ours(h));
                    }
                }
                groups.retain(|g| g.get("hooks").and_then(Value::as_array).map_or(true, |a| !a.is_empty()));
            }
        }
        hooks.retain(|_, groups| groups.as_array().map_or(true, |a| !a.is_empty()));
        hooks_empty = hooks.is_empty();
    }
    if hooks_empty {
        obj.remove("hooks");
    }
    if obj.get("statusLine").map_or(false, is_ours) {
        obj.remove("statusLine");
    }
}

pub fn is_installed() -> bool {
    let Ok(settings) = read() else { return false };
    settings
        .get("hooks")
        .and_then(Value::as_object)
        .map_or(false, |hooks| {
            hooks.values().filter_map(Value::as_array).flatten().any(|group| {
                group.get("hooks").and_then(Value::as_array).map_or(false, |l| l.iter().any(is_ours))
            })
        })
}

fn status_command(token: &str) -> String {
    format!(
        "curl.exe -s -m 2 --noproxy 127.0.0.1 -H \"X-Island-Token: {token}\" -H \"Content-Type: application/json\" --data-binary @- http://127.0.0.1:{PORT}/status"
    )
}

pub fn install(token: &str) -> Result<(), String> {
    let mut settings = read()?;
    if !settings.is_object() {
        settings = json!({});
    }
    strip(&mut settings);
    let obj = settings.as_object_mut().unwrap();
    let hooks = obj.entry("hooks").or_insert_with(|| json!({}));
    if !hooks.is_object() {
        *hooks = json!({});
    }
    let hooks = hooks.as_object_mut().unwrap();
    for event in EVENTS {
        let timeout = if event == "PermissionRequest" { PERMISSION_TIMEOUT_S } else { 3 };
        let hook = json!({
            "type": "http",
            "url": format!("http://127.0.0.1:{PORT}/hook?agent=claude"),
            "timeout": timeout,
            "headers": { "X-Island-Token": token }
        });
        let groups = hooks.entry(event).or_insert_with(|| json!([]));
        if !groups.is_array() {
            *groups = json!([]);
        }
        groups.as_array_mut().unwrap().push(json!({ "hooks": [hook] }));
    }
    // the status line feeds the 5-hour / weekly limit ring; never replace a status line the user made
    if !obj.contains_key("statusLine") {
        obj.insert(
            "statusLine".into(),
            json!({ "type": "command", "command": status_command(token), "padding": 0 }),
        );
    }
    write(&settings)
}

pub fn uninstall() -> Result<(), String> {
    let mut settings = read()?;
    strip(&mut settings);
    write(&settings)
}
