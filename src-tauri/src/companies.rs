//! Mehrfirmen-Verwaltung: eine Registry (companies.json im Config-Dir)
//! kennt alle Unternehmensprofile und das aktive. Jedes Unternehmen hat
//! eigene Einstellungen und eine eigene Datenbank — die Erst-Firma
//! ("default") nutzt die Alt-Pfade, damit bestehende Daten nahtlos
//! weiterlaufen.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::Manager;

pub const DEFAULT_ID: &str = "default";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanyEntry {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Registry {
    pub active: String,
    pub list: Vec<CompanyEntry>,
}

impl Default for Registry {
    fn default() -> Self {
        Registry {
            active: DEFAULT_ID.into(),
            list: vec![CompanyEntry {
                id: DEFAULT_ID.into(),
                label: String::new(),
            }],
        }
    }
}

fn registry_path(app: &tauri::AppHandle) -> PathBuf {
    let dir = app
        .path()
        .app_config_dir()
        .unwrap_or_else(|_| PathBuf::from("."));
    let _ = std::fs::create_dir_all(&dir);
    dir.join("companies.json")
}

pub fn load_registry(app: &tauri::AppHandle) -> Registry {
    let path = registry_path(app);
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return Registry::default();
    };
    match serde_json::from_str(&raw) {
        Ok(r) => r,
        Err(_) => {
            // unlesbare Registry nicht still durch die Standard-Registry
            // ersetzen (weitere Firmen wären dann unsichtbar) — Kopie zur
            // Seite legen; die Firmen-Dateien selbst bleiben unangetastet
            let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
            let _ = std::fs::copy(&path, path.with_file_name(format!("companies.json.corrupt-{stamp}")));
            Registry::default()
        }
    }
}

pub fn store_registry(app: &tauri::AppHandle, registry: &Registry) -> Result<(), String> {
    let json = serde_json::to_string_pretty(registry).map_err(|e| e.to_string())?;
    invoices_core::write_atomic(&registry_path(app), json.as_bytes())
        .map_err(|e| format!("save-failed: {e}"))
}

/// Firmen-IDs werden Teil von Datei- und Ordnernamen — nur ASCII-
/// Buchstaben, Ziffern und Bindestrich (UUID-Form), damit kein `..`,
/// Pfadtrenner oder Laufwerksbuchstabe aus der Oberfläche ins Dateisystem
/// durchschlägt.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// Einstellungs-Datei je Firma — "default" behält den Alt-Namen.
pub fn settings_path(app: &tauri::AppHandle, id: &str) -> PathBuf {
    let dir = app
        .path()
        .app_config_dir()
        .unwrap_or_else(|_| PathBuf::from("."));
    let _ = std::fs::create_dir_all(&dir);
    if id == DEFAULT_ID {
        dir.join("settings.json")
    } else {
        dir.join(format!("settings-{id}.json"))
    }
}

/// Datenbank je Firma — "default" behält den Alt-Pfad.
pub fn db_path(app: &tauri::AppHandle, id: &str) -> PathBuf {
    let dir = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."));
    let dir = if id == DEFAULT_ID {
        dir
    } else {
        dir.join(format!("company-{id}"))
    };
    let _ = std::fs::create_dir_all(&dir);
    dir.join("db.json")
}
