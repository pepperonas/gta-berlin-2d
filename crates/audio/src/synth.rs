//! Synthesizer (Port von `audio.js`): keine Aufnahmen, alles wird erzeugt.
//!
//! Signalweg wie in der Browserfassung: alles Draußen (Umgebung, fremde Autos) läuft über den Bus „outside“ mit
//! einem Tiefpass, der im Auto (Karosserie) und bei Schneedecke die Höhen schluckt; das eigene Fahrzeug (Motor,
//! Reifen, Fahrtwind) hat einen eigenen Bus. Hauptpegel 0,55, Kompressor vor dem Ausgang.
//! Die Parameter kommen je Bild über [`Frame`]; geglättet wird wie mit `setTargetAtTime`.
use crate::dsp::{
    Biquad, Compressor, Env, FilterType, Noise, Osc, Reverb, Smooth, Table, Wave, pan, shape,
};
use crate::sampler::{EngineFrame, SamplerVoice, bank_v10, sfx_bank, sfx_samples_on, weapon_bank};
use berlin_sim::ambience::Mix;
use berlin_sim::enginevoice::{Voice, engine_spectrum};
use berlin_sim::railsound::{RailMix, TrainLayers};
use berlin_sim::soundscape::{CarVoice, EngineState, Footstep, Tires};
use std::collections::HashMap;
use std::sync::Arc;

pub const MASTER: f32 = 0.55;
/// Pegel der Motor-Samples relativ zum Synthese-Motor (eingemessen mit `--audio-wav`, docs/audio.md)
pub const SAMPLE_LEVEL: f32 = 0.38;
/// so viele Sample-Motorstimmen hält der Synthesizer bereit (die Auswahl trifft das Spiel)
pub const SAMPLE_VOICES: usize = 10;
/// Pegel der Schuss-Samples je Waffe (Pistole, MP, Schrotflinte), eingemessen gegen den früheren Synthese-Schuss
/// (Test `gun_samples_are_loud_but_not_clipping`)
pub const GUN_LEVEL: [f32; 3] = [0.85, 0.6, 0.9];
/// Choke: ein neuer Schuss derselben Waffe blendet den Nachhall des vorigen aus (Zeitkonstante s), wenn der
/// vorige schon so alt ist (s) – eine Salve verschwimmt sonst zu Brei, nur der letzte Hall klingt aus
pub const GUN_CHOKE_TC: f32 = 0.03;
pub const GUN_CHOKE_AGE: f64 = 0.03;
/// Klang-Samples (tools/audio/build_sfx.py): Name, Pegel, Tonhöhenstreuung (±), Entfernungs-Tiefpass.
/// Pegel eingemessen auf Zielwerte (Test `sfx_samples_match_the_synth_loudness`): Schritte so laut wie der frühere
/// harte Synthese-Schritt (Gras leiser – die Synthese war dort fast unhörbar), alles andere 20 % über der Synthese; der
/// Fehlschlag-Jingle so laut wie der Erfolg (die Synthese war dort leiser).
#[derive(Debug, Clone, Copy)]
pub struct SfxSpec {
    pub name: &'static str,
    pub level: f32,
    pub spread: f32,
    pub distance: bool,
}
const fn spec(name: &'static str, level: f32, spread: f32, distance: bool) -> SfxSpec {
    SfxSpec {
        name,
        level,
        spread,
        distance,
    }
}
pub const STEP_HARD: SfxSpec = spec("step_hard", 0.129, 0.08, false);
pub const STEP_GRASS: SfxSpec = spec("step_grass", 0.087, 0.08, false);
pub const STEP_SNOW: SfxSpec = spec("step_snow", 0.071, 0.08, false);
pub const STEP_WET: SfxSpec = spec("step_wet", 0.069, 0.08, false);
pub const SWING: SfxSpec = spec("swing", 0.096, 0.1, true);
pub const PUNCH: SfxSpec = spec("punch", 0.314, 0.06, true);
pub const THUD: SfxSpec = spec("thud", 0.279, 0.06, true);
pub const IMPACT: SfxSpec = spec("impact", 0.131, 0.1, true);
pub const HIT: SfxSpec = spec("hit", 0.217, 0.06, true);
pub const RELOAD: SfxSpec = spec("reload", 0.205, 0.03, false);
pub const RELOADED: SfxSpec = spec("reloaded", 0.135, 0.03, false);
pub const WEAPON_SWITCH: SfxSpec = spec("weapon_switch", 0.154, 0.05, false);
pub const UI: SfxSpec = spec("ui", 0.41, 0.02, false);
pub const TICK: SfxSpec = spec("tick", 0.386, 0., false);
pub const PICKUP: SfxSpec = spec("pickup", 0.123, 0., false);
pub const MISSION_START: SfxSpec = spec("mission_start", 0.132, 0., false);
pub const MISSION_SUCCESS: SfxSpec = spec("mission_success", 0.155, 0., false);
pub const MISSION_FAIL: SfxSpec = spec("mission_fail", 0.108, 0., false);
/// Choke-Gruppe für Samples ohne Choke
const NO_CHOKE: u8 = u8::MAX;
const BLOCK: usize = 32;

/// Zustand des selbst gefahrenen Fahrzeugs (oder `active = false` zu Fuß).
#[derive(Debug, Clone, Copy, Default)]
pub struct Vehicle {
    pub active: bool,
    pub in_car: bool,
    pub rain: f32,
    pub engine: Option<EngineState>,
    pub tires: Option<Tires>,
}

/// Einzelklänge (Ereignisse der Simulation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Sfx {
    Crash(f32),
    Hit,
    Horn(f32),
    Knock(f32),
    Door,
    Ui,
    Tick,
    Pickup,
    MissionStart,
    MissionSuccess,
    MissionFail,
    Carjack,
    Footstep(Footstep, f32),
    /// Donner: Lautstärke 0…1, nah (trockener Knall vor dem Grollen)
    Thunder(f32, bool),
    /// Schuss: Waffe (0 Pistole, 1 MP, 2 Schrotflinte) und Lautstärke
    Gun(u8, f32),
    /// Schlag ins Leere bzw. Treffer (Faust/Schläger), Lautstärke
    Swing(f32),
    Punch(f32),
    /// Schlag auf Blech
    Thud(f32),
    /// Kugeleinschlag
    Impact(f32),
    Reload,
    Reloaded,
    WeaponSwitch,
    /// Spritzwasser beim Aufschwimmen (Aquaplaning)
    Splash(f32),
    /// Straßenbahnklingel (zweimal)
    TramBell(f32),
    /// Türgong: öffnen (aufsteigend), schließen (absteigend)
    GongOpen,
    GongClose,
    /// Kirchenglocke: n Schläge (Grundton + unharmonische Teiltöne, langer Nachhall), Lautstärke
    Bells(u32, f32),
    /// Schienenstoß unter einer Achse: dumpfer Schlag mit metallischem Klicken
    RailJoint(f32),
    /// Druckluft: Bremse löst bzw. Türen arbeiten
    AirHiss(f32),
    /// Abfertigung: Warnton vor dem Schließen der Türen
    DepartBeep(f32),
}

/// Alles, was ein Bild an den Klang meldet.
#[derive(Debug, Clone, Default)]
pub struct Frame {
    pub vehicle: Vehicle,
    pub voices: Vec<CarVoice>,
    pub ambience: Mix,
    pub sfx: Vec<Sfx>,
    /// S-/U-Bahn: eigener Zug, Zug am Bahnsteig, Nachhall
    pub rail: RailMix,
    /// Motoren aus Aufnahmen (Spielerauto und Verkehr); diese Autos fehlen in `vehicle.engine` bzw. `voices`
    pub engines: Vec<EngineFrame>,
    /// Pegel des Mischpult-Kanals „engine“
    pub engine_mix: f32,
    /// A/B-Vergleich: solange gesetzt, läuft die Referenzaufnahme (in Schleife) statt der Motor-Samples
    pub reference: Option<Arc<[f32]>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dest {
    Master,
    Outside,
    Engine,
}

/// Rauschen → Filter → geglättete Verstärkung.
#[derive(Debug, Clone)]
struct NoiseLayer {
    noise: Noise,
    filter: Biquad,
    gain: Smooth,
    freq: Smooth,
}
impl NoiseLayer {
    fn new(kind: FilterType, freq: f32, q: f32, seed: u32) -> Self {
        Self {
            noise: Noise::new(seed),
            filter: Biquad::new(kind, freq, q),
            gain: Smooth::new(0.),
            freq: Smooth::new(freq),
        }
    }
    /// Gefiltertes Rauschen ohne Verstärkung.
    #[inline]
    fn filtered(&mut self, sr: f32, block: bool) -> f32 {
        let f = self.freq.tick();
        if block {
            self.filter.set_freq(f);
        }
        self.filter.process(self.noise.tick(), sr)
    }
    #[inline]
    fn next(&mut self, sr: f32, block: bool) -> f32 {
        self.filtered(sr, block) * self.gain.tick()
    }
}

/// Stimme einer S-/U-Bahn (`railsound`): Rollen, Grollen, Fahrtwind, Fahrmotor-Surren, Bremsquietschen.
#[derive(Debug, Clone)]
struct TrainVoice {
    roll: NoiseLayer,
    rumble: NoiseLayer,
    wind: NoiseLayer,
    motor: Osc,
    motor2: Osc,
    motor_g: Smooth,
    motor_f: Smooth,
    motor_bp: Biquad,
    sq: [Osc; 2],
    sqg: Smooth,
    vib: Osc,
    pan: Smooth,
}
impl TrainVoice {
    fn new(seed: &mut u32) -> Self {
        use FilterType::*;
        let mut l = |k, f, q| layer(k, f, q, seed);
        Self {
            roll: l(Bandpass, 400., 0.8),
            rumble: l(Lowpass, 75., 0.7),
            wind: l(Lowpass, 420., 0.5),
            motor: Osc::new(Wave::Saw, 150.),
            motor2: Osc::new(Wave::Sine, 225.),
            motor_g: Smooth::new(0.),
            motor_f: Smooth::new(150.),
            motor_bp: Biquad::new(Bandpass, 300., 1.2),
            sq: [Osc::new(Wave::Sine, 2650.), Osc::new(Wave::Sine, 3970.)],
            sqg: Smooth::new(0.),
            vib: Osc::new(Wave::Sine, 7.),
            pan: Smooth::new(0.),
        }
    }
    /// `k` = Gesamtpegel (im Wagen 1, am Bahnsteig etwas lauter, der Zug ist näher am Ohr als die Wand)
    fn set(&mut self, l: &TrainLayers, sr: f32, k: f32) {
        let set = |x: &mut Smooth, v: f64, tc: f32| x.set(v as f32, tc, sr);
        set(&mut self.roll.gain, 0.11 * k as f64 * l.roll, 0.12);
        set(&mut self.roll.freq, l.roll_f.max(60.), 0.2);
        set(&mut self.rumble.gain, 0.3 * k as f64 * l.rumble, 0.15);
        set(&mut self.wind.gain, 0.08 * k as f64 * l.wind, 0.25);
        set(&mut self.motor_g, 0.028 * k as f64 * l.motor, 0.12);
        set(&mut self.motor_f, l.motor_f.max(40.), 0.12);
        set(&mut self.sqg, 0.016 * k as f64 * l.squeal, 0.08);
    }
    #[inline]
    fn next(&mut self, sr: f32, block: bool) -> f32 {
        let mf = self.motor_f.tick();
        self.motor.freq.value = mf;
        self.motor.freq.target = mf;
        self.motor2.freq.value = mf * 1.5;
        self.motor2.freq.target = mf * 1.5;
        if block {
            self.motor_bp.set_freq(mf * 2.);
        }
        let m =
            self.motor_bp.process(self.motor.next(sr, 0.), sr) + 0.35 * self.motor2.next(sr, 0.);
        let vib = self.vib.next(sr, 0.) * 30.;
        let sq = self.sq[0].next(sr, vib) + 0.5 * self.sq[1].next(sr, vib * 1.5);
        self.roll.next(sr, block)
            + self.rumble.next(sr, block)
            + self.wind.next(sr, block)
            + m * self.motor_g.tick()
            + sq * self.sqg.tick()
    }
}

/// Kurzer Klang: Ton (mit Gleiten und optionalem Tiefpass) oder gefiltertes Rauschen.
#[derive(Debug, Clone)]
struct Shot {
    start: f64,
    env: Env,
    dest: Dest,
    pan: f32,
    tone: Option<(Osc, f32, f32)>, // Oszillator, Startfrequenz, Endfrequenz
    noise: Option<Noise>,
    filter: Option<Biquad>,
    /// Grollen: Hüllkurve aus Stützpunkten (Zeit, Pegel) und Filter-Gleiten (Start, Ende)
    rumble: Option<Rumble>,
}
/// Stützpunkte (Zeit, Pegel), Filterfrequenz am Anfang und am Ende.
type Rumble = (Vec<(f32, f32)>, f32, f32);

/// Abspielende Aufnahme (Schuss): Puffer, Lesekopf, Tempo, Pegel, Choke-Gruppe, Tiefpass der Entfernung.
struct SamplePlay {
    buf: Arc<[f32]>,
    pos: f64,
    rate: f64,
    gain: f32,
    group: u8,
    age: f64,
    release: bool,
    lp_k: f32,
    lp: f32,
    dest: Dest,
}

struct EngineVoice {
    bus: Smooth,
    o1: Osc,
    o2: Osc,
    o3: Osc,
    g1: Smooth,
    g2: Smooth,
    g3: Smooth,
    lp: Biquad,
    lp_f: Smooth,
    body: Biquad,
    body_f: Smooth,
    body_g: Smooth,
    bass: Biquad,
    bass_g: Smooth,
    g: Smooth,
    ex: NoiseLayer,
    am: Osc,
    amg: Smooth,
    clatter: NoiseLayer,
    clm: Smooth,
    whine: Osc,
    whg: Smooth,
    intake: NoiseLayer,
    turbo_noise: NoiseLayer,
    turbo: Osc,
    turbo_g: Smooth,
    reverse: Osc,
    reverse_g: Smooth,
    roll: NoiseLayer,
    cobble: NoiseLayer,
    wet: NoiseLayer,
    snow: NoiseLayer,
    slide: NoiseLayer,
    wind: NoiseLayer,
    squeal: NoiseLayer,
    roof: NoiseLayer,
    roof_low: NoiseLayer,
    sq: [Osc; 2],
    vib: Osc,
    sqg: Smooth,
    custom: bool,
    previous_load: f32,
    previous_boost: f32,
    profile_key: Option<&'static str>,
    release_at: f64,
}

struct CarSlot {
    id: Option<u32>,
    g: Smooth,
    o: Osc,
    o2: Osc,
    og2: Smooth,
    f: Biquad,
    f_f: Smooth,
    tire: NoiseLayer,
    pan: Smooth,
}

struct Ambience {
    hum: NoiseLayer,
    traffic: NoiseLayer,
    water: NoiseLayer,
    rain: NoiseLayer,
    rain_low: NoiseLayer,
    wind: NoiseLayer,
    whistle: NoiseLayer,
    /// Rumpeln der Bahnen (tiefes Rauschen)
    rumble: NoiseLayer,
    /// Martinshorn (Dreieck, tief/hoch im Wechsel)
    siren: Osc,
    siren_g: Smooth,
    next_chirp: f64,
    drops: f64,
    last: f64,
    /// Nachtleben: Stimmengewirr in drei Formantbändern, Richtung, Silbentakt, Lachen, Gläser, Club-Takt
    babble: [NoiseLayer; 3],
    bar_pan: Smooth,
    next_syllable: f64,
    next_laugh: f64,
    next_clink: f64,
    next_beat: f64,
    beat: u32,
}

pub struct Synth {
    pub sr: f32,
    t: f64,
    tick: usize,
    master: Smooth,
    muted: bool,
    muffle: [Biquad; 2],
    muffle_f: Smooth,
    comp: Compressor,
    engine: EngineVoice,
    cars: Vec<CarSlot>,
    amb: Ambience,
    shots: Vec<Shot>,
    tables: HashMap<(&'static str, u32, bool), Arc<Table>>,
    rng: Noise,
    /// eigene Bahn, Bahn am Bahnsteig, Nachhall (Halle, Tunnel)
    ride: TrainVoice,
    pass: TrainVoice,
    hall: Reverb,
    hall_g: Smooth,
    /// Motoren aus Aufnahmen und der Mischpult-Kanal „engine“
    samplers: Vec<SamplerVoice>,
    engine_ch: Smooth,
    reference: Option<(Arc<[f32]>, f64)>,
    /// laufende Aufnahmen (Schüsse, Klang-Samples)
    plays: Vec<SamplePlay>,
    /// Klang-Samples statt Synthese (`GTA_SFX_SAMPLES=0` = aus)
    pub use_samples: bool,
}

fn layer(kind: FilterType, f: f32, q: f32, seed: &mut u32) -> NoiseLayer {
    *seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
    NoiseLayer::new(kind, f, q, *seed | 1)
}

impl Synth {
    pub fn new(sr: f32) -> Self {
        use FilterType::*;
        let mut seed = 0x1234_5678u32;
        let mut l = |k, f, q| layer(k, f, q, &mut seed);
        let mut body = Biquad::new(Peaking, 140., 0.65);
        body.set(140., 0.65, 5.);
        let mut bass = Biquad::new(Lowshelf, 220., 0.);
        bass.set(220., 0., 4.);
        let engine = EngineVoice {
            bus: Smooth::new(0.),
            o1: Osc::new(Wave::Saw, 30.),
            o2: Osc::new(Wave::Square, 15.),
            o3: Osc::new(Wave::Saw, 60.),
            g1: Smooth::new(0.5),
            g2: Smooth::new(0.35),
            g3: Smooth::new(0.15),
            lp: Biquad::new(Lowpass, 380., 0.65),
            lp_f: Smooth::new(380.),
            body,
            body_f: Smooth::new(140.),
            body_g: Smooth::new(5.),
            bass,
            bass_g: Smooth::new(4.),
            g: Smooth::new(0.),
            ex: l(Bandpass, 220., 1.3),
            am: Osc::new(Wave::Square, 30.),
            amg: Smooth::new(0.),
            clatter: l(Bandpass, 1500., 0.6),
            clm: Smooth::new(0.),
            whine: Osc::new(Wave::Sine, 160.),
            whg: Smooth::new(0.),
            intake: l(Bandpass, 900., 0.8),
            turbo_noise: l(Bandpass, 2800., 0.6),
            turbo: Osc::new(Wave::Sine, 950.),
            turbo_g: Smooth::new(0.),
            reverse: Osc::new(Wave::Sine, 180.),
            reverse_g: Smooth::new(0.),
            roll: l(Lowpass, 300., 0.6),
            cobble: l(Lowpass, 140., 1.5),
            wet: l(Highpass, 2600., 0.5),
            snow: l(Bandpass, 1500., 0.9),
            slide: l(Bandpass, 700., 0.8),
            wind: l(Bandpass, 520., 0.6),
            squeal: l(Bandpass, 2300., 5.),
            roof: l(Bandpass, 1600., 0.6),
            roof_low: l(Lowpass, 220., 0.8),
            sq: [
                Osc::new(Wave::Triangle, 960.),
                Osc::new(Wave::Triangle, 1010.),
            ],
            vib: Osc::new(Wave::Sine, 9.),
            sqg: Smooth::new(0.),
            custom: false,
            previous_load: 0.,
            previous_boost: 0.,
            profile_key: None,
            release_at: 0.,
        };
        let cars = (0..4)
            .map(|_| CarSlot {
                id: None,
                g: Smooth::new(0.),
                o: Osc::new(Wave::Saw, 40.),
                o2: Osc::new(Wave::Square, 20.),
                og2: Smooth::new(0.5),
                f: Biquad::new(Lowpass, 500., 0.9),
                f_f: Smooth::new(500.),
                tire: l(Bandpass, 600., 0.5),
                pan: Smooth::new(0.),
            })
            .collect();
        let amb = Ambience {
            hum: l(Lowpass, 180., 0.7),
            traffic: l(Bandpass, 420., 0.6),
            water: l(Highpass, 1400., 0.7),
            rain: l(Highpass, 2600., 0.7),
            rain_low: l(Bandpass, 900., 0.5),
            wind: l(Bandpass, 380., 1.4),
            whistle: l(Bandpass, 900., 12.),
            rumble: l(Lowpass, 90., 0.7),
            siren: Osc::new(Wave::Triangle, 440.),
            siren_g: Smooth::new(0.),
            next_chirp: 0.,
            drops: 0.,
            last: 0.,
            babble: [
                l(Bandpass, 480., 3.),
                l(Bandpass, 1150., 4.),
                l(Bandpass, 2500., 5.),
            ],
            bar_pan: Smooth::new(0.),
            next_syllable: 0.,
            next_laugh: 0.,
            next_clink: 0.,
            next_beat: 0.,
            beat: 0,
        };
        Self {
            sr,
            t: 0.,
            tick: 0,
            master: Smooth::new(MASTER),
            muted: false,
            muffle: [
                Biquad::new(Lowpass, 18000., 0.5),
                Biquad::new(Lowpass, 18000., 0.5),
            ],
            muffle_f: Smooth::new(18000.),
            comp: Compressor::default(),
            engine,
            cars,
            amb,
            shots: Vec::new(),
            tables: HashMap::new(),
            rng: Noise::new(0x9e37_79b9),
            ride: TrainVoice::new(&mut seed),
            pass: TrainVoice::new(&mut seed),
            hall: Reverb::new(sr),
            hall_g: Smooth::new(0.),
            samplers: (0..SAMPLE_VOICES)
                .map(|_| SamplerVoice::new(bank_v10()))
                .collect(),
            engine_ch: Smooth::new(1.),
            reference: None,
            plays: Vec::new(),
            use_samples: sfx_samples_on(),
        }
    }

    pub fn toggle_mute(&mut self) -> bool {
        self.muted = !self.muted;
        let sr = self.sr;
        self.master
            .set(if self.muted { 0. } else { MASTER }, 0.025, sr);
        self.muted
    }
    pub fn time(&self) -> f64 {
        self.t
    }
    pub fn active_shots(&self) -> usize {
        self.shots.len()
    }

    fn table(&mut self, v: &Voice, cyl: u32, intake: bool) -> Arc<Table> {
        self.tables
            .entry((v.id, cyl, intake))
            .or_insert_with(|| {
                let (re, im) = engine_spectrum(v, cyl, intake);
                Arc::new(Table::from_fourier(&re, &im))
            })
            .clone()
    }

    /// Parameter eines Bildes übernehmen.
    pub fn apply(&mut self, f: &Frame) {
        self.set_vehicle(&f.vehicle);
        self.set_voices(&f.voices);
        self.set_ambience(&f.ambience);
        self.set_rail(&f.rail);
        self.set_engines(f);
        for s in &f.sfx {
            self.play(*s);
        }
    }

    fn set_vehicle(&mut self, veh: &Vehicle) {
        let sr = self.sr;
        let (active, in_car) = (veh.active, veh.in_car);
        let eng = veh.engine;
        let profile = eng.map(|e| e.voice);
        let tables = match (active, eng, profile) {
            (true, Some(e), Some(p)) if !e.electric => {
                Some((self.table(&p, e.cyl, false), self.table(&p, e.cyl, true)))
            }
            _ => None,
        };
        let t = self.t;
        let e = &mut self.engine;
        e.bus.set(if active || in_car { 1. } else { 0. }, 0.15, sr);
        let set = |s: &mut Smooth, v: f32, tc: f32| s.set(v, tc, sr);
        let p = profile.unwrap_or_else(|| berlin_sim::enginevoice::voice_for("", false, false));
        let load = eng.map(|e| e.load).unwrap_or(0.) as f32;
        let n = eng.map(|e| e.norm).unwrap_or(0.) as f32;
        let combustion = active && eng.is_some_and(|e| !e.electric);
        let damping = if in_car {
            1. - p.insulation as f32 * 0.4
        } else {
            1.
        };
        let boost = if combustion {
            eng.map(|e| e.boost).unwrap_or(0.) as f32
        } else {
            0.
        };
        set(&mut e.turbo.freq, 950. + boost * 2100., 0.16);
        set(&mut e.turbo_g, boost * 0.003 * damping, 0.15);
        set(&mut e.turbo_noise.gain, boost * 0.008 * damping, 0.12);
        set(
            &mut e.intake.gain,
            if combustion {
                load * n * 0.012 * damping
            } else {
                0.
            },
            0.08,
        );
        set(&mut e.intake.freq, 350. + n * 850., 0.08);
        let speed = eng.map(|e| e.speed).unwrap_or(0.) as f32;
        set(&mut e.reverse.freq, 180. + speed * 7., 0.1);
        let reversing = active && eng.is_some_and(|e| e.reverse && !e.electric);
        set(
            &mut e.reverse_g,
            if reversing {
                (speed * 0.0003).min(0.018)
            } else {
                0.
            },
            0.08,
        );
        match eng {
            Some(es) if active && es.electric => {
                set(&mut e.g, 0., 0.08);
                set(
                    &mut e.whine.freq,
                    160. + es.rpm as f32 / 16000. * 2200.,
                    0.08,
                );
                let moving = (speed / 25.).min(1.);
                set(
                    &mut e.whg,
                    moving * (0.006 + load * 0.014 + es.regen as f32 * 0.009),
                    0.1,
                );
            }
            Some(es) if combustion => {
                set(&mut e.whg, 0., 0.08);
                let fire = (es.fire as f32).max(12.);
                if let Some((a, b)) = tables {
                    e.o1.table = Some(a);
                    e.o3.table = Some(b);
                    e.o2.wave = Wave::Triangle;
                    e.custom = true;
                    let flutter = 1. + p.rough as f32 * 0.025 * (1. - n) * (t as f32 * 13.7).sin();
                    set(&mut e.o1.freq, (es.cycle as f32).max(4.) * flutter, 0.035);
                    set(&mut e.o3.freq, (es.cycle as f32).max(4.), 0.035);
                    set(&mut e.o2.freq, es.rpm as f32 / 60., 0.035);
                }
                set(&mut e.g1, 0.58, 0.08);
                set(&mut e.g2, 0.2 + p.rough as f32 * 0.65 + load * 0.18, 0.08);
                set(&mut e.g3, 0.04 + load * 0.1, 0.08);
                set(&mut e.am.freq, fire, 0.035);
                set(&mut e.body_f, p.resonance as f32, 0.08);
                set(&mut e.body_g, 4. + p.rough as f32 * 8., 0.08);
                set(&mut e.bass_g, 3. + load * 2., 0.08);
                set(
                    &mut e.lp_f,
                    (220. + p.brightness as f32 * (0.18 + n * 0.5 + load * 0.38)) * damping,
                    0.06,
                );
                set(
                    &mut e.g,
                    (0.04 + load * 0.07 + n * 0.018) * p.volume as f32 * damping,
                    0.045,
                );
                set(
                    &mut e.ex.freq,
                    (p.resonance as f32 + fire * 0.8).min(900.),
                    0.05,
                );
                // Lastwechsel: Turbo bläst ab bzw. Sportauspuff knallt
                if e.profile_key == Some(p.id)
                    && e.previous_load > 0.55
                    && load < 0.12
                    && es.shift_t == 0.
                    && t > e.release_at
                {
                    let pb = e.previous_boost;
                    e.release_at = t + 0.8;
                    if pb > 0.25 {
                        self.burst(
                            0.26,
                            1400.,
                            pb * 0.02,
                            FilterType::Bandpass,
                            0.8,
                            0.,
                            0.008,
                            Dest::Engine,
                        );
                    } else if p.pops && n > 0.55 {
                        self.burst(
                            0.12,
                            130.,
                            0.055,
                            FilterType::Bandpass,
                            0.7,
                            0.,
                            0.008,
                            Dest::Engine,
                        );
                    }
                }
            }
            _ => {
                set(&mut e.g, 0., 0.12);
                set(&mut e.whg, 0., 0.12);
            }
        }
        let e = &mut self.engine;
        let exhaust = if combustion {
            (0.009 + load * 0.025) * p.volume as f32 * damping
        } else {
            0.
        };
        let clatter = if combustion && eng.is_some_and(|e| e.diesel) {
            (0.004 + load * 0.006) * damping
        } else {
            0.
        };
        set(&mut e.ex.gain, exhaust, 0.08);
        set(&mut e.amg, exhaust * 0.65, 0.08);
        set(&mut e.clatter.gain, clatter, 0.08);
        set(&mut e.clm, clatter * 0.8, 0.08);
        e.previous_load = if active { load } else { 0. };
        e.previous_boost = boost;
        e.profile_key = if active { Some(p.id) } else { None };
        let tr = if active { veh.tires } else { None };
        let tv = |f: fn(&Tires) -> f64| tr.map(|x| f(&x) as f32).unwrap_or(0.);
        let jitter = self.rng.unit();
        let e = &mut self.engine;
        set(&mut e.roll.gain, 0.05 * tv(|x| x.roll), 0.1);
        set(&mut e.roll.freq, 250. + tv(|x| x.roll) * 900., 0.1);
        set(
            &mut e.cobble.gain,
            0.12 * tv(|x| x.cobble) * (0.6 + 0.4 * jitter),
            0.05,
        );
        set(&mut e.wet.gain, 0.06 * tv(|x| x.wet), 0.1);
        set(
            &mut e.snow.gain,
            0.07 * tv(|x| x.snow) * (0.5 + 0.8 * jitter),
            0.04,
        );
        set(&mut e.slide.gain, 0.09 * tv(|x| x.slide), 0.05);
        set(&mut e.wind.gain, 0.06 * tv(|x| x.wind), 0.2);
        set(&mut e.wind.freq, 380. + tv(|x| x.wind) * 700., 0.2);
        set(&mut e.squeal.gain, 0.05 * tv(|x| x.skid), 0.04);
        set(&mut e.sqg, 0.035 * tv(|x| x.skid), 0.04);
        if tv(|x| x.skid) > 0. {
            // im Drift sinkt das Quietschen mit dem Winkel zum Heulen
            let drop = tv(|x| x.angle) * 300.;
            set(&mut e.sq[0].freq, 960. - tv(|x| x.roll) * 180. - drop, 0.1);
            set(&mut e.sq[1].freq, 1010. - tv(|x| x.roll) * 180. - drop, 0.1);
        }
        let r = if in_car { veh.rain.min(1.6) } else { 0. };
        set(
            &mut e.roof.gain,
            0.05 * r.min(1.) + 0.03 * (r - 1.).max(0.),
            0.4,
        );
        set(&mut e.roof_low.gain, 0.06 * (r - 0.3).max(0.), 0.4);
    }

    /// Motoren aus Aufnahmen: feste Zuordnung Fahrzeug → Stimme (wie bei den Synthese-Stimmen).
    fn set_engines(&mut self, f: &Frame) {
        let sr = self.sr;
        let ch = if f.reference.is_some() {
            0.
        } else {
            f.engine_mix
        };
        self.engine_ch.set(ch, 0.05, sr);
        match (&f.reference, &self.reference) {
            (Some(r), None) => self.reference = Some((r.clone(), 0.)),
            (None, Some(_)) => self.reference = None,
            _ => {}
        }
        for v in &mut self.samplers {
            if v.id.is_some_and(|id| !f.engines.iter().any(|e| e.id == id)) {
                v.release(sr);
            }
        }
        for e in &f.engines {
            let slot = match self.samplers.iter().position(|v| v.id == Some(e.id)) {
                Some(i) => Some(i),
                None => self
                    .samplers
                    .iter()
                    .position(|v| v.silent())
                    .or_else(|| self.samplers.iter().position(|v| v.id.is_none())),
            };
            if let Some(i) = slot {
                self.samplers[i].apply(e, sr);
            }
        }
    }

    /// Fremde Fahrzeuge: feste Zuordnung Auto → Stimme, damit nichts springt.
    fn set_voices(&mut self, list: &[CarVoice]) {
        let sr = self.sr;
        for s in &mut self.cars {
            if s.id.is_some_and(|id| !list.iter().any(|v| v.id == id)) {
                s.id = None;
            }
        }
        for v in list {
            if !self.cars.iter().any(|s| s.id == Some(v.id))
                && let Some(free) = self.cars.iter_mut().find(|s| s.id.is_none())
            {
                free.id = Some(v.id);
            }
        }
        for i in 0..self.cars.len() {
            let v = self.cars[i]
                .id
                .and_then(|id| list.iter().find(|v| v.id == id))
                .copied();
            let table = v
                .filter(|v| !v.engine.electric)
                .map(|v| self.table(&v.engine.voice, v.engine.cyl, false));
            let s = &mut self.cars[i];
            let set = |x: &mut Smooth, val: f32, tc: f32| x.set(val, tc, sr);
            let Some(v) = v else {
                set(&mut s.g, 0., 0.15);
                set(&mut s.tire.gain, 0., 0.15);
                continue;
            };
            let (e, rate, gain) = (v.engine, v.rate as f32, v.gain as f32);
            let p = e.voice;
            set(
                &mut s.g,
                gain * if e.electric {
                    0.025 * v.tire as f32
                } else {
                    0.09 * p.volume as f32
                },
                0.15,
            );
            set(&mut s.tire.gain, 0.05 * gain * v.tire as f32, 0.15);
            if e.electric {
                s.o.table = None;
                s.o.wave = Wave::Sine;
                set(
                    &mut s.o.freq,
                    (160. + e.rpm as f32 / 16000. * 2200.) * rate,
                    0.12,
                );
                set(&mut s.og2, 0., 0.1);
                set(&mut s.f_f, 3500., 0.2);
            } else {
                s.o.table = table;
                set(&mut s.o.freq, (e.cycle as f32 * rate).max(4.), 0.12);
                s.o2.wave = Wave::Triangle;
                set(&mut s.o2.freq, e.rpm as f32 / 60. * rate, 0.12);
                set(
                    &mut s.og2,
                    0.24 + p.rough as f32 * 0.6 + e.load as f32 * 0.15,
                    0.1,
                );
                set(
                    &mut s.f_f,
                    220. + p.brightness as f32
                        * (0.18 + e.norm as f32 * 0.45 + e.load as f32 * 0.3),
                    0.2,
                );
            }
            set(&mut s.tire.freq, 500. * rate + v.tire as f32 * 500., 0.2);
            set(&mut s.pan, v.pan as f32, 0.1);
        }
    }

    fn set_rail(&mut self, r: &RailMix) {
        let sr = self.sr;
        self.ride.set(&r.ride, sr, 0.8);
        self.pass.set(&r.pass, sr, 1.2);
        self.pass.pan.set(r.pass_pan as f32 * 0.8, 0.2, sr);
        self.hall_g.set(r.hall as f32, 0.5, sr);
    }

    fn set_ambience(&mut self, m: &Mix) {
        let sr = self.sr;
        let a = &mut self.amb;
        let set = |x: &mut Smooth, v: f64| x.set(v as f32, 0.6, sr);
        set(&mut a.hum.gain, 0.018 * m.hum);
        set(&mut a.traffic.gain, 0.05 * m.traffic);
        set(&mut a.water.gain, 0.012 * m.water);
        set(&mut a.rumble.gain, 0.16 * m.rumble);
        let rain = m.rain;
        set(
            &mut a.rain.gain,
            0.07 * rain.min(1.) + 0.05 * (rain - 1.).max(0.),
        );
        set(&mut a.rain_low.gain, 0.05 * (rain - 0.6).max(0.));
        set(&mut a.wind.gain, 0.11 * m.wind);
        // Wind: Band steigt mit den Böen, bei starken Böen pfeift es an Kanten
        a.wind.freq.set(260. + m.gust as f32 * 520., 0.5, sr);
        a.whistle
            .gain
            .set(0.03 * (m.wind * (m.gust - 0.35).max(0.)) as f32, 0.4, sr);
        a.whistle.freq.set(700. + m.gust as f32 * 900., 0.6, sr);
        a.siren_g.set(0.07 * m.siren as f32, 0.15, sr);
        if m.siren > 0. {
            a.siren
                .freq
                .set(if m.siren_high { 585. } else { 440. }, 0.02, sr);
        }
        // Regentropfen auf Blech, Pfützen und Blättern (audio.js: je Viertelsekunde bis 22 Tropfen)
        let since = (self.t - a.last).clamp(0., 0.5);
        a.last = self.t;
        a.drops += rain.min(1.6) * 14. * if m.in_car { 0.4 } else { 1. } * 4. * since;
        while self.amb.drops >= 1. {
            self.amb.drops -= 1.;
            let (at, f, d) = (
                self.rng.unit() * 0.25,
                1800. + self.rng.unit() * 5000.,
                0.012 + self.rng.unit() * 0.02,
            );
            let gain = 0.012 + self.rng.unit() * 0.02 * rain.min(1.) as f32;
            self.burst(d, f, gain, FilterType::Bandpass, 3., at, 0., Dest::Outside);
        }
        self.muffle_f
            .set(18000. * 0.04f32.powf(m.muffle as f32), 0.3, sr);
        self.set_bar(m);
        // Vogelstimmen: kurze Tonfolgen, je mehr Grün, desto öfter
        if m.birds > 0.02 && self.t > self.amb.next_chirp {
            let base = 2400. + self.rng.unit() * 2600.;
            let n = 2 + (self.rng.unit() * 4.) as usize;
            for i in 0..n {
                let f = base * (1. + (self.rng.unit() - 0.5) * 0.3);
                let slide = (self.rng.unit() - 0.3) * 900.;
                self.tone(
                    f,
                    0.07,
                    Wave::Sine,
                    0.025 * m.birds as f32,
                    i as f32 * 0.09,
                    slide,
                    0.,
                    Dest::Outside,
                );
            }
            self.amb.next_chirp = self.t + 0.4 + self.rng.unit() as f64 * 2.2 / m.birds;
        }
    }

    /// Nachtleben (audio.js setBar): Stimmengewirr mit Silbenrhythmus, Lachen, Gläserklirren und gedämpfter
    /// Club-Bass (124 BPM, Kick und Bass auf der Offbeat), alles in Richtung der lautesten Quelle.
    fn set_bar(&mut self, m: &Mix) {
        let sr = self.sr;
        let crowd = m.bar as f32;
        let music = m.music as f32;
        let pan = (m.bar_pan * 0.7) as f32;
        self.amb.bar_pan.set(pan, 0.3, sr);
        if self.t >= self.amb.next_syllable {
            for (i, b) in self.amb.babble.iter_mut().enumerate() {
                let base = [0.07, 0.045, 0.02][i] * crowd;
                let g = base * (0.45 + self.rng.unit() * 0.9);
                let f = [480., 1150., 2500.][i] * (0.8 + self.rng.unit() * 0.45);
                b.gain.set(g, 0.03, sr);
                b.freq.set(f, 0.04, sr);
            }
            self.amb.next_syllable = self.t + 0.06 + self.rng.unit() as f64 * 0.05;
        }
        let n0 = self.shots.len();
        if crowd > 0.15 && self.t > self.amb.next_laugh {
            // Lachen: 3–6 „ha“ (Formant auf Rauschen), fallend
            let n = 3 + (self.rng.unit() * 4.) as usize;
            let f0 = 800. + self.rng.unit() * 600.;
            let mut at = 0.;
            for i in 0..n {
                let k = i as f32;
                self.burst(
                    0.09,
                    f0 * (1. - k * 0.05),
                    0.03 * crowd * (1. - k * 0.1),
                    FilterType::Bandpass,
                    7.,
                    at,
                    0.015,
                    Dest::Outside,
                );
                at += 0.12 + self.rng.unit() * 0.03;
            }
            self.amb.next_laugh = self.t + 1.2 + self.rng.unit() as f64 * 5. / crowd as f64;
        }
        if crowd > 0.1 && self.t > self.amb.next_clink {
            let f = 2800. + self.rng.unit() * 1600.;
            let g = 0.02 * crowd;
            self.tone(f, 0.25, Wave::Sine, g, 0., 0., 0., Dest::Outside);
            self.tone(
                f * 2.76,
                0.15,
                Wave::Sine,
                g * 0.5,
                0.,
                0.,
                0.,
                Dest::Outside,
            );
            self.amb.next_clink = self.t + 0.8 + self.rng.unit() as f64 * 4. / crowd as f64;
        }
        if music > 0.03 {
            let spb = 60. / 124.;
            if self.amb.next_beat < self.t {
                self.amb.next_beat = self.t + 0.05;
            }
            while self.amb.next_beat < self.t + 0.6 {
                let at = (self.amb.next_beat - self.t) as f32;
                // Kick: Sinus 130 → 44 Hz
                self.tone(
                    130.,
                    0.3,
                    Wave::Sine,
                    0.22 * music,
                    at,
                    -86.,
                    0.,
                    Dest::Outside,
                );
                if self.amb.beat.is_multiple_of(2) {
                    let f = [55., 55., 65.4, 49.][((self.amb.beat >> 3) % 4) as usize];
                    self.tone(
                        f,
                        spb as f32 * 0.45,
                        Wave::Saw,
                        0.05 * music,
                        at + spb as f32 / 2.,
                        0.,
                        170.,
                        Dest::Outside,
                    );
                }
                self.amb.next_beat += spb;
                self.amb.beat += 1;
            }
        }
        for s in &mut self.shots[n0..] {
            s.pan = pan;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn tone(
        &mut self,
        freq: f32,
        dur: f32,
        wave: Wave,
        gain: f32,
        at: f32,
        slide: f32,
        lowpass: f32,
        dest: Dest,
    ) {
        let end = if slide != 0. {
            (freq + slide).max(30.)
        } else {
            freq
        };
        self.shots.push(Shot {
            start: self.t + at as f64,
            env: Env {
                gain,
                attack: 0.01,
                dur,
            },
            dest,
            pan: 0.,
            tone: Some((Osc::new(wave, freq), freq, end)),
            noise: None,
            filter: (lowpass > 0.).then(|| Biquad::new(FilterType::Lowpass, lowpass, 0.)),
            rumble: None,
        });
    }
    #[allow(clippy::too_many_arguments)]
    fn burst(
        &mut self,
        dur: f32,
        freq: f32,
        gain: f32,
        kind: FilterType,
        q: f32,
        at: f32,
        attack: f32,
        dest: Dest,
    ) {
        let seed = (self.rng.unit() * 1e9) as u32 | 1;
        self.shots.push(Shot {
            start: self.t + at as f64,
            env: Env {
                gain,
                attack: attack.max(0.0005),
                dur,
            },
            dest,
            pan: 0.,
            tone: None,
            noise: Some(Noise::new(seed)),
            filter: Some(Biquad::new(kind, freq, q)),
            rumble: None,
        });
    }

    /// Klang-Sample abspielen: zufällige Variante und Tonhöhe, Pegel `k` (trägt schon die Entfernung),
    /// bei `distance` dunkler mit sinkendem `k`. Falsch, wenn es keine Aufnahme gibt bzw. Samples aus sind – dann
    /// spielt der Aufrufer den Synthese-Klang.
    fn sample(&mut self, sp: SfxSpec, k: f32, dest: Dest) -> bool {
        if !self.use_samples {
            return false;
        }
        let bank = sfx_bank(sp.name);
        if bank.is_empty() {
            return false;
        }
        if k <= 0. {
            return true;
        }
        let v = ((self.rng.unit() * bank.len() as f32) as usize).min(bank.len() - 1);
        let rate = 1. + (self.rng.unit() as f64 - 0.5) * 2. * sp.spread as f64;
        let lp_k = if sp.distance {
            let cutoff = 1200. + 16000. * k.clamp(0., 1.).powi(2);
            1. - (-std::f32::consts::TAU * cutoff / self.sr).exp()
        } else {
            1.
        };
        self.plays.push(SamplePlay {
            buf: bank[v].clone(),
            pos: 0.,
            rate: rate * 48000. / self.sr as f64,
            gain: sp.level * k,
            group: NO_CHOKE,
            age: 0.,
            release: false,
            lp_k,
            lp: 0.,
            dest,
        });
        if self.plays.len() > 48 {
            self.plays.remove(0);
        }
        true
    }

    /// Einzelklang abspielen (SYNTH in `audio.js`).
    pub fn play(&mut self, s: Sfx) {
        use Dest::Master as M;
        use FilterType::*;
        use Wave::*;
        match s {
            Sfx::Crash(k) => {
                self.burst(
                    0.35 + k * 0.3,
                    600. + k * 1800.,
                    0.25 + k * 0.4,
                    Lowpass,
                    0.7,
                    0.,
                    0.,
                    M,
                );
                self.tone(90., 0.25, Sine, 0.25 * k, 0., -50., 0., M);
                let mut i = 1.;
                while i < 2. + k * 3. {
                    let f = 1200. + self.rng.unit() * 1500.;
                    self.burst(0.08, f, 0.12 * k, Bandpass, 3., i * 0.05, 0., M);
                    i += 1.;
                }
                if k > 0.5 {
                    for _ in 0..6 {
                        let (f, at) = (
                            3500. + self.rng.unit() * 3500.,
                            0.08 + self.rng.unit() * 0.3,
                        );
                        self.tone(f, 0.12, Sine, 0.03 * k, at, 0., 0., M);
                    }
                }
            }
            Sfx::Hit if self.sample(HIT, 1., M) => {}
            Sfx::Hit => {
                self.burst(0.12, 400., 0.3, Lowpass, 0.7, 0., 0., M);
                self.tone(160., 0.12, Sine, 0.2, 0., -80., 0., M);
            }
            Sfx::Horn(k) => {
                for f in [415., 523.] {
                    self.tone(f, 0.45, Saw, 0.06 * k, 0., 0., 1800., M);
                }
            }
            Sfx::Knock(k) => {
                self.burst(0.1, 900., 0.25 * k, Lowpass, 0.7, 0., 0., M);
                for (f, g) in [(520., 0.06), (1340., 0.04), (2150., 0.03), (3470., 0.02)] {
                    self.tone(f, 0.6, Sine, g * k, 0., 0., 0., M);
                }
            }
            Sfx::Door => {
                self.burst(0.08, 1500., 0.25, Bandpass, 0.7, 0., 0., M);
                self.tone(120., 0.08, Sine, 0.2, 0.05, 0., 0., M);
            }
            Sfx::Ui if self.sample(UI, 1., M) => {}
            Sfx::Ui => self.tone(880., 0.09, Triangle, 0.12, 0., 0., 0., M),
            // Waffen (audio.js): Knall aus gefiltertem Rauschen plus tiefer Schlag
            // Waffen: echte Aufnahmen (Free Firearm Sound Library, CC0) – Variante und Tonhöhe gestreut, ferne
            // Schüsse dumpfer, ein neuer Schuss derselben Waffe blendet den Nachhall des vorigen aus
            Sfx::Gun(kind, k) => {
                let kind = kind.min(2);
                let bank = &weapon_bank()[kind as usize];
                if bank.is_empty() || k <= 0. {
                    return;
                }
                let v = ((self.rng.unit() * bank.len() as f32) as usize).min(bank.len() - 1);
                let rate = 1. + (self.rng.unit() as f64 - 0.5) * 0.06;
                for p in &mut self.plays {
                    if p.group == kind && p.age > GUN_CHOKE_AGE {
                        p.release = true;
                    }
                }
                // Tiefpass der Entfernung: nah offen, fern dumpf (k = Pegel nach Entfernung, 0…1)
                let cutoff = 1200. + 16000. * k.clamp(0., 1.).powi(2);
                let lp_k = 1. - (-std::f32::consts::TAU * cutoff / self.sr).exp();
                self.plays.push(SamplePlay {
                    buf: bank[v].clone(),
                    pos: 0.,
                    rate: rate * 48000. / self.sr as f64,
                    gain: GUN_LEVEL[kind as usize] * k,
                    group: kind,
                    age: 0.,
                    release: false,
                    lp_k,
                    lp: 0.,
                    dest: Dest::Master,
                });
                // nie mehr als ein paar Dutzend gleichzeitig (Dauerfeuer vieler Schützen)
                if self.plays.len() > 24 {
                    self.plays.remove(0);
                }
            }
            Sfx::Swing(k) if self.sample(SWING, k, M) => {}
            Sfx::Swing(k) => self.burst(0.12, 900., 0.12 * k, Bandpass, 0.7, 0., 0., M),
            Sfx::Punch(k) if self.sample(PUNCH, k, M) => {}
            Sfx::Punch(k) => {
                self.burst(0.08, 500., 0.35 * k, Lowpass, 0.7, 0., 0., M);
                self.tone(110., 0.1, Sine, 0.3 * k, 0., -50., 0., M);
            }
            Sfx::Thud(k) if self.sample(THUD, k, M) => {}
            Sfx::Thud(k) => {
                self.burst(0.1, 1200., 0.25 * k, Bandpass, 0.7, 0., 0., M);
                self.tone(90., 0.12, Triangle, 0.2 * k, 0., 0., 0., M);
            }
            Sfx::Impact(k) if self.sample(IMPACT, k, M) => {}
            Sfx::Impact(k) => self.burst(0.05, 3500., 0.12 * k, Highpass, 0.7, 0., 0., M),
            Sfx::GongOpen => {
                self.tone(659., 0.35, Sine, 0.1, 0., 0., 0., M);
                self.tone(880., 0.45, Sine, 0.1, 0.28, 0., 0., M);
            }
            Sfx::RailJoint(k) => {
                // „ta“: dumpfer Schlag (Rad fällt in die Lücke) und kurzes metallisches Klicken
                self.burst(0.09, 160., 0.16 * k, Lowpass, 1.2, 0., 0.002, M);
                self.burst(0.03, 1700., 0.035 * k, Bandpass, 2.5, 0., 0.001, M);
            }
            Sfx::AirHiss(k) => {
                self.burst(1.1, 3200., 0.045 * k, Highpass, 0.7, 0., 0.03, M);
                self.burst(0.35, 900., 0.03 * k, Bandpass, 0.8, 0., 0.005, M);
            }
            Sfx::DepartBeep(k) => {
                for i in 0..3 {
                    self.tone(1175., 0.16, Sine, 0.045 * k, i as f32 * 0.28, 0., 0., M);
                }
            }
            Sfx::GongClose => {
                self.tone(880., 0.3, Sine, 0.1, 0., 0., 0., M);
                self.tone(659., 0.4, Sine, 0.1, 0.24, 0., 0., M);
            }
            Sfx::Bells(n, k) => {
                for i in 0..n.min(12) {
                    for (f, g) in [
                        (196., 0.09),
                        (392., 0.05),
                        (470., 0.04),
                        (588., 0.03),
                        (784., 0.02),
                    ] {
                        self.tone(f, 3.2, Sine, g * k, i as f32 * 2.1, 0., 0., Dest::Outside);
                    }
                }
            }
            Sfx::TramBell(k) => {
                for at in [0., 0.22] {
                    self.tone(1568., 0.5, Sine, 0.08 * k, at, 0., 0., M);
                    self.tone(2350., 0.35, Sine, 0.04 * k, at, 0., 0., M);
                }
            }
            Sfx::Splash(k) => {
                self.burst(0.35, 700., 0.3 * k, Lowpass, 0.7, 0., 0., M);
                self.burst(0.18, 2200., 0.12 * k, Bandpass, 0.7, 0., 0., M);
            }
            Sfx::Reload if self.sample(RELOAD, 1., M) => {}
            Sfx::Reload => {
                self.tone(1400., 0.04, Sine, 0.06, 0., 0., 0., M);
                self.tone(900., 0.05, Sine, 0.06, 0.12, 0., 0., M);
            }
            Sfx::Reloaded if self.sample(RELOADED, 1., M) => {}
            Sfx::Reloaded => self.tone(1800., 0.04, Sine, 0.07, 0., 0., 0., M),
            Sfx::WeaponSwitch if self.sample(WEAPON_SWITCH, 1., M) => {}
            Sfx::WeaponSwitch => self.tone(1100., 0.04, Triangle, 0.07, 0., 0., 0., M),
            Sfx::Tick if self.sample(TICK, 1., M) => {}
            Sfx::Tick => self.tone(1200., 0.05, Square, 0.06, 0., 0., 0., M),
            Sfx::Pickup if self.sample(PICKUP, 1., M) => {}
            Sfx::Pickup => {
                for (i, f) in [523., 659., 784.].into_iter().enumerate() {
                    self.tone(f, 0.18, Triangle, 0.14, i as f32 * 0.09, 0., 0., M);
                }
            }
            Sfx::MissionStart if self.sample(MISSION_START, 1., M) => {}
            Sfx::MissionStart => {
                for (i, f) in [392., 523.].into_iter().enumerate() {
                    self.tone(f, 0.2, Triangle, 0.14, i as f32 * 0.12, 0., 0., M);
                }
            }
            Sfx::MissionSuccess if self.sample(MISSION_SUCCESS, 1., M) => {}
            Sfx::MissionSuccess => {
                for (i, f) in [523., 659., 784., 1047., 784., 1047.]
                    .into_iter()
                    .enumerate()
                {
                    self.tone(f, 0.22, Square, 0.09, i as f32 * 0.11, 0., 0., M);
                }
            }
            Sfx::MissionFail if self.sample(MISSION_FAIL, 1., M) => {}
            Sfx::MissionFail => {
                for (i, f) in [392., 330., 262., 196.].into_iter().enumerate() {
                    self.tone(f, 0.3, Saw, 0.08, i as f32 * 0.18, 0., 0., M);
                }
            }
            Sfx::Carjack => self.tone(700., 0.3, Saw, 0.05, 0., 400., 0., M),
            Sfx::Thunder(loud, near) => {
                let dur = if near { 5.5 } else { 7. + self.rng.unit() * 3. };
                if near {
                    self.burst(0.35, 3800., 0.5 * loud, Highpass, 0.7, 0., 0., M);
                    self.burst(0.8, 900., 0.45 * loud, Lowpass, 0.7, 0., 0., M);
                }
                // Grollen in Wellen (Echos an Häusern und Wolken)
                let mut pts = vec![(0f32, 0.0001f32)];
                let mut at = if near { 0.15 } else { 0.3 };
                for k in 0..5 {
                    let peak = 0.55 * loud * (1. - k as f32 * 0.15) * (0.6 + self.rng.unit() * 0.4);
                    pts.push((at + 0.25, peak));
                    at += 0.5 + self.rng.unit() * 0.9;
                    pts.push((at, peak * 0.35));
                }
                pts.push((dur, 0.0001));
                let seed = (self.rng.unit() * 1e9) as u32 | 1;
                self.shots.push(Shot {
                    start: self.t,
                    env: Env {
                        gain: 1.,
                        attack: 0.001,
                        dur,
                    },
                    dest: M,
                    pan: 0.,
                    tone: None,
                    noise: Some(Noise::new(seed)),
                    filter: Some(Biquad::new(Lowpass, if near { 420. } else { 160. }, 0.)),
                    rumble: Some((pts, if near { 420. } else { 160. }, 70.)),
                });
            }
            Sfx::Footstep(kind, k)
                if self.sample(
                    match kind {
                        Footstep::Hard => STEP_HARD,
                        Footstep::Grass => STEP_GRASS,
                        Footstep::Snow => STEP_SNOW,
                        Footstep::Wet => STEP_WET,
                    },
                    k,
                    M,
                ) => {}
            Sfx::Footstep(kind, k) => match kind {
                Footstep::Snow => {
                    for i in 0..4 {
                        let (d, f) = (
                            0.03 + self.rng.unit() * 0.03,
                            1100. + self.rng.unit() * 1800.,
                        );
                        self.burst(d, f, 0.07 * k, Bandpass, 1.5, i as f32 * 0.022, 0., M);
                    }
                }
                Footstep::Wet => {
                    self.burst(0.09, 900., 0.08 * k, Lowpass, 0.7, 0., 0., M);
                    self.burst(0.05, 3200., 0.03 * k, Bandpass, 2., 0.02, 0., M);
                }
                Footstep::Grass => self.burst(0.07, 500., 0.07 * k, Lowpass, 0.7, 0., 0., M),
                Footstep::Hard => {
                    self.burst(0.03, 2200., 0.05 * k, Bandpass, 1.2, 0., 0., M);
                    self.tone(95., 0.05, Sine, 0.06 * k, 0., 0., 0., M);
                }
            },
        }
    }

    /// Stereo-Abtastwerte erzeugen (verschachtelt links/rechts).
    pub fn render(&mut self, out: &mut [f32]) {
        let sr = self.sr;
        let dt = 1. / sr as f64;
        for frame in out.chunks_mut(2) {
            let block = self.tick.is_multiple_of(BLOCK);
            self.tick = self.tick.wrapping_add(1);
            // --- eigenes Fahrzeug
            let e = &mut self.engine;
            let o1 = e.o1.next(sr, 0.);
            let o2 = e.o2.next(sr, 0.);
            let o3 = e.o3.next(sr, 0.);
            let (g1, g2, g3) = (e.g1.tick(), e.g2.tick(), e.g3.tick());
            let (lf, bf, bg, sg) = (
                e.lp_f.tick(),
                e.body_f.tick(),
                e.body_g.tick(),
                e.bass_g.tick(),
            );
            if block {
                e.lp.set_freq(lf);
                e.body.set(bf, 0.65, bg);
                e.bass.set(220., 0., sg);
            }
            let sat = shape(g1 * o1 + g3 * o3, 1.45);
            let body = e.body.process(e.lp.process(sat, sr) + g2 * o2, sr);
            let mut eng = e.bass.process(body, sr) * e.g.tick();
            let am = e.am.next(sr, 0.);
            // Auspuff und Diesel-Nageln: Rauschen, dessen Pegel im Zündtakt pulsiert (moduliertes Gain)
            let ex = e.ex.filtered(sr, block) * (e.ex.gain.tick() + e.amg.tick() * am).max(0.);
            let cl =
                e.clatter.filtered(sr, block) * (e.clatter.gain.tick() + e.clm.tick() * am).max(0.);
            eng += ex + cl;
            eng += e.whine.next(sr, 0.) * e.whg.tick();
            eng += e.intake.next(sr, block) + e.turbo_noise.next(sr, block);
            eng += e.turbo.next(sr, 0.) * e.turbo_g.tick();
            eng += e.reverse.next(sr, 0.) * e.reverse_g.tick();
            for l in [
                &mut e.roll,
                &mut e.cobble,
                &mut e.wet,
                &mut e.snow,
                &mut e.slide,
                &mut e.wind,
                &mut e.squeal,
                &mut e.roof,
                &mut e.roof_low,
            ] {
                eng += l.next(sr, block);
            }
            let vib = e.vib.next(sr, 0.) * 25.;
            eng += (e.sq[0].next(sr, vib) + e.sq[1].next(sr, vib)) * e.sqg.tick();
            eng *= e.bus.tick();
            // --- Motoren aus Aufnahmen (Kanal „engine“): das eigene Auto direkt, fremde draußen mit Panorama
            let (mut ol, mut or) = (0f32, 0f32);
            let ech = self.engine_ch.tick() * SAMPLE_LEVEL;
            for v in &mut self.samplers {
                if v.silent() {
                    continue;
                }
                let x = v.next(sr, block) * ech;
                let p = v.pan();
                if v.player {
                    eng += x;
                } else {
                    let (pl, pr) = pan(p);
                    ol += x * pl;
                    or += x * pr;
                }
            }
            if let Some((r, pos)) = &mut self.reference {
                // A/B: Referenz auf den Pegel der Loops gebracht (−12 → −18 LUFS)
                let i = *pos as usize % r.len();
                eng += r[i] * 0.5 * SAMPLE_LEVEL;
                *pos += (48000. / sr) as f64;
                if *pos >= r.len() as f64 {
                    *pos -= r.len() as f64;
                }
            }
            // --- draußen: fremde Autos (Panorama), Umgebung
            for s in &mut self.cars {
                let ff = s.f_f.tick();
                if block {
                    s.f.set_freq(ff);
                }
                let src = s.o.next(sr, 0.) + s.o2.next(sr, 0.) * s.og2.tick();
                let v = s.f.process(src, sr) * s.g.tick() + s.tire.next(sr, block);
                let (pl, pr) = pan(s.pan.tick());
                ol += v * pl;
                or += v * pr;
            }
            let a = &mut self.amb;
            let amb = a.hum.next(sr, block)
                + a.traffic.next(sr, block)
                + a.water.next(sr, block)
                + a.rain.next(sr, block)
                + a.rain_low.next(sr, block)
                + a.wind.next(sr, block)
                + a.whistle.next(sr, block)
                + a.rumble.next(sr, block)
                + a.siren.next(sr, 0.) * a.siren_g.tick();
            let c = std::f32::consts::FRAC_1_SQRT_2;
            ol += amb * c;
            or += amb * c;
            let bab: f32 = a.babble.iter_mut().map(|b| b.next(sr, block)).sum();
            let (bl, br) = pan(a.bar_pan.tick());
            ol += bab * bl;
            or += bab * br;
            // --- S-/U-Bahn: eigener Zug mittig, Zug am Bahnsteig aus seiner Richtung (beide direkt, nicht gedämpft)
            let ride = self.ride.next(sr, block);
            let pv = self.pass.next(sr, block);
            let (pl, pr) = pan(self.pass.pan.tick());
            // --- Einzelklänge
            let (mut ml, mut mr) = (eng + ride + pv * pl, eng + ride + pv * pr);
            let t = self.t;
            for s in &mut self.shots {
                let lt = (t - s.start) as f32;
                if lt < 0. {
                    continue;
                }
                let mut v = match (&mut s.tone, &mut s.noise) {
                    (Some((osc, f0, f1)), _) => {
                        let u = (lt / s.env.dur).min(1.);
                        osc.freq.value = *f0 * (*f1 / *f0).powf(u);
                        osc.freq.target = osc.freq.value;
                        osc.next(sr, 0.)
                    }
                    (None, Some(n)) => n.tick(),
                    _ => 0.,
                };
                let mut gain = s.env.at(lt);
                if let Some((pts, f0, f1)) = &s.rumble {
                    let u = (lt / s.env.dur).min(1.);
                    if let Some(f) = &mut s.filter
                        && block
                    {
                        f.set_freq(*f0 * (*f1 / *f0).powf(u));
                    }
                    gain = envelope(pts, lt);
                }
                if let Some(f) = &mut s.filter {
                    v = f.process(v, sr);
                }
                v *= gain;
                let (pl, pr) = pan(s.pan);
                let (l, r) = (
                    v * pl * std::f32::consts::SQRT_2,
                    v * pr * std::f32::consts::SQRT_2,
                );
                match s.dest {
                    Dest::Outside => {
                        ol += l;
                        or += r;
                    }
                    Dest::Master | Dest::Engine => {
                        ml += l;
                        mr += r;
                    }
                }
            }
            // Schuss-Aufnahmen (mittig; der Pegel trägt schon die Entfernung)
            let choke = (-1. / (GUN_CHOKE_TC * sr)).exp();
            for p in &mut self.plays {
                let i = p.pos as usize;
                if i + 1 >= p.buf.len() {
                    continue;
                }
                let t = (p.pos - i as f64) as f32;
                let x = p.buf[i] * (1. - t) + p.buf[i + 1] * t;
                p.lp += (x - p.lp) * p.lp_k;
                if p.release {
                    p.gain *= choke;
                }
                let v = p.lp * p.gain;
                if p.dest == Dest::Outside {
                    ol += v;
                    or += v;
                } else {
                    ml += v;
                    mr += v;
                }
                p.pos += p.rate;
                p.age += dt;
            }
            if block {
                self.plays
                    .retain(|p| (p.pos as usize) + 1 < p.buf.len() && p.gain > 1e-4);
            }
            let mf = self.muffle_f.tick();
            if block {
                self.muffle[0].set_freq(mf);
                self.muffle[1].set_freq(mf);
            }
            ml += self.muffle[0].process(ol, sr);
            mr += self.muffle[1].process(or, sr);
            // Nachhall in der Bahnhofshalle bzw. im Tunnel (läuft immer, damit Nachklänge nicht abreißen)
            let (wl, wr) = self.hall.process((ml + mr) * 0.5 * self.hall_g.tick());
            ml += wl * 0.8;
            mr += wr * 0.8;
            let g = self.master.tick();
            let (l, r) = self.comp.process(ml * g, mr * g, sr);
            frame[0] = l.clamp(-1., 1.);
            if frame.len() > 1 {
                frame[1] = r.clamp(-1., 1.);
            }
            self.t += dt;
        }
        let t = self.t;
        self.shots
            .retain(|s| (t - s.start) < s.env.dur as f64 + 0.05);
    }
}

/// Stückweise lineare Hüllkurve (Web Audio `linearRampToValueAtTime`).
fn envelope(pts: &[(f32, f32)], t: f32) -> f32 {
    for w in pts.windows(2) {
        let ((t0, a), (t1, b)) = (w[0], w[1]);
        if t <= t1 {
            let u = if t1 > t0 {
                ((t - t0) / (t1 - t0)).clamp(0., 1.)
            } else {
                1.
            };
            return a + (b - a) * u;
        }
    }
    0.
}

#[cfg(test)]
mod tests {
    /// Schüsse aus Aufnahmen: kräftiger als der frühere Synthese-Knall (Spitze 0,12–0,26, Effektivwert der ersten
    /// 0,3 s 0,012–0,044), aber ohne Übersteuern; mit hörbarem Nachhall statt Stille nach 0,3 s.
    /// Lautheit eines Einzelklangs: Mittel über mehrere Auslösungen des lautesten 50-ms-Fensters (Effektivwert),
    /// dazu die Spitze.
    fn sfx_loudness(sfx: Sfx, samples: bool) -> (f32, f32) {
        let mut s = Synth::new(48000.);
        s.use_samples = samples;
        let (mut sum, mut peak) = (0., 0f32);
        for _ in 0..12 {
            s.play(sfx);
            let out = render(&mut s, 0.8);
            let m: Vec<f32> = out.chunks(2).map(|c| (c[0] + c[1]) / 2.).collect();
            let w = 2400;
            let best = m
                .chunks(w / 2)
                .enumerate()
                .map(|(i, _)| {
                    let a = i * w / 2;
                    let b = (a + w).min(m.len());
                    (m[a..b].iter().map(|x| x * x).sum::<f32>() / (b - a) as f32).sqrt()
                })
                .fold(0f32, f32::max);
            sum += best;
            peak = m.iter().fold(peak, |p, x| p.max(x.abs()));
        }
        (sum / 12., peak)
    }

    /// Klänge aus Aufnahmen sind etwa so laut wie die frühere Synthese (nicht leiser, höchstens deutlich lauter,
    /// wo der Synthese-Klang kaum hörbar war) und übersteuern nicht.
    #[test]
    fn sfx_samples_match_the_synth_loudness() {
        use Footstep as F;
        // (Klang, Name, Ziel: Effektivwert des lautesten 50-ms-Fensters)
        let cases = [
            (Sfx::Footstep(F::Hard, 1.), "step_hard", 0.010),
            (Sfx::Footstep(F::Grass, 1.), "step_grass", 0.007),
            (Sfx::Footstep(F::Snow, 1.), "step_snow", 0.008),
            (Sfx::Footstep(F::Wet, 1.), "step_wet", 0.009),
            (Sfx::Swing(1.), "swing", 0.015),
            (Sfx::Punch(1.), "punch", 0.06),
            (Sfx::Thud(1.), "thud", 0.037),
            (Sfx::Impact(1.), "impact", 0.0125),
            (Sfx::Hit, "hit", 0.043),
            (Sfx::Reload, "reload", 0.01),
            (Sfx::Reloaded, "reloaded", 0.011),
            (Sfx::WeaponSwitch, "weapon_switch", 0.009),
            (Sfx::Ui, "ui", 0.0193),
            (Sfx::Tick, "tick", 0.0139),
            (Sfx::Pickup, "pickup", 0.0289),
            (Sfx::MissionStart, "mission_start", 0.03),
            (Sfx::MissionSuccess, "mission_success", 0.035),
            (Sfx::MissionFail, "mission_fail", 0.03),
        ];
        let mut bad = Vec::new();
        for (sfx, name, target) in cases {
            assert!(!sfx_bank(name).is_empty(), "keine Aufnahme für {name}");
            let (syn, _) = sfx_loudness(sfx, false);
            let (smp, peak) = sfx_loudness(sfx, true);
            println!(
                "SFX {name:14} synth {syn:.4} sample {smp:.4} ziel {target:.4} peak {peak:.3}"
            );
            // nah am Ziel, nie leiser als die Synthese, keine Übersteuerung
            if !(0.8..=1.25).contains(&(smp / target)) || smp < syn * 0.9 || peak > 0.95 {
                bad.push(format!(
                    "{name}: {smp:.4} statt {target} (Synthese {syn:.4}, Spitze {peak:.2})"
                ));
            }
        }
        assert!(bad.is_empty(), "Pegel daneben: {bad:?}");
    }

    #[test]
    fn gun_samples_are_loud_but_not_clipping() {
        for k in 0..3u8 {
            let mut s = Synth::new(48000.);
            s.play(Sfx::Gun(k, 1.));
            let mut out = vec![0f32; 48000 * 2];
            s.render(&mut out);
            let m: Vec<f32> = out.chunks(2).map(|c| (c[0] + c[1]) / 2.).collect();
            let rms = |a: usize, b: usize| {
                (m[a..b].iter().map(|x| x * x).sum::<f32>() / (b - a) as f32).sqrt()
            };
            let peak = m.iter().fold(0f32, |p, x| p.max(x.abs()));
            println!(
                "GUN {k} peak {peak:.3} rms {:.4} tail {:.5}",
                rms(0, 14400),
                rms(14400, 48000)
            );
            assert!((0.2..0.95).contains(&peak), "Waffe {k}: Spitze {peak}");
            // mindestens so kräftig wie der frühere Synthese-Schuss (Effektivwert der ersten 0,3 s)
            let before = [0.0254, 0.0123, 0.0435][k as usize];
            assert!(rms(0, 14400) >= before * 0.95, "Waffe {k}: zu leise");
            assert!(rms(14400, 48000) > 0.001, "Waffe {k}: kein Nachhall");
        }
    }

    /// Dauerfeuer: der Nachhall des vorigen Schusses wird ausgeblendet (sonst stapeln sich 13 Hallfahnen je
    /// Sekunde); ein ferner Schuss klingt dumpfer als ein naher.
    #[test]
    fn rapid_fire_chokes_tails_and_distance_darkens() {
        let mut s = Synth::new(48000.);
        let mut chunk = vec![0f32; 3600 * 2];
        for _ in 0..10 {
            s.play(Sfx::Gun(1, 1.));
            s.render(&mut chunk);
        }
        let alive = s.plays.iter().filter(|p| !p.release).count();
        assert_eq!(alive, 1, "nur der letzte Schuss klingt voll aus");
        // Helligkeit: Energie der ersten Differenz im Verhältnis zur Energie
        let bright = |k: f32| {
            let mut s = Synth::new(48000.);
            s.play(Sfx::Gun(0, k));
            let mut out = vec![0f32; 24000 * 2];
            s.render(&mut out);
            let m: Vec<f32> = out.chunks(2).map(|c| c[0]).collect();
            let e0: f32 = m.iter().map(|x| x * x).sum();
            let e1: f32 = m.windows(2).map(|w| (w[1] - w[0]).powi(2)).sum();
            (e1 / e0.max(1e-12)).sqrt()
        };
        assert!(bright(1.) > bright(0.2) * 1.5, "fern nicht dumpfer");
    }
    use super::*;
    use berlin_sim::enginevoice::voice_for;
    const SR: f32 = 48000.;

    fn render(s: &mut Synth, secs: f32) -> Vec<f32> {
        let mut out = vec![0.; (secs * SR) as usize * 2];
        s.render(&mut out);
        out
    }
    fn channel(x: &[f32], ch: usize) -> Vec<f32> {
        x.iter().skip(ch).step_by(2).copied().collect()
    }
    fn goertzel(x: &[f32], f: f32) -> f32 {
        let w = std::f32::consts::TAU * f / SR;
        let c = 2. * w.cos();
        let (mut s1, mut s2) = (0f32, 0f32);
        for &v in x {
            let s = v + c * s1 - s2;
            s2 = s1;
            s1 = s;
        }
        (s1 * s1 + s2 * s2 - c * s1 * s2).max(0.).sqrt() / x.len() as f32
    }
    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|v| v * v).sum::<f32>() / x.len().max(1) as f32).sqrt()
    }
    fn engine(rpm: f64, load: f64, model: &str, cyl: u32) -> EngineState {
        EngineState {
            rpm,
            cycle: rpm / 120.,
            fire: rpm / 60. * cyl as f64 / 2.,
            norm: 0.4,
            load,
            cyl,
            voice: voice_for(model, false, false),
            ..Default::default()
        }
    }

    #[test]
    fn silent_without_input_and_always_bounded() {
        let mut s = Synth::new(SR);
        s.apply(&Frame::default());
        let x = render(&mut s, 0.5);
        assert!(rms(&x) < 1e-4, "{}", rms(&x));
        s.apply(&Frame {
            sfx: vec![Sfx::Crash(1.), Sfx::Horn(1.), Sfx::MissionSuccess],
            ..Default::default()
        });
        let x = render(&mut s, 1.5);
        assert!(rms(&x) > 0.01 && x.iter().all(|v| v.is_finite() && v.abs() <= 1.));
        render(&mut s, 2.);
        assert_eq!(s.active_shots(), 0, "Einzelklänge laufen aus");
    }

    #[test]
    fn engine_sounds_on_its_firing_frequency() {
        let mut s = Synth::new(SR);
        // Sechszylinder bei 3000 U/min: Zündfrequenz 150 Hz, Arbeitszyklus 25 Hz
        let veh = Vehicle {
            active: true,
            in_car: true,
            engine: Some(engine(3000., 0.6, "limousine", 6)),
            tires: Some(Tires::default()),
            rain: 0.,
        };
        s.apply(&Frame {
            vehicle: veh,
            ..Default::default()
        });
        render(&mut s, 0.5);
        let x = channel(&render(&mut s, 1.), 0);
        assert!(rms(&x) > 0.005, "Motor hörbar: {}", rms(&x));
        let on = goertzel(&x, 150.);
        let off = goertzel(&x, 137.5);
        assert!(on > off * 4., "Zündton {on} gegen daneben {off}");
        // höhere Drehzahl → Zündton wandert mit
        s.apply(&Frame {
            vehicle: Vehicle {
                engine: Some(engine(5000., 0.6, "limousine", 6)),
                ..veh
            },
            ..Default::default()
        });
        render(&mut s, 0.5);
        let y = channel(&render(&mut s, 1.), 0);
        assert!(goertzel(&y, 250.) > goertzel(&y, 150.) * 2.);
        // aussteigen: der Bus verstummt
        s.apply(&Frame::default());
        render(&mut s, 2.);
        assert!(rms(&render(&mut s, 0.5)) < 1e-3);
    }

    #[test]
    fn traffic_voices_pan_and_doppler() {
        let mut s = Synth::new(SR);
        let mk = |pan: f64, rate: f64| CarVoice {
            id: 7,
            d: 50.,
            gain: 1.,
            pan,
            rate,
            engine: engine(2400., 0.5, "kompakt", 4),
            tire: 0.3,
        };
        s.apply(&Frame {
            voices: vec![mk(-1., 1.)],
            ..Default::default()
        });
        render(&mut s, 0.6);
        let x = render(&mut s, 0.5);
        let (l, r) = (rms(&channel(&x, 0)), rms(&channel(&x, 1)));
        assert!(l > 0.002 && l > r * 5., "links {l} rechts {r}");
        // Doppler: kommt näher → höher (Zündton 80 Hz · 1,25)
        s.apply(&Frame {
            voices: vec![mk(0., 1.25)],
            ..Default::default()
        });
        render(&mut s, 1.);
        let y = channel(&render(&mut s, 1.), 0);
        assert!(goertzel(&y, 100.) > goertzel(&y, 80.) * 2.);
    }

    #[test]
    fn car_body_muffles_the_city() {
        let amb = Mix {
            traffic: 1.,
            hum: 1.,
            ..Default::default()
        };
        let hi = |muffle: f64| {
            let mut s = Synth::new(SR);
            s.apply(&Frame {
                ambience: Mix { muffle, ..amb },
                ..Default::default()
            });
            render(&mut s, 3.);
            let x = channel(&render(&mut s, 1.), 0);
            // Energie oberhalb von ~4 kHz (die Karosserie setzt die Eckfrequenz bei Dämpfung 0,65 auf ≈ 2,2 kHz)
            let mut hp = crate::dsp::Biquad::new(FilterType::Highpass, 4000., 0.);
            let y: Vec<f32> = x.iter().map(|&v| hp.process(v, SR)).collect();
            (rms(&x), rms(&y))
        };
        let (open_all, open_hi) = hi(0.);
        let (car_all, car_hi) = hi(0.65);
        assert!(open_all > 0.001 && car_all > 0.0005);
        assert!(
            car_hi < open_hi * 0.5,
            "Höhen im Auto gedämpft: {car_hi} gegen {open_hi}"
        );
    }
}
