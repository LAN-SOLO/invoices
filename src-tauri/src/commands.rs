use crate::settings::{self, Settings};
use crate::state::AppState;
use base64::Engine;
use invoices_core::{
    doc_matches, workflow, Customer, Doc, DocKind, DocStatus, NumberFormat, Product, Template,
};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

fn today() -> chrono::NaiveDate {
    chrono::Local::now().date_naive()
}

/// Nummernformat aus den Einstellungen: Jahr immer das des Belegdatums
/// (bzw. das aktuelle), fortlaufend wenn abgeschaltet.
fn number_format(settings: &Settings, doc_year: Option<i32>) -> NumberFormat<'_> {
    NumberFormat {
        year: if settings.number_include_year {
            Some(doc_year.unwrap_or_else(|| {
                use chrono::Datelike;
                today().year()
            }))
        } else {
            None
        },
        digits: settings.number_digits.clamp(1, 8) as usize,
        separator: &settings.number_separator,
    }
}

/// Präfix je Belegart — leere Präfixe fallen auf den Standard zurück,
/// damit nie Nummern wie "-2026-0001" entstehen.
fn prefix_for(settings: &Settings, kind: DocKind) -> String {
    let (configured, fallback) = match kind {
        DocKind::Invoice => (&settings.invoice_prefix, "RE"),
        DocKind::CreditNote => (&settings.credit_prefix, "GS"),
        DocKind::Cancellation => (&settings.cancel_prefix, "ST"),
        DocKind::Quote => (&settings.quote_prefix, "AN"),
        DocKind::OrderConfirmation => (&settings.order_prefix, "AB"),
        DocKind::DeliveryNote => (&settings.delivery_prefix, "LS"),
    };
    let trimmed = configured.trim();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}

/// Speichern; schlägt das fehl, wird der In-Memory-Stand auf den
/// Platten-Stand zurückgesetzt, damit Oberfläche und Datei nicht
/// auseinanderlaufen (z. B. eine vergebene, aber nie gespeicherte Nummer).
fn commit(st: &AppState) -> Result<(), String> {
    if let Err(e) = st.persist() {
        let path = st.db_path.lock().unwrap().clone();
        if let Some(db) = invoices_core::load_db(&path) {
            *st.db.lock().unwrap() = db;
        }
        return Err(e);
    }
    Ok(())
}

// --- customers ---------------------------------------------------------

#[tauri::command]
pub fn list_customers(st: State<'_, AppState>) -> Vec<Customer> {
    let db = st.db.lock().unwrap();
    let mut out = db.customers.clone();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

#[tauri::command]
pub fn upsert_customer(app: AppHandle, st: State<'_, AppState>, customer: Customer) -> Result<(), String> {
    if customer.id.trim().is_empty() {
        return Err("bad-id".into());
    }
    {
        let mut db = st.db.lock().unwrap();
        if let Some(existing) = db.customers.iter_mut().find(|c| c.id == customer.id) {
            *existing = customer;
        } else {
            db.customers.push(customer);
        }
    }
    commit(&st)?;
    let _ = app.emit("db-changed", ());
    Ok(())
}

#[tauri::command]
pub fn delete_customer(app: AppHandle, st: State<'_, AppState>, id: String) -> Result<(), String> {
    {
        let mut db = st.db.lock().unwrap();
        workflow::delete_customer(&mut db, &id)?;
    }
    commit(&st)?;
    let _ = app.emit("db-changed", ());
    Ok(())
}

// --- products ----------------------------------------------------------

#[tauri::command]
pub fn list_products(st: State<'_, AppState>) -> Vec<Product> {
    let db = st.db.lock().unwrap();
    let mut out = db.products.clone();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

#[tauri::command]
pub fn upsert_product(app: AppHandle, st: State<'_, AppState>, product: Product) -> Result<(), String> {
    if product.id.trim().is_empty() {
        return Err("bad-id".into());
    }
    {
        let mut db = st.db.lock().unwrap();
        if let Some(existing) = db.products.iter_mut().find(|p| p.id == product.id) {
            *existing = product;
        } else {
            db.products.push(product);
        }
    }
    commit(&st)?;
    let _ = app.emit("db-changed", ());
    Ok(())
}

#[tauri::command]
pub fn delete_product(app: AppHandle, st: State<'_, AppState>, id: String) -> Result<(), String> {
    {
        let mut db = st.db.lock().unwrap();
        db.products.retain(|p| p.id != id);
    }
    commit(&st)?;
    let _ = app.emit("db-changed", ());
    Ok(())
}

// --- documents ---------------------------------------------------------

#[tauri::command]
pub fn list_docs(st: State<'_, AppState>, query: String) -> Vec<Doc> {
    let db = st.db.lock().unwrap();
    let mut out: Vec<Doc> = db
        .docs
        .iter()
        .filter(|d| doc_matches(d, &query))
        .cloned()
        .collect();
    // newest first: by date, then creation time
    out.sort_by(|a, b| b.date.cmp(&a.date).then(b.created_at.cmp(&a.created_at)));
    out
}

/// Create or update a document. Content edits are only allowed while the
/// document is a draft — finalized documents are immutable (GoBD-Denke:
/// korrigiert wird per Storno/Gutschrift, nicht per Radiergummi).
#[tauri::command]
pub fn upsert_doc(app: AppHandle, st: State<'_, AppState>, doc: Doc) -> Result<(), String> {
    {
        let mut db = st.db.lock().unwrap();
        workflow::upsert_draft(&mut db, doc)?;
    }
    commit(&st)?;
    let _ = app.emit("db-changed", ());
    Ok(())
}

/// Assign a sequential number and set the document live (status Open).
#[tauri::command]
pub fn finalize_doc(app: AppHandle, st: State<'_, AppState>, id: String) -> Result<Doc, String> {
    let finalized = {
        let settings = st.settings.lock().unwrap().clone();
        let mut db = st.db.lock().unwrap();
        let kind = db.docs.iter().find(|d| d.id == id).map(|d| d.kind).ok_or("doc-not-found")?;
        let prefix = prefix_for(&settings, kind);
        workflow::finalize(&mut db, &id, &prefix, |year| number_format(&settings, year))?
    };
    // Nummer nicht vergeben, wenn sie nicht auf die Platte kam — sonst
    // entstünde nach einem Neustart eine Lücke/Doppelung.
    commit(&st)?;
    let _ = app.emit("db-changed", ());
    Ok(finalized)
}

#[tauri::command]
pub fn set_paid(app: AppHandle, st: State<'_, AppState>, id: String, paid: bool) -> Result<(), String> {
    {
        let mut db = st.db.lock().unwrap();
        let today = today().format("%Y-%m-%d").to_string();
        workflow::set_paid(&mut db, &id, paid, &today)?;
    }
    commit(&st)?;
    let _ = app.emit("db-changed", ());
    Ok(())
}

/// Cancel a finalized invoice: creates a linked cancellation document with
/// negated positions — sofort festgeschrieben und nummeriert — and marks
/// the original as cancelled.
#[tauri::command]
pub fn cancel_doc(app: AppHandle, st: State<'_, AppState>, id: String, new_id: String) -> Result<Doc, String> {
    let storno = {
        let settings = st.settings.lock().unwrap().clone();
        let mut db = st.db.lock().unwrap();
        let date = today().format("%Y-%m-%d").to_string();
        let prefix = prefix_for(&settings, DocKind::Cancellation);
        workflow::cancel(
            &mut db,
            &id,
            &new_id,
            &prefix,
            number_format(&settings, None),
            &date,
            &chrono::Local::now().to_rfc3339(),
        )?
    };
    commit(&st)?;
    let _ = app.emit("db-changed", ());
    Ok(storno)
}

#[tauri::command]
pub fn delete_doc(app: AppHandle, st: State<'_, AppState>, id: String) -> Result<(), String> {
    {
        let mut db = st.db.lock().unwrap();
        workflow::delete_draft(&mut db, &id)?;
    }
    commit(&st)?;
    let _ = app.emit("db-changed", ());
    Ok(())
}

// --- templates ---------------------------------------------------------

#[tauri::command]
pub fn list_templates(st: State<'_, AppState>) -> Vec<Template> {
    let db = st.db.lock().unwrap();
    let mut out = db.templates.clone();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

#[tauri::command]
pub fn upsert_template(app: AppHandle, st: State<'_, AppState>, template: Template) -> Result<(), String> {
    if template.id.trim().is_empty() {
        return Err("bad-id".into());
    }
    {
        let mut db = st.db.lock().unwrap();
        if let Some(existing) = db.templates.iter_mut().find(|t| t.id == template.id) {
            *existing = template;
        } else {
            db.templates.push(template);
        }
    }
    commit(&st)?;
    let _ = app.emit("db-changed", ());
    Ok(())
}

#[tauri::command]
pub fn delete_template(app: AppHandle, st: State<'_, AppState>, id: String) -> Result<(), String> {
    {
        let mut db = st.db.lock().unwrap();
        db.templates.retain(|t| t.id != id);
    }
    commit(&st)?;
    let _ = app.emit("db-changed", ());
    Ok(())
}

// --- backup ------------------------------------------------------------

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Backup {
    app: String,
    backup_version: u32,
    exported_at: String,
    db: invoices_core::Db,
    settings: Settings,
}

/// Vollständiges Backup (Belege, Kunden, Artikel, Vorlagen, Zähler,
/// Einstellungen) als eine JSON-Datei.
#[tauri::command]
pub fn export_backup(st: State<'_, AppState>, path: String) -> Result<(), String> {
    let backup = Backup {
        app: "invoices".into(),
        backup_version: 1,
        exported_at: chrono::Local::now().to_rfc3339(),
        db: st.db.lock().unwrap().clone(),
        settings: st.settings.lock().unwrap().clone(),
    };
    let path = checked_path(&path, &["json"])?;
    let json = serde_json::to_string_pretty(&backup).map_err(|e| e.to_string())?;
    invoices_core::write_atomic(&path, json.as_bytes()).map_err(|e| e.to_string())
}

/// Backup wiederherstellen — ersetzt ALLE Daten und Einstellungen.
#[tauri::command]
pub fn import_backup(app: AppHandle, st: State<'_, AppState>, path: String) -> Result<(), String> {
    let path = std::path::PathBuf::from(&path);
    let meta = std::fs::metadata(&path).map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.len() > MAX_BACKUP_BYTES {
        return Err("backup-invalid".into());
    }
    let raw = std::fs::read_to_string(&path).map_err(|_| "backup-invalid".to_string())?;
    let backup: Backup = serde_json::from_str(&raw).map_err(|_| "backup-invalid".to_string())?;
    if backup.app != "invoices" || backup.backup_version == 0 || backup.backup_version > 1 {
        return Err("backup-invalid".into());
    }
    // Sicherheitsnetz: aktuellen Stand vor dem Ersetzen neben die
    // Datenbank legen — eine versehentliche Wiederherstellung ist so
    // umkehrbar.
    {
        let db_path = st.db_path.lock().unwrap().clone();
        if db_path.exists() {
            let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
            let _ = std::fs::copy(&db_path, db_path.with_file_name(format!("db.json.before-restore-{stamp}")));
        }
    }
    let active = st.active_company.lock().unwrap().clone();
    settings::store_for(&app, &active, &backup.settings)?;
    *st.db.lock().unwrap() = backup.db;
    *st.settings.lock().unwrap() = backup.settings;
    commit(&st)?;
    let _ = app.emit("db-changed", ());
    Ok(())
}

/// Automatisches Backup beim Start (settings.auto_backup): schreibt in den
/// Backup-Ordner und behält die letzten 10 Stände.
pub fn auto_backup(st: &AppState) {
    let settings = st.settings.lock().unwrap().clone();
    if !settings.auto_backup || settings.backup_dir.trim().is_empty() {
        return;
    }
    let dir = std::path::PathBuf::from(settings.backup_dir.trim());
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let backup = Backup {
        app: "invoices".into(),
        backup_version: 1,
        exported_at: chrono::Local::now().to_rfc3339(),
        db: st.db.lock().unwrap().clone(),
        settings,
    };
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    if let Ok(json) = serde_json::to_string_pretty(&backup) {
        let _ = invoices_core::write_atomic(
            &dir.join(format!("invoices-backup-{stamp}.json")),
            json.as_bytes(),
        );
    }
    // aufräumen: nur die 10 neuesten behalten
    if let Ok(entries) = std::fs::read_dir(&dir) {
        let mut backups: Vec<_> = entries
            .flatten()
            .filter(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                name.starts_with("invoices-backup-") && name.ends_with(".json")
            })
            .collect();
        backups.sort_by_key(|e| e.file_name());
        while backups.len() > 10 {
            let oldest = backups.remove(0);
            let _ = std::fs::remove_file(oldest.path());
        }
    }
}

// --- settings & files --------------------------------------------------

#[tauri::command]
pub fn get_settings(st: State<'_, AppState>) -> Settings {
    st.settings.lock().unwrap().clone()
}

#[tauri::command]
pub fn set_settings(app: AppHandle, st: State<'_, AppState>, new: Settings) -> Result<(), String> {
    let active = st.active_company.lock().unwrap().clone();
    settings::store_for(&app, &active, &new)?;
    // Registry-Label mit dem Firmennamen synchron halten
    let mut registry = crate::companies::load_registry(&app);
    if let Some(entry) = registry.list.iter_mut().find(|c| c.id == active) {
        entry.label = new.company_name.clone();
        crate::companies::store_registry(&app, &registry)?;
    }
    *st.settings.lock().unwrap() = new;
    let _ = app.emit("db-changed", ());
    Ok(())
}

// --- companies ---------------------------------------------------------

#[tauri::command]
pub fn list_companies(app: AppHandle) -> crate::companies::Registry {
    crate::companies::load_registry(&app)
}

#[tauri::command]
pub fn add_company(
    app: AppHandle,
    st: State<'_, AppState>,
    id: String,
    label: String,
) -> Result<(), String> {
    if !crate::companies::valid_id(&id) || id == crate::companies::DEFAULT_ID {
        return Err("bad-id".into());
    }
    let label = label.trim().to_string();
    if label.is_empty() {
        return Err("bad-label".into());
    }
    let mut registry = crate::companies::load_registry(&app);
    if registry.list.iter().any(|c| c.id == id) {
        return Err("company-exists".into());
    }
    // Startprofil: leere Firma mit Namen — Einstellungen der aktuellen Firma
    // für Sprache und Darstellung übernehmen, Firmendaten bleiben leer.
    let current = st.settings.lock().unwrap().clone();
    let mut fresh = Settings::default();
    fresh.company_name = label.clone();
    fresh.language = current.language;
    fresh.theme = current.theme;
    fresh.accent = current.accent;
    settings::store_for(&app, &id, &fresh)?;
    registry.list.push(crate::companies::CompanyEntry {
        id: id.clone(),
        label,
    });
    crate::companies::store_registry(&app, &registry)?;
    switch_company(app, st, id)
}

/// Aktive Firma wechseln: aktuellen Stand sichern, Ziel-Profil laden.
#[tauri::command]
pub fn switch_company(app: AppHandle, st: State<'_, AppState>, id: String) -> Result<(), String> {
    let mut registry = crate::companies::load_registry(&app);
    if !crate::companies::valid_id(&id) || !registry.list.iter().any(|c| c.id == id) {
        return Err("company-not-found".into());
    }
    st.persist()?;
    let new_settings = settings::load_for(&app, &id);
    let new_db_path = crate::companies::db_path(&app, &id);
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let (new_db, quarantined) = invoices_core::load_db_or_quarantine(&new_db_path, &stamp);
    if quarantined.is_some() {
        *st.quarantined.lock().unwrap() = quarantined;
    }
    *st.settings.lock().unwrap() = new_settings;
    *st.db.lock().unwrap() = new_db;
    *st.db_path.lock().unwrap() = new_db_path;
    *st.active_company.lock().unwrap() = id.clone();
    registry.active = id;
    crate::companies::store_registry(&app, &registry)?;
    let _ = app.emit("db-changed", ());
    Ok(())
}

/// Firma aus der Registry nehmen — Dateien bleiben auf der Platte
/// (bewusst: Belege verschwinden nie einfach).
#[tauri::command]
pub fn delete_company(app: AppHandle, st: State<'_, AppState>, id: String) -> Result<(), String> {
    let active = st.active_company.lock().unwrap().clone();
    if id == active {
        return Err("company-active".into());
    }
    let mut registry = crate::companies::load_registry(&app);
    if registry.list.len() <= 1 {
        return Err("company-last".into());
    }
    registry.list.retain(|c| c.id != id);
    crate::companies::store_registry(&app, &registry)?;
    Ok(())
}

#[tauri::command]
pub fn data_path(st: State<'_, AppState>) -> String {
    st.db_path.lock().unwrap().to_string_lossy().to_string()
}

/// Hinweis für die Oberfläche, falls beim Laden eine unlesbare Datenbank
/// zur Seite gelegt wurde (Pfad der Quarantäne-Datei). Einmalig.
#[tauri::command]
pub fn startup_notice(st: State<'_, AppState>) -> Option<String> {
    st.quarantined
        .lock()
        .unwrap()
        .take()
        .map(|p| p.to_string_lossy().to_string())
}

/// Write binary data (base64) to a path the user picked via the save dialog.
/// Nur die Exportformate der App sind erlaubt — der Command ist kein
/// allgemeiner Datei-Schreiber für beliebige Pfade/Endungen.
#[tauri::command]
pub fn write_file(path: String, data_base64: String) -> Result<(), String> {
    let path = checked_path(&path, &["pdf", "xml", "json", "csv"])?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data_base64)
        .map_err(|e| e.to_string())?;
    invoices_core::write_atomic(&path, &bytes).map_err(|e| e.to_string())
}

/// Logos größer als das sind kein Briefkopf mehr.
const MAX_LOGO_BYTES: u64 = 10 * 1024 * 1024;
/// Obergrenze für eine Backup-Datei (schützt vor versehentlich gewählten
/// Riesendateien).
const MAX_BACKUP_BYTES: u64 = 512 * 1024 * 1024;

/// The configured logo as a data URL (for PDF embedding) — None if unset.
/// Liest nur echte PNG/JPEG-Dateien (Magic Bytes), keine beliebigen Dateien.
#[tauri::command]
pub fn get_logo(st: State<'_, AppState>) -> Option<String> {
    let path = st.settings.lock().unwrap().logo_path.trim().to_string();
    if path.is_empty() {
        return None;
    }
    let meta = std::fs::metadata(&path).ok()?;
    if !meta.is_file() || meta.len() > MAX_LOGO_BYTES {
        return None;
    }
    let bytes = std::fs::read(&path).ok()?;
    let mime = image_mime(&bytes)?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
    Some(format!("data:{mime};base64,{b64}"))
}

/// Bildtyp anhand der Magic Bytes — nur PNG und JPEG.
pub(crate) fn image_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some("image/png")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else {
        None
    }
}

/// Prüft einen vom Speichern-Dialog gelieferten Zielpfad: absolut, mit
/// erlaubter Endung, kein Verzeichnis. Funktioniert mit Windows-
/// (Laufwerksbuchstabe, Backslash, UNC) wie Unix-Pfaden.
pub(crate) fn checked_path(path: &str, allowed_ext: &[&str]) -> Result<std::path::PathBuf, String> {
    let p = std::path::PathBuf::from(path);
    if path.trim().is_empty() || !p.is_absolute() {
        return Err("bad-path".into());
    }
    let ext = p
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if !allowed_ext.contains(&ext.as_str()) {
        return Err("bad-extension".into());
    }
    if p.is_dir() {
        return Err("bad-path".into());
    }
    Ok(p)
}

// --- e-invoice ---------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EInvoiceDto {
    pub xml: String,
    pub suggested_name: String,
}

/// EN-16931-XML (Factur-X/XRechnung) für einen festgeschriebenen Beleg.
#[tauri::command]
pub fn einvoice_xml(st: State<'_, AppState>, id: String) -> Result<EInvoiceDto, String> {
    let (doc, buyer) = {
        let db = st.db.lock().unwrap();
        let doc = db
            .docs
            .iter()
            .find(|d| d.id == id)
            .cloned()
            .ok_or("doc-not-found")?;
        let customer = db.customers.iter().find(|c| c.id == doc.customer_id);
        let buyer = invoices_core::einvoice::Buyer {
            reference: customer.map(|c| c.buyer_reference.clone()).unwrap_or_default(),
            country: customer.map(|c| c.country.clone()).unwrap_or_default(),
            vat_id: customer.map(|c| c.vat_id.clone()).unwrap_or_default(),
            email: customer.map(|c| c.email.clone()).unwrap_or_default(),
        };
        (doc, buyer)
    };
    if doc.status == DocStatus::Draft {
        return Err("doc-draft".into());
    }
    if !matches!(
        doc.kind,
        DocKind::Invoice | DocKind::CreditNote | DocKind::Cancellation
    ) {
        return Err("not-einvoiceable".into());
    }
    let settings = st.settings.lock().unwrap().clone();
    let seller = invoices_core::einvoice::Seller {
        name: settings.company_name.clone(),
        address: settings.company_address.clone(),
        email: settings.company_email.clone(),
        tax_number: settings.tax_number.clone(),
        vat_id: settings.vat_id.clone(),
        iban: settings.iban.clone(),
        bic: settings.bic.clone(),
        bank_name: settings.bank_name.clone(),
        country: settings.country_code.clone(),
        phone: settings.company_phone.clone(),
    };
    let xml = invoices_core::einvoice::einvoice_xml(&doc, &seller, &buyer);
    let base = doc.number.unwrap_or_else(|| "beleg".into());
    Ok(EInvoiceDto {
        xml,
        suggested_name: format!("{base}.xml"),
    })
}

// --- updates -----------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfoDto {
    pub version: String,
    pub notes: Option<String>,
    pub date: Option<String>,
}

#[tauri::command]
pub async fn check_update(app: AppHandle) -> Result<Option<UpdateInfoDto>, String> {
    use tauri_plugin_updater::UpdaterExt;
    let updater = app.updater().map_err(|e| e.to_string())?;
    match updater.check().await {
        Ok(Some(update)) => Ok(Some(UpdateInfoDto {
            version: update.version.clone(),
            notes: update.body.clone(),
            date: update.date.map(|d| d.to_string()),
        })),
        Ok(None) => Ok(None),
        Err(e) => Err(format!("Update-Prüfung fehlgeschlagen: {e}")),
    }
}

#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    use tauri_plugin_updater::UpdaterExt;
    let updater = app.updater().map_err(|e| e.to_string())?;
    let update = updater
        .check()
        .await
        .map_err(|e| format!("Update-Prüfung fehlgeschlagen: {e}"))?
        .ok_or("Kein Update verfügbar")?;
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| format!("Update fehlgeschlagen: {e}"))?;
    app.restart();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn abs(name: &str) -> String {
        std::env::temp_dir().join(name).to_string_lossy().to_string()
    }

    #[test]
    fn checked_path_allows_only_export_formats() {
        assert!(checked_path(&abs("RE-2026-0001.pdf"), &["pdf", "xml"]).is_ok());
        assert!(checked_path(&abs("RE-2026-0001.PDF"), &["pdf"]).is_ok());
        assert_eq!(checked_path(&abs("evil.sh"), &["pdf"]).unwrap_err(), "bad-extension");
        assert_eq!(checked_path(&abs("noext"), &["pdf"]).unwrap_err(), "bad-extension");
        assert_eq!(checked_path("relative.pdf", &["pdf"]).unwrap_err(), "bad-path");
        assert_eq!(checked_path("", &["pdf"]).unwrap_err(), "bad-path");
    }

    #[cfg(windows)]
    #[test]
    fn checked_path_accepts_windows_paths() {
        assert!(checked_path(r"C:\Users\x\Documents\RE-1.pdf", &["pdf"]).is_ok());
        assert!(checked_path(r"\\nas\share\backup.json", &["json"]).is_ok());
        assert_eq!(checked_path(r"C:\x\a.exe", &["pdf"]).unwrap_err(), "bad-extension");
    }

    #[test]
    fn image_mime_checks_magic_bytes() {
        assert_eq!(image_mime(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0]), Some("image/png"));
        assert_eq!(image_mime(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("image/jpeg"));
        assert_eq!(image_mime(b"ssh-rsa AAAA"), None);
        assert_eq!(image_mime(&[]), None);
    }

    #[test]
    fn company_ids_cannot_escape_the_data_dir() {
        use crate::companies::valid_id;
        assert!(valid_id("3f2b8c1e-1d2a-4c7b-9f00-0123456789ab"));
        assert!(valid_id("default"));
        assert!(!valid_id(""));
        assert!(!valid_id("../../etc"));
        assert!(!valid_id(r"..\..\Windows"));
        assert!(!valid_id("C:"));
        assert!(!valid_id("a/b"));
        assert!(!valid_id(&"a".repeat(65)));
    }

    #[test]
    fn empty_prefixes_fall_back_to_defaults() {
        let mut s = Settings::default();
        s.quote_prefix = "  ".into();
        s.invoice_prefix = " R ".into();
        assert_eq!(prefix_for(&s, DocKind::Quote), "AN");
        assert_eq!(prefix_for(&s, DocKind::Invoice), "R");
        assert_eq!(prefix_for(&s, DocKind::DeliveryNote), "LS");
    }
}
