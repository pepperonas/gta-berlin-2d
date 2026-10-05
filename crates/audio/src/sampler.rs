//! Motorstimmen aus Aufnahmen (docs/audio.md): spielt, was `berlin_sim::enginesound` je Bild bestimmt – Loops mit
//! eigener Tonhöhe und Lautstärke (4-Punkt-Hermite-Interpolation, geglättete Parameter), Einzelklänge, Begrenzer,
//! Klangfärbung (Sättigung, Bass-/Höhen-Shelf, Tiefpass) und Entfernung (Tiefpass, Pegel, Panorama, Doppler).
//!
//! Die Samples sind 16-Bit-PCM-WAV (Mono) und stecken im Programm (`include_bytes!`), gelesen ohne Fremdbibliothek.
//! Nur die Referenzaufnahme für den A/B-Vergleich wird bei Bedarf von der Platte geladen.
use crate::dsp::{Biquad, FilterType, Smooth, shape};
use berlin_sim::enginesound::{Bank, SoundOut, config};
use std::sync::{Arc, OnceLock};

macro_rules! v10 {
    ($($f:literal),* $(,)?) => {
        &[$(($f, include_bytes!(concat!("../../../data/audio/engine/v10/", $f)) as &[u8])),*]
    };
}
/// Die Dateien der Bank „v10“ (müssen zum Manifest passen, ein Test prüft das).
const V10: &[(&str, &[u8])] = v10!(
    "on_idle.wav",
    "on_r2600.wav",
    "on_r3300.wav",
    "on_r4900.wav",
    "on_r6000.wav",
    "on_r7200.wav",
    "off_idle.wav",
    "off_r2600.wav",
    "off_r3300.wav",
    "off_r4900.wav",
    "off_r6000.wav",
    "off_r7200.wav",
    "start.wav",
    "blip.wav",
    "shift_1.wav",
    "shift_2.wav",
    "shift_3.wav",
    "pop_1.wav",
    "pop_2.wav",
    "pop_3.wav",
    "pop_4.wav",
    "pop_5.wav",
);

/// RIFF/WAVE mit 16-Bit-PCM lesen; mehrere Kanäle werden gemittelt. (Samples, Samplerate)
pub fn parse_wav(b: &[u8]) -> Option<(Vec<f32>, u32)> {
    if b.len() < 12 || &b[0..4] != b"RIFF" || &b[8..12] != b"WAVE" {
        return None;
    }
    let u16_at = |i: usize| u16::from_le_bytes([b[i], b[i + 1]]);
    let u32_at = |i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
    let (mut ch, mut sr, mut bits, mut fmt_ok) = (1usize, 0u32, 0u16, false);
    let mut i = 12;
    while i + 8 <= b.len() {
        let (id, len) = (&b[i..i + 4], u32_at(i + 4) as usize);
        let body = i + 8;
        if body + len > b.len() {
            return None;
        }
        if id == b"fmt " && len >= 16 {
            fmt_ok = u16_at(body) == 1; // PCM
            ch = u16_at(body + 2).max(1) as usize;
            sr = u32_at(body + 4);
            bits = u16_at(body + 14);
        } else if id == b"data" {
            if !fmt_ok || bits != 16 {
                return None;
            }
            let frames = len / (2 * ch);
            let mut out = Vec::with_capacity(frames);
            for f in 0..frames {
                let mut acc = 0f32;
                for c in 0..ch {
                    let k = body + (f * ch + c) * 2;
                    acc += i16::from_le_bytes([b[k], b[k + 1]]) as f32 / 32768.;
                }
                out.push(acc / ch as f32);
            }
            return Some((out, sr));
        }
        i = body + len + (len & 1);
    }
    None
}

/// Geladene Sample-Bank (gleiche Indizes wie `berlin_sim::enginesound::Bank`).
pub struct SampleBank {
    pub loops: Vec<Arc<[f32]>>,
    pub shots: Vec<Arc<[f32]>>,
    pub sr: f32,
}
fn load(files: &[(&str, &[u8])], bank: &Bank) -> SampleBank {
    let get = |name: &str| -> Arc<[f32]> {
        let bytes = files
            .iter()
            .find(|(f, _)| *f == name)
            .unwrap_or_else(|| panic!("Sample {name} fehlt in der Bank"))
            .1;
        let (x, _) = parse_wav(bytes).unwrap_or_else(|| panic!("{name}: kein 16-Bit-PCM-WAV"));
        x.into()
    };
    SampleBank {
        loops: bank.loops.iter().map(|l| get(&l.file)).collect(),
        shots: bank.shots.iter().map(|s| get(&s.file)).collect(),
        sr: bank.samplerate as f32,
    }
}
pub fn bank_v10() -> &'static SampleBank {
    static B: OnceLock<SampleBank> = OnceLock::new();
    B.get_or_init(|| load(V10, config().bank("v10")))
}

/// Was der Synthesizer je Bild für eine Motorstimme bekommt.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EngineFrame {
    /// Fahrzeug-id; das Spielerauto hat `player = true`
    pub id: u32,
    pub player: bool,
    pub out: SoundOut,
    /// Entfernung und Lage (fremde Autos; das Spielerauto: 1, 0, 1)
    pub gain: f32,
    pub pan: f32,
    pub rate: f32,
    /// Tiefpass der Entfernung (Hz)
    pub lowpass: f32,
}

#[derive(Debug, Clone, Copy)]
struct Slot {
    pos: f64,
    gain: Smooth,
    pitch: Smooth,
}
#[derive(Debug, Clone, Copy)]
struct Shot {
    idx: usize,
    pos: f64,
    gain: f32,
}

/// Eine Motorstimme (Spielerauto oder fremdes Auto).
pub struct SamplerVoice {
    pub id: Option<u32>,
    pub player: bool,
    slots: Vec<Slot>,
    shots: Vec<Shot>,
    rate: Smooth,
    gate: Smooth,
    vol: Smooth,
    drive: Smooth,
    pan: Smooth,
    lp: Biquad,
    low: Biquad,
    high: Biquad,
    dist: Biquad,
    lp_f: Smooth,
    dist_f: Smooth,
}

/// 4-Punkt-Hermite zwischen x[i] und x[i+1] (Ring: der Loop läuft nahtlos weiter).
#[inline]
fn hermite(x: &[f32], pos: f64) -> f32 {
    let n = x.len();
    let i = pos as usize;
    let t = (pos - i as f64) as f32;
    let at = |k: isize| x[((i as isize + k).rem_euclid(n as isize)) as usize];
    let (xm1, x0, x1, x2) = (at(-1), at(0), at(1), at(2));
    let c1 = 0.5 * (x1 - xm1);
    let c2 = xm1 - 2.5 * x0 + 2. * x1 - 0.5 * x2;
    let c3 = 0.5 * (x2 - xm1) + 1.5 * (x0 - x1);
    ((c3 * t + c2) * t + c1) * t + x0
}

impl SamplerVoice {
    pub fn new(bank: &SampleBank) -> Self {
        let slot = Slot {
            pos: 0.,
            gain: Smooth::new(0.),
            pitch: Smooth::new(1.),
        };
        Self {
            id: None,
            player: false,
            // Startversatz je Loop, damit gleichzeitig einsetzende Loops nicht phasengleich kämmen
            slots: (0..bank.loops.len())
                .map(|i| Slot {
                    pos: (i * 977 % bank.loops[i].len().max(1)) as f64,
                    ..slot
                })
                .collect(),
            shots: Vec::new(),
            rate: Smooth::new(1.),
            gate: Smooth::new(1.),
            vol: Smooth::new(0.),
            drive: Smooth::new(0.),
            pan: Smooth::new(0.),
            lp: Biquad::new(FilterType::Lowpass, 9000., 0.),
            low: Biquad::new(FilterType::Lowshelf, 250., 0.),
            high: Biquad::new(FilterType::Highshelf, 3000., 0.),
            dist: Biquad::new(FilterType::Lowpass, 12000., 0.),
            lp_f: Smooth::new(9000.),
            dist_f: Smooth::new(12000.),
        }
    }
    /// Stimme freigeben: leise ausblenden, Einzelklänge laufen aus.
    pub fn release(&mut self, sr: f32) {
        self.id = None;
        self.vol.set(0., 0.08, sr);
    }
    pub fn silent(&self) -> bool {
        self.id.is_none() && self.vol.value < 1e-4 && self.shots.is_empty()
    }
    pub fn apply(&mut self, f: &EngineFrame, sr: f32) {
        self.id = Some(f.id);
        self.player = f.player;
        let o = &f.out;
        for s in self.slots.iter_mut() {
            s.gain.set(0., 0.03, sr);
        }
        for l in &o.layers {
            if l.gain > 0. {
                let s = &mut self.slots[l.idx];
                s.gain.set(l.gain, 0.03, sr);
                s.pitch.set(l.pitch, 0.015, sr);
            }
        }
        for &(idx, g) in &o.shots {
            if self.shots.len() < 8 {
                self.shots.push(Shot {
                    idx,
                    pos: 0.,
                    gain: g,
                });
            }
        }
        self.rate.set(f.rate, 0.05, sr);
        // Begrenzer: schnell zu, schnell auf (kein Klick, aber hörbar abgehackt)
        self.gate.set(o.gate, 0.004, sr);
        self.vol.set(o.volume * f.gain, 0.05, sr);
        self.drive.set(o.tone.drive, 0.05, sr);
        self.pan.set(f.pan, 0.08, sr);
        self.lp_f.set(o.tone.lowpass, 0.05, sr);
        self.dist_f.set(f.lowpass, 0.08, sr);
        self.low.set(250., 0., o.tone.low_db);
        self.high.set(3000., 0., o.tone.high_db);
    }
    /// Nächstes Monosample (vor dem Panorama).
    #[inline]
    pub fn next(&mut self, bank: &SampleBank, sr: f32, block: bool) -> f32 {
        let step = (bank.sr / sr) as f64;
        let rate = self.rate.tick() as f64;
        let mut x = 0f32;
        for (i, s) in self.slots.iter_mut().enumerate() {
            let g = s.gain.tick();
            let p = s.pitch.tick();
            if g < 1e-5 && s.gain.target == 0. {
                continue;
            }
            let buf = &bank.loops[i];
            x += hermite(buf, s.pos) * g;
            s.pos += p as f64 * rate * step;
            let n = buf.len() as f64;
            if s.pos >= n {
                s.pos -= n;
            }
        }
        x *= self.gate.tick();
        for sh in self.shots.iter_mut() {
            let buf = &bank.shots[sh.idx];
            let i = sh.pos as usize;
            if i + 1 < buf.len() {
                let t = (sh.pos - i as f64) as f32;
                x += (buf[i] * (1. - t) + buf[i + 1] * t) * sh.gain;
            }
            sh.pos += rate * step;
        }
        self.shots
            .retain(|sh| (sh.pos as usize) + 1 < bank.shots[sh.idx].len());
        let d = self.drive.tick();
        if d > 0.01 {
            x = x * (1. - d) + shape(x, 1. + d * 3.) * d;
        }
        let (lf, df) = (self.lp_f.tick(), self.dist_f.tick());
        if block {
            self.lp.set_freq(lf);
            self.dist.set_freq(df);
        }
        let y = self.low.process(x, sr);
        let y = self.high.process(y, sr);
        let y = self.lp.process(y, sr);
        let y = self.dist.process(y, sr);
        y * self.vol.tick()
    }
    pub fn pan(&mut self) -> f32 {
        self.pan.tick()
    }
}

/// Referenzaufnahme für den A/B-Vergleich (von der Platte, nur bei Bedarf).
pub fn load_reference() -> Option<Arc<[f32]>> {
    let bank = config().bank("v10");
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/audio/engine/v10")
        .join(&bank.reference);
    let bytes = std::fs::read(path).ok()?;
    parse_wav(&bytes).map(|(x, _)| x.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use berlin_sim::enginesound::{EngineSound, SoundInput};

    fn wav(samples: &[i16], ch: u16, sr: u32) -> Vec<u8> {
        let data: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
        let mut b = Vec::new();
        b.extend_from_slice(b"RIFF");
        b.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
        b.extend_from_slice(b"WAVEfmt ");
        b.extend_from_slice(&16u32.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&ch.to_le_bytes());
        b.extend_from_slice(&sr.to_le_bytes());
        b.extend_from_slice(&(sr * 2 * ch as u32).to_le_bytes());
        b.extend_from_slice(&(2 * ch).to_le_bytes());
        b.extend_from_slice(&16u16.to_le_bytes());
        b.extend_from_slice(b"data");
        b.extend_from_slice(&(data.len() as u32).to_le_bytes());
        b.extend_from_slice(&data);
        b
    }

    #[test]
    fn reads_pcm16_wav() {
        let (x, sr) = parse_wav(&wav(&[0, 16384, -32768, 32767], 1, 48000)).unwrap();
        assert_eq!(sr, 48000);
        assert_eq!(x, vec![0., 0.5, -1., 32767. / 32768.]);
        // Stereo wird gemittelt
        let (x, _) = parse_wav(&wav(&[16384, 0, -16384, -16384], 2, 44100)).unwrap();
        assert_eq!(x, vec![0.25, -0.5]);
        assert!(parse_wav(b"RIFF....WAVE").is_none());
        assert!(parse_wav(b"nope").is_none());
    }

    #[test]
    fn bank_matches_the_manifest() {
        let bank = config().bank("v10");
        let names: Vec<&str> = bank
            .loops
            .iter()
            .map(|l| l.file.as_str())
            .chain(bank.shots.iter().map(|s| s.file.as_str()))
            .collect();
        for n in &names {
            assert!(V10.iter().any(|(f, _)| f == n), "{n} fehlt in V10");
        }
        assert_eq!(
            names.len(),
            V10.len(),
            "V10 enthält Dateien, die das Manifest nicht kennt"
        );
        let b = bank_v10();
        for (l, info) in b.loops.iter().zip(&bank.loops) {
            assert_eq!(l.len(), info.samples, "{}", info.file);
            let peak = l.iter().fold(0f32, |m, v| m.max(v.abs()));
            assert!(peak > 0.05 && peak < 1., "{}: {peak}", info.file);
        }
        assert!(load_reference().is_some_and(|r| r.len() > 48000 * 8));
    }

    #[test]
    fn hermite_is_exact_on_samples_and_wraps() {
        let x = [0., 1., 0., -1.];
        assert_eq!(hermite(&x, 1.), 1.);
        assert!((hermite(&x, 3.5) - hermite(&[-1., 0., 1., 0.], 0.5)).abs() < 1e-6);
    }

    /// Hochdrehen und Gaswegnehmen ohne Klicks: kein Sprung von Sample zu Sample, der nicht auch im Material
    /// selbst vorkommt, und kein stiller Aussetzer.
    #[test]
    fn sweep_has_no_clicks_or_dropouts() {
        let sr = 48000.;
        let bank = bank_v10();
        let info = config().bank("v10");
        let p = config().preset("supercar").unwrap();
        let mut e = EngineSound::new(1);
        let mut v = SamplerVoice::new(bank);
        let mut out = Vec::new();
        let frames = 360;
        for k in 0..frames {
            let u = k as f64 / frames as f64;
            // 0–3 s hochdrehen, dann Gas weg
            let inp = SoundInput {
                rpm: Some(1000. + 7300. * (u * 1.6).min(1.)),
                gear: Some(2),
                throttle: if u < 0.6 { 1. } else { 0. },
                speed: 20.,
                ..Default::default()
            };
            let o = e.step(p, info, &inp, 1. / 60., false);
            v.apply(
                &EngineFrame {
                    id: 1,
                    player: true,
                    out: o,
                    gain: 1.,
                    pan: 0.,
                    rate: 1.,
                    lowpass: 20000.,
                },
                sr,
            );
            for i in 0..(sr / 60.) as usize {
                out.push(v.next(bank, sr, i % 32 == 0));
            }
        }
        let body = &out[(sr * 0.1) as usize..];
        let max_step = body
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0f32, f32::max);
        let peak = body.iter().fold(0f32, |m, x| m.max(x.abs()));
        // ein Klick wäre ein Sprung in der Größenordnung des Spitzenpegels
        assert!(max_step < peak * 0.6, "Sprung {max_step} bei Spitze {peak}");
        // kein Aussetzer: RMS in jedem 50-ms-Fenster deutlich über null
        for w in body.chunks((sr * 0.05) as usize) {
            let rms = (w.iter().map(|x| x * x).sum::<f32>() / w.len() as f32).sqrt();
            assert!(rms > 0.01, "Aussetzer (RMS {rms})");
        }
    }

    /// Die drei Kategorien klingen bei gleicher Eingabe hörbar verschieden: Grundton (Verstimmung) und Helligkeit
    /// (Tiefpass, Shelf, Sättigung) steigen vom Sportwagen über das Supercar zum Hypercar.
    #[test]
    fn categories_sound_different() {
        let sr = 48000.;
        let bank = bank_v10();
        let info = config().bank("v10");
        let render = |name: &str| {
            let p = config().preset(name).unwrap();
            let mut e = EngineSound::new(1);
            let mut v = SamplerVoice::new(bank);
            let inp = SoundInput {
                rpm: Some(4500.),
                gear: Some(3),
                throttle: 0.8,
                speed: 20.,
                ..Default::default()
            };
            let mut out = Vec::new();
            for _ in 0..60 {
                let o = e.step(p, info, &inp, 1. / 60., false);
                v.apply(
                    &EngineFrame {
                        id: 1,
                        player: true,
                        out: o,
                        gain: 1.,
                        pan: 0.,
                        rate: 1.,
                        lowpass: 20000.,
                    },
                    sr,
                );
                for i in 0..800 {
                    out.push(v.next(bank, sr, i % 32 == 0));
                }
            }
            let x = &out[24000..];
            // Helligkeit: Energie der ersten Differenz relativ zur Energie (≈ spektraler Schwerpunkt)
            let e0: f32 = x.iter().map(|v| v * v).sum();
            let e1: f32 = x.windows(2).map(|w| (w[1] - w[0]).powi(2)).sum();
            // Grundton: Maximum der Autokorrelation zwischen 2,5 und 12 ms
            let lag = (120..576)
                .max_by(|&a, &b| {
                    let ac = |l: usize| x.iter().zip(&x[l..]).map(|(p, q)| p * q).sum::<f32>();
                    ac(a).total_cmp(&ac(b))
                })
                .unwrap();
            ((e1 / e0).sqrt(), sr / lag as f32)
        };
        let (s, u, h) = (render("sport"), render("supercar"), render("hypercar"));
        assert!(s.0 < u.0 && u.0 < h.0, "Helligkeit {s:?} {u:?} {h:?}");
        assert!(s.1 < u.1 && u.1 < h.1, "Grundton {s:?} {u:?} {h:?}");
        // Verstimmung 0,9 / 1,0 / 1,08 wiederzufinden (±3 %)
        assert!(
            (s.1 / u.1 - 0.9).abs() < 0.03 && (h.1 / u.1 - 1.08).abs() < 0.03,
            "{s:?} {u:?} {h:?}"
        );
    }

    #[test]
    fn many_voices_render_fast() {
        let sr = 48000.;
        let bank = bank_v10();
        let info = config().bank("v10");
        let p = config().preset("hypercar").unwrap();
        let mut voices: Vec<(EngineSound, SamplerVoice)> = (0..16)
            .map(|i| (EngineSound::new(i), SamplerVoice::new(bank)))
            .collect();
        let t0 = std::time::Instant::now();
        let mut acc = 0f32;
        for k in 0..60 {
            for (i, (e, v)) in voices.iter_mut().enumerate() {
                let inp = SoundInput {
                    rpm: Some(2000. + 300. * i as f64 + 40. * k as f64),
                    gear: Some(3),
                    throttle: 0.6,
                    speed: 20.,
                    ..Default::default()
                };
                let o = e.step(p, info, &inp, 1. / 60., i >= 4);
                v.apply(
                    &EngineFrame {
                        id: i as u32,
                        out: o,
                        gain: 0.5,
                        rate: 1.,
                        lowpass: 4000.,
                        ..Default::default()
                    },
                    sr,
                );
            }
            for i in 0..800 {
                for (_, v) in voices.iter_mut() {
                    acc += v.next(bank, sr, i % 32 == 0);
                }
            }
        }
        let secs = t0.elapsed().as_secs_f64();
        assert!(acc.is_finite());
        // 1 s Klang für 16 Stimmen; im Release-Build weit unter einer Sekunde
        if !cfg!(debug_assertions) {
            assert!(secs < 0.25, "{secs:.3} s für 1 s Audio mit 16 Stimmen");
        }
    }
}
