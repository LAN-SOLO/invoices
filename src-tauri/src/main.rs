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
            let registry = companies::load_registry(&handle);
            companies::store_registry(&handle, &registry);
            let active = registry.active.clone();
            let s = settings::load_for(&handle, &active);
            let db_path = companies::db_path(&handle, &active);
            let db = invoices_core::load_db(&db_path).unwrap_or_default();
            app.manage(AppState {
                db: Mutex::new(db),
                settings: Mutex::new(s),
                db_path: Mutex::new(db_path),
                active_company: Mutex::new(active),
            });
            // automatisches lokales Backup beim Start (falls aktiviert)
            commands::auto_backup(&app.state::<AppState>());
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
            commands::write_file,
            commands::get_logo,
            commands::einvoice_xml,
            commands::check_update,
            commands::install_update,
        ])
        .run(tauri::generate_context!())
        .expect("error while building invoices");
}
