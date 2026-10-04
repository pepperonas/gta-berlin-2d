//! Statistik (Port von `stats.js`): Zähler je Spiel und über alle Spiele, gespeist aus den Ereignissen der Simulation
//! und einer Messung je Schritt (Strecke, Zeit, Tempo). Rein rechnerisch; gespeichert wird im Spiel als JSON.
//! Zähler, deren Quelle in der nativen Fassung noch fehlt (Kampf, Nahverkehr, Teleport, Konsole), bleiben 0 und
//! werden nicht angezeigt.
use crate::events::Event;
use crate::world::World;
use serde_json::{Map, Value};
use std::collections::BTreeMap;

const PX_PER_KM: f64 = 10000.;
const SPEED_TO_KMH: f64 = 0.36;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fmt {
    Km,
    Kmh,
    Time,
    Eur,
    N,
}

/// Anzeige: Abschnitte mit Einträgen (Schlüssel, Beschriftung, Format); nur, was die native Fassung zählt.
/// Eintrag: Schlüssel, Beschriftung, Format.
pub type Row = (&'static str, &'static str, Fmt);
pub const SECTIONS: &[(&str, &[Row])] = &[
    (
        "Unterwegs",
        &[
            ("kmTotal", "Strecke gesamt", Fmt::Km),
            ("kmCar", "davon im Auto", Fmt::Km),
            ("kmFoot", "davon zu Fuß", Fmt::Km),
            ("topKmh", "Höchstgeschwindigkeit", Fmt::Kmh),
            ("timePlayed", "Spielzeit", Fmt::Time),
            ("timeCar", "Zeit im Auto", Fmt::Time),
            ("bridges", "Brücken befahren", Fmt::N),
            ("aquaplanes", "Aquaplaning", Fmt::N),
        ],
    ),
    (
        "Verkehr",
        &[
            ("pedsRunOver", "Menschen überfahren", Fmt::N),
            ("crashes", "Unfälle", Fmt::N),
            ("bollards", "Poller umgefahren", Fmt::N),
            ("carjacks", "Autos geklaut", Fmt::N),
            ("carsEntered", "Autos gefahren", Fmt::N),
            ("ownWrecks", "eigene Autos Schrott", Fmt::N),
        ],
    ),
    (
        "Aufträge",
        &[
            ("missions", "Aufträge erledigt", Fmt::N),
            ("missionsFailed", "Aufträge verpatzt", Fmt::N),
            ("moneyEarned", "Geld verdient", Fmt::Eur),
        ],
    ),
];
/// Rekorde: Höchstwert, nicht Summe.
const MAX_KEYS: &[&str] = &["topKmh"];

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Stats(pub BTreeMap<String, f64>);

impl Stats {
    pub fn get(&self, k: &str) -> f64 {
        self.0.get(k).copied().unwrap_or(0.)
    }
    fn add(&mut self, k: &str, n: f64) {
        let v = self.0.entry(k.to_owned()).or_insert(0.);
        *v = if MAX_KEYS.contains(&k) {
            v.max(n)
        } else {
            *v + n
        };
    }
    /// JSON wie `stats.js` (Schlüssel → Zahl, `v: 1`).
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("v".into(), 1.into());
        for (k, v) in &self.0 {
            m.insert(k.clone(), Value::from(*v));
        }
        Value::Object(m)
    }
    /// Fremden/alten Stand übernehmen: nur endliche, nicht negative Zahlen (normalizeStats).
    pub fn from_json(v: &Value) -> Self {
        let mut s = Self::default();
        if let Some(o) = v.as_object() {
            for (k, x) in o {
                if k != "v"
                    && let Some(n) = x.as_f64().filter(|n| n.is_finite() && *n >= 0.)
                {
                    s.0.insert(k.clone(), n);
                }
            }
        }
        s
    }
    /// Zwei Stände zusammenzählen (Rekorde: Höchstwert).
    pub fn merge(&self, other: &Stats) -> Stats {
        let mut s = self.clone();
        for (k, v) in &other.0 {
            s.add(k, *v);
        }
        s
    }
}

/// Merkt sich je Welt die letzte Lage für Strecke und Übergänge.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Tracker {
    last: Option<(f64, f64)>,
    in_car: Option<u32>,
    lvl: i8,
    money: Option<f64>,
    aqua: bool,
}

/// Ein Simulationsschritt in alle Stände (dieses Spiel, insgesamt) buchen.
pub fn track_step(sets: &mut [&mut Stats], tr: &mut Tracker, w: &World, dt: f64) {
    let mut add = |k: &str, n: f64| {
        for s in sets.iter_mut() {
            s.add(k, n);
        }
    };
    let p = &w.player;
    let car = w.player_car();
    add("timePlayed", dt);
    if car.is_some() {
        add("timeCar", dt);
    }
    // Strecke: Sprünge (Neustart, Laden) zählen nicht
    if let Some((x, y)) = tr.last {
        let d = (p.x - x).hypot(p.y - y);
        if d < 600. {
            add("kmTotal", d / PX_PER_KM);
            add(
                if car.is_some() { "kmCar" } else { "kmFoot" },
                d / PX_PER_KM,
            );
        }
    }
    if let Some(c) = car {
        add("topKmh", c.speed() * SPEED_TO_KMH);
    }
    if p.in_car.is_some() && p.in_car != tr.in_car {
        add("carsEntered", 1.);
    }
    let lvl = car.map_or(p.level.lvl, |c| c.lvl());
    if lvl >= 1 && tr.lvl < 1 {
        add("bridges", 1.);
    }
    let aqua = car.is_some_and(|c| c.aqua > 0.);
    if aqua && !tr.aqua {
        add("aquaplanes", 1.);
    }
    if let Some(m) = tr.money
        && w.money > m
    {
        add("moneyEarned", w.money - m);
    }
    for e in &w.events {
        match *e {
            Event::Hit { player: true, .. } => add("pedsRunOver", 1.),
            Event::Crash { car, strength, .. } if Some(car) == p.in_car && strength > 0.15 => {
                add("crashes", 1.)
            }
            Event::Knock { car, .. } if Some(car) == p.in_car => add("bollards", 1.),
            Event::Carjack { .. } => add("carjacks", 1.),
            Event::Wreck { car, .. } if Some(car) == w.player_car_id || Some(car) == p.in_car => {
                add("ownWrecks", 1.)
            }
            Event::MissionSuccess => add("missions", 1.),
            Event::MissionFail => add("missionsFailed", 1.),
            _ => {}
        }
    }
    *tr = Tracker {
        last: Some((p.x, p.y)),
        in_car: p.in_car,
        lvl,
        money: Some(w.money),
        aqua,
    };
}

/// Wert für die Anzeige (formatStat).
pub fn format_stat(v: f64, f: Fmt) -> String {
    let group = |n: i64| {
        let s = n.unsigned_abs().to_string();
        let mut out = String::new();
        for (i, ch) in s.chars().enumerate() {
            if i > 0 && (s.len() - i).is_multiple_of(3) {
                out.push('.');
            }
            out.push(ch);
        }
        if n < 0 { format!("-{out}") } else { out }
    };
    match f {
        Fmt::Km if v < 1. => format!("{} m", (v * 1000.).round()),
        Fmt::Km => format!("{:.*} km", if v < 10. { 2 } else { 1 }, v).replace('.', ","),
        Fmt::Kmh => format!("{} km/h", v.round()),
        Fmt::Time => {
            let (h, m, s) = ((v / 3600.) as u64, (v / 60.) as u64 % 60, v as u64 % 60);
            if h > 0 {
                format!("{h} h {m:02} min")
            } else if m > 0 {
                format!("{m} min {s:02} s")
            } else {
                format!("{s} s")
            }
        }
        Fmt::Eur => format!("{} €", group(v.round() as i64)),
        Fmt::N => group(v.round() as i64),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn formats_like_the_browser() {
        assert_eq!(format_stat(0.4567, Fmt::Km), "457 m");
        assert_eq!(format_stat(3.456, Fmt::Km), "3,46 km");
        assert_eq!(format_stat(12.34, Fmt::Km), "12,3 km");
        assert_eq!(format_stat(87.6, Fmt::Kmh), "88 km/h");
        assert_eq!(format_stat(42., Fmt::Time), "42 s");
        assert_eq!(format_stat(125., Fmt::Time), "2 min 05 s");
        assert_eq!(format_stat(3725., Fmt::Time), "1 h 02 min");
        assert_eq!(format_stat(12345.4, Fmt::Eur), "12.345 €");
        assert_eq!(format_stat(1500., Fmt::N), "1.500");
    }
    #[test]
    fn records_take_the_maximum_and_json_roundtrips() {
        let mut a = Stats::default();
        a.add("topKmh", 80.);
        a.add("topKmh", 60.);
        a.add("crashes", 2.);
        let mut b = Stats::default();
        b.add("topKmh", 120.);
        b.add("crashes", 3.);
        let m = a.merge(&b);
        assert_eq!((m.get("topKmh"), m.get("crashes")), (120., 5.));
        let j = m.to_json();
        assert_eq!(j["v"], 1);
        let mut bad = j.clone();
        bad["crashes"] = Value::from(-1.);
        bad["fremd"] = Value::from("x");
        assert_eq!(Stats::from_json(&j), m);
        let n = Stats::from_json(&bad);
        assert_eq!(n.get("crashes"), 0.);
        assert!(!n.0.contains_key("fremd"));
    }
}
