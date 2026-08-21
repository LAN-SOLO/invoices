import { useState } from 'react';
import { open, save } from '@tauri-apps/plugin-dialog';
import { api, numberPreview, Settings, UpdateInfo } from '../api';
import { Dict } from '../i18n';

const APP_VERSION = '0.3.0';

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
  const [backupMsg, setBackupMsg] = useState('');

  const backupNow = async () => {
    const stamp = new Date().toISOString().slice(0, 10);
    const path = await save({
      defaultPath: `invoices-backup-${stamp}.json`,
      filters: [{ name: 'JSON', extensions: ['json'] }],
    });
    if (!path) return;
    try {
      await api.exportBackup(path);
      setBackupMsg(t.backupDone);
    } catch (e) {
      setBackupMsg(String(e));
    }
  };

  const restoreBackup = async () => {
    const picked = await open({ filters: [{ name: 'JSON', extensions: ['json'] }] });
    if (typeof picked !== 'string') return;
    if (!window.confirm(t.restoreConfirm)) return;
    try {
      await api.importBackup(picked);
      setBackupMsg(t.restoreDone);
      // Einstellungen wurden mit-restauriert — Modal-Zustand wäre veraltet
      api.getSettings().then((fresh) => setS(fresh));
    } catch {
      setBackupMsg(t.restoreFailed);
    }
  };

  const pickBackupDir = async () => {
    const picked = await open({ directory: true });
    if (typeof picked === 'string') set('backupDir', picked);
  };

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
          <label className="field grow1">
            <span>{t.pdfLayoutLabel}</span>
            <select
              value={s.pdfLayout}
              onChange={(e) => set('pdfLayout', e.target.value as Settings['pdfLayout'])}
            >
              <option value="classic">{t.layoutClassic}</option>
              <option value="modern">{t.layoutModern}</option>
              <option value="compact">{t.layoutCompact}</option>
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
        <div className="row3">
          <label className="field">
            <span>{t.quotePrefix}</span>
            <input
              type="text"
              value={s.quotePrefix}
              onChange={(e) => set('quotePrefix', e.target.value.toUpperCase())}
            />
          </label>
          <label className="field">
            <span>{t.orderPrefix}</span>
            <input
              type="text"
              value={s.orderPrefix}
              onChange={(e) => set('orderPrefix', e.target.value.toUpperCase())}
            />
          </label>
          <label className="field">
            <span>{t.deliveryPrefix}</span>
            <input
              type="text"
              value={s.deliveryPrefix}
              onChange={(e) => set('deliveryPrefix', e.target.value.toUpperCase())}
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
        <div className="fieldlabel">{t.secSecurity}</div>
        <div className="note">{t.backupNote}</div>
        <div className="row2" style={{ marginBottom: 10 }}>
          <button onClick={backupNow}>{t.backupNow}</button>
          <button onClick={restoreBackup}>{t.backupRestore}</button>
          {backupMsg && <span className="dim">{backupMsg}</span>}
        </div>
        <label className="check">
          <input
            type="checkbox"
            checked={s.autoBackup}
            onChange={(e) => set('autoBackup', e.target.checked)}
          />
          {t.autoBackupLabel}
        </label>
        {s.autoBackup && (
          <label className="field">
            <span>{t.backupDir}</span>
            <div className="row2">
              <input
                type="text"
                value={s.backupDir}
                onChange={(e) => set('backupDir', e.target.value)}
              />
              <button onClick={pickBackupDir}>{t.chooseLogo}</button>
            </div>
          </label>
        )}
        <div className="billed-row">
          <span className="chip mini">{t.billedSoon}</span> {t.cloudBackup}
        </div>
        <div className="billed-row">
          <span className="chip mini">{t.billedSoon}</span> {t.scheduleBackup}
        </div>
        <div className="billed-row">
          <span className="chip mini">{t.billedSoon}</span> {t.encryptedBackup}
        </div>

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
