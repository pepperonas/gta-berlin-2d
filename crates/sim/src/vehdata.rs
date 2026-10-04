//! Fahrzeugdaten (`data/vehicles/*.json`): Laden, Vererbung, Ableitungen, Prüfung.
//!
//! Ein Fahrzeug unterscheidet sich von einem anderen ausschließlich über Daten. Ein Eintrag braucht nur wenige
//! Felder; der Rest kommt aus dem Klassen-Preset (`klasse`, `classes.json`) bzw. aus einem Basisfahrzeug (`basis`).
//! Fehlende physikalische Größen werden abgeleitet (Drehmoment aus Leistung und Kurve, Übersetzungen aus Gangzahl,
//! Vmax und Drehzahl, Lenkeinschlag aus dem Wendekreis, Gierträgheit aus Masse und Achsabständen). Intern gilt
//! ausschließlich SI (m, s, kg, N, rad); km/h und Grad nur in den Daten.
//!
//! Kalibrierte Werte (`vehicles.calibrated.json`, geschrieben vom Kalibrier-Werkzeug) liegen getrennt und
//! überschreiben nur die erlaubten Stellschrauben; die Originaldaten bleiben unverändert.
use anyhow::{Context, Result, anyhow, bail, ensure};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, HashMap};
use std::f64::consts::PI;
use std::path::Path;

pub const G: f64 = 9.81;
pub const RHO: f64 = 1.2;

const TIRES: &str = include_str!("../../../data/vehicles/tires.json");
const SURFACES: &str = include_str!("../../../data/vehicles/surfaces.json");
const CURVES: &str = include_str!("../../../data/vehicles/engine-curves.json");
const CLASSES: &str = include_str!("../../../data/vehicles/classes.json");
const VEHICLES: &str = include_str!("../../../data/vehicles/vehicles.json");
const FEEL: &str = include_str!("../../../data/vehicles/feel.json");
const CALIBRATED: &str = include_str!("../../../data/vehicles/vehicles.calibrated.json");

/// Reifentyp (`tires.json`).
#[derive(Debug, Clone, PartialEq)]
pub struct Tire {
    pub id: String,
    /// Haftbeiwert quer auf trockenem Asphalt
    pub mu: f64,
    /// Längs haften Reifen etwas besser als quer
    pub mu_long: f64,
    /// Schräglaufwinkel des Kraftmaximums (rad), Zweiräder ohne Angabe: 6°
    pub peak_slip_angle: f64,
    pub peak_slip_ratio: f64,
    /// Gleitreibung / Haftmaximum
    pub slide_ratio: f64,
    /// Faktor je Untergrundkategorie
    pub factor: HashMap<String, f64>,
    pub rolling: f64,
}
impl Tire {
    /// Magic-Formula-Form (B, C) so, dass das Maximum beim Peak-Schräglauf liegt und die Kurve für große Winkel
    /// auf `slide_ratio` abfällt: sin(C·π/2) = slide_ratio, C·atan(B·α_peak) = π/2.
    pub fn shape(&self) -> (f64, f64) {
        let c = 2. - 2. * self.slide_ratio.clamp(0.3, 0.999).asin() / PI;
        let b = (PI / (2. * c)).tan() / self.peak_slip_angle.max(1e-3);
        (b, c)
    }
}

/// Untergrund (`surfaces.json`).
#[derive(Debug, Clone, PartialEq)]
pub struct SurfaceDef {
    pub id: String,
    pub mu_rel: f64,
    pub category: String,
    pub rolling_extra: f64,
}

/// Spielgefühl-Regler (`feel.json`).
#[derive(Debug, Clone, PartialEq)]
pub struct Feel {
    pub realism: f64,
    pub grip_global: f64,
    pub brake_global: f64,
    pub steer_assist: f64,
    pub drift_assist: u8,
    pub esp_default: String,
    pub aquaplaning: f64,
    pub ice_grip_min: f64,
    pub weight_transfer: f64,
    pub camera_zoom_by_speed: bool,
    pub truck_limiter_tunable: bool,
    pub wall_slide: bool,
}
impl Feel {
    /// Simulation pur (Kalibrierung): Realismus 1, globaler Grip 1.
    pub fn simulation() -> Self {
        Self {
            realism: 1.,
            grip_global: 1.,
            brake_global: 1.,
            ..Self::parse(&serde_json::from_str(FEEL).expect("feel.json")).expect("feel.json")
        }
    }
    fn parse(j: &Value) -> Result<Self> {
        let f = |k: &str, d: f64| j[k].as_f64().unwrap_or(d);
        let b = |k: &str, d: bool| j[k].as_bool().unwrap_or(d);
        Ok(Self {
            realism: f("realismus", 0.7).clamp(0., 1.),
            grip_global: f("grip_global", 1.),
            brake_global: f("bremse_global", 1.),
            steer_assist: f("lenk_assist", 0.5),
            drift_assist: f("drift_assist_stufe", 2.) as u8,
            esp_default: j["esp_spieler_default"].as_str().unwrap_or("sport").into(),
            aquaplaning: f("aquaplaning_staerke", 0.8),
            ice_grip_min: f("eis_grip_minimum", 0.06),
            weight_transfer: f("gewichtsverlagerung_skala", 1.),
            camera_zoom_by_speed: b("kamera_zoom_nach_tempo", true),
            truck_limiter_tunable: b("lkw_begrenzer_tunebar", true),
            wall_slide: b("wandkontakt_gleiten", true),
        })
    }
    /// Arcade-Blende: bei Realismus 0 Grip ×1,25, Bremse ×1,2, Gewichtsverlagerung ×0,6, Aquaplaning ×0,3.
    pub fn grip(&self) -> f64 {
        self.grip_global * lerp(1.25, 1., self.realism)
    }
    pub fn brake(&self) -> f64 {
        self.brake_global * lerp(1.2, 1., self.realism)
    }
    pub fn transfer(&self) -> f64 {
        self.weight_transfer * lerp(0.6, 1., self.realism)
    }
    pub fn aqua(&self) -> f64 {
        self.aquaplaning * lerp(0.3, 1., self.realism)
    }
}
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Drive {
    Fwd,
    Rwd,
    Awd,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Esp {
    Off,
    Sport,
    Full,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Diff {
    Open,
    Lsd,
    Locked,
    Vectoring,
}

/// Motor: Drehmomentkurve, Elektro (konstantes Moment bis Eckgeschwindigkeit) oder Muskelkraft.
#[derive(Debug, Clone, PartialEq)]
pub enum Power {
    Curve(Vec<(f64, f64)>),
    Electric {
        corner_frac: f64,
    },
    Muscle {
        force_max: f64,
        w_cont: f64,
        w_sprint: f64,
    },
}
#[derive(Debug, Clone, PartialEq)]
pub struct Engine {
    pub kind: String,
    pub power: Power,
    /// Spitzenleistung (W) und Spitzenmoment (Nm)
    pub watts: f64,
    pub nm: f64,
    /// Höchstdrehzahl und Drehzahl der Spitzenleistung (1/min)
    pub n_max: f64,
    pub n_peak_power: f64,
    pub n_idle: f64,
    /// tatsächlich erreichte Spitzenleistung (W)
    pub peak_watts: f64,
    /// Kurve oben verbreitert: ab dem ersten Erreichen des Spitzenmoments (Anteil von n_max) gilt min(nm, P/ω).
    /// Gesetzt, wenn Moment und Kurvenform die Spitzenleistung sonst nicht hergäben (moderne Turbos).
    pub band_from: Option<f64>,
    /// Turbo-Lag: Zeitkonstante des Ladedrucks (s), 0 = Sauger/Elektro
    pub lag: f64,
}
impl Engine {
    /// Moment bei Drehzahl n (1/min), Volllast, ohne Turbo-Lag: T(n) = min(nm·kurve(n/n_max), P/ω).
    pub fn torque(&self, n: f64) -> f64 {
        let Power::Curve(c) = &self.power else {
            return self.nm;
        };
        let n = n.max(self.n_idle);
        let x = n / self.n_max;
        let mut t = self.nm * curve_at(c, x);
        let w = n * 2. * PI / 60.;
        if self.band_from.is_some_and(|b| x >= b) {
            t = t.max(self.nm);
        }
        t.min(self.watts / w.max(1.))
    }
}
/// Lineare Interpolation über Stützpunkte; außerhalb konstant.
pub fn curve_at(c: &[(f64, f64)], x: f64) -> f64 {
    let Some(&(x0, y0)) = c.first() else {
        return 1.;
    };
    if x <= x0 {
        return y0;
    }
    for w in c.windows(2) {
        let ((a, ya), (b, yb)) = (w[0], w[1]);
        if x <= b {
            return ya + (yb - ya) * (x - a) / (b - a).max(1e-9);
        }
    }
    c.last().map_or(1., |p| p.1)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Gearbox {
    pub kind: String,
    /// Gesamtübersetzung je Gang (Motorumdrehungen je Radumdrehung); einstufig/CVT: ein Eintrag
    pub ratios: Vec<f64>,
    pub shift: f64,
    pub cvt: bool,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Brake {
    pub kind: String,
    pub abs: bool,
    /// Anteil der Bremskraft vorn
    pub front: f64,
    /// Aufbauzeit (s)
    pub build: f64,
    /// Fading: 0 kaum, 1 mittel, 2 hoch
    pub fading: f64,
    pub retarder: Option<String>,
    /// größte Bremsverzögerung, die die Anlage aufbringen kann (in g; über der Haftung = haftungsbegrenzt)
    pub gain: f64,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Steering {
    pub turning_circle: f64,
    /// größter Radeinschlag (rad), aus dem Wendekreis
    pub delta_max: f64,
    /// tempoabhängige Lenkung: v_s (m/s), Mindesteinschlag bei hohem Tempo (rad)
    pub v_s: f64,
    pub delta_hs: f64,
    pub rear: bool,
    pub rate_factor: f64,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Chassis {
    /// Wanken bzw. Nicken in Grad je g
    pub roll: f64,
    pub pitch: f64,
    pub hz: f64,
    /// Zeitkonstante der Lastverlagerung (s)
    pub tau: f64,
}
#[derive(Debug, Clone, PartialEq)]
pub struct DriftDef {
    pub ability: f64,
    pub max_angle: f64,
    pub rear_grip: f64,
}

/// Fertig aufgelöstes Fahrzeug (SI).
#[derive(Debug, Clone, PartialEq)]
pub struct Vehicle {
    pub id: String,
    pub name: String,
    pub class: String,
    pub model: String,
    pub mass_empty: f64,
    pub mass_full: f64,
    pub cg_empty: f64,
    pub cg_full: f64,
    /// Zuladung 0…1, mit der die Klasse kalibriert wird (LKW/Bus voll, sonst leer)
    pub calib_load: f64,
    pub length: f64,
    pub width: f64,
    pub height: f64,
    pub wheelbase: f64,
    pub track: f64,
    /// Gewichtsanteil vorn
    pub front: f64,
    pub drive: Drive,
    /// Momentenanteil vorn bei Allrad
    pub awd_front: f64,
    pub engine: Engine,
    pub gearbox: Gearbox,
    pub efficiency: f64,
    pub tire: Tire,
    pub tire_width_mm: f64,
    pub tire_kpa: f64,
    pub wheel_r: f64,
    pub cw_a: f64,
    pub cl_a: f64,
    pub brake: Brake,
    pub steering: Steering,
    pub chassis: Chassis,
    pub esp: Esp,
    pub tcs: bool,
    pub diff: Diff,
    pub launch: bool,
    /// Höchsttempo-Begrenzer (m/s)
    pub limiter: Option<f64>,
    pub recuperation_g: f64,
    pub drift: DriftDef,
    pub two_wheel: bool,
    pub max_lean: f64,
    pub wheelie_control: bool,
    /// Zielwerte (Rohschlüssel wie `0_100`, `vmax`, `brems_100`, `quer_g`)
    pub targets: BTreeMap<String, f64>,
    /// Spielmerkmale ohne Physik (Sirene, Fahrgäste, Burnout …)
    pub flags: BTreeMap<String, Value>,
    /// Gierträgheit (kg·m²) bei leerem Fahrzeug
    pub iz: f64,
}
impl Vehicle {
    /// Masse und Schwerpunkthöhe bei Zuladung z (0…1).
    pub fn loaded(&self, z: f64) -> (f64, f64) {
        let z = z.clamp(0., 1.);
        (
            lerp(self.mass_empty, self.mass_full, z),
            lerp(self.cg_empty, self.cg_full, z),
        )
    }
    /// Abstand Schwerpunkt → Vorder- bzw. Hinterachse (m).
    pub fn axle_distances(&self) -> (f64, f64) {
        (
            self.wheelbase * (1. - self.front),
            self.wheelbase * self.front,
        )
    }
    /// Höchsttempo aus Leistung und Widerständen (m/s), ohne Begrenzer und Übersetzung.
    pub fn power_limited_vmax(&self, mass: f64) -> f64 {
        let p = self.engine.watts * self.efficiency;
        let cr = self.tire.rolling;
        let (mut lo, mut hi) = (0.1, 200.);
        for _ in 0..60 {
            let v = (lo + hi) / 2.;
            let need = 0.5 * RHO * self.cw_a * v * v * v + cr * mass * G * v;
            if need > p {
                hi = v;
            } else {
                lo = v;
            }
        }
        lo
    }
}

/// Kalibrierte Stellschrauben je Fahrzeug (`vehicles.calibrated.json`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Calibration {
    pub cw_a: f64,
    pub mu: f64,
    pub gear: f64,
    pub shift: f64,
    pub efficiency: f64,
    pub brake: f64,
}
impl Default for Calibration {
    fn default() -> Self {
        Self {
            cw_a: 1.,
            mu: 1.,
            gear: 1.,
            shift: 1.,
            efficiency: 1.,
            brake: 1.,
        }
    }
}
impl Calibration {
    fn parse(j: &Value) -> Self {
        let f = |k: &str| j[k].as_f64().unwrap_or(1.);
        Self {
            cw_a: f("cwA"),
            mu: f("mu"),
            gear: f("uebersetzung"),
            shift: f("schalt"),
            efficiency: f("wirkungsgrad"),
            brake: f("bremskraft"),
        }
    }
    pub fn to_json(&self) -> Value {
        serde_json::json!({
            "cwA": self.cw_a, "mu": self.mu, "uebersetzung": self.gear, "schalt": self.shift,
            "wirkungsgrad": self.efficiency, "bremskraft": self.brake
        })
    }
    /// Stellschrauben anwenden (Grenzen prüft das Werkzeug).
    pub fn apply(&self, v: &Vehicle) -> Vehicle {
        let mut v = v.clone();
        v.cw_a *= self.cw_a;
        v.tire.mu *= self.mu;
        v.efficiency *= self.efficiency;
        v.gearbox.shift *= self.shift;
        v.brake.gain *= self.brake;
        // Übersetzung: alle Gänge außer dem letzten (der hält die Vmax) gestaucht bzw. gestreckt
        let n = v.gearbox.ratios.len();
        if n > 1 {
            let top = v.gearbox.ratios[n - 1];
            for r in &mut v.gearbox.ratios[..n - 1] {
                *r = top + (*r - top) * self.gear;
            }
        }
        v
    }
}

/// Alle Fahrzeugdaten.
#[derive(Debug, Clone)]
pub struct VehicleDb {
    pub tires: HashMap<String, Tire>,
    pub surfaces: HashMap<String, SurfaceDef>,
    pub curves: HashMap<String, Value>,
    pub classes: Map<String, Value>,
    pub feel: Feel,
    pub vehicles: Vec<Vehicle>,
    pub calibration: HashMap<String, Calibration>,
    /// Rohdaten nach Vererbung (für Werkzeuge und Fehlermeldungen)
    pub merged: HashMap<String, Value>,
}

/// Felder, die ein Fahrzeugeintrag tragen darf (alles andere ist ein Tippfehler).
pub const KEYS: &[&str] = &[
    "id",
    "name",
    "klasse",
    "basis",
    "vorbild",
    "quelle",
    "masse",
    "kw",
    "nm",
    "n_max",
    "lbh",
    "radstand",
    "spur",
    "cg_h",
    "vorn",
    "antrieb",
    "awd_vorn",
    "motor",
    "getriebe",
    "reifen",
    "rad_m",
    "aero",
    "bremse",
    "lenkung",
    "fahrwerk",
    "assist",
    "diff",
    "launch",
    "limiter",
    "rekuperation_g",
    "drift",
    "ziel",
    "max_schraeglage",
    "wheelie_control",
    "zweirad",
    "gelenk",
    "sirene",
    "fahrgaeste",
    "hoehe_kritisch",
    "burnout",
    "rauchfahne",
    "kalibrier_zuladung",
    "wirkungsgrad",
];
/// Spielmerkmale, die unverändert in `flags` landen.
const FLAGS: &[&str] = &[
    "sirene",
    "fahrgaeste",
    "hoehe_kritisch",
    "burnout",
    "rauchfahne",
    "gelenk",
    "vorbild",
    "quelle",
];

/// Tiefes Zusammenführen: Objekte rekursiv, alles andere ersetzt.
pub fn deep_merge(base: &mut Value, over: &Value) {
    match (base, over) {
        (Value::Object(b), Value::Object(o)) => {
            for (k, v) in o {
                match b.get_mut(k) {
                    Some(bv) if bv.is_object() && v.is_object() => deep_merge(bv, v),
                    _ => {
                        b.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        (b, o) => *b = o.clone(),
    }
}

fn num(v: &Value, path: &str) -> Result<f64> {
    v.as_f64().with_context(|| format!("{path}: Zahl erwartet"))
}
/// Zahl oder {leer, voll}.
fn empty_full(v: &Value, path: &str) -> Result<(f64, f64)> {
    if let Some(x) = v.as_f64() {
        return Ok((x, x));
    }
    Ok((num(&v["leer"], path)?, num(&v["voll"], path)?))
}

impl VehicleDb {
    /// Eingebettete Daten (beim Bauen eingelesen).
    pub fn embedded() -> Result<Self> {
        Self::from_strs(TIRES, SURFACES, CURVES, CLASSES, VEHICLES, FEEL, CALIBRATED)
    }
    /// Aus einem Ordner (Werkzeuge; dieselben Dateinamen).
    pub fn from_dir(dir: &Path) -> Result<Self> {
        let r = |n: &str| {
            std::fs::read_to_string(dir.join(n)).with_context(|| format!("{}/{n}", dir.display()))
        };
        let cal = r("vehicles.calibrated.json").unwrap_or_else(|_| "{}".into());
        Self::from_strs(
            &r("tires.json")?,
            &r("surfaces.json")?,
            &r("engine-curves.json")?,
            &r("classes.json")?,
            &r("vehicles.json")?,
            &r("feel.json")?,
            &cal,
        )
    }
    pub fn from_strs(
        tires: &str,
        surfaces: &str,
        curves: &str,
        classes: &str,
        vehicles: &str,
        feel: &str,
        calibrated: &str,
    ) -> Result<Self> {
        let p = |s: &str, n: &str| -> Result<Value> {
            serde_json::from_str(s).with_context(|| format!("{n}: kein gültiges JSON"))
        };
        let tj = p(tires, "tires.json")?;
        let mut tires = HashMap::new();
        for (id, t) in tj["reifen"]
            .as_object()
            .context("tires.json: reifen fehlt")?
        {
            let path = format!("tires.json/{id}");
            let factor = t["faktor"]
                .as_object()
                .with_context(|| format!("{path}: faktor fehlt"))?
                .iter()
                .map(|(k, v)| Ok((k.clone(), num(v, &path)?)))
                .collect::<Result<_>>()?;
            let peak = t["peak_schraeglauf_grad"].as_f64().unwrap_or(6.);
            let zweirad = matches!(
                id.as_str(),
                "fahrrad" | "rennrad" | "scooter_klein" | "scooter_offroad" | "roller"
            ) || id.starts_with("motorrad")
                || id == "cruiser";
            tires.insert(
                id.clone(),
                Tire {
                    id: id.clone(),
                    mu: num(&t["mu_trocken"], &path)?,
                    mu_long: t["mu_laengs"].as_f64().unwrap_or(1.1),
                    peak_slip_angle: peak.to_radians(),
                    peak_slip_ratio: t["peak_schlupf"].as_f64().unwrap_or(0.11),
                    slide_ratio: num(&t["gleit_zu_peak"], &path)?,
                    factor,
                    // Rollwiderstand: Pkw 0,012; Fahrräder und schmale Reifen deutlich weniger, LKW-Reifen 0,006
                    rolling: t["cr"].as_f64().unwrap_or(match id.as_str() {
                        "rennrad" => 0.004,
                        "fahrrad" => 0.006,
                        "lkw" | "bus" => 0.007,
                        _ if zweirad => 0.010,
                        _ => 0.012,
                    }),
                },
            );
        }
        let sj = p(surfaces, "surfaces.json")?;
        let mut surf = HashMap::new();
        for (id, s) in sj["untergruende"]
            .as_object()
            .context("untergruende fehlt")?
        {
            let path = format!("surfaces.json/{id}");
            surf.insert(
                id.clone(),
                SurfaceDef {
                    id: id.clone(),
                    mu_rel: num(&s["mu_rel"], &path)?,
                    category: s["kategorie"].as_str().unwrap_or("trocken").into(),
                    rolling_extra: s["roll_zuschlag"].as_f64().unwrap_or(0.),
                },
            );
        }
        let cj = p(curves, "engine-curves.json")?;
        let curves: HashMap<String, Value> = cj["kurven"]
            .as_object()
            .context("kurven fehlt")?
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let classes = p(classes, "classes.json")?["klassen"]
            .as_object()
            .context("klassen fehlt")?
            .clone();
        let feel = Feel::parse(&p(feel, "feel.json")?)?;
        let calibration = p(calibrated, "vehicles.calibrated.json")?
            .as_object()
            .map(|m| {
                m.iter()
                    .map(|(k, v)| (k.clone(), Calibration::parse(v)))
                    .collect()
            })
            .unwrap_or_default();
        let raw = p(vehicles, "vehicles.json")?;
        let list = raw.as_array().context("vehicles.json: Liste erwartet")?;
        let mut by_id: HashMap<String, Value> = HashMap::new();
        let mut order = Vec::new();
        for v in list {
            let id = v["id"]
                .as_str()
                .context("vehicles.json: Eintrag ohne id")?
                .to_string();
            for k in v.as_object().context("Eintrag ist kein Objekt")?.keys() {
                ensure!(KEYS.contains(&k.as_str()), "{id}: unbekanntes Feld „{k}“");
            }
            ensure!(
                by_id.insert(id.clone(), v.clone()).is_none(),
                "{id}: doppelt"
            );
            order.push(id);
        }
        let mut db = Self {
            tires,
            surfaces: surf,
            curves,
            classes,
            feel,
            vehicles: Vec::new(),
            calibration,
            merged: HashMap::new(),
        };
        for id in &order {
            let m = db.merge(id, &by_id, 0)?;
            db.merged.insert(id.clone(), m);
        }
        for id in &order {
            let v = db
                .resolve(&db.merged[id])
                .with_context(|| format!("Fahrzeug {id}"))?;
            db.vehicles.push(v);
        }
        Ok(db)
    }

    /// Rohdaten mit Vererbung: Klassen-Preset ← Basisfahrzeug ← Eintrag; Kurzformen `kw`, `nm`, `n_max`.
    fn merge(&self, id: &str, raw: &HashMap<String, Value>, depth: usize) -> Result<Value> {
        ensure!(depth < 8, "{id}: Vererbungskette zu lang (Zyklus?)");
        let own = raw
            .get(id)
            .with_context(|| format!("Basis „{id}“ unbekannt"))?;
        let mut out = if let Some(b) = own["basis"].as_str() {
            self.merge(b, raw, depth + 1)?
        } else {
            let k = own["klasse"]
                .as_str()
                .with_context(|| format!("{id}: klasse fehlt"))?;
            self.classes
                .get(k)
                .with_context(|| format!("{id}: Klasse „{k}“ unbekannt"))?
                .clone()
        };
        if let (Some(k), Value::Object(o)) = (own["klasse"].as_str(), &mut out) {
            o.insert("klasse".into(), Value::String(k.into()));
        }
        let mut own = own.clone();
        if let Value::Object(o) = &mut own {
            o.remove("basis");
            // Kurzformen auf oberster Ebene gehören zum Motor
            for k in ["kw", "nm", "n_max"] {
                if let Some(v) = o.remove(k) {
                    let m = o
                        .entry("motor")
                        .or_insert_with(|| Value::Object(Map::new()));
                    if let Value::Object(mm) = m {
                        mm.insert(k.into(), v);
                    }
                }
            }
            // Ein Basisfahrzeug vererbt seine Ziele nicht automatisch weiter, wenn eigene gesetzt sind
        }
        if let Some(Value::Object(mo)) = own.get("motor").cloned().as_ref()
            && let (Some(t_new), Some(t_old)) = (mo.get("typ"), out["motor"].get("typ"))
            && t_new != t_old
        {
            // anderer Motortyp: Drehzahl und Lag der Klasse passen nicht mehr
            if let Value::Object(m) = &mut out["motor"] {
                m.remove("lag");
                if t_new == "elektro" || t_new == "muskel" {
                    m.remove("n_max");
                }
            }
        }
        deep_merge(&mut out, &own);
        if let Value::Object(o) = &mut out {
            o.insert("id".into(), Value::String(id.into()));
        }
        Ok(out)
    }

    fn resolve(&self, m: &Value) -> Result<Vehicle> {
        let id = m["id"].as_str().unwrap_or("?").to_string();
        let class = m["klasse"].as_str().context("klasse fehlt")?.to_string();
        let (mass_empty, mass_full) = empty_full(&m["masse"], "masse")?;
        let (cg_empty, cg_full) = empty_full(&m["cg_h"], "cg_h")?;
        ensure!(
            mass_empty > 0. && mass_full >= mass_empty,
            "masse unplausibel"
        );
        ensure!(
            cg_empty > 0. && cg_full >= cg_empty * 0.99,
            "cg_h unplausibel"
        );
        let lbh: Vec<f64> = m["lbh"]
            .as_array()
            .context("lbh fehlt")?
            .iter()
            .map(|x| num(x, "lbh"))
            .collect::<Result<_>>()?;
        ensure!(lbh.len() == 3, "lbh braucht drei Werte");
        let wheelbase = num(&m["radstand"], "radstand")?;
        let two_wheel = m["zweirad"].as_bool().unwrap_or(false);
        let track = if two_wheel {
            0.
        } else {
            num(&m["spur"], "spur")?
        };
        let front = num(&m["vorn"], "vorn")?;
        ensure!((0.05..0.95).contains(&front), "vorn außerhalb 0,05…0,95");
        let drive = match m["antrieb"].as_str().unwrap_or("RWD") {
            "FWD" => Drive::Fwd,
            "RWD" => Drive::Rwd,
            "AWD" => Drive::Awd,
            x => bail!("antrieb „{x}“ unbekannt (FWD/RWD/AWD)"),
        };
        let awd_front = m["awd_vorn"].as_f64().unwrap_or(0.4).clamp(0., 1.);
        let tm = &m["reifen"];
        let tire_id = tm["typ"].as_str().context("reifen.typ fehlt")?;
        let tire = self
            .tires
            .get(tire_id)
            .with_context(|| format!("Reifen „{tire_id}“ unbekannt"))?
            .clone();
        let wheel_r = match tm["zoll"].as_f64() {
            // kleines Rad: Felge plus Reifenflanke
            Some(z) => z * 0.0254 / 2. * 1.3,
            None => m["rad_m"].as_f64().unwrap_or(0.31),
        };
        let mm = &m["motor"];
        let kind = mm["typ"].as_str().context("motor.typ fehlt")?.to_string();
        let curve = self
            .curves
            .get(&kind)
            .with_context(|| format!("Motorkurve „{kind}“ unbekannt"))?;
        let efficiency = m["wirkungsgrad"].as_f64().unwrap_or(match (&*kind, drive) {
            ("elektro", _) => 0.92,
            ("muskel", _) => 0.95,
            ("diesel_lkw", _) => 0.88,
            (_, Drive::Fwd) => 0.90,
            (_, Drive::Rwd) => 0.88,
            (_, Drive::Awd) => 0.85,
        });
        let targets: BTreeMap<String, f64> = m["ziel"]
            .as_object()
            .map(|o| {
                o.iter()
                    .filter_map(|(k, v)| v.as_f64().map(|x| (k.clone(), x)))
                    .collect()
            })
            .unwrap_or_default();
        let calib_load = m["kalibrier_zuladung"].as_f64().unwrap_or(
            if matches!(class.as_str(), "lkw" | "lkw_sattel" | "bus" | "bus_gelenk") {
                1.
            } else {
                0.
            },
        );
        let aero = &m["aero"];
        let cw_a = num(&aero["cwA"], "aero.cwA")?;
        let cl_a = aero["clA"].as_f64().unwrap_or(0.);
        let limiter = m["limiter"].as_f64().map(|k| k / 3.6);
        // Motor
        let (engine, n_power) = if kind == "muskel" {
            let w_cont = num(&mm["w_dauer"], "motor.w_dauer")?;
            let w_sprint = mm["w_sprint"].as_f64().unwrap_or(w_cont * 3.);
            let f = curve["kraft_max_n"].as_f64().unwrap_or(300.);
            (
                Engine {
                    kind: kind.clone(),
                    power: Power::Muscle {
                        force_max: f,
                        w_cont,
                        w_sprint,
                    },
                    watts: w_sprint,
                    nm: 0.,
                    n_max: 0.,
                    n_peak_power: 0.,
                    n_idle: 0.,
                    peak_watts: w_sprint,
                    band_from: None,
                    lag: 0.,
                },
                0.,
            )
        } else {
            let kw = num(&mm["kw"], "motor.kw")?;
            ensure!(kw > 0., "motor.kw muss positiv sein");
            if kind == "elektro" {
                let corner = curve["eck_anteil_vmax"].as_f64().unwrap_or(0.35);
                (
                    Engine {
                        kind: kind.clone(),
                        power: Power::Electric {
                            corner_frac: corner,
                        },
                        watts: kw * 1000.,
                        nm: mm["nm"].as_f64().unwrap_or(0.),
                        n_max: 0.,
                        n_peak_power: 0.,
                        n_idle: 0.,
                        peak_watts: kw * 1000.,
                        band_from: None,
                        lag: 0.,
                    },
                    0.,
                )
            } else {
                let pts: Vec<(f64, f64)> = curve
                    .as_array()
                    .context("Motorkurve: Stützpunkte erwartet")?
                    .iter()
                    .filter_map(|p| Some((p[0].as_f64()?, p[1].as_f64()?)))
                    .collect();
                ensure!(pts.len() >= 2, "Motorkurve „{kind}“ braucht Stützpunkte");
                let n_max = num(&mm["n_max"], "motor.n_max")?;
                // Spitzenmoment ableiten, falls nicht angegeben: so, dass die Kurve genau die Spitzenleistung erreicht
                let shape_power = |n: f64| curve_at(&pts, n / n_max) * n * 2. * PI / 60.;
                let (mut best_n, mut best) = (n_max, 0.);
                for k in 1..=200 {
                    let n = n_max * k as f64 / 200.;
                    let pw = shape_power(n);
                    if pw > best {
                        (best, best_n) = (pw, n);
                    }
                }
                let nm = mm["nm"].as_f64().unwrap_or(kw * 1000. / best);
                let mut e = Engine {
                    kind: kind.clone(),
                    power: Power::Curve(pts.clone()),
                    watts: kw * 1000.,
                    nm,
                    n_max,
                    n_peak_power: best_n,
                    n_idle: (n_max * 0.13).clamp(500., 1100.),
                    peak_watts: kw * 1000.,
                    band_from: None,
                    lag: mm["lag"].as_f64().unwrap_or(0.),
                };
                // reicht die Kurve nicht für die Spitzenleistung, wird sie oben verbreitert
                let peak = |e: &Engine| {
                    (1..=400)
                        .map(|k| {
                            let n = n_max * k as f64 / 400.;
                            e.torque(n) * n * 2. * PI / 60.
                        })
                        .fold(0., f64::max)
                };
                if peak(&e) < e.watts * 0.98 {
                    e.band_from = pts.iter().find(|p| p.1 >= 0.999).map(|p| p.0);
                }
                // Drehzahl der tatsächlichen Spitzenleistung (mit der Leistungsgrenze)
                let (mut bn, mut bp) = (n_max, 0.);
                for k in 1..=200 {
                    let n = n_max * k as f64 / 200.;
                    let pw = e.torque(n) * n * 2. * PI / 60.;
                    if pw > bp * 1.0005 {
                        (bp, bn) = (pw, n);
                    }
                }
                e.n_peak_power = bn;
                e.peak_watts = bp;
                (e, bn)
            }
        };
        // Vmax (Ziel oder aus der Leistung) für die Übersetzungen
        let (mass_c, _) = (
            lerp(mass_empty, mass_full, calib_load),
            lerp(cg_empty, cg_full, calib_load),
        );
        let mut veh_tmp = Vehicle {
            id: id.clone(),
            name: m["name"].as_str().unwrap_or(&id).to_string(),
            class: class.clone(),
            model: m["vorbild"].as_str().unwrap_or("").to_string(),
            mass_empty,
            mass_full,
            cg_empty,
            cg_full,
            calib_load,
            length: lbh[0],
            width: lbh[1],
            height: lbh[2],
            wheelbase,
            track,
            front,
            drive,
            awd_front,
            engine,
            gearbox: Gearbox {
                kind: String::new(),
                ratios: vec![1.],
                shift: 0.,
                cvt: false,
            },
            efficiency,
            tire,
            tire_width_mm: tm["breite_mm"].as_f64().unwrap_or(205.),
            tire_kpa: tm["kpa"].as_f64().unwrap_or(240.),
            wheel_r,
            cw_a,
            cl_a,
            brake: Brake {
                kind: String::new(),
                abs: true,
                front: 0.7,
                build: 0.1,
                fading: 0.,
                retarder: None,
                gain: 1.3,
            },
            steering: Steering {
                turning_circle: 11.,
                delta_max: 0.6,
                v_s: 22.,
                delta_hs: 0.03,
                rear: false,
                rate_factor: 1.,
            },
            chassis: Chassis {
                roll: 3.,
                pitch: 1.,
                hz: 1.4,
                tau: 0.15,
            },
            esp: Esp::Full,
            tcs: true,
            diff: Diff::Open,
            launch: m["launch"].as_bool().unwrap_or(false),
            limiter,
            recuperation_g: m["rekuperation_g"]
                .as_f64()
                .unwrap_or(if kind == "elektro" { 0.12 } else { 0. }),
            drift: DriftDef {
                ability: 0.,
                max_angle: 0.,
                rear_grip: 0.65,
            },
            two_wheel,
            max_lean: m["max_schraeglage"].as_f64().unwrap_or(40.).to_radians(),
            wheelie_control: m["wheelie_control"].as_bool().unwrap_or(false),
            targets,
            flags: FLAGS
                .iter()
                .filter_map(|k| m.get(*k).map(|v| (k.to_string(), v.clone())))
                .collect(),
            iz: 0.,
        };
        let v_target = veh_tmp
            .targets
            .get("vmax")
            .map(|k| k / 3.6)
            .unwrap_or_else(|| veh_tmp.power_limited_vmax(mass_c));
        let v_top = v_target.min(limiter.unwrap_or(f64::INFINITY));
        // Getriebe
        let gm = &m["getriebe"];
        let gkind = gm["typ"].as_str().unwrap_or("manuell").to_string();
        let gears = gm["gaenge"].as_u64().unwrap_or(5).max(1) as usize;
        let single = matches!(gkind.as_str(), "einstufig" | "cvt") || kind == "muskel";
        let ratios = match &veh_tmp.engine.power {
            Power::Curve(_) => {
                // letzter Gang: Vmax bei der Drehzahl der Spitzenleistung
                let w_peak = n_power * 2. * PI / 60.;
                let top = w_peak * wheel_r / v_top.max(1.);
                if single {
                    vec![top]
                } else {
                    // erster Gang: knapp an der Traktionsgrenze der Antriebsachse bei Spitzenmoment
                    let axle = match drive {
                        Drive::Fwd => front,
                        Drive::Rwd => 1. - front,
                        Drive::Awd => 1.,
                    };
                    let grip = axle * mass_c * G * veh_tmp.tire.mu * veh_tmp.tire.mu_long;
                    // … aber nicht weiter gespreizt als übliche Getriebe dieser Gangzahl (4 Gänge ×4, 5 ×4,75,
                    // 7 ×6,25, 12 ×10): ein schwacher Motor bekäme sonst einen absurd kurzen ersten Gang
                    let spread = 0.75 * gears as f64 + 1.;
                    let first = (grip * wheel_r / (veh_tmp.engine.nm * efficiency))
                        .max(top * 1.8)
                        .min(top * spread);
                    // progressive Stufung: oben enger (Exponent > 1 auf dem Log-Abstand)
                    (0..gears)
                        .map(|k| {
                            let x = (gears - 1 - k) as f64 / (gears - 1).max(1) as f64;
                            top * (first / top).powf(x.powf(1.2))
                        })
                        .collect()
                }
            }
            _ => vec![1.],
        };
        veh_tmp.gearbox = Gearbox {
            kind: gkind.clone(),
            ratios,
            shift: if single {
                0.
            } else {
                gm["schalt"].as_f64().unwrap_or(0.3)
            },
            cvt: gkind == "cvt",
        };
        // Bremse
        let bm = &m["bremse"];
        let bkind = bm["typ"].as_str().unwrap_or("scheibe").to_string();
        veh_tmp.brake = Brake {
            abs: bm["abs"].as_bool().unwrap_or(true),
            front: bm["vorn"].as_f64().unwrap_or(0.68).clamp(0.2, 0.95),
            build: bm["aufbau_s"].as_f64().unwrap_or(match bkind.as_str() {
                "druckluft" => 0.4,
                "trommel" => 0.15,
                _ => 0.1,
            }),
            fading: match bm["fading"].as_str() {
                Some("hoch") => 2.,
                Some("mittel") => 1.,
                _ if bkind == "keramik" => 0.,
                _ if bkind == "trommel" => 2.,
                _ => 0.3,
            },
            retarder: bm["retarder"].as_str().map(str::to_string),
            // reichlich über der Haftung: Pkw-Bremsen blockieren auf trockenem Asphalt
            gain: bm["verzoegerung_g"]
                .as_f64()
                .unwrap_or(match bkind.as_str() {
                    "druckluft" => 0.85,
                    "trommel" => 1.0,
                    _ => 1.4,
                }),
            kind: bkind,
        };
        // Lenkung: größter Radeinschlag aus dem Wendekreis
        let lm = &m["lenkung"];
        let tc = num(&lm["wendekreis"], "lenkung.wendekreis")?;
        let r = tc / 2. - track / 2.;
        ensure!(r > wheelbase * 0.6, "Wendekreis zu klein für den Radstand");
        let mut delta_max = (wheelbase / r).clamp(-1., 1.).asin();
        if let Some(l) = lm["lock_grad"].as_f64() {
            delta_max = delta_max.max(l.to_radians() * 0.9);
        }
        veh_tmp.steering = Steering {
            turning_circle: tc,
            delta_max,
            v_s: lm["v_s"].as_f64().unwrap_or(22.).max(1.),
            delta_hs: (2.5f64).to_radians(),
            rear: lm["hinterachse"].as_bool().unwrap_or(false),
            rate_factor: lm["rate_faktor"].as_f64().unwrap_or(1.),
        };
        if !two_wheel {
            let f = &m["fahrwerk"];
            veh_tmp.chassis = Chassis {
                roll: f["wank"].as_f64().unwrap_or(3.5),
                pitch: f["nick"]
                    .as_f64()
                    .unwrap_or(f["wank"].as_f64().unwrap_or(3.5) * 0.35),
                hz: f["hz"].as_f64().unwrap_or(1.4),
                tau: f["tau"].as_f64().unwrap_or(0.15).max(0.02),
            };
        } else {
            veh_tmp.chassis = Chassis {
                roll: 0.,
                pitch: 0.,
                hz: 1.8,
                tau: 0.08,
            };
        }
        let am = &m["assist"];
        veh_tmp.esp = match am["esp"].as_str().unwrap_or("voll") {
            "aus" => Esp::Off,
            "sport" => Esp::Sport,
            "voll" => Esp::Full,
            x => bail!("assist.esp „{x}“ unbekannt (aus/sport/voll)"),
        };
        veh_tmp.tcs = am["tcs"].as_bool().unwrap_or(veh_tmp.esp != Esp::Off);
        veh_tmp.diff = match m["diff"].as_str().unwrap_or("offen") {
            "offen" => Diff::Open,
            "sperre" => Diff::Lsd,
            "starr" => Diff::Locked,
            "vectoring" => Diff::Vectoring,
            x => bail!("diff „{x}“ unbekannt"),
        };
        let dm = &m["drift"];
        veh_tmp.drift = DriftDef {
            ability: dm["faehigkeit"].as_f64().unwrap_or(0.).clamp(0., 1.),
            max_angle: dm["max_winkel"].as_f64().unwrap_or(40.).to_radians(),
            rear_grip: dm["grip_hinten"]
                .as_f64()
                .unwrap_or(0.75 - 0.2 * dm["faehigkeit"].as_f64().unwrap_or(0.)),
        };
        // Gierträgheit: Iz = m·a·b·1,05
        let (a, b) = veh_tmp.axle_distances();
        veh_tmp.iz = mass_empty * a * b * 1.05;
        ensure!(veh_tmp.iz > 0., "Gierträgheit nicht ableitbar");
        Ok(veh_tmp)
    }

    pub fn get(&self, id: &str) -> Option<&Vehicle> {
        self.vehicles.iter().find(|v| v.id == id)
    }
    /// Fahrzeug mit angewandter Kalibrierung (falls vorhanden).
    pub fn calibrated(&self, id: &str) -> Option<Vehicle> {
        let v = self.get(id)?;
        Some(
            self.calibration
                .get(id)
                .copied()
                .unwrap_or_default()
                .apply(v),
        )
    }
    pub fn surface(&self, id: &str) -> Result<&SurfaceDef> {
        self.surfaces
            .get(id)
            .ok_or_else(|| anyhow!("Untergrund „{id}“ unbekannt"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_vehicles_load_with_plausible_derived_values() {
        let db = VehicleDb::embedded().expect("Daten laden");
        assert!(db.vehicles.len() >= 85, "{}", db.vehicles.len());
        for v in &db.vehicles {
            assert!(
                v.iz > 0. && v.wheel_r > 0.05 && v.efficiency > 0.8,
                "{}",
                v.id
            );
            assert!(
                v.steering.delta_max > 0.1 && v.steering.delta_max < 1.2,
                "{}: {}",
                v.id,
                v.steering.delta_max
            );
            if let Power::Curve(_) = v.engine.power {
                let r = &v.gearbox.ratios;
                assert!(
                    r.windows(2).all(|w| w[0] >= w[1]),
                    "{}: Gänge fallen {r:?}",
                    v.id
                );
                // Spitzenleistung und Spitzenmoment stimmen immer: P(n) erreicht kw, T(n) ≤ nm
                let pk = (1..=400)
                    .map(|k| {
                        let n = v.engine.n_max * k as f64 / 400.;
                        v.engine.torque(n) * n * 2. * PI / 60.
                    })
                    .fold(0., f64::max);
                assert!(pk <= v.engine.watts * 1.0001, "{}: {pk}", v.id);
                assert!(
                    (pk - v.engine.peak_watts).abs() < v.engine.watts * 0.01,
                    "{}",
                    v.id
                );
                assert!(
                    pk >= v.engine.watts * 0.98,
                    "{}: Spitzenleistung {:.0} statt {:.0} kW",
                    v.id,
                    pk / 1e3,
                    v.engine.watts / 1e3
                );
                // und das Spitzenmoment wird erreicht (Kurvenmaximum 1,0 innerhalb der Leistungsgrenze)
                let tk = (1..=400)
                    .map(|k| v.engine.torque(v.engine.n_max * k as f64 / 400.))
                    .fold(0., f64::max);
                assert!(
                    tk <= v.engine.nm * 1.0001 && tk >= v.engine.nm * 0.9,
                    "{}: Moment {tk} von {}",
                    v.id,
                    v.engine.nm
                );
            }
        }
    }

    #[test]
    fn schema_matches_the_loader() {
        // vehicle.schema.json und der Lader kennen genau dieselben Felder
        let schema: Value =
            serde_json::from_str(include_str!("../../../data/vehicles/vehicle.schema.json"))
                .unwrap();
        let props = schema["$defs"]["fahrzeug"]["properties"]
            .as_object()
            .unwrap();
        let mut a: Vec<&str> = props.keys().map(String::as_str).collect();
        let mut b: Vec<&str> = KEYS.to_vec();
        a.sort_unstable();
        b.sort_unstable();
        assert_eq!(a, b);
    }

    #[test]
    fn five_fields_are_enough() {
        let db = VehicleDb::embedded().unwrap();
        let v = db
            .get("kompakt_fuenf_felder")
            .expect("Fünf-Felder-Fahrzeug");
        let raw = db.merged["kompakt_fuenf_felder"].clone();
        assert_eq!(v.engine.watts, 81_000.);
        assert!(
            v.engine.nm > 150. && v.engine.nm < 300.,
            "abgeleitetes Moment {}",
            v.engine.nm
        );
        assert_eq!(v.gearbox.ratios.len(), 7, "aus der Klasse");
        assert_eq!(v.tire.id, "sommer_std");
        assert!(raw["lbh"].is_array());
        // aus dem Vorbild-Eintrag in vehicles.json: genau fünf eigene Felder plus id und name
        let own: Value = serde_json::from_str::<Value>(VEHICLES)
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .find(|x| x["id"] == "kompakt_fuenf_felder")
            .unwrap()
            .clone();
        let mut keys: Vec<&str> = own
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.retain(|k| !matches!(*k, "id" | "name"));
        keys.sort();
        assert_eq!(keys, ["klasse", "kw", "masse", "ziel"]);
        assert_eq!(own["ziel"].as_object().unwrap().len(), 2);
    }

    #[test]
    fn base_vehicle_inherits_and_overrides() {
        let db = VehicleDb::embedded().unwrap();
        let base = db.get("kombi_diesel").unwrap();
        let p = db.get("polizei_kombi").unwrap();
        assert_eq!(p.engine, base.engine);
        assert_eq!(p.mass_empty, base.mass_empty);
        assert_eq!(base.esp, Esp::Full);
        assert_eq!(p.esp, Esp::Sport, "überschrieben");
        assert_eq!(p.flags.get("sirene"), Some(&Value::Bool(true)));
    }

    #[test]
    fn validation_names_the_problem() {
        let bad = |v: &str| {
            VehicleDb::from_strs(TIRES, SURFACES, CURVES, CLASSES, v, FEEL, "{}")
                .err()
                .map(|e| format!("{e:#}"))
                .unwrap_or_default()
        };
        assert!(
            bad(r#"[{"id":"x","klasse":"pkw_klein","masse":900,"kw":40,"tempo":3}]"#)
                .contains("unbekanntes Feld „tempo“")
        );
        assert!(
            bad(r#"[{"id":"x","klasse":"raumschiff","masse":900,"kw":40}]"#)
                .contains("Klasse „raumschiff“")
        );
        assert!(bad(r#"[{"id":"x","klasse":"pkw_klein","masse":900}]"#).contains("motor.kw"));
        assert!(bad(r#"[{"id":"x","klasse":"pkw_klein","masse":900,"kw":40},{"id":"x","klasse":"pkw_klein","masse":9,"kw":4}]"#).contains("doppelt"));
        assert!(bad(r#"[{"id":"x","basis":"y"}]"#).contains("Basis „y“"));
        assert!(
            bad(r#"[{"id":"x","klasse":"pkw_klein","masse":900,"kw":40,"reifen":{"typ":"holz"}}]"#)
                .contains("Reifen „holz“")
        );
    }

    #[test]
    fn magic_formula_peaks_where_the_tire_says() {
        let db = VehicleDb::embedded().unwrap();
        for t in db.tires.values() {
            let (b, c) = t.shape();
            let f = |a: f64| (c * (b * a).atan()).sin();
            let peak = t.peak_slip_angle;
            assert!((f(peak) - 1.).abs() < 1e-9, "{}", t.id);
            assert!(f(peak * 0.7) < 1. && f(peak * 1.3) < 1.);
            // weit jenseits: Gleitreibung
            assert!(
                (f(1.5) - t.slide_ratio).abs() < 0.05,
                "{}: {}",
                t.id,
                f(1.5)
            );
        }
    }
}
