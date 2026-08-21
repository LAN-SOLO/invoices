//! App settings, stored as JSON in the OS config directory.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// "de" | "en"
    pub language: String,
    // --- letterhead / company data (printed on every document) ---
    pub company_name: String,
    /// Multiline postal address.
    pub company_address: String,
    pub company_email: String,
    pub company_phone: String,
    /// Steuernummer.
    pub tax_number: String,
    /// USt-IdNr.
    pub vat_id: String,
    pub bank_name: String,
    pub iban: String,
    pub bic: String,
    /// Absolute path to a logo image (PNG/JPG), printed top right.
    pub logo_path: String,
    // --- invoicing defaults ---
    /// Kleinunternehmerregelung nach §19 UStG — no VAT on documents.
    pub small_business: bool,
    /// Default VAT rate for new positions (0/7/19).
    pub default_vat_rate: u8,
    /// Payment terms in days (due date = date + terms).
    pub payment_terms_days: u32,
    /// Numbering prefixes per document kind.
    pub invoice_prefix: String,
    pub credit_prefix: String,
    pub cancel_prefix: String,
    pub quote_prefix: String,
    pub order_prefix: String,
    pub delivery_prefix: String,
    /// Nummernkreis-Layout: Jahr einbeziehen (Zähler pro Jahr) oder
    /// fortlaufend; Trennzeichen und Stellenzahl.
    pub number_include_year: bool,
    pub number_digits: u8,
    pub number_separator: String,
    /// Free-text footer line on PDFs (e.g. a thank-you note).
    pub pdf_footer: String,
    /// ISO-3166-Ländercode des eigenen Unternehmens (für die E-Rechnung).
    pub country_code: String,
    /// Standard-Einheit für neue Positionen/Artikel.
    pub default_unit: String,
    /// Standard-Einleitungstext für neue Rechnungen.
    pub default_intro: String,
    /// Darstellung: "dark" | "light" + Akzentfarbe.
    pub theme: String,
    pub accent: String,
    /// PDF-Layout: "classic" | "modern" | "compact" | "terminal".
    pub pdf_layout: String,
    /// PDF-Farbschema: Preset-ID plus vier Rollen-Farben (Hex) —
    /// c1 Titel/Überschriften, c2 Akzent/Band, c3 Tabellenkopf, c4 Zebra.
    pub pdf_scheme: String,
    pub pdf_c1: String,
    pub pdf_c2: String,
    pub pdf_c3: String,
    pub pdf_c4: String,
    /// Firmenname & Adresse im PDF-Kopf zeigen (aus = nur Logo).
    pub pdf_show_company_header: bool,
    /// Updates beim Start automatisch installieren (statt nur Hinweis).
    pub auto_update: bool,
    /// Sicherung: automatisches lokales Backup beim Start in diesen Ordner.
    pub auto_backup: bool,
    pub backup_dir: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            language: if sys_locale_is_german() { "de" } else { "en" }.into(),
            company_name: String::new(),
            company_address: String::new(),
            company_email: String::new(),
            company_phone: String::new(),
            tax_number: String::new(),
            vat_id: String::new(),
            bank_name: String::new(),
            iban: String::new(),
            bic: String::new(),
            logo_path: String::new(),
            small_business: false,
            default_vat_rate: 19,
            payment_terms_days: 14,
            invoice_prefix: "RE".into(),
            credit_prefix: "GS".into(),
            cancel_prefix: "ST".into(),
            quote_prefix: "AN".into(),
            order_prefix: "AB".into(),
            delivery_prefix: "LS".into(),
            number_include_year: true,
            number_digits: 4,
            number_separator: "-".into(),
            pdf_footer: String::new(),
            country_code: "DE".into(),
            default_unit: "Stk".into(),
            default_intro: String::new(),
            theme: "dark".into(),
            accent: "sky".into(),
            pdf_layout: "classic".into(),
            pdf_scheme: "ink".into(),
            pdf_c1: "#111827".into(),
            pdf_c2: "#374151".into(),
            pdf_c3: "#111827".into(),
            pdf_c4: "#f3f4f6".into(),
            pdf_show_company_header: true,
            auto_update: false,
            auto_backup: false,
            backup_dir: String::new(),
        }
    }
}

fn sys_locale_is_german() -> bool {
    std::env::var("LANG")
        .or_else(|_| std::env::var("LC_ALL"))
        .map(|l| l.to_lowercase().starts_with("de"))
        .unwrap_or(false)
}

pub fn load_for(app: &tauri::AppHandle, company_id: &str) -> Settings {
    std::fs::read_to_string(crate::companies::settings_path(app, company_id))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn store_for(app: &tauri::AppHandle, company_id: &str, settings: &Settings) {
    if let Ok(json) = serde_json::to_string_pretty(settings) {
        let _ = std::fs::write(crate::companies::settings_path(app, company_id), json);
    }
}
