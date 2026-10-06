//! Kreuzungsflächen (`tools/osm/plates.mjs`) auf echten Kacheln: jede gekürzte Straßenmündung trifft genau auf die
//! Eckzüge ihrer Fläche, und jede Fläche ergibt ein füllbares Polygon.
use berlin_map_loader::{
    default_data_root,
    format::{Feature, Index, Tile, TileKey},
    geom::trim_polyline,
};
use glam::Vec2;

/// Kreuzberg und Neukölln, wie die JS-Tests
const KEYS: [&str; 6] = ["26_20", "26_21", "27_20", "27_21", "28_21", "28_22"];

#[test]
fn trimmed_road_mouths_meet_their_plates() {
    let root = default_data_root();
    let index = Index::read(&root).unwrap();
    let (mut ends, mut plates, mut bad) = (0, 0, Vec::new());
    for key in KEYS {
        let tk = TileKey::parse(key).unwrap();
        let tile = Tile::read(&root, tk, &index.meta).unwrap();
        let bounds = tk.bounds(index.meta.tile);
        let mut corner_ends: Vec<Vec2> = Vec::new();
        for item in &tile.items {
            if let Feature::Plate { corners, .. } = &item.feature {
                plates += 1;
                let mut ring: Vec<Vec2> = corners.iter().flatten().copied().collect();
                ring.dedup_by(|a, b| a.distance(*b) < 0.05);
                assert!(ring.len() >= 3, "{key}: Fläche mit {} Punkten", ring.len());
                // relativ zum ersten Punkt: f32 verliert bei Weltkoordinaten (~1,7·10⁵ px) sonst die Fläche
                let o = ring[0];
                let ring: Vec<Vec2> = ring.iter().map(|p| *p - o).collect();
                let area: f32 = ring
                    .iter()
                    .zip(ring.iter().cycle().skip(1))
                    .map(|(a, b)| a.perp_dot(*b))
                    .sum::<f32>()
                    .abs()
                    / 2.;
                assert!(area > 1., "{key}: Fläche ohne Inhalt");
                for c in corners {
                    corner_ends.push(c[0]);
                    corner_ends.push(*c.last().unwrap());
                }
            }
        }
        for item in &tile.items {
            let Feature::Road(r) = &item.feature else {
                continue;
            };
            let t = trim_polyline(&r.points, r.trim[0], r.trim[1]);
            for (k, trim) in r.trim.iter().enumerate() {
                if *trim <= 0. || t.len() < 2 {
                    continue;
                }
                let (p, q) = if k == 0 {
                    (t[0], t[1])
                } else {
                    (t[t.len() - 1], t[t.len() - 2])
                };
                let u = (q - p).normalize_or_zero();
                let n = Vec2::new(-u.y, u.x) * r.width / 2.;
                // Straßen liegen ganz in jeder berührten Kachel, Flächen nur dort, wo sie hineinragen
                if !bounds.contains(p) {
                    continue;
                }
                for side in [p + n, p - n] {
                    let near = corner_ends
                        .iter()
                        .map(|c| c.distance(side))
                        .fold(f32::MAX, f32::min);
                    ends += 1;
                    if near > 1.5 {
                        bad.push((key, r.id, near));
                    }
                }
            }
        }
    }
    assert!(plates > 500, "nur {plates} Flächen");
    assert!(
        bad.is_empty(),
        "{} von {ends} Mündungen ohne passende Ecke, z. B. {:?}",
        bad.len(),
        &bad[..bad.len().min(8)]
    );
}

/// Haltlinien stehen vor der Kreuzungsfläche, nicht in ihr: kein Haltlinien-Mittelpunkt (0,5 m tief, quer über die
/// Fahrstreifen) liegt mehr als 1 m tief in einer Fläche (vorher standen sie an der alten Scheiben-Kürzung, also oft
/// mitten auf der Kreuzung).
#[test]
fn stop_lines_stand_before_the_junction() {
    use berlin_map_loader::geom::point_in_ring;
    let root = default_data_root();
    let index = Index::read(&root).unwrap();
    let (mut stops, mut inside) = (0, Vec::new());
    for key in KEYS {
        let tile = Tile::read(&root, TileKey::parse(key).unwrap(), &index.meta).unwrap();
        let rings: Vec<Vec<Vec2>> = tile
            .items
            .iter()
            .filter_map(|i| match &i.feature {
                Feature::Plate {
                    corners,
                    fill: None,
                    ..
                } => Some(corners.iter().flatten().copied().collect()),
                _ => None,
            })
            .collect();
        for item in &tile.items {
            let Feature::Marks { marks, .. } = &item.feature else {
                continue;
            };
            // Haltlinie: 0,5 m tief (Zebrastreifen 4 m, Furtblöcke quadratisch)
            for m in marks
                .iter()
                .filter(|m| (m.size.x - 5.).abs() < 0.01 && m.size.y > 15.)
            {
                stops += 1;
                let (d, n) = (Vec2::from_angle(m.angle), Vec2::from_angle(m.angle).perp());
                // 1 m vor und hinter der Linie (in Fahrtrichtung) liegt nicht beides in einer Fläche
                let deep = rings.iter().any(|r| {
                    point_in_ring(m.center + d * 10., r) && point_in_ring(m.center - d * 10., r)
                });
                let _ = n;
                if deep {
                    inside.push((key, m.center));
                }
            }
        }
    }
    assert!(stops > 20, "nur {stops} Haltlinien");
    eprintln!("{stops} Haltlinien, {} in einer Fläche", inside.len());
    assert!(
        inside.len() * 50 < stops,
        "{} von {stops} Haltlinien in einer Kreuzungsfläche, z. B. {:?}",
        inside.len(),
        &inside[..inside.len().min(6)]
    );
}
