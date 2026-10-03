//! Passanten (Port von `pedestrians.js`): gehen auf dem Gehweg links und rechts der echten Straßen, bleiben mal
//! stehen, biegen an Kreuzungen ab oder überqueren die Straße (bevorzugt am Zebrastreifen), warten vor fahrenden
//! Autos, fliehen vor Rasern, Hupen und Unfällen und kehren danach zum Gehweg zurück.
//! Nicht portiert (folgen mit späteren Phasen): Aufenthaltsorte (`life.js`), Schlägereien (`combat.js`).
use crate::car::{Knocked, blocks};
use crate::city::{City, Edge, Pt, Solid, point_along, project_on_polyline};
use crate::collision::{Rect, circle_vs_circle, circle_vs_rect, circle_vs_segment};
use crate::levels::{LevelState, touch};
use crate::math::Rng;
use std::collections::HashMap;

pub const RADIUS: f64 = 6.;
pub const WALK: f64 = 13.;
pub const RUN: f64 = 45.;
pub const SHIRTS: &[u32] = &[
    0xe74c3c, 0x3498db, 0x2ecc71, 0x9b59b6, 0xf39c12, 0x1abc9c, 0xecf0f1, 0x34495e, 0xd35400,
    0xe84393,
];
pub const SKIN: &[u32] = &[0xf2d0b1, 0xe0ac69, 0xc68642, 0x8d5524, 0xf5d6c6];

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PedState {
    Walk,
    Idle,
    Cross,
    Flee,
    Return,
    Down,
    Dead,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CrossTarget {
    pub edge: i64,
    pub side: i8,
    pub s: f64,
    pub dir: i8,
    pub x: f64,
    pub y: f64,
    pub via: Option<Pt>,
}
#[derive(Debug, Clone)]
pub struct Ped {
    pub id: u32,
    pub x: f64,
    pub y: f64,
    pub facing: f64,
    pub edge: i64,
    pub side: i8,
    pub s: f64,
    pub dir: i8,
    pub speed: f64,
    pub state: PedState,
    pub t: f64,
    pub next_idle: f64,
    pub shirt: u32,
    pub skin: u32,
    pub threat: Pt,
    pub target: Option<CrossTarget>,
    pub step: f64,
    pub car_hit_cd: f64,
    pub wait: f64,
    pub cross_t: f64,
    pub return_t: f64,
    pub return_best: f64,
    pub dead_t: f64,
    pub hp: f64,
    pub level: LevelState,
    pub level_init: bool,
}

pub fn walkable(e: &Edge) -> bool {
    e.inside && e.cls >= 3 && e.cls <= 10 && e.cls != 9 && e.len > 20.
}

/// Gehweg-Daten je Kante (Abschnitt von Ecke zu Ecke, Versatz je Seite), gecacht je Stadtzustand.
#[derive(Default)]
pub struct Sidewalks {
    range: HashMap<i64, (f64, f64)>,
    offset: HashMap<(i64, i8), Option<f64>>,
}
impl Sidewalks {
    /// Einträge einer entfernten Kante vergessen.
    pub fn forget(&mut self, edge: i64) {
        self.range.remove(&edge);
        self.offset.remove(&(edge, 1));
        self.offset.remove(&(edge, -1));
    }
}
impl Sidewalks {
    fn check(&mut self, city: &City) {
        // wie die JS-Fassung (Cache am Kantenobjekt): einmal je Kante; nur ein Überlauf leert den Speicher
        if self.range.len() + self.offset.len() > 50_000 {
            self.range.clear();
            self.offset.clear();
        }
        let _ = city;
    }
    /// Gehwegabschnitt einer Kante (an Kreuzungen endet der Gehweg am Rand der Querstraße).
    pub fn walk_range(&mut self, city: &City, e: i64) -> (f64, f64) {
        self.check(city);
        if let Some(&r) = self.range.get(&e) {
            return r;
        }
        let Some(edge) = city.edges.get(&e) else {
            return (0., 0.);
        };
        let s = city.scale;
        let corner = |n: i64| {
            let Some(nd) = city.nodes.get(&n) else {
                return s;
            };
            if nd.edges.len() < 3 {
                return s;
            }
            nd.edges
                .iter()
                .filter(|&&k| k != e)
                .filter_map(|k| city.edges.get(k))
                .map(|o| o.w / 2.)
                .fold(0., f64::max)
                + s
        };
        let (mut a, mut b) = (corner(edge.a), edge.len - corner(edge.b));
        if b - a < 2. * s {
            let m = edge.len / 2.;
            a = m - s;
            b = m + s;
        }
        self.range.insert(e, (a, b));
        (a, b)
    }
    /// Abstand der Laufspur von der Straßenmitte, so dass sie nicht in Häusern liegt (None = kein freier Gehweg).
    pub fn offset(&mut self, city: &mut City, e: i64, side: i8) -> Option<f64> {
        self.check(city);
        if let Some(&o) = self.offset.get(&(e, side)) {
            return o;
        }
        let (w0, w1) = self.walk_range(city, e);
        let edge = city.edges.get(&e)?;
        let s = city.scale;
        let half = edge.w / 2.;
        let pts = edge.pts.clone();
        let n = (((w1 - w0) / (1.5 * s)).ceil() as usize).max(3);
        let mut off = None;
        let mut o = half + 2.2 * s;
        while o >= half + 0.5 * s {
            let mut ok = true;
            for k in 0..=n {
                let p = point_along(&pts, w0 + (w1 - w0) * k as f64 / n as f64);
                let (x, y) = (p.x - p.uy * o * side as f64, p.y + p.ux * o * side as f64);
                if city.in_building(x, y).is_some() {
                    ok = false;
                    break;
                }
            }
            if ok {
                off = Some(o);
                break;
            }
            o -= 0.4 * s;
        }
        self.offset.insert((e, side), off);
        off
    }
    pub fn point(&mut self, city: &mut City, e: i64, side: i8, s: f64) -> Pt {
        let o = self.offset(city, e, side);
        let Some(edge) = city.edges.get(&e) else {
            return (0., 0.);
        };
        let o = o.unwrap_or(edge.w / 2. + 0.5 * city.scale);
        let p = point_along(&edge.pts, s);
        (p.x - p.uy * o * side as f64, p.y + p.ux * o * side as f64)
    }
}

/// Gehweg-Platz.
#[derive(Debug, Clone, Copy)]
pub struct Spot {
    pub edge: i64,
    pub side: i8,
    pub s: f64,
}

pub fn create_ped(id: u32, city: &mut City, sw: &mut Sidewalks, spot: Spot, rng: &mut Rng) -> Ped {
    let (x, y) = sw.point(city, spot.edge, spot.side, spot.s);
    Ped {
        id,
        x,
        y,
        facing: 0.,
        edge: spot.edge,
        side: spot.side,
        s: spot.s,
        dir: if rng.float() < 0.5 { -1 } else { 1 },
        speed: WALK * (0.8 + rng.float() * 0.45),
        state: PedState::Walk,
        t: 0.,
        next_idle: 4. + rng.float() * 10.,
        shirt: SHIRTS[rng.index(SHIRTS.len())],
        skin: SKIN[rng.index(SKIN.len())],
        threat: (x, y),
        target: None,
        step: 0.,
        car_hit_cd: 0.,
        wait: 0.,
        cross_t: 0.,
        return_t: 0.,
        return_best: f64::INFINITY,
        dead_t: 0.,
        hp: 100.,
        level: LevelState::default(),
        level_init: false,
    }
}

/// Nächster Gehweg-Platz zu einer Position.
pub fn nearest_spot(
    city: &mut City,
    sw: &mut Sidewalks,
    x: f64,
    y: f64,
    radius: f64,
) -> Option<Spot> {
    let n = city
        .nearest_edge(x, y, radius, walkable)
        .or_else(|| city.nearest_edge(x, y, radius * 6., walkable))?;
    let mut side = if (x - n.x) * -n.uy + (y - n.y) * n.ux >= 0. {
        1
    } else {
        -1
    };
    if sw.offset(city, n.edge, side).is_none() {
        side = -side;
    }
    let (w0, w1) = sw.walk_range(city, n.edge);
    Some(Spot {
        edge: n.edge,
        side,
        s: n.s.max(w0).min(w1),
    })
}

/// Zufälliger Gehweg-Platz im Ring minR…maxR um (cx, cy).
pub fn spawn_spot(
    city: &mut City,
    sw: &mut Sidewalks,
    rng: &mut Rng,
    cx: f64,
    cy: f64,
    min_r: f64,
    max_r: f64,
) -> Option<Spot> {
    let segs = city.edge_segs.query(&Rect::around(cx, cy, max_r));
    if segs.is_empty() {
        return None;
    }
    for _ in 0..30 {
        let sg = *city.edge_segs.get(segs[rng.index(segs.len())]);
        let Some(eid) = sg.edge else { continue };
        let Some(e) = city.edges.get(&eid) else {
            continue;
        };
        if !walkable(e) {
            continue;
        }
        let t = rng.float();
        let (x, y) = (sg.ax + (sg.bx - sg.ax) * t, sg.ay + (sg.by - sg.ay) * t);
        let d = (x - cx).hypot(y - cy);
        if d < min_r || d > max_r {
            continue;
        }
        let pr = project_on_polyline(&e.pts, x, y)?;
        let mut side = if rng.float() < 0.5 { 1 } else { -1 };
        if sw.offset(city, eid, side).is_none() {
            side = -side;
        }
        if sw.offset(city, eid, side).is_none() {
            continue;
        }
        let (w0, w1) = sw.walk_range(city, eid);
        return Some(Spot {
            edge: eid,
            side,
            s: pr.s.max(w0).min(w1),
        });
    }
    None
}

pub fn scare(p: &mut Ped, from: Pt, duration: f64) {
    if matches!(p.state, PedState::Down | PedState::Dead) {
        return;
    }
    p.state = PedState::Flee;
    p.t = duration;
    p.threat = from;
}

/// Ein fahrendes Auto in der Nähe (für den Blick vor dem Betreten einer Fahrbahn).
#[derive(Debug, Clone, Copy)]
pub struct MovingCar {
    pub x: f64,
    pub y: f64,
    pub speed: f64,
    pub lvl: i8,
}
/// Was ein Passant von der Welt braucht.
pub struct PedCtx<'a> {
    pub city: &'a mut City,
    pub sw: &'a mut Sidewalks,
    pub rng: &'a mut Rng,
    pub knocked: &'a Knocked,
    pub cars: &'a [MovingCar],
    /// Spieler zu Fuß (x, y)
    pub player_on_foot: Option<Pt>,
}

fn move_with_collision(p: &mut Ped, dx: f64, dy: f64, cx: &mut PedCtx) {
    p.x += dx;
    p.y += dy;
    for h in cx.city.solids.query(&Rect::around(p.x, p.y, 10.)) {
        let s = *cx.city.solids.get(h);
        if !blocks(cx.knocked, &s, p.level.lvl) {
            continue;
        }
        let m = match s {
            Solid::Wall { seg, .. } => circle_vs_segment(p.x, p.y, RADIUS, &seg),
            Solid::Circle { x, y, r, .. } => circle_vs_circle(p.x, p.y, RADIUS, x, y, r),
            Solid::Rect(r) => circle_vs_rect(p.x, p.y, RADIUS, &r),
        };
        if let Some(m) = m {
            p.x += m.nx * m.depth;
            p.y += m.ny * m.depth;
        }
    }
}

/// Liegt der nächste Schritt auf einer Fahrbahn und nähert sich ein fahrendes Auto?
fn car_coming(cx: &mut PedCtx, p: &Ped, next: Pt) -> bool {
    if cx.city.on_road(next.0, next.1, 0., None).is_none() {
        return false;
    }
    let me = (p.x, p.y, p.level.lvl);
    let near: Vec<MovingCar> = cx
        .cars
        .iter()
        .copied()
        .filter(|c| (c.x - p.x).hypot(c.y - p.y) < 120. && c.speed > 25.)
        .collect();
    near.into_iter()
        .any(|c| touch(cx.city, me, (c.x, c.y, c.lvl)))
}

/// Am Ende einer Kante: nächste Kante am Knoten wählen, meist auf derselben Straßenseite.
fn next_leg(p: &mut Ped, cx: &mut PedCtx) {
    let Some(e) = cx.city.edges.get(&p.edge).cloned() else {
        p.state = PedState::Idle;
        p.t = 1.;
        return;
    };
    let node = if p.dir > 0 { e.b } else { e.a };
    let cands: Vec<i64> = cx
        .city
        .nodes
        .get(&node)
        .map(|n| n.edges.clone())
        .unwrap_or_default();
    let mut opts = Vec::new();
    for k in cands {
        let Some(o) = cx.city.edges.get(&k) else {
            continue;
        };
        if !walkable(o) || k == e.id {
            continue;
        }
        if cx.sw.offset(cx.city, k, 1).is_some() || cx.sw.offset(cx.city, k, -1).is_some() {
            opts.push(k);
        }
    }
    let next = if opts.is_empty() {
        e.id
    } else {
        opts[cx.rng.index(opts.len())]
    };
    let Some(ne) = cx.city.edges.get(&next).cloned() else {
        return;
    };
    let dir = if next == e.id {
        -p.dir
    } else if ne.a == node {
        1
    } else {
        -1
    };
    let (w0, w1) = cx.sw.walk_range(cx.city, next);
    let s = if dir > 0 { w0 } else { w1 };
    let here = (p.x, p.y);
    let a = cx.sw.point(cx.city, next, 1, s);
    let b = cx.sw.point(cx.city, next, -1, s);
    let mut side: i8 = if (a.0 - here.0).hypot(a.1 - here.1) < (b.0 - here.0).hypot(b.1 - here.1) {
        1
    } else {
        -1
    };
    let same = side;
    if next == e.id || cx.rng.float() < 0.2 {
        side = -side;
    }
    if cx.sw.offset(cx.city, next, side).is_none() {
        side = -side;
    }
    if side != same
        && cx.sw.offset(cx.city, next, same).is_some()
        && let Some(z) = ne
            .crossings
            .iter()
            .find(|c| (c.s - s).abs() < 30. * cx.city.scale)
    {
        let t = cx.sw.point(cx.city, next, side, z.s);
        let via = cx.sw.point(cx.city, next, same, z.s);
        p.state = PedState::Cross;
        p.target = Some(CrossTarget {
            edge: next,
            side,
            s: z.s,
            dir,
            x: t.0,
            y: t.1,
            via: Some(via),
        });
        return;
    }
    let pt = if side == 1 { a } else { b };
    // Um die Ecke über den Schnittpunkt der beiden Gehweglinien, nicht quer durch das Eckhaus.
    let ta = point_along(&e.pts, p.s.max(0.).min(e.len));
    let tb = point_along(&ne.pts, s);
    let (ax, ay) = (ta.ux * p.dir as f64, ta.uy * p.dir as f64);
    let (bx, by) = (tb.ux * dir as f64, tb.uy * dir as f64);
    let den = ax * by - ay * bx;
    let mut via = None;
    if den.abs() > 0.25 {
        let t = ((pt.0 - here.0) * by - (pt.1 - here.1) * bx) / den;
        if t > 0. && t < 40. * cx.city.scale {
            via = Some((here.0 + ax * t, here.1 + ay * t));
        }
    }
    p.state = PedState::Cross;
    p.target = Some(CrossTarget {
        edge: next,
        side,
        s,
        dir,
        x: pt.0,
        y: pt.1,
        via,
    });
}

pub fn update_ped(p: &mut Ped, cx: &mut PedCtx, dt: f64) {
    p.car_hit_cd = (p.car_hit_cd - dt).max(0.);
    let (px, py) = (p.x, p.y);
    match p.state {
        PedState::Walk => 'walk: {
            p.next_idle -= dt;
            if p.next_idle <= 0. {
                p.state = PedState::Idle;
                p.t = 1. + cx.rng.float() * 2.5;
                p.next_idle = 6. + cx.rng.float() * 12.;
                break 'walk;
            }
            let ahead = cx
                .sw
                .point(cx.city, p.edge, p.side, p.s + p.dir as f64 * 14.);
            if cx
                .player_on_foot
                .is_some_and(|(x, y)| (x - ahead.0).hypot(y - ahead.1) < 11.)
            {
                break 'walk;
            }
            if car_coming(cx, p, ahead) {
                break 'walk;
            }
            p.s += p.dir as f64 * p.speed * dt;
            let (w0, w1) = cx.sw.walk_range(cx.city, p.edge);
            if p.s <= w0 || p.s >= w1 {
                p.s = p.s.max(w0).min(w1);
                next_leg(p, cx);
                break 'walk;
            }
            (p.x, p.y) = cx.sw.point(cx.city, p.edge, p.side, p.s);
        }
        PedState::Idle => {
            p.t -= dt;
            if p.t <= 0. {
                p.state = PedState::Walk;
                if cx.rng.float() < 0.3 {
                    p.dir = -p.dir;
                }
            }
        }
        PedState::Cross => 'cross: {
            let Some(mut tg) = p.target else {
                p.state = PedState::Walk;
                break 'cross;
            };
            if tg
                .via
                .is_some_and(|(vx, vy)| (vx - p.x).hypot(vy - p.y) < 3.)
            {
                tg.via = None;
            }
            let aim = tg.via.unwrap_or((tg.x, tg.y));
            let (dx, dy) = (aim.0 - p.x, aim.1 - p.y);
            let d = dx.hypot(dy);
            let v = p.speed * 1.35 * dt;
            p.target = Some(tg);
            if d > v && p.wait < 6. && car_coming(cx, p, (p.x + dx / d * 12., p.y + dy / d * 12.)) {
                p.wait += dt;
                break 'cross;
            }
            p.wait = 0.;
            p.cross_t += dt;
            if tg.via.is_none() && (d <= v || d > 3000. || p.cross_t > 20.) {
                (p.edge, p.side, p.s, p.dir) = (tg.edge, tg.side, tg.s, tg.dir);
                p.state = PedState::Walk;
                p.target = None;
                p.cross_t = 0.;
                (p.x, p.y) = cx.sw.point(cx.city, p.edge, p.side, p.s);
            } else if d > 0. {
                let k = v.min(d) / d;
                move_with_collision(p, dx * k, dy * k, cx);
            }
        }
        PedState::Flee => {
            p.t -= dt;
            let (dx, dy) = (p.x - p.threat.0, p.y - p.threat.1);
            let d = dx.hypot(dy);
            let d = if d > 0. { d } else { 1. };
            let (ox, oy) = (p.x, p.y);
            move_with_collision(p, dx / d * RUN * dt, dy / d * RUN * dt, cx);
            if cx.city.in_building(p.x, p.y).is_some() {
                p.x = ox;
                p.y = oy;
                p.t = 0.;
            }
            if p.t <= 0. {
                match nearest_spot(cx.city, cx.sw, p.x, p.y, 600.) {
                    Some(sp) => {
                        (p.edge, p.side, p.s) = (sp.edge, sp.side, sp.s);
                        p.state = PedState::Return;
                    }
                    None => p.state = PedState::Idle,
                }
            }
        }
        PedState::Return => {
            let (tx, ty) = cx.sw.point(cx.city, p.edge, p.side, p.s);
            let (dx, dy) = (tx - p.x, ty - p.y);
            let d = dx.hypot(dy);
            let v = p.speed * 1.2 * dt;
            p.return_t += dt;
            if d < p.return_best - 5. {
                p.return_best = d;
                p.return_t = 0.;
            }
            if p.return_t > 4. || d <= v.max(2.) || d > 1500. {
                (p.x, p.y) = (tx, ty);
                p.state = PedState::Walk;
                p.return_t = 0.;
                p.return_best = f64::INFINITY;
            } else {
                move_with_collision(p, dx / d * v, dy / d * v, cx);
            }
        }
        PedState::Dead => {
            p.dead_t += dt;
            return;
        }
        PedState::Down => {
            p.t -= dt;
            if p.t <= 0. {
                p.state = PedState::Idle;
                let th = p.threat;
                scare(p, th, 2.);
            }
        }
    }
    let (mx, my) = (p.x - px, p.y - py);
    if mx * mx + my * my > 0.01 {
        p.facing = my.atan2(mx);
        p.step += mx.hypot(my);
    }
}
