use crate::settings::{self, Settings};
use crate::state::AppState;
use base64::Engine;
use invoices_core::{doc_matches, next_number, Customer, Doc, DocKind, DocStatus, Product};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

fn today() -> chrono::NaiveDate {
    chrono::Local::now().date_naive()
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
        };
        let year = db.docs[idx]
            .date
            .get(..4)
            .and_then(|y| y.parse::<i32>().ok())
            .unwrap_or_else(|| {
                use chrono::Datelike;
                today().year()
            });
        let number = next_number(&mut db, &prefix, year);
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
        let year = {
            use chrono::Datelike;
            today().year()
        };
        let number = next_number(&mut db, &settings.cancel_prefix, year);
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

// --- settings & files --------------------------------------------------

#[tauri::command]
pub fn get_settings(st: State<'_, AppState>) -> Settings {
    st.settings.lock().unwrap().clone()
}

#[tauri::command]
pub fn set_settings(app: AppHandle, st: State<'_, AppState>, new: Settings) {
    settings::store(&app, &new);
    *st.settings.lock().unwrap() = new;
    let _ = app.emit("db-changed", ());
}

#[tauri::command]
pub fn data_path(app: AppHandle) -> String {
    crate::state::db_path(&app).to_string_lossy().to_string()
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
