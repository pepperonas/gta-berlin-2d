//! Motorsound aus Aufnahmen (docs/audio.md): reine Steuerlogik, kein Audio. Aus Drehzahl, Gas, Gang, Tempo und
//! Schlupf eines Fahrzeugs wird je Bild bestimmt, welche Loops der Sample-Bank mit welcher Lautstärke und Tonhöhe
//! spielen und welche Einzelklänge (Start, Schalten, Gasstoß, Pops) ausgelöst werden. Abgespielt wird in
//! `berlin-audio` (`sampler.rs`).
//!
//! - **Daten:** Bank = `data/audio/engine/<bank>/manifest.json` (vom Build-Skript), Profile =
//!   `data/audio/engine_profiles.json` (Presets je Fahrzeugklasse, Überschreibungen je Fahrzeug, Mischpult).
//! - **Drehzahl:** kommt aus der Fahrphysik, wenn das Fahrzeug sie rechnet; sonst aus einem virtuellen Getriebe
//!   (Gangstufen als Tempo am Begrenzer). Beides ist reine Klanglogik und ändert die Fahrphysik nicht.
//! - **Überblendung:** gleiche Leistung zwischen den zwei Loops, die die Drehzahl (logarithmisch) einrahmen;
//!   Tonhöhe = Drehzahl / Aufnahmedrehzahl, begrenzt auf 0,7 … 1,4. Gas mischt Last- und Schub-Loops.
//! - **Zufall** (Pops) nur aus Hashes von Fahrzeug und Zähler, nie aus dem Welt-Zufall.
use crate::math::hash01;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::f64::consts::FRAC_PI_2;

const PROFILES: &str = include_str!("../../../data/audio/engine_profiles.json");
const BANK_V10: &str = include_str!("../../../data/audio/engine/v10/manifest.json");
const BANK_V12: &str = include_str!("../../../data/audio/engine/v12/manifest.json");
const BANK_R4: &str = include_str!("../../../data/audio/engine/r4/manifest.json");
const BANK_D4: &str = include_str!("../../../data/audio/engine/d4/manifest.json");
const BANK_D6: &str = include_str!("../../../data/audio/engine/d6/manifest.json");

/// Tonhöhe eines Loops: tiefer klingt verwaschen, höher nach Spielzeug.
pub const PITCH_MIN: f64 = 0.7;
pub const PITCH_MAX: f64 = 1.4;
/// Schaltpause: so lange ist das Gas beim Hochschalten weg (s)
pub const SHIFT_CUT: f64 = 0.14;
/// Pops frühestens wieder nach (s)
const POP_COOLDOWN: f64 = 0.7;
/// Gasstoß beim Runterschalten frühestens wieder nach (s)
const BLIP_COOLDOWN: f64 = 0.5;

#[derive(Debug, Clone, Deserialize)]
pub struct Mix {
    /// Pegel des Mischpult-Kanals „engine“
    pub engine: f64,
    /// höchstens so viele Sample-Motoren gleichzeitig (Spielerauto eingeschlossen)
    pub stimmen: usize,
    pub lod_px: f64,
    pub hoerweite_px: f64,
    pub doppler: f64,
    pub tiefpass_nah_hz: f64,
    pub tiefpass_fern_hz: f64,
}

/// Klangprofil eines Fahrzeugs (Preset plus Überschreibungen).
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Profile {
    #[serde(default)]
    pub name: String,
    pub bank: String,
    pub leerlauf: f64,
    pub begrenzer: f64,
    /// Tempo (km/h) je Gang am Begrenzer – das virtuelle Getriebe
    pub gaenge: Vec<f64>,
    /// Hochschalten bei diesem Anteil der Begrenzerdrehzahl (Vollgas) bzw. bei wenig Gas
    pub schalt_hoch: f64,
    pub schalt_hoch_teillast: f64,
    pub schalt_runter: f64,
    /// Grundverstimmung (multipliziert die Tonhöhe aller Loops)
    pub pitch: f64,
    pub tiefpass_hz: f64,
    pub low_shelf_db: f64,
    pub high_shelf_db: f64,
    /// 0 … 1: weiche Sättigung
    pub saettigung: f64,
    pub pop_chance: f64,
    pub pop_gain: f64,
    pub lautstaerke: f64,
    pub drehzahl_glaettung_s: f64,
    pub gas_glaettung_s: f64,
    pub begrenzer_hz: f64,
    pub start: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShotKind {
    Start,
    Blip,
    Shift,
    Pop,
}

#[derive(Debug, Clone)]
pub struct LoopInfo {
    pub file: String,
    pub rpm: f64,
    pub on: bool,
    pub gain: f64,
    pub samples: usize,
}
#[derive(Debug, Clone)]
pub struct ShotInfo {
    pub file: String,
    pub kind: ShotKind,
    pub gain: f64,
}

/// Sample-Bank: Loops (Last und Schub je Drehzahl) und Einzelklänge.
#[derive(Debug, Clone)]
pub struct Bank {
    pub name: String,
    pub samplerate: u32,
    pub loops: Vec<LoopInfo>,
    pub shots: Vec<ShotInfo>,
    /// Indizes der Last- bzw. Schub-Loops, je nach Drehzahl aufsteigend (die Drehzahlen beider Listen dürfen
    /// verschieden sein: echte Schub-Loops stammen aus dem Ausrollen, nicht aus denselben Stellen wie die Last-Loops)
    pub on: Vec<usize>,
    pub off: Vec<usize>,
    pub reference: String,
}
impl Bank {
    fn parse(j: &Value) -> Self {
        let db = |v: &Value| 10f64.powf(v["gain_db"].as_f64().unwrap_or(0.) / 20.);
        let loops: Vec<LoopInfo> = j["loops"]
            .as_array()
            .expect("loops")
            .iter()
            .map(|l| LoopInfo {
                file: l["datei"].as_str().expect("datei").into(),
                rpm: l["rpm"].as_f64().expect("rpm"),
                on: l["last"] == "on",
                gain: db(l),
                samples: l["samples"].as_u64().expect("samples") as usize,
            })
            .collect();
        let shots = j["einzel"]
            .as_array()
            .expect("einzel")
            .iter()
            .map(|s| ShotInfo {
                file: s["datei"].as_str().expect("datei").into(),
                kind: match s["art"].as_str().unwrap_or("") {
                    "start" => ShotKind::Start,
                    "blip" => ShotKind::Blip,
                    "schalten" => ShotKind::Shift,
                    _ => ShotKind::Pop,
                },
                gain: db(s),
            })
            .collect();
        let sorted = |on: bool| {
            let mut v: Vec<usize> = (0..loops.len()).filter(|&i| loops[i].on == on).collect();
            v.sort_by(|a, b| loops[*a].rpm.total_cmp(&loops[*b].rpm));
            v
        };
        let (on, off) = (sorted(true), sorted(false));
        Self {
            name: j["bank"].as_str().unwrap_or("").into(),
            samplerate: j["samplerate"].as_u64().unwrap_or(48000) as u32,
            reference: j["referenz"].as_str().unwrap_or("").into(),
            loops,
            shots,
            on,
            off,
        }
    }
    pub fn shots_of(&self, kind: ShotKind) -> Vec<usize> {
        (0..self.shots.len())
            .filter(|&i| self.shots[i].kind == kind)
            .collect()
    }
}

pub struct Config {
    pub mix: Mix,
    /// Presets nach Name und aufgelöste Profile je Fahrzeug-id (nur Fahrzeuge mit Überschreibung)
    pub presets: Vec<Profile>,
    vehicles: HashMap<String, Profile>,
    classes: HashMap<String, String>,
    types: HashMap<String, Option<String>>,
    excluded: Vec<String>,
    banks: Vec<Bank>,
}

fn merge(base: &Value, over: &Value) -> Value {
    let mut out = base.clone();
    if let (Some(o), Some(m)) = (out.as_object_mut(), over.as_object()) {
        for (k, v) in m {
            if !k.starts_with('_') && k != "preset" {
                o.insert(k.clone(), v.clone());
            }
        }
    }
    out
}

impl Config {
    fn parse(profiles: &str, banks: &[&str]) -> Self {
        let j: Value = serde_json::from_str(profiles).expect("engine_profiles.json");
        let mix: Mix = serde_json::from_value(j["mix"].clone()).expect("mix");
        let mut presets = Vec::new();
        let mut by_name = HashMap::new();
        for (k, v) in j["presets"].as_object().expect("presets") {
            let mut p: Profile = serde_json::from_value(v.clone()).expect("preset");
            p.name = k.clone();
            by_name.insert(k.clone(), v.clone());
            presets.push(p);
        }
        presets.sort_by_key(|p| match p.name.as_str() {
            "sport" => 0,
            "supercar" => 1,
            "hypercar" => 2,
            "sport4" => 3,
            "kompakt" => 4,
            "diesel" => 5,
            "lkw" => 6,
            _ => 7,
        });
        let classes: HashMap<String, String> = j["zuordnung"]["klassen"]
            .as_object()
            .expect("klassen")
            .iter()
            .map(|(k, v)| (k.clone(), v.as_str().expect("preset").into()))
            .collect();
        // Motortyp → Preset (null = Synthese); Vorrang vor der Klasse
        let types: HashMap<String, Option<String>> = j["zuordnung"]["typen"]
            .as_object()
            .map(|o| {
                o.iter()
                    .filter(|(k, _)| !k.starts_with('_'))
                    .map(|(k, v)| (k.clone(), v.as_str().map(str::to_owned)))
                    .collect()
            })
            .unwrap_or_default();
        let excluded = j["zuordnung"]["ausgenommen"]
            .as_array()
            .expect("ausgenommen")
            .iter()
            .map(|v| v.as_str().expect("id").into())
            .collect();
        let mut vehicles = HashMap::new();
        for (id, over) in j["fahrzeuge"].as_object().expect("fahrzeuge") {
            if id.starts_with('_') {
                continue;
            }
            let preset = over["preset"].as_str().map(str::to_owned).or_else(|| {
                crate::vehdata::game_vehicle(id).and_then(|v| classes.get(&v.class).cloned())
            });
            let Some(preset) = preset else {
                continue;
            };
            let mut p: Profile = serde_json::from_value(merge(&by_name[&preset], over))
                .unwrap_or_else(|e| panic!("Profil {id}: {e}"));
            p.name = format!("{preset}/{id}");
            vehicles.insert(id.clone(), p);
        }
        let banks = banks
            .iter()
            .map(|b| Bank::parse(&serde_json::from_str(b).expect("manifest.json")))
            .collect();
        Self {
            mix,
            presets,
            vehicles,
            classes,
            types,
            excluded,
            banks,
        }
    }
    pub fn preset(&self, name: &str) -> Option<&Profile> {
        self.presets.iter().find(|p| p.name == name)
    }
    pub fn bank(&self, name: &str) -> &Bank {
        self.banks
            .iter()
            .find(|b| b.name == name)
            .unwrap_or(&self.banks[0])
    }
    pub fn banks(&self) -> &[Bank] {
        &self.banks
    }
    /// Profil eines Fahrzeugdatensatzes: Überschreibung je id, sonst Motortyp, sonst Klasse; Elektro und
    /// Ausnahmen: keins.
    pub fn profile_for_vehicle(&self, v: &crate::vehdata::Vehicle) -> Option<&Profile> {
        if self.excluded.contains(&v.id) || v.engine.kind.contains("elektro") {
            return None;
        }
        if let Some(p) = self.vehicles.get(&v.id) {
            return Some(p);
        }
        if let Some(t) = self.types.get(&v.engine.kind) {
            return t.as_deref().and_then(|p| self.preset(p));
        }
        self.classes.get(&v.class).and_then(|p| self.preset(p))
    }
    /// Alle Profile für die Auswahl im Debug-Panel: Presets, dann Fahrzeuge mit Überschreibung (sortiert).
    pub fn all_profiles(&self) -> Vec<&Profile> {
        let mut v: Vec<&Profile> = self.vehicles.values().collect();
        v.sort_by(|a, b| a.name.cmp(&b.name));
        self.presets.iter().chain(v).collect()
    }
}

pub fn config() -> &'static Config {
    static C: std::sync::OnceLock<Config> = std::sync::OnceLock::new();
    C.get_or_init(|| Config::parse(PROFILES, &[BANK_V10, BANK_V12, BANK_R4, BANK_D4, BANK_D6]))
}

/// Motor-Samples an? `GTA_ENGINE_SAMPLES=0` schaltet zum Gegenhören auf den Synthese-Klang zurück.
pub fn samples_enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("GTA_ENGINE_SAMPLES").map_or(true, |v| v != "0"))
}

/// Klangprofil eines Autos im Spiel (über seinen Fahrzeugdatensatz) oder `None` (Synthese-Klang).
pub fn profile_for(car: &crate::car::Car) -> Option<&'static Profile> {
    if !samples_enabled() {
        return None;
    }
    crate::car::vphys_vehicle(car).and_then(|v| config().profile_for_vehicle(v))
}

/// Eingaben je Bild.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SoundInput {
    /// Drehzahl und Gang aus der Fahrphysik, falls das Fahrzeug sie rechnet
    pub rpm: Option<f64>,
    pub gear: Option<usize>,
    /// Begrenzerdrehzahl aus den Fahrzeugdaten (sonst die des Profils)
    pub limiter: Option<f64>,
    pub throttle: f64,
    /// Tempo (m/s)
    pub speed: f64,
    /// Schlupf/Drift 0 … 1: Drehzahl schießt über das Rad hinaus
    pub slip: f64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Layer {
    /// Index in `Bank::loops`
    pub idx: usize,
    pub gain: f32,
    pub pitch: f32,
}

/// Klangfärbung des Profils.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Tone {
    pub lowpass: f32,
    pub low_db: f32,
    pub high_db: f32,
    pub drive: f32,
}

/// Was der Sampler je Bild spielt.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SoundOut {
    /// Sample-Bank, aus der die Indizes stammen
    pub bank: &'static str,
    /// bis zu vier Loops (zwei Drehzahlen × Last/Schub), unbenutzte mit `gain` 0
    pub layers: [Layer; 4],
    /// ausgelöste Einzelklänge: Index in `Bank::shots`, Pegel
    pub shots: Vec<(usize, f32)>,
    /// Begrenzer: 1 = offen, kleiner = abgeschnitten
    pub gate: f32,
    pub tone: Tone,
    pub volume: f32,
    // für Anzeige und Tests
    pub rpm: f64,
    pub throttle: f64,
    pub gear: usize,
    pub limiter: bool,
}

/// Fortlaufender Klangzustand eines Fahrzeugs.
#[derive(Debug, Clone, Default)]
pub struct EngineSound {
    pub rpm: f64,
    pub thr: f64,
    pub gear: usize,
    t: f64,
    shift_t: f64,
    prev_thr: f64,
    pops: Vec<f64>,
    pop_ready: f64,
    blip_ready: f64,
    counter: u32,
    seed: u32,
    started: bool,
    limiter_phase: f64,
}

/// Gewichte gleicher Leistung zwischen den Loops einer Liste (Last oder Schub), die die (Klang-)Drehzahl
/// einrahmen: (Position in der Liste, Gewicht).
pub fn loop_weights(bank: &Bank, list: &[usize], rpm: f64) -> [(usize, f64); 2] {
    let r: Vec<f64> = list.iter().map(|&i| bank.loops[i].rpm).collect();
    let n = r.len();
    if rpm <= r[0] {
        return [(0, 1.), (0, 0.)];
    }
    if rpm >= r[n - 1] {
        return [(n - 1, 1.), (n - 1, 0.)];
    }
    let k = r.windows(2).position(|w| rpm < w[1]).unwrap_or(n - 2);
    // überblendet wird nur, wo beide Loops eine erlaubte Tonhöhe haben; ist die Lücke dafür zu groß, ein schmales
    // Fenster um die (geometrische) Mitte
    let (mut lo, mut hi) = (
        r[k].max(r[k + 1] * PITCH_MIN),
        r[k + 1].min(r[k] * PITCH_MAX),
    );
    if hi <= lo {
        let m = (r[k] * r[k + 1]).sqrt();
        (lo, hi) = (m / 1.02, m * 1.02);
    }
    let u = ((rpm / lo).ln() / (hi / lo).ln()).clamp(0., 1.);
    [(k, (u * FRAC_PI_2).cos()), (k + 1, (u * FRAC_PI_2).sin())]
}

/// Tonhöhengrenzen des Loops an Position `k` seiner Liste: 0,7 … 1,4. Wo der Abstand zum Nachbarn größer als
/// Faktor 2 ist (die Aufnahme gibt dazwischen nichts her), so weit, dass die Tonhöhe bis zur Mitte der Lücke
/// stetig bleibt – sonst spränge sie in der Überblendung.
pub fn pitch_bounds(bank: &Bank, list: &[usize], k: usize) -> (f64, f64) {
    let r = |j: usize| bank.loops[list[j]].rpm;
    let span = PITCH_MAX / PITCH_MIN;
    let mut b = (PITCH_MIN, PITCH_MAX);
    if k + 1 < list.len() && r(k + 1) / r(k) > span {
        b.1 = (r(k + 1) / r(k)).sqrt() * 1.03;
    }
    if k > 0 && r(k) / r(k - 1) > span {
        b.0 = 1. / ((r(k) / r(k - 1)).sqrt() * 1.03);
    }
    b
}

/// Tonhöhe eines Loops für eine Klang-Drehzahl (begrenzt auf 0,7 … 1,4).
pub fn loop_pitch(loop_rpm: f64, rpm: f64) -> f64 {
    (rpm / loop_rpm).clamp(PITCH_MIN, PITCH_MAX)
}

fn pitch_in(bank: &Bank, list: &[usize], k: usize, rpm: f64) -> f64 {
    let (lo, hi) = pitch_bounds(bank, list, k);
    (rpm / bank.loops[list[k]].rpm).clamp(lo, hi)
}

fn smooth(v: &mut f64, target: f64, tc: f64, dt: f64) {
    *v += (target - *v) * (1. - (-dt / tc.max(1e-3)).exp());
}

impl EngineSound {
    pub fn new(seed: u32) -> Self {
        Self {
            gear: 1,
            seed,
            ..Default::default()
        }
    }
    fn rand(&mut self) -> f64 {
        self.counter = self.counter.wrapping_add(1);
        hash01(self.seed as f64 * 7919.13 + self.counter as f64 * 104.729)
    }

    /// Virtuelles Getriebe: Drehzahl aus Tempo und Gang, Hoch- und Runterschalten nach Last.
    fn virtual_rpm(&mut self, p: &Profile, limiter: f64, inp: &SoundInput) -> (f64, i32) {
        let kmh = inp.speed * 3.6;
        let n = p.gaenge.len().max(1);
        self.gear = self.gear.clamp(1, n);
        let rpm_in = |g: usize| kmh / p.gaenge[g - 1] * limiter;
        let thr = inp.throttle.clamp(0., 1.);
        let up =
            limiter * (p.schalt_hoch_teillast + (p.schalt_hoch - p.schalt_hoch_teillast) * thr);
        let down = limiter * p.schalt_runter;
        let mut change = 0;
        if self.shift_t <= 0. {
            if self.gear < n && rpm_in(self.gear) > up && rpm_in(self.gear + 1) > down {
                self.gear += 1;
                change = 1;
            } else if self.gear > 1 && rpm_in(self.gear) < down && rpm_in(self.gear - 1) < up * 0.95
            {
                self.gear -= 1;
                change = -1;
            }
        }
        // im Stand: Gas lässt den Motor frei hochdrehen; sonst folgt er dem Rad, Schlupf lässt ihn darüber schießen
        let free = p.leerlauf + thr * (limiter - p.leerlauf) * 0.45;
        let wheel = rpm_in(self.gear) + inp.slip.clamp(0., 1.) * limiter * 0.3;
        let rpm = if kmh < 4. {
            free.max(wheel)
        } else {
            wheel.max(p.leerlauf)
        };
        (rpm.min(limiter), change)
    }

    /// Ein Bild: Zustand fortschreiben, Ausgabe für den Sampler. `lod` = nur ein Loop ohne Überblendung.
    pub fn step(
        &mut self,
        p: &Profile,
        bank: &'static Bank,
        inp: &SoundInput,
        dt: f64,
        lod: bool,
    ) -> SoundOut {
        let dt = dt.clamp(0., 0.25);
        self.t += dt;
        self.shift_t -= dt;
        let limiter = inp.limiter.unwrap_or(p.begrenzer);
        let thr_raw = inp.throttle.clamp(0., 1.);
        let mut out = SoundOut {
            bank: bank.name.as_str(),
            gate: 1.,
            ..Default::default()
        };
        if !self.started {
            // erstes Bild: Zustand übernehmen, nichts auslösen
            self.started = true;
            self.rpm = inp.rpm.unwrap_or(p.leerlauf);
            self.thr = thr_raw;
            self.prev_thr = thr_raw;
            if let Some(g) = inp.gear {
                self.gear = g.max(1);
            }
        }
        // Drehzahl und Gangwechsel
        let (target, change) = match (inp.rpm, inp.gear) {
            (Some(r), Some(g)) => {
                let c = (g as i32 - self.gear as i32).signum();
                self.gear = g.max(1);
                (r.max(p.leerlauf * 0.9), c)
            }
            (Some(r), None) => (r.max(p.leerlauf * 0.9), 0),
            _ => self.virtual_rpm(p, limiter, inp),
        };
        if change > 0 {
            self.shift_t = SHIFT_CUT;
            if let Some(&s) = bank.shots_of(ShotKind::Shift).get(self.pick(3)) {
                out.shots.push((s, (0.35 + 0.5 * thr_raw) as f32));
            }
            // Zündunterbrechung beim Hochschalten unter Last knallt manchmal
            if thr_raw > 0.7 && self.rand() < p.pop_chance * 0.4 {
                self.pops.push(self.t + 0.02);
            }
        } else if change < 0 && thr_raw < 0.5 && self.t >= self.blip_ready {
            // Zwischengas beim Runterschalten
            self.blip_ready = self.t + BLIP_COOLDOWN;
            if let Some(&s) = bank.shots_of(ShotKind::Blip).first() {
                out.shots.push((s, 0.6));
            }
            self.rpm = self.rpm.max(target * 1.08);
        }
        // Begrenzer: rhythmisches Abschneiden, die Drehzahl zuckt mit
        let at_limit = target >= limiter * 0.985 && thr_raw > 0.6;
        out.limiter = at_limit;
        let mut target = target;
        if at_limit {
            self.limiter_phase = (self.limiter_phase + dt * p.begrenzer_hz).fract();
            let cut = self.limiter_phase > 0.55;
            out.gate = if cut { 0.3 } else { 1. };
            if cut {
                target *= 0.965;
            }
        } else {
            self.limiter_phase = 0.;
        }
        smooth(&mut self.rpm, target, p.drehzahl_glaettung_s, dt);
        let thr_eff = if self.shift_t > 0. { 0. } else { thr_raw };
        smooth(&mut self.thr, thr_eff, p.gas_glaettung_s, dt);
        // Gas weg aus hoher Drehzahl: Pops
        if self.prev_thr > 0.5
            && thr_raw < 0.15
            && self.rpm > limiter * 0.55
            && self.t >= self.pop_ready
            && self.rand() < p.pop_chance
        {
            self.pop_ready = self.t + POP_COOLDOWN;
            let n = 1 + (self.rand() * (1. + 2. * p.pop_chance)) as usize;
            for i in 0..n {
                let at = self.t + 0.04 + i as f64 * (0.08 + 0.12 * self.rand());
                self.pops.push(at);
            }
        }
        self.prev_thr = thr_raw;
        let pops = bank.shots_of(ShotKind::Pop);
        let t = self.t;
        let due = self.pops.iter().filter(|&&a| a <= t).count();
        self.pops.retain(|&a| a > t);
        for _ in 0..due {
            if !pops.is_empty() {
                let s = pops[self.pick(pops.len())];
                let g = p.pop_gain * (0.6 + 0.4 * self.rand());
                out.shots.push((s, g as f32));
            }
        }

        // Loops: Klang-Drehzahl = Drehzahl × Grundverstimmung
        let rs = self.rpm * p.pitch;
        let w_on = loop_weights(bank, &bank.on, rs);
        let w_off = loop_weights(bank, &bank.off, rs);
        // nahe am Leerlauf ist das Last-Loop der Leerlauf selbst; sonst mischt das Gas Last und Schub
        let idle_on = (1. - (self.rpm - p.leerlauf) / 1200.).clamp(0., 1.);
        let load = self.thr.max(idle_on);
        let (g_on, g_off) = (load.sqrt(), (1. - load).sqrt());
        let mut layers = [Layer::default(); 4];
        if lod {
            // nur der nächstgelegene Loop, Last- und Schubpegel als Lautstärke
            let k = if w_on[1].1 > w_on[0].1 {
                w_on[1].0
            } else {
                w_on[0].0
            };
            let i = bank.on[k];
            layers[0] = Layer {
                idx: i,
                gain: (bank.loops[i].gain * (0.6 + 0.4 * load)) as f32,
                pitch: pitch_in(bank, &bank.on, k, rs) as f32,
            };
        } else {
            for (m, (list, w, g)) in [(&bank.on, w_on, g_on), (&bank.off, w_off, g_off)]
                .into_iter()
                .enumerate()
            {
                for (j, &(k, wk)) in w.iter().enumerate() {
                    let i = list[k];
                    layers[m * 2 + j] = Layer {
                        idx: i,
                        gain: (wk * g * bank.loops[i].gain) as f32,
                        pitch: pitch_in(bank, list, k, rs) as f32,
                    };
                }
            }
        }
        out.layers = layers;
        out.tone = Tone {
            lowpass: p.tiefpass_hz as f32,
            low_db: p.low_shelf_db as f32,
            high_db: p.high_shelf_db as f32,
            drive: (p.saettigung * (0.5 + 0.5 * self.thr)) as f32,
        };
        out.volume = p.lautstaerke as f32;
        out.rpm = self.rpm;
        out.throttle = self.thr;
        out.gear = self.gear;
        out
    }
    fn pick(&mut self, n: usize) -> usize {
        ((self.rand() * n as f64) as usize).min(n.saturating_sub(1))
    }
    /// Startgeräusch beim ersten Anlassen (Spielerauto, Profil erlaubt es).
    pub fn start_shot(bank: &Bank, p: &Profile) -> Option<(usize, f32)> {
        p.start
            .then(|| bank.shots_of(ShotKind::Start).first().map(|&s| (s, 0.9)))
            .flatten()
    }
}

/// Räumliche Werte einer fremden Motorstimme.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spatial {
    pub gain: f64,
    pub pan: f64,
    /// Doppler-Faktor der Tonhöhe
    pub rate: f64,
    pub lowpass: f64,
    pub lod: bool,
}

/// Entfernung (px), Versatz quer (px), Annäherungsgeschwindigkeit (px/s, > 0 kommt näher) → räumliche Werte.
pub fn spatial(mix: &Mix, d: f64, dx: f64, closing: f64) -> Spatial {
    let k = (1. - d / mix.hoerweite_px).clamp(0., 1.);
    let c = crate::soundscape::SOUND_SPEED;
    let raw = c / (c - closing.clamp(-1500., 1500.));
    Spatial {
        gain: k * k,
        pan: (dx / 300.).clamp(-1., 1.),
        rate: 1. + (raw - 1.) * mix.doppler,
        // Höhen verschwinden mit der Entfernung (logarithmisch zwischen nah und fern)
        lowpass: mix.tiefpass_nah_hz * (mix.tiefpass_fern_hz / mix.tiefpass_nah_hz).powf(1. - k),
        lod: d > mix.lod_px,
    }
}

/// Stimmen auswählen: das Spielerauto immer, dann die lautesten fremden bis zur Grenze `n`.
pub fn select_voices<T>(player: Option<T>, mut others: Vec<(f64, T)>, n: usize) -> Vec<T> {
    let mut out: Vec<T> = player.into_iter().collect();
    others.sort_by(|a, b| b.0.total_cmp(&a.0));
    out.extend(
        others
            .into_iter()
            .filter(|(g, _)| *g > 1e-3)
            .map(|(_, v)| v)
            .take(n.saturating_sub(out.len())),
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sup() -> &'static Profile {
        config().preset("supercar").unwrap()
    }
    fn bank() -> &'static Bank {
        config().bank("v10")
    }
    fn input(speed_kmh: f64, thr: f64) -> SoundInput {
        SoundInput {
            throttle: thr,
            speed: speed_kmh / 3.6,
            ..Default::default()
        }
    }

    #[test]
    fn profiles_and_bank_load() {
        let c = config();
        assert_eq!(
            c.presets
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>(),
            [
                "sport", "supercar", "hypercar", "sport4", "kompakt", "diesel", "lkw"
            ]
        );
        assert_eq!(
            c.banks()
                .iter()
                .map(|b| b.name.as_str())
                .collect::<Vec<_>>(),
            ["v10", "v12", "r4", "d4", "d6"]
        );
        for b in c.banks() {
            assert!(b.on.len() >= 4 && b.off.len() >= 4, "{}", b.name);
            // jede Liste deckt die Drehzahlen lückenlos ab; wo der Abstand über Faktor 2 liegt, erweitert
            // `pitch_bounds` die Grenzen bis zur Mitte (geprüft unten), mehr als Faktor 2,3 darf es nicht sein
            for list in [&b.on, &b.off] {
                for w in list.windows(2) {
                    let r = b.loops[w[1]].rpm / b.loops[w[0]].rpm;
                    assert!(r > 1. && r <= 2.3, "{}: Lücke {r}", b.name);
                }
            }
        }
        // wer knallen darf, braucht echte Fehlzündungen; Gasstöße gibt es nur, wo die Aufnahme welche hat
        for p in &c.presets {
            if p.pop_chance > 0. {
                assert!(
                    !c.bank(&p.bank).shots_of(ShotKind::Pop).is_empty(),
                    "{}",
                    p.name
                );
            }
        }
        for name in ["v10", "v12"] {
            assert!(!c.bank(name).shots_of(ShotKind::Blip).is_empty(), "{name}");
        }
        // der Vierzylinder hat Start, aber weder Gasstoß noch Fehlzündung (Prüfstand, Alltagsmotor)
        let r4 = c.bank("r4");
        assert!(!r4.shots_of(ShotKind::Start).is_empty() && r4.shots_of(ShotKind::Pop).is_empty());
        // v10 hat Start und Schalten aus der Aufnahme, der Prüfstand (v12) nicht
        let v10 = c.bank("v10");
        assert!(
            !v10.shots_of(ShotKind::Start).is_empty() && !v10.shots_of(ShotKind::Shift).is_empty()
        );
        // echter Schub (v12) liegt auf anderen Drehzahlen als die Last-Loops
        let v12 = c.bank("v12");
        let on: Vec<f64> = v12.on.iter().map(|&i| v12.loops[i].rpm).collect();
        assert!(v12.off.iter().any(|&i| !on.contains(&v12.loops[i].rpm)));
        // Presets unterscheiden sich hörbar: Verstimmung, Begrenzer, Färbung, Pops
        let [s, u, h] = [&c.presets[0], &c.presets[1], &c.presets[2]];
        assert!(s.pitch < u.pitch);
        assert!(s.bank == "v10" && u.bank == "v10" && h.bank == "v12");
        assert!(s.begrenzer < u.begrenzer && u.begrenzer < h.begrenzer);
        assert!(s.tiefpass_hz < u.tiefpass_hz && u.tiefpass_hz < h.tiefpass_hz);
        assert!(s.pop_chance < u.pop_chance && u.pop_chance < h.pop_chance);
    }

    #[test]
    fn combustion_engines_get_their_bank_electric_and_two_stroke_stay_synth() {
        let c = config();
        let get = |id: &str| {
            let v = crate::vehdata::game_vehicle(id).unwrap_or_else(|| panic!("{id}"));
            c.profile_for_vehicle(v).map(|p| p.name.clone())
        };
        assert_eq!(get("supercar_awd").as_deref(), Some("supercar"));
        assert_eq!(
            get("supercar_rwd").as_deref(),
            Some("supercar/supercar_rwd")
        );
        assert_eq!(get("hypercar").as_deref(), Some("hypercar/hypercar"));
        assert_eq!(get("sportwagen_s").as_deref(), Some("sport/sportwagen_s"));
        // Vierzylinder: Alltag über die Klasse, Sportler über die Überschreibung – beide Bank r4
        assert_eq!(get("kompakt").as_deref(), Some("kompakt"));
        assert_eq!(get("kleinwagen").as_deref(), Some("kompakt"));
        for id in ["roadster", "leichtcoupe", "rallye", "drift_coupe"] {
            assert_eq!(get(id), Some(format!("sport4/{id}")), "{id}");
            assert_eq!(
                c.profile_for_vehicle(crate::vehdata::game_vehicle(id).unwrap())
                    .unwrap()
                    .bank,
                "r4"
            );
        }
        // Motortyp vor Klasse: der Diesel-Kombi bekommt den Diesel, obwohl seine Klasse Vierzylinder-Benziner hat
        assert_eq!(get("kombi").as_deref(), Some("kompakt"));
        assert_eq!(get("familienkombi").as_deref(), Some("diesel"));
        assert_eq!(get("transporter_kasten").as_deref(), Some("diesel"));
        for id in ["stadtbus", "sattelzug_40t", "muellwagen"] {
            assert_eq!(get(id).as_deref(), Some("lkw"), "{id}");
        }
        // Elektro, Zweitakter und luftgekühlter Boxer behalten die Synthese
        for id in [
            "hypercar_elektro",
            "e_kompakt",
            "e_bus",
            "trabant",
            "oldtimer_kaefer",
        ] {
            if crate::vehdata::game_vehicle(id).is_some() {
                assert_eq!(get(id), None, "{id}");
            }
        }
        // Überschreibung: nur das Genannte ändert sich
        let base = c.preset("supercar").unwrap();
        let rwd = c
            .profile_for_vehicle(crate::vehdata::game_vehicle("supercar_rwd").unwrap())
            .unwrap();
        assert_eq!(rwd.pitch, 0.95);
        assert_eq!(rwd.begrenzer, base.begrenzer);
        // der Bugatti-artige Hypercar dreht nicht über seine Daten hinaus
        let hy = c
            .profile_for_vehicle(crate::vehdata::game_vehicle("hypercar").unwrap())
            .unwrap();
        assert!(hy.begrenzer < 7500.);
    }

    #[test]
    fn crossfade_keeps_power_and_pitch_stays_in_bounds() {
        let c = config();
        for p in &c.presets {
            let b = c.bank(&p.bank);
            for list in [&b.on, &b.off] {
                let mut r = p.leerlauf * 0.95;
                while r <= p.begrenzer {
                    let rs = r * p.pitch;
                    let w = loop_weights(b, list, rs);
                    let sum: f64 = w.iter().map(|x| x.1 * x.1).sum();
                    assert!((sum - 1.).abs() < 1e-9, "{rs}: {sum}");
                    // jeder hörbare Loop bleibt innerhalb seiner Tonhöhengrenzen, ohne dass sie greifen müssen –
                    // außer unter dem tiefsten bzw. über dem höchsten Loop (dort hält die Grenze)
                    for (k, wk) in w {
                        if wk > 0.05 {
                            let raw = rs / b.loops[list[k]].rpm;
                            let (lo, hi) = pitch_bounds(b, list, k);
                            let (bottom, top) = (k == 0, k == list.len() - 1);
                            assert!(raw >= lo - 0.02 || bottom, "{} bei {rs}: {raw}", p.name);
                            assert!(raw <= hi + 0.02 || top, "{} bei {rs}: {raw}", p.name);
                            assert!(lo >= 0.6 && hi <= 1.6, "{}: Grenzen {lo}…{hi}", p.name);
                        }
                    }
                    r += 50.;
                }
            }
        }
        assert_eq!(loop_pitch(1000., 5000.), PITCH_MAX);
        assert_eq!(loop_pitch(5000., 1000.), PITCH_MIN);
    }

    #[test]
    fn virtual_gearbox_shifts_up_under_load_and_blips_on_the_way_down() {
        let (p, b) = (sup(), bank());
        let mut e = EngineSound::new(1);
        let dt = 1. / 60.;
        let mut gear = 1;
        let mut shifts = 0;
        let mut max_rpm: f64 = 0.;
        let mut v = 0.;
        while v < 280. {
            let o = e.step(p, b, &input(v, 1.), dt, false);
            max_rpm = max_rpm.max(o.rpm);
            if o.gear > gear {
                shifts += 1;
                // beim Hochschalten fällt die Drehzahl, das Gas ist kurz weg
                assert!(o.shots.iter().any(|s| b.shots[s.0].kind == ShotKind::Shift));
            }
            assert!(o.gear >= gear, "unter Last nie zurück");
            gear = o.gear;
            v += 35. * dt;
        }
        assert!(shifts >= 4, "{shifts}");
        assert!(max_rpm <= p.begrenzer + 1.);
        assert!(max_rpm > p.begrenzer * 0.85, "{max_rpm}");
        // ausrollen ohne Gas: runterschalten mit Zwischengas
        let mut blips = 0;
        while v > 10. {
            let o = e.step(p, b, &input(v, 0.), dt, false);
            blips += o
                .shots
                .iter()
                .filter(|s| b.shots[s.0].kind == ShotKind::Blip)
                .count();
            v -= 25. * dt;
        }
        assert!(blips >= 2, "{blips}");
        assert!(e.gear <= 2);
    }

    #[test]
    fn lifting_off_at_high_revs_pops_more_on_hypercars() {
        let b = bank();
        let count = |p: &Profile| {
            let mut n = 0;
            for seed in 0..60 {
                let mut e = EngineSound::new(seed);
                let inp = |thr| SoundInput {
                    rpm: Some(p.begrenzer * 0.8),
                    gear: Some(3),
                    throttle: thr,
                    speed: 30.,
                    ..Default::default()
                };
                for _ in 0..30 {
                    e.step(p, b, &inp(1.), 1. / 60., false);
                }
                for _ in 0..60 {
                    let o = e.step(p, b, &inp(0.), 1. / 60., false);
                    n += o
                        .shots
                        .iter()
                        .filter(|s| b.shots[s.0].kind == ShotKind::Pop)
                        .count();
                }
            }
            n
        };
        let c = config();
        let (sport, hyper) = (
            count(c.preset("sport").unwrap()),
            count(c.preset("hypercar").unwrap()),
        );
        assert!(
            hyper > sport * 2 && sport > 0,
            "sport {sport}, hyper {hyper}"
        );
        // aus niedriger Drehzahl nie
        let p = sup();
        let mut e = EngineSound::new(3);
        let low = |thr| SoundInput {
            rpm: Some(2000.),
            gear: Some(2),
            throttle: thr,
            ..Default::default()
        };
        for _ in 0..30 {
            e.step(p, b, &low(1.), 1. / 60., false);
        }
        for _ in 0..60 {
            let o = e.step(p, b, &low(0.), 1. / 60., false);
            assert!(o.shots.is_empty());
        }
    }

    #[test]
    fn limiter_cuts_rhythmically() {
        let (p, b) = (sup(), bank());
        let mut e = EngineSound::new(9);
        let inp = SoundInput {
            rpm: Some(p.begrenzer),
            gear: Some(2),
            throttle: 1.,
            speed: 20.,
            ..Default::default()
        };
        let gates: Vec<f32> = (0..120)
            .map(|_| e.step(p, b, &inp, 1. / 120., false).gate)
            .collect();
        let cuts = gates.windows(2).filter(|w| w[0] == 1. && w[1] < 1.).count();
        // 1 s bei 14 Hz: etwa 14 Schnitte
        assert!((10..=16).contains(&cuts), "{cuts}");
        // unterhalb des Begrenzers offen
        let mut e = EngineSound::new(9);
        let o = e.step(
            p,
            b,
            &SoundInput {
                rpm: Some(5000.),
                ..inp
            },
            1. / 60.,
            false,
        );
        assert_eq!(o.gate, 1.);
    }

    #[test]
    fn rpm_and_throttle_are_smoothed() {
        let (p, b) = (sup(), bank());
        let mut e = EngineSound::new(1);
        let at = |r| SoundInput {
            rpm: Some(r),
            gear: Some(3),
            throttle: 0.5,
            ..Default::default()
        };
        e.step(p, b, &at(3000.), 1., false);
        e.step(p, b, &at(3000.), 1., false);
        let o = e.step(p, b, &at(6000.), p.drehzahl_glaettung_s, false);
        let k = (o.rpm - 3000.) / 3000.;
        assert!((k - 0.632).abs() < 0.01, "{k}");
    }

    #[test]
    fn lod_plays_one_loop_and_gas_mixes_load() {
        let (p, b) = (sup(), bank());
        let mut e = EngineSound::new(1);
        let inp = SoundInput {
            rpm: Some(4000.),
            gear: Some(3),
            throttle: 1.,
            ..Default::default()
        };
        let o = e.step(p, b, &inp, 1., true);
        assert_eq!(o.layers.iter().filter(|l| l.gain > 0.).count(), 1);
        // voll: Last überwiegt; Gas weg: Schub überwiegt
        let mut e = EngineSound::new(1);
        let full = e.step(p, b, &inp, 1., false);
        let on: f32 = full
            .layers
            .iter()
            .filter(|l| b.loops[l.idx].on)
            .map(|l| l.gain)
            .sum();
        let off: f32 = full
            .layers
            .iter()
            .filter(|l| !b.loops[l.idx].on)
            .map(|l| l.gain)
            .sum();
        assert!(on > 0.9 && off < 0.05, "{on} {off}");
        let lift = e.step(
            p,
            b,
            &SoundInput {
                throttle: 0.,
                ..inp
            },
            1.,
            false,
        );
        let off: f32 = lift
            .layers
            .iter()
            .filter(|l| !b.loops[l.idx].on)
            .map(|l| l.gain)
            .sum();
        assert!(off > 0.3, "{off}");
    }

    #[test]
    fn spatial_and_voice_selection() {
        let m = &config().mix;
        let near = spatial(m, 50., 30., 0.);
        let far = spatial(m, 500., -300., -400.);
        assert!(near.gain > far.gain && near.lowpass > far.lowpass);
        assert!(!near.lod && far.lod);
        assert!(far.rate < 1. && far.pan == -1.);
        assert_eq!(spatial(m, m.hoerweite_px + 1., 0., 0.).gain, 0.);
        // Spielerauto zuerst, dann die lautesten
        let v = select_voices(
            Some("ich"),
            vec![(0.2, "b"), (0.9, "a"), (0.0, "x"), (0.5, "c")],
            3,
        );
        assert_eq!(v, ["ich", "a", "c"]);
    }
}
