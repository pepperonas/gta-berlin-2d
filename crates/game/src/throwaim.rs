//! Wurfvorschau beim Ausholen (Granate, Molotow): gepunkteter Bogen vom Werfer bis zum Aufschlag und ein Ring am
//! Landepunkt, der die Wirkung zeigt (Druckwelle bzw. Brandfläche). Die Punkte kommen aus
//! `World::throw_preview` – derselben Flugbahn wie der echte Wurf; die Weite pendelt mit `throw::charge_reach`.
use berlin_engine::Body;

/// Vorschau eines Spielers: Bahnpunkte (x, y, Höhe), Art des Wurfs.
#[derive(Debug, Clone, PartialEq)]
pub struct Arc {
    pub pts: Vec<(f64, f64, f64)>,
    pub grenade: bool,
}

/// Bildversatz nach oben für eine Höhe (wie die Wurfkörper, `firefx.rs`)
fn lift(z: f64) -> f64 {
    z * 0.55
}

/// Körper (Effekt-Durchgang): Punkte entlang des Bogens (jeder dritte Schritt, wandernd im Takt `t`), ihr Schatten
/// am Boden, dazu der Wirkungsring am Landepunkt.
pub fn bodies(a: &Arc, t: f32, out: &mut Vec<Body>) {
    let Some(&end) = a.pts.last() else { return };
    let color = if a.grenade {
        [1., 0.93, 0.6]
    } else {
        [1., 0.72, 0.38]
    };
    // die Punkte wandern langsam zum Ziel (Laufschrift), so liest sich die Richtung
    let phase = ((t * 9.) as usize) % 3;
    let n = a.pts.len();
    for (i, &(x, y, z)) in a.pts.iter().enumerate().skip(1) {
        if !(i + phase).is_multiple_of(3) || i + 1 == n {
            continue;
        }
        // nah am Werfer kleiner und blasser, damit die Figur frei bleibt
        let k = (i as f32 / 6.).min(1.);
        out.push(Body {
            center: [x as f32, (y - lift(z)) as f32],
            half: [1.7, 1.7],
            angle: 0.,
            shape: 1.,
            depth: 0.584,
            color: [color[0], color[1], color[2], 0.85 * k],
        });
        out.push(Body {
            center: [x as f32, y as f32],
            half: [1.2, 0.9],
            angle: 0.,
            shape: 1.,
            depth: 0.5845,
            color: [0., 0., 0., 0.18 * k],
        });
    }
    // Landepunkt: Wirkungsradius als Ring, pulsierend
    let r = if a.grenade {
        (berlin_sim::fire::BLAST_R * berlin_sim::throw::GRENADE_K) as f32
    } else {
        berlin_sim::throw::FLAME_R as f32
    };
    let pulse = 0.85 + 0.15 * (t * 6.).sin();
    out.push(Body {
        center: [end.0 as f32, end.1 as f32],
        half: [r * pulse, r * pulse],
        angle: 0.,
        shape: 2.,
        depth: 0.5845,
        color: [color[0], color[1], color[2], 0.22],
    });
    out.push(Body {
        center: [end.0 as f32, end.1 as f32],
        half: [3., 3.],
        angle: 0.,
        shape: 1.,
        depth: 0.584,
        color: [color[0], color[1], color[2], 0.9],
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arc(grenade: bool) -> Arc {
        let pts = (0..40)
            .map(|i| {
                let u = i as f64 / 39.;
                (u * 150., 0., 12. + 60. * u * (1. - u))
            })
            .collect();
        Arc { pts, grenade }
    }

    #[test]
    fn dots_follow_the_arc_and_a_ring_marks_the_landing() {
        let mut out = Vec::new();
        bodies(&arc(true), 0., &mut out);
        let dots: Vec<&Body> = out
            .iter()
            .filter(|b| b.shape == 1. && b.color[3] > 0.5 && b.half[0] < 2.)
            .collect();
        assert!(dots.len() >= 8, "Punkte entlang des Bogens: {}", dots.len());
        // in der Mitte des Bogens hochgezogen (Höhe), am Ende auf dem Boden
        assert!(
            dots.iter().any(|b| b.center[1] < -12.),
            "Höhe nach oben versetzt"
        );
        let ring = out.iter().find(|b| b.shape == 2.).expect("Ring");
        assert!((ring.center[0] - 150.).abs() < 0.1);
        let blast = (berlin_sim::fire::BLAST_R * berlin_sim::throw::GRENADE_K) as f32;
        assert!(ring.half[0] > blast * 0.8 && ring.half[0] < blast * 1.05);
    }

    #[test]
    fn the_dots_march_toward_the_target() {
        let at = |t: f32| {
            let mut out = Vec::new();
            bodies(&arc(false), t, &mut out);
            out.iter()
                .filter(|b| b.shape == 1. && b.half[0] < 2. && b.color[3] > 0.5)
                .map(|b| b.center[0])
                .next()
                .unwrap()
        };
        assert_ne!(at(0.), at(0.12), "Punkte wandern");
        let mut out = Vec::new();
        bodies(
            &Arc {
                pts: vec![],
                grenade: false,
            },
            0.,
            &mut out,
        );
        assert!(out.is_empty());
    }
}
