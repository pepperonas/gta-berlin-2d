//! Statistik (Port von `stats.js`): Zähler je Spiel und über alle Spiele, gespeist aus den Ereignissen der Simulation
//! und einer Messung je Schritt (Strecke, Zeit, Tempo). Rein rechnerisch; gespeichert wird im Spiel als JSON.
//! Dazu je Waffe Schüsse/Schläge, Kugeln, Treffer und Tote (flach als `w:<id>:<feld>` gespeichert).
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
            ("teleports", "Teleports", Fmt::N),
        ],
    ),
    (
        "Verkehr",
        &[
            ("pedsRunOver", "Menschen überfahren", Fmt::N),
            ("cyclistsHit", "Radfahrer umgefahren", Fmt::N),
            ("crashes", "Unfälle", Fmt::N),
            ("bollards", "Poller umgefahren", Fmt::N),
            ("carjacks", "Autos geklaut", Fmt::N),
            ("carsEntered", "Autos gefahren", Fmt::N),
            ("ownWrecks", "eigene Autos Schrott", Fmt::N),
        ],
    ),
    (
        "Nahverkehr",
        &[
            ("rides", "Mitfahrten", Fmt::N),
            ("kmTransit", "Strecke als Fahrgast", Fmt::Km),
            ("trainsTaken", "Bahnen geführt", Fmt::N),
            ("kmTrainDriven", "Strecke als Zugführer", Fmt::Km),
            ("stopsServed", "Halte bedient", Fmt::N),
            ("tipsEarned", "Trinkgeld", Fmt::Eur),
            ("hopsOn", "aufgesprungen", Fmt::N),
            ("hopsOff", "abgesprungen", Fmt::N),
        ],
    ),
    (
        "Kampf",
        &[
            ("kills", "Menschen getötet", Fmt::N),
            ("killsShot", "davon erschossen", Fmt::N),
            ("killsMelee", "davon im Nahkampf", Fmt::N),
            ("shots", "Schüsse", Fmt::N),
            ("bullets", "Kugeln", Fmt::N),
            ("hits", "Treffer", Fmt::N),
            ("carsDestroyed", "Autos zerstört", Fmt::N),
            ("cyclistsDown", "Radfahrer vom Rad geholt", Fmt::N),
            ("bikesJacked", "Räder gekapert", Fmt::N),
            ("deaths", "selbst umgehauen", Fmt::N),
        ],
    ),
    (
        "Aufträge",
        &[
            ("missions", "Aufträge erledigt", Fmt::N),
            ("missionsFailed", "Aufträge verpatzt", Fmt::N),
            ("moneyEarned", "Geld verdient", Fmt::Eur),
            ("hospitalFees", "Krankenhauskosten", Fmt::Eur),
            ("cheats", "Konsolenbefehle (Cheats)", Fmt::N),
        ],
    ),
];
/// Zähler je Waffe (Schlüssel `w:<id>:<feld>`).
pub const WEAPON_FIELDS: [&str; 4] = ["shots", "bullets", "hits", "kills"];
pub fn weapon_key(id: &str, field: &str) -> String {
    format!("w:{id}:{field}")
}
/// Trefferquote in Prozent (Kugeln → Treffer, Nahkampf: Schläge → Treffer).
pub fn accuracy(s: &Stats, id: &str) -> f64 {
    let n = match s.get(&weapon_key(id, "bullets")) {
        b if b > 0. => b,
        _ => s.get(&weapon_key(id, "shots")),
    };
    if n > 0. {
        (s.get(&weapon_key(id, "hits")) / n * 100.).round()
    } else {
        0.
    }
}
/// Rekorde: Höchstwert, nicht Summe.
const MAX_KEYS: &[&str] = &["topKmh"];

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Stats(pub BTreeMap<String, f64>);

impl Stats {
    pub fn get(&self, k: &str) -> f64 {
        self.0.get(k).copied().unwrap_or(0.)
    }
    /// Zähler um eins erhöhen (Ereignisse außerhalb der Simulation: Teleport, Konsole).
    pub fn bump(&mut self, k: &str) {
        self.add(k, 1.);
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
impl Tracker {
    /// Geldstand übernehmen, ohne ihn als verdient zu zählen (Schummelgeld der Befehlszeile).
    pub fn set_money(&mut self, m: f64) {
        self.money = Some(m);
    }
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
    // Strecke: Sprünge (Neustart, Laden, Sprung ans Fahrtende) zählen nicht
    let ride_end = w.events.iter().any(|e| matches!(e, Event::RideEnd { .. }));
    if let Some((x, y)) = tr.last {
        let d = (p.x - x).hypot(p.y - y);
        if d < 600. && !ride_end {
            add("kmTotal", d / PX_PER_KM);
            let key = match (&car, &p.ride) {
                (Some(_), _) => "kmCar",
                (None, Some(r)) if r.kind == crate::ride::RideKind::Driver => "kmTrainDriven",
                (None, Some(_)) => "kmTransit",
                (None, None) => "kmFoot",
            };
            add(key, d / PX_PER_KM);
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
    // Geld: jedes Plus aus Aufträgen; Trinkgeld als Zugführer zählt eigens (tipsEarned)
    let tips: f64 = w
        .events
        .iter()
        .map(|e| {
            if let Event::Tip { amount } = e {
                *amount
            } else {
                0.
            }
        })
        .sum();
    if let Some(m) = tr.money
        && w.money - tips > m
    {
        add("moneyEarned", w.money - tips - m);
    }
    for e in &w.events {
        match *e {
            Event::Hit {
                player: true, bike, ..
            } => add(if bike { "cyclistsHit" } else { "pedsRunOver" }, 1.),
            Event::BikeDown { player: true, .. } => add("cyclistsDown", 1.),
            Event::Carjack { bike: true, .. } => add("bikesJacked", 1.),
            Event::Crash { car, strength, .. } if Some(car) == p.in_car && strength > 0.15 => {
                add("crashes", 1.)
            }
            Event::Knock { car, .. } if Some(car) == p.in_car => add("bollards", 1.),
            Event::Carjack { bike: false, .. } => add("carjacks", 1.),
            Event::Wreck { player: true, .. } => add("carsDestroyed", 1.),
            Event::Wreck { car, .. } if Some(car) == w.player_car_id || Some(car) == p.in_car => {
                add("ownWrecks", 1.)
            }
            Event::Board { hop, .. } => {
                add("rides", 1.);
                if hop {
                    add("hopsOn", 1.);
                }
            }
            Event::Alight { hop: true, .. } => add("hopsOff", 1.),
            Event::TrainTake { .. } => add("trainsTaken", 1.),
            Event::DoorsOpen { first: true, .. } => add("stopsServed", 1.),
            Event::Tip { amount } => add("tipsEarned", amount),
            Event::Swing {
                weapon, npc: false, ..
            } => add(&weapon_key(weapon, "shots"), 1.),
            Event::Kill {
                player: true,
                weapon,
                ..
            } => {
                add("kills", 1.);
                add(&weapon_key(weapon, "kills"), 1.);
                let melee = crate::combat::WEAPONS
                    .iter()
                    .chain([&crate::combat::KICK])
                    .find(|w| w.id == weapon)
                    .is_some_and(|w| w.melee);
                add(if melee { "killsMelee" } else { "killsShot" }, 1.);
            }
            Event::Shot {
                ref traces, weapon, ..
            } => {
                let n = traces.len().max(1) as f64;
                add("shots", 1.);
                add("bullets", n);
                add(&weapon_key(weapon, "shots"), 1.);
                add(&weapon_key(weapon, "bullets"), n);
            }
            Event::Throw { weapon, .. } => add(&weapon_key(weapon, "shots"), 1.),
            Event::WeaponHit { weapon, .. } => {
                add("hits", 1.);
                add(&weapon_key(weapon, "hits"), 1.);
            }
            Event::Wasted { .. } => add("deaths", 1.),
            Event::Respawn { fee, .. } => {
                add("hospitalFees", fee);
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
