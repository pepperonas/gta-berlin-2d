import test from 'node:test';
import assert from 'node:assert/strict';
import { realCity, openRealCity, realIndex, geoToPx, reachability } from './helpers/city.js';
import { nearestEdge } from '../web/src/map.js';
import { createWorld, updateWorld } from '../web/src/world.js';
import { createCar, collideCarWorld, isDown } from '../web/src/car.js';
import { KNOCK, PLAYER } from '../web/src/config.js';
import { coverOf, crownOf, roofOffset } from '../web/src/occlusion.js';
import { idle } from './helpers/bot.js';

const CAR_R = 11, FOOT_R = 5; // Freiraum eines Autos (halbe Breite + Rand) bzw. einer Person, px
const knockable = (s) => s.layer !== 'barrier'; // Poller fährt man um

test('Tempelhofer Feld: mit dem Auto und zu Fuß von der Straße aus erreichbar', () => {
  const meta = realIndex().meta, city = openRealCity(), P = (lat, lon) => geoToPx(meta, lat, lon);
  const field = P(52.4735, 13.4010), a = P(52.4860, 13.3830), b = P(52.4600, 13.4190), box = [a[0], a[1], b[0], b[1]];
  city.loadArea(...box);
  const s0 = P(52.4835, 13.4007), ne = nearestEdge(city, s0[0], s0[1], 3000, (e) => e.name === 'Columbiadamm');
  assert.ok(ne, 'Columbiadamm geladen');
  const car = reachability(city, box, [ne.x, ne.y], CAR_R), foot = reachability(city, box, [ne.x, ne.y], FOOT_R);
  assert.ok(car(...field), 'mitten aufs Feld fahren');
  assert.ok(foot(...field), 'mitten aufs Feld gehen');
  // weitere Stellen des Felds (Nord- und Südrand, Ostseite am Tempelhofer Damm)
  for (const [lat, lon] of [[52.4790, 13.3950], [52.4680, 13.4040], [52.4760, 13.4150]]) assert.ok(car(...P(lat, lon)), `Feld bei ${lat},${lon}`);
});

test('Erreichbarkeit: das Auto kommt fast überall hin, wo man zu Fuß hinkommt (Poller fährt es um)', () => {
  const meta = realIndex().meta, city = openRealCity();
  for (const [name, lat, lon, min] of [['Kreuzberg', 52.4970, 13.4150, 0.9], ['Marzahn', 52.5450, 13.5650, 0.9]]) {
    const c = geoToPx(meta, lat, lon), h = 7000, box = [c[0] - h, c[1] - h, c[0] + h, c[1] + h];
    city.loadArea(...box);
    const ne = nearestEdge(city, c[0], c[1], 20000, (e) => e.cls <= 5);
    const foot = reachability(city, box, [ne.x, ne.y], FOOT_R, { cell: 8 });
    const car = reachability(city, box, [ne.x, ne.y], CAR_R, { cell: 8, solidFilter: knockable });
    assert.ok(car.count / foot.count > min, `${name}: Auto ${(100 * car.count / foot.count).toFixed(1)} % der Fußgängerfläche`);
    assert.ok(foot.count > 0.5 * (2 * h / 8) ** 2, `${name}: zu Fuß ist der größte Teil erreichbar`);
  }
});

test('Poller: mit Schwung umgefahren (bleibt liegen, auch nach dem Nachladen), langsam nicht; zu Fuß ein Hindernis', () => {
  const city = realCity(), w = createWorld({ city, cars: 0, pedestrians: 0 });
  const post = city.list('barrier').find((b) => city.solids.query({ x: b.x - 40, y: b.y - 40, w: 80, h: 80 }, []).length === 1);
  assert.ok(post, 'freistehender Poller');
  const hit = (speed) => {
    const car = createCar({ x: post.x - 21 - post.r + 3, y: post.y, angle: 0 });
    car.vx = speed;
    const ev = [];
    collideCarWorld(car, w, ev);
    return { car, ev };
  };
  const slow = hit(KNOCK.speed * 0.5);
  assert.ok(!isDown(w, post), 'langsam: Poller bleibt stehen');
  assert.ok(slow.car.x < post.x - 21 - post.r + 3, 'Auto zurückgeschoben');
  const fast = hit(KNOCK.speed * 3);
  assert.ok(isDown(w, post), 'mit Schwung umgefahren');
  assert.ok(fast.ev.some((e) => e.type === 'knock'), 'Ereignis für Ton');
  assert.ok(fast.car.vx > KNOCK.speed * 2 && fast.car.vx < KNOCK.speed * 3, 'Auto bremst leicht ab, fährt weiter');
  assert.ok(isDown(w, { ...post }), 'nach dem Nachladen (neues Objekt, gleicher Ort) liegt er weiter');
  assert.ok(!isDown(createWorld({ city, cars: 0, pedestrians: 0 }), post), 'andere Welt: steht');
  // zu Fuß: stehender Poller schiebt die Spielfigur weg, liegender nicht
  const w2 = createWorld({ city, cars: 0, pedestrians: 0 });
  Object.assign(w2.player, { x: post.x + 2, y: post.y });
  updateWorld(w2, idle(), 1 / 60);
  assert.ok(Math.hypot(w2.player.x - post.x, w2.player.y - post.y) >= PLAYER.radius + post.r - 0.5, 'Poller hält die Spielfigur auf');
  Object.assign(w.player, { x: post.x + 2, y: post.y });
  updateWorld(w, idle(), 1 / 60);
  assert.ok(Math.hypot(w.player.x - post.x, w.player.y - post.y) < 3, 'über einen liegenden Poller läuft man drüber');
});

test('Verdeckung: Baumkrone, Haus (Dach, Fassade, Durchfahrt) und Viadukt verdecken nur, was vorher gezeichnet wurde', () => {
  const tr = { x: 100, y: 200, size: 40 };
  const c = crownOf(tr);
  assert.equal(coverOf({ x: c.x, y: c.y, key: 150 }, { trees: [tr] }), 'tree', 'unter der Krone');
  assert.equal(coverOf({ x: c.x, y: c.y, key: 250 }, { trees: [tr] }), null, 'vor dem Baum (später gezeichnet): sichtbar');
  assert.equal(coverOf({ x: c.x + 60, y: c.y, key: 150 }, { trees: [tr] }), null, 'neben der Krone');
  const b = { rings: [[0, 0, 200, 0, 200, 100, 0, 100]], cx: 100, cy: 50, bbox: { x: 0, y: 0, w: 200, h: 100 }, height: 200 };
  const cam = { x: 100, y: 50 }, env = { buildings: [b], cam, heightScale: 1 };
  const { dy } = roofOffset(b, cam, 1);
  assert.ok(dy < -50, 'Dach nach oben versetzt');
  assert.equal(coverOf({ x: 100, y: 50, key: 50 }, env), 'building', 'in der Durchfahrt');
  assert.equal(coverOf({ x: 100, y: -20, key: -20 }, env), 'building', 'hinter dem Haus (unter dem Dach)');
  assert.equal(coverOf({ x: 100, y: 130, key: 130 }, env), null, 'vor dem Haus');
  assert.equal(coverOf({ x: 300, y: 50, key: 50 }, env), null, 'daneben');
  assert.equal(coverOf({ x: 100, y: 91, key: 101 }, env), null, 'Auto an der Vorderkante: nach dem Haus gezeichnet, liegt obenauf');
  const viaduct = { pts: [0, 500, 400, 500] };
  assert.equal(coverOf({ x: 200, y: 515, key: 515 }, { bridges: [viaduct], deck: 46 }), 'bridge', 'unter der Hochbahn');
  assert.equal(coverOf({ x: 200, y: 540, key: 540 }, { bridges: [viaduct], deck: 46 }), null, 'daneben');
});

test('Silhouette: verdeckt → Umriss obendrauf (Auto und zu Fuß), frei → nichts', async () => {
  globalThis.Path2D ??= class { constructor() { return new Proxy(this, { get: (t, k) => (k in t ? t[k] : () => {}) }); } };
  globalThis.OffscreenCanvas ??= class { constructor(w, h) { this.width = w; this.height = h; } getContext() { return new Proxy({}, { get: (t, k) => (k === 'createRadialGradient' || k === 'createLinearGradient' ? () => ({ addColorStop() {} }) : () => {}) }); } };
  const { Renderer } = await import('../web/src/render.js');
  const city = realCity(), w = createWorld({ city, cars: 0, pedestrians: 0 });
  w.clock = 12 * 60;
  let sil = 0;
  const ctx = new Proxy({ lineWidth: 1 }, {
    get(t, k) {
      if (k in t) return t[k];
      if (k === 'fill') return () => { if (t.fillStyle === 'rgba(255,122,26,0.22)') sil++; };
      if (k === 'createPattern' || k === 'createLinearGradient' || k === 'createRadialGradient') return () => ({ addColorStop() {} });
      if (k === 'measureText') return () => ({ width: 10 });
      if (k === 'getLineDash') return () => [];
      return () => {};
    },
    set(t, k, v) { t[k] = v; return true; },
  });
  const r = new Renderer(ctx);
  // Spielfigur unter einer Baumkrone (Baum steht südlich von ihr, wird also später gezeichnet)
  const tr = city.list('tree').find((x) => x.size > 30);
  const c = crownOf(tr);
  Object.assign(w.player, { x: c.x, y: c.y });
  w.camera.x = c.x; w.camera.y = c.y;
  sil = 0; r.draw(w, 1280, 720, 1.2);
  assert.equal(r.stats.cover, 'tree');
  assert.equal(sil, 1, 'Umriss gezeichnet');
  // mitten auf einer breiten Straße: frei
  const e = city.list('edge').find((x) => x.cls <= 4 && x.w > 150 && !x.br);
  const [x, y] = [e.pts[0], e.pts[1]];
  Object.assign(w.player, { x, y }); w.camera.x = x; w.camera.y = y;
  sil = 0; r.draw(w, 1280, 720, 1.2);
  const covered = r.stats.cover;
  assert.equal(sil, covered ? 1 : 0, 'Umriss nur, wenn verdeckt');
  // im Auto in einer Tordurchfahrt
  const pass = city.list('edge').find((x) => x.passage);
  assert.ok(pass, 'Tordurchfahrt im Kerngebiet');
  {
    const car = createCar({ x: (pass.pts[0] + pass.pts[pass.pts.length - 2]) / 2, y: (pass.pts[1] + pass.pts[pass.pts.length - 1]) / 2 });
    w.cars.push(car); w.player.inCar = car.id; w.camera.x = car.x; w.camera.y = car.y;
    r.draw(w, 1280, 720, 1.2);
    assert.equal(r.stats.cover, 'building', 'im Auto unter dem Haus');
  }
  assert.ok(covered === null || covered === 'tree', `auf der Straße höchstens von Straßenbäumen verdeckt (${covered})`);
});
