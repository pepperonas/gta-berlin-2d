//! Tageslicht aus der Spieluhr (Port von `daylight.js`): Sonnenrichtung und Schattenlänge, Umgebungslicht je
//! Farbkanal, Dunkelheit, Laternen und Anteil erleuchteter Fenster. Sonnenstand grob für Berlin im Sommer
//! (Sommerzeit): Aufgang 5:30, Mittag 13:00, Untergang 20:30.
use std::f64::consts::PI;

pub const SUNRISE: f64 = 330.;
pub const SUNSET: f64 = 1230.;
const MAX_ELEV: f64 = 58. * PI / 180.;
const AZ_RISE: f64 = 50.;
const AZ_SET: f64 = 310.;
/// Längster Schatten = 2,4 × Höhe.
pub const SHADOW_MAX: f64 = 2.4;

const AMBIENT: [(f64, [f64; 3]); 11] = [
    (0., [0.32, 0.39, 0.48]),
    (270., [0.32, 0.39, 0.48]),
    (330., [0.62, 0.52, 0.58]),
    (390., [0.95, 0.82, 0.72]),
    (480., [1., 1., 1.]),
    (1080., [1., 1., 1.]),
    (1170., [1., 0.88, 0.70]),
    (1230., [0.78, 0.58, 0.62]),
    (1290., [0.43, 0.47, 0.58]),
    (1350., [0.32, 0.39, 0.48]),
    (1440., [0.32, 0.39, 0.48]),
];
const WINDOWS: [(f64, f64); 8] = [
    (0., 0.35),
    (180., 0.12),
    (330., 0.1),
    (420., 0.),
    (1140., 0.),
    (1260., 0.55),
    (1380., 0.6),
    (1440., 0.35),
];

/// Schattenrichtung (zeigt von der Sonne weg; Norden oben, y nach Süden), Länge je Höhe und Stärke 0…1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sun {
    pub dx: f64,
    pub dy: f64,
    pub len: f64,
    pub strength: f64,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Light {
    pub minutes: f64,
    pub elevation: f64,
    /// Azimut in rad ab Nord im Uhrzeigersinn
    pub azimuth: f64,
    pub sun: Sun,
    /// Multiplikator je Farbkanal (1 = volles Tageslicht)
    pub ambient: [f64; 3],
    /// 0 Tag … 1 tiefe Nacht
    pub dark: f64,
    pub lamps_on: bool,
    pub windows_lit: f64,
}

pub fn wrap_minutes(m: f64) -> f64 {
    ((m % 1440.) + 1440.) % 1440.
}
fn smooth(a: f64, b: f64, x: f64) -> f64 {
    let u = ((x - a) / (b - a)).clamp(0., 1.);
    u * u * (3. - 2. * u)
}
fn lerp_ambient(m: f64) -> [f64; 3] {
    for w in AMBIENT.windows(2) {
        let ((m0, a), (m1, b)) = (w[0], w[1]);
        if m <= m1 {
            let u = (m - m0) / if m1 - m0 != 0. { m1 - m0 } else { 1. };
            return [
                a[0] + (b[0] - a[0]) * u,
                a[1] + (b[1] - a[1]) * u,
                a[2] + (b[2] - a[2]) * u,
            ];
        }
    }
    AMBIENT[AMBIENT.len() - 1].1
}
fn lerp_windows(m: f64) -> f64 {
    for w in WINDOWS.windows(2) {
        let ((m0, a), (m1, b)) = (w[0], w[1]);
        if m <= m1 {
            return a + (b - a) * (m - m0) / (m1 - m0);
        }
    }
    WINDOWS[WINDOWS.len() - 1].1
}

pub fn light_at(minutes: f64) -> Light {
    let m = wrap_minutes(minutes);
    let d = (m - SUNRISE) / (SUNSET - SUNRISE);
    let elevation = (PI * d).sin() * MAX_ELEV;
    let azimuth = (AZ_RISE + (AZ_SET - AZ_RISE) * d.clamp(0., 1.)) * PI / 180.;
    let len = SHADOW_MAX.min(1. / elevation.max(0.02).tan());
    let strength = smooth(0., 7. * PI / 180., elevation);
    let ambient = lerp_ambient(m);
    let lum = 0.3 * ambient[0] + 0.55 * ambient[1] + 0.15 * ambient[2];
    Light {
        minutes: m,
        elevation,
        azimuth,
        sun: Sun {
            dx: -azimuth.sin(),
            dy: azimuth.cos(),
            len,
            strength,
        },
        ambient,
        dark: ((1. - lum) / 0.62).clamp(0., 1.),
        lamps_on: m >= 1215. || m < 345.,
        windows_lit: lerp_windows(m),
    }
}

/// „21:30“
pub fn format_clock(minutes: f64) -> String {
    let m = wrap_minutes(minutes).floor() as i64;
    format!("{:02}:{:02}", m / 60, m % 60)
}
/// „21:30“ → 1290; ungültig → None
pub fn parse_clock(s: &str) -> Option<f64> {
    let (h, m) = s.trim().split_once(':')?;
    if h.is_empty()
        || h.len() > 2
        || m.len() != 2
        || !h.chars().chain(m.chars()).all(|c| c.is_ascii_digit())
    {
        return None;
    }
    let (h, m): (u32, u32) = (h.parse().ok()?, m.parse().ok()?);
    (h <= 23 && m <= 59).then_some((h * 60 + m) as f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn noon_night_and_dusk() {
        let noon = light_at(13. * 60.);
        assert!((noon.elevation - MAX_ELEV).abs() < 1e-9);
        assert_eq!(
            (noon.ambient, noon.dark, noon.lamps_on),
            ([1., 1., 1.], 0., false)
        );
        // Mittag: Sonne im Süden, Schatten zeigt nach Norden (−y)
        assert!(
            noon.sun.strength == 1. && noon.sun.dy < -0.99,
            "{:?}",
            noon.sun
        );
        let night = light_at(2. * 60.);
        assert!(night.dark > 0.9 && night.lamps_on && night.sun.strength == 0.);
        assert!((night.windows_lit - (0.35 + (0.12 - 0.35) * 120. / 180.)).abs() < 1e-12);
        // Abends: Sonne im Westen, Schatten nach Osten (+x), lang
        let eve = light_at(19. * 60.);
        assert!(eve.sun.dx > 0.5 && eve.sun.len > 1.5 && eve.sun.len <= SHADOW_MAX);
        assert_eq!(light_at(-60.).minutes, 1380.);
    }
    #[test]
    fn clock_text() {
        assert_eq!(format_clock(1290.), "21:30");
        assert_eq!(format_clock(-1.), "23:59");
        assert_eq!(parse_clock("21:30"), Some(1290.));
        assert_eq!(parse_clock(" 7:05 "), Some(425.));
        for bad in ["24:00", "7:5", "a:00", "", "12:60", "123:00"] {
            assert_eq!(parse_clock(bad), None, "{bad}");
        }
    }
}
