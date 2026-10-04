//! Aussehen der Passanten je Typ (vereinfachter Port von `figure.js figureLook` und `assets.js drawDog`): Oberteil,
//! Haare oder Kopfbedeckung, Tasche und ein Zubehör, das die Silhouette von oben prägt (Kinderwagen, Hund an der
//! Leine, Stock, Aktentasche …). Alles deterministisch aus der Nummer, nur Darstellung.
use berlin_engine::Body;
use berlin_sim::figure::{Kind, h01};
use berlin_sim::pedestrians::Ped;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Acc {
    None,
    Briefcase,
    Cane,
    Shopping,
    Stroller,
    Camera,
    Phone,
    Bottle,
    Dog,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Look {
    pub top: u32,
    pub hair: Option<u32>,
    pub hat: Option<u32>,
    pub bag: Option<u32>,
    pub acc: Acc,
    /// Farbe des Zubehörs (Kinderwagen, Hund)
    pub acc_color: u32,
    pub scale: f32,
}

const HAIR: [u32; 7] = [
    0x2b2118, 0x4a3524, 0x1a1a1a, 0x8a6a3d, 0xc9a45a, 0xa33b20, 0x5a4636,
];
const GREY: [u32; 4] = [0xd8d4cf, 0xb9b5ae, 0xe8e6e2, 0x9a968f];
const BAGS: [u32; 5] = [0x6d4c2f, 0x2f3b52, 0x8a2f2f, 0x3d6b4f, 0x1f1f1f];
const DOGS: [u32; 5] = [0x6b4a2b, 0x1d1d1d, 0xd9b27a, 0x8c8c8c, 0xf2ead8];

fn pick(a: &[u32], r: f64) -> u32 {
    a[((r * a.len() as f64) as usize) % a.len()]
}

pub fn look_of(p: &Ped) -> Look {
    let id = f64::from(p.id);
    let h = |k: f64| h01(id, k);
    let mut l = Look {
        top: p.shirt,
        hair: Some(pick(&HAIR, h(2.))),
        hat: None,
        bag: None,
        acc: Acc::None,
        acc_color: pick(&BAGS, h(18.)),
        scale: 1.,
    };
    match p.kind {
        Kind::Everyday => {
            if h(9.) < 0.12 {
                l.hat = Some(pick(&[0x1d1d1d, 0xc0392b, 0x2f5aa8], h(13.)));
            }
            l.bag = (h(4.) < 0.32).then(|| pick(&BAGS, h(18.)));
        }
        Kind::Business => {
            l.top = pick(&[0x1f2733, 0x2b2d33, 0x3a3f4a, 0x2a3a55, 0x4a4038], h(6.));
            if h(11.) >= 0.3 && h(11.) < 0.4 {
                l.hair = None; // Glatze
            }
            l.acc = if h(12.) < 0.6 {
                Acc::Briefcase
            } else {
                Acc::Phone
            };
            l.acc_color = 0x3a2a1c;
        }
        Kind::Tourist => {
            l.top = pick(
                &[0xff6b6b, 0xffd93d, 0x6bcb77, 0x4d96ff, 0xffffff, 0xff9f43],
                h(6.),
            );
            if h(9.) < 0.8 {
                l.hat = Some(pick(&[0xf5f5f5, 0xe8c46a, 0x2f5aa8, 0xc0392b], h(13.)));
            }
            l.bag = Some(pick(&BAGS, h(18.)));
            l.acc = Acc::Camera;
        }
        Kind::Senior => {
            l.top = pick(
                &[0x8a7a62, 0x6b5a48, 0x5a6068, 0x7a6a8a, 0xa08a6a, 0x4a5a4a],
                h(6.),
            );
            l.hair = (h(11.) >= 0.3).then(|| pick(&GREY, h(2.)));
            if h(9.) < 0.35 {
                l.hat = Some(pick(&[0x4a3a2a, 0x3a3a3a, 0x6a5a4a], h(13.)));
            }
            l.acc = if h(12.) < 0.55 {
                Acc::Cane
            } else if h(12.) < 0.75 {
                Acc::Shopping
            } else {
                Acc::None
            };
            l.scale = 0.96;
        }
        Kind::Teen => {
            l.top = pick(
                &[
                    0x2d3436, 0x6c5ce7, 0xe17055, 0x00b894, 0xfdcb6e, 0xb2bec3, 0xd63031,
                ],
                h(6.),
            );
            if h(9.) < 0.35 {
                l.hat = Some(pick(&[0x1d1d1d, 0xc0392b, 0xf5f5f5], h(13.)));
            }
            l.acc = if h(12.) < 0.5 { Acc::Phone } else { Acc::None };
            l.bag = (h(4.) < 0.4).then(|| pick(&BAGS, h(18.)));
            l.scale = 0.86;
        }
        Kind::Hipster => {
            l.top = pick(
                &[0x5b6b4a, 0x7a4a3a, 0x3a4a5a, 0xc9a24a, 0x2b2b2b, 0x8a6a8a],
                h(6.),
            );
            if h(9.) < 0.55 {
                l.hat = Some(pick(
                    &[0xc0392b, 0xe1a95f, 0x2f3b52, 0x6b8a4a, 0x1d1d1d],
                    h(13.),
                ));
            }
            l.bag = Some(0xe8dcc0);
        }
        Kind::Worker => {
            l.top = pick(&[0xff8c1a, 0xe8e82a], h(6.));
            if h(9.) < 0.5 {
                l.hat = Some(if h(13.) < 0.6 { 0xf5f5f5 } else { 0xf1c40f });
            }
            l.scale = 1.06;
        }
        Kind::Punk => {
            l.top = 0x1d1d1d;
            l.hair = Some(pick(
                &[0xe84393, 0x00cec9, 0xfdcb6e, 0xd63031, 0x6c5ce7, 0x2ecc71],
                h(2.),
            ));
            l.acc = if h(12.) < 0.3 { Acc::Bottle } else { Acc::None };
        }
        Kind::Headscarf => {
            l.top = pick(&[0x3a3a4a, 0x5a4a5a, 0x2f3b52, 0x6b5a4a, 0x4a5a5a], h(6.));
            l.hat = Some(pick(
                &[0x8a2f5a, 0x2f5a8a, 0xd8c8b8, 0x5a8a6a, 0x1d1d1d, 0xc9a24a],
                h(13.),
            ));
            l.acc = if h(12.) < 0.5 {
                Acc::Shopping
            } else {
                Acc::None
            };
        }
        Kind::Parent => {
            l.top = pick(&[0x5a8aa8, 0xa85a6a, 0x6a8a5a, 0x8a7a5a, 0x4a4a5a], h(6.));
            l.acc = Acc::Stroller;
            l.acc_color = pick(&[0x2f3b52, 0x6b3a4a, 0x3a5a4a, 0x5a5a5a, 0x8a6a3a], h(10.));
        }
        Kind::Jogger => {} // Trikot kommt aus p.shirt
        Kind::Dogwalker => {
            l.top = pick(&[0x4a5a3a, 0x5a4a3a, 0x2f3b52, 0x7a6a5a], h(6.));
            if h(9.) < 0.3 {
                l.hat = Some(pick(&[0x6b8a4a, 0x2f3b52], h(13.)));
            }
            l.acc = Acc::Dog;
            l.acc_color = DOGS[p.id as usize % 5];
        }
    }
    l
}

fn rgba(c: u32) -> [f32; 4] {
    [
        ((c >> 16) & 255) as f32 / 255.,
        ((c >> 8) & 255) as f32 / 255.,
        (c & 255) as f32 / 255.,
        1.,
    ]
}
fn shade(c: [f32; 4], k: f32) -> [f32; 4] {
    [c[0] * k, c[1] * k, c[2] * k, c[3]]
}

/// Eine gehende/stehende Person von oben: Schatten, Tasche, Körper, Kopf (Haare/Hut), Zubehör.
pub fn person_bodies(p: &Ped, look: &Look, depth: f32, t: f64, out: &mut Vec<Body>) {
    let (x, y, a) = (p.x as f32, p.y as f32, p.facing as f32);
    let k = look.scale;
    let (c, s) = (a.cos(), a.sin());
    // Punkt im Personenrahmen: f nach vorn, r nach rechts
    let at = |f: f32, r: f32| [x + c * f - s * r, y + s * f + c * r];
    let mut push =
        |center: [f32; 2], half: [f32; 2], angle: f32, shape: f32, d: f32, color: [f32; 4]| {
            out.push(Body {
                center,
                half,
                angle,
                shape,
                depth: depth + d,
                color,
            });
        };
    let step = (p.step * 0.6).sin() as f32;
    push(
        [x + 1.5, y + 2.],
        [6. * k, 6. * k],
        0.,
        1.,
        0.0003,
        [0., 0., 0., 0.25],
    );
    match look.acc {
        Acc::Stroller => {
            // Wagen vor der Person, Griff quer
            push(at(12., 0.), [6., 4.2], a, 0., 0.00025, rgba(look.acc_color));
            push(
                at(13., 0.),
                [3.8, 2.8],
                a,
                1.,
                0.0002,
                shade(rgba(look.acc_color), 1.35),
            );
            push(at(5.5, 0.), [0.7, 4.], a, 4., 0.0002, rgba(0x2a2a2a));
        }
        Acc::Dog => {
            // Hund hinten links an der Leine, trabt mit dem Schritt
            let (dx, dy) = (-15. + step * 1.2, -6.);
            let d = at(dx, dy);
            let col = rgba(look.acc_color);
            let lx = (d[0] + x) / 2.;
            let ly = (d[1] + y) / 2.;
            let len = (d[0] - x).hypot(d[1] - y) / 2.;
            let la = (d[1] - y).atan2(d[0] - x);
            push(
                [lx, ly],
                [len, 0.35],
                la,
                4.,
                0.00035,
                [0.16, 0.12, 0.08, 0.8],
            );
            push(
                [d[0] + 1., d[1] + 1.5],
                [6., 3.],
                a,
                1.,
                0.0004,
                [0., 0., 0., 0.25],
            );
            push(d, [5., 2.4], a, 1., 0.00028, col);
            push(
                [d[0] + c * 5., d[1] + s * 5.],
                [2., 2.],
                0.,
                1.,
                0.00026,
                col,
            );
            let wag = (t * 12.).sin() as f32 * 1.6;
            push(
                [d[0] - c * 6. - s * wag, d[1] - s * 6. + c * wag],
                [1.8, 0.5],
                a,
                4.,
                0.00029,
                shade(col, 0.8),
            );
        }
        _ => {}
    }
    if let Some(b) = look.bag {
        push(at(-4.5 * k, 0.), [2., 3.6 * k], a, 0., 0.0001, rgba(b));
    }
    push([x, y], [4.5 * k, 6.5 * k], a, 1., 0., rgba(look.top));
    // Zubehör in der Hand (rechts) oder Stock
    let hand = at(1.5, 6.5 * k);
    match look.acc {
        Acc::Briefcase | Acc::Shopping => push(
            [hand[0], hand[1]],
            [2.4, 1.2],
            a,
            0.,
            -0.0001,
            rgba(if look.acc == Acc::Shopping {
                0xd8c8a8
            } else {
                look.acc_color
            }),
        ),
        Acc::Cane => push(at(5., 6.), [0.6, 0.6], 0., 1., -0.0001, rgba(0x4a3424)),
        Acc::Camera => push(at(4.5, 0.), [1.2, 2.], a, 0., -0.0004, rgba(0x1d1d1d)),
        Acc::Phone => push(at(4., 2.), [0.9, 1.4], a, 0., -0.0004, rgba(0x15171b)),
        Acc::Bottle => push(hand, [1.3, 0.7], a, 0., -0.0001, rgba(0x2f6b3a)),
        _ => {}
    }
    push([x, y], [3. * k, 3. * k], 0., 1., -0.0002, rgba(p.skin));
    if let Some(hat) = look.hat {
        push(at(-0.4, 0.), [2.9 * k, 2.9 * k], 0., 1., -0.0003, rgba(hat));
    } else if let Some(hair) = look.hair {
        push(
            at(-0.9, 0.),
            [2.6 * k, 2.6 * k],
            0.,
            1.,
            -0.0003,
            rgba(hair),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looks_follow_kind() {
        let mut p: Ped = test_ped();
        p.kind = Kind::Parent;
        assert_eq!(look_of(&p).acc, Acc::Stroller);
        p.kind = Kind::Dogwalker;
        assert_eq!(look_of(&p).acc, Acc::Dog);
        p.kind = Kind::Jogger;
        assert_eq!(look_of(&p).top, p.shirt);
        p.kind = Kind::Headscarf;
        assert!(look_of(&p).hat.is_some());
        let mut out = Vec::new();
        p.kind = Kind::Dogwalker;
        person_bodies(&p, &look_of(&p), 0.6, 0., &mut out);
        assert!(out.len() >= 8, "Person + Hund + Leine: {}", out.len());
        assert!(out.iter().all(|b| b.depth > 0.5 && b.center[0].is_finite()));
    }

    fn test_ped() -> Ped {
        use berlin_sim::pedestrians::PedState;
        Ped {
            id: 7,
            x: 100.,
            y: 50.,
            facing: 0.3,
            edge: 0,
            side: 1,
            s: 0.,
            dir: 1,
            speed: 50.,
            state: PedState::Walk,
            t: 0.,
            next_idle: 0.,
            shirt: 0x123456,
            skin: 0xf2d0b1,
            threat: (0., 0.),
            target: None,
            step: 0.,
            car_hit_cd: 0.,
            wait: 0.,
            cross_t: 0.,
            return_t: 0.,
            return_best: 0.,
            dead_t: 0.,
            hp: 100.,
            level: Default::default(),
            level_init: true,
            fall: 0.,
            fight_t: 0.,
            hit_cd: 0.,
            punch: 0.,
            reported: false,
            hang: None,
            hang_t: 0.,
            kind: Kind::Everyday,
            style: Default::default(),
        }
    }
}
