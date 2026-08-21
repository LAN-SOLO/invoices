import { useMemo, useState } from 'react';
import {
  Customer,
  Doc,
  LineItem,
  Product,
  Settings,
  addDaysIso,
  api,
  centsToInput,
  fmtMoney,
  lineTotalCents,
  parseMoney,
  totals,
} from '../api';
import { Dict, Lang } from '../i18n';
import { IconPlus, IconTrash } from '../icons';
import { CustomerModal } from './Modals';

// Beleg-Editor: Entwürfe sind voll editierbar, festgeschriebene Belege
// öffnen als read-only Ansicht (GoBD-Denke — korrigiert wird per Storno).

interface Props {
  doc: Doc;
  customers: Customer[];
  products: Product[];
  settings: Settings;
  t: Dict;
  lang: Lang;
  onClose: () => void;
  onSaveDraft: (doc: Doc) => void;
  onFinalize: (doc: Doc) => void;
  onPdf: (doc: Doc) => void;
  onPreview: (doc: Doc) => void;
  onSaveTemplate: (doc: Doc) => void;
}

export function DocEditor({
  doc,
  customers,
  products,
  settings,
  t,
  lang,
  onClose,
  onSaveDraft,
  onFinalize,
  onPdf,
  onPreview,
  onSaveTemplate,
}: Props) {
  const readOnly = doc.status !== 'draft';
  const [d, setD] = useState<Doc>({ ...doc, items: doc.items.map((i) => ({ ...i })) });
  const [newCust, setNewCust] = useState<Customer | null>(null);
  // Mengen/Preise als Strings editieren — geparst wird bei Blur/Save.
  const [qtyStr, setQtyStr] = useState<string[]>(
    doc.items.map((i) => String(i.quantity).replace('.', lang === 'de' ? ',' : '.'))
  );
  const [priceStr, setPriceStr] = useState<string[]>(
    doc.items.map((i) => centsToInput(i.unitPriceCents, lang))
  );

  const sums = useMemo(() => totals(d.items, d.smallBusiness), [d.items, d.smallBusiness]);

  const set = <K extends keyof Doc>(key: K, value: Doc[K]) =>
    setD((prev) => ({ ...prev, [key]: value }));

  const setItem = (index: number, patch: Partial<LineItem>) =>
    setD((prev) => ({
      ...prev,
      items: prev.items.map((item, i) => (i === index ? { ...item, ...patch } : item)),
    }));

  const parseQty = (raw: string): number => {
    const value = Number(raw.trim().replace(',', '.'));
    return Number.isFinite(value) && value > 0 ? value : 1;
  };

  const addItem = (product?: Product) => {
    const item: LineItem = product
      ? {
          name: product.name,
          description: product.description,
          quantity: 1,
          unit: product.unit,
          unitPriceCents: product.unitPriceCents,
          vatRate: product.vatRate,
        }
      : {
          name: '',
          description: '',
          quantity: 1,
          unit: settings.defaultUnit || 'Stk',
          unitPriceCents: 0,
          vatRate: settings.defaultVatRate,
        };
    setD((prev) => ({ ...prev, items: [...prev.items, item] }));
    setQtyStr((prev) => [...prev, '1']);
    setPriceStr((prev) => [...prev, centsToInput(item.unitPriceCents, lang)]);
  };

  const removeItem = (index: number) => {
    setD((prev) => ({ ...prev, items: prev.items.filter((_, i) => i !== index) }));
    setQtyStr((prev) => prev.filter((_, i) => i !== index));
    setPriceStr((prev) => prev.filter((_, i) => i !== index));
  };

  const pickCustomer = (id: string) => {
    const customer = customers.find((c) => c.id === id);
    setD((prev) => ({
      ...prev,
      customerId: id,
      customerName: customer?.name ?? '',
      customerAddress: customer?.address ?? '',
    }));
  };

  const kindLabel: Record<Doc['kind'], string> = {
    invoice: t.kindInvoice,
    creditnote: t.kindCreditNote,
    cancellation: t.kindCancellation,
    quote: t.kindQuote,
    orderconfirmation: t.kindOrder,
    deliverynote: t.kindDelivery,
  };
  const title =
    doc.status === 'draft'
      ? kindLabel[d.kind]
      : `${kindLabel[d.kind]} ${d.number ?? ''}`;
  // Fällig/Gültig nur dort, wo es Bedeutung hat
  const dueLabel =
    d.kind === 'invoice' ? t.dueDate : d.kind === 'quote' ? t.validUntil : null;

  return (
    <div className="editor-wrap">
      <div className="editor-head">
        <button className="ghost" onClick={onClose}>
          ← {t.back}
        </button>
        <span className="editor-title">{title}</span>
        {doc.status === 'draft' && <span className="chip mini">{t.editorDraft}</span>}
        <span className="spacer" />
        <button className="ghost" onClick={() => onPreview(d)}>
          {t.actPreview}
        </button>
        {readOnly && (
          <button className="primary" onClick={() => onPdf(d)}>
            {t.actPdf}
          </button>
        )}
      </div>

      <div className="editor-body">
        <div className="editor-grid">
          <label className="field">
            <span>{t.customer}</span>
            <div className="row2">
              <select
                disabled={readOnly}
                value={d.customerId}
                onChange={(e) => pickCustomer(e.target.value)}
              >
                <option value="">{t.chooseCustomer}</option>
                {customers.map((c) => (
                  <option key={c.id} value={c.id}>
                    {c.name}
                  </option>
                ))}
              </select>
              {!readOnly && (
                <button
                  title={t.newCustomer}
                  onClick={() =>
                    setNewCust({
                      id: crypto.randomUUID(),
                      name: '',
                      address: '',
                      email: '',
                      vatId: '',
                      notes: '',
                      buyerReference: '',
                      country: '',
                      contact: '',
                      street: '',
                      postcode: '',
                      city: '',
                      phone: '',
                      website: '',
                      customerNumber: '',
                    })
                  }
                >
                  <IconPlus />
                </button>
              )}
            </div>
          </label>
          <label className="field">
            <span>{t.date}</span>
            <input
              type="date"
              disabled={readOnly}
              value={d.date}
              onChange={(e) => {
                const date = e.target.value;
                setD((prev) => ({
                  ...prev,
                  date,
                  dueDate:
                    prev.kind === 'invoice' || prev.kind === 'quote'
                      ? addDaysIso(date, settings.paymentTermsDays)
                      : prev.dueDate,
                }));
              }}
            />
          </label>
          {dueLabel ? (
            <label className="field">
              <span>{dueLabel}</span>
              <input
                type="date"
                disabled={readOnly}
                value={d.dueDate}
                onChange={(e) => set('dueDate', e.target.value)}
              />
            </label>
          ) : (
            <span />
          )}
        </div>

        {d.customerAddress && (
          <div className="addr-preview">
            {[d.customerName, ...d.customerAddress.split('\n')].filter(Boolean).join(' · ')}
          </div>
        )}

        <label className="field">
          <span>{t.intro}</span>
          <textarea
            rows={2}
            disabled={readOnly}
            placeholder={t.introPlaceholder}
            value={d.intro}
            onChange={(e) => set('intro', e.target.value)}
          />
        </label>

        <div className="fieldlabel">{t.positions}</div>
        <table className="items">
          <thead>
            <tr>
              <th className="w-name">{t.posName}</th>
              <th className="w-qty">{t.posQty}</th>
              <th className="w-unit">{t.posUnit}</th>
              <th className="w-price">{t.posPrice}</th>
              <th className="w-vat">{t.posVat}</th>
              <th className="w-total">{t.posTotal}</th>
              {!readOnly && <th className="w-del" />}
            </tr>
          </thead>
          <tbody>
            {d.items.map((item, i) => (
              <tr key={i}>
                <td>
                  <input
                    type="text"
                    disabled={readOnly}
                    value={item.name}
                    placeholder={t.posName}
                    onChange={(e) => setItem(i, { name: e.target.value })}
                  />
                  <input
                    type="text"
                    className="desc"
                    disabled={readOnly}
                    value={item.description}
                    placeholder={t.posDescription}
                    onChange={(e) => setItem(i, { description: e.target.value })}
                  />
                </td>
                <td>
                  <input
                    type="text"
                    disabled={readOnly}
                    value={qtyStr[i] ?? ''}
                    onChange={(e) =>
                      setQtyStr((prev) => prev.map((s, j) => (j === i ? e.target.value : s)))
                    }
                    onBlur={() => setItem(i, { quantity: parseQty(qtyStr[i] ?? '1') })}
                  />
                </td>
                <td>
                  <input
                    type="text"
                    disabled={readOnly}
                    value={item.unit}
                    onChange={(e) => setItem(i, { unit: e.target.value })}
                  />
                </td>
                <td>
                  <input
                    type="text"
                    disabled={readOnly}
                    value={priceStr[i] ?? ''}
                    onChange={(e) =>
                      setPriceStr((prev) => prev.map((s, j) => (j === i ? e.target.value : s)))
                    }
                    onBlur={() => {
                      const cents = parseMoney(priceStr[i] ?? '0');
                      setItem(i, { unitPriceCents: cents });
                      setPriceStr((prev) =>
                        prev.map((s, j) => (j === i ? centsToInput(cents, lang) : s))
                      );
                    }}
                  />
                </td>
                <td>
                  {d.smallBusiness ? (
                    <span className="dim">—</span>
                  ) : (
                    <select
                      disabled={readOnly}
                      value={item.vatRate}
                      onChange={(e) => setItem(i, { vatRate: Number(e.target.value) })}
                    >
                      <option value={19}>19 %</option>
                      <option value={7}>7 %</option>
                      <option value={0}>0 %</option>
                    </select>
                  )}
                </td>
                <td className="num">{fmtMoney(lineTotalCents(item), lang)}</td>
                {!readOnly && (
                  <td>
                    <button className="icon danger" title={t.actDelete} onClick={() => removeItem(i)}>
                      <IconTrash />
                    </button>
                  </td>
                )}
              </tr>
            ))}
          </tbody>
        </table>

        {!readOnly && (
          <div className="items-actions">
            <button onClick={() => addItem()}>
              <IconPlus /> {t.addPosition}
            </button>
            {products.length > 0 && (
              <select
                value=""
                onChange={(e) => {
                  const product = products.find((p) => p.id === e.target.value);
                  if (product) addItem(product);
                }}
              >
                <option value="">{t.fromProduct}</option>
                {products.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.name} — {fmtMoney(p.unitPriceCents, lang)}
                  </option>
                ))}
              </select>
            )}
          </div>
        )}

        <div className="sums">
          <div>
            <span>{t.netSum}</span>
            <span className="num">{fmtMoney(sums.netCents, lang)}</span>
          </div>
          {sums.vat.map((v) => (
            <div key={v.rate}>
              <span>
                {t.vatSum} {v.rate} %
              </span>
              <span className="num">{fmtMoney(v.vatCents, lang)}</span>
            </div>
          ))}
          <div className="grand">
            <span>{t.grossSum}</span>
            <span className="num">{fmtMoney(sums.grossCents, lang)}</span>
          </div>
          {d.smallBusiness && <div className="note">{t.smallBusinessActive}</div>}
        </div>

        <label className="field">
          <span>{t.notes}</span>
          <textarea
            rows={2}
            disabled={readOnly}
            value={d.notes}
            onChange={(e) => set('notes', e.target.value)}
          />
        </label>

        {!readOnly && (
          <>
            <div className="note">{t.finalizeHint}</div>
            <div className="btnrow">
              <button className="ghost" onClick={() => onSaveTemplate(d)}>
                {t.saveAsTemplate}
              </button>
              <button onClick={() => onSaveDraft(d)}>{t.saveDraft}</button>
              <button className="primary" onClick={() => onFinalize(d)}>
                {t.finalize}
              </button>
            </div>
          </>
        )}
      </div>

      {newCust && (
        <CustomerModal
          customer={newCust}
          t={t}
          onClose={() => setNewCust(null)}
          onSave={(c) => {
            api.upsertCustomer(c).then(() => {
              setD((prev) => ({
                ...prev,
                customerId: c.id,
                customerName: c.name,
                customerAddress: c.address,
              }));
              setNewCust(null);
            });
          }}
        />
      )}
    </div>
  );
}
