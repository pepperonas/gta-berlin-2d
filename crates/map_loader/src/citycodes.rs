//! Wire-format codes, matching web/src/citycodes.js.

pub mod area_kind {
    pub const RAIL: u8 = 0;
    pub const PLAZA: u8 = 1;
    pub const ALLOTMENTS: u8 = 2;
    pub const CEMETERY: u8 = 3;
    pub const GRASS: u8 = 4;
    pub const PITCH: u8 = 5;
    pub const SAND: u8 = 6;
    pub const WOOD: u8 = 7;
    pub const BRIDGE: u8 = 8;
}

pub mod wall_kind {
    pub const OTHER: u8 = 0;
    pub const BORDER: u8 = 1;
    pub const QUAY: u8 = 2;
    pub const RAIL: u8 = 3;
    pub const RAILING: u8 = 4;
    pub const FENCE: u8 = 5;
}

pub mod furn_kind {
    pub const BENCH: u8 = 0;
    pub const BICYCLE: u8 = 1;
    pub const BIN: u8 = 2;
}

pub mod building_kind {
    pub const HOUSE: u8 = 0;
    pub const PUBLIC: u8 = 1;
    pub const INDUSTRIAL: u8 = 2;
    pub const CHURCH: u8 = 3;
    pub const SMALL: u8 = 4;
    pub const SPAETI: u8 = 5;
    pub const WAREHOUSE: u8 = 6;
}

pub mod park {
    pub const NONE: u8 = 0;
    pub const LANE: u8 = 1;
    pub const HALF: u8 = 2;
    pub const KERB: u8 = 3;
}

pub mod surface {
    pub const ASPHALT: u8 = 0;
    pub const COBBLE: u8 = 1;
    pub const PLATES: u8 = 2;
    pub const UNPAVED: u8 = 3;
}

pub mod roof_shape {
    pub const NONE: u8 = 0;
    pub const FLAT: u8 = 1;
    pub const GABLED: u8 = 2;
    pub const HIPPED: u8 = 3;
    pub const PYRAMIDAL: u8 = 4;
    pub const MANSARD: u8 = 5;
    pub const SKILLION: u8 = 6;
    pub const DOME: u8 = 7;
    pub const ROUND: u8 = 8;
}

pub mod roof_mat {
    pub const NONE: u8 = 0;
    pub const TILES: u8 = 1;
    pub const CONCRETE: u8 = 2;
    pub const TAR: u8 = 3;
    pub const METAL: u8 = 4;
    pub const GLASS: u8 = 5;
    pub const SLATE: u8 = 6;
    pub const GREEN: u8 = 7;
}

pub mod wall_mat {
    pub const NONE: u8 = 0;
    pub const PLASTER: u8 = 1;
    pub const BRICK: u8 = 2;
    pub const CONCRETE: u8 = 3;
    pub const GLASS: u8 = 4;
    pub const WOOD: u8 = 5;
    pub const STONE: u8 = 6;
    pub const METAL: u8 = 7;
}

pub mod building_sub {
    pub const NONE: u8 = 0;
    pub const VILLA: u8 = 1;
    pub const TERRACE: u8 = 2;
    pub const APARTMENTS: u8 = 3;
    pub const COMMERCIAL: u8 = 4;
    pub const CIVIC: u8 = 5;
    pub const GARAGE: u8 = 6;
}

pub const ROAD_CLASSES: &[&str] = &[
    "",
    "motorway",
    "trunk",
    "primary",
    "secondary",
    "tertiary",
    "unclassified",
    "residential",
    "living_street",
    "service",
    "pedestrian",
    "track",
    "busway",
];

pub const DTV_ESTIMATE: &[u32] = &[
    0, 45000, 35000, 22000, 14000, 7000, 2500, 900, 200, 150, 0, 0, 300,
];

pub const POI_CATS: &[&str] = &[
    "ubahn",
    "sbahn",
    "bahn",
    "bus",
    "mall",
    "supermarket",
    "shop",
    "food",
    "drink",
    "cafe",
    "service",
    "culture",
    "hotel",
];

pub const PARK_ORIENT: &[&str] = &["parallel", "diagonal", "perpendicular"];

pub const TREE_GENERA: &[&str] = &[
    "sonstige",
    "Tilia",
    "Acer",
    "Platanus",
    "Aesculus",
    "Quercus",
    "Robinia",
    "Betula",
    "Populus",
    "Carpinus",
    "Fraxinus",
    "Prunus",
    "Salix",
    "Sorbus",
    "Crataegus",
    "Ulmus",
    "Nadel",
];

pub const BEZIRKE: &[&str] = &[
    "Charlottenburg-Wilmersdorf",
    "Friedrichshain-Kreuzberg",
    "Lichtenberg",
    "Marzahn-Hellersdorf",
    "Mitte",
    "Neukölln",
    "Pankow",
    "Reinickendorf",
    "Spandau",
    "Steglitz-Zehlendorf",
    "Tempelhof-Schöneberg",
    "Treptow-Köpenick",
];

pub const TRAFFIC_MAX_CLASS: u8 = 8;
pub const DENS_CELL_M: f32 = 64.0;
pub const TREE_TRUNK_M: f32 = 0.5;
pub const TREE_FREE_MAX_CLASS: u8 = 9;
pub const LVL_MIN: i8 = -2;
pub const LVL_MAX: i8 = 3;
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Look {
    pub shape: u8,
    pub rmat: u8,
    pub wmat: u8,
    pub sub: u8,
    pub bez: u8,
}
pub fn unpack_look(v: u32) -> Look {
    Look {
        shape: (v & 15) as u8,
        rmat: ((v >> 4) & 7) as u8,
        wmat: ((v >> 7) & 7) as u8,
        sub: ((v >> 10) & 7) as u8,
        bez: ((v >> 13) & 15) as u8,
    }
}
pub fn pack_look(v: Look) -> u32 {
    v.shape as u32
        | (v.rmat as u32) << 4
        | (v.wmat as u32) << 7
        | (v.sub as u32) << 10
        | (v.bez as u32) << 13
}
pub fn pack_lvl(v: i8) -> u8 {
    v as u8 & 7
}
pub fn unpack_lvl(v: u32) -> i8 {
    ((v as i8 & 7) << 5) >> 5
}
pub fn hash01(n: u32) -> f64 {
    let mut t = n.wrapping_add(0x6d2b79f5);
    t = (t ^ (t >> 15)).wrapping_mul(t | 1);
    t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
    (t ^ (t >> 14)) as f64 / 4294967296.0
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bit_fields_round_trip() {
        let look = Look {
            shape: 8,
            rmat: 7,
            wmat: 6,
            sub: 5,
            bez: 12,
        };
        assert_eq!(unpack_look(pack_look(look)), look);
        for level in -4..=3 {
            assert_eq!(unpack_lvl(pack_lvl(level) as u32), level);
        }
    }
}
