// Befehlszeile im Spiel (rein, ohne DOM): Enter öffnet sie (main.js), die Welt steht solange still (game.js).
// Befehle setzen Uhrzeit, Wetter, Dichte, teleportieren, schummeln oder schalten Debug-Ansichten. Autovervollständigung:
// Befehlsnamen, feste Werte je Argument und Orte (Straßen, Bahnhöfe, Ortsteile, Kieze, Bezirke) mit Vorschlagsliste;
// Tab/→ übernimmt, ↑/↓ wählt (ohne Vorschläge: Verlauf), Enter führt aus, Esc leert bzw. schließt.
// Wie eine Befehlspalette: Tippfehler werden verziehen, ohne Befehlswort versteht die Zeile Uhrzeit („22:30“, „nacht“),
// Wetter („regen“) und Orte („alexanderplatz“); ist die Eingabe unvollständig, nimmt Enter den besten Vorschlag; nach
// Erfolg schließt sie (Umschalt+Enter lässt sie offen), bei Fehlern bleibt sie mit Hinweis offen.
import { WEATHER_KINDS, WX_LABEL, temperatureAt } from './weather.js';
import { parseClock, formatClock } from './daylight.js';
import { WEAPONS } from './combat.js';
import { KINDS } from './fleet.js';
import { CAR_MODELS, SPECS, specLine, vehicleName } from './carmodels.js';
import { createCar } from './car.js';
import { findTeleportSpot, openSpot, playerCar, endRide } from './world.js';
import { PLAYER_HP } from './combat.js';

export const CONSOLE = { maxSuggestions: 8, maxLog: 8, logTime: 8, history: 50 };

// Suchform: klein, ohne Akzente, ß → ss
export const norm = (s) => String(s).toLowerCase().normalize('NFD').replace(/[̀-ͯ]/g, '').replace(/ß/g, 'ss');

// Wetter-Namen wie ?wetter=… (deutsch, ohne Leerzeichen) ↔ interne Art
export const WX_NAMES = { sonnig: 'clear', wolkig: 'cloudy', bedeckt: 'overcast', regen: 'rain', starkregen: 'heavyrain', sturm: 'storm', gewitter: 'thunder', nebel: 'fog', dichternebel: 'densefog', schnee: 'snow', schneesturm: 'heavysnow' };
const WX_ALIAS = { klar: 'clear', sonne: 'clear', heiter: 'clear', bewoelkt: 'cloudy', gewitterregen: 'thunder', dunst: 'fog' }; // angenommen, nicht vorgeschlagen
// Uhrzeit großzügig lesen: 21:30, 21.30, 2130, 21, 21h, 21uhr
export function clockArg(v) {
  const t = String(v ?? '').trim().toLowerCase().replace(/\s*(uhr|h)$/, '');
  const r = /^(\d{1,2})(?:[:.](\d{2}))?$/.exec(t) ?? /^(\d{2})(\d{2})$/.exec(t);
  return r ? parseClock(`${r[1]}:${r[2] ?? '00'}`) : null;
}
const TIME_WORDS = { morgen: '07:30', mittag: '12:00', nachmittag: '15:30', abend: '19:30', daemmerung: '20:45', nacht: '23:30', mitternacht: '00:00' };
const ONOFF = ['an', 'aus'];

// --- Orte für „tp“: aus dem Stadtplan (overview.json) und dem Index, einmal je Stadt gebaut --------------------------
export function placeIndex(city) {
  if (city._placeIndex && city._placeIndexOv === city.overview) return city._placeIndex;
  const out = [], seen = new Set();
  const push = (name, kind, x, y, rank) => {
    if (!name) return;
    const k = norm(name) + '|' + kind;
    if (seen.has(k)) return;
    seen.add(k); out.push({ name, kind, x, y, rank, key: norm(name) });
  };
  const ov = city.overview;
  if (ov) {
    for (const [x, y, name] of ov.labels ?? []) push(name, 'Bezirk', x, y, 0);
    for (const [x, y, name] of ov.ortsteile ?? []) push(name, 'Ortsteil', x, y, 1);
    for (const [x, y, kind, name] of ov.stations ?? []) push(name, kind === 'ubahn' ? 'U-Bahnhof' : kind === 'sbahn' ? 'S-Bahnhof' : 'Bahnhof', x, y, 2);
    for (const [x, y, name] of ov.kieze ?? []) push(name, 'Kiez', x, y, 3);
    // Straßen: je Name der Mittelpunkt des längsten Stücks
    const best = new Map();
    for (const [, n, pts] of ov.roads ?? []) {
      if (n < 0 || !ov.names?.[n]) continue;
      let x = pts[0], y = pts[1], L = 0; const xs = [x], ys = [y];
      for (let i = 2; i < pts.length; i += 2) { const nx = x + pts[i], ny = y + pts[i + 1]; L += Math.hypot(nx - x, ny - y); x = nx; y = ny; xs.push(x); ys.push(y); }
      const m = xs.length >> 1, cur = best.get(n);
      if (!cur || L > cur.L) best.set(n, { L, x: xs[m], y: ys[m] });
    }
    for (const [n, b] of best) push(ov.names[n], 'Straße', b.x, b.y, 4);
  }
  for (const p of Object.values(city.places ?? {})) if (p?.name) push(p.name, 'Ort', p.x, p.y, 1);
  city._placeIndex = out; city._placeIndexOv = ov;
  return out;
}

// Tippfehler-Abstand (Levenshtein, früh abgebrochen ab max + 1)
export function editDistance(a, b, max = 2) {
  if (Math.abs(a.length - b.length) > max) return max + 1;
  let prev = Array.from({ length: b.length + 1 }, (_, j) => j);
  for (let i = 1; i <= a.length; i++) {
    const cur = [i]; let best = i;
    for (let j = 1; j <= b.length; j++) { cur[j] = Math.min(prev[j] + 1, cur[j - 1] + 1, prev[j - 1] + (a[i - 1] === b[j - 1] ? 0 : 1)); best = Math.min(best, cur[j]); }
    if (best > max) return max + 1;
    prev = cur;
  }
  return prev[b.length];
}
// Tippfehler verzeihen: das Getippte ähnelt dem Anfang des Namens oder eines Worts darin (ab 4 Zeichen, 1 Fehler; ab 7: 2)
function fuzzyHit(k, q) {
  if (q.length < 4) return false;
  const max = q.length >= 7 ? 2 : 1;
  for (const w of [k, ...k.split(/[ -]/).filter(Boolean)]) {
    for (const L of [q.length - 1, q.length, q.length + 1]) if (L > 0 && L <= w.length && editDistance(q, w.slice(0, L), max) <= max) return true;
  }
  return false;
}

// Treffer bewerten: Anfang des Namens < Anfang eines Worts < irgendwo < mit Tippfehler; dann Art (Bezirk vor Straße),
// dann kürzer
export function rankMatches(items, query, key = (i) => i.key ?? norm(i.name ?? i)) {
  const q = norm(query).trim();
  const out = [];
  items.forEach((it, i) => {
    const k = key(it);
    let s;
    if (!q) s = 0; else if (k.startsWith(q)) s = 0; else if (k.includes(' ' + q) || k.includes('-' + q)) s = 1; else if (k.includes(q)) s = 2; else if (fuzzyHit(k, q)) s = 3; else return;
    // ohne Eingabe: Reihenfolge der Liste (Art zuerst); mit Eingabe: kürzere Namen zuerst
    out.push({ it, s: s * 100 + (it.rank ?? 0) * 10 + (q ? Math.min(9, k.length / 6) : 0), i });
  });
  return out.sort((a, b) => a.s - b.s || a.i - b.i).map((o) => o.it);
}

const num = (v) => { const n = Number(String(v).replace(',', '.')); return Number.isFinite(n) ? n : null; };
const onOff = (v, cur) => (v === undefined ? !cur : v === 'an' ? true : v === 'aus' ? false : null);

// --- Befehle ---------------------------------------------------------------------------------------------------------
// arg: { name, values?: [..] | (ctx) => [{ label, hint }], rest?: true (Rest der Zeile), optional? }
// run(ctx, args) → Meldung (string) oder { ok: false, msg }. cheat: zählt in der Statistik als Konsolenbefehl.
export const COMMANDS = [
  { name: 'hilfe', aliases: ['help', '?'], help: 'Befehle anzeigen', args: [{ name: 'befehl', optional: true, values: () => COMMANDS.map((c) => ({ label: c.name, hint: c.help })) }],
    run(ctx, [c]) {
      if (c) { const cmd = findCommand(c); return cmd ? `${usage(cmd)} – ${cmd.help}` : { ok: false, msg: `Unbekannter Befehl „${c}“` }; }
      return COMMANDS.map((x) => x.name).join(' · ');
    } },
  { name: 'zeit', aliases: ['uhr', 'time'], help: 'Uhrzeit setzen', args: [{ name: 'HH:MM', values: () => [...Object.entries(TIME_WORDS).map(([w, t]) => ({ label: w, hint: t })), ...['06:00', '09:00', '12:00', '18:00', '21:00', '00:00', '03:00'].map((t) => ({ label: t, hint: '' }))] }],
    run(ctx, [v]) {
      const m = clockArg(TIME_WORDS[norm(v ?? '')] ?? v);
      if (m === null) return { ok: false, msg: 'Zeit als HH:MM, z. B. zeit 21:30 – oder morgen, mittag, abend, nacht' };
      ctx.world.clock = m; return `Uhrzeit ${formatClock(m)}`;
    } },
  { name: 'wetter', aliases: ['weather'], help: 'Wetter festlegen (auto = natürliches Wetter)', args: [{ name: 'wetter', values: () => [{ label: 'auto', hint: 'natürlich' }, ...Object.entries(WX_NAMES).map(([n, k]) => ({ label: n, hint: WX_LABEL[k] }))] }],
    run(ctx, [v]) {
      const k = norm(v ?? '');
      if (k === 'auto') { ctx.world.forceWeather = null; return 'Wetter wieder natürlich'; }
      const kind = WX_NAMES[k] ?? WX_ALIAS[k] ?? (WEATHER_KINDS.includes(k) ? k : null);
      if (!kind) return { ok: false, msg: `Wetter: ${Object.keys(WX_NAMES).join(', ')} oder auto` };
      ctx.world.forceWeather = kind;
      if (['rain', 'heavyrain', 'storm', 'thunder'].includes(kind)) ctx.world.wet = Math.max(ctx.world.wet ?? 0, 0.6);
      return `Wetter: ${WX_LABEL[kind]}`;
    } },
  { name: 'schnee', aliases: ['snow'], help: 'Schneedecke 0–1', args: [{ name: '0–1', values: () => ['0', '0.25', '0.5', '0.75', '1'].map((l) => ({ label: l, hint: '' })) }],
    run(ctx, [v]) { const n = num(v); if (n === null || n < 0 || n > 1) return { ok: false, msg: 'schnee 0 bis 1' }; ctx.world.snow = n; return `Schneedecke ${Math.round(n * 100)} %`; } },
  { name: 'nass', aliases: ['wet'], help: 'Nässe der Straßen 0–1', args: [{ name: '0–1', values: () => ['0', '0.5', '1'].map((l) => ({ label: l, hint: '' })) }],
    run(ctx, [v]) { const n = num(v); if (n === null || n < 0 || n > 1) return { ok: false, msg: 'nass 0 bis 1' }; ctx.world.wet = n; return `Nässe ${Math.round(n * 100)} %`; } },
  { name: 'glaette', aliases: ['ice', 'glätte'], help: 'Glätte der Straßen 0–1 (taut über 0 °C, s. temp)', args: [{ name: '0–1', values: () => ['0', '0.5', '1'].map((l) => ({ label: l, hint: '' })) }],
    run(ctx, [v]) { const n = num(v); if (n === null || n < 0 || n > 1) return { ok: false, msg: 'glaette 0 bis 1' }; ctx.world.ice = n; return `Glätte ${Math.round(n * 100)} %`; } },
  { name: 'temp', aliases: ['temperatur'], help: 'Temperatur zeigen; temp -5 erzwingt sie, temp auto gibt sie frei', args: [{ name: '°C', optional: true, values: () => ['auto', '-5', '0', '5', '20'].map((l) => ({ label: l, hint: '' })) }],
    run(ctx, [v]) {
      const w = ctx.world;
      if (v === undefined) return `${(w.forceTemp ?? temperatureAt(w.seed, w.dayCount, w.clock, w.forceWeather)).toFixed(1).replace('.', ',')} °C${w.forceTemp != null ? ' (erzwungen)' : ''}`;
      if (v === 'auto') { w.forceTemp = null; return 'Temperatur wieder natürlich'; }
      const n = num(v); if (n === null || n < -30 || n > 40) return { ok: false, msg: 'temp -30 bis 40 oder auto' };
      w.forceTemp = n; return `Temperatur ${n} °C`;
    } },
  { name: 'tempo', aliases: ['zeitraffer'], help: 'Tempo der Spieluhr (1 = normal, 0 = Uhr steht)', args: [{ name: 'faktor', values: () => ['0', '0.5', '1', '2', '5', '10', '30'].map((l) => ({ label: l, hint: l === '1' ? 'normal' : '' })) }],
    run(ctx, [v]) { const n = num(v); if (n === null || n < 0 || n > 120) return { ok: false, msg: 'tempo 0 bis 120' }; ctx.world.clockRate = n; return n === 1 ? 'Spieluhr normal' : `Spieluhr × ${n}`; } },
  { name: 'verkehr', aliases: ['traffic'], help: 'Verkehrsdichte (1 = normal)', args: [{ name: 'faktor', values: () => ['0', '0.5', '1', '1.5', '2', '3'].map((l) => ({ label: l, hint: '' })) }],
    run(ctx, [v]) { const n = num(v); if (n === null || n < 0 || n > 3) return { ok: false, msg: 'verkehr 0 bis 3' }; ctx.world.trafficScale = n; return `Verkehr × ${n}`; } },
  { name: 'passanten', aliases: ['peds'], help: 'Fußgängerdichte (1 = normal)', args: [{ name: 'faktor', values: () => ['0', '0.5', '1', '1.5', '2', '3'].map((l) => ({ label: l, hint: '' })) }],
    run(ctx, [v]) { const n = num(v); if (n === null || n < 0 || n > 3) return { ok: false, msg: 'passanten 0 bis 3' }; ctx.world.pedScale = n; return `Passanten × ${n}`; } },
  { name: 'tp', aliases: ['teleport', 'gehe'], help: 'Teleport zu Straße, Bahnhof, Ortsteil, Kiez, Bezirk', args: [{ name: 'ort', rest: true, places: true }],
    run(ctx, [q]) {
      if (!q) return { ok: false, msg: 'tp <Ort>, z. B. tp Kottbusser Tor' };
      const hit = rankMatches(placeIndex(ctx.city), q)[0];
      if (!hit) return { ok: false, msg: `Kein Ort „${q}“` };
      const spot = findTeleportSpot(ctx.world, hit.x, hit.y);
      if (!spot) return { ok: false, msg: `${hit.name} liegt außerhalb` };
      if (ctx.world.player.ride) endRide(ctx.world, 'teleport');
      ctx.game.teleport = { ...spot, auto: true, name: spot.name ?? hit.name }; // game.js bestätigt, sobald die Kacheln da sind
      return `Teleport: ${hit.name} (${hit.kind})`;
    } },
  { name: 'geld', aliases: ['money'], cheat: true, help: 'Geld setzen (+n: dazugeben)', args: [{ name: 'betrag', values: () => ['+1000', '+10000', '0', '100000'].map((l) => ({ label: l, hint: '' })) }],
    run(ctx, [v]) {
      const plus = String(v ?? '').startsWith('+'), n = num(plus ? v.slice(1) : v);
      if (n === null || n < 0 || n > 1e9) return { ok: false, msg: 'geld <betrag> oder geld +<betrag>' };
      ctx.world.money = Math.round(plus ? ctx.world.money + n : n); ctx.game.tracker && (ctx.game.tracker.money = ctx.world.money); // Schummelgeld zählt nicht als verdient
      return `Geld: ${ctx.world.money.toLocaleString('de-DE')} €`;
    } },
  { name: 'leben', aliases: ['heal'], cheat: true, help: 'volle Gesundheit', args: [],
    run(ctx) { const p = ctx.world.player; p.hp = PLAYER_HP; p.dead = false; p.stun = 0; return 'Gesundheit voll'; } },
  { name: 'munition', aliases: ['ammo'], cheat: true, help: 'alle Magazine voll', args: [],
    run(ctx) { const p = ctx.world.player; p.mag = WEAPONS.map((w) => w.mag ?? 0); p.reloadT = 0; return 'Magazine voll'; } },
  { name: 'esp', aliases: ['asr', 'fahrhilfen'], help: 'ASR/ESP im Auto an/aus (aus: Heckantrieb driftet)', args: [{ name: 'an|aus', optional: true, values: () => ONOFF.map((l) => ({ label: l, hint: '' })) }],
    run(ctx, [v]) { const on = onOff(v, ctx.world.esp !== false); if (on === null) return { ok: false, msg: 'esp an|aus' }; ctx.world.esp = on; return `ASR/ESP ${on ? 'an' : 'aus'}`; } },
  { name: 'gott', aliases: ['god'], cheat: true, help: 'unverwundbar an/aus', args: [{ name: 'an|aus', optional: true, values: () => ONOFF.map((l) => ({ label: l, hint: '' })) }],
    run(ctx, [v]) { const on = onOff(v, ctx.world.god); if (on === null) return { ok: false, msg: 'gott an|aus' }; ctx.world.god = on; return `Gottmodus ${on ? 'an' : 'aus'}`; } },
  { name: 'auto', aliases: ['car', 'fahrzeug'], cheat: true, help: 'Fahrzeug neben dir abstellen (Modell oder Art)', args: [{ name: 'art', optional: true, values: () => [...CAR_MODELS.map((m) => ({ label: m, hint: SPECS[m].label })), ...Object.keys(KINDS).map((k) => ({ label: k, hint: KIND_LABEL[k] ?? '' }))] }],
    run(ctx, [v]) {
      const model = v && (CAR_MODELS.includes(norm(v)) ? norm(v) : CAR_MODELS.find((m) => norm(SPECS[m].label) === norm(v)));
      if (model) { // Pkw-Modell mit eigener Technik (carmodels.js)
        const p = ctx.world.player, spot = openSpot(ctx.world, p.x + Math.cos(p.angle ?? 0) * 60, p.y + Math.sin(p.angle ?? 0) * 60, true);
        if (!spot) return { ok: false, msg: 'Kein Platz für ein Fahrzeug' };
        const car = createCar({ x: spot.x, y: spot.y, angle: spot.angle ?? 0, role: 'parked', kind: 'car' });
        car.model = model; car.driver = null; car.lvl = p.lvl;
        ctx.world.cars.push(car);
        return `${vehicleName(car).full} (${specLine(car)}) steht bereit`;
      }
      const kind = v ? (KINDS[norm(v)] ? norm(v) : Object.keys(KIND_LABEL).find((k) => norm(KIND_LABEL[k]) === norm(v)) ?? norm(v)) : 'car'; // auch „fahrrad“, „polizei“
      if (!KINDS[kind]) return { ok: false, msg: `Art: ${Object.keys(KINDS).join(', ')}` };
      const p = ctx.world.player, spot = openSpot(ctx.world, p.x + Math.cos(p.angle ?? 0) * 60, p.y + Math.sin(p.angle ?? 0) * 60, true);
      if (!spot) return { ok: false, msg: 'Kein Platz für ein Fahrzeug' };
      const car = createCar({ x: spot.x, y: spot.y, angle: spot.angle ?? 0, role: 'parked', kind });
      car.driver = null; car.lvl = p.lvl;
      ctx.world.cars.push(car);
      return `${KIND_LABEL[kind] ?? kind} steht bereit`;
    } },
  { name: 'reparieren', aliases: ['repair'], cheat: true, help: 'eigenes Auto reparieren', args: [],
    run(ctx) {
      const c = playerCar(ctx.world) ?? ctx.world.cars.find((o) => o.id === ctx.world.playerCarId);
      if (!c) return { ok: false, msg: 'Kein Auto' };
      c.health = 100; c.wrecked = false; c.wreckT = 0; return 'Auto repariert';
    } },
  { name: 'fps', help: 'Bildrate und Zeichenzeit anzeigen', args: [{ name: 'an|aus', optional: true, values: () => ONOFF.map((l) => ({ label: l, hint: '' })) }],
    run(ctx, [v]) { const on = onOff(v, ctx.game.debug.fps); if (on === null) return { ok: false, msg: 'fps an|aus' }; ctx.game.debug.fps = on; return `FPS-Anzeige ${on ? 'an' : 'aus'}`; } },
  { name: 'ebenen', aliases: ['levels'], help: 'Ebenen und Portale anzeigen', args: [{ name: 'an|aus', optional: true, values: () => ONOFF.map((l) => ({ label: l, hint: '' })) }],
    run(ctx, [v]) { const on = onOff(v, ctx.game.debug.levels); if (on === null) return { ok: false, msg: 'ebenen an|aus' }; ctx.game.debug.levels = on; return `Ebenen-Ansicht ${on ? 'an' : 'aus'}`; } },
  { name: 'silhouetten', help: 'Umrisse verdeckter Figuren an/aus', args: [{ name: 'an|aus', optional: true, values: () => ONOFF.map((l) => ({ label: l, hint: '' })) }],
    run(ctx, [v]) { const on = onOff(v, ctx.game.debug.silhouettes); if (on === null) return { ok: false, msg: 'silhouetten an|aus' }; ctx.game.debug.silhouettes = on; return `Silhouetten ${on ? 'an' : 'aus'}`; } },
  { name: 'qualitaet', aliases: ['qualität', 'quality'], help: 'Zeichenqualität', args: [{ name: 'stufe', values: () => [{ label: 'hoch', hint: '' }, { label: 'niedrig', hint: '' }, { label: 'auto', hint: 'nach Zeichenzeit' }] }],
    run(ctx, [v]) {
      const q = { hoch: 'high', niedrig: 'low', auto: null }[norm(v ?? '')];
      if (q === undefined) return { ok: false, msg: 'qualitaet hoch|niedrig|auto' };
      ctx.game.debug.quality = q; return `Qualität ${v}`;
    } },
  { name: 'aufloesung', aliases: ['auflösung', 'resolution'], help: 'Interne Auflösung (auto = nach Bildrate)', args: [{ name: 'prozent', values: () => [{ label: 'auto', hint: 'nach Bildrate' }, { label: '100', hint: '' }, { label: '85', hint: '' }, { label: '70', hint: '' }] }],
    run(ctx, [v]) {
      const n = norm(v ?? '');
      if (n === 'auto') { ctx.game.debug.resScale = null; return 'Auflösung automatisch'; }
      const p = Number(n);
      if (!(p >= 40 && p <= 100)) return { ok: false, msg: 'aufloesung auto|100|85|70' };
      ctx.game.debug.resScale = p / 100; return `Auflösung ${p} %`;
    } },
  { name: 'bars', aliases: ['nachtleben'], help: 'Bar-Auslastung (Feed-URL setzen, neu laden, aus)', args: [{ name: 'URL|neu|aus', optional: true, values: () => [{ label: 'neu', hint: 'Feed neu laden' }, { label: 'aus', hint: 'nur OSM-Lokale' }] }],
    run(ctx, [v]) {
      const B = ctx.city?.bars;
      if (!v) return B ? `${B.list.length} Bars im Feed, ${B.list.filter((b) => b.osm).length} auf der Karte zugeordnet` : 'Kein Bar-Feed – bars <URL>';
      if (!ctx.bars) return { ok: false, msg: 'Nur im Browser' };
      const con = ctx.game.console, note = (text, ok) => con.log.push({ text, ok, t: (globalThis.performance?.now() ?? 0) / 1000 });
      ctx.bars(v === 'neu' ? undefined : v).then((m) => note(m, true), (err) => note(`Bar-Feed: ${err.message}`, false));
      return v === 'aus' ? 'Bar-Feed aus' : 'Lade Bar-Feed …';
    } },
  { name: 'stats', aliases: ['statistik'], help: 'Statistik anzeigen', args: [],
    run(ctx) { ctx.game.returnTo = 'playing'; ctx.game.screen = 'stats'; ctx.game.console.open = false; return 'Statistik'; } },
];
const KIND_LABEL = { car: 'Pkw', truck: 'Lkw', delivery: 'Lieferwagen', garbage: 'Müllauto', police: 'Polizei', ambulance: 'Rettungswagen', bus: 'Bus', bicycle: 'Fahrrad', escooter: 'E-Roller', motorcycle: 'Motorrad', scooter: 'Roller' };

export function findCommand(name) {
  const n = norm(name ?? '');
  return COMMANDS.find((c) => c.name === n || c.aliases?.some((a) => norm(a) === n)) ?? null;
}
export const usage = (c) => [c.name, ...(c.args ?? []).map((a) => (a.optional ? `[${a.name}]` : `<${a.name}>`))].join(' ');

// Zeile in Wörter (Anführungszeichen halten Leerzeichen zusammen): [{ t, start, end }]
export function tokenize(text) {
  const out = [];
  const re = /"([^"]*)"?|(\S+)/g;
  let m;
  while ((m = re.exec(text))) out.push({ t: m[1] ?? m[2], start: m.index, end: m.index + m[0].length });
  return out;
}

// Ohne Befehlswort: was die Zeile meint – Uhrzeit, Wetter oder Ort. Liefert die Befehlszeile dafür oder null.
export function smartLine(line, ctx) {
  const t = line.trim(), k = norm(t);
  if (!t) return null;
  if (clockArg(t) !== null || TIME_WORDS[k]) return `zeit ${t}`;
  if (WX_NAMES[k] || WX_ALIAS[k]) return `wetter ${t}`;
  if (ctx?.city && k.length >= 3) {
    const hit = rankMatches(placeIndex(ctx.city), t)[0];
    if (hit && hit.key.includes(k)) return `tp ${hit.name}`; // ohne Befehlswort nur sichere Treffer (kein Tippfehler-Raten)
  }
  return null;
}
// Vorschläge dazu (für die erste Stelle), höchstens n
function smartItems(query, ctx, n) {
  const q = norm(query).trim(), out = [];
  if (!q) return out;
  const m = clockArg(query) ?? clockArg(TIME_WORDS[q] ?? '');
  if (m !== null) out.push({ label: `Uhrzeit ${formatClock(m)}`, hint: 'zeit', insert: `zeit ${query.trim()}`, full: true });
  for (const [w, kind] of Object.entries(WX_NAMES)) if (w.startsWith(q) || (q.length >= 4 && fuzzyHit(w, q))) out.push({ label: w, hint: `Wetter: ${WX_LABEL[kind]}`, insert: `wetter ${w}`, full: true });
  if (ctx?.city && q.length >= 3) for (const p of rankMatches(placeIndex(ctx.city), query).slice(0, 4)) out.push({ label: p.name, hint: `tp · ${p.kind}`, insert: `tp ${p.name}`, full: true });
  return out.slice(0, n);
}

// Vorschläge für die Stelle am Zeilenende: { items: [{ label, hint, insert, full? }], from, query, ghost, cmd, argi, help }
export function suggest(text, ctx) {
  const toks = tokenize(text), trailing = /\s$/.test(text) || !text;
  const argi = trailing ? toks.length : toks.length - 1; // welches Wort gerade getippt wird (0 = Befehl)
  const cur = trailing ? { t: '', start: text.length } : toks[toks.length - 1];
  let items = [];
  let from = cur.start, query = cur.t, spec = null;
  const cmd = argi > 0 ? findCommand(toks[0]?.t) : null;
  if (argi === 0) {
    items = rankMatches(COMMANDS.flatMap((c) => [{ label: c.name, hint: c.help, key: c.name }, ...(c.aliases ?? []).map((a) => ({ label: a, hint: `→ ${c.name}`, key: norm(a), rank: 1 }))]), query)
      .map((i) => ({ label: i.label, hint: i.hint, insert: i.label }));
    // Alias nur zeigen, wenn nichts Eigenes passt
    const own = items.filter((i) => !i.hint.startsWith('→'));
    if (own.length) items = own;
    // dazu, was die Zeile ohne Befehlswort bedeuten kann (Uhrzeit, Wetter, Ort) – hinter echten Befehlen
    if (query.length >= 2) items = [...items.slice(0, 3), ...smartItems(query, ctx, CONSOLE.maxSuggestions), ...items.slice(3)];
  } else if (cmd) {
    spec = cmd.args?.[argi - 1] ?? cmd.args?.find((a) => a.rest) ?? null;
    if (spec?.rest) { // Rest der Zeile ist das Argument (Ortsnamen mit Leerzeichen)
      const first = toks[1];
      from = first ? first.start : text.length; query = first ? text.slice(first.start) : '';
    }
    if (spec?.places) items = rankMatches(placeIndex(ctx.city), query).map((p) => ({ label: p.name, hint: p.kind, insert: p.name }));
    else if (spec?.values) items = rankMatches(spec.values(ctx).map((v) => ({ ...v, key: norm(v.label) })), query).map((v) => ({ label: v.label, hint: v.hint, insert: v.label }));
  }
  items = items.slice(0, CONSOLE.maxSuggestions);
  const first = items[0];
  const ghost = first && query && !first.full && norm(first.insert).startsWith(norm(query)) ? first.insert.slice(query.length) : '';
  // Hilfezeile: Aufbau und Zweck des Befehls, den man gerade tippt (oder des obersten Vorschlags)
  const hc = cmd ?? (argi === 0 && first && !first.full ? findCommand(first.insert) : null);
  const help = hc ? `${usage(hc)} – ${hc.help}` : argi === 0 && first?.full ? `Enter: ${first.insert}` : '';
  return { items, from, query, ghost, cmd, argi, spec, help };
}

// Befehl ausführen: { ok, msg, cmd }
export function execute(line, ctx) {
  const toks = tokenize(line);
  if (!toks.length) return { ok: true, msg: '' };
  const cmd = findCommand(toks[0].t);
  if (!cmd) {
    // ohne Befehlswort: Uhrzeit, Wetter oder Ort – außer es ist ein vertippter Befehl („wetr“)
    const typed = norm(toks[0].t), cmdTypo = toks.length === 1 && COMMANDS.some((c) => editDistance(typed, c.name, 1) <= 1);
    const smart = cmdTypo ? null : smartLine(line, ctx);
    if (smart && findCommand(tokenize(smart)[0].t)) return execute(smart, ctx);
    const typo = rankMatches(COMMANDS.map((c) => ({ key: c.name, name: c.name })), toks[0].t).find((c) => editDistance(norm(toks[0].t), c.key, 2) <= 2);
    const near = typo ?? rankMatches(COMMANDS.map((c) => ({ key: c.name, name: c.name })), toks[0].t.slice(0, 2))[0];
    return { ok: false, msg: `Unbekannter Befehl „${toks[0].t}“${near ? ` – meintest du „${near.name}“?` : ''} (hilfe)` };
  }
  const restArg = cmd.args?.findIndex((a) => a.rest) ?? -1;
  const restText = restArg >= 0 ? line.slice(toks[restArg + 1]?.start ?? line.length).trim().replace(/^"|"$/g, '') : '';
  const args = restArg >= 0 ? [...toks.slice(1, restArg + 1).map((t) => t.t), ...(restText ? [restText] : [])] : toks.slice(1).map((t) => t.t);
  const missing = (cmd.args ?? []).findIndex((a, i) => !a.optional && !args[i]);
  if (missing >= 0) {
    // „schnee“ allein meint eher das Wetter als die Schneedecke ohne Wert
    const smart = toks.length === 1 ? smartLine(line, ctx) : null;
    if (smart && !smart.startsWith(cmd.name + ' ') && !smart.startsWith('tp ')) return execute(smart, ctx);
    return { ok: false, msg: `Fehlt: ${usage(cmd)}`, cmd };
  }
  if (!ctx.world && cmd.name !== 'hilfe') return { ok: false, msg: 'Nur im Spiel', cmd };
  const r = cmd.run(ctx, args);
  const res = typeof r === 'string' ? { ok: true, msg: r } : r;
  if (res.ok && cmd.cheat) ctx.game.statQueue?.push({ type: 'cheat', cmd: cmd.name });
  return { ...res, cmd };
}

export function createConsole() { return { open: false, text: '', sel: -1, hist: [], hi: -1, log: [], sugg: null }; }

// Leere Zeile: zuletzt benutzte Befehle zuerst (je einmal, neueste oben), dann die Befehle
function withRecent(con, sugg) {
  if (con.text || !con.hist.length) return sugg;
  const recent = [...new Set([...con.hist].reverse())].slice(0, 3).map((l) => ({ label: l, hint: 'zuletzt', insert: l, full: true }));
  return { ...sugg, items: [...recent, ...sugg.items].slice(0, CONSOLE.maxSuggestions), ghost: '' };
}

// Vorschlag i übernehmen (Tab, →, Mausklick); true, wenn es einen gab
export function consoleAccept(con, i, ctx) {
  const s = con.sugg ?? suggest(con.text, ctx), it = s.items[i];
  if (!it) return false;
  if (it.full) con.text = it.insert;
  else {
    const rest = !!s.spec?.rest;
    const more = s.cmd ? !rest && (s.cmd.args?.length ?? 0) > s.argi : (findCommand(it.insert)?.args?.length ?? 0) > 0;
    con.text = con.text.slice(0, s.from) + (it.insert.includes(' ') && !rest ? `"${it.insert}"` : it.insert) + (more ? ' ' : '');
  }
  con.sel = -1; con.sugg = withRecent(con, suggest(con.text, ctx));
  return true;
}

// Taste verarbeiten (key wie KeyboardEvent.key). Liefert, was passiert ist: 'close' | 'run' | 'edit' | 'nav' | null
// mods: { shift } – Umschalt+Enter lässt die Zeile nach Erfolg offen; Taste 'DeleteWord' (Strg/Alt+Rücktaste) löscht ein Wort
export function consoleKey(con, key, ctx, now = 0, mods = {}) {
  const refresh = () => { con.sugg = withRecent(con, suggest(con.text, ctx)); if (con.sel >= con.sugg.items.length) con.sel = -1; };
  const accept = (i) => consoleAccept(con, i, ctx);
  switch (key) {
    case 'Escape':
      if (con.text) { con.text = ''; con.sel = -1; con.hi = -1; refresh(); return 'edit'; } // erst leeren, dann schließen
      con.open = false; con.sel = -1; return 'close';
    case 'Tab': case 'ArrowRight': if (!accept(con.sel >= 0 ? con.sel : 0)) return null; return 'edit';
    case 'ArrowDown': case 'ArrowUp': {
      const n = con.sugg?.items.length ?? 0, d = key === 'ArrowDown' ? 1 : -1;
      // im Verlauf blättern hat Vorrang, solange man darin ist; sonst ↓ in die Vorschläge, ↑ in den Verlauf
      if (n && con.hi < 0 && (con.sel >= 0 || key === 'ArrowDown' || !con.hist.length)) { con.sel = con.sel < 0 ? (d > 0 ? 0 : n - 1) : (con.sel + d + n) % n; return 'nav'; }
      if (!con.hist.length) return null;
      con.hi = Math.max(-1, Math.min(con.hist.length - 1, con.hi + (d < 0 ? 1 : -1)));
      con.text = con.hi < 0 ? '' : con.hist[con.hist.length - 1 - con.hi]; con.sel = -1; refresh(); return 'nav';
    }
    case 'Enter': {
      if (con.sel >= 0) accept(con.sel);
      let line = con.text.trim();
      if (!line) { con.open = false; return 'close'; }
      let r = execute(line, ctx);
      // unvollständig oder vertippt: mit dem besten Vorschlag noch einmal (nur wenn der dann klappt)
      if (!r.ok) {
        const s = suggest(con.text, ctx), it = s.items[0];
        if (it) {
          const alt = (it.full ? it.insert : con.text.slice(0, s.from) + (it.insert.includes(' ') && !s.spec?.rest ? `"${it.insert}"` : it.insert)).trim();
          if (alt !== line) { const r2 = execute(alt, ctx); if (r2.ok) { r = r2; line = alt; } }
        }
      }
      con.log.push({ text: `> ${line}`, ok: true, t: now }, ...(r.msg ? [{ text: r.msg, ok: r.ok, t: now }] : []));
      con.log.splice(0, Math.max(0, con.log.length - CONSOLE.maxLog));
      if (con.hist[con.hist.length - 1] !== line) con.hist.push(line);
      con.hist.splice(0, Math.max(0, con.hist.length - CONSOLE.history));
      con.hi = -1; con.sel = -1;
      if (r.ok) { con.text = ''; if (ctx.game?.screen === 'stats' || !mods.shift) con.open = false; } // erledigt: zu (Umschalt: offen)
      refresh();
      return 'run';
    }
    case 'Backspace': con.text = con.text.slice(0, -1); con.sel = -1; con.hi = -1; refresh(); return 'edit';
    case 'DeleteWord': con.text = con.text.replace(/\S*\s*$/, ''); con.sel = -1; con.hi = -1; refresh(); return 'edit';
    default:
      if (key.length === 1) { con.text += key; con.sel = -1; con.hi = -1; refresh(); return 'edit'; }
      return null;
  }
}

export function openConsole(con, ctx) { con.open = true; con.text = ''; con.sel = -1; con.hi = -1; con.sugg = withRecent(con, suggest('', ctx)); }
