//! Worker-side tessellation; the render thread only uploads completed batches.
use crate::{
    buildcolors::{rgb, roof_colors, wall_color},
    citycodes::{area_kind, hash01, roof_mat, unpack_look},
    format::{Building, Feature, Polygon, Road},
    geom::{Bounds, clip_ring, cum_lengths, edges, point_along_cum, signed_area},
    roofs::{self, Style},
};
use anyhow::{Result, ensure};
use glam::{Vec2, Vec3};
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub point: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 3],
    pub uv: [f32; 2],
    pub center: [f32; 2],
    pub material: f32,
    pub depth: f32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Sprite {
    pub point: [f32; 3],
    pub size: [f32; 2],
    pub angle: f32,
    pub color: [f32; 3],
    pub cell: f32,
    pub depth: f32,
}
/// Wand-Viereck für den Schattenwurf: Fußpunkt, Wandhöhe (bereits für die Schrägansicht gestaucht) und
/// `extrude` 0 = am Fuß, 1 = um die Schattenlänge versetzt (rechnet der Vertex-Shader aus dem Sonnenstand).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ShadowVertex {
    pub point: [f32; 2],
    pub height: f32,
    pub extrude: f32,
}
#[derive(Debug, Default, Clone)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub sprites: Vec<Sprite>,
    pub shadows: Vec<ShadowVertex>,
    pub shadow_indices: Vec<u32>,
}
impl Mesh {
    pub fn append(&mut self, other: &Self) {
        let base = self.vertices.len() as u32;
        self.vertices.extend_from_slice(&other.vertices);
        self.indices.extend(other.indices.iter().map(|i| base + i));
        self.sprites.extend_from_slice(&other.sprites);
        let base = self.shadows.len() as u32;
        self.shadows.extend_from_slice(&other.shadows);
        self.shadow_indices
            .extend(other.shadow_indices.iter().map(|i| base + i));
    }
    pub fn bytes(&self) -> usize {
        self.vertices.len() * size_of::<Vertex>()
            + self.indices.len() * 4
            + self.sprites.len() * size_of::<Sprite>()
            + self.shadows.len() * size_of::<ShadowVertex>()
            + self.shadow_indices.len() * 4
    }
    /// Schatten eines Prismas: jede Wand überstreicht beim Versetzen ein Viereck; zusammen mit dem Grundriss
    /// (den das Haus selbst bedeckt) ergibt die Vereinigung den ganzen Schatten (lighting.js addBuildingShadow).
    pub fn building_shadow(&mut self, rings: &[Vec<Vec2>], height: f32) {
        for ring in rings {
            for (a, b) in edges(ring) {
                if a == b {
                    continue;
                }
                let base = self.shadows.len() as u32;
                for (p, e) in [(a, 0.), (b, 0.), (b, 1.), (a, 1.)] {
                    self.shadows.push(ShadowVertex {
                        point: p.to_array(),
                        height,
                        extrude: e,
                    });
                }
                self.shadow_indices.extend_from_slice(&[
                    base,
                    base + 1,
                    base + 2,
                    base,
                    base + 2,
                    base + 3,
                ]);
            }
        }
    }
    fn polygon(
        &mut self,
        points: &[Vec2],
        surface: Surface,
        height: f32,
        center: Vec2,
        uv: impl Fn(Vec2) -> Vec2,
    ) {
        if points.len() < 3 {
            return;
        }
        let base = self.vertices.len() as u32;
        for &p in points {
            self.vertices.push(Vertex {
                point: [p.x, p.y, height],
                normal: surface.normal.to_array(),
                color: rgb(surface.color),
                uv: uv(p).to_array(),
                center: center.to_array(),
                material: surface.material,
                depth: surface.depth,
            });
        }
        for i in 1..points.len() - 1 {
            self.indices
                .extend_from_slice(&[base, base + i as u32, base + i as u32 + 1]);
        }
    }
    fn stroke(&mut self, points: &[Vec2], width: f32, surface: Surface) {
        if width <= 0. {
            return;
        }
        // Limited miter joins prevent cracks without producing long spikes at acute bends.
        let mut offsets = Vec::with_capacity(points.len());
        for i in 0..points.len() {
            let previous = points[i] - points[i.saturating_sub(1)];
            let next = points[(i + 1).min(points.len() - 1)] - points[i];
            let a = if previous.length_squared() > 0. {
                previous.normalize()
            } else {
                next.normalize_or_zero()
            };
            let b = if next.length_squared() > 0. {
                next.normalize()
            } else {
                a
            };
            let na = Vec2::new(-a.y, a.x);
            let nb = Vec2::new(-b.y, b.x);
            let m = (na + nb).normalize_or_zero();
            let denom = m.dot(nb).abs().max(0.25);
            offsets.push(m * (width * 0.5 / denom).min(width));
        }
        for i in 0..points.len().saturating_sub(1) {
            if points[i] == points[i + 1] {
                continue;
            }
            let a = points[i];
            let b = points[i + 1];
            self.polygon(
                &[
                    a - offsets[i],
                    b - offsets[i + 1],
                    b + offsets[i + 1],
                    a + offsets[i],
                ],
                surface,
                0.,
                Vec2::ZERO,
                |p| p,
            );
        }
    }
    fn circle(&mut self, p: Vec2, r: f32, surface: Surface) {
        let pts: Vec<_> = (0..24)
            .map(|i| p + Vec2::from_angle(i as f32 * std::f32::consts::TAU / 24.) * r)
            .collect();
        self.polygon(&pts, surface, 0., Vec2::ZERO, |p| p);
    }
}
#[derive(Clone, Copy)]
struct Surface {
    color: u32,
    material: f32,
    depth: f32,
    normal: Vec3,
}
impl Surface {
    fn ground(color: u32, material: f32, depth: f32) -> Self {
        Self {
            color,
            material,
            depth,
            normal: Vec3::Z,
        }
    }
}
/// Returns triangles grouped by outer rings, retaining courtyard/multipolygon holes.
pub fn triangulate(polygon: &Polygon) -> Result<Vec<[Vec2; 3]>> {
    ensure!(
        polygon.rings.len() == polygon.outer.len(),
        "Ringflags passen nicht"
    );
    let parents = crate::geom::ring_parents(&polygon.rings);
    let depths = crate::geom::ring_depths(&parents);
    let mut out = Vec::new();
    for (i, ring) in polygon.rings.iter().enumerate() {
        if !depths[i].is_multiple_of(2) {
            continue;
        }
        let origin = ring[0];
        let mut coords = Vec::<f64>::new();
        let mut points = Vec::new();
        let mut holes = Vec::new();
        let add = |r: &[Vec2], coords: &mut Vec<f64>, points: &mut Vec<Vec2>| {
            let r = if r.len() > 1 && r[0] == r[r.len() - 1] {
                &r[..r.len() - 1]
            } else {
                r
            };
            for &p in r {
                let local = p - origin;
                coords.extend([local.x as f64, local.y as f64]);
                points.push(p);
            }
        };
        add(ring, &mut coords, &mut points);
        for (j, hole) in polygon.rings.iter().enumerate() {
            if parents[j] == Some(i) {
                holes.push(points.len());
                add(hole, &mut coords, &mut points);
            }
        }
        let indices = earcutr::earcut(&coords, &holes, 2)
            .map_err(|e| anyhow::anyhow!("Triangulierung: {e:?}"))?;
        for tri in indices.as_chunks::<3>().0 {
            let triangle = [points[tri[0]], points[tri[1]], points[tri[2]]];
            if signed_area(&triangle).abs() > 0.001 {
                out.push(triangle);
            }
        }
    }
    ensure!(!out.is_empty(), "Polygon hat keine triangulierbare Fläche");
    Ok(out)
}
fn polygon_fill(
    mesh: &mut Mesh,
    polygon: &Polygon,
    surface: Surface,
    clip: Option<Bounds>,
) -> Result<()> {
    for tri in triangulate(polygon)? {
        let points = clip.map_or_else(|| tri.to_vec(), |b| clip_ring(&tri, b));
        mesh.polygon(&points, surface, 0., Vec2::ZERO, |p| p);
    }
    Ok(())
}
fn clip_convex(points: &[Vec2], clip: &[Vec2]) -> Vec<Vec2> {
    let mut points = points.to_vec();
    let sign = signed_area(clip).signum() as f32;
    for (a, b) in edges(clip) {
        let d = b - a;
        if d.length_squared() < 0.001 {
            continue;
        }
        let mut out = Vec::new();
        for (p, q) in edges(&points) {
            let vp = d.perp_dot(p - a) * sign;
            let vq = d.perp_dot(q - a) * sign;
            let pin = vp >= -0.001;
            let qin = vq >= -0.001;
            if pin != qin {
                out.push(p.lerp(q, vp / (vp - vq)));
            }
            if qin {
                out.push(q);
            }
        }
        points = out;
    }
    points
}
/// Material-ID einer Fassade (Textur im Shader, `data/gfx/material_map.json`): Putz 11/12, Klinker 18/19, Beton 20/23 –
/// je Wohnen/Arbeit, weil Arbeitsstätten ihren eigenen Lichttagesgang haben (`window_fs`).
pub fn facade_material(b: &Building, facade: roofs::Facade, workplace: bool) -> f32 {
    use crate::citycodes::wall_mat as wm;
    let lk = unpack_look(b.look);
    let (home, work) = match lk.wmat {
        wm::BRICK => (18., 19.),
        wm::CONCRETE | wm::GLASS | wm::METAL => (20., 23.),
        _ if matches!(facade, roofs::Facade::Platte | roofs::Facade::Industry) => (20., 23.),
        _ => (11., 12.),
    };
    if workplace { work } else { home }
}
/// Fassadendetails: Tür, Schaufenster, Ladenband (`scene.wgsl` zeichnet sie mit eigenen Ortskoordinaten).
pub const DOOR_MATERIAL: f32 = 13.;
pub const SHOPWINDOW_MATERIAL: f32 = 21.;
pub const SHOPSIGN_MATERIAL: f32 = 22.;
fn building_mesh(mesh: &mut Mesh, b: &Building, scale: f32) -> Result<()> {
    let style = roofs::roof_style(b, scale);
    let facade = roofs::facade_style(b);
    let wall = wall_color(b, facade);
    let (skin, center_color) = roof_colors(b, style, wall);
    let depth = 0.45 - b.center.y / 500000. * 0.2;
    // Schattenhöhe wie lighting.js buildingHeight: max(18, Höhe × heightScale 0,5)
    mesh.building_shadow(&b.polygon.rings, (b.height * 0.5).max(18.));
    let axis = roofs::oriented_box(b);
    // Arbeitsstätten (Büros, Schulen, Hallen) haben ihren eigenen Lichttagesgang (windows.js isWorkplace)
    let workplace = matches!(
        b.kind,
        crate::citycodes::building_kind::PUBLIC
            | crate::citycodes::building_kind::INDUSTRIAL
            | crate::citycodes::building_kind::WAREHOUSE
    );
    let wall_surface = Surface {
        color: wall,
        material: facade_material(b, facade, workplace),
        depth: depth + 0.00004,
        normal: Vec3::Z,
    };
    let wall_lines: Vec<_> = if let Some(walls) = &b.walls {
        walls
            .iter()
            .flat_map(|r| r.windows(2).map(|p| (p[0], p[1])))
            .collect()
    } else {
        b.polygon.rings.iter().flat_map(|r| edges(r)).collect()
    };
    for (a, c) in wall_lines {
        let d = c - a;
        let len = d.length();
        if len < 0.1 {
            continue;
        }
        let n = Vec3::new(d.y / len, -d.x / len, 0.);
        // Kontaktschatten am Fuß der Wand (grime.js drawContactShadows): weiches Band außen auf dem Boden
        // mittig auf der Wandlinie: innen deckt das Haus, außen bleibt der Schatten (unabhängig vom Umlaufsinn)
        let mid = (a + c) * 0.5;
        mesh.sprites.push(Sprite {
            point: [mid.x, mid.y, 0.],
            size: [len + 1.2 * scale, 3.2 * scale],
            angle: d.y.atan2(d.x),
            color: [1.; 3],
            cell: 9.,
            depth: 0.84,
        });
        let base = mesh.vertices.len() as u32;
        for (point, z, uv) in [
            (a, 0., [0., 0.]),
            (c, 0., [len, 0.]),
            (c, b.height, [len, b.height]),
            (a, b.height, [0., b.height]),
        ] {
            mesh.vertices.push(Vertex {
                point: [point.x, point.y, z],
                normal: n.to_array(),
                color: rgb(wall_surface.color),
                uv,
                center: b.center.to_array(),
                material: wall_surface.material,
                depth: wall_surface.depth,
            });
        }
        mesh.indices
            .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    let lk = unpack_look(b.look);
    let material = match lk.rmat {
        roof_mat::TILES => 6.,
        roof_mat::SLATE => 7.,
        roof_mat::METAL => 8.,
        roof_mat::GREEN => 4.,
        roof_mat::GLASS => 0.,
        _ => {
            if style.pitched() || style == Style::Berlin {
                6.
            } else if style == Style::Corrugated {
                8.
            } else {
                9.
            }
        }
    };
    let triangles = triangulate(&b.polygon)?;
    for tri in &triangles {
        let base = Surface {
            color: center_color,
            material: if style == Style::Berlin || style == Style::Mansard {
                9.
            } else {
                material
            },
            depth,
            normal: Vec3::Z,
        };
        if style == Style::Dome || style == Style::Round {
            let start = mesh.vertices.len();
            mesh.polygon(tri, base, b.height, b.center, |p| axis.local(p));
            let radius = ((axis.max - axis.min).min_element() * 0.46).max(1.);
            for v in &mut mesh.vertices[start..] {
                let p = Vec2::new(v.point[0], v.point[1]);
                let local = (p - b.center) / radius;
                let z = (1. - local.length_squared()).max(0.05).sqrt();
                v.normal = Vec3::new(local.x, local.y, z).normalize().to_array();
            }
        } else {
            mesh.polygon(tri, base, b.height, b.center, |p| axis.local(p));
        }
    }
    // Fassadendetails (Material 13: z = Anteil der projizierten Wandhöhe, uv.y = Gebäudehöhe; die Schrägansicht
    // hebt jeden Punkt über dem Boden um mindestens 18 px, so liegen Tür und Ladenfront dennoch am Fuß der Wand)
    let full_h = (b.height * 0.5).max(18.);
    let frac = |px: f32| (px / full_h).clamp(0., 1.);
    // uv.x = Abstand vom linken Rand des Details (px), uv.y = Gebäudehöhe (Schrägansicht im Vertex-Shader)
    let wall_quad = |mesh: &mut Mesh,
                     a: Vec2,
                     c: Vec2,
                     u0: f32,
                     u1: f32,
                     z0: f32,
                     z1: f32,
                     color: u32,
                     d: f32,
                     material: f32| {
        let dir = c - a;
        let len = dir.length();
        let base = mesh.vertices.len() as u32;
        for (u, z) in [(u0, z0), (u1, z0), (u1, z1), (u0, z1)] {
            let p = a + dir * u;
            mesh.vertices.push(Vertex {
                point: [p.x, p.y, z],
                normal: [0., 0., 1.],
                color: rgb(color),
                uv: [(u - u0) * len, b.height],
                center: b.center.to_array(),
                material,
                depth: d,
            });
        }
        mesh.indices
            .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    };
    let door_depth = depth + 0.00004 - 0.00001;
    for &(ri, ei, t) in &b.doors {
        let Some(ring) = b.polygon.rings.get(ri) else {
            continue;
        };
        if ring.len() < 2 {
            continue;
        }
        let (a, c) = (ring[ei % ring.len()], ring[(ei + 1) % ring.len()]);
        let len = (c - a).length();
        if len < 14. {
            continue;
        }
        let half = 6. / len;
        let top = frac((22f32).min(full_h * 0.4));
        wall_quad(
            mesh,
            a,
            c,
            (t - half).max(0.),
            (t + half).min(1.),
            0.,
            top,
            0x3b2a1e,
            door_depth,
            DOOR_MATERIAL,
        );
    }
    if b.kind == crate::citycodes::building_kind::SPAETI {
        // Ladenfront an der Wand, die am stärksten zur Kamera (Süden) zeigt: Schaufenster und rotes Band
        let mut front: Option<(Vec2, Vec2, f32)> = None;
        for ring in &b.polygon.rings {
            for (a, c) in edges(ring) {
                let d = c - a;
                let len = d.length();
                if len < 20. {
                    continue;
                }
                let mut n = Vec2::new(d.y, -d.x) / len;
                if roofdecor_point_inside((a + c) * 0.5 + n * 2., &b.polygon.rings) {
                    n = -n;
                }
                let score = n.y * len;
                if front.is_none_or(|f| score > f.2) {
                    front = Some((a, c, score));
                }
            }
        }
        if let Some((a, c, _)) = front {
            let len = (c - a).length();
            let w = len.min(120.);
            let u0 = (len - w) / 2.;
            let (ua, ub) = (u0 / len, (u0 + w) / len);
            wall_quad(
                mesh,
                a,
                c,
                (u0 + 6.) / len,
                (u0 + w - 24.) / len,
                frac(2.),
                frac(15.),
                0x9fd3ff,
                door_depth,
                SHOPWINDOW_MATERIAL,
            );
            wall_quad(
                mesh,
                a,
                c,
                ua,
                ub,
                frac(15.),
                frac(27.),
                0xe03b3b,
                door_depth,
                SHOPSIGN_MATERIAL,
            );
        }
    }
    for facet in roofs::roof_facets(b, style, scale) {
        if signed_area(&facet.points).abs() < 0.01 {
            continue;
        }
        let slope = if style == Style::Mansard { 1.2 } else { 0.65 };
        let normal = Vec3::new(facet.outward.x * slope, facet.outward.y * slope, 1.).normalize();
        let surface = Surface {
            color: skin,
            material,
            depth: depth - 0.00002,
            normal,
        };
        for tri in &triangles {
            let clipped = clip_convex(tri, &facet.points);
            if signed_area(&clipped).abs() > 0.01 {
                mesh.polygon(&clipped, surface, b.height, b.center, |p| axis.local(p));
            }
        }
    }
    // Dachaufbauten und Gauben (roofs.js roofDecor)
    for d in crate::roofdecor::roof_decor(b, style, scale) {
        use crate::roofdecor::Kind;
        let (ax, ay) = (d.angle.cos(), d.angle.sin());
        let (u, v) = (Vec2::new(ax, ay), Vec2::new(-ay, ax));
        let rect = |hx: f32, hy: f32, off: Vec2| {
            let c = d.center + off;
            vec![
                c - u * hx - v * hy,
                c + u * hx - v * hy,
                c + u * hx + v * hy,
                c - u * hx + v * hy,
            ]
        };
        let flat = |mesh: &mut Mesh, pts: &[Vec2], color: u32, dd: f32, normal: Vec3| {
            mesh.polygon(
                pts,
                Surface {
                    color,
                    material: 0.,
                    depth: depth - dd,
                    normal,
                },
                b.height,
                b.center,
                |p| p,
            );
        };
        if d.kind == Kind::Dormer {
            // zwei Dachhälften zur Seite geneigt, Stirnseite in Fassadenfarbe mit Fenster zur Traufe
            let (hx, hy) = (d.half.x, d.half.y);
            let tilt = |s: f32| Vec3::new(u.x * s * 0.6, u.y * s * 0.6, 1.).normalize();
            flat(
                mesh,
                &rect(hx / 2., hy, -u * hx / 2.),
                skin,
                0.00004,
                tilt(-1.),
            );
            flat(
                mesh,
                &rect(hx / 2., hy, u * hx / 2.),
                skin,
                0.00004,
                tilt(1.),
            );
            let s = if d.out.dot(v) >= 0. { 1. } else { -1. };
            flat(
                mesh,
                &rect(hx, 1.2, v * s * (hy - 1.2)),
                wall,
                0.00005,
                Vec3::Z,
            );
            flat(
                mesh,
                &rect(hx * 0.5, 0.8, v * s * (hy - 1.)),
                0x39414d,
                0.00006,
                Vec3::Z,
            );
            continue;
        }
        let (outer, inner) = d.kind.colors();
        let k = match d.kind {
            Kind::Chimney => 0.55,
            Kind::Solar | Kind::Terrace => 0.86,
            _ => 0.7,
        };
        // Schatten zur Seite, Rand, Innenfläche
        flat(
            mesh,
            &rect(d.half.x, d.half.y, Vec2::new(1.2, 1.2)),
            0x2a2a2a,
            0.00003,
            Vec3::Z,
        );
        flat(
            mesh,
            &rect(d.half.x, d.half.y, Vec2::ZERO),
            outer,
            0.00004,
            Vec3::Z,
        );
        flat(
            mesh,
            &rect(d.half.x * k, d.half.y * k, Vec2::ZERO),
            inner,
            0.00005,
            Vec3::Z,
        );
    }
    Ok(())
}
fn roofdecor_point_inside(p: Vec2, rings: &[Vec<Vec2>]) -> bool {
    let mut inside = false;
    for r in rings {
        for (a, b) in edges(r) {
            if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
                inside = !inside;
            }
        }
    }
    inside
}
/// Bordstein zwischen Fahrbahn und Gehweg: 30 cm heller Granit (Körnung der Asphalttextur in hellem Ton), zur Fahrbahn
/// hin eine dunkle Fuge. Liegt hinter der Fahrbahn und vor dem Gehweg – sichtbar bleibt nur der Rand.
fn curb(mesh: &mut Mesh, points: &[Vec2], width: f32, depth: f32, scale: f32) {
    mesh.stroke(
        points,
        width + 2. * CURB_M * scale,
        Surface::ground(0xbdbab2, 1., depth + 0.01),
    );
    mesh.stroke(
        points,
        width + 2. * 0.05 * scale,
        Surface::ground(0x3d3b37, 0., depth + 0.005),
    );
}
/// Kreuzungsfläche (`Feature::Plate`): Asphalt im Umriss (die Eckzüge aneinandergereiht) im Belag der Straßen,
/// vor deren Enden; entlang jedes Eckzugs dieselben Bänder wie an den Straßen (Randstreifen, Gehweg, Bordstein,
/// Rinne), mittig auf der Fahrbahnkante – die innere Hälfte verdeckt der Asphalt, die äußere ist der Gehweg um die Ecke.
fn plate_mesh(
    mesh: &mut Mesh,
    corners: &[Vec<Vec2>],
    fill: Option<&[Vec2]>,
    level: i8,
    surface: u8,
    scale: f32,
) {
    let depth = if level > 0 {
        0.68 - level as f32 * 0.03
    } else if level < 0 {
        0.935
    } else {
        0.85
    };
    let sidewalk = 2. * scale;
    for line in corners {
        mesh.stroke(
            line,
            2. * (sidewalk + 0.12 * scale),
            Surface::ground(0x85827a, 3., depth + 0.031),
        );
        mesh.stroke(
            line,
            2. * sidewalk,
            Surface::ground(0xa8a59d, 3., depth + 0.03),
        );
        curb(mesh, line, 0., depth, scale);
    }
    let mut ring: Vec<Vec2> = Vec::new();
    let outline: Box<dyn Iterator<Item = &Vec2>> = match fill {
        Some(f) => Box::new(f.iter()),
        None => Box::new(corners.iter().flatten()),
    };
    for p in outline {
        if ring.last().is_none_or(|q| q.distance(*p) > 0.05) {
            ring.push(*p);
        }
    }
    if ring.len() > 2 && ring[0].distance(ring[ring.len() - 1]) < 0.05 {
        ring.pop();
    }
    let (color, material) = road_surface(surface);
    let polygon = Polygon {
        rings: vec![ring],
        outer: vec![true],
    };
    // entartet (sollte der Kartenbau ausschließen): nur die Bänder, kein Asphalt
    let _ = polygon_fill(
        mesh,
        &polygon,
        Surface::ground(color, material, depth - 0.0001),
        None,
    );
}
/// Wie weit Längsmarkierungen vor den Enden einer (an Kreuzungsflächen schon gekürzten) Straße aufhören (px, vorn und
/// hinten): an einer Kreuzung 2 m vor der Mündung (dort steht ggf. die Haltlinie), an einer bloßen Teilung der Straße
/// (OSM-Weg endet, die Straße läuft weiter) gar nicht – sonst rissen die Linien mitten auf der Straße ab.
fn end_gaps(r: &Road, scale: f32) -> (f32, f32) {
    let gap = |t: f32| if t > 0. { 2. * scale } else { 0. };
    (gap(r.trim[0]), gap(r.trim[1]))
}

/// Querlage (px, + rechts der Zeichenrichtung) von linkem und rechtem Fahrbahnrand ohne Park-, Radstreifen und
/// Bordsteinradweg – wie street.js laneOffsets.
fn lane_edges(r: &Road) -> (f32, f32) {
    (
        -r.width / 2. + r.park_width[0] + r.cycle[0] + r.track[0],
        r.width / 2. - r.park_width[1] - r.cycle[1] - r.track[1],
    )
}

/// Bekommt die Straße Längsmarkierungen? Berlin-typisch: Hauptstraßen (bis Klasse 5, tertiär) und breite Fahrbahnen
/// (ab 7,5 m ohne Park- und Radstreifen); Wohnstraßen, Pflaster und Spielstraßen bleiben unmarkiert.
pub fn has_lane_lines(r: &Road, scale: f32) -> bool {
    let (x_l, x_r) = lane_edges(r);
    r.surface == 0
        && r.forward + r.backward >= 2
        && (r.class <= 5 || (r.class <= 7 && x_r - x_l >= 7.5 * scale))
}

/// Längsmarkierung: Leitlinie zwischen den Fahrtrichtungen (3 m Strich, 6 m Lücke), auf vierstreifigen Hauptstraßen
/// als durchgezogene Fahrstreifenbegrenzung; zwischen Fahrstreifen gleicher Richtung Leitlinien.
fn lane_lines(mesh: &mut Mesh, r: &Road, cum: &[f32], length: f32, depth: f32, scale: f32) {
    if !has_lane_lines(r, scale) {
        return;
    }
    let (x_l, x_r) = lane_edges(r);
    let lanes = r.forward + r.backward;
    let lw = (x_r - x_l) / lanes as f32;
    let (g0, g1) = end_gaps(r, scale);
    let color = rgb(0xe3dfc9);
    let dashed = |mesh: &mut Mesh, off: f32| {
        // Strich beginnt 1 m hinter der Lücke, damit er an der Mündung nicht als Stummel endet
        let mut s = g0 + 1. * scale;
        while s + 3. * scale <= length - g1 {
            if let Some(p) = point_along_cum(&r.points, cum, s + 1.5 * scale) {
                let pos = p.point + Vec2::new(-p.direction.y, p.direction.x) * off;
                mesh.sprites.push(Sprite {
                    point: [pos.x, pos.y, 0.],
                    size: [3. * scale, 0.12 * scale],
                    angle: p.direction.y.atan2(p.direction.x),
                    color,
                    cell: 7.,
                    depth: depth - 0.001,
                });
            }
            s += 9. * scale;
        }
    };
    for k in 1..lanes {
        let off = x_l + k as f32 * lw;
        let divider = r.forward > 0 && r.backward > 0 && k == r.backward;
        if divider && r.class <= 4 && r.forward >= 2 && r.backward >= 2 {
            let part = crate::geom::trim_polyline(&r.points, g0, g1);
            if part.len() >= 2 {
                let line = crate::geom::offset_polyline(&part, off);
                mesh.stroke(
                    &line,
                    0.14 * scale,
                    Surface::ground(0xe3dfc9, 0., depth - 0.001),
                );
            }
        } else {
            dashed(mesh, off);
        }
    }
}

/// Überlappung der Straße unter ihrer Kreuzungsfläche (px)
const SEAM_PX: f32 = 2.;
/// Farbe und Material der Fahrbahn je Belag (0 Asphalt, 1 Pflaster, 2 Platten, 3 unbefestigt).
fn road_surface(surface: u8) -> (u32, f32) {
    match surface {
        1 => (0x72716b, 2.),
        2 => (0x949087, 3.),
        3 => (0x9b9276, 10.),
        _ => (0x454d50, 1.),
    }
}
/// Breite des Bordsteins in Metern.
pub const CURB_M: f32 = 0.3;
/// Material-ID des Rasenrands (Streifen entlang von Rasenflächen, ausgefranst; `center.x` = Lage quer 0 außen … 1 innen).
pub const FRINGE_MATERIAL: f32 = 16.;
/// Rasenrand: halber Streifen innen, halber außen über dem Nachbarn (Hintergrund, Platz, Sand) – nie über Straßen
/// und Gehwegen (die liegen weiter vorn).
fn fringe(
    mesh: &mut Mesh,
    polygon: &Polygon,
    color: u32,
    depth: f32,
    half: f32,
    clip: Option<Bounds>,
) {
    for (ring, &outer) in polygon.rings.iter().zip(&polygon.outer) {
        // Innen der Fläche: links der Laufrichtung bei positivem Umlauf (Außenring), bei Löchern umgekehrt
        let sign = if (signed_area(ring) > 0.) == outer {
            1.
        } else {
            -1.
        };
        for (a, b) in edges(ring) {
            let d = b - a;
            if d.length_squared() < 0.01 {
                continue;
            }
            let n = Vec2::new(-d.y, d.x).normalize() * sign;
            let quad = [a - n * half, b - n * half, b + n * half, a + n * half];
            let points = clip.map_or_else(|| quad.to_vec(), |c| clip_ring(&quad, c));
            if points.len() < 3 {
                continue;
            }
            let base = mesh.vertices.len() as u32;
            for p in &points {
                let across = (0.5 + (*p - a).dot(n) / (2. * half)).clamp(0., 1.);
                mesh.vertices.push(Vertex {
                    point: [p.x, p.y, 0.],
                    normal: [0., 0., 1.],
                    color: rgb(color),
                    uv: p.to_array(),
                    center: [across, 0.],
                    material: FRINGE_MATERIAL,
                    depth,
                });
            }
            for i in 1..points.len() - 1 {
                mesh.indices
                    .extend_from_slice(&[base, base + i as u32, base + i as u32 + 1]);
            }
        }
    }
}
fn road_mesh(mesh: &mut Mesh, r: &Road, scale: f32) {
    if r.passage {
        return;
    }
    // an Kreuzungsflächen gekürzt: die Fläche übernimmt Fahrbahn, Bordstein und Gehweg um die Ecke
    let trimmed;
    let r = if r.trim[0] > 0. || r.trim[1] > 0. {
        trimmed = Road {
            // 2 px unter die Fläche (sie liegt davor): die Flächenecken sind auf ganze px gerundet, ohne Überlappung
            // schien durch den Haarriss an der Mündung der Rasen
            points: crate::geom::trim_polyline(
                &r.points,
                (r.trim[0] - SEAM_PX).max(0.),
                (r.trim[1] - SEAM_PX).max(0.),
            ),
            ..r.clone()
        };
        if trimmed.points.len() < 2 {
            return;
        }
        &trimmed
    } else {
        r
    };
    let depth = if r.level > 0 {
        0.68 - r.level as f32 * 0.03
    } else if r.level < 0 {
        0.935
    } else {
        0.85
    };
    let sidewalk = if r.class <= 9 { 2. * scale } else { 0. };
    if sidewalk > 0. {
        // Randstreifen außen am Gehweg (Kantensteine, Schmutz in der Fuge zum Boden), dahinter
        mesh.stroke(
            &r.points,
            r.width + 2. * sidewalk + 2. * 0.12 * scale,
            Surface::ground(0x85827a, 3., depth + 0.031),
        );
    }
    mesh.stroke(
        &r.points,
        r.width + 2. * sidewalk,
        Surface::ground(0xa8a59d, 3., depth + 0.03),
    );
    if sidewalk > 0. {
        curb(mesh, &r.points, r.width, depth, scale);
    }
    let (color, material) = road_surface(r.surface);
    mesh.stroke(&r.points, r.width, Surface::ground(color, material, depth));
    if r.fill > 0. {
        let shifted = crate::geom::offset_polyline(&r.points, r.width * 0.5 + r.fill * 0.5);
        mesh.stroke(&shifted, r.fill, Surface::ground(color, material, depth));
    }
    let cum = cum_lengths(&r.points);
    let length = *cum.last().unwrap_or(&0.);
    let margin = r.width * 0.5 + 3. * scale;
    // Parking and cycle lanes follow the stored street cross-section.
    for side in 0..2 {
        let sign = if side == 0 { -1. } else { 1. };
        let inset = r.park_width[side] + r.cycle[side] + r.track[side];
        if inset > 0. {
            let off = sign * (r.width * 0.5 - inset);
            let shifted = crate::geom::offset_polyline(&r.points, off);
            mesh.stroke(
                &shifted,
                0.1 * scale,
                Surface::ground(0xb7b6a8, 0., depth - 0.0003),
            );
        }
    }
    // Fahrradpiktogramme auf Radfahrstreifen, alle 30 m, in Fahrtrichtung des Streifens
    for side in 0..2 {
        let w = r.cycle[side];
        if w < 0.9 * scale {
            continue;
        }
        let sign = if side == 0 { -1. } else { 1. };
        let off = sign * (r.width * 0.5 - r.park_width[side] - w * 0.5);
        let size = (1.6 * scale).min(w * 1.8);
        let mut s = margin + 6. * scale;
        while s < length - margin {
            if let Some(p) = point_along_cum(&r.points, &cum, s) {
                let pos = p.point + Vec2::new(-p.direction.y, p.direction.x) * off;
                // rechts gefahren: links liegender Streifen zeigt gegen die Zeichenrichtung
                let angle = p.direction.y.atan2(p.direction.x)
                    + if side == 0 { std::f32::consts::PI } else { 0. };
                mesh.sprites.push(Sprite {
                    point: [pos.x, pos.y, 0.],
                    size: [size, size],
                    angle,
                    color: rgb(0xe3dfc9),
                    cell: 11.,
                    depth: depth - 0.0009,
                });
            }
            s += 30. * scale;
        }
    }
    lane_lines(mesh, r, &cum, length, depth, scale);
    // Ölband in der Mitte jedes Fahrstreifens (grime.js laneWear): weiche, überlappende Stempel
    let lanes = (r.forward + r.backward) as usize;
    if r.class <= 8 && lanes > 0 && r.surface == 0 {
        let left = r.park_width[0] + r.cycle[0] + r.track[0];
        let right = r.park_width[1] + r.cycle[1] + r.track[1];
        let inner = r.width - left - right;
        if inner > 2. * scale {
            let step = 2.4 * scale;
            for i in 0..lanes {
                let off = -r.width / 2. + left + (i as f32 + 0.5) * inner / lanes as f32;
                let (g0, g1) = end_gaps(r, scale);
                let mut s = g0;
                while s < length - g1 {
                    if let Some(p) = point_along_cum(&r.points, &cum, s) {
                        let pos = p.point + Vec2::new(-p.direction.y, p.direction.x) * off;
                        mesh.sprites.push(Sprite {
                            point: [pos.x, pos.y, 0.],
                            size: [step * 1.6, (inner / lanes as f32 * 0.45).min(1.4 * scale)],
                            angle: p.direction.y.atan2(p.direction.x),
                            color: [1.; 3],
                            cell: 8.,
                            depth: depth - 0.0006,
                        });
                    }
                    s += step;
                }
            }
        }
    }
    // Deterministic road wear with the source module's spacing, dimensions and cap.
    if r.class <= 8 && !r.bridge {
        let mut count = 0;
        let seed = (r.id as u32).wrapping_mul(2654435761);
        for (cell, every, size) in [
            (2., 45., Vec2::splat(0.7)),
            (3., if r.class <= 5 { 90. } else { 45. }, Vec2::new(3., 1.4)),
            (4., 70., Vec2::new(3.5, 0.5)),
            (1., 25., Vec2::new(0.6, 0.4)),
        ] {
            if r.surface == 1 && [3., 4.].contains(&cell) {
                continue;
            }
            let n = (length / (every * scale)).floor() as u32;
            for i in 0..n {
                let s =
                    (i as f32 + 0.2 + hash01(seed.wrapping_add(i * 13 + cell as u32)) as f32 * 0.6)
                        * length
                        / n.max(1) as f32;
                if s < margin || s > length - margin || count >= 120 {
                    continue;
                }
                let Some(p) = point_along_cum(&r.points, &cum, s) else {
                    continue;
                };
                for side in if cell == 1. { vec![-1., 1.] } else { vec![0.] } {
                    let offset = if cell == 1. {
                        side * (r.width * 0.5 - 0.35 * scale)
                    } else {
                        (hash01(seed.wrapping_add(i + 991)) as f32 - 0.5)
                            * (r.width - r.park_width.iter().sum::<f32>())
                            * 0.5
                    };
                    let pos = p.point + Vec2::new(-p.direction.y, p.direction.x) * offset;
                    mesh.sprites.push(Sprite {
                        point: [pos.x, pos.y, 0.],
                        size: (size * scale).to_array(),
                        angle: p.direction.y.atan2(p.direction.x),
                        color: [1.; 3],
                        cell,
                        depth: depth - 0.0008,
                    });
                    count += 1;
                }
            }
        }
        for side in 0..2 {
            if r.park[side] != 1 && r.park[side] != 2 {
                continue;
            }
            let n = (length / (14. * scale)) as u32;
            for i in 0..n {
                if count >= 120 || hash01(seed.wrapping_add(i + side as u32 * 819)) >= 0.5 {
                    continue;
                }
                let s = (i as f32 + 0.5) * 14. * scale;
                if s < margin || s > length - margin {
                    continue;
                }
                if let Some(p) = point_along_cum(&r.points, &cum, s) {
                    let off = (if side == 0 { -1. } else { 1. })
                        * (r.width * 0.5 - r.park_width[side] * 0.5);
                    let pos = p.point + Vec2::new(-p.direction.y, p.direction.x) * off;
                    mesh.sprites.push(Sprite {
                        point: [pos.x, pos.y, 0.],
                        size: [scale, 0.7 * scale],
                        angle: hash01(seed.wrapping_add(i + 511)) as f32 * std::f32::consts::PI,
                        color: [1.; 3],
                        cell: 5.,
                        depth: depth - 0.0008,
                    });
                    count += 1;
                }
            }
        }
    }
}
pub fn prepare(feature: &Feature, scale: f32) -> Result<Mesh> {
    let mut mesh = Mesh::default();
    match feature {
        Feature::Building(b) => building_mesh(&mut mesh, b, scale)?,
        Feature::Water { polygon, clip } => polygon_fill(
            &mut mesh,
            polygon,
            Surface::ground(0x466f78, 5., 0.96),
            *clip,
        )?,
        Feature::Area {
            polygon,
            kind,
            level,
            clip,
        } => {
            let (color, material) = match *kind {
                area_kind::RAIL => (0x777365, 10.),
                area_kind::PLAZA => (0xa39f94, 3.),
                area_kind::ALLOTMENTS => (0x648b50, 4.),
                area_kind::CEMETERY => (0x5b8a47, 4.),
                area_kind::GRASS => (0x5d9340, 4.),
                area_kind::PITCH => (0x4d8c3c, 4.),
                area_kind::SAND => (0xd6c48d, 10.),
                area_kind::WOOD => (0x425e3d, 4.),
                area_kind::BRIDGE => (0x858780, 3.),
                _ => (0x6a8555, 4.),
            };
            let depth = if *kind == area_kind::BRIDGE {
                0.70 - *level as f32 * 0.03
            } else {
                0.975
            };
            polygon_fill(
                &mut mesh,
                polygon,
                Surface::ground(color, material, depth),
                *clip,
            )?;
            // weicher Rand: Rasen franst über den Nachbarn (Hintergrund, Platz, Sand) aus
            if material == 4. && *kind != area_kind::BRIDGE {
                fringe(
                    &mut mesh,
                    polygon,
                    color,
                    depth - 0.0005,
                    0.6 * scale,
                    *clip,
                );
            }
        }
        Feature::Road(r) => road_mesh(&mut mesh, r, scale),
        Feature::Junction { plated: true, .. } => {}
        Feature::Plate {
            corners,
            fill,
            level,
            surface,
        } => plate_mesh(&mut mesh, corners, fill.as_deref(), *level, *surface, scale),
        Feature::Junction {
            point,
            radius,
            level,
            cobble,
            plated: false,
        } => {
            let depth = if *level > 0 {
                0.68 - *level as f32 * 0.03
            } else if *level < 0 {
                0.935
            } else {
                0.85
            };
            mesh.circle(
                *point,
                *radius + 2. * scale,
                Surface::ground(0xa8a59d, 3., depth + 0.03),
            );
            mesh.circle(
                *point,
                *radius + CURB_M * scale,
                Surface::ground(0xbdbab2, 1., depth + 0.01),
            );
            mesh.circle(
                *point,
                *radius + 0.05 * scale,
                Surface::ground(0x3d3b37, 0., depth + 0.005),
            );
            mesh.circle(
                *point,
                *radius,
                Surface::ground(
                    if *cobble { 0x72716b } else { 0x454d50 },
                    if *cobble { 2. } else { 1. },
                    depth - 0.0001,
                ),
            );
        }
        Feature::Line {
            points,
            kind,
            level,
            hidden,
        } => {
            if !hidden {
                let depth = if *level > 0 {
                    0.67 - *level as f32 * 0.03
                } else {
                    0.82
                };
                match kind {
                    0 => mesh.stroke(
                        points,
                        1.8 * scale,
                        Surface::ground(0xaaa38d, 3., depth + 0.08),
                    ),
                    1 => {
                        mesh.stroke(points, 2.8 * scale, Surface::ground(0x6f6c62, 10., depth));
                        for off in [-0.72 * scale, 0.72 * scale] {
                            let shifted = crate::geom::offset_polyline(points, off);
                            mesh.stroke(
                                &shifted,
                                0.12 * scale,
                                Surface::ground(0xb7b4a7, 0., depth - 0.001),
                            );
                        }
                    }
                    _ => mesh.stroke(
                        points,
                        0.18 * scale,
                        Surface::ground(0x776b54, 0., depth - 0.01),
                    ),
                }
            }
        }
        Feature::Marks { level, marks } => {
            let depth = if *level > 0 {
                0.68 - *level as f32 * 0.03
            } else if *level < 0 {
                0.935
            } else {
                0.85
            };
            for m in marks {
                mesh.sprites.push(Sprite {
                    point: [m.center.x, m.center.y, 0.],
                    size: m.size.to_array(),
                    angle: m.angle,
                    color: rgb(0xe3dfc9),
                    cell: 7.,
                    depth: depth - 0.0012,
                });
            }
        }
        Feature::Tree {
            point,
            radius,
            seed,
            genus,
        } => {
            let (cell, color) = tree_look(*genus, *seed);
            mesh.sprites.push(Sprite {
                point: [point.x, point.y, 0.],
                size: [radius * 2., radius * 2.],
                angle: hash01(*seed) as f32 * std::f32::consts::TAU,
                color: rgb(color),
                cell,
                depth: 0.44 - point.y / 500000. * 0.2,
            });
            // Laub unter etwa jedem dritten Laubbaum (am Boden, vor Straßen und Gehwegen)
            // (einige 2,6-m-Flecken unter der Krone; ein Blatt misst so 10–15 cm)
            if *genus != 16 && hash01(seed.wrapping_mul(31).wrapping_add(7)) < 0.4 {
                for k in 0..4u32 {
                    let h = |i: u32| hash01(seed.wrapping_add(17 + k * 7 + i)) as f32;
                    let off =
                        Vec2::from_angle(h(0) * std::f32::consts::TAU) * radius * 0.7 * h(1).sqrt();
                    mesh.sprites.push(Sprite {
                        point: [point.x + off.x, point.y + off.y, 0.],
                        size: [2.6 * scale, 2.6 * scale],
                        angle: h(2) * std::f32::consts::TAU,
                        color: [1.; 3],
                        cell: 10.,
                        depth: 0.846,
                    });
                }
            }
        }
    }
    Ok(mesh)
}
/// Kronenbild (Atlaszelle, engine/atlas.rs) und Laubfarbe einer Baumgattung (`citycodes::TREE_GENERA`): Linde,
/// Platane und Kastanie – die häufigsten Straßenbäume Berlins – haben eigene Kronen; Nadelbäume sind meist Kiefern,
/// der Rest bekommt die allgemeine Nadelkrone; alle anderen Laubbäume die allgemeine Laubkrone in vier Grüntönen.
pub fn tree_look(genus: u8, seed: u32) -> (f32, u32) {
    match genus {
        1 => (12., [0x5b7b44, 0x62824a][seed as usize % 2]),
        3 => (13., [0x6f8d54, 0x7a9358][seed as usize % 2]),
        4 => (14., [0x48693a, 0x51723f][seed as usize % 2]),
        16 if hash01(seed.wrapping_mul(13).wrapping_add(5)) < 0.65 => (15., 0x45644a),
        16 => (6., 0x4a6952),
        _ => (
            0.,
            [0x557547, 0x63814d, 0x748955, 0x4e7044][seed as usize % 4],
        ),
    }
}
#[cfg(test)]
mod tests {
    fn street(class: u8, width_m: f32, surface: u8, trim: [f32; 2]) -> Road {
        Road {
            id: 1,
            points: vec![Vec2::ZERO, Vec2::new(1000., 0.)],
            width: width_m * 10.,
            class,
            level: 0,
            bridge: false,
            passage: false,
            surface,
            forward: 1,
            backward: 1,
            park: [0; 2],
            park_width: [0.; 2],
            cycle: [0.; 2],
            track: [0.; 2],
            fill: 0.,
            trim,
        }
    }

    #[test]
    fn lane_lines_berlin_style_and_end_before_the_junction() {
        let s = 10.;
        // Hauptstraße ja, schmale Wohnstraße nein, breite Wohnstraße ja, Pflaster nie
        assert!(has_lane_lines(&street(4, 7., 0, [0.; 2]), s));
        assert!(!has_lane_lines(&street(7, 6.5, 0, [0.; 2]), s));
        assert!(has_lane_lines(&street(7, 8., 0, [0.; 2]), s));
        assert!(!has_lane_lines(&street(4, 9., 1, [0.; 2]), s));
        // Parkstreifen zählen nicht zur Fahrbahnbreite
        let mut parked = street(7, 9., 0, [0.; 2]);
        parked.park_width = [20., 20.];
        assert!(!has_lane_lines(&parked, s));
        // Lücke nur an Kreuzungsenden (gekürzt), nicht an bloßen Teilungen
        assert_eq!(end_gaps(&street(4, 7., 0, [40., 0.]), s), (20., 0.));
        // Striche bleiben innerhalb der Lücken und haben den 9-m-Takt
        let r = street(4, 7., 0, [40., 40.]);
        let mut mesh = Mesh::default();
        let cum = cum_lengths(&r.points);
        lane_lines(&mut mesh, &r, &cum, 1000., 0.85, s);
        let xs: Vec<f32> = mesh.sprites.iter().map(|q| q.point[0]).collect();
        assert!(!xs.is_empty());
        assert!(
            xs.iter()
                .all(|&x| x - 15. >= 20. - 1e-3 && x + 15. <= 980. + 1e-3),
            "{xs:?}"
        );
        assert!(xs.windows(2).all(|w| (w[1] - w[0] - 90.).abs() < 1e-3));
    }

    use super::*;
    use crate::geom::point_in_ring;

    #[test]
    fn street_trees_get_their_own_crowns() {
        let g = |name: &str| {
            crate::citycodes::TREE_GENERA
                .iter()
                .position(|n| *n == name)
                .unwrap() as u8
        };
        assert_eq!(tree_look(g("Tilia"), 1).0, 12.);
        assert_eq!(tree_look(g("Platanus"), 1).0, 13.);
        assert_eq!(tree_look(g("Aesculus"), 1).0, 14.);
        assert_eq!(tree_look(g("Acer"), 1).0, 0.);
        let pines = (0..200)
            .filter(|s| tree_look(g("Nadel"), *s).0 == 15.)
            .count();
        assert!((100..170).contains(&pines), "Kiefern {pines} von 200");
        assert!((0..200).all(|s| matches!(tree_look(g("Nadel"), s).0, 6. | 15.)));
    }
    #[test]
    fn grass_fringe_frays_outwards_whatever_the_winding() {
        for ring in [
            vec![
                Vec2::ZERO,
                Vec2::new(100., 0.),
                Vec2::new(100., 100.),
                Vec2::new(0., 100.),
            ],
            vec![
                Vec2::ZERO,
                Vec2::new(0., 100.),
                Vec2::new(100., 100.),
                Vec2::new(100., 0.),
            ],
        ] {
            let polygon = Polygon {
                rings: vec![ring.clone()],
                outer: vec![true],
            };
            let mut mesh = Mesh::default();
            fringe(&mut mesh, &polygon, 0x5d9340, 0.97, 4., None);
            assert!(!mesh.vertices.is_empty());
            // je Viereck: Mitte der Außenkante (Lage 0) liegt außen, Mitte der Innenkante (Lage 1) innen
            for q in mesh.vertices.chunks(4) {
                let mid = |a: &Vertex, b: &Vertex| {
                    Vec2::new(a.point[0] + b.point[0], a.point[1] + b.point[1]) * 0.5
                };
                assert!((q[0].center[0], q[1].center[0]) == (0., 0.));
                assert!((q[2].center[0], q[3].center[0]) == (1., 1.));
                assert!(!point_in_ring(mid(&q[0], &q[1]), &ring), "außen");
                assert!(point_in_ring(mid(&q[2], &q[3]), &ring), "innen");
            }
        }
    }
    #[test]
    fn disconnected_rings_and_nested_islands_use_even_odd_fill() {
        let square = |lo: f32, hi: f32| {
            vec![
                Vec2::splat(lo),
                Vec2::new(hi, lo),
                Vec2::splat(hi),
                Vec2::new(lo, hi),
            ]
        };
        let polygon = Polygon {
            rings: vec![
                square(0., 10.),
                square(2., 8.),
                square(4., 6.),
                square(20., 25.),
            ],
            outer: vec![true, false, false, false],
        };
        let triangles = triangulate(&polygon).unwrap();
        assert_eq!(
            triangles.iter().map(|t| signed_area(t).abs()).sum::<f64>(),
            93.
        );
    }
    #[test]
    fn courtyard_and_concavity_survive_tessellation() {
        let outer = vec![
            Vec2::ZERO,
            Vec2::new(10., 0.),
            Vec2::splat(10.),
            Vec2::new(0., 10.),
        ];
        let hole = vec![
            Vec2::splat(3.),
            Vec2::new(7., 3.),
            Vec2::splat(7.),
            Vec2::new(3., 7.),
        ];
        let polygon = Polygon {
            rings: vec![outer, hole],
            outer: vec![true, false],
        };
        let tris = triangulate(&polygon).unwrap();
        let area: f64 = tris.iter().map(|t| signed_area(t).abs()).sum();
        assert!((area - 84.).abs() < 1e-6);
        for t in tris {
            assert!(!point_in_ring((t[0] + t[1] + t[2]) / 3., &polygon.rings[1]));
        }
        let concave = Polygon {
            rings: vec![vec![
                Vec2::ZERO,
                Vec2::new(10., 0.),
                Vec2::new(10., 4.),
                Vec2::new(4., 4.),
                Vec2::new(4., 10.),
                Vec2::new(0., 10.),
            ]],
            outer: vec![true],
        };
        assert_eq!(
            triangulate(&concave)
                .unwrap()
                .iter()
                .map(|t| signed_area(t).abs())
                .sum::<f64>(),
            64.
        );
    }
}
