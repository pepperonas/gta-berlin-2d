//! Radfahrer und E-Roller (Port von `bikes.js`): fahren auf dem Spurgraph des Autoverkehrs (Einbahnstraßen und
//! Abbiegeverbote gelten damit auch für sie), aber nur auf der rechten Spur, seitlich versetzt – auf dem Radstreifen,
//! wo der Querschnitt einen hat, sonst am rechten Fahrbahnrand. Hauptstraßen ohne Radstreifen meiden sie. Die meisten
//! halten bei Rot, manche nicht.
use crate::city::{City, Pt, offset_polyline, point_along, polyline_length};
use crate::levels::LevelState;
use crate::math::Rng;
use crate::roadgraph::{Lane, LaneGraph, LaneId, Light, lane_offsets, signal_state, turn_angle};
use std::collections::HashMap;

/// Kollisionsradius (px)
pub const RADIUS: f64 = 6.;
/// Tempo-Spannen (px/s) für Rad und E-Roller
pub const BIKE_SPEED: (f64, f64) = (45., 62.);
pub const SCOOTER_SPEED: (f64, f64) = (50., 70.);
pub const ACCEL: f64 = 55.;
pub const BRAKE: f64 = 160.;
/// so weit schauen sie voraus (px)
pub const LOOK: f64 = 34.;
/// Anteil, der bei Rot hält
pub const OBEY: f64 = 0.8;
/// Anteil an der Zahl der Fußgänger, davon E-Roller
pub const SHARE: f64 = 0.15;
pub const SCOOTER_SHARE: f64 = 0.25;
/// so nah muss die Spielfigur heran, um ein Rad zu nehmen (px)
pub const GRAB: f64 = 34.;
/// Trikotfarben der Fahrer
pub const RIDER_SHIRTS: [u32; 8] = [
    0x2d3436, 0x0984e3, 0xd63031, 0x00b894, 0xfdcb6e, 0x6c5ce7, 0xe17055, 0xdfe6e9,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Bike,
    Scooter,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Ride,
    Lying,
    Gone,
}
/// Kreuzung queren: gerade vom Ende der einen zum Anfang der nächsten Fahrlinie.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cross {
    pub from: Pt,
    pub to: Pt,
    pub len: f64,
    pub u: f64,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Bike {
    pub id: u32,
    pub kind: Kind,
    pub lane: LaneId,
    pub s: f64,
    pub x: f64,
    pub y: f64,
    pub angle: f64,
    pub speed: f64,
    pub vmax: f64,
    pub state: State,
    pub obeys: bool,
    pub cross: Option<Cross>,
    /// Tretkurbel (Darstellung)
    pub pedal: f64,
    pub seed: u32,
    /// Zeit seit dem Sturz
    pub t: f64,
    pub stuck_t: f64,
    pub push_t: f64,
    pub level: LevelState,
}
impl Bike {
    pub fn shirt(&self) -> u32 {
        RIDER_SHIRTS[self.seed as usize % RIDER_SHIRTS.len()]
    }
}

/// Fahrlinie zu einer Spur (Radstreifenmitte bzw. 0,8 m vom rechten Fahrbahnrand).
#[derive(Debug, Clone, PartialEq)]
pub struct Path {
    pub pts: Vec<Pt>,
    pub len: f64,
    pub on_cycle: bool,
}

/// Radtaugliche Spur: rechte Spur, Straße mit Radstreifen oder Nebenstraße (keine Autobahn/Schnellstraße).
pub fn bikeable(city: &City, lane: &Lane) -> bool {
    let Some(e) = city.edges.get(&lane.edge) else {
        return false;
    };
    let cycle = if lane.dir == 1 {
        e.cs.right.cycle
    } else {
        e.cs.left.cycle
    };
    !lane.bus_only && lane.k + 1 == lane.n && (3..=8).contains(&e.cls) && (cycle > 0. || e.cls >= 5)
}

/// Fahrlinie berechnen (bikes.js bikePath).
pub fn bike_path(city: &City, lane: &Lane) -> Option<Path> {
    let e = city.edges.get(&lane.edge)?;
    let (cs, scale) = (&e.cs, city.scale);
    let lo = lane_offsets(cs, scale);
    let side = if lane.dir == 1 { cs.right } else { cs.left };
    let lf = if lane.dir == 1 {
        *lo.fwd.get(lane.k as usize)?
    } else {
        -*lo.bwd.get(lane.k as usize)?
    };
    let target = cs.width / 2.
        - side.park_w
        - if side.cycle > 0. {
            side.cycle / 2.
        } else {
            0.8 * scale
        };
    let pts = offset_polyline(&lane.pts, (target - lf).max(0.));
    Some(Path {
        len: polyline_length(&pts),
        pts,
        on_cycle: side.cycle > 0.,
    })
}

/// Fahrlinien je Spur, gemerkt (verschwindet eine Spur, verschwindet auch ihr Eintrag).
#[derive(Debug, Default)]
pub struct Paths(HashMap<LaneId, Path>);
impl Paths {
    pub fn get(&mut self, city: &City, lanes: &LaneGraph, id: LaneId) -> Option<&Path> {
        if let std::collections::hash_map::Entry::Vacant(e) = self.0.entry(id) {
            e.insert(bike_path(city, lanes.lane(id)?)?);
        }
        self.0.get(&id)
    }
    /// Einträge entfernter Spuren aufräumen.
    pub fn prune(&mut self, lanes: &LaneGraph) {
        self.0.retain(|id, _| lanes.lane(*id).is_some());
    }
}

#[allow(clippy::too_many_arguments)]
pub fn create(
    id: u32,
    city: &City,
    lanes: &LaneGraph,
    paths: &mut Paths,
    lane: LaneId,
    s: f64,
    rng: &mut Rng,
    kind: Kind,
) -> Option<Bike> {
    let (v0, v1) = match kind {
        Kind::Bike => BIKE_SPEED,
        Kind::Scooter => SCOOTER_SPEED,
    };
    let vmax = v0 + rng.float() * (v1 - v0);
    let obeys = rng.float() < OBEY;
    let pedal = rng.float() * 6.;
    let seed = (rng.float() * 1e6) as u32;
    let p = point_along(&paths.get(city, lanes, lane)?.pts, s);
    Some(Bike {
        id,
        kind,
        lane,
        s,
        x: p.x,
        y: p.y,
        angle: p.uy.atan2(p.ux),
        speed: vmax * 0.8,
        vmax,
        state: State::Ride,
        obeys,
        cross: None,
        pedal,
        seed,
        t: 0.,
        stuck_t: 0.,
        push_t: 0.,
        level: LevelState::default(),
    })
}

/// Nächste radtaugliche Spur (lieber geradeaus); ohne Alternative notfalls jede.
fn next_lane(city: &City, lanes: &mut LaneGraph, lane: LaneId, rng: &mut Rng) -> Option<LaneId> {
    let next = lanes.next(city, lane, false);
    let cur = lanes.lane(lane)?;
    let ok: Vec<LaneId> = next
        .iter()
        .copied()
        .filter(|&n| lanes.lane(n).is_some_and(|l| bikeable(city, l)))
        .collect();
    let pool = if ok.is_empty() { next } else { ok };
    if pool.is_empty() {
        return None;
    }
    let w: Vec<f64> = pool
        .iter()
        .map(|&n| {
            lanes
                .lane(n)
                .map_or(0.3, |m| 0.3 + turn_angle(cur, m).cos().max(0.) * 2.)
        })
        .collect();
    let mut r = rng.float() * w.iter().sum::<f64>();
    for (k, &n) in pool.iter().enumerate() {
        r -= w[k];
        if r <= 0. {
            return Some(n);
        }
    }
    pool.last().copied()
}

/// Was steht im Weg? (x, y, seitliche Breite, Länge entlang der Fahrtrichtung)
#[derive(Debug, Clone, Copy)]
pub struct Obstacle {
    pub x: f64,
    pub y: f64,
    pub lat: f64,
    pub len: f64,
    /// Auto: Ausrichtung und ob es steht (stehender Querverkehr wartet auf das Rad, nicht umgekehrt)
    pub car: Option<(f64, bool)>,
    /// eigene Kennung (Rad), damit ein Rad sich nicht selbst sieht
    pub bike: Option<u32>,
}

/// Abstand zum nächsten Hindernis entlang der Fahrtrichtung oder ∞.
pub fn ahead(b: &Bike, obstacles: &[Obstacle]) -> f64 {
    let (s, c) = b.angle.sin_cos();
    let mut best = f64::INFINITY;
    for o in obstacles {
        let (rx, ry) = (o.x - b.x, o.y - b.y);
        if rx.abs() > 90. || ry.abs() > 90. {
            continue;
        }
        if o.bike == Some(b.id) {
            continue;
        }
        let (mut len, lat) = (o.len, o.lat);
        if let Some((angle, stopped)) = o.car {
            let cos_a = (angle - b.angle).cos();
            if cos_a.abs() < 0.6 && stopped {
                continue;
            }
            len *= cos_a.abs();
        }
        let along = rx * c + ry * s - len;
        if along <= 0. || along > LOOK + len {
            continue;
        }
        if (-rx * s + ry * c).abs() > lat {
            continue;
        }
        best = best.min(along);
    }
    best
}

/// Ein Schritt (bikes.js updateBike). `obstacles`: Autos, Menschen, andere Räder, Spielfigur (ohne dieses Rad).
#[allow(clippy::too_many_arguments)]
pub fn update(
    b: &mut Bike,
    city: &City,
    lanes: &mut LaneGraph,
    paths: &mut Paths,
    rng: &mut Rng,
    time: f64,
    obstacles: &[Obstacle],
    dt: f64,
) {
    if b.state == State::Lying {
        b.t += dt;
        return;
    }
    if b.state == State::Gone {
        return;
    }
    let Some(len) = paths.get(city, lanes, b.lane).map(|p| p.len) else {
        b.state = State::Gone;
        return;
    };
    let to = lanes.lane(b.lane).map(|l| l.to);
    let signal = to.is_some_and(|v| city.signals.contains(&v));
    let mut target = b.vmax;
    // Ampel am Spurende
    let rest = len - b.s;
    if b.cross.is_none() && b.obeys && rest < 60. && signal {
        let light = signal_state(city, to.unwrap_or(0), b.angle, time);
        if light != Light::Green && rest > 6. {
            target = target.min(((rest - 10.) * 2.).max(0.));
        }
    }
    // Notausgang: wer ohne Ampel über 6 s steht, schiebt sich 2 s lang an allem vorbei
    if b.push_t > 0. {
        b.push_t -= dt;
    } else {
        let d = ahead(b, obstacles);
        if d < LOOK {
            target = target.min(((d - 14.) * 2.5).max(0.));
        }
        b.stuck_t = if b.speed < 2. && target < 2. && !(rest < 60. && signal) {
            b.stuck_t + dt
        } else {
            0.
        };
        if b.stuck_t > 6. {
            b.push_t = 2.;
            b.stuck_t = 0.;
        }
    }
    b.speed = if target > b.speed {
        target.min(b.speed + ACCEL * dt)
    } else {
        target.max(b.speed - BRAKE * dt)
    };
    if b.speed < 2. && target < 2. {
        b.speed = 0.;
    }
    let mut mv = b.speed * dt;
    b.pedal += mv * 0.12;
    if let Some(mut cr) = b.cross {
        cr.u += mv;
        if cr.u < cr.len {
            let k = cr.u / cr.len;
            b.x = cr.from.0 + (cr.to.0 - cr.from.0) * k;
            b.y = cr.from.1 + (cr.to.1 - cr.from.1) * k;
            b.angle = (cr.to.1 - cr.from.1).atan2(cr.to.0 - cr.from.0);
            b.cross = Some(cr);
            return;
        }
        mv = cr.u - cr.len;
        b.cross = None;
        b.s = 0.;
    }
    b.s += mv;
    if b.s >= len {
        let Some(next) = next_lane(city, lanes, b.lane, rng) else {
            b.state = State::Gone;
            return;
        };
        let end = paths
            .get(city, lanes, b.lane)
            .and_then(|p| p.pts.last().copied());
        let start = paths
            .get(city, lanes, next)
            .and_then(|p| p.pts.first().copied());
        let (Some(from), Some(to)) = (end, start) else {
            b.state = State::Gone;
            return;
        };
        b.lane = next;
        let l = (to.0 - from.0).hypot(to.1 - from.1);
        if l > 2. {
            b.cross = Some(Cross {
                from,
                to,
                len: l,
                u: 0.,
            });
            (b.x, b.y) = from;
            return;
        }
        b.s = 0.;
    }
    if let Some(p) = paths.get(city, lanes, b.lane) {
        let a = point_along(&p.pts, b.s);
        (b.x, b.y, b.angle) = (a.x, a.y, a.uy.atan2(a.ux));
    }
}

/// Zufälliger Startplatz auf einer radtauglichen Spur im Ring minR…maxR.
#[allow(clippy::too_many_arguments)]
pub fn spawn_spot(
    city: &City,
    lanes: &mut LaneGraph,
    paths: &mut Paths,
    rng: &mut Rng,
    cx: f64,
    cy: f64,
    min_r: f64,
    max_r: f64,
) -> Option<(LaneId, f64)> {
    for _ in 0..30 {
        let (lane, _, _, _) = lanes.spawn_spot(rng, cx, cy, 0., max_r)?;
        if !lanes.lane(lane).is_some_and(|l| bikeable(city, l)) {
            continue;
        }
        let p = paths.get(city, lanes, lane)?;
        let s = rng.float() * p.len;
        let a = point_along(&p.pts, s);
        let d = (a.x - cx).hypot(a.y - cy);
        if d < min_r || d > max_r {
            continue;
        }
        return Some((lane, s));
    }
    None
}

/// Wie gern Leute bei diesem Wetter Rad fahren (weather.js bikeFactor).
pub fn weather_factor(wx: &crate::weather::Params) -> f64 {
    (1. - 0.8 * wx.rain - 0.3 * wx.fog.min(1.) - 0.9 * wx.snow - 0.5 * wx.storm).max(0.)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ahead_sees_only_what_is_in_front() {
        let b = Bike {
            id: 1,
            kind: Kind::Bike,
            lane: 0,
            s: 0.,
            x: 0.,
            y: 0.,
            angle: 0.,
            speed: 50.,
            vmax: 50.,
            state: State::Ride,
            obeys: true,
            cross: None,
            pedal: 0.,
            seed: 3,
            t: 0.,
            stuck_t: 0.,
            push_t: 0.,
            level: LevelState::default(),
        };
        let o = |x, y| Obstacle {
            x,
            y,
            lat: 8.,
            len: 0.,
            car: None,
            bike: None,
        };
        assert_eq!(ahead(&b, &[o(20., 2.)]), 20.);
        assert!(
            ahead(&b, &[o(-20., 0.)]).is_infinite(),
            "hinten zählt nicht"
        );
        assert!(
            ahead(&b, &[o(20., 15.)]).is_infinite(),
            "daneben zählt nicht"
        );
        assert!(ahead(&b, &[o(50., 0.)]).is_infinite(), "zu weit");
        assert_eq!(b.shirt(), RIDER_SHIRTS[3]);
    }
}

/// Abgestellter E-Roller am Gehweg (Darstellung).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParkedScooter {
    pub x: f64,
    pub y: f64,
    pub angle: f64,
    pub lying: bool,
    pub seed: i64,
}

/// Abgestellte E-Roller am Gehweg einer Kante (bikes.js parkedScooters): deterministisch aus der Kanten-ID, an der
/// Hauswandseite des Gehwegs, nie im Haus oder auf der Fahrbahn; jeder fünfte liegt umgekippt.
pub fn parked_scooters(
    city: &mut City,
    sw: &mut crate::pedestrians::Sidewalks,
    eid: i64,
) -> Vec<ParkedScooter> {
    use crate::math::hash01;
    let mut out = Vec::new();
    let Some(e) = city.edges.get(&eid) else {
        return out;
    };
    if !(crate::pedestrians::walkable(e) && e.cls <= 8) {
        return out;
    }
    let (s, eid64) = (city.scale, e.id);
    let id = eid64 as f64;
    let pts = e.pts.clone();
    let (w0, w1) = sw.walk_range(city, eid);
    let n = ((w1 - w0) / (90. * s) + hash01(id * 7. + 3.) * 1.3)
        .floor()
        .max(0.) as usize;
    for k in 0..n {
        let kf = k as f64;
        let side: i8 = if hash01(id * 13. + kf) < 0.5 { 1 } else { -1 };
        let Some(off) = sw.offset(city, eid, side) else {
            continue;
        };
        let st = w0 + hash01(id * 31. + kf * 7.) * (w1 - w0);
        let p = point_along(&pts, st);
        let o = off + 0.8 * s;
        let (x, y) = (p.x - p.uy * o * side as f64, p.y + p.ux * o * side as f64);
        if city.in_building(x, y).is_some() || city.on_road(x, y, 0., None).is_some() {
            continue;
        }
        let lying = hash01(id * 17. + kf) < 0.2;
        out.push(ParkedScooter {
            x,
            y,
            angle: p.uy.atan2(p.ux) + (hash01(id + kf * 3.) - 0.5) * if lying { 3. } else { 0.8 },
            lying,
            seed: eid64 * 10 + k as i64,
        });
    }
    out
}
