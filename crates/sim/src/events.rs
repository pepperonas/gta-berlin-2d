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
    Carjack {
        x: f64,
        y: f64,
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
    },
    Ui,
    MissionStart,
    MissionFail,
    MissionSuccess,
    Pickup,
    Tick,
    Notice(String),
}
