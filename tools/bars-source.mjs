// Bar-Auslastung aus gostumblr (github.com/pepperonas/gostumblr) holen: GET /api/v1/bars/busyness (alle Bars mit
// Koordinaten, Live-Auslastung, „üblich“ und 24-h-Verlauf) plus der Wochenschnitt aller Bars je Wochentag
// (GET /api/v1/bars/busyness/weekly?dow=0..6, 0 = Sonntag). Beides ohne Anmeldung. Geteilt von tools/serve.mjs
// (Live-Durchreichung) und tools/fetch-bars.mjs (Schnappschuss).
export const GOSTUMBLR = 'https://app.gostumblr.com/api/v1/bars/busyness';

const headers = () => ({ accept: 'application/json', ...(process.env.BARS_TOKEN ? { authorization: `Bearer ${process.env.BARS_TOKEN}` } : {}) });
async function getJson(url) {
  const r = await fetch(url, { headers: headers() });
  if (!r.ok) throw new Error(`${url}: HTTP ${r.status}`);
  return r.json();
}

// url: die Bars-Adresse. Endet sie auf /bars/busyness, kommt der Wochenschnitt dazu (Fehler dort sind egal: dann
// klingt jede Bar nach ihrem 24-h-Verlauf und dem typischen Tagesgang). Liefert das Feed-Dokument für das Spiel.
export async function fetchBars(url = GOSTUMBLR, { weekly = true } = {}) {
  const doc = await getJson(url);
  const out = Array.isArray(doc) ? { bars: doc } : { ...doc };
  if (weekly && /\/bars\/busyness\/?$/.test(new URL(url).pathname) && !out.weekly) {
    const base = url.replace(/\/?(\?.*)?$/, '');
    const days = await Promise.all([0, 1, 2, 3, 4, 5, 6].map((dow) => getJson(`${base}/weekly?dow=${dow}`).catch(() => null)));
    if (days.some(Boolean)) out.weekly = days.filter(Boolean).map((d) => ({ dow: d.dow, hours: (d.hours ?? []).map((h) => ({ hour: h.hour, avg_occupancy: h.avg_occupancy })) }));
  }
  out.at ??= Math.round(Date.now() / 1000);
  return out;
}
