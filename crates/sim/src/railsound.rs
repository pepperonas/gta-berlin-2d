//! Klang der S- und U-Bahn (rein, ohne Gerät): aus Tempo, Beschleunigung und Lage (Tunnel, offene Strecke,
//! Bahnhofshalle) die Pegel und Frequenzen der Schichten, die `berlin-audio` erzeugt.
//!
//! - **Rollen**: Rad auf Schiene, bandbegrenztes Rauschen, Band steigt mit dem Tempo.
//! - **Grollen**: tiefes Rauschen des Wagenkastens, im Tunnel deutlich lauter (die Röhre wirft es zurück).
//! - **Fahrmotor**: Umrichter-Surren, dessen Ton mit dem Tempo steigt; laut beim Anfahren und elektrischen Bremsen,
//!   leise beim Rollen.
//! - **Fahrtwind**: draußen leise, im Tunnel das Rauschen der Röhre.
//! - **Quietschen**: Bremsen kurz vor dem Halt.
//! - **Schienenstöße**: je Achse ein „ta“ an jedem Stoß; zwei Achsen je Drehgestell, zwei Drehgestelle je Wagen.
//! - **Halle**: Nachhall im U-Bahnhof und (schwächer) im Tunnel.
use crate::transit::Mode;

/// px je echter Sekunde → km/h (10 px = 1 m)
pub const KMH: f64 = 0.36;
/// px zwischen zwei Schienenstößen (30 m Schienenlänge)
pub const JOINT: f64 = 300.;
/// Achsen eines Wagens relativ zur ersten (px): Drehgestell mit 2,5 m Achsabstand, Drehgestelle 12 m auseinander
pub const AXLES: [f64; 4] = [0., 25., 120., 145.];
/// km/h: darunter quietschen die Bremsen beim Einfahren
pub const SQUEAL_BELOW: f64 = 35.;
/// Nachhall im U-Bahnhof bzw. bei der Fahrt im Tunnel (Anteil 0…1)
pub const HALL_STATION: f64 = 0.55;
pub const HALL_TUNNEL: f64 = 0.3;
/// px: so weit hört man einen Zug im Bahnhof
pub const PLATFORM_HEAR: f64 = 1600.;

/// Pegel (0…1) und Frequenzen (Hz) eines Zuges.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct TrainLayers {
    pub roll: f64,
    pub roll_f: f64,
    pub rumble: f64,
    pub motor: f64,
    pub motor_f: f64,
    pub wind: f64,
    pub squeal: f64,
}

/// Alles zur Bahn in einem Bild.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct RailMix {
    /// der Zug, in dem man sitzt bzw. den man fährt
    pub ride: TrainLayers,
    /// lautester Zug am Bahnsteig (ein- oder ausfahrend, stehend)
    pub pass: TrainLayers,
    /// Richtung des Zugs am Bahnsteig, −1 links … 1 rechts
    pub pass_pan: f64,
    /// Nachhall 0…1
    pub hall: f64,
}

/// Lage eines Zuges für den Klang.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Motion {
    pub mode: Mode,
    /// px je echter Sekunde
    pub v: f64,
    /// px je s², positiv = anfahren, negativ = bremsen
    pub a: f64,
    pub tunnel: bool,
}

/// Schichten eines fahrenden Zuges, so wie man sie im Wagen hört.
pub fn ride_layers(m: &Motion) -> TrainLayers {
    let kmh = (m.v * KMH).max(0.);
    let s = (kmh / 100.).min(1.4);
    // S-Bahn: größere Wagen, tieferer Motorton
    let (motor_base, motor_k) = if m.mode == Mode::SBahn {
        (110., 7.)
    } else {
        (150., 9.)
    };
    // Zug- oder Bremskraft (die Raffung macht beides dreimal so stark – relativ gemessen)
    let a_rel = (m.a.abs() / (12. * crate::transit::RAIL_PACE)).min(1.);
    let moving = (kmh / 3.).min(1.);
    let tunnel = if m.tunnel { 1. } else { 0. };
    if kmh < 1. {
        // steht: Lüfter und Umrichter summen leise, der Wagenkasten brummt kaum
        return TrainLayers {
            motor: 0.35,
            motor_f: if m.mode == Mode::SBahn { 95. } else { 120. },
            rumble: 0.12,
            ..TrainLayers::default()
        };
    }
    TrainLayers {
        roll: s.powf(0.8).min(1.) * (0.75 + 0.25 * tunnel),
        roll_f: 260. + kmh * 6.,
        rumble: (0.25 + 0.6 * s.min(1.)) * moving * (1. + 0.6 * tunnel) / 1.6,
        motor: moving * (0.15 + 0.85 * a_rel) * (1. - (kmh / 160.).min(0.6)),
        motor_f: (motor_base + kmh * motor_k).min(1900.),
        wind: if m.tunnel {
            s.powf(1.5).min(1.)
        } else {
            0.3 * s.powi(2).min(1.)
        },
        squeal: squeal(kmh, m.a),
    }
}

/// Bremsquietschen: nur beim Bremsen, am stärksten kurz vor dem Stand.
pub fn squeal(kmh: f64, a: f64) -> f64 {
    if a >= -1. || !(2. ..SQUEAL_BELOW).contains(&kmh) {
        return 0.;
    }
    let near_stop = 1. - kmh / SQUEAL_BELOW;
    (near_stop * 1.6).min(1.) * (kmh / 6.).min(1.)
}

/// Schienenstöße, die die Achsen des eigenen Wagens zwischen zwei Kilometerständen (px) überfahren: Anzahl.
pub fn joints_between(odo0: f64, odo1: f64) -> u32 {
    if odo1 <= odo0 {
        return 0;
    }
    AXLES
        .iter()
        .map(|a| ((odo1 - a) / JOINT).floor() - ((odo0 - a) / JOINT).floor())
        .map(|n| n.max(0.) as u32)
        .sum()
}
/// Lautstärke eines Schienenstoßes: mit dem Tempo lauter, im Tunnel hallender (das macht die Halle).
pub fn joint_level(v: f64) -> f64 {
    ((v * KMH) / 70.).clamp(0.15, 1.)
}

/// Ein Zug am Bahnsteig, vom Zuhörer aus gesehen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Heard {
    pub mode: Mode,
    /// Abstand zum nächsten Wagen (px)
    pub dist: f64,
    /// seitlich: −1 links … 1 rechts
    pub pan: f64,
    pub v: f64,
    pub a: f64,
    pub dwelling: bool,
}
/// Lautester Zug am Bahnsteig: Fahrschichten, gedämpft mit dem Abstand; ein stehender Zug summt leise
/// (Lüfter, Umrichter).
pub fn platform_layers(trains: &[Heard]) -> (TrainLayers, f64) {
    let mut best = (TrainLayers::default(), 0., 0f64);
    for t in trains {
        if t.dist > PLATFORM_HEAR {
            continue;
        }
        let near = 1. / (1. + t.dist / 250.);
        let mut l = ride_layers(&Motion {
            mode: t.mode,
            v: if t.dwelling { 0. } else { t.v },
            a: t.a,
            tunnel: true,
        });
        // draußen hört man die Räder lauter als im Wagen, den Motor leiser
        l.roll = (l.roll * 1.3).min(1.) * near;
        l.rumble = (l.rumble * 1.4).min(1.) * near;
        l.motor *= 0.8 * near;
        l.wind *= 0.7 * near;
        l.squeal *= near.sqrt();
        let loud = l.roll + l.rumble + l.motor + l.squeal;
        if loud > best.2 {
            best = (l, t.pan, loud);
        }
    }
    (best.0, best.1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(kmh: f64, a: f64, tunnel: bool) -> TrainLayers {
        ride_layers(&Motion {
            mode: Mode::UBahn,
            v: kmh / KMH,
            a,
            tunnel,
        })
    }

    #[test]
    fn faster_is_louder_and_higher_tunnel_roars() {
        let (slow, fast) = (at(20., 0., false), at(90., 0., false));
        assert!(fast.roll > slow.roll && fast.roll_f > slow.roll_f);
        assert!(fast.motor_f > slow.motor_f);
        let tunnel = at(90., 0., true);
        assert!(
            tunnel.rumble > fast.rumble && tunnel.wind > 2. * fast.wind,
            "Röhre"
        );
        // Stand: kein Rollen, nur leises Summen (Lüfter, Umrichter)
        let still = at(0., 0., true);
        assert_eq!((still.roll, still.wind, still.squeal), (0., 0., 0.));
        assert!(still.motor > 0. && still.motor < 0.5 && still.motor_f < 150.);
        // Anfahren: Motor laut, Rollen leise
        let start = at(15., 30., false);
        assert!(start.motor > 0.6 && start.motor > 3. * at(15., 0., false).motor);
        // S-Bahn brummt tiefer
        let s = ride_layers(&Motion {
            mode: Mode::SBahn,
            v: 60. / KMH,
            a: 0.,
            tunnel: false,
        });
        assert!(s.motor_f < at(60., 0., false).motor_f);
        for l in [slow, fast, tunnel, start, s] {
            for v in [l.roll, l.rumble, l.motor, l.wind, l.squeal] {
                assert!((0. ..=1.).contains(&v), "{l:?}");
            }
        }
    }

    #[test]
    fn brakes_squeal_only_while_braking_near_a_stop() {
        assert_eq!(squeal(20., 0.), 0., "rollt");
        assert_eq!(squeal(20., 5.), 0., "fährt an");
        assert_eq!(squeal(80., -20.), 0., "zu schnell");
        assert_eq!(squeal(0.5, -20.), 0., "steht");
        assert!(
            squeal(12., -20.) > squeal(30., -20.),
            "lauter kurz vor dem Stand"
        );
    }

    #[test]
    fn joints_tick_per_axle() {
        assert_eq!(joints_between(0., 0.), 0);
        assert_eq!(joints_between(10., 5.), 0, "rückwärts zählt nicht");
        // ein Wagen über einen Stoß (die letzte Achse läuft 145 px hinter der ersten): vier Achsen, vier Schläge
        assert_eq!(joints_between(290., 290. + 160.), 4);
        assert_eq!(
            joints_between(290., 290. + 150.),
            3,
            "letzte Achse noch davor"
        );
        // eine Schienenlänge: genau vier
        assert_eq!(joints_between(1000., 1000. + JOINT), 4);
        // zwei kurze Schritte = ein langer
        assert_eq!(
            joints_between(1000., 1100.) + joints_between(1100., 1300.),
            joints_between(1000., 1300.)
        );
        assert!(joint_level(100. / KMH) > joint_level(20. / KMH));
    }

    #[test]
    fn the_nearest_moving_train_wins_on_the_platform() {
        let t = |dist, pan, kmh: f64, dwelling| Heard {
            mode: Mode::UBahn,
            dist,
            pan,
            v: kmh / KMH,
            a: if dwelling { 0. } else { -20. },
            dwelling,
        };
        let (l, pan) = platform_layers(&[t(1200., -1., 30., false), t(100., 1., 25., false)]);
        assert_eq!(pan, 1.);
        assert!(l.squeal > 0. && l.roll > 0.);
        // stehender Zug summt nur
        let (l, _) = platform_layers(&[t(50., 0., 0., true)]);
        assert!(l.motor > 0. && l.roll == 0. && l.squeal == 0.);
        // außer Hörweite: Stille
        assert_eq!(
            platform_layers(&[t(5000., 0., 60., false)]).0,
            TrainLayers::default()
        );
    }
}
