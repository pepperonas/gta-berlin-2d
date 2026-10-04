//! Ereignisse eines Simulationsschritts (für Klang, HUD und Effekte; `world.js` `w.events`).

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Crash {
        x: f64,
        y: f64,
        strength: f64,
        car: u32,
    },
    Wreck {
        x: f64,
        y: f64,
        car: u32,
    },
    Knock {
        x: f64,
        y: f64,
        car: u32,
    },
    Horn {
        x: f64,
        y: f64,
        npc: bool,
    },
    Door {
        x: f64,
        y: f64,
    },
    /// Auto schwimmt in einer Pfütze auf (`player` = das eigene)
    Aquaplane {
        x: f64,
        y: f64,
        car: u32,
        player: bool,
    },
    Carjack {
        x: f64,
        y: f64,
        bike: bool,
    },
    Bump {
        x: f64,
        y: f64,
    },
    Hit {
        x: f64,
        y: f64,
        car: u32,
        player: bool,
        speed: f64,
        /// ein Radfahrer (stürzt), sonst ein Fußgänger
        bike: bool,
    },
    Ui,
    MissionStart,
    MissionFail,
    MissionSuccess,
    Pickup,
    Tick,
    Notice(String),
    // Kampf (combat.rs)
    Shot {
        x: f64,
        y: f64,
        a: f64,
        weapon: &'static str,
        /// Endpunkte der Kugeln (Leuchtspuren)
        traces: Vec<(f64, f64)>,
    },
    Swing {
        x: f64,
        y: f64,
        weapon: &'static str,
        hit: bool,
        npc: bool,
    },
    Impact {
        x: f64,
        y: f64,
        metal: bool,
    },
    Blood {
        x: f64,
        y: f64,
        a: f64,
        n: u32,
    },
    Kill {
        x: f64,
        y: f64,
        weapon: &'static str,
        player: bool,
    },
    WeaponHit {
        weapon: &'static str,
        car: bool,
    },
    Thud {
        x: f64,
        y: f64,
    },
    PlayerHurt {
        x: f64,
        y: f64,
        dmg: f64,
    },
    Wasted {
        x: f64,
        y: f64,
    },
    Respawn {
        x: f64,
        y: f64,
        fee: f64,
    },
    Reload {
        weapon: &'static str,
    },
    Reloaded {
        weapon: &'static str,
    },
    WeaponSwitch {
        weapon: &'static str,
    },
    /// Radfahrer vom Rad geholt (Schuss, Schlag)
    BikeDown {
        x: f64,
        y: f64,
        player: bool,
    },
    /// Rettungswagen hat Tote mitgenommen
    PickupBody {
        x: f64,
        y: f64,
    },
}
