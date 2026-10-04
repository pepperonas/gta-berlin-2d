//! Wetter (Port von `weather.js`, rein rechnerisch): für jeden Tag in Blöcken zu 3 Stunden ein Wetterbild aus einem
//! Hash von Welt-Samen und Tagnummer – gleiche Welt, gleiches Wetter, ohne Zufallszahlen der Simulation; 45 Minuten
//! Überblendung zwischen Blöcken. Dazu Wind und Böen, Blitze und Donner, Temperatur, Nässe, Schneedecke und Glätte
//! des Bodens sowie die Wirkung auf das Licht.
use crate::daylight::Light;
use crate::math::{hash01, to_uint32};
use std::f64::consts::PI;

pub const KINDS: [&str; 11] = [
    "clear",
    "cloudy",
    "overcast",
    "rain",
    "heavyrain",
    "storm",
    "thunder",
    "fog",
    "densefog",
    "snow",
    "heavysnow",
];
pub fn label(kind: &str) -> &'static str {
    match kind {
        "clear" => "sonnig",
        "cloudy" => "wolkig",
        "overcast" => "bedeckt",
        "rain" => "Regen",
        "heavyrain" => "Starkregen",
        "storm" => "Sturm",
        "thunder" => "Gewitter",
        "fog" => "Nebel",
        "densefog" => "dichter Nebel",
        "snow" => "Schnee",
        "heavysnow" => "Schneesturm",
        _ => "",
    }
}

/// Werte eines Wetterbilds: Bewölkung, Regen (1 Landregen, 1,6 Starkregen), Nebel (1, 1,7 dicht), Schneefall
/// (0,45 leicht … 1 stark), Sturm (0…1) und Gewitter (0…1).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Params {
    pub cloud: f64,
    pub rain: f64,
    pub fog: f64,
    pub snow: f64,
    pub storm: f64,
    pub thunder: f64,
}
const fn p(cloud: f64, rain: f64, fog: f64, snow: f64, storm: f64, thunder: f64) -> Params {
    Params {
        cloud,
        rain,
        fog,
        snow,
        storm,
        thunder,
    }
}
pub fn params(kind: &str) -> Params {
    match kind {
        "cloudy" => p(0.45, 0., 0., 0., 0., 0.),
        "overcast" => p(0.85, 0., 0., 0., 0., 0.),
        "rain" => p(0.95, 1., 0., 0., 0., 0.),
        "heavyrain" => p(1., 1.6, 0.15, 0., 0., 0.),
        "storm" => p(0.9, 0.7, 0., 0., 1., 0.),
        "thunder" => p(1., 1.4, 0.1, 0., 0.6, 1.),
        "fog" => p(0.6, 0., 1., 0., 0., 0.),
        "densefog" => p(0.7, 0., 1.7, 0., 0., 0.),
        "snow" => p(0.9, 0., 0.1, 0.45, 0., 0.),
        "heavysnow" => p(1., 0., 0.45, 1., 0.5, 0.),
        _ => p(0.08, 0., 0., 0., 0., 0.),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Weather {
    pub kind: &'static str,
    pub p: Params,
    /// Wolkenzug (px/s)
    pub wind: (f64, f64),
}

pub const BLOCK: f64 = 180.;
pub const BLEND: f64 = 45.;
pub const WET_RISE: f64 = 1. / 40.;
pub const WET_DRY: f64 = 1. / 600.;
pub const SNOW_RISE: f64 = 1. / 100.;
pub const SNOW_MELT: f64 = 1. / 1500.;
pub const SNOW_RAIN_MELT: f64 = 1. / 150.;
pub const ICE_RISE: f64 = 1. / 10.;
pub const ICE_MELT: f64 = 1. / 20.;
/// Eisregen: Regen bei Frost überzieht alles in rund 2 Spielminuten mit Glatteis; taut über 0 °C ab
pub const GLAZE_RISE: f64 = 1. / 120.;
pub const GLAZE_MELT: f64 = 1. / 60.;
/// Glatteis aus Eisregen (Regenstärke `rain`, Lufttemperatur `temp_c`).
pub fn step_glaze(glaze: f64, rain: f64, temp_c: f64, dt: f64) -> f64 {
    if temp_c > 0. {
        (glaze - GLAZE_MELT * dt).max(0.)
    } else if rain > 0.05 {
        (glaze + GLAZE_RISE * rain.min(1.) * dt).min(1.)
    } else {
        glaze
    }
}

/// JS `x | 0`: ToInt32.
fn i32of(x: f64) -> f64 {
    to_uint32(x) as i32 as f64
}

/// Wetterlage eines Tages.
pub fn day_type(seed: u32, day: i64) -> &'static str {
    let r = hash01(i32of(seed as f64 * 104729. + day as f64 * 7121. + 91.));
    if r < 0.14 {
        "winter"
    } else if r < 0.28 {
        "unsettled"
    } else {
        "normal"
    }
}

/// Wetterbild eines Blocks (Tag, Block 0…7).
pub fn block_kind(seed: u32, day: u32, block: u32) -> &'static str {
    let (s, d, b) = (seed as f64, day as f64, block as f64);
    let r = hash01(i32of(s * 7919. + d * 131. + b * 17. + 3.));
    let r2 = hash01(i32of(s * 613. + d * 29. + b * 101. + 11.));
    let morning = block == 1 || block == 2;
    let ty = day_type(seed, day as i64);
    if morning && r < if ty == "winter" { 0.2 } else { 0.12 } {
        return if r2 < 0.35 { "densefog" } else { "fog" };
    }
    let p = if morning { r - 0.12 } else { r };
    match ty {
        "winter" => {
            if p < 0.22 {
                "overcast"
            } else if p < 0.36 {
                "cloudy"
            } else if p < 0.74 {
                "snow"
            } else {
                "heavysnow"
            }
        }
        "unsettled" => {
            let warm = (4..=6).contains(&block);
            if p < 0.16 {
                "clear"
            } else if p < 0.34 {
                "cloudy"
            } else if p < 0.48 {
                "overcast"
            } else if p < 0.62 {
                "rain"
            } else if p < 0.72 {
                "heavyrain"
            } else if p < 0.86 || !warm {
                "storm"
            } else {
                "thunder"
            }
        }
        _ => {
            if p < 0.34 {
                "clear"
            } else if p < 0.62 {
                "cloudy"
            } else if p < 0.8 {
                "overcast"
            } else if p < 0.93 {
                "rain"
            } else {
                "heavyrain"
            }
        }
    }
}

fn wind_at(seed: u32, day: u32, storm: f64) -> (f64, f64) {
    let (s, d) = (seed as f64, day as f64);
    let a = hash01(s * 31. + d * 7. + 1.) * PI * 2.;
    let v = 12. + hash01(s + d * 13.) * 30. + storm * 190.;
    (a.cos() * v, a.sin() * v)
}

/// Wetter zur Uhrzeit, stetig über Blockgrenzen und Mitternacht; `force` erzwingt ein Wetterbild.
pub fn weather_at(seed: u32, day: u32, minutes: f64, force: Option<&'static str>) -> Weather {
    if let Some(k) = force {
        let p = params(k);
        return Weather {
            kind: k,
            p,
            wind: wind_at(seed, day, p.storm),
        };
    }
    let m = ((minutes % 1440.) + 1440.) % 1440.;
    let b = (m / BLOCK).floor() as u32;
    let into = m - b as f64 * BLOCK;
    let k0 = block_kind(seed, day, b);
    let nb = (b + 1) % 8;
    let k1 = block_kind(seed, if nb == 0 { day + 1 } else { day }, nb);
    let u = if into < BLOCK - BLEND {
        0.
    } else {
        (into - (BLOCK - BLEND)) / BLEND
    };
    let s = u * u * (3. - 2. * u);
    let (a, c) = (params(k0), params(k1));
    let mix = |x: f64, y: f64| x + (y - x) * s;
    let p = Params {
        cloud: mix(a.cloud, c.cloud),
        rain: mix(a.rain, c.rain),
        fog: mix(a.fog, c.fog),
        snow: mix(a.snow, c.snow),
        storm: mix(a.storm, c.storm),
        thunder: mix(a.thunder, c.thunder),
    };
    Weather {
        kind: if s < 0.5 { k0 } else { k1 },
        p,
        wind: wind_at(seed, day, p.storm),
    }
}

/// Böen: Faktor auf den Wind (1 = mittlerer Wind), bei Sturm heftig und unregelmäßig.
pub fn gust_at(storm: f64, t: f64) -> f64 {
    if storm <= 0. {
        return 1.;
    }
    let g = 0.5 * (t * 0.37).sin() + 0.3 * (t * 1.13 + 1.7).sin() + 0.2 * (t * 2.71 + 0.4).sin();
    (1. + storm * (0.55 * g + 0.25 * (t * 0.21 + 2.).sin().max(0.).powi(6))).max(0.2)
}

// --- Gewitter ----------------------------------------------------------------------------------
pub const STRIKE_SLOT: f64 = 2.4;
pub const STRIKE_CHANCE: f64 = 0.2;
pub const SOUND_SPEED: f64 = 3430.;
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Strike {
    pub t0: f64,
    pub dx: f64,
    pub dy: f64,
    pub dist: f64,
    pub near: bool,
}
/// Blitz im Zeitfenster i (oder keiner).
pub fn strike_in_slot(seed: u32, i: i64, thunder: f64) -> Option<Strike> {
    if thunder <= 0.02 {
        return None;
    }
    let (s, i) = (seed as f64, i as f64);
    if hash01(i32of(s * 48271. + i * 7867. + 5.)) > STRIKE_CHANCE * thunder {
        return None;
    }
    let hx = |k: f64| hash01(i32of(s * 1597. + i * 389. + k * 7919.));
    let near = hx(1.) < 0.3;
    let a = hx(2.) * PI * 2.;
    let dist = if near {
        900. + hx(3.) * 4000.
    } else {
        10000. + hx(3.) * 60000.
    };
    Some(Strike {
        t0: (i + hx(4.) * 0.7) * STRIKE_SLOT,
        dx: a.cos() * dist,
        dy: a.sin() * dist,
        dist,
        near,
    })
}
/// Helligkeit eines Blitzes `age` Sekunden nach dem Einschlag.
pub fn flash_at(age: f64) -> f64 {
    if !(0. ..=0.9).contains(&age) {
        return 0.;
    }
    [
        (0., 0.35, 40.),
        (0.06, 1., 14.),
        (0.21, 0.7, 16.),
        (0.37, 0.45, 18.),
    ]
    .iter()
    .filter(|(t0, ..)| age >= *t0)
    .map(|(t0, a, k)| a * (-(age - t0) * k).exp())
    .fold(0., f64::max)
}
/// Helligkeit aller Blitze zur Zeit t (nahe voll, ferne gedämpft).
pub fn flash_total(seed: u32, t: f64, thunder: f64) -> f64 {
    let i1 = (t / STRIKE_SLOT).floor() as i64;
    (i1 - 2..=i1)
        .filter_map(|i| strike_in_slot(seed, i, thunder))
        .map(|s| flash_at(t - s.t0) * if s.near { 1. } else { 0.45 })
        .fold(0., f64::max)
}
/// Donner, der im Zeitraum (t0, t1] ankommt: (Lautstärke 0…1, nah).
pub fn thunder_between(seed: u32, t0: f64, t1: f64, thunder: f64) -> Vec<(f64, bool)> {
    let mut out = Vec::new();
    if t1 <= t0 || thunder <= 0.02 {
        return out;
    }
    let max_delay = 72000. / SOUND_SPEED;
    for i in ((t0 - max_delay) / STRIKE_SLOT).floor() as i64 - 1..=(t1 / STRIKE_SLOT).floor() as i64
    {
        let Some(s) = strike_in_slot(seed, i, thunder) else {
            continue;
        };
        let at = s.t0 + s.dist / SOUND_SPEED;
        if at > t0 && at <= t1 {
            out.push(((1.6 / (s.dist / 1000.).sqrt()).min(1.), s.near));
        }
    }
    out
}

// --- Boden und Temperatur ----------------------------------------------------------------------
pub fn step_snow(snow: f64, wx: &Params, dt: f64) -> f64 {
    if wx.snow > 0.05 {
        (snow + SNOW_RISE * wx.snow * dt).min(1.)
    } else {
        (snow - (SNOW_MELT + SNOW_RAIN_MELT * wx.rain.min(1.)) * dt).max(0.)
    }
}
pub fn step_wet(wet: f64, rain: f64, dt: f64) -> f64 {
    if rain > 0.05 {
        (wet + WET_RISE * rain * dt).min(1.)
    } else {
        (wet - WET_DRY * dt).max(0.)
    }
}
pub fn step_ice(ice: f64, wet: f64, temp_c: f64, dt: f64) -> f64 {
    if temp_c > 0. {
        (ice - ICE_MELT * dt).max(0.)
    } else if wet > 0.1 {
        (ice + ICE_RISE * dt).min(1.)
    } else {
        ice
    }
}
fn temp_range(seed: u32, day: i64) -> (f64, f64) {
    let (lo, hi) = match day_type(seed, day) {
        "winter" => (-6., 3.),
        "unsettled" => (4., 14.),
        _ => (8., 22.),
    };
    let sh = (hash01(i32of(seed as f64 * 7907. + day as f64 * 3571. + 17.)) - 0.5) * 4.;
    (lo + sh, hi + sh)
}
/// Temperatur (°C): Tiefstwert um 5 Uhr, Höchstwert um 15 Uhr; erzwungener Schnee höchstens +1 °C.
pub fn temperature_at(seed: u32, day: u32, minutes: f64, force: Option<&str>) -> f64 {
    let m = ((minutes % 1440.) + 1440.) % 1440.;
    let ease = |u: f64| 0.5 - 0.5 * (PI * u).cos();
    let (t_low, t_high) = (300., 900.);
    let v = if (t_low..=t_high).contains(&m) {
        let (lo, hi) = temp_range(seed, day as i64);
        lo + (hi - lo) * ease((m - t_low) / (t_high - t_low))
    } else {
        let d = if m > t_high {
            day as i64
        } else {
            day as i64 - 1
        };
        let u = ((if m > t_high { m } else { m + 1440. }) - t_high) / (1440. - t_high + t_low);
        let hi = temp_range(seed, d).1;
        let lo = temp_range(seed, d + 1).0;
        hi + (lo - hi) * ease(u)
    };
    if matches!(force, Some("snow" | "heavysnow")) {
        v.min(1.)
    } else {
        v
    }
}

/// Licht an das Wetter anpassen (Wolken nehmen Schatten, Regen/Nebel machen den Tag grau, Schneedecke hellt auf).
pub fn weather_light(l: &Light, wx: &Params, cover: f64) -> Light {
    let (cloud, rain, fog, snowing) = (wx.cloud, wx.rain.min(1.6), wx.fog.min(1.7), wx.snow);
    let dim = (0.22 * cloud + 0.1 * rain + 0.08 * fog + 0.08 * snowing) * (1. - 0.45 * cover);
    let mut ambient = l.ambient;
    for (i, v) in ambient.iter_mut().enumerate() {
        *v *= (1. - dim) * if i == 2 { 1. } else { 1. - 0.04 * cloud };
    }
    let gloom = (0.55 * rain.min(1.)
        + 0.25 * (rain - 1.).max(0.)
        + 0.45 * fog.min(1.)
        + 0.5 * snowing
        + 0.25 * (cloud - 0.8).max(0.) / 0.2)
        .min(1.);
    let mut out = *l;
    out.windows_lit = l
        .windows_lit
        .max(if gloom > 0.3 { 0.3 * gloom } else { 0. });
    out.sun.strength = l.sun.strength * (1. - 0.85 * cloud);
    out.ambient = ambient;
    let murk = 0.3 * rain.min(1.)
        + 0.12 * (rain - 1.).max(0.)
        + 0.25 * fog.min(1.)
        + 0.1 * (fog - 1.).max(0.)
        + 0.2 * snowing;
    out.dark = (l.dark.max(murk) - 0.18 * cover * l.dark - 0.08 * cover).clamp(0., 1.);
    out.lamps_on = l.lamps_on || ((rain > 0.6 || fog > 0.6 || snowing > 0.6) && l.dark > 0.15);
    out
}

/// Bei Regen, Nebel, Schnee und Sturm gehen weniger Menschen raus.
pub fn people_factor(wx: &Params) -> f64 {
    (1. - 0.45 * wx.rain - 0.15 * wx.fog.min(1.) - 0.35 * wx.snow - 0.3 * wx.storm).max(0.15)
}

#[cfg(test)]
mod tests {
    use super::*;
    // Referenzwerte aus weather.js (Node)
    #[test]
    fn matches_js_reference() {
        assert!((0..10).all(|d| day_type(1996, d) == "normal"));
        assert_eq!(day_type(1, 3), "winter");
        let blocks: Vec<_> = (0..8).map(|b| block_kind(1996, 3, b)).collect();
        assert_eq!(
            blocks,
            [
                "clear",
                "clear",
                "clear",
                "heavyrain",
                "clear",
                "rain",
                "cloudy",
                "rain"
            ]
        );
        let w = weather_at(1996, 2, 500., None);
        assert_eq!(w.kind, "rain");
        assert!(
            (w.p.rain - 0.9657064471879286).abs() < 1e-12
                && (w.p.cloud - 0.920164609053498).abs() < 1e-12
        );
        assert!(
            (w.wind.0 - 11.410728325608332).abs() < 1e-9
                && (w.wind.1 - 8.553294270945335).abs() < 1e-9
        );
        let w2 = weather_at(1989, 5, 1430., None);
        assert_eq!(w2.kind, "snow");
        assert!(
            (w2.p.rain - 0.12620027434842251).abs() < 1e-12
                && (w2.p.fog - 0.08737997256515775).abs() < 1e-12
        );
        for (got, want) in [
            (temperature_at(1996, 0, 300., None), 7.4617810705676675),
            (temperature_at(1996, 0, 900., None), 21.461781070567667),
            (temperature_at(1996, 1, 100., None), 11.260246445658186),
            (temperature_at(1989, 4, 1200., None), 18.712410648571762),
        ] {
            assert!((got - want).abs() < 1e-9, "{got} statt {want}");
        }
        let s = (0..200).find_map(|i| strike_in_slot(1996, i, 1.)).unwrap();
        assert!(
            (s.t0 - 8.509322741422801).abs() < 1e-9
                && (s.dist - 19401.372163556516).abs() < 1e-6
                && !s.near
        );
        assert_eq!(thunder_between(1996, 0., 120., 1.).len(), 8);
        assert!((gust_at(1., 12.3) - 0.8272813550744987).abs() < 1e-12);
    }
    #[test]
    fn ground_and_light() {
        let rain = params("heavyrain");
        let mut wet = 0.;
        for _ in 0..30 {
            wet = step_wet(wet, rain.rain, 1.);
        }
        assert!(wet > 0.99, "nach 30 s Starkregen nass: {wet}");
        assert!(step_wet(1., 0., 60.) < 0.91);
        assert!(step_ice(0., 0.5, -2., 5.) == 0.5 && step_ice(0.5, 0.5, 3., 5.) == 0.25);
        assert!(step_snow(0., &params("heavysnow"), 50.) == 0.5);
        let noon = crate::daylight::light_at(13. * 60.);
        let grey = weather_light(&noon, &params("rain"), 0.);
        assert!(
            grey.sun.strength < 0.25
                && grey.ambient[0] < 0.8
                && grey.dark > 0.25
                && grey.windows_lit > 0.1
        );
        assert_eq!(weather_light(&noon, &params("clear"), 0.).dark, 0.);
        assert!(people_factor(&params("heavyrain")) < 0.4);
        assert!(flash_at(0.06) > 0.99 && flash_at(1.) == 0.);
    }
}
