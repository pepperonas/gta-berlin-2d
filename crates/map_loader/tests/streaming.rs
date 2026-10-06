use berlin_map_loader::{
    default_data_root,
    format::{Feature, Index, Tile, TileKey},
    geom::Bounds,
    mesh,
    stream::{Focus, Snapshot, Streamer},
};
use glam::Vec2;
use std::{
    fs,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        // eigener Ordner je Test: die Uhr ist unter macOS nur mikrosekundengenau, parallel laufende Tests bekamen
        // sonst denselben Ordner (und lasen die kaputte Kachel des anderen bzw. verloren ihn beim Aufräumen)
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let path = std::env::temp_dir().join(format!(
            "berlin-stream-test-{}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(path.join("tiles")).unwrap();
        Self(path)
    }
    fn tile(&self, x: u32, shared: bool) {
        let buildings = if shared {
            serde_json::json!([[42, 100, 0, [[6300, 100, 200, 0, 0, 200, -200, 0, 0, -200]]]])
        } else {
            serde_json::json!([[
                100 + x,
                100,
                0,
                [[x * 6400 + 100, 100, 100, 0, 0, 100, -100, 0, 0, -100]]
            ]])
        };
        let tile = serde_json::json!({"v":3,"t":[x,0],"vertices":{"xy":[]},"buildings":buildings});
        fs::write(self.0.join(format!("tiles/{x}_0.json")), tile.to_string()).unwrap();
    }
    fn index(&self) -> Arc<Index> {
        let mut index = Index::read(&default_data_root()).unwrap();
        index.tiles = vec!["0_0".into(), "1_0".into(), "10_0".into()];
        index.meta.width = 100000.;
        index.meta.height = 6400.;
        Arc::new(index)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn focus(stream: &Streamer, x: f32) {
    let p = Vec2::new(x, 200.);
    stream.focus(Focus {
        position: p,
        view: Bounds {
            min: p - Vec2::splat(150.),
            max: p + Vec2::splat(150.),
        },
        second: None,
    });
}
fn wait(stream: &Streamer, condition: impl Fn(&Snapshot) -> bool) -> Snapshot {
    let deadline = Instant::now() + Duration::from_secs(8);
    let mut last = None;
    while Instant::now() < deadline {
        if let Some(snapshot) = stream.latest() {
            if condition(&snapshot) {
                return snapshot;
            }
            last = Some(snapshot);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("Streaming-Timeout: {:?}", last.map(|s| s.status));
}
#[test]
fn async_stream_deduplicates_and_evicts_on_teleport() {
    let fixture = Fixture::new();
    fixture.tile(0, true);
    fixture.tile(1, true);
    fixture.tile(10, false);
    let stream = Streamer::new(fixture.0.clone(), fixture.index()).unwrap();
    focus(&stream, 6300.);
    let first = wait(&stream, |s| s.status.resident == 2 && s.status.ready());
    assert!(first.errors.is_empty());
    assert_eq!(first.status.unique_objects, 1);
    assert_eq!(
        first
            .tiles
            .values()
            .filter(|m| !m.indices.is_empty())
            .count(),
        1
    );
    focus(&stream, 65000.);
    let second = wait(&stream, |s| {
        s.status.resident == 1 && s.tiles.contains_key(&TileKey { x: 10, y: 0 })
    });
    assert_eq!(second.status.unique_objects, 1);
    assert!(second.status.ready());
    assert!(!second.tiles.contains_key(&TileKey { x: 0, y: 0 }));
    focus(&stream, 6300.);
    let third = wait(&stream, |s| {
        s.status.resident == 2 && s.tiles.contains_key(&TileKey { x: 0, y: 0 })
    });
    assert_eq!(third.status.unique_objects, 1);
}
#[test]
fn corrupted_tile_reports_error_then_recovers() {
    let fixture = Fixture::new();
    fixture.tile(0, true);
    fixture.tile(1, true);
    fixture.tile(10, false);
    fs::write(fixture.0.join("tiles/10_0.json"), "{broken").unwrap();
    let stream = Streamer::new(fixture.0.clone(), fixture.index()).unwrap();
    focus(&stream, 65000.);
    let failed = wait(&stream, |s| s.status.failed == 1);
    assert!(!failed.errors.is_empty());
    assert!(!failed.status.ready());
    fixture.tile(10, false);
    let ready = wait(&stream, |s| s.status.ready());
    assert!(ready.errors.is_empty());
    assert_eq!(ready.status.resident, 1);
}
#[test]
fn actual_berlin_tiles_mesh_with_finite_vertices_and_valid_indices() {
    let root = default_data_root();
    let index = Index::read(&root).unwrap();
    for key in ["37_30", "38_29", "29_24"] {
        let tile = Tile::read(&root, TileKey::parse(key).unwrap(), &index.meta).unwrap();
        let mut triangles = 0;
        let mut buildings = 0;
        for item in tile.items {
            if matches!(item.feature, Feature::Building(_)) {
                buildings += 1;
            }
            let mesh = mesh::prepare(&item.feature, index.meta.scale)
                .unwrap_or_else(|e| panic!("{key} {:?}: {e}", item.id));
            assert!(
                mesh.indices
                    .iter()
                    .all(|&i| (i as usize) < mesh.vertices.len())
            );
            assert!(mesh.vertices.iter().all(|v| {
                v.point
                    .iter()
                    .chain(v.normal.iter())
                    .chain(v.uv.iter())
                    .chain(v.color.iter())
                    .all(|v| v.is_finite())
            }));
            triangles += mesh.indices.len() / 3;
        }
        assert!(buildings > 0 && triangles > 1000);
    }
}
