import test from 'node:test';
import assert from 'node:assert/strict';
import { Hud, BASE } from '../web/src/hud.js';
import { createGame } from '../web/src/game.js';
import { memoryStorage } from '../web/src/save.js';
import { idle } from './helpers/bot.js';
import { realCity } from './helpers/city.js';

// Canvas-Attrappe: merkt sich nichts, liefert nur, was der HUD braucht (Textbreite grob geschätzt).
function fakeCtx() {
  const noop = () => {};
  const gradient = { addColorStop: noop };
  return new Proxy({ measureText: (t) => ({ width: String(t).length * 11 }), createLinearGradient: () => gradient, createRadialGradient: () => gradient, createPattern: () => null, font: '' },
    { get: (o, k) => (k in o ? o[k] : noop), set: (o, k, v) => { o[k] = v; return true; } });
}
globalThis.Path2D ??= class { moveTo() {} lineTo() {} closePath() {} rect() {} arc() {} addPath() {} };

const city = realCity();
const SIZES = [[1920, 1080], [1280, 720], [2560, 1080], [3440, 1440], [1512, 823], [1280, 1024], [1024, 768], [900, 900], [800, 1200], [2560, 900], [640, 360]];
const inside = (r, vw, vh) => r.x >= -0.5 && r.y >= -0.5 && r.x + r.w <= vw + 0.5 && r.y + r.h <= vh + 0.5;
const overlap = (a, b) => a.x < b.x + b.w && a.x + a.w > b.x && a.y < b.y + b.h && a.y + a.h > b.y;

test('HUD skaliert das 16:9-Grundformat in jedes Fenster: Minikarte, Auftrag und Tacho ganz sichtbar, ohne Überlappung', () => {
  const g = createGame({ storage: memoryStorage(), city });
  g.screen = 'playing';
  const { createWorld } = globalThis.__createWorld ?? {};
  void createWorld;
  return import('../web/src/world.js').then(({ createWorld: cw }) => {
    const w = cw({ city, cars: 0, pedestrians: 0 });
    g.world = w; g.worldScale = 1.8; g.hintT = 0;
    w.mission.state = 'toPickup'; w.mission.timer = 100;
    const car = w.cars.find((c) => c.id === w.playerCarId); w.player.inCar = car.id; car.driver = 'player';
    for (const [W, H] of SIZES) {
      const hud = new Hud(fakeCtx());
      hud.begin(W, H);
      assert.ok(Math.abs(hud.s - Math.min(W / BASE.w, H / BASE.h)) < 1e-9, `${W}×${H}: Maßstab`);
      assert.ok(hud.vw >= BASE.w - 1e-6 && hud.vh >= BASE.h - 1e-6, `${W}×${H}: 16:9-Fläche passt ganz hinein`);
      assert.ok(Math.abs(hud.vw * hud.s - W) < 1e-6 && Math.abs(hud.vh * hud.s - H) < 1e-6, 'füllt das Fenster');
      hud.drawGameplay(w, g);
      const L = hud.layout;
      for (const k of ['minimap', 'mission', 'car']) assert.ok(inside(L[k], hud.vw, hud.vh), `${W}×${H}: ${k} ragt aus dem Bild (${JSON.stringify(L[k])}, ${hud.vw.toFixed(0)}×${hud.vh.toFixed(0)})`);
      assert.ok(!overlap(L.minimap, L.car) && !overlap(L.minimap, L.mission) && !overlap(L.car, L.mission), `${W}×${H}: HUD-Elemente überlappen`);
      // zu Fuß: Waffenfeld statt Fahrzeugzustand
      w.player.inCar = null; car.driver = null; w.player.weapon = 3;
      const hud2 = new Hud(fakeCtx()); hud2.begin(W, H); hud2.drawGameplay(w, g);
      const L2 = hud2.layout;
      assert.ok(inside(L2.weapon, hud2.vw, hud2.vh), `${W}×${H}: Waffenfeld ragt aus dem Bild`);
      assert.ok(!overlap(L2.weapon, L2.minimap) && !overlap(L2.weapon, L2.mission), `${W}×${H}: Waffenfeld überlappt`);
      w.player.inCar = car.id; car.driver = 'player';
    }
  });
});

test('Menübildschirme liegen im zentrierten 16:9-Rahmen: Einträge und Klickflächen im Fenster, mittig', () => {
  const g = createGame({ storage: memoryStorage(), city });
  for (const [W, H] of SIZES) {
    const hud = new Hud(fakeCtx());
    hud.begin(W, H);
    hud.drawTitle(g);
    const menus = hud.hits.filter((h) => h.kind === 'menu');
    assert.equal(menus.length, g.titleMenu.items.filter((it) => it.enabled !== false).length, `${W}×${H}: Einträge anklickbar`);
    for (const h of hud.hits) assert.ok(inside(h, hud.vw, hud.vh), `${W}×${H}: Klickfläche außerhalb ${JSON.stringify(h)}`);
    const cx = menus[0].x + menus[0].w / 2;
    assert.ok(Math.abs(cx - hud.vw / 2) < 1, `${W}×${H}: Menü waagerecht mittig`);
    // Eintrag i liegt bei 1280×720 mittig auf y = 350 + 58·i; im größeren Fenster um den Rahmenversatz verschoben
    for (const m of menus) assert.ok(Math.abs(m.y + m.h / 2 - ((hud.vh - BASE.h) / 2 + 350 + 58 * m.i)) < 1, `${W}×${H}: Eintrag ${m.i} nicht im Rahmen`);
    // derselbe Weg wie mit der Maus: Mitte eines Eintrags → dieser Eintrag
    const mid = menus[1];
    assert.ok(hud.hits.filter((h) => mid.x + 5 >= h.x && mid.x + 5 <= h.x + h.w && mid.y + 5 >= h.y && mid.y + 5 <= h.y + h.h).length === 1);
  }
  void idle;
});

test('Steuerungsbildschirm: Tabelle passt in den 720px-Rahmen mit Abstand', () => {
  const g = createGame({ storage: memoryStorage(), city });
  const textCalls = [];
  const rectCalls = [];
  const ctx = fakeCtx();
  const origText = ctx.fillText;
  ctx.fillText = function(text, x, y) { textCalls.push({ text: String(text), x, y }); return origText?.call(this, text, x, y); };
  const origRect = ctx.fillRect;
  ctx.fillRect = function(x, y, w, h) { rectCalls.push({ x, y, w, h }); return origRect?.call(this, x, y, w, h); };

  const hud = new Hud(ctx);
  hud.begin(1920, 1080);
  hud.drawControls();

  // Alle Text-Y-Werte sollten ≤ 700 sein (720px Frame - 20px Abstand)
  const textYs = textCalls.map(t => t.y);
  const maxY = Math.max(...textYs);
  assert.ok(maxY <= 700, `Kontrolltabelle überläuft: max y = ${maxY.toFixed(0)} > 700`);

  // "Mitfahren" Zeile sollte vorhanden sein
  assert.ok(textCalls.some(t => t.text.includes('Mitfahren')), 'Mitfahren-Zeile fehlt');

  // "Bahn führen: Türen / Wenden" sollte NICHT als eigene Zeile vorhanden sein
  const bahnRow = textCalls.filter(t => t.text === 'Bahn führen: Türen / Wenden');
  assert.equal(bahnRow.length, 0, 'Bahn-Zeile sollte entfernt sein');

  // "Türen/Wenden" sollte in der Aktion-Zeile enthalten sein
  assert.ok(textCalls.some(t => t.text.includes('Türen/Wenden')), 'Türen/Wenden fehlt in Aktion-Zeile');

  // Die letzte Tabellenzeile darf den Fußzeilen-Hinweis ("B / Zurück") nicht überdecken: mindestens
  // 12px Abstand zwischen der Unterkante des letzten schattierten Zeilenbands und der Klickfläche des
  // Hinweises (beide aus den echten Zeichenaufrufen gelesen, nicht aus geschätzten Konstanten).
  const footerHit = hud.hits.find((h) => h.kind === 'key');
  assert.ok(footerHit, 'Fußzeilen-Hinweis (B / Zurück) fehlt');
  const rowBands = rectCalls.filter((r) => Math.abs(r.w - 920) < 0.5);
  assert.ok(rowBands.length > 0, 'Keine schattierten Zeilenbänder gefunden');
  const lastRowBandBottom = Math.max(...rowBands.map((r) => r.y + r.h));
  const gap = footerHit.y - lastRowBandBottom;
  assert.ok(gap >= 12, `Letzte Tabellenzeile (Bandunterkante ${lastRowBandBottom.toFixed(0)}) reicht zu nah an den Fußzeilen-Hinweis (dessen Klickfläche beginnt bei ${footerHit.y.toFixed(0)}, Abstand ${gap.toFixed(0)}px)`);
});

test('Fahrgast-/Fahrerleiste: im Bild, ohne NaN, überlappt das Auftragsfeld nicht', async () => {
  const { createWorld } = await import('../web/src/world.js');
  const { createDrive } = await import('../web/src/trainphysics.js');
  const tr = city.transit, p = tr.patterns.find((q) => q.mode === 'tram' && q.name === 'M10');
  const g = createGame({ storage: memoryStorage(), city }); g.screen = 'playing'; g.hintT = 99; g.worldScale = 1.8;
  const w = createWorld({ city, cars: 0, pedestrians: 0 }); g.world = w;
  w.mission.state = 'toPickup'; w.mission.timer = 100;
  w.playerTrain = { pid: p.id, s: p.stops[2] + 40, v: 12, drive: createDrive('tram', 12), nextStop: 3, served: [], atStop: null, leftT: null, passengers: 30 };
  for (const kind of ['driver', 'passenger']) {
    w.player.ride = { kind, ref: { playerTrain: true }, mode: 'tram', car: 0, lastStop: { x: 0, y: 0, name: 'x', i: 2, pid: p.id }, since: 0, line: 'M10', dest: p.stopNames.at(-1), speed: 12 };
    for (const [W, H] of SIZES) {
      const texts = [], ctx = fakeCtx();
      ctx.fillText = (t, x, y) => texts.push({ t: String(t), x, y });
      const hud = new Hud(ctx); hud.begin(W, H); hud.drawGameplay(w, g);
      const L = hud.layout;
      assert.ok(hud.counts?.rideBar, `${kind} ${W}×${H}: Leiste gezeichnet`);
      assert.ok(inside(L.rideBar, hud.vw, hud.vh), `${kind} ${W}×${H}: Leiste ragt aus dem Bild ${JSON.stringify(L.rideBar)}`);
      assert.ok(!overlap(L.rideBar, L.mission), `${kind} ${W}×${H}: Leiste überlappt das Auftragsfeld`);
      assert.ok(texts.every((q) => !q.t.includes('NaN') && !q.t.includes('undefined') && Number.isFinite(q.x) && Number.isFinite(q.y)), `${kind}: ${JSON.stringify(texts.filter((q) => /NaN|undefined/.test(q.t) || !Number.isFinite(q.x) || !Number.isFinite(q.y)))}`);
      assert.ok(texts.some((q) => q.t === 'M10'), 'Linie');
      assert.ok(texts.some((q) => /Halt/.test(q.t)), 'nächster Halt');
      assert.equal(L.weapon, undefined, `${kind} ${W}×${H}: kein Waffenfeld während der Fahrt`);
      if (kind === 'driver') assert.ok(texts.some((q) => /^\d+ km\/h$/.test(q.t)) && texts.some((q) => /Gas|Türen|Zug voraus/.test(q.t)), 'Tacho und Bedienhinweis');
    }
  }
});

test('Temperatur neben der Uhr; Warnschild im Bild, überlappt Tacho, Minikarte und Auftrag nicht', async () => {
  const { createWorld } = await import('../web/src/world.js');
  const g = createGame({ storage: memoryStorage(), city }); g.screen = 'playing'; g.hintT = 99; g.worldScale = 1.8;
  const w = createWorld({ city, cars: 0, pedestrians: 0 }); g.world = w;
  w.mission.state = 'toPickup'; w.mission.timer = 100;
  const car = w.cars.find((c) => c.id === w.playerCarId); w.player.inCar = car.id; car.driver = 'player';
  car.x = city.places.playerCar.x; car.y = city.places.playerCar.y;
  w.temp = -3.4; w.wet = 1; w.ice = 1;
  for (const [W, H] of SIZES) {
    const texts = [], ctx = fakeCtx(); ctx.fillText = (t) => texts.push(String(t));
    const hud = new Hud(ctx); hud.begin(W, H); hud.drawGameplay(w, g);
    assert.ok(texts.some((t) => /-3 °C/.test(t)), `${W}×${H}: Temperatur`);
    assert.ok(texts.includes('Glätte'), `${W}×${H}: Warnschild`);
    const L = hud.layout;
    assert.ok(inside(L.roadWarn, hud.vw, hud.vh), `${W}×${H}: Warnschild im Bild`);
    for (const k of ['car', 'minimap', 'mission']) assert.ok(!overlap(L.roadWarn, L[k]), `${W}×${H}: überlappt ${k}`);
  }
});
