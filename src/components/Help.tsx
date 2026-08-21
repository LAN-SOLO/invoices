import { useState } from 'react';
import { Lang } from '../i18n';

// Selbstständiges Hilfe-System: schwebender ?-Button, First-Run-Tutorial
// und durchsuchbares Handbuch.

interface Step {
  title: string;
  body: string[];
}

interface Section {
  id: string;
  title: string;
  body: string[];
}

interface Content {
  labels: {
    fab: string;
    tutorial: string;
    manual: string;
    search: string;
    next: string;
    back: string;
    skip: string;
    done: string;
    stepOf: (n: number, total: number) => string;
    noResults: string;
  };
  tutorial: Step[];
  sections: Section[];
}

const de: Content = {
  labels: {
    fab: 'Hilfe & Handbuch',
    tutorial: 'Tutorial',
    manual: 'Handbuch',
    search: 'Handbuch durchsuchen …',
    next: 'Weiter',
    back: 'Zurück',
    skip: 'Überspringen',
    done: 'Los geht’s',
    stepOf: (n, total) => `Schritt ${n} von ${total}`,
    noResults: 'Keine Treffer',
  },
  tutorial: [
    {
      title: 'Willkommen bei invoices.',
      body: [
        'invoices schreibt Rechnungen für kleine Unternehmen: Kunden und Artikel verwalten, Rechnung anlegen, PDF exportieren, offene Posten abhaken.',
        'Alle Daten bleiben lokal auf deinem Rechner. Kein Konto, keine Cloud, keine Telemetrie.',
        'Dieses Tutorial dauert zwei Minuten. Du findest es jederzeit wieder über den ?-Knopf unten rechts.',
      ],
    },
    {
      title: 'Erst der Briefkopf',
      body: [
        'Bevor die erste Rechnung rausgeht: Einstellungen (Zahnrad oben rechts) öffnen und einmal ausfüllen —',
        '• Firmenname, Anschrift, E-Mail und Telefon',
        '• Steuernummer bzw. USt-IdNr. und Bankverbindung',
        '• optional ein Logo (PNG/JPG) für die rechte obere Ecke',
        'Diese Angaben landen automatisch auf jedem PDF — Briefkopf, Fußzeile, Bankverbindung.',
        'Kleinunternehmer nach §19 UStG? Ein Haken in den Einstellungen — invoices lässt die USt dann komplett weg und druckt den Pflichthinweis.',
      ],
    },
    {
      title: 'Kunden & Artikel',
      body: [
        'Die Tabs „Kunden“ und „Artikel“ sind deine Stammdaten:',
        '• Kunden — Name, Anschrift, E-Mail; die Anschrift landet im Adressfeld der PDFs',
        '• Artikel — Leistungen und Produkte mit Einheit, Netto-Preis und USt-Satz',
        'Einmal angelegt, fügst du beides mit zwei Klicks in jede Rechnung ein. Für Einzelfälle geht auch eine freie Position ohne Artikel.',
      ],
    },
    {
      title: 'Die erste Rechnung',
      body: [
        '„Neue Rechnung“ oben rechts — dann:',
        '• Kunde wählen, Datum prüfen (das Zahlungsziel rechnet sich selbst)',
        '• Positionen hinzufügen: aus der Artikelliste oder frei',
        '• Netto, USt und Gesamtbetrag rechnen live mit',
        'Als Entwurf speichern, solange noch etwas offen ist — Entwürfe haben keine Nummer und dürfen sich ändern.',
        '„Festschreiben“ vergibt die fortlaufende Nummer (z. B. RE-2026-0001) und sperrt den Beleg. Danach: PDF exportieren und verschicken.',
      ],
    },
    {
      title: 'Offene Posten',
      body: [
        'Die Filterleiste zeigt, wo du stehst: Entwürfe, offen, überfällig, bezahlt, storniert — plus die offene Gesamtsumme rechts.',
        '• Zahlung eingegangen? Haken-Knopf: als bezahlt markieren.',
        '• Rechnung falsch? Stornieren erzeugt eine verkettete Stornorechnung mit eigener Nummer — die Original-Rechnung bleibt unverändert erhalten.',
        'So bleibt die Belegkette sauber: nichts wird gelöscht, alles ist nachvollziehbar.',
      ],
    },
  ],
  sections: [
    {
      id: 'setup',
      title: 'Einrichtung & Briefkopf',
      body: [
        'Einstellungen → „Briefkopf & Firmendaten“: Firmenname, Anschrift, E-Mail, Telefon, Steuernummer, USt-IdNr., Bankverbindung und optional ein Logo.',
        'Alles davon erscheint auf den PDFs: Anschrift im Briefkopf, Steuernummer und Bank in der Fußzeile, das Logo oben rechts.',
        '• Zahlungsziel (Tage) — daraus rechnet sich „Fällig bis“ auf neuen Rechnungen',
        '• Standard-USt-Satz und Standard-Einheit — vorausgewählt für neue Positionen',
        '• Standard-Einleitungstext — steht automatisch in jeder neuen Rechnung',
        '• Nummernkreise — Präfixe für Rechnungen (RE), Gutschriften (GS) und Storno (ST)',
        '• Nummernkreis-Layout — mit oder ohne Jahr, Stellen und Trennzeichen (siehe eigenes Kapitel)',
      ],
    },
    {
      id: 'appearance',
      title: 'Darstellung',
      body: [
        'Einstellungen → „Darstellung“:',
        '• Modus — Dunkel (Standard) oder Hell',
        '• Akzentfarbe — Sky-Blau, Smaragd, Violett oder Bernstein',
        'Beides wirkt sofort nach dem Speichern auf die ganze App.',
      ],
    },
    {
      id: 'numbering',
      title: 'Nummernkreis-Layout',
      body: [
        'Einstellungen → „Nummernkreis-Layout“ bestimmt, wie neue Belegnummern aussehen:',
        '• Jahr im Nummernkreis — es wird immer das aktuelle Jahr eingefügt und der Zähler startet pro Jahr neu bei 0001 (z. B. RE-2026-0001)',
        '• Ohne Jahr — ein fortlaufender Zähler ohne Neustart (z. B. RE-0001, RE-0002, …)',
        '• Stellen (3–6) und Trennzeichen (-, / oder ohne) sind frei wählbar',
        'Die Vorschau in den Einstellungen zeigt das Ergebnis live. Bereits vergebene Nummern ändern sich nie — das Layout gilt ab dem nächsten Festschreiben.',
      ],
    },
    {
      id: 'smallbusiness',
      title: 'Kleinunternehmer (§19 UStG)',
      body: [
        'Der Haken „Kleinunternehmerregelung“ in den Einstellungen gilt für alle neuen Belege:',
        '• Es wird keine Umsatzsteuer berechnet oder ausgewiesen',
        '• Das PDF druckt den Pflichthinweis „Gemäß §19 UStG wird keine Umsatzsteuer berechnet“',
        'Bestehende Belege ändern sich nicht — der Haken wirkt ab dem nächsten neuen Beleg.',
      ],
    },
    {
      id: 'customers',
      title: 'Kunden',
      body: [
        'Tab „Kunden“: Name/Firma, mehrzeilige Anschrift, E-Mail, USt-IdNr. und interne Notizen.',
        'Beim Festschreiben eines Belegs wird die Kunden-Anschrift eingefroren — ändert sich der Kunde später, bleiben alte Belege korrekt.',
        'Löschen: möglich, solange der Kunde in keinem Entwurf steckt. Festgeschriebene Belege behalten ihre Daten ohnehin.',
      ],
    },
    {
      id: 'products',
      title: 'Artikel',
      body: [
        'Tab „Artikel“: Bezeichnung, Beschreibung, Einheit (Stk, h, Tag, pauschal …), Netto-Preis und USt-Satz.',
        'Im Rechnungs-Editor fügst du Artikel über „Aus Artikelliste …“ ein — Menge und Preis lassen sich pro Position anpassen, ohne den Artikel zu verändern.',
      ],
    },
    {
      id: 'documents',
      title: 'Rechnungen & Entwürfe',
      body: [
        'Neue Belege starten als Entwurf: ohne Nummer, frei änderbar, jederzeit löschbar.',
        '„Festschreiben & Nummer vergeben“ macht daraus einen echten Beleg:',
        '• fortlaufende Nummer aus dem Nummernkreis (z. B. RE-2026-0001, pro Jahr neu ab 0001)',
        '• Status „offen“ — der Beleg zählt zu den offenen Posten',
        '• der Inhalt ist ab jetzt gesperrt — korrigiert wird per Storno, nicht per Radiergummi',
        'Gutschriften funktionieren genauso, mit eigenem Nummernkreis (GS).',
      ],
    },
    {
      id: 'chain',
      title: 'Belegkette: Angebot → Rechnung',
      body: [
        'Neben Rechnungen und Gutschriften gibt es Angebote, Auftragsbestätigungen und Lieferscheine — über „Weitere Belegart …“ in der Filterleiste, jede mit eigenem Nummernkreis (AN/AB/LS).',
        '• Angebote haben ein „Gültig bis“-Datum statt einer Fälligkeit',
        '• Lieferscheine kommen ohne Preise — auf dem PDF stehen nur Positionen und Mengen',
        '• Festgeschriebene Angebote und Auftragsbestätigungen haben eine Pfeil-Aktion: „Als Rechnung übernehmen“ — Kunde, Positionen und Texte wandern in einen neuen Rechnungs-Entwurf, der Beleg kennt seine Herkunft',
        'So wird aus einem Angebot in Sekunden eine Rechnung, ohne etwas doppelt zu tippen.',
      ],
    },
    {
      id: 'templates',
      title: 'Vorlagen',
      body: [
        'Wiederkehrende Belege müssen nicht jedes Mal neu getippt werden:',
        '• Im Beleg-Editor: „Als Vorlage speichern“ — sichert Positionen, Einleitung und Notiz unter einem Namen (ohne Kunde)',
        '• In der Filterleiste: „Aus Vorlage …“ — öffnet die Vorlagenliste; ein Klick erzeugt einen frischen Entwurf daraus',
        'Vorlagen lassen sich in derselben Liste löschen (✕). Der Kunde wird im neuen Entwurf gewählt — Vorlagen sind bewusst kundenneutral.',
      ],
    },
    {
      id: 'cancel',
      title: 'Stornieren',
      body: [
        'Storno-Knopf an jeder festgeschriebenen Rechnung: erzeugt eine Stornorechnung mit negierten Beträgen und eigener Nummer (ST-…).',
        'Die Original-Rechnung bleibt vollständig erhalten und wird als „storniert“ markiert — die Stornorechnung verweist auf sie.',
        'Danach einfach eine korrigierte neue Rechnung schreiben.',
      ],
    },
    {
      id: 'dashboard',
      title: 'Übersicht (Dashboard)',
      body: [
        'Der Tab „Übersicht“ ist das Monitoring: alles Wichtige auf einen Blick, live aus deinen Belegen gerechnet.',
        '• Kacheln — offene Posten, überfällige Summe, Umsatz Monat/Jahr, Entwürfe, Kunden',
        '• Monatsumsatz — die letzten 12 Monate als Balken (Gutschriften und Stornos ziehen ab)',
        '• Fällige Rechnungen — die nächsten Fälligkeiten, überfällige rot; Klick öffnet den Beleg',
        '• Top-Kunden — die fünf umsatzstärksten Kunden',
        'Umsatz zählt festgeschriebene Rechnungen (offen + bezahlt) minus Gutschriften und Stornos — Entwürfe, Angebote und Lieferscheine zählen nicht.',
      ],
    },
    {
      id: 'payments',
      title: 'Offene Posten & Zahlungen',
      body: [
        'Die Filterleiste gruppiert alle Belege: Entwürfe, offen, überfällig, bezahlt, storniert.',
        '„Überfällig“ ist jede offene Rechnung, deren Fälligkeitsdatum verstrichen ist.',
        'Rechts in der Leiste steht die Summe aller offenen Beträge — dein Blick auf die Außenstände.',
        'Der Haken-Knopf markiert eine Rechnung als bezahlt (und zurück, falls doch nicht).',
      ],
    },
    {
      id: 'pdf',
      title: 'PDF, Vorschau & Layouts',
      body: [
        'Der PDF-Knopf erzeugt ein A4-Dokument: Briefkopf mit Logo, Absenderzeile, Empfänger, Positionstabelle, USt-Aufstellung und Fußzeile mit Steuernummer und Bankverbindung.',
        'Vorschau: das Augen-Symbol (Liste) bzw. „Vorschau“ (Editor) zeigt den Beleg vor dem Export — auch bei Entwürfen.',
        'Drei Layouts, umschaltbar direkt in der Vorschau:',
        '• Klassisch — nüchterner Briefkopf, dunkle Tabellenköpfe',
        '• Modern — farbiges Kopfband in der Akzentfarbe',
        '• Kompakt — engere Ränder und kleinere Schrift für lange Positionslisten',
        'Das Standard-Layout stellst du unter Einstellungen → Darstellung ein; die Vorschau exportiert im gerade gewählten Layout.',
        'Der Dateiname folgt der Belegnummer (RE-2026-0001.pdf), den Speicherort wählst du im Dialog.',
      ],
    },
    {
      id: 'security',
      title: 'Sicherheit & Backups',
      body: [
        'Einstellungen → „Sicherheit & Backups“: Ein Backup ist eine JSON-Datei mit allem — Belege, Kunden, Artikel, Vorlagen, Nummernkreise und Einstellungen.',
        '• „Backup jetzt erstellen …“ — speichert den kompletten Stand als Datei, wohin du willst',
        '• „Backup wiederherstellen …“ — ersetzt nach Rückfrage ALLE Daten durch den Stand aus der Datei',
        '• Automatisches Backup — legt bei jedem Start ein Backup im gewählten Ordner ab und behält die letzten 10 Stände',
        'Tipp: Den Backup-Ordner auf ein NAS oder in einen synchronisierten Ordner legen — dann sind die Stände auch außer Haus.',
        'Für invoices billed geplant: Cloud-Backup über eigenen Speicher, Backups nach Zeitplan und Ende-zu-Ende-Verschlüsselung.',
      ],
    },
    {
      id: 'einvoice',
      title: 'E-Rechnung (XRechnung & ZUGFeRD)',
      body: [
        'Seit 2025 müssen Unternehmen in Deutschland E-Rechnungen empfangen können; die Pflicht, selbst welche auszustellen, greift stufenweise ab 2027. invoices ist dafür bereit — nach EN 16931:',
        '• XRechnung-XML — der Beleg als reine XML-Datei (CII-Syntax), z. B. für Behörden und Portale',
        '• ZUGFeRD-PDF — das gewohnte PDF mit eingebettetem factur-x.xml: für Menschen lesbar, für Software auswertbar',
        'Beides findest du in den Beleg-Aktionen jeder festgeschriebenen Rechnung (Doppel-Dokument- und </>-Symbol).',
        'Damit die E-Rechnung vollständig ist:',
        '• Briefkopf komplett ausfüllen — inklusive USt-IdNr. oder Steuernummer und IBAN',
        '• PLZ und Ort als eigene Zeile in jeder Anschrift („20095 Hamburg“)',
        '• Für Behörden (B2G): die Leitweg-ID des Kunden in der Kundenkartei hinterlegen',
        'Beträge, Steuern und Summen kommen aus derselben Rechnung wie das PDF — XML und Sichtteil laufen nie auseinander.',
      ],
    },
    {
      id: 'data',
      title: 'Datenablage',
      body: [
        'Alle Daten liegen in einer JSON-Datei im App-Datenordner deines Systems — Kunden, Artikel, Belege und Nummernkreise.',
        'Backups: einfach die Datei sichern (der Pfad steht in den Einstellungen der Zukunft — bis dahin: App-Datenordner des Systems, Ordner „com.lan-solo.invoices“).',
        'invoices ist ein Werkzeug, keine Steuerberatung — Aufbewahrungspflichten (GoBD) bleiben deine Verantwortung. Feste Nummernkreise und unveränderliche Belege helfen dabei.',
      ],
    },
    {
      id: 'updates',
      title: 'Updates',
      body: [
        'invoices prüft beim Start automatisch auf neue Versionen und zeigt einen Hinweis, wenn eine bereitsteht — installiert wird erst nach deinem Klick.',
        'Manuell prüfen: Einstellungen → „Nach Updates suchen“. Vor der Installation siehst du das Changelog.',
        'Updates kommen signiert von GitHub (LAN-SOLO/invoices): Die App prüft die Signatur, bevor irgendetwas installiert wird. Deine Daten bleiben unangetastet.',
      ],
    },
    {
      id: 'privacy',
      title: 'Privatsphäre',
      body: [
        'Alles bleibt lokal: Belege, Kunden, Artikel und Einstellungen liegen auf deinem Rechner. Kein Upload, kein Konto, keine Telemetrie.',
        'Die einzige Netzwerkverbindung ist der Update-Check gegen GitHub.',
      ],
    },
  ],
};

const en: Content = {
  labels: {
    fab: 'Help & manual',
    tutorial: 'Tutorial',
    manual: 'Manual',
    search: 'Search the manual …',
    next: 'Next',
    back: 'Back',
    skip: 'Skip',
    done: 'Let’s go',
    stepOf: (n, total) => `Step ${n} of ${total}`,
    noResults: 'No matches',
  },
  tutorial: [
    {
      title: 'Welcome to invoices.',
      body: [
        'invoices writes invoices for small businesses: manage customers and products, create an invoice, export a PDF, check off open items.',
        'All data stays local on your machine. No account, no cloud, no telemetry.',
        'This tutorial takes two minutes. Reopen it anytime via the ? button in the bottom right.',
      ],
    },
    {
      title: 'Letterhead first',
      body: [
        'Before the first invoice goes out: open Settings (gear, top right) and fill in once —',
        '• company name, address, email and phone',
        '• tax number or VAT ID and bank details',
        '• optionally a logo (PNG/JPG) for the top right corner',
        'These details land on every PDF automatically — letterhead, footer, bank details.',
        'Using the German small-business exemption (§19 UStG)? One checkbox in Settings — invoices then omits VAT entirely and prints the mandatory note.',
      ],
    },
    {
      title: 'Customers & products',
      body: [
        'The “Customers” and “Products” tabs are your master data:',
        '• Customers — name, address, email; the address goes into the PDF address block',
        '• Products — services and goods with unit, net price and VAT rate',
        'Created once, both are two clicks away in every invoice. One-offs work as free positions without a product.',
      ],
    },
    {
      title: 'The first invoice',
      body: [
        '“New invoice”, top right — then:',
        '• choose a customer, check the date (the due date computes itself)',
        '• add positions: from the product list or free-form',
        '• net, VAT and total update live',
        'Save as draft while things are still in flux — drafts have no number and may change.',
        '“Finalize” assigns the sequential number (e.g. RE-2026-0001) and locks the document. Then: export the PDF and send it.',
      ],
    },
    {
      title: 'Open items',
      body: [
        'The filter bar shows where you stand: drafts, open, overdue, paid, cancelled — plus the total open amount on the right.',
        '• Payment arrived? Check button: mark as paid.',
        '• Wrong invoice? Cancelling creates a linked cancellation invoice with its own number — the original stays untouched.',
        'The document chain stays clean: nothing is deleted, everything is traceable.',
      ],
    },
  ],
  sections: [
    {
      id: 'setup',
      title: 'Setup & letterhead',
      body: [
        'Settings → “Letterhead & company data”: company name, address, email, phone, tax number, VAT ID, bank details and optionally a logo.',
        'All of it appears on the PDFs: address in the letterhead, tax number and bank in the footer, the logo top right.',
        '• Payment terms (days) — “due date” on new invoices computes from this',
        '• Default VAT rate and default unit — preselected for new positions',
        '• Default introduction text — added to every new invoice automatically',
        '• Number prefixes — for invoices (RE), credit notes (GS) and cancellations (ST)',
        '• Number range layout — with or without year, digits and separator (see its own chapter)',
      ],
    },
    {
      id: 'appearance',
      title: 'Appearance',
      body: [
        'Settings → “Appearance”:',
        '• Mode — dark (default) or light',
        '• Accent color — sky blue, emerald, violet or amber',
        'Both apply to the whole app right after saving.',
      ],
    },
    {
      id: 'numbering',
      title: 'Number range layout',
      body: [
        'Settings → “Number range layout” controls what new document numbers look like:',
        '• Include year — the current year is always inserted and the counter restarts at 0001 each year (e.g. RE-2026-0001)',
        '• Without year — one continuous counter that never restarts (e.g. RE-0001, RE-0002, …)',
        '• Digits (3–6) and separator (-, / or none) are freely configurable',
        'The preview in Settings shows the result live. Numbers already assigned never change — the layout applies from the next finalization on.',
      ],
    },
    {
      id: 'smallbusiness',
      title: 'Small-business exemption (§19 UStG)',
      body: [
        'The “small-business exemption” checkbox in Settings applies to all new documents:',
        '• no VAT is calculated or shown',
        '• the PDF prints the mandatory §19 UStG note',
        'Existing documents stay unchanged — the checkbox applies from the next new document on.',
      ],
    },
    {
      id: 'customers',
      title: 'Customers',
      body: [
        '“Customers” tab: name/company, multiline address, email, VAT ID and internal notes.',
        'When a document is finalized, the customer address is frozen into it — if the customer changes later, old documents stay correct.',
        'Deleting: possible as long as the customer is not used in a draft. Finalized documents keep their data anyway.',
      ],
    },
    {
      id: 'products',
      title: 'Products',
      body: [
        '“Products” tab: name, description, unit (pcs, h, day, flat …), net price and VAT rate.',
        'In the invoice editor, insert products via “From product list …” — quantity and price are adjustable per position without touching the product.',
      ],
    },
    {
      id: 'documents',
      title: 'Invoices & drafts',
      body: [
        'New documents start as drafts: no number, freely editable, deletable anytime.',
        '“Finalize & assign number” turns them into real documents:',
        '• sequential number from the number range (e.g. RE-2026-0001, restarting at 0001 each year)',
        '• status “open” — the document counts towards open items',
        '• the content is locked from now on — corrections happen via cancellation, not an eraser',
        'Credit notes work the same way, with their own number range (GS).',
      ],
    },
    {
      id: 'chain',
      title: 'Document chain: quote → invoice',
      body: [
        'Besides invoices and credit notes there are quotes, order confirmations and delivery notes — via “More document types …” in the filter bar, each with its own number range (AN/AB/LS).',
        '• Quotes carry a “valid until” date instead of a due date',
        '• Delivery notes come without prices — the PDF lists only positions and quantities',
        '• Finalized quotes and order confirmations have an arrow action: “Turn into invoice” — customer, positions and texts move into a fresh invoice draft that knows its origin',
        'A quote becomes an invoice in seconds, with nothing typed twice.',
      ],
    },
    {
      id: 'templates',
      title: 'Templates',
      body: [
        'Recurring documents don’t need to be retyped every time:',
        '• In the document editor: “Save as template” — stores positions, introduction and note under a name (without a customer)',
        '• In the filter bar: “From template …” — opens the template list; one click creates a fresh draft from it',
        'Templates can be deleted in the same list (✕). The customer is chosen in the new draft — templates are deliberately customer-neutral.',
      ],
    },
    {
      id: 'cancel',
      title: 'Cancelling',
      body: [
        'Cancel button on every finalized invoice: creates a cancellation invoice with negated amounts and its own number (ST-…).',
        'The original invoice is fully preserved and marked “cancelled” — the cancellation references it.',
        'Then simply write a corrected new invoice.',
      ],
    },
    {
      id: 'dashboard',
      title: 'Overview (dashboard)',
      body: [
        'The “Overview” tab is your monitoring: everything important at a glance, computed live from your documents.',
        '• Tiles — open items, overdue sum, revenue month/year, drafts, customers',
        '• Monthly revenue — the last 12 months as bars (credit notes and cancellations subtract)',
        '• Invoices due — the next due dates, overdue in red; clicking opens the document',
        '• Top customers — your five strongest customers by revenue',
        'Revenue counts finalized invoices (open + paid) minus credit notes and cancellations — drafts, quotes and delivery notes don’t count.',
      ],
    },
    {
      id: 'payments',
      title: 'Open items & payments',
      body: [
        'The filter bar groups all documents: drafts, open, overdue, paid, cancelled.',
        '“Overdue” is any open invoice whose due date has passed.',
        'On the right of the bar: the sum of all open amounts — your view of outstanding money.',
        'The check button marks an invoice as paid (and back, if it turns out otherwise).',
      ],
    },
    {
      id: 'pdf',
      title: 'PDF, preview & layouts',
      body: [
        'The PDF button produces an A4 document: letterhead with logo, sender line, recipient, positions table, VAT breakdown and a footer with tax number and bank details.',
        'Preview: the eye icon (list) or “Preview” (editor) shows the document before exporting — drafts included.',
        'Three layouts, switchable right in the preview:',
        '• Classic — sober letterhead, dark table headers',
        '• Modern — colored header band in your accent color',
        '• Compact — tighter margins and smaller type for long position lists',
        'Set the default layout under Settings → Appearance; the preview exports in whatever layout is selected.',
        'The file name follows the document number (RE-2026-0001.pdf); you pick the location in the dialog.',
      ],
    },
    {
      id: 'security',
      title: 'Security & backups',
      body: [
        'Settings → “Security & backups”: a backup is one JSON file with everything — documents, customers, products, templates, number ranges and settings.',
        '• “Create backup now …” — saves the complete state as a file, wherever you want',
        '• “Restore backup …” — after confirmation, replaces ALL data with the file’s contents',
        '• Automatic backup — writes a backup into the chosen folder on every launch and keeps the last 10',
        'Tip: point the backup folder at a NAS or a synced folder — your backups leave the house too.',
        'Planned for invoices billed: cloud backup via your own storage, scheduled backups and end-to-end encryption.',
      ],
    },
    {
      id: 'einvoice',
      title: 'E-invoicing (XRechnung & ZUGFeRD)',
      body: [
        'Since 2025, German businesses must be able to receive e-invoices; the duty to issue them phases in from 2027. invoices is ready — per EN 16931:',
        '• XRechnung XML — the document as a pure XML file (CII syntax), e.g. for public authorities and portals',
        '• ZUGFeRD PDF — the familiar PDF with embedded factur-x.xml: readable for humans, parseable for software',
        'Both live in the document actions of every finalized invoice (the double-document and </> icons).',
        'For a complete e-invoice:',
        '• Fill in the letterhead completely — including VAT ID or tax number and IBAN',
        '• Postcode and city on their own address line (“20095 Hamburg”)',
        '• For public authorities (B2G): store the customer’s Leitweg-ID in the customer record',
        'Amounts, taxes and totals come from the same math as the PDF — XML and visual part never diverge.',
      ],
    },
    {
      id: 'data',
      title: 'Data storage',
      body: [
        'All data lives in one JSON file in your system’s app-data folder — customers, products, documents and number ranges.',
        'Backups: just copy that file (system app-data folder, “com.lan-solo.invoices”).',
        'invoices is a tool, not tax advice — retention duties stay your responsibility. Fixed number ranges and immutable documents help with that.',
      ],
    },
    {
      id: 'updates',
      title: 'Updates',
      body: [
        'invoices checks for new versions on launch and shows a notice when one is available — nothing installs without your click.',
        'Check manually: Settings → “Check for updates”. You see the changelog before installing.',
        'Updates come signed from GitHub (LAN-SOLO/invoices): the app verifies the signature before installing anything. Your data stays untouched.',
      ],
    },
    {
      id: 'privacy',
      title: 'Privacy',
      body: [
        'Everything stays local: documents, customers, products and settings live on your machine. No upload, no account, no telemetry.',
        'The only network connection is the update check against GitHub.',
      ],
    },
  ],
};

const SEEN_KEY = 'invoices.tutorialSeen';

export function Help({ lang }: { lang: Lang }) {
  const c = lang === 'de' ? de : en;
  const [mode, setMode] = useState<'closed' | 'tutorial' | 'manual'>(() =>
    localStorage.getItem(SEEN_KEY) ? 'closed' : 'tutorial'
  );
  const [step, setStep] = useState(0);
  const [sel, setSel] = useState(c.sections[0].id);
  const [q, setQ] = useState('');

  const close = () => {
    localStorage.setItem(SEEN_KEY, '1');
    setMode('closed');
    setStep(0);
  };

  const query = q.trim().toLowerCase();
  const filtered = query
    ? c.sections.filter(
        (s) =>
          s.title.toLowerCase().includes(query) ||
          s.body.some((p) => p.toLowerCase().includes(query))
      )
    : c.sections;
  const current = filtered.find((s) => s.id === sel) ?? filtered[0] ?? null;

  return (
    <>
      <button className="hlp-fab" title={c.labels.fab} onClick={() => setMode('manual')}>
        ?
      </button>
      {mode !== 'closed' && (
        <div className="hlp-overlay" onClick={close}>
          <div className="hlp-modal" onClick={(e) => e.stopPropagation()}>
            <div className="hlp-head">
              <span className="hlp-brand">
                <span className="hlp-name">invoices</span>
                <span className="hlp-dot">.</span>
              </span>
              <button
                className={`hlp-tab ${mode === 'tutorial' ? 'active' : ''}`}
                onClick={() => {
                  setMode('tutorial');
                  setStep(0);
                }}
              >
                {c.labels.tutorial}
              </button>
              <button
                className={`hlp-tab ${mode === 'manual' ? 'active' : ''}`}
                onClick={() => setMode('manual')}
              >
                {c.labels.manual}
              </button>
              <span className="hlp-spacer" />
              <button className="hlp-close" onClick={close}>
                ✕
              </button>
            </div>

            {mode === 'tutorial' && (
              <div className="hlp-tut">
                <div className="hlp-step-count">
                  {c.labels.stepOf(step + 1, c.tutorial.length)}
                </div>
                <h2>{c.tutorial[step].title}</h2>
                {c.tutorial[step].body.map((p, i) =>
                  p.startsWith('• ') ? (
                    <div key={i} className="hlp-li">
                      {p.slice(2)}
                    </div>
                  ) : (
                    <p key={i}>{p}</p>
                  )
                )}
                <div className="hlp-tut-nav">
                  <button className="hlp-ghost" onClick={close}>
                    {c.labels.skip}
                  </button>
                  <span className="hlp-dots">
                    {c.tutorial.map((_, i) => (
                      <span key={i} className={i === step ? 'on' : ''} />
                    ))}
                  </span>
                  {step > 0 && (
                    <button onClick={() => setStep(step - 1)}>{c.labels.back}</button>
                  )}
                  {step < c.tutorial.length - 1 ? (
                    <button className="hlp-primary" onClick={() => setStep(step + 1)}>
                      {c.labels.next}
                    </button>
                  ) : (
                    <button className="hlp-primary" onClick={close}>
                      {c.labels.done}
                    </button>
                  )}
                </div>
              </div>
            )}

            {mode === 'manual' && (
              <div className="hlp-body">
                <div className="hlp-toc">
                  <input
                    type="text"
                    placeholder={c.labels.search}
                    value={q}
                    onChange={(e) => setQ(e.target.value)}
                  />
                  {filtered.length === 0 && (
                    <div className="hlp-empty">{c.labels.noResults}</div>
                  )}
                  {filtered.map((s) => (
                    <button
                      key={s.id}
                      className={`hlp-toc-item ${current?.id === s.id ? 'active' : ''}`}
                      onClick={() => setSel(s.id)}
                    >
                      {s.title}
                    </button>
                  ))}
                </div>
                <div className="hlp-content">
                  {current && (
                    <>
                      <h2>{current.title}</h2>
                      {current.body.map((p, i) =>
                        p.startsWith('• ') ? (
                          <div key={i} className="hlp-li">
                            {p.slice(2)}
                          </div>
                        ) : (
                          <p key={i}>{p}</p>
                        )
                      )}
                    </>
                  )}
                </div>
              </div>
            )}
          </div>
        </div>
      )}
    </>
  );
}
