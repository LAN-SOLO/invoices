# invoices.

Rechnungstool für kleine Unternehmen — schreiben, senden, abhaken. Version 1.0.

- **Belege:** Rechnungen, Gutschriften, Storno, Angebote, Auftragsbestätigungen und Lieferscheine — Entwürfe frei, festgeschrieben unveränderlich, korrigiert wird per Storno. Angebot/AB per Klick zur Rechnung, Vorlagen für Wiederkehrendes.
- **Stammdaten:** Kunden (strukturierte Anschrift, Leitweg-ID) und Artikel einmal anlegen, überall einfügen.
- **Steuern, richtig:** USt 19/7/0 % in ganzzahliger Cent-Rechnung, Kleinunternehmerregelung nach §19 UStG, fortlaufende Nummernkreise (mit/ohne Jahr, Stellen, Trennzeichen), Leistungsdatum (§14 UStG, leer = Belegdatum).
- **PDF:** A4-Beleg mit Briefkopf, Logo, USt-Aufstellung und Bankverbindung — vier Layouts, 20 Farbschemata, Live-Vorschau.
- **E-Rechnung:** XRechnung-/Factur-X-XML (CII, EN 16931) und ZUGFeRD-Hybrid-PDF.
- **Überblick:** Dashboard mit offenen Posten, Überfälligem, Monatsumsatz und Top-Kunden.
- **Mehrere Unternehmen**, Backups (manuell + automatisch beim Start), Hell/Dunkel.

DE/EN, signierte In-App-Updates. Alles lokal — kein Konto, keine Cloud, keine Telemetrie.

## Entwicklung

```sh
pnpm install
pnpm tauri dev
pnpm build              # Typecheck + Frontend-Build
cargo test --workspace  # Core (Geld, Nummernkreise, Workflow, E-Rechnung) + Backend-Helfer (Pfade, Backup, Wiederherstellung)
```

Audit- und Teststand zur 1.0: `AUDIT-1.0.md` (inkl. Runde 2 und deren Verifikation).

JSON-Dateien (Datenbank, Einstellungen, Firmenliste, Backups) werden mit und ohne UTF-8-BOM gelesen (`invoices_core::strip_bom`) — der Windows-Editor speichert mit BOM.

Rückfragen und Hinweise laufen ausschließlich über `src/dialogs.ts` (tauri-plugin-dialog, `await`) — nie `window.confirm/alert/prompt`, die das Plugin durch asynchrone Stubs ersetzt.

## Release-Build (lokal)

```sh
TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.tauri/invoices-updater.key)" \
TAURI_SIGNING_PRIVATE_KEY_PASSWORD="" \
pnpm tauri build --bundles app,dmg
```

Details und Roadmap: `INVOICES_PLAN.md`.
