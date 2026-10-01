// Spielwelt: verbindet Stadt, Spieler, Autos, Passanten und Mission zu einem Simulationsschritt.
// Enthält kein DOM – Eingaben kommen als abstrakter Zustand (siehe input.js), Ausgaben als Ereignisse.
import { PLAYER, STAMINA, PED, TRAFFIC, CAR, PARKED, CLOCK } from './config.js';
import { clamp, damp } from './math.js';
import { mulberry32 } from './rng.js';
import { circleVsRect, circleVsCircle, circleVsObb, circleVsSegment, obbVsRect, obbVsObb, obbVsSegment, obbBounds } from './collision.js';
import { createCar, stepCar, collideCarWorld, collideCars, speedOf, forwardSpeed, CAR_COLORS, damage, blocks } from './car.js';
import { placeOnLane, spawnSpot, driveAi, claimNarrow, narrowFree, dropClaims } from './traffic.js';
import { createPed, updatePed, scare, knockDown, nearestSpot, pedSpawnSpot } from './pedestrians.js';
import { createMission, updateMission, resetMission } from './mission.js';
import { insideBorder, inBuilding, locationName, hash01, surfaceAt, bezirkAt, T } from './map.js';
import { parkingStrip } from './street.js';
import { PARK } from './citycodes.js';
import { pointAlong } from './geom.js';
import { buildLaneGraph, nearestLane } from './roadgraph.js';
import { sidewalkPoint } from './pedestrians.js';
import { resolveSave } from './save.js';
import { findFootPath } from './footpath.js';
import { initCombat, updatePlayerCombat, clickIntent, CLICK, WEAPONS, GUNSHOT_SCARE, BODY_KEEP, hurtPlayer, isFighter, startFight, RESPAWN_DELAY, HOSPITAL_FEE, PLAYER_HP } from './combat.js';
import { failMission } from './mission.js';
import { populationTargets, START_DAY } from './rhythm.js';
import { lifeSpots, walkerStyle, LIFE } from './life.js';
import { pickKind, KINDS, isBikeKind, isMotoKind, isOpenKind } from './fleet.js';
import { pickKind as pickPersonKind, KINDS as PERSON_KINDS } from './figure.js';
import { updateService, manageEmergency } from './services.js';
import { createBike, updateBike, bikeSpawn, BIKE, riderShirt, dismount } from './bikes.js';
import { manageAnimals, updateAnimals } from './animals.js';
import { weatherAt, stepWet, stepSnow, peopleFactor, bikeFactor, temperatureAt, stepIce } from './weather.js';
import { roadCondition, tractionOf, puddleAt, gustPush, aquaYaw, AQUA, DRY } from './traction.js';
import { updateTransit } from './transitlive.js';
import { pointOn, tramTrackNear, TRAIN } from './transit.js';
import { vehicleState, transitNear, alightSpot, stationExit, spotFreeHere, RIDE, elevated } from './ride.js';
import { takeTrain, updatePlayerTrain, leaveTrain, turnAround, atTerminus } from './playertrain.js';
import { stepLevel, initialLevel, touch } from './levels.js';
import { entranceNear } from './station.js';
import { buildGrid, near } from './grid.js';
import { stationsNear, stationById, keepInside, stairAt, arrivalAt, boardable, toLocal, toWorld, STATION, stationName } from './station.js';

// city: dekodierte Karte (map.js decodeCity). cars/pedestrians: Zielbevölkerung um die Kamera.
export function createWorld({ city, seed = 1989, cars = TRAFFIC.cars, pedestrians = TRAFFIC.pedestrians } = {}) {
  if (!city) throw new Error('createWorld braucht eine Karte (city)');
  const rng = mulberry32(seed + 7);
  const w = {
    city, rng, solids: city.solids, cars: [], peds: [], bikes: [], animals: [], events: [], time: 0, clock: CLOCK.start, day: START_DAY, dayCount: 0, seed, wet: 0, snow: 0, ice: 0, temp: 0, forceTemp: null, forceWeather: null, esp: true, abs: true,
    player: { x: 0, y: 0, angle: 0, inCar: null, step: 0, stun: 0 },
    playerCarId: null,
    mission: createMission(),
    money: 0, completed: 0, bestTime: null,
    camera: { x: 0, y: 0, zoom: 1 },
    prompt: null, notice: null, vehInfo: null,
  };

  initCombat(w.player);
  spawnPlayerAndCar(w);
  const pc = city.parked[0];
  w.cars.push(createCar({ x: pc.x, y: pc.y, angle: pc.angle, color: '#16a085', role: 'parked' }));

  w.carTarget = cars; w.pedTarget = pedestrians;
  w.temp = temperatureAt(w.seed, w.dayCount, w.clock, w.forceWeather); // schon vor dem ersten Schritt richtig
  // Tagesrhythmus nur bei der Standardbevölkerung (Tests und Titel-Demo geben feste Zahlen vor)
  w.rhythm = cars === TRAFFIC.cars && pedestrians === TRAFFIC.pedestrians;
  // Wetter ebenso nur dort (sonst immer klar – feste Bilder für Tests und Titel)
  w.weather = weatherAt(seed, 0, w.clock, w.rhythm ? null : 'clear');
  w.camera.x = w.player.x; w.camera.y = w.player.y;
  w.focusKey = `world${++worldCount}`;
  w.loading = !city.focus(w.focusKey, w.camera.x, w.camera.y);
  if (!w.loading) populate(w);
  return w;
}
let worldCount = 0;

// Verkehr, Passanten und Parker verwerfen (nach einem Ortswechsel); am neuen Ort baut streamWorld sie neu auf.
export function resetPopulation(w, keepCar = null) {
  w.cars = w.cars.filter((c) => {
    const keep = c === keepCar || c.id === w.playerCarId || c.id === w.player.inCar || c.cargo || c.role === 'parked';
    if (!keep && c.parkKey) w.parkedKeys?.delete(c.parkKey); // Stellplatz wieder frei
    return keep;
  });
  w.peds = [];
  w.bikes = [];
  w.animals = []; w.flocks?.clear();
  w.transit = null; w._transitPopulated = false;
  w.hangers?.clear();
  if (w.emerg) w.emerg.incidents.length = 0;
  w.populated = false;
}

// Startbevölkerung: im ganzen Umkreis verteilt (auch im Bild), danach nur noch außerhalb der Sicht.
function populate(w) {
  w.populated = true;
  if (w.rhythm) { setTargets(w); w._rhythmT = w.time; }
  manageLife(w, true);
  manageAnimals(w, true);
  for (let k = 0; k < w.carTarget; k++) spawnTraffic(w, 120, TRAFFIC.spawnMax);
  for (let k = 0; k < w.pedTarget; k++) spawnPed(w, 60, TRAFFIC.spawnMax);
  for (let k = 0; k < bikeTarget(w); k++) spawnBike(w, 120, TRAFFIC.spawnMax);
}

// Radfahrer und E-Roller: ein Anteil der Fußgänger-Zielzahl (nur mit Tagesrhythmus)
const bikeTarget = (w) => (w.rhythm ? Math.round(w.pedTarget * BIKE.share * bikeFactor(w.weather)) : 0);
// Zielbevölkerung aus Tagesrhythmus und Ort, bei Regen und Nebel gehen weniger Menschen raus
function setTargets(w) {
  const t = populationTargets(w.city, w.camera.x, w.camera.y, w.clock, w.day, TRAFFIC);
  w.carTarget = Math.round(t.cars * (w.trafficScale ?? 1)); w.pedTarget = Math.max(w.pedScale === 0 ? 0 : 4, Math.round(t.peds * peopleFactor(w.weather) * (w.pedScale ?? 1))); // Dichte (Konsole: verkehr, passanten)
}
function spawnBike(w, minR, maxR) {
  const sp = bikeSpawn(w.city, w.rng, w.camera.x, w.camera.y, minR, maxR);
  if (!sp) return null;
  const b = createBike(w.city, sp.lane, sp.s, w.rng, w.rng() < BIKE.scooterShare ? 'scooter' : 'bike');
  if (w.cars.some((c) => Math.abs(c.x - b.x) < c.hw + 12 && Math.abs(c.y - b.y) < c.hw + 12) || w.bikes.some((o) => Math.hypot(o.x - b.x, o.y - b.y) < 30)) return null;
  w.bikes.push(b);
  return b;
}

// Kacheln um die Kamera nachladen. false = der Stadtteil hier ist noch nicht da (im Browser kommt er asynchron),
// die Welt steht dann still, bis er geladen ist.
export function streamWorld(w) {
  w.loading = !w.city.focus(w.focusKey, w.camera.x, w.camera.y);
  if (!w.loading && !w.populated) populate(w);
  return !w.loading;
}

function spawnPed(w, minR, maxR) {
  const sp = pedSpawnSpot(w.city, w.rng, w.camera.x, w.camera.y, minR, maxR);
  if (!sp) return null;
  const ped = createPed(w.city, sp, w.rng);
  if (w.rhythm) { // Jogger und Hundehalter je nach Tageszeit
    ped.style = walkerStyle(w.clock, w.rng);
    if (ped.style === 'jog') ped.shirt = ['#e84393', '#00b894', '#0984e3', '#fdcb6e', '#d63031'][ped.id % 5];
  }
  assignKind(w, ped);
  w.peds.push(ped);
  return ped;
}

// Menschen-Typ (figure.js): Jogger/Gassigeher aus ihrem Stil, sonst nach Ort, Uhrzeit, Wochentag (und Tätigkeit) aus der
// Nummer – deterministisch, ohne den Welt-Zufall zu verbrauchen. Der Typ bestimmt das Gehtempo mit.
export function assignKind(w, ped, act = null) {
  ped.kind = ped.style === 'jog' ? 'jogger' : ped.style === 'dog' ? 'dogwalker'
    : pickPersonKind(ped.id, { minutes: w.clock, day: w.day, bezirk: bezirkAt(w.city, ped.x, ped.y), act });
  ped.speed *= PERSON_KINDS[ped.kind]?.speed ?? 1;
  return ped;
}

// Stadtleben: Passanten mit Tätigkeit an ihren Plätzen halten (life.js). Neue entstehen nur außer Sicht, außer direkt
// nach dem Aufbau eines Ortes (Spielbeginn, Teleport); wer nicht mehr gebraucht wird, geht außer Sicht wieder.
export function manageLife(w, all = false) {
  if (!w.rhythm) return;
  if (!all && w.time - (w._lifeT ?? -99) < LIFE.every) return;
  w._lifeT = w.time;
  const cam = w.camera, hangers = (w.hangers ??= new Map());
  const inView = (x, y) => Math.abs(x - cam.x) < LIFE.viewHalfX && Math.abs(y - cam.y) < LIFE.viewHalfY;
  const want = new Map(lifeSpots(w.city, cam.x, cam.y, w.clock, w.day).map((s) => [s.key, s]));
  for (const [key, ped] of hangers) {
    const gone = !w.peds.includes(ped) || ped.state !== 'hang';
    if (gone) { hangers.delete(key); if (ped.hang) ped.hang.released = true; continue; }
    const far = Math.hypot(ped.x - cam.x, ped.y - cam.y) > LIFE.despawn;
    if (far || (!want.has(key) && !inView(ped.x, ped.y))) { hangers.delete(key); w.peds.splice(w.peds.indexOf(ped), 1); }
  }
  for (const [key, s] of want) {
    if (hangers.has(key) || hangers.size >= LIFE.maxHangers) continue;
    if (!all && inView(s.x, s.y)) continue;
    const sp = nearestSpot(w.city, s.x, s.y);
    if (!sp) continue;
    const ped = createPed(w.city, sp, w.rng);
    Object.assign(ped, { x: s.x, y: s.y, facing: s.face, state: 'hang', hang: { ...s } });
    assignKind(w, ped, s.act);
    hangers.set(key, ped); w.peds.push(ped);
  }
}

// Bevölkerung um die Kamera halten: Fernes abbauen, Fehlendes im Ring außerhalb der Sicht erzeugen.
function managePopulation(w) {
  const cam = w.camera, far = TRAFFIC.despawn;
  if (w.rhythm && !(w.time - (w._rhythmT ?? -99) < 2)) { // Tageszeit und Ort bestimmen, wie viel los ist
    w._rhythmT = w.time;
    setTargets(w);
  }
  const keep = (c) => c.id === w.playerCarId || c.id === w.player.inCar || c.cargo || c.role === 'parked' || c.role === 'curb' || c.driver === 'player' || (c.duty && !c.done);
  // Festgefahrene KI-Autos außerhalb des Bildes abbauen (sie entstehen anderswo neu), damit sich nirgends ein Knoten hält
  const stuck = (c) => c.driver === 'npc' && c.ai && ((c.ai.stillT ?? 0) > 30 || (c.ai.headOn ?? 0) >= 4) && Math.hypot(c.x - cam.x, c.y - cam.y) > 1100;
  w.cars = w.cars.filter((c) => keep(c) || (Math.hypot(c.x - cam.x, c.y - cam.y) < far && !stuck(c)));
  w.peds = w.peds.filter((p) => Math.hypot(p.x - cam.x, p.y - cam.y) < far);
  const npc = w.cars.filter((c) => !c.duty && (c.driver === 'npc' || (c.driver === null && c.role === 'traffic'))).length;
  if (npc < w.carTarget) spawnTraffic(w, TRAFFIC.spawnMin, TRAFFIC.spawnMax);
  // weniger los als eben (Tageszeit, anderer Ort): Überzählige außer Sicht verschwinden lassen, eins je Schritt
  if (w.rhythm && npc > w.carTarget + 2) {
    const i = w.cars.findIndex((c) => c.driver === 'npc' && !keep(c) && Math.hypot(c.x - cam.x, c.y - cam.y) > TRAFFIC.spawnMin);
    if (i >= 0) { dropClaims(w.cars[i], w); w.cars.splice(i, 1); }
  }
  const walkers = w.peds.filter((q) => q.state === 'walk' && !q.hang);
  if (w.rhythm && walkers.length > w.pedTarget + 4) {
    const q = walkers.find((o) => Math.hypot(o.x - cam.x, o.y - cam.y) > TRAFFIC.spawnMin);
    if (q) w.peds.splice(w.peds.indexOf(q), 1);
  }
  if (w.peds.filter((q) => q.state !== 'dead' && !q.hang).length < w.pedTarget) spawnPed(w, TRAFFIC.spawnMin * 0.8, TRAFFIC.spawnMax);
  // Räder: Fernes und Liegengebliebenes außer Sicht abbauen, Fehlendes im Ring erzeugen
  w.bikes = w.bikes.filter((b) => b.state !== 'gone' && Math.hypot(b.x - cam.x, b.y - cam.y) < far && !(b.state === 'lying' && b.t > 30 && Math.hypot(b.x - cam.x, b.y - cam.y) > 900));
  const riding = w.bikes.filter((b) => b.state === 'ride').length, bt = bikeTarget(w);
  if (riding < bt) spawnBike(w, TRAFFIC.spawnMin, TRAFFIC.spawnMax);
  else if (riding > bt + 2) { const i = w.bikes.findIndex((b) => Math.hypot(b.x - cam.x, b.y - cam.y) > TRAFFIC.spawnMin); if (i >= 0) w.bikes.splice(i, 1); }
}

function spawnPlayerAndCar(w) {
  const { places } = w.city;
  w.player.x = places.playerSpawn.x; w.player.y = places.playerSpawn.y;
  w.player.inCar = null; w.player.angle = -Math.PI / 2;
  let car = w.cars.find((c) => c.id === w.playerCarId);
  if (!car) {
    car = createCar({ x: 0, y: 0, color: '#c0392b', role: 'player' });
    w.cars.push(car);
    w.playerCarId = car.id;
  }
  Object.assign(car, {
    x: places.playerCar.x, y: places.playerCar.y, angle: places.playerCar.angle,
    vx: 0, vy: 0, angVel: 0, health: CAR.health, wrecked: false, wreckT: 0, cargo: false, driver: null, ai: null,
  });
  car.controls = { throttle: 0, brake: 0, steer: 0, handbrake: false };
}

function spawnTraffic(w, minR, maxR) {
  for (let tries = 0; tries < 8; tries++) {
    const sp = spawnSpot(w.city, w.rng, w.camera.x, w.camera.y, minR, maxR);
    if (!sp) return null;
    if (!w.cars.every((o) => Math.hypot(o.x - sp.x, o.y - sp.y) > 70) || !narrowFree(w, sp.lane)) continue;
    // Hauptstraßen mit viel gezähltem Verkehr bekommen mehr Autos als stille Nebenstraßen
    if (w.rng() > Math.min(1, Math.max(0.12, (sp.lane.edge.dtv ?? 8000) / 15000))) continue;
    // Fahrzeugart nach Uhrzeit, Wochentag und Straße (nur mit Tagesrhythmus; höchstens ein Müllauto in der Nähe)
    let kind = w.rhythm ? pickKind(w.clock, w.day, sp.lane.edge.cls, w.rng()) : 'car';
    if (kind === 'garbage' && w.cars.some((o) => o.kind === 'garbage')) kind = 'car';
    const pal = KINDS[kind].colors ?? CAR_COLORS;
    if (kind !== 'car' && !w.cars.every((o) => Math.hypot(o.x - sp.x, o.y - sp.y) > 110)) continue;
    const car = createCar({ x: sp.x, y: sp.y, kind, color: pal[Math.floor(w.rng() * pal.length)] });
    placeOnLane(car, w.city, sp.lane, sp.s, w.rng);
    car.driver = 'npc';
    claimNarrow(w, car, sp.lane); // auf einer Engstelle geboren: Richtung gleich belegen
    w.cars.push(car);
    return car;
  }
  return null;
}

// Teleport-Ziel zu einem Kartenpunkt: im Auto auf die nächste Fahrspur (in Fahrtrichtung), zu Fuß auf den nächsten
// Gehweg. null, wenn der Punkt außerhalb des Spielgebiets liegt oder nichts Passendes in der Nähe ist.
// Liegt das Ziel in einem noch nicht geladenen Stadtteil, kommt { pending: true } zurück (später erneut fragen).
export function findTeleportSpot(w, x, y) {
  const city = w.city;
  if (!insideBorder(city, x, y)) return null;
  if (!city.focus('teleport', x, y)) return { pending: true, x, y };
  // Abseits der Fahrbahn (Park, Feld, Platz, Hof): genau dorthin bzw. an die nächste freie Stelle bis 30 m –
  // sonst landete man an der nächsten Straße, auf dem Tempelhofer Feld also über einen Kilometer daneben.
  const ground = surfaceAt(city, x, y);
  let spot = ground === T.GRASS || ground === T.PLAZA || ground === T.SIDEWALK ? openSpot(w, x, y, !!playerCar(w)) : null;
  if (spot) { spot.name = locationName(city, spot.x, spot.y); return spot; }
  if (playerCar(w)) {
    const hit = nearestLane(buildLaneGraph(city), x, y, undefined, 3000);
    if (hit) {
      const p = hit.lane.pts, i = hit.i;
      spot = { x: hit.x, y: hit.y, angle: Math.atan2(p[i + 3] - p[i + 1], p[i + 2] - p[i]) };
    }
  } else {
    const sp = nearestSpot(city, x, y, 3000);
    if (sp) { const p = sidewalkPoint(city, sp.edge, sp.side, sp.s); spot = { x: p.x, y: p.y, angle: 0 }; }
  }
  if (!spot || !insideBorder(city, spot.x, spot.y) || inBuilding(city, spot.x, spot.y)) return null;
  spot.name = locationName(city, spot.x, spot.y);
  return spot;
}

// Freie Stelle auf offenem Grund (kein Haus, kein Wasser, kein Hindernis) um (x, y), zuerst der Punkt selbst
export function openSpot(w, x, y, car) {
  const city = w.city, S = city.scale;
  const free = (px, py, angle) => {
    if (!insideBorder(city, px, py)) return false;
    const t = surfaceAt(city, px, py);
    if (t === T.BUILDING || t === T.WATER || inBuilding(city, px, py)) return false;
    if (!car) return spotFree(w, px, py, PLAYER.radius + 2);
    const probe = { x: px, y: py, angle, hw: CAR.length / 2 + 4, hh: CAR.width / 2 + 4 };
    for (const s of w.solids.query(obbBounds(probe), [])) {
      if (!blocks(w, s, 0)) continue;
      if (s.seg ? obbVsSegment(probe, s) : s.r !== undefined ? circleVsObb(s.x, s.y, s.r, probe) : obbVsRect(probe, s)) return false;
    }
    // die ganze Karosserie auf festem Grund (nicht halb im Wasser)
    for (const [u, v] of [[1, 1], [-1, 1], [1, -1], [-1, -1]]) {
      const cx = px + Math.cos(angle) * u * probe.hw - Math.sin(angle) * v * probe.hh, cy = py + Math.sin(angle) * u * probe.hw + Math.cos(angle) * v * probe.hh;
      if (surfaceAt(city, cx, cy) === T.WATER) return false;
    }
    return w.cars.every((c) => c.id === w.player.inCar || Math.hypot(c.x - px, c.y - py) > 40);
  };
  const angles = car ? [0, Math.PI / 2, Math.PI / 4, -Math.PI / 4] : [0];
  for (let r = 0; r <= 30 * S; r += 2 * S) {
    const n = r ? Math.max(8, Math.round(2 * Math.PI * r / (2 * S))) : 1;
    for (let k = 0; k < n; k++) {
      const px = x + Math.cos(k / n * 2 * Math.PI) * r, py = y + Math.sin(k / n * 2 * Math.PI) * r;
      for (const a of angles) if (free(px, py, a)) return { x: px, y: py, angle: a };
    }
  }
  return null;
}

export function teleportTo(w, spot) {
  const car = playerCar(w), p = w.player;
  if (car) Object.assign(car, { x: spot.x, y: spot.y, angle: spot.angle, vx: 0, vy: 0, angVel: 0, lvl: undefined });
  if (p.ride) endRide(w, 'teleport'); // Teleport (Karte, Konsole) beendet eine Fahrt
  p.inside = null; p.entryGuard = { x: spot.x, y: spot.y }; // aus dem U-Bahnhof hinaus; landet man auf einem Eingang, nicht gleich hinunter
  p.x = spot.x; p.y = spot.y; p.lvl = undefined; // Ebene neu von der Landestelle
  w.camera.x = spot.x; w.camera.y = spot.y;
  // Verkehr und Passanten sofort am neuen Ort aufbauen (sonst wäre die Straße einige Sekunden leer).
  resetPopulation(w, car);
  streamWorld(w);
  w.city.release('teleport');
}

// --- Geparkte Autos am Straßenrand -------------------------------------------------------
const SLOT_M = { parallel: 5.6, diagonal: 3.0, perpendicular: 2.6 };

// Stellplätze einer Kante (deterministisch, einmal berechnet): Mitte des Parkstreifens, Autoausrichtung je Aufstellung.
// Stellplatz näher am Straßenbahngleis als halbe Bahn + halbes Auto + Luft: dort stünde das Auto im Weg jeder Bahn
const TRAM_CLEAR = TRAIN.tram.W / 2 + CAR.width / 2 + 3;
export function parkingSlots(city, e) {
  if (e._slots && e._slotsTr === city.transit) return e._slots; // Fahrplan nachgeladen: neu (Gleise, s. u.)
  const S = city.scale, slots = [];
  if (e.inside && e.cls <= 8 && !e.bridge) {
    // StVO § 12: kein Parken bis 5 m vor/nach der Ecke einer Kreuzung (Ecke = halbe Breite der Querstraße vom Knoten)
    const cornerGap = (n) => { const nd = city.nodes.get(n); if (!nd) return 5 * S; let r = 0; for (const k of nd.edges) { const o = city.edges.get(k); if (o && o !== e) r = Math.max(r, o.w / 2); } return nd.edges.length > 2 ? r + 5 * S : 2 * S; };
    const m0 = cornerGap(e.a), m1 = cornerGap(e.b), p = { x: 0, y: 0, ux: 1, uy: 0 };
    const avoid = [city.places.dropoff, city.places.playerCar, city.places.pickup, ...(city.parked ?? [])];
    for (const side of [-1, 1]) {
      const ps = parkingStrip(e.cs, side);
      if ((ps.kind !== PARK.lane && ps.kind !== PARK.half) || ps.depth < 0.9 * S) continue;
      const step = SLOT_M[ps.orient] * S;
      for (let i = 0, s = m0 + step / 2; s < e.len - m1 - step / 2; i++, s += step) {
        if (hash01(e.id * 977 + (side + 1) * 31 + i * 7919) >= PARKED.share) continue;
        pointAlong(e.pts, s, p);
        const x = p.x - p.uy * ps.offset, y = p.y + p.ux * ps.offset;
        if (avoid.some((q) => q && Math.hypot(q.x - x, q.y - y) < 12 * S)) continue;
        if (city.transit && tramTrackNear(city.transit, x, y, TRAM_CLEAR)) continue; // nicht aufs Straßenbahngleis
        let angle = Math.atan2(p.uy, p.ux) + (side < 0 ? Math.PI : 0);          // parallel in Fahrtrichtung der Seite
        if (ps.orient === 'perpendicular') angle += side * Math.PI / 2;
        else if (ps.orient === 'diagonal') angle += side * Math.PI / 4;
        slots.push({ key: `${e.id}:${side}:${i}`, x, y, angle });
      }
    }
  }
  e._slots = slots; e._slotsTr = city.transit;
  return slots;
}

function slotFree(w, slot) {
  const probe = { x: slot.x, y: slot.y, angle: slot.angle, hw: CAR.length / 2, hh: CAR.width / 2 };
  for (const s of w.solids.query(obbBounds(probe), [])) {
    if (!blocks(w, s, 0)) continue;
    const m = s.seg ? obbVsSegment(probe, s) : s.r !== undefined ? circleVsObb(s.x, s.y, s.r, probe) : obbVsRect(probe, s);
    if (m && m.depth > 1) return false;
  }
  return w.cars.every((c) => Math.hypot(c.x - slot.x, c.y - slot.y) > 30);
}

// Parkende Autos im Umkreis der Kamera erzeugen, ferne wieder abbauen (Stellplatz wird dann wieder frei).
function manageParked(w) {
  const cam = w.camera;
  w.parkedKeys ??= new Set();
  w.cars = w.cars.filter((c) => {
    if (c.role !== 'curb' || c.driver === 'player' || c.id === w.playerCarId || c.cargo) return true;
    if (Math.hypot(c.x - cam.x, c.y - cam.y) < PARKED.despawn) return true;
    w.parkedKeys.delete(c.parkKey);
    return false;
  });
  if ((w.parkTick = (w.parkTick ?? 0) + 1) % 20 !== 1) return;
  const R = PARKED.radius, seen = new Set();
  for (const sg of w.city.edgeSegs.query({ x: cam.x - R, y: cam.y - R, w: 2 * R, h: 2 * R }, [])) {
    if (seen.has(sg.e)) continue;
    seen.add(sg.e);
    for (const slot of parkingSlots(w.city, sg.e)) {
      if (w.parkedKeys.has(slot.key) || Math.hypot(slot.x - cam.x, slot.y - cam.y) > R || !slotFree(w, slot)) continue;
      const car = createCar({ x: slot.x, y: slot.y, angle: slot.angle, color: CAR_COLORS[Math.floor(hash01(slot.x * 31 + slot.y) * CAR_COLORS.length)], role: 'curb' });
      car.parkKey = slot.key; car.controls.handbrake = true;
      w.parkedKeys.add(slot.key);
      w.cars.push(car);
    }
  }
}

export function playerCar(w) { return w.cars.find((c) => c.id === w.player.inCar) ?? null; }

// Mission neu starten: Spieler zum Späti, eigenes Auto repariert zurück auf den Parkplatz.
export function restartMission(w) {
  if (w.player.ride) endRide(w, 'teleport'); // Fahrgast/Zugführer: Neustart holt ihn aus dem Fahrzeug (Startpunkt s. u.)
  Object.assign(w.player, { inside: null, entryGuard: null, click: null, lvl: undefined }); // auch aus dem U-Bahnhof hinaus
  for (const c of w.cars) c.cargo = false;
  resetMission(w.mission);
  const far = Math.hypot(w.camera.x - w.city.places.playerSpawn.x, w.camera.y - w.city.places.playerSpawn.y) > TRAFFIC.despawn;
  spawnPlayerAndCar(w);
  w.camera.x = w.player.x; w.camera.y = w.player.y;
  if (far) { resetPopulation(w); streamWorld(w); }
}

const tmp = [];
function pushCircleOutOfWorld(w, obj, r) {
  const box = { x: obj.x - r - 2, y: obj.y - r - 2, w: 2 * r + 4, h: 2 * r + 4 };
  for (const s of w.solids.query(box, tmp)) {
    if (!blocks(w, s, obj.lvl)) continue;
    const m = s.seg ? circleVsSegment(obj.x, obj.y, r, s) : s.r !== undefined ? circleVsCircle(obj.x, obj.y, r, s.x, s.y, s.r) : circleVsRect(obj.x, obj.y, r, s);
    if (m) { obj.x += m.nx * m.depth; obj.y += m.ny * m.depth; }
  }
}

function spotFree(w, x, y, r, ignoreCar, lvl = 0) {
  const box = { x: x - r, y: y - r, w: 2 * r, h: 2 * r };
  for (const s of w.solids.query(box, tmp)) {
    if (!blocks(w, s, lvl)) continue;
    const m = s.seg ? circleVsSegment(x, y, r, s) : s.r !== undefined ? circleVsCircle(x, y, r, s.x, s.y, s.r) : circleVsRect(x, y, r, s);
    if (m) return false;
  }
  return w.cars.every((c) => c === ignoreCar || !circleVsObb(x, y, r, c));
}

// only: genau dieses Auto oder Rad (Klick-Steuerung), sonst das nächste in Reichweite; quiet: Türgeräusch kam schon.
// Räder (w.bikes) zählen mit: ein fahrendes wird gekapert (Fahrer runter, flieht), ein liegendes aufgehoben.
function tryEnter(w, only = null, { quiet = false } = {}) {
  const p = w.player;
  let best = null, bd = PLAYER.enterDist;
  const bikes = only ? (w.bikes.includes(only) ? [only] : []) : w.bikes;
  for (const c of only ? (w.cars.includes(only) ? [only] : []) : w.cars) {
    if (c.wrecked) continue;
    const d = Math.hypot(c.x - p.x, c.y - p.y);
    if (d < bd) { bd = d; best = c; }
  }
  for (const b of bikes) {
    if (b.state !== 'ride' && b.state !== 'lying') continue;
    const d = Math.hypot(b.x - p.x, b.y - p.y);
    if (d < Math.min(bd, BIKE_GRAB)) { bd = d; best = b; }
  }
  if (best && w.bikes.includes(best)) best = takeBike(w, best);
  if (!best) return false;
  if (best.driver === 'npc') {
    // Fahrer steigt aus und flieht.
    const ped = fleeingDriver(w, best, p.x, p.y, 3.5);
    if (ped) w.peds.push(ped);
    w.events.push({ type: 'carjack', x: best.x, y: best.y });
  }
  best.driver = 'player'; best.ai = null;
  best.controls = { throttle: 0, brake: 0, steer: 0, handbrake: false };
  best.dyn = null; best.fallen = false; // Fahrdynamik beginnt mit geradem Lenkrad; ein umgefallenes Zweirad wird aufgerichtet
  if (!isBikeKind(best.kind) && !quiet) w.vehInfo = { carId: best.id, t: 0 }; // HUD blendet Name und Technik ein (wie GTA: ohne Kasten)
  p.inCar = best.id;
  if (best.role !== 'player' && !isOpenKind(best.kind) && !w.cars.some((c) => c.id === w.playerCarId && !c.wrecked)) w.playerCarId = best.id; // Zweiräder nie
  if (!quiet) w.events.push({ type: 'door', x: best.x, y: best.y });
  return true;
}

function sideSpot(car, side, extra) {
  const rx = -Math.sin(car.angle), ry = Math.cos(car.angle);
  const d = car.hh + PLAYER.radius + extra;
  return { x: car.x + rx * d * side, y: car.y + ry * d * side };
}

// Raster für Nachbarschaftsfragen (grid.js): Zelle 128 px; ein Passant kann nur Autos berühren, deren Mitte näher als
// halbe Länge + halbe Breite + Radius liegt – beim Bus (12 m) ≈ 80 px
const GRID_CELL = 128, GRID_REACH = 90;

// Einblendung von Name und Technik nach dem Einsteigen (Sekunden, HUD blendet aus)
export const VEH_INFO_S = 4.5;

// Abwurf vom Zweirad: die Maschine fällt um und rutscht aus, der Fahrer landet daneben, benommen und verletzt
export const MOTO = { throwAt: 0.25, stun: 1.2, hurt: [8, 45] };
function throwRider(w, car, strength) {
  const p = w.player, a = car.angle + Math.PI / 2 * (car.id % 2 ? 1 : -1);
  car.driver = null; car.dyn = null; car.fallen = true; car.controls = { throttle: 0, brake: 0, steer: 0, handbrake: true };
  p.inCar = null; p.lvl = car.lvl; p.angle = car.angle;
  p.x = car.x + Math.cos(a) * (car.hh + PLAYER.radius + 4); p.y = car.y + Math.sin(a) * (car.hh + PLAYER.radius + 4);
  pushCircleOutOfWorld(w, p, PLAYER.radius);
  p.stun = MOTO.stun + strength;
  w.events.push({ type: 'thrown', x: p.x, y: p.y, carId: car.id, player: true });
  hurtPlayer(w, MOTO.hurt[0] + strength * MOTO.hurt[1], car.x, car.y);
}

function tryExit(w) {
  const car = playerCar(w);
  if (!car) return false;
  const fx = Math.cos(car.angle), fy = Math.sin(car.angle);
  const cands = [sideSpot(car, -1, 3), sideSpot(car, 1, 3),
    { x: car.x - fx * (car.hw + 10), y: car.y - fy * (car.hw + 10) },
    { x: car.x + fx * (car.hw + 10), y: car.y + fy * (car.hw + 10) }];
  const spot = cands.find((s) => spotFree(w, s.x, s.y, PLAYER.radius, car));
  if (!spot) { w.notice = { text: 'Kein Platz zum Aussteigen', t: 1.5 }; return false; }
  car.driver = null; car.dyn = null; // Fahrdynamik endet: kein Nicken/Einschlag, Motorklang wieder nach Art
  car.controls = { throttle: 0, brake: 0, steer: 0, handbrake: speedOf(car) < 60 };
  w.player.inCar = null;
  w.player.x = spot.x; w.player.y = spot.y;
  w.events.push({ type: 'door', x: car.x, y: car.y });
  return true;
}

// Rad kapern: aus dem Radfahrer (bikes.js) wird ein Fahrzeug der Art 'bicycle'/'escooter' (fleet.js), das die
// Spielfigur mit der Autophysik fährt; ein Fahrer wird heruntergezogen (stolpert nicht, flieht) – bei Tempo stürzt er.
const BIKE_GRAB = 34; // px: so nah muss man an ein Rad heran (fahrende sind schnell weg)
function takeBike(w, b) {
  const kind = b.kind === 'scooter' ? 'escooter' : 'bicycle';
  const car = createCar({ x: b.x, y: b.y, angle: b.angle, kind, color: '#1e272e', role: 'bike' });
  car.seed = b.seed; car.lvl = b.lvl;
  const v = b.state === 'ride' ? b.speed * 0.3 : 0; // wer runtergezogen wird, bremst ab
  car.vx = Math.cos(b.angle) * v; car.vy = Math.sin(b.angle) * v;
  if (b.state === 'ride') {
    const ped = dismount(w, b, w.player.x, w.player.y, { fall: b.speed > 40 });
    if (ped) { const off = sideSpot(car, 1, 10); ped.x = off.x; ped.y = off.y; if (ped.state !== 'down') scare(ped, w.player.x, w.player.y, 3.5); }
    w.events.push({ type: 'carjack', x: b.x, y: b.y, bike: true });
  }
  w.bikes.splice(w.bikes.indexOf(b), 1);
  w.cars.push(car);
  return car;
}

// Fahrer steigt aus und flieht (wird danach ein normaler Passant).
function fleeingDriver(w, car, fromX, fromY, secs) {
  const sp = nearestSpot(w.city, car.x, car.y);
  if (!sp) return null;
  const ped = createPed(w.city, sp, w.rng);
  const s = sideSpot(car, -1, 14); ped.x = s.x; ped.y = s.y;
  ped.kind = hash01(ped.id * 7.3 + 1) < 0.3 ? 'business' : 'everyday'; // wer Auto fährt, schiebt keinen Kinderwagen
  scare(ped, fromX, fromY, secs);
  return ped;
}

// Ebenen aller Objekte fortschreiben; wer noch keine hat (neu erzeugt), bekommt sie von der Fläche, auf der er steht
function updateLevels(w) {
  const city = w.city;
  const upd = (o, angle) => { if (o.lvl == null) o.lvl = initialLevel(city, o.x, o.y, angle); else stepLevel(city, o); };
  for (const c of w.cars) {
    if (c.lvl != null && c.role === 'curb' && Math.abs(c.vx) + Math.abs(c.vy) < 2) continue; // schlafende Parker
    upd(c, c.angle);
  }
  for (const ped of w.peds) upd(ped, ped.angle ?? null);
  for (const b of w.bikes ?? []) upd(b, b.angle);
  const p = w.player, pc = playerCar(w);
  if (pc) p.lvl = pc.lvl; else if (p.ride?.underground) { /* im Tunnel: updateRide setzt die Ebene */ } else if (p.inside) p.lvl = -2; else if (!p.dead) upd(p, null);
}

// Klick-Steuerung zu Fuß (Diablo-Schema, main.js setzt clickWorld/clickPressed/clickHeld/clickForce): Klick auf den
// Boden = hinlaufen (Weg um Hindernisse, footpath.js; gehalten = dem Zeiger folgen), Klick auf eine Person/ein Auto =
// bis in Reichweite laufen und angreifen, Shift-Klick = stehen bleiben und zum Zeiger angreifen. Übersetzt das in die
// gewohnten Eingaben (moveX/moveY, fire, aimWorld); WASD bricht einen Klick ab.
// Diablo-Schema zu Fuß: was der Klick bedeutet, entscheidet allein der Moment des Drückens (combat.js clickIntent).
// Gehalten bleibt es dabei: wer auf den Boden geklickt hat, läuft dem Zeiger nach und greift nie an, was er dabei
// überstreicht; wer eine Person angeklickt hat, greift genau sie an, bis sie liegt (kein Weiterspringen auf die nächste);
// ein Auto weiter weg wird nur angesteuert (daneben stehen bleiben), eines direkt daneben (oder per Doppelklick) nach
// einem kurzen Moment an der Tür bestiegen. Nur Strg greift auf der Stelle an – auch Autos.
function clickControl(w, input, dt) {
  const p = w.player;
  if (Math.hypot(input.moveX, input.moveY) > 0.05) { p.click = null; return input; }
  const at = input.clickWorld;
  const walk = (to) => { if (p.inside) { const t = { x: to.x, y: to.y }, stn = stationById(w.city, p.inside.id); if (stn) keepInside(stn, t, PLAYER.radius); return { path: [{ x: p.x, y: p.y }, t], i: 1 }; } const path = findFootPath(w, p, to, p.lvl ?? 0); return path && path.length > 1 ? { path, i: 1 } : null; }; // im Bahnhof: gerade (keepInside hält am Bahnsteig)
  if (at && input.clickPressed) {
    const it = clickIntent(w, at.x, at.y, !!input.clickForce, { double: !!input.clickDouble });
    p.clickT = 0.15;
    if (it.kind === 'force') p.click = { force: true, x: at.x, y: at.y, obj: it.obj };
    else if (it.kind === 'attack') p.click = { target: it.obj };
    else if (it.kind === 'enter' || it.kind === 'approach') p.click = { enter: it.obj, approach: it.kind === 'approach' };
    else { const c = walk(at); p.click = c && { ...c, follow: true }; }
  } else if (at && input.clickHeld && p.click?.follow && (p.clickT = (p.clickT ?? 0) - dt) <= 0) {
    p.clickT = 0.15; // gehalten: dem Zeiger nachlaufen (Weg alle 0,15 s neu)
    const c = walk(at);
    if (c) p.click = { ...c, follow: true };
  } else if (at && input.clickHeld && p.click?.force && !p.click.obj) { p.click.x = at.x; p.click.y = at.y; }
  const c = p.click;
  if (!c) return input;
  const out = { ...input };
  if (c.force) {
    if (!input.clickHeld && !input.clickPressed) { p.click = null; return input; }
    const o = c.obj && c.obj.state !== 'dead' && !c.obj.wrecked ? c.obj : null;
    return Object.assign(out, { aimWorld: o ? { x: o.x, y: o.y } : { x: c.x, y: c.y }, fire: true, firePressed: !!input.clickPressed || (p.cool ?? 0) <= 0 });
  }
  if (c.enter) {
    const car = c.enter, bike = w.bikes.includes(car);
    if (bike ? car.state !== 'ride' && car.state !== 'lying' : car.wrecked || !w.cars.includes(car) || car.id === p.inCar) { p.click = null; return input; }
    const dx = car.x - p.x, dy = car.y - p.y, d = Math.hypot(dx, dy);
    if (bike && d < BIKE_GRAB - 4 && !c.approach) { p.click = null; tryEnter(w, car); return Object.assign(out, { moveX: 0, moveY: 0, fire: false, firePressed: false }); } // Rad: packen, aufsteigen
    if (d < (bike ? BIKE_GRAB - 4 : PLAYER.enterDist - 2)) {
      if (c.approach) { p.click = null; p.angle = Math.atan2(dy, dx); return Object.assign(out, { moveX: 0, moveY: 0, fire: false, firePressed: false }); } // angekommen
      // an der Tür: kurz stehen bleiben (Tür auf), dann einsteigen
      if (c.doorT === undefined) { c.doorT = CLICK.door; p.angle = Math.atan2(dy, dx); w.events.push({ type: 'door', x: car.x, y: car.y }); }
      if ((c.doorT -= dt) <= 0) { p.click = null; tryEnter(w, car, { quiet: true }); }
      return Object.assign(out, { moveX: 0, moveY: 0, fire: false, firePressed: false });
    }
    // Weg zum Auto (um Häuser herum), neu, wenn es weggefahren ist
    if (bike && car.state === 'ride') return Object.assign(out, { moveX: dx / d, moveY: dy / d, fire: false, firePressed: false }); // fahrendem Rad direkt nach (sprinten hilft)
    if (!c.path || Math.hypot(car.x - c.tx, car.y - c.ty) > 30) { const wk = walk(car); c.path = wk?.path ?? null; c.i = 1; c.tx = car.x; c.ty = car.y; }
    let q = c.path?.[c.i];
    while (q && Math.hypot(q.x - p.x, q.y - p.y) < 5) q = c.path[++c.i];
    const gx = q ? q.x : car.x, gy = q ? q.y : car.y, gd = Math.hypot(gx - p.x, gy - p.y) || 1;
    return Object.assign(out, { moveX: (gx - p.x) / gd, moveY: (gy - p.y) / gd, fire: false, firePressed: false });
  }
  if (c.target) {
    const o = c.target, alive = w.bikes.includes(o) ? o.state === 'ride' : o.state !== 'dead' && !o.wrecked && w.peds.includes(o); // Radfahrer: bis er vom Rad ist
    if (!alive) { p.click = null; return input; }
    const wp = WEAPONS[p.weapon ?? 0], dx = o.x - p.x, dy = o.y - p.y, d = Math.hypot(dx, dy);
    const reach = wp.melee ? (wp.range ?? 30) + 6 : (wp.range ?? 400) * 0.85;
    if (d > reach) return Object.assign(out, { moveX: dx / d, moveY: dy / d });
    // in Reichweite: ein Klick = ein Angriff; gehalten: weiter, bis die Person liegt
    const ready = (p.cool ?? 0) <= 0;
    if (!input.clickHeld && c.done && ready) { p.click = null; return input; }
    if (ready) c.done = true;
    return Object.assign(out, { aimWorld: { x: o.x, y: o.y }, fire: true, firePressed: ready });
  }
  // Weg abarbeiten
  let q = c.path[c.i];
  while (q && Math.hypot(q.x - p.x, q.y - p.y) < 5) q = c.path[++c.i];
  if (!q) { p.click = null; return input; }
  const d = Math.hypot(q.x - p.x, q.y - p.y);
  return Object.assign(out, { moveX: (q.x - p.x) / d, moveY: (q.y - p.y) / d });
}

function updatePlayerOnFoot(w, input, dt) {
  const p = w.player;
  if (p.stun > 0) { p.stun -= dt; return; }
  let mx = input.moveX, my = input.moveY;
  const mag = Math.min(1, Math.hypot(mx, my));
  // Ausdauer: Sprint leert sie, nach kurzer Pause erholt sie sich; leer = nur joggen, bis wieder genug da ist
  p.stamina ??= 1; p.tired ??= false;
  const wantSprint = !!input.sprint && mag > 0.05 && !input.walkSlow;
  if (p.tired && p.stamina >= STAMINA.again) p.tired = false;
  const sprinting = wantSprint && !p.tired && p.stamina > 0;
  if (sprinting) { p.stamina = Math.max(0, p.stamina - dt / STAMINA.drain); p.rest = 0; if (p.stamina === 0) p.tired = true; }
  else if ((p.rest = (p.rest ?? 0) + dt) > STAMINA.pause) p.stamina = Math.min(1, p.stamina + dt / STAMINA.recover);
  if (mag > 0.05) {
    // Stick halb = gehen, darüber joggen; Alt = gehen; Sprint mit Ausdauer
    const speed = sprinting ? PLAYER.sprint : input.walkSlow || mag <= 0.6 ? PLAYER.walk : PLAYER.jog;
    const nx = mx / (Math.hypot(mx, my) || 1), ny = my / (Math.hypot(mx, my) || 1);
    p.x += nx * speed * dt; p.y += ny * speed * dt; p.moveSpeed = speed; // für die Streuung (combat.js spreadFactor)
    p.angle = Math.atan2(ny, nx);
    p.move = p.angle; // Laufrichtung (Beine); p.angle kann danach das Zielen übernehmen (Oberkörper)
    p.step += speed * dt;
  }
  if (mag <= 0.05) p.moveSpeed = 0;
  const st = p.inside && stationById(w.city, p.inside.id);
  if (st) { keepInside(st, p, PLAYER.radius); return; } // im U-Bahnhof: nur Bahnsteig und Säulen
  pushCircleOutOfWorld(w, p, PLAYER.radius);
  p.x = clamp(p.x, 8, w.city.width - 8); p.y = clamp(p.y, 8, w.city.height - 8);
}

function applyDriverInput(car, input) {
  car.controls.throttle = input.throttle;
  car.controls.brake = input.brake;
  car.controls.steer = input.steer;
  car.controls.handbrake = input.handbrake;
  car.horn = input.horn;
}

// Mitfahren (Fahrgast): eigene Taste (input.ride). Einsteigen überall in Reichweite eines Wagens, auch in Fahrt
// (Aufspringen); Aussteigen jederzeit, schnell = Abspringen mit Sturz; unter Tage nur am Bahnsteig, Ausgang an der Straße.
function lastStopOf(st) {
  const i = Math.max(0, Math.min(st.p.stops.length - 1, st.dwelling ? st.stop : st.stop - 1));
  const q = pointOn(st.p, st.p.stops[i]);
  return { x: q.x, y: q.y, name: st.p.stopNames[i] ?? '', i, pid: st.p.id };
}
export function boardTransit(w) {
  const p = w.player;
  if (p.stun > 0) return false; // gestürzt (z. B. gerade abgesprungen): erst aufstehen
  const hit = transitNear(w, p.x, p.y, RIDE.reach).find((h) => !h.ref.playerTrain);
  if (!hit) return false;
  const st = vehicleState(w, hit.ref);
  if (!st) return false;
  const hop = st.speed > RIDE.hopOn;
  p.ride = { kind: 'passenger', ref: hit.ref, mode: st.mode, car: hit.car, lastStop: lastStopOf(st), since: w.time, line: st.p.name, dest: st.p.stopNames[st.p.stopNames.length - 1] };
  w.events.push({ type: 'board', mode: st.mode, line: st.p.name, hop, x: p.x, y: p.y });
  return true;
}
export function alightTransit(w) {
  const p = w.player, r = p.ride, st = vehicleState(w, r.ref);
  if (!st) { endRide(w, 'gone'); return true; }
  if (st.underground || elevated(w, st, st.cars[Math.min(r.car, st.cars.length - 1)])) { // Tunnel/Hochbahn: nur am Bahnhof
    if (!st.dwelling) { w.notice = { text: st.underground ? 'Nur am Bahnsteig' : 'Aussteigen nur am Bahnhof', t: 1.5 }; return false; }
    // unter Tage mit begehbarem Bahnhof: auf dessen Bahnsteig neben dem Wagen; sonst wie bisher an die Straße
    if (st.underground && platformArrival(w, st, r.car)) { w.events.push({ type: 'alight', hop: false, x: p.x, y: p.y, station: true }); return true; }
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
    const fx = p.x + Math.cos(c.angle) * 20, fy = p.y + Math.sin(c.angle) * 20; // Schwung in Fahrtrichtung
    if (spotFreeHere(w, fx, fy, 8, p.lvl ?? 0)) { p.x = fx; p.y = fy; } // nur wenn dort nichts Festes steht
    p.stun = RIDE.stun;
    if (st.speed > RIDE.hurtFrom) hurtPlayer(w, RIDE.hurt, c.x, c.y);
  }
  w.events.push({ type: 'alight', hop, x: p.x, y: p.y });
  return true;
}
// --- U-Bahnhöfe (station.js): hinein über die Eingänge an der Straße, hinaus über die Treppen, Züge am Bahnsteig ----
// Bahnhof, an dessen Bahnsteig Muster pid am Halt i hält (oder null)
function stationOfHalt(w, pid, i) {
  const pat = w.city.transit?.patterns[pid];
  if (!pat) return null;
  const q = pointOn(pat, pat.stops[i]);
  return stationsNear(w.city, q.x, q.y, 800).find((s) => s.halts.some((h) => h.pid === pid && h.i === i)) ?? null;
}
// Aussteigen unter Tage auf den Bahnsteig neben Wagen car
function platformArrival(w, st, car) {
  const stn = stationOfHalt(w, st.p.id, st.stop);
  if (!stn) return false;
  const h = stn.halts.find((x) => x.pid === st.p.id && x.i === st.stop), k = TRAIN[st.p.mode];
  const u = h.dir * (stn.L / 2 - (Math.min(car, k.cars - 1) * (k.carL + k.gap) + k.carL / 2));
  const at = toWorld(stn, u, h.dir * (STATION.half - 12));
  const p = w.player;
  p.ride = null; p.x = at.x; p.y = at.y; p.lvl = -2; p.inside = { id: stn.id, guard: false }; p.click = null;
  keepInside(stn, p, PLAYER.radius);
  return true;
}
// Einsteigen am Bahnsteig (G): haltender Zug auf der eigenen Seite, Figur am Rand neben einem Wagen
function boardAtPlatform(w) {
  const p = w.player, stn = stationById(w.city, p.inside.id);
  if (!stn || p.stun > 0) return false;
  const b = boardable(w, stn, p.x, p.y);
  if (!b) { w.notice = { text: 'Zum Einsteigen an die Bahnsteigkante neben einen haltenden Zug', t: 1.5 }; return false; }
  const vs = vehicleState(w, b.train.ref);
  if (!vs) return false;
  p.inside = null;
  p.ride = { kind: 'passenger', ref: b.train.ref, mode: vs.mode, car: b.car, lastStop: lastStopOf(vs), since: w.time, line: vs.p.name, dest: vs.p.stopNames[vs.p.stopNames.length - 1] };
  w.events.push({ type: 'board', mode: vs.mode, line: vs.p.name, hop: false, x: p.x, y: p.y });
  return true;
}
// Jeden Schritt zu Fuß: Treppe hinauf (Ausgang an der Straße) bzw. an der Straße in einen Eingang hinein
function updateStationPresence(w, input = {}) {
  const p = w.player;
  if (p.inside) {
    const stn = stationById(w.city, p.inside.id);
    if (!stn) { p.inside = null; p.lvl = undefined; return; }
    const e = stairAt(stn, p.x, p.y);
    if (!e) { p.inside.guard = false; return; }
    if (p.inside.guard) return; // gerade heruntergekommen: erst von der Treppe gehen
    const ex = stn.exits[e < 0 ? 0 : 1];
    p.inside = null; p.x = ex.x; p.y = ex.y; p.lvl = undefined; p.click = null;
    p.entryGuard = { x: ex.x, y: ex.y }; // nicht gleich wieder hinunter
    w.events.push({ type: 'station-exit', x: p.x, y: p.y, name: stn.name });
    return;
  }
  if (p.entryGuard && Math.hypot(p.x - p.entryGuard.x, p.y - p.entryGuard.y) > 30) p.entryGuard = null;
  if ((p.lvl ?? 0) !== 0) return;
  // hineinlaufen (Eingang betreten) oder E in der Nähe eines Eingangs (nicht, wenn E gerade etwas anderes meint)
  const near = entranceNear(w._stNear, p.x, p.y, STATION.reach);
  const walkIn = near && near.d <= STATION.entrance && !p.entryGuard;
  const byKey = near && input.action && !w.mission.prompt;
  for (const { stn, ex } of walkIn || byKey ? [near] : []) {
    const at = arrivalAt(stn, ex.e);
    p.x = at.x; p.y = at.y; p.angle = at.angle; p.lvl = -2; p.click = null;
    p.inside = { id: stn.id, guard: true };
    w.events.push({ type: 'station-enter', x: p.x, y: p.y, name: stn.name });
    return;
  }
}
// Ausgang, an dem man bei einem Spielstand im Bahnhof wieder oben steht
export function stationSaveSpot(w) {
  const stn = w.player.inside && stationById(w.city, w.player.inside.id);
  if (!stn) return null;
  const l = toLocal(stn, w.player.x, w.player.y), ex = stn.exits[l.u < 0 ? 0 : 1];
  return { x: ex.x, y: ex.y };
}
export { stationName };

// Wo ein Fahrgast ohne Fahrzeug landet: letzte Haltestelle, bei S-/U-Bahn deren Straßenausgang (nicht das Gleisbett).
// Gemeinsam für endRide und den Spielstand (save.js makeSave).
export function rideExit(w, ride) {
  const pat = w.city.transit?.patterns[ride.lastStop.pid];
  const ex = pat && (pat.mode === 'ubahn' || pat.mode === 'sbahn') ? stationExit(w, pat, ride.lastStop.i) : ride.lastStop;
  return { x: ex.x, y: ex.y };
}
// Fahrt beenden, ohne Fahrzeug (verschwunden, Teleport, K. o.): an der letzten Haltestelle zu Fuß
export function endRide(w, reason) {
  const p = w.player, r = p.ride;
  if (!r) return;
  p.ride = null;
  if (reason !== 'teleport') {
    const ex = rideExit(w, r);
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
  p.x = c.x; p.y = c.y; p.angle = c.angle;
  if (st.underground) p.lvl = -2;
  else if (r.underground) p.lvl = undefined; // aus dem Tunnel: Ebene neu bestimmen (updateLevels)
  if (st.dwelling && r.kind === 'passenger') r.lastStop = lastStopOf(st); // Fahrer: playertrain.js setzt ihn beim Türöffnen
  r.speed = st.speed; r.underground = st.underground;
}

// Wetter am Auto: Haftung (car.traction), Aufschwimmen in einer Pfütze (car.aqua) und Böen
function applyWeather(w, c, dt) {
  const lvl = c.lvl ?? 0;
  // trocken und ohne Sturm: nichts zu prüfen (spart die Abfrage je Auto und Schritt)
  if (!(w.wet > 0) && !(w.snow > 0) && !(w.ice > 0) && !(w.weather?.storm > 0)) { c.traction = DRY; return; }
  c.traction = tractionOf(roadCondition(w, c.x, c.y, lvl));
  const vf = c.vx * Math.cos(c.angle) + c.vy * Math.sin(c.angle);
  let p = null;
  if (vf > AQUA.speed) {
    const ca = Math.cos(c.angle), sa = Math.sin(c.angle), fx = c.hw * 0.7, fy = c.hh * 0.8;
    p = puddleAt(w, c.x, c.y, lvl); // Mitte, dann beide Vorderräder
    for (const s of [-1, 1]) { if (p) break; p = puddleAt(w, c.x + ca * fx - sa * fy * s, c.y + sa * fx + ca * fy * s, lvl); }
  }
  // nicht erneut, solange es noch schwimmt (eine Pfütze ist durchfahren, bevor AQUA.time abläuft)
  if (p && !(c.aqua > 0)) {
    c.aqua = AQUA.time; c.aquaYaw = aquaYaw(p); // Gieren, solange es schwimmt (car.js)
    w.events.push({ type: 'aquaplane', x: c.x, y: c.y, carId: c.id, player: c.id === w.player.inCar });
  }
  const g = gustPush(w, c, lvl);
  if (g) { c.vx += g.ax * dt; c.vy += g.ay * dt; }
}

// Ein fester Simulationsschritt. input: siehe input.js (abstrakte Aktionen).
export function updateWorld(w, input, dt) {
  w.events.length = 0;
  if (w.pendingSave && !resolveSave(w)) { w.loading = true; return; }
  if (!streamWorld(w)) return;
  w.time += dt;
  w.clock += dt * CLOCK.minutesPerSecond * (w.clockRate ?? 1); // Tempo der Spieluhr (Konsole: tempo)
  if (w.clock >= 1440) { w.clock -= 1440; w.day = (w.day + 1) % 7; w.dayCount++; }
  w.weather = weatherAt(w.seed, w.dayCount, w.clock, w.forceWeather ?? (w.rhythm ? null : 'clear'));
  w.wet = stepWet(w.wet, Math.min(1, w.weather.rain), dt);
  const snowWas = w.snow ?? 0;
  w.snow = stepSnow(snowWas, w.weather, dt);
  if (w.snow < snowWas) w.wet = Math.max(w.wet, Math.min(1, w.snow * 1.5)); // Tauwetter: Matsch und nasse Straßen
  w.temp = w.forceTemp ?? temperatureAt(w.seed, w.dayCount, w.clock, w.forceWeather); // °C (Konsole: temp)
  w.ice = stepIce(w.ice ?? 0, w.wet, w.temp, dt); // überfrierende Nässe
  if (w.notice && (w.notice.t -= dt) <= 0) w.notice = null;
  if (w.vehInfo && (w.vehInfo.t += dt) > VEH_INFO_S) w.vehInfo = null;
  const m = w.mission;

  // Briefing/Ergebnis frieren die Welt ein; nur die Mission reagiert auf Eingaben.
  if (m.state === 'briefing' || m.state === 'success' || m.state === 'failed') {
    w.events.push(...updateMission(m, missionCtx(w, input), dt));
    return;
  }

  const p = w.player;
  if (input.ride && !p.dead && !p.inCar && p.ride?.kind !== 'driver') { if (p.ride) alightTransit(w); else if (p.inside) boardAtPlatform(w); else boardTransit(w); }
  else if (input.enterExit && !p.dead && !p.inside) {
    if (p.ride?.kind === 'driver') leaveTrain(w);
    else if (!p.ride) {
      if (p.inCar) tryExit(w);
      else { // am Führerstand einer Bahn (Spitze ≤ RIDE.cab): übernehmen, sonst wie immer ein Auto
        const cab = transitNear(w, p.x, p.y, RIDE.cab + 10).find((h) => h.car === 0 && h.front <= RIDE.cab && h.mode !== 'bus');
        if (!(cab && takeTrain(w, cab))) tryEnter(w);
      }
    }
  }
  // Wenden verbraucht den Tastendruck – sonst öffnete updatePlayerTrain damit gleich die Türen am neuen ersten Halt
  const trainInput = input.action && p.ride?.kind === 'driver' && atTerminus(w) && turnAround(w) ? { ...input, action: false } : input;

  // Bahnhöfe um den Spieler (Eingänge, Hinweis, Minikarte) – alle 0,5 s, auch im Auto und nach Teleports
  if ((w._stT = (w._stT ?? 0) - 1) <= 0 || w._stAt !== w.city.gen) { w._stT = 30; w._stAt = w.city.gen; const o = playerCar(w) ?? p; w._stNear = stationsNear(w.city, o.x, o.y, 1200); }
  const pc = playerCar(w);
  if (pc) {
    if (input.espToggle && !pc.wrecked) {
      w.esp = w.esp === false;
      w.notice = { text: `ESP ${w.esp ? 'AN' : 'AUS'}`, t: 1.6 };
    }
    if (input.absToggle && !pc.wrecked) {
      w.abs = w.abs === false;
      w.notice = { text: `ABS ${w.abs ? 'AN' : 'AUS'}`, t: 1.6 };
    }
    if (pc.wrecked) applyDriverInput(pc, { throttle: 0, brake: 0, steer: 0, handbrake: false, horn: false });
    else applyDriverInput(pc, input);
    pc.esp = w.esp !== false; // ASR/ESP (Befehl „esp aus“ schaltet ab)
    pc.abs = w.abs !== false;
    if (pc.horn && !pc._hornWas) w.events.push({ type: 'horn', x: pc.x, y: pc.y });
    pc._hornWas = pc.horn;
  } else if (!p.dead && !p.ride) { input = clickControl(w, input, dt); if (!p.inCar) { updatePlayerOnFoot(w, input, dt); updateStationPresence(w, input); } }
  if (!p.ride) updatePlayerCombat(w, input, dt);
  if (p.dead) updateKnockout(w, dt);

  // Nachbarschaftsraster für die Hinderniserkennung der KI (während dieser Schleife bewegt sich nichts)
  w._gCars = buildGrid(w.cars, GRID_CELL, w._gCars); w._gPeds = buildGrid(w.peds, GRID_CELL, w._gPeds); w._gridOn = true;
  for (const c of w.cars) if (c.driver === 'npc') { driveAi(c, w, dt); updateService(w, c, dt); }
  w._gridOn = false; // danach haben sich Autos bewegt – das Raster gilt nicht mehr
  for (const c of w.cars) {
    if (c.driver === null && !c.wrecked && c !== pc) { c.controls.throttle = 0; c.controls.brake = 0; c.controls.steer = 0; c.controls.handbrake = true; }
    // Unberührte geparkte Autos schlafen (spart die Weltkollision für hunderte Autos).
    if (c.role === 'curb' && c.driver === null && !c.wrecked && Math.abs(c.vx) + Math.abs(c.vy) < 2 && Math.abs(c.angVel) < 0.01) { c.vx = c.vy = c.angVel = 0; continue; }
    applyWeather(w, c, dt);
    stepCar(c, dt, w.city);
    collideCarWorld(c, w, w.events);
  }
  // Auto gegen Auto: nur Nachbarn aus dem Raster, Paare in derselben Reihenfolge wie die volle Doppelschleife (i < j
  // aufsteigend). Radius großzügig über der größten Summe halber Längen (Bus + Bus ≈ 124 px) plus Wegschieben.
  const pairGrid = w._gPairs = buildGrid(w.cars, GRID_CELL, w._gPairs), nb = w._pairBuf ??= [];
  for (let i = 0; i < w.cars.length; i++) {
    const a = w.cars[i];
    for (const j of near(pairGrid, a.x, a.y, 170, nb)) {
      if (j <= i) continue;
      const b = w.cars[j];
      const r = a.hw + b.hw + 4; if (Math.abs(a.x - b.x) < r && Math.abs(a.y - b.y) < r && touch(w.city, a, b)) collideCars(a, b, w.events);
    }
  }
  // Motorrad/Roller: ein harter Aufprall wirft den Fahrer ab (ab ≈ 27 km/h frontal)
  if (pc && isMotoKind(pc.kind) && w.player.inCar === pc.id) {
    const hit = w.events.find((e) => e.type === 'crash' && e.carId === pc.id && e.strength >= MOTO.throwAt);
    if (hit) throwRider(w, pc, hit.strength);
  }

  updateTransit(w, dt); // Fahrplan-Fahrzeuge, Busse als KI, Straßenbahnen als Hindernisse
  updatePlayerTrain(w, trainInput, dt); // vom Spieler geführter Zug (playertrain.js)
  if (p.ride) updateRide(w); // Fahrgast/Fahrer sitzt im Wagen (nach dem Fortschreiben der Fahrzeuge)
  // Tunnelansicht weich ein-/ausblenden (render.js/tunnelview.js), 0 = oben, 1 = unter Tage
  const ugTarget = p.ride?.underground ? 1 : 0;
  w.underground = (w.underground ?? 0) + (ugTarget - (w.underground ?? 0)) * Math.min(1, dt / 0.6);
  if (Math.abs(w.underground - ugTarget) < 0.01) w.underground = ugTarget;

  if (pc) { p.x = pc.x; p.y = pc.y; p.angle = pc.angle; }
  updateLevels(w); // Ebene je Objekt (Brücke, Boden, Unterführung) – levels.js

  // Wracks: KI-Fahrer steigt aus und flieht, Wrack verschwindet später außer Sicht.
  for (const c of w.cars) {
    if (c.shotAt && c.driver === 'npc' && !c.wrecked) { // beschossen: Fahrer steigt aus und rennt weg
      c.driver = null; c.ai = null;
      const ped = fleeingDriver(w, c, c.shotAt.x, c.shotAt.y, 5);
      if (ped) w.peds.push(ped);
    }
    c.shotAt = null;
    if (!c.wrecked) continue;
    c.wreckT += dt;
    if (c.driver === 'npc') {
      c.driver = null; c.ai = null;
      const ped = fleeingDriver(w, c, c.x, c.y, 3);
      if (ped) w.peds.push(ped);
    }
  }
  const cam = w.camera;
  w.cars = w.cars.filter((c) => !(c.wrecked && c.wreckT > 20 && c.id !== w.playerCarId && c.id !== p.inCar && !c.cargo
      && Math.hypot(c.x - cam.x, c.y - cam.y) > 900));

  // Spieler zu Fuß gegen Autos (nicht im U-Bahnhof – die fahren oben).
  if (!p.inCar && !p.ride && !p.inside) {
    for (const c of w.cars) {
      const mm = circleVsObb(p.x, p.y, PLAYER.radius, c);
      if (!mm || !touch(w.city, p, c)) continue;
      p.x += mm.nx * mm.depth; p.y += mm.ny * mm.depth;
      if (speedOf(c) > 120 && p.stun <= 0 && !p.dead) { p.stun = 0.8; w.events.push({ type: 'bump', x: p.x, y: p.y }); hurtPlayer(w, speedOf(c) * 0.12, c.x, c.y); }
    }
  }

  // Passanten: Bedrohungen wahrnehmen, gegen Autos prüfen, bewegen.
  const threats = [];
  if (pc && !pc.wrecked && speedOf(pc) > 130) threats.push({ x: pc.x, y: pc.y, vx: pc.vx, vy: pc.vy, r: 90 });
  for (const e of w.events) {
    if (e.type === 'horn' && !e.npc) threats.push({ x: e.x, y: e.y, r: 170, always: true });
    if (e.type === 'horn' && e.npc) threats.push({ x: e.x, y: e.y, r: 80, always: true }); // KI hupt: wer direkt davor steht, weicht
    if (e.type === 'crash' && e.strength > 0.25) threats.push({ x: e.x, y: e.y, r: 130, always: true });
    if (e.type === 'shot' && !w.player.inside) threats.push({ x: e.x, y: e.y, r: GUNSHOT_SCARE, always: true }); // unten hört oben keiner
    if ((e.type === 'blood' || e.type === 'swing') && !e.npc) threats.push({ x: e.x, y: e.y, r: e.type === 'blood' ? 220 : 90, always: true, melee: e.type === 'swing' });
  }
  const carGrid = w._gCarsPed = buildGrid(w.cars, GRID_CELL, w._gCarsPed); // Autos stehen während der Passanten-Schleife
  for (const ped of w.peds) {
    if (ped.state === 'dead') { updatePed(ped, w, dt); continue; }
    if (ped.state !== 'down' && ped.state !== 'flee' && ped.state !== 'fight') {
      for (const t of threats) {
        const d = Math.hypot(ped.x - t.x, ped.y - t.y);
        if (d > t.r) continue;
        if (t.melee && isFighter(ped) && !p.dead) { startFight(ped); break; } // Schlägerei in der Nähe: mitmischen
        const toward = t.always || ((ped.x - t.x) * t.vx + (ped.y - t.y) * t.vy) > 0;
        if (toward) { scare(ped, t.x, t.y); break; }
      }
    }
    for (const ci of near(carGrid, ped.x, ped.y, GRID_REACH, w._nearBuf ??= [])) { // nur Autos in der Nähe (Reihenfolge wie w.cars)
      const c = w.cars[ci];
      if (ped.state === 'down') break;
      const mm = circleVsObb(ped.x, ped.y, PED.radius, c);
      if (!mm || !touch(w.city, ped, c)) continue;
      if (speedOf(c) > 55) {
        knockDown(ped, c.x, c.y);
        w.events.push({ type: 'hit', x: ped.x, y: ped.y, carId: c.id, player: c.id === p.inCar, speed: speedOf(c) });
        for (const o of w.peds) if (o !== ped && Math.hypot(o.x - ped.x, o.y - ped.y) < 110) scare(o, ped.x, ped.y);
      } else { ped.x += mm.nx * mm.depth; ped.y += mm.ny * mm.depth; }
    }
    updatePed(ped, w, dt);
  }
  // Räder fahren; wer von einem Auto erwischt wird, stürzt (Fahrer liegt, Rad bleibt liegen)
  for (const b of w.bikes) {
    updateBike(b, w, dt);
    if (b.state !== 'ride') continue;
    for (const c of w.cars) {
      if (Math.abs(c.x - b.x) > c.hw + 8 || Math.abs(c.y - b.y) > c.hw + 8 || speedOf(c) < 60) continue;
      if (!circleVsObb(b.x, b.y, BIKE.r, c)) continue;
      dismount(w, b, c.x, c.y);
      w.events.push({ type: 'hit', x: b.x, y: b.y, strength: Math.min(1, speedOf(c) / 300), carId: c.id, player: c.id === w.player.inCar, bike: true, speed: speedOf(c) });
      break;
    }
  }
  // Tote verschwinden nach einer Weile, aber nur außer Sicht (spätestens nach 5 min)
  w.peds = w.peds.filter((q) => q.state !== 'dead' || (q.deadT < BODY_KEEP || Math.hypot(q.x - cam.x, q.y - cam.y) < 900) && q.deadT < 300);
  // Überzählige (geflohene Fahrer) wieder abbauen, wenn außer Sicht.
  if (w.peds.length > w.pedTarget + 8) {
    const idx = w.peds.findIndex((q) => q.state === 'walk' && Math.hypot(q.x - cam.x, q.y - cam.y) > 900);
    if (idx >= 0) w.peds.splice(idx, 1);
  }
  managePopulation(w);
  manageParked(w);
  manageLife(w);
  manageAnimals(w);
  updateAnimals(w, dt);
  manageEmergency(w, dt);

  w.events.push(...updateMission(m, missionCtx(w, input), dt));
  if (m.state === 'success') {
    w.money += m.result.reward;
    w.completed += 1;
    if (w.bestTime === null || m.result.time < w.bestTime) { w.bestTime = m.result.time; m.result.newBest = true; }
  }

  updateCamera(w, dt);
}

// K. o.: nach kurzer Zeit im nächsten Krankenhaus aufwachen (Stadtteil wird bei Bedarf erst geladen),
// ein laufender Auftrag scheitert, ein Teil des Geldes ist weg.
export function nearestHospital(city, x, y) {
  let best = null, bd = Infinity;
  for (const h of city.hospitals ?? []) { const d = Math.hypot(h.x - x, h.y - y); if (d < bd) { bd = d; best = h; } }
  return best;
}

function updateKnockout(w, dt) {
  const p = w.player;
  p.deadT += dt;
  if (p.deadT < RESPAWN_DELAY) return;
  const h = nearestHospital(w.city, p.x, p.y) ?? w.city.places.playerSpawn;
  const spot = findTeleportSpot(w, h.x, h.y);
  if (spot?.pending) { w.loading = true; return; } // Stadtteil des Krankenhauses lädt noch
  const fee = Math.floor(w.money * HOSPITAL_FEE);
  w.money -= fee;
  teleportTo(w, spot ?? { x: w.city.places.playerSpawn.x, y: w.city.places.playerSpawn.y, angle: 0 });
  Object.assign(p, { dead: false, hp: PLAYER_HP, stun: 0, sinceHurt: 99, hurtFlash: 0, reloadT: 0, lvl: undefined });
  w.notice = { text: `Im Krankenhaus aufgewacht${h.name ? ': ' + h.name : ''}${fee ? ` (−${fee.toLocaleString('de-DE')} €)` : ''}`, t: 5 };
  w.events.push({ type: 'respawn', x: p.x, y: p.y, hospital: h.name ?? null, fee });
  const m = w.mission;
  if (m.state === 'toPickup' || m.state === 'toDropoff') failMission(m, 'K. o. – im Krankenhaus aufgewacht, der Auftrag ist geplatzt.', w.events);
}

function missionCtx(w, input) {
  return { places: w.city.places, player: w.player, cars: w.cars, timeLimit: w.city.timeLimit, input };
}

// Zu Fuß näher dran (Details, Nahkampf, Zielen); am PC mit dem Mausrad einstellbar (main.js → setFootZoom)
export const FOOT_ZOOM = 2, FOOT_ZOOM_RANGE = [1.5, 2.6];
export function setFootZoom(w, z) { w.footZoom = clamp(z, FOOT_ZOOM_RANGE[0], FOOT_ZOOM_RANGE[1]); return w.footZoom; }

export function updateCamera(w, dt) {
  const cam = w.camera, p = w.player, car = playerCar(w);
  let tx = p.x, ty = p.y, zoom = w.footZoom ?? FOOT_ZOOM;
  if (car) {
    tx = car.x + car.vx * 0.45; ty = car.y + car.vy * 0.45;
    zoom = isBikeKind(car.kind) ? (w.footZoom ?? FOOT_ZOOM) * 0.8 // Rad: fast so nah wie zu Fuß
      : isMotoKind(car.kind) ? 1.45 - clamp(speedOf(car) / 500, 0, 1) * 0.45 // Motorrad/Roller: näher (klein), bei Tempo weiter weg
      : 1 - clamp(speedOf(car) / 330, 0, 1) * 0.28;
  }
  if (p.ride) { const ahead = p.ride.kind === 'driver' ? (p.ride.speed ?? 0) * 0.6 : 0; tx = p.x + Math.cos(p.angle) * ahead; ty = p.y + Math.sin(p.angle) * ahead; zoom = 1 - clamp((p.ride.speed ?? 0) / 330, 0, 1) * 0.28; }
  cam.x = damp(cam.x, tx, 5, dt);
  cam.y = damp(cam.y, ty, 5, dt);
  cam.zoom = damp(cam.zoom, zoom, 2, dt);
}

export { forwardSpeed, speedOf, damage, obbVsRect, obbVsObb };
