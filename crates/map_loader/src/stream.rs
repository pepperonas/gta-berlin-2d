//! Bounded residency, shared-feature ownership and latest-focus background streaming.
use crate::{
    format::{FeatureId, Index, Tile, TileKey},
    geom::Bounds,
    mesh::{self, Mesh},
};
use anyhow::Result;
use glam::Vec2;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    path::PathBuf,
    sync::{
        Arc, Condvar, Mutex,
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
const MAX_RESIDENT: usize = 64;
#[derive(Debug, Clone, Copy)]
pub struct Focus {
    pub position: Vec2,
    pub view: Bounds,
    /// zweiter Bildausschnitt (Splitscreen): Kacheln um beide laden und halten
    pub second: Option<(Vec2, Bounds)>,
}
impl Focus {
    fn spots(&self) -> Vec<(Vec2, Bounds)> {
        let mut v = vec![(self.position, self.view)];
        v.extend(self.second);
        v
    }
}
/// Listen reihum zusammenführen (je eine Kachel aus jeder), ohne Doppelte; eine Liste bleibt, wie sie ist.
fn interleave(lists: Vec<Vec<TileKey>>) -> Vec<TileKey> {
    if lists.len() == 1 {
        return lists.into_iter().next().unwrap_or_default();
    }
    let mut out = Vec::new();
    let n = lists.iter().map(Vec::len).max().unwrap_or(0);
    for i in 0..n {
        for l in &lists {
            if let Some(&k) = l.get(i)
                && !out.contains(&k)
            {
                out.push(k);
            }
        }
    }
    out
}
#[derive(Debug, Default, Clone)]
pub struct Status {
    pub resident: usize,
    pub needed: usize,
    pub ready: usize,
    pub failed: usize,
    pub unique_objects: usize,
    pub mesh_bytes: usize,
    pub triangles: usize,
    pub sprites: usize,
}
impl Status {
    pub fn ready(&self) -> bool {
        self.needed > 0 && self.ready == self.needed
    }
}
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub tiles: BTreeMap<TileKey, Arc<Mesh>>,
    pub bounds: BTreeMap<TileKey, Bounds>,
    pub status: Status,
    pub errors: Vec<String>,
}
struct PreparedItem {
    id: Option<FeatureId>,
    mesh: Arc<Mesh>,
}
struct PreparedTile {
    items: Vec<PreparedItem>,
}
#[derive(Default)]
struct Registry {
    tiles: BTreeMap<TileKey, PreparedTile>,
    refs: HashMap<FeatureId, BTreeSet<TileKey>>,
    meshes: BTreeMap<TileKey, Arc<Mesh>>,
}
impl Registry {
    fn owner(&self, id: FeatureId) -> Option<TileKey> {
        self.refs.get(&id).and_then(|r| r.first().copied())
    }
    fn remove(&mut self, key: TileKey, dirty: &mut BTreeSet<TileKey>) {
        let Some(tile) = self.tiles.remove(&key) else {
            return;
        };
        self.meshes.remove(&key);
        dirty.remove(&key);
        for item in tile.items {
            if let Some(id) = item.id {
                let old = self.owner(id);
                if let Some(refs) = self.refs.get_mut(&id) {
                    refs.remove(&key);
                    if refs.is_empty() {
                        self.refs.remove(&id);
                    }
                }
                if old == Some(key)
                    && let Some(owner) = self.owner(id)
                {
                    dirty.insert(owner);
                }
            }
        }
    }
    fn install(&mut self, key: TileKey, tile: PreparedTile, dirty: &mut BTreeSet<TileKey>) {
        for item in &tile.items {
            if let Some(id) = item.id {
                let old = self.owner(id);
                self.refs.entry(id).or_default().insert(key);
                let new = self.owner(id);
                if old != new
                    && let Some(old) = old
                {
                    dirty.insert(old);
                }
            }
        }
        self.tiles.insert(key, tile);
        dirty.insert(key);
    }
    fn rebuild(&mut self, dirty: BTreeSet<TileKey>) {
        for key in dirty {
            let Some(tile) = self.tiles.get(&key) else {
                continue;
            };
            let mut mesh = Mesh::default();
            let mut local_ids = HashSet::new();
            for item in &tile.items {
                if let Some(id) = item.id
                    && (self.owner(id) != Some(key) || !local_ids.insert(id))
                {
                    continue;
                }
                mesh.append(&item.mesh);
            }
            self.meshes.insert(key, Arc::new(mesh));
        }
    }
    fn prepare(&self, tile: Tile, scale: f32) -> Result<PreparedTile> {
        let mut items = Vec::new();
        let mut own = HashMap::<FeatureId, Arc<Mesh>>::new();
        for item in tile.items {
            let existing = item.id.and_then(|id| {
                own.get(&id).cloned().or_else(|| {
                    self.owner(id)
                        .and_then(|owner| self.tiles.get(&owner))
                        .and_then(|t| t.items.iter().find(|f| f.id == Some(id)))
                        .map(|f| f.mesh.clone())
                })
            });
            let mesh = if let Some(mesh) = existing {
                mesh
            } else {
                Arc::new(mesh::prepare(&item.feature, scale)?)
            };
            if let Some(id) = item.id {
                own.insert(id, mesh.clone());
            }
            items.push(PreparedItem { id: item.id, mesh });
        }
        Ok(PreparedTile { items })
    }
}
struct Mailbox {
    focus: Option<Focus>,
    stop: bool,
}
pub struct Streamer {
    shared: Arc<(Mutex<Mailbox>, Condvar)>,
    receiver: Receiver<Snapshot>,
    worker: Option<JoinHandle<()>>,
}
impl Streamer {
    pub fn new(root: PathBuf, index: Arc<Index>) -> Result<Self> {
        let shared = Arc::new((
            Mutex::new(Mailbox {
                focus: None,
                stop: false,
            }),
            Condvar::new(),
        ));
        let worker_shared = shared.clone();
        let (sender, receiver) = mpsc::sync_channel(2);
        let worker = thread::Builder::new()
            .name("berlin-map-stream".into())
            .spawn(move || worker(root, index, worker_shared, sender))?;
        Ok(Self {
            shared,
            receiver,
            worker: Some(worker),
        })
    }
    pub fn focus(&self, focus: Focus) {
        let (lock, cv) = &*self.shared;
        lock.lock().unwrap().focus = Some(focus);
        cv.notify_one();
    }
    pub fn latest(&self) -> Option<Snapshot> {
        let mut last = None;
        while let Ok(snapshot) = self.receiver.try_recv() {
            last = Some(snapshot);
        }
        last
    }
}
impl Drop for Streamer {
    fn drop(&mut self) {
        let (lock, cv) = &*self.shared;
        lock.lock().unwrap().stop = true;
        cv.notify_one();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
fn distance(key: TileKey, p: Vec2, tile: f32) -> f32 {
    let b = key.bounds(tile);
    let q = p.clamp(b.min, b.max);
    q.distance_squared(p)
}
fn keys_around(available: &[TileKey], position: Vec2, radius: f32, tile: f32) -> Vec<TileKey> {
    let mut out: Vec<_> = available
        .iter()
        .copied()
        .filter(|&k| distance(k, position, tile) <= radius * radius)
        .collect();
    out.sort_by(|&a, &b| {
        distance(a, position, tile)
            .total_cmp(&distance(b, position, tile))
            .then_with(|| a.cmp(&b))
    });
    out
}
fn make_snapshot(
    reg: &Registry,
    visible: &[TileKey],
    failures: &HashMap<TileKey, (u32, Instant, String)>,
    tile_size: f32,
) -> Snapshot {
    let status = Status {
        resident: reg.tiles.len(),
        needed: visible.len(),
        ready: visible.iter().filter(|k| reg.tiles.contains_key(k)).count(),
        failed: visible.iter().filter(|k| failures.contains_key(k)).count(),
        unique_objects: reg.refs.len(),
        mesh_bytes: reg.meshes.values().map(|m| m.bytes()).sum(),
        triangles: reg.meshes.values().map(|m| m.indices.len() / 3).sum(),
        sprites: reg.meshes.values().map(|m| m.sprites.len()).sum(),
    };
    // Include actual feature extents: shared objects can extend beyond their owning tile.
    let bounds = reg
        .meshes
        .iter()
        .map(|(&k, m)| {
            let mut b = k.bounds(tile_size);
            for v in &m.vertices {
                let p = Vec2::new(v.point[0], v.point[1]);
                b.min = b.min.min(p);
                b.max = b.max.max(p);
            }
            for s in &m.sprites {
                let p = Vec2::new(s.point[0], s.point[1]);
                let radius = Vec2::new(s.size[0], s.size[1]).length() * 0.5;
                b.min = b.min.min(p - Vec2::splat(radius));
                b.max = b.max.max(p + Vec2::splat(radius));
            }
            (k, b.expand(512.))
        })
        .collect();
    Snapshot {
        tiles: reg.meshes.clone(),
        bounds,
        status,
        errors: failures
            .iter()
            .map(|(key, (_, _, error))| format!("Kachel {key}: {error}"))
            .collect(),
    }
}
fn worker(
    root: PathBuf,
    index: Arc<Index>,
    shared: Arc<(Mutex<Mailbox>, Condvar)>,
    sender: SyncSender<Snapshot>,
) {
    let available: Vec<_> = index
        .tiles
        .iter()
        .filter_map(|s| TileKey::parse(s).ok())
        .collect();
    let mut reg = Registry::default();
    let mut failures = HashMap::<TileKey, (u32, Instant, String)>::new();
    let mut focus: Option<Focus> = None;
    let mut pending: Option<Snapshot> = None;
    let mut last_visible = Vec::new();
    loop {
        let (lock, cv) = &*shared;
        let mut mailbox = lock.lock().unwrap();
        if mailbox.stop {
            break;
        }
        let mut changed = false;
        if let Some(new) = mailbox.focus.take() {
            focus = Some(new);
        }
        drop(mailbox);
        if let Some(f) = focus {
            let spots = f.spots();
            let mut lists = Vec::new();
            let mut keep = HashSet::new();
            for &(position, view) in &spots {
                let view_radius = (view.max - view.min).length() * 0.5 + 512.;
                let load_radius = 7000_f32.max(view_radius);
                let keep_radius = 11000_f32.max(load_radius + index.meta.tile);
                lists.push(keys_around(
                    &available,
                    position,
                    load_radius,
                    index.meta.tile,
                ));
                keep.extend(keys_around(
                    &available,
                    position,
                    keep_radius,
                    index.meta.tile,
                ));
            }
            let mut wanted = interleave(lists);
            wanted.truncate(MAX_RESIDENT);
            let mut visible: Vec<_> = wanted
                .iter()
                .copied()
                .filter(|k| {
                    let b = k.bounds(index.meta.tile);
                    spots.iter().any(|(_, v)| b.intersects(v.expand(256.)))
                })
                .collect();
            visible.sort();
            if visible != last_visible {
                last_visible = visible.clone();
                changed = true;
            }
            let mut dirty = BTreeSet::new();
            let remove: Vec<_> = reg
                .tiles
                .keys()
                .copied()
                .filter(|k| !keep.contains(k))
                .collect();
            for k in remove {
                reg.remove(k, &mut dirty);
                changed = true;
            }
            failures.retain(|k, _| keep.contains(k));
            let next = wanted.iter().copied().find(|k| {
                !reg.tiles.contains_key(k)
                    && failures
                        .get(k)
                        .is_none_or(|(_, retry, _)| Instant::now() >= *retry)
            });
            if let Some(key) = next {
                if reg.tiles.len() >= MAX_RESIDENT {
                    let far = reg
                        .tiles
                        .keys()
                        .copied()
                        .filter(|k| !wanted.contains(k))
                        .max_by(|&a, &b| {
                            let near = |k| {
                                spots
                                    .iter()
                                    .map(|(p, _)| distance(k, *p, index.meta.tile))
                                    .fold(f32::INFINITY, f32::min)
                            };
                            near(a).total_cmp(&near(b))
                        });
                    if let Some(far) = far {
                        reg.remove(far, &mut dirty);
                    }
                }
                let result = Tile::read(&root, key, &index.meta)
                    .and_then(|tile| reg.prepare(tile, index.meta.scale));
                match result {
                    Ok(tile) => {
                        reg.install(key, tile, &mut dirty);
                        failures.remove(&key);
                    }
                    Err(error) => {
                        let attempts = failures.get(&key).map_or(1, |(n, _, _)| n + 1);
                        let delay = [500, 1000, 2000, 3000][(attempts as usize - 1).min(3)];
                        let message = format!("{error:#}");
                        if attempts == 1 {
                            eprintln!("Kachel {key}: {message}");
                        }
                        failures.insert(
                            key,
                            (
                                attempts,
                                Instant::now() + Duration::from_millis(delay),
                                message,
                            ),
                        );
                    }
                }
                changed = true;
            }
            reg.rebuild(dirty);
            if changed {
                pending = Some(make_snapshot(&reg, &visible, &failures, index.meta.tile));
            }
            if let Some(snapshot) = pending.take() {
                match sender.try_send(snapshot) {
                    Ok(()) => {}
                    Err(TrySendError::Full(snapshot)) => pending = Some(snapshot),
                    Err(TrySendError::Disconnected(_)) => break,
                }
            }
            if next.is_some() {
                continue;
            }
        }
        let mailbox = lock.lock().unwrap();
        if mailbox.stop {
            break;
        }
        if mailbox.focus.is_none() {
            let _ = cv.wait_timeout(mailbox, Duration::from_millis(50)).unwrap();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_feature_changes_owner_on_eviction() {
        let a = TileKey { x: 1, y: 1 };
        let b = TileKey { x: 2, y: 1 };
        let id = FeatureId(1, 99);
        let feature = Mesh {
            indices: vec![0, 1, 2],
            vertices: vec![mesh::Vertex::zeroed(); 3],
            ..Default::default()
        };
        let mesh = Arc::new(feature);
        let tile = || PreparedTile {
            items: vec![PreparedItem {
                id: Some(id),
                mesh: mesh.clone(),
            }],
        };
        let mut reg = Registry::default();
        let mut dirty = BTreeSet::new();
        reg.install(b, tile(), &mut dirty);
        reg.rebuild(std::mem::take(&mut dirty));
        reg.install(a, tile(), &mut dirty);
        reg.rebuild(std::mem::take(&mut dirty));
        assert_eq!(reg.meshes[&a].indices.len(), 3);
        assert_eq!(reg.meshes[&b].indices.len(), 0);
        assert_eq!(reg.refs[&id].len(), 2);
        reg.remove(a, &mut dirty);
        reg.rebuild(dirty);
        assert_eq!(reg.meshes[&b].indices.len(), 3);
        assert_eq!(reg.refs[&id].len(), 1);
    }
    use bytemuck::Zeroable;
}
