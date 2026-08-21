// 20 PDF-Farbschemata — jeweils vier Rollen-Farben:
// c1 Titel/Überschriften · c2 Akzent/Band · c3 Tabellenkopf · c4 Zebra/Ton.
// Von nüchtern bis mutig, inklusive gedeckter Kombinationen — jede Rolle
// bleibt per Colorpicker individuell übersteuerbar ("custom").

export interface Scheme {
  id: string;
  c1: string;
  c2: string;
  c3: string;
  c4: string;
}

export const SCHEMES: Scheme[] = [
  { id: 'ink', c1: '#111827', c2: '#374151', c3: '#111827', c4: '#f3f4f6' },
  { id: 'ocean', c1: '#0c4a6e', c2: '#0284c7', c3: '#0c4a6e', c4: '#e0f2fe' },
  { id: 'emerald', c1: '#064e3b', c2: '#059669', c3: '#065f46', c4: '#d1fae5' },
  { id: 'amber', c1: '#7c2d12', c2: '#d97706', c3: '#92400e', c4: '#fef3c7' },
  { id: 'violet', c1: '#4c1d95', c2: '#7c3aed', c3: '#5b21b6', c4: '#ede9fe' },
  { id: 'bordeaux', c1: '#7f1d1d', c2: '#b91c1c', c3: '#7f1d1d', c4: '#fee2e2' },
  { id: 'slate', c1: '#0f172a', c2: '#475569', c3: '#1e293b', c4: '#e2e8f0' },
  { id: 'petrol', c1: '#134e4a', c2: '#0d9488', c3: '#115e59', c4: '#ccfbf1' },
  { id: 'midnight', c1: '#1e1b4b', c2: '#4338ca', c3: '#312e81', c4: '#e0e7ff' },
  { id: 'rose', c1: '#831843', c2: '#db2777', c3: '#9d174d', c4: '#fce7f3' },
  { id: 'olive', c1: '#3f6212', c2: '#65a30d', c3: '#4d7c0f', c4: '#ecfccb' },
  { id: 'copper', c1: '#431407', c2: '#ea580c', c3: '#9a3412', c4: '#ffedd5' },
  { id: 'graphite-yellow', c1: '#18181b', c2: '#eab308', c3: '#18181b', c4: '#fef9c3' },
  { id: 'navy-coral', c1: '#14304d', c2: '#f43f5e', c3: '#14304d', c4: '#ffe4e6' },
  { id: 'sand', c1: '#57534e', c2: '#a8a29e', c3: '#78716c', c4: '#f5f5f4' },
  { id: 'lilac-muted', c1: '#4a4e69', c2: '#9a8c98', c3: '#4a4e69', c4: '#f2e9e4' },
  { id: 'fir-gold', c1: '#14532d', c2: '#ca8a04', c3: '#14532d', c4: '#fef9c3' },
  { id: 'iceblue', c1: '#164e63', c2: '#06b6d4', c3: '#155e75', c4: '#cffafe' },
  { id: 'terracotta', c1: '#7c2d12', c2: '#cc7a4e', c3: '#9a3412', c4: '#f9e8dc' },
  { id: 'mono-bold', c1: '#000000', c2: '#000000', c3: '#000000', c4: '#e5e5e5' },
];

export function hexToRgb(hex: string): [number, number, number] {
  const m = hex.trim().match(/^#?([0-9a-f]{6})$/i);
  if (!m) return [17, 24, 39];
  const n = parseInt(m[1], 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

/// Weißer oder dunkler Text auf gegebener Hintergrundfarbe (Luminanz).
export function contrastText(hex: string): [number, number, number] {
  const [r, g, b] = hexToRgb(hex);
  const lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;
  return lum > 150 ? [30, 30, 30] : [255, 255, 255];
}
