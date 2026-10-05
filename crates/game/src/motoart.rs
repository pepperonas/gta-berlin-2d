//! Motorräder und Roller von oben, aus Teilen gezeichnet (statt eines Rechtecks): Reifen (das Vorderrad lenkt),
//! Schwinge, Motor, Auspuff, Tank, Sitzbank, Heck mit Rücklicht, Lenker mit Spiegeln und je Bauart eigene Teile –
//! Verkleidung mit Scheibe (Superbike), Rundscheinwerfer und Gabel (Naked), Chrom, Trittbretter und breiter Lenker
//! (Cruiser), Karosserie mit Trittbrett und Beinschild (Roller). Dazu der Fahrer mit Knien am Tank, Armen zum Lenker
//! und Helm mit Visier, in der Haltung der Bauart (Superbike geduckt, Cruiser aufrecht zurückgelehnt).
//!
//! Teile sind in Metern im Fahrzeugsystem beschrieben (`a` längs, + nach vorn; `s` quer, + nach rechts; `h` Höhe
//! über dem Boden) und werden mit der Länge des Fahrzeugs skaliert. Schräglage: von oben wandert jedes Teil um
//! h·sin(φ) zur Kurveninnenseite und wird schmaler, der Fahrer am weitesten (im Superbike hängt er zusätzlich
//! innen heraus). Wheelie/Stoppie: das Rad verkürzt sich um den Aufstandspunkt hinten bzw. vorn. Nur Darstellung.
use berlin_engine::Body;

/// Bauart
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Sport,
    Naked,
    Cruiser,
    Scooter,
}

/// Bauart eines Modells (Fahrzeug-id bzw. Pkw-Modellname).
pub fn style_of(model: &str) -> Style {
    match model {
        "superbike" => Style::Sport,
        "cruiser" => Style::Cruiser,
        "roller_45" | "scooter" => Style::Scooter,
        _ => Style::Naked,
    }
}

/// Lage und Zustand eines Zweirads im Bild.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose {
    pub x: f32,
    pub y: f32,
    /// Fahrtrichtung (rad, Bildsystem)
    pub angle: f32,
    /// Halbe Länge (px)
    pub hw: f32,
    /// Schräglage (rad, + nach rechts), Nickwinkel (+ Wheelie), Lenkeinschlag (rad)
    pub lean: f32,
    pub pitch: f32,
    pub steer: f32,
    pub lying: bool,
    pub braking: bool,
    pub rider: bool,
    pub paint: u32,
    /// Fahrerjacke und Helm (aus der Fahrzeug-id)
    pub jacket: u32,
    pub helmet: u32,
    pub depth: f32,
}

fn rgba(c: u32, a: f32) -> [f32; 4] {
    [
        ((c >> 16) & 255) as f32 / 255.,
        ((c >> 8) & 255) as f32 / 255.,
        (c & 255) as f32 / 255.,
        a,
    ]
}
fn shade(c: [f32; 4], k: f32) -> [f32; 4] {
    [
        (c[0] * k).min(1.),
        (c[1] * k).min(1.),
        (c[2] * k).min(1.),
        c[3],
    ]
}

/// Anteil der seitlichen Wanderung in Schräglage, der gezeichnet wird (Lesbarkeit, s. `bodies`)
pub const LEAN_SHIFT: f32 = 0.65;
const TIRE: u32 = 0x2a2c31;
const DARK: u32 = 0x22252b;
const METAL: u32 = 0x5d636d;
const CHROME: u32 = 0xc4c9d2;
const SEAT: u32 = 0x1b1c20;
const GLASS: u32 = 0x8fb1c9;
const LAMP: u32 = 0xfff2c8;
const PANTS: u32 = 0x262a33;
const VISOR: u32 = 0x161a22;

/// Farben für Jacke und Helm aus der Fahrzeug-id (fester Hash, kein Welt-Zufall).
pub fn rider_colors(id: u32) -> (u32, u32) {
    const JACKETS: [u32; 6] = [0x22252c, 0x7c1f22, 0x1f3a6e, 0x45484f, 0x2f4a2a, 0x5a3b22];
    const HELMETS: [u32; 6] = [0xeeeeee, 0x15171b, 0xc8102e, 0x1d4ed8, 0xf2b705, 0x9aa0a8];
    let h = id.wrapping_mul(2654435761);
    (
        JACKETS[(h >> 8) as usize % JACKETS.len()],
        HELMETS[(h >> 20) as usize % HELMETS.len()],
    )
}

/// Ein Teil: Lage (m), halbe Ausdehnung längs/quer (m), Höhe (m), Form (0 Rechteck abgerundet, 1 Ellipse),
/// Farbe, ob es mit dem Lenker dreht.
#[derive(Debug, Clone, Copy)]
struct Part {
    a: f32,
    s: f32,
    h: f32,
    hl: f32,
    hs: f32,
    shape: f32,
    color: [f32; 4],
    steer: bool,
}
const fn part(a: f32, s: f32, h: f32, hl: f32, hs: f32, shape: f32, color: [f32; 4]) -> Part {
    Part {
        a,
        s,
        h,
        hl,
        hs,
        shape,
        color,
        steer: false,
    }
}

/// Teile des Fahrzeugs ohne Fahrer, in Zeichenreihenfolge (unten zuerst).
fn bike_parts(st: Style, paint: [f32; 4], braking: bool) -> Vec<Part> {
    let tire = rgba(TIRE, 1.);
    let dark = rgba(DARK, 1.);
    let metal = rgba(METAL, 1.);
    let chrome = rgba(CHROME, 1.);
    let seat = rgba(SEAT, 1.);
    let lamp = rgba(LAMP, 1.);
    let tail = if braking {
        [1., 0.18, 0.12, 1.]
    } else {
        [0.62, 0.08, 0.07, 1.]
    };
    let hi = shade(paint, 1.35);
    let mut p = Vec::new();
    let steer = |mut q: Part| {
        q.steer = true;
        q
    };
    match st {
        Style::Sport => {
            p.push(part(-0.72, 0., 0.3, 0.31, 0.085, 0., tire));
            p.push(steer(part(0.72, 0., 0.3, 0.3, 0.055, 0., tire)));
            p.push(part(-0.78, 0., 0.5, 0.14, 0.07, 1., dark));
            p.push(steer(part(0.72, 0., 0.55, 0.16, 0.065, 1., paint)));
            p.push(part(-0.42, 0., 0.35, 0.3, 0.05, 0., metal));
            p.push(part(0.03, 0., 0.42, 0.26, 0.16, 0., dark));
            p.push(part(-0.6, 0.11, 0.55, 0.15, 0.05, 1., chrome));
            p.push(part(0.12, 0., 0.5, 0.4, 0.2, 1., paint));
            p.push(part(0.15, 0., 0.86, 0.25, 0.16, 1., paint));
            p.push(part(0.18, -0.04, 0.9, 0.12, 0.06, 1., hi));
            p.push(part(-0.27, 0., 0.82, 0.2, 0.12, 0., seat));
            p.push(part(-0.56, 0., 0.86, 0.2, 0.08, 1., paint));
            p.push(part(-0.74, 0., 0.86, 0.03, 0.05, 0., tail));
            p.push(part(0.44, 0., 0.95, 0.17, 0.19, 1., paint));
            p.push(part(0.4, 0., 1.05, 0.1, 0.12, 1., rgba(GLASS, 0.8)));
            for s in [-0.07, 0.07] {
                p.push(part(0.58, s, 0.8, 0.03, 0.035, 1., lamp));
            }
            p.push(steer(part(0.33, 0., 0.95, 0.025, 0.3, 0., dark)));
            for s in [-0.24, 0.24] {
                p.push(part(0.45, s, 1.05, 0.04, 0.05, 1., dark));
            }
        }
        Style::Naked => {
            p.push(part(-0.72, 0., 0.3, 0.31, 0.08, 0., tire));
            p.push(steer(part(0.72, 0., 0.3, 0.3, 0.055, 0., tire)));
            p.push(part(-0.8, 0., 0.5, 0.12, 0.065, 1., dark));
            p.push(steer(part(0.73, 0., 0.55, 0.15, 0.06, 1., dark)));
            p.push(part(-0.42, 0., 0.35, 0.3, 0.05, 0., metal));
            p.push(part(0.04, 0., 0.45, 0.25, 0.17, 0., metal));
            p.push(part(-0.08, 0.13, 0.3, 0.06, 0.08, 1., dark));
            p.push(part(-0.48, 0.15, 0.5, 0.22, 0.05, 1., chrome));
            for s in [-0.08, 0.08] {
                p.push(steer(part(0.58, s, 0.7, 0.12, 0.02, 0., chrome)));
            }
            p.push(part(0.16, 0., 0.9, 0.26, 0.18, 1., paint));
            p.push(part(0.19, -0.05, 0.95, 0.12, 0.07, 1., hi));
            p.push(part(-0.28, 0., 0.82, 0.22, 0.12, 0., seat));
            p.push(part(-0.58, 0., 0.85, 0.16, 0.07, 1., paint));
            p.push(part(-0.73, 0., 0.85, 0.03, 0.05, 0., tail));
            p.push(steer(part(0.53, 0., 1.0, 0.09, 0.09, 1., dark)));
            p.push(steer(part(0.55, 0., 1.02, 0.06, 0.07, 1., lamp)));
            p.push(steer(part(0.36, 0., 1.05, 0.025, 0.4, 0., dark)));
            for s in [-0.38, 0.38] {
                p.push(steer(part(0.4, s, 1.15, 0.035, 0.045, 1., dark)));
            }
        }
        Style::Cruiser => {
            p.push(part(-0.72, 0., 0.32, 0.32, 0.11, 0., tire));
            p.push(steer(part(0.74, 0., 0.32, 0.32, 0.055, 0., tire)));
            p.push(steer(part(0.74, 0., 0.5, 0.2, 0.08, 1., paint)));
            for s in [-0.3, 0.3] {
                p.push(part(0.15, s, 0.3, 0.12, 0.05, 0., dark));
            }
            p.push(part(0.02, 0., 0.48, 0.2, 0.2, 1., chrome));
            for s in [0.14, 0.2] {
                p.push(part(-0.42, s, 0.4, 0.38, 0.03, 0., chrome));
            }
            p.push(part(-0.62, 0., 0.65, 0.24, 0.13, 1., paint));
            p.push(part(-0.74, 0., 0.68, 0.03, 0.05, 0., tail));
            p.push(part(0.24, 0., 0.85, 0.3, 0.17, 1., paint));
            p.push(part(0.27, -0.05, 0.9, 0.14, 0.06, 1., hi));
            p.push(part(-0.2, 0., 0.7, 0.24, 0.17, 0., rgba(0x3a2a20, 1.)));
            p.push(steer(part(0.55, 0., 1.0, 0.1, 0.1, 1., chrome)));
            p.push(steer(part(0.57, 0., 1.02, 0.07, 0.07, 1., lamp)));
            p.push(steer(part(0.4, 0., 1.1, 0.03, 0.46, 0., chrome)));
            for s in [-0.4, 0.4] {
                p.push(steer(part(0.42, s, 1.2, 0.04, 0.05, 1., chrome)));
            }
        }
        Style::Scooter => {
            p.push(part(-0.6, 0., 0.18, 0.18, 0.06, 0., tire));
            p.push(steer(part(0.62, 0., 0.18, 0.18, 0.055, 0., tire)));
            p.push(part(-0.55, 0.12, 0.3, 0.12, 0.04, 1., chrome));
            p.push(part(0.13, 0., 0.3, 0.22, 0.16, 0., rgba(0x3b3e45, 1.)));
            p.push(part(-0.3, 0., 0.6, 0.45, 0.22, 1., paint));
            p.push(part(-0.26, 0., 0.8, 0.3, 0.13, 0., seat));
            p.push(part(-0.74, 0., 0.65, 0.03, 0.06, 0., tail));
            p.push(part(0.38, 0., 0.7, 0.1, 0.2, 1., paint));
            p.push(steer(part(0.5, 0., 1.0, 0.08, 0.25, 1., paint)));
            p.push(steer(part(0.57, 0., 1.02, 0.03, 0.06, 0., lamp)));
            for s in [-0.27, 0.27] {
                p.push(steer(part(0.5, s, 1.1, 0.035, 0.045, 1., dark)));
            }
        }
    }
    p
}

/// Haltung des Fahrers je Bauart: Helm längs, Rumpf längs, Lenkergriffe (längs, quer), Knie quer, Rumpfhöhe.
struct Stance {
    helm: f32,
    torso: f32,
    grip: (f32, f32),
    knee: f32,
    hang: f32,
}
fn stance(st: Style) -> Stance {
    match st {
        Style::Sport => Stance {
            helm: 0.12,
            torso: -0.06,
            grip: (0.33, 0.27),
            knee: 0.2,
            hang: 0.16,
        },
        Style::Naked => Stance {
            helm: -0.04,
            torso: -0.17,
            grip: (0.36, 0.34),
            knee: 0.17,
            hang: 0.06,
        },
        Style::Cruiser => Stance {
            helm: -0.2,
            torso: -0.3,
            grip: (0.4, 0.4),
            knee: 0.22,
            hang: 0.,
        },
        Style::Scooter => Stance {
            helm: -0.12,
            torso: -0.25,
            grip: (0.5, 0.23),
            knee: 0.13,
            hang: 0.,
        },
    }
}

/// Alle Körper eines Zweirads (mit Fahrer, falls er draufsitzt).
pub fn bodies(p: &Pose, st: Style, out: &mut Vec<Body>) {
    // Meter → px: die Teile sind für ein 2,1 m langes Fahrzeug beschrieben
    let ppm = p.hw / 1.05;
    let (fx, fy) = (p.angle.cos(), p.angle.sin());
    let (rx, ry) = (-fy, fx);
    let lean = if p.lying {
        std::f32::consts::FRAC_PI_2 * if p.lean < 0. { -1. } else { 1. }
    } else {
        p.lean
    };
    let (sl, cl) = (lean.sin(), lean.cos());
    let narrow = 0.55 + 0.45 * cl;
    // Wheelie hebt um das Hinterrad, Stoppie um das Vorderrad: von oben wird das Rad kürzer
    let (pivot, cp) = if p.pitch >= 0. {
        (-0.72, p.pitch.cos())
    } else {
        (0.72, p.pitch.cos())
    };
    let place = |a: f32, s: f32, h: f32| -> [f32; 2] {
        let a = pivot + (a - pivot) * cp;
        // seitliche Wanderung gedämpft (LEAN_SHIFT): in reiner Draufsicht läge der Helm in 45° einen Meter neben der
        // Radspur und der Fahrer wirkte vom Motorrad abgerissen
        let s = s * cl + h * sl * LEAN_SHIFT;
        [p.x + (fx * a + rx * s) * ppm, p.y + (fy * a + ry * s) * ppm]
    };
    let paint = rgba(p.paint, 1.);
    let push = |out: &mut Vec<Body>, q: &Part, extra_s: f32| {
        let ang = p.angle + if q.steer { p.steer } else { 0. };
        out.push(Body {
            center: place(q.a, q.s + extra_s, q.h),
            half: [q.hl * cp.max(0.5) * ppm, q.hs * narrow * ppm],
            angle: ang,
            shape: q.shape,
            depth: p.depth - q.h * 0.0003,
            color: q.color,
        });
    };
    // Schatten am Boden, unter der Fahrlinie
    out.push(Body {
        center: [p.x + 1.5, p.y + 2.],
        half: [
            p.hw * 1.02,
            0.32 * ppm + if p.lying { 0.6 * ppm } else { 0. },
        ],
        angle: p.angle,
        shape: 3.,
        depth: p.depth + 0.0004,
        color: [0., 0., 0., 0.42],
    });
    for q in bike_parts(st, paint, p.braking) {
        push(out, &q, 0.);
    }
    if !p.rider || p.lying {
        return;
    }
    let s = stance(st);
    let pants = rgba(PANTS, 1.);
    let jacket = rgba(p.jacket, 1.);
    // im Superbike hängt der Fahrer innen heraus (zusätzlich zur Schräglage)
    let hang = s.hang * (lean / 0.8).clamp(-1., 1.);
    // Beine: Knie am Tank, das innere bei Schräglage weiter heraus
    for side in [-1f32, 1.] {
        let out_k = if side * lean > 0. {
            1. + 1.4 * lean.abs()
        } else {
            1.
        };
        push(
            out,
            &part(0.04, side * s.knee * out_k, 0.72, 0.2, 0.07, 1., pants),
            hang * 0.6,
        );
    }
    // Rumpf, Arme zu den Griffen, Helm mit Visier
    push(out, &part(s.torso, 0., 1.25, 0.18, 0.2, 1., jacket), hang);
    for side in [-1f32, 1.] {
        let (sa, ss, sh) = (s.torso + 0.08, side * 0.17, 1.28);
        let (ga, gs) = (s.grip.0, side * s.grip.1);
        let a0 = place(sa, ss + hang, sh);
        let a1 = place(ga, gs + hang * 0.3, 1.0);
        let (dx, dy) = (a1[0] - a0[0], a1[1] - a0[1]);
        out.push(Body {
            center: [(a0[0] + a1[0]) / 2., (a0[1] + a1[1]) / 2.],
            half: [(dx.hypot(dy) / 2.).max(0.5), 0.055 * ppm],
            angle: dy.atan2(dx),
            shape: 0.,
            depth: p.depth - 1.2 * 0.0003,
            color: shade(jacket, 0.85),
        });
    }
    push(
        out,
        &part(s.helm, 0., 1.5, 0.14, 0.13, 1., rgba(p.helmet, 1.)),
        hang * 1.2,
    );
    push(
        out,
        &part(s.helm + 0.08, 0., 1.52, 0.05, 0.1, 1., rgba(VISOR, 0.95)),
        hang * 1.2,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pose() -> Pose {
        Pose {
            x: 100.,
            y: 100.,
            angle: 0.,
            hw: 10.5,
            lean: 0.,
            pitch: 0.,
            steer: 0.,
            lying: false,
            braking: false,
            rider: true,
            paint: 0xc8102e,
            jacket: 0x22252c,
            helmet: 0xeeeeee,
            depth: 0.6,
        }
    }
    fn draw(p: &Pose, st: Style) -> Vec<Body> {
        let mut v = Vec::new();
        bodies(p, st, &mut v);
        v
    }

    #[test]
    fn every_style_has_wheels_rider_and_stays_on_its_footprint() {
        for st in [Style::Sport, Style::Naked, Style::Cruiser, Style::Scooter] {
            let b = draw(&pose(), st);
            assert!(b.len() >= 20, "{st:?}: {} Teile", b.len());
            // alles innerhalb der Länge (± etwas Spiegel), schmal wie ein Motorrad
            for x in &b {
                assert!(
                    (x.center[0] - 100.).abs() <= 11.5,
                    "{st:?} längs {:?}",
                    x.center
                );
                assert!(
                    (x.center[1] - 100.).abs() <= 5.,
                    "{st:?} quer {:?}",
                    x.center
                );
            }
            // ohne Fahrer weniger Teile
            let empty = draw(
                &Pose {
                    rider: false,
                    ..pose()
                },
                st,
            );
            assert!(empty.len() + 6 <= b.len());
        }
        // Bauarten unterscheiden sich
        let n: Vec<usize> = [Style::Sport, Style::Naked, Style::Cruiser, Style::Scooter]
            .iter()
            .map(|&s| draw(&pose(), s).len())
            .collect();
        assert!(n.windows(2).any(|w| w[0] != w[1]), "{n:?}");
        assert_eq!(style_of("superbike"), Style::Sport);
        assert_eq!(style_of("roller_45"), Style::Scooter);
        assert_eq!(style_of("cruiser"), Style::Cruiser);
        assert_eq!(style_of("motorrad_naked"), Style::Naked);
    }

    #[test]
    fn leaning_shifts_tall_parts_inside_and_the_rider_most() {
        let up = draw(&pose(), Style::Sport);
        let right = draw(
            &Pose {
                lean: 0.7,
                ..pose()
            },
            Style::Sport,
        );
        // Fahrtrichtung +x: rechts ist +y. Der Helm (letzte Teile) wandert weiter nach rechts als ein Reifen.
        let helm_dy = right[right.len() - 2].center[1] - up[up.len() - 2].center[1];
        let tire_dy = right[1].center[1] - up[1].center[1];
        assert!(
            helm_dy > 6. && helm_dy > tire_dy * 3.,
            "Helm {helm_dy}, Reifen {tire_dy}"
        );
        // Wheelie: das Rad wird von oben kürzer, das Vorderrad rückt nach hinten
        let wheelie = draw(
            &Pose {
                pitch: 0.4,
                ..pose()
            },
            Style::Naked,
        );
        let front = |b: &[Body]| b[2].center[0];
        assert!(front(&wheelie) < front(&draw(&pose(), Style::Naked)) - 0.4);
        // Vorderrad dreht mit dem Lenker, das Hinterrad nicht
        let steered = draw(
            &Pose {
                steer: 0.4,
                ..pose()
            },
            Style::Naked,
        );
        assert_eq!(steered[2].angle, 0.4);
        assert_eq!(steered[1].angle, 0.);
        // Bremslicht leuchtet heller
        let tail = |b: &[Body]| {
            b.iter()
                .map(|x| x.color)
                .find(|c| c[0] > 0.5 && c[1] < 0.2)
                .unwrap()
        };
        // (blaue Lackierung, damit nur das Rücklicht rot ist)
        let blue = Pose {
            paint: 0x1d4ed8,
            ..pose()
        };
        assert!(
            tail(&draw(
                &Pose {
                    braking: true,
                    ..blue
                },
                Style::Sport
            ))[0]
                > tail(&draw(&blue, Style::Sport))[0]
        );
    }
}
