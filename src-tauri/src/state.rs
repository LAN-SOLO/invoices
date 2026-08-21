use crate::settings::Settings;
use invoices_core::Db;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Manager;

pub struct AppState {
    pub db: Mutex<Db>,
    pub settings: Mutex<Settings>,
    pub db_path: PathBuf,
}

pub fn db_path(app: &tauri::AppHandle) -> PathBuf {
    let dir = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."));
    let _ = std::fs::create_dir_all(&dir);
    dir.join("db.json")
}

impl AppState {
    pub fn persist(&self) {
        let db = self.db.lock().unwrap();
        let _ = invoices_core::save_db(&db, &self.db_path);
    }
}
