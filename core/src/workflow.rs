//! Beleg-Workflow als reine Funktionen auf der Datenbank — Entwurf,
//! Festschreiben, Bezahlt-Status, Storno, Löschen. Die Tauri-Commands sind
//! nur noch dünne Hüllen darum (Sperren, Speichern, Events); die Regeln
//! selbst sind hier testbar.
//!
//! Doctrine (GoBD-Denke): Entwürfe sind frei, festgeschriebene Belege sind
//! unveränderlich und verschwinden nie — korrigiert wird per Storno.

use crate::{next_number, Db, Doc, DocKind, DocStatus, NumberFormat};

/// Fehlercodes — die Oberfläche übersetzt sie.
pub type WfResult<T> = Result<T, &'static str>;

/// Entwurf anlegen oder ändern. Über diesen Weg entstehen ausschließlich
/// Entwürfe: Nummer, Status und Zahlungsdatum setzt nur der Workflow selbst,
/// sonst ließe sich der Nummernkreis an der Festschreibung vorbei füllen.
pub fn upsert_draft(db: &mut Db, mut doc: Doc) -> WfResult<()> {
    if doc.id.trim().is_empty() {
        return Err("bad-id");
    }
    if doc.kind == DocKind::Cancellation {
        // Stornos entstehen ausschließlich über `cancel`
        return Err("bad-kind");
    }
    if doc.items.iter().any(|i| !i.quantity.is_finite()) {
        return Err("bad-quantity");
    }
    doc.status = DocStatus::Draft;
    doc.number = None;
    doc.paid_at = None;
    if let Some(existing) = db.docs.iter_mut().find(|d| d.id == doc.id) {
        if existing.status != DocStatus::Draft {
            return Err("doc-finalized");
        }
        // Erstellungszeitpunkt bleibt der des ersten Speicherns
        if !existing.created_at.is_empty() {
            doc.created_at = existing.created_at.clone();
        }
        *existing = doc;
    } else {
        db.docs.push(doc);
    }
    Ok(())
}

/// Jahr aus einem ISO-Datum "YYYY-MM-DD".
pub fn year_of(iso: &str) -> Option<i32> {
    iso.get(..4).and_then(|y| y.parse::<i32>().ok())
}

/// Gültiges ISO-Datum "YYYY-MM-DD" (inkl. Schaltjahr-Prüfung)?
pub fn valid_iso_date(iso: &str) -> bool {
    let b = iso.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    let num = |r: std::ops::Range<usize>| -> Option<u32> {
        let part = iso.get(r)?;
        if part.bytes().all(|c| c.is_ascii_digit()) { part.parse().ok() } else { None }
    };
    let (Some(y), Some(m), Some(d)) = (num(0..4), num(5..7), num(8..10)) else {
        return false;
    };
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let days = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    y >= 1900 && (1..=days).contains(&d)
}

/// Festschreiben: fortlaufende Nummer vergeben, Status "offen".
/// `fmt_for_year` bekommt das Jahr des Belegdatums (falls lesbar).
pub fn finalize<'a>(
    db: &mut Db,
    id: &str,
    prefix: &str,
    fmt_for_year: impl Fn(Option<i32>) -> NumberFormat<'a>,
) -> WfResult<Doc> {
    let idx = db.docs.iter().position(|d| d.id == id).ok_or("doc-not-found")?;
    if db.docs[idx].status != DocStatus::Draft {
        return Err("doc-finalized");
    }
    if db.docs[idx].items.is_empty() || db.docs[idx].items.iter().all(|i| i.name.trim().is_empty()) {
        return Err("doc-empty");
    }
    if db.docs[idx].customer_name.trim().is_empty() {
        return Err("doc-no-customer");
    }
    // Ohne gültiges Belegdatum keine Nummer: das Jahr im Nummernkreis und
    // das Rechnungsdatum der E-Rechnung hängen daran.
    {
        let d = &db.docs[idx];
        if !valid_iso_date(&d.date)
            || (!d.due_date.is_empty() && !valid_iso_date(&d.due_date))
            || (!d.service_date.is_empty() && !valid_iso_date(&d.service_date))
        {
            return Err("doc-bad-date");
        }
    }
    let fmt = fmt_for_year(year_of(&db.docs[idx].date));
    let number = next_number(db, prefix, fmt);
    let doc = &mut db.docs[idx];
    doc.number = Some(number);
    doc.status = DocStatus::Open;
    Ok(doc.clone())
}

/// Rechnung als bezahlt markieren (oder zurück). Nur Rechnungen haben
/// einen Zahlungsstatus.
pub fn set_paid(db: &mut Db, id: &str, paid: bool, today: &str) -> WfResult<()> {
    let doc = db.docs.iter_mut().find(|d| d.id == id).ok_or("doc-not-found")?;
    if doc.kind != DocKind::Invoice {
        return Err("bad-kind");
    }
    match (doc.status, paid) {
        (DocStatus::Open, true) => {
            doc.status = DocStatus::Paid;
            doc.paid_at = Some(today.to_string());
        }
        (DocStatus::Paid, false) => {
            doc.status = DocStatus::Open;
            doc.paid_at = None;
        }
        _ => return Err("bad-status"),
    }
    Ok(())
}

/// Storno einer festgeschriebenen Rechnung: verkettete Stornorechnung mit
/// negierten Preisen, sofort nummeriert; das Original wird "storniert".
pub fn cancel(
    db: &mut Db,
    id: &str,
    new_id: &str,
    prefix: &str,
    fmt: NumberFormat,
    today: &str,
    now_rfc3339: &str,
) -> WfResult<Doc> {
    if new_id.trim().is_empty() || db.docs.iter().any(|d| d.id == new_id) {
        return Err("bad-id");
    }
    let original = db.docs.iter().find(|d| d.id == id).cloned().ok_or("doc-not-found")?;
    if original.kind != DocKind::Invoice {
        return Err("bad-kind");
    }
    if original.status != DocStatus::Open && original.status != DocStatus::Paid {
        return Err("bad-status");
    }
    let mut items = original.items.clone();
    for item in &mut items {
        item.unit_price_cents = -item.unit_price_cents;
    }
    let mut storno = Doc {
        id: new_id.to_string(),
        kind: DocKind::Cancellation,
        number: None,
        customer_id: original.customer_id.clone(),
        customer_name: original.customer_name.clone(),
        customer_address: original.customer_address.clone(),
        date: today.to_string(),
        due_date: today.to_string(),
        // Storno betrifft die Leistung der Original-Rechnung
        service_date: if original.service_date.is_empty() {
            original.date.clone()
        } else {
            original.service_date.clone()
        },
        items,
        // Stornos gehen sofort live — sie dokumentieren, sie fordern nicht.
        status: DocStatus::Open,
        small_business: original.small_business,
        intro: String::new(),
        notes: String::new(),
        paid_at: None,
        related_id: Some(original.id.clone()),
        created_at: now_rfc3339.to_string(),
    };
    storno.number = Some(next_number(db, prefix, fmt));
    if let Some(orig) = db.docs.iter_mut().find(|d| d.id == id) {
        orig.status = DocStatus::Cancelled;
    }
    db.docs.push(storno.clone());
    Ok(storno)
}

/// Nur Entwürfe dürfen verschwinden.
pub fn delete_draft(db: &mut Db, id: &str) -> WfResult<()> {
    let doc = db.docs.iter().find(|d| d.id == id).ok_or("doc-not-found")?;
    if doc.status != DocStatus::Draft {
        return Err("doc-finalized");
    }
    db.docs.retain(|d| d.id != id);
    Ok(())
}

/// Kunden löschen — gesperrt, solange ein Entwurf auf ihn verweist
/// (festgeschriebene Belege tragen ihren eigenen Snapshot).
pub fn delete_customer(db: &mut Db, id: &str) -> WfResult<()> {
    let in_draft = db
        .docs
        .iter()
        .any(|d| d.customer_id == id && d.status == DocStatus::Draft);
    if in_draft {
        return Err("customer-in-draft");
    }
    db.customers.retain(|c| c.id != id);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{totals, Customer, LineItem};

    fn fmt(year: Option<i32>) -> NumberFormat<'static> {
        NumberFormat { year, digits: 4, separator: "-" }
    }

    fn draft(id: &str, kind: DocKind) -> Doc {
        Doc {
            id: id.into(),
            kind,
            customer_id: "c1".into(),
            customer_name: "ACME".into(),
            date: "2026-03-01".into(),
            due_date: "2026-03-15".into(),
            items: vec![LineItem {
                name: "Beratung".into(),
                quantity: 2.0,
                unit: "h".into(),
                unit_price_cents: 9000,
                vat_rate: 19,
                ..Default::default()
            }],
            created_at: "2026-03-01T10:00:00+01:00".into(),
            ..Default::default()
        }
    }

    #[test]
    fn upsert_cannot_smuggle_status_or_number() {
        let mut db = Db::default();
        let mut d = draft("d1", DocKind::Invoice);
        d.status = DocStatus::Paid;
        d.number = Some("RE-2026-9999".into());
        d.paid_at = Some("2026-01-01".into());
        upsert_draft(&mut db, d).unwrap();
        let stored = &db.docs[0];
        assert_eq!(stored.status, DocStatus::Draft);
        assert_eq!(stored.number, None);
        assert_eq!(stored.paid_at, None);
        // Stornos nur über cancel
        assert_eq!(upsert_draft(&mut db, draft("x", DocKind::Cancellation)), Err("bad-kind"));
        assert_eq!(upsert_draft(&mut db, draft(" ", DocKind::Invoice)), Err("bad-id"));
        let mut nan = draft("n", DocKind::Invoice);
        nan.items[0].quantity = f64::NAN;
        assert_eq!(upsert_draft(&mut db, nan), Err("bad-quantity"));
    }

    #[test]
    fn iso_dates_are_validated() {
        assert!(valid_iso_date("2026-03-01"));
        assert!(valid_iso_date("2028-02-29"));
        assert!(!valid_iso_date("2026-02-29"));
        assert!(!valid_iso_date("2100-02-29"));
        assert!(valid_iso_date("2000-02-29"));
        assert!(!valid_iso_date("2026-13-01"));
        assert!(!valid_iso_date("2026-04-31"));
        assert!(!valid_iso_date(""));
        assert!(!valid_iso_date("2026-3-1"));
        assert!(!valid_iso_date("01.03.2026"));
        assert!(!valid_iso_date("2026-0a-01"));
        assert!(!valid_iso_date("+026-01-01"));
    }

    #[test]
    fn finalize_rejects_bad_dates_without_using_a_number() {
        let mut db = Db::default();
        let mut nodate = draft("d1", DocKind::Invoice);
        nodate.date = String::new();
        upsert_draft(&mut db, nodate).unwrap();
        assert_eq!(finalize(&mut db, "d1", "RE", fmt).unwrap_err(), "doc-bad-date");
        let mut baddue = draft("d2", DocKind::Invoice);
        baddue.due_date = "NaN-NaN-NaN".into();
        upsert_draft(&mut db, baddue).unwrap();
        assert_eq!(finalize(&mut db, "d2", "RE", fmt).unwrap_err(), "doc-bad-date");
        let mut badservice = draft("d3", DocKind::Invoice);
        badservice.service_date = "2026-02-30".into();
        upsert_draft(&mut db, badservice).unwrap();
        assert_eq!(finalize(&mut db, "d3", "RE", fmt).unwrap_err(), "doc-bad-date");
        // leere Fälligkeit/Leistungsdatum sind erlaubt (Lieferschein, = Belegdatum)
        let mut ok = draft("d4", DocKind::DeliveryNote);
        ok.due_date = String::new();
        upsert_draft(&mut db, ok).unwrap();
        assert_eq!(
            finalize(&mut db, "d4", "LS", fmt).unwrap().number.as_deref(),
            Some("LS-2026-0001")
        );
        let mut inv = draft("d5", DocKind::Invoice);
        inv.service_date = "2026-02-27".into();
        upsert_draft(&mut db, inv).unwrap();
        assert_eq!(
            finalize(&mut db, "d5", "RE", fmt).unwrap().number.as_deref(),
            Some("RE-2026-0001")
        );
    }

    #[test]
    fn cancellation_keeps_the_service_date_of_the_original() {
        let mut db = Db::default();
        let mut inv = draft("d1", DocKind::Invoice);
        inv.service_date = "2026-02-20".into();
        upsert_draft(&mut db, inv).unwrap();
        finalize(&mut db, "d1", "RE", fmt).unwrap();
        let st = cancel(&mut db, "d1", "s1", "ST", fmt(Some(2026)), "2026-04-01", "now").unwrap();
        assert_eq!(st.service_date, "2026-02-20");
        upsert_draft(&mut db, draft("d2", DocKind::Invoice)).unwrap();
        finalize(&mut db, "d2", "RE", fmt).unwrap();
        let st2 = cancel(&mut db, "d2", "s2", "ST", fmt(Some(2026)), "2026-04-01", "now").unwrap();
        assert_eq!(st2.service_date, "2026-03-01");
    }

    #[test]
    fn finalized_docs_are_immutable_and_undeletable() {
        let mut db = Db::default();
        upsert_draft(&mut db, draft("d1", DocKind::Invoice)).unwrap();
        let fin = finalize(&mut db, "d1", "RE", fmt).unwrap();
        assert_eq!(fin.number.as_deref(), Some("RE-2026-0001"));
        assert_eq!(fin.status, DocStatus::Open);
        // erneut festschreiben, ändern, löschen — alles abgewiesen
        assert_eq!(finalize(&mut db, "d1", "RE", fmt).unwrap_err(), "doc-finalized");
        assert_eq!(upsert_draft(&mut db, draft("d1", DocKind::Invoice)), Err("doc-finalized"));
        assert_eq!(delete_draft(&mut db, "d1"), Err("doc-finalized"));
        assert_eq!(db.docs[0].number.as_deref(), Some("RE-2026-0001"));
    }

    #[test]
    fn finalize_uses_year_of_document_date_and_rejects_empty() {
        let mut db = Db::default();
        let mut d = draft("d1", DocKind::Invoice);
        d.date = "2025-12-31".into();
        upsert_draft(&mut db, d).unwrap();
        assert_eq!(
            finalize(&mut db, "d1", "RE", fmt).unwrap().number.as_deref(),
            Some("RE-2025-0001")
        );
        let mut empty = draft("d2", DocKind::Invoice);
        empty.items[0].name = "  ".into();
        upsert_draft(&mut db, empty).unwrap();
        assert_eq!(finalize(&mut db, "d2", "RE", fmt).unwrap_err(), "doc-empty");
        let mut nocust = draft("d3", DocKind::Invoice);
        nocust.customer_name = String::new();
        upsert_draft(&mut db, nocust).unwrap();
        assert_eq!(finalize(&mut db, "d3", "RE", fmt).unwrap_err(), "doc-no-customer");
        // fehlgeschlagene Versuche verbrauchen keine Nummer
        let mut ok = draft("d4", DocKind::Invoice);
        ok.date = "2025-06-01".into();
        upsert_draft(&mut db, ok).unwrap();
        assert_eq!(
            finalize(&mut db, "d4", "RE", fmt).unwrap().number.as_deref(),
            Some("RE-2025-0002")
        );
    }

    #[test]
    fn paid_toggle_only_for_invoices() {
        let mut db = Db::default();
        upsert_draft(&mut db, draft("d1", DocKind::Invoice)).unwrap();
        assert_eq!(set_paid(&mut db, "d1", true, "2026-03-10"), Err("bad-status"));
        finalize(&mut db, "d1", "RE", fmt).unwrap();
        set_paid(&mut db, "d1", true, "2026-03-10").unwrap();
        assert_eq!(db.docs[0].status, DocStatus::Paid);
        assert_eq!(db.docs[0].paid_at.as_deref(), Some("2026-03-10"));
        assert_eq!(set_paid(&mut db, "d1", true, "2026-03-11"), Err("bad-status"));
        set_paid(&mut db, "d1", false, "2026-03-11").unwrap();
        assert_eq!(db.docs[0].status, DocStatus::Open);
        assert_eq!(db.docs[0].paid_at, None);
        upsert_draft(&mut db, draft("q1", DocKind::Quote)).unwrap();
        finalize(&mut db, "q1", "AN", fmt).unwrap();
        assert_eq!(set_paid(&mut db, "q1", true, "2026-03-10"), Err("bad-kind"));
    }

    #[test]
    fn cancel_creates_linked_negated_numbered_storno() {
        let mut db = Db::default();
        upsert_draft(&mut db, draft("d1", DocKind::Invoice)).unwrap();
        // Entwürfe lassen sich nicht stornieren
        assert_eq!(
            cancel(&mut db, "d1", "s0", "ST", fmt(Some(2026)), "2026-03-20", "now").unwrap_err(),
            "bad-status"
        );
        finalize(&mut db, "d1", "RE", fmt).unwrap();
        set_paid(&mut db, "d1", true, "2026-03-10").unwrap();
        let st = cancel(&mut db, "d1", "s1", "ST", fmt(Some(2026)), "2026-03-20", "now").unwrap();
        assert_eq!(st.kind, DocKind::Cancellation);
        assert_eq!(st.number.as_deref(), Some("ST-2026-0001"));
        assert_eq!(st.related_id.as_deref(), Some("d1"));
        assert_eq!(st.status, DocStatus::Open);
        let orig = db.docs.iter().find(|d| d.id == "d1").unwrap();
        assert_eq!(orig.status, DocStatus::Cancelled);
        assert_eq!(
            totals(&st.items, st.small_business).gross_cents,
            -totals(&orig.items, orig.small_business).gross_cents
        );
        // doppelt stornieren geht nicht, ID-Kollision auch nicht
        assert_eq!(
            cancel(&mut db, "d1", "s2", "ST", fmt(Some(2026)), "2026-03-20", "now").unwrap_err(),
            "bad-status"
        );
        // Stornos/Gutschriften sind nicht stornierbar
        assert_eq!(
            cancel(&mut db, "s1", "s3", "ST", fmt(Some(2026)), "2026-03-20", "now").unwrap_err(),
            "bad-kind"
        );
        upsert_draft(&mut db, draft("d2", DocKind::Invoice)).unwrap();
        finalize(&mut db, "d2", "RE", fmt).unwrap();
        assert_eq!(
            cancel(&mut db, "d2", "s1", "ST", fmt(Some(2026)), "2026-03-20", "now").unwrap_err(),
            "bad-id"
        );
    }

    #[test]
    fn customer_delete_blocked_by_drafts_only() {
        let mut db = Db::default();
        db.customers.push(Customer { id: "c1".into(), name: "ACME".into(), ..Default::default() });
        upsert_draft(&mut db, draft("d1", DocKind::Invoice)).unwrap();
        assert_eq!(delete_customer(&mut db, "c1"), Err("customer-in-draft"));
        finalize(&mut db, "d1", "RE", fmt).unwrap();
        delete_customer(&mut db, "c1").unwrap();
        assert!(db.customers.is_empty());
        // der Beleg behält seinen Snapshot
        assert_eq!(db.docs[0].customer_name, "ACME");
    }

    #[test]
    fn draft_edit_keeps_created_at() {
        let mut db = Db::default();
        upsert_draft(&mut db, draft("d1", DocKind::Invoice)).unwrap();
        let mut edit = draft("d1", DocKind::Invoice);
        edit.created_at = "2030-01-01T00:00:00Z".into();
        edit.notes = "geändert".into();
        upsert_draft(&mut db, edit).unwrap();
        assert_eq!(db.docs[0].created_at, "2026-03-01T10:00:00+01:00");
        assert_eq!(db.docs[0].notes, "geändert");
    }
}
