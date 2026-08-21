import { useState } from 'react';
import { Customer, Product, centsToInput, parseMoney } from '../api';
import { Dict, Lang } from '../i18n';

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
  const [c, setC] = useState<Customer>({ ...customer });
  const set = <K extends keyof Customer>(key: K, value: Customer[K]) =>
    setC((prev) => ({ ...prev, [key]: value }));

  return (
    <div className="overlay" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <h2>{c.name.trim() ? c.name : t.newCustomer}</h2>
        <label className="field">
          <span>{t.custName}</span>
          <input type="text" value={c.name} onChange={(e) => set('name', e.target.value)} />
        </label>
        <label className="field">
          <span>{t.custAddress}</span>
          <textarea rows={3} value={c.address} onChange={(e) => set('address', e.target.value)} />
        </label>
        <label className="field">
          <span>{t.custEmail}</span>
          <input type="text" value={c.email} onChange={(e) => set('email', e.target.value)} />
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
          <button className="primary" disabled={!c.name.trim()} onClick={() => onSave(c)}>
            {t.save}
          </button>
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
