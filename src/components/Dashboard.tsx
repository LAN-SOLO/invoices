import { useMemo } from 'react';
import { Customer, Doc, fmtDate, fmtMoney, isOverdue, totals } from '../api';
import { Dict, Lang } from '../i18n';

// Übersicht/Monitoring: KPI-Kacheln, Monatsumsatz als SVG-Balken,
// fällige Rechnungen und Top-Kunden — alles client-seitig aus den
// vorhandenen Belegen gerechnet, im Terminal-Look.

interface Props {
  docs: Doc[];
  customers: Customer[];
  t: Dict;
  lang: Lang;
  onOpenDoc: (doc: Doc) => void;
}

/// Umsatzbeitrag eines Belegs: Rechnungen +, Gutschriften −, Stornos
/// bringen ihr (negatives) Vorzeichen selbst mit. Entwürfe, Angebote,
/// AB und Lieferscheine zählen nicht.
function revenueCents(doc: Doc): number {
  if (doc.status === 'draft' || doc.status === 'cancelled') return 0;
  const gross = totals(doc.items, doc.smallBusiness).grossCents;
  if (doc.kind === 'invoice' || doc.kind === 'cancellation') return gross;
  if (doc.kind === 'creditnote') return -gross;
  return 0;
}

function monthKey(iso: string): string {
  return iso.slice(0, 7);
}

export function Dashboard({ docs, customers, t, lang, onOpenDoc }: Props) {
  const stats = useMemo(() => {
    const now = new Date();
    const thisYear = String(now.getFullYear());
    const thisMonth = monthKey(now.toISOString());

    const openInvoices = docs.filter((d) => d.kind === 'invoice' && d.status === 'open');
    const overdue = openInvoices.filter(isOverdue);
    const sum = (list: Doc[]) =>
      list.reduce((s, d) => s + totals(d.items, d.smallBusiness).grossCents, 0);

    const revenueYear = docs
      .filter((d) => d.date.startsWith(thisYear))
      .reduce((s, d) => s + revenueCents(d), 0);
    const revenueMonth = docs
      .filter((d) => monthKey(d.date) === thisMonth)
      .reduce((s, d) => s + revenueCents(d), 0);

    // letzte 12 Monate, älteste zuerst
    const months: { key: string; label: string; cents: number }[] = [];
    for (let i = 11; i >= 0; i--) {
      const d = new Date(now.getFullYear(), now.getMonth() - i, 1);
      const key = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}`;
      months.push({
        key,
        label: d.toLocaleDateString(lang === 'de' ? 'de-DE' : 'en-US', { month: 'short' }),
        cents: 0,
      });
    }
    for (const doc of docs) {
      const bucket = months.find((m) => m.key === monthKey(doc.date));
      if (bucket) bucket.cents += revenueCents(doc);
    }

    const byCustomer = new Map<string, number>();
    for (const doc of docs) {
      const cents = revenueCents(doc);
      if (cents === 0 || !doc.customerName) continue;
      byCustomer.set(doc.customerName, (byCustomer.get(doc.customerName) ?? 0) + cents);
    }
    const topCustomers = [...byCustomer.entries()]
      .sort((a, b) => b[1] - a[1])
      .slice(0, 5);

    const dueSoon = [...openInvoices].sort((a, b) => a.dueDate.localeCompare(b.dueDate)).slice(0, 6);

    return {
      openCount: openInvoices.length,
      openSum: sum(openInvoices),
      overdueCount: overdue.length,
      overdueSum: sum(overdue),
      draftCount: docs.filter((d) => d.status === 'draft').length,
      revenueYear,
      revenueMonth,
      months,
      topCustomers,
      dueSoon,
    };
  }, [docs, lang]);

  const money = (cents: number) => fmtMoney(cents, lang);
  const maxMonth = Math.max(...stats.months.map((m) => Math.abs(m.cents)), 1);

  const tiles: { label: string; value: string; extra?: string; tone?: string }[] = [
    {
      label: t.dashOpen,
      value: money(stats.openSum),
      extra: `${stats.openCount} ${t.dashInvoices}`,
      tone: 'warn',
    },
    {
      label: t.dashOverdue,
      value: money(stats.overdueSum),
      extra: `${stats.overdueCount} ${t.dashInvoices}`,
      tone: stats.overdueCount > 0 ? 'bad' : undefined,
    },
    { label: t.dashRevenueMonth, value: money(stats.revenueMonth) },
    { label: t.dashRevenueYear, value: money(stats.revenueYear), tone: 'ok' },
    { label: t.filterDraft, value: String(stats.draftCount) },
    { label: t.tabCustomers, value: String(customers.length) },
  ];

  const chartW = 12 * 44;
  const chartH = 120;

  return (
    <div className="list dashboard">
      <div className="dash-tiles">
        {tiles.map((tile) => (
          <div key={tile.label} className={`dash-tile ${tile.tone ?? ''}`}>
            <span className="dash-label">{tile.label}</span>
            <span className="dash-value">{tile.value}</span>
            {tile.extra && <span className="dash-extra">{tile.extra}</span>}
          </div>
        ))}
      </div>

      <div className="dash-panel">
        <div className="fieldlabel">{t.dashMonthly}</div>
        <svg
          className="dash-chart"
          viewBox={`0 0 ${chartW} ${chartH + 18}`}
          preserveAspectRatio="xMinYMid meet"
        >
          {stats.months.map((m, i) => {
            const h = Math.round((Math.abs(m.cents) / maxMonth) * (chartH - 14));
            const x = i * 44 + 6;
            const negative = m.cents < 0;
            return (
              <g key={m.key}>
                <rect
                  x={x}
                  y={chartH - h}
                  width={30}
                  height={Math.max(h, m.cents !== 0 ? 2 : 0)}
                  rx={2}
                  className={negative ? 'bar neg' : 'bar'}
                >
                  <title>
                    {m.key}: {money(m.cents)}
                  </title>
                </rect>
                <text x={x + 15} y={chartH + 13} textAnchor="middle" className="bar-label">
                  {m.label}
                </text>
              </g>
            );
          })}
        </svg>
      </div>

      <div className="dash-cols">
        <div className="dash-panel grow1">
          <div className="fieldlabel">{t.dashDue}</div>
          {stats.dueSoon.length === 0 && <div className="note">{t.dashNoDue}</div>}
          {stats.dueSoon.map((doc) => (
            <button key={doc.id} className="dash-row" onClick={() => onOpenDoc(doc)}>
              <span className="mono">{doc.number}</span>
              <span className="dash-cust">{doc.customerName}</span>
              <span className={`mono ${isOverdue(doc) ? 'due-bad' : 'dim'}`}>
                {fmtDate(doc.dueDate, lang)}
              </span>
              <span className="mono num grow1">
                {money(totals(doc.items, doc.smallBusiness).grossCents)}
              </span>
            </button>
          ))}
        </div>
        <div className="dash-panel grow1">
          <div className="fieldlabel">{t.dashTopCustomers}</div>
          {stats.topCustomers.length === 0 && <div className="note">{t.dashNoData}</div>}
          {stats.topCustomers.map(([name, cents]) => (
            <div key={name} className="dash-row static">
              <span className="dash-cust">{name}</span>
              <span className="mono num grow1">{money(cents)}</span>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
