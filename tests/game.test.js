import test from 'node:test';
import assert from 'node:assert/strict';
import { createGame, updateGame, createMenu, menuInput } from '../web/src/game.js';
import { memoryStorage } from '../web/src/save.js';
import { idle } from './helpers/bot.js';
import { realCity } from './helpers/city.js';

const city = realCity();

const press = (g, patch) => updateGame(g, { ...idle(), ...patch }, 1 / 60);

test('Menü überspringt deaktivierte Einträge und läuft rund', () => {
  const m = createMenu([{ id: 'a', enabled: false }, { id: 'b' }, { id: 'c' }]);
  assert.equal(m.index, 1);
  menuInput(m, { ...idle(), menuDown: true }); assert.equal(m.index, 2);
  menuInput(m, { ...idle(), menuDown: true }); assert.equal(m.index, 1);
  menuInput(m, { ...idle(), menuUp: true }); assert.equal(m.index, 2);
  assert.equal(menuInput(m, { ...idle(), confirm: true }), 'c');
});

test('Titel → Neues Spiel → Pause → Speichern → Hauptmenü → Fortsetzen (nur Controller-Aktionen)', () => {
  const st = memoryStorage();
  const g = createGame({ storage: st, city });
  assert.equal(g.titleMenu.items[0].enabled, false, 'Fortsetzen ohne Spielstand deaktiviert');
  assert.equal(g.titleMenu.items[g.titleMenu.index].id, 'new');
  press(g, { confirm: true });
  assert.equal(g.screen, 'playing');
  g.world.money = 777;
  press(g, { pause: true });
  assert.equal(g.screen, 'paused');
  press(g, { menuDown: true }); press(g, { confirm: true });
  assert.equal(g.toast.text, 'Spiel gespeichert');
  for (let i = 0; i < 3; i++) press(g, { menuDown: true });
  press(g, { confirm: true });
  assert.equal(g.screen, 'title');
  assert.equal(g.titleMenu.items[0].enabled, true);
  assert.equal(g.titleMenu.index, 0);
  press(g, { confirm: true });
  assert.equal(g.screen, 'playing');
  assert.equal(g.world.money, 777);
});

test('B im Pausemenü setzt fort; Steuerungsseite kehrt zum Aufrufer zurück', () => {
  const g = createGame({ storage: memoryStorage(), city });
  press(g, { confirm: true });
  press(g, { pause: true });
  press(g, { back: true });
  assert.equal(g.screen, 'playing');
  press(g, { pause: true });
  for (let i = 0; i < 3; i++) press(g, { menuDown: true });
  press(g, { confirm: true });
  assert.equal(g.screen, 'controls');
  press(g, { back: true });
  assert.equal(g.screen, 'paused');
});

test('Beenden nur, wenn die Hülle es anbietet', () => {
  assert.ok(!createGame({ storage: memoryStorage(), city }).titleMenu.items.some((i) => i.id === 'quit'));
  const g = createGame({ storage: memoryStorage(), canQuit: true, city });
  for (let i = 0; i < 2; i++) press(g, { menuDown: true });
  press(g, { confirm: true });
  assert.ok(g.quitRequested);
});

test('Gescheiterte Mission: „Erneut versuchen“ setzt zurück zum Späti', () => {
  const g = createGame({ storage: memoryStorage(), city });
  press(g, { confirm: true });
  const m = g.world.mission;
  m.state = 'failed'; m.result = { success: false, reason: 'x' };
  g.world.player.x = 100; g.world.player.y = 100;
  press(g, {});
  press(g, { confirm: true });
  assert.equal(m.state, 'available');
  assert.ok(Math.hypot(g.world.player.x - g.world.city.places.playerSpawn.x, g.world.player.y - g.world.city.places.playerSpawn.y) < 1);
});

test('ohne Karte bleibt der Titel stehen, bis sie geladen ist', async () => {
  const { setCity } = await import('../web/src/game.js');
  const g = createGame({ storage: memoryStorage() });
  press(g, { confirm: true });
  assert.equal(g.screen, 'title');
  setCity(g, city);
  press(g, { confirm: true });
  assert.equal(g.screen, 'playing');
});

test('Teleport: Klick auf den Stadtplan, Bestätigung, Abbruch, Sperren', async () => {
  const { requestTeleport, confirmTeleport } = await import('../web/src/game.js');
  const { playerCar } = await import('../web/src/world.js');
  const { inBuilding, onRoad, insideBorder } = await import('../web/src/map.js');
  const g = createGame({ storage: memoryStorage(), city });
  press(g, { confirm: true });
  const w = g.world;
  const kotti = city.list('poi').find((q) => q.cat === 'ubahn' && q.name === 'Kottbusser Tor');
  assert.equal(requestTeleport(g, kotti.x, kotti.y), false, 'nur bei offenem Stadtplan');
  g.showBigMap = true;
  // außerhalb des Gebiets
  assert.equal(requestTeleport(g, 10, 10), false);
  assert.match(g.toast.text, /außerhalb/);
  // Abbrechen mit B: nichts passiert, Welt stand still
  const before = [w.player.x, w.player.y, w.time];
  assert.ok(requestTeleport(g, kotti.x, kotti.y));
  assert.match(g.teleport.name, /\S/);
  press(g, {});
  assert.equal(w.time, before[2], 'Welt steht während des Dialogs');
  press(g, { back: true });
  assert.equal(g.teleport, null);
  assert.deepEqual([w.player.x, w.player.y], before.slice(0, 2));
  // Bestätigen mit A: zu Fuß auf den Gehweg nahe Kottbusser Tor
  requestTeleport(g, kotti.x, kotti.y);
  press(g, { confirm: true });
  assert.ok(Math.hypot(w.player.x - kotti.x, w.player.y - kotti.y) < 600, 'nahe am Ziel');
  assert.ok(!inBuilding(city, w.player.x, w.player.y) && insideBorder(city, w.player.x, w.player.y));
  assert.equal(g.showBigMap, false);
  assert.ok(w.cars.some((c) => c.driver === 'npc' && Math.hypot(c.x - w.player.x, c.y - w.player.y) < 2000), 'Verkehr am neuen Ort');
  // Im Auto: landet auf einer Fahrspur, in Spurrichtung ausgerichtet; Mausweg (confirmTeleport) wie A
  const car = w.cars.find((c) => c.id === w.playerCarId);
  w.player.inCar = car.id; car.driver = 'player';
  g.showBigMap = true;
  const arc = city.list('poi').find((q) => q.name === 'Neukölln Arcaden');
  assert.ok(requestTeleport(g, arc.x, arc.y));
  confirmTeleport(g, true);
  assert.equal(playerCar(w), car);
  assert.ok(onRoad(city, car.x, car.y), 'Auto steht auf der Fahrbahn');
  assert.ok(Math.hypot(car.x - arc.x, car.y - arc.y) < 1500);
  // während eines Auftrags gesperrt
  g.showBigMap = true;
  w.mission.state = 'toPickup';
  assert.equal(requestTeleport(g, kotti.x, kotti.y), false);
  assert.match(g.toast.text, /Auftrag/);
});

test('Teleport in einen noch nicht geladenen Stadtteil: Ziel lädt erst, dann Dialog (asynchrone Kacheln wie im Browser)', async () => {
  const { requestTeleport } = await import('../web/src/game.js');
  const { openCity, inBuilding, insideBorder, districtAt } = await import('../web/src/map.js');
  const { realIndex, tileLoader } = await import('./helpers/city.js');
  const sync = tileLoader();
  const pending = [];
  // Kacheln kommen erst, wenn der Test sie freigibt (wie ein langsames Netz)
  const slow = openCity(realIndex(), (k) => new Promise((res) => pending.push(() => res(sync(k)))));
  const flush = async () => { while (pending.length) pending.shift()(); await new Promise((r) => setTimeout(r, 0)); };
  const g = createGame({ storage: memoryStorage(), city: slow });
  press(g, { confirm: true });
  assert.ok(g.world.loading, 'Start: Stadtteil lädt noch');
  const t0 = g.world.time;
  press(g, {});
  assert.equal(g.world.time, t0, 'Welt steht, solange nichts geladen ist');
  await flush(); press(g, {}); await flush(); press(g, {});
  assert.ok(!g.world.loading && g.world.time > t0, 'nach dem Laden läuft die Welt');
  assert.ok(g.world.cars.some((c) => c.driver === 'npc'), 'Verkehr aufgebaut');
  // Rathaus Spandau (15 km entfernt): Ziel lädt erst
  const [x, y] = await pxOf(slow, 52.5354, 13.2006);
  g.showBigMap = true;
  assert.ok(requestTeleport(g, x, y));
  assert.ok(g.teleport.pending, 'Ziel noch nicht geladen');
  press(g, { confirm: true });
  assert.ok(g.teleport?.pending, 'bestätigen geht erst, wenn das Ziel da ist');
  for (let i = 0; i < 5 && g.teleport?.pending; i++) { await flush(); press(g, {}); }
  assert.ok(g.teleport && !g.teleport.pending, 'Dialog mit Ortsnamen');
  assert.match(g.teleport.name, /\S/);
  press(g, { confirm: true });
  const w = g.world;
  assert.ok(Math.hypot(w.player.x - x, w.player.y - y) < 600, 'in Spandau angekommen');
  assert.equal(districtAt(slow, w.player.x, w.player.y), 'Spandau');
  assert.ok(insideBorder(slow, w.player.x, w.player.y) && !inBuilding(slow, w.player.x, w.player.y));
  // Das alte Viertel wird irgendwann freigegeben
  for (let i = 0; i < 40; i++) { await flush(); press(g, {}); }
  const giver = slow.places.giver;
  assert.ok(![...slow.tiles.keys()].some((k) => { const [tx, ty] = k.split('_').map(Number); return Math.abs(tx * slow.tile - giver.x) < slow.tile && Math.abs(ty * slow.tile - giver.y) < slow.tile; }), 'Kacheln am Späti entladen');
});

async function pxOf(city, lat, lon) {
  const { makeProjection } = await import('../tools/osm/geo.mjs');
  const { lat0, lon0, bbox: [s, w, n, e] } = city.meta.origin;
  const proj = makeProjection(lat0, lon0);
  const c = [proj(s, w), proj(s, e), proj(n, w), proj(n, e)];
  const minX = Math.min(...c.map((q) => q[0])), maxY = Math.max(...c.map((q) => q[1]));
  const [px, py] = proj(lat, lon);
  return [(px - minX) * city.scale, (maxY - py) * city.scale];
}

test('Maus im Menü: Zeigen wählt aus, Klicken bestätigt; deaktivierte Einträge reagieren nicht', () => {
  const m = createMenu([{ id: 'a', enabled: false }, { id: 'b' }, { id: 'c' }]);
  assert.equal(menuInput(m, { ...idle(), menuHover: 2 }), 'move');
  assert.equal(m.index, 2);
  assert.equal(menuInput(m, { ...idle(), menuHover: 2 }), null, 'gleiche Stelle: kein neuer Ton');
  assert.equal(menuInput(m, { ...idle(), menuHover: 0 }), null, 'deaktiviert: Auswahl bleibt');
  assert.equal(m.index, 2);
  assert.equal(menuInput(m, { ...idle(), menuPick: 0 }), null, 'deaktiviert: Klick bewirkt nichts');
  assert.equal(menuInput(m, { ...idle(), menuPick: 1 }), 'b');
  // Ganzer Weg nur mit der Maus: Titel → „Neues Spiel“ anklicken → Spiel; Pause → „Zum Hauptmenü“ anklicken
  const g = createGame({ storage: memoryStorage(), city });
  const iNew = g.titleMenu.items.findIndex((it) => it.id === 'new');
  press(g, { menuHover: 0 }); // „Fortsetzen“ ist ohne Spielstand deaktiviert
  press(g, { menuPick: iNew });
  assert.equal(g.screen, 'playing');
  press(g, { pause: true });
  press(g, { menuPick: g.pauseMenu.items.findIndex((it) => it.id === 'title') });
  assert.equal(g.screen, 'title');
});

test('Mauszeiger: passender Zeiger je Lage, beim Spielen ausgeblendet, wenn die Maus ruht', async () => {
  const { cursorKind, cursorCss, CURSOR_KINDS } = await import('../web/src/cursor.js');
  assert.equal(cursorKind({}), 'arrow');
  assert.equal(cursorKind({ hit: { kind: 'menu' } }), 'hot');
  assert.equal(cursorKind({ hit: { kind: 'key' } }), 'hot');
  assert.equal(cursorKind({ hit: { kind: 'map' } }), 'target');
  assert.equal(cursorKind({ hit: { kind: 'map' }, dragging: true }), 'move');
  assert.equal(cursorKind({ playing: true, idle: 3 }), 'none');
  assert.equal(cursorKind({ playing: true, idle: 0.5 }), 'arrow');
  assert.equal(cursorKind({ playing: false, idle: 30 }), 'arrow', 'im Menü bleibt er sichtbar');
  for (const k of CURSOR_KINDS.filter((x) => x !== 'none')) {
    const css = cursorCss(k);
    assert.match(css, /^url\("data:image\/svg\+xml,[^"]+"\) \d+ \d+, [a-z]+$/, k);
    const svg = decodeURIComponent(css.slice(css.indexOf(',') + 1, css.indexOf('")')));
    assert.match(svg, /^<svg[^>]*width='32' height='32'[\s\S]*<\/svg>$/, `${k}: gültiges 32-px-SVG`);
    assert.ok(svg.includes('#ffd33d'), `${k}: Spielgelb`);
  }
});
