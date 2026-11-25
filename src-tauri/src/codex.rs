//! Connects the island to OpenAI Codex. Codex reads hooks from ~/.codex/hooks.json (same shape as Claude Code,
//! but only "command" hooks), so each event runs curl.exe, which posts it to the island's local server.
//! Codex asks you once to trust new hooks (/hooks in Codex).
use serde_json::{json, Value};
use std::path::PathBuf;

use crate::server::PORT;

const EVENTS: [&str; 7] = ["SessionStart", "UserPromptSubmit", "PreToolUse", "PostToolUse", "PermissionRequest", "Stop", "SessionEnd"];

fn home() -> Option<PathBuf> {
    std::env::var_os("CODEX_HOME").map(PathBuf::from).or_else(|| dirs::home_dir().map(|h| h.join(".codex")))
}

fn hooks_path() -> Option<PathBuf> {
    home().map(|h| h.join("hooks.json"))
}

fn on_path(name: &str) -> bool {
    std::env::var_os("PATH").map_or(false, |p| {
        std::env::split_paths(&p).any(|dir| ["exe", "cmd", "ps1"].iter().any(|ext| dir.join(format!("{name}.{ext}")).is_file()))
    })
}

/// Codex is on this PC: used before (its folder has more than our hooks.json), on the PATH, or the app is installed.
pub fn found() -> bool {
    let used = home()
        .and_then(|h| std::fs::read_dir(h).ok())
        .map_or(false, |entries| {
            entries.flatten().any(|e| {
                let n = e.file_name().to_string_lossy().to_lowercase();
                !n.starts_with("hooks.json") && n != "tmp"
            })
        });
    let app = dirs::data_local_dir()
        .and_then(|d| std::fs::read_dir(d.join("Packages")).ok())
        .map_or(false, |entries| entries.flatten().any(|e| e.file_name().to_string_lossy().to_lowercase().starts_with("openai.")));
    used || on_path("codex") || app
}

/// No quotes, no `&` and no `@` at the start of a word: older Codex versions and PowerShell break on those.
fn command(token: &str) -> String {
    format!(
        "curl.exe -s -m 3 --noproxy 127.0.0.1 -H X-Island-Token:{token} -H Content-Type:application/json -d@- http://127.0.0.1:{PORT}/codex"
    )
}

fn is_ours(hook: &Value) -> bool {
    hook["type"] == "command"
        && hook["command"].as_str().map_or(false, |c| c.contains(&format!("127.0.0.1:{PORT}/codex")))
}

fn read() -> Result<Value, String> {
    let path = hooks_path().ok_or("Pasta do usuário não encontrada.")?;
    match std::fs::read_to_string(&path) {
        Ok(t) if t.trim().is_empty() => Ok(json!({})),
        Ok(t) => serde_json::from_str(&t).map_err(|_| "~/.codex/hooks.json não é um JSON válido. Nada foi alterado.".to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(json!({})),
        Err(e) => Err(format!("Não consegui ler ~/.codex/hooks.json: {e}")),
    }
}

fn write(value: &Value) -> Result<(), String> {
    let path = hooks_path().ok_or("Pasta do usuário não encontrada.")?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let backup = path.with_file_name("hooks.json.ilha-backup");
    if path.exists() && !backup.exists() {
        let _ = std::fs::copy(&path, &backup);
    }
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())? + "\n";
    crate::write_atomic(&path, &text).map_err(|e| format!("Não consegui salvar ~/.codex/hooks.json: {e}"))
}

fn strip(config: &mut Value) {
    let Some(obj) = config.as_object_mut() else { return };
    let mut empty = false;
    if let Some(hooks) = obj.get_mut("hooks").and_then(Value::as_object_mut) {
        for (_, groups) in hooks.iter_mut() {
            if let Some(groups) = groups.as_array_mut() {
                for g in groups.iter_mut() {
                    if let Some(list) = g.get_mut("hooks").and_then(Value::as_array_mut) {
                        list.retain(|h| !is_ours(h));
                    }
                }
                groups.retain(|g| g.get("hooks").and_then(Value::as_array).map_or(true, |a| !a.is_empty()));
            }
        }
        hooks.retain(|_, g| g.as_array().map_or(true, |a| !a.is_empty()));
        empty = hooks.is_empty();
    }
    if empty {
        obj.remove("hooks");
    }
}

pub fn is_installed() -> bool {
    read().map_or(false, |c| {
        c["hooks"].as_object().map_or(false, |hooks| {
            hooks.values().filter_map(Value::as_array).flatten().any(|g| g["hooks"].as_array().map_or(false, |l| l.iter().any(is_ours)))
        })
    })
}

pub fn install(token: &str) -> Result<(), String> {
    let mut config = read()?;
    if !config.is_object() {
        config = json!({});
    }
    strip(&mut config);
    let obj = config.as_object_mut().unwrap();
    let hooks = obj.entry("hooks").or_insert_with(|| json!({}));
    if !hooks.is_object() {
        *hooks = json!({});
    }
    let hooks = hooks.as_object_mut().unwrap();
    for event in EVENTS {
        // the island only watches, so Codex never has to wait for it (SessionEnd allows no background hook)
        let mut hook = json!({ "type": "command", "command": command(token), "statusMessage": "Ilha" });
        if event == "SessionEnd" {
            hook["timeout"] = json!(3);
        } else {
            hook["timeout"] = json!(5);
            hook["async"] = json!(true);
        }
        let groups = hooks.entry(event).or_insert_with(|| json!([]));
        if !groups.is_array() {
            *groups = json!([]);
        }
        groups.as_array_mut().unwrap().push(json!({ "hooks": [hook] }));
    }
    write(&config)
}

pub fn uninstall() -> Result<(), String> {
    let mut config = read()?;
    strip(&mut config);
    write(&config)
}
