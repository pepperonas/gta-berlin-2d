//! Menschen-Typen (Port von `figure.js` und `walkerStyle` aus `life.js`): wer unterwegs ist, hängt von Ort, Uhrzeit
//! und Wochentag ab. Der Typ wird deterministisch aus der Nummer gewählt (nie aus dem Welt-Zufall); in der Simulation
//! wirkt er nur aufs Gehtempo. Jogger und Hundehalter würfelt dagegen der Welt-Zufall beim Erzeugen, wie in JS.
use crate::life::Act;
use crate::math::Rng;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Kind {
    #[default]
    Everyday,
    Business,
    Tourist,
    Senior,
    Teen,
    Hipster,
    Worker,
    Punk,
    Headscarf,
    Parent,
    Jogger,
    Dogwalker,
}

/// Spaziergänger-Variante (beim Erzeugen gewürfelt)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Style {
    #[default]
    Plain,
    Jog,
    Dog,
}

struct Def {
    kind: Kind,
    label: &'static str,
    weight: f64,
    speed: f64,
    /// [von, bis, Faktor außerhalb] in Minuten
    hours: Option<(f64, f64, f64)>,
    /// Faktor am Wochenende (nur Mo–Fr stark)
    weekday: Option<f64>,
    bez: &'static [(&'static str, f64)],
}

const FK: &str = "Friedrichshain-Kreuzberg";
const CW: &str = "Charlottenburg-Wilmersdorf";
const SZ: &str = "Steglitz-Zehlendorf";
const MH: &str = "Marzahn-Hellersdorf";

const KINDS: [Def; 12] = [
    Def {
        kind: Kind::Everyday,
        label: "Alltag",
        weight: 34.,
        speed: 1.,
        hours: None,
        weekday: None,
        bez: &[],
    },
    Def {
        kind: Kind::Business,
        label: "Büro",
        weight: 9.,
        speed: 1.12,
        hours: Some((420., 1170., 0.15)),
        weekday: Some(0.2),
        bez: &[("Mitte", 2.6), (CW, 1.8), (FK, 1.2)],
    },
    Def {
        kind: Kind::Tourist,
        label: "Tourist",
        weight: 7.,
        speed: 0.78,
        hours: Some((540., 1230., 0.1)),
        weekday: None,
        bez: &[
            ("Mitte", 4.),
            (FK, 1.8),
            (CW, 1.6),
            ("Tempelhof-Schöneberg", 1.1),
        ],
    },
    Def {
        kind: Kind::Senior,
        label: "Senior",
        weight: 9.,
        speed: 0.62,
        hours: Some((480., 1110., 0.12)),
        weekday: None,
        bez: &[
            (SZ, 1.8),
            ("Spandau", 1.5),
            ("Reinickendorf", 1.5),
            ("Treptow-Köpenick", 1.4),
            (MH, 1.3),
        ],
    },
    Def {
        kind: Kind::Teen,
        label: "Jugendliche",
        weight: 8.,
        speed: 1.15,
        hours: Some((780., 1320., 0.2)),
        weekday: None,
        bez: &[("Neukölln", 1.3), (MH, 1.3)],
    },
    Def {
        kind: Kind::Hipster,
        label: "Kiez",
        weight: 7.,
        speed: 0.95,
        hours: Some((600., 1560., 0.3)),
        weekday: None,
        bez: &[
            (FK, 3.),
            ("Neukölln", 2.4),
            ("Pankow", 1.8),
            ("Mitte", 1.2),
            (SZ, 0.3),
            ("Spandau", 0.3),
        ],
    },
    Def {
        kind: Kind::Worker,
        label: "Handwerk",
        weight: 5.,
        speed: 1.02,
        hours: Some((360., 1050., 0.08)),
        weekday: Some(0.15),
        bez: &[],
    },
    Def {
        kind: Kind::Punk,
        label: "Punk",
        weight: 1.5,
        speed: 1.,
        hours: Some((720., 1620., 0.4)),
        weekday: None,
        bez: &[(FK, 3.5), ("Neukölln", 1.8), (SZ, 0.1), ("Spandau", 0.2)],
    },
    Def {
        kind: Kind::Headscarf,
        label: "Kopftuch",
        weight: 4.,
        speed: 0.9,
        hours: Some((480., 1200., 0.2)),
        weekday: None,
        bez: &[
            ("Neukölln", 3.),
            ("Mitte", 1.8),
            (FK, 2.),
            ("Spandau", 1.4),
            (SZ, 0.4),
        ],
    },
    Def {
        kind: Kind::Parent,
        label: "Kinderwagen",
        weight: 5.,
        speed: 0.72,
        hours: Some((540., 1110., 0.03)),
        weekday: None,
        bez: &[("Pankow", 2.), (FK, 1.5)],
    },
    Def {
        kind: Kind::Jogger,
        label: "Jogger",
        weight: 0.,
        speed: 2.3,
        hours: None,
        weekday: None,
        bez: &[],
    },
    Def {
        kind: Kind::Dogwalker,
        label: "Gassi",
        weight: 0.,
        speed: 0.85,
        hours: None,
        weekday: None,
        bez: &[],
    },
];

pub const ALL: [Kind; 12] = [
    Kind::Everyday,
    Kind::Business,
    Kind::Tourist,
    Kind::Senior,
    Kind::Teen,
    Kind::Hipster,
    Kind::Worker,
    Kind::Punk,
    Kind::Headscarf,
    Kind::Parent,
    Kind::Jogger,
    Kind::Dogwalker,
];

fn def(k: Kind) -> &'static Def {
    KINDS
        .iter()
        .find(|d| d.kind == k)
        .expect("jede Art steht in KINDS")
}
impl Kind {
    pub fn label(self) -> &'static str {
        def(self).label
    }
    /// Faktor aufs Gehtempo
    pub fn speed(self) -> f64 {
        def(self).speed
    }
}

/// Reiner Hash 0…1 aus Nummer und Kanal (`figure.js h01`, `sin`-Hash wie in JS).
pub fn h01(id: f64, k: f64) -> f64 {
    let x = (id * 127.1 + k * 311.7).sin() * 43758.5453;
    x - x.floor()
}

fn in_window(m: f64, a: f64, b: f64) -> bool {
    if b > 1440. {
        m >= a || m < b - 1440.
    } else {
        m >= a && m < b
    }
}

/// Ort und Zeit für die Auswahl
#[derive(Debug, Clone, Copy, Default)]
pub struct Ctx<'a> {
    pub minutes: f64,
    /// 0 = Montag
    pub day: u32,
    pub bezirk: Option<&'a str>,
    pub act: Option<Act>,
}

/// Gewicht je Typ an diesem Ort zu dieser Zeit (Reihenfolge wie [`ALL`]).
pub fn kind_weights(ctx: &Ctx) -> [f64; 12] {
    let m = ctx.minutes.rem_euclid(1440.);
    let weekend = ctx.day >= 5;
    let night = !(360. ..1320.).contains(&m);
    let mut out = [0.; 12];
    for (i, k) in KINDS.iter().enumerate() {
        let mut w = k.weight;
        if w == 0. {
            continue;
        }
        if let Some((a, b, f)) = k.hours
            && !in_window(m, a, b)
        {
            w *= f;
        }
        if let Some(f) = k.weekday
            && weekend
        {
            w *= f;
        }
        if let Some(b) = ctx.bezirk
            && let Some(&(_, f)) = k.bez.iter().find(|(n, _)| *n == b)
        {
            w *= f;
        }
        if night && k.kind == Kind::Everyday {
            w *= 0.8;
        }
        // Tätigkeiten: Kinderwagen sitzt nicht auf der Wiese, Senioren spielen keine Gitarre, Büroleute rauchen vor der Tür
        if let Some(act) = ctx.act {
            use Kind::*;
            let id = k.kind;
            if id == Parent && !matches!(act, Act::Browse | Act::Queue | Act::Chat) {
                w = 0.;
            }
            match act {
                Act::Music => {
                    w *= match id {
                        Hipster | Punk => 4.,
                        Teen => 1.5,
                        Everyday => 1.,
                        _ => 0.1,
                    }
                }
                Act::Drink => {
                    w *= match id {
                        Punk | Hipster | Worker => 2.5,
                        Senior | Headscarf | Tourist => 0.3,
                        _ => 1.,
                    }
                }
                Act::Smoke if matches!(id, Business | Worker | Punk) => w *= 1.8,
                Act::Browse if matches!(id, Tourist | Senior | Headscarf) => w *= 1.8,
                _ => {}
            }
        }
        out[i] = w;
    }
    out
}

/// Typ wählen: deterministisch aus der Nummer, gewichtet nach Ort und Zeit.
pub fn pick_kind(id: u32, ctx: &Ctx) -> Kind {
    let w = kind_weights(ctx);
    let sum: f64 = w.iter().sum();
    let mut r = h01(f64::from(id), 17.) * sum;
    for (i, &wi) in w.iter().enumerate() {
        r -= wi;
        if r < 0. && wi > 0. {
            return ALL[i];
        }
    }
    Kind::Everyday
}

fn in_hours(m: f64, a: f64, b: f64) -> bool {
    m >= a && m < b
}

/// Spaziergänger-Varianten: Jogger morgens und abends, Hundehalter tagsüber. Verbraucht genau eine Zufallszahl.
pub fn walker_style(minutes: f64, rng: &mut Rng) -> Style {
    let m = minutes.rem_euclid(1440.);
    let jog = if in_hours(m, 360., 540.) || in_hours(m, 1020., 1260.) {
        0.14
    } else if in_hours(m, 540., 1020.) {
        0.04
    } else {
        0.
    };
    let dog = if in_hours(m, 360., 1320.) { 0.09 } else { 0.02 };
    let r = rng.float();
    if r < jog {
        Style::Jog
    } else if r < jog + dog {
        Style::Dog
    } else {
        Style::Plain
    }
}

/// Trikotfarben der Jogger (`world.js spawnPed`)
pub const JOG_SHIRTS: [u32; 5] = [0xe84393, 0x00b894, 0x0984e3, 0xfdcb6e, 0xd63031];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weights_follow_place_and_time() {
        let day = Ctx {
            minutes: 600.,
            day: 1,
            bezirk: Some("Mitte"),
            act: None,
        };
        let w = kind_weights(&day);
        assert!(w[2] > 20., "Touristen in Mitte am Tag: {}", w[2]);
        let night = Ctx {
            minutes: 180.,
            ..day
        };
        assert!(kind_weights(&night)[2] < 3.);
        let weekend = Ctx { day: 6, ..day };
        assert!(
            kind_weights(&weekend)[1] < w[1] * 0.3,
            "Büro am Sonntag selten"
        );
        // Jogger und Hundehalter kommen nur über den Stil
        assert_eq!((w[10], w[11]), (0., 0.));
        let music = Ctx {
            act: Some(Act::Music),
            ..day
        };
        let wm = kind_weights(&music);
        assert_eq!(wm[9], 0., "kein Kinderwagen beim Musikmachen");
        assert!(wm[5] > w[5] * 3.);
    }

    #[test]
    fn pick_is_deterministic_and_varied() {
        let c = Ctx {
            minutes: 720.,
            day: 2,
            bezirk: Some("Friedrichshain-Kreuzberg"),
            act: None,
        };
        assert_eq!(pick_kind(42, &c), pick_kind(42, &c));
        let kinds: std::collections::HashSet<_> = (0..400).map(|i| pick_kind(i, &c)).collect();
        assert!(kinds.len() >= 8, "{kinds:?}");
        assert!(!kinds.contains(&Kind::Jogger) && !kinds.contains(&Kind::Dogwalker));
        assert!((h01(3., 17.) - 0.5).abs() <= 0.5);
    }

    #[test]
    fn walker_style_by_hour() {
        let count = |min: f64| {
            let mut rng = Rng::new(7);
            let s: Vec<Style> = (0..2000).map(|_| walker_style(min, &mut rng)).collect();
            (
                s.iter().filter(|&&x| x == Style::Jog).count(),
                s.iter().filter(|&&x| x == Style::Dog).count(),
            )
        };
        let (jm, dm) = count(420.);
        assert!(
            (200..380).contains(&jm) && (110..260).contains(&dm),
            "{jm} {dm}"
        );
        let (jn, dn) = count(120.);
        assert_eq!(jn, 0);
        assert!(dn < 80);
    }
}
