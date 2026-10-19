// Prevents an extra console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod companies;
mod settings;
mod state;

use state::AppState;
use std::sync::Mutex;
use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let mut registry = companies::load_registry(&handle);
            // ungültige IDs (manipulierte companies.json) nie als Pfad nutzen
            registry.list.retain(|c| companies::valid_id(&c.id));
            if registry.list.is_empty() {
                registry = companies::Registry::default();
            }
            if !registry.list.iter().any(|c| c.id == registry.active) {
                registry.active = registry.list[0].id.clone();
            }
            let _ = companies::store_registry(&handle, &registry);
            let active = registry.active.clone();
            let s = settings::load_for(&handle, &active);
            let db_path = companies::db_path(&handle, &active);
            let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
            let (db, quarantined) = invoices_core::load_db_or_quarantine(&db_path, &stamp);
            app.manage(AppState {
                db: Mutex::new(db),
                settings: Mutex::new(s),
                db_path: Mutex::new(db_path),
                active_company: Mutex::new(active),
                quarantined: Mutex::new(quarantined),
            });
            // automatisches lokales Backup beim Start (falls aktiviert) —
            // im Hintergrund: ein nicht erreichbarer Netz-/UNC-Ordner darf
            // den Fensterstart nicht blockieren
            let bg = app.handle().clone();
            std::thread::spawn(move || commands::auto_backup(&bg.state::<AppState>()));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_customers,
            commands::upsert_customer,
            commands::delete_customer,
            commands::list_products,
            commands::upsert_product,
            commands::delete_product,
            commands::list_docs,
            commands::upsert_doc,
            commands::finalize_doc,
            commands::set_paid,
            commands::cancel_doc,
            commands::delete_doc,
            commands::list_templates,
            commands::upsert_template,
            commands::delete_template,
            commands::export_backup,
            commands::import_backup,
            commands::get_settings,
            commands::set_settings,
            commands::list_companies,
            commands::add_company,
            commands::switch_company,
            commands::delete_company,
            commands::data_path,
            commands::startup_notice,
            commands::write_file,
            commands::get_logo,
            commands::einvoice_xml,
            commands::check_update,
            commands::install_update,
        ])
        .run(tauri::generate_context!())
        .expect("error while building invoices");
}
