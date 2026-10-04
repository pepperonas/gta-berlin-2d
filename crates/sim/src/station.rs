//! Begehbare U-Bahnhöfe (Port von `station.js` und dem Bahnhofsteil von `world.js`). Die Karte kennt keine
//! Grundrisse; ein Bahnhof entsteht aus dem Fahrplan: alle unterirdischen Halte gleichen Namens mit gleich
//! verlaufender Strecke bilden einen Mittelbahnsteig genau unter der echten Strecke, so lang wie der längste Zug plus
//! Rand, je ein Gleis rechts der Fahrtrichtung, Säulen in der Mitte, an beiden Enden eine Treppe. Oben liegen die
//! Eingänge am nächsten Gehweg über den Treppen (dazu einer am Bahnhofssymbol). Die Figur behält unten ihre echten
//! Koordinaten (Ebene −2); Bewegung und Kollision laufen im Bahnhofsrahmen (u entlang der Strecke, v quer).
use crate::city::{City, Ground};
use crate::collision::{Rect, SpatialHash};
use crate::events::Event;
use crate::math::hash01;
use crate::ride::{LastStop, Ref, Ride, RideKind};
use crate::transit::{Mode, Transit, point_on_shape, position_at};
use crate::world::{Input, Notice, PLAYER_RADIUS, World};
use std::collections::HashMap;
use std::f64::consts::PI;

/// px: halbe Bahnsteigbreite (Mittelbahnsteig ≈ 9 m), Gleismitte, Wand, Rand je Ende
pub const HALF: f64 = 46.;
pub const TRACK: f64 = 64.;
pub const WALL: f64 = 92.;
pub const MARGIN: f64 = 45.;
/// px: Treppe (Länge, Breite)
pub const STAIR_L: f64 = 80.;
pub const STAIR_W: f64 = 38.;
pub const PILLAR: f64 = 7.;
pub const PILLAR_STEP: f64 = 110.;
/// px: so nah an der Bahnsteigkante lässt sich einsteigen
pub const EDGE: f64 = 16.;
/// px: so nah am Eingang reicht F zum Hinuntergehen (und der Hinweis erscheint)
pub const REACH: f64 = 70.;
/// px: Bahnsteig gilt als unterirdisch, wenn hier kein gleichgerichtetes Gleis sichtbar ist
pub const PROBE: f64 = 150.;
/// px: Eingang am Bahnhofssymbol, wenn keiner der Treppeneingänge so nah liegt
pub const POI_ENTRANCE: f64 = 150.;
/// px: Halte gleichen Namens so nah → derselbe Bahnhof
pub const GROUP: f64 = 450.;
/// px: Bahnhöfe um die Figur (Eingänge zeichnen und prüfen)
pub const NEAR: f64 = 1800.;
const TILE_COLORS: [u32; 8] = [
    0xd9c27a, 0x7fb3a0, 0xc77b5e, 0x8aa6c9, 0xe3ddcc, 0xb58db8, 0x9fb86a, 0xd69a5a,
];

fn wrap_pi(a: f64) -> f64 {
    a.rem_euclid(PI)
}
fn ang_diff(a: f64, b: f64) -> f64 {
    let d = (wrap_pi(a) - wrap_pi(b)).abs();
    d.min(PI - d)
}

/// „U Kottbusser Tor (Berlin)“, „S+U Alexanderplatz Bhf (Berlin)“ → „Kottbusser Tor“, „Alexanderplatz“.
pub fn station_name(n: &str) -> String {
    let mut s = n.trim().to_string();
    if let Some(i) = s.rfind(" (")
        && s.ends_with(')')
    {
        s.truncate(i);
    }
    for pre in ["S+U ", "U+S ", "U ", "S "] {
        if let Some(r) = s.strip_prefix(pre) {
            s = r.to_string();
            break;
        }
    }
    for suf in [" Bhf.", " Bhf"] {
        if let Some(r) = s.strip_suffix(suf) {
            s = r.to_string();
            break;
        }
    }
    s.trim().to_string()
}
fn key_of(n: &str) -> String {
    station_name(n).to_lowercase()
}
/// Vergleichsschlüssel Fahrplan ↔ OSM: „Boddinstr.“ = „Boddinstraße“, ohne Satzzeichen und Leerzeichen.
pub fn match_key(n: &str) -> String {
    key_of(n)
        .replace("straße", "str")
        .replace("strasse", "str")
        .replace("str.", "str")
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

/// Halt eines Musters an diesem Bahnsteig: Richtung +1 = mit der Achse.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Halt {
    pub pid: usize,
    pub i: usize,
    pub dir: i8,
}
/// Eingang oben: Bahnsteigende, Lage, Blick zur Straße, Straßenname, Haupteingang (am Bahnhofssymbol).
#[derive(Debug, Clone, PartialEq)]
pub struct Exit {
    pub e: i8,
    pub x: f64,
    pub y: f64,
    pub face: f64,
    pub street: String,
    pub main: bool,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Station {
    pub id: String,
    pub key: String,
    pub name: String,
    pub lines: Vec<String>,
    pub sbahn: bool,
    pub x: f64,
    pub y: f64,
    pub axis: f64,
    pub ax: f64,
    pub ay: f64,
    /// halbe Bahnsteiglänge, längster Zug
    pub hl: f64,
    pub l: f64,
    pub color: u32,
    pub halts: Vec<Halt>,
    pub exits: Vec<Exit>,
}
impl Station {
    /// Welt → Bahnhofsrahmen (u entlang, v quer, rechts positiv).
    pub fn to_local(&self, x: f64, y: f64) -> (f64, f64) {
        let (dx, dy) = (x - self.x, y - self.y);
        (dx * self.ax + dy * self.ay, -dx * self.ay + dy * self.ax)
    }
    pub fn to_world(&self, u: f64, v: f64) -> (f64, f64) {
        (
            self.x + self.ax * u - self.ay * v,
            self.y + self.ay * u + self.ax * v,
        )
    }
    /// Säulen in der Bahnsteigmitte (nicht an den Treppen): u-Lagen.
    pub fn pillars(&self) -> Vec<f64> {
        let lim = self.hl - STAIR_L - 30.;
        let mut out = Vec::new();
        let mut u = -(lim / PILLAR_STEP).floor() * PILLAR_STEP;
        while u <= lim {
            if u.abs() > 40. {
                out.push(u);
            }
            u += PILLAR_STEP;
        }
        out
    }
    /// Figur im Bahnhof halten (Bahnsteigkanten, Säulen); liefert den Rahmenpunkt.
    pub fn keep_inside(&self, x: &mut f64, y: &mut f64, r: f64) -> (f64, f64) {
        let (mut u, mut v) = self.to_local(*x, *y);
        u = u.clamp(-self.hl + r, self.hl - r);
        v = v.clamp(-HALF + r, HALF - r);
        for pu in self.pillars() {
            let (du, dv) = (u - pu, v);
            let d = du.hypot(dv);
            let m = PILLAR + r;
            if d < m {
                if d > 1e-6 {
                    let k = m / d;
                    (u, v) = (pu + du * k, dv * k);
                } else {
                    (u, v) = (pu + m, 0.);
                }
            }
        }
        (*x, *y) = self.to_world(u, v);
        (u, v)
    }
    /// Steht die Figur auf einer Treppe? → Ende (−1/1) oder 0.
    pub fn stair_at(&self, x: f64, y: f64) -> i8 {
        let (u, v) = self.to_local(x, y);
        if v.abs() > STAIR_W / 2. {
            0
        } else if u > self.hl - STAIR_L * 0.45 {
            1
        } else if u < -self.hl + STAIR_L * 0.45 {
            -1
        } else {
            0
        }
    }
    /// Ankunftsplatz unten an der Treppe e (Blick in den Bahnsteig).
    pub fn arrival_at(&self, e: i8) -> (f64, f64, f64) {
        let (x, y) = self.to_world(e as f64 * (self.hl - STAIR_L - 16.), 0.);
        (x, y, self.axis + if e > 0 { PI } else { 0. })
    }
    /// Wartende auf dem Bahnsteig (nur Darstellung, aus dem Bahnhof und der Stunde): (u, v, Seite).
    pub fn waiting(&self, hour: u32) -> Vec<(f64, f64, i8)> {
        let n = 5 + (hash01(self.x * 3. + self.y + hour as f64) * 9.) as usize;
        (0..n)
            .map(|k| {
                let h = |a: f64| hash01(self.x * 7. + self.y * 13. + (k * 101) as f64 + a);
                let side: i8 = if h(1.) < 0.5 { -1 } else { 1 };
                let u = (h(2.) * 2. - 1.) * (self.hl - STAIR_L - 40.);
                (u, side as f64 * (HALF - 12. - h(3.) * 14.), side)
            })
            .collect()
    }
}

/// Zug am Bahnsteig: Referenz, Halt, Richtung, hält, Wagen (u, v, Länge, Breite), Linie, Ziel, Farbe, Art.
#[derive(Debug, Clone, PartialEq)]
pub struct AtPlatform {
    pub r: Ref,
    pub pid: usize,
    pub i: usize,
    pub dir: i8,
    pub dwelling: bool,
    pub cars: Vec<(f64, f64, f64, f64)>,
    pub line: String,
    pub dest: String,
    pub color: String,
    pub mode: Mode,
}
/// Abfahrt (Fahrgastinfo).
#[derive(Debug, Clone, PartialEq)]
pub struct Departure {
    pub line: String,
    pub dest: String,
    pub sec: f64,
    pub dir: i8,
    pub color: String,
}

#[derive(Debug, Clone)]
struct StopIt {
    pid: usize,
    i: usize,
    x: f64,
    y: f64,
    angle: f64,
    name: String,
    key: String,
}
/// Bahnhöfe je Fahrplan: Halte-Index und fertige Bahnsteige (`None` = noch nicht entscheidbar).
#[derive(Default)]
pub struct Cache {
    stops: Vec<StopIt>,
    hash: Option<SpatialHash>,
    by_key: HashMap<String, Vec<usize>>,
    built: HashMap<String, Vec<Station>>,
    pub by_id: HashMap<String, Station>,
}
impl Cache {
    fn index(&mut self, tr: &Transit) {
        if self.hash.is_some() {
            return;
        }
        let mut hash = SpatialHash::new(1600.);
        for p in &tr.patterns {
            if !p.mode.rail() {
                continue;
            }
            let sh = tr.shape_of(p);
            for (i, &s) in p.stops.iter().enumerate() {
                let (x, y, angle) = point_on_shape(sh, s);
                let raw = p.stop_names.get(i).cloned().unwrap_or_default();
                let key = key_of(&raw);
                if key.is_empty() {
                    continue;
                }
                let idx = self.stops.len();
                hash.insert(idx as u32, &Rect::new(x, y, 0., 0.));
                self.by_key.entry(key.clone()).or_default().push(idx);
                self.stops.push(StopIt {
                    pid: p.id,
                    i,
                    x,
                    y,
                    angle,
                    name: station_name(&raw),
                    key,
                });
            }
        }
        self.hash = Some(hash);
    }
}

/// Unterirdisch? Fünf Punkte je haltendem Zug ohne gleichgerichtetes sichtbares Gleis; `None` = Kacheln fehlen.
fn platform_underground(city: &mut City, tr: &Transit, stops: &[&StopIt]) -> Option<bool> {
    for s in stops {
        let p = &tr.patterns[s.pid];
        let sh = tr.shape_of(p);
        let l = p.mode.train_len();
        for k in 0..=4 {
            let (x, y, a) = point_on_shape(sh, p.stops[s.i] - l * k as f64 / 4.);
            if !city.ready(x, y, PROBE) {
                return None;
            }
            if crate::tunnel::rail_at(city, x, y, Some((a.cos(), a.sin())), PROBE) {
                return Some(false);
            }
        }
    }
    Some(true)
}

impl World {
    /// Eingang am nächsten Gehweg zu (wx, wy); `here`: der Punkt selbst, wenn man dort gehen kann.
    fn entrance_at(&mut self, axis: f64, wx: f64, wy: f64, e: i8, here: bool) -> Exit {
        let walkable = here
            && self.city.in_building(wx, wy).is_none()
            && matches!(
                self.city.surface_at(wx, wy, Some(0)),
                Ground::Sidewalk | Ground::Plaza | Ground::Grass
            );
        let mut p = (wx, wy);
        if !walkable
            && let Some(sp) =
                crate::pedestrians::nearest_spot(&mut self.city, &mut self.sidewalks, wx, wy, 300.)
        {
            p = self.sidewalks.point(&mut self.city, sp.edge, sp.side, sp.s);
        }
        if self.city.in_building(p.0, p.1).is_some() {
            p = (wx, wy);
        }
        let road = self.city.nearest_edge(p.0, p.1, 400., |x| x.cls <= 10);
        let street = road
            .as_ref()
            .and_then(|r| self.city.edges.get(&r.edge))
            .map(|e| e.name.clone())
            .unwrap_or_default();
        Exit {
            e,
            x: p.0,
            y: p.1,
            face: road.map_or(axis, |r| (r.y - p.1).atan2(r.x - p.0)),
            street,
            main: false,
        }
    }

    /// Aus Halten gleichen Namens Bahnsteige bilden; `None`, solange Kacheln fehlen.
    fn build_platforms(&mut self, tr: &Transit, its: &[StopIt]) -> Option<Vec<Station>> {
        struct G<'a> {
            key: String,
            name: String,
            axis: f64,
            x: f64,
            y: f64,
            stops: Vec<&'a StopIt>,
        }
        let mut groups: Vec<G> = Vec::new();
        for st in its {
            let found = groups.iter_mut().position(|g| {
                g.key == st.key
                    && ang_diff(g.axis, st.angle) < 0.45
                    && (g.x - st.x).hypot(g.y - st.y) < GROUP
            });
            match found {
                Some(i) => groups[i].stops.push(st),
                None => groups.push(G {
                    key: st.key.clone(),
                    name: st.name.clone(),
                    axis: wrap_pi(st.angle),
                    x: st.x,
                    y: st.y,
                    stops: vec![st],
                }),
            }
        }
        let mut out = Vec::new();
        for g in groups {
            if !platform_underground(&mut self.city, tr, &g.stops)? {
                continue;
            }
            let mut axis = g.axis;
            if axis.cos() < 0. {
                axis -= PI; // Schrift auf den Schildern nicht kopfüber
            }
            let l = g
                .stops
                .iter()
                .map(|s| tr.patterns[s.pid].mode.train_len())
                .fold(0., f64::max);
            let hl = l / 2. + MARGIN;
            let (ax, ay) = (axis.cos(), axis.sin());
            let (mut cx, mut cy) = (0., 0.);
            for s in &g.stops {
                let p = &tr.patterns[s.pid];
                let (x, y, _) =
                    point_on_shape(tr.shape_of(p), p.stops[s.i] - p.mode.train_len() / 2.);
                cx += x;
                cy += y;
            }
            cx /= g.stops.len() as f64;
            cy /= g.stops.len() as f64;
            let mut lines: Vec<String> = g
                .stops
                .iter()
                .map(|s| tr.patterns[s.pid].name.clone())
                .collect();
            lines.sort();
            lines.dedup();
            let has = |m: Mode| g.stops.iter().any(|s| tr.patterns[s.pid].mode == m);
            let kc: Vec<u32> = g.key.chars().map(|c| c as u32).collect();
            let color = TILE_COLORS[(hash01(
                (g.key.chars().count() * 131) as f64
                    + kc.first().copied().unwrap_or(0) as f64 * 7.
                    + kc.get(1).copied().unwrap_or(0) as f64,
            ) * TILE_COLORS.len() as f64) as usize
                % TILE_COLORS.len()];
            let halts = g
                .stops
                .iter()
                .map(|s| Halt {
                    pid: s.pid,
                    i: s.i,
                    dir: if (s.angle - axis).cos() >= 0. { 1 } else { -1 },
                })
                .collect();
            let mut st = Station {
                id: format!("{}|{}|{}", g.key, lines.join(","), (axis * 100.).round()),
                key: g.key.clone(),
                name: g.name.clone(),
                lines,
                sbahn: has(Mode::SBahn) && !has(Mode::UBahn),
                x: cx,
                y: cy,
                axis,
                ax,
                ay,
                hl,
                l,
                color,
                halts,
                exits: Vec::new(),
            };
            for e in [-1i8, 1] {
                let u = e as f64 * (hl - STAIR_L / 2.);
                let ex = self.entrance_at(axis, cx + ax * u, cy + ay * u, e, false);
                st.exits.push(ex);
            }
            // Eingang am Bahnhofssymbol (dort sucht man ihn), wenn keiner der beiden in der Nähe liegt
            let r = hl + 400.;
            let want = if st.sbahn { "sbahn" } else { "ubahn" };
            let mk = match_key(&st.name);
            let poi = self
                .city
                .pois_near(cx, cy, r)
                .into_iter()
                .filter(|q| matches!(q.cat, "ubahn" | "sbahn") && match_key(&q.name) == mk)
                .map(|q| {
                    let d = (q.x - cx).hypot(q.y - cy) + if q.cat == want { 0. } else { 1e6 };
                    (d, q.x, q.y)
                })
                .filter(|q| q.0 < r)
                .min_by(|a, b| a.0.total_cmp(&b.0));
            if let Some((_, px, py)) = poi
                && st
                    .exits
                    .iter()
                    .all(|e| (e.x - px).hypot(e.y - py) > POI_ENTRANCE)
            {
                let (u, _) = st.to_local(px, py);
                let mut ex = self.entrance_at(axis, px, py, if u < 0. { -1 } else { 1 }, true);
                ex.main = true;
                st.exits.push(ex);
            }
            out.push(st);
        }
        Some(out)
    }

    /// Bahnhöfe um (x, y) (zwischengespeichert; noch nicht Entscheidbares wird später neu versucht).
    pub fn stations_near(&mut self, x: f64, y: f64, r: f64) -> Vec<Station> {
        let Some(tr) = self.transit.take() else {
            return Vec::new();
        };
        let mut cache = std::mem::take(&mut self.stations);
        cache.index(&tr);
        let mut hits = Vec::new();
        if let Some(h) = cache.hash.as_mut() {
            h.query(&Rect::around(x, y, r), &mut hits);
        }
        let mut keys: Vec<String> = hits
            .into_iter()
            .map(|i| cache.stops[i as usize].key.clone())
            .collect();
        keys.sort();
        keys.dedup();
        let mut out = Vec::new();
        for k in keys {
            if !cache.built.contains_key(&k) {
                let its: Vec<StopIt> = cache.by_key[&k]
                    .iter()
                    .map(|&i| cache.stops[i].clone())
                    .collect();
                if let Some(list) = self.build_platforms(&tr, &its) {
                    for s in &list {
                        cache.by_id.insert(s.id.clone(), s.clone());
                    }
                    cache.built.insert(k.clone(), list);
                }
            }
            for s in cache.built.get(&k).into_iter().flatten() {
                if (s.x - x).hypot(s.y - y) < r + s.hl {
                    out.push(s.clone());
                }
            }
        }
        self.stations = cache;
        self.transit = Some(tr);
        out
    }
    pub fn station_by_id(&self, id: &str) -> Option<&Station> {
        self.stations.by_id.get(id)
    }

    /// Züge am Bahnsteig: haltende stehen mit der Spitze am Bahnsteigende, ein- und ausfahrende gleiten entlang.
    pub fn trains_at(&self, st: &Station) -> Vec<AtPlatform> {
        let Some(tr) = self.transit.as_ref() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for h in &st.halts {
            let Some(s) = self.transit_state.tracked.get(&h.pid) else {
                continue;
            };
            let p = &tr.patterns[h.pid];
            let (n, cl, cw, gap) = p.mode.train();
            let l = p.mode.train_len();
            let stop_s = p.stops[h.i];
            for v in &s.veh {
                if v.gone || v.live.is_some() {
                    continue;
                }
                let pos = position_at(p, v.tau);
                let ds = pos.s - stop_s;
                if ds < -st.hl - l || ds > st.hl + l {
                    continue;
                }
                let dir = h.dir as f64;
                let head = dir * (l / 2. + ds);
                let cars = (0..n)
                    .map(|c| {
                        (
                            head - dir * (c as f64 * (cl + gap) + cl / 2.),
                            dir * TRACK,
                            cl,
                            cw,
                        )
                    })
                    .collect();
                out.push(AtPlatform {
                    r: Ref::Veh {
                        pid: h.pid,
                        key: v.key.clone(),
                    },
                    pid: h.pid,
                    i: h.i,
                    dir: h.dir,
                    dwelling: pos.dwelling && pos.stop == h.i && ds.abs() < 1.,
                    cars,
                    line: p.name.clone(),
                    dest: station_name(p.stop_names.last().map_or("", String::as_str)),
                    color: p.color.clone(),
                    mode: p.mode,
                });
            }
        }
        out
    }
    /// Der Zug, in den man an (x, y) einsteigen kann (hält, eigene Seite, Figur am Rand neben einem Wagen).
    pub fn boardable(&self, st: &Station, x: f64, y: f64) -> Option<(AtPlatform, usize)> {
        let (u, v) = st.to_local(x, y);
        if v.abs() < HALF - EDGE {
            return None;
        }
        let side = v.signum() as i8;
        for t in self.trains_at(st) {
            if !t.dwelling || t.dir != side {
                continue;
            }
            if let Some(car) = t.cars.iter().position(|c| (c.0 - u).abs() <= c.2 / 2. + 4.) {
                return Some((t, car));
            }
        }
        None
    }
    /// Nächste Abfahrten (je Richtung die zwei nächsten).
    pub fn departures(&self, st: &Station) -> Vec<Departure> {
        let Some(tr) = self.transit.as_ref() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for h in &st.halts {
            let Some(s) = self.transit_state.tracked.get(&h.pid) else {
                continue;
            };
            let p = &tr.patterns[h.pid];
            let last = h.i == p.off.len() - 1;
            let arrive = if h.i == 0 {
                0.
            } else {
                p.off[h.i]
                    - if last {
                        0.
                    } else {
                        p.dwell.min((p.off[h.i] - p.off[h.i - 1]) * 0.4)
                    }
            };
            for v in &s.veh {
                if v.gone || v.live.is_some() {
                    continue;
                }
                let sec = arrive - v.tau;
                if sec < -p.dwell || sec > 1200. {
                    continue;
                }
                out.push(Departure {
                    line: p.name.clone(),
                    dest: station_name(p.stop_names.last().map_or("", String::as_str)),
                    sec: sec.max(0.),
                    dir: h.dir,
                    color: p.color.clone(),
                });
            }
        }
        out.sort_by(|a, b| a.sec.total_cmp(&b.sec));
        let (mut a, mut b) = (0, 0);
        out.retain(|d| {
            let c = if d.dir < 0 { &mut a } else { &mut b };
            *c += 1;
            *c <= 2
        });
        out
    }

    /// Nächster Eingang im Umkreis r: (Bahnhof, Eingang, Abstand).
    pub fn entrance_near(&self, x: f64, y: f64, r: f64) -> Option<(Station, Exit, f64)> {
        let mut best: Option<(Station, Exit, f64)> = None;
        for stn in &self.st_near {
            for ex in &stn.exits {
                let d = (ex.x - x).hypot(ex.y - y);
                if d < r && best.as_ref().is_none_or(|b| d < b.2) {
                    best = Some((stn.clone(), ex.clone(), d));
                }
            }
        }
        best
    }

    /// Bahnhöfe um die Figur (bzw. ihr Auto) alle 0,5 s und nach jedem Kachelwechsel neu.
    pub fn refresh_stations(&mut self) {
        self.st_tick = self.st_tick.saturating_sub(1);
        if self.st_tick > 0 && self.st_gen == self.city.generation {
            return;
        }
        self.st_tick = 30;
        self.st_gen = self.city.generation;
        let (x, y) = self
            .player_car()
            .map(|c| (c.x, c.y))
            .unwrap_or((self.player.x, self.player.y));
        self.st_near = if self.transit.is_some() {
            self.stations_near(x, y, 1200.)
        } else {
            Vec::new()
        };
    }

    /// Zu Fuß: Treppe hinauf (Ausgang an der Straße) bzw. an der Straße mit F in einen Eingang hinunter.
    pub fn update_station_presence(&mut self, input: &Input) {
        if let Some(ins) = self.player.inside.clone() {
            let Some(stn) = self.station_by_id(&ins.id).cloned() else {
                self.player.inside = None;
                self.player.level.lvl = 0;
                return;
            };
            let e = stn.stair_at(self.player.x, self.player.y);
            if e == 0 {
                if let Some(i) = self.player.inside.as_mut() {
                    i.guard = false;
                }
                return;
            }
            if ins.guard {
                return; // gerade heruntergekommen: erst von der Treppe gehen
            }
            let ex = &stn.exits[if e < 0 { 0 } else { 1 }];
            self.player.inside = None;
            (self.player.x, self.player.y) = (ex.x, ex.y);
            self.player.level.lvl = 0;
            self.player.click = None;
            self.player.entry_guard = Some((ex.x, ex.y)); // nicht gleich wieder hinunter
            self.events.push(Event::StationExit {
                x: ex.x,
                y: ex.y,
                name: stn.name.clone(),
            });
            return;
        }
        if let Some((gx, gy)) = self.player.entry_guard
            && (self.player.x - gx).hypot(self.player.y - gy) > 30.
        {
            self.player.entry_guard = None;
        }
        if self.player.level.lvl != 0 || !input.enter_exit || self.player.entry_guard.is_some() {
            return;
        }
        let Some((stn, ex, _)) = self.entrance_near(self.player.x, self.player.y, REACH) else {
            return;
        };
        let (x, y, a) = stn.arrival_at(ex.e);
        (self.player.x, self.player.y, self.player.angle) = (x, y, a);
        self.player.level.lvl = -2;
        self.player.click = None;
        self.player.inside = Some(Inside {
            id: stn.id.clone(),
            guard: true,
        });
        self.events.push(Event::StationEnter {
            x,
            y,
            name: stn.name.clone(),
        });
    }

    /// Einsteigen am Bahnsteig (G): haltender Zug auf der eigenen Seite, Figur am Rand neben einem Wagen.
    pub fn board_at_platform(&mut self) -> bool {
        let Some(ins) = self.player.inside.clone() else {
            return false;
        };
        let Some(stn) = self.station_by_id(&ins.id).cloned() else {
            return false;
        };
        if self.player.stun > 0. {
            return false;
        }
        let Some((t, car)) = self.boardable(&stn, self.player.x, self.player.y) else {
            self.notice = Some(Notice {
                text: "Zum Einsteigen an die Bahnsteigkante neben einen haltenden Zug".into(),
                t: 1.5,
            });
            return false;
        };
        let Some(vs) = self.vehicle_state(&t.r) else {
            return false;
        };
        let tr = self.transit.as_ref().expect("Fahrplan");
        let p = &tr.patterns[vs.pid];
        let i = vs.stop.min(p.stops.len() - 1);
        let (sx, sy, _) = point_on_shape(tr.shape_of(p), p.stops[i]);
        let last = LastStop {
            x: sx,
            y: sy,
            name: p.stop_names.get(i).cloned().unwrap_or_default(),
            i,
            pid: vs.pid,
        };
        let (line, dest) = (
            p.name.clone(),
            p.stop_names.last().cloned().unwrap_or_default(),
        );
        self.player.inside = None;
        self.player.ride = Some(Ride {
            kind: RideKind::Passenger,
            r: t.r,
            mode: vs.mode,
            car,
            last_stop: last,
            since: self.time,
            line: line.clone(),
            dest,
            speed: 0.,
            underground: true,
        });
        let (x, y) = (self.player.x, self.player.y);
        self.events.push(Event::Board {
            line,
            hop: false,
            x,
            y,
        });
        true
    }

    /// Aussteigen unter Tage auf den Bahnsteig neben Wagen `car` (wenn der Halt einen begehbaren Bahnhof hat).
    pub fn platform_arrival(&mut self, pid: usize, stop: usize, car: usize) -> bool {
        let Some((qx, qy)) = self.transit.as_ref().map(|tr| {
            let p = &tr.patterns[pid];
            let (x, y, _) = point_on_shape(tr.shape_of(p), p.stops[stop.min(p.stops.len() - 1)]);
            (x, y)
        }) else {
            return false;
        };
        let stns = self.stations_near(qx, qy, 800.);
        let Some(stn) = stns
            .into_iter()
            .find(|s| s.halts.iter().any(|h| h.pid == pid && h.i == stop))
        else {
            return false;
        };
        let h = *stn
            .halts
            .iter()
            .find(|h| h.pid == pid && h.i == stop)
            .expect("Halt");
        let mode = self
            .transit
            .as_ref()
            .map_or(Mode::UBahn, |t| t.patterns[pid].mode);
        let (n, cl, _, gap) = mode.train();
        let dir = h.dir as f64;
        let u = dir * (stn.l / 2. - (car.min(n - 1) as f64 * (cl + gap) + cl / 2.));
        let (mut x, mut y) = stn.to_world(u, dir * (HALF - 12.));
        stn.keep_inside(&mut x, &mut y, PLAYER_RADIUS);
        self.player.ride = None;
        (self.player.x, self.player.y) = (x, y);
        self.player.level.lvl = -2;
        self.player.click = None;
        self.player.inside = Some(Inside {
            id: stn.id.clone(),
            guard: false,
        });
        true
    }
}

/// Figur im Bahnhof (Bahnhofs-ID; `guard` = gerade heruntergekommen, erst von der Treppe gehen).
#[derive(Debug, Clone, PartialEq)]
pub struct Inside {
    pub id: String,
    pub guard: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names_and_frames() {
        assert_eq!(station_name("U Kottbusser Tor (Berlin)"), "Kottbusser Tor");
        assert_eq!(
            station_name("S+U Alexanderplatz Bhf (Berlin)"),
            "Alexanderplatz"
        );
        assert_eq!(
            match_key("U Boddinstr. (Berlin)"),
            match_key("Boddinstraße")
        );
        let st = Station {
            id: "x".into(),
            key: "x".into(),
            name: "X".into(),
            lines: vec![],
            sbahn: false,
            x: 100.,
            y: 50.,
            axis: 0.5,
            ax: 0.5f64.cos(),
            ay: 0.5f64.sin(),
            hl: 600.,
            l: 1000.,
            color: 0,
            halts: vec![],
            exits: vec![],
        };
        let (wx, wy) = st.to_world(120., -30.);
        let (u, v) = st.to_local(wx, wy);
        assert!((u - 120.).abs() < 1e-9 && (v + 30.).abs() < 1e-9);
        let (mut x, mut y) = st.to_world(2000., 500.);
        let (u, v) = st.keep_inside(&mut x, &mut y, 7.);
        assert!(
            (u - 593.).abs() < 1e-9 && (v - 39.).abs() < 1e-9,
            "an Kante und Ende gehalten"
        );
        let pil = st.pillars();
        assert!(
            !pil.is_empty()
                && pil
                    .iter()
                    .all(|u| u.abs() > 40. && u.abs() <= 600. - STAIR_L - 30.)
        );
        let (mut x, mut y) = st.to_world(pil[0], 0.);
        let (u, v) = st.keep_inside(&mut x, &mut y, 7.);
        assert!(
            (u - pil[0]).hypot(v) >= PILLAR + 7. - 1e-9,
            "nicht in der Säule"
        );
        let (sx, sy) = st.to_world(590., 0.);
        assert_eq!(st.stair_at(sx, sy), 1);
        assert_eq!(st.stair_at(st.x, st.y), 0);
        let (ax, ay, _) = st.arrival_at(-1);
        assert_eq!(st.stair_at(ax, ay), 0, "Ankunft neben der Treppe");
        assert_eq!(st.waiting(8), st.waiting(8));
    }
}
