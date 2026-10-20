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
| geprüft (Runde 2) | `cargo audit`: rustls 0.23.43 → 0.23.46 (4 Advisories behoben); verbleibend `zip` 4.6.1 (über tauri-plugin-updater, nur `extract()` betroffen — der Updater lädt signierte DMG/MSI/NSIS-Artefakte, nicht erreichbar) und Unmaintained-Warnungen (unic-*, proc-macro-error, zbus_names) | ok / akzeptiert |

## 4. Bekannte Grenzen

- **ZUGFeRD-PDF ist nicht streng PDF/A-3b-konform:** veraPDF meldet 2 Regelverstöße (Klausel 6.2.11.4.1, Helvetica nicht eingebettet). Das eingebettete XML ist EN-16931-valide. Lösung: eigene TTF-Schrift einbetten (1.1).
- **XRechnung im engeren Sinn (KoSIT-CIUS):** Das XML nutzt die Kennung `urn:cen.eu:en16931:2017` (Factur-X/ZUGFeRD EN 16931). Behördenportale, die die XRechnung-CIUS-Kennung und deren Zusatzregeln (BR-DE-*) verlangen, können es ablehnen.
- ~~Kein eigenes Leistungsdatum / leere ApplicableHeaderTradeDelivery~~ — in Runde 2 behoben (Feld „Leistungsdatum“, BT-72). Ein Leistungs*zeitraum* (BG-14) wird weiterhin über die Positionsbeschreibung abgebildet.
- **Sprache beim Erststart:** Das Backend liest `LANG` (unter Windows und bei Finder/Dock-Start unter macOS nicht gesetzt); seit der Verifikation übernimmt das Frontend beim Erststart `navigator.language` der Webview. Auf beiden Systemen manuell prüfen (Testliste Punkt 1).
- **Kundenlöschen:** E-Mail/USt-IdNr./Leitweg-ID der E-Rechnung kommen live aus der Kundenkartei; nach dem Löschen eines Kunden fehlen sie in später exportierten E-Rechnungen (Name/Anschrift bleiben als Snapshot).
- **jsPDF-Standardschriften (WinAnsi):** Zeichen außerhalb Westeuropas (z. B. ł, ő, kyrillisch) werden im PDF falsch dargestellt.
- **Windows-Kompilat:** Seit Runde 2 läuft `cargo check --target x86_64-pc-windows-msvc --workspace --tests` lokal fehlerfrei (C-Teile mit Stub-Headern, ohne Linken). Gelinkt, ausgeführt und paketiert wird nur in der CI.
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
cargo test --workspace          # 35 Core-Tests + 13 Backend-Tests (+1 nur unter Windows)
INVOICES_XML_DUMP=<ordner> cargo test -p invoices-core dump   # Beispiel-XMLs für Validatoren
java -jar Mustang-CLI-2.26.0.jar --action validate --source <ordner>/invoice.xml
```

## 7. Verifikation (unabhängige Zweitprüfung, 2026-10-19)

Geprüft wurde der komplette Diff `f818cc3..4c3a584` plus eigene Stichproben. Selbst ausgeführt:

- `cargo test --workspace`: vor den Nachbesserungen 4 + 29 grün; danach **5 + 29 grün** (neuer Test `failed_first_save_rolls_back_to_empty_db`).
- `pnpm build` (tsc + vite): fehlerfrei. Lint: Repo hat keine ESLint-Konfiguration.
- `pnpm audit`: 29 Befunde, ausschließlich `jspdf` (2.5.2) und `dompurify` (transitiv). Erreichbarkeit nachgeprüft: die App nutzt nur `text`, `addImage` (Logo, im Backend auf PNG/JPEG-Magic-Bytes beschränkt — GIF/BMP-DoS nicht erreichbar), `getImageProperties`, `autoTable`, `output('arraybuffer')` — kein `html`, `addJS`, AcroForm, FreeText, `output('…newwindow')`. Einschätzung „Upgrade für 1.1“ bestätigt.
- IPC-Abgleich: alle 30 `invoke`-Namen und Argumentnamen in `src/api.ts` passen zu den Rust-Signaturen (camelCase → snake_case, `new`, `dataBase64`, `newId`); alle 20 Backend-Fehlercodes sind in DE und EN übersetzt; `en` ist als `typeof de` typisiert (fehlende Schlüssel wären Typfehler).
- Version 1.0.0 in `package.json`, `tauri.conf.json`, `src-tauri/Cargo.toml`, `Cargo.lock`; Anzeige über `getVersion()`. `build.yml`: nichts versionsabhängig, Signatur/Notarisierung unverändert; MSI akzeptiert 1.0.0.

Widerlegt bzw. ergänzt:

- „Nach Wiederherstellung lädt die App neu“ — es wird kein Fenster-Reload ausgelöst, sondern Einstellungen und Listen frisch vom Backend geholt; wirkt gleich (Abbrechen holt den alten Stand nicht zurück).
- Die Sprach-Voreinstellung war nicht nur unter Windows falsch: auch per Finder/Dock gestartete macOS-Apps erben kein `LANG`.

Zusätzlich behoben:

1. **Dateiname bei Trennzeichen „/“:** Mit dem angebotenen Nummernkreis-Trennzeichen „/“ wurde `RE/2026/0001.pdf` vom Speichern-Dialog als Ordner `RE/2026` + Datei `0001.pdf` gelesen (tauri-plugin-dialog `set_default_path`). Jetzt `safeFileName()` für PDF-, XML-, ZUGFeRD- und Vorschau-Export (ersetzt `\ / : * ? " < > |` und Steuerzeichen durch „-“, auch für Windows-verbotene Zeichen in Präfixen). Hilfe DE/EN ergänzt.
2. **Rollback bei fehlgeschlagenem Erst-Speichern:** `commit()` setzte den Speicher nur zurück, wenn `db.json` lesbar war. Existierte sie noch nicht (frische Installation, Datenordner nicht beschreibbar), blieb z. B. eine vergebene Nummer im Speicher. Jetzt „fehlt“ → leere Datenbank, „kaputt“ → nichts überschreiben. Neuer Rust-Test.
3. **Kaputte `companies.json`:** wurde still durch die Standard-Registry ersetzt (weitere Firmen unsichtbar). Jetzt Kopie `companies.json.corrupt-<stamp>` vor dem Ersetzen; Firmendateien bleiben ohnehin liegen.
4. **Auto-Backup blockierte den Start:** lief synchron im `setup` — ein nicht erreichbarer Netz-/UNC-Backup-Ordner (Windows: SMB-Timeout) hätte das Fenster verzögert. Läuft jetzt in einem Hintergrund-Thread.
5. **Quarantäne-Hinweis beim Firmenwechsel:** Eine beim Wechsel zur Seite gelegte, unlesbare Firmen-Datenbank wurde nicht gemeldet (Hinweis nur beim App-Start). Jetzt auch nach Wechsel/Wiederherstellung.
6. **Erststart-Sprache:** siehe oben — `navigator.language` wird beim allerersten Start übernommen (nur wenn noch kein Firmenname gesetzt und das Tutorial nie gesehen wurde).

Nur statisch geprüft (braucht einen Menschen bzw. echtes System): Punkte 4–6 oben im laufenden Tauri-Fenster, native Dialoge mit Umlaut-/Leerzeichen-/Laufwerkspfaden, Windows-Kompilat (nur CI), Mustang/veraPDF-Ergebnisse der Erstprüfung wurden nicht wiederholt (Werkzeuge nicht mehr vorhanden).

## 8. Runde 2 (2026-10-20)

Ausgangsstand `753bb3e`. Abgearbeitet: Resteliste der Runde 1 und die Fehlermuster L1–L13 aus den anderen Apps.

Behoben:

1. **L1 — `window.confirm`/`window.alert`:** tauri-plugin-dialog 2.7 ersetzt sie durch asynchrone Stubs; `if (window.confirm(…))` war damit **immer wahr** — Storno, Entwurf/Kunde/Artikel/Vorlage löschen, Firma entfernen und Backup wiederherstellen liefen auch bei „Abbrechen“; der Quarantäne-Hinweis (`window.alert`) erschien nie. Jetzt `src/dialogs.ts` (`confirm()`/`message()` des Plugins, immer mit `await`, Knöpfe mit eindeutigen Texten wie „Stornieren“/„Abbrechen“; schlägt der Dialog fehl, wird NICHT ausgeführt). Berechtigung `dialog:default` enthält `allow-message`. Rückgabewert gegen rfd 0.16 (macOS/Windows) geprüft: Rust liefert den Text des geklickten Knopfs.
2. **L2 — Backend-Fehler roh/deutsch:** Update-Fehler kamen als deutscher Klartext („Update-Prüfung fehlgeschlagen: …“) auch in der englischen Oberfläche; `write_file`/`export_backup` lieferten rohe OS-Texte. Jetzt nur Fehlercodes (`write-failed`, `read-failed`, `bad-name`, `bad-data`, `doc-bad-date`, `auto-backup-failed`, `restore-safety-failed`, `update-check-failed`, `update-failed`, `update-none`, `update-busy`), alle DE/EN übersetzt; `errText()` hängt das OS-Detail in Klammern an. Vorschau-Export zeigte `String(e)` — jetzt übersetzt. Installationsfehler im Update-Banner/Auto-Update wurden verschluckt — jetzt angezeigt.
3. **L4 — Backup mit UTF-8-BOM** (Windows-Editor) wurde als „kein gültiges Backup“ abgelehnt; jetzt akzeptiert (Test). CSV-Export gibt es nicht; `csv` wurde aus den erlaubten `write_file`-Endungen entfernt.
4. **L5 — Dateisystem:** reservierte Windows-Namen (CON, NUL, COM1 … auch mit Endung) werden im Backend abgelehnt (`bad-name`); Backup-Ordner „D:“ wird zu „D:\“, relative Backup-Ordner werden abgelehnt; `write_atomic` nutzt eindeutige Temp-Namen (parallele Exporte auf dasselbe Ziel kollidieren nicht mehr) und räumt eine halb geschriebene Temp-Datei weg; vorgeschlagene XML-Dateinamen werden auch im Backend bereinigt; Firmen-IDs werden ohne Groß-/Kleinschreibung verglichen (APFS/NTFS).
5. **L8/L9 — Hauptthread:** `write_file`, `export_backup`, `import_backup`, `get_logo` laufen als `async`-Befehle im Thread-Pool (Netzlaufwerk/gesperrte Datei friert das Fenster nicht ein). Wiederherstellung hält dabei den DB-Lock, schreibt erst die Datenbank, dann die Einstellungen — kein paralleler Befehl kann den alten Stand darüberschreiben. Doppeltes „Installieren“ (Banner + Einstellungen) wird im Backend abgewiesen.
6. **L10 — Datum:** Dateiname „Backup jetzt erstellen“ nutzte das UTC-Datum (`toISOString`) — kurz nach Mitternacht der Vortag; jetzt lokal. Geleertes Belegdatum ergab ein Fälligkeitsdatum „NaN-NaN-NaN“, und festgeschrieben wurde trotzdem (Belegdatum leer in PDF und E-Rechnung); jetzt prüft `finalize` Beleg-, Fälligkeits- und Leistungsdatum (`doc-bad-date`, inkl. Schaltjahr), es wird keine Nummer verbraucht (Tests).
7. **L11 — stille Fehler:** Auto-Backup-Fehler (Ordner nicht erreichbar) wurden verschluckt — jetzt Hinweis beim Start (`backup_notice` + Event `backup-failed`). Wiederherstellung lief weiter, wenn die Sicherheitskopie `db.json.before-restore-*` nicht angelegt werden konnte — jetzt Abbruch ohne Änderung (Test). Kunde aus dem Editor anlegen hatte kein `catch`. „Zurück zur Liste“ verwarf ungespeicherte Entwurfsänderungen ohne Rückfrage — jetzt Rückfrage. Kaputtes Logo konnte die Vorschau abstürzen lassen — jetzt Fehlermeldung.
8. **L12 — Texte:** Löschknöpfe für Kunden, Artikel, Vorlagen und Positionen hießen „Entwurf löschen“ — jetzt „Löschen“; Dateifilter „Bild“ und die Musterzeilen der Layout-Vorschau („Position A“, „119,00 €“) waren hart deutsch; „1 Belege“ → „1 Beleg“; `<html lang>` folgt der Sprache; Summenspalte bricht nicht mehr um. Hilfe DE/EN um Leistungsdatum, Rückfragen, Auto-Backup-Fehler, Sicherheitskopie, BOM und reservierte Namen ergänzt.
9. **Leistungsdatum (Rest aus Runde 1):** neues Feld für Rechnung/Gutschrift (Storno übernimmt das des Originals), leer = Belegdatum; steht auf dem PDF und als BT-72 (`ActualDeliverySupplyChainEvent`) in XRechnung/ZUGFeRD. Alte Datenbanken laden unverändert (`serde(default)`).

Geprüft ohne Befund: L3 (Erststart-Sprache, im Browser mit `navigator.language` en-US/de-DE nachgestellt), L6 (keine Tastenkürzel-Beschriftungen), L7 (keine Fremdinhalte; Vorschau ist eine lokale `blob:`-PDF, CSP `frame-src blob:`), L13 (Endpoint/Pubkey gesetzt, Versionsvergleich macht das Plugin, Fehler jetzt in UI-Sprache).

Ausgeführt:

- `cargo test --workspace`: **12 Backend- + 34 Core-Tests grün** (neu u. a. reservierte Namen, BOM-Backup, Auto-Backup-Fehler, Wiederherstellung mit/ohne Sicherheitskopie, Datumsprüfung, Leistungsdatum im XML/Storno, parallele atomare Schreibvorgänge).
- `cargo check --target x86_64-pc-windows-msvc --workspace --tests`: fehlerfrei (inkl. `#[cfg(windows)]`-Tests, die nur kompiliert, nicht ausgeführt wurden).
- `pnpm build` (tsc + vite): fehlerfrei.
- UI im Headless-Chromium (900 × 580 = Mindestfenstergröße, DE/EN × Hell/Dunkel, gemockte IPC mit Plugin-2.7-Verhalten, `window.confirm` als Falle): 51 Prüfungen je Kombination grün — Kunde anlegen, Löschen abbrechen, Rechnung mit Leistungsdatum, geleertes Datum → übersetzter Fehler ohne Nummer, Zurück mit Änderungen → Rückfrage, Vorschau, PDF-Export auf gesperrte Datei → übersetzte Meldung, Storno/Entwurf löschen mit Abbrechen und OK, Wiederherstellen ohne Sicherheitskopie, Update-Fehler, keine Überläufe, keine Fremdsprach-Reste.

Offen / nicht in Code lösbar:

- PDF/A-3b (Schrift einbetten), XRechnung-CIUS-Kennung, jsPDF-4-Upgrade — wie in Abschnitt 3/4.
- Echte Tauri-IPC end-to-end, native Dialoge (Knopftexte, Esc = Abbrechen), WebView2-Vorschau, gesperrte Dateien/AV-Scanner, UNC-/NAS-Backup-Ordner, Installer und Updater: nur auf echter Hardware prüfbar. Zusätzlich zur Testliste:
  21. Storno, Entwurf löschen, Kunde löschen, Firma entfernen, Backup wiederherstellen: jeweils **„Abbrechen“ und Esc** → nichts passiert; „Stornieren“/„Löschen“ → Aktion läuft (macOS und Windows).
  22. Auto-Backup-Ordner auf ein abgezogenes Laufwerk/aus geschaltetes NAS → beim Start Hinweis, Fenster bleibt bedienbar.
  23. Leistungsdatum setzen, festschreiben, ZUGFeRD exportieren → im Viewer/Validator BT-72 vorhanden; ohne Leistungsdatum = Belegdatum.
  24. Windows: Export als `CON.pdf` → Meldung „reserviert“.

## 9. Runde 2 – Verifikation (unabhängige Zweitprüfung, 2026-10-20)

Geprüft: Diff `753bb3e..eef87ec` auf Regressionen (plattformübergreifend, neue async-Befehle/Races) und L1–L13 an Stellen, die Runde 2 nicht genannt hatte.

Bestätigt (selbst ausgeführt bzw. im Quelltext nachgelesen):

- L1: keine `window.confirm/alert/prompt` mehr im Frontend (grep). `confirm()` aus `@tauri-apps/plugin-dialog` 2.7.2 sendet `plugin:dialog|message` mit `{OkCancelCustom:[ok, cancel]}` und vergleicht das Ergebnis mit dem OK-Text; Rust (`tauri-plugin-dialog` 2.7.2, `rfd` 0.16 mit `common-controls-v6`) liefert unter macOS/Windows den Text des geklickten Knopfs, Esc/Schließen ergibt „Cancel“ bzw. den Abbrechen-Text — beides ≠ OK-Text. OK- und Abbrechen-Texte sind in DE und EN bei allen Rückfragen verschieden. `dialog:default` enthält `allow-message`.
- Zusätzlich im Headless-Chromium (Mock wie Runde 2, `window.confirm` als Falle) die von Runde 2 nicht geprüften Rückfragen: Artikel löschen, Vorlage löschen, Firma entfernen (je Abbrechen + OK) und Backup wiederherstellen mit Abbrechen (kein `import_backup`) — 13/13 je Kombination DE/EN × Hell/Dunkel grün; die 51 Prüfungen aus Runde 2 laufen unverändert 51/51.
- Alle Fehlercodes des Backends haben DE- und EN-Texte (Abgleich per Skript). Lock-Reihenfolge der Wiederherstellung (DB-Pfad freigegeben, dann DB-Lock, dann Einstellungen) kann mit `persist()` nicht verklemmen.
- `cargo test --workspace` (12 Backend + 35 Core), `pnpm build`, Windows-Typecheck `cargo check --target x86_64-pc-windows-msvc --workspace --tests` (nach erzwungener Neuprüfung beider Crates) fehlerfrei. `cargo audit`: nur noch die bekannten `zip`-Advisories (über tauri-plugin-updater) und Unmaintained-Warnungen; `pnpm audit`: unverändert 29 (jsPDF/DOMPurify, siehe Abschnitt 8).

Behoben:

1. **L4 — BOM auch in den eigenen Datendateien:** `db.json`, Einstellungen und `companies.json` wurden mit `serde_json::from_str` gelesen — mit dem Windows-Editor gespeichert (UTF-8 mit BOM) galt die **Datenbank als kaputt, wurde in Quarantäne gelegt und die App startete leer**; Einstellungen fielen auf Standard zurück. Jetzt `invoices_core::strip_bom` in allen drei Ladern (Test `db_with_utf8_bom_loads_instead_of_quarantine`).
2. **L2 — Detail-Code unübersetzt:** „Auto-Backup fehlgeschlagen (bad-path)“ zeigte den rohen Code als Detail (relativer Backup-Ordner). `errText()` übersetzt jetzt auch ein Detail, das selbst ein bekannter Code ist (im UI-Test DE/EN geprüft).
3. **L11 — falsche Rückfrage:** Scheiterte das Festschreiben nach dem automatischen Speichern (z. B. `doc-bad-date`), fragte „Zurück zur Liste“ nach „ungespeicherten Änderungen“, obwohl alles gespeichert war — Vergleichsbasis war der Stand beim Öffnen. Jetzt ist die Basis der zuletzt gespeicherte Stand (App setzt ihn nach dem Speichern). UI-Test: unberührter neuer/wieder geöffneter Entwurf und gespeichert-aber-nicht-festgeschrieben → keine Rückfrage; geänderter Entwurf → Rückfrage. Vor dem Fix schlugen diese Prüfungen nachweislich fehl.

Bewusst belassen (gering, dokumentiert):

- Scheitert beim Wiederherstellen nach der Datenbank das Schreiben der Einstellungen, ist die DB schon ersetzt (Sicherheitskopie `db.json.before-restore-*` liegt daneben, Fehler wird angezeigt).
- `import_backup` läuft jetzt im Thread-Pool, `switch_company` auf dem Hauptthread; ein Firmenwechsel *während* einer laufenden Wiederherstellung könnte Einstellungen und DB verschiedenen Firmen zuordnen. Über die Oberfläche praktisch nicht auslösbar (beide im selben Einstellungsdialog, Wiederherstellen schließt ihn).
- Auto-Backup-Fehler kann im ungünstigen Timing doppelt erscheinen (nativer Hinweis + Toast) — nie gar nicht.
- Ein Backup-Ordner `\backups` (laufwerksrelativ ohne Buchstaben) gilt unter Windows jetzt als relativ und wird gemeldet statt still auf dem aktuellen Laufwerk angelegt.

Nur auf echter Hardware prüfbar (zusätzlich zu Abschnitt 5 Punkte 1–20 und Abschnitt 8 Punkte 21–24):

25. Windows: `db.json` im Editor öffnen, speichern (UTF-8 mit BOM), App starten → Daten da, kein Quarantäne-Hinweis.
26. Windows: native Rückfragen zeigen die eigenen Knopftexte („Stornieren“/„Abbrechen“) und Esc bricht ab (rfd nutzt mit `common-controls-v6` den TaskDialog; das Standard-Manifest von tauri-build 2.6.3 bindet Common Controls 6.0 ein — im Quelltext geprüft, nicht ausgeführt).


## 10. Runde 3 (2026-10-20, Abschluss)

Ausgangsstand `c705845`. Schwerpunkt: jede Faktenaussage aus Hilfe (DE/EN), README und In-App-Texten gegen den Code geprüft; End-to-end-Journeys im Headless-Chromium; Restposten PDF/A, jsPDF/DOMPurify, Teil-Wiederherstellung.

Falsche Aussagen — Code korrigiert:

1. **„Beim Festschreiben wird die Kunden-Anschrift eingefroren“** stimmte nicht: Name/Anschrift wurden beim *Auswählen* des Kunden im Entwurf kopiert. Wer die Anschrift danach in der Kundenkartei korrigierte, bekam beim Festschreiben die alte. Jetzt übernimmt „Festschreiben“ Name und Anschrift aus dem aktuellen Kundenstand (`App.tsx` `finalize`). Journey-Test schlug vor dem Fix nachweislich fehl.
2. **Dashboard-Umsatz** (Hilfe: „Rechnungen minus Gutschriften und Stornos“): Eine stornierte Rechnung zählte 0 **und** ihre Stornorechnung wurde abgezogen — Umsatz und Top-Kunden wurden um den Betrag zu niedrig (negativ). Jetzt zählt die stornierte Rechnung weiter, Storno hebt sie auf (Netto 0).
3. **Artikel-Preis ohne Blur verloren** (Hilfe/Runde 1: „Preise … übernimmt der Editor auch direkt nach dem Tippen“ — galt nur für den Beleg-Editor): Im Artikel-Dialog ging ein getippter Preis beim direkten Klick auf „Speichern“ verloren (0 €). Jetzt wird der Text beim Speichern geparst. Vorher nachweislich 0.
4. **Fälligkeit von Gutschrift/AB/Lieferschein** folgte einem geänderten Belegdatum nicht (blieb auf dem Anlagedatum, konnte vor dem Belegdatum liegen und ging so in die E-Rechnung der Gutschrift). Jetzt = Belegdatum, wie beim Anlegen.

Falsche/ungenaue Aussagen — Text korrigiert (DE + EN gleichlautend):

- Kunden: „mehrzeilig oder strukturiert“ → es gibt nur strukturierte Felder (Straße, PLZ, Ort, Land); alte mehrzeilige Anschriften werden beim Öffnen übernommen. Ergänzt: E-Mail/USt-IdNr./Leitweg-ID kommen live aus der Kartei.
- „Als Rechnung übernehmen“: „Kunde, Positionen und Texte“ → Einleitungstext ja, interne Notiz nein.
- Rückfragen: Knopfliste um „Verwerfen“/„Entfernen“ (Discard/Remove) ergänzt.
- Updates: „Vor der Installation siehst du das Changelog“ → nur unter Einstellungen → App; der Banner zeigt nur die Version.
- E-Rechnung (Hilfe + Einstellungen-Hinweis): Aktionen gibt es für Rechnung, Gutschrift **und** Stornorechnung, nicht für „jeden festgeschriebenen Beleg“ (Angebot/AB/Lieferschein haben keine).
- Layout „Modern“: Kopfband in der Akzentfarbe des **PDF-Farbschemas** (nicht der App-Akzentfarbe).
- Datenablage: weitere Unternehmen liegen in Unterordnern `company-…`.

Geprüft und zutreffend (Auswahl): 20 Farbschemata, 4 Akzentfarben, Stellen 3–6, Trennzeichen -,/,ohne; Präfixe RE/GS/ST/AN/AB/LS; Zahlungsziel/Standard-USt/Einheit/Einleitung nur für neue Rechnungen; §19 je Beleg eingefroren; Storno nur für offene/bezahlte Rechnungen, Jahr des Erstellungstags, Leistungsdatum vom Original; Storno-Preis negiert → im XML negative Menge (BR-27); Auto-Backup behält 10; Rename-Wiederholung bei gesperrter Datei (5 Versuche, Temp-Datei wird entfernt); Notiz erscheint nicht auf dem PDF; Lieferschein ohne Preise; Fällige Rechnungen (6), Top-5-Kunden, 12 Monate; Update-Endpoint GitHub mit Pubkey; Identifier `com.lan-solo.invoices`.

Restposten:

- **Teil-Wiederherstellung:** Scheitert nach dem Schreiben der Datenbank das Speichern der Einstellungen, wird die Datenbank jetzt auf den vorherigen Stand zurückgeschrieben (Speicher + Platte) → `restore-rolled-back`, „nichts verändert“. Scheitert auch der Rückweg → `restore-partial` (Hinweis auf `db.json.before-restore-…`), die Oberfläche lädt die Listen neu. Beide Codes DE/EN übersetzt. Neuer Rust-Test `restore_rolls_back_db_when_settings_cannot_be_written`.
- **PDF/A-3b:** Schriften lassen sich mit dem vorhandenen Stand **nicht** einbetten — im Repo und im Lockfile gibt es keine TTF/OTF (`@pdf-lib/standard-fonts` enthält nur AFM-Metriken, `fontkit` fehlt). Eine frei lizenzierte Schrift (z. B. DejaVu/Liberation/Noto) müsste als neues Asset aufgenommen und mit `addFont` (Identity-H) eingebunden werden → Entscheidung des Inhabers, 1.1. **Behoben** wurde die Metadaten-Konsistenz (ISO 19005-3, 6.6.2.3): jsPDF schrieb `Producer (jsPDF 2.5.2)` und ein eigenes `CreationDate` ins Info-Dictionary, das XMP hatte kein `pdf:Producer` und ein Datum mit Millisekunden. Jetzt setzen `embedZugferd` Info-Title/Producer/Creator/CreationDate/ModDate und XMP (`pdf:Producer`, `xmp:CreateDate` sekundengenau UTC) auf identische Werte. Im exportierten ZUGFeRD-PDF (UI-Harness, echte Bytes) geprüft: Info-Producer = XMP-Producer, Info-CreationDate = XMP-CreateDate, `/AF` + `factur-x.xml`, `GTS_PDFA1`-OutputIntent. veraPDF/Mustang wurden **nicht** erneut ausgeführt (nicht installiert).
- **jsPDF 2.5.2 / DOMPurify:** Es gibt kein Patch-/Minor-Update, das Befunde schließt — jsPDF-Fixes ab 3.0.5 bzw. 4.2.1 (Major, verlangt jspdf-autotable 5), DOMPurify-Fixes nur in 3.x, jsPDF 2.5.2 verlangt `^2.5.4`. `pnpm audit` unverändert 29; Erreichbarkeit wie in Abschnitt 9 (DOMPurify liegt nur als nachgeladener Chunk für das ungenutzte `jsPDF.html()` im Bundle). Bleibt für 1.1.

End-to-end-Journeys (UI-Mock im Headless-Chromium, DE/EN × Hell/Dunkel; „Neustart“ = Seite neu laden, Mock-Datenbank und Einstellungen überleben in `sessionStorage`; Mock wie Runde 2 mit Plugin-2.7-Verhalten, `window.confirm` als Falle, Storno mit negiertem Preis wie `core::workflow::cancel`):

1. Erststart: `navigator.language` de-DE/en-US → Sprache, Tutorial erscheint.
2. Briefkopf speichern → Neustart → Firmenname da, Tutorial nicht erneut.
3. Kunde mit Straße/PLZ/Ort → Anschrift zusammengesetzt; Artikel mit „1.234,50“/„1,234.50“ ohne Blur gespeichert.
4. Rechnung: Artikel + freie Position, Menge „2,5“ ohne Blur, Entwurf speichern → **Neustart mitten in der Arbeit** → Entwurf da; Kundenanschrift ändern → festschreiben → RE-JJJJ-0001 mit neuer Anschrift, Titel zeigt Nummer.
5. Export: Speichern-Dialog abgebrochen (nichts geschrieben, kein Fehler), Platte voll (os error 28), schreibgeschützter Ordner (os error 13), Erfolg; XML; ZUGFeRD mit Metadatenprüfung.
6. Bezahlt → offen → Storno → ST-JJJJ-0001; Dashboard-Umsatz vorher > 0, nachher 0,00 €.
7. Angebot („Gültig bis“) festschreiben → AN-JJJJ-0001 → „Als Rechnung übernehmen“ → Einleitung übernommen, Notiz leer, Entwurf mit Herkunft.
8. Gutschrift: Belegdatum ändern → Fälligkeit folgt.
9. Vorlage speichern, daraus Entwurf (unberührt → „Zurück“ ohne Rückfrage), löschen mit Abbrechen/OK.
10. Wiederherstellen: `restore-rolled-back` und `restore-partial` übersetzt, Dialog bleibt offen.

Ergebnis: Journeys 47/47 je Kombination (vor den Fixes 9 Prüfungen rot), Regression aus Runde 2 51/51 je Kombination.

Ausgeführt: `cargo test --workspace` (13 Backend + 35 Core), `cargo clippy --workspace --all-targets` (nur die vorbestehenden Stil-Warnungen, keine neuen), Windows-Typecheck `cargo check --target x86_64-pc-windows-msvc --workspace --tests`, `pnpm build`, `cargo audit` (nur bekannte `zip`-Advisories über tauri-plugin-updater + Unmaintained), `pnpm audit` (29, s. o.).

Nur auf echter Hardware prüfbar (zusätzlich zu Punkten 1–26):

27. Kunde in Rechnungsentwurf wählen, Anschrift in der Kundenkartei ändern, Entwurf festschreiben → PDF zeigt neue Anschrift.
28. Artikel anlegen, Preis tippen und sofort „Speichern“ klicken (macOS-WebKit und WebView2) → Preis erhalten.
29. ZUGFeRD-PDF mit veraPDF (PDF/A-3b) prüfen: erwartet nur noch die Schrift-Regeln (6.2.11.4.1), keine Metadaten-Regeln (6.6.2.3.x); Mustang: XML valide.
30. Wiederherstellen, während die Einstellungsdatei schreibgeschützt ist (macOS `chmod -w settings*.json` bzw. Windows Schreibschutz) → Meldung „zurückgesetzt … nichts verändert“, Daten unverändert.
