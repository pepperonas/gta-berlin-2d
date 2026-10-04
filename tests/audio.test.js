import test from 'node:test';
import assert from 'node:assert/strict';
import { Sound } from '../web/src/audio.js';

// Nachgebaute Web-Audio-Umgebung: merkt sich Quellen, ob sie schleifen, gestoppt werden und welche Lautstärke sie haben.
function fakeAudio() {
  const sources = [];
  const param = (v = 0) => ({ value: v, target: v, setTargetAtTime(x) { this.target = x; }, setValueAtTime(x) { this.value = x; }, linearRampToValueAtTime() {}, exponentialRampToValueAtTime() {} });
  const node = (extra) => ({ connect() {}, ...extra });
  class Ctx {
    constructor() { this.state = 'running'; this.currentTime = 0; this.sampleRate = 48000; this.destination = node(); }
    resume() { return Promise.resolve(); }
    createGain() { return node({ gain: param(1) }); }
    createBiquadFilter() { return node({ frequency: param(), Q: param(), gain: param(), type: '' }); }
    createOscillator() { const o = node({ frequency: param(), type: '', started: false, stopped: false, start() { this.started = true; }, stop() { this.stopped = true; } }); sources.push(o); return o; }
    createBufferSource() { const s = node({ buffer: null, loop: false, started: false, stopped: false, start() { this.started = true; }, stop() { this.stopped = true; } }); sources.push(s); return s; }
    createBuffer(ch, len) { return { getChannelData: () => new Float32Array(len) }; }
    createStereoPanner() { return node({ pan: param(0) }); }
    createWaveShaper() { return node({ curve: null, oversample: 'none' }); }
    createDynamicsCompressor() { return node({ threshold: param(), knee: param(), ratio: param(), attack: param(), release: param() }); }
  }
  return { Ctx, sources };
}

test('Ton: im Spiel läuft kein Dauerrauschen, nur der Motor (und der schweigt außerhalb des Autos)', () => {
  const { Ctx, sources } = fakeAudio();
  globalThis.AudioContext = Ctx;
  try {
    const s = new Sound();
    s.unlock();
    for (let i = 0; i < 10; i++) s.setEngine(false, 0, 0); // zu Fuß
    // Einzige Dauerquellen: die Rauschschichten (Umgebung, Nachtleben, Reifen/Wind) – alle stumm, bis etwas sie setzt
    const looping = sources.filter((x) => x.started && !x.stopped && x.loop);
    assert.equal(looping.length, s.loops.length, 'nur die registrierten Schichten schleifen');
    for (const g of s.loops) assert.equal(g.gain.value, 0, 'Schicht startet stumm');
    for (const [k, g] of Object.entries(s.amb)) assert.equal(g.gain.value, 0, `${k} startet stumm`);
    const bursts = () => sources.filter((x) => x.buffer && x.started && !x.loop);
    assert.ok(bursts().every((x) => x.stopped), 'Rausch-Stöße (Crash, Tür) enden von selbst');
    assert.equal(s.engine.g.gain.target, 0, 'Motor stumm, wenn man nicht fährt');
    s.play('crash', 1);
    assert.ok(bursts().length > 0 && bursts().every((x) => x.stopped), 'Crash-Geräusch ist begrenzt');
  } finally { delete globalThis.AudioContext; }
});

test('Umgebungsklang: Mischung steuert die Schichten, Stille bleibt still, Martinshorn wechselt den Ton', () => {
  const { Ctx } = fakeAudio();
  globalThis.AudioContext = Ctx;
  try {
    const s = new Sound();
    s.unlock();
    const zero = { hum: 0, traffic: 0, birds: 0, bar: 0, water: 0, rumble: 0, sirens: [] };
    s.setAmbience(zero);
    for (const [k, g] of Object.entries(s.amb)) assert.equal(g.gain.target, 0, `${k} still`);
    for (const b of s.babble) assert.equal(b.g.gain.target, 0, 'kein Stimmengewirr ohne Bars');
    assert.equal(s.siren.g.gain.target, 0, 'kein Horn ohne Einsatzwagen');
    s.setAmbience({ ...zero, hum: 1, traffic: 1, rumble: 1, sirens: [{ d: 100, gain: 1, high: true }] });
    for (const k of ['hum', 'traffic', 'rumble']) assert.ok(s.amb[k].gain.target > 0, `${k} hörbar`);
    assert.equal(s.amb.bar.gain.target, 0, 'Bar bleibt still');
    assert.ok(s.siren.g.gain.target > 0);
    assert.equal(s.siren.o.frequency.target, 585);
    s.setAmbience({ ...zero, sirens: [{ d: 100, gain: 1, high: false }] });
    assert.equal(s.siren.o.frequency.target, 440);
  } finally { delete globalThis.AudioContext; }
});

test('Töne im Nahverkehr: Ein-/Aussteigen, Auf-/Abspringen, Türgong, Trinkgeld, Zug voraus – jeder Ton existiert', async () => {
  const { soundFor, SYNTH } = await import('../web/src/audio.js');
  assert.equal(soundFor({ type: 'board' }), 'door');
  assert.equal(soundFor({ type: 'alight' }), 'door');
  assert.equal(soundFor({ type: 'board', hop: true }), 'hit', 'Aufspringen rumst');
  assert.equal(soundFor({ type: 'alight', hop: true }), 'hit', 'Abspringen rumst');
  assert.equal(soundFor({ type: 'train-take' }), 'door');
  assert.notEqual(soundFor({ type: 'doors-open' }), soundFor({ type: 'doors-close' }), 'Türgong auf/zu klingt verschieden');
  assert.equal(soundFor({ type: 'tip' }), 'pickup');
  assert.equal(soundFor({ type: 'train-blocked' }), 'tram-bell');
  assert.equal(soundFor({ type: 'shot', weapon: 'pistol' }), 'pistol', 'bestehende Zuordnung bleibt');
  assert.equal(soundFor({ type: 'unbekannt' }), null);
  for (const type of ['board', 'alight', 'train-take', 'doors-open', 'doors-close', 'tip', 'train-blocked', 'crash', 'bump', 'knock', 'wasted', 'respawn'])
    assert.ok(SYNTH[soundFor({ type })], `${type} → ${soundFor({ type })}: Klang fehlt`);
});

test('Aquaplaning platscht', async () => {
  const { soundFor, SYNTH } = await import('../web/src/audio.js');
  assert.equal(soundFor({ type: 'aquaplane' }), 'splash');
  assert.ok(SYNTH.splash);
});

test('Fahrzeugklang: Motor folgt der Zündfrequenz, Reifen/Quietschen nach Zustand, im Auto wird es draußen dumpf', () => {
  const { Ctx } = fakeAudio();
  globalThis.AudioContext = Ctx;
  try {
    const s = new Sound();
    s.unlock();
    const tires = { roll: 0.5, cobble: 0, wet: 0, snow: 0, skid: 1, slide: 0, wind: 0.25 };
    s.setVehicle(true, { rpm: 3000, fire: 100, load: 0, norm: 0.4, diesel: false }, tires, { inCar: true, rain: 1 });
    const idle = s.engine.g.gain.target;
    s.setVehicle(true, { rpm: 3000, fire: 100, load: 1, norm: 0.4, diesel: false }, tires, { inCar: true, rain: 1 });
    assert.equal(s.engine.o1.frequency.target, 100, 'Grundton = Zündfrequenz');
    assert.equal(s.engine.o2.frequency.target, 50, 'halbe Zündfrequenz für den unrunden Lauf');
    // Pegel seit dem Klang-Umbau (b4d1fc3f) mit Stimmprofil und Dämmung: unter Last deutlich lauter als ohne
    assert.ok(s.engine.g.gain.target > idle * 2, `unter Last laut (${s.engine.g.gain.target} gegen ${idle})`);
    assert.ok(s.engine.sqg.gain.target > 0, 'Quietschen beim Rutschen');
    assert.ok(s.engine.tires.roof.g.gain.target > 0, 'Regen trommelt aufs Dach');
    assert.equal(s.engine.clm.gain.target, 0, 'Benziner nagelt nicht');
    s.setVehicle(true, { rpm: 1500, fire: 75, load: 0.5, norm: 0.3, diesel: true }, null);
    assert.ok(s.engine.clm.gain.target > 0, 'Diesel nagelt');
    s.setVehicle(false, null, null, { inCar: false });
    assert.equal(s.engine.bus.gain.target, 0, 'zu Fuß schweigt das Fahrzeug');
    const zero = { hum: 0, traffic: 0, birds: 0, bar: 0, water: 0, rumble: 0, sirens: [] };
    s.setAmbience({ ...zero, muffle: 0 });
    const open = s.muffleF.frequency.target;
    s.setAmbience({ ...zero, muffle: 0.65 });
    assert.ok(s.muffleF.frequency.target < open / 5, 'im Auto dumpf');
  } finally { delete globalThis.AudioContext; }
});

test('Fremde Autos: Stimmen bleiben ihrem Auto treu, Doppler hebt den Ton beim Näherkommen', () => {
  const { Ctx } = fakeAudio();
  globalThis.AudioContext = Ctx;
  try {
    const s = new Sound();
    s.unlock();
    const v = (id, rate, gain = 0.5) => ({ id, gain, pan: 0.3, rate, fire: 40, diesel: false, tire: 0.3 });
    s.setVoices([v(7, 1.05), v(9, 0.95)]);
    const slot7 = s.voices.findIndex((x) => x.id === 7);
    assert.ok(Math.abs(s.voices[slot7].o.frequency.target - 42) < 1e-9, 'nähert sich: höher');
    s.setVoices([v(12, 1), v(7, 0.95)]);
    assert.equal(s.voices.findIndex((x) => x.id === 7), slot7, 'Auto 7 behält seine Stimme');
    assert.ok(s.voices[slot7].o.frequency.target < 40, 'entfernt sich: tiefer');
    s.setVoices([]);
    assert.ok(s.voices.every((x) => x.id === null && x.g.gain.target === 0), 'ohne Autos still');
  } finally { delete globalThis.AudioContext; }
});

test('Nachtleben: Stimmengewirr, Club-Bass und Richtung aus der Mischung', () => {
  const { Ctx, sources } = fakeAudio();
  globalThis.AudioContext = Ctx;
  try {
    const s = new Sound();
    s.unlock();
    const zero = { hum: 0, traffic: 0, birds: 0, bar: 0, water: 0, rumble: 0, sirens: [] };
    const before = sources.length;
    s.setAmbience({ ...zero, bar: 0.8, music: 0.8, barPan: -1 });
    assert.ok(s.babble.every((b) => b.g.gain.target > 0), 'Stimmen hörbar');
    assert.ok(s.barPan.pan.target < 0, 'von links');
    const kicks = sources.slice(before).filter((x) => x.type === 'sine' && !x.buffer);
    assert.ok(kicks.length >= 1, 'Bass aus dem Club');
    assert.ok(sources.slice(before).every((x) => x.stopped), 'Einzelklänge enden von selbst');
  } finally { delete globalThis.AudioContext; }
});
