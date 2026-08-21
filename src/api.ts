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

export function lineTotalCents(item: LineItem): number {
  return Math.round(item.quantity * item.unitPriceCents);
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
      vat.push({ rate, baseCents: base, vatCents: Math.round((base * rate) / 100) });
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
  d.setDate(d.getDate() + days);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(
    d.getDate()
  ).padStart(2, '0')}`;
}

export function isOverdue(doc: Doc): boolean {
  return doc.status === 'open' && !!doc.dueDate && doc.dueDate < todayIso();
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

/// Parse a money input ("1.234,56" or "1234.56") to cents — NaN-safe.
export function parseMoney(raw: string): number {
  const cleaned = raw
    .trim()
    .replace(/[€\s]/g, '')
    .replace(/\.(?=\d{3}(\D|$))/g, '')
    .replace(',', '.');
  const value = Number(cleaned);
  return Number.isFinite(value) ? Math.round(value * 100) : 0;
}

export function centsToInput(cents: number, lang: 'de' | 'en'): string {
  const s = (cents / 100).toFixed(2);
  return lang === 'de' ? s.replace('.', ',') : s;
}
