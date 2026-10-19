# invoices. — Audit zur Version 1.0.0

Stand: 2026-10-19 · Basis: `f818cc3` (0.4.0) · Ziel: Test-Build 1.0.0 für macOS + Windows

Legende Teststatus:

- **automatisch** — Unit-/Integrationstest im Repo (`cargo test --workspace`)
- **extern validiert** — mit einem unabhängigen Prüfwerkzeug geprüft (Mustang 2.26 / veraPDF)
- **UI-Mock** — Web-Frontend im Headless-Chrome gegen simulierte Tauri-Commands (`@tauri-apps/api/mocks`) durchgeklickt; prüft Oberfläche und IPC-Aufrufe, NICHT das echte Rust-Backend
- **Node-Harness** — Frontend-Module (PDF, ZUGFeRD, Geldparser) in Node ausgeführt, Ergebnisdateien geprüft
- **statisch** — nur Code-Review, braucht einen Menschen auf dem Zielsystem

## 1. Feature-Inventar

| Bereich | Feature | Status |
|---|---|---|
| Belege | Rechnung anlegen, Entwurf speichern, löschen | automatisch (Workflow) + UI-Mock |
| | Festschreiben → fortlaufende Nummer, Sperre | automatisch + UI-Mock |
| | Nummer nach Jahr des Belegdatums, fehlgeschlagene Versuche verbrauchen keine Nummer | automatisch |
| | Gutschrift, Angebot, Auftragsbestätigung, Lieferschein (Präfixe GS/AN/AB/LS) | automatisch (Nummern) + UI-Mock (Angebot) |
| | Angebot/AB → Rechnungs-Entwurf („Als Rechnung übernehmen“) | UI-Mock |
| | Storno: verkettete ST-Rechnung, negierte Beträge, Original „storniert“ | automatisch + UI-Mock |
| | Bezahlt / zurück auf offen (nur Rechnungen) | automatisch + UI-Mock |
| | Vorlagen speichern, verwenden, löschen | UI-Mock |
| | Suche, Filter (Alle/Entwürfe/Offen/Überfällig/Bezahlt/Storniert) | UI-Mock (Filter), automatisch (Suche) |
| Stammdaten | Kunden anlegen/ändern/löschen (Sperre bei Entwurf), strukturierte Anschrift | automatisch + UI-Mock |
| | Kunde direkt im Editor anlegen | statisch |
| | Artikel anlegen/ändern/löschen, in Beleg übernehmen | UI-Mock |
| Geld/Steuer | Zeilenbetrag, USt je Satz auf Summenbasis, §19 | automatisch (Rust) + Node-Harness (TS-Spiegel) |
| | Preis-Eingabe DE/EN („1.234,56“, „1,234.56“, „12,5“, „-5“) | Node-Harness + UI-Mock |
| PDF | 4 Layouts × DE/EN, alle 6 Belegarten, Entwurf, 60 Positionen (Seitenumbruch) | Node-Harness (Dateien gerendert und gesichtet) |
| | Vorschau (Blob-iframe), Layout-Umschalter, Export | UI-Mock |
| | Logo (PNG/JPEG) | statisch (Magic-Byte-Prüfung automatisch) |
| E-Rechnung | XRechnung-/Factur-X-XML: Rechnung, Kleinunternehmer, Gutschrift, Storno, 0-%-Position | **extern validiert** (Mustang, Factur-X EN 16931: alle „valid“) |
| | ZUGFeRD-PDF (eingebettetes factur-x.xml) | extern validiert: XML valid, PDF/A-3b **nicht** konform (Fonts) — siehe Grenzen |
| Dashboard | Kacheln, 12-Monats-Balken, Fälligkeiten, Top-Kunden | UI-Mock |
| Einstellungen | 4 Reiter, Live-Wirkung, Abbrechen nimmt zurück | UI-Mock |
| | Präfixe (alle sechs Pflicht), Nummernkreis-Layout + Vorschau | UI-Mock (Präfix), automatisch (Layout) |
| | Hell/Dunkel, Akzent, 20 Farbschemata, Colorpicker | UI-Mock (Hell/Dunkel), statisch (Rest) |
| | Sprache DE/EN | UI-Mock |
| Mehrfirmen | Anlegen, Wechseln, Entfernen (nur aus Registry) | UI-Mock (Frontend) + automatisch (ID-Validierung) |
| Sicherheit | Backup exportieren / wiederherstellen (Abweisung ungültiger Dateien) | UI-Mock; Backend-Prüfungen statisch |
| | Auto-Backup beim Start, 10 Stände | statisch |
| | Quarantäne kaputter Datenbank + Hinweis | automatisch (Core) + UI-Mock (Hinweis) |
| App | Update-Check, Banner, Auto-Update, Version aus `getVersion` | UI-Mock (Version), statisch (Updater) |
| | Hilfe: Tutorial, Handbuch, DE/EN-Umschalter | UI-Mock (öffnet) |

## 2. Gefundene und behobene Fehler

1. **Datenverlust bei kaputter `db.json`:** Laden schlug still fehl → leere Datenbank → nächstes Speichern überschrieb die Datei. Jetzt Quarantäne (`db.json.corrupt-<stamp>`) + Hinweis. Gleiches für Einstellungen (Kopie).
2. **Speicherfehler wurden verschluckt** (`let _ = save_db`): Oberfläche meldete „Gespeichert“. Jetzt Fehler an die UI, In-Memory-Stand wird auf Platte zurückgesetzt (keine vergebene, ungespeicherte Nummer).
3. **E-Rechnung BR-27 verletzt:** Storno-Positionen mit negativem Preis → für Validatoren ungültig. Jetzt negative Menge × positiver Preis.
4. **E-Rechnung BR-CO-26 verletzt** für Kleinunternehmer ohne USt-IdNr. Jetzt Steuernummer als Verkäufer-Kennung (BT-29) + Registrierung FC.
5. **E-Rechnung unvollständig:** Verkäufer-Kontakt, elektronische Adressen (E-Mail), Käufer-USt-IdNr. fehlten; Gutschriften ohne Fälligkeit (BR-CO-25).
6. **Rundung über `f64`:** 2,675 × 1,00 € ergab 2,67 € statt 2,68 €; jetzt ganzzahlig und symmetrisch (Rust + TS-Spiegel identisch).
7. **Editor:** Nach „Festschreiben“ zeigten Titel und PDF-Export keine Nummer (lokale Kopie veraltet). Getippte Menge/Preis ohne Blur gingen beim Speichern verloren.
8. **Preiseingabe „1,234.56“** ergab 0 €.
9. **Gutschriften, Stornos, AB, Lieferscheine** wurden am Folgetag als „überfällig“ markiert und im Filter „Offen“ geführt.
10. **Backup-Wiederherstellung:** „Abbrechen“ danach holte den alten Einstellungsstand in die Oberfläche zurück. Jetzt lädt die App frisch.
11. **Rohe Fehlercodes** („doc-finalized“, „customer-in-draft“ für jeden Fehler) — jetzt übersetzte Meldungen DE/EN; viele Aktionen hatten gar kein Fehler-Handling.
12. **React-Warnung** (setState im Render) bei Live-Einstellungen; Dashboard-Monat nach UTC statt lokal; Versionsanzeige hartkodiert; PDF-Spalte „Pos.“ brach um; „Pos.“ in der Vorlagenliste nicht übersetzt; leere Präfixe für AN/AB/LS speicherbar.

## 3. Sicherheitsbefunde

| Schwere | Befund | Status |
|---|---|---|
| hoch | Pfad-Traversal über Firmen-ID: `add_company`/`switch_company` bauten `settings-<id>.json` und `company-<id>/` aus Frontend-Eingabe | behoben (`valid_id`: nur `[A-Za-z0-9-]`, ≤ 64; manipulierte Registry wird beim Start bereinigt) |
| hoch | Datenintegrität: Nummernkreis per `upsert_doc` umgehbar (Status/Nummer vom Frontend gesetzt), Stornos frei anlegbar | behoben (Workflow in `core/workflow.rs`, nur Entwürfe über upsert) |
| mittel | `write_file` schrieb beliebige Bytes an beliebige Pfade/Endungen | behoben (absolut + Endung pdf/xml/json/csv, atomar) |
| mittel | `get_logo` las beliebige Dateien des Nutzers ins Frontend | behoben (nur PNG/JPEG per Magic Bytes, ≤ 10 MB) |
| mittel | Backup-Import ohne Größen-/Versionsprüfung, ersetzte Daten ohne Rückweg | behoben (≤ 512 MB, `backupVersion` 1, Sicherungskopie `db.json.before-restore-*`) |
| mittel | jsPDF 2.5.2 mit bekannten Advisories (u. a. GHSA für AcroForm/addJS/html/BMP/LFI) | **offen** — Fix erst in jsPDF 4.2.1 (Major-Sprung, verlangt jspdf-autotable 5). Genutzt werden nur `text`, `addImage` (eigenes, geprüftes PNG/JPEG), `autoTable`, `output` — die betroffenen APIs (`html`, `addJS`, AcroForm, Node-`loadFile`) nicht. Upgrade für 1.1 empfohlen. |
| niedrig | DOMPurify (transitiv über jsPDF, nur für `jsPDF.html()`, ungenutzt) | offen, nicht erreichbar |
| niedrig | `source-map-js` (nur Dev/Build) | behoben (Patch-Update im Lockfile) |
| niedrig | Ungenutzte `opener:default`-Berechtigung | behoben (entfernt) |
| niedrig | CSP ohne `object-src`/`base-uri`/`form-action` | behoben (ergänzt) |
| info | XML-Escaping: Apostroph und XML-1.0-Steuerzeichen | behoben |
| info | XMP-Titel im ZUGFeRD-PDF unescaped | behoben |
| geprüft | Updater: Pubkey gesetzt, Endpoint HTTPS (GitHub), Signaturprüfung durch Tauri | ok |
| geprüft | Kein `dangerouslySetInnerHTML`, kein `eval`, keine Shell-/Command-Aufrufe, keine HTTP-Requests außer Updater | ok |
| geprüft | localStorage nur `invoices.tutorialSeen` | ok |
| geprüft | Kein TLS-Abschalten, keine Secrets im Code | ok |
| nicht geprüft | `cargo audit` (nicht installiert) | offen |

## 4. Bekannte Grenzen

- **ZUGFeRD-PDF ist nicht streng PDF/A-3b-konform:** veraPDF meldet 2 Regelverstöße (Klausel 6.2.11.4.1, Helvetica nicht eingebettet). Das eingebettete XML ist EN-16931-valide. Lösung: eigene TTF-Schrift einbetten (1.1).
- **XRechnung im engeren Sinn (KoSIT-CIUS):** Das XML nutzt die Kennung `urn:cen.eu:en16931:2017` (Factur-X/ZUGFeRD EN 16931). Behördenportale, die die XRechnung-CIUS-Kennung und deren Zusatzregeln (BR-DE-*) verlangen, können es ablehnen.
- **Kein eigenes Leistungsdatum:** Das Belegdatum gilt als Rechnungsdatum; den Leistungszeitraum bitte in die Positionsbeschreibung schreiben (§14 UStG).
- **Kopfzeile ApplicableHeaderTradeDelivery bleibt leer** (Warnung PEPPOL-EN16931-R008, kein Fehler).
- **Sprache beim Erststart unter Windows:** Die Voreinstellung liest `LANG`; Windows setzt das nicht → Englisch, bis in Einstellungen → App umgestellt wird.
- **Kundenlöschen:** E-Mail/USt-IdNr./Leitweg-ID der E-Rechnung kommen live aus der Kundenkartei; nach dem Löschen eines Kunden fehlen sie in später exportierten E-Rechnungen (Name/Anschrift bleiben als Snapshot).
- **jsPDF-Standardschriften (WinAnsi):** Zeichen außerhalb Westeuropas (z. B. ł, ő, kyrillisch) werden im PDF falsch dargestellt.
- **Windows-Kompilat** konnte lokal nicht geprüft werden (`ring` lässt sich ohne MSVC nicht cross-kompilieren); `invoices-core` kompiliert für `x86_64-pc-windows-msvc`. Der CI-Build ist der Nachweis.
- **Kein Mahnwesen, keine wiederkehrenden Rechnungen, kein CSV-Export** (Roadmap, `INVOICES_PLAN.md`).

## 5. Manuelle Testliste (macOS und Windows)

Jeweils mit frischem Profil **und** mit einem Datenstand aus 0.4.0 (Update-Pfad) testen.

1. Start: Fenster erscheint, Tutorial läuft, Hilfe-?-Knopf, DE/EN-Umschalter in der Hilfe.
2. Einstellungen → Unternehmen: Briefkopf, Steuernummer, IBAN, Logo (PNG und JPG) wählen; Speichern; App neu starten → Werte da.
3. Kunde anlegen (Straße/PLZ/Ort), Artikel anlegen (Preis „1.234,50“).
4. Rechnung: Kunde, Artikel + freie Position, Menge „2,5“ tippen und **sofort** „Entwurf speichern“ → Menge erhalten.
5. Festschreiben → Titel zeigt RE-JJJJ-0001; PDF exportieren in einen Ordner mit Umlauten/Leerzeichen (Windows: auch anderes Laufwerk, z. B. `D:\`).
6. Vorschau: alle vier Layouts, Export aus der Vorschau.
7. XRechnung-XML und ZUGFeRD-PDF exportieren; ZUGFeRD-PDF in einem Viewer öffnen (Anhang `factur-x.xml` sichtbar).
8. Bezahlt / zurück auf offen; Storno → ST-Nummer, Original „storniert“, Storno „festgeschrieben“.
9. Angebot festschreiben → „Als Rechnung übernehmen“; Lieferschein-PDF ohne Preise; Gutschrift — am Folgetag NICHT „überfällig“.
10. Vorlage speichern/verwenden/löschen.
11. Kunde mit Entwurf löschen → verständliche Meldung.
12. Backup erstellen; Backup wiederherstellen (gültig + eine beliebige andere JSON → Ablehnung); prüfen, dass `db.json.before-restore-*` im Datenordner liegt.
13. Auto-Backup aktivieren, Ordner wählen (Windows: Netzlaufwerk/UNC), App 2× starten → 2 Dateien.
14. Zweites Unternehmen anlegen, wechseln, zurückwechseln, entfernen.
15. Hell-Modus, Akzentfarbe, Farbschema + Colorpicker; Abbrechen stellt zurück.
16. Robustheit: App beenden, `db.json` mit Texteditor beschädigen, starten → Hinweis mit Pfad, Datei `db.json.corrupt-*` vorhanden; Backup wiederherstellen.
17. Speicherfehler: Datenordner schreibschützen (macOS: `chmod -w`, Windows: Eigenschaften → Schreibgeschützt bzw. Ordnerrechte), Kunde speichern → Fehlermeldung statt „Gespeichert“.
18. Windows: PDF exportieren, während dieselbe Datei im PDF-Viewer offen ist → Fehlermeldung, keine `.tmp`-Reste.
19. Update-Check (Einstellungen → App) zeigt Version 1.0.0; Signaturprüfung beim nächsten echten Release.
20. macOS: Gatekeeper öffnet die notarisierte DMG/App ohne Warnung; Windows: SmartScreen-Verhalten des NSIS-/MSI-Installers notieren.

## 6. Testbefehle

```sh
pnpm build                      # tsc + vite build — fehlerfrei
cargo test --workspace          # 29 Core-Tests + 4 Backend-Tests (+1 nur unter Windows)
INVOICES_XML_DUMP=<ordner> cargo test -p invoices-core dump   # Beispiel-XMLs für Validatoren
java -jar Mustang-CLI-2.26.0.jar --action validate --source <ordner>/invoice.xml
```
