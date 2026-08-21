# invoices.

Rechnungstool für kleine Unternehmen — schreiben, senden, abhaken.

- **Belege:** Rechnungen, Gutschriften und Storno — Entwürfe frei, festgeschrieben unveränderlich, korrigiert wird per Storno.
- **Stammdaten:** Kunden und Artikel einmal anlegen, überall einfügen.
- **Steuern, richtig:** USt 19/7/0 %, Kleinunternehmerregelung nach §19 UStG, fortlaufende Nummernkreise pro Jahr.
- **PDF:** A4-Beleg mit Briefkopf, Logo, USt-Aufstellung und Bankverbindung in der Fußzeile.
- **Offene Posten:** offen, überfällig, bezahlt — inklusive Summe der Außenstände.

DE/EN, signierte In-App-Updates. Alles lokal — kein Konto, keine Cloud, keine Telemetrie.

## Entwicklung

```sh
pnpm install
pnpm tauri dev
cargo test -p invoices-core
```

## Release-Build (lokal)

```sh
TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.tauri/invoices-updater.key)" \
TAURI_SIGNING_PRIVATE_KEY_PASSWORD="" \
pnpm tauri build --bundles app,dmg
```

Details und Roadmap: `INVOICES_PLAN.md`.
