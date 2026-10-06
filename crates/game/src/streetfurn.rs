//! Straßenmöbel (Port von `render.js drawLamp`, `drawSign`/`signBoard`): Laternen mit Mast, Ausleger und Kopf
//! (Berliner Gaslaternen mit dunklem Dach), der nachts leuchtet; Wegweiser an Kreuzungszufahrten mit zwei Pfosten und
//! einer Tafel – gelbe Zeilen für Orte, weiße für Straßen, Pfeil in Richtung der Ausfahrt, Bundesstraßen als gelbes
//! Kästchen. Alles liegt in der Welt (Dächer und Kronen verdecken es, nachts wird es dunkel): Pfosten, Tafel, Zeilen,
//! Pfeile als Körper, der Text als SDF-Zeichen der HUD-Schrift (`hud::world_glyph`, Body-Form 8).
use berlin_engine::Body;
use berlin_engine::hud;
use berlin_sim::city::Sign;
use berlin_sim::lamps::Lamp;

/// Masthöhe in der Schrägansicht (8 m × heightScale / 2), Pfostenhöhe des Wegweisers
pub const LAMP_H: f32 = 20.;
pub const SIGN_POST: f32 = 30.;
const SIGN_FONT: f32 = 10.;
const PAD: f32 = 3.;
const ROW_H: f32 = SIGN_FONT + 5.;
const ARROW_W: f32 = 13.;
/// Großbuchstabenhöhe der Schildschrift (Welt-px), wie früher die Bitmapschrift (7/8 der Größe)
const SIGN_CAP: f32 = SIGN_FONT * 0.875;
const INK: [f32; 4] = [0.067, 0.067, 0.067, 1.];
const YELLOW: [f32; 4] = [0.96, 0.77, 0.09, 1.];

/// Schriftgröße (em in Welt-px) der Schildschrift
fn sign_em() -> f32 {
    SIGN_CAP / hud::cap_height()
}
/// Breite eines Texts in Welt-px (Laufweiten der Schrift; fehlende Zeichen mit einem halben em)
fn text_w(t: &str) -> f32 {
    let em = sign_em();
    t.chars()
        .map(|c| hud::world_glyph(c).map_or(0.5, |g| g.adv) * em)
        .sum()
}
/// Text als SDF-Zeichen (Body-Form 8: die Farbe trägt das Atlas-Rechteck, die Tinte ist im Shader fest)
fn text_bodies(t: &str, x: f32, base: f32, depth: f32, out: &mut Vec<Body>) {
    let em = sign_em();
    let mut pen = x;
    for c in t.chars() {
        let Some(g) = hud::world_glyph(c) else {
            pen += 0.5 * em;
            continue;
        };
        if g.rect[2] > 0. {
            let (w, h) = (g.w * em, g.h * em);
            out.push(Body {
                center: [pen + g.left * em + w / 2., base + g.top * em + h / 2.],
                half: [w / 2., h / 2.],
                angle: 0.,
                shape: 8.,
                depth,
                color: g.rect,
            });
        }
        pen += g.adv * em;
    }
}
fn rect(cx: f32, cy: f32, hw: f32, hh: f32, depth: f32, color: [f32; 4], out: &mut Vec<Body>) {
    out.push(Body {
        center: [cx, cy],
        half: [hw, hh],
        angle: 0.,
        shape: 4.,
        depth,
        color,
    });
}

fn rgba(c: u32, a: f32) -> [f32; 4] {
    [
        ((c >> 16) & 255) as f32 / 255.,
        ((c >> 8) & 255) as f32 / 255.,
        (c & 255) as f32 / 255.,
        a,
    ]
}

/// Kopf der Laterne: Gaslaterne über dem Mast, sonst am Ausleger zur Fahrbahn.
pub fn lamp_head(lp: &Lamp) -> (f32, f32) {
    let (x, y) = (lp.x as f32, lp.y as f32);
    if lp.gas {
        (x, y - LAMP_H)
    } else {
        (x + lp.nx as f32 * 9., y - LAMP_H + lp.ny as f32 * 9.)
    }
}

/// Laternen: Schatten am Fuß, Mast, Ausleger, Kopf (an = hell)
pub fn lamp_bodies(lamps: &[Lamp], on: bool, out: &mut Vec<Body>) {
    for lp in lamps {
        let (x, y) = (lp.x as f32, lp.y as f32);
        let (hx, hy) = lamp_head(lp);
        let depth = 0.6;
        let pole = rgba(if lp.gas { 0x2f3a33 } else { 0x4a5058 }, 1.);
        out.push(Body {
            center: [x + 1., y + 1.],
            half: [2.2, 2.2],
            angle: 0.,
            shape: 1.,
            depth: 0.83,
            color: [0., 0., 0., 0.25],
        });
        out.push(Body {
            center: [x, y - LAMP_H / 2.],
            half: [LAMP_H / 2., 0.9],
            angle: std::f32::consts::FRAC_PI_2,
            shape: 0.,
            depth,
            color: pole,
        });
        let lit = if on {
            rgba(if lp.gas { 0xffd9a0 } else { 0xfff6de }, 1.)
        } else {
            rgba(0xc9ccd1, 1.)
        };
        if lp.gas {
            out.push(Body {
                center: [hx, hy],
                half: [2.6, 2.5],
                angle: 0.,
                shape: 0.,
                depth: depth - 0.0002,
                color: lit,
            });
            out.push(Body {
                center: [hx, hy - 4.],
                half: [3.6, 1.6],
                angle: 0.,
                shape: 1.,
                depth: depth - 0.0003,
                color: pole,
            });
        } else {
            let (ax, ay) = (x, y - LAMP_H);
            let (dx, dy) = (hx - ax, hy - ay);
            out.push(Body {
                center: [(ax + hx) / 2., (ay + hy) / 2.],
                half: [dx.hypot(dy) / 2., 0.8],
                angle: dy.atan2(dx),
                shape: 0.,
                depth: depth - 0.0001,
                color: pole,
            });
            let a = (lp.ny as f32).atan2(lp.nx as f32);
            out.push(Body {
                center: [hx, hy],
                half: [3.5, 2.2],
                angle: a,
                shape: 0.,
                depth: depth - 0.0002,
                color: rgba(0x3a4048, 1.),
            });
            out.push(Body {
                center: [hx, hy],
                half: [2.5, 1.2],
                angle: a,
                shape: 0.,
                depth: depth - 0.0003,
                color: lit,
            });
        }
    }
}

/// Lage der Tafel (Welt-px): links, oben, Breite, Höhe und je Zeile der Text.
#[derive(Debug, Clone, PartialEq)]
pub struct Board {
    pub left: f32,
    pub top: f32,
    pub w: f32,
    pub h: f32,
    pub texts: Vec<String>,
}
pub fn board(sg: &Sign) -> Board {
    let texts: Vec<String> = sg
        .rows
        .iter()
        .map(|r| {
            r.dests
                .iter()
                .take(2)
                .cloned()
                .collect::<Vec<_>>()
                .join(" · ")
        })
        .collect();
    let ref_w = |r: &berlin_sim::city::SignRow| {
        if r.refn.is_empty() {
            0.
        } else {
            text_w(&r.refn) + 5.
        }
    };
    let w = sg
        .rows
        .iter()
        .zip(&texts)
        .map(|(r, t)| ARROW_W + 4. + text_w(t) + if r.refn.is_empty() { 0. } else { ref_w(r) + 4. })
        .fold(50f32, f32::max)
        .ceil()
        + 2. * PAD;
    let h = sg.rows.len() as f32 * ROW_H + 2. * PAD;
    // Tafel reicht vom Pfosten weg von der Straße (rechts der Fahrtrichtung liegt der Gehweg)
    let rx = -(sg.angle as f32).sin();
    let x = sg.x as f32;
    let left = if rx > 0.35 {
        x - 6.
    } else if rx < -0.35 {
        x - w + 6.
    } else {
        x - w / 2.
    };
    Board {
        left,
        top: sg.y as f32 - SIGN_POST - h,
        w,
        h,
        texts,
    }
}

/// Wegweiser in der Welt: Schatten, Pfosten, Tafel mit Rahmen, je Zeile Grund (gelb Orte, weiß Straßen), Pfeil in
/// Kartenrichtung der Ausfahrt (Norden oben), Ziele und Nummernkästchen.
pub fn sign_bodies(signs: &[Sign], out: &mut Vec<Body>) {
    for sg in signs.iter().filter(|s| s.vis && !s.rows.is_empty()) {
        let (x, y) = (sg.x as f32, sg.y as f32);
        out.push(Body {
            center: [x + 1., y + 0.5],
            half: [2.5, 1.5],
            angle: 0.,
            shape: 1.,
            depth: 0.83,
            color: [0., 0., 0., 0.25],
        });
        out.push(Body {
            center: [x, y - SIGN_POST / 2.],
            half: [1.1, SIGN_POST / 2.],
            angle: 0.,
            shape: 0.,
            depth: 0.6,
            color: rgba(0x6b7078, 1.),
        });
        let b = board(sg);
        let d = 0.6 - 0.0002;
        rect(
            b.left + b.w / 2.,
            b.top + b.h / 2.,
            b.w / 2.,
            b.h / 2.,
            d,
            [0.11, 0.11, 0.11, 1.],
            out,
        );
        for (i, r) in sg.rows.iter().enumerate() {
            let y0 = b.top + PAD + i as f32 * ROW_H;
            let yc = y0 + ROW_H / 2.;
            let bg = if r.street() {
                [0.957, 0.957, 0.94, 1.]
            } else {
                YELLOW
            };
            rect(
                b.left + b.w / 2.,
                yc - 0.5,
                b.w / 2. - 1.,
                (ROW_H - 1.) / 2.,
                d - 0.00005,
                bg,
                out,
            );
            out.push(Body {
                center: [b.left + PAD + ARROW_W / 2., yc],
                half: [4.5, 4.5],
                angle: r.dir as f32,
                shape: 9.,
                depth: d - 0.0001,
                color: INK,
            });
            let base = yc + SIGN_CAP / 2.;
            text_bodies(
                &b.texts[i],
                b.left + PAD + ARROW_W + 4.,
                base,
                d - 0.00015,
                out,
            );
            if !r.refn.is_empty() {
                let rw = text_w(&r.refn) + 5.;
                let rx0 = b.left + b.w - PAD - rw;
                let (cy, hh) = (y0 + 1.5 + (ROW_H - 4.) / 2., (ROW_H - 4.) / 2.);
                rect(
                    rx0 + rw / 2.,
                    cy,
                    rw / 2. + 1.,
                    hh + 1.,
                    d - 0.0001,
                    INK,
                    out,
                );
                rect(rx0 + rw / 2., cy, rw / 2., hh, d - 0.00012, YELLOW, out);
                text_bodies(&r.refn, rx0 + 2.5, base, d - 0.00015, out);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use berlin_sim::city::SignRow;

    fn sign(angle: f64) -> Sign {
        Sign {
            x: 100.,
            y: 200.,
            angle,
            name: "Heerstraße / Wilhelmstraße".into(),
            vis: true,
            rows: vec![
                SignRow {
                    dir: 0.3,
                    turn: 0.,
                    dests: vec!["Zentrum".into(), "Westend".into()],
                    refn: "B 2".into(),
                },
                SignRow {
                    dir: 2.4,
                    turn: 2.,
                    dests: vec!["Wilhelmstraße".into()],
                    refn: String::new(),
                },
            ],
        }
    }

    #[test]
    fn board_sits_above_the_post_and_beside_the_road() {
        let b = board(&sign(0.));
        assert_eq!(b.h, 2. * ROW_H + 2. * PAD);
        assert!((b.top + b.h - (200. - SIGN_POST)).abs() < 1e-4);
        assert_eq!(b.texts[0], "Zentrum · Westend");
        assert!(b.w > 100.);
        // Fahrtrichtung Osten: rechts ist Süden, rx = 0 → mittig; Richtung Norden (−π/2): rechts ist Osten
        assert!((b.left + b.w / 2. - 100.).abs() < 1e-3);
        let n = board(&sign(-std::f64::consts::FRAC_PI_2));
        assert!((n.left - 94.).abs() < 1e-3, "{}", n.left);
        assert!(!sign(0.).rows[0].street() && sign(0.).rows[1].street());
    }

    #[test]
    fn sign_text_lies_on_the_board() {
        let sg = sign(0.);
        let b = board(&sg);
        let mut out = Vec::new();
        sign_bodies(&[sg], &mut out);
        let glyphs: Vec<_> = out.iter().filter(|g| g.shape == 8.).collect();
        // "Zentrum · Westend", "B 2", "Wilhelmstraße" ohne Leerzeichen
        assert!(glyphs.len() >= 25, "{}", glyphs.len());
        for g in &glyphs {
            assert!(
                g.center[0] - g.half[0] >= b.left - 0.5
                    && g.center[0] + g.half[0] <= b.left + b.w + 0.5
            );
            assert!(
                g.center[1] - g.half[1] >= b.top - 0.5
                    && g.center[1] + g.half[1] <= b.top + b.h + 0.5
            );
            assert!(g.depth < 0.6, "vor dem Pfosten");
        }
        assert_eq!(
            out.iter().filter(|g| g.shape == 9.).count(),
            2,
            "ein Pfeil je Zeile"
        );
    }

    #[test]
    fn lamp_heads_and_bodies() {
        let lp = Lamp {
            x: 10.,
            y: 50.,
            nx: 1.,
            ny: 0.,
            rgb: [255; 3],
            gas: false,
            main: true,
        };
        assert_eq!(lamp_head(&lp), (19., 30.));
        let mut out = Vec::new();
        lamp_bodies(&[lp, Lamp { gas: true, ..lp }], true, &mut out);
        assert_eq!(out.len(), 5 + 4);
    }
}
