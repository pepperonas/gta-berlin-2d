// Menschen: Typen und Aussehen (figure.js), Gangbild (gait.js), Zeichnung (people.js), Vergabe in der Welt (world.js)
import test from 'node:test';
import assert from 'node:assert/strict';
import { KINDS, KIND_IDS, kindWeights, pickKind, figureLook, PLAYER_LOOK, h01 } from '../web/src/figure.js';
import { createAnim, stepAnim, gaitPose, legFrame, GAIT } from '../web/src/gait.js';
import { drawPerson, ACT_ARMS } from '../web/src/people.js';
import { createWorld, updateWorld, assignKind } from '../web/src/world.js';
import { realCity } from './helpers/city.js';
import { idle } from './helpers/bot.js';

const share = (ids, ctx) => { const n = {}; for (const id of ids) { const k = pickKind(id, ctx); n[k] = (n[k] ?? 0) + 1; } for (const k in n) n[k] /= ids.length; return n; };
const IDS = Array.from({ length: 4000 }, (_, i) => i + 1);

test('Typen: Ort, Uhrzeit und Wochentag bestimmen, wer unterwegs ist', () => {
  const mitteWorkday = share(IDS, { minutes: 12 * 60, day: 2, bezirk: 'Mitte' });
  const mitteSundayNight = share(IDS, { minutes: 23 * 60 + 30, day: 6, bezirk: 'Mitte' });
  assert.ok(mitteWorkday.business > 0.12, `Büroleute mittags in Mitte (${mitteWorkday.business})`);
  assert.ok((mitteSundayNight.business ?? 0) < mitteWorkday.business / 4, 'Sonntagnacht kaum Büroleute');
  assert.ok(mitteWorkday.tourist > 0.08, 'Touristen in Mitte');
  const xberg = share(IDS, { minutes: 21 * 60, day: 4, bezirk: 'Friedrichshain-Kreuzberg' });
  const zehlendorf = share(IDS, { minutes: 21 * 60, day: 4, bezirk: 'Steglitz-Zehlendorf' });
  assert.ok(xberg.hipster + (xberg.punk ?? 0) > 3 * ((zehlendorf.hipster ?? 0) + (zehlendorf.punk ?? 0)), 'Kiez in Kreuzberg, nicht in Zehlendorf');
  const night = share(IDS, { minutes: 3 * 60, day: 5, bezirk: 'Neukölln' });
  assert.ok((night.senior ?? 0) < 0.04 && (night.parent ?? 0) < 0.01, 'nachts keine Senioren und Kinderwagen');
  const day = share(IDS, { minutes: 11 * 60, day: 1, bezirk: 'Pankow' });
  assert.ok(Object.keys(day).length >= 8, `tagsüber viele verschiedene (${Object.keys(day)})`);
  for (const k of ['jogger', 'dogwalker']) assert.equal(day[k] ?? 0, 0, `${k} nur über den Gehstil`);
  // Tätigkeiten: Gitarre spielen eher Kiez/Punk, nie Kinderwagen; kein Kinderwagen auf der Wiese
  const music = share(IDS, { minutes: 16 * 60, day: 5, bezirk: 'Friedrichshain-Kreuzberg', act: 'music' });
  assert.ok((music.hipster ?? 0) + (music.punk ?? 0) > 0.35); assert.equal(music.parent ?? 0, 0);
  assert.equal(share(IDS, { minutes: 14 * 60, day: 5, act: 'lie' }).parent ?? 0, 0);
  assert.equal(pickKind(77, { minutes: 600, bezirk: 'Mitte' }), pickKind(77, { minutes: 600, bezirk: 'Mitte' }), 'deterministisch');
  for (const w of Object.values(kindWeights({ minutes: 600 }))) assert.ok(w >= 0);
});

test('Aussehen: jeder Typ vollständig, deterministisch, Alltagslook übernimmt das Hemd; Spieler fest', () => {
  const fields = ['top', 'topColor', 'pants', 'shoes', 'skin', 'hair', 'hairStyle', 'scale'];
  const tops = new Set(), hats = new Set(), accs = new Set();
  for (const kind of KIND_IDS) for (let id = 1; id < 40; id++) {
    const look = figureLook({ id, kind });
    for (const f of fields) assert.ok(look[f] !== undefined && look[f] !== null, `${kind}.${f}`);
    assert.match(look.topColor, /^#[0-9a-f]{6}$/i, `${kind} Farbe`);
    tops.add(look.top); hats.add(look.hat); accs.add(look.acc);
  }
  for (const t of ['tee', 'jacket', 'suit', 'coat', 'hoodie', 'vest', 'sport']) assert.ok(tops.has(t), `Oberteil ${t}`);
  for (const h of ['cap', 'beanie', 'hat', 'helmet', 'scarf', 'sunhat']) assert.ok(hats.has(h), `Kopfbedeckung ${h}`);
  for (const a of ['briefcase', 'camera', 'cane', 'stroller', 'shopping', 'phone']) assert.ok(accs.has(a), `Zubehör ${a}`);
  const p = { id: 5, kind: 'senior' };
  assert.equal(figureLook(p), figureLook(p), 'einmal berechnet');
  assert.deepEqual(figureLook({ id: 5, kind: 'senior' }), figureLook({ id: 5, kind: 'senior' }));
  assert.equal(figureLook({ id: 9, kind: 'everyday', shirt: '#123456' }).topColor, '#123456');
  const q = { id: 9, kind: 'everyday', shirt: '#123456' }; figureLook(q); q.shirt = '#abcdef';
  assert.equal(figureLook(q).topColor, '#abcdef', 'neues Hemd (z. B. Radfahrer vom Rad geholt) wird übernommen');
  assert.equal(figureLook({ id: 3, kind: 'senior' }).stoop, 1); assert.ok(figureLook({ id: 3, kind: 'teen' }).scale < 1, 'Jugendliche kleiner');
  assert.equal(PLAYER_LOOK.topColor, '#ff7a1a'); assert.ok(Object.isFrozen(PLAYER_LOOK));
  assert.ok(KINDS.senior.speed < 0.8 && KINDS.parent.speed < 0.8 && KINDS.business.speed > 1 && KINDS.jogger.speed > 2, 'Tempo je Typ');
  assert.ok(h01(1, 1) >= 0 && h01(1, 1) < 1);
});

// Figur mit gleichmäßiger Geschwindigkeit bewegen, Anim-Zustand und Posen aufzeichnen
function simulate(speeds, fps = 60, facing = 0) {
  const p = { step: 0 }, a = createAnim(p), out = [];
  let t = 0;
  for (const v of speeds) { t += 1 / fps; p.step += v / fps; stepAnim(a, p, t, facing); out.push({ ...a, pose: gaitPose(a, t) }); }
  return out;
}

test('Gang: Stand ohne Schritt, Gehen voller Ausschlag, Rennen längere Schritte, Takt mit dem Tempo gedeckelt', () => {
  const stand = simulate(Array(120).fill(0)).at(-1);
  assert.equal(stand.amp, 0); assert.ok(Math.abs(stand.pose.footL.x) + Math.abs(stand.pose.footR.x) < 1e-12);
  const walk = simulate(Array(180).fill(80)), run = simulate(Array(180).fill(155));
  const w = walk.at(-1), r = run.at(-1);
  assert.ok(w.amp > 0.97 && w.run < 0.05, 'Gehen'); assert.ok(r.run > 0.97, 'Rennen');
  const maxStride = (seq) => Math.max(...seq.slice(-60).map((s) => Math.abs(s.pose.footL.x)));
  assert.ok(maxStride(run) > maxStride(walk) * 1.4, 'Rennen greift weiter aus');
  const cycles = (seq) => (seq.at(-1).phase - seq.at(-61).phase) / (2 * Math.PI);
  assert.ok(cycles(walk) > 1.6 && cycles(walk) < 2.6, `Gehtakt ${cycles(walk)}/s`);
  assert.ok(cycles(run) <= GAIT.cadenceMax + 1e-9 && cycles(run) > cycles(walk), 'Laufen schneller, aber gedeckelt');
  assert.ok(Math.abs(r.pose.lean) > 1, 'Vorlage beim Rennen'); assert.equal(w.pose.lean, 0);
  // gegengleich: Füße und Arme
  const mid = walk[150].pose;
  assert.ok(Math.abs(mid.footL.x + mid.footR.x) < 1e-9);
  assert.ok(Math.sign(mid.handR.x) === Math.sign(mid.footL.x) || Math.abs(mid.footL.x) < 0.1, 'rechte Hand schwingt mit dem linken Fuß');
});

test('Gang: Anlaufen und Anhalten ohne Sprünge – der Fuß landet neben dem anderen, statt mitten im Schritt zu frieren', () => {
  const seq = simulate([...Array(30).fill(0), ...Array(90).fill(80), ...Array(120).fill(0), ...Array(60).fill(155), ...Array(90).fill(0)]);
  // stetig: die Geschwindigkeit eines Fußes/einer Hand ändert sich von Bild zu Bild nur wenig (ein Sprung wäre ein Ausreißer
  // der zweiten Differenz, schnelles Rennen allein nicht)
  let kink = 0;
  for (let i = 2; i < seq.length; i++) for (const f of [(s) => s.pose.footL.x, (s) => s.pose.handR.x, (s) => s.pose.footL.lift]) {
    kink = Math.max(kink, Math.abs(f(seq[i]) - 2 * f(seq[i - 1]) + f(seq[i - 2])));
  }
  // Obergrenze = Krümmung der Schrittkurve selbst beim schnellsten Takt und weitesten Schritt (Sinus: A·ω²)
  const bound = 7.6 * (2 * Math.PI * GAIT.cadenceMax / 60) ** 2 * 1.1;
  assert.ok(kink < bound, `größter Knick ${kink.toFixed(3)} px/Bild² (Grenze ${bound.toFixed(3)})`);
  assert.ok(Math.abs(seq.at(-1).pose.footL.x) < 0.2 && Math.abs(seq[239].pose.footL.x) < 0.3, 'nach dem Anhalten Füße nebeneinander');
  // Bildrate egal: 30 und 144 Hz ergeben dieselbe Bewegung pro Sekunde
  const a30 = simulate(Array(60).fill(80), 30).at(-1), a144 = simulate(Array(288).fill(80), 144).at(-1);
  assert.ok(Math.abs(a30.phase - a144.phase) < 0.35, `Phase unabhängig von der Bildrate (${a30.phase.toFixed(2)} / ${a144.phase.toFixed(2)})`);
});

test('Gang: Drehung geglättet und begrenzt, Beine folgen der Laufrichtung, rückwärts gehen beim Zielen', () => {
  const p = { step: 0 }, a = createAnim(p);
  stepAnim(a, p, 0, 0);
  stepAnim(a, p, 1 / 60, Math.PI / 2);
  assert.ok(a.face > 0 && a.face <= GAIT.turnRate / 60 + 1e-9, 'höchstens turnRate je Sekunde');
  for (let i = 2; i < 30; i++) stepAnim(a, p, i / 60, Math.PI / 2);
  assert.ok(Math.abs(a.face - Math.PI / 2) < 1e-9, 'kommt an');
  // kürzester Weg über ±π
  const b = createAnim({ step: 0, facing: 3 }); stepAnim(b, { step: 0 }, 0, 3); stepAnim(b, { step: 0 }, 1 / 60, -3);
  assert.ok(b.face > 3 || b.face < -3, 'dreht über π, nicht zurück durch 0');
  assert.deepEqual(legFrame({ face: 0, move: 0 }), { rot: 0, back: false });
  assert.equal(legFrame({ face: 0, move: 0.8 }).rot, 0.8, 'Hüfte dreht mit');
  const back = legFrame({ face: 0, move: Math.PI - 0.2 });
  assert.equal(back.back, true); assert.ok(Math.abs(back.rot + 0.2) < 1e-9, 'rückwärts, Hüfte nur leicht gedreht');
  assert.ok(Math.abs(legFrame({ face: 0, move: 1.55 }).rot) <= 1.4, 'Hüfte höchstens ±80°');
  // Stock, Kinderwagen, Tragen verändern die Haltung
  const w = simulate(Array(120).fill(60)).at(-1);
  const base = gaitPose(w, 0), stroller = gaitPose(w, 0, { stroller: true }), stoop = gaitPose(w, 0, { stoop: 1 }), carry = gaitPose(w, 0, { carry: true });
  assert.equal(stroller.handL.x, stroller.handR.x, 'beide Hände am Griff');
  assert.ok(stoop.lean > base.lean, 'gebeugt');
  assert.ok(Math.abs(carry.handL.x) <= Math.abs(base.handL.x) + 1e-9, 'mit Tasche weniger Armschwung');
});

// Aufzeichnender Canvas-Ersatz: keine ungültigen Zahlen, save/restore ausgeglichen, Transform zurückgesetzt
function recorder() {
  let depth = 0, maxDepth = 0; const calls = [];
  const ctx = new Proxy({}, {
    get(t, k) {
      if (k in t) return t[k];
      if (k === 'save') return () => { depth++; maxDepth = Math.max(maxDepth, depth); };
      if (k === 'restore') return () => { depth--; };
      return (...args) => { if (args.some((v) => typeof v === 'number' && !Number.isFinite(v))) throw new Error(`ungültig in ${String(k)}: ${args}`); calls.push(k); };
    },
    set(t, k, v) { t[k] = v; return true; },
  });
  return { ctx, calls, depth: () => depth };
}

test('Zeichnen: jeder Typ in Stand, Gehen, Rennen, Tätigkeit, Sitzen, liegend, mit Waffe und Tritt – ohne ungültige Werte', () => {
  const sun = { dx: 0.5, dy: 0.7, len: 1.2, strength: 1 };
  let variants = 0;
  for (const kind of [undefined, ...KIND_IDS]) for (let id = 1; id <= 6; id++) {
    const player = kind === undefined;
    for (const [speed, extra] of [[0, {}], [70, {}], [150, {}], [0, { act: 'smoke' }], [0, { act: 'sit' }], [0, { act: 'lie' }], [0, { down: true, dead: true }],
      [70, { weapon: 'pistol', attack: { kind: 'shot', t: 0.05 } }], [0, { weapon: 'bat', attack: { kind: 'swing', t: 0.1 } }], [0, { attack: { kind: 'kick', t: 0.2 } }], [60, { sun: null }]]) {
      const r = recorder(), p = { id: player ? undefined : id * 31, kind, x: 100, y: 50, facing: 1, angle: 1, move: 2.5, step: 0 };
      for (let f = 0; f < 5; f++) { p.step += speed / 60; drawPerson(r.ctx, p, { player, sun, time: f / 60, ...extra }); }
      assert.equal(r.depth(), 0, `${kind}/${JSON.stringify(extra)}: save/restore`);
      assert.ok(r.calls.includes('fill'), 'gezeichnet');
      variants++;
    }
  }
  for (const act of Object.keys(ACT_ARMS)) { const r = recorder(); drawPerson(r.ctx, { id: 4, kind: 'hipster', x: 0, y: 0, step: 0 }, { act, time: 1 }); assert.equal(r.depth(), 0, act); }
  assert.ok(variants > 100);
});

test('Zeichnen: Typen sehen verschieden aus (Kinderwagen, Stock, Warnweste, Kopftuch …), der Spieler ist der Spieler', () => {
  const sig = (kind, id, player = false) => {
    const r = recorder(); const styles = [];
    const ctx = new Proxy(r.ctx, { set(t, k, v) { if (k === 'fillStyle' || k === 'strokeStyle') styles.push(v); t[k] = v; return true; } });
    drawPerson(ctx, { id, kind, x: 0, y: 0, facing: 0, step: 0 }, { player, time: 0 });
    return { n: r.calls.length, styles: new Set(styles) };
  };
  const kinds = KIND_IDS.map((k) => sig(k, 12));
  const keys = new Set(kinds.map((s) => `${s.n}|${[...s.styles].sort().join()}`));
  assert.equal(keys.size, KIND_IDS.length, 'jeder Typ zeichnet anders');
  assert.ok(sig('parent', 12).n > sig('everyday', 12).n + 8, 'Kinderwagen dazu');
  assert.ok(sig('worker', 12).styles.has('#dfe6ea'), 'Reflexstreifen');
  assert.ok(sig(undefined, undefined, true).styles.has('#ff7a1a'), 'Spieler in Orange');
});

test('Welt: Passanten bekommen einen Typ und dessen Tempo, ohne den Welt-Zufall zu verbrauchen', () => {
  const city = realCity();
  const w = createWorld({ city, seed: 7 });
  for (let i = 0; i < 240; i++) updateWorld(w, idle(), 1 / 60);
  assert.ok(w.peds.length > 20);
  const kinds = new Set(w.peds.map((p) => p.kind));
  assert.ok(w.peds.every((p) => KINDS[p.kind]), 'jeder hat einen Typ');
  assert.ok(kinds.size >= 5, `verschiedene Typen: ${[...kinds]}`);
  // Tempo: Typ-Faktor wird angewandt; der Welt-Zufall wird nicht angefasst
  const rng = w.rng; w.rng = () => { throw new Error('Welt-Zufall verbraucht'); };
  const ped = { id: 424242, x: w.player.x, y: w.player.y, speed: 36 };
  assignKind(w, ped);
  w.rng = rng;
  assert.ok(KINDS[ped.kind]); assert.ok(Math.abs(ped.speed - 36 * KINDS[ped.kind].speed) < 1e-9);
  const jog = assignKind(w, { id: 1, x: 0, y: 0, speed: 36, style: 'jog' }), dog = assignKind(w, { id: 2, x: 0, y: 0, speed: 36, style: 'dog' });
  assert.equal(jog.kind, 'jogger'); assert.ok(jog.speed > 80, 'Jogger laufen'); assert.equal(dog.kind, 'dogwalker');
  const senior = assignKind({ ...w, clock: 600, day: 1 }, { id: [...Array(500).keys()].find((i) => pickKind(i, { minutes: 600, day: 1, bezirk: 'Friedrichshain-Kreuzberg' }) === 'senior'), x: w.player.x, y: w.player.y, speed: 36 });
  assert.equal(senior.kind, 'senior'); assert.ok(senior.speed < 36 * 0.7, 'Senioren gehen langsamer');
});

test('Zeichnen: Rumpf und Kopf als gespeicherte Bilder (geteilt, begrenzt), niedrige Detailstufe spart Striche', async () => {
  const { personSpriteCount, setPeopleDetail } = await import('../web/src/people.js');
  const saved = globalThis.OffscreenCanvas;
  let made = 0;
  globalThis.OffscreenCanvas = class {
    constructor(w, h) { this.width = w; this.height = h; made++; }
    getContext() { return new Proxy({}, { get: (t, k) => (k === 'createRadialGradient' || k === 'createLinearGradient' ? () => ({ addColorStop() {} }) : () => {}) }); }
  };
  try {
    const ctx = new Proxy({}, { get: (t, k) => (k in t ? t[k] : () => {}), set: (t, k, v) => { t[k] = v; return true; } });
    const p = { id: 777, kind: 'tourist', x: 0, y: 0, facing: 0, step: 0 };
    drawPerson(ctx, p, { time: 0 });
    const n = made;
    assert.ok(n >= 1 && n <= 2, `Rumpf und Kopf je ein Bild (${n})`);
    for (let f = 1; f < 30; f++) { p.step += 1; drawPerson(ctx, p, { time: f / 60 }); }
    assert.equal(made, n, 'beim Gehen wiederverwendet');
    for (let i = 0; i < 2500; i++) drawPerson(ctx, { id: 5000 + i, kind: KIND_IDS[i % KIND_IDS.length], x: 0, y: 0, facing: 0, step: 0 }, { time: 0 });
    assert.ok(personSpriteCount() <= 900, `Speicher begrenzt (${personSpriteCount()})`);
    assert.ok(made < 2 * 2500, 'gleiche Köpfe/Rümpfe werden geteilt');
  } finally { globalThis.OffscreenCanvas = saved; }
  const strokes = (on) => {
    setPeopleDetail(on);
    let n = 0;
    const ctx = new Proxy({}, { get: (t, k) => (k in t ? t[k] : k === 'stroke' ? () => { n++; } : () => {}), set: (t, k, v) => { t[k] = v; return true; } });
    const p = { id: 31, kind: 'everyday', x: 0, y: 0, facing: 0, step: 0 };
    for (let f = 0; f < 5; f++) { p.step += 1.2; drawPerson(ctx, p, { time: f / 60 }); }
    return n;
  };
  const hi = strokes(true), lo = strokes(false);
  setPeopleDetail(true);
  assert.ok(lo < hi, `niedrig ${lo} < hoch ${hi} Striche`);
});
