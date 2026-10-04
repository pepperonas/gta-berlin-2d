//! Untergrund je Rad (Fahrphysik Phase 4, rein): aus dem Belag unter dem Rad und dem Wetter am Boden wird eine
//! gewichtete Mischung der Untergründe aus `data/vehicles/surfaces.json`, daraus mit dem Reifen (Faktor je
//! Kategorie trocken/nass/schnee/eis/lose) der `vphys::Ground` des Rads.
//!
//! Regeln:
//! - Nässe mischt zwischen trockenem und nassem Belag. Starkregen legt auf Hauptstraßen einen Wasserfilm in die
//!   Spurrinnen (Aquaplaning ab 2,5 mm), Pfützen am Fahrbahnrand sind tiefer.
//! - Schnee: Hauptstraßen werden festgefahren (über 0 °C Matsch), Nebenstraßen bleiben Neuschnee.
//! - Glätte: Reif- und Blankeis (`eis`), Eisregen legt flächig Glatteis (`glatteis`).
//! - Überdachte Stellen (Durchfahrt, unter einer Brücke) bleiben trocken; Brücken frieren zuerst (das steckt
//!   schon in `traction::road_condition`).
//! - Straßenbahnschienen liegen in der Fahrbahn: das Rad auf der Schiene hat deren Haftung (nass sehr wenig).
use crate::vehdata::{Tire, VehicleDb};
use crate::vphys::Ground;

/// Belag unter einem Rad.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Material {
    Asphalt,
    Concrete,
    Cobble,
    Plates,
    Unpaved,
    Grass,
    Sand,
    /// Wasser (Ufer, Fluss): wie nasses Gras, sehr zäh
    Water,
}

/// Was am Rad liegt.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spot {
    pub material: Material,
    /// Hauptstraße (Spurrinnen, festgefahrener Schnee, Wasserfilm)
    pub main: bool,
    /// das Rad steht auf einer Straßenbahnschiene
    pub rail: bool,
    /// das Rad steht in einer Pfütze
    pub puddle: bool,
}
impl Default for Spot {
    fn default() -> Self {
        Self {
            material: Material::Asphalt,
            main: false,
            rail: false,
            puddle: false,
        }
    }
}

/// Wetter am Boden (0…1; `rain` = Regenstärke am Himmel bis 1,6, `temp` in °C).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Weather {
    pub wet: f64,
    pub snow: f64,
    pub ice: f64,
    /// Glatteis aus Eisregen
    pub glaze: f64,
    pub rain: f64,
    pub temp: f64,
    /// überdacht: trocken, kein Schnee, kein Eis
    pub covered: bool,
}

/// Regen ab dieser Stärke legt auf Hauptstraßen einen Wasserfilm.
pub const FILM_RAIN: f64 = 1.1;
/// Wasserhöhe (mm): Film bei Regenstärke 1,6, Pfütze
pub const FILM_MM: f64 = 3.2;
pub const PUDDLE_MM: f64 = 6.;
/// Unebenheit je Belag
pub const ROUGH_COBBLE: f64 = 0.6;
pub const ROUGH_UNPAVED: f64 = 0.4;
pub const ROUGH_GRASS: f64 = 0.3;
pub const ROUGH_PLATES: f64 = 0.15;

/// Mischung von Untergründen (Kennung aus `surfaces.json`, Gewicht), Wasserhöhe und Unebenheit.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Mix {
    pub parts: Vec<(&'static str, f64)>,
    pub water_mm: f64,
    pub rough: f64,
}
impl Mix {
    fn add(&mut self, id: &'static str, w: f64) {
        if w <= 0. {
            return;
        }
        match self.parts.iter_mut().find(|p| p.0 == id) {
            Some(p) => p.1 += w,
            None => self.parts.push((id, w)),
        }
    }
    /// Alle Gewichte mit `k` malnehmen (für das Überblenden zu einer neuen Schicht).
    fn scale(&mut self, k: f64) {
        for p in &mut self.parts {
            p.1 *= k;
        }
        self.parts.retain(|p| p.1 > 1e-9);
    }
    /// Gewicht eines Untergrunds (Tests, Anzeige).
    pub fn weight(&self, id: &str) -> f64 {
        self.parts.iter().filter(|p| p.0 == id).map(|p| p.1).sum()
    }
    /// Der Untergrund mit dem größten Gewicht.
    pub fn main(&self) -> &'static str {
        self.parts
            .iter()
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map_or("asphalt_trocken", |p| p.0)
    }
}

fn clamp01(v: f64) -> f64 {
    v.clamp(0., 1.)
}

/// Trockener und nasser Untergrund eines Belags.
fn pair(m: Material) -> (&'static str, &'static str) {
    match m {
        Material::Asphalt => ("asphalt_trocken", "asphalt_nass"),
        Material::Concrete | Material::Plates => ("beton_trocken", "beton_nass"),
        Material::Cobble => ("kopfstein_trocken", "kopfstein_nass"),
        Material::Unpaved => ("schotter", "schotter"),
        Material::Grass | Material::Water => ("gras_trocken", "gras_nass"),
        Material::Sand => ("sand", "sand"),
    }
}

/// Untergrund-Mischung an einem Rad.
pub fn resolve(spot: &Spot, w: &Weather) -> Mix {
    let mut m = Mix {
        rough: match spot.material {
            Material::Cobble => ROUGH_COBBLE,
            Material::Unpaved => ROUGH_UNPAVED,
            Material::Grass | Material::Water => ROUGH_GRASS,
            Material::Plates => ROUGH_PLATES,
            _ => 0.,
        },
        ..Default::default()
    };
    let (dry, wet_id) = if spot.rail {
        ("schiene_trocken", "schiene_nass")
    } else {
        pair(spot.material)
    };
    let wet = if spot.material == Material::Water {
        1.
    } else if w.covered {
        0.
    } else {
        clamp01(w.wet)
    };
    m.add(dry, 1. - wet);
    // Wasserfilm in den Spurrinnen der Hauptstraßen bei Starkregen; Pfützen tiefer
    let paved = matches!(
        spot.material,
        Material::Asphalt | Material::Concrete | Material::Plates | Material::Cobble
    );
    let film =
        if !w.covered && spot.main && paved && spot.material == Material::Asphalt && !spot.rail {
            clamp01((w.rain - FILM_RAIN) / (1.6 - FILM_RAIN)) * wet
        } else {
            0.
        };
    m.add(wet_id, wet * (1. - film));
    m.add("asphalt_wasserfilm", wet * film);
    if !w.covered && paved {
        m.water_mm = if spot.puddle && wet > crate::traction::PUDDLE_WET {
            PUDDLE_MM * wet
        } else {
            FILM_MM * film
        };
    }
    if w.covered {
        return m;
    }
    // Schnee: festgefahren bzw. Matsch auf Hauptstraßen, Neuschnee auf Nebenstraßen
    let snow = clamp01(w.snow);
    if snow > 0. {
        let id = if !paved || !spot.main {
            "neuschnee"
        } else if w.temp > 0. {
            "matsch"
        } else {
            "schnee_fest"
        };
        m.scale(1. - snow);
        m.add(id, snow);
    }
    let ice = clamp01(w.ice);
    if ice > 0. {
        m.scale(1. - ice);
        m.add("eis", ice);
    }
    let glaze = clamp01(w.glaze);
    if glaze > 0. {
        m.scale(1. - glaze);
        m.add("glatteis", glaze);
    }
    m
}

/// `vphys::Ground` für einen Reifen: Haftung und Rollwiderstand gewichtet über die Mischung, der Reifenfaktor je
/// Kategorie ist eingerechnet (`tire_factor` bleibt 1).
pub fn ground(db: &VehicleDb, tire: &Tire, mix: &Mix) -> Ground {
    let (mut mu, mut roll, mut sum) = (0., 0., 0.);
    for &(id, w) in &mix.parts {
        let Ok(s) = db.surface(id) else {
            continue;
        };
        let f = tire.factor.get(&s.category).copied().unwrap_or(1.);
        mu += s.mu_rel * f * w;
        roll += s.rolling_extra * w;
        sum += w;
    }
    if sum <= 0. {
        return Ground::DRY;
    }
    Ground {
        mu_rel: mu / sum,
        tire_factor: 1.,
        rolling_extra: roll / sum,
        rough: mix.rough,
        water_mm: mix.water_mm,
        curb: 0.,
        groove: 0.,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vehdata::shared;

    fn asphalt(main: bool) -> Spot {
        Spot {
            main,
            ..Default::default()
        }
    }

    #[test]
    fn dry_and_wet_asphalt_match_the_table() {
        let db = shared();
        let tire = &db.tires["sommer_std"];
        let g = ground(db, tire, &resolve(&asphalt(true), &Weather::default()));
        assert_eq!(g.mu_rel, 1.);
        let wet = Weather {
            wet: 1.,
            rain: 1.,
            ..Default::default()
        };
        let g = ground(db, tire, &resolve(&asphalt(true), &wet));
        assert!((g.mu_rel - 0.72).abs() < 1e-9, "{}", g.mu_rel);
        assert_eq!(g.water_mm, 0.);
    }

    #[test]
    fn heavy_rain_lays_a_water_film_on_main_roads_only() {
        let heavy = Weather {
            wet: 1.,
            rain: 1.6,
            ..Default::default()
        };
        let main = resolve(&asphalt(true), &heavy);
        assert!(main.weight("asphalt_wasserfilm") > 0.99);
        assert!(main.water_mm >= 3.);
        let side = resolve(&asphalt(false), &heavy);
        assert_eq!(side.weight("asphalt_wasserfilm"), 0.);
        assert_eq!(side.water_mm, 0.);
        // Pfütze am Rand, auch in Nebenstraßen
        let puddle = resolve(
            &Spot {
                puddle: true,
                ..asphalt(false)
            },
            &heavy,
        );
        assert!(puddle.water_mm > 5.);
        // überdacht: trocken
        let cov = resolve(
            &asphalt(true),
            &Weather {
                covered: true,
                ..heavy
            },
        );
        assert_eq!(cov.main(), "asphalt_trocken");
        assert_eq!(cov.water_mm, 0.);
    }

    #[test]
    fn snow_is_packed_on_main_roads_slush_when_mild_fresh_on_side_streets() {
        let snow = Weather {
            snow: 1.,
            temp: -3.,
            ..Default::default()
        };
        assert_eq!(resolve(&asphalt(true), &snow).main(), "schnee_fest");
        assert_eq!(resolve(&asphalt(false), &snow).main(), "neuschnee");
        let mild = Weather { temp: 1.5, ..snow };
        assert_eq!(resolve(&asphalt(true), &mild).main(), "matsch");
    }

    #[test]
    fn winter_tyres_grip_far_better_on_snow_than_summer_ones() {
        let db = shared();
        let mix = resolve(
            &asphalt(false),
            &Weather {
                snow: 1.,
                temp: -2.,
                ..Default::default()
            },
        );
        let summer = ground(db, &db.tires["sommer_uhp"], &mix).mu_rel * db.tires["sommer_uhp"].mu;
        let winter = ground(db, &db.tires["winter"], &mix).mu_rel * db.tires["winter"].mu;
        assert!(winter > summer * 1.8, "Winter {winter}, Sommer {summer}");
    }

    #[test]
    fn wet_cobbles_and_wet_rails_are_treacherous() {
        let db = shared();
        let tire = &db.tires["sommer_std"];
        let wet = Weather {
            wet: 1.,
            rain: 1.,
            ..Default::default()
        };
        let cob = ground(
            db,
            tire,
            &resolve(
                &Spot {
                    material: Material::Cobble,
                    ..Default::default()
                },
                &wet,
            ),
        );
        assert!((cob.mu_rel - 0.5).abs() < 1e-9);
        assert!(cob.rough > 0.5);
        let rail = ground(
            db,
            tire,
            &resolve(
                &Spot {
                    rail: true,
                    ..Default::default()
                },
                &wet,
            ),
        );
        assert!((rail.mu_rel - 0.3).abs() < 1e-9);
    }

    #[test]
    fn glaze_and_ice_cover_everything_but_not_under_a_roof() {
        let w = Weather {
            glaze: 1.,
            ..Default::default()
        };
        assert_eq!(resolve(&asphalt(true), &w).main(), "glatteis");
        let ice = Weather {
            ice: 0.6,
            ..Default::default()
        };
        assert!((resolve(&asphalt(true), &ice).weight("eis") - 0.6).abs() < 1e-9);
        let cov = Weather { covered: true, ..w };
        assert_eq!(resolve(&asphalt(true), &cov).main(), "asphalt_trocken");
    }
}
