import { engineSpectrum, voiceFor } from './enginevoice.js';

// Selbst erzeugte Klänge (Web Audio, keine Fremd-Samples). Austauschbar: liegt in assets/manifest.json
// unter "sounds" eine Datei für einen Schlüssel (crash, horn, pickup, success, fail, ui, uiMove, door, tick, hit),
// wird diese statt der Synthese abgespielt.
//
// Signalweg: alles Draußen (Umgebung, fremde Autos, Nachtleben) läuft über den Bus „outside“ mit einem Tiefpass, der im
// Auto (Karosserie) und bei Schneedecke die Höhen schluckt; das eigene Fahrzeug (Motor, Reifen, Fahrtwind, Regen aufs
// Dach) erhält eine zum Modell passende Klangmischung. Ein Kompressor vor dem Ausgang verhindert Übersteuern, wenn viel zugleich passiert.
// Was klingt, rechnen ambience.js, nightlife.js und soundscape.js rein aus; hier wird nur synthetisiert.
export class Sound {
  constructor() { this.ctx = null; this.buffers = {}; this.master = null; this.engine = null; this.muted = false; this.manifest = {}; }

  // Browser verlangen eine Nutzergeste; in der Xbox-Hülle ist Autoplay per Startargument erlaubt.
  unlock() {
    if (!this.ctx) {
      const AC = globalThis.AudioContext || globalThis.webkitAudioContext;
      if (!AC) return;
      this.ctx = new AC();
      const c = this.ctx;
      this.master = c.createGain(); this.master.gain.value = this.muted ? 0 : 0.55;
      if (c.createDynamicsCompressor) {
        const k = c.createDynamicsCompressor();
        k.threshold.value = -14; k.knee.value = 12; k.ratio.value = 4; k.attack.value = 0.005; k.release.value = 0.25;
        this.master.connect(k); k.connect(c.destination);
      } else this.master.connect(c.destination);
      this.outside = c.createGain();
      this.muffleF = c.createBiquadFilter(); this.muffleF.type = 'lowpass'; this.muffleF.frequency.value = 18000; this.muffleF.Q.value = 0.5;
      this.outside.connect(this.muffleF); this.muffleF.connect(this.master);
      this.noise = this.makeNoise();
      this.loops = [];
      this.startEngine();
      this.startAmbience();
      this.loadOverrides();
    }
    if (this.ctx.state === 'suspended') this.ctx.resume().catch(() => {});
  }

  get ready() { return this.ctx && this.ctx.state === 'running'; }

  toggleMute() {
    this.muted = !this.muted;
    if (this.master && this.ctx) this.master.gain.setTargetAtTime(this.muted ? 0 : 0.55, this.ctx.currentTime, 0.025);
    return this.muted;
  }

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
    const len = this.ctx.sampleRate * 2;
    const buf = this.ctx.createBuffer(1, len, this.ctx.sampleRate);
    const d = buf.getChannelData(0);
    for (let i = 0; i < len; i++) d[i] = Math.random() * 2 - 1;
    return buf;
  }

  // Stereo-Richtung (fällt ohne StereoPanner auf eine neutrale Verstärkung zurück)
  panner() {
    const c = this.ctx;
    if (c.createStereoPanner) return c.createStereoPanner();
    const g = c.createGain(); g.pan = null; return g;
  }
  setPan(p, v, t) { if (p.pan) p.pan.setTargetAtTime(Math.max(-1, Math.min(1, v)), t, 0.1); }

  // Rauschschicht: Rauschen → Filter → Verstärkung (stumm) → Ziel; Rückgabe { g, f }
  noiseLayer(type, freq, q = 0.7, dest = this.outside) {
    const c = this.ctx, s = c.createBufferSource(); s.buffer = this.noise; s.loop = true;
    const f = c.createBiquadFilter(); f.type = type; f.frequency.value = freq; f.Q.value = q;
    const g = c.createGain(); g.gain.value = 0;
    s.connect(f); f.connect(g); g.connect(dest); s.start(0, Math.random() * 1.5);
    this.loops.push(g);
    return { g, f };
  }

  // Weiche Sättigung (Verbrennungsmotor klingt rau, nicht nach reiner Sägezahnwelle)
  shaper(drive = 2.5) {
    const c = this.ctx;
    if (!c.createWaveShaper) return c.createGain();
    const ws = c.createWaveShaper(), n = 1024, curve = new Float32Array(n);
    for (let i = 0; i < n; i++) { const x = i / (n - 1) * 2 - 1; curve[i] = Math.tanh(drive * x) / Math.tanh(drive); }
    ws.curve = curve; ws.oversample = '2x';
    return ws;
  }

  // Druckimpulse eines ganzen Arbeitszyklus statt generischer Sägezahn-Motoren.
  engineWave(osc, profile, cylinders, intake = false) {
    if (!this.ctx.createPeriodicWave || !osc.setPeriodicWave) return false;
    this.engineWaves ??= new Map();
    const key = `${profile.key}|${cylinders}|${intake}`;
    if (!this.engineWaves.has(key)) {
      const { real, imag } = engineSpectrum(profile, cylinders, intake);
      this.engineWaves.set(key, this.ctx.createPeriodicWave(real, imag));
    }
    if (osc.voiceKey !== key) { osc.setPeriodicWave(this.engineWaves.get(key)); osc.voiceKey = key; }
    return true;
  }

  // Kurzer Druckabbau des Turbos beziehungsweise dezentes Auspuff-Nachverbrennen.
  engineRelease(boost, pop) {
    const c = this.ctx, t = c.currentTime, s = c.createBufferSource(); s.buffer = this.noise;
    const f = c.createBiquadFilter(); f.type = 'bandpass'; f.frequency.value = pop ? 130 : 1400; f.Q.value = pop ? 0.7 : 0.8;
    const g = c.createGain(); g.gain.setValueAtTime(0, t);
    g.gain.linearRampToValueAtTime(pop ? 0.055 : boost * 0.02, t + 0.008);
    g.gain.exponentialRampToValueAtTime(0.0001, t + (pop ? 0.12 : 0.26));
    s.connect(f); f.connect(g); g.connect(this.engine.bus);
    s.onended = () => { s.disconnect(); f.disconnect(); g.disconnect(); };
    s.start(t); s.stop(t + 0.3);
  }

  startEngine() {
    const c = this.ctx;
    const bus = c.createGain(); bus.gain.value = 0; bus.connect(this.master);
    const osc = (type) => { const o = c.createOscillator(); o.type = type; o.start(); return o; };
    const o1 = osc('sawtooth'), o2 = osc('square'), o3 = osc('sawtooth');
    const g1 = c.createGain(), g2 = c.createGain(), g3 = c.createGain();
    g1.gain.value = 0.5; g2.gain.value = 0.35; g3.gain.value = 0.15;
    const sh = this.shaper(1.45);
    const f = c.createBiquadFilter(); f.type = 'lowpass'; f.frequency.value = 380; f.Q.value = 0.65;
    const body = c.createBiquadFilter(); body.type = 'peaking'; body.frequency.value = 140; body.Q.value = 0.65; body.gain.value = 5;
    const bass = c.createBiquadFilter(); bass.type = 'lowshelf'; bass.frequency.value = 220; bass.gain.value = 4;
    o1.connect(g1); o2.connect(g2); o3.connect(g3); g1.connect(sh); g2.connect(body); g3.connect(sh); sh.connect(f); f.connect(body); body.connect(bass);
    const g = c.createGain(); g.gain.value = 0; bass.connect(g); g.connect(bus);
    // Auspuff: Rauschen, im Zündtakt moduliert
    const ex = this.noiseLayer('bandpass', 220, 1.3, bus);
    const am = osc('square'), amg = c.createGain(); amg.gain.value = 0; am.connect(amg); amg.connect(ex.g.gain);
    // Diesel-Nageln: hohes Rauschen, im Zündtakt geschaltet
    const cl = this.noiseLayer('bandpass', 1500, 0.6, bus);
    // Elektromotor: Summen, das mit der Drehzahl steigt (statt Zündtakt)
    const wh = osc('sine'), whg = c.createGain(); whg.gain.value = 0; wh.connect(whg); whg.connect(bus);
    const clm = c.createGain(); clm.gain.value = 0; am.connect(clm); clm.connect(cl.g.gain);
    const intake = this.noiseLayer('bandpass', 900, 0.8, bus);
    const turboNoise = this.noiseLayer('bandpass', 2800, 0.6, bus);
    const turbo = osc('sine'), turboG = c.createGain(); turboG.gain.value = 0; turbo.connect(turboG); turboG.connect(bus);
    const reverse = osc('sine'), reverseG = c.createGain(); reverseG.gain.value = 0; reverse.connect(reverseG); reverseG.connect(bus);
    const tires = {
      roll: this.noiseLayer('lowpass', 300, 0.6, bus), cobble: this.noiseLayer('lowpass', 140, 1.5, bus),
      wet: this.noiseLayer('highpass', 2600, 0.5, bus), snow: this.noiseLayer('bandpass', 1500, 0.9, bus),
      slide: this.noiseLayer('bandpass', 700, 0.8, bus), wind: this.noiseLayer('bandpass', 520, 0.6, bus),
      squeal: this.noiseLayer('bandpass', 2300, 5, bus),
      roof: this.noiseLayer('bandpass', 1600, 0.6, bus), roofLow: this.noiseLayer('lowpass', 220, 0.8, bus),
    };
    // Quietschen: zwei leicht verstimmte Töne mit Vibrato
    const sq = [osc('triangle'), osc('triangle')], sqg = c.createGain(); sqg.gain.value = 0;
    sq[0].frequency.value = 960; sq[1].frequency.value = 1010;
    const vib = osc('sine'), vibg = c.createGain(); vib.frequency.value = 9; vibg.gain.value = 25; vib.connect(vibg);
    for (const o of sq) { vibg.connect(o.frequency); o.connect(sqg); }
    sqg.connect(bus);
    this.engine = { g1, g2, g3, body, bass, intake, turboNoise, turbo, turboG, reverse, reverseG, o1, o2, o3, f, g, bus, ex, am, amg, cl, clm, tires, sqg, sq, wh, whg };
  }

  // eng: soundscape.js stepEngine-Zustand (rpm, fire, load, norm, diesel); tires: soundscape.js tireState
  setVehicle(active, eng, tires, { inCar = active, rain = 0 } = {}) {
    if (!this.ready) return;
    const e = this.engine, t = this.ctx.currentTime, set = (p, v, tc = 0.08) => p.setTargetAtTime(v, t, tc);
    set(e.bus.gain, active || inCar ? 1 : 0, 0.15);
    const p = eng?.profile ?? voiceFor(undefined, eng ?? {}), load = eng?.load ?? 0, n = eng?.norm ?? 0;
    const combustion = active && eng && !eng.electric;
    const damping = inCar ? 1 - p.insulation * 0.4 : 1;
    const boost = combustion ? eng.boost ?? 0 : 0;
    set(e.turbo.frequency, 950 + boost * 2100, 0.16);
    set(e.turboG.gain, boost * 0.003 * damping, 0.15);
    set(e.turboNoise.g.gain, boost * 0.008 * damping, 0.12);
    set(e.intake.g.gain, combustion ? load * n * 0.012 * damping : 0);
    set(e.intake.f.frequency, 350 + n * 850);
    set(e.reverse.frequency, 180 + (eng?.speed ?? 0) * 7, 0.1);
    set(e.reverseG.gain, active && eng?.reverse && !eng.electric ? Math.min(0.018, (eng.speed ?? 0) * 0.0003) : 0);
    if (active && eng?.electric) {
      set(e.g.gain, 0); set(e.wh.frequency, 160 + (eng.rpm ?? 0) / 16000 * 2200, 0.08);
      const moving = Math.min(1, (eng.speed ?? 0) / 25);
      set(e.whg.gain, moving * (0.006 + load * 0.014 + (eng.regen ?? 0) * 0.009), 0.1);
    } else if (combustion) {
      set(e.whg.gain, 0);
      const fire = Math.max(12, eng.fire), custom = !!eng.profile && this.engineWave(e.o1, p, eng.cyl);
      if (custom) {
        this.engineWave(e.o3, p, eng.cyl, true); e.o2.type = 'triangle';
        const flutter = 1 + p.rough * 0.025 * (1 - n) * Math.sin(t * 13.7);
        set(e.o1.frequency, Math.max(4, eng.cycle) * flutter, 0.035);
        set(e.o3.frequency, Math.max(4, eng.cycle), 0.035);
        set(e.o2.frequency, eng.rpm / 60, 0.035);
      } else {
        set(e.o1.frequency, fire, 0.035); set(e.o2.frequency, fire * 0.5, 0.035); set(e.o3.frequency, fire * 2, 0.035);
      }
      // Kurbelwellen-Grundton umgeht die Sättigung und trägt das tiefe Knurren unter Last.
      set(e.g1.gain, 0.58); set(e.g2.gain, 0.2 + p.rough * 0.65 + load * 0.18); set(e.g3.gain, 0.04 + load * 0.1);
      set(e.am.frequency, fire, 0.035);
      set(e.body.frequency, p.resonance); set(e.body.gain, 4 + p.rough * 8);
      set(e.bass.gain, 3 + load * 2);
      set(e.f.frequency, (220 + p.brightness * (0.18 + n * 0.5 + load * 0.38)) * damping, 0.06);
      set(e.g.gain, (0.04 + load * 0.07 + n * 0.018) * p.volume * damping, 0.045);
      set(e.ex.f.frequency, Math.min(900, p.resonance + fire * 0.8), 0.05);
      if (e.profileKey === p.key && e.previousLoad > 0.55 && load < 0.12 && !eng.shiftT && t > (e.releaseAt ?? 0)) {
        if (e.previousBoost > 0.25) this.engineRelease(e.previousBoost, false);
        else if (p.pops && n > 0.55) this.engineRelease(0, true);
        e.releaseAt = t + 0.8;
      }
    } else { set(e.g.gain, 0, 0.12); set(e.whg.gain, 0, 0.12); }
    // Positive Hüllkurven: Pulsieren ohne Phasenumkehr des Rauschens.
    const exhaust = combustion ? (0.009 + load * 0.025) * p.volume * damping : 0;
    const clatter = combustion && eng.diesel ? (0.004 + load * 0.006) * damping : 0;
    set(e.ex.g.gain, exhaust); set(e.amg.gain, exhaust * 0.65);
    set(e.cl.g.gain, clatter); set(e.clm.gain, clatter * 0.8);
    e.previousLoad = active ? load : 0; e.previousBoost = boost; e.profileKey = active ? p.key : null;
    const T = e.tires, tr = active && tires ? tires : null;
    set(T.roll.g.gain, tr ? 0.05 * tr.roll : 0, 0.1); set(T.roll.f.frequency, 250 + (tr?.roll ?? 0) * 900, 0.1);
    set(T.cobble.g.gain, tr ? 0.12 * tr.cobble * (0.6 + 0.4 * Math.random()) : 0, 0.05);
    set(T.wet.g.gain, tr ? 0.06 * tr.wet : 0, 0.1);
    set(T.snow.g.gain, tr ? 0.07 * tr.snow * (0.5 + 0.8 * Math.random()) : 0, 0.04);
    set(T.slide.g.gain, tr ? 0.09 * tr.slide : 0, 0.05);
    set(T.wind.g.gain, tr ? 0.06 * tr.wind : 0, 0.2); set(T.wind.f.frequency, 380 + (tr?.wind ?? 0) * 700, 0.2);
    set(T.squeal.g.gain, tr ? 0.05 * tr.skid : 0, 0.04);
    set(e.sqg.gain, tr ? 0.035 * tr.skid : 0, 0.04);
    if (tr?.skid) for (const [i, o] of e.sq.entries()) set(o.frequency, (i ? 1010 : 960) - tr.roll * 180, 0.1);
    // Regen aufs Blechdach: hell trommelnd plus dumpfes Prasseln
    const r = inCar ? Math.min(1.6, rain) : 0;
    set(T.roof.g.gain, 0.05 * Math.min(1, r) + 0.03 * Math.max(0, r - 1), 0.4);
    set(T.roofLow.g.gain, 0.06 * Math.max(0, r - 0.3), 0.4);
  }

  // Kompatibel zur alten Schnittstelle (Menüs, Tests): speedNorm 0..1, throttle 0..1
  setEngine(active, speedNorm, throttle) {
    const rpm = 800 + speedNorm * 5000 + throttle * 600;
    this.setVehicle(active, { rpm, fire: rpm / 60 * 2, load: throttle, norm: speedNorm, diesel: false }, null, { inCar: active });
  }

  // Fremde Fahrzeuge: vier Stimmen (je Motor + Reifen, Richtung, Doppler), feste Zuordnung Auto → Stimme, damit nichts
  // springt; voices aus soundscape.js carVoices
  startVoices() {
    const c = this.ctx;
    this.voices = [];
    for (let i = 0; i < 4; i++) {
      const pan = this.panner(); pan.connect(this.outside);
      const g = c.createGain(); g.gain.value = 0; g.connect(pan);
      const o = c.createOscillator(); o.type = 'sawtooth'; const o2 = c.createOscillator(); o2.type = 'square';
      const og2 = c.createGain(); og2.gain.value = 0.5;
      const f = c.createBiquadFilter(); f.type = 'lowpass'; f.frequency.value = 500; f.Q.value = 0.9;
      o.connect(f); o2.connect(og2); og2.connect(f); f.connect(g); o.start(); o2.start();
      const tire = this.noiseLayer('bandpass', 600, 0.5, pan);
      this.voices.push({ id: null, g, o, o2, og2, f, pan, tire });
    }
  }

  setVoices(list) {
    if (!this.ready) return;
    if (!this.voices) this.startVoices();
    const t = this.ctx.currentTime, want = new Map(list.map((v) => [v.id, v]));
    for (const s of this.voices) if (s.id !== null && !want.has(s.id)) s.id = null;
    for (const v of list) if (!this.voices.some((s) => s.id === v.id)) { const free = this.voices.find((s) => s.id === null); if (free) free.id = v.id; }
    for (const s of this.voices) {
      const v = s.id === null ? null : want.get(s.id);
      s.g.gain.setTargetAtTime(v ? v.gain * (v.electric ? 0.025 * v.tire : 0.09 * (v.profile?.volume ?? 1)) : 0, t, 0.15);
      s.tire.g.gain.setTargetAtTime(v ? 0.05 * v.gain * v.tire : 0, t, 0.15);
      if (!v) continue;
      const fire = Math.max(10, v.fire * v.rate);
      if (v.electric) {
        s.o.type = 'sine'; s.o.voiceKey = null;
        s.o.frequency.setTargetAtTime((160 + v.rpm / 16000 * 2200) * v.rate, t, 0.12);
        s.og2.gain.setTargetAtTime(0, t, 0.1);
        s.f.frequency.setTargetAtTime(3500, t, 0.2);
      } else {
        const custom = v.profile && this.engineWave(s.o, v.profile, v.cyl);
        s.o.frequency.setTargetAtTime(custom ? Math.max(4, v.cycle * v.rate) : fire, t, 0.12);
        s.o2.type = custom ? 'triangle' : 'square';
        s.o2.frequency.setTargetAtTime(custom ? v.rpm / 60 * v.rate : fire * 0.5, t, 0.12);
        s.og2.gain.setTargetAtTime(custom ? 0.24 + v.profile.rough * 0.6 + v.load * 0.15 : 0.5, t, 0.1);
        s.f.frequency.setTargetAtTime(v.profile ? 220 + v.profile.brightness * (0.18 + v.norm * 0.45 + v.load * 0.3) : (v.diesel ? 260 : 380) + v.tire * 900, t, 0.2);
      }
      s.tire.f.frequency.setTargetAtTime(500 * v.rate + v.tire * 500, t, 0.2);
      this.setPan(s.pan, v.pan, t);
    }
  }

  // Umgebung: gefiltertes Rauschen je Schicht (Stadt, Verkehr, Wasser, Hochbahn, Regen, Wind), Stimmengewirr aus drei
  // Formantbändern (klingt nach Stimmen, nicht nach Rauschen), Martinshorn-Oszillator
  startAmbience() {
    const c = this.ctx, layer = (type, freq, q = 0.7) => this.noiseLayer(type, freq, q).g;
    this.barPan = this.panner(); this.barPan.connect(this.outside);
    this.babble = [[480, 3], [1150, 4], [2500, 5]].map(([fr, q]) => this.noiseLayer('bandpass', fr, q, this.barPan));
    const wind = this.noiseLayer('bandpass', 380, 1.4), whistle = this.noiseLayer('bandpass', 900, 12);
    this.amb = {
      hum: layer('lowpass', 180), traffic: layer('bandpass', 420, 0.6), water: layer('highpass', 1400), bar: this.babble[0].g,
      rumble: layer('lowpass', 90), rain: layer('highpass', 2600), rainLow: layer('bandpass', 900, 0.5), wind: wind.g,
    };
    this.windLayer = wind; this.whistle = whistle; // Heulen: Filterfrequenz folgt den Böen (setAmbience)
    const o = c.createOscillator(); o.type = 'triangle'; o.frequency.value = 440;
    const g = c.createGain(); g.gain.value = 0; o.connect(g); g.connect(this.outside); o.start();
    this.siren = { o, g };
    this.nextChirp = 0; this.nextBeat = 0; this.beat = 0; this.nextLaugh = 0; this.nextClink = 0;
  }

  // mix aus ambience.js (0..1 je Schicht, sirens nach Entfernung)
  setAmbience(mix) {
    if (!this.ready || !this.amb) return;
    const t = this.ctx.currentTime, a = this.amb, set = (g, v) => g.gain.setTargetAtTime(v, t, 0.6);
    const rain = mix.rain ?? 0;
    set(a.hum, 0.018 * mix.hum); set(a.traffic, 0.05 * mix.traffic); set(a.water, 0.012 * mix.water);
    set(a.rumble, 0.16 * mix.rumble); set(a.rain, 0.07 * Math.min(1, rain) + 0.05 * Math.max(0, rain - 1));
    set(a.rainLow, 0.05 * Math.max(0, rain - 0.6)); // Starkregen prasselt auch tief
    // Wind: Grundrauschen, dessen Band mit den Böen steigt, dazu Pfeifen an Kanten bei starken Böen
    const wind = mix.wind ?? 0, gust = mix.gust ?? 0;
    set(a.wind, 0.11 * wind);
    this.windLayer.f.frequency.setTargetAtTime(260 + gust * 520, t, 0.5);
    this.whistle.g.gain.setTargetAtTime(0.03 * wind * Math.max(0, gust - 0.35), t, 0.4);
    this.whistle.f.frequency.setTargetAtTime(700 + gust * 900, t, 0.6);
    // Dämpfung: im Auto und bei Schneedecke schluckt die Hülle die Höhen
    this.muffleF.frequency.setTargetAtTime(18000 * Math.pow(0.04, mix.muffle ?? 0), t, 0.3);
    this.setBar(mix, t);
    const sr = mix.sirens?.[0];
    this.siren.g.gain.setTargetAtTime(sr ? 0.07 * sr.gain : 0, t, 0.15);
    if (sr) this.siren.o.frequency.setTargetAtTime(sr.high ? 585 : 440, t, 0.02);
    if (mix.birds > 0.02 && t > this.nextChirp) { // Vogelstimmen: kurze Tonfolgen, je mehr Grün, desto öfter
      const base = 2400 + Math.random() * 2600, n = 2 + Math.floor(Math.random() * 4);
      for (let i = 0; i < n; i++) this.tone(base * (1 + (Math.random() - 0.5) * 0.3), 0.07, { type: 'sine', gain: 0.025 * mix.birds, at: i * 0.09, slide: (Math.random() - 0.3) * 900, dest: this.outside });
      this.nextChirp = t + 0.4 + Math.random() * 2.2 / mix.birds;
    }
    // Regentropfen: einzelne Tropfen auf Blech, Pfützen, Blättern (körnig statt nur Rauschen)
    const drops = Math.round(Math.min(1.6, rain) * 14 * (mix.inCar ? 0.4 : 1));
    for (let i = 0; i < drops; i++) this.drop(Math.random() * 0.25, 0.012 + Math.random() * 0.02 * Math.min(1, rain));
  }

  // Nachtleben: Stimmengewirr (Formanten zittern wie Silben), Lachen, Gläserklirren, gedämpfter Bass aus dem Club
  setBar(mix, t) {
    const crowd = mix.bar ?? 0, music = mix.music ?? 0;
    this.setPan(this.barPan, (mix.barPan ?? 0) * 0.7, t);
    for (const [i, b] of this.babble.entries()) {
      const base = [0.07, 0.045, 0.02][i] * crowd;
      // Silbenrhythmus: in 60–110 ms Stufen auf zufällige Pegel und Formantlagen
      for (let k = 0, at = t; k < 3; k++, at += 0.06 + Math.random() * 0.05) {
        b.g.gain.setTargetAtTime(base * (0.45 + Math.random() * 0.9), at, 0.03);
        b.f.frequency.setTargetAtTime([480, 1150, 2500][i] * (0.8 + Math.random() * 0.45), at, 0.04);
      }
    }
    if (crowd > 0.15 && t > this.nextLaugh) { this.laugh(0.03 * crowd); this.nextLaugh = t + 1.2 + Math.random() * 5 / crowd; }
    if (crowd > 0.1 && t > this.nextClink) { this.clink(0.02 * crowd); this.nextClink = t + 0.8 + Math.random() * 4 / crowd; }
    // Club-Bass: 124 BPM Kick und Bass auf der Offbeat, durch Wände nur tief hörbar; vorausplanen bis 0,6 s
    if (music > 0.03) {
      const spb = 60 / 124;
      if (this.nextBeat < t) this.nextBeat = t + 0.05;
      while (this.nextBeat < t + 0.6) {
        const at = this.nextBeat - t;
        this.kick(0.22 * music, at);
        if (this.beat % 2 === 0) this.tone([55, 55, 65.4, 49][(this.beat >> 3) % 4], spb * 0.45, { type: 'sawtooth', gain: 0.05 * music, at: at + spb / 2, lowpass: 170, dest: this.barPan });
        this.nextBeat += spb; this.beat++;
      }
    }
  }

  kick(gain, at) {
    const c = this.ctx, t = c.currentTime + at, o = c.createOscillator(), g = c.createGain();
    o.type = 'sine'; o.frequency.setValueAtTime(130, t); o.frequency.exponentialRampToValueAtTime(44, t + 0.13);
    g.gain.setValueAtTime(0.0001, t); g.gain.linearRampToValueAtTime(gain, t + 0.005); g.gain.exponentialRampToValueAtTime(0.0001, t + 0.3);
    o.connect(g); g.connect(this.barPan); o.start(t); o.stop(t + 0.35);
  }

  // Lachen: 3–6 „ha“ (Vokal-Formant auf Rauschen), fallend
  laugh(gain) {
    const n = 3 + Math.floor(Math.random() * 4), f0 = 800 + Math.random() * 600;
    for (let i = 0; i < n; i++) this.burst(0.09, { freq: f0 * (1 - i * 0.05), gain: gain * (1 - i * 0.1), type: 'bandpass', q: 7, at: i * (0.12 + Math.random() * 0.03), dest: this.barPan, attack: 0.015 });
  }
  clink(gain) {
    const f = 2800 + Math.random() * 1600;
    this.tone(f, 0.25, { type: 'sine', gain, dest: this.barPan }); this.tone(f * 2.76, 0.15, { type: 'sine', gain: gain * 0.5, dest: this.barPan });
  }
  drop(at, gain) {
    this.burst(0.012 + Math.random() * 0.02, { freq: 1800 + Math.random() * 5000, gain, type: 'bandpass', q: 3, at });
  }

  // Schritt: 'hard' (Gehweg), 'snow' (Knirschen aus Mikro-Stößen), 'wet' (Platschen), 'grass' (dumpf)
  footstep(kind = 'hard', strength = 1) {
    if (!this.ready) return;
    const k = strength;
    if (kind === 'snow') for (let i = 0; i < 4; i++) this.burst(0.03 + Math.random() * 0.03, { freq: 1100 + Math.random() * 1800, gain: 0.07 * k, type: 'bandpass', q: 1.5, at: i * 0.022 });
    else if (kind === 'wet') { this.burst(0.09, { freq: 900, gain: 0.08 * k }); this.burst(0.05, { freq: 3200, gain: 0.03 * k, type: 'bandpass', q: 2, at: 0.02 }); }
    else if (kind === 'grass') this.burst(0.07, { freq: 500, gain: 0.07 * k });
    else { this.burst(0.03, { freq: 2200, gain: 0.05 * k, type: 'bandpass', q: 1.2 }); this.tone(95, 0.05, { type: 'sine', gain: 0.06 * k }); }
  }

  // Donner: nah ein trockener Knall, dann langes, an- und abschwellendes Grollen; fern nur tiefes Grollen
  thunder(loud, near) {
    if (!this.ready) return;
    const c = this.ctx, t = c.currentTime, dur = near ? 5.5 : 7 + Math.random() * 3;
    if (near) { this.burst(0.35, { freq: 3800, gain: 0.5 * loud, type: 'highpass' }); this.burst(0.8, { freq: 900, gain: 0.45 * loud }); }
    const s = c.createBufferSource(); s.buffer = this.noise; s.loop = true;
    const f = c.createBiquadFilter(); f.type = 'lowpass'; f.frequency.setValueAtTime(near ? 420 : 160, t); f.frequency.exponentialRampToValueAtTime(70, t + dur);
    const g = c.createGain(); g.gain.setValueAtTime(0.0001, t);
    // Grollen in Wellen (Echos an Häusern und Wolken)
    let at = t + (near ? 0.15 : 0.3);
    for (let k = 0; k < 5; k++) {
      const peak = (0.55 * loud) * (1 - k * 0.15) * (0.6 + Math.random() * 0.4);
      g.gain.linearRampToValueAtTime(peak, at + 0.25); at += 0.5 + Math.random() * 0.9;
      g.gain.linearRampToValueAtTime(peak * 0.35, at);
    }
    g.gain.exponentialRampToValueAtTime(0.0001, t + dur);
    s.connect(f); f.connect(g); g.connect(this.master); s.start(t, Math.random()); s.stop(t + dur + 0.1);
  }

  // Kirchenglocke: n Schläge (Grundton + unharmonische Teiltöne, langer Nachhall)
  bells(n) {
    if (!this.ready) return;
    for (let i = 0; i < n; i++) for (const [f, g] of [[196, 0.09], [392, 0.05], [470, 0.04], [588, 0.03], [784, 0.02]]) this.tone(f, 3.2, { type: 'sine', gain: g, at: i * 2.1, dest: this.outside });
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

  tone(freq, dur, { type = 'square', gain = 0.12, at = 0, slide = 0, lowpass = 0, dest = this.master } = {}) {
    const c = this.ctx, t = c.currentTime + at;
    const o = c.createOscillator(), g = c.createGain();
    o.type = type; o.frequency.setValueAtTime(freq, t);
    if (slide) o.frequency.exponentialRampToValueAtTime(Math.max(30, freq + slide), t + dur);
    g.gain.setValueAtTime(0, t); g.gain.linearRampToValueAtTime(gain, t + 0.01); g.gain.exponentialRampToValueAtTime(0.0001, t + dur);
    if (lowpass) { const f = c.createBiquadFilter(); f.type = 'lowpass'; f.frequency.value = lowpass; o.connect(f); f.connect(g); } else o.connect(g);
    g.connect(dest); o.start(t); o.stop(t + dur + 0.05);
  }

  burst(dur, { freq = 800, gain = 0.3, type = 'lowpass', q = 0.7, at = 0, attack = 0, dest = this.master } = {}) {
    const c = this.ctx, t = c.currentTime + at;
    const s = c.createBufferSource(); s.buffer = this.noise;
    const f = c.createBiquadFilter(); f.type = type; f.frequency.value = freq; f.Q.value = q;
    const g = c.createGain();
    if (attack) { g.gain.setValueAtTime(0.0001, t); g.gain.linearRampToValueAtTime(gain, t + attack); } else g.gain.setValueAtTime(gain, t);
    g.gain.exponentialRampToValueAtTime(0.0001, t + dur);
    s.connect(f); f.connect(g); g.connect(dest); s.start(t, Math.random() * 1.5); s.stop(t + dur + 0.05);
  }
}

export const SYNTH = {
  // Blech knautscht (mehrere Stöße), dumpfer Schlag, bei harten Unfällen Glassplitter
  crash: (s, k) => {
    s.burst(0.35 + k * 0.3, { freq: 600 + k * 1800, gain: 0.25 + k * 0.4 }); s.tone(90, 0.25, { type: 'sine', gain: 0.25 * k, slide: -50 });
    for (let i = 1; i < 2 + k * 3; i++) s.burst(0.08, { freq: 1200 + Math.random() * 1500, gain: 0.12 * k, type: 'bandpass', q: 3, at: i * 0.05 });
    if (k > 0.5) for (let i = 0; i < 6; i++) s.tone(3500 + Math.random() * 3500, 0.12, { type: 'sine', gain: 0.03 * k, at: 0.08 + Math.random() * 0.3 });
  },
  hit: (s) => { s.burst(0.12, { freq: 400, gain: 0.3 }); s.tone(160, 0.12, { type: 'sine', gain: 0.2, slide: -80 }); },
  // Zweiklanghupe wie beim Pkw (Terz, rau, bandbegrenzt)
  horn: (s, k = 1) => { for (const f of [415, 523]) s.tone(f, 0.45, { type: 'sawtooth', gain: 0.06 * k, lowpass: 1800 }); },
  // Poller umgefahren: metallisches Scheppern (unharmonische Teiltöne) plus Schlag
  knock: (s, k = 1) => { s.burst(0.1, { freq: 900, gain: 0.25 * k }); for (const [f, g] of [[520, 0.06], [1340, 0.04], [2150, 0.03], [3470, 0.02]]) s.tone(f, 0.6, { type: 'sine', gain: g * k }); },
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
  // Türgong der Bahn: zwei Töne, beim Öffnen aufwärts, beim Schließen abwärts
  splash: (s) => { s.burst(0.35, { freq: 700, gain: 0.3 }); s.burst(0.18, { freq: 2200, gain: 0.12, type: 'bandpass' }); },
  'gong-open': (s) => { s.tone(659, 0.35, { type: 'sine', gain: 0.1 }); s.tone(880, 0.45, { type: 'sine', gain: 0.1, at: 0.28 }); },
  'gong-close': (s) => { s.tone(880, 0.3, { type: 'sine', gain: 0.1 }); s.tone(659, 0.4, { type: 'sine', gain: 0.1, at: 0.24 }); },
};

// Welches Spielereignis welchen Klang auslöst (null = keiner); main.js spielt ihn ab
const EVENT_SOUND = {
  'tram-bell': 'tram-bell', crash: 'crash', hit: 'hit', horn: 'horn', door: 'door', ui: 'ui', 'ui-move': 'ui-move', 'ui-back': 'ui-back', tick: 'tick',
  pickup: 'pickup', 'mission-start': 'mission-start', 'mission-success': 'mission-success', 'mission-fail': 'mission-fail', carjack: 'carjack',
  bump: 'hit', knock: 'knock', thud: 'thud', impact: 'impact', reload: 'reload', reloaded: 'reloaded', weapon: 'weapon', 'player-hurt': 'punch',
  wasted: 'mission-fail', respawn: 'pickup',
  // Nahverkehr: Ein-/Aussteigen (Auf-/Abspringen rumst), Führerstand, Türgong, Trinkgeld, Zug voraus
  'train-take': 'door', 'doors-open': 'gong-open', 'doors-close': 'gong-close', tip: 'pickup', 'train-blocked': 'tram-bell', aquaplane: 'splash',
};
export function soundFor(e) {
  if (e.type === 'shot') return e.weapon ?? null;
  if (e.type === 'swing') return e.hit ? 'punch' : 'swing';
  if (e.type === 'board' || e.type === 'alight') return e.hop ? 'hit' : 'door';
  return EVENT_SOUND[e.type] ?? null;
}
