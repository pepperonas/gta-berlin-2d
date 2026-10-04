//! DSP-Bausteine nach dem Vorbild der Web-Audio-Knoten, die `audio.js` verwendet: geglättete Parameter
//! (`setTargetAtTime`), Oszillatoren mit bandbegrenzten Kanten (PolyBLEP) und Wellentabellen (`PeriodicWave`),
//! Rauschen, Biquad-Filter nach der Web-Audio-Spezifikation, Stereo-Panner, weiche Sättigung, Kompressor und
//! Hüllkurven für kurze Klänge.
use std::f32::consts::{PI, TAU};

/// Parameter, der sich exponentiell seinem Ziel nähert (Web Audio `setTargetAtTime`).
#[derive(Debug, Clone, Copy)]
pub struct Smooth {
    pub value: f32,
    pub target: f32,
    k: f32,
}
impl Smooth {
    pub fn new(v: f32) -> Self {
        Self {
            value: v,
            target: v,
            k: 1.,
        }
    }
    /// Ziel und Zeitkonstante (s) setzen.
    pub fn set(&mut self, target: f32, tc: f32, sr: f32) {
        self.target = target;
        self.k = if tc <= 0. {
            1.
        } else {
            1. - (-1. / (tc * sr)).exp()
        };
    }
    #[inline]
    pub fn tick(&mut self) -> f32 {
        self.value += (self.target - self.value) * self.k;
        self.value
    }
}

/// Rauschen (xorshift32), gleichverteilt in [-1, 1).
#[derive(Debug, Clone, Copy)]
pub struct Noise(u32);
impl Noise {
    pub fn new(seed: u32) -> Self {
        Self(seed.max(1))
    }
    #[inline]
    pub fn tick(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x as f32 / u32::MAX as f32) * 2. - 1.
    }
    /// Zufallszahl in [0, 1) für Variationen (Vogelstimmen, Tropfen).
    pub fn unit(&mut self) -> f32 {
        (self.tick() + 1.) * 0.5
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wave {
    Sine,
    Saw,
    Square,
    Triangle,
}

#[inline]
fn poly_blep(t: f32, dt: f32) -> f32 {
    if t < dt {
        let x = t / dt;
        x + x - x * x - 1.
    } else if t > 1. - dt {
        let x = (t - 1.) / dt;
        x * x + x + x + 1.
    } else {
        0.
    }
}

/// Wellentabelle aus Fourierkoeffizienten (normiert auf Spitze 1 wie `PeriodicWave`).
#[derive(Debug, Clone)]
pub struct Table(Vec<f32>);
pub const TABLE_LEN: usize = 2048;
impl Table {
    pub fn from_fourier(re: &[f64], im: &[f64]) -> Self {
        let mut t = vec![0f32; TABLE_LEN];
        for (i, s) in t.iter_mut().enumerate() {
            let ph = TAU as f64 * i as f64 / TABLE_LEN as f64;
            let mut v = 0.;
            for h in 1..re.len().min(im.len()) {
                // Web Audio: x(t) = Σ re[h]·cos(hωt) + im[h]·sin(hωt)
                v += re[h] * (h as f64 * ph).cos() + im[h] * (h as f64 * ph).sin();
            }
            *s = v as f32;
        }
        let peak = t.iter().fold(0f32, |m, v| m.max(v.abs()));
        if peak > 0. {
            t.iter_mut().for_each(|v| *v /= peak);
        }
        Self(t)
    }
    #[inline]
    fn at(&self, phase: f32) -> f32 {
        let x = phase * TABLE_LEN as f32;
        let i = x as usize % TABLE_LEN;
        let f = x - x.floor();
        let a = self.0[i];
        let b = self.0[(i + 1) % TABLE_LEN];
        a + (b - a) * f
    }
}

/// Oszillator mit festem Grundtyp oder Wellentabelle.
#[derive(Debug, Clone)]
pub struct Osc {
    pub wave: Wave,
    pub table: Option<std::sync::Arc<Table>>,
    pub freq: Smooth,
    phase: f32,
}
impl Osc {
    pub fn new(wave: Wave, freq: f32) -> Self {
        Self {
            wave,
            table: None,
            freq: Smooth::new(freq),
            phase: 0.,
        }
    }
    /// Ein Wert; `fm` = zusätzliche Frequenz (Vibrato) in Hz.
    #[inline]
    pub fn next(&mut self, sr: f32, fm: f32) -> f32 {
        let f = (self.freq.tick() + fm).max(0.);
        let dt = (f / sr).min(0.5);
        let t = self.phase;
        let v = if let Some(tab) = &self.table {
            tab.at(t)
        } else {
            match self.wave {
                Wave::Sine => (TAU * t).sin(),
                Wave::Saw => 2. * t - 1. - poly_blep(t, dt),
                Wave::Square => {
                    let mut s = if t < 0.5 { 1. } else { -1. };
                    s += poly_blep(t, dt);
                    s -= poly_blep((t + 0.5) % 1., dt);
                    s
                }
                // Obertöne fallen mit 1/n²: naiv genügt
                Wave::Triangle => 1. - 4. * (t - 0.5).abs(),
            }
        };
        self.phase += dt;
        if self.phase >= 1. {
            self.phase -= 1.;
        }
        v
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterType {
    Lowpass,
    Highpass,
    Bandpass,
    Peaking,
    Lowshelf,
}

/// Biquad nach der Web-Audio-Spezifikation (Lowpass/Highpass: Q in dB; Bandpass/Peaking: Güte; Shelf/Peak: Gain in dB).
#[derive(Debug, Clone, Copy)]
pub struct Biquad {
    pub kind: FilterType,
    pub freq: f32,
    pub q: f32,
    pub gain: f32,
    b: [f32; 3],
    a: [f32; 2],
    z: [f32; 2],
    dirty: bool,
}
impl Biquad {
    pub fn new(kind: FilterType, freq: f32, q: f32) -> Self {
        Self {
            kind,
            freq,
            q,
            gain: 0.,
            b: [1., 0., 0.],
            a: [0., 0.],
            z: [0.; 2],
            dirty: true,
        }
    }
    pub fn set(&mut self, freq: f32, q: f32, gain: f32) {
        if freq != self.freq || q != self.q || gain != self.gain {
            self.freq = freq;
            self.q = q;
            self.gain = gain;
            self.dirty = true;
        }
    }
    pub fn set_freq(&mut self, freq: f32) {
        let (q, g) = (self.q, self.gain);
        self.set(freq, q, g);
    }
    fn update(&mut self, sr: f32) {
        self.dirty = false;
        let f0 = self.freq.clamp(1., sr * 0.49);
        let w0 = TAU * f0 / sr;
        let (sw, cw) = w0.sin_cos();
        let a_gain = 10f32.powf(self.gain / 40.);
        let (b0, b1, b2, a0, a1, a2) = match self.kind {
            FilterType::Lowpass | FilterType::Highpass => {
                let alpha = sw / (2. * 10f32.powf(self.q / 20.));
                let (b0, b1) = if self.kind == FilterType::Lowpass {
                    ((1. - cw) / 2., 1. - cw)
                } else {
                    ((1. + cw) / 2., -(1. + cw))
                };
                (b0, b1, b0, 1. + alpha, -2. * cw, 1. - alpha)
            }
            FilterType::Bandpass => {
                let alpha = sw / (2. * self.q.max(1e-4));
                (alpha, 0., -alpha, 1. + alpha, -2. * cw, 1. - alpha)
            }
            FilterType::Peaking => {
                let alpha = sw / (2. * self.q.max(1e-4));
                (
                    1. + alpha * a_gain,
                    -2. * cw,
                    1. - alpha * a_gain,
                    1. + alpha / a_gain,
                    -2. * cw,
                    1. - alpha / a_gain,
                )
            }
            FilterType::Lowshelf => {
                let alpha = sw / 2. * 2f32.sqrt(); // S = 1
                let sa = 2. * a_gain.sqrt() * alpha;
                (
                    a_gain * ((a_gain + 1.) - (a_gain - 1.) * cw + sa),
                    2. * a_gain * ((a_gain - 1.) - (a_gain + 1.) * cw),
                    a_gain * ((a_gain + 1.) - (a_gain - 1.) * cw - sa),
                    (a_gain + 1.) + (a_gain - 1.) * cw + sa,
                    -2. * ((a_gain - 1.) + (a_gain + 1.) * cw),
                    (a_gain + 1.) + (a_gain - 1.) * cw - sa,
                )
            }
        };
        self.b = [b0 / a0, b1 / a0, b2 / a0];
        self.a = [a1 / a0, a2 / a0];
    }
    #[inline]
    pub fn process(&mut self, x: f32, sr: f32) -> f32 {
        if self.dirty {
            self.update(sr);
        }
        // transponierte Direktform II
        let y = self.b[0] * x + self.z[0];
        self.z[0] = self.b[1] * x - self.a[0] * y + self.z[1];
        self.z[1] = self.b[2] * x - self.a[1] * y;
        if y.is_finite() {
            y
        } else {
            self.z = [0.; 2];
            0.
        }
    }
}

/// Gleichleistungs-Panorama wie `StereoPannerNode` für ein Monosignal: (links, rechts).
#[inline]
pub fn pan(p: f32) -> (f32, f32) {
    let x = (p.clamp(-1., 1.) + 1.) * 0.5 * PI * 0.5;
    (x.cos(), x.sin())
}

/// Weiche Sättigung (Verbrennungsmotor klingt rau).
#[inline]
pub fn shape(x: f32, drive: f32) -> f32 {
    (drive * x).tanh() / drive.tanh()
}

/// Kompressor vor dem Ausgang (Schwelle −14 dB, weiches Knie 12 dB, 4:1, 5 ms / 250 ms).
#[derive(Debug, Clone, Copy)]
pub struct Compressor {
    env: f32,
}
impl Default for Compressor {
    fn default() -> Self {
        Self { env: 0. }
    }
}
impl Compressor {
    pub fn process(&mut self, l: f32, r: f32, sr: f32) -> (f32, f32) {
        let x = l.abs().max(r.abs());
        let coef = if x > self.env {
            1. - (-1. / (0.005 * sr)).exp()
        } else {
            1. - (-1. / (0.25 * sr)).exp()
        };
        self.env += (x - self.env) * coef;
        let db = 20. * self.env.max(1e-6).log10();
        let (thr, knee, ratio) = (-14., 12., 4.);
        let over = db - thr;
        let reduce = if over <= -knee / 2. {
            0.
        } else if over >= knee / 2. {
            over * (1. - 1. / ratio)
        } else {
            let k = over + knee / 2.;
            (1. - 1. / ratio) * k * k / (2. * knee)
        };
        let g = 10f32.powf(-reduce / 20.);
        (l * g, r * g)
    }
}

/// Hüllkurve kurzer Klänge: linearer Anstieg auf `gain`, dann exponentieller Abfall auf 0,0001 bis `dur`.
#[derive(Debug, Clone, Copy)]
pub struct Env {
    pub gain: f32,
    pub attack: f32,
    pub dur: f32,
}
impl Env {
    #[inline]
    pub fn at(&self, t: f32) -> f32 {
        if t < 0. || t > self.dur {
            0.
        } else if t < self.attack {
            self.gain * t / self.attack
        } else {
            let u = ((t - self.attack) / (self.dur - self.attack).max(1e-4)).min(1.);
            self.gain * (0.0001 / self.gain.max(1e-6)).powf(u)
        }
    }
}

/// Kammfilter mit gedämpfter Rückkopplung (Freeverb).
#[derive(Debug, Clone)]
struct Comb {
    buf: Vec<f32>,
    i: usize,
    store: f32,
}
impl Comb {
    fn new(len: usize) -> Self {
        Self {
            buf: vec![0.; len.max(1)],
            i: 0,
            store: 0.,
        }
    }
    #[inline]
    fn process(&mut self, x: f32, feedback: f32, damp: f32) -> f32 {
        let y = self.buf[self.i];
        self.store = y * (1. - damp) + self.store * damp;
        self.buf[self.i] = x + self.store * feedback;
        self.i = (self.i + 1) % self.buf.len();
        y
    }
}
/// Allpass (Freeverb, Faktor 0,5).
#[derive(Debug, Clone)]
struct AllPass {
    buf: Vec<f32>,
    i: usize,
}
impl AllPass {
    fn new(len: usize) -> Self {
        Self {
            buf: vec![0.; len.max(1)],
            i: 0,
        }
    }
    #[inline]
    fn process(&mut self, x: f32) -> f32 {
        let b = self.buf[self.i];
        self.buf[self.i] = x + b * 0.5;
        self.i = (self.i + 1) % self.buf.len();
        b - x
    }
}
/// Nachhall für Bahnhofshalle und Tunnel: vier Kammfilter und zwei Allpässe je Kanal (Längen nach Freeverb, rechts
/// leicht versetzt für Breite). `room` 0…1 = Nachhallzeit, `damp` 0…1 = Höhen schlucken.
#[derive(Debug, Clone)]
pub struct Reverb {
    combs: [Vec<Comb>; 2],
    alls: [Vec<AllPass>; 2],
    pub room: f32,
    pub damp: f32,
}
impl Reverb {
    pub fn new(sr: f32) -> Self {
        let k = sr / 44100.;
        let combs = |off: usize| {
            [1116, 1277, 1422, 1557]
                .iter()
                .map(|&n| Comb::new(((n + off) as f32 * k) as usize))
                .collect()
        };
        let alls = |off: usize| {
            [556, 341]
                .iter()
                .map(|&n| AllPass::new(((n + off) as f32 * k) as usize))
                .collect()
        };
        Self {
            combs: [combs(0), combs(23)],
            alls: [alls(0), alls(23)],
            room: 0.6,
            damp: 0.3,
        }
    }
    /// Ein Abtastwert hinein, Hallanteil links/rechts heraus.
    #[inline]
    pub fn process(&mut self, x: f32) -> (f32, f32) {
        let fb = 0.7 + 0.28 * self.room.clamp(0., 1.);
        let damp = self.damp.clamp(0., 1.);
        let mut out = [0f32; 2];
        for (ch, o) in out.iter_mut().enumerate() {
            let mut y: f32 = self.combs[ch]
                .iter_mut()
                .map(|c| c.process(x * 0.25, fb, damp))
                .sum();
            for a in &mut self.alls[ch] {
                y = a.process(y);
            }
            *o = y;
        }
        (out[0], out[1])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const SR: f32 = 48000.;

    fn goertzel(x: &[f32], f: f32) -> f32 {
        let w = TAU * f / SR;
        let c = 2. * w.cos();
        let (mut s1, mut s2) = (0f32, 0f32);
        for &v in x {
            let s = v + c * s1 - s2;
            s2 = s1;
            s1 = s;
        }
        (s1 * s1 + s2 * s2 - c * s1 * s2).sqrt() / x.len() as f32
    }

    #[test]
    fn smoothing_matches_set_target_at_time() {
        let mut s = Smooth::new(0.);
        s.set(1., 0.1, SR);
        for _ in 0..(0.1 * SR) as usize {
            s.tick();
        }
        assert!(
            (s.value - (1. - (-1f32).exp())).abs() < 0.001,
            "{}",
            s.value
        );
    }

    #[test]
    fn oscillators_hit_their_frequency() {
        for w in [Wave::Sine, Wave::Saw, Wave::Square, Wave::Triangle] {
            let mut o = Osc::new(w, 440.);
            let x: Vec<f32> = (0..SR as usize / 2).map(|_| o.next(SR, 0.)).collect();
            let on = goertzel(&x, 440.);
            let off = goertzel(&x, 523.);
            assert!(on > 0.1 && on > off * 20., "{w:?}: {on} vs {off}");
            assert!(x.iter().all(|v| v.abs() <= 1.3));
        }
    }

    #[test]
    fn wavetable_from_fourier() {
        let (mut re, mut im) = (vec![0.; 8], vec![0.; 8]);
        im[1] = 1.; // reiner Sinus
        re[3] = 0.5;
        let t = Table::from_fourier(&re, &im);
        let mut o = Osc::new(Wave::Sine, 100.);
        o.table = Some(std::sync::Arc::new(t));
        let x: Vec<f32> = (0..SR as usize).map(|_| o.next(SR, 0.)).collect();
        let (h1, h3, h2) = (goertzel(&x, 100.), goertzel(&x, 300.), goertzel(&x, 200.));
        assert!(
            (h3 / h1 - 0.5).abs() < 0.02 && h2 < 0.01 * h1,
            "{h1} {h2} {h3}"
        );
        assert!(x.iter().fold(0f32, |m, v| m.max(v.abs())) <= 1.001);
    }

    #[test]
    fn filters_pass_and_stop() {
        let tone = |f: f32, filt: &mut Biquad| {
            let mut o = Osc::new(Wave::Sine, f);
            let x: Vec<f32> = (0..SR as usize / 4)
                .map(|_| filt.process(o.next(SR, 0.), SR))
                .collect();
            goertzel(&x[x.len() / 2..], f) * 2.
        };
        let mut lp = Biquad::new(FilterType::Lowpass, 400., 0.);
        assert!(
            tone(100., &mut lp) > 0.9
                && tone(4000., &mut Biquad::new(FilterType::Lowpass, 400., 0.)) < 0.02
        );
        let mut hp = Biquad::new(FilterType::Highpass, 2000., 0.);
        assert!(tone(200., &mut hp) < 0.02);
        let bp = |f| tone(f, &mut Biquad::new(FilterType::Bandpass, 1000., 5.));
        assert!((bp(1000.) - 1.).abs() < 0.05 && bp(300.) < 0.1);
        let mut pk = Biquad::new(FilterType::Peaking, 140., 0.65);
        pk.set(140., 0.65, 6.);
        assert!((tone(140., &mut pk) - 10f32.powf(6. / 20.)).abs() < 0.05);
        let mut ls = Biquad::new(FilterType::Lowshelf, 220., 0.);
        ls.set(220., 0., 4.);
        assert!(
            tone(40., &mut ls) > 1.4
                && (tone(8000., &mut Biquad::new(FilterType::Lowshelf, 220., 0.)) - 1.).abs()
                    < 0.05
        );
    }

    #[test]
    fn pan_compressor_envelope() {
        let (l, r) = pan(0.);
        assert!((l - r).abs() < 1e-6 && (l * l + r * r - 1.).abs() < 1e-5);
        assert!(pan(-1.).1.abs() < 1e-6);
        let mut c = Compressor::default();
        let mut peak = 0f32;
        for _ in 0..4800 {
            peak = c.process(2., 2., SR).0;
        }
        assert!(peak < 1.2, "laute Signale werden gezähmt: {peak}");
        let mut q = Compressor::default();
        assert!(
            (q.process(0.05, 0.05, SR).0 - 0.05).abs() < 1e-6,
            "leise bleibt unverändert"
        );
        let e = Env {
            gain: 0.2,
            attack: 0.01,
            dur: 0.5,
        };
        assert_eq!(e.at(0.), 0.);
        assert!((e.at(0.01) - 0.2).abs() < 1e-6 && e.at(0.5) <= 0.00011 && e.at(0.6) == 0.);
        let mut n = Noise::new(1);
        let mean: f32 = (0..10000).map(|_| n.tick()).sum::<f32>() / 10000.;
        assert!(mean.abs() < 0.05);
    }
    #[test]
    fn reverb_rings_on_and_dies_away() {
        let sr = 48000.;
        let mut r = Reverb::new(sr);
        // ein Klick, dann Stille: Hall setzt verzögert ein (erste Reflexion nach ~25 ms) und klingt in ~1,5 s ab
        let buckets = 30; // je 0,1 s
        let mut energy = vec![0f32; buckets];
        for n in 0..(sr as usize * 3) {
            let (l, rr) = r.process(if n == 0 { 1. } else { 0. });
            assert!(l.is_finite() && rr.is_finite());
            energy[n * buckets / (sr as usize * 3)] += l * l + rr * rr;
        }
        let first_reflection = (0.02 * sr) as usize;
        let mut q = Reverb::new(sr);
        let early: f32 = (0..first_reflection)
            .map(|n| q.process(if n == 0 { 1. } else { 0. }).0.abs())
            .sum();
        assert_eq!(early, 0., "nichts vor der ersten Reflexion");
        assert!(energy[0] > 0., "Hall hörbar");
        // −60 dB (Faktor 10⁶ in der Energie) nach etwa 1,5 s
        assert!(energy[16] < energy[0] * 1e-5, "{:?}", &energy[..18]);
        assert!(
            energy[8] > energy[0] * 1e-5,
            "nicht zu trocken: {:?}",
            &energy[..10]
        );
        // ohne Eingang bleibt er still
        let mut z = Reverb::new(sr);
        assert!((0..1000).all(|_| z.process(0.) == (0., 0.)));
    }
}
