# Nahverkehr: mitfahren und selbst fahren – Umsetzungsplan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Der Spieler fährt in Bus, Straßenbahn, S- und U-Bahn als Fahrgast mit (eigene Taste G / Steuerkreuz unten) und
führt Straßenbahn, S- und U-Bahn selbst (Gas/Bremse, Türen, Haltestellen, Wenden); unter Tage gibt es eine Tunnelansicht.

**Architecture:** Drei reine Module tragen die Logik: `tunnel.js` (wo eine Linie unter Tage liegt), `ride.js`
(Fahrzeuge in Reichweite, Lage eines Fahrzeugs, Ein-/Aussteigen), `trainphysics.js` (Zugfahrt). `playertrain.js`
bindet den vom Spieler geführten Zug in die Welt ein; `world.js` ruft beides auf, `render.js`/`hud.js` zeichnen
Tunnel, Züge unter Tage und die Leisten. Alles deterministisch, ohne `w.rng`, in Node testbar.

**Tech Stack:** Plain ES modules, Canvas 2D, `node:test`, keine Abhängigkeiten.

**Spec:** `docs/superpowers/specs/2026-09-28-nahverkehr-mitfahren-fahren-design.md`

## Global Constraints

- Null Abhängigkeiten; Simulation DOM-frei und deterministisch (`world.rng` nie für Darstellung oder neue Zufallsentscheidungen).
- Einheiten: 10 px = 1 m; Tempo in px/s; km/h = px/s × 0,36.
- Tasten: Fahrgast G / Steuerkreuz unten (Eingabefeld `ride`), Fahrer F/rechte Maus tippen / Y (`enterExit`), Türen/Wenden E / A (`action`), Gas/Bremse W,S / RT,LT, Notbremse Leertaste / B (`handbrake`).
- Höchsttempo: Straßenbahn 60, U-Bahn 70, S-Bahn 100 km/h. Aufspringen ab 25 km/h Relativtempo; Abspringen ab 10 km/h (1,2 s betäubt, ab 40 km/h 10 HP Schaden).
- Reichweite Fahrgast 25 px zum Wagen-Rechteck; Führerstand 30 px um die Spitze; Haltestelle ±250 px; Trinkgeld bis 10 € (voll bei ±30 px und größter Verzögerung ≤ 1,3 m/s² in den letzten 8 s).
- Unter Tage = S/U-Bahn und kein sichtbares Gleis (`layer === 'rail'`) innerhalb 50 px; nicht geladene Kachel = nicht unter Tage.
- Versionierung: jede Etappe mit Verhaltensänderung bumpt SemVer in `package.json`, `web/src/version.js`, `xbox/GtaBerlin/Package.appxmanifest` (`X.Y.Z.0`), datierter CHANGELOG-Eintrag, **annotierter** Tag `git tag -a vX.Y.Z -m "X.Y.Z"` (leichte Tags überträgt `--follow-tags` nicht), `git push --follow-tags`. README „Stand und Prüfumfang“ + Testzahl, CLAUDE.md Architektur nachführen.
- Testserver für Browserprüfungen: `PORT=8091 npm start` (Port 8080 gehört einem anderen Projekt); nie `pkill` auf serve.mjs.
- Jede neue Schutzprüfung bekommt eine Mutationsprobe (Fehler absichtlich einbauen, Test muss scheitern, zurücksetzen und per `git diff` prüfen).

## Review Focus

1. **Fahrzeug verschwindet während der Fahrt** (Muster verlässt die verfolgte Zone nach Teleport, Fahrplan-Fahrzeug erreicht das Linienende, Bus wird abgebaut) → Notausstieg an `ride.lastStop` zu Fuß, nie `NaN`-Position oder Absturz. Test in Task 4.
2. **Spieler wird während der Fahrt getroffen/umgehauen oder der Auftrag endet** (Kampf, Mission-Ergebnis, Konsole `tp`) → Fahrt endet sauber (tp: am Ziel zu Fuß; K. o.: Fahrt vorher beendet). Test in Task 4.
3. **Speichern während einer Fahrt oder im Führerstand** → Laden an der letzten Haltestelle zu Fuß, nicht im Tunnel/Gleisbett. Test in Task 4.
4. **Kacheln unter dem Zug noch nicht geladen** (schnelle S-Bahn, Browser lädt asynchron) → Tunnelerkennung meldet „nicht unter Tage“, Welt friert wie sonst beim Nachladen ein, kein Flackern der Tunnelansicht. Test in Task 2.
5. **Spielerzug und Fahrplan-Zug auf derselben Strecke / Wenden ohne Gegenmuster** → nie ineinander; Wenden ohne passendes Muster lässt nur Aussteigen zu. Tests in Task 6.

---

## Dateistruktur

| Datei | Neu/Ändern | Verantwortung |
|---|---|---|
| `web/src/input.js`, `web/src/idle.js` | ändern | Eingabefeld `ride` (G / Steuerkreuz unten) |
| `web/src/tunnel.js` | neu, rein | `railAt`, `undergroundAt`, `undergroundAtS` (mit Cache je Muster) |
| `web/src/ride.js` | neu, rein | `vehicleState`, `transitNear`, `boardTarget`, `alightSpot`, `stationExit` |
| `web/src/trainphysics.js` | neu, rein | `TRAIN_DRIVE`, `createDrive`, `stepDrive`, `stopInfo`, `tipFor` |
| `web/src/playertrain.js` | neu | `takeTrain`, `updatePlayerTrain`, `leaveTrain`, `turnAround`, Blockieren anderer Züge |
| `web/src/world.js` | ändern | Fahrt in `updateWorld`, Kamera, Fußlogik aussetzen, Speichern |
| `web/src/save.js` | ändern | Position während Fahrt = `ride.lastStop` |
| `web/src/transitlive.js` | ändern | Spielerzug in `railObs`, Fahrplan-Züge hinter dem Spielerzug warten, `transitVisible` für unter Tage |
| `web/src/render.js` | ändern | Spielerzug zeichnen, Spieler während Fahrt unsichtbar, Tunnel-Durchgang |
| `web/src/tunnelview.js` | neu | Tunnel zeichnen (Röhre, Lichter, Bahnsteige, Züge unter Tage) |
| `web/src/hud.js` | ändern | Fahrgast-Leiste, Fahrer-Anzeige (Tacho, Türen, nächster Halt), Steuerungstabelle |
| `web/src/stats.js` | ändern | neue Zähler |
| `web/src/main.js`, `web/src/audio.js` | ändern | Tasten, `playingOnFoot` ohne Fahrt, Töne |
| Tests | neu: `tests/tunnel.test.js`, `tests/ride.test.js`, `tests/trainphysics.test.js`, `tests/playertrain.test.js`; ändern: `tests/input.test.js`, `tests/stats.test.js` | |

---

### Task 1: Eingabefeld `ride`

**Files:**
- Modify: `web/src/input.js:41-99` (readKeys, readPad, EMPTY, frame)
- Modify: `web/src/idle.js`
- Modify: `web/src/hud.js` (Steuerungstabelle `controlsContent`)
- Test: `tests/input.test.js`

**Interfaces:**
- Produces: `input.ride` (boolean, Flanke) im abstrakten Eingabeobjekt; `idleInput.ride === false`.

- [ ] **Step 1: Failing test** – in `tests/input.test.js` anhängen:

```js
test('Mitfahren: G bzw. Steuerkreuz unten als eigene Flanke; S/Pfeil runter lösen es nicht aus', async () => {
  const { InputState, readKeys, readPad } = await import('../web/src/input.js');
  const { idleInput } = await import('../web/src/idle.js');
  assert.equal(idleInput.ride, false);
  const st = new InputState();
  assert.equal(st.frame(readKeys(new Set(['KeyG'])), 1 / 60).ride, true);
  assert.equal(st.frame(readKeys(new Set(['KeyG'])), 1 / 60).ride, false, 'nur die Flanke');
  const st2 = new InputState();
  assert.equal(st2.frame(readKeys(new Set(['KeyS', 'ArrowDown'])), 1 / 60).ride, false);
  const pad = { buttons: Array.from({ length: 16 }, (_, i) => ({ pressed: i === 13, value: i === 13 ? 1 : 0 })), axes: [0, 0, 0, 0] };
  assert.equal(new InputState().frame(readPad(pad), 1 / 60).ride, true, 'Steuerkreuz unten');
});
```

- [ ] **Step 2: Run** `node --test tests/input.test.js` → FAIL (`ride` undefined).

- [ ] **Step 3: Implement**
  - `readPad`: im Rückgabeobjekt `rideBtn: b(BTN.DOWN),` ergänzen.
  - `readKeys`: `rideBtn: any('KeyG'),` ergänzen.
  - `EMPTY`: `rideBtn: false,` ergänzen.
  - `frame`: im Objekt `out` nach `enterExit: edge('y'),` → `ride: edge('rideBtn'),`.
  - `idle.js`: `enterExit: false,` → `enterExit: false, ride: false,`.
  - `hud.js controlsContent` rows: nach `['Einsteigen / Aussteigen', 'Y', 'F / rechte Maus tippen'],` einfügen
    `['Mitfahren (Bus, Tram, S/U-Bahn)', 'Steuerkreuz unten', 'G'],` und `['Bahn führen: Türen / Wenden', 'A', 'E'],`.

- [ ] **Step 4: Run** `node --test tests/input.test.js tests/hud-layout.test.js` → PASS (hud-layout prüft, dass die Tabelle passt; falls sie überläuft, Zeilenhöhe in `controlsContent` von der vorhandenen Schrittweite um 2 px senken).

- [ ] **Step 5: Commit** `git add web/src/input.js web/src/idle.js web/src/hud.js tests/input.test.js && git commit -m "Input: dedicated ride key (G / D-pad down)"`

---

### Task 2: Tunnelerkennung `tunnel.js`

**Files:**
- Create: `web/src/tunnel.js`
- Modify: `web/src/render.js:1320` (railNear durch `railAt` ersetzen)
- Test: `tests/tunnel.test.js`

**Interfaces:**
- Consumes: `city.render.query(box, out)`, Features mit `layer === 'rail'`, `pts`, `lvl`; `city.ready(x, y, r)`; `pointOn(p, s)` aus transit.js; `segDist2` aus geom.js.
- Produces:
  - `railAt(city, x, y) → feature | null` (sichtbares Gleis ≤ 50 px)
  - `undergroundAt(city, mode, x, y) → boolean` (tram/bus nie; Kachel nicht bereit → false)
  - `undergroundAtS(city, p, s) → boolean` (Lage auf Muster p; Cache `p._ug: Map<Math.round(s/60), boolean>`, gelöscht wenn `city.gen` sich ändert)
  - `TUNNEL = { probe: 50, step: 60 }`

- [ ] **Step 1: Failing test** `tests/tunnel.test.js`:

```js
import test from 'node:test';
import assert from 'node:assert/strict';
import { railAt, undergroundAt, undergroundAtS, TUNNEL } from '../web/src/tunnel.js';
import { pointOn } from '../web/src/transit.js';
import { realCity, realTransit } from './helpers/city.js';

const city = realCity(), tr = realTransit();
const longest = (name) => tr.patterns.filter((p) => p.name === name).sort((a, b) => b.stops.length - a.stops.length)[0];
const stopAt = (p, name) => p.stops[p.stopNames.indexOf(name)];

test('Tunnel: U8 am Kottbusser Tor unter Tage, U1 an der Skalitzer Straße oberirdisch, Tram nie', () => {
  const u8 = longest('U8'), u1 = longest('U1'), m10 = longest('M10');
  const k8 = pointOn(u8, stopAt(u8, 'U Kottbusser Tor'));
  assert.equal(undergroundAt(city, 'ubahn', k8.x, k8.y), true, 'U8 Kotti');
  assert.equal(undergroundAtS(city, u8, stopAt(u8, 'U Kottbusser Tor')), true);
  const k1 = pointOn(u1, stopAt(u1, 'U Kottbusser Tor'));
  assert.equal(undergroundAt(city, 'ubahn', k1.x, k1.y), false, 'U1 Hochbahn');
  assert.ok(railAt(city, k1.x, k1.y), 'Gleis der Hochbahn');
  const m = pointOn(m10, m10.stops[3]);
  assert.equal(undergroundAt(city, 'tram', m.x, m.y), false);
  assert.equal(undergroundAt(city, 'bus', k8.x, k8.y), false);
});

test('Tunnel: nicht geladene Kachel gilt als oberirdisch; Cache folgt dem Kachelstand', () => {
  const fake = { gen: 1, ready: () => false, render: { query: () => [] } };
  assert.equal(undergroundAt(fake, 'ubahn', 100, 100), false, 'ohne Kachel keine Tunnelansicht');
  const p = { id: 1, mode: 'ubahn', shape: { pts: [0, 0, 1000, 0], cum: [0, 1000], len: 1000 } };
  const ready = { gen: 1, ready: () => true, render: { query: () => [] } };
  assert.equal(undergroundAtS(ready, p, 500), true);
  ready.render.query = () => [{ layer: 'rail', pts: [0, 0, 1000, 0] }]; ready.gen = 2;
  assert.equal(undergroundAtS(ready, p, 500), false, 'neuer Kachelstand verwirft den Cache');
  assert.equal(TUNNEL.probe, 50);
});
```

Hinweis: Haltestellennamen erst mit `node -e` prüfen (`realTransit`-Äquivalent: `JSON.parse` von `web/data/berlin/transit.json`, `names`), falls sie z. B. „U Kottbusser Tor (Berlin)“ heißen; dann `stopAt` mit `findIndex((n) => n.startsWith('U Kottbusser Tor'))`.

- [ ] **Step 2: Run** `node --test tests/tunnel.test.js` → FAIL (Modul fehlt).

- [ ] **Step 3: Implement** `web/src/tunnel.js`:

```js
// Unter Tage (rein): U- und S-Bahn fahren dort im Tunnel, wo die Karte kein sichtbares Gleis hat (der Build verwirft
// Tunnelgleise). Dieselbe Regel entscheidet, ob Züge oben gezeichnet werden (render.js) und wann die Tunnelansicht
// kommt. Eine noch nicht geladene Kachel gilt als oberirdisch – sonst blitzte beim Nachladen die Tunnelansicht auf.
import { segDist2 } from './geom.js';
import { pointOn } from './transit.js';

export const TUNNEL = { probe: 50, step: 60 };
const q = [];

export function railAt(city, x, y) {
  const r = TUNNEL.probe, r2 = r * r;
  for (const f of city.render.query({ x: x - r, y: y - r, w: 2 * r, h: 2 * r }, q)) {
    if (f.layer !== 'rail') continue;
    const p = f.pts;
    for (let i = 0; i < p.length - 2; i += 2) if (segDist2(x, y, p[i], p[i + 1], p[i + 2], p[i + 3]) < r2) return f;
  }
  return null;
}

export function undergroundAt(city, mode, x, y) {
  if (mode !== 'ubahn' && mode !== 'sbahn') return false;
  if (!city.ready(x, y, 1)) return false;
  return !railAt(city, x, y);
}

// Lage auf einem Muster; Ergebnis je 60-px-Stück zwischengespeichert, bis sich der Kachelstand ändert
export function undergroundAtS(city, p, s) {
  if (p.mode !== 'ubahn' && p.mode !== 'sbahn') return false;
  const c = p._ug && p._ug.gen === city.gen ? p._ug : (p._ug = { gen: city.gen, m: new Map() });
  const k = Math.round(s / TUNNEL.step);
  let v = c.m.get(k);
  if (v === undefined) {
    const pt = pointOn(p, k * TUNNEL.step);
    v = undergroundAt(city, p.mode, pt.x, pt.y);
    if (city.ready(pt.x, pt.y, 1)) c.m.set(k, v); // Unfertiges nicht merken
  }
  return v;
}
```

`render.js collectMovers`: die Zeile `const railNear = …` durch `const railNear = (x, y) => railAt(city, x, y);` ersetzen und oben `import { railAt } from './tunnel.js';` ergänzen.

Prüfen, dass `city.gen` bei jedem Kachel-Ein/Ausbau steigt: `grep -n "gen++\|gen +=" web/src/map.js`. Falls nicht, in `install`/`uninstall` `city.gen++` ergänzen (vorhanden ist `gen: 0` im city-Objekt und `lane.next` nutzt `city.gen`).

- [ ] **Step 4: Run** `node --test tests/tunnel.test.js tests/transit.test.js tests/render.test.js` → PASS.

- [ ] **Step 5: Mutationsprobe**: in `undergroundAt` die Zeile `if (!city.ready(x, y, 1)) return false;` entfernen → Test 2 muss scheitern; zurücksetzen. `if (city.ready…) c.m.set` → immer setzen: Test 2 (gen) bleibt grün? Dann zusätzlich prüfen: Cache-Invalidierung durch `gen` entfernen (`p._ug.gen === city.gen` → `p._ug`) → Test 2 muss scheitern.

- [ ] **Step 6: Commit** `git add web/src/tunnel.js web/src/render.js tests/tunnel.test.js && git commit -m "Tunnel detection per pattern (tunnel.js)"`

---

### Task 3: Reine Fahrt-Abfragen `ride.js`

**Files:**
- Create: `web/src/ride.js`
- Test: `tests/ride.test.js`

**Interfaces:**
- Consumes: `positionAt`, `pointOn`, `trainCars`, `TRAIN`, `BUS` (transit.js); `circleVsObb` (collision.js); `undergroundAtS` (Task 2); `w.transit.tracked`, `w.city.transit.patterns`, `w.cars` (Bus: `c.duty.bus`, `c.duty.pid`), `w.playerTrain` (Task 6: `{ pid, s, v, cars? }`).
- Produces:
  - `vehicleState(w, ref) → { mode, p, s, speed, cars:[{x,y,angle,L,W,first}], dwelling, stop, underground } | null`
    - `ref = { pid, key }` (Fahrplan), `{ carId }` (Bus-Auto), `{ playerTrain: true }`
  - `transitNear(w, x, y, r) → [{ ref, mode, dist, car (index), front (px bis Spitze) }]` sortiert nach `dist`
  - `speedOfPattern(p, tau) → px/s` (0 beim Halten)
  - `alightSpot(w, st, carIndex) → { x, y } | null` (Freiplatz neben dem Wagen, rechts zuerst)
  - `stationExit(w, p, stopIndex) → { x, y }` (Straßenausgang)
  - `RIDE = { reach: 25, cab: 30, hopOn: 25 / 0.36, hopOff: 10 / 0.36, hurtFrom: 40 / 0.36, stun: 1.2, hurt: 10 }`

- [ ] **Step 1: Failing tests** `tests/ride.test.js`:

```js
import test from 'node:test';
import assert from 'node:assert/strict';
import { vehicleState, transitNear, speedOfPattern, alightSpot, stationExit, RIDE } from '../web/src/ride.js';
import { positionAt, pointOn } from '../web/src/transit.js';
import { createWorld, updateWorld, resetPopulation } from '../web/src/world.js';
import { realCity, realTransit } from './helpers/city.js';
import { idle } from './helpers/bot.js';

const city = realCity(), tr = realTransit();
const m10 = tr.patterns.filter((p) => p.name === 'M10').sort((a, b) => b.stops.length - a.stops.length)[0];

function worldWithTram(tauAt = 'moving') {
  const mid = pointOn(m10, m10.stops[Math.floor(m10.stops.length / 2)]);
  const w = createWorld({ city }); w.mission.state = 'idle'; w.clock = 12 * 60; w.day = 1;
  w.camera.x = mid.x; w.camera.y = mid.y; w.player.x = mid.x; w.player.y = mid.y; resetPopulation(w);
  for (let i = 0; i < 30; i++) updateWorld(w, idle(), 1 / 60);
  const s = w.transit.tracked.get(m10.id);
  let tau = 0; const want = tauAt === 'moving' ? (q) => !q.dwelling : (q) => q.dwelling;
  while (!(positionAt(m10, tau).s > m10.stops[Math.floor(m10.stops.length / 2)] - 400 && want(positionAt(m10, tau)))) tau += 0.5;
  const v = { tau, delay: 0, key: 'test' }; s.veh.push(v);
  return { w, v };
}

test('Fahrzeuglage: Wagen, Tempo aus dem Fahrplan, beim Halten 0; verschwundenes Fahrzeug → null', () => {
  const { w, v } = worldWithTram('moving');
  const st = vehicleState(w, { pid: m10.id, key: 'test' });
  assert.equal(st.mode, 'tram'); assert.equal(st.cars.length, 3); assert.ok(st.speed > 20, `Tempo ${st.speed}`);
  assert.equal(st.speed, speedOfPattern(m10, v.tau));
  v.gone = true; w.transit.tracked.get(m10.id).veh = w.transit.tracked.get(m10.id).veh.filter((x) => !x.gone);
  assert.equal(vehicleState(w, { pid: m10.id, key: 'test' }), null);
  assert.equal(vehicleState(w, { carId: -1 }), null);
  let tau = 0; while (!positionAt(m10, tau).dwelling || tau < 30) tau += 0.5;
  assert.equal(speedOfPattern(m10, tau), 0);
});

test('In Reichweite: nur nah am Wagen-Rechteck (25 px), sortiert, mit Wagennummer und Abstand zur Spitze', () => {
  const { w } = worldWithTram('moving');
  const st = vehicleState(w, { pid: m10.id, key: 'test' });
  const c2 = st.cars[2], nx = -Math.sin(c2.angle), ny = Math.cos(c2.angle);
  const near = transitNear(w, c2.x + nx * (c2.W / 2 + 10), c2.y + ny * (c2.W / 2 + 10), RIDE.reach);
  const hit = near.find((h) => h.ref.key === 'test');
  assert.ok(hit, 'neben Wagen 3'); assert.equal(hit.car, 2); assert.ok(hit.front > 200);
  assert.ok(!transitNear(w, c2.x + nx * (c2.W / 2 + 40), c2.y + ny * (c2.W / 2 + 40), RIDE.reach).some((h) => h.ref.key === 'test'), 'zu weit');
  for (let i = 1; i < near.length; i++) assert.ok(near[i].dist >= near[i - 1].dist);
});

test('Aussteigen: Platz neben dem Wagen, sonst null; Straßenausgang am Bahnhof', () => {
  const { w } = worldWithTram('dwell');
  const st = vehicleState(w, { pid: m10.id, key: 'test' });
  const sp = alightSpot(w, st, 1);
  assert.ok(sp && Math.hypot(sp.x - st.cars[1].x, sp.y - st.cars[1].y) < 60);
  const u8 = tr.patterns.filter((p) => p.name === 'U8').sort((a, b) => b.stops.length - a.stops.length)[0];
  const i = u8.stopNames.findIndex((n) => n.startsWith('U Kottbusser Tor'));
  const ex = stationExit(w, u8, i), stop = pointOn(u8, u8.stops[i]);
  assert.ok(Math.hypot(ex.x - stop.x, ex.y - stop.y) < 400, 'Ausgang in der Nähe des Bahnsteigs');
});
```

- [ ] **Step 2: Run** `node --test tests/ride.test.js` → FAIL (Modul fehlt).

- [ ] **Step 3: Implement** `web/src/ride.js`:

```js
// Mitfahren (rein): Lage eines Nahverkehrsfahrzeugs aus seiner Referenz, Fahrzeuge in Reichweite, Plätze zum Aussteigen.
// Referenzen: { pid, key } = Fahrplan-Fahrzeug (transit.js), { carId } = Bus als KI-Auto, { playerTrain: true } = Zug,
// den der Spieler führt (playertrain.js). Verschwindet ein Fahrzeug, liefert vehicleState null (world.js steigt dann aus).
import { positionAt, pointOn, trainCars, BUS } from './transit.js';
import { circleVsObb } from './collision.js';
import { undergroundAtS } from './tunnel.js';
import { nearestPoi } from './map.js';
import { nearestSpot, sidewalkPoint } from './pedestrians.js';

export const RIDE = { reach: 25, cab: 30, hopOn: 25 / 0.36, hopOff: 10 / 0.36, hurtFrom: 40 / 0.36, stun: 1.2, hurt: 10 };

// Tempo aus dem Fahrplan: Abstand der Halte durch die Fahrzeit ohne Haltezeit (wie transitlive.js)
export function speedOfPattern(p, tau) {
  const pos = positionAt(p, tau);
  if (pos.dwelling) return 0;
  const i = pos.stop;
  return (p.stops[i] - p.stops[i - 1]) / Math.max(1, p.off[i] - p.off[i - 1] - Math.min(p.dwell, (p.off[i] - p.off[i - 1]) * 0.4));
}

function busCars(c) { return [{ x: c.x, y: c.y, angle: c.angle, L: BUS.L, W: BUS.W, first: true, last: true }]; }

export function vehicleState(w, ref) {
  const tr = w.city.transit;
  if (!tr || !ref) return null;
  if (ref.carId !== undefined) {
    const c = w.cars.find((x) => x.id === ref.carId && x.duty?.bus && !x.wrecked);
    if (!c) return null;
    const p = tr.patterns[c.duty.pid];
    return { mode: 'bus', p, s: c.duty.s, speed: Math.hypot(c.vx, c.vy), cars: busCars(c), dwelling: !!c.duty.boarding, stop: c.duty.stop, underground: false };
  }
  if (ref.playerTrain) {
    const t = w.playerTrain;
    if (!t) return null;
    const p = tr.patterns[t.pid];
    return { mode: p.mode, p, s: t.s, speed: t.v, cars: trainCars(p, t.s), dwelling: t.v < 3, stop: t.nextStop ?? 0, underground: undergroundAtS(w.city, p, t.s) };
  }
  const s = w.transit?.tracked.get(ref.pid);
  const v = s?.veh.find((x) => x.key === ref.key && !x.gone);
  if (!v || v.live) return null;
  const p = tr.patterns[ref.pid], pos = positionAt(p, v.tau);
  if (pos.done) return null;
  return { mode: p.mode, p, s: pos.s, speed: v.blockedT > 0 ? 0 : speedOfPattern(p, v.tau), cars: trainCars(p, pos.s), dwelling: pos.dwelling, stop: pos.stop, underground: undergroundAtS(w.city, p, pos.s) };
}

// Abstand Punkt → Wagen-Rechteck (0 innen)
function distToCar(x, y, c) {
  const dx = x - c.x, dy = y - c.y, ca = Math.cos(c.angle), sa = Math.sin(c.angle);
  const lx = Math.abs(dx * ca + dy * sa) - c.L / 2, ly = Math.abs(-dx * sa + dy * ca) - c.W / 2;
  return Math.hypot(Math.max(0, lx), Math.max(0, ly));
}

export function transitNear(w, x, y, r) {
  const tr = w.city.transit, out = [];
  if (!tr) return out;
  const consider = (ref) => {
    const st = vehicleState(w, ref);
    if (!st || st.underground) return;
    st.cars.forEach((c, i) => {
      const d = distToCar(x, y, c);
      if (d > r) return;
      const f = st.cars[0], a = f.angle, fx = f.x + Math.cos(a) * f.L / 2, fy = f.y + Math.sin(a) * f.L / 2;
      out.push({ ref, mode: st.mode, dist: d, car: i, front: Math.hypot(x - fx, y - fy) });
    });
  };
  for (const [pid, s] of w.transit?.tracked ?? []) {
    const p = tr.patterns[pid];
    if (p.mode === 'bus') continue;
    for (const v of s.veh) if (!v.gone && !v.live) {
      const head = pointOn(p, positionAt(p, v.tau).s);
      if (Math.abs(head.x - x) < 2200 && Math.abs(head.y - y) < 2200) consider({ pid, key: v.key });
    }
  }
  for (const c of w.cars) if (c.duty?.bus && c.driver === 'npc' && Math.abs(c.x - x) < 200 && Math.abs(c.y - y) < 200) consider({ carId: c.id });
  if (w.playerTrain) consider({ playerTrain: true });
  // Nur je Fahrzeug der nächste Wagen
  const best = new Map();
  for (const h of out) { const k = JSON.stringify(h.ref); if (!best.has(k) || best.get(k).dist > h.dist) best.set(k, h); }
  return [...best.values()].sort((a, b) => a.dist - b.dist);
}

// Freier Platz neben Wagen i: rechts in Fahrtrichtung zuerst, dann links, dann hinter dem letzten Wagen
export function alightSpot(w, st, i) {
  const c = st.cars[Math.min(i, st.cars.length - 1)], nx = -Math.sin(c.angle), ny = Math.cos(c.angle), d = c.W / 2 + 12;
  const cands = [[c.x + nx * d, c.y + ny * d], [c.x - nx * d, c.y - ny * d]];
  const last = st.cars[st.cars.length - 1];
  cands.push([last.x - Math.cos(last.angle) * (last.L / 2 + 14), last.y - Math.sin(last.angle) * (last.L / 2 + 14)]);
  for (const [x, y] of cands) {
    if (w.cars.some((o) => circleVsObb(x, y, 8, o))) continue;
    if (w.solids.query({ x: x - 8, y: y - 8, w: 16, h: 16 }, []).some((s) => (s.lvl ?? 0) === (w.player.lvl ?? 0) && s.seg)) continue;
    return { x, y };
  }
  return null;
}

// Straßenausgang eines Bahnhofs: Bahnhofs-POI mit passendem Namen in der Nähe, sonst nächster Gehweg
export function stationExit(w, p, stopIndex) {
  const at = pointOn(p, p.stops[stopIndex]), name = (p.stopNames[stopIndex] ?? '').replace(/^[SU]\s+/, '').replace(/\s*\(.*\)$/, '');
  const poi = nearestPoi(w.city, at.x, at.y, 500, (q) => (q.cat === 'ubahn' || q.cat === 'sbahn' || q.cat === 'bahn') && (!name || q.name.includes(name)))
    ?? nearestPoi(w.city, at.x, at.y, 500, (q) => q.cat === 'ubahn' || q.cat === 'sbahn' || q.cat === 'bahn');
  const base = poi ?? at;
  const spot = nearestSpot(w.city, base.x, base.y);
  return spot ? sidewalkPoint(w.city, spot.edge, spot.side, spot.s) : { x: base.x, y: base.y };
}
```

Vorher prüfen: `grep -n "export function nearestPoi" -A8 web/src/map.js` (Signatur `nearestPoi(city, x, y, radius, filter)`; Rückgabe hat `name`, `cat`, `x`, `y`) und `sidewalkPoint(city, e, side, s, out)` in pedestrians.js (vorhanden). `w.solids` ist ein SpatialHash (world.js nutzt `w.solids.query(box, tmp)`); Segmente tragen `seg`.

- [ ] **Step 4: Run** `node --test tests/ride.test.js` → PASS.

- [ ] **Step 5: Mutationsprobe**: in `transitNear` `if (d > r) return;` → `if (d > r * 3) return;` → Test 2 muss scheitern; `if (pos.done) return null;` entfernen → Test 1 (null) prüfen, ob er scheitert (sonst Test um ein Fahrzeug am Linienende ergänzen: `v.tau = m10.duration + 1` → `vehicleState` null).

- [ ] **Step 6: Commit** `git add web/src/ride.js tests/ride.test.js && git commit -m "Ride queries: vehicle state, vehicles in reach, alight spots, station exits"`

---

### Task 4: Mitfahren in der Welt (Fahrgast)

**Files:**
- Modify: `web/src/world.js` (updateWorld ab Zeile ~521, updateCamera ~685, neue Funktionen `boardTransit`, `alightTransit`, `updateRide`, `endRide`)
- Modify: `web/src/save.js:11-20` (`makeSave`)
- Modify: `web/src/console.js` (Befehl `tp`: `endRide` vor dem Teleport)
- Modify: `web/src/combat.js` (`hurtPlayer`: bei `w.player.ride` erst `endRide`)
- Modify: `web/src/transitlive.js:24` (`tramBlocked`: Spieler während Fahrt ignorieren)
- Modify: `web/src/main.js` (`playingOnFoot` + `!game.world.player.ride`)
- Test: `tests/ride.test.js` (weitere Tests)

**Interfaces:**
- Consumes: Task 3 (`vehicleState`, `transitNear`, `alightSpot`, `stationExit`, `RIDE`), `input.ride`.
- Produces:
  - `w.player.ride = { kind: 'passenger', ref, mode, car, lastStop: { x, y, name }, since, line, dest }`
  - exportiert aus world.js: `endRide(w, reason)` (Notausstieg an `lastStop`), `boardTransit(w)`, `alightTransit(w)`
  - Ereignisse: `{ type: 'board', mode, line, hop }`, `{ type: 'alight', hop }`, `{ type: 'ride-end', reason }`

- [ ] **Step 1: Failing tests** (an `tests/ride.test.js` anhängen):

```js
import { endRide } from '../web/src/world.js';
import { makeSave, validateSave } from '../web/src/save.js';
const press = (w, patch) => updateWorld(w, { ...idle(), ...patch }, 1 / 60);

function standBeside(w, key, car = 1) {
  const st = vehicleState(w, { pid: m10.id, key }), c = st.cars[car], nx = -Math.sin(c.angle), ny = Math.cos(c.angle);
  w.player.x = c.x + nx * (c.W / 2 + 12); w.player.y = c.y + ny * (c.W / 2 + 12);
}

test('Fahrgast: G neben einer haltenden Tram steigt ein, Spieler fährt mit, G steigt neben der Tür aus', () => {
  const { w } = worldWithTram('dwell');
  standBeside(w, 'test');
  press(w, { ride: true });
  assert.equal(w.player.ride?.kind, 'passenger'); assert.equal(w.player.ride.mode, 'tram');
  assert.ok(w.events.some((e) => e.type === 'board' && !e.hop));
  let moved = 0; const x0 = w.player.x;
  for (let i = 0; i < 60 * 40; i++) { press(w, {}); w.camera.x = w.player.x; w.camera.y = w.player.y; moved = Math.max(moved, Math.hypot(w.player.x - x0, w.player.y - w.player.y)); if (vehicleState(w, w.player.ride.ref)?.dwelling && i > 60 * 20) break; }
  assert.ok(Math.hypot(w.player.x - x0, 0) > 100 || moved > 100, 'fährt mit');
  const st = vehicleState(w, w.player.ride.ref);
  press(w, { ride: true });
  assert.equal(w.player.ride, null);
  assert.ok(Math.hypot(w.player.x - st.cars[1].x, w.player.y - st.cars[1].y) < 80, 'neben dem Wagen');
  assert.ok(w.player.stun <= 0, 'kein Sturz bei stehender Bahn');
});

test('Aufspringen und Abspringen während der Fahrt; Fahrt-Ende, wenn das Fahrzeug verschwindet', () => {
  const { w, v } = worldWithTram('moving');
  standBeside(w, 'test', 0);
  press(w, { ride: true });
  assert.ok(w.player.ride, 'aufgesprungen'); assert.ok(w.events.some((e) => e.type === 'board' && e.hop));
  for (let i = 0; i < 20; i++) press(w, {});
  // schnell genug zum Abspringen? Tempo fest setzen über die Tram-Lage
  const hp = w.player.hp;
  press(w, { ride: true });
  assert.equal(w.player.ride, null);
  if (speedOfPattern(m10, v.tau) > RIDE.hopOff) { assert.ok(w.player.stun > 0, 'betäubt nach dem Absprung'); assert.ok(w.player.hp <= hp); }
  // Fahrzeug verschwindet → Notausstieg an der letzten Haltestelle
  standBeside(w, 'test', 0); press(w, { ride: true });
  const last = w.player.ride.lastStop;
  v.gone = true;
  press(w, {});
  assert.equal(w.player.ride, null);
  assert.ok(Number.isFinite(w.player.x) && Math.hypot(w.player.x - last.x, w.player.y - last.y) < 400, 'an der letzten Haltestelle');
  assert.ok(w.events.some((e) => e.type === 'ride-end'));
});

test('Während der Fahrt: kein Laufen, kein Schießen, Kamera folgt; Speichern = letzte Haltestelle zu Fuß; tp beendet die Fahrt', () => {
  const { w } = worldWithTram('dwell');
  standBeside(w, 'test'); press(w, { ride: true });
  const st = vehicleState(w, w.player.ride.ref);
  press(w, { moveX: 1, fire: true, firePressed: true });
  assert.ok(Math.hypot(w.player.x - st.cars[w.player.ride.car].x, w.player.y - st.cars[w.player.ride.car].y) < 5, 'sitzt im Wagen');
  assert.ok(!w.events.some((e) => e.type === 'shot' || e.type === 'swing'));
  const save = validateSave(makeSave(w));
  assert.ok(Math.hypot(save.player.x - w.player.ride.lastStop.x, save.player.y - w.player.ride.lastStop.y) < 1);
  endRide(w, 'teleport');
  assert.equal(w.player.ride, null);
  // umgehauen während der Fahrt (z. B. Schuss durchs Fenster): Fahrt endet, niemand fährt als Toter weiter
  standBeside(w, 'test'); press(w, { ride: true });
  assert.ok(w.player.ride);
  w.player.dead = true; press(w, {});
  assert.equal(w.player.ride, null);
});
```

- [ ] **Step 2: Run** `node --test tests/ride.test.js` → FAIL.

- [ ] **Step 3: Implement** in `world.js`:
  - Import: `import { vehicleState, transitNear, alightSpot, stationExit, RIDE } from './ride.js';`
  - Neue Funktionen (vor `updateWorld`):

```js
// Mitfahren (Fahrgast): eigene Taste (input.ride). Einsteigen überall in Reichweite eines Wagens, auch in Fahrt
// (Aufspringen); Aussteigen jederzeit, schnell = Abspringen mit Sturz; unter Tage nur am Bahnsteig, Ausgang an der Straße.
function lastStopOf(st) {
  const i = Math.max(0, Math.min(st.p.stops.length - 1, st.dwelling ? st.stop : st.stop - 1));
  const q = pointOn(st.p, st.p.stops[i]);
  return { x: q.x, y: q.y, name: st.p.stopNames[i] ?? '', i, pid: st.p.id };
}
export function boardTransit(w) {
  const p = w.player;
  const hit = transitNear(w, p.x, p.y, RIDE.reach).find((h) => !h.ref.playerTrain);
  if (!hit) return false;
  const st = vehicleState(w, hit.ref);
  const hop = st.speed > RIDE.hopOn;
  p.ride = { kind: 'passenger', ref: hit.ref, mode: st.mode, car: hit.car, lastStop: lastStopOf(st), since: w.time, line: st.p.name, dest: st.p.stopNames[st.p.stopNames.length - 1] };
  w.events.push({ type: 'board', mode: st.mode, line: st.p.name, hop, x: p.x, y: p.y });
  return true;
}
export function alightTransit(w) {
  const p = w.player, r = p.ride, st = vehicleState(w, r.ref);
  if (!st) { endRide(w, 'gone'); return true; }
  if (st.underground) {
    if (!st.dwelling) { w.notice = { text: 'Nur am Bahnsteig', t: 1.5 }; return false; }
    const ex = stationExit(w, st.p, st.stop);
    p.ride = null; p.x = ex.x; p.y = ex.y; p.lvl = 0;
    w.events.push({ type: 'alight', hop: false, x: p.x, y: p.y });
    return true;
  }
  const spot = alightSpot(w, st, r.car);
  if (!spot) { w.notice = { text: 'Kein Platz zum Aussteigen', t: 1.5 }; return false; }
  const hop = st.speed > RIDE.hopOff;
  p.ride = null; p.x = spot.x; p.y = spot.y;
  if (hop) {
    const c = st.cars[Math.min(r.car, st.cars.length - 1)];
    p.x += Math.cos(c.angle) * 20; p.y += Math.sin(c.angle) * 20; // Schwung in Fahrtrichtung
    p.stun = RIDE.stun;
    if (st.speed > RIDE.hurtFrom) hurtPlayer(w, RIDE.hurt, c.x, c.y);
  }
  w.events.push({ type: 'alight', hop, x: p.x, y: p.y });
  return true;
}
// Fahrt beenden, ohne Fahrzeug (verschwunden, Teleport, K. o.): an der letzten Haltestelle zu Fuß
export function endRide(w, reason) {
  const p = w.player, r = p.ride;
  if (!r) return;
  p.ride = null;
  if (reason !== 'teleport') {
    const tr = w.city.transit, pat = tr?.patterns[r.lastStop.pid];
    const ex = pat && (pat.mode === 'ubahn' || pat.mode === 'sbahn') ? stationExit(w, pat, r.lastStop.i) : r.lastStop;
    p.x = ex.x; p.y = ex.y; p.lvl = 0;
  }
  w.events.push({ type: 'ride-end', reason, x: p.x, y: p.y });
}
function updateRide(w) {
  const p = w.player, r = p.ride;
  if (p.dead) { endRide(w, 'ko'); return; }
  const st = vehicleState(w, r.ref);
  if (!st) { endRide(w, 'gone'); return; }
  const c = st.cars[Math.min(r.car, st.cars.length - 1)];
  p.x = c.x; p.y = c.y; p.angle = c.angle; p.lvl = st.underground ? -2 : (w.transit && c.lvl) || p.lvl;
  if (st.dwelling) r.lastStop = lastStopOf(st);
  r.speed = st.speed; r.underground = st.underground;
}
```

  - `pointOn` import aus transit.js und `hurtPlayer` (schon importiert) prüfen.
  - In `updateWorld` die Zeile `if (input.enterExit && !p.dead) { … }` ersetzen durch:

```js
  if (input.ride && !p.dead && !p.inCar) { if (p.ride) alightTransit(w); else boardTransit(w); }
  else if (input.enterExit && !p.dead && !p.ride) { if (p.inCar) tryExit(w); else tryEnter(w); }
```

  - `} else if (!p.dead) updatePlayerOnFoot(w, input, dt);` → `} else if (!p.dead && !p.ride) updatePlayerOnFoot(w, input, dt);`
  - `updatePlayerCombat(w, input, dt);` → `if (!p.ride) updatePlayerCombat(w, input, dt);`
  - nach `updateTransit(w, dt);` : `if (p.ride) updateRide(w);`
  - Spieler-zu-Fuß-gegen-Autos-Block: `if (!p.inCar) {` → `if (!p.inCar && !p.ride) {`
  - `updateCamera`: nach `if (car) { … }` ergänzen:

```js
  if (p.ride) { tx = p.x; ty = p.y; zoom = 1 - clamp((p.ride.speed ?? 0) / 330, 0, 1) * 0.28; }
```

  - `save.js makeSave`: `const pos = playerCar(w) ?? w.player;` → `const pos = w.player.ride ? w.player.ride.lastStop : playerCar(w) ?? w.player;`
  - `combat.js hurtPlayer`: am Anfang `if (w.player.ride && !w.player.ride.kind) {}` ist nicht nötig; stattdessen: in `updateKnockout`-Pfad nichts – Fahrgäste werden nicht getroffen, weil `updatePlayerCombat` aussetzt und Passanten den Spieler im Wagen nicht erreichen. Für Schüsse von Gegnern (`updateFight` zielt auf `w.player`): in `combat.js` `updateFight` am Anfang `if (w.player.ride) { ped.state = 'walk'; return; }` (Gegner lassen ab). `grep -n "export function updateFight" web/src/combat.js` für die Stelle.
  - `console.js` `tp`-Befehl: vor `ctx.game.teleport = …` → `if (ctx.world.player.ride) endRide(ctx.world, 'teleport');` (Import `endRide` aus world.js).
  - `transitlive.js tramBlocked`: `if (!w.player.inCar && !w.player.dead && hit(…))` → `if (!w.player.inCar && !w.player.ride && !w.player.dead && hit(…))`; ebenso in `collideRail`: `if (!pl.inCar)` → `if (!pl.inCar && !pl.ride)`.
  - `main.js`: `playingOnFoot` um `&& !game.world.player.ride` ergänzen.
  - `mission.js`: Aufträge nutzen `action`; Fahrgäste sollen nicht einladen: `grep -n "player.inCar" web/src/mission.js` – wo „zu Fuß“ geprüft wird, `&& !ctx.player.ride` ergänzen.

- [ ] **Step 4: Run** `node --test tests/ride.test.js tests/transit.test.js tests/mission.test.js tests/combat.test.js tests/save.test.js tests/console.test.js` → PASS.

- [ ] **Step 5: Mutationsproben**: (a) `if (hop) {` → `if (false) {` → Test 2 muss scheitern (wenn die Tram schnell genug ist; sonst im Test `v.tau` so wählen, dass `speedOfPattern > RIDE.hopOff`); (b) `if (!st) { endRide(w, 'gone'); return; }` in `updateRide` entfernen → Test 2 scheitert; (c) `makeSave`-Änderung zurücknehmen → Test 3 scheitert; (d) `!p.ride` vor `updatePlayerCombat` entfernen → Test 3 scheitert.

- [ ] **Step 6: Commit** `git add web/src/world.js web/src/save.js web/src/console.js web/src/combat.js web/src/transitlive.js web/src/main.js web/src/mission.js tests/ride.test.js && git commit -m "Ride as passenger: board anywhere (hop on), alight/hop off, emergency exit, save at last stop"`

---

### Task 5: Zugphysik `trainphysics.js`

**Files:**
- Create: `web/src/trainphysics.js`
- Modify: `web/src/config.js` (`TRAIN_DRIVE`)
- Test: `tests/trainphysics.test.js`

**Interfaces:**
- Produces:
  - `TRAIN_DRIVE = { tram: { vmax: 60 / 0.36, acc: 13, brake: 15, emergency: 25 }, ubahn: { vmax: 70 / 0.36, acc: 11, brake: 12, emergency: 25 }, sbahn: { vmax: 100 / 0.36, acc: 10, brake: 12, emergency: 25 }, roll: 0.6, stopZone: 250, stopExact: 30, stillV: 3, tipMax: 10, gentle: 13, doorsAuto: 20 }` (px/s², 10 px = 1 m ⇒ 13 px/s² = 1,3 m/s²)
  - `createDrive(mode, v = 0) → { mode, v, doors: 'closed', doorT: 0, decel: [], stopped: false }`
  - `stepDrive(d, { throttle, brake, emergency, limit }, dt) → d` (limit = maximal erlaubter Weg bis Hindernis/Endhalt in px; der Zug bremst selbsttätig so, dass er davor steht)
  - `stopInfo(p, s) → { i, dist } | null` (nächste Haltestelle in ±stopZone, Vorzeichen: Spitze hinter (−)/vor (+) dem Halt)
  - `tipFor(dist, maxDecel) → € (0…10)`
  - `brakeDistance(v, a) → px`

- [ ] **Step 1: Failing tests** `tests/trainphysics.test.js`:

```js
import test from 'node:test';
import assert from 'node:assert/strict';
import { createDrive, stepDrive, stopInfo, tipFor, brakeDistance } from '../web/src/trainphysics.js';
import { TRAIN_DRIVE } from '../web/src/config.js';

const run = (d, input, sec, fps = 60) => { for (let i = 0; i < sec * fps; i++) stepDrive(d, { limit: Infinity, ...input }, 1 / fps); return d; };

test('Zugfahrt: Höchsttempo je Art, nie rückwärts, bildratenunabhängig', () => {
  for (const [mode, kmh] of [['tram', 60], ['ubahn', 70], ['sbahn', 100]]) {
    const d = run(createDrive(mode), { throttle: 1 }, 120);
    assert.ok(Math.abs(d.v * 0.36 - kmh) < 0.5, `${mode} ${d.v * 0.36}`);
  }
  const d = run(createDrive('ubahn', 50), { brake: 1 }, 30);
  assert.equal(d.v, 0);
  const a = run(createDrive('sbahn'), { throttle: 1 }, 10, 30), b = run(createDrive('sbahn'), { throttle: 1 }, 10, 144);
  assert.ok(Math.abs(a.v - b.v) < 1, 'gleich bei 30 und 144 Hz');
  const roll = run(createDrive('tram', 100), {}, 5);
  assert.ok(roll.v < 100 && roll.v > 80, 'rollt aus, langsam');
});

test('Zugfahrt: Bremsweg, Notbremse kürzer, Zwangsbremsung vor Hindernis, steht bei offenen Türen', () => {
  const v0 = 70 / 0.36;
  const bd = (input) => { const d = createDrive('ubahn', v0); let s = 0; while (d.v > 0) { stepDrive(d, { limit: Infinity, ...input }, 1 / 60); s += d.v / 60; } return s; };
  const normal = bd({ brake: 1 }), emergency = bd({ emergency: true });
  assert.ok(Math.abs(normal - brakeDistance(v0, TRAIN_DRIVE.ubahn.brake)) < 25, `Bremsweg ${normal}`);
  assert.ok(emergency < normal * 0.6, 'Notbremse');
  // Hindernis 400 px voraus bei voller Fahrt: steht davor
  const d = createDrive('sbahn', 100 / 0.36); let s = 0;
  for (let i = 0; i < 60 * 30 && (d.v > 0 || i < 10); i++) { stepDrive(d, { throttle: 1, limit: 1800 - s }, 1 / 60); s += d.v / 60; }
  assert.ok(s <= 1800 + 1 && d.v === 0, `hält vor dem Hindernis (${s.toFixed(0)} px)`);
  const o = createDrive('tram'); o.doors = 'open';
  run(o, { throttle: 1 }, 2); assert.equal(o.v, 0);
});

test('Haltestellen und Trinkgeld', () => {
  const p = { stops: [0, 3000, 6000] };
  assert.deepEqual(stopInfo(p, 2900), { i: 1, dist: -100 });
  assert.deepEqual(stopInfo(p, 3120), { i: 1, dist: 120 });
  assert.equal(stopInfo(p, 4500), null);
  assert.equal(tipFor(0, 10), 10); assert.equal(tipFor(29, 12), 10);
  assert.ok(tipFor(120, 10) > 0 && tipFor(120, 10) < 10, 'anteilig');
  assert.equal(tipFor(260, 10), 0);
  assert.ok(tipFor(0, 25) < 5, 'harte Bremsung kostet');
});
```

- [ ] **Step 2: Run** `node --test tests/trainphysics.test.js` → FAIL.

- [ ] **Step 3: Implement** – `config.js` anhängen:

```js
// Selbst gefahrene Bahnen (trainphysics.js): Höchsttempo, Anfahren, Bremsen in px/s bzw. px/s² (10 px = 1 m)
export const TRAIN_DRIVE = {
  tram: { vmax: 60 / 0.36, acc: 13, brake: 15, emergency: 25 },
  ubahn: { vmax: 70 / 0.36, acc: 11, brake: 12, emergency: 25 },
  sbahn: { vmax: 100 / 0.36, acc: 10, brake: 12, emergency: 25 },
  roll: 0.6, stopZone: 250, stopExact: 30, stillV: 3, tipMax: 10, gentle: 13, doorsAuto: 20,
};
```

`web/src/trainphysics.js`:

```js
// Zug führen (rein): nur Tempo entlang der Linie. Anfahren mit abnehmender Zugkraft über 40 % der Höchstgeschwindigkeit,
// Bremse und Notbremse konstant, Ausrollen langsam; vor einem Hindernis/Endhalt (limit = freier Weg in px) bremst der Zug
// selbsttätig so, dass er davor steht. Bei offenen Türen steht er. Nie rückwärts.
import { TRAIN_DRIVE } from './config.js';

export const brakeDistance = (v, a) => (v * v) / (2 * a);

export function createDrive(mode, v = 0) { return { mode, v, doors: 'closed', doorT: 0, decel: [], stopped: v <= 0 }; }

export function stepDrive(d, input, dt) {
  const k = TRAIN_DRIVE[d.mode], v0 = d.v;
  let a;
  if (d.doors !== 'closed') { d.v = 0; d.doorT += dt; return d; }
  if (input.emergency) a = -k.emergency;
  else if (input.brake > 0) a = -k.brake * input.brake;
  else if (input.throttle > 0) a = k.acc * input.throttle * (d.v < k.vmax * 0.4 ? 1 : Math.max(0, (k.vmax - d.v) / (k.vmax * 0.6)));
  else a = -TRAIN_DRIVE.roll;
  let v = Math.max(0, Math.min(k.vmax, d.v + a * dt));
  // Zwangsbremsung: nie weiter als limit (Hindernis voraus, Endhalt)
  const lim = input.limit ?? Infinity;
  if (lim < Infinity) {
    const vAllowed = Math.sqrt(Math.max(0, 2 * k.emergency * Math.max(0, lim - v * dt)));
    if (v > vAllowed) v = Math.max(0, Math.min(v, vAllowed));
    if (lim <= 1) v = 0;
  }
  d.v = v;
  const decel = Math.max(0, (v0 - v) / dt);
  d.decel.push([dt, decel]);
  let tsum = 0; for (let i = d.decel.length - 1; i >= 0; i--) { tsum += d.decel[i][0]; if (tsum > 8) { d.decel.splice(0, i); break; } }
  d.stopped = v < TRAIN_DRIVE.stillV;
  return d;
}

export const maxDecel = (d) => d.decel.reduce((m, [, x]) => Math.max(m, x), 0);

export function stopInfo(p, s) {
  let best = null;
  p.stops.forEach((st, i) => { const dist = s - st; if (Math.abs(dist) <= TRAIN_DRIVE.stopZone && (!best || Math.abs(dist) < Math.abs(best.dist))) best = { i, dist }; });
  return best;
}

export function tipFor(dist, maxDec) {
  const z = TRAIN_DRIVE.stopZone, e = TRAIN_DRIVE.stopExact;
  const place = Math.abs(dist) <= e ? 1 : Math.max(0, 1 - (Math.abs(dist) - e) / (z - e));
  const gentle = maxDec <= TRAIN_DRIVE.gentle ? 1 : Math.max(0, 1 - (maxDec - TRAIN_DRIVE.gentle) / TRAIN_DRIVE.gentle);
  return Math.round(TRAIN_DRIVE.tipMax * place * gentle * 100) / 100;
}
```

Hinweis: `maxDecel` wird für `tipFor` genutzt (Task 6). Der Bildraten-Test verlangt, dass `stepDrive` nur von `v` und `dt` abhängt – ist so.

- [ ] **Step 4: Run** `node --test tests/trainphysics.test.js` → PASS (bei Abweichung im Bremsweg-Test die Toleranz nicht aufweiten, sondern die Integration prüfen: Bremsweg bei 60 Hz ≈ analytisch ± v·dt).

- [ ] **Step 5: Mutationsproben**: Zwangsbremsung (`if (v > vAllowed) …`) entfernen → Test 2 scheitert; Tür-Zeile entfernen → Test 2 scheitert; `Math.max(0, …)` bei `v` → negative Tempi → Test 1 scheitert; `gentle`-Faktor auf 1 → Test 3 scheitert.

- [ ] **Step 6: Commit** `git add web/src/config.js web/src/trainphysics.js tests/trainphysics.test.js && git commit -m "Train driving physics (trainphysics.js)"`

---

### Task 6: Spielerzug in der Welt (`playertrain.js`)

**Files:**
- Create: `web/src/playertrain.js`
- Modify: `web/src/world.js` (Übernahme mit `enterExit` am Führerstand, `updatePlayerTrain` aufrufen, Kamera, Aussteigen)
- Modify: `web/src/transitlive.js` (Spielerzug in `railObs`; Fahrplan-Züge desselben Musters hinter ihm warten)
- Test: `tests/playertrain.test.js`

**Interfaces:**
- Consumes: Task 2 (`undergroundAtS`), Task 3 (`transitNear`, `vehicleState`, `alightSpot`, `stationExit`, `RIDE`), Task 5 (`createDrive`, `stepDrive`, `stopInfo`, `tipFor`, `maxDecel`, `TRAIN_DRIVE`).
- Produces:
  - `w.playerTrain = { pid, s, v, drive, nextStop, served: [], atStop: null|{ i, dist }, leftT: null, passengers: 0 }`
  - `takeTrain(w, hit) → boolean` (hit aus `transitNear` mit `front <= RIDE.cab`, `car === 0`, Modus tram/sbahn/ubahn)
  - `updatePlayerTrain(w, input, dt)` (Fahrer: Eingaben; ohne Fahrer: steht, nach 30 s außer Sicht entfernt)
  - `leaveTrain(w) → boolean` (Aussteigen als Fahrer; unter Tage nur am Bahnsteig)
  - `turnAround(w) → boolean`
  - `trainAhead(w, t) → px` (freier Weg bis zum nächsten Zug auf gleicher Strecke, `Infinity` wenn keiner)
  - `w.player.ride = { kind: 'driver', ref: { playerTrain: true }, … }` (gleiches Feld wie Task 4)
  - Ereignisse: `train-take`, `doors-open`, `doors-close`, `tip {amount}`, `train-blocked`, `turnaround`

- [ ] **Step 1: Failing tests** `tests/playertrain.test.js`:

```js
import test from 'node:test';
import assert from 'node:assert/strict';
import { takeTrain, trainAhead, turnAround } from '../web/src/playertrain.js';
import { transitNear, vehicleState, RIDE } from '../web/src/ride.js';
import { positionAt, pointOn } from '../web/src/transit.js';
import { createWorld, updateWorld, resetPopulation } from '../web/src/world.js';
import { realCity, realTransit } from './helpers/city.js';
import { idle } from './helpers/bot.js';

const city = realCity(), tr = realTransit();
const pat = (name, mode) => tr.patterns.filter((p) => p.name === name && p.mode === mode).sort((a, b) => b.stops.length - a.stops.length)[0];
const press = (w, patch = {}) => { updateWorld(w, { ...idle(), ...patch }, 1 / 60); w.camera.x = w.player.x; w.camera.y = w.player.y; };

function atFrontOf(p, stopIdx) {
  const at = pointOn(p, p.stops[stopIdx]);
  const w = createWorld({ city }); w.mission.state = 'idle'; w.clock = 12 * 60; w.day = 1;
  w.camera.x = at.x; w.camera.y = at.y; w.player.x = at.x; w.player.y = at.y; resetPopulation(w);
  for (let i = 0; i < 30; i++) press(w);
  const s = w.transit.tracked.get(p.id);
  let tau = 0; while (!(positionAt(p, tau).stop === stopIdx && positionAt(p, tau).dwelling)) tau += 0.5;
  s.veh.push({ tau, delay: 0, key: 'mine' });
  const st = vehicleState(w, { pid: p.id, key: 'mine' }), f = st.cars[0];
  w.player.x = f.x + Math.cos(f.angle) * (f.L / 2 + 10); w.player.y = f.y + Math.sin(f.angle) * (f.L / 2 + 10);
  return w;
}

test('Übernehmen: Y am Führerstand macht aus der Fahrplan-Bahn den Spielerzug – gleiche Lage, Fahrplan-Fahrzeug weg', () => {
  const p = pat('M10', 'tram'), w = atFrontOf(p, 3);
  const before = vehicleState(w, { pid: p.id, key: 'mine' });
  press(w, { enterExit: true });
  assert.ok(w.playerTrain, 'Spielerzug'); assert.equal(w.player.ride.kind, 'driver');
  assert.ok(Math.abs(w.playerTrain.s - before.s) < 1);
  assert.equal(vehicleState(w, { pid: p.id, key: 'mine' }), null, 'aus dem Fahrplan genommen');
  // weiter hinten stehend (nicht am Führerstand) übernimmt man nicht
  const w2 = atFrontOf(p, 3), st = vehicleState(w2, { pid: p.id, key: 'mine' }), c = st.cars[2];
  w2.player.x = c.x + 20; w2.player.y = c.y + 20;
  press(w2, { enterExit: true });
  assert.equal(w2.playerTrain ?? null, null);
});

test('Fahren: Gas bewegt entlang der Linie, Türen nur im Stand an der Haltestelle, Trinkgeld beim sauberen Halt', () => {
  const p = pat('M10', 'tram'), w = atFrontOf(p, 3);
  press(w, { enterExit: true });
  const t = w.playerTrain, s0 = t.s;
  for (let i = 0; i < 60 * 8; i++) press(w, { throttle: 1 });
  assert.ok(t.s > s0 + 200, 'fährt'); assert.ok(t.v > 0);
  press(w, { action: true });
  assert.equal(t.drive.doors, 'closed', 'Türen nicht in Fahrt');
  // sanft auf die nächste Haltestelle zu
  const target = p.stops[4]; let money = w.money;
  for (let i = 0; i < 60 * 120 && !(t.v === 0 && Math.abs(t.s - target) < 60); i++) {
    const rest = target - t.s, want = Math.sqrt(Math.max(0, 2 * 10 * Math.max(0, rest - 10)));
    press(w, t.v > want ? { brake: Math.min(1, (t.v - want) / 20 + 0.3) } : { throttle: rest > 20 ? 0.6 : 0 });
  }
  assert.ok(Math.abs(t.s - target) < 60, `steht am Halt (${(t.s - target).toFixed(0)} px)`);
  press(w, { action: true });
  assert.equal(t.drive.doors, 'open');
  assert.ok(w.events.some((e) => e.type === 'tip') && w.money > money, 'Trinkgeld');
  for (let i = 0; i < 60 * 25; i++) press(w, { throttle: 1 });
  assert.equal(t.drive.doors, 'closed', 'schließen nach 20 s selbst');
});

test('Zug voraus: der Spielerzug hält davor und fährt nie hinein; Fahrplan-Zug dahinter wartet', () => {
  const p = pat('U1', 'ubahn'), w = atFrontOf(p, 2);
  press(w, { enterExit: true });
  const t = w.playerTrain, s = w.transit.tracked.get(p.id);
  // Fahrplan-Zug 500 px voraus (Spitze), auf demselben Muster
  let tau = 0; while (positionAt(p, tau).s < t.s + 500 + 6 * 165) tau += 0.2;
  const ahead = { tau, delay: 0, key: 'ahead' }; s.veh.push(ahead);
  const ahead0 = positionAt(p, tau).s;
  assert.ok(trainAhead(w, t) < 700);
  for (let i = 0; i < 60 * 30; i++) { ahead.tau = tau; press(w, { throttle: 1 }); }
  const tail = ahead0 - 6 * 165;
  assert.ok(t.s < tail - 50, `Abstand gehalten (${(tail - t.s).toFixed(0)} px)`);
  // Fahrplan-Zug hinter dem Spielerzug holt nicht auf
  let tb = 0; while (positionAt(p, tb).s < t.s - 6 * 165 - 700) tb += 0.2;
  const behind = { tau: tb, delay: 0, key: 'behind' }; s.veh.push(behind);
  for (let i = 0; i < 60 * 60; i++) press(w);
  assert.ok(positionAt(p, behind.tau).s < t.s - 6 * 165 - 400, 'wartet hinter dem Spielerzug');
});

test('Linienende: Zug hält, Wenden auf die Gegenrichtung, sonst nur aussteigen', () => {
  const p = pat('M10', 'tram'), n = p.stops.length, w = atFrontOf(p, n - 2);
  press(w, { enterExit: true });
  const t = w.playerTrain;
  for (let i = 0; i < 60 * 180; i++) press(w, { throttle: 1 });
  assert.ok(t.s <= p.stops[n - 1] + 1 && t.v === 0, 'steht am Endhalt');
  press(w, { action: true }); assert.equal(t.drive.doors, 'open', 'erst Türen');
  press(w, { action: true }); assert.equal(t.drive.doors, 'closed');
  const ok = turnAround(w);
  const back = tr.patterns.find((q) => q.name === p.name && q.id !== p.id && Math.hypot(pointOn(q, q.stops[0]).x - pointOn(p, p.stops[n - 1]).x, pointOn(q, q.stops[0]).y - pointOn(p, p.stops[n - 1]).y) < 600);
  assert.equal(ok, !!back);
  if (back) { assert.equal(w.playerTrain.pid, back.id); assert.ok(Math.abs(w.playerTrain.s - back.stops[0]) < 5); }
});
```

- [ ] **Step 2: Run** `node --test tests/playertrain.test.js` → FAIL.

- [ ] **Step 3: Implement** `web/src/playertrain.js`:

```js
// Vom Spieler geführter Zug: verlässt den Fahrplan (das virtuelle Fahrzeug wird entfernt) und fährt mit trainphysics.js
// entlang seiner Linie. Züge auf derselben Strecke voraus begrenzen den Weg (Zwangsbremsung), Fahrplan-Züge desselben
// Musters dahinter warten (transitlive.js blocked). Türen, Haltestellen, Trinkgeld, Wenden am Linienende.
import { positionAt, pointOn, trainCars, TRAIN } from './transit.js';
import { createDrive, stepDrive, stopInfo, tipFor, maxDecel } from './trainphysics.js';
import { TRAIN_DRIVE } from './config.js';
import { hash01 } from './map.js';
import { RIDE, vehicleState, alightSpot, stationExit } from './ride.js';

const trainLen = (mode) => { const k = TRAIN[mode]; return k.cars * k.carL + (k.cars - 1) * k.gap; };
const SAFE = 80; // px Abstand zum Zug voraus

export function takeTrain(w, hit) {
  if (!hit || hit.ref.carId !== undefined || hit.ref.playerTrain || hit.car !== 0 || hit.front > RIDE.cab) return false;
  const st = vehicleState(w, hit.ref);
  if (!st || st.mode === 'bus') return false;
  const s = w.transit.tracked.get(hit.ref.pid), v = s.veh.find((x) => x.key === hit.ref.key);
  v.gone = true; // aus dem Fahrplan
  const pos = positionAt(st.p, v.tau);
  w.playerTrain = { pid: st.p.id, s: st.s, v: st.speed, drive: createDrive(st.mode, st.speed), nextStop: pos.stop, served: [], atStop: null, leftT: null, passengers: 20 + Math.floor(hash01(st.p.id * 31 + Math.floor(w.clock)) * 60) };
  w.player.ride = { kind: 'driver', ref: { playerTrain: true }, mode: st.mode, car: 0, lastStop: { ...pointOn(st.p, st.p.stops[Math.max(0, pos.stop - 1)]), name: st.p.stopNames[Math.max(0, pos.stop - 1)], i: Math.max(0, pos.stop - 1), pid: st.p.id }, since: w.time, line: st.p.name, dest: st.p.stopNames[st.p.stopNames.length - 1] };
  w.events.push({ type: 'train-take', mode: st.mode, line: st.p.name, x: w.player.x, y: w.player.y });
  return true;
}

// Freier Weg bis zum Heck des nächsten Zugs voraus auf derselben Strecke (gleiches Muster oder Muster, deren Weg hier
// auf ≤ 30 px mit dem eigenen übereinstimmt), minus Sicherheitsabstand
export function trainAhead(w, t) {
  const tr = w.city.transit, p = tr.patterns[t.pid];
  let free = Infinity;
  const look = 2500;
  const probe = pointOn(p, t.s + 200);
  for (const [pid, s] of w.transit?.tracked ?? []) {
    const q = tr.patterns[pid];
    if (q.mode === 'bus' || q.mode !== p.mode) continue;
    for (const v of s.veh) {
      if (v.gone) continue;
      const qs = positionAt(q, v.tau).s, head = pointOn(q, qs);
      if (Math.abs(head.x - probe.x) > look || Math.abs(head.y - probe.y) > look) continue;
      // Heck des anderen Zugs auf den eigenen Weg projizieren: nur gleiche Strecke (Abstand ≤ 30 px) zählt
      const tailS = qs - trainLen(q.mode);
      const tail = pointOn(q, tailS);
      for (let d = 0; d <= look; d += 20) {
        const m = pointOn(p, t.s + d);
        if (Math.hypot(m.x - tail.x, m.y - tail.y) <= 30) { free = Math.min(free, Math.max(0, d - SAFE)); break; }
      }
    }
  }
  return free;
}

export function updatePlayerTrain(w, input, dt) {
  const t = w.playerTrain;
  if (!t) return;
  const tr = w.city.transit, p = tr.patterns[t.pid], k = TRAIN_DRIVE, driving = w.player.ride?.kind === 'driver';
  const endFree = Math.max(0, p.stops[p.stops.length - 1] - t.s);
  const aheadFree = trainAhead(w, t);
  const limit = Math.min(endFree, aheadFree);
  const inp = driving ? { throttle: input.throttle, brake: input.brake, emergency: !!input.handbrake, limit } : { brake: 1, limit };
  const wasBlocked = t.blocked;
  stepDrive(t.drive, inp, dt);
  t.blocked = aheadFree < 400 && t.drive.v < 5;
  if (t.blocked && !wasBlocked) w.events.push({ type: 'train-blocked' });
  t.v = t.drive.v; t.s += t.v * dt;
  while (t.nextStop < p.stops.length - 1 && t.s > p.stops[t.nextStop] + k.stopZone) t.nextStop++;
  t.atStop = t.drive.stopped ? stopInfo(p, t.s) : null;
  // Türen (E/A): nur im Stand an einer Haltestelle
  if (driving && input.action) {
    if (t.drive.doors === 'closed' && t.atStop) {
      t.drive.doors = 'open'; t.drive.doorT = 0;
      const tip = t.served.includes(t.atStop.i) ? 0 : tipFor(t.atStop.dist, maxDecel(t.drive));
      t.served.push(t.atStop.i);
      const out = Math.floor(hash01(t.pid * 97 + t.atStop.i + Math.floor(w.clock / 10)) * 12), inn = Math.floor(hash01(t.pid * 53 + t.atStop.i * 7 + Math.floor(w.clock / 10)) * 14);
      t.passengers = Math.max(0, t.passengers - out) + inn;
      w.events.push({ type: 'doors-open', out, inn });
      if (tip > 0) { w.money += Math.round(tip); w.events.push({ type: 'tip', amount: Math.round(tip) }); }
      const q = pointOn(p, p.stops[t.atStop.i]);
      w.player.ride.lastStop = { x: q.x, y: q.y, name: p.stopNames[t.atStop.i], i: t.atStop.i, pid: p.id };
    } else if (t.drive.doors === 'open') { t.drive.doors = 'closed'; w.events.push({ type: 'doors-close' }); }
  }
  if (t.drive.doors === 'open' && t.drive.doorT > k.doorsAuto) { t.drive.doors = 'closed'; w.events.push({ type: 'doors-close' }); }
  // ohne Fahrer: nach 30 s außer Sicht entfernen
  if (!driving) {
    t.leftT = (t.leftT ?? 0) + dt;
    const h = pointOn(p, t.s), cam = w.camera;
    if (t.leftT > 30 && (Math.abs(h.x - cam.x) > 1400 || Math.abs(h.y - cam.y) > 900)) w.playerTrain = null;
  }
}

export function leaveTrain(w) {
  const t = w.playerTrain, st = vehicleState(w, { playerTrain: true });
  if (!t || !st) return false;
  if (st.underground) {
    if (!t.atStop || t.drive.v > 0) { w.notice = { text: 'Nur am Bahnsteig', t: 1.5 }; return false; }
    const ex = stationExit(w, st.p, t.atStop.i);
    w.player.ride = null; w.player.x = ex.x; w.player.y = ex.y; w.player.lvl = 0;
  } else {
    const spot = alightSpot(w, st, 0);
    if (!spot) { w.notice = { text: 'Kein Platz zum Aussteigen', t: 1.5 }; return false; }
    w.player.ride = null; w.player.x = spot.x; w.player.y = spot.y;
    if (t.v > RIDE.hopOff) w.player.stun = RIDE.stun;
  }
  t.leftT = 0;
  w.events.push({ type: 'alight', hop: t.v > RIDE.hopOff, x: w.player.x, y: w.player.y });
  return true;
}

// Am Endhalt in die Gegenrichtung: Muster derselben Linie, dessen erster Halt ≤ 60 m vom eigenen Endhalt liegt
export function turnAround(w) {
  const t = w.playerTrain;
  if (!t || t.drive.v > 0) return false;
  const tr = w.city.transit, p = tr.patterns[t.pid], n = p.stops.length;
  if (t.s < p.stops[n - 1] - k_zone()) return false;
  const end = pointOn(p, p.stops[n - 1]);
  let best = null, bd = 600;
  for (const q of tr.patterns) {
    if (q.name !== p.name || q.id === p.id || q.mode !== p.mode) continue;
    const a = pointOn(q, q.stops[0]), d = Math.hypot(a.x - end.x, a.y - end.y);
    if (d < bd) { bd = d; best = q; }
  }
  if (!best) { w.notice = { text: 'Hier kann nicht gewendet werden', t: 2 }; return false; }
  Object.assign(t, { pid: best.id, s: best.stops[0], nextStop: 1, served: [0], atStop: { i: 0, dist: 0 } });
  t.drive.v = 0; t.v = 0;
  w.player.ride.line = best.name; w.player.ride.dest = best.stopNames[best.stopNames.length - 1];
  w.events.push({ type: 'turnaround', line: best.name });
  return true;
}
const k_zone = () => TRAIN_DRIVE.stopZone;

// Am Endhalt, Zug steht, Türen zu und der Halt ist bedient: dann heißt E/A „Wenden“ statt „Türen“
export function atTerminus(w) {
  const t = w.playerTrain;
  if (!t) return false;
  const p = w.city.transit.patterns[t.pid], last = p.stops.length - 1;
  return t.drive.v === 0 && t.drive.doors === 'closed' && Math.abs(t.s - p.stops[last]) <= TRAIN_DRIVE.stopZone && t.served.includes(last);
}
```

In `updatePlayerTrain` die Türbedingung `if (driving && input.action) {` ersetzen durch `if (driving && input.action && !atTerminus(w)) {`.

`world.js` Anbindung:
  - Import `import { takeTrain, updatePlayerTrain, leaveTrain, turnAround } from './playertrain.js';`
  - Eingaben (ersetzt die Zeilen aus Task 4):

```js
  if (input.ride && !p.dead && !p.inCar && p.ride?.kind !== 'driver') { if (p.ride) alightTransit(w); else boardTransit(w); }
  else if (input.enterExit && !p.dead) {
    if (p.ride?.kind === 'driver') leaveTrain(w);
    else if (!p.ride) {
      if (p.inCar) tryExit(w);
      else { const cab = transitNear(w, p.x, p.y, RIDE.cab + 10).find((h) => h.car === 0 && h.front <= RIDE.cab && h.mode !== 'bus'); if (!(cab && takeTrain(w, cab))) tryEnter(w); }
    }
  }
  if (input.action && p.ride?.kind === 'driver' && atTerminus(w)) turnAround(w); // vor updatePlayerTrain: sonst öffnete E/A die Türen erneut
```

    Import in world.js um `atTerminus` erweitern. Wenden und Türen teilen sich E/A; `atTerminus` entscheidet an beiden Stellen.
  - nach `updateTransit(w, dt);`: `updatePlayerTrain(w, input, dt);` und dann `if (p.ride) updateRide(w);` (Fahrer-Lage kommt über `vehicleState({playerTrain:true})` – `updateRide` funktioniert unverändert; `car: 0`).
  - `updateCamera`: für den Fahrer weiter nach vorn schauen: `if (p.ride) { const st = …speed; tx = p.x + Math.cos(p.angle) * (p.ride.speed ?? 0) * 0.6; … }`.
  - `alightTransit` (Task 4) nur für `kind === 'passenger'`; Fahrgast im Spielerzug gibt es nicht.

`transitlive.js`:
  - In `updateTransit` beim `stepTransit`-Aufruf den `blocked`-Rückruf erweitern: Fahrplan-Fahrzeuge desselben Musters, deren Spitze hinter dem Spielerzug-Heck liegt und näher als 600 px kommt, warten:

```js
const pt = w.playerTrain;
const behindPlayer = (p, v) => pt && pt.pid === p.id && positionAt(p, v.tau).s < pt.s && pt.s - trainLenOf(p.mode) - positionAt(p, v.tau).s < 600;
stepTransit(st, tr, w.camera, w.clock, w.day, dt, w.time, (p, v) => behindPlayer(p, v) || (p.mode === 'tram' && tramBlocked(w, p, v) && (v.blockedT = (v.blockedT ?? 0) + dt) >= 0));
```

    mit `const trainLenOf = (m) => TRAIN[m].cars * TRAIN[m].carL + (TRAIN[m].cars - 1) * TRAIN[m].gap;` (TRAIN importieren).
  - Nach der Straßenbahn-Schleife für `railObs`: Spielerzug ergänzen, wenn Straßenbahn oder oberirdisch:

```js
if (pt) {
  const p = tr.patterns[pt.pid];
  for (const c of trainCars(p, pt.s)) if (p.mode === 'tram' || !undergroundAtS(w.city, p, pt.s)) w.railObs.push({ x: c.x, y: c.y, angle: c.angle, hw: c.L / 2, hh: c.W / 2, vx: Math.cos(c.angle) * pt.v, vy: Math.sin(c.angle) * pt.v, tram: true });
}
```

  - Straßenbahn als Spielerzug hält vor Hindernissen auf dem Gleis. In `transitlive.js` die Prüfung aus `tramBlocked` herausziehen und `tramBlocked` darauf umstellen:

```js
// Steht an (x, y) etwas im Weg einer Straßenbahn? (Spieler zu Fuß, Autos, fahrende Räder, Passanten)
export function obstacleAt(w, x, y) {
  const hit = (ox, oy, r) => Math.hypot(ox - x, oy - y) < r;
  if (!w.player.inCar && !w.player.ride && !w.player.dead && hit(w.player.x, w.player.y, 22)) return true;
  for (const c of w.cars) if (hit(c.x, c.y, 24 + c.hw * 0.4)) return true;
  for (const b of w.bikes ?? []) if (b.state === 'ride' && hit(b.x, b.y, 18)) return true;
  for (const ped of w.peds) if (ped.state !== 'dead' && ped.state !== 'hang' && hit(ped.x, ped.y, 16)) return true;
  return false;
}
// in tramBlocked die Schleife ersetzen durch:
for (const d of [20, 45, 75]) { const q = pointOn(p, head + d); if (obstacleAt(w, q.x, q.y)) return true; }
```

    In `playertrain.js`:

```js
import { obstacleAt } from './transitlive.js';
// freier Weg der eigenen Straßenbahn bis zum ersten Hindernis auf dem Gleis (px), Infinity ohne Hindernis
export function tramFree(w, p, s) {
  for (const d of [20, 45, 75, 110, 150, 200]) { const q = pointOn(p, s + d); if (obstacleAt(w, q.x, q.y)) return Math.max(0, d - 15); }
  return Infinity;
}
// in updatePlayerTrain nach `const limit = …`:
//   const limit = Math.min(endFree, aheadFree, p.mode === 'tram' ? tramFree(w, p, t.s) : Infinity);
```

    `const limit` in `updatePlayerTrain` entsprechend ändern. Test in `tests/playertrain.test.js` ergänzen:

```js
test('Eigene Straßenbahn hält vor einem Auto auf dem Gleis', async () => {
  const { createCar } = await import('../web/src/car.js');
  const p = pat('M10', 'tram'), w = atFrontOf(p, 3);
  press(w, { enterExit: true });
  const t = w.playerTrain, q = pointOn(p, t.s + 900);
  const car = createCar({ x: q.x, y: q.y, angle: q.angle }); car.driver = null; w.cars.push(car);
  for (let i = 0; i < 60 * 30; i++) press(w, { throttle: 1 });
  const head = pointOn(p, t.s);
  assert.ok(Math.hypot(head.x - car.x, head.y - car.y) > 30, `Spitze ${Math.hypot(head.x - car.x, head.y - car.y).toFixed(0)} px vor dem Auto`);
  assert.equal(t.v, 0);
});
```

- [ ] **Step 4: Run** `node --test tests/playertrain.test.js tests/ride.test.js tests/transit.test.js tests/traffic.test.js` → PASS. Falls der Sanft-Halt-Test den Halt verfehlt, zuerst die Testfahrerlogik prüfen (Bremskurve), nicht die Toleranz aufweiten.

- [ ] **Step 5: Mutationsproben**: `limit`-Übergabe an `stepDrive` entfernen → Test 3 scheitert; `behindPlayer` im Rückruf entfernen → Test 3 (hinten) scheitert; Türen ohne `t.atStop`-Bedingung → Test 2 scheitert; `v.gone = true` in `takeTrain` entfernen → Test 1 scheitert; Wende-Radius 600 → 10 → Test 4 (falls Gegenmuster existiert) scheitert.

- [ ] **Step 6: Commit** `git add web/src/playertrain.js web/src/world.js web/src/transitlive.js tests/playertrain.test.js && git commit -m "Drive trams, S-Bahn and U-Bahn: take over at the cab, doors, stops, tips, turnaround, trains ahead"`

---

### Task 7: Darstellung (Spielerzug, Tunnelansicht, Leisten)

**Files:**
- Create: `web/src/tunnelview.js`
- Modify: `web/src/render.js` (collectMovers: Spielerzug zeichnen, Spieler während Fahrt nicht zeichnen; nach Welt/Lightmap Tunnel-Durchgang)
- Modify: `web/src/transitlive.js transitVisible` (unter Tage: Züge liefern, wenn `underTrains` gewünscht)
- Modify: `web/src/world.js` (`w.underground` weich nachführen)
- Modify: `web/src/hud.js` (`drawRideBar(world)`, im `drawGameplay` aufrufen)
- Test: `tests/render.test.js`, `tests/hud-layout.test.js` (Ergänzungen)

**Interfaces:**
- Consumes: `vehicleState`, `undergroundAtS`, `trainCars`, `drawTrainCar` (railart.js, Signatur `drawTrainCar(ctx, car, mode, sun, lit, t)`), `w.player.ride`, `w.playerTrain`.
- Produces:
  - `w.underground` (0…1), fortgeschrieben in `updateWorld`: Ziel 1, wenn `p.ride?.underground`, sonst 0; `w.underground += (target - w.underground) * Math.min(1, dt / 0.6)`.
  - `drawTunnels(ctx, world, v, t, fade)` in tunnelview.js (zeichnet Abdunkelung, Röhren, Bahnsteige, Züge unter Tage; gibt `{ tubes, platforms, trains }` zurück)
  - `renderer.stats.tunnel` (Rückgabe oben)
  - `hud.drawRideBar(world)` → `hud.counts.rideBar = true`

- [ ] **Step 1: Failing tests** – `tests/render.test.js` anhängen (dort gibt es bereits Canvas-Stubs und Welt-Aufbau; die vorhandenen Hilfsfunktionen des Files verwenden, z. B. `recordingCtx`/`draw`-Aufruf wie in den bestehenden Tests – vorher `grep -n "^function\|^const .*= (" tests/render.test.js` lesen):

```js
test('Tunnelansicht: als Fahrgast in der U8 unter Tage wird die Röhre mit Bahnsteigen und dem eigenen Zug gezeichnet', async () => {
  const { drawTunnels } = await import('../web/src/tunnelview.js');
  const tr = realTransit(), u8 = tr.patterns.filter((p) => p.name === 'U8').sort((a, b) => b.stops.length - a.stops.length)[0];
  const i = u8.stopNames.findIndex((n) => n.startsWith('U Kottbusser Tor')), at = pointOn(u8, u8.stops[i]);
  const w = createWorld({ city }); w.mission.state = 'idle'; w.clock = 12 * 60; w.day = 1;
  w.camera.x = at.x; w.camera.y = at.y; w.player.x = at.x; w.player.y = at.y; resetPopulation(w);
  for (let k = 0; k < 30; k++) updateWorld(w, idle(), 1 / 60);
  let tau = 0; while (!(positionAt(u8, tau).stop === i && positionAt(u8, tau).dwelling)) tau += 0.5;
  w.transit.tracked.get(u8.id).veh.push({ tau, delay: 0, key: 'mine' });
  w.player.ride = { kind: 'passenger', ref: { pid: u8.id, key: 'mine' }, mode: 'ubahn', car: 2, lastStop: { x: at.x, y: at.y, name: 'x', i, pid: u8.id }, since: 0, line: 'U8', dest: 'x' };
  for (let k = 0; k < 90; k++) updateWorld(w, idle(), 1 / 60);
  assert.ok(w.underground > 0.8, `Überblendung ${w.underground}`);
  const ctx = stubCtx(); // vorhandener Canvas-Stub aus render.test.js
  const r = drawTunnels(ctx, w, { x: at.x - 800, y: at.y - 450, w: 1600, h: 900 }, 0, w.underground);
  assert.ok(r.tubes >= 1 && r.platforms >= 1 && r.trains >= 1, JSON.stringify(r));
  assert.equal(drawTunnels(ctx, w, { x: at.x - 800, y: at.y - 450, w: 1600, h: 900 }, 0, 0).tubes, 0, 'oben: nichts');
});
```

  `tests/hud-layout.test.js` anhängen: `drawRideBar` für Fahrgast (Linie, „nächster Halt“) und Fahrer (Tacho-Text in km/h, „Türen“) zeichnet ohne `NaN` und innerhalb der Bildgrenzen (vorhandene Messhelfer der Datei nutzen).

- [ ] **Step 2: Run** `node --test tests/render.test.js tests/hud-layout.test.js` → FAIL.

- [ ] **Step 3: Implement**
  - `world.js updateWorld` nach `if (p.ride) updateRide(w);`:

```js
  const ugTarget = p.ride?.underground ? 1 : 0;
  w.underground = (w.underground ?? 0) + (ugTarget - (w.underground ?? 0)) * Math.min(1, dt / 0.6);
  if (Math.abs(w.underground - ugTarget) < 0.01) w.underground = ugTarget;
```

  - `web/src/tunnelview.js`:

```js
// Tunnelansicht: unter Tage wird die Stadt abgedunkelt; U-/S-Bahn-Linien, die im Bild unter Tage liegen, erscheinen als
// Betonröhre mit Gleisen und Lichtern, Bahnhöfe als helle Bahnsteige mit Namen, Züge in der Röhre werden gezeichnet.
// fade = world.underground (0…1). Nur Darstellung.
import { positionAt, pointOn, trainCars, TRAIN } from './transit.js';
import { undergroundAtS } from './tunnel.js';
import { drawTrainCar } from './railart.js';

const TUBE = '#3a3d44', LIGHT = 'rgba(255,236,190,0.85)';

export function drawTunnels(ctx, world, v, t, fade) {
  const out = { tubes: 0, platforms: 0, trains: 0 };
  if (fade <= 0.01) return out;
  const tr = world.city.transit, st = world.transit;
  if (!tr || !st) return out;
  ctx.save();
  ctx.globalAlpha = 0.72 * fade; ctx.fillStyle = '#07080c'; ctx.fillRect(v.x, v.y, v.w, v.h);
  ctx.globalAlpha = fade;
  const own = world.player.ride ? (world.player.ride.ref.playerTrain ? world.playerTrain?.pid : world.player.ride.ref.pid) : null;
  const inView = (x, y, pad = 300) => x > v.x - pad && x < v.x + v.w + pad && y > v.y - pad && y < v.y + v.h + pad;
  const drawn = new Set();
  for (const [pid] of st.tracked) {
    const p = tr.patterns[pid];
    if (p.mode !== 'ubahn' && p.mode !== 'sbahn') continue;
    if (drawn.has(p.shape)) continue;
    // Abschnitte unter Tage im Bild, in 60-px-Schritten
    const W = TRAIN[p.mode].W + 20, pts = [];
    const flush = () => {
      if (pts.length < 4) { pts.length = 0; return; }
      ctx.lineCap = 'round'; ctx.lineJoin = 'round';
      ctx.strokeStyle = TUBE; ctx.lineWidth = W; ctx.globalAlpha = fade * (pid === own ? 1 : 0.7);
      ctx.beginPath(); ctx.moveTo(pts[0], pts[1]); for (let i = 2; i < pts.length; i += 2) ctx.lineTo(pts[i], pts[i + 1]); ctx.stroke();
      ctx.strokeStyle = '#1c1e22'; ctx.lineWidth = 6; ctx.stroke();
      out.tubes++; pts.length = 0;
    };
    for (let s = 0; s <= p.shape.len; s += 60) {
      const q = pointOn(p, s);
      if (inView(q.x, q.y) && undergroundAtS(world.city, p, s)) {
        pts.push(q.x, q.y);
        if (Math.round(s / 60) % 4 === 0) { ctx.fillStyle = LIGHT; ctx.fillRect(q.x - 1.5, q.y - 1.5, 3, 3); }
      } else flush();
    }
    flush();
    drawn.add(p.shape);
    // Bahnsteige unter Tage
    p.stops.forEach((sv, i) => {
      const q = pointOn(p, sv);
      if (!inView(q.x, q.y) || !undergroundAtS(world.city, p, sv)) return;
      ctx.save(); ctx.translate(q.x, q.y); ctx.rotate(q.angle);
      ctx.fillStyle = 'rgba(214,214,206,0.95)';
      for (const side of [-1, 1]) ctx.fillRect(-200, side * (W / 2 + 2) - (side < 0 ? 40 : 0), 400, 40);
      ctx.fillStyle = p.color ? `#${p.color.replace('#', '')}` : '#ffd33d';
      ctx.fillRect(-200, -W / 2 - 44, 400, 4);
      ctx.restore();
      ctx.fillStyle = '#fff'; ctx.font = 'bold 22px system-ui, sans-serif'; ctx.textAlign = 'center';
      ctx.fillText(p.stopNames[i].replace(/^[SU]\s+/, ''), q.x, q.y - W / 2 - 54);
      out.platforms++;
    });
  }
  // Züge in der Röhre (Fahrplan und Spielerzug)
  const drawTrain = (p, s, lit) => { for (const c of trainCars(p, s)) if (inView(c.x, c.y) && undergroundAtS(world.city, p, s)) { drawTrainCar(ctx, c, p.mode, null, lit, t); out.trains++; } };
  for (const [pid, s] of st.tracked) {
    const p = tr.patterns[pid];
    if (p.mode !== 'ubahn' && p.mode !== 'sbahn') continue;
    for (const veh of s.veh) if (!veh.gone) { const pos = positionAt(p, veh.tau); drawTrain(p, pos.s, !pos.dwelling); }
  }
  if (world.playerTrain) { const p = tr.patterns[world.playerTrain.pid]; if (p.mode !== 'tram') drawTrain(p, world.playerTrain.s, true); }
  ctx.restore();
  return out;
}
```

    Vorher `grep -n "export function drawTrainCar" web/src/railart.js` und die Farbe `p.color` prüfen (Format in transit.json `lines: [name, mode, color]`).
  - `render.js`:
    - `collectMovers`: Spieler nur zeichnen, wenn `!pl.inCar && !pl.ride`.
    - Spielerzug oberirdisch: nach den Fahrplan-Zügen

```js
    const ptn = world.playerTrain;
    if (ptn) {
      const p = city.transit.patterns[ptn.pid];
      for (const c of trainCars(p, ptn.s)) {
        if (p.mode !== 'tram' && !railAt(city, c.x, c.y)) continue; // unter Tage: Tunnel-Durchgang
        const o = { x: c.x, y: c.y, lvl: p.mode === 'tram' ? (upper.length ? trackLevel(c.x, c.y, upper, ground) : 0) : railAt(city, c.x, c.y)?.lvl ?? 0 };
        add(o, c.y + 4, c.y + 4, () => drawTrainCar(ctx, c, p.mode, L.sun, true, t), c.L / 2, c.W / 2, c.angle, p.mode === 'tram' ? {} : { late: true, skipCover: true });
      }
    }
```

    - Tunnel-Durchgang: in `draw()` nach dem Lightmap/`drawCovered`-Block (render.js ~843), in Weltkoordinaten (dieselbe Transformation wie die Welt), `this.stats.tunnel = drawTunnels(ctx, world, v, t, world.underground ?? 0);`. Unter Tage den Tag-/Nacht-Lichtdurchgang nicht doppelt abdunkeln: bei `world.underground > 0.5` `drawLightmap` überspringen.
  - `hud.js`: `drawRideBar(world)` oben mittig (unter dem Missionsfeld frei lassen: Breite 460, `y = m.y`, links vom Missionspanel, falls dieses sichtbar ist: `x = vw/2 - 230`):

```js
  drawRideBar(world) {
    const r = world.player.ride;
    if (!r) return;
    const c = this.ctx, vw = this.vw, w = 460, x = vw / 2 - w / 2, y = this.m.y, h = r.kind === 'driver' ? 92 : 64;
    const st = vehicleState(world, r.ref);
    this.panel(x, y, w, h);
    const col = st?.p.color ? `#${String(st.p.color).replace('#', '')}` : YELLOW;
    c.fillStyle = col; rr(c, x + 14, y + 12, 58, 26, 6); c.fill();
    this.text(r.line, x + 43, y + 31, { size: 16, weight: 900, align: 'center', color: '#fff', shadow: false });
    this.text(`→ ${r.dest.replace(/^[SU]\s+/, '')}`, x + 84, y + 31, { size: 16, weight: 700 });
    const next = st ? st.p.stopNames[Math.min(st.p.stopNames.length - 1, st.stop)] : '';
    const line2 = st?.dwelling ? `Hält: ${next}` : `Nächster Halt: ${next}`;
    this.text(line2.replace(/\s\(.*\)$/, ''), x + 14, y + 56, { size: 15, weight: 600, color: '#ddd' });
    if (r.kind === 'passenger') this.text(st?.underground && !st.dwelling ? 'Aussteigen nur am Bahnsteig' : 'G: aussteigen', x + w - 14, y + 56, { size: 13, weight: 700, align: 'right', color: '#aaa' });
    if (r.kind === 'driver' && world.playerTrain) {
      const t = world.playerTrain, kmh = Math.round(t.v * 0.36);
      this.text(`${kmh} km/h`, x + 14, y + 82, { size: 18, weight: 800, color: t.blocked ? '#ff8080' : '#fff' });
      const doors = t.drive.doors === 'open' ? 'Türen offen – E/A schließen' : t.atStop ? 'E/A: Türen öffnen' : t.blocked ? 'Zug voraus' : 'W/RT Gas · S/LT Bremse · Leertaste/B Notbremse';
      this.text(doors, x + w - 14, y + 82, { size: 13, weight: 700, align: 'right', color: t.atStop ? YELLOW : '#aaa' });
    }
    this.counts = this.counts ?? {}; this.counts.rideBar = true;
  }
```

    `import { vehicleState } from './ride.js';` in hud.js; in `drawGameplay` aufrufen (`this.drawRideBar(world);` nach dem Ort-/Geld-Block). Die Minikarte bleibt; bei `world.underground > 0.5` die Minikarte mit 0,6 Deckkraft zeichnen.

- [ ] **Step 4: Run** `node --test tests/render.test.js tests/hud-layout.test.js tests/transit.test.js` → PASS.

- [ ] **Step 5: Browser (Testserver :8091)**: `PORT=8091 npm start &`; im Spiel `tp kottbusser tor`, an die U1 (Hochbahn) stellen, mit G mitfahren, bei Bewegung Kamera/Leiste prüfen; U8 im Tunnel: `tp hermannplatz`, ein Zug im Bahnhof – vom Bahnsteig aus nicht erreichbar (erwartet: nur oberirdisch einsteigen) → stattdessen U7/U8 über eine oberirdische Stelle erreichen oder für die Sichtprüfung per Konsole `world.player.ride` setzen (Playwright `evaluate`); Tunnelansicht screenshotten, Bildrate mit `fps an` (Median ≤ 16 ms bei 1280×720). M10 fahren: Warschauer Str. → Haltestelle anfahren, Türen, Trinkgeld. Screenshots nach `.playwright-mcp/`.

- [ ] **Step 6: Mutationsproben**: in `drawTunnels` `if (fade <= 0.01) return out;` entfernen → Test „oben: nichts“ scheitert; Spieler-Ausblendung in collectMovers entfernen → ergänzenden Test in render.test.js schreiben, der beim Fahrgast `drawPerson`-Aufrufe für den Spieler zählt (0 erwartet) und scheitert.

- [ ] **Step 7: Commit** `git add web/src/tunnelview.js web/src/render.js web/src/transitlive.js web/src/world.js web/src/hud.js tests/render.test.js tests/hud-layout.test.js && git commit -m "Tunnel view, player train drawing, ride/driver HUD bar"`

---

### Task 8: Statistik, Töne, Doku, Release

**Files:**
- Modify: `web/src/stats.js` (STAT_SECTIONS „Unterwegs“ + „Nahverkehr“, `trackStep`)
- Modify: `web/src/audio.js` (Ereignisse → vorhandene Klänge: `board`/`alight` → `door`, `hop-on`/`hop-off` → `bump`, `doors-open`/`doors-close` → Türgong (zwei kurze Töne, wie `ui-move` höher/tiefer), `tip` → `money`/`ui`, `train-blocked` → `tram-bell`)
- Modify: `README.md` (Steuerung, Abschnitt „Nahverkehr“, Stand und Prüfumfang + Testzahl), `docs/TECHNIK.md` (Abschnitt Nahverkehr mitfahren/fahren), `CLAUDE.md` (Architektur: ride.js, playertrain.js, trainphysics.js, tunnel.js, tunnelview.js), `CHANGELOG.md` `## [0.29.0] – 2026-09-28`, Versionen (package.json, web/src/version.js, xbox/GtaBerlin/Package.appxmanifest `0.29.0.0`)
- Test: `tests/stats.test.js`

**Interfaces:**
- Consumes: Ereignisse `board {hop}`, `alight {hop}`, `doors-open`, `tip {amount}`, `train-take`; `w.player.ride`.
- Produces: Zähler `rides`, `kmTransit`, `kmTrainDriven`, `stopsServed`, `tipsEarned`, `hopsOn`, `hopsOff`, `trainsTaken`.

- [ ] **Step 1: Failing test** (`tests/stats.test.js` anhängen):

```js
test('Nahverkehr: Mitfahrten, Strecke im Nahverkehr und als Zugführer, bediente Halte, Trinkgeld, Auf-/Abspringen', () => {
  const w = fakeWorld(), tr = createTracker(), S = sets();
  trackStep(S, tr, w, [], 0);
  trackStep(S, tr, w, [{ type: 'board', hop: true }, { type: 'train-take' }], 0);
  w.player.ride = { kind: 'passenger' };
  for (let i = 0; i < 100; i++) { w.player.x += 10; trackStep(S, tr, w, [], 1 / 60); }
  w.player.ride = { kind: 'driver' };
  for (let i = 0; i < 100; i++) { w.player.x += 20; trackStep(S, tr, w, [], 1 / 60); }
  trackStep(S, tr, w, [{ type: 'doors-open' }, { type: 'tip', amount: 7 }, { type: 'alight', hop: true }], 0);
  const s = S[0];
  assert.equal(s.rides, 1); assert.equal(s.hopsOn, 1); assert.equal(s.hopsOff, 1); assert.equal(s.trainsTaken, 1);
  assert.ok(Math.abs(s.kmTransit - 0.1) < 1e-9 && Math.abs(s.kmTrainDriven - 0.2) < 1e-9);
  assert.equal(s.kmFoot, 0, 'Fahrt zählt nicht als Fußweg');
  assert.equal(s.stopsServed, 1); assert.equal(s.tipsEarned, 7);
  assert.equal(s.moneyEarned, 0, 'Trinkgeld getrennt (world.money unverändert in der Attrappe)');
});
```

- [ ] **Step 2: Run** `node --test tests/stats.test.js` → FAIL.

- [ ] **Step 3: Implement** in `stats.js`:
  - `STAT_SECTIONS` neuer Abschnitt nach „Verkehr“: `['Nahverkehr', [['rides', 'Mitfahrten', 'n'], ['kmTransit', 'Strecke als Fahrgast', 'km'], ['trainsTaken', 'Bahnen geführt', 'n'], ['kmTrainDriven', 'Strecke als Zugführer', 'km'], ['stopsServed', 'Halte bedient', 'n'], ['tipsEarned', 'Trinkgeld', 'eur'], ['hopsOn', 'aufgesprungen', 'n'], ['hopsOff', 'abgesprungen', 'n']]]`
  - In `trackStep` Strecke: `const riding = p.ride; add(sets, car ? 'kmCar' : riding ? (riding.kind === 'driver' ? 'kmTrainDriven' : 'kmTransit') : 'kmFoot', d / PX_PER_KM);`
  - Ereignisse: `case 'board': add(sets, 'rides'); if (e.hop) add(sets, 'hopsOn'); break; case 'alight': if (e.hop) add(sets, 'hopsOff'); break; case 'train-take': add(sets, 'trainsTaken'); break; case 'doors-open': add(sets, 'stopsServed'); break; case 'tip': add(sets, 'tipsEarned', e.amount ?? 0); break;`
  - Trinkgeld soll nicht doppelt als „Geld verdient“ zählen: in `trackStep` vor der Geldprüfung `const tips = events.reduce((a, e) => a + (e.type === 'tip' ? e.amount ?? 0 : 0), 0);` und `if (tr.money !== null && world.money - tips > tr.money) add(sets, 'moneyEarned', world.money - tips - tr.money);`.
  - Statistikseite (hud.js `drawStats`) hat zwei Spalten mit je zwei Abschnitten; neuer Abschnitt → Spalte 1: Unterwegs + Verkehr, Spalte 2: Kampf + Aufträge + Nahverkehr würde den 720er-Rahmen sprengen. Lösung: `cols = [[S[0], S[1]], [S[2], S[4]], [S[3]]]` → drei Spalten mit `colW = 360`; Test `statsBottom <= 720` in stats.test.js muss weiter grün sein. Waffen-Tabelle bleibt darunter.
- [ ] **Step 4: Run** `npm test` (volle Suite) → alles grün. Testzahl notieren.
- [ ] **Step 5: Doku + Version**: README Steuerungstabelle (Zeilen „Mitfahren“, „Bahn führen“, „Türen/Wenden“), neuer Abschnitt „Nahverkehr“, „Stand und Prüfumfang“ mit den neuen Tests und der Testzahl; TECHNIK-Abschnitt (Fahrzeug-Referenzen, Spielerzug verlässt den Fahrplan, Zwangsbremsung, Tunnel aus Linienweg + fehlendem Gleis, Grenzen: keine Signale, keine Bahnhofs-Innenräume); CLAUDE.md-Architekturpunkt; CHANGELOG `0.29.0`; Versionen in drei Dateien; `node --test tests/version.test.js`.
- [ ] **Step 6: Browser-Schlussprüfung** wie Task 7 Step 5, zusätzlich Statistikseite (Nahverkehr-Abschnitt sichtbar, passt in 1280×720).
- [ ] **Step 7: Commit, Tag, Push**

```bash
git add -A web/src tests README.md CHANGELOG.md docs/TECHNIK.md CLAUDE.md package.json xbox/GtaBerlin/Package.appxmanifest
git commit -m "Public transport: ride as passenger (G / D-pad down), drive trams/S-Bahn/U-Bahn, tunnel view, stats (0.29.0)"
git tag -a v0.29.0 -m "0.29.0" && git push --follow-tags
```
