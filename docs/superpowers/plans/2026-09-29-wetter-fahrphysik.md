# Wetterabhängige Fahrphysik (0.30.0) – Umsetzungsplan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Nässe, Schnee, Glätte, Pfützen und Sturmböen wirken auf Autos, KI-Verkehr und den eigenen Zug; Temperatur und
Warnschild im HUD.

**Architecture:** Neues reines Modul `web/src/traction.js` bewertet eine Stelle (`roadCondition`) und macht daraus
Haftungsfaktoren (`tractionOf`), Böenschub (`gustPush`) und Schienenhaftung (`adhesionOf`). `weather.js` bekommt die
reine Temperaturkurve `temperatureAt` und `stepIce`. `world.js` schreibt je Schritt `car.traction` (ersetzt
`car.wet`/`car.snow`), Aquaplaning und Böen; `car.js`, `traffic.js`, `trainphysics.js` lesen nur Faktoren.

**Tech Stack:** Plain ES modules, `node:test`, keine Abhängigkeiten.

**Spec:** `docs/superpowers/specs/2026-09-29-wetter-fahrphysik-design.md`

## Global Constraints

- Null Abhängigkeiten; Simulation DOM-frei und deterministisch – Aquaplaning-Richtung und Böen nie aus `world.rng`, nur aus Hashes und `world.time`.
- Einheiten: 10 px = 1 m; km/h = px/s × 0,36; `CLOCK.minutesPerSecond` = 1 (1 Spielminute je Sekunde).
- Stufe „spürbar, aber fair“; Faktoren (1 = trocken): nass brake 0,77 accel 0,85 lat 0,82 steer 0,95 · Schnee 0,5/0,55/0,55/0,8 · Glätte 0,33/0,4/0,35/0,65; multipliziert, Untergrenze = Glättewert je Spalte.
- Aquaplaning: Pfütze aus `edgePuddles`, Nässe > 0,3, Vorwärtstempo > 70 km/h (194 px/s), 0,35 s, lat ×0,15, steer ×0,2, brake ×0,3, Gierimpuls ≤ 0,6 rad/s aus Pfützen-Hash.
- Böen: `storm × max(0, gustAt − 0.9)`, Brücke ×1,6, ÷ `hw·hh/(21·10)`, nicht bei < 5 px/s oder geparkten Autos; Maßstab Pkw 50 km/h: 0,5–1 m Versatz je voller Böe, Brücke ≈ 1,5 m.
- Temperatur: Winter −6…+3 °C, wechselhaft +4…+14, normal +8…+22, ±2 °C je Tag, Tiefst 5:00, Höchst 15:00, erzwungener Schnee ≤ +1 °C.
- Glätte `w.ice`: wächst bei `wet > 0.1` und ≤ 0 °C (voll in ≈ 10 Spielminuten), taut über 0 °C (≈ 20 Spielminuten); Brücke ×1,5; überdacht 0.
- Zug: Haftung oberirdisch nass 0,75, Frost 0,6 (Untergrenze 0,6), Tunnel 1; skaliert Bremse, Notbremse, Zugkraft.
- Trockenes Wetter: alle Faktoren exakt 1 – bestehende Fahr-, Verkehrs- und Zugtests unverändert grün.
- Versionierung: 0.30.0 in `package.json`, `web/src/version.js`, `xbox/GtaBerlin/Package.appxmanifest` (`0.30.0.0`), datierter CHANGELOG, annotierter Tag, `git push --follow-tags`; README „Stand und Prüfumfang“ + Testzahl, CLAUDE.md Architektur.
- Testserver für Browserprüfungen: `PORT=8091 npm start` (8080 gehört einem anderen Projekt), nie `pkill`; beenden per PID.
- Jede neue Schutzprüfung bekommt eine Mutationsprobe (vorher committen, Fehler einbauen, Test muss scheitern, mit `git checkout -- <datei>` zurück).

## Review Focus

1. **Geparkte/schlafende Autos bei Sturm** → bewegen sich nie (kein Böenschub, weckt sie nicht auf). Test in Task 3.
2. **Aquaplaning bei 30 fps und in derselben Pfütze** → gleiches Verhalten wie bei 60 fps, ein Ereignis je Pfütze, kein Dauer-Neuauslösen. Test in Task 3.
3. **Überdachte Stellen** (Durchfahrt, unter einer Brücke) bei Nässe/Glätte → trocken. Test in Task 2.
4. **Spielstand mit Glätte und alter Spielstand ohne `ice`** → lädt, alter ohne Glätte. Test in Task 1.
5. **KI-Schlange an Rot auf Glätte** → hält mit Abstand, kein Auffahren; Engstellen staut nicht dauerhaft. Test in Task 4.

---

## Dateistruktur

| Datei | Rolle |
|---|---|
| `web/src/weather.js` (ändern) | `temperatureAt`, `ICE`, `stepIce` |
| `web/src/traction.js` (neu) | `TRACTION`, `DRY`, `AQUA`, `GUST`, `puddleAt`, `roadCondition`, `tractionOf`, `gustPush`, `adhesionOf`, `roadWarning` |
| `web/src/world.js` (ändern) | `w.ice`, `w.temp`, `w.forceTemp`; je Auto `applyWeather` vor `stepCar` |
| `web/src/car.js` (ändern) | `stepCar` wendet `car.traction` und `car.aqua` an, `car.spin` |
| `web/src/traffic.js` (ändern) | KI liest `car.traction` |
| `web/src/trainphysics.js`, `web/src/playertrain.js` (ändern) | `input.adhesion`, `trainAdhesion` |
| `web/src/save.js`, `web/src/console.js` (ändern) | `ice`, `glaette`, `temp` |
| `web/src/hud.js`, `web/src/render.js`, `web/src/audio.js`, `web/src/stats.js` (ändern) | Temperatur, Warnschild, Spritzwasser, Platschen, Zähler |

---

### Task 1: Temperatur, Glätte, Spielstand, Konsole

**Files:**
- Modify: `web/src/weather.js` (nach `stepWet`), `web/src/world.js:39` (Weltobjekt) und `:590–593` (Wetterschritt), `web/src/save.js:17,35–36,56–57`, `web/src/console.js:104–107`
- Test: `tests/weather.test.js`, `tests/unwetter.test.js`, `tests/console.test.js`

**Interfaces:**
- Abweichung von der Spec (bewusst): `w.ice` wird ohne `w.rhythm`-Sperre fortgeschrieben. Ohne Tagesrhythmus ist das Wetter fest „klar“, Nässe entsteht dort nur per Konsole – dann soll Glätte aber genau so entstehen können (Prüfen, Tests). Im Ledger vermerken.
- Produces: `temperatureAt(seed, dayCount, minutes, force = null) → °C`, `ICE = { rise: 1/10, melt: 1/20 }`, `stepIce(ice, wet, tempC, dt) → 0…1`; Weltfelder `w.ice` (0…1, gespeichert), `w.temp` (°C, je Schritt), `w.forceTemp` (null oder °C); Konsole `glaette 0–1`, `temp`, `temp -5`, `temp auto`.

- [ ] **Step 1: Failing tests** – an `tests/weather.test.js` anhängen (Import-Zeile um `temperatureAt, stepIce, ICE, dayType` ergänzen, falls nicht vorhanden):

```js
test('Temperatur: Spanne je Tagestyp, kältester Punkt 5 Uhr, wärmster 15 Uhr, stetig über Mitternacht', async () => {
  const { temperatureAt, dayType } = await import('../web/src/weather.js');
  const seed = 1989, day = (type) => { let d = 0; while (dayType(seed, d) !== type) d++; return d; };
  const w = day('winter'), n = day('normal');
  assert.ok(temperatureAt(seed, w, 300) >= -8 && temperatureAt(seed, w, 300) <= -4, `Winter früh ${temperatureAt(seed, w, 300)}`);
  assert.ok(temperatureAt(seed, w, 900) >= 1 && temperatureAt(seed, w, 900) <= 5, `Winter mittags ${temperatureAt(seed, w, 900)}`);
  assert.ok(temperatureAt(seed, n, 900) >= 20 && temperatureAt(seed, n, 900) <= 24);
  for (const d of [w, n]) {
    let lo = Infinity, hi = -Infinity, loAt = 0, hiAt = 0;
    for (let m = 0; m < 1440; m += 10) { const t = temperatureAt(seed, d, m); if (t < lo) { lo = t; loAt = m; } if (t > hi) { hi = t; hiAt = m; } }
    assert.ok(Math.abs(hiAt - 900) <= 10, `Höchstwert ${hiAt}`);
    assert.ok(Math.abs(loAt - 300) <= 10 || loAt === 0 || loAt >= 1430, `Tiefstwert ${loAt}`); // Folgetag kann kälter sein
  }
  let jump = 0;
  for (let d = 0; d < 5; d++) for (let m = 0; m < 1440; m++) {
    const a = temperatureAt(seed, d, m), b = m < 1439 ? temperatureAt(seed, d, m + 1) : temperatureAt(seed, d + 1, 0);
    jump = Math.max(jump, Math.abs(b - a));
  }
  assert.ok(jump < 0.1, `Sprung ${jump.toFixed(3)} °C je Minute`);
  assert.equal(temperatureAt(seed, n, 900), temperatureAt(seed, n, 900), 'deterministisch');
  assert.ok(temperatureAt(seed, n, 900, 'snow') <= 1 && temperatureAt(seed, n, 900, 'heavysnow') <= 1, 'erzwungener Schnee ≤ +1 °C');
});

test('Glätte: wächst nur bei Nässe und Frost, taut darüber, bleibt trocken stehen', async () => {
  const { stepIce, ICE } = await import('../web/src/weather.js');
  assert.ok(Math.abs(stepIce(0, 1, -2, 1) - ICE.rise) < 1e-9);
  assert.equal(stepIce(0, 0.05, -2, 1), 0, 'trocken: keine Glätte');
  assert.ok(Math.abs(stepIce(0.5, 1, 1, 1) - (0.5 - ICE.melt)) < 1e-9, 'taut über 0 °C');
  assert.equal(stepIce(0.5, 0, -3, 1), 0.5, 'trocken und kalt: bleibt');
  let ice = 0; for (let i = 0; i < 12 * 60; i++) ice = stepIce(ice, 1, -1, 1 / 60);
  assert.equal(ice, 1, 'voll nach ≈ 10 Spielminuten (1 Spielminute je Sekunde)');
});

test('Welt: Glätte aus Temperatur und Nässe; erzwungene Temperatur', async () => {
  const { createWorld, updateWorld } = await import('../web/src/world.js');
  const w = createWorld({ city, cars: 0, pedestrians: 0 }); w.mission.state = 'idle';
  w.wet = 1; w.forceWeather = 'rain'; w.forceTemp = -3;
  for (let i = 0; i < 12 * 60; i++) updateWorld(w, idle(), 1 / 60);
  assert.equal(w.temp, -3); assert.ok(w.ice > 0.95, `Glätte ${w.ice}`);
  w.forceTemp = 2;
  for (let i = 0; i < 5 * 60; i++) updateWorld(w, idle(), 1 / 60);
  assert.ok(w.ice < 0.8, 'taut');
  w.forceTemp = null;
  updateWorld(w, idle(), 1 / 60);
  assert.equal(w.temp, (await import('../web/src/weather.js')).temperatureAt(w.seed, w.dayCount, w.clock, w.forceWeather));
});
```

  An `tests/unwetter.test.js` anhängen (dort gibt es `city`, `createWorld`, `makeSave`, `applySave`, `validateSave` bereits als Importe; sonst ergänzen):

```js
test('Spielstand: Glätte wird gespeichert; alter Spielstand ohne Glätte lädt ohne', async () => {
  const { makeSave, applySave, validateSave } = await import('../web/src/save.js');
  const { createWorld } = await import('../web/src/world.js');
  const w = createWorld({ city }); w.ice = 0.63;
  const s = makeSave(w);
  assert.equal(s.ice, 0.63);
  const w2 = createWorld({ city }); applySave(w2, s);
  assert.equal(w2.ice, 0.63);
  assert.equal(validateSave({ ...s, ice: 7 }).ice, null, 'ungültige Glätte verworfen');
  const old = { ...s }; delete old.ice;
  const w3 = createWorld({ city }); applySave(w3, validateSave(old) ?? old);
  assert.equal(w3.ice, 0, 'alter Spielstand: keine Glätte');
});
```

  An `tests/console.test.js` anhängen (Muster wie die bestehenden `run(l)`-Tests: `execute(line, ctx)` mit `ctx = { game, world, city }`; oben vorhandene Hilfen nutzen, sonst so):

```js
test('Konsole: glaette setzt die Glätte, temp zeigt und erzwingt die Temperatur', async () => {
  const { execute } = await import('../web/src/console.js');
  const { createWorld } = await import('../web/src/world.js');
  const { createGame } = await import('../web/src/game.js');
  const { memoryStorage } = await import('../web/src/save.js');
  const world = createWorld({ city, cars: 0, pedestrians: 0 }), game = createGame({ storage: memoryStorage(), city });
  const ctx = { game, world, city };
  assert.ok(execute('glaette 0.8', ctx).ok); assert.equal(world.ice, 0.8);
  assert.equal(execute('glaette 3', ctx).ok, false);
  assert.match(execute('temp', ctx).msg, /°C/);
  assert.ok(execute('temp -5', ctx).ok); assert.equal(world.forceTemp, -5);
  assert.ok(execute('temp auto', ctx).ok); assert.equal(world.forceTemp, null);
  assert.equal(execute('temp 99', ctx).ok, false);
});
```

  (Falls `tests/console.test.js` `city` nicht schon definiert: `const city = realCity();` mit Import aus `./helpers/city.js` ergänzen. Rückgabe von `execute`: prüfen, ob erfolgreiche Befehle `{ ok: true, msg }` liefern – die bestehenden Befehle geben einen String zurück, den `execute` einpackt; die Tests oben lesen `.ok` und `.msg` wie die vorhandenen Tests.)

- [ ] **Step 2: Run** `node --test tests/weather.test.js tests/unwetter.test.js tests/console.test.js` → FAIL (Funktionen fehlen).

- [ ] **Step 3: Implement**

  `web/src/weather.js`, nach `stepWet`:

```js
// Temperatur (rein): Tageskurve mit Tiefstwert um 5 Uhr und Höchstwert um 15 Uhr, Spanne nach Tagestyp, je Tag ±2 °C aus
// dem Samen. Stetig über Mitternacht (die Nacht läuft vom Höchstwert des Tages zum Tiefstwert des Folgetags).
// Erzwungener Schnee (Konsole) höchstens +1 °C, damit Glätte und Schnee zusammenpassen.
const TEMP = { winter: [-6, 3], unsettled: [4, 14], normal: [8, 22] }, T_LOW = 300, T_HIGH = 900;
function tempRange(seed, d) {
  const [lo, hi] = TEMP[dayType(seed, d)], sh = (hash01((seed * 7907 + d * 3571 + 17) | 0) - 0.5) * 4;
  return [lo + sh, hi + sh];
}
export function temperatureAt(seed, dayCount, minutes, force = null) {
  const m = ((minutes % 1440) + 1440) % 1440, ease = (u) => 0.5 - 0.5 * Math.cos(Math.PI * u);
  let v;
  if (m >= T_LOW && m <= T_HIGH) { const [lo, hi] = tempRange(seed, dayCount); v = lo + (hi - lo) * ease((m - T_LOW) / (T_HIGH - T_LOW)); }
  else {
    const d = m > T_HIGH ? dayCount : dayCount - 1, u = ((m > T_HIGH ? m : m + 1440) - T_HIGH) / (1440 - T_HIGH + T_LOW);
    const hi = tempRange(seed, d)[1], lo = tempRange(seed, d + 1)[0];
    v = hi + (lo - hi) * ease(u);
  }
  return force === 'snow' || force === 'heavysnow' ? Math.min(v, 1) : v;
}

// Glätte des Bodens je Sekunde: überfrierende Nässe (voll in ~10 Spielminuten), taut über 0 °C (~20 Spielminuten)
export const ICE = { rise: 1 / 10, melt: 1 / 20 };
export function stepIce(ice, wet, tempC, dt) {
  if (tempC > 0) return Math.max(0, ice - ICE.melt * dt);
  return wet > 0.1 ? Math.min(1, ice + ICE.rise * dt) : ice;
}
```

  `web/src/world.js`: im Weltobjekt (Zeile 39) `wet: 0, snow: 0,` → `wet: 0, snow: 0, ice: 0, temp: 0, forceTemp: null,`; Import aus `./weather.js` um `temperatureAt, stepIce` erweitern; im Wetterschritt nach der Tauwetter-Zeile:

```js
  w.temp = w.forceTemp ?? temperatureAt(w.seed, w.dayCount, w.clock, w.forceWeather);
  w.ice = stepIce(w.ice ?? 0, w.wet, w.temp, dt);
```

  `web/src/save.js`: in `makeSave` neben `snow` `ice: Math.round((w.ice ?? 0) * 100) / 100,`; in `validateSave` `const ice = num(s.ice, 0, 1) ? s.ice : null;` und `ice` ins Rückgabeobjekt; in `applySave` `w.ice = s.ice ?? 0;` (fehlt im alten Stand ⇒ 0).

  `web/src/console.js`: nach dem `nass`-Befehl (Import `temperatureAt` aus `./weather.js`):

```js
  { name: 'glaette', aliases: ['ice', 'glätte'], help: 'Glätte der Straßen 0–1 (taut über 0 °C, s. temp)', args: [{ name: '0–1', values: () => ['0', '0.5', '1'].map((l) => ({ label: l, hint: '' })) }],
    run(ctx, [v]) { const n = num(v); if (n === null || n < 0 || n > 1) return { ok: false, msg: 'glaette 0 bis 1' }; ctx.world.ice = n; return `Glätte ${Math.round(n * 100)} %`; } },
  { name: 'temp', aliases: ['temperatur'], help: 'Temperatur zeigen; temp -5 erzwingt sie, temp auto gibt sie frei', args: [{ name: '°C', values: () => ['auto', '-5', '0', '5', '20'].map((l) => ({ label: l, hint: '' })) }],
    run(ctx, [v]) {
      const w = ctx.world;
      if (v === undefined) return `${(w.forceTemp ?? temperatureAt(w.seed, w.dayCount, w.clock, w.forceWeather)).toFixed(1).replace('.', ',')} °C${w.forceTemp != null ? ' (erzwungen)' : ''}`;
      if (v === 'auto') { w.forceTemp = null; return 'Temperatur wieder natürlich'; }
      const n = num(v); if (n === null || n < -30 || n > 40) return { ok: false, msg: 'temp -30 bis 40 oder auto' };
      w.forceTemp = n; return `Temperatur ${n} °C`;
    } },
```

  (`num` ist im File vorhanden; `v` kommt als Token-Text. Wenn Befehle mit Argument `undefined` anders ankommen, am `zeit`-Befehl im selben File nachsehen und genauso behandeln.)

- [ ] **Step 4: Run** `node --test tests/weather.test.js tests/unwetter.test.js tests/console.test.js tests/save.test.js` → PASS.

- [ ] **Step 5: Mutationsproben** (vorher committen): `stepIce` ohne `wet > 0.1`-Bedingung → Glätte-Test scheitert; `Math.min(v, 1)` entfernen → Temperatur-Test scheitert; `w.ice = s.ice ?? 0` → `if (s.ice != null) w.ice = s.ice` lassen und im Test `w3.ice = 0.4` vorbelegen → Alt-Stand-Test scheitert (Probe zeigt, dass Laden zurücksetzt).

- [ ] **Step 6: Commit** `git add web/src/weather.js web/src/world.js web/src/save.js web/src/console.js tests/weather.test.js tests/unwetter.test.js tests/console.test.js && git commit -m "Weather: temperature curve, black ice state, save and console"`

---

### Task 2: Straßenzustand und Haftungsfaktoren (`traction.js`)

**Files:**
- Create: `web/src/traction.js`
- Test: `tests/traction.test.js`

**Interfaces:**
- Consumes: `w.wet`, `w.snow`, `w.ice`, `w.weather.{storm,wind}`, `w.time` (Task 1); `edgePuddles(city, e)` (`wetfx.js`), `gustAt(wx, t)` (`weather.js`), `city.edgeSegs`, `segDist2` (`geom.js`).
- Produces:
  - `TRACTION = { wet:{brake,accel,lat,steer}, snow:{…}, ice:{…}, bridgeIce: 1.5, puddleWet: 0.3, rail: { wet: 0.75, ice: 0.6 } }`
  - `DRY = { brake: 1, accel: 1, lat: 1, steer: 1 }` (eingefroren)
  - `AQUA = { speed: 194, time: 0.35, lat: 0.15, steer: 0.2, brake: 0.3, yaw: 0.6 }`
  - `GUST = { push: 45, threshold: 0.9, bridge: 1.6, warn: 8 }`
  - `puddleAt(world, x, y, lvl = 0) → { x, y, rx, ry, a } | null`
  - `roadCondition(world, x, y, lvl = 0) → { wet, snow, ice, puddle, covered, bridge }`
  - `tractionOf(cond) → { brake, accel, lat, steer }`
  - `gustPush(world, car, lvl = 0) → { ax, ay } | null` (px/s²)
  - `adhesionOf(cond) → 0.6…1`

- [ ] **Step 1: Failing tests** `tests/traction.test.js`:

```js
import test from 'node:test';
import assert from 'node:assert/strict';
import { roadCondition, tractionOf, gustPush, adhesionOf, puddleAt, TRACTION, DRY, GUST } from '../web/src/traction.js';
import { edgePuddles } from '../web/src/wetfx.js';
import { createWorld } from '../web/src/world.js';
import { createCar } from '../web/src/car.js';
import { weatherAt } from '../web/src/weather.js';
import { realCity } from './helpers/city.js';

const city = realCity();
const world = () => { const w = createWorld({ city, cars: 0, pedestrians: 0 }); w.wet = 0; w.snow = 0; w.ice = 0; return w; };
const mid = (e) => { const k = (e.pts.length >> 1) & ~1; return { x: e.pts[k], y: e.pts[k + 1] }; };

test('Haftung: trocken exakt 1, Tabellenwerte je Zustand, gemischt mit Untergrenze', () => {
  assert.deepEqual(tractionOf({ wet: 0, snow: 0, ice: 0 }), DRY);
  assert.deepEqual(tractionOf({ wet: 1, snow: 0, ice: 0 }), TRACTION.wet);
  assert.deepEqual(tractionOf({ wet: 0, snow: 1, ice: 0 }), TRACTION.snow);
  assert.deepEqual(tractionOf({ wet: 0, snow: 0, ice: 1 }), TRACTION.ice);
  const half = tractionOf({ wet: 0.5, snow: 0, ice: 0 });
  assert.ok(Math.abs(half.brake - (1 - 0.5 * (1 - TRACTION.wet.brake))) < 1e-9, 'linear nach Stärke');
  const worst = tractionOf({ wet: 1, snow: 1, ice: 1 });
  for (const k of ['brake', 'accel', 'lat', 'steer']) assert.equal(worst[k], TRACTION.ice[k], `${k}: Untergrenze Glätte`);
});

test('Straßenzustand: Weltwerte, Brücke glatter, überdacht (Durchfahrt, unter der Brücke) trocken', () => {
  const w = world(); w.wet = 1; w.snow = 0.4; w.ice = 0.5;
  const road = city.list('edge').find((e) => !e.bridge && !e.passage && (e.lvl ?? 0) === 0 && e.cls <= 6 && e.len > 200);
  const r = roadCondition(w, mid(road).x, mid(road).y, 0);
  assert.equal(r.wet, 1); assert.equal(r.snow, 0.4); assert.equal(r.ice, 0.5); assert.equal(r.covered, false);
  const bridge = city.list('edge').find((e) => e.bridge && (e.lvl ?? 0) >= 1 && e.len > 60);
  const b = mid(bridge);
  assert.equal(roadCondition(w, b.x, b.y, bridge.lvl).ice, 0.75, 'Brücke ×1,5');
  const under = roadCondition(w, b.x, b.y, 0);
  assert.equal(under.covered, true, 'unter der Brücke überdacht');
  assert.equal(under.wet + under.snow + under.ice, 0, 'unter der Brücke trocken');
  const passage = city.list('edge').find((e) => e.passage && e.len > 20);
  assert.ok(passage, 'Durchfahrt im Testgebiet');
  const p = mid(passage);
  assert.equal(roadCondition(w, p.x, p.y, passage.lvl ?? 0).covered, true, 'Durchfahrt überdacht');
});

test('Pfütze: nur in der gezeichneten Ellipse und nur bei Nässe', () => {
  const w = world();
  const e = city.list('edge').find((x) => !x.bridge && (x.lvl ?? 0) === 0 && edgePuddles(city, x).length);
  const pd = edgePuddles(city, e)[0];
  w.wet = 0.2; assert.equal(roadCondition(w, pd.x, pd.y, 0).puddle, null, 'zu trocken');
  w.wet = 1; assert.ok(roadCondition(w, pd.x, pd.y, 0).puddle, 'in der Pfütze');
  assert.deepEqual(puddleAt(w, pd.x, pd.y, 0), pd);
  const off = pd.rx + 30;
  assert.equal(puddleAt(w, pd.x - Math.sin(pd.a) * off, pd.y + Math.cos(pd.a) * off, 0), null, 'daneben');
});

test('Böen: quer zur Windrichtung, Brücke ×1,6, schwere Fahrzeuge weniger, stehende und geparkte gar nicht', () => {
  const w = world(); w.weather = weatherAt(1, 0, 600, 'storm'); w.time = 0;
  // Zeitpunkt einer kräftigen Böe
  let t = 0; while (!gustPush(w, Object.assign(createCar({ x: 0, y: 0 }), { vx: 140, vy: 0 }), 0)) { t += 0.1; w.time = t; }
  const car = Object.assign(createCar({ x: 0, y: 0 }), { vx: 140, vy: 0 });
  const g = gustPush(w, car, 0), gb = gustPush(w, car, 1);
  const wl = Math.hypot(w.weather.wind.x, w.weather.wind.y);
  assert.ok(Math.abs(g.ax / Math.hypot(g.ax, g.ay) - w.weather.wind.x / wl) < 1e-9, 'Windrichtung');
  assert.ok(Math.abs(Math.hypot(gb.ax, gb.ay) / Math.hypot(g.ax, g.ay) - GUST.bridge) < 1e-9, 'Brücke');
  const van = Object.assign(createCar({ x: 0, y: 0 }), { vx: 140, vy: 0, hw: 32, hh: 13 });
  assert.ok(Math.hypot(...Object.values(gustPush(w, van, 0))) < Math.hypot(g.ax, g.ay), 'schwerer: weniger');
  assert.equal(gustPush(w, Object.assign(createCar({ x: 0, y: 0 }), { vx: 2, vy: 0 }), 0), null, 'steht');
  const parked = Object.assign(createCar({ x: 0, y: 0 }), { vx: 140, vy: 0, role: 'curb', driver: null });
  assert.equal(gustPush(w, parked, 0), null, 'geparkt');
  w.weather = weatherAt(1, 0, 600, 'clear');
  assert.equal(gustPush(w, car, 0), null, 'kein Sturm');
});

test('Schienenhaftung: trocken 1, nass 0,75, Frost 0,6, nie darunter', () => {
  assert.equal(adhesionOf({ wet: 0, snow: 0, ice: 0 }), 1);
  assert.equal(adhesionOf({ wet: 1, snow: 0, ice: 0 }), TRACTION.rail.wet);
  assert.equal(adhesionOf({ wet: 1, snow: 1, ice: 1 }), TRACTION.rail.ice);
});
```

- [ ] **Step 2: Run** `node --test tests/traction.test.js` → FAIL (Modul fehlt).

- [ ] **Step 3: Implement** `web/src/traction.js`:

```js
// Wetter auf der Straße (rein): wie nass, verschneit und glatt eine Stelle ist, ob dort eine Pfütze steht, wie stark eine
// Böe schiebt – und was daraus für Bremsen, Anfahren, Seitenhalt und Lenkung folgt. world.js schreibt das Ergebnis je
// Schritt ans Auto (car.traction); car.js, traffic.js und trainphysics.js lesen nur die Faktoren. Überdachte Stellen
// (Durchfahrt, Boden unter einer Brücke) sind trocken, Brücken frieren zuerst. Kein world.rng: Aquaplaning-Richtung aus
// dem Pfützen-Hash, Böen aus weather.js gustAt(world.time).
import { edgePuddles } from './wetfx.js';
import { gustAt } from './weather.js';
import { segDist2 } from './geom.js';
import { hash01 } from './map.js';

export const TRACTION = {
  wet: { brake: 0.77, accel: 0.85, lat: 0.82, steer: 0.95 },
  snow: { brake: 0.5, accel: 0.55, lat: 0.55, steer: 0.8 },
  ice: { brake: 0.33, accel: 0.4, lat: 0.35, steer: 0.65 },
  bridgeIce: 1.5, puddleWet: 0.3,
  rail: { wet: 0.75, ice: 0.6 },
};
export const DRY = Object.freeze({ brake: 1, accel: 1, lat: 1, steer: 1 });
export const AQUA = { speed: 70 / 0.36, time: 0.35, lat: 0.15, steer: 0.2, brake: 0.3, yaw: 0.6 };
export const GUST = { push: 45, threshold: 0.9, bridge: 1.6, warn: 8 };
const KEYS = ['brake', 'accel', 'lat', 'steer'];
const q = [];
const clamp01 = (v) => Math.min(1, Math.max(0, v ?? 0));

// Nächste Fahrbahn derselben Ebene unter (x, y) und ob etwas darüber liegt (höhere Ebene oder Durchfahrt)
function under(world, x, y, lvl) {
  let covered = false, near = null, nd = Infinity;
  for (const s of world.city.edgeSegs.query({ x: x - 40, y: y - 40, w: 80, h: 80 }, q)) {
    const e = s.e;
    if (e.junction) continue;
    const d2 = segDist2(x, y, s.ax, s.ay, s.bx, s.by), half = e.w / 2;
    if (d2 > half * half) continue;
    const el = e.lvl ?? 0;
    if (el > lvl || (e.passage && el === lvl)) covered = true;
    if (el === lvl && d2 < nd) { nd = d2; near = e; }
  }
  return { covered, near };
}

function inPuddle(city, e, x, y) {
  for (const p of edgePuddles(city, e)) {
    const dx = x - p.x, dy = y - p.y, c = Math.cos(p.a), s = Math.sin(p.a);
    const lx = (dx * c + dy * s) / p.rx, ly = (-dx * s + dy * c) / p.ry;
    if (lx * lx + ly * ly <= 1) return p;
  }
  return null;
}

export function puddleAt(world, x, y, lvl = 0) {
  if ((world.wet ?? 0) <= TRACTION.puddleWet) return null;
  const u = under(world, x, y, lvl);
  return u.covered || !u.near ? null : inPuddle(world.city, u.near, x, y);
}

export function roadCondition(world, x, y, lvl = 0) {
  const u = under(world, x, y, lvl), bridge = lvl >= 1;
  if (u.covered) return { wet: 0, snow: 0, ice: 0, puddle: null, covered: true, bridge };
  const wet = clamp01(world.wet), snow = clamp01(world.snow), ice = Math.min(1, clamp01(world.ice) * (bridge ? TRACTION.bridgeIce : 1));
  const puddle = wet > TRACTION.puddleWet && u.near ? inPuddle(world.city, u.near, x, y) : null;
  return { wet, snow, ice, puddle, covered: false, bridge };
}

export function tractionOf(c) {
  const mix = (amt, f) => 1 - (1 - f) * clamp01(amt), out = {};
  for (const k of KEYS) out[k] = Math.max(TRACTION.ice[k], mix(c.wet, TRACTION.wet[k]) * mix(c.snow, TRACTION.snow[k]) * mix(c.ice, TRACTION.ice[k]));
  return out;
}

export function adhesionOf(c) {
  const mix = (amt, f) => 1 - (1 - f) * clamp01(amt);
  return Math.max(TRACTION.rail.ice, mix(c.wet, TRACTION.rail.wet) * mix(c.ice, TRACTION.rail.ice));
}

export function gustPush(world, car, lvl = 0) {
  const wx = world.weather, storm = wx?.storm ?? 0;
  if (storm <= 0 || !wx.wind || (car.role === 'curb' && car.driver === null) || Math.hypot(car.vx, car.vy) < 5) return null;
  const g = gustAt(wx, world.time) - GUST.threshold;
  if (g <= 0) return null;
  const wl = Math.hypot(wx.wind.x, wx.wind.y) || 1;
  const a = GUST.push * storm * g * (lvl >= 1 ? GUST.bridge : 1) / ((car.hw * car.hh) / (21 * 10));
  return { ax: (wx.wind.x / wl) * a, ay: (wx.wind.y / wl) * a };
}

// Gierimpuls beim Aufschwimmen: Richtung und Stärke aus dem Pfützen-Hash (±AQUA.yaw rad/s)
export const aquaYaw = (p) => (hash01(Math.round(p.x) * 73856 + Math.round(p.y) * 19349) * 2 - 1) * AQUA.yaw;
```

- [ ] **Step 4: Run** `node --test tests/traction.test.js` → PASS. Falls „Durchfahrt im Testgebiet“ fehlt: im Kreuzberg-/Neukölln-Ausschnitt nach `e.passage` suchen (`city.list('edge').filter((e) => e.passage).length`), sie existieren (Durchfahrten sind im Build markiert); nur wenn es wirklich keine gibt, den Teil als eigenen Test mit künstlicher Kante schreiben und das in den Ledger.

- [ ] **Step 5: Mutationsproben**: `Math.max(TRACTION.ice[k], …)` → ohne `max` → Untergrenze-Test scheitert; `el > lvl` → `el > lvl + 5` → Brücken-Überdachung scheitert; `puddleWet`-Vergleich entfernen → „zu trocken“ scheitert; `car.role === 'curb'`-Bedingung entfernen → „geparkt“ scheitert.

- [ ] **Step 6: Commit** `git add web/src/traction.js tests/traction.test.js && git commit -m "Traction: road condition, grip factors, gusts, rail adhesion"`

---

### Task 3: Autophysik mit Wetter (Haftung, Aquaplaning, Böen)

**Files:**
- Modify: `web/src/car.js:38–72` (`stepCar`), `web/src/world.js:626–634` (Autoschleife)
- Modify: `tests/weather.test.js:83–92`, `tests/unwetter.test.js:148–154` (setzten `c.wet`/`c.snow` – das Feld gibt es danach nicht mehr)
- Test: `tests/car.test.js` (anhängen)

**Interfaces:**
- Consumes: `roadCondition`, `tractionOf`, `puddleAt`, `gustPush`, `aquaYaw`, `AQUA`, `DRY` (Task 2).
- Produces: `car.traction` (Faktoren, fehlt = trocken), `car.aqua` (Restzeit s), `car.spin` (0/1, nur Darstellung), Ereignis `aquaplane { x, y, carId, player }`; `world.js applyWeather(w, car)` (nicht exportiert).

- [ ] **Step 1: Failing tests** – an `tests/car.test.js` anhängen:

```js
import { tractionOf, TRACTION, AQUA } from '../web/src/traction.js';

const brakeDist = (traction) => {
  const c = createCar({ x: 0, y: 0 }); c.traction = traction; c.vx = 50 / 0.36; c.vy = 0; c.angle = 0;
  c.controls.brake = 1;
  let x0 = c.x; for (let i = 0; i < 600 && Math.hypot(c.vx, c.vy) > 1; i++) stepCar(c, 1 / 60, null);
  return c.x - x0;
};

test('Bremsweg aus 50 km/h: trocken : nass : Schnee : Glätte ≈ 1 : 1,3 : 2 : 3', () => {
  const d0 = brakeDist(undefined), dw = brakeDist(tractionOf({ wet: 1 })), ds = brakeDist(tractionOf({ snow: 1 })), di = brakeDist(tractionOf({ ice: 1 }));
  assert.ok(Math.abs(dw / d0 - 1.3) < 0.15, `nass ${(dw / d0).toFixed(2)}`);
  assert.ok(Math.abs(ds / d0 - 2) < 0.25, `Schnee ${(ds / d0).toFixed(2)}`);
  assert.ok(Math.abs(di / d0 - 3) < 0.4, `Glätte ${(di / d0).toFixed(2)}`);
  assert.equal(brakeDist({ brake: 1, accel: 1, lat: 1, steer: 1 }), d0, 'trocken = ohne Wetter');
});

test('Anfahren auf Glätte begrenzt und als Durchdrehen markiert; Lenkung schwächer', () => {
  const run = (traction) => { const c = createCar({ x: 0, y: 0 }); c.traction = traction; c.controls.throttle = 1; for (let i = 0; i < 60; i++) stepCar(c, 1 / 60, null); return c; };
  const dry = run(undefined), ice = run(tractionOf({ ice: 1 }));
  assert.ok(Math.hypot(ice.vx, ice.vy) < Math.hypot(dry.vx, dry.vy) * 0.5, 'langsamer');
  assert.equal(ice.spin, 1); assert.equal(dry.spin ?? 0, 0);
  const turn = (traction) => { const c = createCar({ x: 0, y: 0 }); c.traction = traction; c.vx = 150; c.controls.steer = 1; c.controls.throttle = 0.3; for (let i = 0; i < 30; i++) stepCar(c, 1 / 60, null); return Math.abs(c.angle); };
  assert.ok(turn(tractionOf({ ice: 1 })) < turn(undefined), 'lenkt weniger ein');
});

test('Aquaplaning: Seitenhalt und Lenkung fast weg, solange car.aqua läuft', () => {
  const slide = (aqua) => { const c = createCar({ x: 0, y: 0 }); c.vx = 250; c.vy = 150; c.aqua = aqua; for (let i = 0; i < 12; i++) stepCar(c, 1 / 60, null); return Math.abs(-c.vx * Math.sin(c.angle) + c.vy * Math.cos(c.angle)); };
  assert.ok(slide(AQUA.time) > slide(0) * 3, `${slide(AQUA.time)} vs ${slide(0)}`);
  const c = createCar({ x: 0, y: 0 }); c.aqua = 0.1; stepCar(c, 1 / 60, null);
  assert.ok(Math.abs(c.aqua - (0.1 - 1 / 60)) < 1e-9, 'läuft ab');
});
```

  Welt-Tests (Aquaplaning an einer echten Pfütze, Böen, geparkte Autos, 30 fps) an `tests/traction.test.js` anhängen:

```js
import { updateWorld } from '../web/src/world.js';
import { idle } from './helpers/bot.js';
import { AQUA } from '../web/src/traction.js';

function puddleRun(fps, kmh) {
  const w = createWorld({ city, cars: 0, pedestrians: 0 }); w.mission.state = 'idle'; w.wet = 1; w.forceWeather = 'rain';
  const e = city.list('edge').find((x) => !x.bridge && (x.lvl ?? 0) === 0 && x.cls <= 6 && edgePuddles(city, x).length && edgePuddles(city, x)[0].rx > 12);
  const pd = edgePuddles(city, e)[0];
  const car = w.cars.find((c) => c.id === w.playerCarId);
  w.player.inCar = car.id; car.driver = 'player';
  const v = kmh / 0.36; car.angle = pd.a; car.x = pd.x - Math.cos(pd.a) * v * 0.3; car.y = pd.y - Math.sin(pd.a) * v * 0.3; car.lvl = 0;
  car.vx = Math.cos(pd.a) * v; car.vy = Math.sin(pd.a) * v; car.angVel = 0;
  w.camera.x = car.x; w.camera.y = car.y;
  const events = [];
  for (let i = 0; i < fps; i++) { updateWorld(w, { ...idle(), throttle: 1 }, 1 / fps); events.push(...w.events.filter((x) => x.type === 'aquaplane')); if (car.aqua > 0) car.sawAqua = true; }
  return { events, car };
}

test('Aquaplaning in einer echten Pfütze: nur über 70 km/h, ein Ereignis, gleich bei 30 und 60 fps', () => {
  const slow = puddleRun(60, 50), fast = puddleRun(60, 90), fast30 = puddleRun(30, 90);
  assert.equal(slow.events.length, 0, 'langsam: kein Aquaplaning');
  assert.equal(fast.events.length, 1, 'schnell: genau ein Ereignis');
  assert.equal(fast30.events.length, 1, '30 fps: ebenso');
  assert.ok(fast.events[0].player, 'Spielerauto markiert');
  assert.equal(puddleRun(60, 90).car.angle.toFixed(6), fast.car.angle.toFixed(6), 'deterministisch');
});

test('Böen in der Welt: Pkw versetzt 0,5–1 m, auf der Brücke mehr, geparkte Autos bewegen sich nie', () => {
  const offset = (lvl) => {
    const w = createWorld({ city, cars: 0, pedestrians: 0 }); w.mission.state = 'idle'; w.forceWeather = 'storm';
    w.weather = weatherAt(w.seed, 0, 600, 'storm');
    // Wind quer zur Fahrtrichtung: Auto fährt senkrecht zum Wind
    const a = Math.atan2(w.weather.wind.y, w.weather.wind.x) + Math.PI / 2;
    // stärkste Böe der ersten 10 min suchen, dort 3 s messen
    let best = 0, bt = 0; for (let t = 0; t < 600; t += 0.1) { const g = gustPushProbe(w, t); if (g > best) { best = g; bt = t; } }
    const c = createCar({ x: 0, y: 0 }); c.angle = a; c.vx = Math.cos(a) * 139; c.vy = Math.sin(a) * 139; c.lvl = lvl; c.driver = 'player';
    let side = 0;
    for (let t = bt - 1.5; t < bt + 1.5; t += 1 / 60) {
      w.time = t;
      const push = gustPush(w, c, lvl);
      if (push) { c.vx += push.ax / 60; c.vy += push.ay / 60; }
      c.controls.throttle = 0.35; stepCar(c, 1 / 60, null);
    }
    side = Math.abs(-c.x * Math.sin(a) + c.y * Math.cos(a));
    return side / 10; // m
  };
  const ground = offset(0), onBridge = offset(1);
  assert.ok(ground >= 0.5 && ground <= 1, `Versatz ${ground.toFixed(2)} m`);
  assert.ok(onBridge > ground * 1.4 && onBridge <= 2, `Brücke ${onBridge.toFixed(2)} m`);
  const w = createWorld({ city }); w.mission.state = 'idle'; w.forceWeather = 'storm';
  for (let i = 0; i < 60; i++) updateWorld(w, idle(), 1 / 60);
  const parked = w.cars.filter((c) => c.role === 'curb' && c.driver === null).slice(0, 10).map((c) => ({ c, x: c.x, y: c.y }));
  assert.ok(parked.length > 3, 'geparkte Autos im Bild');
  for (let i = 0; i < 20 * 60; i++) updateWorld(w, idle(), 1 / 60);
  for (const p of parked) assert.ok(Math.hypot(p.c.x - p.x, p.c.y - p.y) < 0.01, 'geparktes Auto bewegt sich nicht');
});
const gustPushProbe = (w, t) => { w.time = t; const p = gustPush(w, Object.assign(createCar({ x: 0, y: 0 }), { vx: 140, vy: 0 }), 0); return p ? Math.hypot(p.ax, p.ay) : 0; };
```

  (Import `stepCar` aus `../web/src/car.js` in `tests/traction.test.js` ergänzen.)

- [ ] **Step 2: Run** `node --test tests/car.test.js tests/traction.test.js` → FAIL.

- [ ] **Step 3: Implement**

  `web/src/car.js` – Import `import { DRY, AQUA } from './traction.js';` und in `stepCar`:

```js
  const tr = car.traction ?? DRY, aq = (car.aqua ?? 0) > 0;
  const kBrake = tr.brake * (aq ? AQUA.brake : 1), kLat = tr.lat * (aq ? AQUA.lat : 1), kSteer = tr.steer * (aq ? AQUA.steer : 1);
```
  direkt nach der Berechnung von `vr`; dann:
  - Gas: `vf += CAR.accel * pw * ctl.throttle * t * tr.accel * dt;` und direkt danach `car.spin = ctl.throttle > 0.8 && tr.accel < 0.7 && vf < 150 ? 1 : 0;` (ohne Gas: `car.spin = 0;` am Anfang der Funktion setzen).
  - Bremse: `CAR.brake * ctl.brake * kBrake * dt`; Handbremse: `CAR.handbrake * kBrake * dt`.
  - Seitenhalt: die Zeile mit `car.wet`/`car.snow` ersetzen durch
    `const grip = (ctl.handbrake ? CAR.handbrakeGrip : CAR.grip * surf.grip) * kLat;` und den Kommentar darüber durch `// Wetter (traction.js, von world.js je Schritt gesetzt): Seitenhalt; beim Aquaplaning fast keiner`.
  - Lenkung: `const target = ctl.steer * CAR.steerRate * speedFactor * sign(vf) * (ctl.handbrake ? 1.35 : 1) * kSteer;`
  - am Ende: `if (car.aqua > 0) car.aqua = Math.max(0, car.aqua - dt);`

  `web/src/world.js` – Import `import { roadCondition, tractionOf, puddleAt, gustPush, aquaYaw, AQUA } from './traction.js';` und vor `updateWorld`:

```js
// Wetter am Auto: Haftung (car.traction), Aufschwimmen in einer Pfütze (car.aqua, einmal je Pfütze) und Böen
function applyWeather(w, c, dt) {
  const lvl = c.lvl ?? 0;
  c.traction = tractionOf(roadCondition(w, c.x, c.y, lvl));
  const vf = c.vx * Math.cos(c.angle) + c.vy * Math.sin(c.angle);
  let p = null;
  if (vf > AQUA.speed) {
    const ca = Math.cos(c.angle), sa = Math.sin(c.angle), fx = c.hw * 0.7, fy = c.hh * 0.8;
    for (const s of [-1, 1]) { p = puddleAt(w, c.x + ca * fx - sa * fy * s, c.y + sa * fx + ca * fy * s, lvl); if (p) break; }
  }
  if (p && p !== c._aquaP && !(c.aqua > 0)) {
    c.aqua = AQUA.time; c.angVel += aquaYaw(p);
    w.events.push({ type: 'aquaplane', x: c.x, y: c.y, carId: c.id, player: c.id === w.player.inCar });
  }
  if (p) c._aquaP = p; else if (!(c.aqua > 0)) c._aquaP = null;
  const g = gustPush(w, c, lvl);
  if (g) { c.vx += g.ax * dt; c.vy += g.ay * dt; }
}
```

  In der Autoschleife die Zeile `c.wet = w.wet; c.snow = w.snow;` durch `applyWeather(w, c, dt);` ersetzen (sie steht nach dem `continue` für schlafende Parker – die bleiben unberührt).

  Bestehende Tests anpassen (sie setzten die entfernten Felder): `tests/weather.test.js` Zeile 86 `c.wet = wet;` → `c.traction = tractionOf({ wet });` (Import `tractionOf` aus `../web/src/traction.js`); `tests/unwetter.test.js` Zeile 149 `c.snow = snow;` → `c.traction = tractionOf({ snow });` (Import ergänzen). Die Aussagen der Tests bleiben (nasse/verschneite Fahrbahn rutscht mehr).

- [ ] **Step 4: Run** `node --test tests/car.test.js tests/traction.test.js tests/weather.test.js tests/unwetter.test.js tests/traffic.test.js` → PASS. Den Böen-Maßstab stellt nur `GUST.push` ein: liegt der Versatz außerhalb 0,5–1 m, `GUST.push` proportional anpassen (Versatz ∝ push), bis der Test passt; Wert in den Ledger. Die Brücke folgt dann über `GUST.bridge`.

- [ ] **Step 5: Mutationsproben**: `* kBrake` bei der Bremse entfernen → Bremsweg-Test scheitert; `p !== c._aquaP` entfernen → „genau ein Ereignis“ scheitert; `vf > AQUA.speed` → `vf > 0` → „langsam: kein Aquaplaning“ scheitert; die `continue`-Zeile für schlafende Parker vor `applyWeather` verschieben (Parker bekommen Böen) → „geparktes Auto bewegt sich nicht“ scheitert (falls nicht: `gustPush`-Parker-Bedingung zusätzlich entfernen und die Probe dokumentieren).

- [ ] **Step 6: Commit** `git add web/src/car.js web/src/world.js tests/car.test.js tests/traction.test.js tests/weather.test.js tests/unwetter.test.js && git commit -m "Car physics: weather grip, aquaplaning at puddles, storm gusts"`

---

### Task 4: KI-Verkehr passt sich an

**Files:**
- Modify: `web/src/traffic.js:318–380` (`driveAi`)
- Test: `tests/traffic.test.js` (anhängen)

**Interfaces:**
- Consumes: `car.traction` (Task 3; fehlt = `DRY`).
- Produces: Verhalten: Zieltempo × (0,6 + 0,4·brake); Bremskurven mit `90·brake` (Ampel, Zebrastreifen, Tor) und Kurvenanfahrt mit `260·brake`; Kurvendeckel `55·lat`; Folgeabstand: Anfahrrampe `(d − GAP_PX)·2,2·brake`, Wirkbereich `125/brake`. **Stillstandsabstand `GAP_PX` bleibt** (Schlangen und Reservierungen unverändert kompakt; der Abstand in Fahrt wächst mit 1/brake – so ist „Abstand × 1/brake“ der Spec umgesetzt, ohne Staus an Engstellen).

- [ ] **Step 1: Failing tests** – an `tests/traffic.test.js` anhängen:

```js
test('Glätte: KI fährt langsamer und hält an Rot mit Abstand, ohne aufzufahren', async () => {
  const { signalState } = await import('../web/src/signals.js');
  const { placeOnLane } = await import('../web/src/traffic.js');
  const { createCar } = await import('../web/src/car.js');
  const { laneDir } = await import('../web/src/roadgraph.js');
  const { obbVsObb } = await import('../web/src/collision.js');
  const lane = [...g.lanes].find((l) => city.signals.has(l.to) && l.len > 900 && l.edge.cls <= 5 && l.next.length && !l.edge.bridge);
  const [ux, uy] = laneDir(lane, true), heading = Math.atan2(uy, ux);
  const run = (icy) => {
    const w = createWorld({ city, cars: 0, pedestrians: 0 });
    if (icy) { w.wet = 1; w.ice = 1; w.forceTemp = -5; w.forceWeather = 'overcast'; }
    let t0 = 0.5; while (!(signalState(city, lane.to, heading, t0) === 'red' && signalState(city, lane.to, heading, t0 - 0.5) !== 'red')) t0 += 0.5;
    w.time = t0;
    const cars = [0, 1, 2].map((k) => { const c = createCar({ x: 0, y: 0 }); c.driver = 'npc'; placeOnLane(c, city, lane, lane.len - 650 - k * 80, w.rng); w.cars.push(c); return c; });
    w.camera.x = cars[1].x; w.camera.y = cars[1].y;
    let crashes = 0, overlap = 0, top = 0;
    for (let i = 0; i < 30 * 60; i++) {
      updateWorld(w, idle(), 1 / 60);
      w.camera.x = cars[1].x; w.camera.y = cars[1].y;
      crashes += w.events.filter((e) => e.type === 'crash').length;
      for (let a = 0; a < 3; a++) for (let b = a + 1; b < 3; b++) if (obbVsObb(cars[a], cars[b])) overlap++;
      if (i < 8 * 60) top = Math.max(top, Math.hypot(cars[0].vx, cars[0].vy));
    }
    return { cars, crashes, overlap, top };
  };
  const dry = run(false), ice = run(true);
  assert.ok(ice.top < dry.top * 0.85, `Glätte langsamer (${ice.top.toFixed(0)} vs ${dry.top.toFixed(0)} px/s)`);
  assert.ok(ice.cars.every((c) => Math.hypot(c.vx, c.vy) < 5), 'alle stehen an Rot');
  assert.equal(ice.crashes, 0, 'niemand fährt auf'); assert.equal(ice.overlap, 0, 'keine Berührung');
});

test('Glätte: 3 min an einer engen Stelle – niemand steht über 90 s, kaum Zusammenstöße', async () => {
  const { speedOf } = await import('../web/src/car.js');
  const p = city.places.giver, w = createWorld({ city, seed: 5 });
  w.wet = 1; w.ice = 1; w.forceTemp = -5; w.forceWeather = 'overcast';
  w.camera.x = p.x; w.camera.y = p.y;
  const still = new Map(); let crashes = 0, worst = 0;
  for (let i = 0; i < 180 * 60; i++) {
    updateWorld(w, idle(), 1 / 60); w.camera.x = p.x; w.camera.y = p.y;
    crashes += w.events.filter((e) => e.type === 'crash').length;
    if (i % 30) continue;
    for (const c of w.cars) { if (c.driver !== 'npc') continue; const t = speedOf(c) < 5 ? (still.get(c.id) ?? 0) + 0.5 : 0; still.set(c.id, t); worst = Math.max(worst, t); }
  }
  assert.ok(worst < 90, `ein Auto stand ${worst} s am Stück`);
  assert.ok(crashes < 15, `${crashes} Zusammenstöße in 3 min`);
});
```

- [ ] **Step 2: Run** `node --test --test-name-pattern="Glätte" tests/traffic.test.js` → FAIL (erster Test: nicht langsamer bzw. Auffahren bei Glätte).

- [ ] **Step 3: Implement** in `driveAi` (Import `import { DRY } from './traction.js';`):
  - Direkt nach `let target = (ai.cap[ai.i] ?? 100) * ai.cruiseK;`:
    `const tr = car.traction ?? DRY, kb = tr.brake; target *= 0.6 + 0.4 * kb; // Wetter: vorsichtiger (traction.js)`
  - Kurvenanfahrt: `2 * 260 *` → `2 * 260 * kb *`.
  - `if (Math.abs(diff) > 0.6) target = Math.min(target, 55);` → `… Math.min(target, 55 * tr.lat);`
  - Ampel, Zebrastreifen, Einfahrtstor: jede `Math.sqrt(2 * 90 * …)` → `Math.sqrt(2 * 90 * kb * …)` (drei Stellen, Zeilen ~356, ~362, ~371). Das `brakeDist` für Gelb (`vf * vf / (2 * 300)`) → `(2 * 300 * kb)`.
  - Folgeabstand: `if (d < 125) target = Math.min(target, Math.max(0, (d - GAP_PX) * 2.2));` → `if (d < 125 / kb) target = Math.min(target, Math.max(0, (d - GAP_PX) * 2.2 * kb));`

- [ ] **Step 4: Run** `node --test tests/traffic.test.js` → PASS (alle, auch die bestehenden Dauertests – trocken ist `kb = 1`, alles unverändert).

- [ ] **Step 5: Mutationsproben**: `target *= 0.6 + 0.4 * kb` entfernen → „langsamer“ scheitert; Ampel-`* kb` entfernen → Glätte-Rot-Test (Auffahren/Überfahren der Linie) scheitert – falls nicht, zusätzlich prüfen, dass `cars[0]` vor der Haltelinie steht (`ai.lightDist > 0`), und diese Zusicherung ergänzen.

- [ ] **Step 6: Commit** `git add web/src/traffic.js tests/traffic.test.js && git commit -m "Traffic AI adapts to weather: slower, earlier braking, longer following distance"`

---

### Task 5: Haftung des eigenen Zugs

**Files:**
- Modify: `web/src/trainphysics.js:10–34` (`stepDrive`), `web/src/playertrain.js:72` (`const inp = …`)
- Test: `tests/trainphysics.test.js`, `tests/playertrain.test.js` (anhängen)

**Interfaces:**
- Consumes: `roadCondition`, `adhesionOf` (Task 2); `undergroundAtS` (`tunnel.js`), `railAt` (`tunnel.js`).
- Produces: `stepDrive(d, { …, adhesion })` (fehlt = 1); `trainAdhesion(w, t) → 0.6…1` (exportiert aus `playertrain.js`).

- [ ] **Step 1: Failing tests** – an `tests/trainphysics.test.js`:

```js
test('Haftung: nasse Schienen verlängern den Bremsweg, Zwangsbremsung bleibt unter der verringerten Kurve', () => {
  const stop = (adhesion) => { const d = createDrive('sbahn', 100 / 0.36); let s = 0; for (let i = 0; i < 60 * 60 && d.v > 0; i++) { stepDrive(d, { brake: 1, limit: Infinity, adhesion }, 1 / 60); s += d.v / 60; } return s; };
  assert.ok(Math.abs(stop(0.75) / stop(1) - 1 / 0.75) < 0.05, `nass ${(stop(0.75) / stop(1)).toFixed(2)}`);
  assert.equal(stop(undefined), stop(1), 'ohne Angabe wie trocken');
  const k = TRAIN_DRIVE.sbahn, adh = 0.6, dist = brakeDistance(k.vmax, k.emergency * adh) + 300, d = createDrive('sbahn', k.vmax);
  let s = 0;
  for (let i = 0; i < 60 * 90 && (d.v > 0 || i < 5); i++) {
    const lim = dist - s; stepDrive(d, { throttle: 1, limit: lim, adhesion: adh }, 1 / 60);
    const vAllowed = Math.sqrt(Math.max(0, 2 * k.emergency * adh * Math.max(0, lim - d.v / 60)));
    assert.ok(d.v <= vAllowed + 0.5, `v ${d.v.toFixed(1)} über ${vAllowed.toFixed(1)}`);
    s += d.v / 60;
  }
  assert.equal(d.v, 0); assert.ok(s <= dist, 'hält vor dem Hindernis');
  const acc = (adhesion) => { const q = createDrive('ubahn', 0); for (let i = 0; i < 120; i++) stepDrive(q, { throttle: 1, limit: Infinity, adhesion }, 1 / 60); return q.v; };
  assert.ok(acc(0.6) < acc(1) * 0.7, 'Räder drehen durch: weniger Zugkraft');
});
```

  an `tests/playertrain.test.js`:

```js
test('Haftung des eigenen Zugs: oberirdisch nass/Frost geringer, im Tunnel immer 1', async () => {
  const { trainAdhesion } = await import('../web/src/playertrain.js');
  const p = pat('M10', 'tram'), w = atFrontOf(p, 3);
  press(w, { enterExit: true });
  w.wet = 0; w.ice = 0; assert.equal(trainAdhesion(w, w.playerTrain), 1);
  w.wet = 1; assert.equal(trainAdhesion(w, w.playerTrain), 0.75);
  w.ice = 1; assert.equal(trainAdhesion(w, w.playerTrain), 0.6);
  const u8 = tr.patterns.filter((q) => q.name === 'U8' && q.mode === 'ubahn').sort((a, b) => b.stops.length - a.stops.length)[0];
  const i = u8.stopNames.findIndex((n) => n.includes('Kottbusser Tor'));
  assert.equal(trainAdhesion(w, { pid: u8.id, s: u8.stops[i] }), 1, 'unter Tage trocken');
});
```

- [ ] **Step 2: Run** `node --test tests/trainphysics.test.js tests/playertrain.test.js` → FAIL.

- [ ] **Step 3: Implement**

  `web/src/trainphysics.js stepDrive`: nach `const k = …` `const adh = input.adhesion ?? 1;` und
  - `a = -k.emergency * adh` · `a = -k.brake * input.brake * adh` · Zugkraft `a = k.acc * adh * input.throttle * (…)`
  - `vAllowed = Math.sqrt(Math.max(0, 2 * k.emergency * adh * Math.max(0, lim - v * dt)))`
  - Deckel der aufgezeichneten Verzögerung `Math.min(k.emergency * adh, …)`.

  `web/src/playertrain.js` (Importe `roadCondition, adhesionOf` aus `./traction.js`, `undergroundAtS, railAt` aus `./tunnel.js`):

```js
// Schienenhaftung an der Zugspitze: oberirdisch aus dem Wetter (traction.js), im Tunnel immer trocken
export function trainAdhesion(w, t) {
  const p = w.city.transit.patterns[t.pid];
  if (undergroundAtS(w.city, p, t.s)) return 1;
  const h = pointOn(p, t.s), lvl = p.mode === 'tram' ? 0 : railAt(w.city, h.x, h.y)?.lvl ?? 0;
  return adhesionOf(roadCondition(w, h.x, h.y, lvl));
}
```

  und in `updatePlayerTrain` die Eingabe um `adhesion` erweitern:
  `const adhesion = trainAdhesion(w, t);` vor `const inp`, dann `{ throttle: …, brake: …, emergency: …, limit, adhesion }` bzw. `{ brake: 1, limit, adhesion }`.

  (Hinweis: die Brücken-Glätte ×1,5 wirkt über `lvl ≥ 1` auch auf Viadukte; `adhesionOf` begrenzt auf 0,6.)

- [ ] **Step 4: Run** `node --test tests/trainphysics.test.js tests/playertrain.test.js` → PASS.

- [ ] **Step 5: Mutationsproben**: `* adh` in `vAllowed` entfernen → Kurven-Test scheitert; Tunnel-Rückgabe `return 1` entfernen → Tunnel-Test scheitert (falls die Stelle zufällig trocken ist: im Test `w.wet = 1` vor der Tunnelzeile lassen – steht so).

- [ ] **Step 6: Commit** `git add web/src/trainphysics.js web/src/playertrain.js tests/trainphysics.test.js tests/playertrain.test.js && git commit -m "Player train: rail adhesion from weather, tunnels dry"`

---

### Task 6: HUD, Spritzwasser, Ton, Statistik

**Files:**
- Modify: `web/src/traction.js` (`roadWarning`), `web/src/hud.js` (Zeile mit `${icon} ${dayName(…)} ${formatClock(…)}` und Tacho-/Fahrerleiste), `web/src/render.js` (`handleEvents`, Rauch-Partikel ~Zeile 518, Partikel zeichnen ~Zeile 803), `web/src/audio.js` (`SYNTH`, `EVENT_SOUND`), `web/src/stats.js`
- Test: `tests/traction.test.js`, `tests/hud-layout.test.js`, `tests/audio.test.js`, `tests/stats.test.js`

**Interfaces:**
- Consumes: Ereignis `aquaplane`, `car.aqua`, `car.spin`, `w.temp`, `roadCondition`, `gustPush`, `GUST.warn`.
- Produces: `roadWarning(world) → 'Aquaplaning!' | 'Glätte' | 'Schnee' | 'Sturm' | 'Nässe' | null`; `hud.layout.roadWarn` (Rechteck); Ton `splash`; Partikel `spray`; Statistik `aquaplanes`.

- [ ] **Step 1: Failing tests**

  `tests/traction.test.js`:

```js
test('Warnschild: Vorrang Aquaplaning > Glätte > Schnee > Sturm > Nässe; zu Fuß keins', async () => {
  const { roadWarning } = await import('../web/src/traction.js');
  const w = createWorld({ city, cars: 0, pedestrians: 0 });
  const car = w.cars.find((c) => c.id === w.playerCarId) ?? (w.cars.push(createCar({ x: w.player.x, y: w.player.y })), w.cars.at(-1));
  w.player.inCar = null;
  w.wet = 1; assert.equal(roadWarning(w), null, 'zu Fuß: nichts');
  w.player.inCar = car.id; car.driver = 'player';
  w.wet = 0; w.snow = 0; w.ice = 0; w.weather = weatherAt(1, 0, 600, 'clear'); assert.equal(roadWarning(w), null);
  w.wet = 1; assert.equal(roadWarning(w), 'Nässe');
  w.snow = 0.5; assert.equal(roadWarning(w), 'Schnee');
  w.ice = 0.5; assert.equal(roadWarning(w), 'Glätte');
  car.aqua = 0.2; assert.equal(roadWarning(w), 'Aquaplaning!');
});
```

  (Falls die Welt ohne `cars` kein Spielerauto hat: wie im Code oben ein Auto anlegen; `roadWarning` prüft Glätte/Schnee/Nässe an der Autoposition über `roadCondition` – die Position des Autos muss auf einer nicht überdachten Fläche liegen; den Startplatz `places.playerCar` verwenden: `car.x = city.places.playerCar.x; car.y = city.places.playerCar.y;`.)

  `tests/hud-layout.test.js`:

```js
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
```

  `tests/audio.test.js`:

```js
test('Aquaplaning platscht', async () => {
  const { soundFor, SYNTH } = await import('../web/src/audio.js');
  assert.equal(soundFor({ type: 'aquaplane' }), 'splash');
  assert.ok(SYNTH.splash);
});
```

  `tests/stats.test.js`:

```js
test('Statistik: Aquaplaning nur mit dem Spielerauto', () => {
  const w = fakeWorld(), tr = createTracker(), S = sets();
  trackStep(S, tr, w, [], 0);
  trackStep(S, tr, w, [{ type: 'aquaplane', player: true }, { type: 'aquaplane', player: false }], 0);
  assert.equal(S[0].aquaplanes, 1);
});
```

- [ ] **Step 2: Run** `node --test tests/traction.test.js tests/hud-layout.test.js tests/audio.test.js tests/stats.test.js` → FAIL.

- [ ] **Step 3: Implement**

  `web/src/traction.js`:

```js
// Warnschild im HUD für das Fahrzeug des Spielers (Auto oder geführter Zug); zu Fuß und als Fahrgast keins
export function roadWarning(world) {
  const p = world.player, car = p.inCar ? world.cars.find((c) => c.id === p.inCar) : null;
  if (!car && p.ride?.kind !== 'driver') return null;
  if (car?.aqua > 0) return 'Aquaplaning!';
  const at = car ?? p, lvl = at.lvl ?? 0, c = roadCondition(world, at.x, at.y, lvl);
  if (c.ice > 0.2) return 'Glätte';
  if (c.snow > 0.2) return 'Schnee';
  const g = car ? gustPush(world, car, lvl) : null;
  if (g && Math.hypot(g.ax, g.ay) > GUST.warn) return 'Sturm';
  if (c.wet > 0.3) return 'Nässe';
  return null;
}
```

  `web/src/hud.js` (Import `roadWarning` aus `./traction.js`):
  - Uhrzeile: an den Text `${icon} ${dayName(world.day ?? 4)} ${formatClock(world.clock)}` anhängen: `` · ${Math.round(world.temp ?? 0)} °C`` (bei `-0` → `0`: `Math.round(world.temp ?? 0) || 0`).
  - Nach dem Fahrzeugzustand-Block (`if (car) { … }`) und vor `drawWeaponPanel`:

```js
    // Warnschild (Wetter an der Stelle): über dem Tacho, als Zugführer unter der Fahrerleiste
    const warn = roadWarning(world);
    if (warn) {
      const L = this.layout ?? {}, w = 250, h = 30;
      const x = car ? vw - m.x - w : (L.rideBar ? L.rideBar.x + L.rideBar.w - w : vw - m.x - w);
      const y = car ? vh - m.y - 106 - h - 10 : (L.rideBar ? L.rideBar.y + L.rideBar.h + 8 : m.y);
      const blink = warn === 'Aquaplaning!' && Math.floor(world.time * 6) % 2 === 0;
      this.layout = { ...L, roadWarn: { x, y, w, h } };
      c.fillStyle = blink ? 'rgba(255,80,60,0.85)' : 'rgba(255,190,40,0.85)'; rr(c, x, y, w, h, 8); c.fill();
      this.text('⚠', x + 14, y + 21, { size: 16, weight: 800, color: '#1a1a1a', shadow: false });
      this.text(warn, x + 38, y + 21, { size: 16, weight: 800, color: '#1a1a1a', shadow: false });
    }
```

  (Zeigt „Glätte“ als eigenen `fillText`-Aufruf – der Test sucht genau diesen Text.)

  `web/src/render.js`:
  - `handleEvents`: Fall `aquaplane` – 14 Partikel `{ kind: 'spray', x: e.x, y: e.y, vx, vy, life: 0.5, max: 0.5, r: 2 }` mit Richtungen aus `(i / 14) * 2π` und Tempo 40–90 (Darstellung darf `Math.random` nutzen wie die übrigen Partikel dort).
  - Bei den Auto-Partikeln (Rauch beschädigter Autos, ~Zeile 518): `if (c.spin && Math.random() < 0.5) this.particles.push({ kind: (world.snow ?? 0) > 0.2 || (world.ice ?? 0) > 0.2 ? 'snowdust' : 'spray', x: c.x - Math.cos(c.angle) * c.hw * 0.8, y: c.y - Math.sin(c.angle) * c.hw * 0.8, vx: -Math.cos(c.angle) * 40, vy: -Math.sin(c.angle) * 40, life: 0.4, max: 0.4, r: 2 });`
  - Partikel zeichnen (Schleife bei `// 9) Partikel`): `else if (p.kind === 'spray') { ctx.fillStyle = \`rgba(200,220,240,${a * 0.8})\`; ctx.fillRect(p.x - 1, p.y - 1, 2.5, 2.5); } else if (p.kind === 'snowdust') { ctx.fillStyle = \`rgba(250,250,255,${a * 0.9})\`; ctx.fillRect(p.x - 1, p.y - 1, 2.5, 2.5); }` vor dem letzten `else`.

  `web/src/audio.js`: in `SYNTH` `splash: (s) => { s.burst(0.35, { freq: 700, gain: 0.3 }); s.burst(0.18, { freq: 2200, gain: 0.12, type: 'bandpass' }); },` und in `EVENT_SOUND` `aquaplane: 'splash',`.

  `web/src/stats.js`: in `Unterwegs` `['aquaplanes', 'Aquaplaning', 'n']` nach `['bridges', …]`; in `trackStep` `case 'aquaplane': if (e.player) add(sets, 'aquaplanes'); break;`.

- [ ] **Step 4: Run** `node --test tests/traction.test.js tests/hud-layout.test.js tests/audio.test.js tests/stats.test.js` → PASS (die Statistikseite passt weiter in den 720er-Rahmen – bestehender Test).

- [ ] **Step 5: Mutationsproben**: Vorrang Glätte/Schnee vertauschen → Warnschild-Test scheitert; Warnschild-y ohne `- h - 10` → überlappt `car` → HUD-Test scheitert; `if (e.player)` entfernen → Statistik-Test scheitert.

- [ ] **Step 6: Commit** `git add web/src/traction.js web/src/hud.js web/src/render.js web/src/audio.js web/src/stats.js tests/traction.test.js tests/hud-layout.test.js tests/audio.test.js tests/stats.test.js && git commit -m "HUD temperature and road warning, spray, splash sound, aquaplaning stat"`

---

### Task 7: Doku, Browserprüfung, Release

**Files:**
- Modify: `README.md` (Abschnitt „Wetter und Fahren“ nach „Nahverkehr“, Konsolentabelle `glaette`/`temp`, Statistik-Text, „Stand und Prüfumfang“ + Testzahl), `docs/TECHNIK.md` (Abschnitt „Wetter auf der Straße“ nach „Mitfahren und selbst fahren“), `CLAUDE.md` (Architekturpunkt „Weather on the road“ nach dem Weather-Punkt), `CHANGELOG.md` (`## [0.30.0] – <Datum des Releases>`), `package.json`, `web/src/version.js`, `xbox/GtaBerlin/Package.appxmanifest`

- [ ] **Step 1: Volle Suite** `npm test > /tmp/suite.log 2>&1; grep -E "^# (tests|pass|fail)" /tmp/suite.log` → alles grün; Testzahl notieren.
- [ ] **Step 2: Doku**
  - README „Wetter und Fahren“: Faktoren in Worten (nass Bremsweg ≈ +30 %, Schnee ≈ doppelt, Glätte ≈ dreifach), Glätte aus Temperatur (Anzeige neben der Uhr), Aquaplaning an Pfützen ab 70 km/h, Böen (Brücken stärker), KI fährt vorsichtiger, eigener Zug bremst auf nassen/vereisten Schienen schlechter (Tunnel trocken), Warnschild; Konsolentabelle: `` `glaette 0.8` · `temp` · `temp -5` · `temp auto` `` | Glätte setzen, Temperatur zeigen/erzwingen/freigeben; Statistik-Satz um „Aquaplaning“ ergänzen; Prüfumfang-Punkt „Wetter-Fahrphysik“ mit den Tests dieser Etappe und der Mutationszahl.
  - TECHNIK: `temperatureAt`/`stepIce`, `traction.js` (Faktoren, Mischung mit Untergrenze, überdacht/Brücke, Pfützen aus `edgePuddles`, Böen aus `gustAt`), KI-Anpassung (Stillstandsabstand bleibt), Zughaftung, Grenzen.
  - CLAUDE.md: „**Weather on the road:** `weather.js temperatureAt` (pure) + `w.ice` (`stepIce`, saved; `w.forceTemp` via console `temp`) → `traction.js roadCondition/tractionOf` (grip factors, floor = ice value; covered spots dry, bridges ×1.5 ice; puddles = `edgePuddles`), written per step to `car.traction` by `world.js applyWeather` (also `car.aqua` aquaplaning once per puddle, `gustPush`); `car.js`, `traffic.js` (slower, earlier braking, longer following distance; standstill gap unchanged) and `trainphysics.js` (`input.adhesion` from `playertrain.js trainAdhesion`, tunnels = 1) read only factors. Missing `car.traction` = dry.“
  - CHANGELOG 0.30.0 (Neu: Fahrphysik je Wetter, Glätte/Temperatur, Aquaplaning, Böen, KI, Zug, HUD, Konsole, Statistik).
  - Versionen: 0.30.0 in `package.json`, `web/src/version.js`, `Identity Version="0.30.0.0"`. `node --test tests/version.test.js` → PASS.
- [ ] **Step 3: Browser** (`PORT=8091 npm start`, PID merken): Neues Spiel; Konsole `wetter regen`, `nass 1`, Auto fahren und aus 50 km/h bremsen (fühlbar länger), durch eine Pfütze mit > 70 km/h (Spritzwasser, Warnschild blinkt); `temp -5`, `glaette 1` → Warnschild „Glätte“, Anfahren dreht durch; `wetter sturm` auf der Oberbaumbrücke (Versatz spürbar); Uhrzeile mit Temperatur. Screenshots nach `.playwright-mcp/`. Server per PID beenden.
- [ ] **Step 4: Commit, Tag, Push**

```bash
git add -A web/src tests README.md CHANGELOG.md docs/TECHNIK.md CLAUDE.md package.json xbox/GtaBerlin/Package.appxmanifest
git commit -m "Weather driving physics: grip, black ice, aquaplaning, gusts, cautious AI, rail adhesion (0.30.0)"
git tag -a v0.30.0 -m "0.30.0" && git push --follow-tags
```
