//! E-Rechnung nach EN 16931: erzeugt Factur-X/XRechnung-kompatibles
//! CII-XML (CrossIndustryInvoice, Profil EN 16931). Dasselbe XML dient
//! als eigenständige XRechnung-Datei und als `factur-x.xml` im
//! ZUGFeRD-Hybrid-PDF.
//!
//! Doctrine: Beträge kommen aus derselben Cents-Rechnung wie das PDF
//! (`totals`), damit XML und Sichtteil nie auseinanderlaufen.

use crate::{line_total_cents, totals, Doc, DocKind};

/// Verkäufer-Stammdaten für die E-Rechnung (aus den App-Einstellungen).
#[derive(Debug, Clone, Default)]
pub struct Seller {
    pub name: String,
    /// Mehrzeilige Anschrift (ohne Namen).
    pub address: String,
    pub email: String,
    pub tax_number: String,
    pub vat_id: String,
    pub iban: String,
    pub bic: String,
    pub bank_name: String,
    /// ISO-3166-Code, leer = "DE".
    pub country: String,
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// "1234,50 €"-Cents → "1234.50" (EN 16931 verlangt Dezimalpunkt).
fn amount(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let abs = cents.abs();
    format!("{sign}{}.{:02}", abs / 100, abs % 100)
}

fn qty(q: f64) -> String {
    // bis zu 4 Nachkommastellen, ohne überflüssige Nullen
    let s = format!("{:.4}", q);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() { "0".into() } else { s.to_string() }
}

/// "2026-08-21" → "20260821" (Format 102). Fällt bei Murks auf Eingabe zurück.
fn date102(iso: &str) -> String {
    iso.chars().filter(|c| c.is_ascii_digit()).collect()
}

/// UN/ECE-Rec-20-Einheitencode aus der freien Einheit — Fallback C62 (unit).
pub fn unit_code(unit: &str) -> &'static str {
    match unit.trim().to_lowercase().as_str() {
        "stk" | "stück" | "stueck" | "pcs" | "piece" | "pieces" => "H87",
        "h" | "std" | "stunde" | "stunden" | "hour" | "hours" => "HUR",
        "tag" | "tage" | "day" | "days" => "DAY",
        "kg" => "KGM",
        "g" => "GRM",
        "m" => "MTR",
        "km" => "KMT",
        "m2" | "m²" | "qm" => "MTK",
        "l" | "liter" => "LTR",
        "monat" | "monate" | "month" | "months" => "MON",
        _ => "C62",
    }
}

/// Zerlegt eine mehrzeilige Anschrift heuristisch: eine Zeile im Muster
/// "PLZ Ort" wird zu (postcode, city); der Rest wird zu Adresszeilen.
pub fn split_address(address: &str) -> (Vec<String>, Option<(String, String)>) {
    let mut lines: Vec<String> = Vec::new();
    let mut post_city: Option<(String, String)> = None;
    for line in address.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if post_city.is_none() {
            let mut parts = line.splitn(2, ' ');
            if let (Some(first), Some(rest)) = (parts.next(), parts.next()) {
                let digits = first.chars().all(|c| c.is_ascii_digit());
                if digits && (4..=5).contains(&first.len()) && !rest.trim().is_empty() {
                    post_city = Some((first.to_string(), rest.trim().to_string()));
                    continue;
                }
            }
        }
        lines.push(line.to_string());
    }
    (lines, post_city)
}

fn party_xml(role: &str, name: &str, address: &str, country: &str, vat_id: &str) -> String {
    let (lines, post_city) = split_address(address);
    let country = if country.trim().is_empty() { "DE" } else { country.trim() };
    let mut out = String::new();
    out.push_str(&format!("<ram:{role}>"));
    out.push_str(&format!("<ram:Name>{}</ram:Name>", esc(name)));
    out.push_str("<ram:PostalTradeAddress>");
    if let Some((post, _)) = &post_city {
        out.push_str(&format!("<ram:PostcodeCode>{}</ram:PostcodeCode>", esc(post)));
    }
    if let Some(line) = lines.first() {
        out.push_str(&format!("<ram:LineOne>{}</ram:LineOne>", esc(line)));
    }
    if let Some(line) = lines.get(1) {
        out.push_str(&format!("<ram:LineTwo>{}</ram:LineTwo>", esc(line)));
    }
    if let Some((_, city)) = &post_city {
        out.push_str(&format!("<ram:CityName>{}</ram:CityName>", esc(city)));
    }
    out.push_str(&format!("<ram:CountryID>{}</ram:CountryID>", esc(country)));
    out.push_str("</ram:PostalTradeAddress>");
    if !vat_id.trim().is_empty() {
        out.push_str(&format!(
            "<ram:SpecifiedTaxRegistration><ram:ID schemeID=\"VA\">{}</ram:ID></ram:SpecifiedTaxRegistration>",
            esc(vat_id.trim())
        ));
    }
    out.push_str(&format!("</ram:{role}>"));
    out
}

/// Belegtyp nach UNTDID 1001: Rechnung 380, Gutschrift 381, Storno 384.
/// Angebote/AB/Lieferscheine sind keine E-Rechnungen — der Command weist
/// sie ab, bevor es hierher kommt.
fn type_code(kind: DocKind) -> &'static str {
    match kind {
        DocKind::Invoice => "380",
        DocKind::CreditNote => "381",
        DocKind::Cancellation => "384",
        DocKind::Quote | DocKind::OrderConfirmation | DocKind::DeliveryNote => "380",
    }
}

/// Erzeugt das vollständige CII-XML für einen festgeschriebenen Beleg.
pub fn einvoice_xml(doc: &Doc, seller: &Seller, buyer_reference: &str, buyer_country: &str) -> String {
    let sums = totals(&doc.items, doc.small_business);
    let currency = "EUR";
    let number = doc.number.clone().unwrap_or_else(|| "ENTWURF".into());

    let mut xml = String::with_capacity(6000);
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str(
        "<rsm:CrossIndustryInvoice \
         xmlns:rsm=\"urn:un:unece:uncefact:data:standard:CrossIndustryInvoice:100\" \
         xmlns:ram=\"urn:un:unece:uncefact:data:standard:ReusableAggregateBusinessInformationEntity:100\" \
         xmlns:udt=\"urn:un:unece:uncefact:data:standard:UnqualifiedDataType:100\">",
    );

    // Kontext: Profil EN 16931
    xml.push_str(
        "<rsm:ExchangedDocumentContext><ram:GuidelineSpecifiedDocumentContextParameter>\
         <ram:ID>urn:cen.eu:en16931:2017</ram:ID>\
         </ram:GuidelineSpecifiedDocumentContextParameter></rsm:ExchangedDocumentContext>",
    );

    // Kopf
    xml.push_str("<rsm:ExchangedDocument>");
    xml.push_str(&format!("<ram:ID>{}</ram:ID>", esc(&number)));
    xml.push_str(&format!("<ram:TypeCode>{}</ram:TypeCode>", type_code(doc.kind)));
    xml.push_str(&format!(
        "<ram:IssueDateTime><udt:DateTimeString format=\"102\">{}</udt:DateTimeString></ram:IssueDateTime>",
        date102(&doc.date)
    ));
    xml.push_str("</rsm:ExchangedDocument>");

    xml.push_str("<rsm:SupplyChainTradeTransaction>");

    // Positionen
    for (i, item) in doc.items.iter().enumerate() {
        xml.push_str("<ram:IncludedSupplyChainTradeLineItem>");
        xml.push_str(&format!(
            "<ram:AssociatedDocumentLineDocument><ram:LineID>{}</ram:LineID></ram:AssociatedDocumentLineDocument>",
            i + 1
        ));
        xml.push_str("<ram:SpecifiedTradeProduct>");
        xml.push_str(&format!("<ram:Name>{}</ram:Name>", esc(&item.name)));
        if !item.description.trim().is_empty() {
            xml.push_str(&format!(
                "<ram:Description>{}</ram:Description>",
                esc(item.description.trim())
            ));
        }
        xml.push_str("</ram:SpecifiedTradeProduct>");
        xml.push_str(&format!(
            "<ram:SpecifiedLineTradeAgreement><ram:NetPriceProductTradePrice>\
             <ram:ChargeAmount>{}</ram:ChargeAmount>\
             </ram:NetPriceProductTradePrice></ram:SpecifiedLineTradeAgreement>",
            amount(item.unit_price_cents)
        ));
        xml.push_str(&format!(
            "<ram:SpecifiedLineTradeDelivery><ram:BilledQuantity unitCode=\"{}\">{}</ram:BilledQuantity></ram:SpecifiedLineTradeDelivery>",
            unit_code(&item.unit),
            qty(item.quantity)
        ));
        let (category, rate) = if doc.small_business {
            ("E", 0)
        } else if item.vat_rate == 0 {
            ("Z", 0)
        } else {
            ("S", item.vat_rate)
        };
        xml.push_str("<ram:SpecifiedLineTradeSettlement>");
        xml.push_str(&format!(
            "<ram:ApplicableTradeTax><ram:TypeCode>VAT</ram:TypeCode>\
             <ram:CategoryCode>{category}</ram:CategoryCode>\
             <ram:RateApplicablePercent>{rate}</ram:RateApplicablePercent></ram:ApplicableTradeTax>"
        ));
        xml.push_str(&format!(
            "<ram:SpecifiedTradeSettlementLineMonetarySummation>\
             <ram:LineTotalAmount>{}</ram:LineTotalAmount>\
             </ram:SpecifiedTradeSettlementLineMonetarySummation>",
            amount(line_total_cents(item))
        ));
        xml.push_str("</ram:SpecifiedLineTradeSettlement>");
        xml.push_str("</ram:IncludedSupplyChainTradeLineItem>");
    }

    // Vertragsparteien
    xml.push_str("<ram:ApplicableHeaderTradeAgreement>");
    if !buyer_reference.trim().is_empty() {
        xml.push_str(&format!(
            "<ram:BuyerReference>{}</ram:BuyerReference>",
            esc(buyer_reference.trim())
        ));
    }
    xml.push_str(&party_xml(
        "SellerTradeParty",
        &seller.name,
        &seller.address,
        &seller.country,
        &seller.vat_id,
    ));
    xml.push_str(&party_xml(
        "BuyerTradeParty",
        &doc.customer_name,
        &doc.customer_address,
        buyer_country,
        "",
    ));
    xml.push_str("</ram:ApplicableHeaderTradeAgreement>");

    xml.push_str("<ram:ApplicableHeaderTradeDelivery/>");

    // Zahlung & Steuern
    xml.push_str("<ram:ApplicableHeaderTradeSettlement>");
    xml.push_str(&format!(
        "<ram:InvoiceCurrencyCode>{currency}</ram:InvoiceCurrencyCode>"
    ));
    let iban: String = seller
        .iban
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_uppercase();
    let iban = iban.strip_prefix("IBAN").unwrap_or(&iban).to_string();
    if !iban.is_empty() {
        xml.push_str("<ram:SpecifiedTradeSettlementPaymentMeans><ram:TypeCode>58</ram:TypeCode>");
        xml.push_str(&format!(
            "<ram:PayeePartyCreditorFinancialAccount><ram:IBANID>{}</ram:IBANID></ram:PayeePartyCreditorFinancialAccount>",
            esc(&iban)
        ));
        if !seller.bic.trim().is_empty() {
            xml.push_str(&format!(
                "<ram:PayeeSpecifiedCreditorFinancialInstitution><ram:BICID>{}</ram:BICID></ram:PayeeSpecifiedCreditorFinancialInstitution>",
                esc(seller.bic.trim())
            ));
        }
        xml.push_str("</ram:SpecifiedTradeSettlementPaymentMeans>");
    }
    if doc.small_business {
        xml.push_str(&format!(
            "<ram:ApplicableTradeTax><ram:CalculatedAmount>0.00</ram:CalculatedAmount>\
             <ram:TypeCode>VAT</ram:TypeCode>\
             <ram:ExemptionReason>Gemäß §19 UStG wird keine Umsatzsteuer berechnet (Kleinunternehmerregelung).</ram:ExemptionReason>\
             <ram:BasisAmount>{}</ram:BasisAmount>\
             <ram:CategoryCode>E</ram:CategoryCode>\
             <ram:RateApplicablePercent>0</ram:RateApplicablePercent></ram:ApplicableTradeTax>",
            amount(sums.net_cents)
        ));
    } else {
        // Zero-rated-Basis (Satz 0) separat ausweisen
        let zero_base: i64 = doc
            .items
            .iter()
            .filter(|i| i.vat_rate == 0)
            .map(line_total_cents)
            .sum();
        for v in &sums.vat {
            xml.push_str(&format!(
                "<ram:ApplicableTradeTax><ram:CalculatedAmount>{}</ram:CalculatedAmount>\
                 <ram:TypeCode>VAT</ram:TypeCode>\
                 <ram:BasisAmount>{}</ram:BasisAmount>\
                 <ram:CategoryCode>S</ram:CategoryCode>\
                 <ram:RateApplicablePercent>{}</ram:RateApplicablePercent></ram:ApplicableTradeTax>",
                amount(v.vat_cents),
                amount(v.base_cents),
                v.rate
            ));
        }
        if zero_base != 0 {
            xml.push_str(&format!(
                "<ram:ApplicableTradeTax><ram:CalculatedAmount>0.00</ram:CalculatedAmount>\
                 <ram:TypeCode>VAT</ram:TypeCode>\
                 <ram:BasisAmount>{}</ram:BasisAmount>\
                 <ram:CategoryCode>Z</ram:CategoryCode>\
                 <ram:RateApplicablePercent>0</ram:RateApplicablePercent></ram:ApplicableTradeTax>",
                amount(zero_base)
            ));
        }
    }
    if doc.kind == DocKind::Invoice && !doc.due_date.trim().is_empty() {
        xml.push_str(&format!(
            "<ram:SpecifiedTradePaymentTerms><ram:DueDateDateTime>\
             <udt:DateTimeString format=\"102\">{}</udt:DateTimeString>\
             </ram:DueDateDateTime></ram:SpecifiedTradePaymentTerms>",
            date102(&doc.due_date)
        ));
    }
    let vat_sum: i64 = sums.vat.iter().map(|v| v.vat_cents).sum();
    xml.push_str("<ram:SpecifiedTradeSettlementHeaderMonetarySummation>");
    xml.push_str(&format!(
        "<ram:LineTotalAmount>{}</ram:LineTotalAmount>",
        amount(sums.net_cents)
    ));
    xml.push_str(&format!(
        "<ram:TaxBasisTotalAmount>{}</ram:TaxBasisTotalAmount>",
        amount(sums.net_cents)
    ));
    xml.push_str(&format!(
        "<ram:TaxTotalAmount currencyID=\"{currency}\">{}</ram:TaxTotalAmount>",
        amount(vat_sum)
    ));
    xml.push_str(&format!(
        "<ram:GrandTotalAmount>{}</ram:GrandTotalAmount>",
        amount(sums.gross_cents)
    ));
    xml.push_str(&format!(
        "<ram:DuePayableAmount>{}</ram:DuePayableAmount>",
        amount(sums.gross_cents)
    ));
    xml.push_str("</ram:SpecifiedTradeSettlementHeaderMonetarySummation>");
    xml.push_str("</ram:ApplicableHeaderTradeSettlement>");

    xml.push_str("</rsm:SupplyChainTradeTransaction>");
    xml.push_str("</rsm:CrossIndustryInvoice>\n");
    xml
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DocStatus, LineItem};

    fn seller() -> Seller {
        Seller {
            name: "LAN-SOLO UG".into(),
            address: "Musterstr. 1\n20095 Hamburg".into(),
            email: "buero@lan-solo.de".into(),
            tax_number: "12/345/6789".into(),
            vat_id: "DE308701478".into(),
            iban: "IBAN: DE15 2004 0000 0347 2727 00".into(),
            bic: "COBADEFFXXX".into(),
            bank_name: "Commerzbank".into(),
            country: String::new(),
        }
    }

    fn doc() -> Doc {
        Doc {
            id: "d1".into(),
            kind: DocKind::Invoice,
            number: Some("RE-2026-0001".into()),
            customer_name: "ACME & Söhne GmbH".into(),
            customer_address: "Beispielweg 2\n10115 Berlin".into(),
            date: "2026-08-21".into(),
            due_date: "2026-09-04".into(),
            status: DocStatus::Open,
            items: vec![
                LineItem {
                    name: "Beratung".into(),
                    description: String::new(),
                    quantity: 1.5,
                    unit: "h".into(),
                    unit_price_cents: 9000,
                    vat_rate: 19,
                },
                LineItem {
                    name: "Bücher".into(),
                    description: "Fachliteratur".into(),
                    quantity: 2.0,
                    unit: "Stk".into(),
                    unit_price_cents: 1000,
                    vat_rate: 7,
                },
            ],
            ..Default::default()
        }
    }

    #[test]
    fn xml_contains_mandatory_fields_and_sums() {
        let xml = einvoice_xml(&doc(), &seller(), "04011000-1234-56", "");
        assert!(xml.contains("urn:cen.eu:en16931:2017"));
        assert!(xml.contains("<ram:ID>RE-2026-0001</ram:ID>"));
        assert!(xml.contains("<ram:TypeCode>380</ram:TypeCode>"));
        assert!(xml.contains(">20260821<"));
        assert!(xml.contains("<ram:BuyerReference>04011000-1234-56</ram:BuyerReference>"));
        // 1,5 × 90 € = 135 € (19 %), 2 × 10 € = 20 € (7 %) → netto 155,00
        assert!(xml.contains("<ram:LineTotalAmount>135.00</ram:LineTotalAmount>"));
        assert!(xml.contains("<ram:LineTotalAmount>155.00</ram:LineTotalAmount>"));
        // USt: 25,65 + 1,40 = 27,05 → brutto 182,05
        assert!(xml.contains("<ram:TaxTotalAmount currencyID=\"EUR\">27.05</ram:TaxTotalAmount>"));
        assert!(xml.contains("<ram:GrandTotalAmount>182.05</ram:GrandTotalAmount>"));
        assert!(xml.contains("<ram:DuePayableAmount>182.05</ram:DuePayableAmount>"));
        // Escaping
        assert!(xml.contains("ACME &amp; Söhne GmbH"));
        // IBAN bereinigt (Prefix "IBAN:" und Leerzeichen raus)
        assert!(xml.contains("<ram:IBANID>DE15200400000347272700</ram:IBANID>"));
        // Einheitencodes
        assert!(xml.contains("unitCode=\"HUR\">1.5<"));
        assert!(xml.contains("unitCode=\"H87\">2<"));
        // Adress-Heuristik
        assert!(xml.contains("<ram:PostcodeCode>20095</ram:PostcodeCode>"));
        assert!(xml.contains("<ram:CityName>Hamburg</ram:CityName>"));
        assert!(xml.contains("<ram:CountryID>DE</ram:CountryID>"));
    }

    #[test]
    fn small_business_uses_category_e_with_reason() {
        let mut d = doc();
        d.small_business = true;
        let xml = einvoice_xml(&d, &seller(), "", "");
        assert!(xml.contains("<ram:CategoryCode>E</ram:CategoryCode>"));
        assert!(xml.contains("§19 UStG"));
        assert!(!xml.contains("<ram:CategoryCode>S</ram:CategoryCode>"));
        // brutto = netto
        assert!(xml.contains("<ram:GrandTotalAmount>155.00</ram:GrandTotalAmount>"));
        assert!(xml.contains("<ram:TaxTotalAmount currencyID=\"EUR\">0.00</ram:TaxTotalAmount>"));
    }

    #[test]
    fn credit_note_and_cancellation_type_codes() {
        let mut d = doc();
        d.kind = DocKind::CreditNote;
        assert!(einvoice_xml(&d, &seller(), "", "").contains("<ram:TypeCode>381</ram:TypeCode>"));
        d.kind = DocKind::Cancellation;
        assert!(einvoice_xml(&d, &seller(), "", "").contains("<ram:TypeCode>384</ram:TypeCode>"));
    }

    #[test]
    fn address_split_heuristic() {
        let (lines, pc) = split_address("Traute-Lafrenz-Str. 106\n21035 Hamburg\nDeutschland");
        assert_eq!(lines, vec!["Traute-Lafrenz-Str. 106".to_string(), "Deutschland".to_string()]);
        assert_eq!(pc, Some(("21035".into(), "Hamburg".into())));
        // ohne PLZ-Zeile: alles bleibt Adresszeile
        let (lines2, pc2) = split_address("Irgendwo 5");
        assert_eq!(lines2, vec!["Irgendwo 5".to_string()]);
        assert_eq!(pc2, None);
    }

    #[test]
    fn negative_amounts_for_cancellation() {
        let mut d = doc();
        d.kind = DocKind::Cancellation;
        for item in &mut d.items {
            item.unit_price_cents = -item.unit_price_cents;
        }
        let xml = einvoice_xml(&d, &seller(), "", "");
        assert!(xml.contains("<ram:GrandTotalAmount>-182.05</ram:GrandTotalAmount>"));
    }
}
