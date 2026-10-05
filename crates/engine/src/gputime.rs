//! GPU-Zeit je Bild über Zeitstempel-Abfragen (nur für `--messung` und nur, wenn der Adapter `TIMESTAMP_QUERY`
//! kann). Anfang des ersten und Ende des letzten Durchgangs eines Bildes; ausgelesen wird über einen Ring von
//! Lesepuffern, ohne auf die GPU zu warten (ein Bild, dessen Puffer noch belegt ist, wird nicht gemessen).
use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};

const SLOTS: usize = 4;
/// Abstand der Auflösungsziele (wgpu verlangt 256-Byte-Ausrichtung für `resolve_query_set`).
const STRIDE: u64 = wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT;
const FREE: u8 = 0;
const PENDING: u8 = 1;
const READY: u8 = 2;

pub(crate) struct GpuTimer {
    pub set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    read: Vec<wgpu::Buffer>,
    state: Vec<Arc<AtomicU8>>,
    /// Nanosekunden je Zeitstempel-Einheit
    period: f32,
    next: usize,
    /// laufendes Bild: belegter Platz
    current: Option<usize>,
    /// gemessene Bilder (ms)
    pub samples: Vec<f32>,
}

impl GpuTimer {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Option<Self> {
        if !device.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            return None;
        }
        let set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("GPU-Zeit"),
            ty: wgpu::QueryType::Timestamp,
            count: (SLOTS * 2) as u32,
        });
        let resolve = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("GPU-Zeit auflösen"),
            size: STRIDE * SLOTS as u64,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let read = (0..SLOTS)
            .map(|_| {
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("GPU-Zeit lesen"),
                    size: 16,
                    usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                })
            })
            .collect();
        Some(Self {
            set,
            resolve,
            read,
            state: (0..SLOTS).map(|_| Arc::new(AtomicU8::new(FREE))).collect(),
            period: queue.get_timestamp_period(),
            next: 0,
            current: None,
            samples: Vec::new(),
        })
    }
    /// Platz für das nächste Bild: Index des Anfangs-Zeitstempels (Ende = +1), `None` = Bild nicht messen.
    pub fn begin(&mut self) -> Option<u32> {
        let slot = self.next;
        if self.state[slot].load(Ordering::Acquire) != FREE {
            self.current = None;
            return None;
        }
        self.next = (self.next + 1) % SLOTS;
        self.current = Some(slot);
        Some(slot as u32 * 2)
    }
    /// Nach den Durchgängen des Bildes: Zeitstempel in den Lesepuffer kopieren.
    pub fn resolve(&self, encoder: &mut wgpu::CommandEncoder) {
        let Some(slot) = self.current else {
            return;
        };
        let q = slot as u32 * 2;
        let at = slot as u64 * STRIDE;
        encoder.resolve_query_set(&self.set, q..q + 2, &self.resolve, at);
        encoder.copy_buffer_to_buffer(&self.resolve, at, &self.read[slot], 0, 16);
    }
    /// Nach dem Abschicken: Lesen anstoßen, fertige Plätze einsammeln.
    pub fn after_submit(&mut self, device: &wgpu::Device) {
        if let Some(slot) = self.current.take() {
            let state = self.state[slot].clone();
            state.store(PENDING, Ordering::Release);
            self.read[slot]
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |r| {
                    state.store(if r.is_ok() { READY } else { FREE }, Ordering::Release);
                });
        }
        let _ = device.poll(wgpu::PollType::Poll);
        for slot in 0..SLOTS {
            if self.state[slot].load(Ordering::Acquire) != READY {
                continue;
            }
            let ticks = {
                let data = self.read[slot].slice(..).get_mapped_range();
                let t0 = u64::from_le_bytes(data[0..8].try_into().unwrap_or_default());
                let t1 = u64::from_le_bytes(data[8..16].try_into().unwrap_or_default());
                t1.saturating_sub(t0)
            };
            self.read[slot].unmap();
            self.state[slot].store(FREE, Ordering::Release);
            let ms = ticks as f64 * self.period as f64 / 1e6;
            // Unsinn (Zähler übergelaufen, Treiber liefert 0) nicht mitzählen
            if ms > 0. && ms < 1000. {
                self.samples.push(ms as f32);
            }
        }
    }
}
