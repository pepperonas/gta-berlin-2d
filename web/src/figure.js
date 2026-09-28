// Menschen-Typen (rein, ohne DOM): wer unterwegs ist, hängt von Ort, Uhrzeit und Wochentag ab; das Aussehen folgt aus
// Typ und Nummer. Alles deterministisch aus der Nummer (nie aus dem Welt-Zufall – der würde Verkehr und Tests ändern).
// Verhalten je Typ: Gehtempo (speed), Zubehör, das die Haltung bestimmt (Stock, Kinderwagen, Aktentasche).

// weight: Grundgewicht; bez: Faktor je Bezirk; hours: [von, bis, Faktor außerhalb]; weekday: nur Mo–Fr stark
export const KINDS = {
  everyday: { label: 'Alltag', weight: 34, speed: 1 },
  business: { label: 'Büro', weight: 9, speed: 1.12, hours: [420, 1170, 0.15], weekday: 0.2, bez: { Mitte: 2.6, 'Charlottenburg-Wilmersdorf': 1.8, 'Friedrichshain-Kreuzberg': 1.2 } },
  tourist: { label: 'Tourist', weight: 7, speed: 0.78, hours: [540, 1230, 0.1], bez: { Mitte: 4, 'Friedrichshain-Kreuzberg': 1.8, 'Charlottenburg-Wilmersdorf': 1.6, 'Tempelhof-Schöneberg': 1.1 } },
  senior: { label: 'Senior', weight: 9, speed: 0.62, hours: [480, 1110, 0.12], bez: { 'Steglitz-Zehlendorf': 1.8, Spandau: 1.5, Reinickendorf: 1.5, 'Treptow-Köpenick': 1.4, 'Marzahn-Hellersdorf': 1.3 } },
  teen: { label: 'Jugendliche', weight: 8, speed: 1.15, hours: [780, 1320, 0.2], bez: { Neukölln: 1.3, 'Marzahn-Hellersdorf': 1.3 } },
  hipster: { label: 'Kiez', weight: 7, speed: 0.95, hours: [600, 1560, 0.3], bez: { 'Friedrichshain-Kreuzberg': 3, Neukölln: 2.4, Pankow: 1.8, Mitte: 1.2, 'Steglitz-Zehlendorf': 0.3, Spandau: 0.3 } },
  worker: { label: 'Handwerk', weight: 5, speed: 1.02, hours: [360, 1050, 0.08], weekday: 0.15 },
  punk: { label: 'Punk', weight: 1.5, speed: 1, hours: [720, 1620, 0.4], bez: { 'Friedrichshain-Kreuzberg': 3.5, Neukölln: 1.8, 'Steglitz-Zehlendorf': 0.1, Spandau: 0.2 } },
  headscarf: { label: 'Kopftuch', weight: 4, speed: 0.9, hours: [480, 1200, 0.2], bez: { Neukölln: 3, Mitte: 1.8, 'Friedrichshain-Kreuzberg': 2, Spandau: 1.4, 'Steglitz-Zehlendorf': 0.4 } },
  parent: { label: 'Kinderwagen', weight: 5, speed: 0.72, hours: [540, 1110, 0.03], bez: { Pankow: 2, 'Friedrichshain-Kreuzberg': 1.5 } },
  jogger: { label: 'Jogger', weight: 0, speed: 2.3 },
  dogwalker: { label: 'Gassi', weight: 0, speed: 0.85 },
};
export const KIND_IDS = Object.keys(KINDS);

// Reiner Hash 0…1 aus Nummer und Kanal
export const h01 = (id, k) => { const x = Math.sin((id ?? 1) * 127.1 + k * 311.7) * 43758.5453; return x - Math.floor(x); };

const inWindow = (m, a, b) => (b > 1440 ? m >= a || m < b - 1440 : m >= a && m < b);

// Gewichte je Typ an diesem Ort zu dieser Zeit. ctx: { minutes, day (0 = Mo), bezirk, cold (0…1), act }
export function kindWeights(ctx = {}) {
  const m = (((ctx.minutes ?? 720) % 1440) + 1440) % 1440, weekend = (ctx.day ?? 0) >= 5, night = m < 360 || m >= 1320;
  const out = {};
  for (const id of KIND_IDS) {
    const k = KINDS[id];
    let w = k.weight;
    if (!w) { out[id] = 0; continue; }
    if (k.hours && !inWindow(m, k.hours[0], k.hours[1])) w *= k.hours[2];
    if (k.weekday !== undefined && weekend) w *= k.weekday;
    w *= k.bez?.[ctx.bezirk] ?? 1;
    if (night && id === 'everyday') w *= 0.8;
    // Tätigkeiten: Kinderwagen sitzt nicht auf der Wiese, Senioren spielen keine Gitarre, Büroleute rauchen vor der Tür
    if (ctx.act) {
      if (id === 'parent' && ctx.act !== 'browse' && ctx.act !== 'queue' && ctx.act !== 'chat') w = 0;
      if (ctx.act === 'music') w *= id === 'hipster' || id === 'punk' ? 4 : id === 'teen' ? 1.5 : id === 'everyday' ? 1 : 0.1;
      if (ctx.act === 'drink') w *= id === 'punk' || id === 'hipster' || id === 'worker' ? 2.5 : id === 'senior' || id === 'headscarf' || id === 'tourist' ? 0.3 : 1;
      if (ctx.act === 'smoke') w *= id === 'business' || id === 'worker' || id === 'punk' ? 1.8 : 1;
      if (ctx.act === 'browse') w *= id === 'tourist' || id === 'senior' || id === 'headscarf' ? 1.8 : 1;
    }
    out[id] = w;
  }
  return out;
}

// Typ wählen: deterministisch aus der Nummer, gewichtet nach Ort und Zeit
export function pickKind(id, ctx = {}) {
  const w = kindWeights(ctx);
  let sum = 0;
  for (const k of KIND_IDS) sum += w[k];
  let r = h01(id, 17) * sum;
  for (const k of KIND_IDS) { r -= w[k]; if (r < 0 && w[k] > 0) return k; }
  return 'everyday';
}

// --- Aussehen ---------------------------------------------------------------------------------------------------------
const PANTS = ['#2c3e50', '#34495e', '#1f2a36', '#5d4e3c', '#3d5a80', '#6b6b6b', '#2b2b2b', '#7a5c3a'];
const JEANS = ['#3d5a80', '#2c3e66', '#4a6a94', '#26324a', '#1d2433'];
const HAIR = ['#2b2118', '#4a3524', '#1a1a1a', '#8a6a3d', '#c9a45a', '#a33b20', '#5a4636'];
const GREY = ['#d8d4cf', '#b9b5ae', '#e8e6e2', '#9a968f'];
const SKIN = ['#f2d0b1', '#e0ac69', '#c68642', '#8d5524', '#f5d6c6'];
const BAGS = ['#6d4c2f', '#2f3b52', '#8a2f2f', '#3d6b4f', '#1f1f1f'];
const SHOES = ['#1e2126', '#3a2a1c', '#f0f0f0', '#2c2c34', '#7a5230'];
const pick = (a, r) => a[Math.floor(r * a.length) % a.length];

// Stil-Tabelle je Typ: Oberteil (Schnitt + Farben), Hose, Haare, Kopfbedeckung, Zubehör, Statur
const STYLE = {
  everyday: (h, p) => ({ top: h(5) < 0.3 ? 'jacket' : 'tee', topColor: p.shirt ?? pick(['#e74c3c', '#3498db', '#2ecc71', '#9b59b6', '#f39c12', '#1abc9c', '#ecf0f1', '#34495e'], h(6)),
    pants: h(7) < 0.55 ? pick(JEANS, h(8)) : pick(PANTS, h(8)), hat: h(9) < 0.12 ? 'cap' : null, bag: h(4) < 0.2 ? 'backpack' : h(4) < 0.32 ? 'tote' : null }),
  business: (h) => ({ top: 'suit', topColor: pick(['#1f2733', '#2b2d33', '#3a3f4a', '#2a3a55', '#4a4038'], h(6)), tie: pick(['#8a2f2f', '#2f4f8a', '#6b5a2a', '#4a4a4a'], h(10)),
    pants: null, shoes: '#15171b', hairStyle: h(11) < 0.3 ? 'long' : h(11) < 0.4 ? 'bald' : 'short', acc: h(12) < 0.6 ? 'briefcase' : 'phone' }),
  tourist: (h) => ({ top: 'tee', topColor: pick(['#ff6b6b', '#ffd93d', '#6bcB77', '#4d96ff', '#ffffff', '#ff9f43'], h(6)), pants: pick(['#c8b48a', '#8a7a5a', '#5d6b7a', '#e0d6c0'], h(7)),
    hat: h(9) < 0.6 ? 'cap' : h(9) < 0.8 ? 'sunhat' : null, hatColor: pick(['#f5f5f5', '#e8c46a', '#2f5aa8', '#c0392b'], h(13)), bag: 'backpack', acc: 'camera' }),
  senior: (h) => ({ top: 'coat', topColor: pick(['#8a7a62', '#6b5a48', '#5a6068', '#7a6a8a', '#a08a6a', '#4a5a4a'], h(6)), pants: pick(['#5a5a5a', '#6b5d4a', '#3f4550'], h(7)),
    hair: pick(GREY, h(2)), hairStyle: h(11) < 0.3 ? 'bald' : h(11) < 0.55 ? 'bun' : 'short', hat: h(9) < 0.35 ? 'hat' : null, hatColor: pick(['#4a3a2a', '#3a3a3a', '#6a5a4a'], h(13)),
    acc: h(12) < 0.55 ? 'cane' : h(12) < 0.75 ? 'shopping' : null, stoop: 1, scale: 0.96 }),
  teen: (h) => ({ top: 'hoodie', topColor: pick(['#2d3436', '#6c5ce7', '#e17055', '#00b894', '#fdcb6e', '#b2bec3', '#d63031'], h(6)), pants: pick(['#1d1d1d', '#3d5a80', '#6b6b6b'], h(7)),
    shoes: h(14) < 0.6 ? '#f0f0f0' : '#1e2126', hat: h(9) < 0.35 ? 'capback' : null, hatColor: pick(['#1d1d1d', '#c0392b', '#f5f5f5'], h(13)), acc: h(12) < 0.5 ? 'phone' : null,
    headphones: h(15) < 0.4, scale: 0.86, bag: h(4) < 0.4 ? 'backpack' : null }),
  hipster: (h) => ({ top: h(5) < 0.5 ? 'jacket' : 'tee', topColor: pick(['#5b6b4a', '#7a4a3a', '#3a4a5a', '#c9a24a', '#2b2b2b', '#8a6a8a'], h(6)), pants: pick(['#1d1d1d', '#3a3a3a', '#26324a'], h(7)),
    hat: h(9) < 0.55 ? 'beanie' : null, hatColor: pick(['#c0392b', '#e1a95f', '#2f3b52', '#6b8a4a', '#1d1d1d'], h(13)), beard: h(16) < 0.55, bag: 'tote', bagColor: '#e8dcc0', hairStyle: h(11) < 0.3 ? 'bun' : 'short' }),
  worker: (h) => ({ top: 'vest', topColor: pick(['#ff8c1a', '#e8e82a'], h(6)), under: pick(['#3a4a5a', '#5a4a3a', '#2b2b2b'], h(7)), pants: pick(['#3a4a5a', '#2b3a2b', '#4a3a2a'], h(8)),
    hat: h(9) < 0.5 ? 'helmet' : h(9) < 0.7 ? 'beanie' : null, hatColor: h(13) < 0.6 ? '#f5f5f5' : '#f1c40f', shoes: '#4a3a24', scale: 1.06 }),
  punk: (h) => ({ top: 'jacket', topColor: '#1d1d1d', studs: true, pants: h(7) < 0.5 ? '#1d1d1d' : '#6a2a2a', hairStyle: 'mohawk', hair: pick(['#e84393', '#00cec9', '#fdcb6e', '#d63031', '#6c5ce7', '#2ecc71'], h(2)),
    shoes: '#15171b', acc: h(12) < 0.3 ? 'bottle' : null }),
  headscarf: (h) => ({ top: 'coat', topColor: pick(['#3a3a4a', '#5a4a5a', '#2f3b52', '#6b5a4a', '#4a5a5a'], h(6)), pants: '#2b2b2b', hat: 'scarf', hatColor: pick(['#8a2f5a', '#2f5a8a', '#d8c8b8', '#5a8a6a', '#1d1d1d', '#c9a24a'], h(13)),
    acc: h(12) < 0.5 ? 'shopping' : null }),
  parent: (h) => ({ top: h(5) < 0.5 ? 'jacket' : 'tee', topColor: pick(['#5a8aa8', '#a85a6a', '#6a8a5a', '#8a7a5a', '#4a4a5a'], h(6)), pants: pick(JEANS, h(7)), acc: 'stroller',
    strollerColor: pick(['#2f3b52', '#6b3a4a', '#3a5a4a', '#5a5a5a', '#8a6a3a'], h(10)), hairStyle: h(11) < 0.5 ? 'long' : 'short' }),
  jogger: (h, p) => ({ top: 'sport', topColor: p.shirt ?? pick(['#e84393', '#00b894', '#0984e3', '#fdcb6e', '#d63031'], h(6)), pants: '#1d1d1d', shorts: h(7) < 0.6, shoes: pick(['#f0f0f0', '#ff6b6b', '#4d96ff'], h(14)),
    headphones: h(15) < 0.6, hairStyle: h(11) < 0.4 ? 'ponytail' : 'short' }),
  dogwalker: (h) => ({ top: h(5) < 0.5 ? 'coat' : 'jacket', topColor: pick(['#4a5a3a', '#5a4a3a', '#2f3b52', '#7a6a5a'], h(6)), pants: pick(JEANS, h(7)), hat: h(9) < 0.3 ? 'beanie' : null, hatColor: pick(['#6b8a4a', '#2f3b52'], h(13)) }),
};

// Vollständiges Aussehen: { kind, top, topColor, under, tie, pants, shorts, shoes, skin, hair, hairStyle, hat, hatColor, beard,
// headphones, bag, bagColor, acc, strollerColor, stoop, scale, studs }. Einmal je Person berechnet.
export function figureLook(p, kind = p.kind ?? 'everyday') {
  if (p._fig && p._fig.kind === kind && p._fig.shirt === p.shirt) return p._fig;
  const h = (k) => h01(p.id, k);
  const s = (STYLE[kind] ?? STYLE.everyday)(h, p);
  const long = h(11) < 0.4;
  const look = {
    kind, shirt: p.shirt,
    top: s.top ?? 'tee', topColor: s.topColor, under: s.under ?? null, tie: s.tie ?? null, studs: !!s.studs,
    pants: s.pants ?? s.topColor, shorts: !!s.shorts, shoes: s.shoes ?? pick(SHOES, h(14)),
    skin: p.skin ?? pick(SKIN, h(3)), hair: s.hair ?? pick(HAIR, h(2)),
    hairStyle: s.hairStyle ?? (long ? 'long' : h(11) < 0.5 ? 'curly' : 'short'),
    hat: s.hat ?? null, hatColor: s.hatColor ?? '#2b2b2b', beard: !!s.beard, headphones: !!s.headphones,
    bag: s.bag ?? null, bagColor: s.bagColor ?? pick(BAGS, h(18)),
    acc: s.acc ?? null, strollerColor: s.strollerColor ?? '#2f3b52', stoop: s.stoop ?? 0, scale: s.scale ?? 1,
  };
  if (p.id !== undefined) p._fig = look;
  return look;
}

// Der Spieler: orange Jacke mit dunklem Saum, dunkle Jeans, weiße Turnschuhe, kurze dunkle Haare
export const PLAYER_LOOK = Object.freeze({
  kind: 'player', top: 'jacket', topColor: '#ff7a1a', under: '#2b2b2b', tie: null, studs: false, pants: '#26324a', shorts: false, shoes: '#f2f2f2',
  skin: '#f2d0b1', hair: '#2b2118', hairStyle: 'short', hat: null, hatColor: null, beard: false, headphones: false,
  bag: null, bagColor: null, acc: null, strollerColor: null, stoop: 0, scale: 1.05,
});
