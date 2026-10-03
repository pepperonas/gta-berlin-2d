//! Berlin building palettes from buildcolors.js.
use crate::{
    citycodes::{
        building_kind as k, building_sub as sub, roof_mat as rm, unpack_look, wall_mat as wm,
    },
    format::Building,
    roofs::{Facade, Style},
};

const TILE: &[u32] = &[
    0x9c5a44, 0xa8664c, 0x8f5240, 0xb0725a, 0x8a4a3a, 0xa4553f, 0x7e4636, 0xb8694c,
];
const SLATE: &[u32] = &[0x4b4f55, 0x55595f, 0x44474d, 0x5c5f63, 0x3d4045];
const BROWN_TILE: &[u32] = &[0x6e5548, 0x5e4a40];
const FLAT: &[u32] = &[
    0x7d7b78, 0x86837e, 0x737271, 0x8e8a84, 0x6b6a6a, 0x9a958d, 0x807d77,
];
const METAL: &[u32] = &[0x8d959c, 0x7f8890, 0x9aa1a6];
const GREEN: &[u32] = &[0x6a8a4e, 0x5f7f45, 0x738f55];
const COPPER: &[u32] = &[0x5f8f7f, 0x6c9a86, 0x57857a];
const GLASS_ROOF: &[u32] = &[0x8fb0c4];
const STUCCO: &[u32] = &[
    0xd8c7a3, 0xe3d5b5, 0xcdb58e, 0xc9b9a4, 0xd9c2b0, 0xbfb6a4, 0xe6dcc6, 0xcfc0a0, 0xb9a78d,
    0xd7cbb0, 0xc7b49a, 0xdcc9a8, 0xa9b0a8, 0xc9bfae, 0xd3b9a0, 0xb8b3a6,
];
const PLATTE: &[u32] = &[
    0xe7d9a8, 0xd7b98c, 0xc9d6dc, 0xe3c3a4, 0xd9d3c6, 0xb9cfb4, 0xe6e1d3, 0xcfd9e4, 0xe8cfa0,
    0xd4c9e0,
];
const VILLA: &[u32] = &[
    0xece6da, 0xe5dcc8, 0xd9d2c4, 0xefe8d8, 0xe8dfcc, 0xa8674c, 0xdcd3c0,
];
const MODERN: &[u32] = &[0xd8d6d0, 0xc9c7c2, 0xe2e0da, 0xb8bcc0, 0xa9aeb3, 0xcfcac0];
const BRICK: &[u32] = &[0x9b5b43, 0xa8674c, 0x8c4f3b, 0xb07156, 0x7f4a3a];
const CONCRETE: &[u32] = &[0xb5b3ad, 0xa9a8a3, 0xc2bfb7];
const GLASS_WALL: &[u32] = &[0x8fa4b4, 0x7e95a6, 0x9db1bf];
const WOOD: &[u32] = &[0x9a7a5a, 0x8b6c4e];
const STONE: &[u32] = &[0xc8bca4, 0xbfb39a];
const METAL_WALL: &[u32] = &[0x9aa3aa];
fn pick(p: &[u32], n: u32) -> u32 {
    p[n as usize % p.len()]
}
pub fn rgb(c: u32) -> [f32; 3] {
    [
        (c >> 16 & 255) as f32 / 255.,
        (c >> 8 & 255) as f32 / 255.,
        (c & 255) as f32 / 255.,
    ]
}
fn mix(a: u32, b: u32, t: f32) -> u32 {
    let channel = |sh: u32| {
        ((((a >> sh) & 255) as f32 * (1. - t) + ((b >> sh) & 255) as f32 * t).round() as u32) << sh
    };
    channel(16) | channel(8) | channel(0)
}
pub fn wall_color(b: &Building, facade: Facade) -> u32 {
    let lk = unpack_look(b.look);
    let s = b.seed;
    if let Some(c) = b.wall_rgb {
        return c;
    }
    match lk.wmat {
        wm::BRICK => return pick(BRICK, s),
        wm::CONCRETE => {
            return pick(
                if facade == Facade::Platte {
                    PLATTE
                } else {
                    CONCRETE
                },
                s,
            );
        }
        wm::GLASS => return pick(GLASS_WALL, s),
        wm::WOOD => return pick(WOOD, s),
        wm::STONE => return pick(STONE, s),
        wm::METAL => return pick(METAL_WALL, s),
        _ => {}
    }
    if b.kind != k::HOUSE {
        return pick(
            match b.kind {
                k::PUBLIC => &[0xa3b1a0, 0x9aa3ab, 0xb0aaa0, 0xc2b8a6, 0xc9c0ae, 0xb7a38c],
                k::INDUSTRIAL => &[0x8f9aa6, 0x9aa0a3, 0x858d93, 0xa3a8a0, 0x9a6a55],
                k::CHURCH => &[0xa0674e, 0x8f5a45, 0xb8ab94],
                k::SMALL => &[0x9d968c, 0x8c877f, 0xa79f92],
                k::SPAETI => &[0xd9c46a],
                k::WAREHOUSE => &[0x7f8a93],
                _ => STUCCO,
            },
            s,
        );
    }
    if facade == Facade::Platte {
        return pick(PLATTE, s);
    }
    if facade == Facade::Modern {
        return pick(MODERN, s);
    }
    if lk.sub == sub::VILLA || lk.sub == sub::TERRACE || b.meters <= 9. {
        return pick(VILLA, s);
    }
    pick(STUCCO, s)
}
pub fn roof_colors(b: &Building, style: Style, wall: u32) -> (u32, u32) {
    let lk = unpack_look(b.look);
    let s = b.seed >> 3;
    let r = (s % 997) as f32 / 997.;
    let flat = mix(pick(FLAT, s >> 4), wall, 0.12);
    let banded = style == Style::Berlin || style == Style::Mansard;
    if let Some(c) = b.roof_rgb {
        return (c, if banded { flat } else { c });
    }
    let skin = match lk.rmat {
        rm::TILES => pick(TILE, s),
        rm::CONCRETE => pick(BROWN_TILE, s),
        rm::TAR => pick(FLAT, s),
        rm::METAL => pick(METAL, s),
        rm::GLASS => pick(GLASS_ROOF, s),
        rm::SLATE => pick(SLATE, s),
        rm::GREEN => pick(GREEN, s),
        _ => {
            if b.kind == k::SPAETI {
                mix(wall, 0x6f6a62, 0.4)
            } else if style == Style::Corrugated {
                if b.kind == k::WAREHOUSE {
                    0x7f8a93
                } else if r < 0.2 {
                    0x8a5a4a
                } else {
                    pick(METAL, s)
                }
            } else if style == Style::Dome {
                if r < 0.7 {
                    pick(COPPER, s)
                } else {
                    pick(SLATE, s)
                }
            } else if b.kind == k::CHURCH {
                if r < 0.55 {
                    pick(SLATE, s)
                } else if r < 0.8 {
                    pick(COPPER, s)
                } else {
                    pick(TILE, s)
                }
            } else if style == Style::Mansard {
                if r < 0.75 {
                    pick(SLATE, s)
                } else {
                    pick(TILE, s)
                }
            } else if style == Style::Round {
                pick(METAL, s)
            } else if style == Style::Berlin {
                if r < 0.55 {
                    pick(TILE, s)
                } else if r < 0.85 {
                    pick(SLATE, s)
                } else {
                    pick(BROWN_TILE, s)
                }
            } else if style.pitched() {
                if b.kind == k::SMALL {
                    mix(wall, 0x6f6a62, 0.5)
                } else if r < 0.56 {
                    pick(TILE, s)
                } else if r < 0.86 {
                    pick(SLATE, s)
                } else {
                    pick(BROWN_TILE, s)
                }
            } else {
                let bounds = crate::geom::Bounds::of(&b.polygon.rings[0]);
                let size = bounds.max - bounds.min;
                let big = size.x * size.y > 60000.;
                if big && r < 0.07 {
                    pick(GREEN, s)
                } else if big && b.kind == k::PUBLIC && r < 0.15 {
                    pick(METAL, s)
                } else {
                    flat
                }
            }
        }
    };
    (skin, if banded { flat } else { skin })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn color_conversion() {
        assert_eq!(rgb(0xff0000), [1., 0., 0.]);
        assert_eq!(mix(0, 0xffffff, 0.5), 0x808080);
    }
}
