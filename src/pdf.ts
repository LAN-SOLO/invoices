// PDF-Erzeugung mit jsPDF: A4-Beleg mit Briefkopf, Empfängerblock,
// Positionstabelle und USt-Aufstellung. Erzeugt Bytes — gespeichert
// wird über den Save-Dialog + Rust-Command (write_file).

import { jsPDF } from 'jspdf';
import autoTable from 'jspdf-autotable';
import {
  Doc,
  Settings,
  fmtDate,
  fmtMoney,
  lineTotalCents,
  totals,
} from './api';
import { Dict, Lang } from './i18n';

const M = 20; // page margin (mm)
const PAGE_W = 210;

function docTitle(doc: Doc, t: Dict): string {
  if (doc.kind === 'creditnote') return t.pdf.creditNote;
  if (doc.kind === 'cancellation') return t.pdf.cancellation;
  return t.pdf.invoice;
}

export function buildPdf(
  doc: Doc,
  settings: Settings,
  t: Dict,
  lang: Lang,
  logoDataUrl: string | null,
  relatedNumber: string | null
): { bytes: Uint8Array; suggestedName: string } {
  const pdf = new jsPDF({ unit: 'mm', format: 'a4' });
  const money = (cents: number) => fmtMoney(cents, lang);

  // --- letterhead ---
  let y = M;
  if (logoDataUrl) {
    try {
      const props = pdf.getImageProperties(logoDataUrl);
      const h = 18;
      const w = (props.width / props.height) * h;
      pdf.addImage(logoDataUrl, PAGE_W - M - w, y, w, h);
    } catch {
      /* kaputtes Logo bricht kein PDF */
    }
  }
  pdf.setFont('helvetica', 'bold');
  pdf.setFontSize(14);
  pdf.setTextColor(30);
  pdf.text(settings.companyName || '', M, y + 5);
  pdf.setFont('helvetica', 'normal');
  pdf.setFontSize(9);
  pdf.setTextColor(90);
  const companyLines = settings.companyAddress.split('\n').filter(Boolean);
  pdf.text(companyLines, M, y + 11);

  // --- recipient block (DIN-ish position) ---
  y = 50;
  pdf.setFontSize(7.5);
  pdf.setTextColor(130);
  const senderLine = [settings.companyName, ...companyLines].filter(Boolean).join(' · ');
  pdf.text(senderLine, M, y);
  pdf.setDrawColor(200);
  pdf.line(M, y + 1.5, M + 85, y + 1.5);
  pdf.setFontSize(10);
  pdf.setTextColor(30);
  const recipient = [doc.customerName, ...doc.customerAddress.split('\n')].filter(Boolean);
  pdf.text(recipient, M, y + 8);

  // --- meta block right ---
  const metaX = PAGE_W - M - 60;
  pdf.setFontSize(9);
  const meta: [string, string][] = [
    [t.pdf.number, doc.number ?? '—'],
    [t.pdf.date, fmtDate(doc.date, lang)],
  ];
  if (doc.kind === 'invoice') meta.push([t.pdf.due, fmtDate(doc.dueDate, lang)]);
  if (doc.kind === 'cancellation' && relatedNumber) meta.push([t.pdf.cancels, relatedNumber]);
  let metaY = y + 8;
  for (const [k, v] of meta) {
    pdf.setTextColor(120);
    pdf.text(k, metaX, metaY);
    pdf.setTextColor(30);
    pdf.text(v, metaX + 28, metaY);
    metaY += 5;
  }

  // --- title + intro ---
  y = 92;
  pdf.setFont('helvetica', 'bold');
  pdf.setFontSize(13);
  pdf.setTextColor(20);
  pdf.text(`${docTitle(doc, t)} ${doc.number ?? ''}`.trim(), M, y);
  y += 8;
  if (doc.intro.trim()) {
    pdf.setFont('helvetica', 'normal');
    pdf.setFontSize(9.5);
    pdf.setTextColor(60);
    const intro = pdf.splitTextToSize(doc.intro.trim(), PAGE_W - 2 * M);
    pdf.text(intro, M, y);
    y += intro.length * 4.5 + 3;
  }

  // --- positions table ---
  const body = doc.items.map((item, i) => [
    String(i + 1),
    item.description.trim() ? `${item.name}\n${item.description.trim()}` : item.name,
    String(item.quantity).replace('.', lang === 'de' ? ',' : '.'),
    item.unit,
    money(item.unitPriceCents),
    doc.smallBusiness ? '—' : `${item.vatRate} %`,
    money(lineTotalCents(item)),
  ]);
  autoTable(pdf, {
    startY: y,
    margin: { left: M, right: M },
    head: [[t.pdf.pos, t.pdf.name, t.pdf.qty, t.pdf.unit, t.pdf.unitPrice, t.pdf.vat, t.pdf.total]],
    body,
    styles: { font: 'helvetica', fontSize: 9, cellPadding: 2, textColor: 40 },
    headStyles: { fillColor: [15, 23, 40], textColor: 235, fontStyle: 'bold' },
    columnStyles: {
      0: { cellWidth: 10 },
      2: { cellWidth: 14, halign: 'right' },
      3: { cellWidth: 16 },
      4: { cellWidth: 24, halign: 'right' },
      5: { cellWidth: 14, halign: 'right' },
      6: { cellWidth: 26, halign: 'right' },
    },
  });
  // jspdf-autotable stores its cursor on the doc instance
  y = (pdf as unknown as { lastAutoTable: { finalY: number } }).lastAutoTable.finalY + 6;

  // --- totals ---
  const sums = totals(doc.items, doc.smallBusiness);
  const sumX = PAGE_W - M - 70;
  const sumLines: [string, string, boolean][] = [[t.pdf.netSum, money(sums.netCents), false]];
  for (const v of sums.vat) {
    sumLines.push([t.pdf.vatLine(v.rate), money(v.vatCents), false]);
  }
  sumLines.push([t.pdf.grossSum, money(sums.grossCents), true]);
  pdf.setFontSize(9.5);
  for (const [label, value, strong] of sumLines) {
    if (y > 265) {
      pdf.addPage();
      y = M;
    }
    pdf.setFont('helvetica', strong ? 'bold' : 'normal');
    pdf.setTextColor(strong ? 20 : 70);
    if (strong) {
      pdf.setDrawColor(120);
      pdf.line(sumX, y - 3.5, PAGE_W - M, y - 3.5);
    }
    pdf.text(label, sumX, y);
    pdf.text(value, PAGE_W - M, y, { align: 'right' });
    y += 5.5;
  }

  // --- notes: §19 / payment terms ---
  y += 3;
  pdf.setFont('helvetica', 'normal');
  pdf.setFontSize(9);
  pdf.setTextColor(70);
  const noteLines: string[] = [];
  if (doc.smallBusiness) noteLines.push(t.pdf.smallBusinessNote);
  if (doc.kind === 'invoice' && doc.dueDate) {
    noteLines.push(t.pdf.paymentNote(fmtDate(doc.dueDate, lang)));
  }
  if (settings.pdfFooter.trim()) noteLines.push(settings.pdfFooter.trim());
  for (const line of noteLines) {
    const wrapped = pdf.splitTextToSize(line, PAGE_W - 2 * M);
    if (y + wrapped.length * 4.5 > 270) {
      pdf.addPage();
      y = M;
    }
    pdf.text(wrapped, M, y);
    y += wrapped.length * 4.5 + 2;
  }

  // --- footer on every page ---
  const pages = pdf.getNumberOfPages();
  const footerCols: string[][] = [];
  const col1 = [settings.companyName, ...companyLines].filter(Boolean);
  const col2 = [
    settings.taxNumber ? `${t.pdf.taxNumber}: ${settings.taxNumber}` : '',
    settings.vatId ? `${t.pdf.vatId}: ${settings.vatId}` : '',
    settings.companyEmail,
    settings.companyPhone,
  ].filter(Boolean);
  const col3 = [
    settings.bankName ? `${t.pdf.bank}: ${settings.bankName}` : '',
    settings.iban ? `IBAN: ${settings.iban}` : '',
    settings.bic ? `BIC: ${settings.bic}` : '',
  ].filter(Boolean);
  footerCols.push(col1, col2, col3);
  for (let p = 1; p <= pages; p++) {
    pdf.setPage(p);
    pdf.setDrawColor(200);
    pdf.line(M, 277, PAGE_W - M, 277);
    pdf.setFontSize(7.5);
    pdf.setTextColor(120);
    const colW = (PAGE_W - 2 * M) / 3;
    footerCols.forEach((col, i) => {
      pdf.text(col.slice(0, 4), M + i * colW, 281);
    });
    pdf.text(t.pdf.page(p, pages), PAGE_W - M, 293, { align: 'right' });
  }

  const base = doc.number ?? `${docTitle(doc, t)}-Entwurf`;
  return {
    bytes: new Uint8Array(pdf.output('arraybuffer')),
    suggestedName: `${base}.pdf`,
  };
}

export function bytesToBase64(bytes: Uint8Array): string {
  let bin = '';
  const chunk = 0x8000;
  for (let i = 0; i < bytes.length; i += chunk) {
    bin += String.fromCharCode(...bytes.subarray(i, i + chunk));
  }
  return btoa(bin);
}
