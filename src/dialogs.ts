import { confirm, message } from '@tauri-apps/plugin-dialog';

// Native Rückfragen und Hinweise. NIE window.confirm/alert/prompt nutzen:
// tauri-plugin-dialog 2.7 ersetzt sie im Webview durch asynchrone Stubs —
// `if (window.confirm(…))` wäre dann immer wahr (die Aktion liefe auch bei
// „Abbrechen“), window.alert zeigte nichts. Diese Helfer gehen über den
// Plugin-Befehl `message` (Berechtigung in `dialog:default`) und werden
// immer mit `await` benutzt.

export interface ConfirmLabels {
  ok: string;
  cancel: string;
}

/** Ja/Nein-Rückfrage; `true` nur bei ausdrücklicher Bestätigung. */
export async function confirmDialog(
  text: string,
  labels: ConfirmLabels,
  kind: 'warning' | 'info' = 'warning'
): Promise<boolean> {
  try {
    return await confirm(text, {
      title: 'invoices',
      kind,
      okLabel: labels.ok,
      cancelLabel: labels.cancel,
    });
  } catch {
    // Dialog nicht verfügbar → sicherheitshalber NICHT ausführen
    return false;
  }
}

/** Hinweis mit OK-Knopf; `false`, wenn er nicht angezeigt werden konnte
 *  (dann zeigt der Aufrufer den Text anders an, z. B. als Toast). */
export async function notifyDialog(
  text: string,
  okLabel: string,
  kind: 'warning' | 'info' | 'error' = 'warning'
): Promise<boolean> {
  try {
    await message(text, { title: 'invoices', kind, okLabel });
    return true;
  } catch {
    return false;
  }
}
