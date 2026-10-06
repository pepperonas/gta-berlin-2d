//! Autoradio: zwölf Online-Sender (`data/radio.json`), live gestreamt und dekodiert.
//!
//! Je Sender läuft ein eigener Thread: HTTP über `ureq`, MP3 über `symphonia`, die Abtastwerte (Stereo, Abtastrate
//! des Senders) landen in einem Puffer. Der Audio-Thread zieht daraus mit linearer Umrechnung auf die Ausgaberate
//! (`Radio::fill`). Beim Verbinden und ohne Empfang rauscht es leise. Wird das Radio nicht mehr gebraucht (Aussteigen,
//! Pause), läuft der Stream noch `LINGER_S` stumm weiter, damit kurzes Unterbrechen nicht neu verbindet.
//! Gestreamt wird nur, wenn `Synth::enable_radio` aufgerufen wurde (Live-Ausgabe) – nie in Tests oder beim
//! WAV-Export.
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

pub const LIST: &str = include_str!("../../../data/radio.json");
/// Lautstärke des Radios bei 100 % (linear, über Master); eingestellt wird ein Anteil davon (`Frame::radio_volume`)
pub const RADIO_GAIN: f32 = 0.42;
/// Voreinstellung der Radiolautstärke (%), bis der Spieler sie ändert
pub const DEFAULT_VOLUME: u8 = 40;
/// so lange läuft ein nicht mehr gebrauchter Stream noch weiter (s)
pub const LINGER_S: f64 = 20.;
/// vorpuffern, bevor gespielt wird (s), und höchstens puffern (s)
const PREBUFFER_S: f32 = 0.6;
const MAX_BUFFER_S: f32 = 4.;

#[derive(Debug, Clone, PartialEq)]
pub struct Station {
    pub name: String,
    pub genre: String,
    pub place: String,
    pub url: String,
    pub page: String,
}

/// Alle Sender in Schaltreihenfolge.
pub fn stations() -> &'static [Station] {
    static S: OnceLock<Vec<Station>> = OnceLock::new();
    S.get_or_init(|| {
        let v: serde_json::Value = serde_json::from_str(LIST).unwrap_or_default();
        v["sender"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|s| {
                        let t = |k: &str| s[k].as_str().unwrap_or("").to_string();
                        Station {
                            name: t("name"),
                            genre: t("genre"),
                            place: t("ort"),
                            url: t("url"),
                            page: t("seite"),
                        }
                    })
                    .collect()
            })
            .unwrap_or_default()
    })
}

/// Zustand eines Streams (für die Anzeige).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Off,
    Connecting,
    Playing,
    NoSignal,
}
impl State {
    fn code(self) -> u8 {
        self as u8
    }
    fn from(c: u8) -> Self {
        match c {
            1 => Self::Connecting,
            2 => Self::Playing,
            3 => Self::NoSignal,
            _ => Self::Off,
        }
    }
}

/// Gemeinsam zwischen Stream-Thread und Audio-Thread.
struct Shared {
    buf: Mutex<VecDeque<[f32; 2]>>,
    sr: AtomicU32,
    state: AtomicU8,
    stop: AtomicBool,
}

struct Stream {
    station: usize,
    shared: Arc<Shared>,
}
impl Drop for Stream {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
    }
}

/// Wiedergabe im Audio-Thread.
pub struct Radio {
    stream: Option<Stream>,
    /// gewünschter Sender (None = aus)
    want: Option<usize>,
    /// seit wann nicht mehr gewünscht (Weiterlaufen ohne Ton)
    idle_since: Option<Instant>,
    gain: f32,
    target: f32,
    /// Umrechnung: Bruchteil zwischen zwei Quellabtastwerten
    t: f64,
    prev: [f32; 2],
    cur: [f32; 2],
    playing: bool,
    noise: u32,
}
impl Default for Radio {
    fn default() -> Self {
        Self {
            stream: None,
            want: None,
            idle_since: None,
            gain: 0.,
            target: 0.,
            t: 0.,
            prev: [0.; 2],
            cur: [0.; 2],
            playing: false,
            noise: 0x1234_5678,
        }
    }
}

impl Radio {
    /// Gewünschten Sender und Lautstärke (0…1) setzen (jedes Bild): wechselt den Stream bei Bedarf, `None` blendet
    /// aus.
    pub fn set(&mut self, want: Option<usize>, volume: f32) {
        let want = want.filter(|&i| i < stations().len());
        let vol = RADIO_GAIN * volume.clamp(0., 1.);
        if want.is_some() {
            self.target = vol;
        }
        if want == self.want {
            if want.is_none()
                && let Some(t) = self.idle_since
                && t.elapsed().as_secs_f64() > LINGER_S
            {
                self.stream = None;
                self.idle_since = None;
            }
            return;
        }
        self.want = want;
        match want {
            None => {
                self.target = 0.;
                self.idle_since = Some(Instant::now());
            }
            Some(i) => {
                self.idle_since = None;
                self.target = vol;
                if self.stream.as_ref().is_none_or(|s| s.station != i) {
                    self.stream = Some(spawn(i));
                    self.playing = false;
                    self.t = 0.;
                    self.prev = [0.; 2];
                    self.cur = [0.; 2];
                }
            }
        }
    }
    /// Zustand des gewünschten Senders.
    pub fn state(&self) -> State {
        match (&self.stream, self.want) {
            (Some(s), Some(_)) => State::from(s.shared.state.load(Ordering::Relaxed)),
            _ => State::Off,
        }
    }
    /// `n` Stereo-Abtastwerte bei Ausgaberate `sr` dazumischen (`out` verschachtelt links/rechts).
    pub fn fill(&mut self, out: &mut [f32], sr: f32) {
        let Some(st) = &self.stream else { return };
        if self.target == 0. && self.gain < 1e-4 {
            self.gain = 0.;
            return;
        }
        let shared = st.shared.clone();
        let src = shared.sr.load(Ordering::Relaxed).max(1) as f64;
        let step = src / sr as f64;
        let state = State::from(shared.state.load(Ordering::Relaxed));
        let mut q = match shared.buf.lock() {
            Ok(q) => q,
            Err(_) => return,
        };
        if !self.playing && q.len() as f32 >= PREBUFFER_S * src as f32 {
            self.playing = true;
        }
        // Lautstärke weich (≈ 0,15 s)
        let k = 1. - (-1. / (0.15 * sr)).exp();
        for frame in out.chunks_mut(2) {
            self.gain += (self.target - self.gain) * k;
            let (l, r) = if self.playing {
                self.t += step;
                while self.t >= 1. {
                    self.t -= 1.;
                    self.prev = self.cur;
                    match q.pop_front() {
                        Some(s) => self.cur = s,
                        None => {
                            // Puffer leer: neu vorpuffern
                            self.playing = false;
                            break;
                        }
                    }
                }
                let u = self.t as f32;
                (
                    self.prev[0] + (self.cur[0] - self.prev[0]) * u,
                    self.prev[1] + (self.cur[1] - self.prev[1]) * u,
                )
            } else {
                // Verbinden bzw. kein Empfang: leises Rauschen wie zwischen zwei Sendern
                self.noise ^= self.noise << 13;
                self.noise ^= self.noise >> 17;
                self.noise ^= self.noise << 5;
                let n = (self.noise as f32 / u32::MAX as f32 - 0.5)
                    * if state == State::NoSignal {
                        0.06
                    } else {
                        0.035
                    };
                (n, n)
            };
            frame[0] += l * self.gain;
            if frame.len() > 1 {
                frame[1] += r * self.gain;
            }
        }
    }
}

fn spawn(station: usize) -> Stream {
    let shared = Arc::new(Shared {
        buf: Mutex::new(VecDeque::new()),
        sr: AtomicU32::new(44100),
        state: AtomicU8::new(State::Connecting.code()),
        stop: AtomicBool::new(false),
    });
    let s = shared.clone();
    let url = stations()[station].url.clone();
    let _ = std::thread::Builder::new()
        .name("radio".into())
        .spawn(move || {
            while !s.stop.load(Ordering::Relaxed) {
                s.state.store(State::Connecting.code(), Ordering::Relaxed);
                if let Err(e) = run(&url, &s) {
                    if s.stop.load(Ordering::Relaxed) {
                        break;
                    }
                    eprintln!("Radio: {e}");
                    s.state.store(State::NoSignal.code(), Ordering::Relaxed);
                    // erneut versuchen, aber nicht hektisch
                    for _ in 0..30 {
                        if s.stop.load(Ordering::Relaxed) {
                            return;
                        }
                        std::thread::sleep(Duration::from_millis(100));
                    }
                }
            }
        });
    Stream { station, shared }
}

/// Einen Stream öffnen und dekodieren, bis er endet, abbricht oder gestoppt wird.
fn run(url: &str, s: &Shared) -> anyhow::Result<()> {
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::DecoderOptions;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::{MediaSourceStream, ReadOnlySource};
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;
    let resp = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(6))
        .timeout_read(Duration::from_secs(10))
        .build()
        .get(url)
        .set("User-Agent", "gta-berlin/0.1 (Autoradio)")
        .call()?;
    let reader = resp.into_reader();
    let mss = MediaSourceStream::new(Box::new(ReadOnlySource::new(reader)), Default::default());
    let mut hint = Hint::new();
    hint.with_extension("mp3");
    let probed = symphonia::default::get_probe().format(
        &hint,
        mss,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    )?;
    let mut format = probed.format;
    let track = format
        .default_track()
        .ok_or_else(|| anyhow::anyhow!("keine Tonspur"))?;
    let id = track.id;
    let mut dec =
        symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default())?;
    let mut sbuf: Option<SampleBuffer<f32>> = None;
    loop {
        if s.stop.load(Ordering::Relaxed) {
            return Ok(());
        }
        let packet = format.next_packet()?;
        if packet.track_id() != id {
            continue;
        }
        let decoded = match dec.decode(&packet) {
            Ok(d) => d,
            // einzelner kaputter Rahmen: überspringen
            Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
            Err(e) => return Err(e.into()),
        };
        let spec = *decoded.spec();
        let ch = spec.channels.count().max(1);
        s.sr.store(spec.rate, Ordering::Relaxed);
        let sb = match &mut sbuf {
            Some(b) if b.capacity() >= decoded.capacity() * ch => b,
            _ => sbuf.insert(SampleBuffer::<f32>::new(decoded.capacity() as u64, spec)),
        };
        sb.copy_interleaved_ref(decoded);
        let max = (MAX_BUFFER_S * spec.rate as f32) as usize;
        // Gegendruck: nicht mehr als `MAX_BUFFER_S` vorhalten
        while s.buf.lock().map(|q| q.len()).unwrap_or(0) > max {
            if s.stop.load(Ordering::Relaxed) {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        if let Ok(mut q) = s.buf.lock() {
            for f in sb.samples().chunks(ch) {
                let l = f[0];
                let r = if ch > 1 { f[1] } else { l };
                q.push_back([l, r]);
            }
        }
        s.state.store(State::Playing.code(), Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn twelve_stations_with_distinct_genres_and_working_urls_listed() {
        let s = stations();
        assert_eq!(s.len(), 12);
        let mut genres: Vec<&str> = s.iter().map(|x| x.genre.as_str()).collect();
        genres.sort();
        genres.dedup();
        assert_eq!(genres.len(), 12, "jedes Genre einmal");
        assert!(
            s.iter()
                .all(|x| x.url.starts_with("http") && !x.name.is_empty())
        );
        assert!(
            s.iter().filter(|x| x.place == "Berlin").count() >= 8,
            "Berlin zuerst"
        );
    }

    #[test]
    fn plays_buffered_samples_resampled_and_hisses_while_connecting() {
        // ohne Netz: einen Stream von Hand füttern
        let shared = Arc::new(Shared {
            buf: Mutex::new(VecDeque::new()),
            sr: AtomicU32::new(24000),
            state: AtomicU8::new(State::Connecting.code()),
            stop: AtomicBool::new(true),
        });
        let mut r = Radio {
            stream: Some(Stream {
                station: 0,
                shared: shared.clone(),
            }),
            want: Some(0),
            target: RADIO_GAIN,
            gain: RADIO_GAIN,
            ..Default::default()
        };
        // verbindet noch: leises Rauschen, kein Schweigen, aber leise
        let mut out = vec![0f32; 960];
        r.fill(&mut out, 48000.);
        let peak = out.iter().fold(0f32, |m, v| m.max(v.abs()));
        assert!(peak > 0. && peak < 0.05, "Rauschen {peak}");
        // eine Sekunde Gleichspannung 0,5 / −0,5 bei 24 kHz: wird nach dem Vorpuffern gespielt
        shared
            .buf
            .lock()
            .unwrap()
            .extend(std::iter::repeat_n([0.5, -0.5], 24000));
        shared.state.store(State::Playing.code(), Ordering::Relaxed);
        // eine halbe Ausgabesekunde bei 48 kHz
        let mut out = vec![0f32; 48000];
        r.fill(&mut out, 48000.);
        let mid = &out[24000..24010];
        assert!((mid[0] - 0.5 * RADIO_GAIN).abs() < 1e-3, "links {}", mid[0]);
        assert!(
            (mid[1] + 0.5 * RADIO_GAIN).abs() < 1e-3,
            "rechts {}",
            mid[1]
        );
        // 24 kHz → 48 kHz: nach einer halben Ausgabesekunde ist die halbe Quelle verbraucht
        let left = shared.buf.lock().unwrap().len();
        assert!((11900..=12100).contains(&left), "Rest {left}");
        // aus: blendet weg (nach 1,5 s nichts mehr zu hören)
        r.set(None, 1.);
        let mut out = vec![0f32; 48000 * 3];
        r.fill(&mut out, 48000.);
        assert!(out[out.len() - 2].abs() < 1e-3);
    }

    /// Mit Netz: jeder Sender liefert binnen weniger Sekunden dekodierte Musik (`cargo test -p berlin-audio
    /// live_streams -- --ignored`).
    #[test]
    #[ignore]
    fn live_streams_decode() {
        let mut bad = Vec::new();
        for (i, st) in stations().iter().enumerate() {
            let s = spawn(i);
            let t0 = Instant::now();
            let mut ok = false;
            while t0.elapsed().as_secs() < 12 {
                std::thread::sleep(Duration::from_millis(200));
                let q = s.shared.buf.lock().unwrap();
                let sr = s.shared.sr.load(Ordering::Relaxed) as usize;
                if q.len() > sr && q.iter().any(|f| f[0].abs() > 0.01) {
                    ok = true;
                    break;
                }
            }
            println!("{:28} {}", st.name, if ok { "ok" } else { "KEIN TON" });
            if !ok {
                bad.push(st.name.clone());
            }
        }
        assert!(bad.is_empty(), "ohne Ton: {bad:?}");
    }
}

#[cfg(test)]
mod volume_tests {
    use super::*;
    #[test]
    fn volume_scales_the_target_and_zero_mutes() {
        // ohne Netz: Stream von Hand, damit `set` nicht verbindet
        let mut r = Radio {
            stream: Some(Stream {
                station: 0,
                shared: Arc::new(Shared {
                    buf: Mutex::new(VecDeque::new()),
                    sr: AtomicU32::new(48000),
                    state: AtomicU8::new(State::Playing.code()),
                    stop: AtomicBool::new(true),
                }),
            }),
            ..Default::default()
        };
        r.set(Some(0), 0.4);
        assert!((r.target - RADIO_GAIN * 0.4).abs() < 1e-6);
        r.set(Some(0), 1.);
        assert!(
            (r.target - RADIO_GAIN).abs() < 1e-6,
            "Lautstärke folgt sofort"
        );
        r.set(Some(0), 0.);
        assert_eq!(r.target, 0.);
        assert!(
            r.stream.as_ref().is_some_and(|s| s.station == 0),
            "kein Neuverbinden"
        );
    }
}
