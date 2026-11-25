//! Local HTTP server that Claude Code's hooks and status line talk to. Only listens on 127.0.0.1 and
//! only accepts requests that carry the island's token.
use axum::{
    body::Bytes,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use serde_json::{json, Value};
use std::{collections::HashMap, sync::Arc, time::Duration};

use crate::core::Core;

pub const PORT: u16 = 47900;

pub fn spawn(core: Arc<Core>) {
    tauri::async_runtime::spawn(async move {
        let router = Router::new()
            .route("/hook", post(hook))
            .route("/codex", post(codex))
            .route("/status", post(status))
            .with_state(core.clone());
        match tokio::net::TcpListener::bind(("127.0.0.1", PORT)).await {
            Ok(listener) => {
                if let Err(e) = axum::serve(listener, router).await {
                    *core.server_error.lock().unwrap() = Some(format!("O servidor local parou: {e}"));
                    core.emit();
                }
            }
            Err(e) => {
                *core.server_error.lock().unwrap() =
                    Some(format!("A porta {PORT} já está em uso ({e}). Outra cópia da Ilha está aberta?"));
                core.emit();
            }
        }
    });
}

/// A valid answer that decides nothing. An empty body made Claude Code warn that the hook gave no usable answer.
fn no_verdict() -> Response {
    Json(json!({})).into_response()
}

fn token(headers: &HeaderMap) -> Option<&str> {
    headers.get("x-island-token").and_then(|v| v.to_str().ok())
}

/// Clears the island's permission card if Claude Code stops waiting (it closed the request or it timed out).
struct Pending {
    core: Arc<Core>,
    sid: String,
    req: u64,
    answered: bool,
}

impl Drop for Pending {
    fn drop(&mut self) {
        if !self.answered {
            self.core.cancel_permission(&self.sid, self.req);
        }
    }
}

async fn hook(
    State(core): State<Arc<Core>>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let agent = query.get("agent").cloned().unwrap_or_else(|| "claude".into());
    handle(core, &agent, headers, body).await
}

/// Codex posts here (its hook command avoids `?` and `&`).
async fn codex(State(core): State<Arc<Core>>, headers: HeaderMap, body: Bytes) -> Response {
    handle(core, "codex", headers, body).await
}

async fn handle(core: Arc<Core>, agent: &str, headers: HeaderMap, body: Bytes) -> Response {
    if !core.authorized(token(&headers)) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let Ok(event) = serde_json::from_slice::<Value>(&body) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Some((sid, req, rx)) = core.on_hook(agent, &event) else {
        return no_verdict();
    };
    let mut pending = Pending { core: core.clone(), sid, req, answered: false };
    let wait = Duration::from_secs(crate::claude::PERMISSION_TIMEOUT_S - 10);
    match tokio::time::timeout(wait, rx).await {
        Ok(Ok(decision)) => {
            pending.answered = true;
            Json(json!({ "hookSpecificOutput": { "hookEventName": "PermissionRequest", "decision": decision } }))
                .into_response()
        }
        // no answer from the island: Claude Code asks you itself, as usual
        _ => no_verdict(),
    }
}

async fn status(State(core): State<Arc<Core>>, headers: HeaderMap, body: Bytes) -> Response {
    if !core.authorized(token(&headers)) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let data = serde_json::from_slice::<Value>(&body).unwrap_or(Value::Null);
    (StatusCode::OK, core.on_status(&data) + "\n").into_response()
}
