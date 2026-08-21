import { useState } from 'react';
import { Customer, Product, Template, centsToInput, parseMoney } from '../api';
import { Dict, Lang } from '../i18n';

/// Alt-Daten (nur mehrzeilige Anschrift) in die strukturierten Felder
/// heben: Zeile "PLZ Ort" wird erkannt, die erste andere Zeile wird Straße.
function migrateCustomer(c: Customer): Customer {
  if (c.street || c.postcode || c.city || !c.address.trim()) return c;
  const out = { ...c };
  for (const line of c.address.split('\n').map((l) => l.trim()).filter(Boolean)) {
    const m = line.match(/^(\d{4,5})\s+(.+)$/);
    if (m && !out.postcode) {
      out.postcode = m[1];
      out.city = m[2];
    } else if (!out.street) {
      out.street = line;
    }
  }
  return out;
}

/// Strukturierte Felder zur Anschrift für PDF-Snapshot und E-Rechnung
/// zusammensetzen.
export function composeAddress(c: Customer): string {
  const lines = [c.street.trim(), `${c.postcode.trim()} ${c.city.trim()}`.trim()];
  if (c.country.trim() && c.country.trim().toUpperCase() !== 'DE') {
    lines.push(c.country.trim().toUpperCase());
  }
  const composed = lines.filter(Boolean).join('\n');
  return composed || c.address;
}

export function CustomerModal({
  customer,
  t,
  onClose,
  onSave,
}: {
  customer: Customer;
  t: Dict;
  onClose: () => void;
  onSave: (c: Customer) => void;
}) {
  const [c, setC] = useState<Customer>(() => migrateCustomer({ ...customer }));
  const set = <K extends keyof Customer>(key: K, value: Customer[K]) =>
    setC((prev) => ({ ...prev, [key]: value }));

  return (
    <div className="overlay" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <h2>{c.name.trim() ? c.name : t.newCustomer}</h2>
        <div className="row2">
          <label className="field grow1">
            <span>{t.custName}</span>
            <input type="text" value={c.name} onChange={(e) => set('name', e.target.value)} />
          </label>
          <label className="field" style={{ width: 150 }}>
            <span>{t.custNumber}</span>
            <input
              type="text"
              value={c.customerNumber}
              onChange={(e) => set('customerNumber', e.target.value)}
            />
          </label>
        </div>
        <label className="field">
          <span>{t.custContact}</span>
          <input type="text" value={c.contact} onChange={(e) => set('contact', e.target.value)} />
        </label>
        <label className="field">
          <span>{t.custStreet}</span>
          <input type="text" value={c.street} onChange={(e) => set('street', e.target.value)} />
        </label>
        <div className="row2">
          <label className="field" style={{ width: 110 }}>
            <span>{t.custPostcode}</span>
            <input
              type="text"
              value={c.postcode}
              onChange={(e) => set('postcode', e.target.value)}
            />
          </label>
          <label className="field grow1">
            <span>{t.custCity}</span>
            <input type="text" value={c.city} onChange={(e) => set('city', e.target.value)} />
          </label>
        </div>
        <div className="row2">
          <label className="field grow1">
            <span>{t.custEmail}</span>
            <input type="text" value={c.email} onChange={(e) => set('email', e.target.value)} />
          </label>
          <label className="field grow1">
            <span>{t.custPhone}</span>
            <input type="text" value={c.phone} onChange={(e) => set('phone', e.target.value)} />
          </label>
        </div>
        <label className="field">
          <span>{t.custWebsite}</span>
          <input type="text" value={c.website} onChange={(e) => set('website', e.target.value)} />
        </label>
        <label className="field">
          <span>{t.custVatId}</span>
          <input type="text" value={c.vatId} onChange={(e) => set('vatId', e.target.value)} />
        </label>
        <div className="row2">
          <label className="field grow1">
            <span>{t.custBuyerRef}</span>
            <input
              type="text"
              value={c.buyerReference}
              onChange={(e) => set('buyerReference', e.target.value)}
            />
          </label>
          <label className="field" style={{ width: 140 }}>
            <span>{t.custCountry}</span>
            <input
              type="text"
              value={c.country}
              placeholder="DE"
              onChange={(e) => set('country', e.target.value.toUpperCase())}
            />
          </label>
        </div>
        <label className="field">
          <span>{t.custNotes}</span>
          <textarea rows={2} value={c.notes} onChange={(e) => set('notes', e.target.value)} />
        </label>
        <div className="btnrow">
          <button onClick={onClose}>{t.cancel}</button>
          <button
            className="primary"
            disabled={!c.name.trim()}
            onClick={() => onSave({ ...c, address: composeAddress(c) })}
          >
            {t.save}
          </button>
        </div>
      </div>
    </div>
  );
}

export function SaveTemplateModal({
  t,
  onClose,
  onSave,
}: {
  t: Dict;
  onClose: () => void;
  onSave: (name: string) => void;
}) {
  const [name, setName] = useState('');
  return (
    <div className="overlay" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <h2>{t.saveAsTemplate}</h2>
        <label className="field">
          <span>{t.templateNamePlaceholder}</span>
          <input
            type="text"
            autoFocus
            value={name}
            onChange={(e) => setName(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && name.trim()) onSave(name.trim());
            }}
          />
        </label>
        <div className="btnrow">
          <button onClick={onClose}>{t.cancel}</button>
          <button className="primary" disabled={!name.trim()} onClick={() => onSave(name.trim())}>
            {t.save}
          </button>
        </div>
      </div>
    </div>
  );
}

export function TemplatesModal({
  templates,
  kindLabel,
  t,
  onClose,
  onUse,
  onDelete,
}: {
  templates: Template[];
  kindLabel: (kind: Template['kind']) => string;
  t: Dict;
  onClose: () => void;
  onUse: (template: Template) => void;
  onDelete: (id: string) => void;
}) {
  return (
    <div className="overlay" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <h2>{t.manageTemplates}</h2>
        {templates.length === 0 && <div className="note">{t.noTemplates}</div>}
        {templates.map((template) => (
          <div key={template.id} className="template-row">
            <button className="grow1 tpl-use" onClick={() => onUse(template)}>
              {template.name}
              <span className="dim"> · {kindLabel(template.kind)} · {template.items.length} Pos.</span>
            </button>
            <button
              className="icon danger"
              title={t.actDelete}
              onClick={() => {
                if (window.confirm(t.confirmDeleteTemplate)) onDelete(template.id);
              }}
            >
              ✕
            </button>
          </div>
        ))}
        <div className="btnrow">
          <button onClick={onClose}>{t.close}</button>
        </div>
      </div>
    </div>
  );
}

export function ProductModal({
  product,
  t,
  lang,
  onClose,
  onSave,
}: {
  product: Product;
  t: Dict;
  lang: Lang;
  onClose: () => void;
  onSave: (p: Product) => void;
}) {
  const [p, setP] = useState<Product>({ ...product });
  const [priceStr, setPriceStr] = useState(centsToInput(product.unitPriceCents, lang));
  const set = <K extends keyof Product>(key: K, value: Product[K]) =>
    setP((prev) => ({ ...prev, [key]: value }));

  return (
    <div className="overlay" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <h2>{p.name.trim() ? p.name : t.newProduct}</h2>
        <label className="field">
          <span>{t.prodName}</span>
          <input type="text" value={p.name} onChange={(e) => set('name', e.target.value)} />
        </label>
        <label className="field">
          <span>{t.prodDescription}</span>
          <textarea
            rows={2}
            value={p.description}
            onChange={(e) => set('description', e.target.value)}
          />
        </label>
        <div className="row3">
          <label className="field">
            <span>{t.prodUnit}</span>
            <input type="text" value={p.unit} onChange={(e) => set('unit', e.target.value)} />
          </label>
          <label className="field">
            <span>{t.prodPrice}</span>
            <input
              type="text"
              value={priceStr}
              onChange={(e) => setPriceStr(e.target.value)}
              onBlur={() => {
                const cents = parseMoney(priceStr);
                set('unitPriceCents', cents);
                setPriceStr(centsToInput(cents, lang));
              }}
            />
          </label>
          <label className="field">
            <span>{t.prodVat}</span>
            <select value={p.vatRate} onChange={(e) => set('vatRate', Number(e.target.value))}>
              <option value={19}>19 %</option>
              <option value={7}>7 %</option>
              <option value={0}>0 %</option>
            </select>
          </label>
        </div>
        <div className="btnrow">
          <button onClick={onClose}>{t.cancel}</button>
          <button className="primary" disabled={!p.name.trim()} onClick={() => onSave(p)}>
            {t.save}
          </button>
        </div>
      </div>
    </div>
  );
}
