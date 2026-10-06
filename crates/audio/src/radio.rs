//! Autoradio: zwölf Online-Sender (`data/radio.json`), live gestreamt und dekodiert.
//!
//! Je Sender läuft ein eigener Thread: HTTP über `ureq`, MP3 über `symphonia`, die Abtastwerte (Stereo, Abtastrate
//! des Senders) landen in einem Puffer. Der Audio-Thread zieht daraus mit linearer Umrechnung auf die Ausgaberate
//! (`Radio::fill`). Beim Verbinden und ohne Empfang rauscht es leise. Wird das Radio nicht mehr gebraucht (Aussteigen,
//! Pause), läuft der Stream noch `LINGER_S` stumm weiter, damit kurzes Unterbrechen nicht neu verbindet.
//! Gestreamt wird nur, wenn `Synth::enable_radio` aufgerufen wurde (Live-Ausgabe) – nie in Tests oder beim
//! WAV-Export. Titel und Interpret kommen aus den ICY-Metadaten (`Icy-MetaData: 1`): der Sender schiebt alle
//! `icy-metaint` Bytes einen Block `StreamTitle='…';` in den MP3-Strom, `IcyReader` schneidet ihn heraus.
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
    /// laufender Titel aus den ICY-Metadaten („Interpret - Titel“, wie gesendet)
    title: Mutex<Option<String>>,
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
    /// Laufender Titel des gewünschten Senders („Interpret - Titel“, wie gesendet), sofern bekannt.
    pub fn title(&self) -> Option<String> {
        match (&self.stream, self.want) {
            (Some(s), Some(_)) => s.shared.title.lock().ok().and_then(|t| t.clone()),
            _ => None,
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
        title: Mutex::new(None),
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

/// Entfernt die ICY-Metadatenblöcke aus dem Strom (alle `metaint` Bytes: Längenbyte × 16, dann Text) und legt den
/// Titel in `Shared::title` ab. `metaint` 0 = der Sender schickt keine Metadaten.
struct IcyReader<R> {
    inner: R,
    metaint: usize,
    left: usize,
    shared: Arc<Shared>,
}
impl<R: std::io::Read> std::io::Read for IcyReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.metaint == 0 {
            return self.inner.read(buf);
        }
        if self.left == 0 {
            let mut len = [0u8; 1];
            self.inner.read_exact(&mut len)?;
            let n = len[0] as usize * 16;
            if n > 0 {
                let mut meta = vec![0u8; n];
                self.inner.read_exact(&mut meta)?;
                if let Some(t) = parse_icy_title(&meta)
                    && let Ok(mut slot) = self.shared.title.lock()
                {
                    *slot = (!t.is_empty()).then_some(t);
                }
            }
            self.left = self.metaint;
        }
        let n = buf.len().min(self.left);
        let got = self.inner.read(&mut buf[..n])?;
        self.left -= got;
        Ok(got)
    }
}

/// `StreamTitle='…';` aus einem ICY-Metadatenblock (UTF-8, sonst Latin-1; Nullbytes am Ende). `None` ohne Feld,
/// leerer Text = Feld leer (Sender ohne Titel, z. B. Werbung).
pub fn parse_icy_title(meta: &[u8]) -> Option<String> {
    let end = meta.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
    let raw = &meta[..end];
    let text = match std::str::from_utf8(raw) {
        Ok(t) => t.to_string(),
        Err(_) => raw.iter().map(|&b| b as char).collect(),
    };
    let start = text.find("StreamTitle='")? + "StreamTitle='".len();
    let rest = &text[start..];
    let stop = rest.find("';").unwrap_or(rest.len());
    Some(rest[..stop].trim().to_string())
}

/// Was angezeigt wird: (Interpret, Titel) aus einem ICY-Titel. `None`, wenn der Sender statt eines Liedes nur sich
/// selbst meldet („FluxFM - Livestream“, „STAR FM MAXIMUM ROCK Berlin“). Formate: „Interpret - Titel“ und
/// „"Titel" von Interpret“ (Radio Berlin 88,8); alles andere als Titel ohne Interpret (Sendungsname).
pub fn song_info(raw: &str, station: &str) -> Option<(Option<String>, String)> {
    let norm = |t: &str| {
        t.chars()
            .filter(|c| c.is_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect::<String>()
    };
    let t = raw.trim();
    let first = station
        .split_whitespace()
        .next()
        .map(norm)
        .unwrap_or_default();
    if t.is_empty()
        || norm(t).contains("livestream")
        || (!first.is_empty() && norm(t).starts_with(&first))
    {
        return None;
    }
    let unquote = |x: &str| {
        x.trim()
            .trim_matches(|c| matches!(c, '"' | '\'' | '„' | '“' | '”'))
            .trim()
            .to_string()
    };
    if let Some((title, artist)) = t.rsplit_once(" von ")
        && title.trim_start().starts_with(['"', '„', '“'])
    {
        return Some((Some(artist.trim().to_string()), unquote(title)));
    }
    match t.split_once(" - ") {
        Some((a, b)) if !a.trim().is_empty() && !b.trim().is_empty() => {
            Some((Some(a.trim().to_string()), b.trim().to_string()))
        }
        _ => Some((None, t.to_string())),
    }
}

/// Einen Stream öffnen und dekodieren, bis er endet, abbricht oder gestoppt wird.
fn run(url: &str, s: &Arc<Shared>) -> anyhow::Result<()> {
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
        .set("Icy-MetaData", "1")
        .call()?;
    let metaint = resp
        .header("icy-metaint")
        .and_then(|v| v.trim().parse::<usize>().ok())
        .unwrap_or(0);
    let reader = IcyReader {
        inner: resp.into_reader(),
        metaint,
        left: metaint,
        shared: s.clone(),
    };
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
            title: Mutex::new(None),
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
    fn icy_metadata_is_cut_out_and_the_title_parsed() {
        use std::io::Read;
        assert_eq!(
            parse_icy_title(b"StreamTitle='Daft Punk - Around the World';StreamUrl='';\0\0"),
            Some("Daft Punk - Around the World".into())
        );
        assert_eq!(parse_icy_title(b"StreamTitle='';\0"), Some(String::new()));
        assert_eq!(parse_icy_title(b"StreamUrl='x';"), None);
        // Latin-1 (kein gültiges UTF-8)
        assert_eq!(
            parse_icy_title(b"StreamTitle='Die \xc4rzte - Schrei nach Liebe';"),
            Some("Die Ärzte - Schrei nach Liebe".into())
        );
        let song = |t: &str, st: &str| song_info(t, st);
        assert_eq!(
            song("Die Ärzte - Schrei nach Liebe", "Star FM"),
            Some((Some("Die Ärzte".into()), "Schrei nach Liebe".into()))
        );
        assert_eq!(
            song(
                "\"Gettin' Jiggy Wit It\" von Will Smith",
                "Radio Berlin 88,8"
            ),
            Some((Some("Will Smith".into()), "Gettin' Jiggy Wit It".into()))
        );
        assert_eq!(
            song("Wie es euch gefällt.", "rbb radio3"),
            Some((None, "Wie es euch gefällt.".into())),
            "Sendung ohne Interpret"
        );
        // der Sender meldet nur sich selbst
        assert_eq!(song("FluxFM - Livestream", "FluxFM"), None);
        assert_eq!(
            song("FluxFM - Sound Of Berlin", "FluxFM Sound of Berlin"),
            None
        );
        assert_eq!(song("STAR FM MAXIMUM ROCK Berlin", "Star FM"), None);
        assert_eq!(song("104.6 RTL Berlin Livestream", "104.6 RTL"), None);
        assert_eq!(song("", "Star FM"), None);
        // Strom: 4 Bytes Ton, Metadaten (1 × 16 Bytes), 4 Bytes Ton, leerer Block, 2 Bytes Ton
        let mut meta = b"StreamTitle='A - B';".to_vec();
        meta.resize(32, 0);
        let mut data = b"abcd".to_vec();
        data.push(2);
        data.extend(&meta);
        data.extend(b"efgh");
        data.push(0);
        data.extend(b"ij");
        let shared = Arc::new(Shared {
            buf: Mutex::new(VecDeque::new()),
            title: Mutex::new(None),
            sr: AtomicU32::new(44100),
            state: AtomicU8::new(0),
            stop: AtomicBool::new(false),
        });
        let mut r = IcyReader {
            inner: std::io::Cursor::new(data),
            metaint: 4,
            left: 4,
            shared: shared.clone(),
        };
        let mut out = Vec::new();
        r.read_to_end(&mut out).unwrap();
        assert_eq!(out, b"abcdefghij", "nur der Ton bleibt");
        assert_eq!(shared.title.lock().unwrap().as_deref(), Some("A - B"));
    }

    #[test]
    fn volume_scales_the_target_and_zero_mutes() {
        // ohne Netz: Stream von Hand, damit `set` nicht verbindet
        let mut r = Radio {
            stream: Some(Stream {
                station: 0,
                shared: Arc::new(Shared {
                    buf: Mutex::new(VecDeque::new()),
                    title: Mutex::new(None),
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
