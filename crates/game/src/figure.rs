//! Aussehen der Passanten je Typ (vereinfachter Port von `figure.js figureLook` und `assets.js drawDog`): Oberteil,
//! Haare oder Kopfbedeckung, Tasche und ein Zubehör, das die Silhouette von oben prägt (Kinderwagen, Hund an der
//! Leine, Stock, Aktentasche …). Alles deterministisch aus der Nummer, nur Darstellung.
use crate::figart::{self, Part};
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
    pub pants: u32,
    pub shoes: u32,
    /// gemalte Teile (`figart`): Rumpf nach Statur, Frisur, Kopfbedeckung, Tasche
    pub build: Part,
    pub hair_part: Part,
    pub hat_part: Part,
    pub bag_part: Part,
}

const HAIR: [u32; 7] = [
    0x2b2118, 0x4a3524, 0x1a1a1a, 0x8a6a3d, 0xc9a45a, 0xa33b20, 0x5a4636,
];
const GREY: [u32; 4] = [0xd8d4cf, 0xb9b5ae, 0xe8e6e2, 0x9a968f];
const BAGS: [u32; 5] = [0x6d4c2f, 0x2f3b52, 0x8a2f2f, 0x3d6b4f, 0x1f1f1f];
const PANTS: [u32; 8] = [
    0x2c3e50, 0x34495e, 0x1f2a36, 0x5d4e3c, 0x3d5a80, 0x6b6b6b, 0x2b2b2b, 0x7a5c3a,
];
const JEANS: [u32; 5] = [0x3d5a80, 0x2c3e66, 0x4a6a94, 0x26324a, 0x1d2433];
const SHOES: [u32; 5] = [0x1e2126, 0x3a2a1c, 0xf0f0f0, 0x2c2c34, 0x7a5230];
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
        pants: if h(7.) < 0.55 {
            pick(&JEANS, h(8.))
        } else {
            pick(&PANTS, h(8.))
        },
        shoes: pick(&SHOES, h(14.)),
        build: [
            Part::TorsoSlim,
            Part::TorsoMid,
            Part::TorsoMid,
            Part::TorsoBroad,
        ][(h(21.) * 4.) as usize % 4],
        hair_part: [
            Part::HairShort,
            Part::HairShort,
            Part::HairLong,
            Part::HairBun,
            Part::HairCurly,
            Part::HairPony,
        ][(h(22.) * 6.) as usize % 6],
        hat_part: if h(23.) < 0.5 {
            Part::HatCap
        } else {
            Part::HatBeanie
        },
        bag_part: if h(24.) < 0.4 {
            Part::Backpack
        } else {
            Part::Bag
        },
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
            l.pants = l.top;
            l.shoes = 0x15171b;
            l.build = Part::TorsoMid;
            l.hair_part = if h(22.) < 0.7 {
                Part::HairShort
            } else {
                Part::HairBun
            };
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
            l.bag_part = Part::Backpack;
            l.hat_part = if h(25.) < 0.6 {
                Part::HatSun
            } else {
                Part::HatCap
            };
            l.acc = Acc::Camera;
        }
        Kind::Senior => {
            l.top = pick(
                &[0x8a7a62, 0x6b5a48, 0x5a6068, 0x7a6a8a, 0xa08a6a, 0x4a5a4a],
                h(6.),
            );
            l.hair = (h(11.) >= 0.3).then(|| pick(&GREY, h(2.)));
            l.hair_part = if h(22.) < 0.6 {
                Part::HairShort
            } else {
                Part::HairBun
            };
            l.hat_part = Part::HatCap;
            l.build = Part::TorsoMid;
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
            l.bag_part = Part::Backpack;
            l.hat_part = Part::HatCap;
            l.build = Part::TorsoSlim;
            l.hair_part = [
                Part::HairCurly,
                Part::HairPony,
                Part::HairShort,
                Part::HairLong,
            ][(h(22.) * 4.) as usize % 4];
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
            l.bag_part = Part::Bag;
            l.hat_part = Part::HatBeanie;
            l.hair_part =
                [Part::HairBun, Part::HairCurly, Part::HairLong][(h(22.) * 3.) as usize % 3];
        }
        Kind::Worker => {
            l.top = pick(&[0xff8c1a, 0xe8e82a], h(6.));
            l.pants = pick(&[0x3a4a5a, 0x2b3a2b, 0x4a3a2a], h(8.));
            l.shoes = 0x4a3a24;
            l.build = Part::TorsoBroad;
            l.hat_part = Part::HatHard;
            if h(9.) < 0.5 {
                l.hat = Some(if h(13.) < 0.6 { 0xf5f5f5 } else { 0xf1c40f });
            }
            l.scale = 1.06;
        }
        Kind::Punk => {
            l.top = 0x1d1d1d;
            l.pants = if h(7.) < 0.5 { 0x1d1d1d } else { 0x6a2a2a };
            l.hair = Some(pick(
                &[0xe84393, 0x00cec9, 0xfdcb6e, 0xd63031, 0x6c5ce7, 0x2ecc71],
                h(2.),
            ));
            l.acc = if h(12.) < 0.3 { Acc::Bottle } else { Acc::None };
            l.hair_part = Part::HairMohawk;
        }
        Kind::Headscarf => {
            l.top = pick(&[0x3a3a4a, 0x5a4a5a, 0x2f3b52, 0x6b5a4a, 0x4a5a5a], h(6.));
            l.hat_part = Part::HatScarf;
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
        Kind::Jogger => {
            // Trikot kommt aus p.shirt
            l.pants = 0x1d1d1d;
            l.shoes = pick(&[0xf0f0f0, 0xff6b6b, 0x4d96ff], h(14.));
            l.build = Part::TorsoSlim;
            l.hair_part = if h(22.) < 0.5 {
                Part::HairPony
            } else {
                Part::HairShort
            };
        }
        Kind::Dogwalker => {
            l.top = pick(&[0x4a5a3a, 0x5a4a3a, 0x2f3b52, 0x7a6a5a], h(6.));
            if h(9.) < 0.3 {
                l.hat = Some(pick(&[0x6b8a4a, 0x2f3b52], h(13.)));
            }
            l.hat_part = Part::HatBeanie;
            l.acc = Acc::Dog;
            l.acc_color = DOGS[p.id as usize % 5];
        }
    }
    // Körpergröße streut je Person (±8 %), über die Größe des Typs hinaus
    l.scale *= 0.92 + 0.16 * h(30.) as f32;
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

/// Wer gezeichnet wird: Lage, Blickrichtung, zurückgelegte Strecke (treibt den Schritt), Tempo-Lage.
#[derive(Debug, Clone, Copy)]
pub struct Who {
    pub x: f64,
    pub y: f64,
    pub facing: f64,
    /// zurückgelegte Strecke (px): die Schrittphase hängt an der Strecke, nicht an der Zeit
    pub step: f64,
    /// 0 steht … 1 geht voll aus
    pub amp: f32,
    /// 0 geht … 1 rennt
    pub run: f32,
    pub skin: u32,
    /// Hände an einer Schusswaffe ([links, rechts] als (vor, quer) in px, `weaponart::hands`): Arme gestreckt, Hände
    /// vor der Waffe, der Oberkörper dreht nicht mit dem Schritt
    pub hold: Option<[[f32; 2]; 2]>,
    /// Treffer-Reaktion: Richtung, aus der der Treffer kam (rad, Weltsystem), und Zeit seit dem Treffer (s)
    pub hit: Option<(f32, f32)>,
}

/// Gangbild aus dem Tempo relativ zum normalen Gehen (`q` = Tempo / `WALK`): (Ausschlag, Anteil Laufbewegung).
pub fn gait_of(q: f32) -> (f32, f32) {
    let amp = (0.75 + 0.45 * (q - 0.8)).clamp(0.65, 1.3);
    let brisk = ((q - 1.15) / 0.5).clamp(0., 0.4);
    (amp, brisk)
}
/// Wie lange ein Treffer sichtbar nachwirkt (s)
pub const HIT_REACT_S: f32 = 0.45;
/// Stärke der Treffer-Reaktion über die Zeit: in 50 ms voll ausgeschlagen (der Ruck), dann federnd zurück (0 … 1).
pub fn hit_react(t: f32) -> f32 {
    if !(0. ..HIT_REACT_S).contains(&t) {
        0.
    } else if t < 0.05 {
        t / 0.05
    } else {
        let u = (t - 0.05) / (HIT_REACT_S - 0.05);
        (1. - u) * (1. - u) * (1. + 0.6 * (u * 9.).sin() * (1. - u))
    }
    .clamp(0., 1.)
}
impl Who {
    pub fn of(p: &Ped) -> Self {
        use berlin_sim::pedestrians::PedState as S;
        let moving = matches!(p.state, S::Walk | S::Cross | S::Flee | S::Return);
        let run = p.state == S::Flee || p.kind == Kind::Jogger;
        // Gangbild nach dem eigenen Tempo: Bummler mit kurzem Schritt, Eilige ausgreifend und mit Schwung
        let q = (p.speed / berlin_sim::world::WALK) as f32;
        let (amp, brisk) = gait_of(q);
        Self {
            x: p.x,
            y: p.y,
            facing: p.facing,
            step: p.step,
            amp: if moving { amp } else { 0. },
            run: if moving && run {
                1.
            } else if moving {
                brisk
            } else {
                0.
            },
            skin: p.skin,
            hold: None,
            // getroffen und noch auf den Beinen: Richtung vom Angreifer weg
            hit: (p.state != S::Dead && (p.hurt_t as f32) < HIT_REACT_S).then(|| {
                (
                    ((p.y - p.threat.1).atan2(p.x - p.threat.0)) as f32,
                    p.hurt_t as f32,
                )
            }),
        }
    }
}

/// Der Spieler (figure.js PLAYER_LOOK): orange Jacke, dunkle Jeans, weiße Turnschuhe, kurze dunkle Haare.
pub fn player_look() -> Look {
    Look {
        top: 0xff7a1a,
        hair: Some(0x2b2118),
        hat: None,
        bag: None,
        acc: Acc::None,
        acc_color: 0,
        scale: 1.05,
        pants: 0x26324a,
        shoes: 0xf2f2f2,
        build: Part::TorsoMid,
        hair_part: Part::HairShort,
        hat_part: Part::HatCap,
        bag_part: Part::Bag,
    }
}

/// Spieler 2 (Koop): andere Jacke, Pferdeschwanz, Mütze – auf einen Blick vom ersten zu unterscheiden.
pub fn player2_look() -> Look {
    Look {
        top: 0x1e9e6a,
        hair: Some(0x8a4b22),
        hat: Some(0xf2b134),
        bag: None,
        acc: Acc::None,
        acc_color: 0,
        scale: 1.05,
        pants: 0x2f2a28,
        shoes: 0x1d1d1d,
        build: Part::TorsoMid,
        hair_part: Part::HairPony,
        hat_part: Part::HatBeanie,
        bag_part: Part::Bag,
    }
}

/// Pose (gait.js gaitPose) ohne eigenen Zustand: Phase aus der Strecke (ein Doppelschritt ≈ 28 px gehend, 52 px
/// rennend – das entspricht der Kadenz in gait.js), Ausschlag aus dem Bewegungszustand. Lokale Koordinaten:
/// +x vorn, +y rechts. Liefert Füße (x vor/zurück), Hände (x, y) und den Hüftschwung.
pub fn pose(who: &Who, acc: Acc) -> ([f32; 2], [[f32; 2]; 2], f32) {
    let len = 28. + 24. * who.run as f64;
    let ph = (who.step / len * std::f64::consts::TAU) as f32;
    let (s, amp, run) = (ph.sin(), who.amp, who.run);
    let stride = amp * (4.6 + run * 3.) * if acc == Acc::Stroller { 0.8 } else { 1. };
    let swing = amp
        * (3.2 + run * 2.2)
        * if acc == Acc::Briefcase || acc == Acc::Shopping {
            0.45
        } else {
            1.
        };
    let inward = run * 1.2;
    let mut hands = [
        [-s * swing + run * 1.2, -(5.2 - inward)],
        [s * swing + run * 1.2, 5.2 - inward],
    ];
    match acc {
        Acc::Stroller => hands = [[6.2, -2.6], [6.2, 2.6]],
        Acc::Cane => hands[1] = [3.2 + s * amp * 1.4, 5.4],
        Acc::Phone | Acc::Camera => hands[1] = [3.6, 1.6],
        _ => {}
    }
    (
        [s * stride, -s * stride],
        hands,
        s * amp * (0.1 + run * 0.08),
    )
}

/// Eine gehende/stehende Person von oben (people.js drawPerson, vereinfacht): Schatten, Schuhe und Hosenbeine im
/// Schritt, Tasche, Oberkörper mit Schultern (dreht gegen die Hüfte), Arme mit Ellbogen und Händen, Kopf mit Ohren,
/// Haaren oder Kopfbedeckung, Zubehör (Kinderwagen, Hund an der Leine, Stock, Tasche …).
pub fn person_bodies(who: &Who, look: &Look, depth: f32, t: f64, out: &mut Vec<Body>) {
    let (mut x, mut y, a) = (who.x as f32, who.y as f32, who.facing as f32);
    let k = look.scale;
    let (feet, mut hands, mut twist) = pose(who, look.acc);
    // wer steht, verlagert langsam das Gewicht (Phase je Person aus der Lage)
    if who.amp < 0.05 && who.hold.is_none() {
        let ph = (who.x * 0.37 + who.y * 0.71) as f32;
        let sway = (t as f32 * 1.1 + ph).sin();
        x += -a.sin() * sway * 0.35 * k;
        y += a.cos() * sway * 0.35 * k;
        twist += sway * 0.05;
    }
    // Treffer: der Körper ruckt vom Angreifer weg, der Oberkörper dreht ab, die Arme zucken zur Brust
    let hit = who.hit.map_or(0., |(_, t)| hit_react(t));
    if let Some((dir, _)) = who.hit {
        x += dir.cos() * 2.4 * k * hit;
        y += dir.sin() * 2.4 * k * hit;
        // von welcher Seite: der Oberkörper dreht mit dem Stoß
        let side = (dir - a).sin();
        twist += 0.55 * hit * if side >= 0. { 1. } else { -1. };
        for (i, h) in hands.iter_mut().enumerate() {
            let tgt = [1.8, if i == 0 { -2.6 } else { 2.6 }];
            h[0] += (tgt[0] - h[0]) * hit * 0.8;
            h[1] += (tgt[1] - h[1]) * hit * 0.8;
        }
    }
    if let Some(h) = who.hold {
        hands = [[h[0][0] / k, h[0][1] / k], [h[1][0] / k, h[1][1] / k]];
        twist = 0.;
    }
    // Hände an der Waffe liegen vor ihr (sonst unter dem Oberkörper)
    let hand_d = if who.hold.is_some() { -0.0004 } else { 0. };
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
    // im ersten Moment blitzt die Figur rötlich auf
    let flash = who.hit.map_or(0., |(_, t)| (1. - t / 0.12).clamp(0., 1.));
    let tint = |c: [f32; 4]| {
        [
            c[0] + (1. - c[0]) * flash * 0.7,
            c[1] * (1. - flash * 0.45),
            c[2] * (1. - flash * 0.45),
            c[3],
        ]
    };
    let skin = tint(rgba(who.skin));
    let top = tint(rgba(look.top));
    push(
        [x + 1.5, y + 2.],
        [6. * k, 6. * k],
        0.,
        1.,
        0.0004,
        [0., 0., 0., 0.25],
    );
    match look.acc {
        Acc::Stroller => {
            // Wagen vor der Person, Griff quer
            push(
                at(12.4, 0.),
                figart::half([6.6, 4.8]),
                a,
                figart::shape(Part::Stroller),
                0.00025,
                rgba(look.acc_color),
            );
            push(at(6.2, 0.), [0.7, 4.], a, 4., 0.0002, rgba(0x2a2a2a));
        }
        Acc::Dog => {
            // Hund hinten links an der Leine, trabt im Schritt
            let step = (who.step * 0.6).sin() as f32;
            let d = at(-15. + step * 1.2, -6.);
            let col = rgba(look.acc_color);
            let hand = at(hands[0][0] * k, hands[0][1] * k);
            let (lx, ly) = ((d[0] + hand[0]) / 2., (d[1] + hand[1]) / 2.);
            let len = (d[0] - hand[0]).hypot(d[1] - hand[1]) / 2.;
            let la = (d[1] - hand[1]).atan2(d[0] - hand[0]);
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
            for (lf, lr) in [(2., -2.4), (2., 2.4), (-3., -2.4), (-3., 2.4)] {
                let o = if lf > 0. { step } else { -step } * 1.5;
                let q = [d[0] + c * (lf + o) - s * lr, d[1] + s * (lf + o) + c * lr];
                push(q, [0.8, 0.6], a, 4., 0.00031, shade(col, 0.7));
            }
            // gemalter Hund (Rumpf, Kopf, Ohren, Nase): zwei Größen aus der Fellfarbe
            let size = if look.acc_color & 0x10 == 0 {
                0.85
            } else {
                1.12
            };
            push(
                [d[0] + c * 1.3 * size, d[1] + s * 1.3 * size],
                figart::half([6.7 * size, 5.7 * size]),
                a,
                figart::shape(Part::Dog),
                0.00027,
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
    // Schuhe und Hosenbeine (unter dem Oberkörper, die Hüfte schwingt mit)
    let pants = rgba(look.pants);
    for (i, side) in [-1.9f32, 1.9].into_iter().enumerate() {
        let fx = feet[i] * k;
        let f = at(fx + 1.2, side * k);
        push(f, [1.9 * k, 1.15 * k], a, 0., 0.00018, rgba(look.shoes));
        let leg = at(fx * 0.5, side * k);
        push(
            leg,
            [(fx.abs() * 0.5 + 1.6) * k, 1.3 * k],
            a,
            0.,
            0.00016,
            pants,
        );
    }
    if let Some(b) = look.bag {
        push(
            at(-4.4 * k, 0.),
            figart::half([2.1, 3.7 * k]),
            a + twist,
            figart::shape(look.bag_part),
            0.0001,
            rgba(b),
        );
    }
    // Arme: Oberarm (Schulter → Ellbogen) im Oberteil, Unterarm, Hand
    let ta = a + twist;
    let (tc, ts) = (ta.cos(), ta.sin());
    let tat = |f: f32, r: f32| [x + tc * f - ts * r, y + ts * f + tc * r];
    for (i, side) in [-1f32, 1.].into_iter().enumerate() {
        let sh = [0., side * 4.4 * k];
        let hd = [hands[i][0] * k, hands[i][1] * k];
        let el = [
            (sh[0] + hd[0]) / 2. - 0.6,
            (sh[1] + hd[1]) / 2. + side * 0.9,
        ];
        let seg =
            |out: &mut Vec<Body>, p0: [f32; 2], p1: [f32; 2], w: f32, color: [f32; 4], d: f32| {
                let (a0, a1) = (tat(p0[0], p0[1]), tat(p1[0], p1[1]));
                let (dx, dy) = (a1[0] - a0[0], a1[1] - a0[1]);
                out.push(Body {
                    center: [(a0[0] + a1[0]) / 2., (a0[1] + a1[1]) / 2.],
                    half: [dx.hypot(dy) / 2. + w * 0.5, w],
                    angle: dy.atan2(dx),
                    shape: 0.,
                    depth: depth + d,
                    color,
                });
            };
        seg(out, sh, el, 1.25 * k, shade(top, 0.92), 0.00006);
        seg(out, el, hd, 1.05 * k, shade(top, 0.85), 0.00005 + hand_d);
        let h = tat(hd[0], hd[1]);
        out.push(Body {
            center: h,
            half: [1.05 * k, 1.05 * k],
            angle: 0.,
            shape: 1.,
            depth: depth + 0.00004 + hand_d,
            color: skin,
        });
    }
    // Oberkörper (gemalt, `figart`): Schultern breit, vorn/hinten schmal, Nähte und Falten
    out.push(Body {
        center: [x, y],
        half: figart::half([5.3 * k, 6.1 * k]),
        angle: ta,
        shape: figart::shape(look.build),
        depth,
        color: top,
    });
    // Zubehör in der Hand
    let hand = tat(hands[1][0] * k, hands[1][1] * k);
    let mut push2 =
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
    match look.acc {
        Acc::Briefcase | Acc::Shopping => push2(
            [hand[0] + ts * 1.6, hand[1] - tc * 1.6],
            figart::half([2.6, 1.35]),
            ta,
            figart::shape(if look.acc == Acc::Shopping {
                Part::Shopping
            } else {
                Part::Briefcase
            }),
            0.00003,
            rgba(if look.acc == Acc::Shopping {
                0xd8c8a8
            } else {
                look.acc_color
            }),
        ),
        Acc::Cane => push2(at(5.2, 6.), [0.65, 0.65], 0., 1., 0.00003, rgba(0x4a3424)),
        Acc::Camera => push2(hand, [1.2, 1.8], ta, 0., -0.00003, rgba(0x1d1d1d)),
        Acc::Phone => push2(hand, [0.9, 1.3], ta, 0., -0.00003, rgba(0x15171b)),
        Acc::Bottle => push2(hand, [1.3, 0.7], ta, 0., -0.00003, rgba(0x2f6b3a)),
        _ => {}
    }
    // Kopf (gemalt: Gesicht, Ohren, Nase), darüber Frisur oder Kopfbedeckung – alle mit demselben Kopfraster
    let hh = figart::half([2.85 * k / figart::HEAD_R; 2]);
    push2(
        at(0.5, 0.),
        hh,
        a,
        figart::shape(Part::Head),
        -0.00012,
        skin,
    );
    if let Some(hat) = look.hat {
        push2(
            at(0.5, 0.),
            hh,
            a,
            figart::shape(look.hat_part),
            -0.00016,
            rgba(hat),
        );
    } else if let Some(hair) = look.hair {
        push2(
            at(0.5, 0.),
            hh,
            a,
            figart::shape(look.hair_part),
            -0.00016,
            rgba(hair),
        );
    }
}

/// Eine liegende Person (tot oder umgeworfen).
#[derive(Debug, Clone, Copy)]
pub struct Lying {
    pub x: f64,
    pub y: f64,
    /// Sturzrichtung (rad, Weltsystem): der Kopf liegt in dieser Richtung
    pub fall: f64,
    /// Zahl für die Haltung der Glieder (Nummer der Person)
    pub seed: f64,
    /// Sekunden seit dem Sturz (treibt das Hinfallen und die Blutlache)
    pub since: f64,
    pub skin: u32,
    /// tot: Blutlache, die langsam wächst
    pub blood: bool,
}

/// Wie lange das Hinfallen dauert (s)
pub const FALL_S: f32 = 0.4;
/// Endgröße der Blutlache (px, Halbachse quer) und Zeitkonstante, mit der sie wächst (s)
pub const POOL_R: f32 = 7.;
pub const POOL_TAU: f32 = 9.;

/// Wie weit die Person schon liegt: 0 steht noch … 1 liegt (Hinfallen mit kleinem Nachfedern).
pub fn fall_progress(since: f32) -> f32 {
    let u = (since / FALL_S).clamp(0., 1.);
    let e = 1. - (1. - u).powi(3);
    e + (u * std::f32::consts::PI).sin() * 0.06 * (1. - u)
}

/// Halbachse der Blutlache (px) nach `since` Sekunden; wächst schnell an und läuft dann langsam aus.
pub fn pool_radius(since: f32) -> f32 {
    let t = (since - FALL_S).max(0.);
    POOL_R * (1. - (-t / POOL_TAU).exp()).sqrt()
}

/// Liegende Person von oben: Rumpf, Hüfte, abgespreizte Beine mit Schuhen, Arme in einer von mehreren Haltungen
/// (am Körper, zur Seite, über dem Kopf, angewinkelt), Kopf mit Haaren oder Kopfbedeckung; die Tasche liegt daneben.
/// Die Haltung kommt aus der Nummer, nie aus dem Zufall der Welt. Beim Hinfallen streckt sich der Körper in
/// `FALL_S` vom Standpunkt aus; unter Toten wächst eine Blutlache.
pub fn lying_bodies(l: &Lying, look: &Look, depth: f32, out: &mut Vec<Body>) {
    let k = look.scale;
    let (x, y) = (l.x as f32, l.y as f32);
    let r = |n: f64| h01(l.seed, n) as f32;
    // leicht schräg zur Sturzrichtung, der Rumpf etwas verdreht
    let a = l.fall as f32 + (r(1.) - 0.5) * 0.5;
    let e = fall_progress(l.since as f32);
    let (c, s) = (a.cos(), a.sin());
    // Punkt im Körperrahmen: u zum Kopf, v quer; beim Hinfallen von der Mitte aus gestreckt
    let at = |u: f32, v: f32| {
        let (u, v) = (u * k * (0.25 + 0.75 * e), v * k * (0.7 + 0.3 * e));
        [x + c * u - s * v, y + s * u + c * v]
    };
    let push = |out: &mut Vec<Body>,
                center: [f32; 2],
                half: [f32; 2],
                angle: f32,
                shape: f32,
                d: f32,
                col: [f32; 4]| {
        out.push(Body {
            center,
            half,
            angle,
            shape,
            depth: depth + d,
            color: col,
        });
    };
    // Glied von p0 nach p1 (Körperrahmen) als Kapsel
    let limb = |out: &mut Vec<Body>, p0: [f32; 2], p1: [f32; 2], w: f32, d: f32, col: [f32; 4]| {
        let (a0, a1) = (at(p0[0], p0[1]), at(p1[0], p1[1]));
        let (dx, dy) = (a1[0] - a0[0], a1[1] - a0[1]);
        out.push(Body {
            center: [(a0[0] + a1[0]) / 2., (a0[1] + a1[1]) / 2.],
            half: [dx.hypot(dy) / 2. + w * 0.5, w],
            angle: dy.atan2(dx),
            shape: 0.,
            depth: depth + d,
            color: col,
        });
    };
    // Schatten und Blutlache (unter dem Oberkörper, zum Kopf hin)
    push(
        out,
        at(0., 0.),
        [12. * k * e.max(0.5), 6. * k],
        a,
        1.,
        0.0005,
        [0., 0., 0., 0.18],
    );
    if l.blood {
        // unregelmäßig aus mehreren Flecken, unter Kopf und Brust, dunkel und leicht glänzend
        let pr = pool_radius(l.since as f32) * k;
        if pr > 0.3 {
            for j in 0..4 {
                let jf = j as f64;
                let u = 4. + (r(30. + jf) - 0.3) * 7.;
                let v = (r(40. + jf) - 0.5) * 7.;
                let sz = 0.5 + r(50. + jf) * 0.45;
                let col = if j == 0 {
                    [0.3, 0.015, 0.02, 0.93]
                } else {
                    [0.36, 0.02, 0.03, 0.88]
                };
                push(
                    out,
                    at(u, v),
                    [pr * sz * 1.3, pr * sz],
                    a + r(60. + jf) * 3.,
                    1.,
                    0.00046 - j as f32 * 0.000002,
                    col,
                );
            }
        }
    }
    let top = rgba(look.top);
    let pants = rgba(look.pants);
    let skin = rgba(l.skin);
    // Beine: aus der Hüfte abgespreizt, eines manchmal im Knie angewinkelt
    for (i, side) in [-1f32, 1.].into_iter().enumerate() {
        let spread = 0.06 + r(10. + i as f64) * 0.38;
        let hip = [-3., side * 1.9];
        let dir = std::f32::consts::PI + side * spread;
        let bend = if r(12. + i as f64) < 0.35 {
            side * 0.9
        } else {
            0.
        };
        let knee = [hip[0] + dir.cos() * 4.6, hip[1] + dir.sin() * 4.6];
        let d2 = dir - bend;
        let foot = [knee[0] + d2.cos() * 4.4, knee[1] + d2.sin() * 4.4];
        limb(out, hip, knee, 1.45 * k, 0.00022, pants);
        limb(out, knee, foot, 1.3 * k, 0.00021, shade(pants, 0.92));
        let sh = [foot[0] + d2.cos() * 1.1, foot[1] + d2.sin() * 1.1];
        let fp = at(sh[0], sh[1]);
        push(
            out,
            fp,
            [1.9 * k, 1.15 * k],
            a + d2,
            0.,
            0.0002,
            rgba(look.shoes),
        );
    }
    // Arme: Haltung je Seite
    for (i, side) in [-1f32, 1.].into_iter().enumerate() {
        let sh = [5., side * 4.4];
        // Winkel des Oberarms gegen die Körperachse (0 = zum Kopf, π = zu den Füßen)
        let pick = r(20. + i as f64);
        let (up, fore) = if pick < 0.35 {
            (std::f32::consts::PI - 0.25, 0.15) // am Körper entlang
        } else if pick < 0.65 {
            (std::f32::consts::FRAC_PI_2 + 0.2, 0.5) // zur Seite
        } else if pick < 0.85 {
            (0.45, -0.3) // über den Kopf
        } else {
            (2.1, -1.6) // angewinkelt, Hand auf dem Bauch
        };
        let ua = side * up;
        let el = [sh[0] + ua.cos() * 4.2, sh[1] + ua.sin() * 4.2];
        let fa = ua + side * fore;
        let hd = [el[0] + fa.cos() * 3.8, el[1] + fa.sin() * 3.8];
        let (dd, df) = if pick >= 0.85 {
            (-0.00008, -0.00009)
        } else {
            (0.00012, 0.00011)
        };
        limb(out, sh, el, 1.25 * k, dd, shade(top, 0.9));
        limb(out, el, hd, 1.05 * k, df, shade(top, 0.84));
        push(
            out,
            at(hd[0], hd[1]),
            [1.05 * k, 1.05 * k],
            0.,
            1.,
            df - 0.00001,
            skin,
        );
    }
    // Hüfte (Hose) und Rumpf
    push(out, at(-2.2, 0.), [2.4 * k, 3.9 * k], a, 0., 0.00008, pants);
    push(
        out,
        at(1.6, 0.),
        [4.3 * k * (0.25 + 0.75 * e).max(0.5), 4.9 * k],
        a + (r(4.) - 0.5) * 0.25,
        0.,
        0.,
        top,
    );
    // Kopf etwas zur Seite gedreht, Haare zum Scheitel hin
    let ha = a + (r(5.) - 0.5) * 1.2;
    let hc = at(9.3, (r(6.) - 0.5) * 1.6);
    push(out, hc, [2.85 * k, 2.85 * k], 0., 1., -0.0001, skin);
    let crown = [hc[0] + ha.cos() * 0.9 * k, hc[1] + ha.sin() * 0.9 * k];
    if let Some(hat) = look.hat {
        push(out, crown, [2.5 * k, 2.6 * k], ha, 1., -0.00014, rgba(hat));
    } else if let Some(hair) = look.hair {
        push(out, crown, [2.1 * k, 2.6 * k], ha, 1., -0.00014, rgba(hair));
    }
    // Tasche ist heruntergefallen
    if let Some(b) = look.bag {
        let bp = at(-1. + r(7.) * 6., (r(8.) - 0.5) * 22.);
        push(
            out,
            bp,
            figart::half([2.1, 3.7 * k]),
            a + r(9.) * 3.,
            figart::shape(look.bag_part),
            0.00015,
            rgba(b),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dead_fall_down_and_bleed_out_slowly() {
        assert!(fall_progress(0.) < 0.05);
        assert!((fall_progress(FALL_S) - 1.).abs() < 1e-6);
        assert!(fall_progress(FALL_S * 0.5) > 0.6);
        // die Lache fängt erst nach dem Hinfallen an und wächst stetig bis fast zur Endgröße
        assert_eq!(pool_radius(0.2), 0.);
        let rs: Vec<f32> = [1., 3., 10., 30., 120.]
            .iter()
            .map(|&t| pool_radius(t))
            .collect();
        assert!(rs.windows(2).all(|w| w[1] > w[0]), "{rs:?}");
        assert!(rs[4] > POOL_R * 0.98 && rs[4] <= POOL_R);
    }

    #[test]
    fn a_lying_body_is_long_has_a_pool_only_when_dead_and_varies_by_person() {
        let p = test_ped();
        let look = look_of(&p);
        let mut l = Lying {
            x: 0.,
            y: 0.,
            fall: 0.,
            seed: 7.,
            since: 60.,
            skin: p.skin,
            blood: true,
        };
        let mut a = Vec::new();
        lying_bodies(&l, &look, 0.6, &mut a);
        // Ausdehnung längs (x) deutlich größer als quer: der Körper liegt ausgestreckt in Sturzrichtung
        let ext = |v: &[Body], f: fn(&Body) -> f32| {
            v.iter()
                .filter(|b| b.color[3] > 0.5 && b.color[0] < 0.3 || b.depth < 0.6004)
                .map(f)
                .fold((f32::MAX, f32::MIN), |(lo, hi), x| (lo.min(x), hi.max(x)))
        };
        let (x0, x1) = ext(&a, |b| b.center[0]);
        assert!(x1 - x0 > 18., "Länge {}", x1 - x0);
        // der Kopf (Hautfarbe, vorn) liegt in Sturzrichtung
        let skin = rgba(p.skin);
        let head = a
            .iter()
            .filter(|b| b.color == skin && b.half[0] > 2.5)
            .map(|b| b.center[0])
            .fold(f32::MIN, f32::max);
        assert!(head > 7., "Kopf bei {head}");
        let red = |v: &[Body]| {
            v.iter()
                .filter(|b| b.color[0] > 0.25 && b.color[1] < 0.05)
                .count()
        };
        assert_eq!(red(&a), 4);
        l.blood = false;
        let mut b = Vec::new();
        lying_bodies(&l, &look, 0.6, &mut b);
        assert_eq!(red(&b), 0);
        // andere Person, andere Haltung
        l.seed = 8.;
        let mut c = Vec::new();
        lying_bodies(&l, &look, 0.6, &mut c);
        assert!(
            b.iter()
                .zip(&c)
                .any(|(p, q)| (p.center[0] - q.center[0]).abs() > 0.5
                    || (p.center[1] - q.center[1]).abs() > 0.5)
        );
        // gerade gefallen: noch zusammengesunken (kürzer)
        l.since = 0.05;
        let mut d = Vec::new();
        lying_bodies(&l, &look, 0.6, &mut d);
        let (y0, y1) = ext(&d, |b| b.center[0]);
        assert!(y1 - y0 < (x1 - x0) * 0.6, "{} vs {}", y1 - y0, x1 - x0);
    }

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
        person_bodies(&Who::of(&p), &look_of(&p), 0.6, 0., &mut out);
        assert!(out.len() >= 8, "Person + Hund + Leine: {}", out.len());
        assert!(out.iter().all(|b| b.depth > 0.5 && b.center[0].is_finite()));
    }

    #[test]
    fn pose_follows_distance_and_state() {
        let mut who = Who::of(&test_ped());
        who.step = 7.; // Viertel eines Doppelschritts: Füße weit auseinander
        let (feet, hands, _) = pose(&who, Acc::None);
        assert!(feet[0] > 4. && (feet[0] + feet[1]).abs() < 1e-5, "{feet:?}");
        assert!(
            hands[0][0] < 0. && hands[1][0] > 0.,
            "Arme gegengleich zu den Beinen"
        );
        who.amp = 0.;
        let (still, _, _) = pose(&who, Acc::None);
        assert_eq!(still, [0., 0.], "im Stand nebeneinander");
        who.amp = 1.;
        who.run = 1.;
        who.step = 13.;
        let (run, _, _) = pose(&who, Acc::None);
        assert!(run[0] > feet[0], "rennend weiter");
        let (_, pushing, _) = pose(&who, Acc::Stroller);
        assert_eq!(pushing[0][0], 6.2, "beide Hände am Kinderwagen");
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
            hurt_t: f64::INFINITY,
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

    #[test]
    fn a_hit_jolts_the_body_away_from_the_shooter_and_settles() {
        // Hüllkurve: Ruck in 50 ms, danach zurück auf 0
        assert_eq!(hit_react(0.), 0.);
        assert!((hit_react(0.05) - 1.).abs() < 1e-5);
        assert!(hit_react(0.2) > 0. && hit_react(0.2) < 1.);
        assert_eq!(hit_react(HIT_REACT_S), 0.);
        assert_eq!(hit_react(-1.), 0.);
        // getroffen von links (Schütze bei −x): der Oberkörper (letzter großer Körper) rückt nach +x
        let mut p = test_ped();
        p.state = berlin_sim::pedestrians::PedState::Idle;
        let look = look_of(&p);
        let torso_x = |p: &Ped| {
            let mut out = Vec::new();
            person_bodies(&Who::of(p), &look, 0.6, 0., &mut out);
            // Oberkörper: der Körper mit der Kleidungsfarbe, mittig gelegen (größte Fläche)
            out.iter()
                .skip(1)
                .max_by(|a, b| (a.half[0] * a.half[1]).total_cmp(&(b.half[0] * b.half[1])))
                .unwrap()
                .center[0]
        };
        let calm = torso_x(&p);
        p.threat = (p.x - 100., p.y);
        p.hurt_t = 0.05;
        let hit = torso_x(&p);
        assert!(hit > calm + 1.5, "{calm} → {hit}");
        // tot oder lange her: keine Reaktion
        p.hurt_t = 5.;
        assert!((torso_x(&p) - calm).abs() < 1e-4);
    }

    #[test]
    fn gait_follows_the_walking_pace() {
        let (slow, _) = gait_of(0.7);
        let (norm, nb) = gait_of(1.);
        let (fast, fb) = gait_of(1.4);
        assert!(slow < norm && norm < fast, "{slow} {norm} {fast}");
        assert_eq!(nb, 0., "normales Gehen ohne Laufbewegung");
        assert!(
            fb > 0. && fb <= 0.4,
            "Eilige gehen zügig, rennen aber nicht"
        );
    }
}
