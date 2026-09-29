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
    createBiquadFilter() { return node({ frequency: param(), Q: param(), type: '' }); }
    createOscillator() { const o = node({ frequency: param(), type: '', started: false, stopped: false, start() { this.started = true; }, stop() { this.stopped = true; } }); sources.push(o); return o; }
    createBufferSource() { const s = node({ buffer: null, loop: false, started: false, stopped: false, start() { this.started = true; }, stop() { this.stopped = true; } }); sources.push(s); return s; }
    createBuffer(ch, len) { return { getChannelData: () => new Float32Array(len) }; }
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
    // Einzige Dauerquellen: die Umgebungsschichten (ambience.js) – stumm, bis eine Mischung gesetzt ist
    const looping = sources.filter((x) => x.started && !x.stopped && x.loop);
    assert.equal(looping.length, Object.keys(s.amb).length, 'nur die Umgebungsschichten schleifen');
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
