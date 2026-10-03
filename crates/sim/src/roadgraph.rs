//! Fahrspurgraph für den KI-Verkehr (Port von `roadgraph.js`, `street.js` und `signals.js`).
//! Rechtsverkehr: Spuren je Richtung aus dem Querschnitt; an Kreuzungen gekürzt, Abbiegeverbinder als kubische
//! Bézierkurven. Der Graph wächst mit den nachgeladenen Kacheln (Kantenereignisse des Stadtmodells).
use crate::city::{
    City, CrossSection, EdgeEvent, Pt, ROAD_CLASS_BUSWAY, offset_polyline, point_along,
    polyline_length,
};
use crate::collision::{Rect, SpatialHash};
use crate::math::{Rng, hash01};
use berlin_map_loader::citycodes::TRAFFIC_MAX_CLASS;
use std::collections::HashMap;

/// Lage der Fahrstreifen im Querschnitt (positiv = rechts in Wegrichtung).
#[derive(Debug, Clone, PartialEq)]
pub struct LaneOffsets {
    pub fwd: Vec<f64>,
    pub bwd: Vec<f64>,
    pub center: f64,
    pub lane_w: f64,
    pub narrow: bool,
}
/// `street.js laneOffsets`: ist eine Straße mit Gegenverkehr so eng, dass je Richtung weniger als 2,6 m bleiben,
/// fahren beide Richtungen in der Mitte der Restfahrbahn.
pub fn lane_offsets(cs: &CrossSection, unit: f64) -> LaneOffsets {
    let xl = -cs.width / 2. + cs.left.park_w + cs.left.cycle;
    let xr = cs.width / 2. - cs.right.park_w - cs.right.cycle;
    let n = cs.fwd + cs.bwd;
    let lw = (xr - xl) / n.max(1) as f64;
    let center = xl + cs.bwd as f64 * lw;
    if cs.fwd > 0 && cs.bwd > 0 && lw < 2.6 * unit {
        let mid = (xl + xr) / 2.;
        return LaneOffsets {
            fwd: vec![mid; cs.fwd as usize],
            bwd: vec![mid; cs.bwd as usize],
            center: mid,
            lane_w: lw,
            narrow: true,
        };
    }
    LaneOffsets {
        fwd: (0..cs.fwd)
            .map(|i| center + (i as f64 + 0.5) * lw)
            .collect(),
        bwd: (0..cs.bwd)
            .map(|j| center - (j as f64 + 0.5) * lw)
            .collect(),
        center,
        lane_w: lw,
        narrow: false,
    }
}
/// Mitte des Parkstreifens einer Seite (−1 links, +1 rechts) und seine Tiefe.
pub fn parking_strip(cs: &CrossSection, side: i8) -> (f64, f64, u8, u8) {
    let s = if side < 0 { cs.left } else { cs.right };
    (
        side as f64 * (cs.width / 2. - s.park_w / 2.),
        s.park_w,
        s.orient,
        s.park,
    )
}

/// Reisetempo aus dem Tempolimit (km/h → px/s), Spielstraßen nicht unter 30 px/s.
pub fn cruise_for(kmh: f64) -> f64 {
    (kmh / 3.6 * 10.).max(30.)
}
pub fn drivable(e: &crate::city::Edge) -> bool {
    e.inside
        && (e.cls <= TRAFFIC_MAX_CLASS || e.cls == ROAD_CLASS_BUSWAY)
        && e.len > 5.
        && !e.blocked
        && !e.passage
}
/// Teilstück zwischen den Bogenlängen s0 und s1.
pub fn cut_polyline(pts: &[Pt], s0: f64, s1: f64) -> Vec<Pt> {
    let mut out = Vec::new();
    let mut acc = 0.;
    for w in pts.windows(2) {
        let ((ax, ay), (bx, by)) = (w[0], w[1]);
        let l = (bx - ax).hypot(by - ay);
        let (a, b) = (s0.max(acc), s1.min(acc + l));
        if b > a && l > 0. {
            let (ta, tb) = ((a - acc) / l, (b - acc) / l);
            if out.is_empty() {
                out.push((ax + (bx - ax) * ta, ay + (by - ay) * ta));
            }
            out.push((ax + (bx - ax) * tb, ay + (by - ay) * tb));
        }
        acc += l;
    }
    out
}

pub type LaneId = u32;
#[derive(Debug, Clone)]
pub struct Lane {
    pub id: LaneId,
    pub key: u32,
    pub edge: i64,
    pub dir: i8,
    pub k: u32,
    pub n: u32,
    pub from: i64,
    pub to: i64,
    pub pts: Vec<Pt>,
    pub len: f64,
    pub cruise: f64,
    pub narrow: bool,
    pub bus_only: bool,
    cells: Vec<(u32, Vec<(i32, i32)>)>,
}
#[derive(Debug, Clone, Copy)]
pub struct LaneSeg {
    pub lane: LaneId,
    pub i: usize,
    pub ax: f64,
    pub ay: f64,
    pub bx: f64,
    pub by: f64,
}

/// Spurgraph, mitwachsend mit den Kacheln.
pub struct LaneGraph {
    pub lanes: HashMap<LaneId, Lane>,
    segs: crate::city::Slab<LaneSeg>,
    hash: SpatialHash,
    out: HashMap<i64, Vec<LaneId>>,
    pub by_edge: HashMap<i64, Vec<LaneId>>,
    next_cache: HashMap<(LaneId, bool), (u64, Vec<LaneId>)>,
    next_id: LaneId,
    synced_gen: u64,
}
impl Default for LaneGraph {
    fn default() -> Self {
        Self {
            lanes: HashMap::new(),
            segs: Default::default(),
            hash: SpatialHash::new(320.),
            out: HashMap::new(),
            by_edge: HashMap::new(),
            next_cache: HashMap::new(),
            next_id: 0,
            synced_gen: u64::MAX,
        }
    }
}

impl LaneGraph {
    /// Kantenereignisse des Stadtmodells übernehmen (neue Spuren anlegen, entfernte abbauen).
    pub fn sync(&mut self, city: &mut City) {
        let events = std::mem::take(&mut city.edge_events);
        for ev in events {
            match ev {
                EdgeEvent::Added(id) => {
                    if !self.by_edge.contains_key(&id) {
                        self.add(city, id);
                    }
                }
                EdgeEvent::Removed(id) => self.remove(id),
            }
        }
        if self.synced_gen != city.generation {
            self.synced_gen = city.generation;
            self.next_cache.clear();
        }
    }
    fn add(&mut self, city: &City, id: i64) {
        let Some(e) = city.edges.get(&id) else { return };
        if !drivable(e) {
            return;
        }
        let made = self.make_lanes(city, e);
        if made.is_empty() {
            return;
        }
        let ids: Vec<LaneId> = made.iter().map(|l| l.id).collect();
        for mut l in made {
            for i in 0..l.pts.len() - 1 {
                let ((ax, ay), (bx, by)) = (l.pts[i], l.pts[i + 1]);
                let sg = LaneSeg {
                    lane: l.id,
                    i,
                    ax,
                    ay,
                    bx,
                    by,
                };
                let h = self.segs.insert(sg);
                let keys = self.hash.insert(
                    h,
                    &Rect::new(ax.min(bx), ay.min(by), (bx - ax).abs(), (by - ay).abs()),
                );
                l.cells.push((h, keys));
            }
            let list = self.out.entry(l.from).or_default();
            let pos = list
                .iter()
                .position(|o| {
                    let ol = &self.lanes[o];
                    ol.edge > l.edge || (ol.edge == l.edge && ol.key > l.key)
                })
                .unwrap_or(list.len());
            list.insert(pos, l.id);
            self.lanes.insert(l.id, l);
        }
        self.by_edge.insert(id, ids);
    }
    fn remove(&mut self, edge: i64) {
        let Some(ids) = self.by_edge.remove(&edge) else {
            return;
        };
        for id in ids {
            let Some(l) = self.lanes.remove(&id) else {
                continue;
            };
            if let Some(list) = self.out.get_mut(&l.from) {
                list.retain(|&x| x != id);
                if list.is_empty() {
                    self.out.remove(&l.from);
                }
            }
            for (h, keys) in &l.cells {
                self.hash.remove(*h, keys);
                self.segs.remove(*h);
            }
        }
    }
    fn make_lanes(&mut self, city: &City, e: &crate::city::Edge) -> Vec<Lane> {
        let s = city.scale;
        let lo = lane_offsets(&e.cs, s);
        let trim = |n: i64| city.nodes.get(&n).map(|nd| nd.trim).unwrap_or(0.);
        let mut made = Vec::new();
        let rev: Vec<Pt> = e.pts.iter().rev().copied().collect();
        let mut push = |this: &mut Self,
                        dir: i8,
                        off: f64,
                        key: u32,
                        k: u32,
                        n: u32,
                        narrow: bool,
                        bus_only: bool| {
            let base = if dir == 1 { &e.pts } else { &rev };
            let (from, to) = if dir == 1 { (e.a, e.b) } else { (e.b, e.a) };
            let raw = offset_polyline(base, off);
            let l = polyline_length(&raw);
            let (mut s0, mut s1) = (trim(from), l - trim(to));
            if s1 - s0 < l * 0.3 {
                let m = l / 2.;
                s0 = s0.min(m - l * 0.15);
                s1 = s1.max(m + l * 0.15);
            }
            let pts = cut_polyline(&raw, s0, s1);
            if pts.len() < 2 {
                return;
            }
            let id = this.next_id;
            this.next_id += 1;
            made.push(Lane {
                id,
                key,
                edge: e.id,
                dir,
                k,
                n,
                from,
                to,
                len: polyline_length(&pts),
                pts,
                cruise: cruise_for(e.cs.maxspeed),
                narrow,
                bus_only,
                cells: Vec::new(),
            });
        };
        for dir in [1i8, -1] {
            let n = if dir == 1 { e.cs.fwd } else { e.cs.bwd };
            for k in 0..n {
                let off = if dir == 1 {
                    lo.fwd[k as usize]
                } else {
                    -lo.bwd[k as usize]
                };
                let key = if dir == 1 { 0 } else { 100 } + k;
                push(
                    self,
                    dir,
                    off,
                    key,
                    k,
                    n,
                    lo.narrow && e.cs.fwd > 0 && e.cs.bwd > 0,
                    e.cls == ROAD_CLASS_BUSWAY,
                );
            }
        }
        // Gegenbusspur in Einbahnstraßen: eigene Spur nur für Busse am linken Fahrbahnrand
        if e.cs.bus_contra && (e.cs.fwd == 0) != (e.cs.bwd == 0) {
            let dir = if e.cs.fwd > 0 { -1 } else { 1 };
            push(
                self,
                dir,
                (1.4 * s).max(e.cs.width / 2. - 1.6 * s),
                if dir == 1 { 50 } else { 150 },
                0,
                1,
                false,
                true,
            );
        }
        made
    }

    pub fn lane(&self, id: LaneId) -> Option<&Lane> {
        self.lanes.get(&id)
    }
    /// Nachfolger für den allgemeinen Verkehr (ohne Busspuren); `bus` = Linienbusse.
    pub fn next(&mut self, city: &City, id: LaneId, bus: bool) -> Vec<LaneId> {
        let Some(l) = self.lanes.get(&id) else {
            return Vec::new();
        };
        let with_bus = bus || l.bus_only;
        if let Some((g, v)) = self.next_cache.get(&(id, with_bus))
            && *g == city.generation
        {
            return v.clone();
        }
        let at: Vec<&Lane> = self
            .out
            .get(&l.to)
            .map(|v| v.iter().filter_map(|i| self.lanes.get(i)).collect())
            .unwrap_or_default();
        let at: Vec<&Lane> = at.into_iter().filter(|m| with_bus || !m.bus_only).collect();
        let banned = |m: &Lane| city.turn_bans.contains(&(l.edge, l.to, m.edge));
        let all: Vec<&Lane> = at
            .iter()
            .copied()
            .filter(|m| m.edge != l.edge && !banned(m))
            .collect();
        let ok: Vec<LaneId> = all
            .iter()
            .filter(|m| {
                let a = turn_angle(l, m);
                if a > 0.5 {
                    l.k == l.n - 1 && m.k == m.n - 1
                } else if a < -0.5 {
                    l.k == 0 && m.k == 0
                } else {
                    m.k == l.k.min(m.n - 1)
                }
            })
            .map(|m| m.id)
            .collect();
        let res = if !ok.is_empty() {
            ok
        } else if !all.is_empty() {
            all.iter().map(|m| m.id).collect()
        } else {
            let any: Vec<LaneId> = at
                .iter()
                .filter(|m| m.id != l.id && !banned(m))
                .map(|m| m.id)
                .collect();
            if !any.is_empty() {
                any
            } else {
                at.iter().filter(|m| m.id != l.id).map(|m| m.id).collect()
            }
        };
        self.next_cache
            .insert((id, with_bus), (city.generation, res.clone()));
        res
    }

    /// Nächste Spur wählen: lieber geradeaus, Wenden nur in Sackgassen.
    pub fn choose_next(&mut self, city: &City, id: LaneId, rng: &mut Rng) -> Option<LaneId> {
        let opts = self.next(city, id, false);
        if opts.is_empty() {
            return None;
        }
        let l = &self.lanes[&id];
        let w: Vec<f64> = opts
            .iter()
            .map(|m| 0.4 + turn_angle(l, &self.lanes[m]).cos().max(0.) * 2.2)
            .collect();
        let total: f64 = w.iter().sum();
        let mut r = rng.float() * total;
        for (i, x) in w.iter().enumerate() {
            r -= x;
            if r <= 0. {
                return Some(opts[i]);
            }
        }
        opts.last().copied()
    }

    /// Nächste Spur zu einer Position; die Richtung zählt mit (Winkelabweichung in px bewertet).
    pub fn nearest_lane(
        &mut self,
        x: f64,
        y: f64,
        angle: Option<f64>,
        radius: f64,
        allow_bus: bool,
    ) -> Option<NearLane> {
        let mut out = Vec::new();
        self.hash.query(&Rect::around(x, y, radius), &mut out);
        let mut best: Option<NearLane> = None;
        for h in out {
            let Some(s) = self.segs.get(h) else { continue };
            let Some(l) = self.lanes.get(&s.lane) else {
                continue;
            };
            if l.bus_only && !allow_bus {
                continue;
            }
            let (dx, dy) = (s.bx - s.ax, s.by - s.ay);
            let l2 = dx * dx + dy * dy;
            let len = if l2 > 0. { l2.sqrt() } else { 1. };
            let t = if l2 > 0. {
                (((x - s.ax) * dx + (y - s.ay) * dy) / l2).clamp(0., 1.)
            } else {
                0.
            };
            let (px, py) = (s.ax + dx * t, s.ay + dy * t);
            let align = angle
                .map(|a| (a.cos() * dx + a.sin() * dy) / len)
                .unwrap_or(1.);
            let score = (px - x).hypot(py - y) + (1. - align) * 60.;
            if best.is_none_or(|b| score < b.score) {
                best = Some(NearLane {
                    score,
                    lane: s.lane,
                    i: s.i,
                    t,
                    x: px,
                    y: py,
                });
            }
        }
        best
    }

    /// Zufälliges Spurstück mit Abstand minR…maxR zu (cx, cy) (`traffic.js spawnSpot`).
    pub fn spawn_spot(
        &mut self,
        rng: &mut Rng,
        cx: f64,
        cy: f64,
        min_r: f64,
        max_r: f64,
    ) -> Option<(LaneId, f64, f64, f64)> {
        let mut segs = Vec::new();
        self.hash.query(&Rect::around(cx, cy, max_r), &mut segs);
        if segs.is_empty() {
            return None;
        }
        for _ in 0..40 {
            let h = segs[rng.index(segs.len())];
            let Some(sg) = self.segs.get(h).copied() else {
                continue;
            };
            let Some(l) = self.lanes.get(&sg.lane) else {
                continue;
            };
            if l.bus_only {
                continue;
            }
            let t = rng.float();
            let (x, y) = (sg.ax + (sg.bx - sg.ax) * t, sg.ay + (sg.by - sg.ay) * t);
            let d = (x - cx).hypot(y - cy);
            if d < min_r || d > max_r {
                continue;
            }
            let s: f64 = l.pts[..=sg.i]
                .windows(2)
                .map(|w| (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1))
                .sum::<f64>()
                + (x - sg.ax).hypot(y - sg.ay);
            return Some((sg.lane, s, x, y));
        }
        None
    }
    pub fn len(&self) -> usize {
        self.lanes.len()
    }
    pub fn is_empty(&self) -> bool {
        self.lanes.is_empty()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct NearLane {
    pub score: f64,
    pub lane: LaneId,
    pub i: usize,
    pub t: f64,
    pub x: f64,
    pub y: f64,
}

pub fn lane_dir(l: &Lane, at_end: bool) -> (f64, f64) {
    let p = &l.pts;
    let n = p.len();
    let ((ax, ay), (bx, by)) = if at_end {
        (p[n - 2], p[n - 1])
    } else {
        (p[0], p[1])
    };
    let len = (bx - ax).hypot(by - ay);
    let len = if len > 0. { len } else { 1. };
    ((bx - ax) / len, (by - ay) / len)
}
/// Abbiegewinkel (rad, 0 = geradeaus, + = rechts) von Spur a auf Spur b.
pub fn turn_angle(a: &Lane, b: &Lane) -> f64 {
    let (ux, uy) = lane_dir(a, true);
    let (vx, vy) = lane_dir(b, false);
    (ux * vy - uy * vx).atan2(ux * vx + uy * vy)
}
/// Verbinder-Punkte vom Ende von a zum Anfang von b (ohne Endpunkte).
pub fn connector(a: &Lane, b: &Lane) -> Vec<Pt> {
    let (x0, y0) = *a.pts.last().expect("Spur ohne Punkte");
    let (x3, y3) = b.pts[0];
    let d = (x3 - x0).hypot(y3 - y0);
    if d < 4. {
        return Vec::new();
    }
    let (ux, uy) = lane_dir(a, true);
    let (vx, vy) = lane_dir(b, false);
    let k = (d * 0.45).max(12.);
    let (x1, y1, x2, y2) = (x0 + ux * k, y0 + uy * k, x3 - vx * k, y3 - vy * k);
    let steps = ((d / 25.).round() as i64).clamp(2, 8);
    (1..steps)
        .map(|st| {
            let t = st as f64 / steps as f64;
            let u = 1. - t;
            (
                u * u * u * x0 + 3. * u * u * t * x1 + 3. * u * t * t * x2 + t * t * t * x3,
                u * u * u * y0 + 3. * u * u * t * y1 + 3. * u * t * t * y2 + t * t * t * y3,
            )
        })
        .collect()
}
/// Punkt in Bogenlänge s auf einer Spur.
pub fn lane_point(l: &Lane, s: f64) -> Pt {
    let a = point_along(&l.pts, s.min(l.len));
    (a.x, a.y)
}

/// Ampel: fester 50-s-Umlauf mit zwei Achsen (Achse 0 = Richtung der ersten Zufahrt ± 45°).
pub const SIGNAL_CYCLE: f64 = 50.;
pub const SIGNAL_GREEN: f64 = 20.;
pub const SIGNAL_YELLOW: f64 = 3.;
pub const SIGNAL_ALL_RED: f64 = 2.;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Light {
    Green,
    Yellow,
    Red,
}
fn signal_axis(city: &City, v: i64) -> f64 {
    let Some(nd) = city.nodes.get(&v) else {
        return 0.;
    };
    let es: Vec<_> = nd.edges.iter().filter_map(|k| city.edges.get(k)).collect();
    let Some(e) = es.iter().find(|x| x.cls <= 8).or(es.first()) else {
        return 0.;
    };
    let p = &e.pts;
    let n = p.len();
    let ((x0, y0), (x1, y1)) = if e.a == v {
        (p[0], p[1])
    } else {
        (p[n - 1], p[n - 2])
    };
    (y1 - y0).atan2(x1 - x0)
}
pub fn signal_state(city: &City, v: i64, heading: f64, time: f64) -> Light {
    let axis = if (heading - signal_axis(city, v)).cos().abs() >= std::f64::consts::FRAC_1_SQRT_2 {
        0
    } else {
        1
    };
    let half = SIGNAL_GREEN + SIGNAL_YELLOW + SIGNAL_ALL_RED;
    let t = ((time + hash01((v * 13 + 5) as f64) * SIGNAL_CYCLE) % SIGNAL_CYCLE + SIGNAL_CYCLE)
        % SIGNAL_CYCLE
        - if axis == 1 { half } else { 0. };
    if (0. ..SIGNAL_GREEN).contains(&t) {
        Light::Green
    } else if (SIGNAL_GREEN..SIGNAL_GREEN + SIGNAL_YELLOW).contains(&t) {
        Light::Yellow
    } else {
        Light::Red
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::city::Side;

    fn cs(width: f64, fwd: u32, bwd: u32, park: f64) -> CrossSection {
        CrossSection {
            width,
            fwd,
            bwd,
            left: Side {
                park_w: park,
                ..Default::default()
            },
            right: Side {
                park_w: park,
                ..Default::default()
            },
            maxspeed: 50.,
            ..Default::default()
        }
    }
    #[test]
    fn lane_offsets_keep_right_and_merge_when_narrow() {
        let lo = lane_offsets(&cs(140., 2, 2, 0.), 10.);
        assert_eq!(lo.fwd, vec![17.5, 52.5]);
        assert_eq!(lo.bwd, vec![-17.5, -52.5]);
        // 6 m Fahrbahn, 2 × 2 m Parkstreifen: je Richtung 1 m → beide in der Mitte
        let lo = lane_offsets(&cs(60., 1, 1, 20.), 10.);
        assert!(lo.narrow && lo.fwd == vec![0.] && lo.bwd == vec![0.]);
        let lo = lane_offsets(&cs(40., 1, 0, 0.), 10.);
        assert_eq!(lo.fwd, vec![0.]);
    }
    #[test]
    fn cut_and_cruise() {
        let p = cut_polyline(&[(0., 0.), (100., 0.), (100., 100.)], 50., 150.);
        assert_eq!(p, vec![(50., 0.), (100., 0.), (100., 50.)]);
        assert_eq!(cruise_for(50.), 50. / 3.6 * 10.);
        assert_eq!(cruise_for(5.), 30.);
    }
}
