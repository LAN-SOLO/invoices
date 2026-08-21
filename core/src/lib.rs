//! invoices core: invoicing domain — customers, products, documents,
//! money math (cents), VAT breakdown and sequential numbering.
//!
//! Doctrine: money is integer cents, VAT is computed per rate on the
//! summed net base (kaufmännisch gerundet), and document numbers are
//! sequential per kind and year — once assigned, never reused.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

pub mod einvoice;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Customer {
    pub id: String,
    pub name: String,
    /// Multiline postal address (without the name).
    pub address: String,
    pub email: String,
    pub vat_id: String,
    pub notes: String,
    /// Leitweg-ID bzw. Käufer-Referenz (BT-10) für die E-Rechnung.
    pub buyer_reference: String,
    /// ISO-3166-Ländercode; leer = "DE".
    pub country: String,
    // Strukturierte Stammdaten (seit 0.3.0) — `address` bleibt als
    // zusammengesetzte Fassung für PDF-Snapshot und Alt-Daten erhalten.
    pub contact: String,
    pub street: String,
    pub postcode: String,
    pub city: String,
    pub phone: String,
    pub website: String,
    pub customer_number: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Product {
    pub id: String,
    pub name: String,
    pub description: String,
    /// e.g. "Stk", "h", "Tag", "pauschal"
    pub unit: String,
    /// Net unit price in cents.
    pub unit_price_cents: i64,
    /// 0, 7 or 19 (percent).
    pub vat_rate: u8,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LineItem {
    pub name: String,
    pub description: String,
    pub quantity: f64,
    pub unit: String,
    pub unit_price_cents: i64,
    pub vat_rate: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DocKind {
    Invoice,
    CreditNote,
    Cancellation,
    Quote,
    #[serde(rename = "orderconfirmation")]
    OrderConfirmation,
    #[serde(rename = "deliverynote")]
    DeliveryNote,
}

impl Default for DocKind {
    fn default() -> Self {
        DocKind::Invoice
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DocStatus {
    Draft,
    Open,
    Paid,
    Cancelled,
}

impl Default for DocStatus {
    fn default() -> Self {
        DocStatus::Draft
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Doc {
    pub id: String,
    pub kind: DocKind,
    /// Assigned on finalize — drafts have none.
    pub number: Option<String>,
    pub customer_id: String,
    /// Snapshot at creation time — the document stays stable even if
    /// the customer record changes later.
    pub customer_name: String,
    pub customer_address: String,
    /// ISO date "YYYY-MM-DD".
    pub date: String,
    pub due_date: String,
    pub items: Vec<LineItem>,
    pub status: DocStatus,
    pub small_business: bool,
    pub intro: String,
    pub notes: String,
    pub paid_at: Option<String>,
    /// Cancellations/credit notes reference their origin document.
    pub related_id: Option<String>,
    pub created_at: String,
}

/// Wiederverwendbare Beleg-Vorlage: Positionen + Texte, ohne Kunde.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Template {
    pub id: String,
    pub name: String,
    pub kind: DocKind,
    pub items: Vec<LineItem>,
    pub intro: String,
    pub notes: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Db {
    pub customers: Vec<Customer>,
    pub products: Vec<Product>,
    pub docs: Vec<Doc>,
    pub templates: Vec<Template>,
    /// Sequential counters keyed by "<prefix>-<year>", e.g. "RE-2026".
    pub counters: HashMap<String, u32>,
}

// --- money -------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VatLine {
    pub rate: u8,
    pub base_cents: i64,
    pub vat_cents: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Totals {
    pub net_cents: i64,
    pub vat: Vec<VatLine>,
    pub gross_cents: i64,
}

/// Net line total in cents: quantity × unit price, kaufmännisch gerundet.
pub fn line_total_cents(item: &LineItem) -> i64 {
    (item.quantity * item.unit_price_cents as f64).round() as i64
}

/// Totals for a document: net sum, VAT grouped per rate (computed on the
/// summed base per rate), gross. Small-business documents (§19 UStG)
/// carry no VAT at all.
pub fn totals(items: &[LineItem], small_business: bool) -> Totals {
    let mut net: i64 = 0;
    let mut bases: HashMap<u8, i64> = HashMap::new();
    for item in items {
        let line = line_total_cents(item);
        net += line;
        *bases.entry(item.vat_rate).or_insert(0) += line;
    }
    let mut vat: Vec<VatLine> = Vec::new();
    if !small_business {
        let mut rates: Vec<u8> = bases.keys().copied().filter(|r| *r > 0).collect();
        rates.sort_unstable();
        for rate in rates {
            let base = bases[&rate];
            let amount = (base as f64 * rate as f64 / 100.0).round() as i64;
            vat.push(VatLine {
                rate,
                base_cents: base,
                vat_cents: amount,
            });
        }
    }
    let vat_sum: i64 = vat.iter().map(|v| v.vat_cents).sum();
    Totals {
        net_cents: net,
        vat,
        gross_cents: net + vat_sum,
    }
}

// --- numbering ---------------------------------------------------------

/// Layout des Nummernkreises: mit Jahr (Zähler startet pro Jahr neu) oder
/// fortlaufend ohne Jahr; Trennzeichen und Stellenzahl sind konfigurierbar.
#[derive(Debug, Clone, Copy)]
pub struct NumberFormat<'a> {
    /// `Some(jahr)` = Jahr einbeziehen (aktuelles Jahr, Zähler pro Jahr),
    /// `None` = fortlaufender Zähler ohne Jahr.
    pub year: Option<i32>,
    /// Mindest-Stellenzahl des Zählers (mit Nullen aufgefüllt), 1..=8.
    pub digits: usize,
    /// Trennzeichen zwischen den Teilen, z. B. "-", "/" oder "".
    pub separator: &'a str,
}

/// Draws the next sequential number for `prefix`, formats it according to
/// `fmt` and advances the counter. Counter keys: "<prefix>-<year>" when a
/// year is included (compatible with pre-0.2.0 data), plain "<prefix>"
/// for continuous numbering.
pub fn next_number(db: &mut Db, prefix: &str, fmt: NumberFormat) -> String {
    let key = match fmt.year {
        Some(year) => format!("{prefix}-{year}"),
        None => prefix.to_string(),
    };
    let counter = db.counters.entry(key).or_insert(0);
    *counter += 1;
    let digits = fmt.digits.clamp(1, 8);
    let counter_str = format!("{:0width$}", counter, width = digits);
    match fmt.year {
        Some(year) => format!("{prefix}{sep}{year}{sep}{counter_str}", sep = fmt.separator),
        None => format!("{prefix}{sep}{counter_str}", sep = fmt.separator),
    }
}

// --- persistence -------------------------------------------------------

pub fn load_db(path: &Path) -> Option<Db> {
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn save_db(db: &Db, path: &Path) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(db)?;
    // write via temp file + rename so a crash never truncates the db
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, path)
}

// --- search ------------------------------------------------------------

/// Case-insensitive match over number, customer, item names and notes.
pub fn doc_matches(doc: &Doc, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let q = query.to_lowercase();
    doc.number
        .as_deref()
        .map(|n| n.to_lowercase().contains(&q))
        .unwrap_or(false)
        || doc.customer_name.to_lowercase().contains(&q)
        || doc.items.iter().any(|i| i.name.to_lowercase().contains(&q))
        || doc.notes.to_lowercase().contains(&q)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(qty: f64, price: i64, rate: u8) -> LineItem {
        LineItem {
            name: "x".into(),
            description: String::new(),
            quantity: qty,
            unit: "Stk".into(),
            unit_price_cents: price,
            vat_rate: rate,
        }
    }

    #[test]
    fn line_total_rounds_commercially() {
        // 3 × 3,33 € = 9,99 €
        assert_eq!(line_total_cents(&item(3.0, 333, 19)), 999);
        // 1,5 h × 90,00 € = 135,00 €
        assert_eq!(line_total_cents(&item(1.5, 9000, 19)), 13500);
        // 0,333 × 1,00 € = 0,33 €
        assert_eq!(line_total_cents(&item(0.333, 100, 19)), 33);
    }

    #[test]
    fn totals_mixed_rates() {
        let items = vec![item(1.0, 10000, 19), item(2.0, 500, 7), item(1.0, 1000, 0)];
        let t = totals(&items, false);
        assert_eq!(t.net_cents, 12000);
        assert_eq!(t.vat.len(), 2); // rate 0 never shows up as a VAT line
        assert_eq!(
            t.vat[0],
            VatLine { rate: 7, base_cents: 1000, vat_cents: 70 }
        );
        assert_eq!(
            t.vat[1],
            VatLine { rate: 19, base_cents: 10000, vat_cents: 1900 }
        );
        assert_eq!(t.gross_cents, 13970);
    }

    #[test]
    fn totals_vat_rounds_on_summed_base() {
        // two lines of 0,33 € net at 19 %: VAT on 0,66 € = 0,1254 → 0,13 €
        let items = vec![item(1.0, 33, 19), item(1.0, 33, 19)];
        let t = totals(&items, false);
        assert_eq!(t.net_cents, 66);
        assert_eq!(t.vat[0].vat_cents, 13);
        assert_eq!(t.gross_cents, 79);
    }

    #[test]
    fn totals_small_business_has_no_vat() {
        let items = vec![item(1.0, 10000, 19)];
        let t = totals(&items, true);
        assert!(t.vat.is_empty());
        assert_eq!(t.gross_cents, t.net_cents);
    }

    fn fmt_year(year: i32) -> NumberFormat<'static> {
        NumberFormat { year: Some(year), digits: 4, separator: "-" }
    }

    #[test]
    fn numbering_is_sequential_per_prefix_and_year() {
        let mut db = Db::default();
        assert_eq!(next_number(&mut db, "RE", fmt_year(2026)), "RE-2026-0001");
        assert_eq!(next_number(&mut db, "RE", fmt_year(2026)), "RE-2026-0002");
        assert_eq!(next_number(&mut db, "GS", fmt_year(2026)), "GS-2026-0001");
        assert_eq!(next_number(&mut db, "RE", fmt_year(2027)), "RE-2027-0001");
        // the old year's counter is untouched
        assert_eq!(next_number(&mut db, "RE", fmt_year(2026)), "RE-2026-0003");
    }

    #[test]
    fn numbering_layouts() {
        let mut db = Db::default();
        // fortlaufend ohne Jahr — eigener Zähler, unabhängig vom Jahres-Zähler
        let cont = NumberFormat { year: None, digits: 5, separator: "-" };
        assert_eq!(next_number(&mut db, "RE", cont), "RE-00001");
        assert_eq!(next_number(&mut db, "RE", fmt_year(2026)), "RE-2026-0001");
        assert_eq!(next_number(&mut db, "RE", cont), "RE-00002");
        // Trennzeichen-Varianten
        let slash = NumberFormat { year: Some(2026), digits: 3, separator: "/" };
        assert_eq!(next_number(&mut db, "GS", slash), "GS/2026/001");
        let none = NumberFormat { year: None, digits: 4, separator: "" };
        assert_eq!(next_number(&mut db, "ST", none), "ST0001");
    }

    #[test]
    fn numbering_counter_key_is_stable_across_layout_changes() {
        // Wechsel des Layouts (Stellen/Trennzeichen) darf den Zähler nicht
        // zurücksetzen — nur Jahr an/aus wechselt den Zähler-Schlüssel.
        let mut db = Db::default();
        next_number(&mut db, "RE", fmt_year(2026));
        let wide = NumberFormat { year: Some(2026), digits: 6, separator: "/" };
        assert_eq!(next_number(&mut db, "RE", wide), "RE/2026/000002");
    }

    #[test]
    fn doc_search_matches_number_customer_and_items() {
        let doc = Doc {
            number: Some("RE-2026-0001".into()),
            customer_name: "ACME GmbH".into(),
            items: vec![item(1.0, 100, 19)],
            ..Default::default()
        };
        assert!(doc_matches(&doc, ""));
        assert!(doc_matches(&doc, "re-2026"));
        assert!(doc_matches(&doc, "acme"));
        assert!(doc_matches(&doc, "0001"));
        assert!(!doc_matches(&doc, "nope"));
    }

    #[test]
    fn db_roundtrip() {
        let dir = std::env::temp_dir().join("invoices-core-test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("db.json");
        let mut db = Db::default();
        db.customers.push(Customer {
            id: "c1".into(),
            name: "ACME GmbH".into(),
            ..Default::default()
        });
        next_number(&mut db, "RE", fmt_year(2026));
        save_db(&db, &path).unwrap();
        let loaded = load_db(&path).unwrap();
        assert_eq!(loaded.customers.len(), 1);
        assert_eq!(loaded.counters.get("RE-2026"), Some(&1));
        let _ = std::fs::remove_file(&path);
    }
}
