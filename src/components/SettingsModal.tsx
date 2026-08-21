import { useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { api, numberPreview, Settings, UpdateInfo } from '../api';
import { Dict } from '../i18n';

const APP_VERSION = '0.2.0';

export function SettingsModal({
  settings,
  t,
  onClose,
  onSave,
}: {
  settings: Settings;
  t: Dict;
  onClose: () => void;
  onSave: (s: Settings) => void;
}) {
  const [s, setS] = useState<Settings>({ ...settings });
  const [updState, setUpdState] = useState<'idle' | 'checking' | 'none' | 'error'>('idle');
  const [update, setUpdate] = useState<UpdateInfo | null>(null);
  const [installing, setInstalling] = useState(false);

  const set = <K extends keyof Settings>(key: K, value: Settings[K]) =>
    setS((prev) => ({ ...prev, [key]: value }));

  const pickLogo = async () => {
    const picked = await open({
      filters: [{ name: 'Bild', extensions: ['png', 'jpg', 'jpeg'] }],
    });
    if (typeof picked === 'string') set('logoPath', picked);
  };

  const checkUpdates = () => {
    setUpdState('checking');
    setUpdate(null);
    api
      .checkUpdate()
      .then((u) => {
        if (u) {
          setUpdate(u);
          setUpdState('idle');
        } else {
          setUpdState('none');
        }
      })
      .catch(() => setUpdState('error'));
  };

  return (
    <div className="overlay" onClick={onClose}>
      <div className="modal wide" onClick={(e) => e.stopPropagation()}>
        <h2>
          <span className="brand">
            <span className="name">invoices</span>
            <span className="dot">.</span>
          </span>{' '}
          — {t.settings}
        </h2>

        <label className="field">
          <span>{t.language}</span>
          <select value={s.language} onChange={(e) => set('language', e.target.value as 'de' | 'en')}>
            <option value="de">Deutsch</option>
            <option value="en">English</option>
          </select>
        </label>

        <div className="sep" />
        <div className="fieldlabel">{t.secAppearance}</div>
        <div className="row2">
          <label className="field grow1">
            <span>{t.themeLabel}</span>
            <select
              value={s.theme}
              onChange={(e) => set('theme', e.target.value as Settings['theme'])}
            >
              <option value="dark">{t.themeDark}</option>
              <option value="light">{t.themeLight}</option>
            </select>
          </label>
          <label className="field grow1">
            <span>{t.accentLabel}</span>
            <select
              value={s.accent}
              onChange={(e) => set('accent', e.target.value as Settings['accent'])}
            >
              <option value="sky">{t.accentSky}</option>
              <option value="emerald">{t.accentEmerald}</option>
              <option value="violet">{t.accentViolet}</option>
              <option value="amber">{t.accentAmber}</option>
            </select>
          </label>
        </div>

        <div className="sep" />
        <div className="fieldlabel">{t.secCompany}</div>
        <div className="note">{t.companyHint}</div>
        <label className="field">
          <span>{t.companyName}</span>
          <input type="text" value={s.companyName} onChange={(e) => set('companyName', e.target.value)} />
        </label>
        <label className="field">
          <span>{t.companyAddress}</span>
          <textarea rows={3} value={s.companyAddress} onChange={(e) => set('companyAddress', e.target.value)} />
        </label>
        <div className="row2">
          <label className="field grow1">
            <span>{t.companyEmail}</span>
            <input type="text" value={s.companyEmail} onChange={(e) => set('companyEmail', e.target.value)} />
          </label>
          <label className="field grow1">
            <span>{t.companyPhone}</span>
            <input type="text" value={s.companyPhone} onChange={(e) => set('companyPhone', e.target.value)} />
          </label>
        </div>
        <div className="row2">
          <label className="field grow1">
            <span>{t.taxNumber}</span>
            <input type="text" value={s.taxNumber} onChange={(e) => set('taxNumber', e.target.value)} />
          </label>
          <label className="field grow1">
            <span>{t.vatId}</span>
            <input type="text" value={s.vatId} onChange={(e) => set('vatId', e.target.value)} />
          </label>
        </div>
        <label className="field">
          <span>{t.bankName}</span>
          <input type="text" value={s.bankName} onChange={(e) => set('bankName', e.target.value)} />
        </label>
        <div className="row2">
          <label className="field grow1">
            <span>{t.iban}</span>
            <input type="text" value={s.iban} onChange={(e) => set('iban', e.target.value)} />
          </label>
          <label className="field grow1">
            <span>{t.bic}</span>
            <input type="text" value={s.bic} onChange={(e) => set('bic', e.target.value)} />
          </label>
        </div>
        <label className="field">
          <span>{t.logoPath}</span>
          <div className="row2">
            <input type="text" value={s.logoPath} onChange={(e) => set('logoPath', e.target.value)} />
            <button onClick={pickLogo}>{t.chooseLogo}</button>
          </div>
        </label>

        <div className="sep" />
        <div className="fieldlabel">{t.secInvoicing}</div>
        <label className="check">
          <input
            type="checkbox"
            checked={s.smallBusiness}
            onChange={(e) => set('smallBusiness', e.target.checked)}
          />
          {t.smallBusiness}
        </label>
        <div className="row3">
          <label className="field">
            <span>{t.defaultVatRate}</span>
            <select
              value={s.defaultVatRate}
              onChange={(e) => set('defaultVatRate', Number(e.target.value))}
            >
              <option value={19}>19 %</option>
              <option value={7}>7 %</option>
              <option value={0}>0 %</option>
            </select>
          </label>
          <label className="field">
            <span>{t.paymentTerms}</span>
            <input
              type="number"
              min={0}
              max={90}
              value={s.paymentTermsDays}
              onChange={(e) => set('paymentTermsDays', Math.max(0, Number(e.target.value) || 0))}
            />
          </label>
          <label className="field">
            <span>{t.invoicePrefix}</span>
            <input
              type="text"
              value={s.invoicePrefix}
              onChange={(e) => set('invoicePrefix', e.target.value.toUpperCase())}
            />
          </label>
        </div>
        <div className="row2">
          <label className="field grow1">
            <span>{t.creditPrefix}</span>
            <input
              type="text"
              value={s.creditPrefix}
              onChange={(e) => set('creditPrefix', e.target.value.toUpperCase())}
            />
          </label>
          <label className="field grow1">
            <span>{t.cancelPrefix}</span>
            <input
              type="text"
              value={s.cancelPrefix}
              onChange={(e) => set('cancelPrefix', e.target.value.toUpperCase())}
            />
          </label>
        </div>

        <div className="sep" />
        <div className="fieldlabel">{t.secNumbering}</div>
        <label className="check">
          <input
            type="checkbox"
            checked={s.numberIncludeYear}
            onChange={(e) => set('numberIncludeYear', e.target.checked)}
          />
          {t.numberIncludeYear}
        </label>
        <div className="row2">
          <label className="field grow1">
            <span>{t.numberDigits}</span>
            <select
              value={s.numberDigits}
              onChange={(e) => set('numberDigits', Number(e.target.value))}
            >
              {[3, 4, 5, 6].map((d) => (
                <option key={d} value={d}>
                  {d}
                </option>
              ))}
            </select>
          </label>
          <label className="field grow1">
            <span>{t.numberSeparator}</span>
            <select
              value={s.numberSeparator}
              onChange={(e) => set('numberSeparator', e.target.value)}
            >
              <option value="-">-</option>
              <option value="/">/</option>
              <option value="">{t.sepNone}</option>
            </select>
          </label>
        </div>
        <div className="note">
          {'// '}
          {t.numberPreviewLabel}:{' '}
          {numberPreview(s.invoicePrefix, s.numberIncludeYear, s.numberDigits, s.numberSeparator)}
        </div>

        <div className="row2">
          <label className="field grow1">
            <span>{t.defaultUnit}</span>
            <input
              type="text"
              value={s.defaultUnit}
              onChange={(e) => set('defaultUnit', e.target.value)}
            />
          </label>
          <label className="field grow1">
            <span>{t.countryCode}</span>
            <input
              type="text"
              value={s.countryCode}
              placeholder="DE"
              onChange={(e) => set('countryCode', e.target.value.toUpperCase())}
            />
          </label>
        </div>
        <label className="field">
          <span>{t.defaultIntro}</span>
          <input
            type="text"
            value={s.defaultIntro}
            onChange={(e) => set('defaultIntro', e.target.value)}
          />
        </label>

        <div className="sep" />
        <div className="fieldlabel">{t.secEInvoice}</div>
        <div className="note">{t.einvoiceNote}</div>

        <label className="field">
          <span>{t.pdfFooter}</span>
          <input
            type="text"
            placeholder={t.pdfFooterHint}
            value={s.pdfFooter}
            onChange={(e) => set('pdfFooter', e.target.value)}
          />
        </label>

        <div className="sep" />
        <div className="fieldlabel">{t.updates}</div>
        <label className="check">
          <input
            type="checkbox"
            checked={s.autoUpdate}
            onChange={(e) => set('autoUpdate', e.target.checked)}
          />
          {t.autoUpdate}
        </label>
        <div className="updatebox">
          <span>
            {t.version} {APP_VERSION}
          </span>
          <button onClick={checkUpdates} disabled={updState === 'checking'}>
            {updState === 'checking' ? t.checking : t.checkUpdates}
          </button>
          {updState === 'none' && <span>{t.upToDate}</span>}
          {updState === 'error' && <span style={{ color: 'var(--red)' }}>{t.updateError}</span>}
          {update && (
            <>
              <span>
                {t.updateAvailable} <strong>{update.version}</strong>
              </span>
              <button
                className="primary"
                disabled={installing}
                onClick={() => {
                  setInstalling(true);
                  api.installUpdate().catch(() => setInstalling(false));
                }}
              >
                {t.installUpdate}
              </button>
            </>
          )}
        </div>
        {update?.notes && <div className="note">{update.notes}</div>}

        <div className="note">{t.dataNote}</div>

        <div className="btnrow">
          <button onClick={onClose}>{t.cancel}</button>
          <button
            className="primary"
            disabled={!s.invoicePrefix.trim() || !s.creditPrefix.trim() || !s.cancelPrefix.trim()}
            onClick={() => onSave(s)}
          >
            {t.save}
          </button>
        </div>
      </div>
    </div>
  );
}
