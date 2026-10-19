import { invoke } from '@tauri-apps/api/core';

export type DocKind =
  | 'invoice'
  | 'creditnote'
  | 'cancellation'
  | 'quote'
  | 'orderconfirmation'
  | 'deliverynote';
export type DocStatus = 'draft' | 'open' | 'paid' | 'cancelled';

export interface Customer {
  id: string;
  name: string;
  address: string;
  email: string;
  vatId: string;
  notes: string;
  /** Leitweg-ID bzw. Käufer-Referenz (BT-10) für die E-Rechnung. */
  buyerReference: string;
  /** ISO-3166-Ländercode; leer = DE. */
  country: string;
  contact: string;
  street: string;
  postcode: string;
  city: string;
  phone: string;
  website: string;
  customerNumber: string;
}

export interface Template {
  id: string;
  name: string;
  kind: DocKind;
  items: LineItem[];
  intro: string;
  notes: string;
}

export interface Product {
  id: string;
  name: string;
  description: string;
  unit: string;
  unitPriceCents: number;
  vatRate: number;
}

export interface LineItem {
  name: string;
  description: string;
  quantity: number;
  unit: string;
  unitPriceCents: number;
  vatRate: number;
}

export interface Doc {
  id: string;
  kind: DocKind;
  number: string | null;
  customerId: string;
  customerName: string;
  customerAddress: string;
  date: string;
  dueDate: string;
  /** Leistungsdatum (ISO); leer = Belegdatum. */
  serviceDate: string;
  items: LineItem[];
  status: DocStatus;
  smallBusiness: boolean;
  intro: string;
  notes: string;
  paidAt: string | null;
  relatedId: string | null;
  createdAt: string;
}

export interface Settings {
  language: 'de' | 'en';
  companyName: string;
  companyAddress: string;
  companyEmail: string;
  companyPhone: string;
  taxNumber: string;
  vatId: string;
  bankName: string;
  iban: string;
  bic: string;
  logoPath: string;
  smallBusiness: boolean;
  defaultVatRate: number;
  paymentTermsDays: number;
  invoicePrefix: string;
  creditPrefix: string;
  cancelPrefix: string;
  quotePrefix: string;
  orderPrefix: string;
  deliveryPrefix: string;
  numberIncludeYear: boolean;
  numberDigits: number;
  numberSeparator: string;
  pdfFooter: string;
  countryCode: string;
  defaultUnit: string;
  defaultIntro: string;
  theme: 'dark' | 'light';
  accent: 'sky' | 'emerald' | 'violet' | 'amber';
  pdfLayout: 'classic' | 'modern' | 'compact' | 'terminal';
  pdfScheme: string;
  pdfC1: string;
  pdfC2: string;
  pdfC3: string;
  pdfC4: string;
  pdfShowCompanyHeader: boolean;
  autoUpdate: boolean;
  autoBackup: boolean;
  backupDir: string;
}

export interface CompanyEntry {
  id: string;
  label: string;
}

export interface CompanyRegistry {
  active: string;
  list: CompanyEntry[];
}

export interface EInvoice {
  xml: string;
  suggestedName: string;
}

export interface UpdateInfo {
  version: string;
  notes: string | null;
  date: string | null;
}

export const api = {
  listCustomers: () => invoke<Customer[]>('list_customers'),
  upsertCustomer: (customer: Customer) => invoke<void>('upsert_customer', { customer }),
  deleteCustomer: (id: string) => invoke<void>('delete_customer', { id }),
  listProducts: () => invoke<Product[]>('list_products'),
  upsertProduct: (product: Product) => invoke<void>('upsert_product', { product }),
  deleteProduct: (id: string) => invoke<void>('delete_product', { id }),
  listDocs: (query: string) => invoke<Doc[]>('list_docs', { query }),
  upsertDoc: (doc: Doc) => invoke<void>('upsert_doc', { doc }),
  finalizeDoc: (id: string) => invoke<Doc>('finalize_doc', { id }),
  setPaid: (id: string, paid: boolean) => invoke<void>('set_paid', { id, paid }),
  cancelDoc: (id: string, newId: string) => invoke<Doc>('cancel_doc', { id, newId }),
  deleteDoc: (id: string) => invoke<void>('delete_doc', { id }),
  getSettings: () => invoke<Settings>('get_settings'),
  setSettings: (s: Settings) => invoke<void>('set_settings', { new: s }),
  dataPath: () => invoke<string>('data_path'),
  startupNotice: () => invoke<string | null>('startup_notice'),
  backupNotice: () => invoke<string | null>('backup_notice'),
  writeFile: (path: string, dataBase64: string) =>
    invoke<void>('write_file', { path, dataBase64 }),
  getLogo: () => invoke<string | null>('get_logo'),
  einvoiceXml: (id: string) => invoke<EInvoice>('einvoice_xml', { id }),
  listTemplates: () => invoke<Template[]>('list_templates'),
  upsertTemplate: (template: Template) => invoke<void>('upsert_template', { template }),
  deleteTemplate: (id: string) => invoke<void>('delete_template', { id }),
  exportBackup: (path: string) => invoke<void>('export_backup', { path }),
  importBackup: (path: string) => invoke<void>('import_backup', { path }),
  listCompanies: () => invoke<CompanyRegistry>('list_companies'),
  addCompany: (id: string, label: string) => invoke<void>('add_company', { id, label }),
  switchCompany: (id: string) => invoke<void>('switch_company', { id }),
  deleteCompany: (id: string) => invoke<void>('delete_company', { id }),
  checkUpdate: () => invoke<UpdateInfo | null>('check_update'),
  installUpdate: () => invoke<void>('install_update'),
};

// --- shared helpers ----------------------------------------------------

/// Ganzzahlige Division mit kaufmännischer Rundung, halbe Cents weg von
/// null — spiegelt core::div_round (symmetrisch für Storno-Beträge).
function divRound(numerator: number, denominator: number): number {
  const q = Math.floor((Math.abs(numerator) + denominator / 2) / denominator);
  return numerator < 0 ? -q : q;
}

/// Menge auf 4 Nachkommastellen als Ganzzahl — spiegelt core::quantity_units.
function quantityUnits(quantity: number): number {
  if (!Number.isFinite(quantity)) return 0;
  const scaled = Math.abs(quantity) * 10000;
  return quantity < 0 ? -Math.round(scaled) : Math.round(scaled);
}

export function lineTotalCents(item: LineItem): number {
  return divRound(quantityUnits(item.quantity) * item.unitPriceCents, 10000);
}

export interface VatLine {
  rate: number;
  baseCents: number;
  vatCents: number;
}

export interface Totals {
  netCents: number;
  vat: VatLine[];
  grossCents: number;
}

/// Mirror of core::totals — kept in sync so the live preview matches the PDF.
export function totals(items: LineItem[], smallBusiness: boolean): Totals {
  let net = 0;
  const bases = new Map<number, number>();
  for (const item of items) {
    const line = lineTotalCents(item);
    net += line;
    bases.set(item.vatRate, (bases.get(item.vatRate) ?? 0) + line);
  }
  const vat: VatLine[] = [];
  if (!smallBusiness) {
    for (const rate of [...bases.keys()].filter((r) => r > 0).sort((a, b) => a - b)) {
      const base = bases.get(rate)!;
      vat.push({ rate, baseCents: base, vatCents: divRound(base * rate, 100) });
    }
  }
  const vatSum = vat.reduce((s, v) => s + v.vatCents, 0);
  return { netCents: net, vat, grossCents: net + vatSum };
}

export function fmtMoney(cents: number, lang: 'de' | 'en'): string {
  return new Intl.NumberFormat(lang === 'de' ? 'de-DE' : 'en-US', {
    style: 'currency',
    currency: 'EUR',
  }).format(cents / 100);
}

export function fmtDate(iso: string, lang: 'de' | 'en'): string {
  if (!iso) return '';
  const d = new Date(`${iso}T00:00:00`);
  return d.toLocaleDateString(lang === 'de' ? 'de-DE' : 'en-US', {
    day: '2-digit',
    month: '2-digit',
    year: 'numeric',
  });
}

export function todayIso(): string {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(
    d.getDate()
  ).padStart(2, '0')}`;
}

export function addDaysIso(iso: string, days: number): string {
  const d = new Date(`${iso}T00:00:00`);
  // geleertes/ungültiges Datumsfeld → kein „NaN-NaN-NaN“
  if (Number.isNaN(d.getTime())) return '';
  d.setDate(d.getDate() + days);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(
    d.getDate()
  ).padStart(2, '0')}`;
}

/// Überfällig sind nur offene Rechnungen — Gutschriften, Stornos,
/// Angebote, AB und Lieferscheine sind keine Forderungen.
export function isOverdue(doc: Doc): boolean {
  return (
    doc.kind === 'invoice' && doc.status === 'open' && !!doc.dueDate && doc.dueDate < todayIso()
  );
}

/** Vorschau des Nummernkreises, z. B. "RE-2026-0001" — spiegelt core::next_number. */
export function numberPreview(
  prefix: string,
  includeYear: boolean,
  digits: number,
  separator: string
): string {
  const counter = String(1).padStart(Math.min(Math.max(digits, 1), 8), '0');
  return includeYear
    ? `${prefix}${separator}${new Date().getFullYear()}${separator}${counter}`
    : `${prefix}${separator}${counter}`;
}

/// Parse a money input to cents — NaN-safe. Akzeptiert deutsche und
/// englische Schreibweisen: "1.234,56", "1234,56", "1,234.56", "1234.56",
/// "12,5", "-5". Stehen Punkt und Komma beide drin, ist das letzte der
/// Dezimaltrenner; ein einzelner Trenner vor genau drei Ziffern gilt als
/// Tausenderpunkt ("1.234" = 1234), sonst als Dezimaltrenner.
export function parseMoney(raw: string): number {
  let s = raw.trim().replace(/[€\s\u00a0\u202f']/g, '');
  const lastDot = s.lastIndexOf('.');
  const lastComma = s.lastIndexOf(',');
  if (lastDot >= 0 && lastComma >= 0) {
    const decimal = lastDot > lastComma ? '.' : ',';
    const thousands = decimal === '.' ? ',' : '.';
    s = s.split(thousands).join('').replace(decimal, '.');
  } else if (lastComma >= 0 || lastDot >= 0) {
    const sep = lastComma >= 0 ? ',' : '.';
    const parts = s.split(sep);
    const isThousands =
      parts.length > 2 || (parts.length === 2 && parts[1].length === 3 && sep === '.');
    s = isThousands ? parts.join('') : parts.join('.');
  }
  const value = Number(s);
  if (!Number.isFinite(value)) return 0;
  // auf ganze Cents runden (symmetrisch)
  const cents = Math.round(Math.abs(value) * 100);
  return value < 0 ? -cents : cents;
}

/// Fehler aus dem Backend (Fehlercode wie "doc-finalized" oder
/// "save-failed: …") in einen lesbaren Text übersetzen.
/// Bekannte Codes werden in der UI-Sprache gezeigt; ein Detail nach dem
/// Doppelpunkt (Betriebssystem-Meldung) steht in Klammern dahinter.
export function errText(e: unknown, errors: Record<string, string>): string {
  const raw = e instanceof Error ? e.message : String(e);
  const idx = raw.indexOf(':');
  const code = (idx >= 0 ? raw.slice(0, idx) : raw).trim();
  const known = errors[code];
  if (!known) return raw;
  const detail = idx >= 0 ? raw.slice(idx + 1).trim() : '';
  // Detail kann selbst ein Code sein („auto-backup-failed: bad-path“)
  return detail ? `${known} (${errors[detail] ?? detail})` : known;
}

/// Dateiname für den Speichern-Dialog: Pfadtrenner und unter Windows
/// verbotene Zeichen ersetzen. Sonst würde z. B. die Belegnummer
/// "RE/2026/0001" (Trennzeichen „/“) als Ordner "RE/2026" + Datei
/// "0001.pdf" gelesen.
export function safeFileName(name: string): string {
  const cleaned = name
    .replace(/[\\/:*?"<>|\u0000-\u001f]/g, '-')
    .replace(/[. ]+$/, '')
    .trim();
  return cleaned || 'beleg';
}

export function centsToInput(cents: number, lang: 'de' | 'en'): string {
  const s = (cents / 100).toFixed(2);
  return lang === 'de' ? s.replace('.', ',') : s;
}
