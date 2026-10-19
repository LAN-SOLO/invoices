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
    /// Telefon für den Verkäufer-Kontakt (BT-42).
    pub phone: String,
}

/// Käufer-Angaben, die nicht im Beleg-Snapshot stecken (aus der Kundenkartei).
#[derive(Debug, Clone, Default)]
pub struct Buyer {
    /// Leitweg-ID bzw. Käufer-Referenz (BT-10).
    pub reference: String,
    /// ISO-3166-Code, leer = "DE".
    pub country: String,
    /// USt-IdNr. des Käufers (BT-48).
    pub vat_id: String,
    /// E-Mail als elektronische Adresse des Käufers (BT-49).
    pub email: String,
}

/// XML-Escaping für Text und Attribute; Steuerzeichen (außer Tab/Zeilen-
/// umbruch), die XML 1.0 verbietet, werden entfernt.
fn esc(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control() || matches!(c, '\t' | '\n' | '\r'))
        .collect::<String>()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
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

struct Party<'a> {
    role: &'a str,
    name: &'a str,
    address: &'a str,
    country: &'a str,
    vat_id: &'a str,
    /// Steuernummer (schemeID "FC") — nur beim Verkäufer.
    tax_number: &'a str,
    email: &'a str,
    /// Kontakt (Name/Telefon/E-Mail) — nur beim Verkäufer.
    contact: Option<(&'a str, &'a str)>,
}

fn party_xml(p: &Party) -> String {
    let Party { role, name, address, country, vat_id, .. } = *p;
    let (lines, post_city) = split_address(address);
    let country = if country.trim().is_empty() { "DE" } else { country.trim() };
    let mut out = String::new();
    out.push_str(&format!("<ram:{role}>"));
    if vat_id.trim().is_empty() && !p.tax_number.trim().is_empty() {
        // BR-CO-26: ohne USt-IdNr. braucht der Verkäufer eine Kennung
        // (BT-29) — typischer Fall Kleinunternehmer; die Steuernummer dient
        // dann als Verkäufer-Kennung.
        out.push_str(&format!("<ram:ID>{}</ram:ID>", esc(p.tax_number.trim())));
    }
    out.push_str(&format!("<ram:Name>{}</ram:Name>", esc(name)));
    if let Some((phone, email)) = p.contact {
        if !phone.trim().is_empty() || !email.trim().is_empty() {
            out.push_str("<ram:DefinedTradeContact>");
            out.push_str(&format!("<ram:PersonName>{}</ram:PersonName>", esc(name)));
            if !phone.trim().is_empty() {
                out.push_str(&format!(
                    "<ram:TelephoneUniversalCommunication><ram:CompleteNumber>{}</ram:CompleteNumber></ram:TelephoneUniversalCommunication>",
                    esc(phone.trim())
                ));
            }
            if !email.trim().is_empty() {
                out.push_str(&format!(
                    "<ram:EmailURIUniversalCommunication><ram:URIID>{}</ram:URIID></ram:EmailURIUniversalCommunication>",
                    esc(email.trim())
                ));
            }
            out.push_str("</ram:DefinedTradeContact>");
        }
    }
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
    if !p.email.trim().is_empty() {
        // elektronische Adresse (BT-34/BT-49), Schema EM = E-Mail
        out.push_str(&format!(
            "<ram:URIUniversalCommunication><ram:URIID schemeID=\"EM\">{}</ram:URIID></ram:URIUniversalCommunication>",
            esc(p.email.trim())
        ));
    }
    if !p.tax_number.trim().is_empty() {
        // Steuernummer (BT-32) — Pflicht, wenn keine USt-IdNr. vorhanden ist
        // (BR-CO-26), etwa bei Kleinunternehmern
        out.push_str(&format!(
            "<ram:SpecifiedTaxRegistration><ram:ID schemeID=\"FC\">{}</ram:ID></ram:SpecifiedTaxRegistration>",
            esc(p.tax_number.trim())
        ));
    }
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
pub fn einvoice_xml(doc: &Doc, seller: &Seller, buyer: &Buyer) -> String {
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
        // EN 16931 BR-27: der Netto-Preis darf nie negativ sein — Storno-
        // Positionen (negierter Preis) werden als negative Menge zum
        // positiven Preis ausgedrückt; der Zeilenbetrag bleibt identisch.
        let (price, quantity) = if item.unit_price_cents < 0 {
            (-item.unit_price_cents, -item.quantity)
        } else {
            (item.unit_price_cents, item.quantity)
        };
        xml.push_str(&format!(
            "<ram:SpecifiedLineTradeAgreement><ram:NetPriceProductTradePrice>\
             <ram:ChargeAmount>{}</ram:ChargeAmount>\
             </ram:NetPriceProductTradePrice></ram:SpecifiedLineTradeAgreement>",
            amount(price)
        ));
        xml.push_str(&format!(
            "<ram:SpecifiedLineTradeDelivery><ram:BilledQuantity unitCode=\"{}\">{}</ram:BilledQuantity></ram:SpecifiedLineTradeDelivery>",
            unit_code(&item.unit),
            qty(quantity)
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
    if !buyer.reference.trim().is_empty() {
        xml.push_str(&format!(
            "<ram:BuyerReference>{}</ram:BuyerReference>",
            esc(buyer.reference.trim())
        ));
    }
    xml.push_str(&party_xml(&Party {
        role: "SellerTradeParty",
        name: &seller.name,
        address: &seller.address,
        country: &seller.country,
        vat_id: &seller.vat_id,
        tax_number: &seller.tax_number,
        email: &seller.email,
        contact: Some((&seller.phone, &seller.email)),
    }));
    xml.push_str(&party_xml(&Party {
        role: "BuyerTradeParty",
        name: &doc.customer_name,
        address: &doc.customer_address,
        country: &buyer.country,
        vat_id: &buyer.vat_id,
        tax_number: "",
        email: &buyer.email,
        contact: None,
    }));
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
    // Fälligkeit (BT-9): bei Rechnungen und überall dort, wo ein positiver
    // Betrag fällig ist (BR-CO-25 verlangt dann Fälligkeit oder Bedingungen)
    if !doc.due_date.trim().is_empty() && (doc.kind == DocKind::Invoice || sums.gross_cents > 0) {
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
            phone: "+49 40 123456".into(),
        }
    }

    fn buyer(reference: &str) -> Buyer {
        Buyer { reference: reference.into(), ..Default::default() }
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
        let xml = einvoice_xml(&doc(), &seller(), &buyer("04011000-1234-56"));
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
        let xml = einvoice_xml(&d, &seller(), &buyer(""));
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
        assert!(einvoice_xml(&d, &seller(), &buyer("")).contains("<ram:TypeCode>381</ram:TypeCode>"));
        d.kind = DocKind::Cancellation;
        assert!(einvoice_xml(&d, &seller(), &buyer("")).contains("<ram:TypeCode>384</ram:TypeCode>"));
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
        let xml = einvoice_xml(&d, &seller(), &buyer(""));
        assert!(xml.contains("<ram:GrandTotalAmount>-182.05</ram:GrandTotalAmount>"));
        // BR-27: Preise nie negativ — das Vorzeichen wandert in die Menge
        assert!(!xml.contains("<ram:ChargeAmount>-"));
        assert!(xml.contains("<ram:ChargeAmount>90.00</ram:ChargeAmount>"));
        assert!(xml.contains("unitCode=\"HUR\">-1.5<"));
        assert!(xml.contains("<ram:LineTotalAmount>-135.00</ram:LineTotalAmount>"));
    }

    #[test]
    fn seller_tax_number_contact_and_electronic_addresses() {
        let mut s = seller();
        s.vat_id = String::new();
        let b = Buyer {
            reference: String::new(),
            country: "AT".into(),
            vat_id: "ATU12345678".into(),
            email: "einkauf@acme.example".into(),
        };
        let xml = einvoice_xml(&doc(), &s, &b);
        // ohne USt-IdNr. trägt die Steuernummer (BR-CO-26)
        assert!(xml.contains("<ram:ID schemeID=\"FC\">12/345/6789</ram:ID>"));
        // … und als Verkäufer-Kennung BT-29 (BR-CO-26)
        assert!(xml.contains("<ram:SellerTradeParty><ram:ID>12/345/6789</ram:ID><ram:Name>"));
        assert!(!xml.contains("schemeID=\"VA\">DE"));
        // Verkäufer-Kontakt + elektronische Adressen
        assert!(xml.contains("<ram:CompleteNumber>+49 40 123456</ram:CompleteNumber>"));
        assert!(xml.contains("<ram:URIID schemeID=\"EM\">buero@lan-solo.de</ram:URIID>"));
        assert!(xml.contains("<ram:URIID schemeID=\"EM\">einkauf@acme.example</ram:URIID>"));
        // Käufer-USt-IdNr. und Land
        assert!(xml.contains("<ram:ID schemeID=\"VA\">ATU12345678</ram:ID>"));
        assert!(xml.contains("<ram:CountryID>AT</ram:CountryID>"));
    }

    #[test]
    fn credit_note_carries_due_date_and_xml_is_clean() {
        let mut d = doc();
        d.kind = DocKind::CreditNote;
        d.customer_name = "Böse\u{0001} <Firma>".into();
        let xml = einvoice_xml(&d, &seller(), &buyer(""));
        assert!(xml.contains("<ram:DueDateDateTime>"));
        assert!(xml.contains("Böse &lt;Firma&gt;"));
        assert!(!xml.contains('\u{0001}'));
    }

    #[test]
    fn xml_element_order_follows_cii_schema() {
        // Reihenfolge in TradeParty: Name, DefinedTradeContact,
        // PostalTradeAddress, URIUniversalCommunication, SpecifiedTaxRegistration
        let xml = einvoice_xml(&doc(), &seller(), &buyer(""));
        let seller_part = &xml[xml.find("<ram:SellerTradeParty>").unwrap()
            ..xml.find("</ram:SellerTradeParty>").unwrap()];
        let pos = |needle: &str| seller_part.find(needle).unwrap_or_else(|| panic!("{needle}"));
        assert!(pos("<ram:Name>") < pos("<ram:DefinedTradeContact>"));
        assert!(pos("<ram:DefinedTradeContact>") < pos("<ram:PostalTradeAddress>"));
        assert!(pos("<ram:PostalTradeAddress>") < pos("<ram:URIUniversalCommunication>"));
        assert!(pos("<ram:URIUniversalCommunication>") < pos("schemeID=\"FC\""));
        assert!(pos("schemeID=\"FC\"") < pos("schemeID=\"VA\""));
    }

    /// Schreibt Beispiel-XMLs für externe Validatoren (Mustang/KoSIT), wenn
    /// `INVOICES_XML_DUMP=<ordner>` gesetzt ist — sonst ohne Wirkung.
    #[test]
    fn dump_samples_for_external_validation() {
        let Ok(dir) = std::env::var("INVOICES_XML_DUMP") else { return };
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut b = buyer("04011000-1234-56");
        b.email = "einkauf@acme.example".into();
        std::fs::write(dir.join("invoice.xml"), einvoice_xml(&doc(), &seller(), &b)).unwrap();
        let mut sb = doc();
        sb.small_business = true;
        let mut s = seller();
        s.vat_id = String::new();
        std::fs::write(dir.join("small-business.xml"), einvoice_xml(&sb, &s, &b)).unwrap();
        let mut st = doc();
        st.kind = DocKind::Cancellation;
        st.number = Some("ST-2026-0001".into());
        for item in &mut st.items {
            item.unit_price_cents = -item.unit_price_cents;
        }
        std::fs::write(dir.join("cancellation.xml"), einvoice_xml(&st, &seller(), &b)).unwrap();
        let mut cn = doc();
        cn.kind = DocKind::CreditNote;
        cn.number = Some("GS-2026-0001".into());
        std::fs::write(dir.join("creditnote.xml"), einvoice_xml(&cn, &seller(), &b)).unwrap();
        let mut mixed = doc();
        mixed.items.push(crate::LineItem {
            name: "Porto".into(),
            quantity: 1.0,
            unit: "pauschal".into(),
            unit_price_cents: 250,
            vat_rate: 0,
            ..Default::default()
        });
        std::fs::write(dir.join("mixed-zero.xml"), einvoice_xml(&mixed, &seller(), &b)).unwrap();
    }
}
