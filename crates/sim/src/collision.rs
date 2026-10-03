//! Kollisionen (Port von `collision.js`): Kreis, achsenparallele Rechtecke, gedrehte Boxen (OBB, Autos) und
//! Wandsegmente. Alle Tests liefern einen [`Contact`], dessen Normale vom Hindernis weg zum ersten Objekt zeigt;
//! das erste Objekt wird um `depth` entlang der Normalen herausgeschoben.

/// Normale (nx, ny) vom Hindernis zum ersten Objekt und Eindringtiefe.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Contact {
    pub nx: f64,
    pub ny: f64,
    pub depth: f64,
}
impl Contact {
    pub fn inverted(self) -> Self {
        Self {
            nx: -self.nx,
            ny: -self.ny,
            depth: self.depth,
        }
    }
}

/// Achsenparalleles Rechteck (x, y = linke obere Ecke).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}
impl Rect {
    pub const fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }
    /// Quadrat mit Halbmesser r um (x, y).
    pub fn around(x: f64, y: f64, r: f64) -> Self {
        Self::new(x - r, y - r, 2. * r, 2. * r)
    }
}

/// Wandsegment von a nach b.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Segment {
    pub ax: f64,
    pub ay: f64,
    pub bx: f64,
    pub by: f64,
}
impl Segment {
    pub fn bounds(&self) -> Rect {
        Rect::new(
            self.ax.min(self.bx),
            self.ay.min(self.by),
            (self.bx - self.ax).abs(),
            (self.by - self.ay).abs(),
        )
    }
}

/// Gedrehte Box: `hw` = halbe Länge (Fahrtrichtung), `hh` = halbe Breite.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Obb {
    pub x: f64,
    pub y: f64,
    pub angle: f64,
    pub hw: f64,
    pub hh: f64,
}
impl Obb {
    /// vorwärts (fx, fy) und rechts (rx, ry)
    pub fn axes(&self) -> [f64; 4] {
        let (s, c) = self.angle.sin_cos();
        [c, s, -s, c]
    }
    pub fn corners(&self) -> [(f64, f64); 4] {
        let [fx, fy, rx, ry] = self.axes();
        [(1., 1.), (1., -1.), (-1., -1.), (-1., 1.)].map(|(a, b)| {
            (
                self.x + fx * self.hw * a + rx * self.hh * b,
                self.y + fy * self.hw * a + ry * self.hh * b,
            )
        })
    }
    pub fn bounds(&self) -> Rect {
        let [fx, fy, rx, ry] = self.axes();
        let ex = fx.abs() * self.hw + rx.abs() * self.hh;
        let ey = fy.abs() * self.hw + ry.abs() * self.hh;
        Rect::new(self.x - ex, self.y - ey, ex * 2., ey * 2.)
    }
    fn project(&self, axes: &[f64; 4], nx: f64, ny: f64) -> (f64, f64) {
        let r = self.hw * (axes[0] * nx + axes[1] * ny).abs()
            + self.hh * (axes[2] * nx + axes[3] * ny).abs();
        let c = self.x * nx + self.y * ny;
        (c - r, c + r)
    }
}

pub fn circle_vs_rect(cx: f64, cy: f64, r: f64, rect: &Rect) -> Option<Contact> {
    let px = rect.x.max(cx.min(rect.x + rect.w));
    let py = rect.y.max(cy.min(rect.y + rect.h));
    let (dx, dy) = (cx - px, cy - py);
    let d2 = dx * dx + dy * dy;
    if d2 > r * r {
        return None;
    }
    if d2 > 1e-9 {
        let d = d2.sqrt();
        return Some(Contact {
            nx: dx / d,
            ny: dy / d,
            depth: r - d,
        });
    }
    // Mittelpunkt im Rechteck: kürzester Weg hinaus.
    let left = cx - rect.x;
    let right = rect.x + rect.w - cx;
    let top = cy - rect.y;
    let bottom = rect.y + rect.h - cy;
    let m = left.min(right).min(top).min(bottom);
    Some(if m == left {
        Contact {
            nx: -1.,
            ny: 0.,
            depth: left + r,
        }
    } else if m == right {
        Contact {
            nx: 1.,
            ny: 0.,
            depth: right + r,
        }
    } else if m == top {
        Contact {
            nx: 0.,
            ny: -1.,
            depth: top + r,
        }
    } else {
        Contact {
            nx: 0.,
            ny: 1.,
            depth: bottom + r,
        }
    })
}

pub fn circle_vs_circle(ax: f64, ay: f64, ar: f64, bx: f64, by: f64, br: f64) -> Option<Contact> {
    let (dx, dy, rr) = (ax - bx, ay - by, ar + br);
    let d2 = dx * dx + dy * dy;
    if d2 >= rr * rr {
        return None;
    }
    let d = d2.sqrt();
    if d < 1e-9 {
        return Some(Contact {
            nx: 1.,
            ny: 0.,
            depth: rr,
        });
    }
    Some(Contact {
        nx: dx / d,
        ny: dy / d,
        depth: rr - d,
    })
}

/// Trennende Achsen: kleinste Überlappung = kürzester Weg hinaus (auch wenn ein Intervall das andere enthält,
/// z. B. eine Wand ohne Dicke). Die Normale zeigt von b nach a.
fn sat(
    axes: &[(f64, f64)],
    proj_a: impl Fn(f64, f64) -> (f64, f64),
    proj_b: impl Fn(f64, f64) -> (f64, f64),
    (ax, ay): (f64, f64),
    (bx, by): (f64, f64),
) -> Option<Contact> {
    let (mut best, mut bnx, mut bny) = (f64::INFINITY, 0., 0.);
    for &(nx, ny) in axes {
        let (a0, a1) = proj_a(nx, ny);
        let (b0, b1) = proj_b(nx, ny);
        if a1 <= b0 || b1 <= a0 {
            return None;
        }
        let overlap = (a1 - b0).min(b1 - a0);
        if overlap < best {
            best = overlap;
            bnx = nx;
            bny = ny;
        }
    }
    if (ax - bx) * bnx + (ay - by) * bny < 0. {
        bnx = -bnx;
        bny = -bny;
    }
    Some(Contact {
        nx: bnx,
        ny: bny,
        depth: best,
    })
}

pub fn obb_vs_rect(o: &Obb, rect: &Rect) -> Option<Contact> {
    let axes = o.axes();
    let (cx, cy) = (rect.x + rect.w / 2., rect.y + rect.h / 2.);
    sat(
        &[(axes[0], axes[1]), (axes[2], axes[3]), (1., 0.), (0., 1.)],
        |nx, ny| o.project(&axes, nx, ny),
        |nx, ny| {
            let r = rect.w / 2. * nx.abs() + rect.h / 2. * ny.abs();
            let c = cx * nx + cy * ny;
            (c - r, c + r)
        },
        (o.x, o.y),
        (cx, cy),
    )
}

pub fn obb_vs_obb(a: &Obb, b: &Obb) -> Option<Contact> {
    let (aa, ba) = (a.axes(), b.axes());
    sat(
        &[
            (aa[0], aa[1]),
            (aa[2], aa[3]),
            (ba[0], ba[1]),
            (ba[2], ba[3]),
        ],
        |nx, ny| a.project(&aa, nx, ny),
        |nx, ny| b.project(&ba, nx, ny),
        (a.x, a.y),
        (b.x, b.y),
    )
}

/// Normale zeigt von der Box zum Kreis (der Kreis wird herausgeschoben).
pub fn circle_vs_obb(cx: f64, cy: f64, r: f64, o: &Obb) -> Option<Contact> {
    let (dx, dy) = (cx - o.x, cy - o.y);
    let reach = o.hw + o.hh + r; // grob vorab: außerhalb des Umkreises → nichts (ohne Winkelrechnung)
    if dx > reach || dx < -reach || dy > reach || dy < -reach {
        return None;
    }
    let (fy, fx) = o.angle.sin_cos();
    let (rx, ry) = (-fy, fx);
    let (lx, ly) = (dx * fx + dy * fy, dx * rx + dy * ry);
    let qx = lx.clamp(-o.hw, o.hw);
    let qy = ly.clamp(-o.hh, o.hh);
    let (ex, ey) = (lx - qx, ly - qy);
    let d2 = ex * ex + ey * ey;
    if d2 > r * r {
        return None;
    }
    let (nlx, nly, depth) = if d2 > 1e-9 {
        let d = d2.sqrt();
        (ex / d, ey / d, r - d)
    } else {
        let (px, py) = (o.hw - lx.abs(), o.hh - ly.abs());
        let side = |v: f64| {
            if v > 0. {
                1.
            } else if v < 0. {
                -1.
            } else {
                1.
            }
        };
        if px < py {
            (side(lx), 0., px + r)
        } else {
            (0., side(ly), py + r)
        }
    };
    Some(Contact {
        nx: nlx * fx + nly * rx,
        ny: nlx * fy + nly * ry,
        depth,
    })
}

/// Normale zeigt von der Wand zum Kreis.
pub fn circle_vs_segment(cx: f64, cy: f64, r: f64, s: &Segment) -> Option<Contact> {
    let (dx, dy) = (s.bx - s.ax, s.by - s.ay);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 0. {
        (((cx - s.ax) * dx + (cy - s.ay) * dy) / l2).clamp(0., 1.)
    } else {
        0.
    };
    let (px, py) = (s.ax + dx * t, s.ay + dy * t);
    let (ex, ey) = (cx - px, cy - py);
    let d2 = ex * ex + ey * ey;
    if d2 >= r * r {
        return None;
    }
    if d2 > 1e-9 {
        let d = d2.sqrt();
        return Some(Contact {
            nx: ex / d,
            ny: ey / d,
            depth: r - d,
        });
    }
    let l = if l2 > 0. { l2.sqrt() } else { 1. };
    Some(Contact {
        nx: -dy / l,
        ny: dx / l,
        depth: r,
    })
}

/// Box gegen Wandsegment (SAT mit den beiden Box-Achsen und der Segmentnormale).
pub fn obb_vs_segment(o: &Obb, s: &Segment) -> Option<Contact> {
    let axes = o.axes();
    let (dx, dy) = (s.bx - s.ax, s.by - s.ay);
    let l = dx.hypot(dy);
    let l = if l > 0. { l } else { 1. };
    let (mx, my) = ((s.ax + s.bx) / 2., (s.ay + s.by) / 2.);
    sat(
        &[(axes[0], axes[1]), (axes[2], axes[3]), (-dy / l, dx / l)],
        |nx, ny| o.project(&axes, nx, ny),
        |nx, ny| {
            let (a, b) = (s.ax * nx + s.ay * ny, s.bx * nx + s.by * ny);
            if a < b { (a, b) } else { (b, a) }
        },
        (o.x, o.y),
        (mx, my),
    )
}

/// Raster-Hash für statische Hindernisse und Abfragen. Einträge sind Handles (`u32`, vom Aufrufer vergeben);
/// `insert` liefert die belegten Zellen, damit nachgeladene Kartenteile wieder entfernt werden können.
/// Abfragen liefern jeden Handle genau einmal, in der Reihenfolge, in der sie zuerst gefunden werden.
#[derive(Debug, Clone)]
pub struct SpatialHash {
    cell: f64,
    map: std::collections::HashMap<(i32, i32), Vec<u32>>,
    stamps: Vec<u32>,
    stamp: u32,
}
impl SpatialHash {
    pub fn new(cell: f64) -> Self {
        assert!(cell > 0. && cell.is_finite(), "Zellgröße muss positiv sein");
        Self {
            cell,
            map: Default::default(),
            stamps: Vec::new(),
            stamp: 0,
        }
    }
    pub fn cell(&self) -> f64 {
        self.cell
    }
    fn range(&self, b: &Rect) -> (i32, i32, i32, i32) {
        let c = self.cell;
        (
            (b.x / c).floor() as i32,
            ((b.x + b.w) / c).floor() as i32,
            (b.y / c).floor() as i32,
            ((b.y + b.h) / c).floor() as i32,
        )
    }
    pub fn insert(&mut self, item: u32, b: &Rect) -> Vec<(i32, i32)> {
        let (x0, x1, y0, y1) = self.range(b);
        let mut keys = Vec::with_capacity(((x1 - x0 + 1) * (y1 - y0 + 1)).max(1) as usize);
        for y in y0..=y1 {
            for x in x0..=x1 {
                self.map.entry((x, y)).or_default().push(item);
                keys.push((x, y));
            }
        }
        keys
    }
    pub fn remove(&mut self, item: u32, keys: &[(i32, i32)]) {
        for k in keys {
            if let Some(list) = self.map.get_mut(k) {
                if let Some(i) = list.iter().rposition(|&v| v == item) {
                    list.remove(i);
                }
                if list.is_empty() {
                    self.map.remove(k);
                }
            }
        }
    }
    /// Alle Handles in den Zellen, die `b` berührt (ohne Doppelte), in `out`.
    pub fn query(&mut self, b: &Rect, out: &mut Vec<u32>) {
        out.clear();
        self.stamp = self.stamp.wrapping_add(1);
        if self.stamp == 0 {
            self.stamps.iter_mut().for_each(|s| *s = 0);
            self.stamp = 1;
        }
        let (x0, x1, y0, y1) = self.range(b);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let Some(list) = self.map.get(&(x, y)) else {
                    continue;
                };
                for &it in list {
                    let i = it as usize;
                    if i >= self.stamps.len() {
                        self.stamps.resize(i + 1, 0);
                    }
                    if self.stamps[i] != self.stamp {
                        self.stamps[i] = self.stamp;
                        out.push(it);
                    }
                }
            }
        }
    }
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

/// Nachbarschaftsraster für bewegte Objekte (Port von `grid.js`): nur Beschleunigung, ohne Einfluss aufs Ergebnis.
/// `near` liefert Indizes aufsteigend sortiert – genau die Reihenfolge einer Schleife über alle Objekte.
#[derive(Debug, Default, Clone)]
pub struct Grid {
    cell: f64,
    map: std::collections::HashMap<(i64, i64), Vec<usize>>,
    pool: Vec<Vec<usize>>,
}
impl Grid {
    pub fn build(&mut self, points: impl Iterator<Item = (f64, f64)>, cell: f64) {
        for (_, mut v) in self.map.drain() {
            v.clear();
            self.pool.push(v);
        }
        self.cell = cell;
        for (i, (x, y)) in points.enumerate() {
            let k = ((x / cell).floor() as i64, (y / cell).floor() as i64);
            let pool = &mut self.pool;
            self.map
                .entry(k)
                .or_insert_with(|| pool.pop().unwrap_or_default())
                .push(i);
        }
    }
    pub fn near(&self, x: f64, y: f64, r: f64, out: &mut Vec<usize>) {
        out.clear();
        let c = self.cell;
        if c <= 0. {
            return;
        }
        let (x0, x1) = (((x - r) / c).floor() as i64, ((x + r) / c).floor() as i64);
        let (y0, y1) = (((y - r) / c).floor() as i64, ((y + r) / c).floor() as i64);
        for cx in x0..=x1 {
            for cy in y0..=y1 {
                if let Some(a) = self.map.get(&(cx, cy)) {
                    out.extend_from_slice(a);
                }
            }
        }
        out.sort_unstable();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const EPS: f64 = 1e-9;

    fn car(x: f64, y: f64, angle: f64) -> Obb {
        Obb {
            x,
            y,
            angle,
            hw: 21.,
            hh: 10.,
        }
    }

    #[test]
    fn circle_rect_pushes_out_nearest_side() {
        let r = Rect::new(0., 0., 100., 50.);
        assert!(circle_vs_rect(-20., 25., 10., &r).is_none());
        let c = circle_vs_rect(-5., 25., 10., &r).unwrap();
        assert!((c.nx + 1.).abs() < EPS && c.ny.abs() < EPS && (c.depth - 5.).abs() < EPS);
        // Mittelpunkt innen: kürzester Weg (oben, 3 px) plus Radius
        let c = circle_vs_rect(50., 3., 4., &r).unwrap();
        assert_eq!((c.nx, c.ny), (0., -1.));
        assert!((c.depth - 7.).abs() < EPS);
    }

    #[test]
    fn circle_circle_degenerate_centre() {
        assert!(circle_vs_circle(0., 0., 1., 3., 0., 1.).is_none());
        let c = circle_vs_circle(0., 0., 2., 3., 0., 2.).unwrap();
        assert!((c.nx + 1.).abs() < EPS && (c.depth - 1.).abs() < EPS);
        let c = circle_vs_circle(5., 5., 2., 5., 5., 3.).unwrap();
        assert_eq!((c.nx, c.ny, c.depth), (1., 0., 5.));
    }

    #[test]
    fn obb_corners_and_bounds() {
        let o = car(100., 50., std::f64::consts::FRAC_PI_2);
        let b = o.bounds();
        assert!((b.w - 20.).abs() < 1e-9 && (b.h - 42.).abs() < 1e-9);
        for (x, y) in o.corners() {
            assert!(
                x >= b.x - 1e-9
                    && x <= b.x + b.w + 1e-9
                    && y >= b.y - 1e-9
                    && y <= b.y + b.h + 1e-9
            );
        }
    }

    #[test]
    fn sat_obb_rect_and_obb_obb() {
        let r = Rect::new(30., -50., 40., 100.);
        // Auto mit Front 21 px voraus, Wand ab x = 30: Überlappung 1 px, Normale nach links (vom Rechteck weg)
        let c = obb_vs_rect(&car(10., 0., 0.), &r).unwrap();
        assert!((c.nx + 1.).abs() < EPS && (c.depth - 1.).abs() < 1e-9);
        assert!(obb_vs_rect(&car(8., 0., 0.), &r).is_none());
        // Gedreht um 45°: keine Berührung, obwohl sich die achsenparallelen Hüllen schneiden
        let o = car(0., 0., std::f64::consts::FRAC_PI_4);
        let far = Rect::new(19., -26., 10., 10.);
        assert!(o.bounds().x + o.bounds().w > far.x);
        assert!(obb_vs_rect(&o, &far).is_none());
        let a = car(0., 0., 0.);
        let b = car(40., 0., 0.);
        let c = obb_vs_obb(&a, &b).unwrap();
        assert!((c.nx + 1.).abs() < EPS && (c.depth - 2.).abs() < 1e-9);
        assert!(obb_vs_obb(&a, &car(43., 0., 0.)).is_none());
    }

    #[test]
    fn circle_obb_rotated_and_inside() {
        let o = car(0., 0., std::f64::consts::FRAC_PI_2); // Länge zeigt nach unten
        let c = circle_vs_obb(0., 25., 5., &o).unwrap();
        assert!(c.nx.abs() < 1e-9 && (c.ny - 1.).abs() < 1e-9 && (c.depth - 1.).abs() < 1e-9);
        assert!(circle_vs_obb(15., 0., 4., &o).is_none());
        // Mittelpunkt in der Box: entlang der kürzeren Halbachse hinaus
        let c = circle_vs_obb(0., 0., 3., &car(0., 0., 0.)).unwrap();
        assert!(c.nx.abs() < EPS && (c.ny - 1.).abs() < EPS && (c.depth - 13.).abs() < EPS);
    }

    #[test]
    fn segments_have_zero_thickness_but_still_block() {
        let s = Segment {
            ax: 0.,
            ay: 0.,
            bx: 100.,
            by: 0.,
        };
        let c = circle_vs_segment(50., 3., 5., &s).unwrap();
        assert!((c.ny - 1.).abs() < EPS && (c.depth - 2.).abs() < EPS);
        // genau auf der Linie: Normale = linke Senkrechte der Richtung
        let c = circle_vs_segment(50., 0., 5., &s).unwrap();
        assert_eq!((c.nx, c.ny, c.depth), (-0., 1., 5.));
        // Auto steckt halb in der Wand: kürzester Weg hinaus (nicht durch die Wand hindurch)
        let c = obb_vs_segment(&car(50., 6., 0.), &s).unwrap();
        assert!(
            (c.ny - 1.).abs() < EPS && (c.depth - 4.).abs() < 1e-9,
            "{c:?}"
        );
        let c = obb_vs_segment(&car(50., -6., 0.), &s).unwrap();
        assert!((c.ny + 1.).abs() < EPS && (c.depth - 4.).abs() < 1e-9);
        assert!(obb_vs_segment(&car(50., 11., 0.), &s).is_none());
        // Segmentende: das Auto davor wird nicht von der Verlängerung erfasst
        assert!(obb_vs_segment(&car(125., 0., 0.), &s).is_none());
    }

    #[test]
    fn spatial_hash_query_remove_dedup() {
        let mut h = SpatialHash::new(10.);
        let a = h.insert(0, &Rect::new(0., 0., 25., 5.));
        h.insert(1, &Rect::new(50., 50., 1., 1.));
        h.insert(2, &Rect::new(-15., -15., 2., 2.));
        assert_eq!(a.len(), 3);
        let mut out = Vec::new();
        h.query(&Rect::new(0., 0., 30., 30.), &mut out);
        assert_eq!(out, vec![0]); // über drei Zellen, aber einmal
        h.query(&Rect::new(-20., -20., 100., 100.), &mut out);
        out.sort();
        assert_eq!(out, vec![0, 1, 2]);
        h.remove(0, &a);
        h.query(&Rect::new(0., 0., 30., 30.), &mut out);
        assert!(out.is_empty());
        h.remove(1, &h.clone().insert(1, &Rect::new(50., 50., 1., 1.)));
        h.query(&Rect::new(-20., -20., 100., 100.), &mut out);
        assert_eq!(out, vec![2]);
        // Der Stempel läuft über: weiter keine Doppelten
        h.stamp = u32::MAX;
        h.query(&Rect::new(-20., -20., 1., 1.), &mut out);
        h.query(&Rect::new(-20., -20., 1., 1.), &mut out);
        assert_eq!(out, vec![2]);
    }

    #[test]
    fn grid_near_matches_full_scan_order() {
        let pts: Vec<(f64, f64)> = (0..200)
            .map(|i| ((i * 37 % 500) as f64, (i * 91 % 700) as f64))
            .collect();
        let mut g = Grid::default();
        g.build(pts.iter().copied(), 64.);
        let mut out = Vec::new();
        for &(x, y, r) in &[(250., 350., 90.), (0., 0., 10.), (499., 699., 300.)] {
            g.near(x, y, r, &mut out);
            assert!(out.windows(2).all(|w| w[0] < w[1]));
            for (i, &(px, py)) in pts.iter().enumerate() {
                if (px - x).hypot(py - y) <= r {
                    assert!(out.contains(&i), "Punkt {i} fehlt");
                }
            }
        }
    }
}
