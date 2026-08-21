import { useCallback, useEffect, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { save } from '@tauri-apps/plugin-dialog';
import {
  Customer,
  Doc,
  Product,
  Settings,
  UpdateInfo,
  addDaysIso,
  api,
  fmtDate,
  fmtMoney,
  isOverdue,
  todayIso,
  totals,
} from './api';
import { dicts, Lang } from './i18n';
import { DocEditor } from './components/DocEditor';
import { CustomerModal, ProductModal } from './components/Modals';
import { SettingsModal } from './components/SettingsModal';
import { Help } from './components/Help';
import { buildPdf, bytesToBase64 } from './pdf';
import { embedZugferd } from './zugferd';
import {
  IconBox,
  IconCancel,
  IconCheck,
  IconEInvoice,
  IconEdit,
  IconGear,
  IconInvoice,
  IconPdf,
  IconPlus,
  IconTrash,
  IconUsers,
  IconXml,
} from './icons';

type Tab = 'docs' | 'customers' | 'products';
type DocFilter = 'all' | 'draft' | 'open' | 'overdue' | 'paid' | 'cancelled';

const uid = () => crypto.randomUUID();

export default function App() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [tab, setTab] = useState<Tab>('docs');
  const [query, setQuery] = useState('');
  const [filter, setFilter] = useState<DocFilter>('all');
  const [docs, setDocs] = useState<Doc[]>([]);
  const [customers, setCustomers] = useState<Customer[]>([]);
  const [products, setProducts] = useState<Product[]>([]);
  const [editorDoc, setEditorDoc] = useState<Doc | null>(null);
  const [editCustomer, setEditCustomer] = useState<Customer | null>(null);
  const [editProduct, setEditProduct] = useState<Product | null>(null);
  const [showSettings, setShowSettings] = useState(false);
  const [toast, setToast] = useState<string | null>(null);
  const [updateAvail, setUpdateAvail] = useState<UpdateInfo | null>(null);
  const [installing, setInstalling] = useState(false);
  const toastTimer = useRef<number | undefined>(undefined);

  const lang: Lang = settings?.language ?? 'de';
  const t = dicts[lang];

  const showToast = useCallback((msg: string) => {
    setToast(msg);
    window.clearTimeout(toastTimer.current);
    toastTimer.current = window.setTimeout(() => setToast(null), 1800);
  }, []);

  const refresh = useCallback(() => {
    api.listDocs(query).then(setDocs).catch(() => {});
    api.listCustomers().then(setCustomers).catch(() => {});
    api.listProducts().then(setProducts).catch(() => {});
  }, [query]);

  useEffect(() => {
    api.getSettings().then((s) => {
      setSettings(s);
      // stiller Update-Check beim Start — mit Auto-Update wird direkt
      // installiert, sonst nur der Banner gezeigt
      api
        .checkUpdate()
        .then((u) => {
          if (!u) return;
          setUpdateAvail(u);
          if (s.autoUpdate) {
            setInstalling(true);
            api.installUpdate().catch(() => setInstalling(false));
          }
        })
        .catch(() => {});
    });
  }, []);

  // Darstellung aus den Einstellungen auf <html> spiegeln
  useEffect(() => {
    if (!settings) return;
    document.documentElement.dataset.theme = settings.theme;
    document.documentElement.dataset.accent = settings.accent;
  }, [settings]);

  useEffect(refresh, [refresh]);

  useEffect(() => {
    const sub = listen('db-changed', refresh);
    return () => {
      sub.then((un) => un());
    };
  }, [refresh]);

  const newDoc = (kind: 'invoice' | 'creditnote') => {
    if (!settings) return;
    const date = todayIso();
    setEditorDoc({
      id: uid(),
      kind,
      number: null,
      customerId: '',
      customerName: '',
      customerAddress: '',
      date,
      dueDate: kind === 'invoice' ? addDaysIso(date, settings.paymentTermsDays) : date,
      items: [],
      status: 'draft',
      smallBusiness: settings.smallBusiness,
      intro: kind === 'invoice' ? settings.defaultIntro : '',
      notes: '',
      paidAt: null,
      relatedId: null,
      createdAt: new Date().toISOString(),
    });
  };

  const saveDraft = async (doc: Doc, silent = false): Promise<boolean> => {
    if (!doc.customerId) {
      showToast(t.needCustomer);
      return false;
    }
    try {
      await api.upsertDoc(doc);
      if (!silent) showToast(t.saved);
      return true;
    } catch (e) {
      showToast(String(e));
      return false;
    }
  };

  const finalize = async (doc: Doc) => {
    if (doc.items.length === 0 || doc.items.every((i) => !i.name.trim())) {
      showToast(t.needItems);
      return;
    }
    if (!(await saveDraft(doc, true))) return;
    try {
      const finalized = await api.finalizeDoc(doc.id);
      setEditorDoc(finalized);
      showToast(`${t.finalized}: ${finalized.number}`);
    } catch (e) {
      showToast(String(e));
    }
  };

  const exportPdf = async (doc: Doc) => {
    if (!settings) return;
    try {
      const logo = await api.getLogo().catch(() => null);
      const related = doc.relatedId
        ? docs.find((x) => x.id === doc.relatedId)?.number ?? null
        : null;
      const { bytes, suggestedName } = buildPdf(doc, settings, t, lang, logo, related);
      const path = await save({
        defaultPath: suggestedName,
        filters: [{ name: 'PDF', extensions: ['pdf'] }],
      });
      if (!path) return;
      await api.writeFile(path, bytesToBase64(bytes));
      showToast(t.pdfSaved);
    } catch (e) {
      showToast(String(e));
    }
  };

  const exportXml = async (doc: Doc) => {
    try {
      const { xml, suggestedName } = await api.einvoiceXml(doc.id);
      const path = await save({
        defaultPath: suggestedName,
        filters: [{ name: 'XML', extensions: ['xml'] }],
      });
      if (!path) return;
      await api.writeFile(path, bytesToBase64(new TextEncoder().encode(xml)));
      showToast(t.xmlSaved);
    } catch (e) {
      showToast(String(e));
    }
  };

  const exportZugferd = async (doc: Doc) => {
    if (!settings) return;
    try {
      const [{ xml }, logo] = await Promise.all([
        api.einvoiceXml(doc.id),
        api.getLogo().catch(() => null),
      ]);
      const related = doc.relatedId
        ? docs.find((x) => x.id === doc.relatedId)?.number ?? null
        : null;
      const { bytes } = buildPdf(doc, settings, t, lang, logo, related);
      const hybrid = await embedZugferd(bytes, xml, doc.number ?? 'invoice');
      const path = await save({
        defaultPath: `${doc.number ?? 'beleg'}-zugferd.pdf`,
        filters: [{ name: 'PDF', extensions: ['pdf'] }],
      });
      if (!path) return;
      await api.writeFile(path, bytesToBase64(hybrid));
      showToast(t.zugferdSaved);
    } catch (e) {
      showToast(String(e));
    }
  };

  const cancelInvoice = async (doc: Doc) => {
    if (!window.confirm(t.confirmCancel)) return;
    try {
      const storno = await api.cancelDoc(doc.id, uid());
      showToast(`${t.cancelled}: ${storno.number}`);
    } catch (e) {
      showToast(String(e));
    }
  };

  if (!settings) return null;

  const filteredDocs = docs.filter((doc) => {
    switch (filter) {
      case 'draft':
        return doc.status === 'draft';
      case 'open':
        return doc.status === 'open';
      case 'overdue':
        return isOverdue(doc);
      case 'paid':
        return doc.status === 'paid';
      case 'cancelled':
        return doc.status === 'cancelled';
      default:
        return true;
    }
  });

  const openCents = docs
    .filter((doc) => doc.status === 'open')
    .reduce((sum, doc) => sum + totals(doc.items, doc.smallBusiness).grossCents, 0);

  const filters: { key: DocFilter; label: string }[] = [
    { key: 'all', label: t.filterAll },
    { key: 'draft', label: t.filterDraft },
    { key: 'open', label: t.filterOpen },
    { key: 'overdue', label: t.filterOverdue },
    { key: 'paid', label: t.filterPaid },
    { key: 'cancelled', label: t.filterCancelled },
  ];

  const statusChip = (doc: Doc) => {
    if (doc.status === 'draft') return <span className="chip mini">{t.statusDraft}</span>;
    if (doc.status === 'paid') return <span className="chip mini ok">{t.statusPaid}</span>;
    if (doc.status === 'cancelled') return <span className="chip mini dim">{t.statusCancelled}</span>;
    if (isOverdue(doc)) return <span className="chip mini bad">{t.statusOverdue}</span>;
    return <span className="chip mini warn">{t.statusOpen}</span>;
  };

  const kindLabel = (doc: Doc) =>
    doc.kind === 'creditnote'
      ? t.kindCreditNote
      : doc.kind === 'cancellation'
        ? t.kindCancellation
        : t.kindInvoice;

  return (
    <div className="app">
      <div className="header">
        <div className="brand">
          <span className="name">invoices</span>
          <span className="dot">.</span>
        </div>
        <span className="tagline">{t.tagline}</span>
        {!editorDoc && (
          <>
            <div className="tabs">
              <button className={tab === 'docs' ? 'active' : ''} onClick={() => setTab('docs')}>
                <IconInvoice /> {t.tabDocs}
              </button>
              <button
                className={tab === 'customers' ? 'active' : ''}
                onClick={() => setTab('customers')}
              >
                <IconUsers /> {t.tabCustomers}
              </button>
              <button
                className={tab === 'products' ? 'active' : ''}
                onClick={() => setTab('products')}
              >
                <IconBox /> {t.tabProducts}
              </button>
            </div>
            <input
              className="grow"
              type="text"
              placeholder={t.search}
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
            {tab === 'docs' && (
              <button className="primary" onClick={() => newDoc('invoice')}>
                <IconPlus /> {t.newInvoice}
              </button>
            )}
            {tab === 'customers' && (
              <button
                className="primary"
                onClick={() =>
                  setEditCustomer({
                    id: uid(),
                    name: '',
                    address: '',
                    email: '',
                    vatId: '',
                    notes: '',
                    buyerReference: '',
                    country: '',
                  })
                }
              >
                <IconPlus /> {t.newCustomer}
              </button>
            )}
            {tab === 'products' && (
              <button
                className="primary"
                onClick={() =>
                  setEditProduct({
                    id: uid(),
                    name: '',
                    description: '',
                    unit: settings.defaultUnit || 'Stk',
                    unitPriceCents: 0,
                    vatRate: settings.defaultVatRate,
                  })
                }
              >
                <IconPlus /> {t.newProduct}
              </button>
            )}
            <button className="ghost" title={t.settings} onClick={() => setShowSettings(true)}>
              <IconGear />
            </button>
          </>
        )}
      </div>

      {editorDoc ? (
        <DocEditor
          doc={editorDoc}
          customers={customers}
          products={products}
          settings={settings}
          t={t}
          lang={lang}
          onClose={() => setEditorDoc(null)}
          onSaveDraft={(d) => {
            saveDraft(d).then((ok) => {
              if (ok) setEditorDoc(null);
            });
          }}
          onFinalize={finalize}
          onPdf={exportPdf}
        />
      ) : tab === 'docs' ? (
        <>
          <div className="filters">
            {filters.map((f) => (
              <button
                key={f.key}
                className={`chip ${filter === f.key ? 'active' : ''}`}
                onClick={() => setFilter(f.key)}
              >
                {f.label}
              </button>
            ))}
            <button className="chip alt" onClick={() => newDoc('creditnote')}>
              <IconPlus /> {t.newCreditNote}
            </button>
            <span className="count">
              {docs.length} {t.docsCount} · {fmtMoney(openCents, lang)} {t.openSum}
            </span>
          </div>
          <div className="list">
            {filteredDocs.length === 0 && (
              <div className="empty">
                {query || filter !== 'all' ? t.emptyFiltered : t.emptyDocs}
              </div>
            )}
            {filteredDocs.length > 0 && (
              <table className="doctable">
                <thead>
                  <tr>
                    <th>{t.colNumber}</th>
                    <th>{t.colCustomer}</th>
                    <th>{t.colDate}</th>
                    <th>{t.colDue}</th>
                    <th className="num">{t.colTotal}</th>
                    <th />
                    <th className="actions" />
                  </tr>
                </thead>
                <tbody>
                  {filteredDocs.map((doc) => {
                    const sums = totals(doc.items, doc.smallBusiness);
                    return (
                      <tr key={doc.id} onClick={() => setEditorDoc(doc)}>
                        <td className="mono">
                          {doc.number ?? t.draftNumber}
                          {doc.kind !== 'invoice' && (
                            <span className="kind"> · {kindLabel(doc)}</span>
                          )}
                        </td>
                        <td>{doc.customerName || '—'}</td>
                        <td className="mono">{fmtDate(doc.date, lang)}</td>
                        <td className="mono">
                          {doc.kind === 'invoice' ? fmtDate(doc.dueDate, lang) : '—'}
                        </td>
                        <td className="num mono">{fmtMoney(sums.grossCents, lang)}</td>
                        <td>{statusChip(doc)}</td>
                        <td className="actions" onClick={(e) => e.stopPropagation()}>
                          {doc.status !== 'draft' && (
                            <>
                              <button className="icon" title={t.actPdf} onClick={() => exportPdf(doc)}>
                                <IconPdf />
                              </button>
                              <button
                                className="icon"
                                title={t.actZugferd}
                                onClick={() => exportZugferd(doc)}
                              >
                                <IconEInvoice />
                              </button>
                              <button className="icon" title={t.actXml} onClick={() => exportXml(doc)}>
                                <IconXml />
                              </button>
                            </>
                          )}
                          {doc.status === 'open' && (
                            <button
                              className="icon"
                              title={t.actMarkPaid}
                              onClick={() => api.setPaid(doc.id, true).then(() => showToast(t.markedPaid))}
                            >
                              <IconCheck />
                            </button>
                          )}
                          {doc.status === 'paid' && (
                            <button
                              className="icon"
                              title={t.actMarkUnpaid}
                              onClick={() => api.setPaid(doc.id, false).then(() => showToast(t.markedUnpaid))}
                            >
                              <IconCheck />
                            </button>
                          )}
                          {doc.kind === 'invoice' &&
                            (doc.status === 'open' || doc.status === 'paid') && (
                              <button
                                className="icon danger"
                                title={t.actCancel}
                                onClick={() => cancelInvoice(doc)}
                              >
                                <IconCancel />
                              </button>
                            )}
                          {doc.status === 'draft' && (
                            <>
                              <button className="icon" title={t.actEdit} onClick={() => setEditorDoc(doc)}>
                                <IconEdit />
                              </button>
                              <button
                                className="icon danger"
                                title={t.actDelete}
                                onClick={() => {
                                  if (window.confirm(t.confirmDeleteDraft)) api.deleteDoc(doc.id);
                                }}
                              >
                                <IconTrash />
                              </button>
                            </>
                          )}
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            )}
          </div>
        </>
      ) : tab === 'customers' ? (
        <div className="list">
          {customers.length === 0 && <div className="empty">{t.emptyCustomers}</div>}
          {customers.length > 0 && (
            <table className="doctable">
              <thead>
                <tr>
                  <th>{t.custName}</th>
                  <th>{t.custEmail}</th>
                  <th>{t.custVatId}</th>
                  <th className="actions" />
                </tr>
              </thead>
              <tbody>
                {customers
                  .filter(
                    (c) =>
                      !query.trim() ||
                      c.name.toLowerCase().includes(query.trim().toLowerCase()) ||
                      c.email.toLowerCase().includes(query.trim().toLowerCase())
                  )
                  .map((c) => (
                    <tr key={c.id} onClick={() => setEditCustomer(c)}>
                      <td>{c.name}</td>
                      <td className="mono">{c.email || '—'}</td>
                      <td className="mono">{c.vatId || '—'}</td>
                      <td className="actions" onClick={(e) => e.stopPropagation()}>
                        <button className="icon" title={t.actEdit} onClick={() => setEditCustomer(c)}>
                          <IconEdit />
                        </button>
                        <button
                          className="icon danger"
                          title={t.actDelete}
                          onClick={() => {
                            if (!window.confirm(t.confirmDeleteCustomer)) return;
                            api.deleteCustomer(c.id).catch(() => showToast(t.customerInDraft));
                          }}
                        >
                          <IconTrash />
                        </button>
                      </td>
                    </tr>
                  ))}
              </tbody>
            </table>
          )}
        </div>
      ) : (
        <div className="list">
          {products.length === 0 && <div className="empty">{t.emptyProducts}</div>}
          {products.length > 0 && (
            <table className="doctable">
              <thead>
                <tr>
                  <th>{t.prodName}</th>
                  <th>{t.prodUnit}</th>
                  <th className="num">{t.prodPrice}</th>
                  <th className="num">{t.prodVat}</th>
                  <th className="actions" />
                </tr>
              </thead>
              <tbody>
                {products
                  .filter(
                    (p) => !query.trim() || p.name.toLowerCase().includes(query.trim().toLowerCase())
                  )
                  .map((p) => (
                    <tr key={p.id} onClick={() => setEditProduct(p)}>
                      <td>{p.name}</td>
                      <td className="mono">{p.unit}</td>
                      <td className="num mono">{fmtMoney(p.unitPriceCents, lang)}</td>
                      <td className="num mono">{p.vatRate} %</td>
                      <td className="actions" onClick={(e) => e.stopPropagation()}>
                        <button className="icon" title={t.actEdit} onClick={() => setEditProduct(p)}>
                          <IconEdit />
                        </button>
                        <button
                          className="icon danger"
                          title={t.actDelete}
                          onClick={() => {
                            if (window.confirm(t.confirmDeleteProduct)) api.deleteProduct(p.id);
                          }}
                        >
                          <IconTrash />
                        </button>
                      </td>
                    </tr>
                  ))}
              </tbody>
            </table>
          )}
        </div>
      )}

      {editCustomer && (
        <CustomerModal
          customer={editCustomer}
          t={t}
          onClose={() => setEditCustomer(null)}
          onSave={(c) => {
            api.upsertCustomer(c).then(() => {
              setEditCustomer(null);
              showToast(t.saved);
            });
          }}
        />
      )}

      {editProduct && (
        <ProductModal
          product={editProduct}
          t={t}
          lang={lang}
          onClose={() => setEditProduct(null)}
          onSave={(p) => {
            api.upsertProduct(p).then(() => {
              setEditProduct(null);
              showToast(t.saved);
            });
          }}
        />
      )}

      {showSettings && (
        <SettingsModal
          settings={settings}
          t={t}
          onClose={() => setShowSettings(false)}
          onSave={(s) => {
            api.setSettings(s).then(() => {
              setSettings(s);
              setShowSettings(false);
            });
          }}
        />
      )}

      {updateAvail && (
        <div className="upd-banner">
          <span>
            {t.updateBanner} <strong>{updateAvail.version}</strong>
          </span>
          <button
            className="primary"
            disabled={installing}
            onClick={() => {
              setInstalling(true);
              api.installUpdate().catch(() => setInstalling(false));
            }}
          >
            {installing ? t.updateInstalling : t.updateInstall}
          </button>
          <button className="ghost" onClick={() => setUpdateAvail(null)}>
            {t.updateLater}
          </button>
        </div>
      )}

      <Help lang={lang} />
      {toast && <div className="toast">{toast}</div>}
    </div>
  );
}
