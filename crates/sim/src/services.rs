//! Blaulichtfahrzeuge (Port von `services.js`): Tote auf der Straße → Rettungswagen mit Martinshorn, der am
//! Einsatzort hält und den Toten mitnimmt; Schüsse → Streifenwagen zum Tatort, hält dort mit Blaulicht. Dazu fährt
//! ab und zu ein Einsatz einfach vorbei. Einsatzfahrzeuge entstehen außer Sicht und fahren über den Spurgraph zum
//! Ziel (`traffic::set_goal`); über Rot fahren sie langsam.
use crate::car::Driver;
use crate::events::Event;
use crate::pedestrians::PedState;
use crate::world::World;

/// Sekunden bis zur Alarmierung
pub const AMBULANCE_DELAY: f64 = 12.;
pub const POLICE_DELAY: f64 = 9.;
/// px: am Einsatzort
pub const ARRIVE: f64 = 110.;
/// Halt am Einsatzort (s)
pub const SCENE_AMBULANCE: f64 = 14.;
pub const SCENE_POLICE: f64 = 18.;
/// px: so weit nimmt der Rettungswagen Tote mit
pub const PICKUP: f64 = 150.;
/// s ohne Ankunft: Einsatz abbrechen
pub const GIVE_UP: f64 = 150.;
/// s: nicht bei jedem Schuss ein neuer Streifenwagen
pub const POLICE_COOLDOWN: f64 = 45.;
pub const MAX_EACH: usize = 2;
/// s zwischen vorbeifahrenden Einsätzen
pub const PASS_MIN: f64 = 150.;
pub const PASS_MAX: f64 = 330.;
/// halbe Sichtweite (life.js LIFE.viewHalfX/Y), außerhalb entstehen und verschwinden Einsatzfahrzeuge
const VIEW_HALF: (f64, f64) = (1100., 650.);
/// Martinshorn: tief/hoch im Wechsel (fleet.js SIREN)
pub const SIREN_LOW: f64 = 440.;
pub const SIREN_HIGH: f64 = 585.;
pub const SIREN_PERIOD: f64 = 1.2;
pub fn siren_high(t: f64) -> bool {
    t % SIREN_PERIOD >= SIREN_PERIOD / 2.
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Police,
    Ambulance,
}
impl Kind {
    pub fn id(self) -> &'static str {
        match self {
            Kind::Police => "police",
            Kind::Ambulance => "ambulance",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Incident {
    pub kind: Kind,
    pub x: f64,
    pub y: f64,
    /// Sekunden bis zur Alarmierung
    pub t: f64,
    pub car: Option<u32>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Drive,
    Scene,
}
/// Einsatz eines Fahrzeugs: Ziel, Durchfahrt oder Einsatz, Beginn, Phase.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Duty {
    pub goal: (f64, f64),
    pub pass: bool,
    pub since: f64,
    pub phase: Phase,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Emergency {
    pub incidents: Vec<Incident>,
    pub pass_t: f64,
    pub last_police: f64,
}
impl Emergency {
    pub fn new(r: f64) -> Self {
        Self {
            incidents: Vec::new(),
            pass_t: PASS_MIN + r * (PASS_MAX - PASS_MIN),
            last_police: -1e9,
        }
    }
}

fn out_of_view(w: &World, x: f64, y: f64, pad: f64) -> bool {
    (x - w.camera.x).abs() > VIEW_HALF.0 + pad || (y - w.camera.y).abs() > VIEW_HALF.1 + pad
}

/// Einsatzfahrzeug außer Sicht auf eine Spur setzen und zum Ziel schicken.
fn dispatch(
    w: &mut World,
    kind: Kind,
    to: (f64, f64),
    near: (f64, f64),
    min_r: f64,
    max_r: f64,
) -> Option<u32> {
    for _ in 0..14 {
        let (lane, s, x, y) = w
            .lanes
            .spawn_spot(&mut w.rng, near.0, near.1, min_r, max_r)?;
        if !out_of_view(w, x, y, 80.) || !w.cars.iter().all(|o| (o.x - x).hypot(o.y - y) > 90.) {
            continue;
        }
        let Some(id) = w.put_npc_car(lane, s, x, y, kind.id()) else {
            continue;
        };
        let i = w.cars.iter().position(|c| c.id == id)?;
        let (lanes, city) = (&mut w.lanes, &w.city);
        if !crate::traffic::set_goal(&mut w.cars[i], lanes, city, to) {
            w.remove_car(id);
            continue;
        }
        let c = &mut w.cars[i];
        if let Some(ai) = c.ai.as_mut() {
            ai.urgent = true;
            ai.cruise_k = 1.3;
        }
        c.siren = true;
        return Some(id);
    }
    None
}

fn finish(c: &mut crate::car::Car, siren_on: bool) {
    c.done = true;
    c.blue = false;
    c.siren = siren_on && c.driver == Some(Driver::Npc);
    if let Some(ai) = c.ai.as_mut() {
        ai.field = None;
        ai.urgent = siren_on;
        ai.cruise_k = if siren_on { 1.25 } else { 1. };
    }
}

/// Einsätze verwalten: Alarmierung, Anfahrt, Einsatzort, Abfahrt, Abbau (services.js manageEmergency).
pub fn manage_emergency(w: &mut World, dt: f64) {
    if !w.services {
        return;
    }
    // neue Einsätze aus dem Geschehen
    let mut new = Vec::new();
    for p in &mut w.peds {
        if p.state == PedState::Dead && !p.reported {
            p.reported = true;
            new.push(Incident {
                kind: Kind::Ambulance,
                x: p.x,
                y: p.y,
                t: AMBULANCE_DELAY,
                car: None,
            });
        }
    }
    for e in &w.events {
        if let Event::Shot { x, y, .. } = *e
            && w.time - w.emergency.last_police > POLICE_COOLDOWN
        {
            w.emergency.last_police = w.time;
            new.push(Incident {
                kind: Kind::Police,
                x,
                y,
                t: POLICE_DELAY,
                car: None,
            });
        }
    }
    w.emergency.incidents.extend(new);
    // Tote in der Nähe eines schon laufenden Rettungseinsatzes zusammenfassen
    let inc = std::mem::take(&mut w.emergency.incidents);
    let mut kept: Vec<Incident> = Vec::new();
    for a in inc {
        let dup = a.kind == Kind::Ambulance
            && a.car.is_none()
            && kept
                .iter()
                .any(|b| b.kind == Kind::Ambulance && (a.x - b.x).hypot(a.y - b.y) < PICKUP);
        if !dup {
            kept.push(a);
        }
    }
    w.emergency.incidents = kept;
    let active = |w: &World, k: Kind| {
        w.cars
            .iter()
            .filter(|c| c.kind == k.id() && c.duty.is_some() && !c.done)
            .count()
    };
    for k in 0..w.emergency.incidents.len() {
        let inc = w.emergency.incidents[k];
        if inc.car.is_some() {
            continue;
        }
        let t = inc.t - dt;
        w.emergency.incidents[k].t = t;
        if t > 0. {
            continue;
        }
        if active(w, inc.kind) >= MAX_EACH {
            w.emergency.incidents[k].t = 3.;
            continue;
        }
        let Some(id) = dispatch(w, inc.kind, (inc.x, inc.y), (inc.x, inc.y), 1000., 2000.) else {
            w.emergency.incidents[k].t = 1.;
            continue;
        };
        w.emergency.incidents[k].car = Some(id);
        let since = w.time;
        if let Some(c) = w.cars.iter_mut().find(|c| c.id == id) {
            c.duty = Some(Duty {
                goal: (inc.x, inc.y),
                pass: false,
                since,
                phase: Phase::Drive,
            });
        }
    }
    // vorbeifahrende Einsätze (Stadtgeräusch): Start außer Sicht, Ziel gegenüber der Kamera
    w.emergency.pass_t -= dt;
    if w.emergency.pass_t <= 0. {
        w.emergency.pass_t = PASS_MIN + w.rng.float() * (PASS_MAX - PASS_MIN);
        let kind = if w.rng.float() < 0.5 {
            Kind::Police
        } else {
            Kind::Ambulance
        };
        let a = w.rng.float() * std::f64::consts::TAU;
        let (cx, cy) = (w.camera.x, w.camera.y);
        let (sx, sy) = (cx + a.cos() * 1000., cy + a.sin() * 1000.);
        let goal = (2. * cx - sx, 2. * cy - sy);
        if let Some(id) = dispatch(w, kind, goal, (sx, sy), 0., 500.) {
            let since = w.time;
            if let Some(c) = w.cars.iter_mut().find(|c| c.id == id) {
                c.duty = Some(Duty {
                    goal,
                    pass: true,
                    since,
                    phase: Phase::Drive,
                });
            }
        }
    }
    // Ablauf je Einsatzfahrzeug
    let mut pickups = Vec::new();
    let now = w.time;
    for c in &mut w.cars {
        let Some(mut d) = c.duty else { continue };
        if c.done {
            continue;
        }
        if c.wrecked || c.driver != Some(Driver::Npc) {
            finish(c, false);
            continue;
        }
        let dist = (c.x - d.goal.0).hypot(c.y - d.goal.1);
        match d.phase {
            Phase::Drive => {
                if dist < if d.pass { 250. } else { ARRIVE } {
                    if d.pass {
                        finish(c, true);
                        continue;
                    }
                    d.phase = Phase::Scene;
                    c.siren = false;
                    c.blue = true;
                    if let Some(ai) = c.ai.as_mut() {
                        ai.hold = if c.kind == "ambulance" {
                            SCENE_AMBULANCE
                        } else {
                            SCENE_POLICE
                        };
                    }
                } else if now - d.since > GIVE_UP {
                    finish(c, false);
                    continue;
                }
            }
            Phase::Scene => {
                if c.ai.as_ref().is_none_or(|a| a.hold <= 0.) {
                    // Rettungswagen: Tote mitnehmen, mit Sondersignal ins Krankenhaus
                    let amb = c.kind == "ambulance";
                    if amb {
                        pickups.push(d.goal);
                    }
                    finish(c, amb);
                    continue;
                }
            }
        }
        c.duty = Some(d);
    }
    for (gx, gy) in pickups {
        w.peds
            .retain(|p| !(p.state == PedState::Dead && (p.x - gx).hypot(p.y - gy) < PICKUP));
        w.events.push(Event::PickupBody { x: gx, y: gy });
    }
    let done: Vec<u32> = w.cars.iter().filter(|c| c.done).map(|c| c.id).collect();
    w.emergency
        .incidents
        .retain(|i| i.car.is_none_or(|id| !done.contains(&id)));
    // fertige Einsatzfahrzeuge außer Sicht abbauen
    let gone: Vec<u32> = w
        .cars
        .iter()
        .filter(|c| {
            c.done
                && c.duty.is_some()
                && c.driver == Some(Driver::Npc)
                && out_of_view(w, c.x, c.y, 200.)
        })
        .map(|c| c.id)
        .collect();
    for id in gone {
        w.remove_car(id);
    }
}
