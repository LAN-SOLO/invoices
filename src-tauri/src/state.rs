use crate::settings::Settings;
use invoices_core::Db;
use std::path::PathBuf;
use std::sync::Mutex;

pub struct AppState {
    pub db: Mutex<Db>,
    pub settings: Mutex<Settings>,
    /// Pfad der aktiven Firmen-Datenbank — wechselt beim Firmenwechsel.
    pub db_path: Mutex<PathBuf>,
    pub active_company: Mutex<String>,
}

impl AppState {
    pub fn persist(&self) {
        let db = self.db.lock().unwrap();
        let path = self.db_path.lock().unwrap();
        let _ = invoices_core::save_db(&db, &path);
    }
}
