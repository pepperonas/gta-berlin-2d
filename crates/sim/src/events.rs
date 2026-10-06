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
        /// von der Spielfigur zerstört (Waffe)
        player: bool,
    },
    Knock {
        x: f64,
        y: f64,
        car: u32,
    },
    /// Wrack fängt Feuer (explodiert nach `car::BURN_S`)
    CarFire {
        x: f64,
        y: f64,
        car: u32,
    },
    /// Explosion (Druckwelle schon angewandt): Fahrzeug (`car`) oder Handgranate (`car` = None); `strength` 0…1
    Explosion {
        x: f64,
        y: f64,
        car: Option<u32>,
        strength: f64,
    },
    /// Wurfwaffe geworfen (Handgranate, Molotow)
    Throw {
        x: f64,
        y: f64,
        weapon: &'static str,
    },
    /// Handgranate prallt auf (Boden, Wand, Auto)
    Bounce {
        x: f64,
        y: f64,
        strength: f64,
    },
    /// Molotow zerschellt: Feuer am Boden (`r` = Radius in px)
    Shatter {
        x: f64,
        y: f64,
        r: f64,
    },
    /// Rad fährt über einen Bordstein (Fahrphysik)
    Curb {
        x: f64,
        y: f64,
        car: u32,
    },
    Horn {
        x: f64,
        y: f64,
        npc: bool,
    },
    /// Vollbremsung im Linienbus: stehende Fahrgäste stürzen (Spielhaken für Missionen und Wertung)
    PassengersFell {
        x: f64,
        y: f64,
        car: u32,
    },
    Door {
        x: f64,
        y: f64,
    },
    /// Mitfahren: eingestiegen (aufgesprungen), ausgestiegen (abgesprungen), Fahrt ohne Fahrzeug beendet
    Board {
        line: String,
        hop: bool,
        x: f64,
        y: f64,
    },
    Alight {
        hop: bool,
        x: f64,
        y: f64,
    },
    RideEnd {
        x: f64,
        y: f64,
    },
    /// Zug führen: übernommen, Zug voraus, Türen, Trinkgeld, gewendet
    TrainTake {
        line: String,
        x: f64,
        y: f64,
    },
    TrainBlocked,
    DoorsOpen {
        out: u32,
        inn: u32,
        first: bool,
    },
    DoorsClose,
    Tip {
        amount: f64,
    },
    TurnAround {
        line: String,
    },
    /// U-Bahnhof betreten bzw. über die Treppe verlassen
    StationEnter {
        x: f64,
        y: f64,
        name: String,
    },
    /// Umsteigen über eine Treppe zu einem anderen Bahnsteig desselben Bahnhofs
    StationTransfer {
        x: f64,
        y: f64,
        name: String,
        level: i8,
        lines: Vec<String>,
    },
    StationExit {
        x: f64,
        y: f64,
        name: String,
    },
    /// Straßenbahn klingelt vor einem Hindernis
    TramBell {
        x: f64,
        y: f64,
    },
    /// Bus hält (Linie, Haltestelle) bzw. Wartende steigen ein
    BusStop {
        x: f64,
        y: f64,
        line: String,
        stop: String,
    },
    BusBoard {
        x: f64,
        y: f64,
        n: usize,
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
    /// Spieler springt ab bzw. landet
    Jump {
        x: f64,
        y: f64,
    },
    Land {
        x: f64,
        y: f64,
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
