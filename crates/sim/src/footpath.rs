//! Wegfindung zu Fuß (Port von `footpath.js`): A* auf einem 8-px-Raster (0,8 m) im Rechteck um Start und Ziel. Eine
//! Zelle ist frei, wenn ein Kreis mit dem Spielerradius dort weder feste Hindernisse (dieselbe Prüfung wie die
//! Kollision) noch Häuser oder stehende Autos berührt. Zellen werden erst beim Besuch geprüft. Unerreichbares Ziel →
//! Weg zur nächstgelegenen erreichbaren Zelle. Der Weg wird über Sichtlinien geglättet.
use crate::car::blocks;
use crate::city::Solid;
use crate::collision::{Rect, circle_vs_circle, circle_vs_obb, circle_vs_rect, circle_vs_segment};
use crate::world::{PLAYER_RADIUS, World};
use std::cmp::Ordering;
use std::collections::BinaryHeap;

pub const CELL: f64 = 8.;
pub const MARGIN: f64 = 200.;
pub const MAX_SIDE: f64 = 2500.;
pub const MAX_NODES: usize = 60_000;

/// Kann die Spielfigur hier stehen?
pub fn foot_free(w: &mut World, x: f64, y: f64, lvl: i8) -> bool {
    let r = PLAYER_RADIUS;
    if w.city.in_building(x, y).is_some() {
        return false;
    }
    for h in w
        .city
        .solids
        .query(&Rect::new(x - r, y - r, 2. * r, 2. * r))
    {
        let s = w.city.solids.get(h);
        if !blocks(&w.knocked, s, lvl) {
            continue;
        }
        let hit = match s {
            Solid::Wall { seg, .. } => circle_vs_segment(x, y, r, seg).is_some(),
            Solid::Circle {
                x: cx,
                y: cy,
                r: cr,
                ..
            } => circle_vs_circle(x, y, r, *cx, *cy, *cr).is_some(),
            Solid::Rect(rc) => circle_vs_rect(x, y, r, rc).is_some(),
        };
        if hit {
            return false;
        }
    }
    !w.cars.iter().any(|c| {
        c.vx.abs() + c.vy.abs() < 5.
            && (c.x - x).abs() < c.hw + r + 2.
            && (c.y - y).abs() < c.hw + r + 2.
            && circle_vs_obb(x, y, r, &c.obb()).is_some()
    })
}

#[derive(Clone, Copy, PartialEq)]
struct Node(f64, usize);
impl Eq for Node {}
impl Ord for Node {
    fn cmp(&self, o: &Self) -> Ordering {
        // kleinstes f zuerst; bei Gleichstand die kleinere Zelle (deterministisch)
        o.0.total_cmp(&self.0).then(o.1.cmp(&self.1))
    }
}
impl PartialOrd for Node {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

struct Grid {
    x0: f64,
    y0: f64,
    nx: usize,
    ny: usize,
    /// 0 = ungeprüft, 1 = frei, 2 = belegt
    free: Vec<u8>,
    lvl: i8,
}
impl Grid {
    fn center(&self, i: usize) -> (f64, f64) {
        (
            self.x0 + ((i % self.nx) as f64 + 0.5) * CELL,
            self.y0 + ((i / self.nx) as f64 + 0.5) * CELL,
        )
    }
    fn idx(&self, x: f64, y: f64) -> Option<usize> {
        let (gx, gy) = (
            ((x - self.x0) / CELL).floor(),
            ((y - self.y0) / CELL).floor(),
        );
        (gx >= 0. && gy >= 0. && (gx as usize) < self.nx && (gy as usize) < self.ny)
            .then(|| gy as usize * self.nx + gx as usize)
    }
    fn is_free(&mut self, w: &mut World, i: usize) -> bool {
        if self.free[i] == 0 {
            let (x, y) = self.center(i);
            self.free[i] = if foot_free(w, x, y, self.lvl) { 1 } else { 2 };
        }
        self.free[i] == 1
    }
    /// Sichtlinie: alle Zellen entlang der Linie frei (Abtastung je halbe Zelle).
    fn line_free(&mut self, w: &mut World, a: (f64, f64), b: (f64, f64)) -> bool {
        let n = ((b.0 - a.0).hypot(b.1 - a.1) / (CELL / 2.)).ceil() as usize;
        (1..n).all(|k| {
            let t = k as f64 / n as f64;
            self.idx(a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
                .is_some_and(|i| self.is_free(w, i))
        })
    }
}

/// Weg von `from` nach `to` als Punktliste (Start zuerst) oder `None`, wenn der Start außerhalb liegt.
pub fn find_foot_path(
    w: &mut World,
    from: (f64, f64),
    to: (f64, f64),
    lvl: i8,
) -> Option<Vec<(f64, f64)>> {
    let (mut x0, mut y0) = (from.0.min(to.0) - MARGIN, from.1.min(to.1) - MARGIN);
    let (mut x1, mut y1) = (from.0.max(to.0) + MARGIN, from.1.max(to.1) + MARGIN);
    if x1 - x0 > MAX_SIDE {
        (x0, x1) = (from.0 - MAX_SIDE / 2., from.0 + MAX_SIDE / 2.);
    }
    if y1 - y0 > MAX_SIDE {
        (y0, y1) = (from.1 - MAX_SIDE / 2., from.1 + MAX_SIDE / 2.);
    }
    let (nx, ny) = (
        ((x1 - x0) / CELL).ceil() as usize,
        ((y1 - y0) / CELL).ceil() as usize,
    );
    let n = nx * ny;
    let mut g = Grid {
        x0,
        y0,
        nx,
        ny,
        free: vec![0; n],
        lvl,
    };
    let s = g.idx(from.0, from.1)?;
    let t = g.idx(to.0, to.1);
    let h = |g: &Grid, i: usize| {
        let (x, y) = g.center(i);
        (x - to.0).hypot(y - to.1)
    };
    let mut cost = vec![f64::INFINITY; n];
    let mut prev = vec![usize::MAX; n];
    let mut closed = vec![false; n];
    let mut heap = BinaryHeap::new();
    cost[s] = 0.;
    heap.push(Node(h(&g, s), s));
    let (mut best, mut best_h, mut visited) = (s, h(&g, s), 0);
    const DIRS: [(i64, i64, f64); 8] = [
        (1, 0, 1.),
        (-1, 0, 1.),
        (0, 1, 1.),
        (0, -1, 1.),
        (1, 1, std::f64::consts::SQRT_2),
        (1, -1, std::f64::consts::SQRT_2),
        (-1, 1, std::f64::consts::SQRT_2),
        (-1, -1, std::f64::consts::SQRT_2),
    ];
    while let Some(Node(_, i)) = heap.pop() {
        visited += 1;
        if visited > MAX_NODES {
            break;
        }
        if closed[i] {
            continue;
        }
        closed[i] = true;
        let hi = h(&g, i);
        if hi < best_h {
            best_h = hi;
            best = i;
        }
        if Some(i) == t {
            break;
        }
        let (gx, gy) = ((i % nx) as i64, (i / nx) as i64);
        for (dx, dy, c) in DIRS {
            let (ax, ay) = (gx + dx, gy + dy);
            if ax < 0 || ay < 0 || ax >= nx as i64 || ay >= ny as i64 {
                continue;
            }
            let j = ay as usize * nx + ax as usize;
            if closed[j] || !g.is_free(w, j) {
                continue;
            }
            // keine Ecken schneiden
            if dx != 0
                && dy != 0
                && (!g.is_free(w, gy as usize * nx + ax as usize)
                    || !g.is_free(w, ay as usize * nx + gx as usize))
            {
                continue;
            }
            let ng = cost[i] + c * CELL;
            if ng < cost[j] {
                cost[j] = ng;
                prev[j] = i;
                heap.push(Node(ng + h(&g, j), j));
            }
        }
    }
    let reached = t.is_some_and(|t| closed[t]);
    let end = if reached { t.unwrap_or(best) } else { best };
    let mut cells = vec![end];
    while let Some(&c) = cells.last() {
        if prev[c] == usize::MAX {
            break;
        }
        cells.push(prev[c]);
    }
    cells.reverse();
    let mut pts: Vec<(f64, f64)> = cells.iter().map(|&i| g.center(i)).collect();
    if reached && g.is_free(w, end) {
        *pts.last_mut()? = to; // genau aufs Ziel
    }
    pts[0] = from;
    // glätten: von jedem Punkt zum entferntesten, der noch frei zu sehen ist
    if pts.len() <= 2 {
        return Some(pts);
    }
    let mut out = vec![pts[0]];
    let mut i = 0;
    while i < pts.len() - 1 {
        let mut j = pts.len() - 1;
        while j > i + 1 && !g.line_free(w, pts[i], pts[j]) {
            j -= 1;
        }
        out.push(pts[j]);
        i = j;
    }
    Some(out)
}
