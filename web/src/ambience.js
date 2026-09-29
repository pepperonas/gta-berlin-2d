// Umgebungsklang (rein rechnerisch): was man an der Kamera gerade hören müsste – Stadtrauschen, Verkehr, Vögel in
// Grün und Bäumen bei Tag, Gemurmel vor Bars am Abend, Wasser am Ufer, das Rumpeln der Hochbahn, Martinshörner in der
// Nähe und die Kirchenglocke zur vollen Stunde. audio.js macht daraus Klang; hier steht nur die Mischung (testbar).
import { gustAt } from './weather.js';
import { AREA_KIND, BUILDING_KIND } from './citycodes.js';
import { sirenHigh } from './fleet.js';
import { positionAt, pointOn } from './transit.js';
import { nightlifeAt } from './nightlife.js';
import { frontOf } from './life.js';

export const AMB = { hear: 1200, siren: 3000, bells: 1800, hochbahnHear: 450, trainEvery: 150, trainLen: 14 };
const clamp01 = (v) => Math.max(0, Math.min(1, v));
const wrap = (m) => ((m % 1440) + 1440) % 1440;

// Vogelgesang über den Tag: Morgenchor 4:30–8:00, tagsüber leiser, Abendgesang, nachts still
export function birdLevel(minutes) {
  const m = wrap(minutes);
  if (m < 270 || m > 1290) return 0;
  if (m < 330) return (m - 270) / 60;
  if (m < 480) return 1;
  if (m < 600) return 1 - (m - 480) / 120 * 0.6;
  if (m < 1110) return 0.4;
  if (m < 1200) return 0.4 + (m - 1110) / 90 * 0.3;
  return 0.7 * (1 - (m - 1200) / 90);
}

// Mischung an der Kamera (alle Werte 0..1, sirens: Liste nach Entfernung)
export function ambienceAt(world) {
  const cam = world.camera, city = world.city, H = AMB.hear;
  const box = { x: cam.x - H, y: cam.y - H, w: 2 * H, h: 2 * H };
  let green = 0, water = 0, hochbahn = Infinity;
  for (const f of city.render.query(box, [])) {
    if (f.layer === 'area' && (f.kind === AREA_KIND.grass || f.kind === AREA_KIND.wood || f.kind === AREA_KIND.cemetery || f.kind === AREA_KIND.allotments)) green += Math.min(1, (f.bbox.w * f.bbox.h) / 4e6);
    else if (f.layer === 'tree') green += 0.012;
    else if (f.layer === 'water' && Math.abs(f.bbox.x + f.bbox.w / 2 - cam.x) < f.bbox.w / 2 + 400 && Math.abs(f.bbox.y + f.bbox.h / 2 - cam.y) < f.bbox.h / 2 + 400) water = 1;
    else if (f.layer === 'rail' && f.bridge) hochbahn = Math.min(hochbahn, nearestOnLine(f.pts, cam.x, cam.y));
  }
  let traffic = 0;
  for (const c of world.cars) {
    if (!c.driver || c.wrecked) continue;
    const d = Math.hypot(c.x - cam.x, c.y - cam.y);
    if (d < H) traffic += (Math.hypot(c.vx, c.vy) / 250) * (1 - d / H) * (c.kind === 'truck' || c.kind === 'garbage' ? 2 : 1);
  }
  let bar = 0;
  for (const p of world.peds) if (p.state === 'hang' && (p.hang.act === 'drink' || p.hang.act === 'smoke' || p.hang.act === 'queue' || p.hang.act === 'chat' || p.hang.act === 'sit')) {
    const d = Math.hypot(p.x - cam.x, p.y - cam.y);
    if (d < 400) bar += 0.12 * (1 - d / 400);
  }
  // Bahnen: mit Fahrplan das Rumpeln echter Züge in der Nähe (auch im Tunnel unter der Straße), sonst ein fester
  // Takt an der Hochbahn
  let rumble = 0;
  if (world.transit && world.city.transit) rumble = trainRumble(world, cam);
  else {
    const phase = world.time % AMB.trainEvery, train = phase < AMB.trainLen || Math.abs(phase - AMB.trainEvery / 2) < AMB.trainLen / 2;
    rumble = hochbahn < AMB.hochbahnHear && train ? 1 - hochbahn / AMB.hochbahnHear : 0;
  }
  const sirens = [];
  for (const c of world.cars) if (c.siren) {
    const d = Math.hypot(c.x - cam.x, c.y - cam.y);
    if (d < AMB.siren) sirens.push({ d, gain: (1 - d / AMB.siren) ** 2, high: sirenHigh(world.time + c.id * 0.37) });
  }
  sirens.sort((a, b) => a.d - b.d);
  // Nachtleben: Stimmengewirr und gedämpfte Musik vor Bars, Kneipen und Clubs (nightlife.js, mit Auslastungs-Feed)
  const nl = nightlifeAt(city, cam.x, cam.y, world.clock ?? 0, world.day ?? 0, { front: (q) => frontOf(city, q), now: Date.now() / 1000, weather: world.weather });
  const inCar = world.player?.inCar != null;
  const night = wrap(world.clock) < 360 || wrap(world.clock) > 1260;
  // Schnee schluckt den Stadtlärm (Schneedecke und fallender Schnee dämpfen), Sturm heult in Böen
  const wx = world.weather, hush = 1 - 0.45 * clamp01(world.snow ?? 0) - 0.2 * clamp01(wx?.snow ?? 0);
  return {
    hum: (night ? 0.35 : 0.6) * hush,
    traffic: clamp01(traffic / 2) * hush,
    wind: clamp01((wx?.storm ?? 0) * gustAt(wx, world.time) * 0.8 + 0.15 * (wx?.snow ?? 0) * (wx?.storm ?? 0)),
    birds: clamp01(Math.min(1, green) * birdLevel(world.clock) * (1 - Math.min(1, (world.weather?.rain ?? 0) + (world.weather?.storm ?? 0) + (world.weather?.snow ?? 0)))), // bei Regen, Sturm, Schnee schweigen die Vögel
    rain: Math.min(1.6, world.weather?.rain ?? 0), // bis 1,6 bei Starkregen
    bar: clamp01(Math.max(bar, nl.crowd)),
    music: nl.music * hush,
    barPan: nl.pan,
    nightFeed: nl.feed,
    // Dämpfung von außen: im Auto (Karosserie) und bei Schneedecke (schluckt die Höhen)
    muffle: clamp01((inCar ? 0.65 : 0) + 0.35 * clamp01(world.snow ?? 0)),
    inCar,
    gust: (wx?.storm ?? 0) > 0 ? clamp01((gustAt(wx, world.time) - 0.2) / 1.6) : 0,
    storm: clamp01(wx?.storm ?? 0),
    snowfall: clamp01(wx?.snow ?? 0),
    water: water * (0.4 + (night ? 0.2 : 0)),
    rumble: clamp01(rumble),
    sirens,
  };
}

function trainRumble(world, cam) {
  let r = 0;
  const tr = world.city.transit;
  for (const [id, s] of world.transit.tracked) {
    const p = tr.patterns[id];
    if (p.mode === 'bus') continue;
    for (const v of s.veh) {
      const q = pointOn(p, positionAt(p, v.tau).s), d = Math.hypot(q.x - cam.x, q.y - cam.y);
      const R = p.mode === 'tram' ? 250 : AMB.hochbahnHear;
      if (d < R) r = Math.max(r, (1 - d / R) * (p.mode === 'tram' ? 0.5 : 1));
    }
  }
  return r;
}

function nearestOnLine(p, x, y) {
  let best = Infinity;
  for (let i = 0; i < p.length - 2; i += 2) {
    const ax = p[i], ay = p[i + 1], dx = p[i + 2] - ax, dy = p[i + 3] - ay, L2 = dx * dx + dy * dy || 1;
    const t = Math.max(0, Math.min(1, ((x - ax) * dx + (y - ay) * dy) / L2));
    best = Math.min(best, Math.hypot(ax + dx * t - x, ay + dy * t - y));
  }
  return best;
}

// Kirchenglocke: Uhr hat seit dem letzten Schritt eine volle Stunde überschritten und eine Kirche ist in Hörweite
// → Anzahl der Schläge (1–12), sonst 0
export function bellStrikes(world, prevClock) {
  const a = wrap(prevClock), b = wrap(world.clock);
  const crossed = b < a ? true : Math.floor(b / 60) !== Math.floor(a / 60);
  if (!crossed) return 0;
  const cam = world.camera, R = AMB.bells;
  for (const f of world.city.render.query({ x: cam.x - R, y: cam.y - R, w: 2 * R, h: 2 * R }, [])) {
    if (f.layer === 'building' && f.kind === BUILDING_KIND.church) return (Math.floor(b / 60) % 12) || 12;
  }
  return 0;
}
