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
        match invoices_core::try_load_db(&path) {
            Ok(db) => *st.db.lock().unwrap() = db,
            // noch nie gespeichert (Erststart) → Platten-Stand ist „leer“
            Err(invoices_core::LoadError::Missing) => {
                *st.db.lock().unwrap() = invoices_core::Db::default()
            }
            // unlesbar: Speicher-Stand behalten, nichts überschreiben
            Err(invoices_core::LoadError::Corrupt(_)) => {}
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
/// Einstellungen) als eine JSON-Datei. `async`: läuft nicht auf dem
/// UI-Thread — ein langsames Netz-/UNC-Ziel friert das Fenster nicht ein.
#[tauri::command(async)]
pub fn export_backup(st: State<'_, AppState>, path: String) -> Result<(), String> {
    let backup = Backup {
        app: "invoices".into(),
        backup_version: 1,
        exported_at: chrono::Local::now().to_rfc3339(),
        db: st.db.lock().unwrap().clone(),
        settings: st.settings.lock().unwrap().clone(),
    };
    let path = checked_path(&path, &["json"])?;
    let json = serde_json::to_string_pretty(&backup).map_err(|e| format!("write-failed: {e}"))?;
    invoices_core::write_atomic(&path, json.as_bytes()).map_err(|e| format!("write-failed: {e}"))
}

/// Backup-Datei lesen und prüfen. Akzeptiert ein UTF-8-BOM (Datei mit
/// dem Windows-Editor geöffnet und gespeichert).
fn parse_backup(raw: &[u8]) -> Result<Backup, String> {
    let raw = raw.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(raw);
    let backup: Backup = serde_json::from_slice(raw).map_err(|_| "backup-invalid".to_string())?;
    if backup.app != "invoices" || backup.backup_version == 0 || backup.backup_version > 1 {
        return Err("backup-invalid".into());
    }
    Ok(backup)
}

/// Backup wiederherstellen — ersetzt ALLE Daten und Einstellungen.
/// `async`: großes Backup lesen/parsen nicht auf dem UI-Thread.
#[tauri::command(async)]
pub fn import_backup(app: AppHandle, st: State<'_, AppState>, path: String) -> Result<(), String> {
    let path = std::path::PathBuf::from(&path);
    let meta = std::fs::metadata(&path).map_err(|e| format!("read-failed: {e}"))?;
    if !meta.is_file() || meta.len() > MAX_BACKUP_BYTES {
        return Err("backup-invalid".into());
    }
    let raw = std::fs::read(&path).map_err(|e| format!("read-failed: {e}"))?;
    let backup = parse_backup(&raw)?;
    let active = st.active_company.lock().unwrap().clone();
    restore_into(&st, backup, |s| settings::store_for(&app, &active, s))?;
    let _ = app.emit("db-changed", ());
    Ok(())
}

/// Kern der Wiederherstellung (testbar ohne AppHandle): Sicherheitskopie,
/// dann Datenbank, dann Einstellungen. Scheitert die Sicherheitskopie,
/// bleibt alles unverändert.
fn restore_into(
    st: &AppState,
    backup: Backup,
    store_settings: impl FnOnce(&Settings) -> Result<(), String>,
) -> Result<(), String> {
    // Sicherheitsnetz: aktuellen Stand vor dem Ersetzen neben die
    // Datenbank legen — eine versehentliche Wiederherstellung ist so
    // umkehrbar. Klappt das nicht, wird NICHT wiederhergestellt.
    let db_path = st.db_path.lock().unwrap().clone();
    if db_path.exists() {
        let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
        std::fs::copy(&db_path, db_path.with_file_name(format!("db.json.before-restore-{stamp}")))
            .map_err(|e| format!("restore-safety-failed: {e}"))?;
    }
    // Erst die Datenbank auf die Platte, dann die Einstellungen — und den
    // DB-Lock dabei halten, damit kein parallel laufender Befehl den alten
    // Speicher-Stand über die wiederhergestellte Datei schreibt.
    {
        let mut db = st.db.lock().unwrap();
        invoices_core::save_db(&backup.db, &db_path).map_err(|e| format!("save-failed: {e}"))?;
        *db = backup.db;
    }
    store_settings(&backup.settings)?;
    *st.settings.lock().unwrap() = backup.settings;
    Ok(())
}

/// Automatisches Backup beim Start (settings.auto_backup): schreibt in den
/// Backup-Ordner und behält die letzten 10 Stände. Ein Fehler (Ordner
/// nicht erreichbar, Platte voll) wird gemeldet, nicht verschluckt.
pub fn auto_backup(st: &AppState) -> Result<(), String> {
    let settings = st.settings.lock().unwrap().clone();
    if !settings.auto_backup || settings.backup_dir.trim().is_empty() {
        return Ok(());
    }
    let dir = backup_dir_path(&settings.backup_dir)
        .ok_or_else(|| "auto-backup-failed: bad-path".to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("auto-backup-failed: {e}"))?;
    let backup = Backup {
        app: "invoices".into(),
        backup_version: 1,
        exported_at: chrono::Local::now().to_rfc3339(),
        db: st.db.lock().unwrap().clone(),
        settings,
    };
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let json = serde_json::to_string_pretty(&backup).map_err(|e| format!("auto-backup-failed: {e}"))?;
    invoices_core::write_atomic(&dir.join(format!("invoices-backup-{stamp}.json")), json.as_bytes())
        .map_err(|e| format!("auto-backup-failed: {e}"))?;
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
    Ok(())
}

/// Backup-Ordner aus dem (frei editierbaren) Einstellungsfeld: „D:“ ist
/// unter Windows laufwerks-relativ (aktuelles Verzeichnis auf D:) — gemeint
/// ist die Wurzel „D:\“. Relative Pfade werden abgelehnt.
pub(crate) fn backup_dir_path(raw: &str) -> Option<std::path::PathBuf> {
    let raw = raw.trim();
    let b = raw.as_bytes();
    let fixed = if b.len() == 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
        format!("{raw}\\")
    } else {
        raw.to_string()
    };
    let p = std::path::PathBuf::from(fixed);
    p.is_absolute().then_some(p)
}

/// Fehler des automatischen Backups beim Start (einmalig abholen) — die
/// Oberfläche bekommt ihn zusätzlich als Event „backup-failed“, falls sie
/// schon lauscht.
#[tauri::command]
pub fn backup_notice(st: State<'_, AppState>) -> Option<String> {
    st.backup_error.lock().unwrap().take()
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
    // Ordnernamen: APFS/NTFS unterscheiden Groß-/Kleinschreibung nicht
    if registry.list.iter().any(|c| c.id.eq_ignore_ascii_case(&id)) {
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
/// `async`: läuft nicht auf dem UI-Thread (Netzlaufwerk, gesperrte Datei
/// mit Wiederholversuchen).
#[tauri::command(async)]
pub fn write_file(path: String, data_base64: String) -> Result<(), String> {
    let path = checked_path(&path, &["pdf", "xml", "json"])?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data_base64)
        .map_err(|_| "bad-data".to_string())?;
    invoices_core::write_atomic(&path, &bytes).map_err(|e| format!("write-failed: {e}"))
}

/// Logos größer als das sind kein Briefkopf mehr.
const MAX_LOGO_BYTES: u64 = 10 * 1024 * 1024;
/// Obergrenze für eine Backup-Datei (schützt vor versehentlich gewählten
/// Riesendateien).
const MAX_BACKUP_BYTES: u64 = 512 * 1024 * 1024;

/// The configured logo as a data URL (for PDF embedding) — None if unset.
/// Liest nur echte PNG/JPEG-Dateien (Magic Bytes), keine beliebigen Dateien.
#[tauri::command(async)]
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
    if is_reserved_name(&p) {
        return Err("bad-name".into());
    }
    if p.is_dir() {
        return Err("bad-path".into());
    }
    Ok(p)
}

/// Unter Windows reservierte Gerätenamen (CON, NUL, COM1 …) — auch mit
/// Endung („CON.pdf“) und unabhängig von Groß-/Kleinschreibung. Gilt auf
/// allen Systemen, damit ein Export nach Windows/SMB nicht scheitert.
pub(crate) fn is_reserved_name(p: &std::path::Path) -> bool {
    let Some(name) = p.file_name().map(|n| n.to_string_lossy().to_string()) else {
        return false;
    };
    let stem = name.split('.').next().unwrap_or("").trim_end().to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.len() == 4
            && stem.as_bytes()[3].is_ascii_digit())
}

/// Dateiname aus einer Belegnummer: Pfadtrenner und unter Windows
/// verbotene Zeichen werden „-“ (Spiegel von `safeFileName` im Frontend).
pub(crate) fn safe_file_stem(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if c.is_control() || r#"\/:*?"<>|"#.contains(c) { '-' } else { c })
        .collect();
    let cleaned = cleaned.trim().trim_end_matches(['.', ' ']).to_string();
    if cleaned.is_empty() { "beleg".into() } else { cleaned }
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
    let base = safe_file_stem(&doc.number.unwrap_or_default());
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
    let updater = app.updater().map_err(|e| format!("update-check-failed: {e}"))?;
    match updater.check().await {
        Ok(Some(update)) => Ok(Some(UpdateInfoDto {
            version: update.version.clone(),
            notes: update.body.clone(),
            date: update.date.map(|d| d.to_string()),
        })),
        Ok(None) => Ok(None),
        // Fehlercode statt Klartext — die Oberfläche übersetzt (DE/EN)
        Err(e) => Err(format!("update-check-failed: {e}")),
    }
}

#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    use std::sync::atomic::{AtomicBool, Ordering};
    use tauri_plugin_updater::UpdaterExt;
    // Banner und Einstellungen können beide „Installieren“ auslösen —
    // nie zwei Downloads/Installationen gleichzeitig
    static BUSY: AtomicBool = AtomicBool::new(false);
    if BUSY.swap(true, Ordering::SeqCst) {
        return Err("update-busy".into());
    }
    struct Release;
    impl Drop for Release {
        fn drop(&mut self) {
            BUSY.store(false, Ordering::SeqCst);
        }
    }
    let _release = Release;
    let updater = app.updater().map_err(|e| format!("update-check-failed: {e}"))?;
    let update = updater
        .check()
        .await
        .map_err(|e| format!("update-check-failed: {e}"))?
        .ok_or("update-none")?;
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| format!("update-failed: {e}"))?;
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

    #[test]
    fn reserved_windows_names_are_rejected() {
        for bad in ["CON.pdf", "con.pdf", "NUL.xml", "Com1.pdf", "lpt9.json", "AUX.tar.pdf", "CON .pdf"] {
            assert_eq!(checked_path(&abs(bad), &["pdf", "xml", "json"]).unwrap_err(), "bad-name", "{bad}");
        }
        for ok in ["CONTRACT.pdf", "COM10.pdf", "RE-CON-1.pdf", "Rechnung Müller.pdf", "COMX.pdf"] {
            assert!(checked_path(&abs(ok), &["pdf"]).is_ok(), "{ok}");
        }
    }

    #[test]
    fn suggested_names_are_safe() {
        assert_eq!(safe_file_stem("RE/2026/0001"), "RE-2026-0001");
        assert_eq!(safe_file_stem("A:B*C?D\"<E>|F"), "A-B-C-D--E--F");
        assert_eq!(safe_file_stem("RE-1. "), "RE-1");
        assert_eq!(safe_file_stem(""), "beleg");
        assert_eq!(safe_file_stem("Ä\u{7}Ö"), "Ä-Ö");
    }

    fn backup_json() -> String {
        let b = Backup {
            app: "invoices".into(),
            backup_version: 1,
            exported_at: "x".into(),
            db: invoices_core::Db::default(),
            settings: Settings::default(),
        };
        serde_json::to_string(&b).unwrap()
    }

    #[test]
    fn backup_with_utf8_bom_is_accepted() {
        let json = backup_json();
        assert!(parse_backup(json.as_bytes()).is_ok());
        let mut bom = vec![0xEF, 0xBB, 0xBF];
        bom.extend_from_slice(json.as_bytes());
        assert!(parse_backup(&bom).is_ok());
        assert_eq!(parse_backup(b"{}").err().as_deref(), Some("backup-invalid"));
        assert_eq!(
            parse_backup(json.replace("\"invoices\"", "\"other\"").as_bytes()).err().as_deref(),
            Some("backup-invalid")
        );
    }

    fn test_state(dir: &std::path::Path) -> AppState {
        AppState {
            db: std::sync::Mutex::new(invoices_core::Db::default()),
            settings: std::sync::Mutex::new(Settings::default()),
            db_path: std::sync::Mutex::new(dir.join("db.json")),
            active_company: std::sync::Mutex::new("default".into()),
            quarantined: std::sync::Mutex::new(None),
            backup_error: std::sync::Mutex::new(None),
        }
    }

    #[test]
    fn restore_writes_db_and_settings_and_keeps_a_safety_copy() {
        let dir = std::env::temp_dir().join(format!("invoices-rs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let st = test_state(&dir);
        st.persist().unwrap();
        let mut backup = parse_backup(backup_json().as_bytes()).unwrap();
        backup.db.customers.push(invoices_core::Customer { id: "c1".into(), name: "Müller".into(), ..Default::default() });
        backup.settings.company_name = "Restored GmbH".into();
        let mut stored = None;
        restore_into(&st, backup, |s| {
            stored = Some(s.company_name.clone());
            Ok(())
        })
        .unwrap();
        assert_eq!(stored.as_deref(), Some("Restored GmbH"));
        assert_eq!(st.db.lock().unwrap().customers.len(), 1);
        assert_eq!(st.settings.lock().unwrap().company_name, "Restored GmbH");
        let on_disk = invoices_core::try_load_db(&dir.join("db.json")).ok().unwrap();
        assert_eq!(on_disk.customers.len(), 1);
        let copies = std::fs::read_dir(&dir)
            .unwrap()
            .filter(|e| e.as_ref().unwrap().file_name().to_string_lossy().starts_with("db.json.before-restore-"))
            .count();
        assert_eq!(copies, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn restore_aborts_without_safety_copy() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("invoices-rs2-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let st = test_state(&dir);
        st.persist().unwrap();
        // Ordner schreibgeschützt → keine Sicherheitskopie möglich
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
        let mut backup = parse_backup(backup_json().as_bytes()).unwrap();
        backup.settings.company_name = "X".into();
        let res = restore_into(&st, backup, |_| panic!("settings must not be stored"));
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        if let Err(e) = res {
            assert!(e.starts_with("restore-safety-failed"), "{e}");
            assert_eq!(st.settings.lock().unwrap().company_name, "");
        } else {
            // als root laufen Rechte ins Leere — dann ist nichts zu prüfen
            eprintln!("skipped: permissions not enforced");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn relative_backup_dirs_are_rejected() {
        assert_eq!(backup_dir_path(""), None);
        assert_eq!(backup_dir_path("backups"), None);
        #[cfg(unix)]
        {
            assert_eq!(backup_dir_path("D:"), None); // auf Unix kein Laufwerk
            assert_eq!(backup_dir_path(" /tmp/x "), Some(std::path::PathBuf::from("/tmp/x")));
        }
    }

    #[test]
    fn auto_backup_reports_unreachable_folder() {
        // Backup-Ordner unter einer Datei → create_dir_all scheitert
        let base = std::env::temp_dir().join(format!("invoices-ab-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&base);
        let file = base.join("not-a-dir");
        std::fs::write(&file, b"x").unwrap();
        let mut s = Settings::default();
        s.auto_backup = true;
        s.backup_dir = file.join("sub").to_string_lossy().to_string();
        let st = AppState {
            db: std::sync::Mutex::new(invoices_core::Db::default()),
            settings: std::sync::Mutex::new(s),
            db_path: std::sync::Mutex::new(base.join("db.json")),
            active_company: std::sync::Mutex::new("default".into()),
            quarantined: std::sync::Mutex::new(None),
            backup_error: std::sync::Mutex::new(None),
        };
        assert!(auto_backup(&st).unwrap_err().starts_with("auto-backup-failed"));
        // funktionierender Ordner (mit Umlaut und Leerzeichen): Datei entsteht
        let good = base.join("Sicherung Müller");
        st.settings.lock().unwrap().backup_dir = good.to_string_lossy().to_string();
        auto_backup(&st).unwrap();
        let n = std::fs::read_dir(&good).unwrap().count();
        assert_eq!(n, 1);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[cfg(windows)]
    #[test]
    fn checked_path_accepts_windows_paths() {
        assert!(checked_path(r"C:\Users\x\Documents\RE-1.pdf", &["pdf"]).is_ok());
        assert!(checked_path(r"\\nas\share\backup.json", &["json"]).is_ok());
        assert_eq!(checked_path(r"C:\x\a.exe", &["pdf"]).unwrap_err(), "bad-extension");
        assert!(checked_path(r"D:\RE-2026-0001.pdf", &["pdf"]).is_ok());
        assert_eq!(checked_path(r"C:RE-1.pdf", &["pdf"]).unwrap_err(), "bad-path");
        assert_eq!(checked_path(r"C:\Users\x\CON.pdf", &["pdf"]).unwrap_err(), "bad-name");
        assert_eq!(backup_dir_path("D:"), Some(std::path::PathBuf::from(r"D:\")));
        assert!(backup_dir_path(r"\\nas\share\backups").is_some());
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
    fn failed_first_save_rolls_back_to_empty_db() {
        // Ordner existiert nicht → Speichern scheitert; da noch nie etwas
        // auf der Platte lag, muss der Speicher wieder leer sein (keine
        // vergebene, ungespeicherte Nummer).
        let missing_dir = std::env::temp_dir()
            .join(format!("invoices-no-such-dir-{}", std::process::id()));
        let mut db = invoices_core::Db::default();
        db.customers.push(Customer { id: "c1".into(), name: "ACME".into(), ..Default::default() });
        let st = AppState {
            db: std::sync::Mutex::new(db),
            settings: std::sync::Mutex::new(Settings::default()),
            db_path: std::sync::Mutex::new(missing_dir.join("db.json")),
            active_company: std::sync::Mutex::new("default".into()),
            quarantined: std::sync::Mutex::new(None),
            backup_error: std::sync::Mutex::new(None),
        };
        assert!(commit(&st).unwrap_err().starts_with("save-failed"));
        assert!(st.db.lock().unwrap().customers.is_empty());
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
