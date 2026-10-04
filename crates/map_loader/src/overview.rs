//! Stadtplan von ganz Berlin (`overview.json` + Grenzen aus `index.json`), wie `hud.js buildOverview`:
//! Flächen und Wasser als Dreiecke, Straßen/Bahnen/Bezirksgrenzen als Linien, deren Breite in Bildschirmpunkten
//! angegeben ist (der Shader dehnt sie je Zoom). Dazu die Beschriftungsdaten für `maplabels`.
use anyhow::{Context, Result};
use bytemuck::{Pod, Zeroable};
use glam::Vec2;
use serde_json::Value;
use std::path::Path;

/// Ein Eckpunkt der Stadtplanzeichnung: Weltpunkt, Versatz in halben Linienbreiten (Normale + Kappe) und
/// Linienbreite in Bildschirmpunkten (grob · fein, `hud.js`: ab Zoom 4 dicker). Flächen haben Breite 0.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct OverlayVertex {
    pub pos: [f32; 2],
    pub offset: [f32; 2],
    pub width: [f32; 2],
    pub color: [f32; 4],
}

#[derive(Debug, Clone, Default)]
pub struct OverlayMesh {
    pub vertices: Vec<OverlayVertex>,
    pub indices: Vec<u32>,
}
impl OverlayMesh {
    /// Gefüllte Fläche (äußere Ringe mit ihren Löchern), Ringe als flache Punktlisten.
    pub fn fill(&mut self, outer: &[Vec2], holes: &[Vec<Vec2>], color: [f32; 4]) {
        if outer.len() < 3 {
            return;
        }
        let mut coords: Vec<f64> =
            Vec::with_capacity((outer.len() + holes.iter().map(Vec::len).sum::<usize>()) * 2);
        let mut hole_idx = Vec::with_capacity(holes.len());
        for p in outer {
            coords.extend([p.x as f64, p.y as f64]);
        }
        for h in holes.iter().filter(|h| h.len() >= 3) {
            hole_idx.push(coords.len() / 2);
            for p in h {
                coords.extend([p.x as f64, p.y as f64]);
            }
        }
        let Ok(tris) = earcutr::earcut(&coords, &hole_idx, 2) else {
            return;
        };
        let base = self.vertices.len() as u32;
        for c in coords.as_chunks::<2>().0 {
            self.vertices.push(OverlayVertex {
                pos: [c[0] as f32, c[1] as f32],
                offset: [0.; 2],
                width: [0.; 2],
                color,
            });
        }
        self.indices
            .extend(tris.into_iter().map(|i| base + i as u32));
    }
    /// Linienzug mit eckigen Kappen (je Abschnitt ein Viereck; überlappende Enden schließen die Knicke).
    pub fn line(&mut self, pts: &[Vec2], width: [f32; 2], color: [f32; 4], closed: bool) {
        let n = pts.len();
        let segs = if closed { n } else { n.saturating_sub(1) };
        for i in 0..segs {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            let d = b - a;
            if d.length_squared() < 1e-6 {
                continue;
            }
            let d = d.normalize();
            let nrm = Vec2::new(-d.y, d.x);
            let base = self.vertices.len() as u32;
            for (p, o) in [(a, -d + nrm), (a, -d - nrm), (b, d + nrm), (b, d - nrm)] {
                self.vertices.push(OverlayVertex {
                    pos: p.into(),
                    offset: o.into(),
                    width,
                    color,
                });
            }
            self.indices
                .extend([base, base + 1, base + 2, base + 1, base + 3, base + 2]);
        }
    }
}

/// Punktbeschriftung: Weltpunkt, Text, Fläche (Rang; größere zuerst).
#[derive(Debug, Clone, PartialEq)]
pub struct PointLabel {
    pub at: Vec2,
    pub text: String,
    pub area: f32,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Station {
    pub at: Vec2,
    /// `ubahn`, `sbahn`, …
    pub cat: String,
    pub name: String,
}

pub struct Overview {
    pub mesh: OverlayMesh,
    pub width: f32,
    pub height: f32,
    pub bezirke: Vec<PointLabel>,
    pub ortsteile: Vec<PointLabel>,
    pub kieze: Vec<PointLabel>,
    pub stations: Vec<Station>,
    /// Straßen: je Name die Mitte des längsten Stücks (Ortssuche der Befehlszeile)
    pub streets: Vec<PointLabel>,
}

/// Straßen je Name: Mitte des längsten Stücks, Länge als Rang (`console.js placeIndex`).
pub fn street_points(ov: &Value) -> Vec<PointLabel> {
    let names = ov["names"].as_array().cloned().unwrap_or_default();
    let mut best: std::collections::BTreeMap<usize, (f32, Vec2)> = Default::default();
    for r in ov["roads"].as_array().into_iter().flatten() {
        let Some(n) = r[1].as_i64().filter(|&n| n >= 0).map(|n| n as usize) else {
            continue;
        };
        if names
            .get(n)
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
        {
            continue;
        }
        let pts = undelta(&r[2]);
        if pts.is_empty() {
            continue;
        }
        let len: f32 = pts.windows(2).map(|w| w[0].distance(w[1])).sum();
        let mid = pts[pts.len() / 2];
        if best.get(&n).is_none_or(|b| len > b.0) {
            best.insert(n, (len, mid));
        }
    }
    best.into_iter()
        .map(|(n, (len, at))| PointLabel {
            at,
            text: names[n].as_str().unwrap_or("").to_owned(),
            area: len,
        })
        .collect()
}

/// Farben wie `hud.js` (sRGB 0…1).
fn hex(c: u32, a: f32) -> [f32; 4] {
    [
        ((c >> 16) & 255) as f32 / 255.,
        ((c >> 8) & 255) as f32 / 255.,
        (c & 255) as f32 / 255.,
        a,
    ]
}
/// Flächenfarbe je `AREA_KIND` (hud.js MINI_AREA).
fn area_color(kind: u64) -> [f32; 4] {
    hex(
        match kind {
            0 => 0x4a4640,
            2 => 0x35602c,
            6 => 0x6b6040,
            7 => 0x284d22,
            8 => 0x4a4540,
            _ => 0x2f5a2a,
        },
        1.,
    )
}

/// Deltakodierte Punktliste (`geom.js undelta`).
pub fn undelta(v: &Value) -> Vec<Vec2> {
    let d: Vec<f64> = v
        .as_array()
        .map(|a| a.iter().filter_map(Value::as_f64).collect())
        .unwrap_or_default();
    let mut out = Vec::with_capacity(d.len() / 2);
    let (mut x, mut y) = (0., 0.);
    for (i, p) in d.as_chunks::<2>().0.iter().enumerate() {
        if i == 0 {
            (x, y) = (p[0], p[1]);
        } else {
            x += p[0];
            y += p[1];
        }
        out.push(Vec2::new(x as f32, y as f32));
    }
    out
}

/// Ringgruppen `[[flag, delta] …]`: flag 1 beginnt eine neue Fläche, 0 ist ein Loch der vorigen.
fn fill_rings(mesh: &mut OverlayMesh, rings: &Value, color: [f32; 4]) {
    let mut outer: Option<Vec<Vec2>> = None;
    let mut holes = Vec::new();
    for r in rings.as_array().into_iter().flatten() {
        let pts = undelta(&r[1]);
        if r[0].as_u64() == Some(0) && outer.is_some() {
            holes.push(pts);
        } else {
            if let Some(o) = outer.take() {
                mesh.fill(&o, &holes, color);
            }
            holes.clear();
            outer = Some(pts);
        }
    }
    if let Some(o) = outer {
        mesh.fill(&o, &holes, color);
    }
}

fn point_labels(v: &Value) -> Vec<PointLabel> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(|e| {
            Some(PointLabel {
                at: Vec2::new(e[0].as_f64()? as f32, e[1].as_f64()? as f32),
                text: e[2].as_str()?.to_owned(),
                area: e[3].as_f64().unwrap_or(0.) as f32,
            })
        })
        .collect()
}

impl Overview {
    pub fn read(root: &Path) -> Result<Self> {
        let ov: Value = serde_json::from_slice(
            &std::fs::read(root.join("overview.json")).context("overview.json fehlt")?,
        )?;
        let index: Value = serde_json::from_slice(&std::fs::read(root.join("index.json"))?)?;
        Ok(Self::build(&ov, &index))
    }
    pub fn build(ov: &Value, index: &Value) -> Self {
        let meta = &index["meta"];
        let (width, height) = (
            meta["width"].as_f64().unwrap_or(0.) as f32,
            meta["height"].as_f64().unwrap_or(0.) as f32,
        );
        let mut mesh = OverlayMesh::default();
        // Zeichenreihenfolge wie hud.js: Flächen, Wasser, Straßen (klein → groß), Bahnen, außerhalb, Bezirke, Grenze
        for a in ov["areas"].as_array().into_iter().flatten() {
            fill_rings(&mut mesh, &a[1], area_color(a[0].as_u64().unwrap_or(4)));
        }
        for w in ov["water"].as_array().into_iter().flatten() {
            fill_rings(&mut mesh, w, hex(0x1f4f78, 1.));
        }
        let roads = ov["roads"].as_array().cloned().unwrap_or_default();
        type Class = (u32, [f32; 2], fn(u64) -> bool);
        let classes: [Class; 4] = [
            (0x6b6f78, [0.6, 1.4], |c| c > 5),
            (0x8d919a, [1.1, 2.2], |c| c == 5),
            (0xb9a66a, [1.6, 3.2], |c| c == 3 || c == 4),
            (0xe0a84a, [2.2, 4.], |c| c <= 2),
        ];
        for (color, width, pick) in classes {
            for r in roads.iter().filter(|r| pick(r[0].as_u64().unwrap_or(9))) {
                mesh.line(&undelta(&r[2]), width, hex(color, 1.), false);
            }
        }
        for r in ov["rails"].as_array().into_iter().flatten() {
            mesh.line(
                &undelta(r),
                [1.2, 1.2],
                [40. / 255., 36. / 255., 32. / 255., 0.9],
                false,
            );
        }
        let border = index["border"].get(0).map(undelta).unwrap_or_default();
        if border.len() >= 3 {
            let (m, w, h) = (1e6, width, height);
            let outside = [
                Vec2::new(-m, -m),
                Vec2::new(w + m, -m),
                Vec2::new(w + m, h + m),
                Vec2::new(-m, h + m),
            ];
            mesh.fill(&outside, std::slice::from_ref(&border), [0., 0., 0., 0.55]);
        }
        for b in index["bezirke"].as_array().into_iter().flatten() {
            for r in b["r"].as_array().into_iter().flatten() {
                mesh.line(&undelta(r), [1., 1.], [1., 1., 1., 0.35], true);
            }
        }
        mesh.line(&border, [2.5, 2.5], hex(0xffd33d, 1.), true);
        let stations = ov["stations"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|s| {
                Some(Station {
                    at: Vec2::new(s[0].as_f64()? as f32, s[1].as_f64()? as f32),
                    cat: s[2].as_str()?.to_owned(),
                    name: s[3].as_str().unwrap_or("").to_owned(),
                })
            })
            .collect();
        Self {
            mesh,
            width,
            height,
            bezirke: point_labels(&ov["labels"]),
            ortsteile: point_labels(&ov["ortsteile"]),
            kieze: point_labels(&ov["kieze"]),
            stations,
            streets: street_points(ov),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn undelta_and_shapes() {
        let pts = undelta(&serde_json::json!([10, 20, 5, -5, -15, 0]));
        assert_eq!(
            pts,
            vec![Vec2::new(10., 20.), Vec2::new(15., 15.), Vec2::new(0., 15.)]
        );
        let mut m = OverlayMesh::default();
        let sq = [
            Vec2::ZERO,
            Vec2::new(10., 0.),
            Vec2::new(10., 10.),
            Vec2::new(0., 10.),
        ];
        let hole = vec![
            Vec2::new(3., 3.),
            Vec2::new(6., 3.),
            Vec2::new(6., 6.),
            Vec2::new(3., 6.),
        ];
        m.fill(&sq, &[hole], [1.; 4]);
        assert_eq!(m.indices.len(), 8 * 3, "Quadrat mit Loch = 8 Dreiecke");
        let v0 = m.vertices.len();
        m.line(&sq, [1., 2.], [1.; 4], false);
        assert_eq!(m.vertices.len() - v0, 3 * 4);
        let last = m.vertices.last().unwrap();
        assert_eq!(last.width, [1., 2.]);
        assert!((Vec2::from(last.offset).length() - 2f32.sqrt()).abs() < 1e-5);
    }
    #[test]
    fn real_overview_loads() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/data/berlin");
        let ov = Overview::read(&root).unwrap();
        assert_eq!(ov.bezirke.len(), 12);
        assert!(ov.ortsteile.len() > 90 && ov.kieze.len() > 500 && ov.stations.len() > 300);
        assert!(ov.mesh.indices.len() > 300_000 && ov.mesh.indices.len().is_multiple_of(3));
        assert!(
            ov.mesh
                .indices
                .iter()
                .all(|&i| (i as usize) < ov.mesh.vertices.len())
        );
        assert!(ov.width > 400_000. && ov.height > 300_000.);
    }
}
