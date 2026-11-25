//! Updates from GitHub Releases (josevictoremiliano/island). Every release carries a latest.json and an installer
//! signed with the project's key; Tauri checks that signature, so only our own builds can be installed.
//! Flow: check (30 s after start, then every 6 h, or on demand) → "new version" notice → download in the background
//! with progress → install as soon as no AI is working or waiting for you (or right away when you click "Install now").
use serde::Serialize;
use std::{sync::{Arc, Mutex}, time::Duration};
use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::core::Core;

const FIRST_CHECK: Duration = Duration::from_secs(30);
const EVERY: Duration = Duration::from_secs(6 * 60 * 60);

#[derive(Serialize, Clone, Default)]
pub struct UpdateState {
    /// idle | checking | latest | downloading | ready | installing | error
    pub status: String,
    pub current: String,
    pub version: String,
    pub notes: String,
    pub downloaded: u64,
    pub total: u64,
    pub error: Option<String>,
    pub checked_at: i64,
}

static STATE: Mutex<Option<UpdateState>> = Mutex::new(None);
static READY: Mutex<Option<(Update, Vec<u8>)>> = Mutex::new(None);
static BUSY: Mutex<bool> = Mutex::new(false);

pub fn current() -> UpdateState {
    STATE.lock().unwrap().clone().unwrap_or_else(|| UpdateState {
        status: "idle".into(),
        current: env!("CARGO_PKG_VERSION").into(),
        ..Default::default()
    })
}

fn set(app: &AppHandle, change: impl FnOnce(&mut UpdateState)) {
    let mut state = current();
    change(&mut state);
    *STATE.lock().unwrap() = Some(state.clone());
    let _ = app.emit("update", state);
}

/// Check GitHub; when there is a new version, download it (and install it when that is wanted and safe).
pub async fn check(app: AppHandle, core: Arc<Core>, manual: bool) {
    {
        let mut busy = BUSY.lock().unwrap();
        if *busy {
            return;
        }
        *busy = true;
    }
    run(&app, &core, manual).await;
    *BUSY.lock().unwrap() = false;
}

async fn run(app: &AppHandle, core: &Arc<Core>, manual: bool) {
    if READY.lock().unwrap().is_some() {
        // already downloaded: only the install is left
        if manual || core.cfg().auto_update {
            install_when_free(app, core, manual).await;
        }
        return;
    }
    set(app, |s| {
        s.status = "checking".into();
        s.error = None;
    });
    let found = match app.updater() {
        Ok(updater) => updater.check().await.map_err(|e| e.to_string()),
        Err(e) => Err(e.to_string()),
    };
    let update = match found {
        Ok(Some(u)) => u,
        Ok(None) => {
            set(app, |s| {
                s.status = "latest".into();
                s.checked_at = crate::core::now();
            });
            return;
        }
        Err(e) => {
            set(app, |s| {
                s.status = "error".into();
                s.error = Some(format!("Não consegui procurar atualizações: {e}"));
                s.checked_at = crate::core::now();
            });
            return;
        }
    };
    let version = update.version.clone();
    set(app, |s| {
        s.status = "downloading".into();
        s.version = version.clone();
        s.notes = update.body.clone().unwrap_or_default();
        s.downloaded = 0;
        s.total = 0;
        s.checked_at = crate::core::now();
    });
    core.notice("update", &format!("Nova versão disponível: v{version}"), "Baixando em segundo plano…");

    // download, telling the island how far it is (at most ~6 times a second)
    let progress_app = app.clone();
    let mut got: u64 = 0;
    let mut last_emit = std::time::Instant::now() - Duration::from_secs(1);
    let bytes = update
        .download(
            move |chunk, total| {
                got += chunk as u64;
                if last_emit.elapsed() >= Duration::from_millis(160) {
                    last_emit = std::time::Instant::now();
                    let total = total.unwrap_or(0);
                    set(&progress_app, |s| {
                        s.downloaded = got;
                        s.total = total;
                    });
                }
            },
            || {},
        )
        .await;
    match bytes {
        Ok(bytes) => {
            let size = bytes.len() as u64;
            *READY.lock().unwrap() = Some((update, bytes));
            set(app, |s| {
                s.status = "ready".into();
                s.downloaded = size;
                s.total = size;
            });
            if manual || core.cfg().auto_update {
                install_when_free(app, core, manual).await;
            } else {
                core.notice("update", &format!("A versão v{version} está pronta"), "Instale em Configurações → Geral → Atualizações.");
            }
        }
        Err(e) => set(app, |s| {
            s.status = "error".into();
            s.error = Some(format!("O download da versão v{version} falhou: {e}"));
        }),
    }
}

/// Install now, or (automatic) wait until no AI is working or waiting for you: an update must never cut a
/// permission question in half.
async fn install_when_free(app: &AppHandle, core: &Arc<Core>, now: bool) {
    if !now {
        let mut told = false;
        while core.busy() {
            if !told {
                told = true;
                let v = current().version;
                core.notice("update", &format!("A versão v{v} está pronta"), "Instalo assim que a IA terminar o que está fazendo.");
            }
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
    }
    install(app, core);
}

pub fn install(app: &AppHandle, core: &Arc<Core>) {
    let Some((update, bytes)) = READY.lock().unwrap().take() else { return };
    let v = update.version.clone();
    set(app, |s| s.status = "installing".into());
    core.notice("update", &format!("Atualizando para v{v}"), "A Ilha fecha e volta sozinha em alguns segundos.");
    // give the notice a moment to show; on Windows the installer then closes the app and opens the new one
    std::thread::sleep(Duration::from_millis(1200));
    if let Err(e) = update.install(bytes) {
        set(app, |s| {
            s.status = "error".into();
            s.error = Some(format!("A instalação falhou: {e}"));
        });
    }
}

pub fn spawn(app: AppHandle, core: Arc<Core>) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST_CHECK).await;
        loop {
            check(app.clone(), core.clone(), false).await;
            tokio::time::sleep(EVERY).await;
        }
    });
}
