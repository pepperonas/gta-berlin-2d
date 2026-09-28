// Selbst erzeugte Klänge (Web Audio, keine Fremd-Samples). Austauschbar: liegt in assets/manifest.json
// unter "sounds" eine Datei für einen Schlüssel (crash, horn, pickup, success, fail, ui, uiMove, door, tick, hit),
// wird diese statt der Synthese abgespielt.
export class Sound {
  constructor() { this.ctx = null; this.buffers = {}; this.master = null; this.engine = null; this.muted = false; this.manifest = {}; }

  // Browser verlangen eine Nutzergeste; in der Xbox-Hülle ist Autoplay per Startargument erlaubt.
  unlock() {
    if (!this.ctx) {
      const AC = globalThis.AudioContext || globalThis.webkitAudioContext;
      if (!AC) return;
      this.ctx = new AC();
      this.master = this.ctx.createGain(); this.master.gain.value = 0.55; this.master.connect(this.ctx.destination);
      this.noise = this.makeNoise();
      this.startEngine();
      this.startAmbience();
      this.loadOverrides();
    }
    if (this.ctx.state === 'suspended') this.ctx.resume().catch(() => {});
  }

  get ready() { return this.ctx && this.ctx.state === 'running'; }

  setManifest(m) { this.manifest = m?.sounds ?? {}; }

  async loadOverrides() {
    for (const [key, file] of Object.entries(this.manifest)) {
      try {
        const res = await fetch('assets/' + file);
        this.buffers[key] = await this.ctx.decodeAudioData(await res.arrayBuffer());
      } catch { console.warn(`Sound "${key}" (${file}) nicht ladbar – Synthese bleibt`); }
    }
  }

  makeNoise() {
    const len = this.ctx.sampleRate;
    const buf = this.ctx.createBuffer(1, len, this.ctx.sampleRate);
    const d = buf.getChannelData(0);
    for (let i = 0; i < len; i++) d[i] = Math.random() * 2 - 1;
    return buf;
  }

  startEngine() {
    const c = this.ctx;
    const o1 = c.createOscillator(), o2 = c.createOscillator();
    o1.type = 'sawtooth'; o2.type = 'square';
    const f = c.createBiquadFilter(); f.type = 'lowpass'; f.frequency.value = 380;
    const g = c.createGain(); g.gain.value = 0;
    o1.connect(f); o2.connect(f); f.connect(g); g.connect(this.master);
    o1.start(); o2.start();
    this.engine = { o1, o2, f, g };
  }

  // Umgebung: gefiltertes Rauschen je Schicht (Stadt, Verkehr, Wasser, Bar, Hochbahn) plus Martinshorn-Oszillator
  startAmbience() {
    const c = this.ctx, layer = (type, freq, q = 0.7) => {
      const s = c.createBufferSource(); s.buffer = this.noise; s.loop = true;
      const f = c.createBiquadFilter(); f.type = type; f.frequency.value = freq; f.Q.value = q;
      const g = c.createGain(); g.gain.value = 0;
      s.connect(f); f.connect(g); g.connect(this.master); s.start(0, Math.random());
      return g;
    };
    this.amb = { hum: layer('lowpass', 180), traffic: layer('bandpass', 420, 0.6), water: layer('highpass', 1400), bar: layer('bandpass', 850, 1.8), rumble: layer('lowpass', 90), rain: layer('highpass', 2600) };
    const o = c.createOscillator(); o.type = 'triangle'; o.frequency.value = 440;
    const g = c.createGain(); g.gain.value = 0; o.connect(g); g.connect(this.master); o.start();
    this.siren = { o, g };
    this.nextChirp = 0;
  }

  // mix aus ambience.js (0..1 je Schicht, sirens nach Entfernung)
  setAmbience(mix) {
    if (!this.ready || !this.amb) return;
    const t = this.ctx.currentTime, a = this.amb, set = (g, v) => g.gain.setTargetAtTime(v, t, 0.6);
    set(a.hum, 0.018 * mix.hum); set(a.traffic, 0.05 * mix.traffic); set(a.water, 0.012 * mix.water);
    set(a.bar, 0.05 * mix.bar * (0.7 + 0.3 * Math.sin(t * 2.3) * Math.sin(t * 0.7))); set(a.rumble, 0.16 * mix.rumble); set(a.rain, 0.07 * (mix.rain ?? 0));
    const sr = mix.sirens?.[0];
    this.siren.g.gain.setTargetAtTime(sr ? 0.07 * sr.gain : 0, t, 0.15);
    if (sr) this.siren.o.frequency.setTargetAtTime(sr.high ? 585 : 440, t, 0.02);
    if (mix.birds > 0.02 && t > this.nextChirp) { // Vogelstimmen: kurze Tonfolgen, je mehr Grün, desto öfter
      const base = 2400 + Math.random() * 2600, n = 2 + Math.floor(Math.random() * 4);
      for (let i = 0; i < n; i++) this.tone(base * (1 + (Math.random() - 0.5) * 0.3), 0.07, { type: 'sine', gain: 0.025 * mix.birds, at: i * 0.09, slide: (Math.random() - 0.3) * 900 });
      this.nextChirp = t + 0.4 + Math.random() * 2.2 / mix.birds;
    }
  }

  // Kirchenglocke: n Schläge (Grundton + unharmonische Teiltöne, langer Nachhall)
  bells(n) {
    if (!this.ready) return;
    for (let i = 0; i < n; i++) for (const [f, g] of [[196, 0.09], [392, 0.05], [470, 0.04], [588, 0.03], [784, 0.02]]) this.tone(f, 3.2, { type: 'sine', gain: g, at: i * 2.1 });
  }

  // speedNorm 0..1, throttle 0..1; active=false blendet den Motor aus.
  setEngine(active, speedNorm, throttle) {
    if (!this.ready) return;
    const e = this.engine, t = this.ctx.currentTime;
    const base = 38 + speedNorm * 95 + throttle * 12;
    e.o1.frequency.setTargetAtTime(base, t, 0.08);
    e.o2.frequency.setTargetAtTime(base * 0.502, t, 0.08);
    e.f.frequency.setTargetAtTime(260 + speedNorm * 900 + throttle * 400, t, 0.1);
    e.g.gain.setTargetAtTime(active ? 0.07 + throttle * 0.06 + speedNorm * 0.04 : 0, t, 0.12);
  }

  play(name, strength = 1) {
    if (!this.ready) return;
    if (this.buffers[name]) {
      const s = this.ctx.createBufferSource(); s.buffer = this.buffers[name];
      const g = this.ctx.createGain(); g.gain.value = strength; s.connect(g); g.connect(this.master); s.start();
      return;
    }
    const synth = SYNTH[name];
    if (synth) synth(this, strength);
  }

  tone(freq, dur, { type = 'square', gain = 0.12, at = 0, slide = 0 } = {}) {
    const c = this.ctx, t = c.currentTime + at;
    const o = c.createOscillator(), g = c.createGain();
    o.type = type; o.frequency.setValueAtTime(freq, t);
    if (slide) o.frequency.exponentialRampToValueAtTime(Math.max(30, freq + slide), t + dur);
    g.gain.setValueAtTime(0, t); g.gain.linearRampToValueAtTime(gain, t + 0.01); g.gain.exponentialRampToValueAtTime(0.0001, t + dur);
    o.connect(g); g.connect(this.master); o.start(t); o.stop(t + dur + 0.05);
  }

  burst(dur, { freq = 800, gain = 0.3, type = 'lowpass' } = {}) {
    const c = this.ctx, t = c.currentTime;
    const s = c.createBufferSource(); s.buffer = this.noise;
    const f = c.createBiquadFilter(); f.type = type; f.frequency.value = freq;
    const g = c.createGain(); g.gain.setValueAtTime(gain, t); g.gain.exponentialRampToValueAtTime(0.0001, t + dur);
    s.connect(f); f.connect(g); g.connect(this.master); s.start(t, Math.random() * 0.5); s.stop(t + dur + 0.05);
  }
}

const SYNTH = {
  crash: (s, k) => { s.burst(0.35 + k * 0.3, { freq: 600 + k * 1800, gain: 0.25 + k * 0.4 }); s.tone(90, 0.25, { type: 'sine', gain: 0.25 * k, slide: -50 }); },
  hit: (s) => { s.burst(0.12, { freq: 400, gain: 0.3 }); s.tone(160, 0.12, { type: 'sine', gain: 0.2, slide: -80 }); },
  horn: (s) => { s.tone(392, 0.45, { gain: 0.08 }); s.tone(494, 0.45, { gain: 0.08 }); },
  // Straßenbahnklingel: zwei helle Schläge
  'tram-bell': (s) => { for (const at of [0, 0.22]) { s.tone(1568, 0.5, { type: 'sine', gain: 0.08, at }); s.tone(2350, 0.35, { type: 'sine', gain: 0.04, at }); } },
  door: (s) => { s.burst(0.08, { freq: 1500, gain: 0.25, type: 'bandpass' }); s.tone(120, 0.08, { type: 'sine', gain: 0.2, at: 0.05 }); },
  ui: (s) => s.tone(880, 0.09, { type: 'triangle', gain: 0.12 }),
  'ui-move': (s) => s.tone(620, 0.05, { type: 'triangle', gain: 0.08 }),
  'ui-back': (s) => s.tone(440, 0.09, { type: 'triangle', gain: 0.1, slide: -120 }),
  tick: (s) => s.tone(1200, 0.05, { type: 'square', gain: 0.06 }),
  pickup: (s) => [523, 659, 784].forEach((f, i) => s.tone(f, 0.18, { type: 'triangle', gain: 0.14, at: i * 0.09 })),
  'mission-start': (s) => [392, 523].forEach((f, i) => s.tone(f, 0.2, { type: 'triangle', gain: 0.14, at: i * 0.12 })),
  'mission-success': (s) => [523, 659, 784, 1047, 784, 1047].forEach((f, i) => s.tone(f, 0.22, { type: 'square', gain: 0.09, at: i * 0.11 })),
  'mission-fail': (s) => [392, 330, 262, 196].forEach((f, i) => s.tone(f, 0.3, { type: 'sawtooth', gain: 0.08, at: i * 0.18 })),
  carjack: (s) => s.tone(700, 0.3, { type: 'sawtooth', gain: 0.05, slide: 400 }),
  // Waffen: Knall aus gefiltertem Rauschen plus tiefer Schlag
  pistol: (s) => { s.burst(0.16, { freq: 2600, gain: 0.45 }); s.tone(140, 0.12, { type: 'sine', gain: 0.3, slide: -90 }); },
  smg: (s) => { s.burst(0.07, { freq: 3200, gain: 0.32 }); s.tone(170, 0.06, { type: 'sine', gain: 0.18, slide: -80 }); },
  shotgun: (s) => { s.burst(0.42, { freq: 1500, gain: 0.6 }); s.tone(80, 0.3, { type: 'sine', gain: 0.4, slide: -40 }); },
  swing: (s) => s.burst(0.12, { freq: 900, gain: 0.12, type: 'bandpass' }),
  punch: (s) => { s.burst(0.08, { freq: 500, gain: 0.35 }); s.tone(110, 0.1, { type: 'sine', gain: 0.3, slide: -50 }); },
  thud: (s) => { s.burst(0.1, { freq: 1200, gain: 0.25, type: 'bandpass' }); s.tone(90, 0.12, { type: 'triangle', gain: 0.2 }); },
  impact: (s) => s.burst(0.05, { freq: 3500, gain: 0.12, type: 'highpass' }),
  reload: (s) => { s.tone(1400, 0.04, { gain: 0.06 }); s.tone(900, 0.05, { gain: 0.06, at: 0.12 }); },
  reloaded: (s) => s.tone(1800, 0.04, { gain: 0.07 }),
  weapon: (s) => s.tone(1100, 0.04, { type: 'triangle', gain: 0.07 }),
};
