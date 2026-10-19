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
    /// Pfad einer beim Laden zur Seite gelegten, unlesbaren Datenbank —
    /// die Oberfläche zeigt dazu einen Hinweis.
    pub quarantined: Mutex<Option<PathBuf>>,
}

impl AppState {
    /// Datenbank atomar speichern. Fehler gehen an die Oberfläche — ein
    /// stiller Fehlschlag hieße: der Nutzer glaubt, gespeichert zu haben.
    pub fn persist(&self) -> Result<(), String> {
        let db = self.db.lock().unwrap();
        let path = self.db_path.lock().unwrap();
        invoices_core::save_db(&db, &path).map_err(|e| format!("save-failed: {e}"))
    }
}
