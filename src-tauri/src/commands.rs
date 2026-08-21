use crate::settings::{self, Settings};
use crate::state::AppState;
use base64::Engine;
use invoices_core::{
    doc_matches, next_number, Customer, Doc, DocKind, DocStatus, NumberFormat, Product, Template,
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

// --- customers ---------------------------------------------------------

#[tauri::command]
pub fn list_customers(st: State<'_, AppState>) -> Vec<Customer> {
    let db = st.db.lock().unwrap();
    let mut out = db.customers.clone();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

#[tauri::command]
pub fn upsert_customer(app: AppHandle, st: State<'_, AppState>, customer: Customer) {
    {
        let mut db = st.db.lock().unwrap();
        if let Some(existing) = db.customers.iter_mut().find(|c| c.id == customer.id) {
            *existing = customer;
        } else {
            db.customers.push(customer);
        }
    }
    st.persist();
    let _ = app.emit("db-changed", ());
}

#[tauri::command]
pub fn delete_customer(app: AppHandle, st: State<'_, AppState>, id: String) -> Result<(), String> {
    {
        let mut db = st.db.lock().unwrap();
        // finalized documents keep their snapshot, so deleting is safe —
        // but block it while drafts still reference the customer.
        let in_draft = db
            .docs
            .iter()
            .any(|d| d.customer_id == id && d.status == DocStatus::Draft);
        if in_draft {
            return Err("customer-in-draft".into());
        }
        db.customers.retain(|c| c.id != id);
    }
    st.persist();
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
pub fn upsert_product(app: AppHandle, st: State<'_, AppState>, product: Product) {
    {
        let mut db = st.db.lock().unwrap();
        if let Some(existing) = db.products.iter_mut().find(|p| p.id == product.id) {
            *existing = product;
        } else {
            db.products.push(product);
        }
    }
    st.persist();
    let _ = app.emit("db-changed", ());
}

#[tauri::command]
pub fn delete_product(app: AppHandle, st: State<'_, AppState>, id: String) {
    {
        let mut db = st.db.lock().unwrap();
        db.products.retain(|p| p.id != id);
    }
    st.persist();
    let _ = app.emit("db-changed", ());
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
        if let Some(existing) = db.docs.iter_mut().find(|d| d.id == doc.id) {
            if existing.status != DocStatus::Draft {
                return Err("doc-finalized".into());
            }
            *existing = doc;
        } else {
            db.docs.push(doc);
        }
    }
    st.persist();
    let _ = app.emit("db-changed", ());
    Ok(())
}

/// Assign a sequential number and set the document live (status Open).
#[tauri::command]
pub fn finalize_doc(app: AppHandle, st: State<'_, AppState>, id: String) -> Result<Doc, String> {
    let finalized = {
        let settings = st.settings.lock().unwrap().clone();
        let mut db = st.db.lock().unwrap();
        let idx = db
            .docs
            .iter()
            .position(|d| d.id == id)
            .ok_or("doc-not-found")?;
        if db.docs[idx].status != DocStatus::Draft {
            return Err("doc-finalized".into());
        }
        if db.docs[idx].items.is_empty() {
            return Err("doc-empty".into());
        }
        let prefix = match db.docs[idx].kind {
            DocKind::Invoice => settings.invoice_prefix.clone(),
            DocKind::CreditNote => settings.credit_prefix.clone(),
            DocKind::Cancellation => settings.cancel_prefix.clone(),
            DocKind::Quote => settings.quote_prefix.clone(),
            DocKind::OrderConfirmation => settings.order_prefix.clone(),
            DocKind::DeliveryNote => settings.delivery_prefix.clone(),
        };
        let doc_year = db.docs[idx].date.get(..4).and_then(|y| y.parse::<i32>().ok());
        let fmt = number_format(&settings, doc_year);
        let number = next_number(&mut db, &prefix, fmt);
        let doc = &mut db.docs[idx];
        doc.number = Some(number);
        doc.status = DocStatus::Open;
        doc.clone()
    };
    st.persist();
    let _ = app.emit("db-changed", ());
    Ok(finalized)
}

#[tauri::command]
pub fn set_paid(app: AppHandle, st: State<'_, AppState>, id: String, paid: bool) -> Result<(), String> {
    {
        let mut db = st.db.lock().unwrap();
        let doc = db
            .docs
            .iter_mut()
            .find(|d| d.id == id)
            .ok_or("doc-not-found")?;
        match (doc.status, paid) {
            (DocStatus::Open, true) => {
                doc.status = DocStatus::Paid;
                doc.paid_at = Some(today().format("%Y-%m-%d").to_string());
            }
            (DocStatus::Paid, false) => {
                doc.status = DocStatus::Open;
                doc.paid_at = None;
            }
            _ => return Err("bad-status".into()),
        }
    }
    st.persist();
    let _ = app.emit("db-changed", ());
    Ok(())
}

/// Cancel a finalized invoice: creates a linked cancellation document with
/// negated positions (as a draft, so it can be reviewed before finalizing)
/// and marks the original as cancelled.
#[tauri::command]
pub fn cancel_doc(app: AppHandle, st: State<'_, AppState>, id: String, new_id: String) -> Result<Doc, String> {
    let storno = {
        let settings = st.settings.lock().unwrap().clone();
        let mut db = st.db.lock().unwrap();
        let original = db
            .docs
            .iter()
            .find(|d| d.id == id)
            .cloned()
            .ok_or("doc-not-found")?;
        if original.status != DocStatus::Open && original.status != DocStatus::Paid {
            return Err("bad-status".into());
        }
        if original.kind != DocKind::Invoice {
            return Err("bad-kind".into());
        }
        let mut items = original.items.clone();
        for item in &mut items {
            item.unit_price_cents = -item.unit_price_cents;
        }
        let date = today().format("%Y-%m-%d").to_string();
        let mut storno = Doc {
            id: new_id,
            kind: DocKind::Cancellation,
            number: None,
            customer_id: original.customer_id.clone(),
            customer_name: original.customer_name.clone(),
            customer_address: original.customer_address.clone(),
            date: date.clone(),
            due_date: date.clone(),
            items,
            status: DocStatus::Open,
            small_business: original.small_business,
            intro: String::new(),
            notes: String::new(),
            paid_at: None,
            related_id: Some(original.id.clone()),
            created_at: chrono::Local::now().to_rfc3339(),
        };
        // Stornos gehen sofort live — sie dokumentieren, sie fordern nicht.
        let fmt = number_format(&settings, None);
        let number = next_number(&mut db, &settings.cancel_prefix, fmt);
        storno.number = Some(number);
        if let Some(orig) = db.docs.iter_mut().find(|d| d.id == id) {
            orig.status = DocStatus::Cancelled;
        }
        db.docs.push(storno.clone());
        storno
    };
    st.persist();
    let _ = app.emit("db-changed", ());
    Ok(storno)
}

#[tauri::command]
pub fn delete_doc(app: AppHandle, st: State<'_, AppState>, id: String) -> Result<(), String> {
    {
        let mut db = st.db.lock().unwrap();
        let doc = db.docs.iter().find(|d| d.id == id).ok_or("doc-not-found")?;
        // only drafts may vanish — finalized documents stay forever
        if doc.status != DocStatus::Draft {
            return Err("doc-finalized".into());
        }
        db.docs.retain(|d| d.id != id);
    }
    st.persist();
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
pub fn upsert_template(app: AppHandle, st: State<'_, AppState>, template: Template) {
    {
        let mut db = st.db.lock().unwrap();
        if let Some(existing) = db.templates.iter_mut().find(|t| t.id == template.id) {
            *existing = template;
        } else {
            db.templates.push(template);
        }
    }
    st.persist();
    let _ = app.emit("db-changed", ());
}

#[tauri::command]
pub fn delete_template(app: AppHandle, st: State<'_, AppState>, id: String) {
    {
        let mut db = st.db.lock().unwrap();
        db.templates.retain(|t| t.id != id);
    }
    st.persist();
    let _ = app.emit("db-changed", ());
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
    let json = serde_json::to_string_pretty(&backup).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

/// Backup wiederherstellen — ersetzt ALLE Daten und Einstellungen.
#[tauri::command]
pub fn import_backup(app: AppHandle, st: State<'_, AppState>, path: String) -> Result<(), String> {
    let raw = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let backup: Backup = serde_json::from_str(&raw).map_err(|_| "backup-invalid".to_string())?;
    if backup.app != "invoices" {
        return Err("backup-invalid".into());
    }
    *st.db.lock().unwrap() = backup.db;
    let active = st.active_company.lock().unwrap().clone();
    settings::store_for(&app, &active, &backup.settings);
    *st.settings.lock().unwrap() = backup.settings;
    st.persist();
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
        let _ = std::fs::write(dir.join(format!("invoices-backup-{stamp}.json")), json);
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
pub fn set_settings(app: AppHandle, st: State<'_, AppState>, new: Settings) {
    let active = st.active_company.lock().unwrap().clone();
    settings::store_for(&app, &active, &new);
    // Registry-Label mit dem Firmennamen synchron halten
    let mut registry = crate::companies::load_registry(&app);
    if let Some(entry) = registry.list.iter_mut().find(|c| c.id == active) {
        entry.label = new.company_name.clone();
        crate::companies::store_registry(&app, &registry);
    }
    *st.settings.lock().unwrap() = new;
    let _ = app.emit("db-changed", ());
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
    let mut registry = crate::companies::load_registry(&app);
    if registry.list.iter().any(|c| c.id == id) {
        return Err("company-exists".into());
    }
    registry.list.push(crate::companies::CompanyEntry {
        id: id.clone(),
        label: label.clone(),
    });
    crate::companies::store_registry(&app, &registry);
    // Startprofil: leere Firma mit Namen, frische Datenbank
    let mut fresh = Settings::default();
    fresh.company_name = label;
    settings::store_for(&app, &id, &fresh);
    switch_company(app, st, id)
}

/// Aktive Firma wechseln: aktuellen Stand sichern, Ziel-Profil laden.
#[tauri::command]
pub fn switch_company(app: AppHandle, st: State<'_, AppState>, id: String) -> Result<(), String> {
    let mut registry = crate::companies::load_registry(&app);
    if !registry.list.iter().any(|c| c.id == id) {
        return Err("company-not-found".into());
    }
    st.persist();
    let new_settings = settings::load_for(&app, &id);
    let new_db_path = crate::companies::db_path(&app, &id);
    let new_db = invoices_core::load_db(&new_db_path).unwrap_or_default();
    *st.settings.lock().unwrap() = new_settings;
    *st.db.lock().unwrap() = new_db;
    *st.db_path.lock().unwrap() = new_db_path;
    *st.active_company.lock().unwrap() = id.clone();
    registry.active = id;
    crate::companies::store_registry(&app, &registry);
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
    crate::companies::store_registry(&app, &registry);
    Ok(())
}

#[tauri::command]
pub fn data_path(st: State<'_, AppState>) -> String {
    st.db_path.lock().unwrap().to_string_lossy().to_string()
}

/// Write binary data (base64) to a path the user picked via the save dialog.
#[tauri::command]
pub fn write_file(path: String, data_base64: String) -> Result<(), String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data_base64)
        .map_err(|e| e.to_string())?;
    std::fs::write(&path, bytes).map_err(|e| e.to_string())
}

/// The configured logo as a data URL (for PDF embedding) — None if unset.
#[tauri::command]
pub fn get_logo(st: State<'_, AppState>) -> Option<String> {
    let path = st.settings.lock().unwrap().logo_path.trim().to_string();
    if path.is_empty() {
        return None;
    }
    let bytes = std::fs::read(&path).ok()?;
    let mime = if path.to_lowercase().ends_with(".jpg") || path.to_lowercase().ends_with(".jpeg") {
        "image/jpeg"
    } else {
        "image/png"
    };
    let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
    Some(format!("data:{mime};base64,{b64}"))
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
    let (doc, buyer_reference, buyer_country) = {
        let db = st.db.lock().unwrap();
        let doc = db
            .docs
            .iter()
            .find(|d| d.id == id)
            .cloned()
            .ok_or("doc-not-found")?;
        let customer = db.customers.iter().find(|c| c.id == doc.customer_id);
        (
            doc,
            customer.map(|c| c.buyer_reference.clone()).unwrap_or_default(),
            customer.map(|c| c.country.clone()).unwrap_or_default(),
        )
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
    };
    let xml = invoices_core::einvoice::einvoice_xml(&doc, &seller, &buyer_reference, &buyer_country);
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
