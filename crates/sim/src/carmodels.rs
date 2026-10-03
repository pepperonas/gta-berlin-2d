//! Pkw-Modelle und ihre Technik (Port von `carmodels.js` und `fleet.js`): Antrieb, Motorlage, Masse,
//! Gewichtsverteilung, Schwerpunkt, Radstand, Spur, Leistung, Höchsttempo, Reifenhaftung; dazu die Fahrzeugarten.
//! Modellnamen sind erfunden (Berliner Orte), keine echten Marken.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Drive {
    Fwd,
    Rwd,
    Awd,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    Front,
    Mid,
    Rear,
    Floor,
}
/// Technische Daten (SI): mass kg, front = Gewichtsanteil vorn, h = Schwerpunkthöhe m, wb = Radstand m, track = Spur m,
/// kw, vmax km/h, mu = Haftung trocken, yaw = Faktor der Gierträgheit, v_low = Drehmomentgrenze bis (m/s),
/// bias = Bremskraftanteil vorn, brake_k = Bremsanlage relativ zu heute.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spec {
    pub label: &'static str,
    pub drive: Drive,
    pub engine: Engine,
    pub mass: f64,
    pub front: f64,
    pub h: f64,
    pub wb: f64,
    pub track: f64,
    pub kw: f64,
    pub vmax: f64,
    pub mu: f64,
    pub yaw: f64,
    pub v_low: f64,
    pub bias: f64,
    pub awd_front: f64,
    pub steer_max: f64,
    pub brake_k: f64,
    pub two_wheel: bool,
    pub electric: bool,
    pub diesel: bool,
}
const fn s(label: &'static str, drive: Drive, engine: Engine, v: [f64; 8]) -> Spec {
    Spec {
        label,
        drive,
        engine,
        mass: v[0],
        front: v[1],
        h: v[2],
        wb: v[3],
        track: v[4],
        kw: v[5],
        vmax: v[6],
        mu: v[7],
        yaw: 1.,
        v_low: 7.,
        bias: 0.68,
        awd_front: 0.5,
        steer_max: 0.62,
        brake_k: 1.,
        two_wheel: false,
        electric: false,
        diesel: false,
    }
}

pub const SPECS: &[(&str, Spec)] = &[
    (
        "zweitakter",
        Spec {
            yaw: 1.05,
            v_low: 5.,
            ..s(
                "Zweitakter",
                Drive::Fwd,
                Engine::Front,
                [650., 0.6, 0.56, 2.02, 1.2, 19., 107., 0.82],
            )
        },
    ),
    (
        "kleinwagen",
        s(
            "Kleinwagen",
            Drive::Fwd,
            Engine::Front,
            [1100., 0.63, 0.54, 2.47, 1.45, 60., 170., 0.95],
        ),
    ),
    (
        "kompakt",
        s(
            "Kompaktwagen",
            Drive::Fwd,
            Engine::Front,
            [1350., 0.61, 0.55, 2.63, 1.55, 110., 210., 1.],
        ),
    ),
    (
        "limousine",
        s(
            "Limousine",
            Drive::Rwd,
            Engine::Front,
            [1600., 0.52, 0.53, 2.85, 1.58, 180., 240., 1.02],
        ),
    ),
    (
        "taxi",
        Spec {
            diesel: true,
            ..s(
                "Taxi",
                Drive::Rwd,
                Engine::Front,
                [1750., 0.53, 0.56, 2.94, 1.58, 120., 210., 0.98],
            )
        },
    ),
    (
        "kombi",
        Spec {
            awd_front: 0.4,
            ..s(
                "Kombi",
                Drive::Awd,
                Engine::Front,
                [1750., 0.58, 0.57, 2.82, 1.58, 170., 230., 1.],
            )
        },
    ),
    (
        "transporter",
        Spec {
            yaw: 1.1,
            diesel: true,
            ..s(
                "Transporter",
                Drive::Fwd,
                Engine::Front,
                [2100., 0.6, 0.85, 3.0, 1.7, 100., 160., 0.9],
            )
        },
    ),
    (
        "elektro",
        Spec {
            yaw: 0.95,
            v_low: 3.5,
            awd_front: 0.45,
            electric: true,
            ..s(
                "Elektro-SUV",
                Drive::Awd,
                Engine::Floor,
                [2250., 0.49, 0.55, 2.9, 1.65, 300., 220., 1.],
            )
        },
    ),
    (
        "gelaende",
        Spec {
            yaw: 1.05,
            diesel: true,
            ..s(
                "Geländewagen",
                Drive::Awd,
                Engine::Front,
                [2300., 0.52, 0.85, 2.85, 1.65, 190., 190., 0.92],
            )
        },
    ),
    (
        "sportwagen",
        Spec {
            yaw: 0.78,
            bias: 0.6,
            ..s(
                "Sportwagen",
                Drive::Rwd,
                Engine::Mid,
                [1400., 0.42, 0.45, 2.6, 1.6, 320., 300., 1.15],
            )
        },
    ),
    (
        "heckcoupe",
        Spec {
            yaw: 1.18,
            bias: 0.58,
            ..s(
                "Heckmotor-Coupé",
                Drive::Rwd,
                Engine::Rear,
                [1450., 0.38, 0.48, 2.45, 1.55, 290., 290., 1.1],
            )
        },
    ),
    (
        "hothatch",
        Spec {
            yaw: 0.95,
            ..s(
                "Hot Hatch",
                Drive::Fwd,
                Engine::Front,
                [1380., 0.62, 0.5, 2.62, 1.56, 221., 250., 1.08],
            )
        },
    ),
    (
        "roadster",
        Spec {
            yaw: 0.85,
            ..s(
                "Roadster",
                Drive::Rwd,
                Engine::Front,
                [1050., 0.5, 0.45, 2.31, 1.5, 97., 205., 1.05],
            )
        },
    ),
    (
        "musclecar",
        Spec {
            yaw: 1.1,
            v_low: 8.,
            brake_k: 0.85,
            ..s(
                "Muscle-Car",
                Drive::Rwd,
                Engine::Front,
                [1650., 0.56, 0.52, 2.74, 1.52, 330., 250., 0.95],
            )
        },
    ),
    (
        "oldtimer",
        Spec {
            yaw: 1.1,
            brake_k: 0.7,
            steer_max: 0.58,
            ..s(
                "Oldtimer",
                Drive::Rwd,
                Engine::Front,
                [1300., 0.53, 0.58, 2.75, 1.45, 60., 150., 0.75],
            )
        },
    ),
    (
        "pickup",
        Spec {
            yaw: 1.1,
            diesel: true,
            ..s(
                "Pick-up",
                Drive::Rwd,
                Engine::Front,
                [2150., 0.6, 0.8, 3.1, 1.65, 150., 180., 0.9],
            )
        },
    ),
    (
        "kleinbus",
        Spec {
            yaw: 1.15,
            brake_k: 0.8,
            ..s(
                "Kleinbus",
                Drive::Rwd,
                Engine::Rear,
                [1500., 0.44, 0.9, 2.4, 1.4, 55., 115., 0.85],
            )
        },
    ),
    (
        "rallye",
        Spec {
            yaw: 0.9,
            awd_front: 0.35,
            ..s(
                "Rallye-Kompakt",
                Drive::Awd,
                Engine::Front,
                [1280., 0.58, 0.5, 2.52, 1.55, 190., 230., 1.08],
            )
        },
    ),
    (
        "supersport",
        Spec {
            yaw: 0.78,
            bias: 0.6,
            awd_front: 0.3,
            ..s(
                "Supersportwagen",
                Drive::Awd,
                Engine::Mid,
                [1422., 0.43, 0.44, 2.62, 1.67, 470., 325., 1.2],
            )
        },
    ),
    (
        "gtcoupe",
        Spec {
            yaw: 0.9,
            bias: 0.62,
            ..s(
                "GT-Coupé",
                Drive::Rwd,
                Engine::Front,
                [1615., 0.47, 0.46, 2.63, 1.63, 390., 318., 1.15],
            )
        },
    ),
    (
        "leichtbau",
        Spec {
            yaw: 0.75,
            bias: 0.62,
            ..s(
                "Mittelmotor-Leichtbau",
                Drive::Rwd,
                Engine::Mid,
                [1100., 0.44, 0.44, 2.42, 1.55, 185., 250., 1.1],
            )
        },
    ),
    (
        "elektrosport",
        Spec {
            yaw: 0.9,
            v_low: 3.5,
            awd_front: 0.4,
            electric: true,
            bias: 0.62,
            ..s(
                "Elektro-Sportlimousine",
                Drive::Awd,
                Engine::Floor,
                [2300., 0.48, 0.45, 2.9, 1.66, 560., 260., 1.12],
            )
        },
    ),
    (
        "sprinter",
        Spec {
            yaw: 1.15,
            steer_max: 0.56,
            diesel: true,
            ..s(
                "Großraumtransporter",
                Drive::Rwd,
                Engine::Front,
                [2700., 0.52, 1.05, 3.66, 1.72, 120., 160., 0.88],
            )
        },
    ),
    (
        "hochdach",
        Spec {
            yaw: 1.05,
            diesel: true,
            ..s(
                "Hochdachkombi",
                Drive::Fwd,
                Engine::Front,
                [1650., 0.6, 0.75, 2.75, 1.56, 90., 180., 0.92],
            )
        },
    ),
    (
        "powerkombi",
        Spec {
            awd_front: 0.4,
            yaw: 1.0,
            ..s(
                "Power-Kombi",
                Drive::Awd,
                Engine::Front,
                [2075., 0.57, 0.52, 2.93, 1.67, 441., 280., 1.12],
            )
        },
    ),
    (
        "familienkombi",
        Spec {
            diesel: true,
            ..s(
                "Familienkombi",
                Drive::Fwd,
                Engine::Front,
                [1650., 0.6, 0.55, 2.79, 1.58, 147., 225., 1.0],
            )
        },
    ),
    (
        "business",
        s(
            "Businesslimousine",
            Drive::Rwd,
            Engine::Front,
            [1800., 0.51, 0.52, 2.98, 1.6, 250., 250., 1.04],
        ),
    ),
    (
        "sportlimo",
        Spec {
            yaw: 0.95,
            v_low: 8.,
            ..s(
                "Sportlimousine",
                Drive::Rwd,
                Engine::Front,
                [1730., 0.52, 0.5, 2.86, 1.62, 375., 290., 1.12],
            )
        },
    ),
    (
        "luxus",
        Spec {
            yaw: 1.1,
            ..s(
                "Luxuslimousine",
                Drive::Rwd,
                Engine::Front,
                [2100., 0.52, 0.54, 3.1, 1.65, 320., 250., 1.02],
            )
        },
    ),
    (
        "coupe",
        Spec {
            yaw: 0.92,
            ..s(
                "Sportcoupé",
                Drive::Rwd,
                Engine::Front,
                [1700., 0.53, 0.48, 2.75, 1.6, 338., 285., 1.1],
            )
        },
    ),
    (
        "leichtcoupe",
        Spec {
            yaw: 0.88,
            ..s(
                "Leichtes Coupé",
                Drive::Rwd,
                Engine::Front,
                [1275., 0.53, 0.46, 2.575, 1.54, 172., 226., 1.0],
            )
        },
    ),
    (
        "gklasse",
        Spec {
            yaw: 1.1,
            steer_max: 0.58,
            ..s(
                "Geländewagen (Kastenform)",
                Drive::Awd,
                Engine::Front,
                [2560., 0.5, 0.95, 2.89, 1.64, 310., 210., 0.92],
            )
        },
    ),
    (
        "defender",
        Spec {
            yaw: 1.1,
            diesel: true,
            ..s(
                "Expeditions-Geländewagen",
                Drive::Awd,
                Engine::Front,
                [2350., 0.5, 0.88, 3.02, 1.7, 221., 191., 0.9],
            )
        },
    ),
    (
        "niva",
        Spec {
            yaw: 1.05,
            brake_k: 0.85,
            steer_max: 0.6,
            ..s(
                "Kleiner Geländewagen",
                Drive::Awd,
                Engine::Front,
                [1285., 0.55, 0.75, 2.2, 1.43, 61., 142., 0.85],
            )
        },
    ),
    (
        "kompaktsuv",
        s(
            "Kompakt-SUV",
            Drive::Fwd,
            Engine::Front,
            [1600., 0.6, 0.66, 2.68, 1.58, 110., 200., 0.98],
        ),
    ),
    (
        "sportsuv",
        Spec {
            awd_front: 0.4,
            ..s(
                "Sport-SUV",
                Drive::Awd,
                Engine::Front,
                [2050., 0.53, 0.66, 2.9, 1.66, 250., 245., 1.05],
            )
        },
    ),
    (
        "grosssuv",
        Spec {
            awd_front: 0.4,
            yaw: 1.05,
            ..s(
                "Großes SUV",
                Drive::Awd,
                Engine::Front,
                [2200., 0.52, 0.7, 2.98, 1.68, 250., 243., 1.02],
            )
        },
    ),
    (
        "motorcycle",
        Spec {
            two_wheel: true,
            yaw: 1.,
            bias: 0.75,
            steer_max: 0.5,
            v_low: 6.,
            ..s(
                "Motorrad",
                Drive::Rwd,
                Engine::Mid,
                [280., 0.5, 0.66, 1.45, 1., 110., 240., 1.1],
            )
        },
    ),
    (
        "scooter",
        Spec {
            two_wheel: true,
            yaw: 1.,
            bias: 0.6,
            steer_max: 0.55,
            v_low: 5.,
            ..s(
                "Motorroller",
                Drive::Rwd,
                Engine::Rear,
                [215., 0.42, 0.6, 1.35, 1., 11., 95., 1.],
            )
        },
    ),
    (
        "police",
        s(
            "Streifenwagen",
            Drive::Rwd,
            Engine::Front,
            [1750., 0.53, 0.56, 2.94, 1.58, 190., 240., 1.02],
        ),
    ),
    (
        "ambulance",
        Spec {
            yaw: 1.15,
            steer_max: 0.55,
            diesel: true,
            ..s(
                "Rettungswagen",
                Drive::Rwd,
                Engine::Front,
                [4200., 0.45, 1.15, 3.66, 1.75, 140., 140., 0.85],
            )
        },
    ),
    (
        "delivery",
        Spec {
            yaw: 1.1,
            steer_max: 0.58,
            diesel: true,
            ..s(
                "Paketwagen",
                Drive::Fwd,
                Engine::Front,
                [2900., 0.55, 1.0, 3.3, 1.7, 105., 145., 0.88],
            )
        },
    ),
    (
        "truck",
        Spec {
            yaw: 1.2,
            v_low: 4.,
            steer_max: 0.55,
            diesel: true,
            ..s(
                "Lkw 7,5 t",
                Drive::Rwd,
                Engine::Front,
                [7500., 0.45, 1.4, 4.2, 1.95, 150., 100., 0.8],
            )
        },
    ),
    (
        "garbage",
        Spec {
            yaw: 1.2,
            v_low: 3.,
            steer_max: 0.55,
            diesel: true,
            ..s(
                "Müllwagen",
                Drive::Rwd,
                Engine::Front,
                [18000., 0.38, 1.6, 4.6, 2.05, 240., 85., 0.75],
            )
        },
    ),
    (
        "bus",
        Spec {
            yaw: 1.25,
            v_low: 3.,
            steer_max: 0.6,
            diesel: true,
            ..s(
                "Linienbus",
                Drive::Rwd,
                Engine::Rear,
                [12500., 0.35, 1.25, 6.0, 2.1, 220., 90., 0.78],
            )
        },
    ),
];
/// Pkw-Modelle im Verkehr mit ihrem Anteil (Summe 1).
pub const CAR_MODELS: &[(&str, f64)] = &[
    ("kleinwagen", 0.11),
    ("kompakt", 0.09),
    ("limousine", 0.1),
    ("kombi", 0.071),
    ("transporter", 0.06),
    ("taxi", 0.07),
    ("elektro", 0.05),
    ("gelaende", 0.025),
    ("sportwagen", 0.01),
    ("heckcoupe", 0.01),
    ("zweitakter", 0.01),
    ("hothatch", 0.015),
    ("roadster", 0.015),
    ("musclecar", 0.01),
    ("oldtimer", 0.01),
    ("pickup", 0.01),
    ("kleinbus", 0.01),
    ("rallye", 0.005),
    ("supersport", 0.004),
    ("gtcoupe", 0.006),
    ("leichtbau", 0.006),
    ("elektrosport", 0.008),
    ("sprinter", 0.03),
    ("hochdach", 0.03),
    ("powerkombi", 0.008),
    ("familienkombi", 0.04),
    ("business", 0.04),
    ("sportlimo", 0.01),
    ("luxus", 0.015),
    ("coupe", 0.01),
    ("leichtcoupe", 0.008),
    ("gklasse", 0.008),
    ("defender", 0.008),
    ("niva", 0.006),
    ("kompaktsuv", 0.05),
    ("sportsuv", 0.012),
    ("grosssuv", 0.02),
];
/// Hersteller und Typ (erfunden).
pub const NAMES: &[(&str, &str, &str)] = &[
    ("kleinwagen", "Havel", "Piccolo"),
    ("kompakt", "Spree", "Ronda"),
    ("limousine", "Teltower", "T6"),
    ("taxi", "Teltower", "T6 Droschke"),
    ("kombi", "Märker", "Allwetter"),
    ("transporter", "Spree", "Kasten"),
    ("elektro", "Voltwerk", "E-Terra"),
    ("gelaende", "Grunewald", "Keiler"),
    ("sportwagen", "Oberbaum", "Furia"),
    ("heckcoupe", "Wannsee", "Boxer 6"),
    ("zweitakter", "Lausitz", "Kolibri"),
    ("hothatch", "Spree", "Ronda Sport"),
    ("roadster", "Köpenick", "Spyder"),
    ("musclecar", "Tempelhof", "Stier V8"),
    ("oldtimer", "Adlershof", "Kanzler"),
    ("pickup", "Grunewald", "Kipper"),
    ("kleinbus", "Havel", "Kiezbus"),
    ("rallye", "Märker", "Schotter"),
    ("police", "Teltower", "T6 Streife"),
    ("ambulance", "Spree", "Kasten RTW"),
    ("delivery", "Spree", "Kasten Paket"),
    ("truck", "Oberlausitz", "L75"),
    ("garbage", "Kiezwerk", "Presse 26"),
    ("bus", "Kiezwerk", "Stadtbus 12"),
    ("motorcycle", "Tegel", "Blitz 1000"),
    ("scooter", "Kiezflitzer", "125"),
    ("supersport", "Oberbaum", "Tempesta"),
    ("gtcoupe", "Tempelhof", "GT 40"),
    ("leichtbau", "Adlershof", "Flèche"),
    ("elektrosport", "Voltwerk", "Blitz GT"),
    ("sprinter", "Spree", "Großraum 316"),
    ("hochdach", "Havel", "Kasten Hoch"),
    ("powerkombi", "Märker", "RS Avant"),
    ("familienkombi", "Teltower", "T4 Tourer"),
    ("business", "Teltower", "T5"),
    ("sportlimo", "Teltower", "T3 RS"),
    ("luxus", "Grunewald", "Senator"),
    ("coupe", "Wannsee", "Coupé 240"),
    ("leichtcoupe", "Lausitz", "Hachi"),
    ("gklasse", "Grunewald", "Kommandant"),
    ("defender", "Grunewald", "Förster 110"),
    ("niva", "Lausitz", "Taiga"),
    ("kompaktsuv", "Spree", "Tiga"),
    ("sportsuv", "Oberbaum", "Cayo"),
    ("grosssuv", "Teltower", "TX7"),
];

/// Fahrzeugart (fleet.js `KINDS`): Maße L × W in px, Motorleistung, eigenes Höchsttempo (Räder), Anzug.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Kind {
    pub name: &'static str,
    pub l: f64,
    pub w: f64,
    pub power: f64,
    pub top: Option<f64>,
    pub accel: f64,
    pub bike: bool,
    pub moto: bool,
    pub colors: &'static [u32],
}
const fn k(name: &'static str, l: f64, w: f64, power: f64, colors: &'static [u32]) -> Kind {
    Kind {
        name,
        l,
        w,
        power,
        top: None,
        accel: 1.,
        bike: false,
        moto: false,
        colors,
    }
}
pub const CAR_COLORS: &[u32] = &[
    0xc0392b, 0x2e86de, 0xf1c40f, 0x27ae60, 0xecf0f1, 0x8e44ad, 0x34495e, 0xe67e22, 0x16a085,
    0x7f8c8d,
];
pub const KINDS: &[Kind] = &[
    k("car", 42., 20., 1., CAR_COLORS),
    k(
        "truck",
        76.,
        24.,
        0.7,
        &[0xe9e6df, 0x35495e, 0xa93226, 0x1e6f5c],
    ),
    k(
        "delivery",
        52.,
        21.,
        0.85,
        &[0xffcc00, 0x5b3a1e, 0xf4f4f2, 0x1d3a8a],
    ),
    k("garbage", 86., 25., 0.6, &[0xf07d00]),
    k("police", 48., 20., 1.1, &[0xe8ecef]),
    k("ambulance", 60., 22., 1., &[0xf5f5f2]),
    k("bus", 120., 25., 0.75, &[0xf0cf1f]),
    Kind {
        top: Some(72.),
        accel: 0.55,
        bike: true,
        ..k("bicycle", 18., 7., 0.45, CAR_COLORS)
    },
    Kind {
        top: Some(56.),
        accel: 0.5,
        bike: true,
        ..k("escooter", 14., 6., 0.4, CAR_COLORS)
    },
    Kind {
        moto: true,
        ..k(
            "motorcycle",
            22.,
            8.,
            1.15,
            &[0xb3261e, 0x1d1f24, 0xe8e6e1, 0x2d5da8, 0xf0a202],
        )
    },
    Kind {
        moto: true,
        ..k(
            "scooter",
            18.,
            7.,
            0.6,
            &[0x8fc1b5, 0xe9e4d6, 0xc0392b, 0x3d3f45, 0xf2c14e],
        )
    },
];
pub fn kind(name: &str) -> &'static Kind {
    KINDS.iter().find(|k| k.name == name).unwrap_or(&KINDS[0])
}
/// Offenes Zweirad (Rad, E-Roller, Motorrad, Roller): keine Kisten, kein Missionsauto.
pub fn is_open_kind(name: &str) -> bool {
    let k = kind(name);
    k.bike || k.moto
}

pub fn spec(model: &str) -> Option<&'static Spec> {
    SPECS.iter().find(|(k, _)| *k == model).map(|(_, s)| s)
}
fn h01(n: f64) -> f64 {
    let x = (n * 91.345 + 12.9898).sin() * 43758.5453;
    x - x.floor()
}
/// Modell eines Fahrzeugs: fest (`model`), Sonderfahrzeug nach Art, Spielerauto Limousine, sonst aus der Nummer.
pub fn car_model(
    id: u32,
    kind: &str,
    role_player: bool,
    model: Option<&'static str>,
) -> &'static str {
    if let Some(m) = model {
        return m;
    }
    if kind != "car"
        && let Some((k, _)) = SPECS.iter().find(|(k, _)| *k == kind)
    {
        return k;
    }
    if role_player {
        return "limousine";
    }
    let mut r = h01(id as f64);
    for (m, share) in CAR_MODELS {
        r -= share;
        if r < 0. {
            return m;
        }
    }
    "limousine"
}
pub fn spec_of(model: &str) -> &'static Spec {
    spec(model).unwrap_or_else(|| spec("limousine").expect("Limousine fehlt"))
}
/// Leistung in PS (1 kW = 1,36 PS)
pub fn ps_of(s: &Spec) -> i64 {
    (s.kw * 1.36).round() as i64
}
/// „Oberbaum Furia“
pub fn vehicle_name(model: &str) -> String {
    let (_, make, typ) = NAMES
        .iter()
        .find(|(k, ..)| *k == model)
        .or_else(|| NAMES.iter().find(|(k, ..)| *k == "limousine"))
        .expect("Namen");
    format!("{make} {typ}")
}
/// „Sportwagen · Mittelmotor · RWD · 435 PS“
pub fn spec_line(model: &str) -> String {
    let s = spec_of(model);
    if s.two_wheel {
        let line = match model {
            "motorcycle" => "Motorrad · Vierzylinder · Kette",
            "scooter" => "Motorroller · Einzylinder · Automatik",
            _ => s.label,
        };
        return format!("{line} · {} PS", ps_of(s));
    }
    let engine = match s.engine {
        Engine::Front => "Frontmotor",
        Engine::Mid => "Mittelmotor",
        Engine::Rear => "Heckmotor",
        Engine::Floor => "Elektromotoren",
    };
    let drive = match s.drive {
        Drive::Fwd => "FWD",
        Drive::Rwd => "RWD",
        Drive::Awd => "AWD",
    };
    format!("{} · {engine} · {drive} · {} PS", s.label, ps_of(s))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tables_are_complete() {
        assert_eq!(CAR_MODELS.len(), 37);
        let sum: f64 = CAR_MODELS.iter().map(|(_, s)| s).sum();
        assert!((sum - 1.).abs() < 1e-9, "Anteile summieren sich zu {sum}");
        for (m, _) in CAR_MODELS {
            assert!(
                spec(m).is_some() && NAMES.iter().any(|(k, ..)| k == m),
                "{m}"
            );
        }
        for k in KINDS.iter().filter(|k| k.name != "car" && !k.bike) {
            assert!(spec(k.name).is_some(), "{}", k.name);
        }
        assert_eq!(
            spec_line("sportwagen"),
            "Sportwagen · Mittelmotor · RWD · 435 PS"
        );
        assert_eq!(vehicle_name("sportwagen"), "Oberbaum Furia");
        assert!(spec("motorcycle").unwrap().two_wheel);
        assert_eq!(spec("oldtimer").unwrap().brake_k, 0.7);
    }
    #[test]
    fn model_choice() {
        assert_eq!(car_model(7, "car", true, None), "limousine");
        assert_eq!(car_model(7, "truck", false, None), "truck");
        assert_eq!(car_model(7, "car", false, Some("rallye")), "rallye");
        // Verteilung ungefähr wie die Anteile
        let n = (1..20000)
            .filter(|&i| car_model(i, "car", false, None) == "kleinwagen")
            .count();
        assert!((1800..2600).contains(&n), "{n}");
    }
}
