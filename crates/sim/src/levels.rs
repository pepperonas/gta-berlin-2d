//! Ebene je Objekt (Port von `levels.js`): 0 = Boden, ≥ 1 Brücke, < 0 offene Unterführung. Die Ebene wechselt nur
//! in Portalen (Knoten, die Wege verschiedener Ebenen teilen); wer eine Brücke nur unter- oder überquert, bleibt.
use crate::city::{City, PolyKind, Pt, point_in_rings};
use crate::collision::Rect;
use berlin_map_loader::citycodes::area_kind;
use std::f64::consts::PI;

pub const PATH_HALF: f64 = 12.;
pub const MARGIN: f64 = 5.;
pub const CHECK: u32 = 10;
pub const LOST: f64 = 30.;
pub const SLACK: f64 = 25.;
pub const MAX_HALF: f64 = 100.;

/// Zustand eines Objekts für die Ebenenführung.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct LevelState {
    pub lvl: i8,
    pub lost_t: u32,
}

struct Piece {
    lvl: i8,
    d: f64,
    t: f64,
    half: f64,
    angle: f64,
    ends: Option<(Pt, Pt)>,
    path: Option<Vec<Pt>>,
}

fn pieces(city: &mut City, x: f64, y: f64, r: f64) -> Vec<Piece> {
    let mut out = Vec::new();
    for h in city.edge_segs.query(&Rect::around(x, y, r)) {
        let s = *city.edge_segs.get(h);
        let Some(e) = s.edge.and_then(|id| city.edges.get(&id)) else {
            continue;
        };
        let (dx, dy) = (s.bx - s.ax, s.by - s.ay);
        let l2 = dx * dx + dy * dy;
        let l2 = if l2 > 0. { l2 } else { 1. };
        let t = ((x - s.ax) * dx + (y - s.ay) * dy) / l2;
        let tc = t.clamp(0., 1.);
        let d = (s.ax + dx * tc - x).hypot(s.ay + dy * tc - y);
        let cross = (x - s.ax) * -dy + (y - s.ay) * dx;
        let side = if cross < 0. { -1. } else { 1. };
        let tr = if side > 0. {
            e.cs.right.track
        } else {
            e.cs.left.track
        };
        let half = e.w / 2.
            + if e.fill != 0. && e.fill.signum() == side {
                e.fill.abs()
            } else {
                0.
            }
            + if tr > 0. { 4. + tr } else { 0. };
        out.push(Piece {
            lvl: e.lvl,
            d,
            t,
            half,
            angle: dy.atan2(dx),
            ends: Some((e.pts[0], *e.pts.last().expect("Kante ohne Punkte"))),
            path: None,
        });
    }
    for h in city.paths.query(&Rect::around(x, y, r)) {
        let f = city.paths.get(h);
        for w in f.pts.windows(2) {
            let ((ax, ay), (bx, by)) = (w[0], w[1]);
            let (dx, dy) = (bx - ax, by - ay);
            let l2 = dx * dx + dy * dy;
            let l2 = if l2 > 0. { l2 } else { 1. };
            let t = ((x - ax) * dx + (y - ay) * dy) / l2;
            let tc = t.clamp(0., 1.);
            let d = (ax + dx * tc - x).hypot(ay + dy * tc - y);
            if d < r {
                out.push(Piece {
                    lvl: f.lvl,
                    d,
                    t,
                    half: PATH_HALF,
                    angle: dy.atan2(dx),
                    ends: None,
                    path: Some(f.pts.clone()),
                });
            }
        }
    }
    out
}

/// Liegt unter (x, y) eine Fläche der Ebene `lvl`?
pub fn level_here(city: &mut City, x: f64, y: f64, lvl: i8, r: f64) -> bool {
    if lvl == 0 {
        return true;
    }
    if pieces(city, x, y, 60. + r)
        .iter()
        .any(|p| p.lvl == lvl && p.d <= p.half + r)
    {
        return true;
    }
    for h in city.edge_segs.query(&Rect::around(x, y, 1.)) {
        if let Some(j) = city.edge_segs.get(h).junction
            && lvl >= j.lo
            && lvl <= j.hi
            && (x - j.x).hypot(y - j.y) <= j.r + r
        {
            return true;
        }
    }
    for h in city.polys.query(&Rect::around(x, y, 1.)) {
        let p = city.polys.get(h);
        if let PolyKind::Area {
            kind: area_kind::BRIDGE,
            lvl: l,
        } = p.kind
            && (if l != 0 { l } else { 1 }) == lvl
            && point_in_rings(x, y, &p.rings)
        {
            return true;
        }
    }
    false
}

fn angle_mod_pi(a: f64, b: f64) -> f64 {
    let d = ((a - b) % PI + PI) % PI;
    d.min(PI - d)
}

/// Ebene beim Erzeugen/Teleport: die Straße unter einem, die in Blickrichtung verläuft; sonst Boden, wenn dort Boden
/// ist; sonst die einzige vorhandene Ebene.
pub fn initial_level(city: &mut City, x: f64, y: f64, angle: Option<f64>, r: f64) -> i8 {
    let mut best: Option<(i8, f64, bool)> = None;
    let mut ground = false;
    for p in pieces(city, x, y, 60. + r) {
        if p.d > p.half + r {
            continue;
        }
        if p.lvl == 0 {
            ground = true;
        }
        let da = angle.map(|a| angle_mod_pi(a, p.angle)).unwrap_or(0.);
        let score = da * 100. + p.d;
        if best.is_none_or(|b| score < b.1) {
            best = Some((p.lvl, score, da < 0.35));
        }
    }
    match best {
        None => 0,
        Some((lvl, _, aligned)) if angle.is_some() && aligned => lvl,
        Some((lvl, _, _)) => {
            if ground {
                0
            } else {
                lvl
            }
        }
    }
}

fn through(p: &Piece, portals: &[crate::city::Portal]) -> bool {
    for portal in portals {
        if p.lvl < portal.lo || p.lvl > portal.hi {
            continue;
        }
        let near = |(x, y): Pt| (x - portal.x).abs() < 3. && (y - portal.y).abs() < 3.;
        if let Some((a, b)) = p.ends {
            if near(a) || near(b) {
                return true;
            }
            continue;
        }
        match &p.path {
            None => return true,
            Some(path) => {
                if path.iter().any(|&q| near(q)) {
                    return true;
                }
            }
        }
    }
    false
}

/// Ebene eines Objekts einen Schritt weiterführen.
pub fn step_level(city: &mut City, x: f64, y: f64, angle: Option<f64>, st: &mut LevelState) -> i8 {
    let l = st.lvl;
    let portals: Vec<_> = city
        .portals
        .query(&Rect::around(x, y, 1.))
        .into_iter()
        .map(|h| *city.portals.get(h))
        .filter(|p| l >= p.lo && l <= p.hi && (x - p.x).hypot(y - p.y) <= p.r)
        .collect();
    if !portals.is_empty() {
        let reach = portals.iter().map(|p| p.r).fold(0., f64::max);
        let (mut own, mut own_rel) = (f64::INFINITY, f64::INFINITY);
        let (mut best, mut best_ex, mut best_rel, mut best_aligned) =
            (None, f64::INFINITY, f64::INFINITY, false);
        let aligned = |a: f64, b: f64| angle_mod_pi(a, b) < 0.3;
        for p in pieces(city, x, y, reach + 60.) {
            let thru = through(&p, &portals);
            if (p.lvl != l || thru) && (p.t <= 0.001 || p.t >= 0.999) {
                continue;
            }
            let skip = if p.lvl == l {
                match angle {
                    Some(a) => !aligned(a, p.angle),
                    None => !thru,
                }
            } else {
                !thru
            };
            if skip {
                continue;
            }
            let half = p.half.min(MAX_HALF);
            let ex = p.d - half;
            let rel = p.d / if half != 0. { half } else { 1. };
            if p.lvl == l {
                own = own.min(ex);
                own_rel = own_rel.min(rel);
            } else if ex < best_ex {
                best_ex = ex;
                best_rel = rel;
                best = Some(p.lvl);
                best_aligned = angle.is_none_or(|a| aligned(a, p.angle));
            }
        }
        if let Some(b) = best
            && best_ex <= SLACK
            && ((own > 0. && best_ex < own)
                || (best_ex <= 0. && best_aligned && best_rel < 0.5 * own_rel))
        {
            st.lvl = b;
        }
        st.lost_t = 0;
        return st.lvl;
    }
    if l != 0 {
        st.lost_t += 1;
        if st.lost_t >= CHECK {
            st.lost_t = 0;
            if !level_here(city, x, y, l, LOST) {
                st.lvl = 0;
            }
        }
    }
    st.lvl
}

/// Können sich zwei Objekte berühren? Auf derselben Ebene immer, sonst nur gemeinsam im verbindenden Portal.
pub fn touch(city: &mut City, (ax, ay, la): (f64, f64, i8), (bx, by, lb): (f64, f64, i8)) -> bool {
    if la == lb {
        return true;
    }
    for h in city.portals.query(&Rect::around(ax, ay, 1.)) {
        let p = *city.portals.get(h);
        if la < p.lo || la > p.hi || lb < p.lo || lb > p.hi {
            continue;
        }
        if (ax - p.x).hypot(ay - p.y) <= p.r + 30. && (bx - p.x).hypot(by - p.y) <= p.r + 30. {
            return true;
        }
    }
    false
}
