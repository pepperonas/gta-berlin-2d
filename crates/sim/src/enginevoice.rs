//! Motorcharaktere (Port von `enginevoice.js`): Syntheseprofile je Bauart (Auspuffimpulse, Resonanz, Helligkeit,
//! Rauheit, Turbo, Dämmung, Bankversatz) und das Fourierspektrum eines kompletten Arbeitszyklus. Keine Aufnahmen
//! konkreter Fabrikate.
use std::f64::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Voice {
    pub id: &'static str,
    pub resonance: f64,
    pub brightness: f64,
    pub rough: f64,
    pub width: f64,
    pub volume: f64,
    pub turbo: f64,
    pub insulation: f64,
    pub bank: f64,
    pub pops: bool,
}
#[allow(clippy::too_many_arguments)]
const fn p(
    id: &'static str,
    resonance: f64,
    brightness: f64,
    rough: f64,
    width: f64,
    volume: f64,
    turbo: f64,
    insulation: f64,
    bank: f64,
) -> Voice {
    Voice {
        id,
        resonance,
        brightness,
        rough,
        width,
        volume,
        turbo,
        insulation,
        bank,
        pops: false,
    }
}
pub const VOICES: &[Voice] = &[
    p("triple", 120., 1000., 0.16, 0.044, 0.9, 0.35, 0.6, 0.),
    p("four", 130., 1050., 0.09, 0.038, 0.82, 0.35, 0.6, 0.),
    p("six", 110., 1150., 0.06, 0.035, 0.82, 0.3, 0.76, 0.),
    p("five", 125., 1150., 0.14, 0.04, 0.98, 0.4, 0.6, 0.),
    p("boxer", 105., 950., 0.17, 0.048, 1., 0., 0.32, 0.3),
    p("flatSix", 130., 1550., 0.13, 0.035, 1.12, 0., 0.38, 0.23),
    p("v8", 85., 850., 0.3, 0.065, 1.25, 0., 0.32, 0.62),
    p("v8turbo", 95., 1150., 0.21, 0.05, 1.12, 0.85, 0.64, 0.45),
    p("flatV8", 140., 1800., 0.1, 0.032, 1.12, 0., 0.38, 0.),
    p("v10", 155., 2100., 0.085, 0.03, 1.15, 0., 0.3, 0.18),
    p("diesel4", 100., 800., 0.19, 0.034, 0.97, 0.72, 0.65, 0.),
    p("diesel6", 80., 700., 0.15, 0.044, 1.05, 0.8, 0.65, 0.),
    p("heavy", 65., 600., 0.22, 0.055, 1.3, 1., 0.4, 0.),
    p("twoStroke", 310., 2700., 0.21, 0.012, 0.94, 0., 0.22, 0.),
    p("bike", 310., 4300., 0.045, 0.009, 0.85, 0., 0., 0.),
    p("single", 160., 1800., 0.24, 0.05, 0.78, 0., 0., 0.),
    p("electric", 0., 0., 0., 0., 0.55, 0., 0.82, 0.),
];
pub const MODEL_VOICES: &[(&str, &str)] = &[
    ("zweitakter", "twoStroke"),
    ("kleinwagen", "triple"),
    ("kompakt", "four"),
    ("limousine", "six"),
    ("taxi", "diesel4"),
    ("kombi", "five"),
    ("transporter", "diesel4"),
    ("gelaende", "diesel6"),
    ("elektro", "electric"),
    ("sportwagen", "flatV8"),
    ("heckcoupe", "flatSix"),
    ("hothatch", "four"),
    ("roadster", "four"),
    ("musclecar", "v8"),
    ("oldtimer", "six"),
    ("pickup", "diesel4"),
    ("kleinbus", "boxer"),
    ("rallye", "triple"),
    ("supersport", "v10"),
    ("gtcoupe", "v8turbo"),
    ("leichtbau", "four"),
    ("elektrosport", "electric"),
    ("sprinter", "diesel4"),
    ("hochdach", "diesel4"),
    ("powerkombi", "v8turbo"),
    ("familienkombi", "diesel4"),
    ("business", "six"),
    ("sportlimo", "six"),
    ("luxus", "v8turbo"),
    ("coupe", "six"),
    ("leichtcoupe", "boxer"),
    ("gklasse", "v8turbo"),
    ("defender", "diesel6"),
    ("niva", "four"),
    ("kompaktsuv", "four"),
    ("sportsuv", "six"),
    ("grosssuv", "six"),
    ("police", "six"),
    ("ambulance", "diesel4"),
    ("delivery", "diesel4"),
    ("truck", "heavy"),
    ("garbage", "heavy"),
    ("bus", "heavy"),
    ("motorcycle", "bike"),
    ("scooter", "single"),
];
const NATURAL: &[&str] = &["roadster", "niva", "oldtimer", "kleinwagen"];
const SPORT: &[&str] = &[
    "hothatch",
    "rallye",
    "leichtbau",
    "sportlimo",
    "coupe",
    "leichtcoupe",
];

/// Profil eines Modells (mit modellbezogenen Abweichungen wie in der JS-Fassung).
pub fn voice_for(model: &str, electric: bool, diesel: bool) -> Voice {
    let id = MODEL_VOICES
        .iter()
        .find(|(m, _)| *m == model)
        .map(|(_, v)| *v)
        .unwrap_or(if electric {
            "electric"
        } else if diesel {
            "diesel4"
        } else {
            "four"
        });
    let mut v = *VOICES.iter().find(|v| v.id == id).expect("Profil");
    let sport = SPORT.contains(&model);
    v.pops = sport || model == "gtcoupe" || model == "supersport";
    if NATURAL.contains(&model) {
        v.turbo = 0.;
    }
    if sport {
        v.brightness *= 1.05;
        v.rough *= 1.2;
        v.volume *= 1.08;
        v.insulation *= 0.65;
    }
    match model {
        "luxus" => {
            v.insulation = 0.94;
            v.volume *= 0.65;
        }
        "roadster" => v.insulation = 0.15,
        "oldtimer" => {
            v.rough = 0.14;
            v.brightness = 1000.;
            v.insulation = 0.35;
        }
        _ => {}
    }
    v
}

/// Fourierkoeffizienten der Druckstöße eines Arbeitszyklus (128 Harmonische; Index 0 bleibt 0).
/// Bankversatz und Pulsstärken bilden u. a. den Crossplane-V8 ab.
pub fn engine_spectrum(v: &Voice, cylinders: u32, intake: bool) -> (Vec<f64>, Vec<f64>) {
    let (mut re, mut im) = (vec![0.; 129], vec![0.; 129]);
    let n = cylinders.max(1) as usize;
    let bank_order: Vec<f64> = if n == 8 {
        [0., 1., 1., 0., 1., 0., 0., 1.].to_vec()
    } else {
        (0..n).map(|i| (i % 2) as f64).collect()
    };
    for h in 1..re.len() {
        for (i, &bank) in bank_order.iter().enumerate() {
            let delay = bank * v.bank * 0.035;
            let phase =
                2. * PI * h as f64 * (i as f64 / n as f64 + delay + if intake { 0.19 } else { 0. });
            let strength = (1. - bank * v.bank * 0.45) * (1. + v.rough * (i as f64 * 2.39).sin());
            let shape = 1. / (1. + (h as f64 * v.width * if intake { 1.6 } else { 1. }).powi(2));
            re[h] += phase.cos() * strength * shape / n as f64;
            im[h] -= phase.sin() * strength * shape / n as f64;
        }
    }
    (re, im)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn profiles_and_spectrum() {
        assert_eq!(voice_for("musclecar", false, false).id, "v8");
        assert_eq!(voice_for("unbekannt", true, false).id, "electric");
        assert_eq!(voice_for("unbekannt", false, true).id, "diesel4");
        assert_eq!(voice_for("roadster", false, false).insulation, 0.15);
        assert!(voice_for("rallye", false, false).pops);
        assert_eq!(voice_for("kleinwagen", false, false).turbo, 0.);
        // Vierzylinder (gleichmäßige Zündfolge): Energie liegt auf jeder 4. Harmonischen des Zyklus
        let v = voice_for("kompakt", false, false);
        let (re, im) = engine_spectrum(&Voice { rough: 0., ..v }, 4, false);
        let mag = |h: usize| re[h].hypot(im[h]);
        assert!(mag(4) > 0.5 && mag(1) < 1e-9 && mag(2) < 1e-9 && mag(3) < 1e-9);
        // Crossplane-V8 hat dank Bankversatz auch Energie zwischen den Zündharmonischen (das Bollern)
        let (re, im) = engine_spectrum(&voice_for("musclecar", false, false), 8, false);
        assert!(re[1].hypot(im[1]) > 0.01);
    }
}
