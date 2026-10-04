//! Stadtmodell der Simulation (Port der Simulationsteile von `map.js` und `levels.js`).
//!
//! Die Darstellung (`berlin-map-loader`) und die Simulation lesen dieselben Kacheln, brauchen aber Verschiedenes:
//! hier zählen Straßengraph mit Querschnitt, Knoten mit Kreuzungsradius, Kollisionswände, Gebäude- und
//! Flächenpolygone für den Untergrund, Poller, Bäume, Ampeln, Abbiegeverbote und Portale. Kacheln werden um Foki
//! nachgeladen; Objekte, die in mehreren Kacheln liegen (globale Nummer), werden einmal angelegt und erst entfernt,
//! wenn keine Kachel sie mehr hält. Abfragen laufen über Raster-Hashes wie in der JS-Fassung.
use crate::collision::{Rect, Segment, SpatialHash};
use anyhow::{Context, Result, ensure};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc;

pub type Pt = (f64, f64);

/// Untergrund (map.js `T`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ground {
    Road,
    Sidewalk,
    Building,
    Grass,
    Water,
    Plaza,
    Cobble,
}
impl Ground {
    pub fn is_road(self) -> bool {
        matches!(self, Ground::Road | Ground::Cobble)
    }
}

/// Nachladen (px um den Fokus): bis `ready` muss alles da sein, bis `load` wird vorgeladen, jenseits `keep` frei.
pub const STREAM_READY: f64 = 4000.;
pub const STREAM_LOAD: f64 = 7000.;
pub const STREAM_KEEP: f64 = 11000.;
pub const ROAD_CLASS_BUSWAY: u8 = 12;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Side {
    pub park: u8,
    pub park_w: f64,
    pub orient: u8,
    pub cycle: f64,
    pub track: f64,
}
/// Straßenquerschnitt in px.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CrossSection {
    pub width: f64,
    pub fwd: u32,
    pub bwd: u32,
    pub left: Side,
    pub right: Side,
    pub maxspeed: f64,
    pub surface: u8,
    pub lit: bool,
    pub gaslight: bool,
    pub bus_contra: bool,
}
#[derive(Debug, Clone)]
pub struct Crossing {
    pub x: f64,
    pub y: f64,
    pub s: f64,
    pub kind: u8, // 0 Zebra, 1 Ampel, 2 markiert
}
#[derive(Debug, Clone)]
pub struct Edge {
    pub id: i64,
    pub a: i64,
    pub b: i64,
    pub cls: u8,
    pub w: f64,
    pub cs: CrossSection,
    pub name: String,
    pub oneway: i8,
    pub bridge: bool,
    pub inside: bool,
    pub lvl: i8,
    pub blocked: bool,
    pub passage: bool,
    pub pts: Vec<Pt>,
    pub len: f64,
    pub dtv: f64,
    pub fill: f64,
    pub crossings: Vec<Crossing>,
}
#[derive(Debug, Clone)]
pub struct Node {
    pub id: i64,
    pub x: f64,
    pub y: f64,
    /// Kanten am Knoten, nach Nummer sortiert (gleiche Reihenfolge, egal in welcher Folge Kacheln laden)
    pub edges: Vec<i64>,
    pub trim: f64,
}
#[derive(Debug, Clone, Copy)]
pub struct Junction {
    pub x: f64,
    pub y: f64,
    pub r: f64,
    pub node: i64,
    pub cobble: bool,
    pub hi: i8,
    pub lo: i8,
    pub cls: u8,
}
/// Stück einer Straße (oder Kreuzungsscheibe) im Abfrage-Raster.
#[derive(Debug, Clone, Copy)]
pub struct EdgeSeg {
    pub edge: Option<i64>,
    pub junction: Option<Junction>,
    pub i: usize,
    pub ax: f64,
    pub ay: f64,
    pub bx: f64,
    pub by: f64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WallKind {
    Border,
    Wall,
    Building,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WallSub {
    Other,
    Quay,
    Rail,
    Railing,
    Fence,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CircleKind {
    Tree,
    /// Poller/Schranke: kann umgefahren werden (Schlüssel = gerundete Lage)
    Barrier {
        key: (i64, i64),
    },
}
/// Festes Hindernis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Solid {
    Wall {
        seg: Segment,
        kind: WallKind,
        sub: WallSub,
        lvl: i8,
    },
    Circle {
        x: f64,
        y: f64,
        r: f64,
        kind: CircleKind,
    },
    Rect(Rect),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolyKind {
    Building,
    Water,
    Area { kind: u8, lvl: i8 },
}
#[derive(Debug, Clone)]
pub struct Poly {
    pub kind: PolyKind,
    pub rings: Vec<Vec<Pt>>,
    pub id: i64,
    /// Gebäudeart (`citycodes.js BUILDING_KIND`: 3 = Kirche), sonst 0
    pub bkind: u8,
}
#[derive(Debug, Clone, Copy)]
pub struct Portal {
    pub x: f64,
    pub y: f64,
    pub r: f64,
    pub lo: i8,
    pub hi: i8,
}
/// Gleis (Mittellinie; Tunnelgleise verwirft der Build): Brücke, U-Bahn, Ebene.
#[derive(Debug, Clone)]
pub struct RailLine {
    pub pts: Vec<Pt>,
    pub bridge: bool,
    pub subway: bool,
    pub lvl: i8,
}
#[derive(Debug, Clone)]
pub struct LevelPath {
    pub pts: Vec<Pt>,
    pub lvl: i8,
}
#[derive(Debug, Clone, Copy, Default)]
pub struct Place {
    pub x: f64,
    pub y: f64,
    pub angle: f64,
}
#[derive(Debug, Clone, Default)]
pub struct Places {
    pub giver: Place,
    pub player_spawn: Place,
    pub dropoff: Place,
    pub player_car: Place,
    pub pickup: Place,
    pub parked: Vec<Place>,
    pub crates: Vec<Rect>,
    pub time_limit: Option<f64>,
}
#[derive(Debug, Clone)]
pub struct Hospital {
    pub x: f64,
    pub y: f64,
    pub name: String,
}

/// Einfache Slot-Liste mit stabilen Handles für Raster-Einträge.
#[derive(Debug, Clone)]
pub struct Slab<T> {
    items: Vec<Option<T>>,
    free: Vec<u32>,
}
impl<T> Default for Slab<T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            free: Vec::new(),
        }
    }
}
impl<T> Slab<T> {
    pub fn insert(&mut self, v: T) -> u32 {
        if let Some(i) = self.free.pop() {
            self.items[i as usize] = Some(v);
            i
        } else {
            self.items.push(Some(v));
            (self.items.len() - 1) as u32
        }
    }
    pub fn remove(&mut self, i: u32) -> Option<T> {
        let v = self.items.get_mut(i as usize)?.take();
        if v.is_some() {
            self.free.push(i);
        }
        v
    }
    pub fn get(&self, i: u32) -> Option<&T> {
        self.items.get(i as usize)?.as_ref()
    }
    pub fn len(&self) -> usize {
        self.items.len() - self.free.len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.items.iter().flatten()
    }
}

/// Raster + Slot-Liste.
#[derive(Debug, Clone)]
pub struct Layer<T> {
    pub slab: Slab<T>,
    pub hash: SpatialHash,
    buf: Vec<u32>,
}
impl<T> Layer<T> {
    fn new(cell: f64) -> Self {
        Self {
            slab: Slab::default(),
            hash: SpatialHash::new(cell),
            buf: Vec::new(),
        }
    }
    fn add(&mut self, v: T, b: &Rect) -> (u32, Vec<(i32, i32)>) {
        let h = self.slab.insert(v);
        let keys = self.hash.insert(h, b);
        (h, keys)
    }
    fn drop_item(&mut self, h: u32, keys: &[(i32, i32)]) {
        self.hash.remove(h, keys);
        self.slab.remove(h);
    }
    /// Handles im Rechteck (ohne Doppelte, in Fundreihenfolge).
    pub fn query(&mut self, b: &Rect) -> Vec<u32> {
        let mut out = std::mem::take(&mut self.buf);
        self.hash.query(b, &mut out);
        let res = out.clone();
        self.buf = out;
        res
    }
    pub fn get(&self, h: u32) -> &T {
        self.slab.get(h).expect("Raster-Eintrag ohne Objekt")
    }
}

#[derive(Debug)]
enum Owned {
    Solid(u32, Vec<(i32, i32)>),
    Seg(u32, Vec<(i32, i32)>),
    Poly(u32, Vec<(i32, i32)>),
    Portal(u32, Vec<(i32, i32)>),
    Path(u32, Vec<(i32, i32)>),
    Edge(i64),
    Crossing(i64),
    Poi(u32, Vec<(i32, i32)>),
    Addr(u32, Vec<(i32, i32)>),
    Furn(u32, Vec<(i32, i32)>),
    Rail(u32, Vec<(i32, i32)>),
    Dens(String),
}

/// Ort von Interesse (Bahnhof, Laden, Bar …): Kategorie aus `POI_CATS`, Name, OSM-Art.
#[derive(Debug, Clone, PartialEq)]
pub struct Poi {
    pub x: f64,
    pub y: f64,
    pub cat: &'static str,
    pub name: String,
    pub kind: String,
}
/// Hausnummer an einer Straße.
#[derive(Debug, Clone, PartialEq)]
pub struct Address {
    pub x: f64,
    pub y: f64,
    pub street: String,
    pub nr: String,
}
/// Stadtmöbel: 0 Bank, 1 Fahrradständer, 2 Mülleimer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Furn {
    pub x: f64,
    pub y: f64,
    pub kind: u8,
}
/// Einwohnerdichte einer Kachel (Raster `cell` px, Werte Einwohner je Zelle).
#[derive(Debug, Clone, PartialEq)]
pub struct Dens {
    pub x0: f64,
    pub y0: f64,
    pub cell: f64,
    pub per: usize,
    pub vals: Vec<f64>,
}
/// Kategorien der POIs (citycodes.js POI_CATS).
pub const POI_CATS: [&str; 13] = [
    "ubahn",
    "sbahn",
    "bahn",
    "bus",
    "mall",
    "supermarket",
    "shop",
    "food",
    "drink",
    "cafe",
    "service",
    "culture",
    "hotel",
];
/// Kiez bzw. Ortsteil mit Namen.
#[derive(Debug, Clone, PartialEq)]
pub struct Named {
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub rings: Vec<Vec<Pt>>,
}
#[derive(Debug, Default)]
struct Entry {
    refs: u32,
    owned: Vec<Owned>,
}
#[derive(Debug, Default)]
struct TileState {
    shared: Vec<String>,
    local: Vec<Entry>,
    signals: Vec<i64>,
    bans: Vec<(i64, i64, i64)>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeEvent {
    Added(i64),
    Removed(i64),
}

/// Dekodierte Kachel (nur die Simulationsteile), unabhängig vom Einbau – kann im Hintergrund entstehen.
#[derive(Debug, Clone)]
pub struct TileData {
    pub key: (u32, u32),
    names: Vec<String>,
    vertices: Vec<(i64, f64, f64, f64)>,
    edges: Vec<RawEdge>,
    junctions: Vec<Junction>,
    walls: Vec<(i64, u8, Vec<Pt>, i8)>,
    buildings: Vec<RawBuilding>,
    water: Vec<(i64, Vec<Vec<Pt>>)>,
    areas: Vec<(i64, u8, Vec<Vec<Pt>>, i8)>,
    paths: Vec<(i64, i8, Vec<Pt>)>,
    portals: Vec<Portal>,
    barriers: Vec<(f64, f64)>,
    trees: Vec<(f64, f64, f64)>,
    crossings: Vec<(f64, f64, i64, u8)>,
    signals: Vec<i64>,
    bans: Vec<(i64, i64, i64)>,
    pois: Vec<Poi>,
    addrs: Vec<Address>,
    furn: Vec<Furn>,
    rails: Vec<(i64, bool, bool, Vec<Pt>, i8)>,
    dens: Option<(f64, Vec<f64>)>,
}
/// Gebäude: (ID, Höhe m, Ringe, eigene Wandzüge)
type RawBuilding = (i64, f64, u8, Vec<Vec<Pt>>, Option<Vec<Vec<Pt>>>);
#[derive(Debug, Clone)]
struct RawEdge {
    gid: i64,
    ia: usize,
    ib: usize,
    cls: u8,
    w: f64,
    name: i64,
    oneway: i8,
    flags: u32,
    mid: Vec<Pt>,
    x: Vec<f64>,
    dtv100: f64,
    fill: f64,
}

fn arr(v: &Value) -> Result<&Vec<Value>> {
    v.as_array().context("Array erwartet")
}
fn num(v: &Value) -> Result<f64> {
    let n = v.as_f64().context("Zahl erwartet")?;
    ensure!(n.is_finite(), "Nicht-endliche Zahl");
    Ok(n)
}
fn int(v: &Value) -> Result<i64> {
    v.as_i64()
        .or_else(|| v.as_f64().map(|f| f as i64))
        .context("Ganzzahl erwartet")
}
fn opt_num(r: &[Value], i: usize) -> Result<f64> {
    r.get(i).map(num).transpose().map(|v| v.unwrap_or(0.))
}
fn row(v: &Value, n: usize) -> Result<&Vec<Value>> {
    let r = arr(v)?;
    ensure!(r.len() >= n, "Kartenzeile zu kurz: {} < {n}", r.len());
    Ok(r)
}
/// Delta-Koordinaten (erstes Paar absolut) → Punkte.
pub fn undelta(v: &Value) -> Result<Vec<Pt>> {
    let flat = arr(v)?;
    ensure!(flat.len() % 2 == 0, "Ungerade Koordinatenliste");
    let mut out = Vec::with_capacity(flat.len() / 2);
    let (mut x, mut y) = (0., 0.);
    for (i, p) in flat.chunks(2).enumerate() {
        let (dx, dy) = (num(&p[0])?, num(&p[1])?);
        if i == 0 {
            x = dx;
            y = dy;
        } else {
            x += dx;
            y += dy;
        }
        out.push((x, y));
    }
    Ok(out)
}
fn flagged_rings(v: &Value) -> Result<Vec<Vec<Pt>>> {
    arr(v)?.iter().map(|r| undelta(&row(r, 2)?[1])).collect()
}
pub fn unpack_lvl(v: u32) -> i8 {
    ((v as i8 & 7) << 5) >> 5
}

impl TileData {
    pub fn read(root: &Path, key: &str) -> Result<Self> {
        let path = root.join("tiles").join(format!("{key}.json"));
        let bytes = std::fs::read(&path).with_context(|| format!("{} lesen", path.display()))?;
        Self::decode(&bytes).with_context(|| format!("Kachel {key}"))
    }
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let j: Value = serde_json::from_slice(bytes)?;
        ensure!(
            j["v"].as_u64() == Some(3),
            "Kachelformat {} statt 3",
            j["v"]
        );
        let t = row(&j["t"], 2)?;
        let key = (int(&t[0])? as u32, int(&t[1])? as u32);
        let empty = Value::Array(vec![]);
        let list = |k: &str| -> Result<&Vec<Value>> {
            let v = j.get(k).unwrap_or(&empty);
            if v.is_null() { arr(&empty) } else { arr(v) }
        };
        let names = list("names")?
            .iter()
            .map(|v| v.as_str().unwrap_or("").to_owned())
            .collect();
        let vid = arr(&j["vertices"]["id"])?;
        let vxy = undelta(&j["vertices"]["xy"])?;
        let vtrim = arr(&j["vertices"]["trim"])?;
        ensure!(
            vid.len() == vxy.len() && vid.len() == vtrim.len(),
            "Knotenlisten unterschiedlicher Länge"
        );
        let mut vertices = Vec::with_capacity(vid.len());
        for i in 0..vid.len() {
            vertices.push((int(&vid[i])?, vxy[i].0, vxy[i].1, num(&vtrim[i])?));
        }
        let mut edges = Vec::new();
        for e in list("edges")? {
            let r = row(e, 10)?;
            let (ia, ib) = (int(&r[1])? as usize, int(&r[2])? as usize);
            ensure!(
                ia < vertices.len() && ib < vertices.len(),
                "Ungültiger Straßenknoten"
            );
            edges.push(RawEdge {
                gid: int(&r[0])?,
                ia,
                ib,
                cls: int(&r[3])? as u8,
                w: num(&r[4])?,
                name: int(&r[5])?,
                oneway: int(&r[6])? as i8,
                flags: int(&r[7])? as u32,
                mid: undelta(&r[8])?,
                x: arr(&r[9])?.iter().map(num).collect::<Result<_>>()?,
                dtv100: opt_num(r, 10)?,
                fill: opt_num(r, 11)?,
            });
        }
        let mut junctions = Vec::new();
        for v in list("junctions")? {
            let r = row(v, 5)?;
            let fl = int(&r[4])? as u32;
            junctions.push(Junction {
                node: int(&r[0])?,
                x: num(&r[1])?,
                y: num(&r[2])?,
                r: num(&r[3])?,
                cobble: fl & 2 != 0,
                hi: unpack_lvl(fl >> 2),
                lo: unpack_lvl(fl >> 5),
                cls: r.get(5).map(int).transpose()?.unwrap_or(8) as u8,
            });
        }
        let mut walls = Vec::new();
        for v in list("walls")? {
            let r = row(v, 3)?;
            walls.push((
                int(&r[0])?,
                int(&r[1])? as u8,
                undelta(&r[2])?,
                opt_num(r, 3)? as i8,
            ));
        }
        let mut buildings = Vec::new();
        for v in list("buildings")? {
            let r = row(v, 4)?;
            let rings = arr(&r[3])?
                .iter()
                .map(undelta)
                .collect::<Result<Vec<_>>>()?;
            ensure!(
                !rings.is_empty() && rings[0].len() >= 3,
                "Gebäude ohne Grundriss"
            );
            let walls = match r.get(4) {
                Some(w) if w.is_array() => {
                    Some(arr(w)?.iter().map(undelta).collect::<Result<Vec<_>>>()?)
                }
                _ => None,
            };
            buildings.push((
                int(&r[0])?,
                num(&r[1])? / 10.,
                opt_num(r, 2)? as u8,
                rings,
                walls,
            ));
        }
        let water = list("water")?
            .iter()
            .map(|v| {
                let r = row(v, 2)?;
                Ok((int(&r[0])?, flagged_rings(&r[1])?))
            })
            .collect::<Result<_>>()?;
        let areas = list("areas")?
            .iter()
            .map(|v| {
                let r = row(v, 3)?;
                Ok((
                    int(&r[0])?,
                    int(&r[1])? as u8,
                    flagged_rings(&r[2])?,
                    opt_num(r, 3)? as i8,
                ))
            })
            .collect::<Result<_>>()?;
        let paths = list("paths")?
            .iter()
            .map(|v| {
                let r = row(v, 3)?;
                Ok((
                    int(&r[0])?,
                    unpack_lvl(int(&r[1])? as u32 >> 2),
                    undelta(&r[2])?,
                ))
            })
            .collect::<Result<_>>()?;
        let portals = list("portals")?
            .iter()
            .map(|v| {
                let r = row(v, 5)?;
                Ok(Portal {
                    x: num(&r[0])?,
                    y: num(&r[1])?,
                    r: num(&r[2])?,
                    lo: int(&r[3])? as i8,
                    hi: int(&r[4])? as i8,
                })
            })
            .collect::<Result<_>>()?;
        let flat = |k: &str| -> Result<Vec<f64>> { list(k)?.iter().map(num).collect() };
        let mut barriers = Vec::new();
        let b = flat("barriers")?;
        ensure!(b.len() % 6 == 0, "Pollerliste unvollständig");
        for c in b.chunks(6) {
            let (x, y, qx, qy, n) = (c[0], c[1], c[3], c[4], c[5] as i64);
            for k in 0..n {
                let o = (k as f64 - (n - 1) as f64 / 2.) * 1.8 * 10.;
                barriers.push((x + qx / 1000. * o, y + qy / 1000. * o));
            }
        }
        let posts = flat("posts")?;
        ensure!(posts.len() % 3 == 0, "Pfostenliste unvollständig");
        barriers.extend(posts.chunks(3).map(|c| (c[0], c[1])));
        let c = flat("crossings")?;
        ensure!(c.len() % 4 == 0, "Querungsliste unvollständig");
        let crossings = c
            .chunks(4)
            .map(|c| (c[0], c[1], c[2] as i64, c[3] as u8))
            .collect();
        let signals = list("signals")?.iter().map(int).collect::<Result<_>>()?;
        let tb = flat("turnBans")?;
        let bans = tb
            .chunks(3)
            .filter(|c| c.len() == 3)
            .map(|c| (c[0] as i64, c[1] as i64, c[2] as i64))
            .collect();
        let tr = &j["trees"];
        let mut trees = Vec::new();
        if tr.is_object() {
            let xy = undelta(&tr["xy"])?;
            let rr = tr.get("r").map(arr).transpose()?;
            for (i, (x, y)) in xy.into_iter().enumerate() {
                let r = rr
                    .and_then(|r| r.get(i))
                    .and_then(Value::as_f64)
                    .unwrap_or(0.);
                trees.push((
                    x,
                    y,
                    if r > 0. {
                        r / 100.
                    } else {
                        berlin_map_loader::citycodes::TREE_TRUNK_M as f64
                    },
                ));
            }
        }
        let name_of = |v: &Value, names: &Vec<String>| {
            v.as_i64()
                .and_then(|i| names.get(i as usize))
                .cloned()
                .unwrap_or_default()
        };
        let names: Vec<String> = names;
        let mut pois = Vec::new();
        for r in list("pois")? {
            let r = row(r, 3)?;
            let cat = POI_CATS
                .get(int(&r[2])? as usize)
                .copied()
                .unwrap_or("shop");
            pois.push(Poi {
                x: num(&r[0])?,
                y: num(&r[1])?,
                cat,
                name: r.get(3).map(|v| name_of(v, &names)).unwrap_or_default(),
                kind: r.get(4).map(|v| name_of(v, &names)).unwrap_or_default(),
            });
        }
        let mut addrs = Vec::new();
        let a = &j["addresses"];
        if a.is_object() {
            let xy = undelta(&a["xy"])?;
            let st = arr(&a["street"])?;
            let nr = arr(&a["nr"])?;
            for (i, (x, y)) in xy.into_iter().enumerate() {
                addrs.push(Address {
                    x,
                    y,
                    street: st.get(i).map(|v| name_of(v, &names)).unwrap_or_default(),
                    nr: nr
                        .get(i)
                        .map(|v| {
                            v.as_str()
                                .map(str::to_owned)
                                .unwrap_or_else(|| v.to_string())
                        })
                        .unwrap_or_default(),
                });
            }
        }
        let mut furn = Vec::new();
        for r in list("furn")? {
            let r = row(r, 3)?;
            furn.push(Furn {
                x: num(&r[0])?,
                y: num(&r[1])?,
                kind: int(&r[2])? as u8,
            });
        }
        let mut rails = Vec::new();
        for v in list("rails")? {
            let r = row(v, 4)?;
            rails.push((
                int(&r[0])?,
                int(&r[1])? != 0,
                int(&r[2])? != 0,
                undelta(&r[3])?,
                opt_num(r, 4)? as i8,
            ));
        }
        let dens = j["dens"].as_object().and_then(|d| {
            Some((
                d.get("cell")?.as_f64()?,
                d.get("vals")?
                    .as_array()?
                    .iter()
                    .filter_map(Value::as_f64)
                    .collect(),
            ))
        });
        Ok(Self {
            pois,
            addrs,
            furn,
            rails,
            dens,
            key,
            names,
            vertices,
            edges,
            junctions,
            walls,
            buildings,
            water,
            areas,
            paths,
            portals,
            barriers,
            trees,
            crossings,
            signals,
            bans,
        })
    }
}

/// Lieferant von Kacheln: synchron (Tests, Werkzeuge) oder aus einem Hintergrund-Thread (Spiel).
pub trait TileSource: Send {
    /// Kachel anfordern; synchrone Quellen liefern sie beim nächsten `poll`.
    fn request(&mut self, key: &str);
    /// Fertige Kacheln (Fehler mit Text).
    fn poll(&mut self) -> Vec<(String, Result<TileData, String>)>;
}
/// Liest Kacheln sofort von der Platte.
pub struct DiskSource {
    root: PathBuf,
    ready: Vec<(String, Result<TileData, String>)>,
}
impl DiskSource {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            ready: Vec::new(),
        }
    }
}
impl TileSource for DiskSource {
    fn request(&mut self, key: &str) {
        let r = TileData::read(&self.root, key).map_err(|e| format!("{e:#}"));
        self.ready.push((key.to_owned(), r));
    }
    fn poll(&mut self) -> Vec<(String, Result<TileData, String>)> {
        std::mem::take(&mut self.ready)
    }
}
/// Liest und dekodiert Kacheln in einem eigenen Thread; die Simulation baut sie nur noch ein.
pub struct ThreadedSource {
    tx: Option<mpsc::Sender<String>>,
    rx: mpsc::Receiver<(String, Result<TileData, String>)>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl ThreadedSource {
    pub fn new(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        let (tx, jobs) = mpsc::channel::<String>();
        let (done, rx) = mpsc::channel();
        let worker = std::thread::Builder::new()
            .name("sim-kacheln".into())
            .spawn(move || {
                while let Ok(key) = jobs.recv() {
                    let r = TileData::read(&root, &key).map_err(|e| format!("{e:#}"));
                    if done.send((key, r)).is_err() {
                        break;
                    }
                }
            })?;
        Ok(Self {
            tx: Some(tx),
            rx,
            worker: Some(worker),
        })
    }
}
impl TileSource for ThreadedSource {
    fn request(&mut self, key: &str) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(key.to_owned());
        }
    }
    fn poll(&mut self) -> Vec<(String, Result<TileData, String>)> {
        self.rx.try_iter().collect()
    }
}
impl Drop for ThreadedSource {
    fn drop(&mut self) {
        self.tx.take();
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum LoadState {
    Loading,
    Ready,
    Failed {
        fails: u32,
        retry_at: f64,
        error: String,
    },
}
/// Wiederholpausen nach Ladefehlern (Sekunden der Simulationsuhr des Stadtmodells).
pub const RETRY_S: [f64; 4] = [0.5, 1., 2., 3.];

pub struct City {
    pub scale: f64,
    /// Kopf aus index.json (Projektion für Geokoordinaten)
    pub meta: Option<berlin_map_loader::format::Meta>,
    /// Bar-Auslastungs-Feed (nightlife.rs)
    pub bars: Option<crate::nightlife::Bars>,
    pub tile: f64,
    pub width: f64,
    pub height: f64,
    pub places: Places,
    pub hospitals: Vec<Hospital>,
    pub border: Vec<Pt>,
    border_bounds: Rect,
    pub nodes: HashMap<i64, Node>,
    pub edges: HashMap<i64, Edge>,
    pub signals: HashSet<i64>,
    pub turn_bans: HashSet<(i64, i64, i64)>,
    pub solids: Layer<Solid>,
    pub edge_segs: Layer<EdgeSeg>,
    pub polys: Layer<Poly>,
    pub portals: Layer<Portal>,
    pub paths: Layer<LevelPath>,
    pub pois: Layer<Poi>,
    pub addresses: Layer<Address>,
    pub furn: Layer<Furn>,
    /// Gleismittellinien (oberirdisch; für Tunnel/Zugsichtbarkeit)
    pub rails: Layer<RailLine>,
    pub dens: HashMap<String, Dens>,
    /// Kieze (Punkt + Name) und Ortsteile (Ringe) aus dem Index
    pub kieze: Vec<Named>,
    pub districts: Vec<Named>,
    /// Bezirke (Ringe) aus dem Index
    pub bezirke: Vec<Named>,
    /// Kanten, die seit dem letzten Abholen dazugekommen/weggefallen sind (Spurgraph)
    pub edge_events: Vec<EdgeEvent>,
    /// steigt mit jeder Änderung (Kachel ein-/ausgebaut)
    pub generation: u64,
    available: BTreeSet<String>,
    tiles: BTreeMap<String, LoadState>,
    installed: HashMap<String, TileState>,
    reg: HashMap<String, Entry>,
    sig_count: HashMap<i64, u32>,
    ban_count: HashMap<(i64, i64, i64), u32>,
    focuses: BTreeMap<String, (f64, f64)>,
    pinned: HashSet<String>,
    source: Box<dyn TileSource>,
    clock: f64,
    pub errors: Vec<String>,
}

fn place(v: &Value) -> Place {
    Place {
        x: v["x"].as_f64().unwrap_or(0.),
        y: v["y"].as_f64().unwrap_or(0.),
        angle: v["angle"].as_f64().unwrap_or(0.),
    }
}

impl City {
    /// Öffnet die Karte (`index.json`) mit einer Kachelquelle.
    pub fn open(root: &Path, source: Box<dyn TileSource>) -> Result<Self> {
        let bytes = std::fs::read(root.join("index.json"))
            .with_context(|| format!("{}/index.json lesen", root.display()))?;
        let j: Value = serde_json::from_slice(&bytes)?;
        Self::from_index(&j, source)
    }
    pub fn from_index(j: &Value, source: Box<dyn TileSource>) -> Result<Self> {
        let m = &j["meta"];
        ensure!(
            m["version"].as_u64() == Some(3),
            "Kartenformat {} wird nicht unterstützt (erwartet 3)",
            m["version"]
        );
        let p = &j["places"];
        let places = Places {
            giver: place(&p["giver"]),
            player_spawn: place(&p["playerSpawn"]),
            dropoff: place(&p["dropoff"]),
            player_car: place(&p["playerCar"]),
            pickup: place(&p["pickup"]),
            parked: p["parked"]
                .as_array()
                .map(|a| a.iter().map(place).collect())
                .unwrap_or_default(),
            crates: p["crates"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .map(|c| {
                            Rect::new(
                                c["x"].as_f64().unwrap_or(0.),
                                c["y"].as_f64().unwrap_or(0.),
                                c["w"].as_f64().unwrap_or(20.),
                                c["h"].as_f64().unwrap_or(20.),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default(),
            time_limit: p["timeLimit"].as_f64(),
        };
        let hospitals = j["hospitals"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|h| {
                        Some(Hospital {
                            x: h[0].as_f64()?,
                            y: h[1].as_f64()?,
                            name: h[2].as_str().unwrap_or("").to_owned(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let border = match j["border"].as_array().and_then(|b| b.first()) {
            Some(r) => undelta(r)?,
            None => Vec::new(),
        };
        let border_bounds = bounds_of(&border);
        let mut city = Self {
            meta: serde_json::from_value(m.clone()).ok(),
            bars: None,
            scale: num(&m["scale"])?,
            tile: num(&m["tile"])?,
            width: num(&m["width"])?,
            height: num(&m["height"])?,
            places,
            hospitals,
            border,
            border_bounds,
            nodes: HashMap::new(),
            edges: HashMap::new(),
            signals: HashSet::new(),
            turn_bans: HashSet::new(),
            solids: Layer::new(128.),
            edge_segs: Layer::new(320.),
            polys: Layer::new(320.),
            portals: Layer::new(256.),
            paths: Layer::new(320.),
            pois: Layer::new(400.),
            addresses: Layer::new(400.),
            furn: Layer::new(256.),
            rails: Layer::new(256.),
            dens: HashMap::new(),
            kieze: j["kieze"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|k| {
                    Some(Named {
                        name: k["n"].as_str()?.to_owned(),
                        x: k["x"].as_f64()?,
                        y: k["y"].as_f64()?,
                        rings: Vec::new(),
                    })
                })
                .collect(),
            districts: j["districts"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|d| {
                    let rings: Vec<Vec<Pt>> = d["r"]
                        .as_array()?
                        .iter()
                        .filter_map(|r| undelta(r).ok())
                        .collect();
                    let b = bounds_of(rings.first()?);
                    Some(Named {
                        name: d["n"].as_str()?.to_owned(),
                        x: b.x + b.w / 2.,
                        y: b.y + b.h / 2.,
                        rings,
                    })
                })
                .collect(),
            bezirke: j["bezirke"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|d| {
                    let rings: Vec<Vec<Pt>> = d["r"]
                        .as_array()?
                        .iter()
                        .filter_map(|r| undelta(r).ok())
                        .collect();
                    let b = bounds_of(rings.first()?);
                    Some(Named {
                        name: d["n"].as_str()?.to_owned(),
                        x: b.x + b.w / 2.,
                        y: b.y + b.h / 2.,
                        rings,
                    })
                })
                .collect(),
            edge_events: Vec::new(),
            generation: 0,
            available: j["tiles"]
                .as_array()
                .context("Kachelliste fehlt")?
                .iter()
                .filter_map(|t| t.as_str().map(str::to_owned))
                .collect(),
            tiles: BTreeMap::new(),
            installed: HashMap::new(),
            reg: HashMap::new(),
            sig_count: HashMap::new(),
            ban_count: HashMap::new(),
            focuses: BTreeMap::new(),
            pinned: HashSet::new(),
            source,
            clock: 0.,
            errors: Vec::new(),
        };
        // Missionskisten liegen immer da
        for c in city.places.crates.clone() {
            city.solids.add(Solid::Rect(c), &c);
        }
        Ok(city)
    }

    pub fn tiles_around(&self, x: f64, y: f64, r: f64) -> Vec<String> {
        let t = self.tile;
        let mut out = Vec::new();
        let (x0, x1) = (
            ((x - r) / t).floor().max(0.) as i64,
            ((x + r) / t).floor() as i64,
        );
        let (y0, y1) = (
            ((y - r) / t).floor().max(0.) as i64,
            ((y + r) / t).floor() as i64,
        );
        for tx in x0..=x1 {
            for ty in y0..=y1 {
                let k = format!("{tx}_{ty}");
                if self.available.contains(&k) {
                    out.push(k);
                }
            }
        }
        // die nächsten zuerst
        out.sort_by(|a, b| {
            let d = |k: &String| {
                let (tx, ty) = k
                    .split_once('_')
                    .map(|(a, b)| {
                        (
                            a.parse::<f64>().unwrap_or(0.),
                            b.parse::<f64>().unwrap_or(0.),
                        )
                    })
                    .unwrap_or_default();
                ((tx + 0.5) * t - x).hypot((ty + 0.5) * t - y)
            };
            d(a).total_cmp(&d(b))
        });
        out
    }

    /// Fokus (Kamera, Teleport-Ziel): fordert ringsum Kacheln an, baut eingetroffene ein und gibt Fernes frei.
    /// `true`, wenn alles bis `STREAM_READY` geladen ist.
    pub fn focus(&mut self, key: &str, x: f64, y: f64) -> bool {
        self.focuses.insert(key.to_owned(), (x, y));
        for k in self.tiles_around(x, y, STREAM_LOAD) {
            self.request(&k);
        }
        self.pump();
        self.evict();
        self.ready(x, y, STREAM_READY)
    }
    pub fn release(&mut self, key: &str) {
        self.focuses.remove(key);
    }
    /// Fortschreiben der Uhr für Wiederholversuche.
    pub fn tick(&mut self, dt: f64) {
        self.clock += dt;
    }
    fn request(&mut self, key: &str) {
        match self.tiles.get(key) {
            Some(&LoadState::Failed { retry_at, .. }) if self.clock >= retry_at => {}
            Some(_) => return,
            None => {}
        }
        self.tiles.insert(key.to_owned(), LoadState::Loading);
        self.source.request(key);
    }
    /// Eingetroffene Kacheln einbauen.
    pub fn pump(&mut self) -> usize {
        let mut n = 0;
        for (key, res) in self.source.poll() {
            if self.tiles.get(&key) != Some(&LoadState::Loading) {
                continue; // inzwischen verworfen
            }
            match res {
                Ok(data) => {
                    self.install(&key, data);
                    n += 1;
                }
                Err(error) => {
                    let fails = 1;
                    if !self.errors.iter().any(|e| e.contains(&key)) {
                        self.errors.push(format!("Kachel {key}: {error}"));
                    }
                    self.tiles.insert(
                        key,
                        LoadState::Failed {
                            fails,
                            retry_at: self.clock + RETRY_S[0],
                            error,
                        },
                    );
                }
            }
        }
        n
    }
    pub fn ready(&self, x: f64, y: f64, r: f64) -> bool {
        self.tiles_around(x, y, r)
            .iter()
            .all(|k| self.tiles.get(k) == Some(&LoadState::Ready))
    }
    pub fn is_installed(&self, key: &str) -> bool {
        self.installed.contains_key(key)
    }
    pub fn installed_count(&self) -> usize {
        self.installed.len()
    }
    /// Alle Kacheln eines Bereichs sofort laden (nur mit synchroner Quelle sinnvoll) und festhalten.
    pub fn load_area(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, pin: bool) {
        let t = self.tile;
        for tx in (x0 / t).floor().max(0.) as i64..=(x1 / t).floor() as i64 {
            for ty in (y0 / t).floor().max(0.) as i64..=(y1 / t).floor() as i64 {
                let k = format!("{tx}_{ty}");
                if !self.available.contains(&k) {
                    continue;
                }
                if pin {
                    self.pinned.insert(k.clone());
                }
                self.request(&k);
            }
        }
        self.pump();
    }
    fn evict(&mut self) {
        let mut keep: HashSet<String> = self.pinned.clone();
        for &(x, y) in self.focuses.values() {
            keep.extend(self.tiles_around(x, y, STREAM_KEEP));
        }
        let drop: Vec<String> = self
            .tiles
            .keys()
            .filter(|k| !keep.contains(k.as_str()))
            .cloned()
            .collect();
        for k in drop {
            self.unload(&k);
        }
    }
    pub fn unload(&mut self, key: &str) {
        self.pinned.remove(key);
        self.tiles.remove(key);
        let Some(t) = self.installed.remove(key) else {
            return;
        };
        for e in t.local {
            self.destroy(e);
        }
        for k in t.shared.iter().rev() {
            let gone = match self.reg.get_mut(k) {
                Some(e) => {
                    e.refs -= 1;
                    e.refs == 0
                }
                None => false,
            };
            if gone {
                let e = self.reg.remove(k).expect("Eintrag fehlt");
                self.destroy(e);
            }
        }
        for v in t.signals {
            dec(&mut self.sig_count, &mut self.signals, v);
        }
        for b in t.bans {
            dec(&mut self.ban_count, &mut self.turn_bans, b);
        }
        self.generation += 1;
    }

    fn destroy(&mut self, e: Entry) {
        for o in e.owned {
            match o {
                Owned::Solid(h, k) => self.solids.drop_item(h, &k),
                Owned::Seg(h, k) => self.edge_segs.drop_item(h, &k),
                Owned::Poly(h, k) => self.polys.drop_item(h, &k),
                Owned::Portal(h, k) => self.portals.drop_item(h, &k),
                Owned::Path(h, k) => self.paths.drop_item(h, &k),
                Owned::Poi(h, k) => self.pois.drop_item(h, &k),
                Owned::Addr(h, k) => self.addresses.drop_item(h, &k),
                Owned::Furn(h, k) => self.furn.drop_item(h, &k),
                Owned::Rail(h, k) => self.rails.drop_item(h, &k),
                Owned::Dens(key) => {
                    self.dens.remove(&key);
                }
                Owned::Crossing(edge) => {
                    if let Some(e) = self.edges.get_mut(&edge) {
                        e.crossings.pop();
                    }
                }
                Owned::Edge(id) => {
                    self.edge_events.push(EdgeEvent::Removed(id));
                    if let Some(e) = self.edges.remove(&id) {
                        for n in [e.a, e.b] {
                            let empty = match self.nodes.get_mut(&n) {
                                Some(nd) => {
                                    nd.edges.retain(|&k| k != id);
                                    nd.edges.is_empty()
                                }
                                None => false,
                            };
                            if empty {
                                self.nodes.remove(&n);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Gemeinsames Objekt anlegen (nur beim ersten Halter) und Halter zählen.
    fn acquire(
        &mut self,
        t: &mut TileState,
        key: String,
        create: impl FnOnce(&mut Self) -> Vec<Owned>,
    ) {
        if let Some(e) = self.reg.get_mut(&key) {
            e.refs += 1;
        } else {
            let owned = create(self);
            self.reg.insert(key.clone(), Entry { refs: 1, owned });
        }
        t.shared.push(key);
    }

    fn add_line(
        &mut self,
        pts: &[Pt],
        closed: bool,
        kind: WallKind,
        sub: WallSub,
        lvl: i8,
    ) -> Vec<Owned> {
        let mut out = Vec::new();
        let n = pts.len();
        let segs = if closed { n } else { n.saturating_sub(1) };
        for i in 0..segs {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            if a == b {
                continue;
            }
            let seg = Segment {
                ax: a.0,
                ay: a.1,
                bx: b.0,
                by: b.1,
            };
            let (h, k) = self.solids.add(
                Solid::Wall {
                    seg,
                    kind,
                    sub,
                    lvl,
                },
                &seg.bounds(),
            );
            out.push(Owned::Solid(h, k));
        }
        out
    }

    fn install(&mut self, key: &str, d: TileData) {
        let s = self.scale;
        let mut t = TileState {
            signals: d.signals.clone(),
            bans: d.bans.clone(),
            ..Default::default()
        };
        let names = d.names;
        let nm = |i: i64| {
            if i >= 0 {
                names.get(i as usize).cloned().unwrap_or_default()
            } else {
                String::new()
            }
        };
        for e in d.edges {
            let a = d.vertices[e.ia];
            let b = d.vertices[e.ib];
            let name = nm(e.name);
            self.acquire(&mut t, format!("e{}", e.gid), |city| {
                for v in [a, b] {
                    city.nodes.entry(v.0).or_insert_with(|| Node {
                        id: v.0,
                        x: v.1,
                        y: v.2,
                        edges: Vec::new(),
                        trim: v.3,
                    });
                }
                let mut pts = vec![(a.1, a.2)];
                pts.extend(&e.mid);
                pts.push((b.1, b.2));
                let dm = |v: f64| v / 10. * s;
                let x = if e.x.len() == 2 {
                    vec![
                        if e.oneway == -1 { 0. } else { 1. },
                        if e.oneway == 1 { 0. } else { 1. },
                        0.,
                        0.,
                        0.,
                        0.,
                        0.,
                        0.,
                        0.,
                        0.,
                        e.x[0],
                        e.x[1],
                        0.,
                    ]
                } else {
                    e.x.clone()
                };
                let g = |i: usize| x.get(i).copied().unwrap_or(0.);
                let cs = CrossSection {
                    width: dm(e.w),
                    fwd: g(0) as u32,
                    bwd: g(1) as u32,
                    left: Side {
                        park: g(2) as u8,
                        park_w: dm(g(3)),
                        orient: g(4) as u8,
                        cycle: dm(g(8)),
                        track: dm(g(13)),
                    },
                    right: Side {
                        park: g(5) as u8,
                        park_w: dm(g(6)),
                        orient: g(7) as u8,
                        cycle: dm(g(9)),
                        track: dm(g(14)),
                    },
                    maxspeed: g(10),
                    surface: g(11) as u8,
                    lit: g(12) as u32 & 1 != 0,
                    gaslight: g(12) as u32 & 2 != 0,
                    bus_contra: g(12) as u32 & 4 != 0,
                };
                let edge = Edge {
                    id: e.gid,
                    a: a.0,
                    b: b.0,
                    cls: e.cls,
                    w: dm(e.w),
                    cs,
                    name,
                    oneway: e.oneway,
                    bridge: e.flags & 1 != 0,
                    inside: e.flags & 2 != 0,
                    lvl: unpack_lvl(e.flags >> 4),
                    blocked: e.flags & 4 != 0,
                    passage: e.flags & 8 != 0,
                    len: polyline_length(&pts),
                    dtv: e.dtv100.abs() * 100.,
                    fill: dm(e.fill),
                    crossings: Vec::new(),
                    pts,
                };
                let mut owned = Vec::new();
                let rr = edge.w / 2. + s;
                for i in 0..edge.pts.len() - 1 {
                    let ((ax, ay), (bx, by)) = (edge.pts[i], edge.pts[i + 1]);
                    let seg = EdgeSeg {
                        edge: Some(edge.id),
                        junction: None,
                        i,
                        ax,
                        ay,
                        bx,
                        by,
                    };
                    let bb = Rect::new(
                        ax.min(bx) - rr,
                        ay.min(by) - rr,
                        (bx - ax).abs() + 2. * rr,
                        (by - ay).abs() + 2. * rr,
                    );
                    let (h, k) = city.edge_segs.add(seg, &bb);
                    owned.push(Owned::Seg(h, k));
                }
                for n in [edge.a, edge.b] {
                    let nd = city.nodes.get_mut(&n).expect("Knoten");
                    if let Err(i) = nd.edges.binary_search(&edge.id) {
                        nd.edges.insert(i, edge.id);
                    }
                }
                city.edge_events.push(EdgeEvent::Added(edge.id));
                city.edges.insert(edge.id, edge);
                owned.push(Owned::Edge(e.gid));
                owned
            });
        }
        for j in d.junctions {
            self.acquire(&mut t, format!("j{}", j.node), |city| {
                let seg = EdgeSeg {
                    edge: None,
                    junction: Some(j),
                    i: 0,
                    ax: j.x,
                    ay: j.y,
                    bx: j.x,
                    by: j.y,
                };
                let (h, k) = city.edge_segs.add(seg, &Rect::around(j.x, j.y, j.r));
                vec![Owned::Seg(h, k)]
            });
        }
        for (gid, lvl, pts) in d.paths {
            if lvl == 0 {
                continue; // Wege am Boden zählen für die Ebenen nicht
            }
            self.acquire(&mut t, format!("g{gid}"), |city| {
                let b = bounds_of(&pts);
                let (h, k) = city.paths.add(LevelPath { pts, lvl }, &b);
                vec![Owned::Path(h, k)]
            });
        }
        for (gid, bridge, subway, pts, lvl) in d.rails {
            self.acquire(&mut t, format!("r{gid}"), |city| {
                let b = bounds_of(&pts);
                let line = RailLine {
                    pts,
                    bridge,
                    subway,
                    lvl,
                };
                let (h, k) = city.rails.add(line, &b);
                vec![Owned::Rail(h, k)]
            });
        }
        for (gid, kind, pts, lvl) in d.walls {
            self.acquire(&mut t, format!("g{gid}"), |city| {
                use berlin_map_loader::citycodes::wall_kind as wk;
                let (k, sub) = match kind {
                    wk::BORDER => (WallKind::Border, WallSub::Other),
                    wk::QUAY => (WallKind::Wall, WallSub::Quay),
                    wk::RAIL => (WallKind::Wall, WallSub::Rail),
                    wk::RAILING => (WallKind::Wall, WallSub::Railing),
                    wk::FENCE => (WallKind::Wall, WallSub::Fence),
                    _ => (WallKind::Wall, WallSub::Other),
                };
                city.add_line(&pts, false, k, sub, lvl)
            });
        }
        for (gid, _height, bkind, rings, walls) in d.buildings {
            self.acquire(&mut t, format!("g{gid}"), |city| {
                let b = bounds_of(&rings[0]);
                let mut owned = Vec::new();
                match &walls {
                    Some(ws) => {
                        for w in ws {
                            owned.extend(city.add_line(
                                w,
                                false,
                                WallKind::Building,
                                WallSub::Other,
                                0,
                            ));
                        }
                    }
                    None => {
                        for r in &rings {
                            owned.extend(city.add_line(
                                r,
                                true,
                                WallKind::Building,
                                WallSub::Other,
                                0,
                            ));
                        }
                    }
                }
                let (h, k) = city.polys.add(
                    Poly {
                        kind: PolyKind::Building,
                        rings,
                        id: gid,
                        bkind,
                    },
                    &b,
                );
                owned.push(Owned::Poly(h, k));
                owned
            });
        }
        let area =
            |city: &mut Self, t: &mut TileState, gid: i64, kind: PolyKind, rings: Vec<Vec<Pt>>| {
                let create = |city: &mut Self| {
                    let b = bounds_of(&rings.concat());
                    let (h, k) = city.polys.add(
                        Poly {
                            kind,
                            rings,
                            id: gid,
                            bkind: 0,
                        },
                        &b,
                    );
                    vec![Owned::Poly(h, k)]
                };
                if gid >= 0 {
                    city.acquire(t, format!("g{gid}"), create);
                } else {
                    let owned = create(city);
                    t.local.push(Entry { refs: 1, owned });
                }
            };
        for (gid, rings) in d.water {
            area(self, &mut t, gid, PolyKind::Water, rings);
        }
        for (gid, kind, rings, lvl) in d.areas {
            area(self, &mut t, gid, PolyKind::Area { kind, lvl }, rings);
        }
        for p in d.portals {
            let k = format!("p{},{}", p.x, p.y);
            self.acquire(&mut t, k, |city| {
                let (h, k) = city.portals.add(p, &Rect::around(p.x, p.y, p.r));
                vec![Owned::Portal(h, k)]
            });
        }
        for (x, y) in d.barriers {
            let r = 0.15 * s;
            if self.post_on_road(x, y, r) {
                continue; // Sicherheitsnetz: kein Poller auf einer befahrbaren Fahrbahn
            }
            let (h, k) = self.solids.add(
                Solid::Circle {
                    x,
                    y,
                    r,
                    kind: CircleKind::Barrier {
                        key: (x.round() as i64, y.round() as i64),
                    },
                },
                &Rect::around(x, y, r),
            );
            t.local.push(Entry {
                refs: 1,
                owned: vec![Owned::Solid(h, k)],
            });
        }
        for (x, y, r) in d.trees {
            let r = r * s;
            if self.tree_on_road(x, y, r) {
                continue;
            }
            let (h, k) = self.solids.add(
                Solid::Circle {
                    x,
                    y,
                    r,
                    kind: CircleKind::Tree,
                },
                &Rect::around(x, y, r),
            );
            t.local.push(Entry {
                refs: 1,
                owned: vec![Owned::Solid(h, k)],
            });
        }
        for (x, y, eid, kind) in d.crossings {
            let Some(e) = self.edges.get_mut(&eid) else {
                continue;
            };
            let s = project_on_polyline(&e.pts, x, y).map(|p| p.s).unwrap_or(0.);
            e.crossings.push(Crossing { x, y, s, kind });
            t.local.push(Entry {
                refs: 1,
                owned: vec![Owned::Crossing(eid)],
            });
        }
        for q in d.pois {
            let (h, k) = self.pois.add(q.clone(), &Rect::new(q.x, q.y, 0., 0.));
            t.local.push(Entry {
                refs: 1,
                owned: vec![Owned::Poi(h, k)],
            });
        }
        for a in d.addrs {
            let (h, k) = self.addresses.add(a.clone(), &Rect::new(a.x, a.y, 0., 0.));
            t.local.push(Entry {
                refs: 1,
                owned: vec![Owned::Addr(h, k)],
            });
        }
        for f in d.furn {
            let (h, k) = self.furn.add(f, &Rect::new(f.x, f.y, 0., 0.));
            t.local.push(Entry {
                refs: 1,
                owned: vec![Owned::Furn(h, k)],
            });
        }
        if let Some((cell, vals)) = d.dens {
            let tl = self.tile;
            self.dens.insert(
                key.to_owned(),
                Dens {
                    x0: d.key.0 as f64 * tl,
                    y0: d.key.1 as f64 * tl,
                    cell,
                    per: (tl / cell).round().max(1.) as usize,
                    vals,
                },
            );
            t.local.push(Entry {
                refs: 1,
                owned: vec![Owned::Dens(key.to_owned())],
            });
        }
        for &v in &t.signals {
            *self.sig_count.entry(v).or_insert(0) += 1;
            self.signals.insert(v);
        }
        for &b in &t.bans {
            *self.ban_count.entry(b).or_insert(0) += 1;
            self.turn_bans.insert(b);
        }
        self.installed.insert(key.to_owned(), t);
        self.tiles.insert(key.to_owned(), LoadState::Ready);
        self.generation += 1;
    }

    fn post_on_road(&mut self, x: f64, y: f64, r: f64) -> bool {
        for h in self.edge_segs.query(&Rect::around(x, y, r)) {
            let s = *self.edge_segs.get(h);
            let half = match (s.edge.and_then(|id| self.edges.get(&id)), s.junction) {
                (Some(e), _)
                    if !e.blocked
                        && !e.passage
                        && e.cls <= berlin_map_loader::citycodes::TRAFFIC_MAX_CLASS =>
                {
                    e.w / 2.
                }
                (None, Some(j)) => j.r,
                _ => continue,
            };
            if seg_dist2(x, y, s.ax, s.ay, s.bx, s.by) < (half + r) * (half + r) {
                return true;
            }
        }
        false
    }
    /// Steht ein Kreis (Stamm, Mast) ganz oder teilweise auf einer Fahrbahn bis Klasse 9?
    pub fn tree_on_road(&mut self, x: f64, y: f64, r: f64) -> bool {
        for h in self.edge_segs.query(&Rect::around(x, y, r)) {
            let s = *self.edge_segs.get(h);
            let half = match (s.edge.and_then(|id| self.edges.get(&id)), s.junction) {
                (Some(e), _) if e.cls <= berlin_map_loader::citycodes::TREE_FREE_MAX_CLASS => {
                    e.w / 2.
                }
                (None, Some(j)) if j.cls <= berlin_map_loader::citycodes::TREE_FREE_MAX_CLASS => {
                    j.r
                }
                _ => continue,
            };
            if seg_dist2(x, y, s.ax, s.ay, s.bx, s.by) < (half + r) * (half + r) {
                return true;
            }
        }
        false
    }

    // --- Abfragen ----------------------------------------------------------------------------

    pub fn in_building(&mut self, x: f64, y: f64) -> Option<i64> {
        for h in self.polys.query(&Rect::new(x, y, 0., 0.)) {
            let p = self.polys.get(h);
            if p.kind == PolyKind::Building && point_in_rings(x, y, &p.rings) {
                return Some(p.id);
            }
        }
        None
    }

    fn on_level(seg: &EdgeSeg, e: Option<&Edge>, lvl: i8) -> bool {
        match (e, seg.junction) {
            (_, Some(j)) => j.lo <= lvl && lvl <= j.hi,
            (Some(e), _) => e.lvl == lvl,
            _ => false,
        }
    }
    /// Fahrbahn (Kante; `None` für eine Kreuzungsscheibe) unter (x, y): (Kanten-ID oder None, Belag Pflaster?)
    pub fn on_road(
        &mut self,
        x: f64,
        y: f64,
        margin: f64,
        lvl: Option<i8>,
    ) -> Option<(Option<i64>, bool)> {
        for h in self.edge_segs.query(&Rect::new(x, y, 0., 0.)) {
            let s = *self.edge_segs.get(h);
            let e = s.edge.and_then(|id| self.edges.get(&id));
            if let Some(l) = lvl
                && !Self::on_level(&s, e, l)
            {
                continue;
            }
            let (half, cobble) = match (e, s.junction) {
                (Some(e), _) => (
                    e.w / 2.,
                    e.cs.surface == berlin_map_loader::citycodes::surface::COBBLE,
                ),
                (None, Some(j)) => (j.r, j.cobble),
                _ => continue,
            };
            let r = half + margin;
            if seg_dist2(x, y, s.ax, s.ay, s.bx, s.by) <= r * r {
                return Some((s.edge, cobble));
            }
        }
        None
    }

    /// Untergrund an (x, y). `lvl` = Ebene dessen, der dort steht (Brücke: nur die Brücke zählt).
    pub fn surface_at(&mut self, x: f64, y: f64, lvl: Option<i8>) -> Ground {
        if x < 0. || y < 0. || x > self.width || y > self.height {
            return Ground::Building;
        }
        if let Some((_, cobble)) = self.on_road(x, y, 0., lvl) {
            return if cobble { Ground::Cobble } else { Ground::Road };
        }
        if lvl.is_some_and(|l| l >= 1) {
            return Ground::Plaza;
        }
        use berlin_map_loader::citycodes::area_kind as ak;
        let mut best = Ground::Sidewalk;
        let mut on_bridge = false;
        for h in self.polys.query(&Rect::new(x, y, 0., 0.)) {
            let p = self.polys.get(h);
            if !point_in_rings(x, y, &p.rings) {
                continue;
            }
            match p.kind {
                PolyKind::Area {
                    kind: ak::BRIDGE, ..
                } if lvl.is_some() => continue,
                PolyKind::Building => return Ground::Building,
                PolyKind::Water => {
                    if !on_bridge {
                        best = Ground::Water;
                    }
                }
                PolyKind::Area {
                    kind: ak::BRIDGE, ..
                } => {
                    on_bridge = true;
                    best = Ground::Plaza;
                }
                PolyKind::Area { kind, .. } => {
                    if best != Ground::Water {
                        best = if matches!(
                            kind,
                            ak::GRASS
                                | ak::WOOD
                                | ak::CEMETERY
                                | ak::ALLOTMENTS
                                | ak::PITCH
                                | ak::SAND
                        ) {
                            Ground::Grass
                        } else {
                            Ground::Plaza
                        };
                    }
                }
            }
        }
        best
    }

    pub fn inside_border(&self, x: f64, y: f64) -> bool {
        if self.border.len() < 3 {
            return x >= 0. && y >= 0. && x <= self.width && y <= self.height;
        }
        let b = &self.border_bounds;
        x >= b.x
            && y >= b.y
            && x <= b.x + b.w
            && y <= b.y + b.h
            && point_in_ring(x, y, &self.border)
    }

    /// Alle geladenen Kreuzungsscheiben (Darstellung).
    pub fn junction_discs(&self) -> impl Iterator<Item = &Junction> {
        self.edge_segs
            .slab
            .iter()
            .filter_map(|s| s.junction.as_ref())
    }

    /// Nächste Kante (ohne Kreuzungsscheiben) im Umkreis, die `filter` erfüllt.
    pub fn nearest_edge(
        &mut self,
        x: f64,
        y: f64,
        radius: f64,
        filter: impl Fn(&Edge) -> bool,
    ) -> Option<NearEdge> {
        let mut best: Option<(i64, f64)> = None;
        for h in self.edge_segs.query(&Rect::around(x, y, radius)) {
            let s = *self.edge_segs.get(h);
            let Some(id) = s.edge else { continue };
            let Some(e) = self.edges.get(&id) else {
                continue;
            };
            if !filter(e) {
                continue;
            }
            let d2 = seg_dist2(x, y, s.ax, s.ay, s.bx, s.by);
            if d2 > radius * radius || best.is_some_and(|(_, b)| d2 >= b) {
                continue;
            }
            best = Some((id, d2));
        }
        let (id, _) = best?;
        let e = &self.edges[&id];
        let p = project_on_polyline(&e.pts, x, y)?;
        Some(NearEdge {
            edge: id,
            s: p.s,
            d: p.d2.sqrt(),
            x: p.x,
            y: p.y,
            ux: p.ux,
            uy: p.uy,
        })
    }

    /// Nächste Hausnummer (optional an einer bestimmten Straße) im Umkreis.
    pub fn nearest_address(
        &mut self,
        x: f64,
        y: f64,
        radius: f64,
        street: Option<&str>,
    ) -> Option<Address> {
        let mut best: Option<(f64, Address)> = None;
        for h in self.addresses.query(&Rect::around(x, y, radius)) {
            let a = self.addresses.get(h);
            if street.is_some_and(|st| a.street != st) {
                continue;
            }
            let d = (a.x - x).hypot(a.y - y);
            if d <= radius && best.as_ref().is_none_or(|(b, _)| d < *b) {
                best = Some((d, a.clone()));
            }
        }
        best.map(|(_, a)| a)
    }
    /// POIs im Umkreis (Kopien).
    pub fn pois_near(&mut self, x: f64, y: f64, radius: f64) -> Vec<Poi> {
        self.pois
            .query(&Rect::around(x, y, radius))
            .into_iter()
            .map(|h| self.pois.get(h).clone())
            .filter(|q| (q.x - x).hypot(q.y - y) <= radius)
            .collect()
    }
    /// Ortsteil an einer Stelle.
    pub fn district_at(&self, x: f64, y: f64) -> Option<&str> {
        self.districts
            .iter()
            .find(|d| point_in_rings(x, y, &d.rings))
            .map(|d| d.name.as_str())
    }
    /// Steht eine Kirche (geladen) im Umkreis?
    pub fn church_near(&mut self, x: f64, y: f64, r: f64) -> bool {
        self.polys
            .query(&Rect::around(x, y, r))
            .into_iter()
            .any(|h| {
                let p = self.polys.get(h);
                p.kind == PolyKind::Building && p.bkind == 3
            })
    }
    /// Bezirk an einer Stelle.
    pub fn bezirk_at(&self, x: f64, y: f64) -> Option<&str> {
        self.bezirke
            .iter()
            .find(|d| point_in_rings(x, y, &d.rings))
            .map(|d| d.name.as_str())
    }
    /// Einwohner je Dichtezelle an einer Stelle (0, wo nichts geladen ist).
    pub fn density_at(&self, x: f64, y: f64) -> f64 {
        let (tx, ty) = ((x / self.tile).floor(), (y / self.tile).floor());
        let Some(d) = self.dens.get(&format!("{}_{}", tx as i64, ty as i64)) else {
            return 0.;
        };
        let (cx, cy) = (
            ((x - d.x0) / d.cell) as usize,
            ((y - d.y0) / d.cell) as usize,
        );
        if cx >= d.per || cy >= d.per {
            return 0.;
        }
        d.vals.get(cy * d.per + cx).copied().unwrap_or(0.)
    }
    /// Ortsangabe wie `map.js locationName`: Straße (an Kreuzungen beide, sonst mit Hausnummer), abseits der Kiez,
    /// dann der Ortsteil.
    pub fn location_name(&mut self, x: f64, y: f64) -> String {
        let s = self.scale;
        if let Some(near) = self.nearest_edge(x, y, 25. * s, |e| !e.name.is_empty() && e.cls <= 10)
        {
            let e = self.edges[&near.edge].clone();
            if near.d <= e.w / 2. + 8. * s {
                for nid in [e.a, e.b] {
                    let Some(nd) = self.nodes.get(&nid) else {
                        continue;
                    };
                    if (nd.x - x).hypot(nd.y - y) > e.w / 2. + 6. * s {
                        continue;
                    }
                    let other = nd
                        .edges
                        .iter()
                        .filter_map(|k| self.edges.get(k))
                        .find(|o| !o.name.is_empty() && o.name != e.name && o.cls <= 8);
                    if let Some(o) = other {
                        return format!("{} / {}", e.name, o.name);
                    }
                }
                return match self.nearest_address(x, y, e.w / 2. + 25. * s, Some(&e.name)) {
                    Some(a) => format!("{} {}", e.name, a.nr),
                    None => e.name,
                };
            }
        }
        let kiez = self
            .kieze
            .iter()
            .map(|k| ((k.x - x).hypot(k.y - y), &k.name))
            .filter(|(d, _)| *d < 700. * s)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, n)| n.clone());
        kiez.or_else(|| self.district_at(x, y).map(str::to_owned))
            .unwrap_or_else(|| "Berlin".into())
    }

    pub fn nearest_hospital(&self, x: f64, y: f64) -> Option<&Hospital> {
        self.hospitals.iter().min_by(|a, b| {
            (a.x - x)
                .hypot(a.y - y)
                .total_cmp(&(b.x - x).hypot(b.y - y))
        })
    }

    /// Mittelpunkt eines Knotens.
    pub fn node(&self, id: i64) -> Option<&Node> {
        self.nodes.get(&id)
    }
    pub fn is_junction(&self, v: i64) -> bool {
        self.nodes.get(&v).is_some_and(|n| n.edges.len() >= 3)
    }
}

fn dec<K: std::hash::Hash + Eq + Copy>(counts: &mut HashMap<K, u32>, set: &mut HashSet<K>, k: K) {
    let n = counts.get(&k).copied().unwrap_or(1).saturating_sub(1);
    if n > 0 {
        counts.insert(k, n);
    } else {
        counts.remove(&k);
        set.remove(&k);
    }
}

#[derive(Debug, Clone, Copy)]
pub struct NearEdge {
    pub edge: i64,
    pub s: f64,
    pub d: f64,
    pub x: f64,
    pub y: f64,
    pub ux: f64,
    pub uy: f64,
}

// --- Geometrie in f64 (Port der benötigten Teile von geom.js) ---------------------------------

pub fn bounds_of(pts: &[Pt]) -> Rect {
    if pts.is_empty() {
        return Rect::default();
    }
    let (mut x0, mut y0, mut x1, mut y1) = (
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    );
    for &(x, y) in pts {
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    }
    Rect::new(x0, y0, x1 - x0, y1 - y0)
}
pub fn point_in_ring(x: f64, y: f64, ring: &[Pt]) -> bool {
    let mut inside = false;
    let n = ring.len();
    let mut j = n.wrapping_sub(1);
    for i in 0..n {
        let ((xi, yi), (xj, yj)) = (ring[i], ring[j]);
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}
/// Gerade-/Ungerade-Regel über alle Ringe (Innenhöfe sind Löcher).
pub fn point_in_rings(x: f64, y: f64, rings: &[Vec<Pt>]) -> bool {
    rings.iter().filter(|r| point_in_ring(x, y, r)).count() % 2 == 1
}
pub fn seg_dist2(px: f64, py: f64, ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    let (dx, dy) = (bx - ax, by - ay);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 0. {
        (((px - ax) * dx + (py - ay) * dy) / l2).clamp(0., 1.)
    } else {
        0.
    };
    let (ex, ey) = (ax + dx * t - px, ay + dy * t - py);
    ex * ex + ey * ey
}
pub fn polyline_length(pts: &[Pt]) -> f64 {
    pts.windows(2)
        .map(|w| (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1))
        .sum()
}
/// Punkt auf dem Linienzug in Bogenlänge `s` samt Richtung (geklemmt).
#[derive(Debug, Clone, Copy, Default)]
pub struct Along {
    pub x: f64,
    pub y: f64,
    pub ux: f64,
    pub uy: f64,
    pub s: f64,
    pub d2: f64,
}
pub fn point_along(pts: &[Pt], s: f64) -> Along {
    let n = pts.len();
    let mut s = s.max(0.);
    if n == 0 {
        return Along {
            ux: 1.,
            ..Default::default()
        };
    }
    for i in 0..n.saturating_sub(1) {
        let ((ax, ay), (bx, by)) = (pts[i], pts[i + 1]);
        let (dx, dy) = (bx - ax, by - ay);
        let l = dx.hypot(dy);
        if s <= l || i == n - 2 {
            let t = if l > 0. { (s / l).min(1.) } else { 0. };
            let (ux, uy) = if l > 0. { (dx / l, dy / l) } else { (1., 0.) };
            return Along {
                x: ax + dx * t,
                y: ay + dy * t,
                ux,
                uy,
                s: 0.,
                d2: 0.,
            };
        }
        s -= l;
    }
    Along {
        x: pts[n - 1].0,
        y: pts[n - 1].1,
        ux: 1.,
        ..Default::default()
    }
}
pub fn project_on_polyline(pts: &[Pt], x: f64, y: f64) -> Option<Along> {
    let mut best: Option<Along> = None;
    let mut acc = 0.;
    for w in pts.windows(2) {
        let ((ax, ay), (bx, by)) = (w[0], w[1]);
        let (dx, dy) = (bx - ax, by - ay);
        let l2 = dx * dx + dy * dy;
        let l = l2.sqrt();
        let t = if l2 > 0. {
            (((x - ax) * dx + (y - ay) * dy) / l2).clamp(0., 1.)
        } else {
            0.
        };
        let (px, py) = (ax + dx * t, ay + dy * t);
        let d2 = (px - x).powi(2) + (py - y).powi(2);
        if best.is_none_or(|b| d2 < b.d2) {
            let (ux, uy) = if l > 0. { (dx / l, dy / l) } else { (1., 0.) };
            best = Some(Along {
                x: px,
                y: py,
                ux,
                uy,
                s: acc + t * l,
                d2,
            });
        }
        acc += l;
    }
    best
}
/// Parallele Linie im Abstand d (positiv = rechts in Laufrichtung bei y nach Süden), wie `geom.js`.
pub fn offset_polyline(pts: &[Pt], d: f64) -> Vec<Pt> {
    let n = pts.len();
    (0..n)
        .map(|i| {
            let (a, b) = (i.saturating_sub(1), (i + 1).min(n - 1));
            let (mut dx, mut dy) = (pts[b].0 - pts[a].0, pts[b].1 - pts[a].1);
            let l = dx.hypot(dy);
            let l = if l > 0. { l } else { 1. };
            dx /= l;
            dy /= l;
            (pts[i].0 - dy * d, pts[i].1 + dx * d)
        })
        .collect()
}

/// Mehrfach genutzt: Testkarte aus Bausteinen (ohne Dateien), für Unit-Tests der Simulation.
pub mod fixture {
    use super::*;
    /// Quelle, die vorgefertigte Kachel-JSONs liefert.
    pub struct MemSource(
        pub HashMap<String, Vec<u8>>,
        Vec<(String, Result<TileData, String>)>,
    );
    impl MemSource {
        pub fn new(tiles: HashMap<String, Vec<u8>>) -> Self {
            Self(tiles, Vec::new())
        }
    }
    impl TileSource for MemSource {
        fn request(&mut self, key: &str) {
            let r = match self.0.get(key) {
                Some(b) => TileData::decode(b).map_err(|e| format!("{e:#}")),
                None => Err("fehlt".into()),
            };
            self.1.push((key.to_owned(), r));
        }
        fn poll(&mut self) -> Vec<(String, Result<TileData, String>)> {
            std::mem::take(&mut self.1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn index() -> Value {
        json!({
            "meta": {"version": 3, "scale": 10, "tile": 6400, "width": 12800, "height": 6400},
            "tiles": ["0_0", "1_0"],
            "border": [[0, 0, 12800, 0, 0, 6400, -12800, 0]],
            "places": {"giver": {"x": 100, "y": 100}, "playerSpawn": {"x": 100, "y": 120}, "dropoff": {"x": 300, "y": 100},
                       "playerCar": {"x": 200, "y": 100, "angle": 0}, "pickup": {"x": 9000, "y": 100},
                       "crates": [{"x": 9100, "y": 50, "w": 20, "h": 20}], "timeLimit": 200},
            "hospitals": [[500, 500, "Charité"]]
        })
    }
    /// Kachel 0_0: Straße von (0, 1000) nach (6400, 1000), Haus, Baum, Wasser; Kachel 1_0 setzt die Straße fort
    /// (dieselbe Kante 7 liegt in beiden Kacheln, genau wie bei mehrfach abgelegten Objekten im Build).
    fn tiles() -> HashMap<String, Vec<u8>> {
        let t0 = json!({"v": 3, "t": [0, 0], "names": ["Teststraße"],
            "vertices": {"id": [1, 2], "xy": [0, 1000, 6400, 0], "trim": [0, 0]},
            "edges": [[7, 0, 1, 5, 120, 0, 0, 2, [], [1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 50, 0, 1], 80]],
            "junctions": [], "walls": [[90, 1, [0, 0, 0, 6400]]],
            "buildings": [[300, 120, 0, [[1000, 1200, 400, 0, 0, 400, -400, 0]]]],
            "water": [[-1, [[1, [3000, 2000, 1000, 0, 0, 1000, -1000, 0]]]]],
            "areas": [[400, 4, [[1, [5000, 2000, 500, 0, 0, 500, -500, 0]]]]],
            "barriers": [], "posts": [2000, 1000, 0, 2500, 1200, 0], "crossings": [], "signals": [2], "turnBans": [],
            "trees": {"xy": [1500, 1300, 100, 0], "g": [0, 0], "c": [0, 0], "r": [0, 0]}});
        let t1 = json!({"v": 3, "t": [1, 0], "names": [],
            "vertices": {"id": [1, 2, 3], "xy": [0, 1000, 6400, 0, 3200, 0], "trim": [0, 0, 0]},
            "edges": [[7, 0, 1, 5, 120, 0, 0, 2, [], [1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 50, 0, 1]],
                      [8, 1, 2, 7, 80, -1, 0, 2, [], [0, 30]]],
            "signals": [], "turnBans": [], "trees": {"xy": [], "g": [], "c": [], "r": []}});
        HashMap::from([
            ("0_0".into(), t0.to_string().into_bytes()),
            ("1_0".into(), t1.to_string().into_bytes()),
        ])
    }
    fn city() -> City {
        City::from_index(&index(), Box::new(fixture::MemSource::new(tiles()))).unwrap()
    }

    #[test]
    fn decodes_and_queries_surfaces() {
        let mut c = city();
        assert!(c.focus("cam", 1000., 1000.));
        assert_eq!(c.surface_at(1000., 1000., None), Ground::Road);
        assert_eq!(c.surface_at(1200., 1400., None), Ground::Building);
        assert_eq!(c.in_building(1200., 1400.), Some(300));
        assert_eq!(c.surface_at(3500., 2500., None), Ground::Water);
        assert_eq!(c.surface_at(5200., 2200., None), Ground::Grass);
        assert_eq!(c.surface_at(800., 3000., None), Ground::Sidewalk);
        assert_eq!(c.surface_at(-5., 3000., None), Ground::Building);
        assert!(c.signals.contains(&2));
        let e = &c.edges[&7];
        assert_eq!(
            (e.name.as_str(), e.cs.fwd, e.cs.bwd, e.cs.maxspeed, e.inside),
            ("Teststraße", 1, 1, 50., true)
        );
        assert!((e.w - 120.).abs() < 1e-9 && (e.len - 6400.).abs() < 1e-9 && e.dtv == 8000.);
        // Pfosten auf der Fahrbahn fällt weg (Sicherheitsnetz), der daneben bleibt
        let posts = c
            .solids
            .slab
            .iter()
            .filter(|s| {
                matches!(
                    s,
                    Solid::Circle {
                        kind: CircleKind::Barrier { .. },
                        ..
                    }
                )
            })
            .count();
        assert_eq!(posts, 1);
        assert!(c.inside_border(5000., 3000.) && !c.inside_border(-1., 3000.));
        let n = c.nearest_edge(3000., 1100., 300., |_| true).unwrap();
        assert_eq!(n.edge, 7);
        assert!((n.d - 100.).abs() < 1e-9 && (n.s - 3000.).abs() < 1e-9);
        assert_eq!(c.nearest_hospital(0., 0.).unwrap().name, "Charité");
    }

    #[test]
    fn shared_objects_are_refcounted_and_unloaded() {
        let mut c = city();
        c.load_area(0., 0., 12799., 100., false);
        assert_eq!(c.installed_count(), 2);
        assert_eq!(c.nodes[&2].edges, vec![7, 8]);
        let solids = c.solids.slab.len();
        // Kante 7 nur einmal, obwohl sie in beiden Kacheln steht
        let segs7 = c
            .edge_segs
            .slab
            .iter()
            .filter(|s| s.edge == Some(7))
            .count();
        assert_eq!(segs7, 1);
        c.unload("0_0");
        assert!(c.edges.contains_key(&7), "Kachel 1_0 hält die Kante noch");
        assert!(c.solids.slab.len() < solids);
        assert!(c.signals.is_empty());
        c.unload("1_0");
        assert!(c.edges.is_empty() && c.nodes.is_empty() && c.edge_segs.slab.is_empty());
        assert_eq!(c.solids.slab.len(), 1, "nur die Missionskiste bleibt");
        let added = c
            .edge_events
            .iter()
            .filter(|e| matches!(e, EdgeEvent::Added(_)))
            .count();
        let removed = c
            .edge_events
            .iter()
            .filter(|e| matches!(e, EdgeEvent::Removed(_)))
            .count();
        assert_eq!((added, removed), (2, 2));
    }

    #[test]
    fn failed_tiles_retry_after_pause() {
        let mut tiles = tiles();
        tiles.insert("0_0".into(), b"{kaputt".to_vec());
        let mut c = City::from_index(&index(), Box::new(fixture::MemSource::new(tiles))).unwrap();
        assert!(!c.focus("cam", 1000., 1000.));
        assert_eq!(c.errors.len(), 1);
        c.focus("cam", 1000., 1000.);
        assert_eq!(c.errors.len(), 1, "Fehler einmal melden");
        c.tick(1.);
        c.focus("cam", 1000., 1000.);
        assert_eq!(c.errors.len(), 1);
    }

    #[test]
    fn geometry_helpers() {
        let pts = [(0., 0.), (100., 0.), (100., 100.)];
        let a = point_along(&pts, 150.);
        assert!((a.x - 100.).abs() < 1e-9 && (a.y - 50.).abs() < 1e-9 && a.uy == 1.);
        assert_eq!(point_along(&pts, 500.).y, 100.);
        let p = project_on_polyline(&pts, 120., 40.).unwrap();
        assert!((p.s - 140.).abs() < 1e-9 && (p.d2 - 400.).abs() < 1e-9);
        let o = offset_polyline(&[(0., 0.), (100., 0.)], 10.);
        assert_eq!(o, vec![(0., 10.), (100., 10.)]); // rechts bei y nach Süden
        let ring = vec![(0., 0.), (10., 0.), (10., 10.), (0., 10.)];
        let hole = vec![(3., 3.), (7., 3.), (7., 7.), (3., 7.)];
        assert!(point_in_rings(1., 1., &[ring.clone(), hole.clone()]));
        assert!(!point_in_rings(5., 5., &[ring, hole]));
    }
}
