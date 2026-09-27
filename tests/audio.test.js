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
    createBiquadFilter() { return node({ frequency: param(), type: '' }); }
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
    const looping = sources.filter((x) => x.started && !x.stopped && x.loop);
    assert.equal(looping.length, 0, 'keine endlos schleifende Geräuschquelle (Rauschen)');
    assert.ok(sources.filter((x) => x.buffer && x.started).every((x) => x.stopped), 'Rausch-Stöße (Crash, Tür) enden von selbst');
    assert.equal(s.engine.g.gain.target, 0, 'Motor stumm, wenn man nicht fährt');
    s.play('crash', 1);
    assert.ok(sources.filter((x) => x.buffer && x.started).every((x) => x.stopped), 'Crash-Geräusch ist begrenzt');
  } finally { delete globalThis.AudioContext; }
});
