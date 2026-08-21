# invoices — Plan

Rechnungstool für kleine Unternehmen: Kunden, Artikel, Belege, PDF, offene Posten.
Website: https://lan-solo.com/de/tools/invoices/

## Architektur

- `core/` — reine Rust-Domänenlogik, unit-getestet: Geldrechnung in Cents,
  USt-Aufstellung pro Satz (gerundet auf der summierten Basis), fortlaufende
  Nummernkreise pro Präfix und Jahr, Suche, JSON-Persistenz (atomar via tmp+rename).
- `src-tauri/` — Tauri-2-Shell: Commands (CRUD Kunden/Artikel/Belege, Festschreiben,
  Storno, Bezahlt-Status, Settings, PDF-Datei schreiben, Logo laden), Updater.
  Festgeschriebene Belege sind unveränderlich — Korrektur nur per Storno.
- `src/` — React/TypeScript (Vite, Port 1432), Terminal-Look (Mono-UI).
  PDF-Erzeugung im Frontend via jsPDF + autotable; gespeichert über Save-Dialog
  und Rust-Command `write_file`.

## Datenablage

- Belege/Kunden/Artikel/Nummernkreise: `app_data_dir()/db.json` (ein JSON, atomar geschrieben)
- Einstellungen: `app_config_dir()/settings.json`
- Kein Netz außer Update-Check (GitHub, signierte Manifeste)

## Phasen

- [x] Phase 0 — Gerüst: Tauri 2 nach screencap-Vorbild, Icons aus Website-SVG,
      Updater-Schlüsselpaar (`~/.tauri/invoices-updater.key`), CI-Workflow
- [x] Phase 1 — v0.1.1: Kunden, Artikel, Rechnungen/Gutschriften/Storno,
      Festschreiben mit Nummernkreisen, §19, PDF-Export, offene Posten,
      Hilfe (Tutorial + Handbuch), DE/EN, In-App-Updates
- [ ] Phase 2 — Angebote, Auftragsbestätigungen, Lieferscheine (Belegkette per Klick)
- [ ] Phase 3 — E-Rechnung: XRechnung (EN 16931) erzeugen, später ZUGFeRD
- [ ] Phase 4 — Wiederkehrende Rechnungen, Zahlungserinnerung & Mahnung
- [ ] Phase 5 — Auswertungen (Umsatz je Monat/Kunde), CSV-Export
- [ ] Phase 6 — später: invoices billed (12 €/Jahr) — Lizenzmodell nach
      ALL_BACKED_PLAN-Muster (signierte Offline-Lizenzen)

## Build

```sh
pnpm install && pnpm tauri dev     # Entwicklung
cargo test --workspace             # Tests (Core-Geldrechnung!)
```

Release: Tag `v*` pushen — CI baut macOS (arm64+intel), Windows, Linux und
veröffentlicht mit latest.json (Updater) und stabilen Download-Aliassen.

## Hinweis macOS

Builds sind nicht notarisiert (Beta). Meldet macOS „beschädigt“:
`xattr -dr com.apple.quarantine /Applications/invoices.app`
