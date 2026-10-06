//! Öffentlicher Verkehr aus dem VBB-Fahrplan (Port von `transit.js`, Daten `transit.json` aus `tools/osm/transit.mjs`).
//! Die Spieluhr läuft 60× schneller, als die Fahrzeuge fahren. Deshalb bestimmt der Fahrplan nur den TAKT je Uhrzeit
//! (Abfahrten pro Stunde an diesem Wochentag); die Fahrzeuge selbst fahren in Echtzeit mit den Fahr- und Haltezeiten
//! des Fahrplans. Jeder Fahrtverlauf („Muster“) nahe der Kamera führt eine Liste virtueller Fahrzeuge, die nur aus
//! ihrer Fahrzeit τ bestehen – so laufen sie auch dort, wo die Karte gerade nicht geladen ist.
use crate::city::{Pt, seg_dist2, undelta};
use crate::collision::{Rect, SpatialHash};
use crate::math::hash01;
use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

/// px um die Kamera, in denen Muster verfolgt werden
pub const TRACK: f64 = 12000.;
/// s zwischen zwei Suchen nach Mustern
pub const EVERY: f64 = 1.;
/// px: so nah an der Kamera werden Busse zu echten Fahrzeugen
pub const BUS_LIVE: f64 = 2200.;
pub const BUS_L: f64 = 120.;
pub const BUS_W: f64 = 25.;
pub const BUS_COLOR: u32 = 0xf0cf1f;
/// S- und U-Bahn laufen gerafft: ihre Fahrplanuhr geht so viel schneller als die echte Zeit. Tempo, Fahrzeit und
/// der Countdown der Anzeigen (in Fahrplanminuten) folgen daraus; die Anzeige zählt also schneller herunter.
pub const RAIL_PACE: f64 = 3.;
/// S- und U-Bahn fahren zusätzlich dichter: Abfahrten je echter Zeit gegenüber dem Fahrplan.
pub const RAIL_TAKT: f64 = 6.;
/// Haltezeit von S- und U-Bahn in echten Sekunden.
pub const RAIL_DWELL_S: f64 = 8.;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Mode {
    Bus,
    Tram,
    SBahn,
    UBahn,
    Other,
}
impl Mode {
    pub fn parse(s: &str) -> Self {
        match s {
            "bus" => Mode::Bus,
            "tram" => Mode::Tram,
            "sbahn" => Mode::SBahn,
            "ubahn" => Mode::UBahn,
            _ => Mode::Other,
        }
    }
    /// Haltezeit in Fahrplansekunden (S/U: `RAIL_DWELL_S` echte Sekunden)
    pub fn dwell(self) -> f64 {
        match self {
            Mode::Bus => 12.,
            Mode::Tram => 15.,
            Mode::SBahn | Mode::UBahn => RAIL_DWELL_S * RAIL_PACE,
            Mode::Other => 15.,
        }
    }
    /// Fahrplansekunden je echter Sekunde
    pub fn pace(self) -> f64 {
        if self.rail() { RAIL_PACE } else { 1. }
    }
    /// Abfahrten je echter Zeit gegenüber dem Fahrplan
    pub fn takt(self) -> f64 {
        if self.rail() { RAIL_TAKT } else { 1. }
    }
    /// Wagen: Anzahl, Länge, Breite, Lücke (px)
    pub fn train(self) -> (usize, f64, f64, f64) {
        match self {
            Mode::Tram => (3, 100., 24., 4.),
            Mode::SBahn => (6, 180., 30., 6.),
            _ => (6, 160., 26., 5.),
        }
    }
    pub fn train_len(self) -> f64 {
        let (n, l, _, g) = self.train();
        n as f64 * l + (n - 1) as f64 * g
    }
    pub fn rail(self) -> bool {
        matches!(self, Mode::SBahn | Mode::UBahn)
    }
}

#[derive(Debug, Clone)]
pub struct Shape {
    pub pts: Vec<Pt>,
    pub cum: Vec<f64>,
    pub len: f64,
}

#[derive(Debug, Clone)]
pub struct Pattern {
    pub id: usize,
    pub name: String,
    pub mode: Mode,
    pub color: String,
    pub shape: usize,
    /// Bogenlänge der Halte (px)
    pub stops: Vec<f64>,
    pub stop_names: Vec<String>,
    /// Abfahrt ab dem ersten Halt (s)
    pub off: Vec<f64>,
    /// Abfahrten je Tagesart (Mo–Fr, Sa, So) in Minuten seit Mitternacht
    pub deps: [Vec<f64>; 3],
    pub dwell: f64,
    pub duration: f64,
}

/// Abschnitt eines Linienwegs im räumlichen Index.
#[derive(Debug, Clone, Copy)]
struct Seg {
    shape: usize,
    tram: bool,
    a: Pt,
    b: Pt,
}

pub struct Transit {
    pub patterns: Vec<Pattern>,
    pub shapes: Vec<Shape>,
    pub attribution: String,
    by_shape: Vec<Vec<usize>>,
    segs: Vec<Seg>,
    hash: SpatialHash,
}

fn undelta1(v: &Value) -> Vec<f64> {
    let mut out: Vec<f64> = Vec::new();
    for x in v.as_array().into_iter().flatten() {
        let d = x.as_f64().unwrap_or(0.);
        out.push(out.last().map_or(d, |l| l + d));
    }
    out
}

impl Transit {
    pub fn read(root: &Path) -> Result<Self> {
        let bytes = std::fs::read(root.join("transit.json"))
            .with_context(|| format!("{}/transit.json lesen", root.display()))?;
        Self::from_json(&serde_json::from_slice(&bytes)?)
    }
    pub fn from_json(j: &Value) -> Result<Self> {
        let shapes: Vec<Shape> = j["shapes"]
            .as_array()
            .context("shapes fehlt")?
            .iter()
            .map(|d| {
                let pts = undelta(d)?;
                let mut cum = vec![0.];
                for w in pts.windows(2) {
                    let l = cum.last().copied().unwrap_or(0.);
                    cum.push(l + (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1));
                }
                let len = *cum.last().unwrap_or(&0.);
                Ok(Shape { pts, cum, len })
            })
            .collect::<Result<_>>()?;
        let names: Vec<String> = j["names"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|n| n.as_str().unwrap_or("").to_string())
            .collect();
        let lines = j["lines"].as_array().context("lines fehlt")?;
        let mut patterns = Vec::new();
        for (id, p) in j["patterns"]
            .as_array()
            .context("patterns fehlt")?
            .iter()
            .enumerate()
        {
            let line = &lines[p["l"].as_u64().unwrap_or(0) as usize];
            let mode = Mode::parse(line[1].as_str().unwrap_or(""));
            let off: Vec<f64> = p["off"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_f64)
                .collect();
            let d = p["d"].as_array();
            let dep = |i: usize| d.and_then(|d| d.get(i)).map(undelta1).unwrap_or_default();
            patterns.push(Pattern {
                id,
                name: line[0].as_str().unwrap_or("").to_string(),
                mode,
                color: line[2].as_str().unwrap_or("").to_string(),
                shape: p["s"].as_u64().unwrap_or(0) as usize,
                stops: p["st"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_f64)
                    .collect(),
                stop_names: p["sn"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|i| {
                        names
                            .get(i.as_u64().unwrap_or(0) as usize)
                            .cloned()
                            .unwrap_or_default()
                    })
                    .collect(),
                duration: off.last().copied().unwrap_or(0.),
                off,
                deps: [dep(0), dep(1), dep(2)],
                dwell: mode.dwell(),
            });
        }
        let mut by_shape = vec![Vec::new(); shapes.len()];
        for p in &patterns {
            if let Some(v) = by_shape.get_mut(p.shape) {
                v.push(p.id);
            }
        }
        let mut hash = SpatialHash::new(3200.);
        let mut segs = Vec::new();
        for (si, list) in by_shape.iter().enumerate() {
            if list.is_empty() {
                continue;
            }
            let tram = list.iter().any(|&i| patterns[i].mode == Mode::Tram);
            for w in shapes[si].pts.windows(2) {
                let (a, b) = (w[0], w[1]);
                let r = Rect::new(
                    a.0.min(b.0),
                    a.1.min(b.1),
                    (b.0 - a.0).abs(),
                    (b.1 - a.1).abs(),
                );
                hash.insert(segs.len() as u32, &r);
                segs.push(Seg {
                    shape: si,
                    tram,
                    a,
                    b,
                });
            }
        }
        Ok(Self {
            patterns,
            shapes,
            attribution: j["attribution"].as_str().unwrap_or("").to_string(),
            by_shape,
            segs,
            hash,
        })
    }
    pub fn shape_of(&self, p: &Pattern) -> &Shape {
        &self.shapes[p.shape]
    }
    /// Muster, deren Weg nahe (x, y) verläuft (aufsteigend nach ID).
    pub fn patterns_near(&mut self, x: f64, y: f64, r: f64) -> Vec<usize> {
        let mut hits = Vec::new();
        self.hash.query(&Rect::around(x, y, r), &mut hits);
        let mut out: Vec<usize> = hits
            .into_iter()
            .flat_map(|h| self.by_shape[self.segs[h as usize].shape].iter().copied())
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    }
    /// Liegt ein Straßenbahngleis näher als r an (x, y)? (Die Karte kennt keine Tramgleise, nur der Fahrplan.)
    pub fn tram_track_near(&mut self, x: f64, y: f64, r: f64) -> bool {
        let mut hits = Vec::new();
        self.hash.query(&Rect::around(x, y, r), &mut hits);
        hits.into_iter().any(|h| {
            let s = self.segs[h as usize];
            s.tram && seg_dist2(x, y, s.a.0, s.a.1, s.b.0, s.b.1) < r * r
        })
    }
    /// Richtung (rad) des nächsten Straßenbahngleises näher als r an (x, y).
    pub fn tram_track_angle(&mut self, x: f64, y: f64, r: f64) -> Option<f64> {
        let mut hits = Vec::new();
        self.hash.query(&Rect::around(x, y, r), &mut hits);
        hits.into_iter()
            .map(|h| self.segs[h as usize])
            .filter(|s| s.tram)
            .map(|s| (seg_dist2(x, y, s.a.0, s.a.1, s.b.0, s.b.1), s))
            .filter(|(d2, _)| *d2 < r * r)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, s)| (s.b.1 - s.a.1).atan2(s.b.0 - s.a.0))
    }
    /// Abschnitte von Straßenbahnwegen im Rechteck (je Weg einmal).
    pub fn tram_segments(&mut self, r: &Rect) -> Vec<(Pt, Pt)> {
        let mut hits = Vec::new();
        self.hash.query(r, &mut hits);
        hits.sort_unstable();
        hits.into_iter()
            .map(|h| self.segs[h as usize])
            .filter(|s| s.tram)
            .map(|s| (s.a, s.b))
            .collect()
    }
    /// Punkt und Richtung bei Bogenlänge s.
    pub fn point_on(&self, p: &Pattern, s: f64) -> (f64, f64, f64) {
        point_on_shape(self.shape_of(p), s)
    }
}

/// Punkt auf einem Linienweg (binäre Suche über die Bogenlängen): x, y, Winkel.
pub fn point_on_shape(sh: &Shape, s: f64) -> (f64, f64, f64) {
    let pts = &sh.pts;
    if pts.len() < 2 {
        return pts.first().map_or((0., 0., 0.), |p| (p.0, p.1, 0.));
    }
    let s = s.clamp(0., sh.len);
    let i = match sh.cum.binary_search_by(|c| c.total_cmp(&s)) {
        Ok(i) => i.min(pts.len() - 2),
        Err(i) => i.saturating_sub(1).min(pts.len() - 2),
    };
    let (a, b) = (pts[i], pts[i + 1]);
    let l = sh.cum[i + 1] - sh.cum[i];
    let u = if l > 0. { (s - sh.cum[i]) / l } else { 0. };
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    (a.0 + dx * u, a.1 + dy * u, dy.atan2(dx))
}

/// Tagesart aus dem Wochentag (0 = Mo): Mo–Fr, Sa, So.
pub fn day_type(day: u32) -> usize {
    match day % 7 {
        5 => 1,
        6 => 2,
        _ => 0,
    }
}

/// Abfahrten je Stunde um die Uhrzeit (±30 min; Fahrten nach Mitternacht zählen zum Vortag).
pub fn departures_per_hour(p: &Pattern, minutes: f64, day: u32) -> usize {
    let m = minutes.rem_euclid(1440.);
    let (dt, prev) = (day_type(day), day_type(day + 6));
    p.deps[dt]
        .iter()
        .filter(|&&d| d >= m - 30. && d < m + 30.)
        .count()
        + p.deps[prev]
            .iter()
            .filter(|&&d| d - 1440. >= m - 30. && d - 1440. < m + 30.)
            .count()
}

/// Lage zur Fahrzeit τ (s seit Abfahrt am ersten Halt).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pos {
    pub s: f64,
    /// Index des nächsten bzw. aktuellen Halts
    pub stop: usize,
    pub dwelling: bool,
    pub done: bool,
    /// Tempo (px je echter Sekunde), 0 beim Halten
    pub v: f64,
}
/// Anteil der Fahrzeit zwischen zwei Halten, in dem S- und U-Bahn anfahren bzw. bremsen (je Ende).
pub const RAMP: f64 = 0.25;
/// Fahrprofil zwischen zwei Halten (Zeitanteil u ∈ 0…1): zurückgelegter Wegeanteil und Tempo relativ zum Mittel.
/// Bahnen fahren trapezförmig (gleichmäßig anfahren, fahren, bremsen); die Fahrzeit bleibt die des Fahrplans.
/// Bus und Straßenbahn fahren weiter gleichmäßig (die Straßenbahn bremst ohnehin vor Hindernissen).
pub fn run_profile(mode: Mode, u: f64) -> (f64, f64) {
    let u = u.clamp(0., 1.);
    if !mode.rail() {
        return (u, 1.);
    }
    let vmax = 1. / (1. - RAMP);
    if u < RAMP {
        (vmax * u * u / (2. * RAMP), vmax * u / RAMP)
    } else if u > 1. - RAMP {
        let w = 1. - u;
        (1. - vmax * w * w / (2. * RAMP), vmax * w / RAMP)
    } else {
        (vmax * (u - RAMP / 2.), vmax)
    }
}
/// Zwischen zwei Halten fährt das Fahrzeug nach `run_profile`; die letzten `dwell` Sekunden vor der Abfahrt steht es.
pub fn position_at(p: &Pattern, tau: f64) -> Pos {
    let (off, st) = (&p.off, &p.stops);
    let n = off.len().min(st.len());
    if n == 0 {
        return Pos {
            s: 0.,
            stop: 0,
            dwelling: true,
            done: true,
            v: 0.,
        };
    }
    if tau <= 0. {
        return Pos {
            s: st[0],
            stop: 0,
            dwelling: true,
            done: false,
            v: 0.,
        };
    }
    if tau >= off[n - 1] {
        return Pos {
            s: st[n - 1],
            stop: n - 1,
            dwelling: true,
            done: true,
            v: 0.,
        };
    }
    let mut i = 1;
    while i < n - 1 && off[i] <= tau {
        i += 1;
    }
    let (dep, next) = (off[i - 1], off[i]);
    let hold = if i < n - 1 {
        p.dwell.min((next - dep) * 0.4)
    } else {
        0.
    };
    let run = (next - dep - hold).max(1.);
    let u = (tau - dep) / run;
    if u >= 1. {
        return Pos {
            s: st[i],
            stop: i,
            dwelling: true,
            done: false,
            v: 0.,
        };
    }
    let (frac, rel) = run_profile(p.mode, u);
    Pos {
        s: st[i - 1] + (st[i] - st[i - 1]) * frac,
        stop: i,
        dwelling: false,
        done: false,
        v: p.mode.pace() * (st[i] - st[i - 1]) / run * rel,
    }
}
/// Fahrtempo (px je echter Sekunde), 0 beim Halten.
pub fn speed_at(_p: &Pattern, pos: &Pos) -> f64 {
    if pos.dwelling || pos.stop == 0 {
        return 0.;
    }
    pos.v
}

/// Virtuelles Fahrzeug eines Musters.
#[derive(Debug, Clone, PartialEq)]
pub struct Veh {
    pub tau: f64,
    pub delay: f64,
    pub key: String,
    /// als echtes Fahrzeug unterwegs (Bus: Auto-ID)
    pub live: Option<u32>,
    pub gone: bool,
    /// Straßenbahn: Wartezeit vor einem Hindernis, Klingel schon geläutet
    pub blocked_t: f64,
    pub rang: bool,
}
impl Veh {
    fn new(tau: f64, key: String) -> Self {
        Self {
            tau,
            delay: 0.,
            key,
            live: None,
            gone: false,
            blocked_t: 0.,
            rang: false,
        }
    }
}

/// Virtuelle Fahrzeuge beim ersten Verfolgen: gleichmäßig im aktuellen Takt verteilt (Phase fest je Muster).
/// Abstand in Fahrplanzeit: echter Abstand (Fahrplantakt verdichtet) mal Raffung.
pub fn initial_vehicles(p: &Pattern, per_hour: usize, seed: u32) -> Vec<Veh> {
    if per_hour == 0 {
        return Vec::new();
    }
    let h = 3600. * p.mode.pace() / (per_hour as f64 * p.mode.takt());
    let mut tau = hash01((p.id * 13) as f64 + seed as f64) * h;
    let mut out = Vec::new();
    while tau < p.duration {
        out.push(Veh::new(tau, format!("{}:{}:{seed}", p.id, tau.round())));
        tau += h;
    }
    out
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Track {
    pub veh: Vec<Veh>,
    pub acc: f64,
    pub n: u32,
}
/// Verfolgte Muster um die Kamera.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct State {
    pub tracked: BTreeMap<usize, Track>,
    pub last_scan: Option<f64>,
    pub seed: u32,
}

/// Verfolgte Muster fortschreiben: Suche nach Mustern nahe der Kamera (`scan`), dann `advance`.
#[allow(clippy::too_many_arguments)]
pub fn step_transit(
    st: &mut State,
    tr: &mut Transit,
    cam: Pt,
    minutes: f64,
    day: u32,
    dt: f64,
    t: f64,
    blocked: &mut dyn FnMut(&Pattern, &mut Veh, f64) -> bool,
) {
    scan(st, tr, &[cam], minutes, day, t);
    advance(st, tr, minutes, day, dt, blocked);
}

/// Einmal je Sekunde: Muster um die Kamera verfolgen (neue mit Fahrzeugen im aktuellen Takt), ferne vergessen.
/// `cams`: Kameramitten (im Koop beide Spieler); verfolgt wird, was nahe irgendeiner liegt.
pub fn scan(st: &mut State, tr: &mut Transit, cams: &[Pt], minutes: f64, day: u32, t: f64) {
    if st.last_scan.is_some_and(|l| t - l < EVERY) {
        return;
    }
    st.last_scan = Some(t);
    let mut near = tr.patterns_near(cams[0].0, cams[0].1, TRACK);
    if cams.len() > 1 {
        for c in &cams[1..] {
            near.extend(tr.patterns_near(c.0, c.1, TRACK));
        }
        near.sort_unstable();
        near.dedup();
    }
    st.tracked.retain(|id, _| near.binary_search(id).is_ok());
    for id in near {
        st.tracked.entry(id).or_insert_with(|| {
            let p = &tr.patterns[id];
            Track {
                veh: initial_vehicles(p, departures_per_hour(p, minutes, day), st.seed),
                acc: hash01((id * 7 + 1) as f64),
                n: 0,
            }
        });
    }
}

/// Fahrzeuge rücken vor, neue fahren im Takt ab, am Endhalt fallen sie weg. `blocked(p, veh, dt)` → true hält ein
/// Fahrzeug an (Straßenbahn vor einem Hindernis, Zug hinter dem Spielerzug).
pub fn advance(
    st: &mut State,
    tr: &Transit,
    minutes: f64,
    day: u32,
    dt: f64,
    blocked: &mut dyn FnMut(&Pattern, &mut Veh, f64) -> bool,
) {
    for (&id, s) in st.tracked.iter_mut() {
        let p = &tr.patterns[id];
        s.acc += dt * p.mode.takt() * departures_per_hour(p, minutes, day) as f64 / 3600.;
        if s.acc >= 1. {
            s.acc -= 1.;
            s.n += 1;
            s.veh.push(Veh::new(0., format!("{id}:n{}", s.n)));
        }
        for v in s.veh.iter_mut() {
            if v.live.is_some() {
                continue;
            }
            if blocked(p, v, dt) {
                v.delay += dt;
                continue;
            }
            v.tau += dt * p.mode.pace();
        }
        s.veh.retain(|v| !v.gone && v.tau <= p.duration + p.dwell);
    }
}

/// Wagen einer Bahn (Spitze bei s): Mitte, Winkel, Länge, Breite, erster/letzter.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CarPos {
    pub x: f64,
    pub y: f64,
    pub angle: f64,
    pub l: f64,
    pub w: f64,
    pub first: bool,
    pub last: bool,
}
pub fn train_cars(sh: &Shape, mode: Mode, s: f64) -> Vec<CarPos> {
    let (n, cl, cw, gap) = mode.train();
    (0..n)
        .map(|i| {
            let mid = s - cl / 2. - i as f64 * (cl + gap);
            let (x, y, _) = point_on_shape(sh, mid);
            let (fx, fy, _) = point_on_shape(sh, mid + cl / 2.);
            let (bx, by, _) = point_on_shape(sh, mid - cl / 2.);
            CarPos {
                x,
                y,
                angle: (fy - by).atan2(fx - bx),
                l: cl,
                w: cw,
                first: i == 0,
                last: i == n - 1,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn mini() -> Transit {
        Transit::from_json(&json!({
            "v": 1, "attribution": "VBB",
            "lines": [["M10", "tram", "#c00"], ["100", "bus", ""]],
            "names": ["A", "B", "C"],
            "shapes": [[0, 0, 1000, 0, 1000, 0]],
            "patterns": [
                { "l": 0, "s": 0, "st": [0, 1000, 2000], "sn": [0, 1, 2], "off": [0, 100, 200],
                  "d": [[600, 10, 10, 10], [], []] },
                { "l": 1, "s": 0, "st": [0, 2000], "sn": [0, 2], "off": [0, 300], "d": [[], [], []] }
            ]
        }))
        .unwrap()
    }
    #[test]
    fn timetable_positions_and_cars() {
        let mut tr = mini();
        let p = tr.patterns[0].clone();
        assert_eq!(p.mode, Mode::Tram);
        assert_eq!(p.deps[0], vec![600., 610., 620., 630.]);
        assert_eq!(departures_per_hour(&p, 610., 0), 4);
        assert_eq!(departures_per_hour(&p, 610., 5), 0, "Samstag kein Takt");
        assert_eq!(departures_per_hour(&p, 610., 6), 0);
        let a = position_at(&p, 0.);
        assert!(a.dwelling && a.s == 0.);
        let b = position_at(&p, 42.5);
        assert!(!b.dwelling && (b.s - 500.).abs() < 1e-9, "{b:?}");
        assert!(
            position_at(&p, 95.).dwelling,
            "steht vor der Abfahrt am Halt"
        );
        assert!(position_at(&p, 250.).done);
        assert!((speed_at(&p, &b) - 1000. / 85.).abs() < 1e-9);
        let (x, y, a) = tr.point_on(&p, 1500.);
        assert!((x - 1500.).abs() < 1e-9 && y == 0. && a == 0.);
        let cars = train_cars(tr.shape_of(&p), p.mode, 1500.);
        assert_eq!(cars.len(), 3);
        assert!((cars[0].x - 1450.).abs() < 1e-9 && cars[0].first && cars[2].last);
        assert_eq!(tr.patterns_near(500., 10., 50.), vec![0, 1]);
        assert!(tr.tram_track_near(500., 10., 15.));
        assert!(!tr.tram_track_near(500., 40., 15.));
        let v = initial_vehicles(&p, 60, 0);
        assert!(
            (3..=4).contains(&v.len()),
            "alle 60 s eins auf 200 s Fahrzeit"
        );
        assert!(
            v.windows(2)
                .all(|w| (w[1].tau - w[0].tau - 60.).abs() < 1e-9)
        );
        assert!(initial_vehicles(&p, 0, 0).is_empty());
    }
    #[test]
    fn stepping_departs_and_retires() {
        let mut tr = mini();
        let mut st = State::default();
        let mut never = |_: &Pattern, _: &mut Veh, _: f64| false;
        step_transit(&mut st, &mut tr, (500., 0.), 610., 0, 0.5, 0., &mut never);
        assert!(st.tracked.contains_key(&0) && st.tracked.contains_key(&1));
        // eine Stunde: vier Abfahrten im Takt (die Zählung hängt am Takt der Uhrzeit, die hier steht)
        let mut departed = 0;
        for k in 0..7200 {
            let before = st.tracked[&0].n;
            step_transit(
                &mut st,
                &mut tr,
                (500., 0.),
                610.,
                0,
                0.5,
                k as f64 * 0.5,
                &mut never,
            );
            departed += (st.tracked[&0].n - before) as usize;
        }
        assert!((3..=5).contains(&departed), "{departed}");
        assert!(
            st.tracked[&0].veh.iter().all(|v| v.tau <= 200. + 15.),
            "am Endhalt weg"
        );
        // blockiert: steht
        let mut st2 = State::default();
        let mut stop = |_: &Pattern, _: &mut Veh, _: f64| true;
        step_transit(&mut st2, &mut tr, (500., 0.), 610., 0, 1., 0., &mut stop);
        let t0: Vec<f64> = st2.tracked[&0].veh.iter().map(|v| v.tau).collect();
        step_transit(&mut st2, &mut tr, (500., 0.), 610., 0, 1., 0.5, &mut stop);
        let t1: Vec<f64> = st2.tracked[&0].veh.iter().map(|v| v.tau).collect();
        assert_eq!(t0, t1);
    }
    /// Eine U-Bahn-Linie: 3 Halte à 1000 px, 120 s Fahrplanzeit je Abschnitt, Takt alle 5 min (12/h).
    fn ubahn() -> Transit {
        let deps: Vec<f64> = std::iter::once(600.)
            .chain(std::iter::repeat_n(5., 11))
            .collect();
        Transit::from_json(&json!({
            "v": 1, "attribution": "VBB",
            "lines": [["U8", "ubahn", "#0a3c85"]],
            "names": ["A", "B", "C"],
            "shapes": [[0, 0, 2000, 0]],
            "patterns": [{ "l": 0, "s": 0, "st": [0, 1000, 2000], "sn": [0, 1, 2], "off": [0, 120, 240],
                           "d": [deps, [], []] }]
        }))
        .unwrap()
    }
    #[test]
    fn rail_runs_faster_denser_and_dwells_eight_real_seconds() {
        let mut tr = ubahn();
        let p = tr.patterns[0].clone();
        assert_eq!(p.mode.pace(), RAIL_PACE);
        assert_eq!(departures_per_hour(&p, 630., 0), 12);
        // Haltezeit: 8 echte Sekunden = 24 Fahrplansekunden
        assert_eq!(p.dwell, RAIL_DWELL_S * RAIL_PACE);
        // Tempo: 1000 px in (120 − 24) Fahrplansekunden, gerafft dreimal so schnell; in der Mitte des Abschnitts
        // fährt der Zug mit Spitzentempo (Mittel / (1 − RAMP)), an den Enden steht er fast
        let mean = 3. * 1000. / 96.;
        let pos = position_at(&p, 48.);
        assert!((speed_at(&p, &pos) - mean / (1. - RAMP)).abs() < 1e-9);
        assert!(
            speed_at(&p, &position_at(&p, 0.5)) < mean * 0.05,
            "fährt sanft an"
        );
        assert!(
            speed_at(&p, &position_at(&p, 95.5)) < mean * 0.05,
            "bremst bis fast zum Stand"
        );
        // ein Bus bleibt ungerafft
        assert_eq!(Mode::Bus.pace(), 1.);
        assert_eq!(Mode::Tram.takt(), 1.);
        // eine echte Viertelstunde: 12/h × 6 = 72/h → 18 Abfahrten; die Fahrzeit 240 s dauert 80 echte Sekunden
        let mut st = State::default();
        let mut never = |_: &Pattern, _: &mut Veh, _: f64| false;
        step_transit(&mut st, &mut tr, (500., 0.), 630., 0, 0., 0., &mut never);
        st.tracked.get_mut(&0).unwrap().veh.clear();
        let mut departed = 0;
        let mut first: Option<(f64, f64)> = None; // (echte Abfahrtzeit, Ankunft am Endhalt)
        for k in 0..(15 * 60 * 10) {
            let t = k as f64 * 0.1;
            let before = st.tracked[&0].n;
            step_transit(&mut st, &mut tr, (500., 0.), 630., 0, 0.1, t, &mut never);
            if st.tracked[&0].n > before {
                departed += 1;
                first.get_or_insert((t, f64::NAN));
            }
            if let Some(f) = first.as_mut()
                && f.1.is_nan()
                && st.tracked[&0]
                    .veh
                    .first()
                    .is_some_and(|v| position_at(&p, v.tau).done)
            {
                f.1 = t;
            }
        }
        assert!((17..=19).contains(&departed), "{departed} Abfahrten");
        let (dep, arr) = first.unwrap();
        assert!((arr - dep - 80.).abs() < 0.5, "Fahrzeit {} s", arr - dep);
        // Zugfolge beim ersten Verfolgen: 3600 · 3 / (12 · 6) = 150 Fahrplansekunden = 50 echte Sekunden
        let v = initial_vehicles(&p, 12, 0);
        assert!(
            v.windows(2)
                .all(|w| (w[1].tau - w[0].tau - 150.).abs() < 1e-9)
        );
    }
    #[test]
    fn rail_profile_accelerates_cruises_brakes_and_keeps_the_timetable() {
        // Weg 0 → 1 in der Zeit 0 → 1, stetig, Tempo als Ableitung des Wegs, Mittel 1
        let n = 10_000;
        let mut prev = 0.;
        let mut dist = 0.;
        for k in 1..=n {
            let u = k as f64 / n as f64;
            let (f, v) = run_profile(Mode::UBahn, u);
            assert!(
                f >= prev - 1e-12 && f - prev < 2. / n as f64,
                "stetig, vorwärts"
            );
            dist += v / n as f64;
            prev = f;
        }
        assert!(
            (prev - 1.).abs() < 1e-12 && (dist - 1.).abs() < 1e-3,
            "{prev} {dist}"
        );
        assert_eq!(run_profile(Mode::UBahn, 0.), (0., 0.));
        assert_eq!(run_profile(Mode::UBahn, 0.5).1, 1. / (1. - RAMP));
        assert_eq!(
            run_profile(Mode::Tram, 0.3),
            (0.3, 1.),
            "Straßenbahn gleichmäßig"
        );
    }
}
