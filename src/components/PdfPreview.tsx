import { useEffect, useMemo, useState } from 'react';
import { save } from '@tauri-apps/plugin-dialog';
import { api, Doc, Settings } from '../api';
import { Dict, Lang } from '../i18n';
import { buildPdf, bytesToBase64, PdfLayout } from '../pdf';

// PDF-Vorschau mit Layout-Umschalter: rendert das Dokument live als
// Blob-URL in ein iframe (WKWebView/WebView2 zeigen PDFs nativ an)
// und exportiert im gewählten Layout.

interface Props {
  doc: Doc;
  settings: Settings;
  relatedNumber: string | null;
  t: Dict;
  lang: Lang;
  onClose: () => void;
  showToast: (msg: string) => void;
}

const LAYOUTS: PdfLayout[] = ['classic', 'modern', 'compact', 'terminal'];

export function PdfPreview({ doc, settings, relatedNumber, t, lang, onClose, showToast }: Props) {
  const [layout, setLayout] = useState<PdfLayout>(
    (settings.pdfLayout as PdfLayout) ?? 'classic'
  );
  const [logo, setLogo] = useState<string | null>(null);
  const [logoLoaded, setLogoLoaded] = useState(false);
  const [url, setUrl] = useState<string | null>(null);

  useEffect(() => {
    api
      .getLogo()
      .catch(() => null)
      .then((l) => {
        setLogo(l);
        setLogoLoaded(true);
      });
  }, []);

  const layoutLabel: Record<PdfLayout, string> = useMemo(
    () => ({
      classic: t.layoutClassic,
      modern: t.layoutModern,
      compact: t.layoutCompact,
      terminal: t.layoutTerminal,
    }),
    [t]
  );

  useEffect(() => {
    if (!logoLoaded) return;
    const { bytes } = buildPdf(doc, settings, t, lang, logo, relatedNumber, layout);
    const blobUrl = URL.createObjectURL(
      new Blob([bytes.slice().buffer], { type: 'application/pdf' })
    );
    setUrl(blobUrl);
    return () => URL.revokeObjectURL(blobUrl);
  }, [doc, settings, t, lang, logo, logoLoaded, relatedNumber, layout]);

  const exportPdf = async () => {
    try {
      const { bytes, suggestedName } = buildPdf(doc, settings, t, lang, logo, relatedNumber, layout);
      const path = await save({
        defaultPath: suggestedName,
        filters: [{ name: 'PDF', extensions: ['pdf'] }],
      });
      if (!path) return;
      await api.writeFile(path, bytesToBase64(bytes));
      showToast(t.pdfSaved);
      onClose();
    } catch (e) {
      showToast(String(e));
    }
  };

  return (
    <div className="overlay" onClick={onClose}>
      <div className="preview-modal" onClick={(e) => e.stopPropagation()}>
        <div className="preview-head">
          <span className="fieldlabel" style={{ margin: 0 }}>
            {t.actPreview}
          </span>
          {LAYOUTS.map((l) => (
            <button
              key={l}
              className={`chip ${layout === l ? 'active' : ''}`}
              onClick={() => setLayout(l)}
            >
              {layoutLabel[l]}
            </button>
          ))}
          <span className="spacer" />
          <button className="primary" onClick={exportPdf}>
            {t.actPdf}
          </button>
          <button className="ghost" onClick={onClose}>
            {t.close}
          </button>
        </div>
        <div className="preview-body">
          {url && <iframe title="pdf" src={url} />}
        </div>
        <div className="note" style={{ margin: '6px 12px' }}>
          {t.previewNote}
        </div>
      </div>
    </div>
  );
}
