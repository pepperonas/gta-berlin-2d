// Dunkle Auspuffabstimmung: breitere Druckpulse reduzieren scharfe Obertöne.
// Akustische Charaktere: Auspuffimpulse, Ansaugung, Resonanzen, Turbo und Dämmung.
// Syntheseprofile, keine Aufnahmen konkreter Fabrikate. Ein Zyklus entspricht 720° (Zweitakter: 360°).
const P = (resonance, brightness, rough, width, volume, turbo = 0, insulation = 0.6, bank = 0) =>
  ({ resonance, brightness, rough, width, volume, turbo, insulation, bank });
export const VOICES = {
  triple: P(120, 1000, 0.16, 0.044, 0.9, 0.35),
  four: P(130, 1050, 0.09, 0.038, 0.82, 0.35),
  six: P(110, 1150, 0.06, 0.035, 0.82, 0.3, 0.76),
  five: P(125, 1150, 0.14, 0.04, 0.98, 0.4),
  boxer: P(105, 950, 0.17, 0.048, 1, 0, 0.32, 0.3),
  flatSix: P(130, 1550, 0.13, 0.035, 1.12, 0, 0.38, 0.23),
  v8: P(85, 850, 0.3, 0.065, 1.25, 0, 0.32, 0.62),
  v8turbo: P(95, 1150, 0.21, 0.05, 1.12, 0.85, 0.64, 0.45),
  flatV8: P(140, 1800, 0.1, 0.032, 1.12, 0, 0.38),
  v10: P(155, 2100, 0.085, 0.03, 1.15, 0, 0.3, 0.18),
  diesel4: P(100, 800, 0.19, 0.034, 0.97, 0.72, 0.65),
  diesel6: P(80, 700, 0.15, 0.044, 1.05, 0.8, 0.65),
  heavy: P(65, 600, 0.22, 0.055, 1.3, 1, 0.4),
  twoStroke: P(310, 2700, 0.21, 0.012, 0.94, 0, 0.22),
  bike: P(310, 4300, 0.045, 0.009, 0.85, 0, 0),
  single: P(160, 1800, 0.24, 0.05, 0.78, 0, 0),
  electric: P(0, 0, 0, 0, 0.55, 0, 0.82),
};
export const MODEL_VOICES = {
  zweitakter: 'twoStroke', kleinwagen: 'triple', kompakt: 'four', limousine: 'six', taxi: 'diesel4', kombi: 'five',
  transporter: 'diesel4', gelaende: 'diesel6', elektro: 'electric', sportwagen: 'flatV8', heckcoupe: 'flatSix',
  hothatch: 'four', roadster: 'four', musclecar: 'v8', oldtimer: 'six', pickup: 'diesel4', kleinbus: 'boxer', rallye: 'triple',
  supersport: 'v10', gtcoupe: 'v8turbo', leichtbau: 'four', elektrosport: 'electric', sprinter: 'diesel4', hochdach: 'diesel4',
  powerkombi: 'v8turbo', familienkombi: 'diesel4', business: 'six', sportlimo: 'six', luxus: 'v8turbo', coupe: 'six',
  leichtcoupe: 'boxer', gklasse: 'v8turbo', defender: 'diesel6', niva: 'four', kompaktsuv: 'four', sportsuv: 'six', grosssuv: 'six',
  police: 'six', ambulance: 'diesel4', delivery: 'diesel4', truck: 'heavy', garbage: 'heavy', bus: 'heavy', motorcycle: 'bike', scooter: 'single',
};
const NATURAL = new Set(['roadster', 'niva', 'oldtimer', 'kleinwagen']);
const SPORT = new Set(['hothatch', 'rallye', 'leichtbau', 'sportlimo', 'coupe', 'leichtcoupe']);
const cache = new Map();
export function voiceFor(model, engine = {}) {
  const id = MODEL_VOICES[model] ?? (engine.electric ? 'electric' : engine.diesel ? 'diesel4' : 'four');
  const key = `${model}|${id}`;
  if (!cache.has(key)) {
    const p = { ...VOICES[id], id, key, pops: SPORT.has(model) || model === 'gtcoupe' || model === 'supersport' };
    if (NATURAL.has(model)) p.turbo = 0;
    if (SPORT.has(model)) { p.brightness *= 1.05; p.rough *= 1.2; p.volume *= 1.08; p.insulation *= 0.65; }
    if (model === 'luxus') { p.insulation = 0.94; p.volume *= 0.65; }
    if (model === 'roadster') p.insulation = 0.15;
    if (model === 'oldtimer') { p.rough = 0.14; p.brightness = 1000; p.insulation = 0.35; }
    cache.set(key, Object.freeze(p));
  }
  return cache.get(key);
}

// Fourierkoeffizienten einzelner Druckstöße über einen kompletten Arbeitszyklus.
// Unterschiedliche Bankwege und Pulsstärken bilden insbesondere den tiefen Crossplane-V8-Charakter ab.
export function engineSpectrum(profile, cylinders, intake = false) {
  const real = new Float32Array(129), imag = new Float32Array(129), n = Math.max(1, cylinders);
  const bankOrder = n === 8 ? [0, 1, 1, 0, 1, 0, 0, 1] : Array.from({ length: n }, (_, i) => i % 2);
  for (let h = 1; h < real.length; h++) {
    for (let i = 0; i < n; i++) {
      const bank = bankOrder[i], delay = bank * profile.bank * 0.035;
      const phase = Math.PI * 2 * h * (i / n + delay + (intake ? 0.19 : 0));
      const strength = (1 - bank * profile.bank * 0.45) * (1 + profile.rough * Math.sin(i * 2.39));
      const shape = 1 / (1 + (h * profile.width * (intake ? 1.6 : 1)) ** 2);
      real[h] += Math.cos(phase) * strength * shape / n;
      imag[h] -= Math.sin(phase) * strength * shape / n;
    }
  }
  return { real, imag };
}
