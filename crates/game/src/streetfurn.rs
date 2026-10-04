//! Straßenmöbel (Port von `render.js drawLamp`, `drawSign`/`signBoard`): Laternen mit Mast, Ausleger und Kopf
//! (Berliner Gaslaternen mit dunklem Dach), der nachts leuchtet; Wegweiser an Kreuzungszufahrten mit zwei Pfosten und
//! einer Tafel – gelbe Zeilen für Orte, weiße für Straßen, Pfeil in Richtung der Ausfahrt, Bundesstraßen als gelbes
//! Kästchen. Tafel und Pfosten liegen in der Szene (Dächer verdecken sie), Schrift und Pfeile im HUD darüber.
use berlin_engine::Body;
use berlin_engine::camera::Camera;
use berlin_engine::hud::{Align, Hud};
use berlin_sim::city::Sign;
use berlin_sim::lamps::Lamp;
use glam::Vec2;

/// Masthöhe in der Schrägansicht (8 m × heightScale / 2), Pfostenhöhe des Wegweisers
pub const LAMP_H: f32 = 20.;
pub const SIGN_POST: f32 = 30.;
const SIGN_FONT: f32 = 10.;
const PAD: f32 = 3.;
const ROW_H: f32 = SIGN_FONT + 5.;
const ARROW_W: f32 = 13.;
/// Breite eines Zeichens der Bitmapschrift bei Größe `SIGN_FONT` (Welt-px), grob
const CHAR_W: f32 = 6.2;

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
            r.refn.chars().count() as f32 * CHAR_W + 5.
        }
    };
    let w = sg
        .rows
        .iter()
        .zip(&texts)
        .map(|(r, t)| {
            ARROW_W
                + 4.
                + t.chars().count() as f32 * CHAR_W
                + if r.refn.is_empty() { 0. } else { ref_w(r) + 4. }
        })
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

/// Pfosten in der Szene (die Tafel selbst zeichnet das HUD, damit sie zur Bitmapschrift passt)
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
    }
}

/// Tafeln im Bildschirm über dem Pfosten: Rahmen, Zeilen (gelb Orte, weiß Straßen), Pfeil, Ziele, Nummernkästchen.
/// Ausgemessen mit der Breite der tatsächlich gezeichneten Bitmapschrift.
pub fn sign_texts(signs: &[Sign], camera: &Camera, viewport: Vec2, h: &mut Hud) {
    let ink = [0.067, 0.067, 0.067, 1.];
    let sc = h.scale;
    for sg in signs.iter().filter(|s| s.vis && !s.rows.is_empty()) {
        let b = board(sg);
        let to = |x: f32, y: f32| camera.world_to_screen(Vec2::new(x, y), 0., viewport) / sc;
        let post = to(sg.x as f32, sg.y as f32 - SIGN_POST);
        let k = (to(sg.x as f32 + 100., sg.y as f32).x - to(sg.x as f32, sg.y as f32).x) / 100.;
        if k < 0.35 {
            continue; // zu klein zum Lesen
        }
        let size = SIGN_FONT * k;
        let (pad, row_h, arrow) = (PAD * k, ROW_H * k, ARROW_W * k);
        let ref_w = |h: &Hud, r: &berlin_sim::city::SignRow| {
            if r.refn.is_empty() {
                0.
            } else {
                h.text_width(&r.refn, size) + 5. * k
            }
        };
        let w = sg
            .rows
            .iter()
            .zip(&b.texts)
            .map(|(r, t)| {
                let rw = ref_w(h, r);
                arrow + 4. * k + h.text_width(t, size) + if rw > 0. { rw + 4. * k } else { 0. }
            })
            .fold(50. * k, f32::max)
            + 2. * pad;
        let hgt = sg.rows.len() as f32 * row_h + 2. * pad;
        let rx = -(sg.angle as f32).sin();
        let left = if rx > 0.35 {
            post.x - 6. * k
        } else if rx < -0.35 {
            post.x - w + 6. * k
        } else {
            post.x - w / 2.
        };
        let top = post.y - hgt;
        if left + w < 0. || left > h.width || top + hgt < 0. || top > 720. {
            continue;
        }
        h.rect(left, top, w, hgt, [0.11, 0.11, 0.11, 1.], 1.);
        for (i, r) in sg.rows.iter().enumerate() {
            let y0 = top + pad + i as f32 * row_h;
            let bg = if r.street() {
                [0.957, 0.957, 0.94, 1.]
            } else {
                [0.96, 0.77, 0.09, 1.]
            };
            h.rect(left + k, y0, w - 2. * k, row_h - k, bg, 0.);
            let yc = y0 + row_h / 2.;
            // Pfeil zeigt in die Kartenrichtung der Ausfahrt (Norden oben)
            h.triangle(left + pad + arrow / 2., yc, 4.5 * k, r.dir as f32, ink);
            h.text(
                &b.texts[i],
                left + pad + arrow + 4. * k,
                yc + 3.5 * k,
                size,
                ink,
                Align::Left,
                false,
            );
            let rw = ref_w(h, r);
            if rw > 0. {
                let rx0 = left + w - pad - rw;
                h.rect(
                    rx0 - k,
                    y0 + 1.5 * k - k,
                    rw + 2. * k,
                    row_h - 4. * k + 2. * k,
                    ink,
                    0.,
                );
                h.rect(
                    rx0,
                    y0 + 1.5 * k,
                    rw,
                    row_h - 4. * k,
                    [0.96, 0.77, 0.09, 1.],
                    0.,
                );
                h.text(
                    &r.refn,
                    rx0 + 2.5 * k,
                    yc + 3.5 * k,
                    size,
                    ink,
                    Align::Left,
                    false,
                );
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
