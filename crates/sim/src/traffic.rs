//! Verkehr (Port von `traffic.js`): Rechtsverkehr auf dem echten Straßennetz. KI-Fahrer folgen ihrer Spur (Pure
//! Pursuit), wählen an Kreuzungen die nächste Spur, bremsen vor Kurven, roten Ampeln, Zebrastreifen und
//! Hindernissen, lösen Blockaden und fahren sich frei.
//!
//! Vorfahrt = Reservierungen, nicht StVO: Kreuzungen ohne Ampel (eine Zufahrt zur Zeit, Kolonne darf nachrücken,
//! kreuzungsfreie Bewegungen dürfen mit hinein) und Engstellen (eine Richtung zur Zeit). Die Reservierungen gehören
//! der Welt (`Reservations`); jedes Auto führt seine eigenen Ansprüche dort.
use crate::car::{Car, Driver};
use crate::city::{City, Pt, point_along};
use crate::collision::Grid;
use crate::events::Event;
use crate::levels::touch;
use crate::math::{Rng, wrap_angle};
use crate::roadgraph::{LaneGraph, LaneId, Light, connector, lane_dir, signal_state, turn_angle};
use std::collections::{BTreeSet, HashMap};

pub const LOOKAHEAD: f64 = 420.;
pub const GAP_PX: f64 = 57.;
pub const GATE_STOP: f64 = 24.;
pub const NARROW_WAIT: f64 = 50.;
pub const ENTRY_LOOK: f64 = 100.;
pub const CLAIM_AT: f64 = 45.;
pub const PLATOON_S: f64 = 6.;
pub const HOLD_STILL_S: f64 = 2.;
pub const REROUTE_S: f64 = 3.;
pub const BLINK_AHEAD: f64 = 110.;

/// Routenstück: ab Routenpunkt `k0` gehört die Route zu `lane`; `k_end` = letzter Punkt vor der Kreuzung.
#[derive(Debug, Clone)]
pub struct Seg {
    pub uid: u64,
    pub lane: LaneId,
    pub k0: i64,
    pub k_end: Option<i64>,
}
#[derive(Debug, Clone, Copy)]
pub struct Stop {
    pub k: i64,
    pub v: i64,
    pub heading: f64,
}
/// KI-Zustand eines Autos.
#[derive(Debug, Clone)]
pub struct Ai {
    pub route: Vec<Pt>,
    pub cap: Vec<f64>,
    pub i: usize,
    pub lane: LaneId,
    pub stops: Vec<Stop>,
    pub cruise_k: f64,
    pub segs: Vec<Seg>,
    pub blocked_t: f64,
    pub stuck_t: f64,
    pub reverse_t: f64,
    pub horn_t: f64,
    pub still_t: f64,
    pub denied_t: f64,
    pub head_on: u32,
    pub hold: f64,
    pub blink: i8,
    pub light: Option<Light>,
    next_uid: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimKind {
    /// bis das Auto auf dem Stück ist
    Mark,
    /// Kreuzung ohne Ampel (Knoten)
    Junction(i64),
    /// Engstelle (Gruppenschlüssel)
    Narrow(i64),
}
#[derive(Debug, Clone, Copy)]
pub struct Claim {
    pub kind: ClaimKind,
    pub seg: u64,
    pub lane: Option<LaneId>,
}
#[derive(Debug, Clone, Copy)]
pub struct Move {
    pub ax: f64,
    pub ay: f64,
    pub bx: f64,
    pub by: f64,
    pub to: LaneId,
}
#[derive(Debug, Clone)]
pub struct JRes {
    pub approach: (i64, i8),
    pub cars: BTreeSet<u32>,
    pub since: f64,
    pub moves: HashMap<u32, Move>,
}
#[derive(Debug, Clone)]
pub struct NRes {
    pub dir: i8,
    pub cars: BTreeSet<u32>,
}
/// Reservierungen der Welt plus die Ansprüche je Auto.
#[derive(Debug, Default, Clone)]
pub struct Reservations {
    pub jres: HashMap<i64, JRes>,
    pub nres: HashMap<i64, NRes>,
    pub claims: HashMap<u32, Vec<Claim>>,
    narrow_keys: HashMap<i64, i64>,
    keys_gen: u64,
    nres_gen: u64,
}

/// Momentaufnahme eines Verkehrsteilnehmers für die Hinderniserkennung (während der KI-Schleife bewegt sich nichts).
#[derive(Debug, Clone, Copy)]
pub struct Agent {
    pub id: u32,
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub angle: f64,
    pub hw: f64,
    pub hh: f64,
    pub lvl: i8,
    pub driver: Option<Driver>,
    pub wrecked: bool,
}
impl Agent {
    pub fn of(c: &Car) -> Self {
        Self {
            id: c.id,
            x: c.x,
            y: c.y,
            vx: c.vx,
            vy: c.vy,
            angle: c.angle,
            hw: c.hw,
            hh: c.hh,
            lvl: c.lvl(),
            driver: c.driver,
            wrecked: c.wrecked,
        }
    }
}
/// Fußgänger für die Hinderniserkennung.
#[derive(Debug, Clone, Copy)]
pub struct Walker {
    pub x: f64,
    pub y: f64,
    pub lvl: i8,
    pub alive: bool,
}

/// Was die KI aus der Welt braucht.
pub struct Ctx<'a> {
    pub city: &'a mut City,
    pub lanes: &'a mut LaneGraph,
    pub agents: &'a [Agent],
    pub walkers: &'a [Walker],
    pub agent_grid: Option<&'a Grid>,
    pub walker_grid: Option<&'a Grid>,
    /// Spieler zu Fuß: (x, y, Ebene)
    pub player_on_foot: Option<(f64, f64, i8)>,
    pub rng: &'a mut Rng,
    pub time: f64,
    pub res: &'a mut Reservations,
    pub events: &'a mut Vec<Event>,
}

fn turn_speed(angle: f64) -> f64 {
    let a = angle.abs();
    if a < 0.25 {
        f64::INFINITY
    } else {
        (150. - a * 70.).clamp(38., 140.)
    }
}

impl Ai {
    fn new(lane: LaneId, cruise_k: f64) -> Self {
        Self {
            route: Vec::new(),
            cap: Vec::new(),
            i: 0,
            lane,
            stops: Vec::new(),
            cruise_k,
            segs: Vec::new(),
            blocked_t: 0.,
            stuck_t: 0.,
            reverse_t: 0.,
            horn_t: 0.,
            still_t: 0.,
            denied_t: 0.,
            head_on: 0,
            hold: 0.,
            blink: 0,
            light: None,
            next_uid: 0,
        }
    }
    fn append_lane(&mut self, lanes: &LaneGraph, city: &City, lane: LaneId, from_s: f64) {
        let Some(l) = lanes.lane(lane) else { return };
        let uid = self.next_uid;
        self.next_uid += 1;
        self.segs.push(Seg {
            uid,
            lane,
            k0: self.route.len() as i64,
            k_end: None,
        });
        if from_s > 0. {
            let q = point_along(&l.pts, from_s.min(l.len));
            self.route.push((q.x, q.y));
            self.cap.push(l.cruise);
        }
        let mut acc = 0.;
        for (i, &p) in l.pts.iter().enumerate() {
            if i > 0 {
                acc += (p.0 - l.pts[i - 1].0).hypot(p.1 - l.pts[i - 1].1);
            }
            if acc <= from_s && from_s > 0. {
                continue;
            }
            self.route.push(p);
            self.cap.push(l.cruise);
        }
        self.lane = lane;
        if city.signals.contains(&l.to) {
            let (ux, uy) = lane_dir(l, true);
            self.stops.push(Stop {
                k: self.route.len() as i64 - 1,
                v: l.to,
                heading: uy.atan2(ux),
            });
        }
    }
    fn extend_route(
        &mut self,
        lanes: &mut LaneGraph,
        city: &City,
        rng: &mut Rng,
        forced: Option<LaneId>,
    ) -> bool {
        let Some(next) = forced.or_else(|| lanes.choose_next(city, self.lane, rng)) else {
            return false;
        };
        let (Some(cur), Some(nl)) = (lanes.lane(self.lane), lanes.lane(next)) else {
            return false;
        };
        let v = turn_speed(turn_angle(cur, nl))
            .min(nl.cruise)
            .min(cur.cruise);
        let con = connector(cur, nl);
        if let Some(s) = self.segs.last_mut() {
            s.k_end = Some(self.route.len() as i64 - 1);
        }
        if let Some(c) = self.cap.last_mut() {
            *c = c.min(v);
        }
        for p in con {
            self.route.push(p);
            self.cap.push(v);
        }
        self.append_lane(lanes, city, next, 0.);
        true
    }
    fn remaining(&self, x: f64, y: f64) -> f64 {
        let r = &self.route;
        let Some(&(nx, ny)) = r.get(self.i + 1) else {
            return 0.;
        };
        let mut l = (nx - x).hypot(ny - y);
        let mut k = self.i + 1;
        while k + 1 < r.len() && l < LOOKAHEAD {
            l += (r[k + 1].0 - r[k].0).hypot(r[k + 1].1 - r[k].1);
            k += 1;
        }
        l
    }
    pub fn current_seg(&self) -> usize {
        let mut j = 0;
        for (k, s) in self.segs.iter().enumerate() {
            if s.k0 <= self.i as i64 {
                j = k;
            }
        }
        j
    }
    fn route_dist(&self, x: f64, y: f64, k: i64) -> f64 {
        let r = &self.route;
        let Some(&(nx, ny)) = r.get(self.i + 1) else {
            return 0.;
        };
        let mut d = (nx - x).hypot(ny - y);
        let mut q = self.i as i64 + 1;
        while q < k && d < 400. && (q as usize) + 1 < r.len() {
            let (a, b) = (r[q as usize], r[q as usize + 1]);
            d += (b.0 - a.0).hypot(b.1 - a.1);
            q += 1;
        }
        d
    }
}

pub fn init_ai(
    car: &mut Car,
    lanes: &mut LaneGraph,
    city: &City,
    lane: LaneId,
    s: f64,
    rng: &mut Rng,
) {
    let mut ai = Ai::new(lane, 0.85 + rng.float() * 0.3);
    ai.append_lane(lanes, city, lane, s);
    if ai.route.len() < 2 {
        ai.extend_route(lanes, city, rng, None);
    }
    car.ai = Some(Box::new(ai));
}

/// Setzt ein Auto auf eine Spur (Bogenlänge s) und richtet es aus.
pub fn place_on_lane(
    car: &mut Car,
    lanes: &mut LaneGraph,
    city: &City,
    lane: LaneId,
    s: f64,
    rng: &mut Rng,
) {
    if let Some(l) = lanes.lane(lane) {
        let p = &l.pts;
        let mut acc = 0.;
        for i in 0..p.len() - 1 {
            let len = (p[i + 1].0 - p[i].0).hypot(p[i + 1].1 - p[i].1);
            if acc + len >= s || i == p.len() - 2 {
                let t = if len > 0. {
                    ((s - acc) / len).clamp(0., 1.)
                } else {
                    0.
                };
                car.x = p[i].0 + (p[i + 1].0 - p[i].0) * t;
                car.y = p[i].1 + (p[i + 1].1 - p[i].1) * t;
                car.angle = (p[i + 1].1 - p[i].1).atan2(p[i + 1].0 - p[i].0);
                break;
            }
            acc += len;
        }
    }
    car.vx = 0.;
    car.vy = 0.;
    car.ang_vel = 0.;
    init_ai(car, lanes, city, lane, s, rng);
}

/// Nächste Spur zum Auto finden (nach Unfall, Abdrängen, Übernahme durch die KI).
pub fn replan(car: &mut Car, lanes: &mut LaneGraph, city: &City, rng: &mut Rng) {
    let bus = car.kind == "bus";
    let hit = lanes
        .nearest_lane(car.x, car.y, Some(car.angle), 600., bus)
        .or_else(|| lanes.nearest_lane(car.x, car.y, Some(car.angle), 3000., bus));
    let Some(hit) = hit else {
        car.ai = None;
        return;
    };
    let l = &lanes.lanes[&hit.lane];
    let p = &l.pts;
    let mut s: f64 = p[..=hit.i]
        .windows(2)
        .map(|w| (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1))
        .sum();
    s += (hit.x - p[hit.i].0).hypot(hit.y - p[hit.i].1) + 30.;
    let keep = car.ai.take();
    init_ai(car, lanes, city, hit.lane, s, rng);
    if let (Some(k), Some(ai)) = (keep, car.ai.as_mut()) {
        ai.cruise_k = k.cruise_k;
        ai.hold = k.hold;
    }
}

// --- Hindernisse ---------------------------------------------------------------------------------

struct Obstacles {
    d_car: f64,
    d_other: f64,
    player_block: bool,
    blocker: Option<Agent>,
    ped_block: bool,
}

/// Weg der nächsten ~150 px ab der Fahrzeugmitte entlang der Route.
fn ahead_path(car: &Car, ai: &Ai) -> Option<Vec<Pt>> {
    let r = &ai.route;
    if r.len() < 2 {
        return None;
    }
    let mut out = vec![(car.x, car.y)];
    let (mut acc, mut x, mut y) = (0., car.x, car.y);
    let mut k = ai.i + 1;
    while k < r.len() && acc < 150. + car.hw {
        let (nx, ny) = r[k];
        acc += (nx - x).hypot(ny - y);
        out.push((nx, ny));
        x = nx;
        y = ny;
        k += 1;
    }
    (out.len() >= 2).then_some(out)
}

fn obstacle_ahead(car: &Car, ai: &Ai, cx: &mut Ctx) -> Obstacles {
    let (s, c) = car.angle.sin_cos();
    let mut o = Obstacles {
        d_car: f64::INFINITY,
        d_other: f64::INFINITY,
        player_block: false,
        blocker: None,
        ped_block: false,
    };
    let path = ahead_path(car, ai);
    let my_l = car.hw - 21.;
    let my_w = car.hh - 10.;
    let me = (car.x, car.y, car.lvl());
    let mut check = |cx: &mut Ctx,
                     ox: f64,
                     oy: f64,
                     lvl: i8,
                     lat: f64,
                     obj: Option<&Agent>,
                     is_ai: bool,
                     is_player: bool,
                     is_ped: bool| {
        let (rx, ry) = (ox - car.x, oy - car.y);
        let ol = obj.map(|a| a.hw - 21.).unwrap_or(0.);
        let ext = my_l + ol;
        if rx * rx + ry * ry > (110. + ext).powi(2) {
            return;
        }
        if !touch(cx.city, me, (ox, oy, lvl)) {
            return;
        }
        let lat = lat + my_w + obj.map(|a| a.hh - 10.).unwrap_or(0.);
        let (mut along, side);
        if let Some(path) = &path {
            along = f64::INFINITY;
            let mut sd = f64::INFINITY;
            let mut acc = 0.;
            for (k, w) in path.windows(2).enumerate() {
                let ((ax, ay), (bx, by)) = (w[0], w[1]);
                let (dx, dy) = (bx - ax, by - ay);
                let l = dx.hypot(dy);
                let l = if l > 0. { l } else { 1. };
                let u = (((ox - ax) * dx + (oy - ay) * dy) / l).clamp(0., l);
                let d = (ax + dx * u / l - ox).hypot(ay + dy * u / l - oy);
                if d < sd && !(k == 0 && u == 0.) {
                    sd = d;
                    along = acc + u;
                }
                acc += l;
            }
            side = sd;
        } else {
            along = rx * c + ry * s;
            side = (-rx * s + ry * c).abs();
        }
        if along <= 0. || along > 110. + ext || side > lat {
            return;
        }
        let along = (along - ext).max(1.);
        if is_ai {
            if along < o.d_car {
                o.d_car = along;
                o.blocker = obj.copied();
            }
        } else {
            if along < o.d_other {
                o.d_other = along;
                o.ped_block = is_ped;
            }
            if is_player {
                o.player_block = true;
            }
        }
    };
    let reach = 110. + my_l + 60.;
    let mut idx = Vec::new();
    match cx.agent_grid {
        Some(g) => g.near(car.x, car.y, reach, &mut idx),
        None => idx.extend(0..cx.agents.len()),
    }
    for &k in &idx {
        let a = cx.agents[k];
        if a.id == car.id {
            continue;
        }
        let parked_like = a.driver.is_none() && a.vx.abs() + a.vy.abs() < 10.;
        check(
            cx,
            a.x,
            a.y,
            a.lvl,
            if parked_like { 18. } else { 22. },
            Some(&a),
            a.driver == Some(Driver::Npc) && !a.wrecked,
            a.driver == Some(Driver::Player),
            false,
        );
    }
    let mut pidx = Vec::new();
    match cx.walker_grid {
        Some(g) => g.near(car.x, car.y, reach, &mut pidx),
        None => pidx.extend(0..cx.walkers.len()),
    }
    for &k in &pidx {
        let p = cx.walkers[k];
        if p.alive {
            check(cx, p.x, p.y, p.lvl, 16., None, false, false, true);
        }
    }
    if let Some((px, py, pl)) = cx.player_on_foot {
        check(cx, px, py, pl, 17., None, false, true, false);
    }
    o
}

/// Abstand zum nächsten Zebrastreifen voraus, an dem ein Fußgänger steht oder geht.
fn zebra_ahead(car: &Car, cx: &mut Ctx) -> f64 {
    let (s, c) = car.angle.sin_cos();
    let mut best = f64::INFINITY;
    let mut seen = Vec::new();
    for h in cx
        .city
        .edge_segs
        .query(&crate::collision::Rect::around(car.x, car.y, 200.))
    {
        let Some(id) = cx.city.edge_segs.get(h).edge else {
            continue;
        };
        if seen.contains(&id) {
            continue;
        }
        seen.push(id);
        let Some(e) = cx.city.edges.get(&id) else {
            continue;
        };
        for z in e.crossings.iter().filter(|z| z.kind == 0) {
            let (rx, ry) = (z.x - car.x, z.y - car.y);
            let along = rx * c + ry * s;
            if along <= 0. || along > 200. || (-rx * s + ry * c).abs() > e.w / 2. + 20. {
                continue;
            }
            let reach = e.w / 2. + 25.;
            if cx
                .walkers
                .iter()
                .any(|p| p.alive && (p.x - z.x).hypot(p.y - z.y) < reach && p.lvl == car.lvl())
            {
                best = best.min(along);
            }
        }
    }
    best
}

// --- Fahren --------------------------------------------------------------------------------------

/// Ein KI-Schritt: setzt `car.controls` (und hält ein stehendes Auto fest).
pub fn drive_ai(car: &mut Car, cx: &mut Ctx, dt: f64) {
    if car.wrecked {
        return;
    }
    if car.ai.as_ref().is_none_or(|a| a.route.len() < 2) {
        replan(car, cx.lanes, cx.city, cx.rng);
        let Some(ai) = car.ai.as_ref() else { return };
        let (seg, lane) = (ai.segs[0].uid, ai.segs[0].lane);
        claim_narrow(cx, car.id, lane, seg);
    }
    if cx.res.nres_gen != cx.city.generation {
        rekey_narrow(cx);
    }
    let mut ai = car.ai.take().expect("KI fehlt");
    let finished = drive_inner(car, &mut ai, cx, dt);
    if finished != Some(true) {
        car.ai = Some(ai);
    }
    if finished == Some(false) {
        // weit abgekommen: neu planen
        replan(car, cx.lanes, cx.city, cx.rng);
        if let Some(ai) = car.ai.as_ref() {
            let (seg, lane) = (ai.segs[0].uid, ai.segs[0].lane);
            claim_narrow(cx, car.id, lane, seg);
        }
    }
}

/// `Some(false)` = neu planen, `None` = normal.
fn drive_inner(car: &mut Car, ai: &mut Ai, cx: &mut Ctx, dt: f64) -> Option<bool> {
    // Fortschritt: zum Segment weiterschalten, das vor dem Auto liegt
    let (mut px, mut py) = (car.x, car.y);
    while let (Some(&(ax, ay)), Some(&(bx, by))) = (ai.route.get(ai.i), ai.route.get(ai.i + 1)) {
        let (dx, dy) = (bx - ax, by - ay);
        let l2 = dx * dx + dy * dy;
        let l2 = if l2 > 0. { l2 } else { 1. };
        let t = ((car.x - ax) * dx + (car.y - ay) * dy) / l2;
        px = ax + dx * t.clamp(0., 1.);
        py = ay + dy * t.clamp(0., 1.);
        if t > 1. || (bx - car.x).hypot(by - car.y) < 10. {
            ai.i += 1;
            if ai.i + 1 >= ai.route.len() && !ai.extend_route(cx.lanes, cx.city, cx.rng, None) {
                break;
            }
            continue;
        }
        break;
    }
    if ai.i > 24 {
        let n = ai.i - 2;
        ai.route.drain(..n);
        ai.cap.drain(..n);
        ai.i = 2;
        let n = n as i64;
        for st in &mut ai.stops {
            st.k -= n;
        }
        for sg in &mut ai.segs {
            sg.k0 -= n;
            if let Some(k) = sg.k_end.as_mut() {
                *k -= n;
            }
        }
        while ai.segs.len() > 1 && ai.segs[1].k0 <= 0 && ai.segs[0].k_end.is_some_and(|k| k < 0) {
            ai.segs.remove(0);
        }
    }
    while ai.remaining(car.x, car.y) < LOOKAHEAD {
        if !ai.extend_route(cx.lanes, cx.city, cx.rng, None) {
            break;
        }
    }
    if (px - car.x).hypot(py - car.y) > 260. {
        return Some(false);
    }
    let r = &ai.route;
    let vf = car.forward_speed();
    let look = (vf.abs() * 0.35).clamp(36., 90.);
    let (mut aim_x, mut aim_y, mut acc, mut found) = (px, py, 0., false);
    let mut target = ai.cap.get(ai.i).copied().unwrap_or(100.) * ai.cruise_k;
    let tr = car.traction;
    let kb = tr.brake;
    target *= 0.6 + 0.4 * kb;
    let (mut x0, mut y0) = (px, py);
    for (k, &(x1, y1)) in r.iter().enumerate().skip(ai.i + 1) {
        let l = (x1 - x0).hypot(y1 - y0);
        if !found && acc + l >= look {
            let u = (look - acc) / if l > 0. { l } else { 1. };
            aim_x = x0 + (x1 - x0) * u;
            aim_y = y0 + (y1 - y0) * u;
            found = true;
        }
        acc += l;
        let cap = ai.cap[k];
        if acc < 220. && cap < target {
            target = target.min((cap * cap + 2. * 260. * kb * (acc - 20.).max(0.)).sqrt());
        }
        x0 = x1;
        y0 = y1;
        if acc > 240. && found {
            break;
        }
    }
    if !found {
        aim_x = x0;
        aim_y = y0;
    }
    let (dx, dy) = (aim_x - car.x, aim_y - car.y);
    let ctl = &mut car.controls;
    if ai.reverse_t > 0. {
        ai.reverse_t -= dt;
        ctl.throttle = 0.;
        ctl.brake = 1.;
        ctl.handbrake = false;
        ctl.steer = -(wrap_angle(dy.atan2(dx) - car.angle) * 2.).clamp(-1., 1.);
        return None;
    }
    let diff = wrap_angle(dy.atan2(dx) - car.angle);
    ctl.steer = (diff * 2.4).clamp(-1., 1.);
    if diff.abs() > 0.6 {
        target = target.min(55. * tr.lat);
    }
    // Ampel: bei Rot (und Gelb, wenn noch Bremsweg bleibt) an der Haltelinie halten
    let along = |q: &Stop| {
        let (sx, sy) = r[q.k.max(0) as usize];
        (sx - car.x) * q.heading.cos() + (sy - car.y) * q.heading.sin()
    };
    while let Some(st) = ai.stops.first() {
        if st.k < ai.i as i64 - 3 || st.k < 0 || along(st) < -8. {
            ai.stops.remove(0);
        } else {
            break;
        }
    }
    ai.light = None;
    if let Some(st) = ai.stops.first().copied() {
        let mut dist = along(&st);
        if st.k > ai.i as i64 + 1 {
            dist = ai.route_dist(car.x, car.y, st.k);
        }
        if dist < 400. {
            let light = signal_state(cx.city, st.v, st.heading, cx.time);
            let brake_dist = vf * vf / (2. * 300. * kb);
            if light == Light::Red || (light == Light::Yellow && dist > brake_dist + 10.) {
                target = target.min((2. * 90. * kb * (dist - 15.).max(0.)).sqrt());
            }
            ai.light = Some(light);
        }
    }
    let zc = zebra_ahead(car, cx);
    if zc < 200. {
        target = target.min((2. * 90. * kb * (zc - 30.).max(0.)).sqrt());
    }
    let ob = obstacle_ahead(car, ai, cx);
    let d = ob.d_other.min(ob.d_car);
    let gate = entry_gate(car, ai, cx, d > 70. && vf > -2., dt);
    ai.blink = blink_for(ai, cx.lanes, car.x, car.y);
    if gate < f64::INFINITY {
        target = target.min((2. * 90. * kb * (gate - GATE_STOP).max(0.)).sqrt());
    }
    ai.still_t = if vf.abs() < 5. { ai.still_t + dt } else { 0. };
    release_claims(car, ai, cx, ai.still_t);
    // Selbstheilung: wer auf einer Engstelle fährt, hält sie auch
    let j = ai.current_seg();
    if let Some(here) = ai.segs.get(j)
        && cx.lanes.lane(here.lane).is_some_and(|l| l.narrow)
    {
        let edge = cx.lanes.lanes[&here.lane].edge;
        let nk = narrow_key(cx, edge);
        let has = cx
            .res
            .claims
            .get(&car.id)
            .is_some_and(|v| v.iter().any(|c| c.kind == ClaimKind::Narrow(nk)));
        if !has {
            claim_narrow(cx, car.id, here.lane, here.uid);
        }
    }
    if vf > 40. {
        ai.head_on = 0;
    }
    if d < 125. / kb {
        target = target.min(((d - GAP_PX) * 2.2 * kb).max(0.));
    }
    if ai.hold > 0. {
        ai.hold -= dt;
        target = 0.;
    }
    if target < 1. && vf.abs() < 6. {
        ai.blocked_t += dt;
        if let Some(b) = ob.blocker
            && ob.d_car <= ob.d_other
            && ai.blocked_t > 3.5
            && (b.angle - car.angle).cos() < 0.5
        {
            let mine = cx
                .res
                .claims
                .get(&car.id)
                .is_some_and(|v| v.iter().any(|c| matches!(c.kind, ClaimKind::Narrow(_))));
            let theirs = cx
                .res
                .claims
                .get(&b.id)
                .is_some_and(|v| v.iter().any(|c| matches!(c.kind, ClaimKind::Narrow(_))));
            let back = if theirs && !mine {
                true
            } else if mine && !theirs {
                false
            } else {
                car.id > b.id
            };
            if back {
                ai.reverse_t = 1.2;
                ai.blocked_t = 0.;
                ai.head_on += 1;
            }
        }
        if ((ob.player_block && ai.blocked_t > 1.5)
            || (ob.ped_block && ob.d_other < ob.d_car && ai.blocked_t > 2.5))
            && ai.horn_t <= 0.
        {
            cx.events.push(Event::Horn {
                x: car.x,
                y: car.y,
                npc: true,
            });
            ai.horn_t = 3.;
        }
    } else {
        ai.blocked_t = 0.;
    }
    ai.horn_t -= dt;
    let ctl = &mut car.controls;
    if target < 3. {
        ctl.throttle = 0.;
        ctl.brake = if vf > 1. { 1. } else { 0. };
        ctl.handbrake = false;
        if vf.abs() < 4. {
            car.vx = 0.;
            car.vy = 0.;
            car.ang_vel = 0.;
        }
        return None;
    }
    if vf < target - 8. {
        ctl.throttle = ((target - vf) / 60.).clamp(0.25, 1.);
        ctl.brake = 0.;
    } else if vf > target + 8. {
        ctl.throttle = 0.;
        ctl.brake = ((vf - target) / 70.).clamp(0.25, 1.);
    } else {
        ctl.throttle = 0.15;
        ctl.brake = 0.;
    }
    ctl.handbrake = false;
    if ctl.throttle > 0.3 && vf.abs() < 8. && d >= 110. {
        ai.stuck_t += dt;
        if ai.stuck_t > 1.8 {
            ai.reverse_t = 1.;
            ai.stuck_t = 0.;
        }
    } else {
        ai.stuck_t = 0.;
    }
    None
}

/// Blinker (Anzeige): +1 rechts, −1 links, 0 geradeaus.
fn blink_for(ai: &Ai, lanes: &LaneGraph, x: f64, y: f64) -> i8 {
    if ai.segs.is_empty() {
        return 0;
    }
    let j = ai.current_seg();
    let (Some(cur), Some(next)) = (ai.segs.get(j), ai.segs.get(j + 1)) else {
        return 0;
    };
    let Some(k_end) = cur.k_end else { return 0 };
    if k_end >= ai.i as i64 && ai.route_dist(x, y, k_end) > BLINK_AHEAD {
        return 0;
    }
    let (Some(a), Some(b)) = (lanes.lane(cur.lane), lanes.lane(next.lane)) else {
        return 0;
    };
    let t = turn_angle(a, b);
    if t > 0.5 {
        1
    } else if t < -0.5 {
        -1
    } else {
        0
    }
}

// --- Reservierungen ------------------------------------------------------------------------------

/// Zusammenhängende enge Abschnitte derselben Straße sind EINE Engstelle; Schlüssel = kleinste Kantennummer.
pub fn narrow_key(cx: &mut Ctx, edge: i64) -> i64 {
    if cx.res.keys_gen != cx.city.generation {
        cx.res.narrow_keys.clear();
        cx.res.keys_gen = cx.city.generation;
    }
    if let Some(&k) = cx.res.narrow_keys.get(&edge) {
        return k;
    }
    let is_narrow = |e: i64| {
        cx.lanes.by_edge.get(&e).is_some_and(|v| {
            v.iter()
                .any(|l| cx.lanes.lanes.get(l).is_some_and(|l| l.narrow))
        })
    };
    let name = cx
        .city
        .edges
        .get(&edge)
        .map(|e| e.name.clone())
        .unwrap_or_default();
    let mut seen = vec![edge];
    let mut todo = vec![edge];
    let mut key = edge;
    while let Some(e) = todo.pop() {
        let Some(ed) = cx.city.edges.get(&e) else {
            continue;
        };
        for v in [ed.a, ed.b] {
            for &k in cx
                .city
                .nodes
                .get(&v)
                .map(|n| n.edges.as_slice())
                .unwrap_or(&[])
            {
                if seen.contains(&k) {
                    continue;
                }
                let Some(o) = cx.city.edges.get(&k) else {
                    continue;
                };
                if name.is_empty() || o.name != name || !is_narrow(k) {
                    continue;
                }
                seen.push(k);
                todo.push(k);
                key = key.min(k);
            }
        }
    }
    for k in seen {
        cx.res.narrow_keys.insert(k, key);
    }
    key
}

/// Fahrtrichtung auf der Engstelle (+1 ungefähr in Richtung der Bezugskante).
fn narrow_dir(cx: &mut Ctx, lane: LaneId) -> i8 {
    let Some(l) = cx.lanes.lane(lane) else {
        return 1;
    };
    let edge = l.edge;
    let key = narrow_key(cx, edge);
    let l = &cx.lanes.lanes[&lane];
    let Some(r) = cx.city.edges.get(&key).or_else(|| cx.city.edges.get(&edge)) else {
        return 1;
    };
    let (a, b) = (r.pts[0], *r.pts.last().expect("Kante"));
    let (c, d) = (l.pts[0], *l.pts.last().expect("Spur"));
    if (b.0 - a.0) * (d.0 - c.0) + (b.1 - a.1) * (d.1 - c.1) >= 0. {
        1
    } else {
        -1
    }
}

/// Endet diese Engstelle (in Fahrtrichtung) in einer Sackgasse, an der nur Wenden bleibt?
fn dead_end_narrow(cx: &mut Ctx, lane: LaneId) -> bool {
    let Some(edge) = cx.lanes.lane(lane).map(|l| l.edge) else {
        return false;
    };
    let nk = narrow_key(cx, edge);
    let mut l = Some(lane);
    for _ in 0..30 {
        let Some(cur) = l else { break };
        let ce = cx.lanes.lanes.get(&cur).map(|x| x.edge);
        let nx = cx.lanes.next(cx.city, cur, false);
        if nx.is_empty()
            || nx
                .iter()
                .all(|m| cx.lanes.lanes.get(m).map(|x| x.edge) == ce)
        {
            return true;
        }
        l = None;
        for m in nx {
            let Some(ml) = cx.lanes.lanes.get(&m) else {
                continue;
            };
            if ml.narrow && Some(ml.edge) != ce {
                let me = ml.edge;
                if narrow_key(cx, me) == nk {
                    l = Some(m);
                    break;
                }
            }
        }
    }
    false
}

/// Reservierung gilt nur, solange ihr Auto noch KI-gefahren und intakt ist und sie selbst noch führt.
fn prune(cx: &Ctx, cars: &BTreeSet<u32>, owns: ClaimKind) -> BTreeSet<u32> {
    cars.iter()
        .copied()
        .filter(|id| {
            let alive = cx
                .agents
                .iter()
                .find(|a| a.id == *id)
                .is_some_and(|a| a.driver == Some(Driver::Npc) && !a.wrecked);
            alive
                && cx
                    .res
                    .claims
                    .get(id)
                    .is_some_and(|v| v.iter().any(|c| c.kind == owns))
        })
        .collect()
}

fn move_of(cx: &Ctx, from: LaneId, to: LaneId) -> Option<Move> {
    let (a, b) = (cx.lanes.lane(from)?, cx.lanes.lane(to)?);
    let (ax, ay) = *a.pts.last()?;
    Some(Move {
        ax,
        ay,
        bx: b.pts[0].0,
        by: b.pts[0].1,
        to,
    })
}
/// Kreuzen sich zwei Bewegungen (oder kommen sich näher als eine Autobreite, oder münden in dieselbe Spur)?
pub fn moves_conflict(m: &Move, n: &Move) -> bool {
    if m.to == n.to {
        return true;
    }
    let cr = |ax: f64, ay: f64, bx: f64, by: f64, cx: f64, cy: f64| {
        (bx - ax) * (cy - ay) - (by - ay) * (cx - ax)
    };
    let d1 = cr(m.ax, m.ay, m.bx, m.by, n.ax, n.ay);
    let d2 = cr(m.ax, m.ay, m.bx, m.by, n.bx, n.by);
    let d3 = cr(n.ax, n.ay, n.bx, n.by, m.ax, m.ay);
    let d4 = cr(n.ax, n.ay, n.bx, n.by, m.bx, m.by);
    if ((d1 > 0.) != (d2 > 0.)) && ((d3 > 0.) != (d4 > 0.)) {
        return true;
    }
    let ps = |px: f64, py: f64, ax: f64, ay: f64, bx: f64, by: f64| {
        crate::city::seg_dist2(px, py, ax, ay, bx, by).sqrt()
    };
    ps(n.ax, n.ay, m.ax, m.ay, m.bx, m.by)
        .min(ps(n.bx, n.by, m.ax, m.ay, m.bx, m.by))
        .min(ps(m.ax, m.ay, n.ax, n.ay, n.bx, n.by))
        .min(ps(m.bx, m.by, n.ax, n.ay, n.bx, n.by))
        < 20.
}

pub fn may_enter(cx: &mut Ctx, car: &Car, from: LaneId, to: LaneId) -> bool {
    let (Some(fl), Some(tl)) = (cx.lanes.lane(from), cx.lanes.lane(to)) else {
        return false;
    };
    let v = fl.to;
    let (from_edge, from_dir) = (fl.edge, fl.dir);
    let (to_edge, to_narrow) = (tl.edge, tl.narrow);
    let (sx, sy) = tl.pts[0];
    let (ux, uy) = (tl.pts[1].0 - sx, tl.pts[1].1 - sy);
    let len = ux.hypot(uy);
    let len = if len > 0. { len } else { 1. };
    let me = (car.x, car.y, car.lvl());
    for o in cx.agents {
        if o.id == car.id || o.wrecked {
            continue;
        }
        let (rx, ry) = (o.x - sx, o.y - sy);
        let along = (rx * ux + ry * uy) / len;
        let lat = (-rx * uy + ry * ux).abs() / len;
        if along > -10.
            && along < 55.
            && lat < 14.
            && o.vx.hypot(o.vy) < 25.
            && touch(cx.city, me, (o.x, o.y, o.lvl))
        {
            return false;
        }
    }
    if !cx.city.signals.contains(&v)
        && cx.city.is_junction(v)
        && let Some(h) = cx.res.jres.get(&v).cloned()
    {
        {
            let live = prune(cx, &h.cars, ClaimKind::Junction(v));
            if let Some(hm) = cx.res.jres.get_mut(&v) {
                hm.cars = live.clone();
            }
            if !live.is_empty() && !live.contains(&car.id) {
                let mine = move_of(cx, from, to);
                let clear = live.iter().all(|id| match (h.moves.get(id), mine) {
                    (Some(m), Some(me)) => !moves_conflict(m, &me),
                    _ => false,
                });
                if !clear && (h.approach != (from_edge, from_dir) || cx.time - h.since > PLATOON_S)
                {
                    return false;
                }
            }
        }
    }
    if to_narrow {
        let nk = narrow_key(cx, to_edge);
        if let Some(t) = cx.res.nres.get(&nk).cloned() {
            let live = prune(cx, &t.cars, ClaimKind::Narrow(nk));
            if let Some(tm) = cx.res.nres.get_mut(&nk) {
                tm.cars = live.clone();
            }
            if !live.is_empty()
                && !live.contains(&car.id)
                && (t.dir != narrow_dir(cx, to) || dead_end_narrow(cx, to))
            {
                return false;
            }
        }
    }
    true
}

fn push_claim(cx: &mut Ctx, car: u32, c: Claim) {
    cx.res.claims.entry(car).or_default().push(c);
}

fn claim(cx: &mut Ctx, car: &Car, from: LaneId, next: &Seg) {
    let Some(fl) = cx.lanes.lane(from) else {
        return;
    };
    let (v, approach) = (fl.to, (fl.edge, fl.dir));
    let to = next.lane;
    push_claim(
        cx,
        car.id,
        Claim {
            kind: ClaimKind::Mark,
            seg: next.uid,
            lane: None,
        },
    );
    if !cx.city.signals.contains(&v) && cx.city.is_junction(v) {
        push_claim(
            cx,
            car.id,
            Claim {
                kind: ClaimKind::Junction(v),
                seg: next.uid,
                lane: None,
            },
        );
        let mv = move_of(cx, from, to);
        let live = cx
            .res
            .jres
            .get(&v)
            .map(|h| prune(cx, &h.cars, ClaimKind::Junction(v)));
        let fresh = live.as_ref().is_none_or(|l| l.is_empty());
        let time = cx.time;
        let h = cx.res.jres.entry(v).or_insert_with(|| JRes {
            approach,
            cars: BTreeSet::new(),
            since: time,
            moves: HashMap::new(),
        });
        if fresh {
            *h = JRes {
                approach,
                cars: BTreeSet::new(),
                since: time,
                moves: HashMap::new(),
            };
        } else if let Some(l) = live {
            h.cars = l;
        }
        h.cars.insert(car.id);
        if let Some(m) = mv {
            h.moves.insert(car.id, m);
        }
    }
    if cx.lanes.lane(to).is_some_and(|l| l.narrow) {
        claim_narrow(cx, car.id, to, next.uid);
    }
}

pub fn claim_narrow(cx: &mut Ctx, car: u32, lane: LaneId, seg: u64) {
    let Some(l) = cx.lanes.lane(lane) else { return };
    if !l.narrow {
        return;
    }
    let edge = l.edge;
    let nk = narrow_key(cx, edge);
    push_claim(
        cx,
        car,
        Claim {
            kind: ClaimKind::Narrow(nk),
            seg,
            lane: Some(lane),
        },
    );
    let live = cx
        .res
        .nres
        .get(&nk)
        .map(|t| prune(cx, &t.cars, ClaimKind::Narrow(nk)));
    if live.as_ref().is_none_or(|l| l.is_empty()) {
        let dir = narrow_dir(cx, lane);
        cx.res.nres.insert(
            nk,
            NRes {
                dir,
                cars: BTreeSet::new(),
            },
        );
    } else if let (Some(l), Some(t)) = (live, cx.res.nres.get_mut(&nk)) {
        t.cars = l;
    }
    cx.res
        .nres
        .get_mut(&nk)
        .expect("Engstelle")
        .cars
        .insert(car);
}

/// Darf auf dieser Spur (Engstelle) gerade ein Auto erzeugt werden?
pub fn narrow_free(cx: &mut Ctx, lane: LaneId) -> bool {
    let Some(l) = cx.lanes.lane(lane) else {
        return true;
    };
    if !l.narrow {
        return true;
    }
    let edge = l.edge;
    let nk = narrow_key(cx, edge);
    let Some(t) = cx.res.nres.get(&nk).cloned() else {
        return true;
    };
    prune(cx, &t.cars, ClaimKind::Narrow(nk)).is_empty()
        || (t.dir == narrow_dir(cx, lane) && !dead_end_narrow(cx, lane))
}

/// Nach dem Nachladen können Engstellen-Gruppen wachsen: Belegung aus den Ansprüchen neu aufbauen.
fn rekey_narrow(cx: &mut Ctx) {
    cx.res.nres_gen = cx.city.generation;
    cx.res.nres.clear();
    let ids: Vec<u32> = cx.res.claims.keys().copied().collect();
    for id in ids {
        let alive = cx
            .agents
            .iter()
            .find(|a| a.id == id)
            .is_some_and(|a| a.driver == Some(Driver::Npc) && !a.wrecked);
        if !alive {
            continue;
        }
        let claims = cx.res.claims.get(&id).cloned().unwrap_or_default();
        let mut updated = Vec::with_capacity(claims.len());
        for mut c in claims {
            if let (ClaimKind::Narrow(_), Some(lane)) = (c.kind, c.lane) {
                let Some(edge) = cx.lanes.lane(lane).map(|l| l.edge) else {
                    updated.push(c);
                    continue;
                };
                let nk = narrow_key(cx, edge);
                c.kind = ClaimKind::Narrow(nk);
                if !cx.res.nres.contains_key(&nk) {
                    let dir = narrow_dir(cx, lane);
                    cx.res.nres.insert(
                        nk,
                        NRes {
                            dir,
                            cars: BTreeSet::new(),
                        },
                    );
                }
                cx.res.nres.get_mut(&nk).expect("Engstelle").cars.insert(id);
            }
            updated.push(c);
        }
        cx.res.claims.insert(id, updated);
    }
}

/// Abstand bis zur Haltelinie, wenn das Auto vor der nächsten Kreuzung warten muss (sonst ∞).
fn entry_gate(car: &Car, ai: &mut Ai, cx: &mut Ctx, can_go: bool, dt: f64) -> f64 {
    if ai.segs.is_empty() {
        return f64::INFINITY;
    }
    let j = ai.current_seg();
    let (Some(cur), Some(next)) = (ai.segs.get(j).cloned(), ai.segs.get(j + 1).cloned()) else {
        return f64::INFINITY;
    };
    let Some(k_end) = cur.k_end else {
        return f64::INFINITY;
    };
    let has = |cx: &Ctx| {
        cx.res
            .claims
            .get(&car.id)
            .is_some_and(|v| v.iter().any(|c| c.seg == next.uid))
    };
    if k_end < ai.i as i64 {
        if !has(cx) {
            claim(cx, car, cur.lane, &next);
        }
        return f64::INFINITY;
    }
    let dist = ai.route_dist(car.x, car.y, k_end);
    if dist > ENTRY_LOOK || has(cx) {
        return f64::INFINITY;
    }
    if !may_enter(cx, car, cur.lane, next.lane) {
        ai.denied_t += dt;
        if ai.denied_t > REROUTE_S && dist < 60. {
            let alt: Vec<LaneId> = cx
                .lanes
                .next(cx.city, cur.lane, false)
                .into_iter()
                .filter(|&l| l != next.lane)
                .collect();
            let alt: Vec<LaneId> = alt
                .into_iter()
                .filter(|&l| may_enter(cx, car, cur.lane, l))
                .collect();
            if !alt.is_empty() {
                let pick = alt[cx.rng.index(alt.len())];
                reroute_at(ai, j, pick, cx);
                ai.denied_t = 0.;
                return f64::INFINITY;
            }
        }
        let narrow = cx.lanes.lane(next.lane).is_some_and(|l| l.narrow);
        return if narrow { dist - NARROW_WAIT } else { dist };
    }
    ai.denied_t = 0.;
    if (dist < CLAIM_AT && can_go) || dist < 15. {
        claim(cx, car, cur.lane, &next);
    }
    f64::INFINITY
}

/// Route ab dem Ende von Stück j verwerfen und über die Spur `alt` neu aufbauen.
fn reroute_at(ai: &mut Ai, j: usize, alt: LaneId, cx: &mut Ctx) {
    let k = ai.segs[j].k_end.expect("Stück ohne Ende");
    ai.route.truncate((k + 1) as usize);
    ai.cap.truncate((k + 1) as usize);
    ai.segs.truncate(j + 1);
    ai.segs[j].k_end = None;
    ai.stops.retain(|st| st.k <= k);
    ai.lane = ai.segs[j].lane;
    ai.extend_route(cx.lanes, cx.city, cx.rng, Some(alt));
}

/// Reservierungen freigeben: Kreuzung, sobald das Auto hindurch ist; Engstelle, sobald es sie verlassen hat;
/// alles, was vor der Linie steht und nicht losfahren kann.
fn release_claims(car: &Car, ai: &Ai, cx: &mut Ctx, still_t: f64) {
    let Some(claims) = cx.res.claims.get(&car.id).cloned() else {
        return;
    };
    if claims.is_empty() {
        return;
    }
    let j = ai.current_seg();
    let before_line = ai
        .segs
        .get(j)
        .and_then(|s| s.k_end)
        .is_some_and(|k| k >= ai.i as i64);
    let mut keep = Vec::new();
    let mut dropped = Vec::new();
    let cur_lane = ai.segs.get(j).map(|s| s.lane);
    for cl in claims {
        let idx = ai.segs.iter().position(|s| s.uid == cl.seg);
        let ok = match idx {
            None => false,
            Some(idx) if idx == j + 1 && before_line && still_t > HOLD_STILL_S => false,
            Some(idx) => match cl.kind {
                ClaimKind::Mark => idx > j,
                ClaimKind::Narrow(nk) => {
                    if idx >= j {
                        true
                    } else {
                        let edge = cur_lane
                            .and_then(|l| cx.lanes.lane(l))
                            .filter(|l| l.narrow)
                            .map(|l| l.edge);
                        edge.is_some_and(|e| narrow_key(cx, e) == nk)
                    }
                }
                ClaimKind::Junction(v) => {
                    if idx > j {
                        true
                    } else if idx < j {
                        false
                    } else {
                        cx.city
                            .nodes
                            .get(&v)
                            .is_none_or(|nd| (car.x - nd.x).hypot(car.y - nd.y) <= nd.trim + 30.)
                    }
                }
            },
        };
        if ok {
            keep.push(cl);
        } else {
            dropped.push(cl);
        }
    }
    for cl in &dropped {
        match cl.kind {
            ClaimKind::Junction(v) if !keep.iter().any(|c| c.kind == cl.kind) => {
                if let Some(h) = cx.res.jres.get_mut(&v) {
                    h.cars.remove(&car.id);
                }
            }
            ClaimKind::Narrow(nk) if !keep.iter().any(|c| c.kind == cl.kind) => {
                if let Some(t) = cx.res.nres.get_mut(&nk) {
                    t.cars.remove(&car.id);
                }
            }
            _ => {}
        }
    }
    cx.res.claims.insert(car.id, keep);
}

/// Auto aus allen Reservierungen nehmen (beim Abbauen, Umplanen, Übernahme).
pub fn drop_claims(res: &mut Reservations, car: u32) {
    for cl in res.claims.remove(&car).unwrap_or_default() {
        match cl.kind {
            ClaimKind::Junction(v) => {
                if let Some(h) = res.jres.get_mut(&v) {
                    h.cars.remove(&car);
                }
            }
            ClaimKind::Narrow(nk) => {
                if let Some(t) = res.nres.get_mut(&nk) {
                    t.cars.remove(&car);
                }
            }
            ClaimKind::Mark => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn conflicting_moves() {
        // Kreuz: Nord→Süd und West→Ost kreuzen sich, Nord→Süd und Süd→Nord (Gegenverkehr geradeaus) nicht
        let ns = Move {
            ax: 5.,
            ay: -50.,
            bx: 5.,
            by: 50.,
            to: 1,
        };
        let we = Move {
            ax: -50.,
            ay: 5.,
            bx: 50.,
            by: 5.,
            to: 2,
        };
        let sn = Move {
            ax: -25.,
            ay: 50.,
            bx: -25.,
            by: -50.,
            to: 3,
        };
        assert!(moves_conflict(&ns, &we));
        assert!(!moves_conflict(&ns, &sn));
        assert!(
            moves_conflict(&ns, &Move { to: 1, ..sn }),
            "gleiche Zielspur"
        );
        assert_eq!(turn_speed(0.1), f64::INFINITY);
        assert!((turn_speed(std::f64::consts::FRAC_PI_2) - 40.04).abs() < 0.01);
    }
}
