//! Schusswaffen in der Hand der Spielfigur, aus Teilen gezeichnet (statt eines Balkens): Griff, Verschluss bzw.
//! Gehäuse, Lauf mit dunkler Mündung, Schaft, Vorderschaft – dazu die Haltung (wo die Hände liegen). Pistole und
//! Maschinenpistole werden beidhändig gehalten, die Schrotflinte liegt an der rechten Schulter, die linke Hand am
//! Vorderschaft. Das Laufende liegt genau auf `berlin_sim::combat::muzzle`: dort beginnen Mündungsfeuer und
//! Leuchtspur (die Kugeln kommen aus dem Lauf).
//!
//! Lage in px im Figur-System: `f` vor der Figur, `r` quer (+ = rechts); halbe Ausdehnung längs/quer.

/// Ein Teil der Waffe.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Part {
    pub f: f32,
    pub r: f32,
    pub hf: f32,
    pub hr: f32,
    /// 0 Rechteck, 1 Ellipse
    pub shape: f32,
    pub color: u32,
}
const fn p(f: f32, r: f32, hf: f32, hr: f32, shape: f32, color: u32) -> Part {
    Part {
        f,
        r,
        hf,
        hr,
        shape,
        color,
    }
}

const POLYMER: u32 = 0x24262b;
const GUNMETAL: u32 = 0x3b3f46;
const EDGE: u32 = 0x7a808a;
const STEEL: u32 = 0x50555d;
const BORE: u32 = 0x08090a;
const WOOD: u32 = 0x6e4524;
const WOOD_DARK: u32 = 0x4e2f18;
const SIGHT: u32 = 0xd9d4c4;

/// Teile einer Schusswaffe in Zeichenreihenfolge (unten zuerst); `None` für Nahkampfwaffen und Wurfkörper.
pub fn parts(weapon: &str) -> Option<&'static [Part]> {
    const PISTOL: &[Part] = &[
        // Griff unter der rechten Hand, Verschluss mit heller Oberkante, dunkle Mündung
        p(7.3, 0.7, 1.4, 1.1, 0., POLYMER),
        p(9.5, 0.4, 3.5, 0.85, 0., GUNMETAL),
        p(9.4, 0.1, 3.2, 0.24, 0., EDGE),
        p(12.75, 0.4, 0.25, 0.35, 1., BORE),
    ];
    const SMG: &[Part] = &[
        // Schulterstütze, Gehäuse mit Schiene, Pistolengriff, Lauf
        p(2.6, 2.2, 1.7, 0.45, 0., POLYMER),
        p(8.5, 0.9, 4., 1.15, 0., POLYMER),
        p(8.4, 0.55, 3.6, 0.3, 0., EDGE),
        p(6., 1.9, 1.1, 0.9, 0., GUNMETAL),
        p(14.5, 0.9, 2., 0.45, 0., STEEL),
        p(16.3, 0.9, 0.2, 0.3, 1., BORE),
    ];
    const SHOTGUN: &[Part] = &[
        // Holzschaft an der Schulter, Verschlussgehäuse, Magazinrohr unter dem Lauf, Lauf, Vorderschaft, Korn
        p(3.8, 2.4, 3., 1., 0., WOOD),
        p(3.4, 2.15, 2.5, 0.3, 0., WOOD_DARK),
        p(8.3, 1.2, 1.8, 1.05, 0., GUNMETAL),
        p(13., 1.6, 3.6, 0.32, 0., GUNMETAL),
        p(14.6, 1., 5.4, 0.5, 0., STEEL),
        p(14.55, 0.8, 5.2, 0.14, 0., EDGE),
        p(13.5, 1.3, 1.6, 0.85, 0., WOOD),
        p(19.7, 1., 0.25, 0.25, 1., SIGHT),
        p(19.85, 1., 0.18, 0.3, 1., BORE),
    ];
    match weapon {
        "pistol" => Some(PISTOL),
        "smg" => Some(SMG),
        "shotgun" => Some(SHOTGUN),
        _ => None,
    }
}

/// Wo die Hände an der Waffe liegen: [links, rechts] als (vor, quer) in px.
pub fn hands(weapon: &str) -> Option<[[f32; 2]; 2]> {
    match weapon {
        "pistol" => Some([[6.9, -0.3], [7.5, 1.1]]),
        "smg" => Some([[11.2, 0.9], [6., 1.8]]),
        "shotgun" => Some([[13.5, 1.0], [6.6, 2.1]]),
        _ => None,
    }
}

/// Vorderes Ende des Laufs (größtes `f + hf` unter den Teilen auf der Laufachse) – muss die Mündung sein.
#[cfg(test)]
pub fn barrel_end(weapon: &str) -> Option<(f32, f32)> {
    parts(weapon)?
        .iter()
        .max_by(|a, b| (a.f + a.hf).total_cmp(&(b.f + b.hf)))
        .map(|q| (q.f + q.hf, q.r))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bullets_leave_from_the_end_of_the_barrel() {
        for w in ["pistol", "smg", "shotgun"] {
            let (f, r) = barrel_end(w).unwrap();
            let (mf, mr) = berlin_sim::combat::muzzle(w);
            assert!(
                (f - mf as f32).abs() < 0.3 && (r - mr as f32).abs() < 0.3,
                "{w}: Laufende ({f}, {r}) gegen Mündung ({mf}, {mr})"
            );
            // die Hände liegen hinter der Mündung, auf der Waffe (nicht daneben in der Luft)
            let h = hands(w).unwrap();
            for [hf, hr] in h {
                assert!(hf < f - 1., "{w}: Hand vor der Mündung");
                assert!(
                    parts(w)
                        .unwrap()
                        .iter()
                        .any(|q| (hf - q.f).abs() <= q.hf + 1.2 && (hr - q.r).abs() <= q.hr + 1.2),
                    "{w}: Hand ({hf}, {hr}) greift ins Leere"
                );
            }
        }
        assert!(parts("bat").is_none() && hands("knife").is_none());
        // längere Waffen haben weiter vorn liegende Mündungen
        let end = |w| barrel_end(w).unwrap().0;
        assert!(end("pistol") < end("smg") && end("smg") < end("shotgun"));
    }
}
