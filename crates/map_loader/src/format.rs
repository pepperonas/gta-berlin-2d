use crate::{
    citycodes::unpack_lvl,
    geom::{Bounds, undelta},
};
use anyhow::{Context, Result, ensure};
use glam::Vec2;
use serde::Deserialize;
use serde_json::Value;
use std::{collections::HashSet, fs, path::Path};
#[derive(Debug, Clone, Deserialize)]
pub struct Origin {
    pub lat0: f64,
    pub lon0: f64,
    pub bbox: [f64; 4],
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    pub version: u32,
    pub scale: f32,
    pub tile: f32,
    pub tiles_x: u32,
    pub tiles_y: u32,
    pub width: f32,
    pub height: f32,
    pub origin: Origin,
    pub attribution: String,
}
#[derive(Debug, Clone, Deserialize)]
pub struct Place {
    pub x: f32,
    pub y: f32,
}
impl Place {
    pub fn point(&self) -> Vec2 {
        Vec2::new(self.x, self.y)
    }
}
#[derive(Debug, Clone, Deserialize)]
pub struct Places {
    #[serde(rename = "playerSpawn")]
    pub player_spawn: Place,
}
#[derive(Debug, Clone, Deserialize)]
pub struct Index {
    pub meta: Meta,
    pub tiles: Vec<String>,
    pub places: Places,
}
impl Index {
    pub fn read(root: &Path) -> Result<Self> {
        let path = root.join("index.json");
        let index: Self = serde_json::from_slice(
            &fs::read(&path).with_context(|| format!("{} lesen", path.display()))?,
        )?;
        ensure!(
            index.meta.version == 3,
            "Kartenformat {} wird nicht unterstützt (erwartet 3)",
            index.meta.version
        );
        let m = &index.meta;
        ensure!(
            m.scale.is_finite()
                && m.scale > 0.0
                && m.tile.is_finite()
                && m.tile > 0.0
                && m.width.is_finite()
                && m.width > 0.0
                && m.height.is_finite()
                && m.height > 0.0
                && m.tiles_x > 0
                && m.tiles_y > 0,
            "Ungültige Kartenabmessungen"
        );
        ensure!(
            index.places.player_spawn.point().is_finite(),
            "Ungültiger Startpunkt"
        );
        let mut seen = HashSet::new();
        for key in &index.tiles {
            let key = TileKey::parse(key)?;
            ensure!(
                key.x < m.tiles_x && key.y < m.tiles_y,
                "Kachel außerhalb des Rasters"
            );
            ensure!(seen.insert(key), "Doppelte Kachel");
        }
        Ok(index)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TileKey {
    pub x: u32,
    pub y: u32,
}
impl TileKey {
    pub fn parse(s: &str) -> Result<Self> {
        let (x, y) = s.split_once('_').context("Ungültiger Kachelschlüssel")?;
        Ok(Self {
            x: x.parse()?,
            y: y.parse()?,
        })
    }
    pub fn bounds(self, tile: f32) -> Bounds {
        let min = Vec2::new(self.x as f32, self.y as f32) * tile;
        Bounds {
            min,
            max: min + Vec2::splat(tile),
        }
    }
}
impl std::fmt::Display for TileKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}_{}", self.x, self.y)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FeatureId(pub u8, pub i64);
#[derive(Debug, Clone)]
pub struct Polygon {
    pub rings: Vec<Vec<Vec2>>,
    pub outer: Vec<bool>,
}
#[derive(Debug, Clone)]
pub struct Building {
    pub id: i64,
    pub height: f32,
    pub meters: f32,
    pub kind: u8,
    pub polygon: Polygon,
    pub walls: Option<Vec<Vec<Vec2>>>,
    pub look: u32,
    pub roof_rgb: Option<u32>,
    pub wall_rgb: Option<u32>,
    pub center: Vec2,
    pub seed: u32,
    /// Hauseingänge: Ring, Kante, Lage entlang der Kante (0…1)
    pub doors: Vec<(usize, usize, f32)>,
}
#[derive(Debug, Clone)]
pub struct Road {
    pub id: i64,
    pub points: Vec<Vec2>,
    pub width: f32,
    pub class: u8,
    pub level: i8,
    pub bridge: bool,
    pub passage: bool,
    pub surface: u8,
    pub forward: u8,
    pub backward: u8,
    pub park: [u8; 2],
    pub park_width: [f32; 2],
    pub cycle: [f32; 2],
    pub track: [f32; 2],
    pub fill: f32,
}
#[derive(Debug, Clone)]
pub enum Feature {
    Building(Building),
    Area {
        polygon: Polygon,
        kind: u8,
        level: i8,
        clip: Option<Bounds>,
    },
    Water {
        polygon: Polygon,
        clip: Option<Bounds>,
    },
    Road(Road),
    Junction {
        point: Vec2,
        radius: f32,
        level: i8,
        cobble: bool,
    },
    Line {
        points: Vec<Vec2>,
        kind: u8,
        level: i8,
        hidden: bool,
    },
    Tree {
        point: Vec2,
        radius: f32,
        seed: u32,
        genus: u8,
    },
}
#[derive(Debug, Clone)]
pub struct Item {
    pub id: Option<FeatureId>,
    pub feature: Feature,
}
#[derive(Debug, Clone)]
pub struct Tile {
    pub key: TileKey,
    pub items: Vec<Item>,
}
#[derive(Deserialize)]
struct RawVertices {
    xy: Vec<f32>,
}
#[derive(Deserialize, Default)]
struct RawTrees {
    #[serde(default)]
    xy: Vec<f32>,
    #[serde(default)]
    g: Vec<u8>,
    #[serde(default)]
    c: Vec<f32>,
}
#[derive(Deserialize)]
struct RawTile {
    v: u32,
    t: [u32; 2],
    vertices: RawVertices,
    #[serde(default)]
    edges: Vec<Value>,
    #[serde(default)]
    junctions: Vec<Value>,
    #[serde(default)]
    paths: Vec<Value>,
    #[serde(default)]
    rails: Vec<Value>,
    #[serde(default)]
    fences: Vec<Value>,
    #[serde(default)]
    walls: Vec<Value>,
    #[serde(default)]
    buildings: Vec<Value>,
    #[serde(default)]
    areas: Vec<Value>,
    #[serde(default)]
    water: Vec<Value>,
    #[serde(default)]
    trees: RawTrees,
}
fn array(v: &Value) -> Result<&[Value]> {
    v.as_array().map(|v| v.as_slice()).context("Array erwartet")
}
fn number(v: &Value) -> Result<f32> {
    let n = v.as_f64().context("Zahl erwartet")? as f32;
    ensure!(n.is_finite(), "Nicht-endliche Zahl");
    Ok(n)
}
fn integer(v: &Value) -> Result<i64> {
    v.as_i64().context("Ganzzahl erwartet")
}
fn code(v: &Value) -> Result<u8> {
    Ok(u8::try_from(integer(v)?)?)
}
fn optional(row: &[Value], i: usize) -> u32 {
    row.get(i).and_then(Value::as_u64).unwrap_or(0) as u32
}
fn coords(v: &Value) -> Result<Vec<Vec2>> {
    let flat = array(v)?.iter().map(number).collect::<Result<Vec<_>>>()?;
    undelta(&flat)
}
fn row(v: &Value, n: usize) -> Result<&[Value]> {
    let r = array(v)?;
    ensure!(r.len() >= n, "Kartenzeile zu kurz: {} < {n}", r.len());
    Ok(r)
}
fn polygon(v: &Value, flags: bool) -> Result<Polygon> {
    let mut rings = Vec::new();
    let mut outer = Vec::new();
    for (i, r) in array(v)?.iter().enumerate() {
        let (is_outer, points) = if flags {
            let pair = row(r, 2)?;
            (integer(&pair[0])? != 0, coords(&pair[1])?)
        } else {
            (i == 0, coords(r)?)
        };
        ensure!(points.len() >= 3, "Ring mit weniger als drei Punkten");
        rings.push(points);
        outer.push(is_outer);
    }
    ensure!(!rings.is_empty(), "Polygon ohne Ringe");
    Ok(Polygon { rings, outer })
}
fn gid(v: &Value) -> Result<Option<FeatureId>> {
    let id = integer(v)?;
    Ok(if id >= 0 {
        Some(FeatureId(1, id))
    } else {
        None
    })
}
impl Tile {
    pub fn read(root: &Path, key: TileKey, meta: &Meta) -> Result<Self> {
        let path = root.join("tiles").join(format!("{key}.json"));
        let bytes = fs::read(&path).with_context(|| format!("{} lesen", path.display()))?;
        Self::decode(&bytes, key, meta).with_context(|| format!("Kachel {key}"))
    }
    pub fn decode(bytes: &[u8], key: TileKey, meta: &Meta) -> Result<Self> {
        let raw: RawTile = serde_json::from_slice(bytes)?;
        ensure!(raw.v == 3, "Kachelformat {} statt 3", raw.v);
        ensure!(raw.t == [key.x, key.y], "Falsche Kachelkoordinaten");
        let scale = meta.scale;
        let vertices = undelta(&raw.vertices.xy)?;
        let mut items = Vec::new();
        for e in raw.edges {
            let r = row(&e, 10)?;
            let ia = usize::try_from(integer(&r[1])?)?;
            let ib = usize::try_from(integer(&r[2])?)?;
            let mut points = vec![*vertices.get(ia).context("Ungültiger Straßenknoten")?];
            points.extend(coords(&r[8])?);
            points.push(*vertices.get(ib).context("Ungültiger Straßenknoten")?);
            let flags = integer(&r[7])? as u32;
            let cs = array(&r[9])?;
            let width = number(&r[4])? * scale / 10.0;
            ensure!(width > 0.0, "Ungültige Straßenbreite");
            let c = |i: usize| -> f32 { cs.get(i).and_then(Value::as_f64).unwrap_or(0.0) as f32 };
            let short = cs.len() == 2;
            let surface = if short { c(1) } else { c(11) } as u8;
            items.push(Item {
                id: Some(FeatureId(0, integer(&r[0])?)),
                feature: Feature::Road(Road {
                    id: integer(&r[0])?,
                    points,
                    width,
                    class: code(&r[3])?,
                    level: unpack_lvl(flags >> 4),
                    bridge: flags & 1 != 0,
                    passage: flags & 8 != 0,
                    surface,
                    forward: if short { 1 } else { c(0) as u8 },
                    backward: if short { 1 } else { c(1) as u8 },
                    park: if short {
                        [0; 2]
                    } else {
                        [c(2) as u8, c(5) as u8]
                    },
                    park_width: if short {
                        [0.; 2]
                    } else {
                        [c(3) * scale / 10., c(6) * scale / 10.]
                    },
                    cycle: if short {
                        [0.; 2]
                    } else {
                        [c(8) * scale / 10., c(9) * scale / 10.]
                    },
                    track: if short {
                        [0.; 2]
                    } else {
                        [c(13) * scale / 10., c(14) * scale / 10.]
                    },
                    fill: r.get(11).map(number).transpose()?.unwrap_or(0.) * scale / 10.,
                }),
            });
        }
        for j in raw.junctions {
            let r = row(&j, 6)?;
            let flags = integer(&r[4])? as u32;
            items.push(Item {
                id: Some(FeatureId(2, integer(&r[0])?)),
                feature: Feature::Junction {
                    point: Vec2::new(number(&r[1])?, number(&r[2])?),
                    radius: number(&r[3])?,
                    level: unpack_lvl(flags >> 2),
                    cobble: flags & 2 != 0,
                },
            });
        }
        for b in raw.buildings {
            let r = row(&b, 4)?;
            let mut polygon = polygon(&r[3], false)?;
            polygon.outer = crate::geom::ring_depths(&crate::geom::ring_parents(&polygon.rings))
                .iter()
                .map(|depth| depth % 2 == 0)
                .collect();
            let center =
                polygon.rings[0].iter().copied().sum::<Vec2>() / polygon.rings[0].len() as f32;
            let id = integer(&r[0])?;
            let meters = number(&r[1])? / 10.;
            let walls = if let Some(v) = r.get(4).filter(|v| v.is_array()) {
                Some(array(v)?.iter().map(coords).collect::<Result<Vec<_>>>()?)
            } else {
                None
            };
            let seed = (crate::citycodes::hash01((id as u32).wrapping_mul(7).wrapping_add(3)) * 1e9)
                .floor() as u32;
            let doors = r
                .get(5)
                .and_then(|v| v.as_array())
                .map(|list| {
                    list.iter()
                        .filter_map(|d| {
                            let d = d.as_array()?;
                            Some((
                                d.first()?.as_u64()? as usize,
                                d.get(1)?.as_u64()? as usize,
                                d.get(2)?.as_f64()? as f32 / 1000.,
                            ))
                        })
                        .collect()
                })
                .unwrap_or_default();
            items.push(Item {
                id: gid(&r[0])?,
                feature: Feature::Building(Building {
                    id,
                    height: meters * scale,
                    meters,
                    kind: code(&r[2])?,
                    polygon,
                    walls,
                    look: optional(r, 6),
                    roof_rgb: optional(r, 7).checked_sub(1),
                    wall_rgb: optional(r, 8).checked_sub(1),
                    center,
                    seed,
                    doors,
                }),
            });
        }
        for a in raw.areas {
            let r = row(&a, 3)?;
            let id = gid(&r[0])?;
            items.push(Item {
                id,
                feature: Feature::Area {
                    polygon: polygon(&r[2], true)?,
                    kind: code(&r[1])?,
                    level: r.get(3).map(integer).transpose()?.unwrap_or(0) as i8,
                    clip: if id.is_none() {
                        Some(key.bounds(meta.tile))
                    } else {
                        None
                    },
                },
            });
        }
        for a in raw.water {
            let r = row(&a, 2)?;
            let id = gid(&r[0])?;
            items.push(Item {
                id,
                feature: Feature::Water {
                    polygon: polygon(&r[1], true)?,
                    clip: if id.is_none() {
                        Some(key.bounds(meta.tile))
                    } else {
                        None
                    },
                },
            });
        }
        // Line kinds: 0 path, 1 rail, 2 fence; tunnels aren't drawn as surface tracks.
        for p in raw.paths {
            let r = row(&p, 3)?;
            let flags = integer(&r[1])? as u32;
            items.push(Item {
                id: gid(&r[0])?,
                feature: Feature::Line {
                    points: coords(&r[2])?,
                    kind: 0,
                    level: unpack_lvl(flags >> 2),
                    hidden: flags & 2 != 0,
                },
            });
        }
        for p in raw.rails {
            let r = row(&p, 4)?;
            let level = r.get(4).map(integer).transpose()?.unwrap_or(0) as i8;
            items.push(Item {
                id: gid(&r[0])?,
                feature: Feature::Line {
                    points: coords(&r[3])?,
                    kind: 1,
                    level,
                    hidden: integer(&r[2])? != 0 && integer(&r[1])? == 0,
                },
            });
        }
        for p in raw.fences {
            let r = row(&p, 3)?;
            items.push(Item {
                id: gid(&r[0])?,
                feature: Feature::Line {
                    points: coords(&r[2])?,
                    kind: 2,
                    level: 0,
                    hidden: false,
                },
            });
        }
        for p in raw.walls {
            let r = row(&p, 3)?;
            if code(&r[1])? == crate::citycodes::wall_kind::FENCE {
                items.push(Item {
                    id: gid(&r[0])?,
                    feature: Feature::Line {
                        points: coords(&r[2])?,
                        kind: 2,
                        level: 0,
                        hidden: false,
                    },
                });
            }
        }
        let trees = undelta(&raw.trees.xy)?;
        ensure!(
            trees.len() == raw.trees.g.len() && trees.len() == raw.trees.c.len(),
            "Baumlisten unterschiedlicher Länge"
        );
        for (i, point) in trees.into_iter().enumerate() {
            let seed = (crate::citycodes::hash01(
                (point.x as u32)
                    .wrapping_mul(7919)
                    .wrapping_add(point.y as u32),
            ) * 1e6)
                .floor() as u32;
            let crown = raw.trees.c[i];
            let radius = if crown > 0.0 {
                crown / 20. * scale
            } else {
                (2.2 + crate::citycodes::hash01(seed + 11) as f32 * 1.4) * scale
            };
            items.push(Item {
                id: None,
                feature: Feature::Tree {
                    point,
                    radius,
                    seed,
                    genus: raw.trees.g[i],
                },
            });
        }
        Ok(Self { key, items })
    }
}
