//! The file tray: drop files on the island, drag them out again later (into an e-mail, a chat, a folder).
//! Only the paths are kept (in shelf.json); the files themselves stay where they are.
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};
use tauri::{AppHandle, Emitter};

const MAX_ITEMS: usize = 40;

#[derive(Serialize, Deserialize, Clone)]
pub struct ShelfItem {
    pub id: String,
    pub path: String,
    pub name: String,
    pub size: u64,
    pub dir: bool,
    pub added_at: i64,
    #[serde(default)]
    pub missing: bool,
}

static ITEMS: Mutex<Vec<ShelfItem>> = Mutex::new(Vec::new());
static FILE: OnceLock<PathBuf> = OnceLock::new();

pub fn init(data: &Path) {
    let file = data.join("shelf.json");
    let items: Vec<ShelfItem> = std::fs::read_to_string(&file)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    *ITEMS.lock().unwrap() = items;
    let _ = FILE.set(file);
}

fn save(items: &[ShelfItem]) {
    if let (Some(file), Ok(text)) = (FILE.get(), serde_json::to_string(items)) {
        let _ = crate::write_atomic(file, &text);
    }
}

pub fn list() -> Vec<ShelfItem> {
    let mut items = ITEMS.lock().unwrap().clone();
    for item in items.iter_mut() {
        item.missing = !Path::new(&item.path).exists();
    }
    items
}

fn emit(app: &AppHandle) {
    let _ = app.emit("shelf", list());
}

pub fn add(app: &AppHandle, paths: &[PathBuf]) {
    {
        let mut items = ITEMS.lock().unwrap();
        for path in paths {
            let text = path.to_string_lossy().to_string();
            items.retain(|i| i.path != text);
            let meta = std::fs::metadata(path).ok();
            items.insert(
                0,
                ShelfItem {
                    id: uuid::Uuid::new_v4().simple().to_string(),
                    name: path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| text.clone()),
                    path: text,
                    size: meta.as_ref().map(|m| m.len()).unwrap_or(0),
                    dir: meta.as_ref().map(|m| m.is_dir()).unwrap_or(false),
                    added_at: crate::core::now(),
                    missing: false,
                },
            );
        }
        items.truncate(MAX_ITEMS);
        save(&items);
    }
    emit(app);
}

pub fn remove(app: &AppHandle, id: Option<&str>) {
    {
        let mut items = ITEMS.lock().unwrap();
        match id {
            Some(id) => items.retain(|i| i.id != id),
            None => items.clear(),
        }
        save(&items);
    }
    emit(app);
}

pub fn paths(ids: &[String]) -> Vec<PathBuf> {
    ITEMS
        .lock()
        .unwrap()
        .iter()
        .filter(|i| ids.contains(&i.id))
        .map(|i| PathBuf::from(&i.path))
        .filter(|p| p.exists())
        .collect()
}

fn path_of(id: &str) -> Result<PathBuf, String> {
    paths(&[id.to_string()]).into_iter().next().ok_or_else(|| "Esse arquivo não existe mais nesse lugar.".to_string())
}

pub fn open(id: &str) -> Result<(), String> {
    let path = path_of(id)?;
    std::process::Command::new("explorer.exe").arg(&path).spawn().map(|_| ()).map_err(|e| e.to_string())
}

pub fn reveal(id: &str) -> Result<(), String> {
    let path = path_of(id)?;
    std::process::Command::new("explorer.exe")
        .arg(format!("/select,{}", path.display()))
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}
