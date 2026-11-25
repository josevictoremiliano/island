//! The island's state: Claude Code sessions, permission requests waiting for an answer, history and settings.
use chrono::{Local, TimeZone};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use tauri_plugin_notification::NotificationExt;
use tokio::sync::oneshot;

use crate::config::Config;

const MAX_SESSIONS: usize = 8;
const MAX_STEPS: usize = 40;
const MAX_HISTORY: usize = 300;
const QUESTION_NOTIFICATIONS: [&str; 3] = ["elicitation_dialog", "elicitation_url_dialog", "agent_needs_input"];

pub fn now() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn clip(text: &str, max: usize) -> String {
    let text = text.trim();
    if text.chars().count() <= max {
        return text.to_string();
    }
    text.chars().take(max).collect::<String>() + "…"
}

fn file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

#[derive(Serialize, Clone, Default)]
pub struct Step {
    pub id: String,
    pub label: String,
    pub target: String,
    pub state: String,
}

#[derive(Serialize, Clone)]
pub struct Permission {
    pub kind: String,
    pub tool: String,
    pub what: String,
    pub always: bool,
}

#[derive(Serialize, Clone, Default)]
pub struct Session {
    pub id: String,
    pub agent: String,
    pub project: String,
    pub cwd: String,
    /// idle | running | question | done | error
    pub state: String,
    pub detail: String,
    pub prompt: String,
    pub steps: Vec<Step>,
    pub files: Vec<String>,
    pub last_message: String,
    pub question: String,
    pub permission: Option<Permission>,
    #[serde(skip)]
    pub suggestions: Vec<Value>,
    pub started_at: i64,
    pub ended_at: i64,
    pub updated_at: i64,
    pub model: String,
    pub context: Option<i64>,
}

#[derive(Serialize, serde::Deserialize, Clone)]
pub struct HistoryItem {
    pub id: String,
    pub agent: String,
    pub project: String,
    pub prompt: String,
    pub result: String,
    pub message: String,
    pub files: usize,
    pub duration_s: i64,
    pub at: i64,
}

#[derive(Serialize, serde::Deserialize, Clone, Default)]
pub struct Limit {
    pub used: i64,
    pub resets_at: Value,
}

#[derive(Serialize, serde::Deserialize, Clone, Default)]
pub struct Usage {
    pub five_hour: Option<Limit>,
    pub seven_day: Option<Limit>,
    pub updated_at: i64,
}

#[derive(Serialize, Clone)]
pub struct Stats {
    pub total: usize,
    pub today: usize,
    pub streak: usize,
}

#[derive(Serialize, Clone)]
pub struct ClaudeInfo {
    pub found: bool,
    pub installed: bool,
    pub seen: bool,
}

#[derive(Serialize, Clone)]
pub struct Snapshot {
    pub sessions: Vec<Session>,
    pub usage: Usage,
    pub codex_usage: Usage,
    pub stats: Stats,
    pub history: Vec<HistoryItem>,
    pub cfg: Config,
    pub claude: ClaudeInfo,
    pub codex: ClaudeInfo,
    pub server_error: Option<String>,
    pub shortcut_error: Option<String>,
    pub usage_error: Option<String>,
    pub version: String,
}

#[derive(Clone, Copy, Default, serde::Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

pub struct Core {
    pub app: AppHandle,
    pub token: String,
    data: PathBuf,
    cfg: Mutex<Config>,
    sessions: Mutex<Vec<Session>>,
    waiting: Mutex<HashMap<String, (u64, oneshot::Sender<Value>)>>,
    next_request: Mutex<u64>,
    history: Mutex<Vec<HistoryItem>>,
    usage: Mutex<Usage>,
    codex_usage: Mutex<Usage>,
    claude_installed: Mutex<bool>,
    claude_seen: Mutex<bool>,
    codex_installed: Mutex<bool>,
    codex_seen: Mutex<bool>,
    pub server_error: Mutex<Option<String>>,
    shortcut_error: Mutex<Option<String>>,
    usage_error: Mutex<Option<String>>,
    limit_warned: Mutex<std::collections::HashSet<String>>,
    pub hit: Mutex<Vec<Rect>>,
}

impl Core {
    pub fn new(app: AppHandle, data: PathBuf) -> (Self, bool) {
        let token_path = data.join("token");
        let token = match std::fs::read_to_string(&token_path) {
            Ok(t) if t.trim().len() >= 16 => t.trim().to_string(),
            _ => {
                let t = uuid::Uuid::new_v4().simple().to_string();
                let _ = std::fs::write(&token_path, &t);
                t
            }
        };
        let (mut cfg, first_run) = Config::load(&data.join("config.json"));
        let installed = crate::claude::is_installed();
        cfg.claude = installed;
        cfg.save(&data.join("config.json"));
        let usage: Usage = std::fs::read_to_string(data.join("usage.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        let history = std::fs::read_to_string(data.join("history.json"))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        (
            Self {
                app,
                token,
                data,
                cfg: Mutex::new(cfg),
                sessions: Mutex::new(Vec::new()),
                waiting: Mutex::new(HashMap::new()),
                next_request: Mutex::new(1),
                history: Mutex::new(history),
                usage: Mutex::new(usage),
                codex_usage: Mutex::new(Usage::default()),
                claude_installed: Mutex::new(installed),
                claude_seen: Mutex::new(false),
                codex_installed: Mutex::new(crate::codex::is_installed()),
                codex_seen: Mutex::new(false),
                server_error: Mutex::new(None),
                shortcut_error: Mutex::new(None),
                usage_error: Mutex::new(None),
                limit_warned: Mutex::new(Default::default()),
                hit: Mutex::new(Vec::new()),
            },
            first_run,
        )
    }

    pub fn cfg(&self) -> Config {
        self.cfg.lock().unwrap().clone()
    }

    pub fn authorized(&self, token: Option<&str>) -> bool {
        token == Some(self.token.as_str())
    }

    // ---------- snapshot ----------

    fn stats(history: &[HistoryItem]) -> Stats {
        let day = |at: i64| Local.timestamp_millis_opt(at).single().map(|d| d.date_naive());
        let done: Vec<_> = history.iter().filter(|h| h.result == "done").collect();
        let today = Local::now().date_naive();
        let days: std::collections::BTreeSet<_> = done.iter().filter_map(|h| day(h.at)).collect();
        let mut streak = 0;
        let mut cursor = if days.contains(&today) { today } else { today.pred_opt().unwrap() };
        while days.contains(&cursor) {
            streak += 1;
            cursor = cursor.pred_opt().unwrap();
        }
        Stats {
            total: done.len(),
            today: done.iter().filter(|h| day(h.at) == Some(today)).count(),
            streak,
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        let history = self.history.lock().unwrap();
        Snapshot {
            sessions: self.sessions.lock().unwrap().clone(),
            usage: self.usage.lock().unwrap().clone(),
            codex_usage: self.codex_usage.lock().unwrap().clone(),
            stats: Self::stats(&history),
            history: history.iter().rev().take(150).cloned().collect(),
            cfg: self.cfg(),
            claude: ClaudeInfo {
                found: crate::claude::found(),
                installed: *self.claude_installed.lock().unwrap(),
                seen: *self.claude_seen.lock().unwrap(),
            },
            codex: ClaudeInfo {
                found: crate::codex::found(),
                installed: *self.codex_installed.lock().unwrap(),
                seen: *self.codex_seen.lock().unwrap(),
            },
            server_error: self.server_error.lock().unwrap().clone(),
            shortcut_error: self.shortcut_error.lock().unwrap().clone(),
            usage_error: self.usage_error.lock().unwrap().clone(),
            version: env!("CARGO_PKG_VERSION").into(),
        }
    }

    pub fn emit(&self) {
        let _ = self.app.emit("state", self.snapshot());
    }

    // ---------- hooks ----------

    fn describe(ev: &Value) -> (String, String) {
        let tool = ev["tool_name"].as_str().unwrap_or("");
        let input = &ev["tool_input"];
        let s = |k: &str| input[k].as_str().unwrap_or("").to_string();
        let (label, target) = match tool {
            "Read" => ("Lendo", file_name(&s("file_path"))),
            "Edit" | "MultiEdit" => ("Editando", file_name(&s("file_path"))),
            "Write" => ("Criando", file_name(&s("file_path"))),
            "NotebookEdit" => ("Editando", file_name(&s("notebook_path"))),
            "Bash" | "PowerShell" => ("Rodando", clip(&s("command"), 140)),
            "Grep" => ("Buscando", s("pattern")),
            "Glob" => ("Procurando", s("pattern")),
            "WebFetch" => ("Abrindo", s("url")),
            "WebSearch" => ("Pesquisando", s("query")),
            "Task" | "Agent" => ("Subagente", s("description")),
            "TodoWrite" => ("Planejando", String::new()),
            "AskUserQuestion" => ("Perguntando", String::new()),
            "ExitPlanMode" => ("Plano pronto", String::new()),
            t if t.starts_with("mcp__") => ("Ferramenta", t.rsplit("__").next().unwrap_or(t).to_string()),
            t => return (t.to_string(), String::new()),
        };
        (label.to_string(), target)
    }

    fn permission_detail(ev: &Value) -> String {
        let input = &ev["tool_input"];
        let s = |k: &str| input[k].as_str().unwrap_or("").to_string();
        match ev["tool_name"].as_str().unwrap_or("") {
            "Bash" | "PowerShell" => clip(&s("command"), 700),
            "Edit" | "MultiEdit" | "Write" | "Read" => s("file_path"),
            "WebFetch" => s("url"),
            _ => clip(&serde_json::to_string(input).unwrap_or_default(), 400),
        }
    }

    /// Handles one hook event. For a permission request it returns a receiver that gets the island's decision.
    pub fn on_hook(&self, agent: &str, ev: &Value) -> Option<(String, u64, oneshot::Receiver<Value>)> {
        let name = ev["hook_event_name"].as_str().unwrap_or("");
        let sid = ev["session_id"].as_str().unwrap_or("").to_string();
        if sid.is_empty() {
            return None;
        }
        *(if agent == "codex" { &self.codex_seen } else { &self.claude_seen }).lock().unwrap() = true;
        let now = now();
        // any later event means an open permission question was answered elsewhere (e.g. in the terminal)
        if !matches!(name, "PermissionRequest" | "Notification" | "PreToolUse") {
            self.waiting.lock().unwrap().remove(&sid);
        }
        if name == "SessionEnd" {
            self.sessions.lock().unwrap().retain(|s| s.id != sid);
            self.emit();
            return None;
        }

        let mut result = None;
        let mut finished: Option<(&str, Session)> = None;
        {
            let mut list = self.sessions.lock().unwrap();
            let idx = match list.iter().position(|s| s.id == sid) {
                Some(i) => i,
                None => {
                    list.push(Session {
                        id: sid.clone(),
                        agent: agent.into(),
                        state: "idle".into(),
                        started_at: now,
                        ..Default::default()
                    });
                    list.len() - 1
                }
            };
            let s = &mut list[idx];
            if let Some(cwd) = ev["cwd"].as_str() {
                if s.cwd.is_empty() {
                    s.cwd = cwd.to_string();
                    s.project = file_name(cwd);
                }
            }
            s.updated_at = now;
            let start_if_needed = |s: &mut Session| {
                if s.state != "running" && s.state != "question" {
                    s.state = "running".into();
                    s.started_at = now;
                    s.ended_at = 0;
                }
            };

            match name {
                "UserPromptSubmit" => {
                    s.state = "running".into();
                    s.started_at = now;
                    s.ended_at = 0;
                    s.steps.clear();
                    s.files.clear();
                    s.permission = None;
                    s.question.clear();
                    s.last_message.clear();
                    s.prompt = clip(ev["prompt"].as_str().unwrap_or(""), 300);
                    s.detail = "Pensando…".into();
                }
                "PreToolUse" => {
                    start_if_needed(s);
                    let (label, target) = Self::describe(ev);
                    s.detail = if target.is_empty() { label.clone() } else { format!("{label} {target}") };
                    if matches!(ev["tool_name"].as_str(), Some("Edit" | "MultiEdit" | "Write" | "NotebookEdit")) {
                        if !target.is_empty() && !s.files.contains(&target) {
                            s.files.push(target.clone());
                        }
                    }
                    s.steps.push(Step {
                        id: ev["tool_use_id"].as_str().unwrap_or("").into(),
                        label,
                        target,
                        state: "running".into(),
                    });
                    if s.steps.len() > MAX_STEPS {
                        s.steps.remove(0);
                    }
                }
                "PostToolUse" | "PostToolUseFailure" => {
                    let id = ev["tool_use_id"].as_str().unwrap_or("");
                    let failed = name == "PostToolUseFailure";
                    if let Some(step) = s.steps.iter_mut().rev().find(|st| st.id == id || (id.is_empty() && st.state == "running")) {
                        step.state = if failed { "failed" } else { "done" }.into();
                    }
                    s.permission = None;
                    s.suggestions.clear();
                    start_if_needed(s);
                    if s.state == "question" {
                        s.state = "running".into();
                    }
                    s.detail = "Pensando…".into();
                }
                "PermissionRequest" => {
                    let tool = ev["tool_name"].as_str().unwrap_or("");
                    if agent == "codex" {
                        // Codex runs its hooks in the background: the island can only say it is waiting
                        let (label, target) = Self::describe(ev);
                        s.state = "question".into();
                        s.detail = "Quer permissão: responda no Codex".into();
                        s.question = if target.is_empty() { label } else { format!("{label} {target}") };
                        finished = Some(("question", s.clone()));
                    } else if tool == "AskUserQuestion" {
                        // answered in Claude Code itself; the island only says it is waiting
                        let q = ev["tool_input"]["questions"][0]["question"].as_str().unwrap_or("Claude tem uma pergunta.");
                        s.state = "question".into();
                        s.detail = "Tem uma pergunta para você".into();
                        s.question = clip(q, 400);
                        finished = Some(("question", s.clone()));
                    } else {
                        s.suggestions = ev["permission_suggestions"]
                            .as_array()
                            .map(|a| {
                                a.iter()
                                    .filter(|u| {
                                        let t = u["type"].as_str().unwrap_or("");
                                        (t == "addRules" && u["behavior"] == "allow")
                                            || (t == "setMode" && u["mode"] != "bypassPermissions")
                                            || t == "addDirectories"
                                    })
                                    .take(6)
                                    .cloned()
                                    .collect()
                            })
                            .unwrap_or_default();
                        let (label, target) = Self::describe(ev);
                        let plan = tool == "ExitPlanMode";
                        s.permission = Some(Permission {
                            kind: if plan { "plan" } else { "tool" }.into(),
                            tool: if plan { "Plano pronto para aprovar".into() } else if target.is_empty() { label } else { format!("{label} {target}") },
                            what: if plan { clip(ev["tool_input"]["plan"].as_str().unwrap_or(""), 1500) } else { Self::permission_detail(ev) },
                            always: !s.suggestions.is_empty(),
                        });
                        s.state = "question".into();
                        s.detail = "Quer sua permissão".into();
                        let mut next = self.next_request.lock().unwrap();
                        let req = *next;
                        *next += 1;
                        let (tx, rx) = oneshot::channel();
                        self.waiting.lock().unwrap().insert(sid.clone(), (req, tx));
                        result = Some((sid.clone(), req, rx));
                        finished = Some(("permission", s.clone()));
                    }
                }
                "Notification" => {
                    let kind = ev["notification_type"].as_str().unwrap_or("");
                    if QUESTION_NOTIFICATIONS.contains(&kind) && s.state != "question" {
                        s.state = "question".into();
                        s.detail = "Esperando você".into();
                        s.question = clip(ev["message"].as_str().unwrap_or("Claude precisa de você."), 400);
                        finished = Some(("question", s.clone()));
                    }
                }
                "Stop" => {
                    let message = ev["last_assistant_message"].as_str().unwrap_or("").trim().to_string();
                    let asks = message.lines().last().map_or(false, |l| l.trim_end().ends_with('?'));
                    s.state = if asks { "question" } else { "done" }.into();
                    s.ended_at = now;
                    s.permission = None;
                    s.suggestions.clear();
                    for st in s.steps.iter_mut().filter(|st| st.state == "running") {
                        st.state = "done".into();
                    }
                    s.last_message = clip(&message, 4000);
                    s.detail = if asks { "Tem uma pergunta para você" } else { "Pronto" }.into();
                    s.question = if asks { clip(&message, 400) } else { String::new() };
                    finished = Some((if asks { "question" } else { "done" }, s.clone()));
                }
                "StopFailure" => {
                    s.state = "error".into();
                    s.ended_at = now;
                    s.permission = None;
                    s.detail = clip(ev["error"].as_str().or(ev["message"].as_str()).unwrap_or("Algo deu errado"), 160);
                    finished = Some(("error", s.clone()));
                }
                _ => {}
            }
            list.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
            list.truncate(MAX_SESSIONS);
        }
        if let Some((kind, s)) = finished {
            self.finish(kind, &s, name == "Stop" || name == "StopFailure");
        }
        self.emit();
        result
    }

    fn finish(&self, kind: &str, s: &Session, ended: bool) {
        let cfg = self.cfg();
        let project = if !s.project.is_empty() { s.project.clone() } else if s.agent == "codex" { "Codex".to_string() } else { "Claude Code".to_string() };
        if ended {
            let mut history = self.history.lock().unwrap();
            history.push(HistoryItem {
                id: s.id.clone(),
                agent: s.agent.clone(),
                project: project.clone(),
                prompt: s.prompt.clone(),
                result: if kind == "question" { "done".into() } else { kind.into() },
                message: clip(&s.last_message, 600),
                files: s.files.len(),
                duration_s: ((s.ended_at - s.started_at) / 1000).max(0),
                at: now(),
            });
            let excess = history.len().saturating_sub(MAX_HISTORY);
            history.drain(0..excess);
            if let Ok(text) = serde_json::to_string(&*history) {
                let _ = crate::write_atomic(&self.data.join("history.json"), &text);
            }
        }
        let _ = cfg;
        let agent = if s.agent == "codex" { "Codex" } else { "Claude Code" };
        // "project: what you asked" (or what it answered), like the card in the screenshot
        let about = |text: &str| clip(&format!("{project}: {}", text.replace('\n', " ")), 180);
        let (id, title, body) = match kind {
            "done" => ("ai_done", format!("{agent} terminou"), about(if s.prompt.is_empty() { &s.last_message } else { &s.prompt })),
            "error" => ("ai_error", format!("{agent}: erro"), about(&s.detail)),
            "permission" => (
                "ai_permission",
                format!("{agent} quer permissão"),
                about(&s.permission.as_ref().map(|p| format!("{} {}", p.tool, p.what)).unwrap_or_default()),
            ),
            _ => ("ai_question", format!("{agent} tem uma pergunta"), about(&s.question)),
        };
        self.notice(id, &title, &body);
        let _ = self.app.emit_to("island", "chime", if kind == "permission" { "question" } else { kind });
    }

    /// One notice, shown where you chose for its kind (Settings → Notifications): a card under the island,
    /// Windows' own notification, or nothing. A card falls back to Windows while the island is hidden (fullscreen).
    pub fn notice(&self, id: &str, title: &str, body: &str) {
        let mode = self.cfg().notices.get(id).cloned().unwrap_or_else(|| "card".into());
        if mode == "off" {
            return;
        }
        let visible = self
            .app
            .get_webview_window("island")
            .map_or(false, |w| w.is_visible().unwrap_or(false));
        if mode == "windows" || !visible {
            let _ = self.app.notification().builder().title(title).body(if body.is_empty() { " " } else { body }).show();
        } else {
            let _ = self.app.emit_to("island", "notice", json!({ "id": id, "title": title, "body": body, "at": now() }));
        }
    }

    /// "Limit almost full": once per window, when the 5-hour use crosses the level you chose.
    fn check_limit(&self, who: &str, five: &Option<Limit>) {
        let Some(l) = five else { return };
        let level = self.cfg().limit_warn as i64;
        if level <= 0 || l.used < level {
            return;
        }
        let key = format!("{who}|{}|{level}", l.resets_at);
        if !self.limit_warned.lock().unwrap().insert(key) {
            return;
        }
        let reset = match &l.resets_at {
            Value::Number(n) => n.as_i64().map(|s| if s < 1_000_000_000_000 { s * 1000 } else { s }),
            Value::String(s) => chrono::DateTime::parse_from_rfc3339(s).ok().map(|d| d.timestamp_millis()),
            _ => None,
        };
        let body = reset
            .and_then(|ms| Local.timestamp_millis_opt(ms).single())
            .map(|d| format!("Às {} ele volta a encher.", d.format("%H:%M")))
            .unwrap_or_default();
        self.notice("limit", &format!("Você usou {}% do limite de 5h do {who}", l.used), &body);
    }

    /// The island answered a permission request.
    pub fn decide(&self, sid: &str, behavior: &str) -> Result<(), String> {
        let entry = self.waiting.lock().unwrap().remove(sid);
        let suggestions = {
            let mut list = self.sessions.lock().unwrap();
            let s = list.iter_mut().find(|s| s.id == sid);
            let suggestions = s.as_ref().map(|s| s.suggestions.clone()).unwrap_or_default();
            if let Some(s) = s {
                s.permission = None;
                s.suggestions.clear();
                s.state = "running".into();
                s.detail = if behavior == "deny" { "Negado. Continuando…" } else { "Permitido. Continuando…" }.into();
            }
            suggestions
        };
        let decision = match behavior {
            "allow" => json!({ "behavior": "allow" }),
            "always" => json!({ "behavior": "allow", "updatedPermissions": suggestions }),
            _ => json!({ "behavior": "deny", "message": "O usuário negou pela Ilha." }),
        };
        self.emit();
        match entry.map(|(_, tx)| tx.send(decision)) {
            Some(Ok(())) => Ok(()),
            _ => Err("O Claude Code não está mais esperando. Responda no terminal.".into()),
        }
    }

    /// A permission request ended without an island answer (timeout, or answered in the terminal).
    pub fn cancel_permission(&self, sid: &str, req: u64) {
        let mut waiting = self.waiting.lock().unwrap();
        if waiting.get(sid).map(|(r, _)| *r) != Some(req) {
            return;
        }
        waiting.remove(sid);
        drop(waiting);
        if let Some(s) = self.sessions.lock().unwrap().iter_mut().find(|s| s.id == sid) {
            if s.permission.is_some() {
                s.permission = None;
                s.state = "running".into();
                s.detail = "Respondido no terminal".into();
            }
        }
        self.emit();
    }

    /// An AI is working or waiting for you (an update waits for this to end).
    pub fn busy(&self) -> bool {
        self.sessions.lock().unwrap().iter().any(|s| s.permission.is_some() || s.state == "running" || (s.state == "question" && s.ended_at == 0))
    }

    pub fn dismiss(&self, sid: &str) {
        self.sessions.lock().unwrap().retain(|s| s.id != sid || s.permission.is_some() || s.state == "running");
        self.emit();
    }

    // ---------- status line ----------

    pub fn on_status(&self, data: &Value) -> String {
        let pct = |v: &Value| v.as_f64().map(|f| f.round() as i64);
        let limit = |v: &Value| {
            pct(&v["used_percentage"]).map(|used| Limit { used, resets_at: v["resets_at"].clone() })
        };
        let model = data["model"]["display_name"].as_str().or(data["model"]["id"].as_str()).unwrap_or("").to_string();
        let context = pct(&data["context_window"]["used_percentage"]);
        {
            let mut usage = self.usage.lock().unwrap();
            if let Some(l) = limit(&data["rate_limits"]["five_hour"]) {
                usage.five_hour = Some(l);
            }
            if let Some(l) = limit(&data["rate_limits"]["seven_day"]) {
                usage.seven_day = Some(l);
            }
            usage.updated_at = now();
            // keep the last numbers: the status line only runs in the terminal, not in the app's Code tab
            if let Ok(text) = serde_json::to_string(&*usage) {
                let _ = crate::write_atomic(&self.data.join("usage.json"), &text);
            }
        }
        let five_now = self.usage.lock().unwrap().five_hour.clone();
        self.check_limit("Claude", &five_now);
        if let Some(sid) = data["session_id"].as_str() {
            if let Some(s) = self.sessions.lock().unwrap().iter_mut().find(|s| s.id == sid) {
                s.model = model.clone();
                s.context = context;
            }
        }
        self.emit();
        let usage = self.usage.lock().unwrap();
        let mut parts = vec!["Ilha".to_string()];
        if !model.is_empty() {
            parts.push(model);
        }
        if let Some(l) = &usage.five_hour {
            parts.push(format!("5h: {}%", l.used));
        }
        if let Some(l) = &usage.seven_day {
            parts.push(format!("semana: {}%", l.used));
        }
        if let Some(c) = context {
            parts.push(format!("contexto: {c}%"));
        }
        parts.join(" · ")
    }

    /// New limit numbers from your account (account_usage.rs).
    pub fn apply_usage(&self, five: Option<Limit>, seven: Option<Limit>, error: Option<String>) {
        {
            let mut usage = self.usage.lock().unwrap();
            if five.is_some() {
                usage.five_hour = five;
            }
            if seven.is_some() {
                usage.seven_day = seven;
            }
            usage.updated_at = now();
            if let Ok(text) = serde_json::to_string(&*usage) {
                let _ = crate::write_atomic(&self.data.join("usage.json"), &text);
            }
        }
        *self.usage_error.lock().unwrap() = error;
        let five = self.usage.lock().unwrap().five_hour.clone();
        self.check_limit("Claude", &five);
        self.emit();
    }

    /// Codex's limits, read from its session files (codex_usage.rs). Only emits when something changed.
    pub fn apply_codex_usage(&self, five: Option<Limit>, seven: Option<Limit>) {
        {
            let mut usage = self.codex_usage.lock().unwrap();
            let same = |a: &Option<Limit>, b: &Option<Limit>| match (a, b) {
                (Some(a), Some(b)) => a.used == b.used && a.resets_at == b.resets_at,
                (None, None) => true,
                _ => false,
            };
            if same(&usage.five_hour, &five) && same(&usage.seven_day, &seven) {
                return;
            }
            usage.five_hour = five;
            usage.seven_day = seven;
            usage.updated_at = now();
        }
        let five = self.codex_usage.lock().unwrap().five_hour.clone();
        self.check_limit("Codex", &five);
        self.emit();
    }

    pub fn set_usage_error(&self, error: Option<String>) {
        *self.usage_error.lock().unwrap() = error;
        let five = self.usage.lock().unwrap().five_hour.clone();
        self.check_limit("Claude", &five);
        self.emit();
    }

    // ---------- settings ----------

    pub fn apply_shortcut(&self) {
        let shortcut = self.cfg().shortcut;
        let gs = self.app.global_shortcut();
        let _ = gs.unregister_all();
        let error = if shortcut.trim().is_empty() {
            None
        } else {
            gs.register(shortcut.as_str())
                .err()
                .map(|e| format!("Não consegui usar o atalho {shortcut}: {e}"))
        };
        if let Some(e) = &error {
            self.notice("keys_taken", "Atalho ocupado", e);
        }
        *self.shortcut_error.lock().unwrap() = error;
    }

    pub fn save_config(&self, new: Config) -> Result<(), String> {
        use tauri_plugin_autostart::ManagerExt;
        let old = self.cfg();
        if new.autostart != old.autostart {
            let launcher = self.app.autolaunch();
            let r = if new.autostart { launcher.enable() } else { launcher.disable() };
            r.map_err(|e| format!("Não consegui mudar a inicialização com o Windows: {e}"))?;
        }
        let mut new = new;
        new.links.truncate(4);
        for link in new.links.iter_mut() {
            link.target = crate::links::normalize(&link.target);
        }
        new.claude = *self.claude_installed.lock().unwrap();
        new.save(&self.data.join("config.json"));
        let shortcut_changed = new.shortcut != old.shortcut;
        if new.usage_from_account && !old.usage_from_account {
            crate::account_usage::NOW.store(true, std::sync::atomic::Ordering::SeqCst);
        }
        if !new.usage_from_account {
            *self.usage_error.lock().unwrap() = None;
        }
        *self.cfg.lock().unwrap() = new;
        if shortcut_changed {
            self.apply_shortcut();
        }
        self.emit();
        Ok(())
    }

    pub fn set_claude(&self, on: bool) -> Result<(), String> {
        if on {
            crate::claude::install(&self.token)?;
        } else {
            crate::claude::uninstall()?;
        }
        let installed = crate::claude::is_installed();
        *self.claude_installed.lock().unwrap() = installed;
        {
            let mut cfg = self.cfg.lock().unwrap();
            cfg.claude = installed;
            cfg.save(&self.data.join("config.json"));
        }
        self.emit();
        Ok(())
    }

    pub fn set_codex(&self, on: bool) -> Result<(), String> {
        if on {
            crate::codex::install(&self.token)?;
        } else {
            crate::codex::uninstall()?;
        }
        *self.codex_installed.lock().unwrap() = crate::codex::is_installed();
        self.emit();
        Ok(())
    }

    pub fn clear_history(&self) {
        self.history.lock().unwrap().clear();
        let _ = std::fs::remove_file(self.data.join("history.json"));
        self.emit();
    }

    pub fn note_path(&self) -> PathBuf {
        self.data.join("note.txt")
    }

    pub fn notify(&self, title: &str, body: &str) {
        let _ = self.app.notification().builder().title(title).body(body).show();
    }
}
