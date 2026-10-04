//! Navigation über ganz Berlin: ein Straßengraph aus allen Kacheln (einmal beim Start, im Hintergrund gebaut) und eine
//! Wegsuche mit einem Kostenmodell, das **wenig befahrene Schleichwege bevorzugt**.
//!
//! Die Streaming-Welt kennt nur die Kacheln um die Kamera; eine Route quer durch die Stadt braucht das ganze Netz.
//! Der Graph liest deshalb aus jeder Kachel nur Knoten (globale Nummern verbinden die Kacheln), Straßen, Ampeln und
//! Abbiegeverbote – Straßen liegen in mehreren Kacheln und werden über ihre globale Nummer entdoppelt.
//!
//! **Kosten** (Sekunden, im Auto):
//! - Fahrzeit aus Tempolimit (sonst je Straßenklasse ein Stadttempo), mal 0,9 (Stadtverkehr).
//! - Stark befahrene Straßen (gezählte Kfz/Tag, `dtv`) kosten bis zu 50 % mehr Zeit: Stau, Lieferverkehr, Ampelphasen
//!   – genau das, was einen Schleichweg attraktiv macht.
//! - Ampeln an der Kreuzung +12 s (mittlere Wartezeit); ohne Ampel +4 s nur beim Einbiegen in bzw. Kreuzen einer
//!   wichtigeren Straße (Lücke abwarten) – gleichrangige Wohnstraßen-Ecken kosten nichts extra.
//! - Abbiegen: rechts +4 s, links +8 s, Wenden nur in der Sackgasse (+20 s). Ohne diese Strafen zöge die Route im
//!   Zickzack durch jedes Wohngebiet.
//! - Einbahnstraßen und Abbiegeverbote gelten; Zufahrten, Fußgängerzonen und gesperrte Abschnitte fährt kein Auto.
//!
//! Zu Fuß: 5 km/h auf allen Straßen in beide Richtungen, Ampeln +5 s, keine Abbiegestrafen.
//!
//! Gesucht wird mit A* über **gerichtete Kanten** (nicht Knoten), damit Abbiegekosten und -verbote exakt gelten.
use anyhow::{Context, Result};
use serde::Deserialize;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::path::Path;

/// Mit dem Auto (Einbahn, Abbiegeverbote, nur Kfz-Straßen) oder zu Fuß.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Car,
    Foot,
}

/// Ampel: mittlere Wartezeit (s)
pub const SIGNAL_S: f32 = 12.;
/// Einbiegen in bzw. Kreuzen einer wichtigeren Straße ohne Ampel (Lücke abwarten) (s)
pub const JUNCTION_S: f32 = 4.;
pub const RIGHT_S: f32 = 4.;
pub const LEFT_S: f32 = 8.;
pub const UTURN_S: f32 = 20.;
/// Aufschlag für die verkehrsreichsten Straßen (Anteil der Fahrzeit) und ab welcher Verkehrsmenge er voll gilt
pub const TRAFFIC_MAX: f32 = 0.5;
pub const TRAFFIC_FULL: f32 = 40_000.;
const FOOT_MS: f32 = 1.4;
/// Gitterzelle zum Einrasten (Kartenpixel)
const CELL: f32 = 500.;

#[derive(Debug, Clone)]
pub struct REdge {
    pub id: i64,
    pub a: u32,
    pub b: u32,
    /// Kartenpixel, von a nach b
    pub pts: Vec<[f32; 2]>,
    pub len: f32,
    pub cls: u8,
    /// 1 = nur a→b, −1 = nur b→a
    pub oneway: i8,
    pub maxspeed: f32,
    pub dtv: f32,
    /// Autos dürfen hier fahren (Klasse bis Spielstraße, nicht gesperrt)
    pub car: bool,
}
#[derive(Debug, Clone, Copy)]
struct Arc {
    edge: u32,
    to: u32,
    fwd: bool,
    car: bool,
}

/// Straßengraph von ganz Berlin.
#[derive(Debug, Default)]
pub struct RouteGraph {
    /// Kartenpixel je Meter
    pub scale: f32,
    pub nodes: Vec<[f32; 2]>,
    pub edges: Vec<REdge>,
    arcs: Vec<Arc>,
    out: Vec<Vec<u32>>,
    signal: Vec<bool>,
    bans: HashSet<(u32, u32, u32)>,
    grid: HashMap<(i32, i32), Vec<u32>>,
}

/// Eine gefundene Route: Punktfolge in Kartenpixeln (vom Start zum Ziel), Länge und geschätzte Zeit.
#[derive(Debug, Clone, PartialEq)]
pub struct Route {
    pub pts: Vec<(f64, f64)>,
    pub len_m: f64,
    pub secs: f64,
    /// Kanten in Fahrreihenfolge (für Tests und Auswertung)
    pub edges: Vec<i64>,
}

/// Wo ein Punkt aufs Netz trifft: Kante, Abstand ab Knoten a (Pixel), Lotpunkt.
#[derive(Debug, Clone, Copy)]
pub struct Snap {
    pub edge: u32,
    pub along: f32,
    pub at: [f32; 2],
    pub dist: f32,
}

// --- Bau aus den Kacheln -----------------------------------------------------------------------------------------

#[derive(Deserialize, Default)]
struct RawVerts {
    #[serde(default)]
    id: Vec<i64>,
    #[serde(default)]
    xy: Vec<f64>,
}
#[derive(Deserialize)]
struct RawTile {
    #[serde(default)]
    vertices: RawVerts,
    #[serde(default)]
    edges: Vec<Vec<serde_json::Value>>,
    #[serde(default)]
    signals: Vec<i64>,
    #[serde(default, rename = "turnBans")]
    turn_bans: serde_json::Value,
}
struct PEdge {
    gid: i64,
    a: (i64, [f32; 2]),
    b: (i64, [f32; 2]),
    mid: Vec<[f32; 2]>,
    cls: u8,
    oneway: i8,
    flags: u32,
    maxspeed: f32,
    dtv: f32,
}
#[derive(Default)]
struct Parsed {
    edges: Vec<PEdge>,
    signals: Vec<i64>,
    bans: Vec<(i64, i64, i64)>,
}

fn undelta(v: &[f64]) -> Vec<[f32; 2]> {
    let (mut x, mut y) = (0., 0.);
    v.chunks(2)
        .enumerate()
        .filter(|(_, p)| p.len() == 2)
        .map(|(i, p)| {
            if i == 0 {
                (x, y) = (p[0], p[1]);
            } else {
                x += p[0];
                y += p[1];
            }
            [x as f32, y as f32]
        })
        .collect()
}
fn flat_numbers(v: &serde_json::Value, out: &mut Vec<f64>) {
    match v {
        serde_json::Value::Number(n) => out.push(n.as_f64().unwrap_or(0.)),
        serde_json::Value::Array(a) => a.iter().for_each(|x| flat_numbers(x, out)),
        _ => {}
    }
}

fn parse_tile(bytes: &[u8]) -> Result<Parsed> {
    let t: RawTile = serde_json::from_slice(bytes)?;
    let verts = undelta(&t.vertices.xy);
    let num = |v: &serde_json::Value| v.as_f64().unwrap_or(0.);
    let mut out = Parsed::default();
    for r in &t.edges {
        if r.len() < 10 {
            continue;
        }
        let (ia, ib) = (num(&r[1]) as usize, num(&r[2]) as usize);
        let (Some(&ida), Some(&idb), Some(&pa), Some(&pb)) = (
            t.vertices.id.get(ia),
            t.vertices.id.get(ib),
            verts.get(ia),
            verts.get(ib),
        ) else {
            continue;
        };
        let mut midf = Vec::new();
        flat_numbers(&r[8], &mut midf);
        let mut cs = Vec::new();
        flat_numbers(&r[9], &mut cs);
        // kurzer Querschnitt [Tempo, Belag], langer: Tempo an Stelle 10
        let maxspeed = if cs.len() == 2 {
            cs[0]
        } else {
            cs.get(10).copied().unwrap_or(0.)
        };
        out.edges.push(PEdge {
            gid: num(&r[0]) as i64,
            a: (ida, pa),
            b: (idb, pb),
            mid: undelta(&midf),
            cls: num(&r[3]) as u8,
            oneway: num(&r[6]) as i8,
            flags: num(&r[7]) as u32,
            maxspeed: maxspeed as f32,
            dtv: (r.get(10).map(num).unwrap_or(0.).abs() * 100.) as f32,
        });
    }
    out.signals = t.signals;
    let mut tb = Vec::new();
    flat_numbers(&t.turn_bans, &mut tb);
    out.bans = tb
        .chunks(3)
        .filter(|c| c.len() == 3)
        .map(|c| (c[0] as i64, c[1] as i64, c[2] as i64))
        .collect();
    Ok(out)
}

impl RouteGraph {
    /// Aus allen Kacheln unter `root/tiles` (bzw. nur denen, die `keep(x, y)` behält), parallel gelesen.
    pub fn build(root: &Path, keep: Option<&(dyn Fn(u32, u32) -> bool + Sync)>) -> Result<Self> {
        let scale = std::fs::read(root.join("index.json"))
            .ok()
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
            .and_then(|v| v["meta"]["scale"].as_f64())
            .unwrap_or(10.) as f32;
        let mut files: Vec<_> = std::fs::read_dir(root.join("tiles"))
            .context("Kacheln für die Navigation")?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                let Some(stem) = p.file_stem().and_then(|s| s.to_str()) else {
                    return false;
                };
                let mut it = stem.split('_').map(|s| s.parse::<u32>());
                match (it.next(), it.next()) {
                    (Some(Ok(x)), Some(Ok(y))) => keep.is_none_or(|k| k(x, y)),
                    _ => false,
                }
            })
            .collect();
        files.sort();
        let threads = std::thread::available_parallelism()
            .map_or(4, |n| n.get())
            .min(12);
        let chunk = files.len().div_ceil(threads.max(1)).max(1);
        let parts: Vec<Parsed> = std::thread::scope(|s| {
            let handles: Vec<_> = files
                .chunks(chunk)
                .map(|fs| {
                    s.spawn(move || {
                        let mut acc = Parsed::default();
                        for f in fs {
                            if let Ok(b) = std::fs::read(f)
                                && let Ok(p) = parse_tile(&b)
                            {
                                acc.edges.extend(p.edges);
                                acc.signals.extend(p.signals);
                                acc.bans.extend(p.bans);
                            }
                        }
                        acc
                    })
                })
                .collect();
            handles.into_iter().filter_map(|h| h.join().ok()).collect()
        });
        Ok(Self::from_parts(parts, scale))
    }

    fn from_parts(parts: Vec<Parsed>, scale: f32) -> Self {
        let mut g = RouteGraph {
            scale,
            ..Default::default()
        };
        let mut ids: HashMap<i64, u32> = HashMap::new();
        let mut seen: HashSet<i64> = HashSet::new();
        let mut edge_idx: HashMap<i64, u32> = HashMap::new();
        let mut node = |g: &mut RouteGraph, id: i64, p: [f32; 2]| -> u32 {
            *ids.entry(id).or_insert_with(|| {
                g.nodes.push(p);
                (g.nodes.len() - 1) as u32
            })
        };
        let mut signals = Vec::new();
        let mut bans = Vec::new();
        for part in parts {
            signals.extend(part.signals);
            bans.extend(part.bans);
            for e in part.edges {
                if !seen.insert(e.gid) {
                    continue;
                }
                let a = node(&mut g, e.a.0, e.a.1);
                let b = node(&mut g, e.b.0, e.b.1);
                let mut pts = vec![e.a.1];
                pts.extend(e.mid);
                pts.push(e.b.1);
                let len = pts
                    .windows(2)
                    .map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]))
                    .sum::<f32>();
                let blocked = e.flags & 4 != 0;
                let car = e.cls >= 1
                    && e.cls <= berlin_map_loader::citycodes::TRAFFIC_MAX_CLASS
                    && !blocked;
                edge_idx.insert(e.gid, g.edges.len() as u32);
                g.edges.push(REdge {
                    id: e.gid,
                    a,
                    b,
                    pts,
                    len: len.max(0.01),
                    cls: e.cls,
                    oneway: e.oneway,
                    maxspeed: e.maxspeed,
                    dtv: e.dtv,
                    car,
                });
            }
        }
        g.out = vec![Vec::new(); g.nodes.len()];
        g.signal = vec![false; g.nodes.len()];
        for s in signals {
            if let Some(&n) = ids.get(&s) {
                g.signal[n as usize] = true;
            }
        }
        for (i, e) in g.edges.iter().enumerate() {
            for fwd in [true, false] {
                let car = e.car
                    && match e.oneway {
                        1 => fwd,
                        -1 => !fwd,
                        _ => true,
                    };
                let (from, to) = if fwd { (e.a, e.b) } else { (e.b, e.a) };
                g.out[from as usize].push(g.arcs.len() as u32);
                g.arcs.push(Arc {
                    edge: i as u32,
                    to,
                    fwd,
                    car,
                });
            }
        }
        for (ea, n, eb) in bans {
            if let (Some(&a), Some(&n), Some(&b)) =
                (edge_idx.get(&ea), ids.get(&n), edge_idx.get(&eb))
            {
                g.bans.insert((a, n, b));
            }
        }
        // Raster zum Einrasten
        for (i, e) in g.edges.iter().enumerate() {
            for w in e.pts.windows(2) {
                let (x0, x1) = (w[0][0].min(w[1][0]), w[0][0].max(w[1][0]));
                let (y0, y1) = (w[0][1].min(w[1][1]), w[0][1].max(w[1][1]));
                for cx in (x0 / CELL).floor() as i32..=(x1 / CELL).floor() as i32 {
                    for cy in (y0 / CELL).floor() as i32..=(y1 / CELL).floor() as i32 {
                        let v = g.grid.entry((cx, cy)).or_default();
                        if v.last() != Some(&(i as u32)) {
                            v.push(i as u32);
                        }
                    }
                }
            }
        }
        g
    }

    pub fn is_empty(&self) -> bool {
        self.edges.is_empty()
    }

    // --- Kosten ----------------------------------------------------------------------------------------------------

    /// Stadttempo (km/h): Tempolimit, sonst je Klasse.
    pub fn speed_kmh(e: &REdge) -> f32 {
        if e.maxspeed > 0. {
            return e.maxspeed;
        }
        match e.cls {
            1 => 80.,
            2 => 60.,
            3..=5 => 50.,
            6 => 40.,
            7 => 30.,
            8 => 7.,
            _ => 20.,
        }
    }
    /// Zeit (s) für `px` Kartenpixel auf Kante `e`.
    pub fn travel_s(&self, e: &REdge, px: f32, mode: Mode) -> f32 {
        let m = px / self.scale;
        match mode {
            Mode::Foot => m / FOOT_MS,
            Mode::Car => {
                let v = Self::speed_kmh(e) * 0.9 / 3.6;
                let busy = 1. + TRAFFIC_MAX * (e.dtv / TRAFFIC_FULL).clamp(0., 1.);
                m / v.max(0.5) * busy
            }
        }
    }
    /// Kosten am Knoten beim Wechsel von Bogen `a` auf Bogen `b`.
    fn turn_s(&self, a: &Arc, b: &Arc, mode: Mode) -> f32 {
        let n = a.to as usize;
        if mode == Mode::Foot {
            return if self.signal[n] { 5. } else { 0. };
        }
        // Ampel: mittlere Wartezeit. Sonst warten nur, wer auf eine wichtigere Straße einbiegt oder sie kreuzt (eine
        // Lücke im Verkehr abpassen) – gleichrangige Wohnstraßen-Kreuzungen kosten nichts extra, sonst bestrafte
        // jede Ecke im Wohngebiet genau die Schleichwege.
        let mut c = if self.signal[n] {
            SIGNAL_S
        } else {
            let mine = self.edges[a.edge as usize]
                .cls
                .min(self.edges[b.edge as usize].cls);
            let major = self.out[n]
                .iter()
                .map(|&i| self.arcs[i as usize].edge)
                .filter(|&e| e != a.edge && e != b.edge)
                .map(|e| self.edges[e as usize].cls)
                .min();
            if major.is_some_and(|m| m < mine)
                || self.edges[b.edge as usize].cls < self.edges[a.edge as usize].cls
            {
                JUNCTION_S
            } else {
                0.
            }
        };
        if a.edge == b.edge {
            return c + UTURN_S;
        }
        let (d0, d1) = (self.dir_at_end(a), self.dir_at_start(b));
        let cross = d0[0] * d1[1] - d0[1] * d1[0];
        let dot = d0[0] * d1[0] + d0[1] * d1[1];
        // Bildschirmkoordinaten (y nach unten): positives Kreuzprodukt = im Uhrzeigersinn = rechts
        if dot < 0.87 {
            c += if cross > 0. { RIGHT_S } else { LEFT_S };
        }
        c
    }
    fn dir_at_end(&self, a: &Arc) -> [f32; 2] {
        let p = &self.edges[a.edge as usize].pts;
        let (u, v) = if a.fwd {
            (p[p.len() - 2], p[p.len() - 1])
        } else {
            (p[1], p[0])
        };
        norm([v[0] - u[0], v[1] - u[1]])
    }
    fn dir_at_start(&self, a: &Arc) -> [f32; 2] {
        let p = &self.edges[a.edge as usize].pts;
        let (u, v) = if a.fwd {
            (p[0], p[1])
        } else {
            (p[p.len() - 1], p[p.len() - 2])
        };
        norm([v[0] - u[0], v[1] - u[1]])
    }
    fn arc_ok(&self, a: &Arc, mode: Mode) -> bool {
        mode == Mode::Foot || a.car
    }

    // --- Einrasten -------------------------------------------------------------------------------------------------

    /// Nächste Straße (im Auto nur befahrbare) bis `max` Kartenpixel um (x, y).
    pub fn snap(&self, x: f64, y: f64, mode: Mode, max: f32) -> Option<Snap> {
        let (x, y) = (x as f32, y as f32);
        let (cx, cy) = ((x / CELL).floor() as i32, (y / CELL).floor() as i32);
        let mut best: Option<Snap> = None;
        let rings = (max / CELL).ceil() as i32 + 1;
        let mut done = HashSet::new();
        for r in 0..=rings {
            if let Some(b) = best
                && b.dist < (r - 1).max(0) as f32 * CELL
            {
                break;
            }
            for gx in cx - r..=cx + r {
                for gy in cy - r..=cy + r {
                    if (gx - cx).abs() != r && (gy - cy).abs() != r {
                        continue;
                    }
                    for &i in self.grid.get(&(gx, gy)).into_iter().flatten() {
                        if !done.insert(i) {
                            continue;
                        }
                        let e = &self.edges[i as usize];
                        if mode == Mode::Car && !e.car {
                            continue;
                        }
                        let mut acc = 0.;
                        for w in e.pts.windows(2) {
                            let (p, along) = closest(w[0], w[1], [x, y]);
                            let d = (p[0] - x).hypot(p[1] - y);
                            if d < max && best.is_none_or(|b| d < b.dist) {
                                best = Some(Snap {
                                    edge: i,
                                    along: acc + along,
                                    at: p,
                                    dist: d,
                                });
                            }
                            acc += (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]);
                        }
                    }
                }
            }
        }
        best
    }

    // --- Wegsuche ---------------------------------------------------------------------------------------------------

    /// Route von (fx, fy) nach (tx, ty). `None`, wenn eines der Enden keine Straße in Reichweite hat oder es keine
    /// Verbindung gibt.
    pub fn route(&self, from: (f64, f64), to: (f64, f64), mode: Mode) -> Option<Route> {
        let s = self.snap(from.0, from.1, mode, 3000.)?;
        let t = self.snap(to.0, to.1, mode, 4000.)?;
        let es = &self.edges[s.edge as usize];
        let et = &self.edges[t.edge as usize];
        let allowed = |e: &REdge, fwd: bool| {
            mode == Mode::Foot
                || match e.oneway {
                    1 => fwd,
                    -1 => !fwd,
                    _ => true,
                }
        };
        let speed_max = match mode {
            Mode::Car => 130. / 3.6 * self.scale,
            Mode::Foot => FOOT_MS * self.scale,
        };
        let h = |n: u32| {
            let p = self.nodes[n as usize];
            (p[0] - t.at[0]).hypot(p[1] - t.at[1]) / speed_max
        };
        let n = self.arcs.len();
        let mut g = vec![f32::INFINITY; n];
        let mut prev = vec![u32::MAX; n];
        let mut heap = BinaryHeap::new();
        // Start: auf der eigenen Kante zu einem ihrer Enden (Bögen derselben Kante, Richtung erlaubt)
        for &ai in self.out[es.a as usize]
            .iter()
            .chain(&self.out[es.b as usize])
        {
            let a = self.arcs[ai as usize];
            if a.edge != s.edge || !self.arc_ok(&a, mode) {
                continue;
            }
            let px = if a.fwd { es.len - s.along } else { s.along };
            let c = self.travel_s(es, px, mode);
            if c < g[ai as usize] {
                g[ai as usize] = c;
                heap.push(Item {
                    f: c + h(a.to),
                    g: c,
                    arc: ai,
                });
            }
        }
        // Ziel auf derselben Kante in erlaubter Richtung: direkt
        // (Kosten, letzter Bogen, direkt auf der Startkante, Einfahrt in die Zielkante von a)
        let mut best: Option<(f32, u32, bool, bool)> = None;
        if s.edge == t.edge {
            let fwd = t.along >= s.along;
            if allowed(es, fwd) {
                best = Some((
                    self.travel_s(es, (t.along - s.along).abs(), mode),
                    u32::MAX,
                    true,
                    fwd,
                ));
            }
        }
        let mut pops = 0;
        while let Some(Item { f, g: gc, arc }) = heap.pop() {
            if best.is_some_and(|b| f >= b.0) {
                break;
            }
            if gc > g[arc as usize] {
                continue;
            }
            pops += 1;
            if pops > 600_000 {
                break;
            }
            let a = self.arcs[arc as usize];
            // am Knoten a.to: ins Ziel einbiegen? (nicht, wer die Zielkante gerade ganz durchfahren hat)
            for fwd in [true, false] {
                let enter = if fwd { et.a } else { et.b };
                if enter != a.to || !allowed(et, fwd) || a.edge == t.edge {
                    continue;
                }
                let into = self.out[a.to as usize]
                    .iter()
                    .map(|&i| self.arcs[i as usize])
                    .find(|x| x.edge == t.edge && x.fwd == fwd);
                let Some(into) = into else { continue };
                if self.bans.contains(&(a.edge, a.to, t.edge)) {
                    continue;
                }
                let px = if fwd { t.along } else { et.len - t.along };
                let c = gc + self.turn_s(&a, &into, mode) + self.travel_s(et, px, mode);
                if best.is_none_or(|b| c < b.0) {
                    best = Some((c, arc, false, fwd));
                }
            }
            for &bi in &self.out[a.to as usize] {
                let b = self.arcs[bi as usize];
                if !self.arc_ok(&b, mode) {
                    continue;
                }
                if b.edge == a.edge && self.out[a.to as usize].len() > 1 {
                    continue; // Wenden nur in der Sackgasse (eine einzige Straße am Knoten)
                }
                if mode == Mode::Car && self.bans.contains(&(a.edge, a.to, b.edge)) {
                    continue;
                }
                let e = &self.edges[b.edge as usize];
                let c = gc + self.turn_s(&a, &b, mode) + self.travel_s(e, e.len, mode);
                if c < g[bi as usize] {
                    g[bi as usize] = c;
                    prev[bi as usize] = arc;
                    heap.push(Item {
                        f: c + h(b.to),
                        g: c,
                        arc: bi,
                    });
                }
            }
        }
        let (cost, last, direct, fwd_into) = best?;
        // Punktfolge: Start → (Teil der Startkante) → volle Bögen → (Teil der Zielkante) → Ziel
        let mut pts: Vec<(f64, f64)> = vec![(from.0, from.1), (s.at[0] as f64, s.at[1] as f64)];
        let mut edges = vec![es.id];
        if direct {
            piece(&mut pts, es, s.along, t.along);
        } else {
            let mut chain = Vec::new();
            let mut c = last;
            while c != u32::MAX {
                chain.push(c);
                c = prev[c as usize];
            }
            chain.reverse();
            let first = self.arcs[chain[0] as usize];
            piece(&mut pts, es, s.along, if first.fwd { es.len } else { 0. });
            for &ai in &chain[1..] {
                let a = self.arcs[ai as usize];
                let e = &self.edges[a.edge as usize];
                piece(
                    &mut pts,
                    e,
                    if a.fwd { 0. } else { e.len },
                    if a.fwd { e.len } else { 0. },
                );
                edges.push(e.id);
            }
            piece(&mut pts, et, if fwd_into { 0. } else { et.len }, t.along);
            if edges.last() != Some(&et.id) {
                edges.push(et.id);
            }
        }
        pts.push((to.0, to.1));
        pts.dedup_by(|a, b| (a.0 - b.0).hypot(a.1 - b.1) < 0.5);
        let len_px: f64 = pts
            .windows(2)
            .map(|w| (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1))
            .sum();
        Some(Route {
            pts,
            len_m: len_px / self.scale as f64,
            secs: cost as f64,
            edges,
        })
    }
}

/// Stück einer Kante von `s0` nach `s1` (Pixel ab Knoten a; rückwärts, wenn s1 < s0) an `out` anhängen.
fn piece(out: &mut Vec<(f64, f64)>, e: &REdge, s0: f32, s1: f32) {
    let at = |s: f32| -> [f32; 2] {
        let mut acc = 0.;
        for w in e.pts.windows(2) {
            let l = (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]);
            if acc + l >= s {
                let t = if l > 0. { (s - acc) / l } else { 0. };
                return [
                    w[0][0] + (w[1][0] - w[0][0]) * t,
                    w[0][1] + (w[1][1] - w[0][1]) * t,
                ];
            }
            acc += l;
        }
        e.pts[e.pts.len() - 1]
    };
    let mut cum = vec![0f32];
    for w in e.pts.windows(2) {
        cum.push(cum.last().unwrap() + (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]));
    }
    let push = |out: &mut Vec<(f64, f64)>, p: [f32; 2]| out.push((p[0] as f64, p[1] as f64));
    push(out, at(s0));
    if s1 >= s0 {
        for (i, &c) in cum.iter().enumerate() {
            if c > s0 && c < s1 {
                push(out, e.pts[i]);
            }
        }
    } else {
        for (i, &c) in cum.iter().enumerate().rev() {
            if c < s0 && c > s1 {
                push(out, e.pts[i]);
            }
        }
    }
    push(out, at(s1));
}

fn norm(v: [f32; 2]) -> [f32; 2] {
    let l = v[0].hypot(v[1]).max(1e-6);
    [v[0] / l, v[1] / l]
}
/// Lotpunkt auf der Strecke a–b und sein Abstand ab a.
fn closest(a: [f32; 2], b: [f32; 2], p: [f32; 2]) -> ([f32; 2], f32) {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 0. {
        (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0., 1.)
    } else {
        0.
    };
    ([a[0] + dx * t, a[1] + dy * t], t * l2.sqrt())
}

#[derive(PartialEq)]
struct Item {
    f: f32,
    g: f32,
    arc: u32,
}
impl Eq for Item {}
impl Ord for Item {
    fn cmp(&self, o: &Self) -> Ordering {
        o.f.total_cmp(&self.f).then_with(|| self.arc.cmp(&o.arc))
    }
}
impl PartialOrd for Item {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Kleines Gitter: Hauptstraße (viel Verkehr, Ampeln) gegen eine parallele ruhige Nebenstraße.
    fn edge(
        gid: i64,
        a: (i64, [f32; 2]),
        b: (i64, [f32; 2]),
        cls: u8,
        oneway: i8,
        dtv: f32,
        speed: f32,
    ) -> PEdge {
        PEdge {
            gid,
            a,
            b,
            mid: vec![],
            cls,
            oneway,
            flags: 0,
            maxspeed: speed,
            dtv,
        }
    }
    /// Knoten 1 –(Hauptstraße 4 Abschnitte, Ampeln)– 5; 30 m daneben 10 –(Nebenstraße)– 14, Verbinder an den Enden.
    fn town(main_dtv: f32) -> RouteGraph {
        let p = |x: f32, y: f32| [x, y];
        let mut edges = Vec::new();
        for i in 0..4 {
            let x0 = i as f32 * 2000.;
            edges.push(edge(
                100 + i,
                (1 + i, p(x0, 0.)),
                (2 + i, p(x0 + 2000., 0.)),
                3,
                0,
                main_dtv,
                50.,
            ));
            edges.push(edge(
                200 + i,
                (10 + i, p(x0, 300.)),
                (11 + i, p(x0 + 2000., 300.)),
                7,
                0,
                800.,
                30.,
            ));
        }
        edges.push(edge(
            300,
            (1, p(0., 0.)),
            (10, p(0., 300.)),
            7,
            0,
            500.,
            30.,
        ));
        edges.push(edge(
            301,
            (5, p(8000., 0.)),
            (14, p(8000., 300.)),
            7,
            0,
            500.,
            30.,
        ));
        RouteGraph::from_parts(
            vec![Parsed {
                edges,
                signals: vec![2, 3, 4],
                bans: vec![],
            }],
            10.,
        )
    }

    #[test]
    fn busy_signalled_main_road_loses_to_the_quiet_parallel_street() {
        // Start und Ziel mitten in den Verbindern: beide Wege biegen gleich oft ab
        let (from, to) = ((0., 150.), (8000., 150.));
        let g = town(45_000.);
        let r = g.route(from, to, Mode::Car).expect("Route");
        assert!(
            r.edges.iter().any(|e| (200..204).contains(e)),
            "Schleichweg: {:?}",
            r.edges
        );
        // ohne Verkehr und ohne Ampeln bleibt die schnellere Hauptstraße vorn
        let mut quiet = town(0.);
        quiet.signal.iter_mut().for_each(|s| *s = false);
        let r = quiet.route(from, to, Mode::Car).expect("Route");
        assert!(
            r.edges.iter().all(|e| !(200..204).contains(e)),
            "Hauptstraße: {:?}",
            r.edges
        );
        assert!(r.edges.iter().any(|e| (100..104).contains(e)));
        // Punktfolge beginnt am Start und endet am Ziel, Länge = 15 + 800 + 15 m
        assert_eq!(r.pts[0], from);
        assert_eq!(*r.pts.last().unwrap(), to);
        assert!((r.len_m - 830.).abs() < 1., "{}", r.len_m);
    }

    #[test]
    fn one_way_streets_and_turn_bans_hold_in_the_car_not_on_foot() {
        let p = |x: f32, y: f32| [x, y];
        // Dreieck: direkt 1→2 ist Einbahn 2→1, Umweg über 3
        let parts = vec![Parsed {
            edges: vec![
                edge(1, (1, p(0., 0.)), (2, p(1000., 0.)), 7, -1, 0., 30.),
                edge(2, (1, p(0., 0.)), (3, p(500., 800.)), 7, 0, 0., 30.),
                edge(3, (3, p(500., 800.)), (2, p(1000., 0.)), 7, 0, 0., 30.),
            ],
            signals: vec![],
            bans: vec![],
        }];
        let g = RouteGraph::from_parts(parts, 10.);
        let car = g.route((10., 0.), (990., 0.), Mode::Car).unwrap();
        assert!(
            car.edges.contains(&2) && car.edges.contains(&3),
            "Umweg: {:?}",
            car.edges
        );
        let foot = g.route((10., 0.), (990., 0.), Mode::Foot).unwrap();
        assert_eq!(foot.edges, vec![1], "zu Fuß gilt keine Einbahn");
        // Abbiegeverbot 2 → 3 an Knoten 3: dann gibt es mit dem Auto keinen Weg
        let parts = vec![Parsed {
            edges: vec![
                edge(1, (1, p(0., 0.)), (2, p(1000., 0.)), 7, -1, 0., 30.),
                edge(2, (1, p(0., 0.)), (3, p(500., 800.)), 7, 0, 0., 30.),
                edge(3, (3, p(500., 800.)), (2, p(1000., 0.)), 7, 0, 0., 30.),
            ],
            signals: vec![],
            bans: vec![(2, 3, 3)],
        }];
        let g = RouteGraph::from_parts(parts, 10.);
        assert!(g.route((10., 0.), (990., 0.), Mode::Car).is_none());
    }

    #[test]
    fn turns_cost_right_less_than_left_and_straight_nothing() {
        let p = |x: f32, y: f32| [x, y];
        // Kreuzung C (Knoten 2): von Westen kommend, nach Süden (y wächst = rechts), Norden (links), Osten (geradeaus)
        let g = RouteGraph::from_parts(
            vec![Parsed {
                edges: vec![
                    edge(1, (1, p(0., 0.)), (2, p(1000., 0.)), 7, 0, 0., 30.),
                    edge(2, (2, p(1000., 0.)), (3, p(1000., 1000.)), 7, 0, 0., 30.),
                    edge(3, (2, p(1000., 0.)), (4, p(1000., -1000.)), 7, 0, 0., 30.),
                    edge(4, (2, p(1000., 0.)), (5, p(2000., 0.)), 7, 0, 0., 30.),
                ],
                signals: vec![],
                bans: vec![],
            }],
            10.,
        );
        let arc = |e: u32| {
            g.arcs
                .iter()
                .copied()
                .find(|a| a.edge == e && a.fwd)
                .unwrap()
        };
        let (inc, south, north, east) = (arc(0), arc(1), arc(2), arc(3));
        // gleichrangige Wohnstraßen: nur die Abbiegestrafe
        assert_eq!(
            g.turn_s(&inc, &south, Mode::Car),
            RIGHT_S,
            "nach Süden = rechts"
        );
        assert_eq!(
            g.turn_s(&inc, &north, Mode::Car),
            LEFT_S,
            "nach Norden = links"
        );
        assert_eq!(g.turn_s(&inc, &east, Mode::Car), 0., "geradeaus");
        assert_eq!(
            g.turn_s(&inc, &south, Mode::Foot),
            0.,
            "zu Fuß keine Abbiegestrafe"
        );
        // dieselbe Kreuzung, die Nord-Süd-Straße ist eine Hauptstraße: Lücke abwarten
        let mut major = g;
        for e in &mut major.edges {
            if e.id == 2 || e.id == 3 {
                e.cls = 3;
            }
        }
        assert_eq!(
            major.turn_s(&inc, &east, Mode::Car),
            JUNCTION_S,
            "Hauptstraße kreuzen"
        );
        assert_eq!(
            major.turn_s(&inc, &south, Mode::Car),
            JUNCTION_S + RIGHT_S,
            "auf die Hauptstraße einbiegen"
        );
    }

    #[test]
    fn snap_and_same_edge_route() {
        let g = town(0.);
        let s = g.snap(1000., -40., Mode::Car, 500.).unwrap();
        assert_eq!(g.edges[s.edge as usize].id, 100);
        assert!((s.along - 1000.).abs() < 1. && (s.dist - 40.).abs() < 1.);
        let r = g.route((500., 0.), (1500., 0.), Mode::Car).unwrap();
        assert_eq!(r.edges, vec![100]);
        assert!((r.len_m - 100.).abs() < 0.5);
        assert!(
            g.snap(1000., -9000., Mode::Car, 500.).is_none(),
            "zu weit weg"
        );
    }
}
