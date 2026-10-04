//! Ausgabe: Echtzeit über `cpal` (macOS CoreAudio, Windows WASAPI) und Offline als WAV-Datei.
//! Der Synthesizer liegt hinter einem Mutex; das Spiel setzt je Bild die Parameter, der Audio-Thread rendert.
use crate::synth::{Frame, Synth};
use anyhow::{Context, Result, bail};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};

/// Laufende Tonausgabe. Fällt das Gerät weg, läuft das Spiel stumm weiter.
pub struct Audio {
    synth: Arc<Mutex<Synth>>,
    _stream: cpal::Stream,
    pub sample_rate: u32,
    pub device: String,
}
impl Audio {
    pub fn start() -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .context("Kein Audio-Ausgabegerät")?;
        let supported = device
            .default_output_config()
            .context("Keine Audio-Ausgabekonfiguration")?;
        let config = supported.config();
        let (sr, channels) = (config.sample_rate, config.channels as usize);
        let synth = Arc::new(Mutex::new(Synth::new(sr as f32)));
        let s = synth.clone();
        let mut buf: Vec<f32> = Vec::new();
        let err = |e| eprintln!("Audio-Fehler: {e}");
        macro_rules! stream {
            ($t:ty, $conv:expr) => {
                device.build_output_stream(
                    config,
                    move |out: &mut [$t], _: &cpal::OutputCallbackInfo| {
                        let frames = out.len() / channels.max(1);
                        buf.resize(frames * 2, 0.);
                        match s.lock() {
                            Ok(mut synth) => synth.render(&mut buf),
                            Err(_) => buf.iter_mut().for_each(|v| *v = 0.),
                        }
                        for (i, frame) in out.chunks_mut(channels.max(1)).enumerate() {
                            for (c, v) in frame.iter_mut().enumerate() {
                                let x: f32 = buf[i * 2 + c.min(1)];
                                *v = $conv(x);
                            }
                        }
                    },
                    err,
                    None,
                )?
            };
        }
        let stream = match supported.sample_format() {
            cpal::SampleFormat::F32 => stream!(f32, |x: f32| x),
            cpal::SampleFormat::I16 => stream!(i16, |x: f32| (x * i16::MAX as f32) as i16),
            cpal::SampleFormat::U16 => {
                stream!(u16, |x: f32| ((x * 0.5 + 0.5) * u16::MAX as f32) as u16)
            }
            f => bail!("Nicht unterstütztes Sample-Format {f:?}"),
        };
        stream.play()?;
        let name = device
            .description()
            .map(|d| d.to_string())
            .unwrap_or_else(|_| "Standardausgabe".into());
        Ok(Self {
            synth,
            _stream: stream,
            sample_rate: sr,
            device: name,
        })
    }
    /// Parameter eines Bildes übernehmen.
    pub fn apply(&self, f: &Frame) {
        if let Ok(mut s) = self.synth.lock() {
            s.apply(f);
        }
    }
    /// Einzelnes Geräusch sofort abspielen (Menüklick).
    pub fn play(&self, s: crate::synth::Sfx) {
        if let Ok(mut synth) = self.synth.lock() {
            synth.play(s);
        }
    }
    pub fn toggle_mute(&self) -> bool {
        self.synth
            .lock()
            .map(|mut s| s.toggle_mute())
            .unwrap_or(true)
    }
}

/// Stereo-Abtastwerte (verschachtelt, −1…1) als 16-Bit-WAV schreiben.
pub fn write_wav(path: &std::path::Path, samples: &[f32], sample_rate: u32) -> Result<()> {
    let data_len = (samples.len() * 2) as u32;
    let mut b = Vec::with_capacity(44 + data_len as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes()); // PCM
    b.extend_from_slice(&2u16.to_le_bytes()); // Stereo
    b.extend_from_slice(&sample_rate.to_le_bytes());
    b.extend_from_slice(&(sample_rate * 4).to_le_bytes());
    b.extend_from_slice(&4u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for &v in samples {
        b.extend_from_slice(&((v.clamp(-1., 1.) * 32767.) as i16).to_le_bytes());
    }
    if let Some(dir) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, b).with_context(|| format!("{} schreiben", path.display()))
}

#[cfg(test)]
mod tests {
    #[test]
    fn wav_header() {
        let p = std::env::temp_dir().join(format!("gta-berlin-wav-{}.wav", std::process::id()));
        super::write_wav(&p, &[0., 1., -1., 0.5], 48000).unwrap();
        let b = std::fs::read(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert_eq!(&b[..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(b[4..8].try_into().unwrap()), 36 + 8);
        assert_eq!(u32::from_le_bytes(b[24..28].try_into().unwrap()), 48000);
        assert_eq!(i16::from_le_bytes(b[46..48].try_into().unwrap()), 32767);
        assert_eq!(b.len(), 44 + 8);
    }
}
