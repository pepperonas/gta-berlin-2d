//! Frei belegbare Steuerung (Tastatur und Xbox-Controller) samt Kennlinien für Trigger und Lenkstick.
//!
//! Jede Spielaktion hat bis zu zwei Tastaturtasten und eine Controller-Taste. Fest bleiben – damit man sich nie
//! aussperrt – die Menübedienung (Pfeile, Enter, Esc, A/B in Menüs), die Zifferntasten 1–6 für die Waffen, die Maus
//! und die Sticks. Gespeichert wird in `settings.json` neben dem Spielstand (`"bindings"`), nur die Abweichungen vom
//! Standard, damit spätere Standardänderungen weiter ankommen.
use berlin_engine::KeyCode;
use berlin_engine::Keys;
use berlin_engine::pad::Pad;
use std::collections::BTreeMap;

/// Wo eine Aktion wirkt – zwei Aktionen dürfen dieselbe Taste nur haben, wenn sich ihre Bereiche nicht überschneiden.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Context {
    Foot,
    Car,
    Both,
}
impl Context {
    fn overlaps(self, o: Context) -> bool {
        self == Context::Both || o == Context::Both || self == o
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Action {
    Up,
    Down,
    Left,
    Right,
    Sprint,
    /// Fahrrad: kräftig treten (Ausdauer) – im Fahrzeug getrennt vom Sprinten, weil A dort die Handbremse ist
    PedalSprint,
    Slow,
    Throttle,
    Brake,
    Handbrake,
    Horn,
    /// Autoradio: nächster bzw. vorheriger Sender (AUS gehört zur Runde)
    RadioNext,
    RadioPrev,
    Esp,
    Abs,
    /// Fahrhilfen-Rad (Controller: rechter Stick drücken): ESP, ABS, Sirene
    AssistWheel,
    EnterExit,
    Use,
    /// Aktion im Fahrzeug (Auftrag, Einladen): am Controller nicht auf A, das ist im Fahrzeug die Handbremse
    UseCar,
    Ride,
    /// Springen (zu Fuß): über Zäune, Poller, Kisten
    Jump,
    Fire,
    Kick,
    Reload,
    NextWeapon,
    PrevWeapon,
    WeaponWheel,
    ZoomIn,
    ZoomOut,
    Map,
    Pause,
    Console,
    Save,
    Clock,
    Weather,
    Mute,
    /// Grafik HD / Pixel umschalten
    GraphicsMode,
    /// Entwickler-Anzeige der Fahrphysik (nur Entwickler-Build): ein/aus, Regler wählen und verstellen, Änderungen
    /// ausgeben
    DebugToggle,
    DebugPrev,
    DebugNext,
    DebugLess,
    DebugMore,
    DebugExport,
    /// Motorsound-Anzeige (Entwickler-Build) und A/B-Vergleich mit der Referenzaufnahme
    EngineDebug,
    EngineAb,
}

/// Controller-Tasten (Xbox-Bezeichnungen); Trigger gelten als Taste ab halbem Weg, Gas/Bremse lesen sie analog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PadButton {
    A,
    B,
    X,
    Y,
    LB,
    RB,
    LT,
    RT,
    View,
    Menu,
    Up,
    Down,
    Left,
    Right,
    LS,
    RS,
}
pub const PAD_BUTTONS: [PadButton; 16] = [
    PadButton::A,
    PadButton::B,
    PadButton::X,
    PadButton::Y,
    PadButton::LB,
    PadButton::RB,
    PadButton::LT,
    PadButton::RT,
    PadButton::View,
    PadButton::Menu,
    PadButton::Up,
    PadButton::Down,
    PadButton::Left,
    PadButton::Right,
    PadButton::LS,
    PadButton::RS,
];
impl PadButton {
    pub fn name(self) -> &'static str {
        match self {
            PadButton::A => "A",
            PadButton::B => "B",
            PadButton::X => "X",
            PadButton::Y => "Y",
            PadButton::LB => "LB",
            PadButton::RB => "RB",
            PadButton::LT => "LT",
            PadButton::RT => "RT",
            PadButton::View => "Ansicht",
            PadButton::Menu => "Menü",
            PadButton::Up => "Steuerkreuz ↑",
            PadButton::Down => "Steuerkreuz ↓",
            PadButton::Left => "Steuerkreuz ←",
            PadButton::Right => "Steuerkreuz →",
            PadButton::LS => "Linker Stick drücken",
            PadButton::RS => "Rechter Stick drücken",
        }
    }
    /// Kurzname für Speicherung (stabil, ASCII).
    pub fn key(self) -> &'static str {
        match self {
            PadButton::A => "A",
            PadButton::B => "B",
            PadButton::X => "X",
            PadButton::Y => "Y",
            PadButton::LB => "LB",
            PadButton::RB => "RB",
            PadButton::LT => "LT",
            PadButton::RT => "RT",
            PadButton::View => "View",
            PadButton::Menu => "Menu",
            PadButton::Up => "DPadUp",
            PadButton::Down => "DPadDown",
            PadButton::Left => "DPadLeft",
            PadButton::Right => "DPadRight",
            PadButton::LS => "LS",
            PadButton::RS => "RS",
        }
    }
    pub fn from_key(s: &str) -> Option<Self> {
        PAD_BUTTONS.iter().copied().find(|b| b.key() == s)
    }
    /// Analogwert 0…1 (Trigger) bzw. 0/1.
    pub fn value(self, p: &Pad) -> f32 {
        let b = |v: bool| if v { 1. } else { 0. };
        match self {
            PadButton::A => b(p.a),
            PadButton::B => b(p.b),
            PadButton::X => b(p.x),
            PadButton::Y => b(p.y),
            PadButton::LB => b(p.lb),
            PadButton::RB => b(p.rb),
            PadButton::LT => p.lt,
            PadButton::RT => p.rt,
            PadButton::View => b(p.view),
            PadButton::Menu => b(p.menu),
            PadButton::Up => b(p.up),
            PadButton::Down => b(p.down),
            PadButton::Left => b(p.left),
            PadButton::Right => b(p.right),
            PadButton::LS => b(p.ls),
            PadButton::RS => b(p.rs),
        }
    }
    pub fn held(self, p: &Pad) -> bool {
        self.value(p) > 0.5
    }
    /// Gedrückt seit dem letzten Schritt (Flanke). Trigger: über die Flanke ihres Halbwegs (`Pad::lt_down`).
    pub fn pressed(self, e: &Pad) -> bool {
        self.held(e)
    }
}

/// Eine Aktion: Bezeichnung, Bereich, Standardbelegung.
pub struct Info {
    pub action: Action,
    pub label: &'static str,
    pub context: Context,
    pub keys: [Option<KeyCode>; 2],
    pub pad: Option<PadButton>,
    /// Controller-Spalte fest (Stick), nicht belegbar
    pub pad_fixed: Option<&'static str>,
}
const fn info(
    action: Action,
    label: &'static str,
    context: Context,
    keys: [Option<KeyCode>; 2],
    pad: Option<PadButton>,
    pad_fixed: Option<&'static str>,
) -> Info {
    Info {
        action,
        label,
        context,
        keys,
        pad,
        pad_fixed,
    }
}
use Action as A;
use Context::{Both, Car, Foot};
use PadButton as P;
/// Alle belegbaren Aktionen in der Reihenfolge der Belegungstafel.
pub const ACTIONS: &[Info] = &[
    info(
        A::Up,
        "Laufen vorwärts",
        Foot,
        [Some(KeyCode::KeyW), Some(KeyCode::ArrowUp)],
        None,
        Some("Linker Stick"),
    ),
    info(
        A::Down,
        "Laufen rückwärts",
        Foot,
        [Some(KeyCode::KeyS), Some(KeyCode::ArrowDown)],
        None,
        Some("Linker Stick"),
    ),
    info(
        A::Left,
        "Links (laufen, lenken)",
        Both,
        [Some(KeyCode::KeyA), Some(KeyCode::ArrowLeft)],
        None,
        Some("Linker Stick"),
    ),
    info(
        A::Right,
        "Rechts (laufen, lenken)",
        Both,
        [Some(KeyCode::KeyD), Some(KeyCode::ArrowRight)],
        None,
        Some("Linker Stick"),
    ),
    info(
        A::Sprint,
        "Sprinten",
        Foot,
        [Some(KeyCode::ShiftLeft), Some(KeyCode::ShiftRight)],
        Some(P::A),
        None,
    ),
    info(
        A::PedalSprint,
        "Kräftig treten (Fahrrad)",
        Car,
        [Some(KeyCode::ShiftLeft), Some(KeyCode::ShiftRight)],
        Some(P::Up),
        None,
    ),
    info(
        A::Slow,
        "Langsam / ruhig zielen",
        Foot,
        [Some(KeyCode::AltLeft), Some(KeyCode::AltRight)],
        Some(P::LT),
        None,
    ),
    info(
        A::Throttle,
        "Gas",
        Car,
        [Some(KeyCode::KeyW), Some(KeyCode::ArrowUp)],
        Some(P::RT),
        None,
    ),
    info(
        A::Brake,
        "Bremse / rückwärts",
        Car,
        [Some(KeyCode::KeyS), Some(KeyCode::ArrowDown)],
        Some(P::LT),
        None,
    ),
    info(
        A::Handbrake,
        "Handbremse",
        Car,
        [Some(KeyCode::Space), None],
        Some(P::A),
        None,
    ),
    info(
        A::Horn,
        "Hupe (halten: Sirene)",
        Car,
        [Some(KeyCode::KeyH), None],
        Some(P::LS),
        None,
    ),
    info(
        A::RadioNext,
        "Radio: nächster Sender",
        Car,
        [Some(KeyCode::KeyR), None],
        Some(P::RB),
        None,
    ),
    info(
        A::RadioPrev,
        "Radio: vorheriger Sender",
        Car,
        [Some(KeyCode::KeyQ), None],
        Some(P::LB),
        None,
    ),
    info(
        A::Esp,
        "ESP an/aus",
        Car,
        [Some(KeyCode::KeyX), None],
        Some(P::B),
        None,
    ),
    info(
        A::Abs,
        "ABS an/aus",
        Car,
        [Some(KeyCode::KeyY), Some(KeyCode::KeyZ)],
        Some(P::Down),
        None,
    ),
    info(
        A::AssistWheel,
        "Fahrhilfen-Rad (ESP, ABS, Sirene)",
        Car,
        [None, None],
        Some(P::RS),
        None,
    ),
    info(
        A::EnterExit,
        "Ein-/Aussteigen · Bahnhof",
        Both,
        [Some(KeyCode::KeyF), None],
        Some(P::Y),
        None,
    ),
    info(
        A::Use,
        "Aktion (Auftrag, Einladen)",
        Foot,
        [Some(KeyCode::KeyE), None],
        Some(P::A),
        None,
    ),
    info(
        A::UseCar,
        "Aktion im Fahrzeug (Auftrag, Einladen)",
        Car,
        [Some(KeyCode::KeyE), None],
        Some(P::Right),
        None,
    ),
    info(
        A::Ride,
        "Mitfahren (Bus, Bahn)",
        Foot,
        [Some(KeyCode::KeyG), None],
        Some(P::Down),
        None,
    ),
    info(
        A::Jump,
        "Springen",
        Foot,
        [Some(KeyCode::Space), None],
        Some(P::X),
        None,
    ),
    info(
        A::Fire,
        "Angreifen / schießen",
        Foot,
        [Some(KeyCode::ControlLeft), Some(KeyCode::ControlRight)],
        Some(P::RT),
        None,
    ),
    info(
        A::Kick,
        "Treten",
        Foot,
        [Some(KeyCode::KeyV), None],
        Some(P::LS),
        None,
    ),
    info(
        A::Reload,
        "Nachladen",
        Foot,
        [Some(KeyCode::KeyR), None],
        Some(P::B),
        None,
    ),
    info(
        A::NextWeapon,
        "Nächste Waffe",
        Foot,
        [Some(KeyCode::KeyQ), None],
        Some(P::RB),
        None,
    ),
    info(
        A::PrevWeapon,
        "Vorherige Waffe",
        Foot,
        [None, None],
        Some(P::LB),
        None,
    ),
    info(
        A::WeaponWheel,
        "Waffenrad (drücken: auf/zu)",
        Foot,
        [None, None],
        Some(P::RS),
        None,
    ),
    info(
        A::ZoomIn,
        "Kamera näher",
        Foot,
        [Some(KeyCode::Equal), Some(KeyCode::NumpadAdd)],
        Some(P::Right),
        None,
    ),
    info(
        A::ZoomOut,
        "Kamera weiter",
        Foot,
        [Some(KeyCode::Minus), Some(KeyCode::NumpadSubtract)],
        Some(P::Left),
        None,
    ),
    info(
        A::Map,
        "Stadtplan",
        Both,
        [Some(KeyCode::Tab), None],
        Some(P::View),
        None,
    ),
    info(
        A::Pause,
        "Pause",
        Both,
        [Some(KeyCode::Escape), Some(KeyCode::KeyP)],
        Some(P::Menu),
        None,
    ),
    info(
        A::Console,
        "Befehlszeile",
        Both,
        [Some(KeyCode::Enter), Some(KeyCode::NumpadEnter)],
        None,
        None,
    ),
    info(
        A::Save,
        "Speichern",
        Both,
        [Some(KeyCode::F5), None],
        None,
        None,
    ),
    info(
        A::Clock,
        "Uhr +1 Stunde",
        Both,
        [Some(KeyCode::KeyT), None],
        None,
        None,
    ),
    info(
        A::Weather,
        "Wetter weiter",
        Both,
        [Some(KeyCode::KeyN), None],
        None,
        None,
    ),
    info(
        A::Mute,
        "Ton an/aus",
        Both,
        [Some(KeyCode::KeyM), None],
        None,
        None,
    ),
    info(
        A::GraphicsMode,
        "Grafik: HD / Pixel",
        Both,
        [Some(KeyCode::F8), None],
        None,
        None,
    ),
    info(
        A::DebugToggle,
        "Physik-Anzeige (Entwickler)",
        Both,
        [Some(KeyCode::F3), None],
        None,
        None,
    ),
    info(
        A::DebugPrev,
        "Physik-Regler davor",
        Both,
        [Some(KeyCode::PageUp), None],
        None,
        None,
    ),
    info(
        A::DebugNext,
        "Physik-Regler danach",
        Both,
        [Some(KeyCode::PageDown), None],
        None,
        None,
    ),
    info(
        A::DebugLess,
        "Physik-Regler weniger",
        Both,
        [Some(KeyCode::Comma), None],
        None,
        None,
    ),
    info(
        A::DebugMore,
        "Physik-Regler mehr",
        Both,
        [Some(KeyCode::Period), None],
        None,
        None,
    ),
    info(
        A::DebugExport,
        "Physik-Änderungen ausgeben",
        Both,
        [Some(KeyCode::F6), None],
        None,
        None,
    ),
    info(
        A::EngineDebug,
        "Motorsound-Anzeige (Entwickler)",
        Both,
        [Some(KeyCode::F4), None],
        None,
        None,
    ),
    info(
        A::EngineAb,
        "Motorsound A/B (Referenz)",
        Both,
        [Some(KeyCode::F7), None],
        None,
        None,
    ),
];
pub fn info_of(a: Action) -> &'static Info {
    ACTIONS
        .iter()
        .find(|i| i.action == a)
        .expect("Aktion in ACTIONS")
}
/// Gewollte Doppelbelegungen: A tippen = Aktion, A halten = sprinten (wie bisher).
fn compatible(a: Action, b: Action) -> bool {
    matches!((a, b), (A::Sprint, A::Use) | (A::Use, A::Sprint))
}

/// Tasten mit Speichername und Anzeige (die belegbaren Tasten; andere werden beim Belegen ignoriert).
pub const KEYS: &[(KeyCode, &str, &str)] = &[
    (KeyCode::KeyA, "KeyA", "A"),
    (KeyCode::KeyB, "KeyB", "B"),
    (KeyCode::KeyC, "KeyC", "C"),
    (KeyCode::KeyD, "KeyD", "D"),
    (KeyCode::KeyE, "KeyE", "E"),
    (KeyCode::KeyF, "KeyF", "F"),
    (KeyCode::KeyG, "KeyG", "G"),
    (KeyCode::KeyH, "KeyH", "H"),
    (KeyCode::KeyI, "KeyI", "I"),
    (KeyCode::KeyJ, "KeyJ", "J"),
    (KeyCode::KeyK, "KeyK", "K"),
    (KeyCode::KeyL, "KeyL", "L"),
    (KeyCode::KeyM, "KeyM", "M"),
    (KeyCode::KeyN, "KeyN", "N"),
    (KeyCode::KeyO, "KeyO", "O"),
    (KeyCode::KeyP, "KeyP", "P"),
    (KeyCode::KeyQ, "KeyQ", "Q"),
    (KeyCode::KeyR, "KeyR", "R"),
    (KeyCode::KeyS, "KeyS", "S"),
    (KeyCode::KeyT, "KeyT", "T"),
    (KeyCode::KeyU, "KeyU", "U"),
    (KeyCode::KeyV, "KeyV", "V"),
    (KeyCode::KeyW, "KeyW", "W"),
    (KeyCode::KeyX, "KeyX", "X"),
    (KeyCode::KeyY, "KeyY", "Y"),
    (KeyCode::KeyZ, "KeyZ", "Z"),
    (KeyCode::Digit0, "Digit0", "0"),
    (KeyCode::Digit7, "Digit7", "7"),
    (KeyCode::Digit8, "Digit8", "8"),
    (KeyCode::Digit9, "Digit9", "9"),
    (KeyCode::ArrowUp, "ArrowUp", "↑"),
    (KeyCode::ArrowDown, "ArrowDown", "↓"),
    (KeyCode::ArrowLeft, "ArrowLeft", "←"),
    (KeyCode::ArrowRight, "ArrowRight", "→"),
    (KeyCode::Space, "Space", "Leertaste"),
    (KeyCode::Enter, "Enter", "Enter"),
    (KeyCode::NumpadEnter, "NumpadEnter", "Num-Enter"),
    (KeyCode::Escape, "Escape", "Esc"),
    (KeyCode::Tab, "Tab", "Tab"),
    (KeyCode::Backspace, "Backspace", "Rücktaste"),
    (KeyCode::ShiftLeft, "ShiftLeft", "Umschalt"),
    (KeyCode::ShiftRight, "ShiftRight", "Umschalt rechts"),
    (KeyCode::ControlLeft, "ControlLeft", "Strg"),
    (KeyCode::ControlRight, "ControlRight", "Strg rechts"),
    (KeyCode::AltLeft, "AltLeft", "Alt"),
    (KeyCode::AltRight, "AltRight", "Alt Gr"),
    (KeyCode::CapsLock, "CapsLock", "Feststell"),
    (KeyCode::Minus, "Minus", "ß / -"),
    (KeyCode::Equal, "Equal", "´ / ="),
    (KeyCode::Comma, "Comma", ","),
    (KeyCode::Period, "Period", "."),
    (KeyCode::Slash, "Slash", "- / /"),
    (KeyCode::Semicolon, "Semicolon", "Ö"),
    (KeyCode::Quote, "Quote", "Ä"),
    (KeyCode::BracketLeft, "BracketLeft", "Ü"),
    (KeyCode::BracketRight, "BracketRight", "+"),
    (KeyCode::Backslash, "Backslash", "#"),
    (KeyCode::Backquote, "Backquote", "^"),
    (KeyCode::IntlBackslash, "IntlBackslash", "<"),
    (KeyCode::Insert, "Insert", "Einfg"),
    (KeyCode::Delete, "Delete", "Entf"),
    (KeyCode::Home, "Home", "Pos1"),
    (KeyCode::End, "End", "Ende"),
    (KeyCode::PageUp, "PageUp", "Bild ↑"),
    (KeyCode::PageDown, "PageDown", "Bild ↓"),
    (KeyCode::F1, "F1", "F1"),
    (KeyCode::F2, "F2", "F2"),
    (KeyCode::F3, "F3", "F3"),
    (KeyCode::F4, "F4", "F4"),
    (KeyCode::F5, "F5", "F5"),
    (KeyCode::F6, "F6", "F6"),
    (KeyCode::F7, "F7", "F7"),
    (KeyCode::F8, "F8", "F8"),
    (KeyCode::F9, "F9", "F9"),
    (KeyCode::F10, "F10", "F10"),
    (KeyCode::F11, "F11", "F11"),
    (KeyCode::F12, "F12", "F12"),
    (KeyCode::Numpad0, "Numpad0", "Num 0"),
    (KeyCode::Numpad1, "Numpad1", "Num 1"),
    (KeyCode::Numpad2, "Numpad2", "Num 2"),
    (KeyCode::Numpad3, "Numpad3", "Num 3"),
    (KeyCode::Numpad4, "Numpad4", "Num 4"),
    (KeyCode::Numpad5, "Numpad5", "Num 5"),
    (KeyCode::Numpad6, "Numpad6", "Num 6"),
    (KeyCode::Numpad7, "Numpad7", "Num 7"),
    (KeyCode::Numpad8, "Numpad8", "Num 8"),
    (KeyCode::Numpad9, "Numpad9", "Num 9"),
    (KeyCode::NumpadAdd, "NumpadAdd", "Num +"),
    (KeyCode::NumpadSubtract, "NumpadSubtract", "Num -"),
    (KeyCode::NumpadMultiply, "NumpadMultiply", "Num *"),
    (KeyCode::NumpadDivide, "NumpadDivide", "Num /"),
];
pub fn key_name(k: KeyCode) -> Option<&'static str> {
    KEYS.iter().find(|e| e.0 == k).map(|e| e.2)
}
fn key_from(s: &str) -> Option<KeyCode> {
    KEYS.iter().find(|e| e.1 == s).map(|e| e.0)
}
fn key_id(k: KeyCode) -> Option<&'static str> {
    KEYS.iter().find(|e| e.0 == k).map(|e| e.1)
}
/// Zifferntasten 1–6 wählen fest die Waffe und lassen sich nicht belegen.
pub fn bindable_key(k: KeyCode) -> bool {
    key_id(k).is_some()
}

/// Feste Kennwerte der Controller-Kennlinien.
pub const TRIGGER_DEAD: f32 = 0.06;
pub const TRIGGER_FULL: f32 = 0.96;
pub const THROTTLE_GAMMA: f32 = 1.35;
/// Tastatur: Gas und Bremse fahren in diesen Zeiten auf (s) bzw. zurück – eine Taste ist kein Pedal, ohne Rampe
/// riss jeder Druck die volle Kraft an
pub const PEDAL_UP: f64 = 0.15;
pub const PEDAL_DOWN: f64 = 0.08;
/// Pedalstellung `cur` einen Schritt `dt` zur Tastenstellung `target` (0/1) nachführen.
/// s, die ein einzelner Druck auf LB bzw. RB auf den anderen wartet (gleichzeitig = Befehlszeile)
pub const CHORD_WINDOW: f64 = 0.09;

/// LB + RB gleichzeitig öffnen die Befehlszeile. Damit ein einzelner Druck (nächste Waffe, treten) nicht mit auslöst,
/// wartet er bis `CHORD_WINDOW` auf den anderen Knopf und wird erst dann (oder beim Loslassen) weitergegeben.
#[derive(Debug, Clone, Copy, Default)]
pub struct ShoulderChord {
    /// wartender Druck: RB (sonst LB) und Wartezeit
    wait: Option<(bool, f64)>,
    /// beide gedrückt: bis beide los sind, gibt es nichts weiter
    both: bool,
}
impl ShoulderChord {
    /// Gehaltene Knöpfe und Flanken dieses Schritts → (Befehlszeile, LB-Druck, RB-Druck).
    pub fn step(
        &mut self,
        lb: bool,
        rb: bool,
        lb_edge: bool,
        rb_edge: bool,
        dt: f64,
    ) -> (bool, bool, bool) {
        if self.both {
            if !lb && !rb {
                self.both = false;
            }
            return (false, false, false);
        }
        if lb && rb && (lb_edge || rb_edge || self.wait.is_some()) {
            self.both = true;
            self.wait = None;
            return (true, false, false);
        }
        let mut out = (false, false, false);
        if let Some((is_rb, t)) = self.wait.as_mut() {
            *t += dt;
            let held = if *is_rb { rb } else { lb };
            if !held || *t >= CHORD_WINDOW - 1e-9 {
                if *is_rb {
                    out.2 = true;
                } else {
                    out.1 = true;
                }
                self.wait = None;
            }
        }
        if self.wait.is_none() {
            if lb_edge && !rb {
                self.wait = Some((false, 0.));
            } else if rb_edge && !lb {
                self.wait = Some((true, 0.));
            }
        }
        out
    }
}

/// s gehalten, bis die Hupe zur Sirene wird (nur Fahrzeuge mit Sirene)
pub const SIREN_HOLD: f64 = 0.45;

/// So lange klingt die Hupe nach einem kurzen Druck in einem Fahrzeug mit Sirene (s)
pub const HORN_TAP: f64 = 0.3;

/// Hupe mit langem Druck: ohne Sirene hupt sie, solange gehalten. In einem Fahrzeug mit Sirene hupt ein Druck nicht
/// sofort – erst beim Loslassen vor `SIREN_HOLD` kommt ein kurzer Hupton (`HORN_TAP`); wer länger hält, schaltet die
/// Sirene (einmal je Druck), ohne zu hupen.
#[derive(Debug, Clone, Copy, Default)]
pub struct HornPress {
    t: f64,
    fired: bool,
    /// Rest des Huptons nach einem kurzen Druck
    tap: f64,
}
impl HornPress {
    /// Liefert (Hupe, Sirene umschalten).
    pub fn step(&mut self, held: bool, siren: bool, dt: f64) -> (bool, bool) {
        if !held {
            let tap = if siren && self.t > 0. && !self.fired {
                HORN_TAP
            } else {
                (self.tap - dt).max(0.)
            };
            *self = Self {
                tap,
                ..Self::default()
            };
            return (tap > 0., false);
        }
        self.t += dt;
        self.tap = 0.;
        if !siren {
            return (true, false);
        }
        if !self.fired && self.t >= SIREN_HOLD - 1e-9 {
            self.fired = true;
            return (false, true);
        }
        (false, false)
    }
}

pub fn pedal_ramp(cur: f64, target: f64, dt: f64) -> f64 {
    if target > cur {
        (cur + dt / PEDAL_UP).min(target)
    } else {
        (cur - dt / PEDAL_DOWN).max(target)
    }
}
pub const BRAKE_GAMMA: f32 = 1.7;
pub const STEER_DEAD: f32 = 0.1;

/// Trigger → Pedal: kleine Totzone (Ruherauschen), Sättigung kurz vor dem Anschlag, progressiver Verlauf (feiner
/// Anfang). Volle Auslenkung bleibt 1.
pub fn trigger_curve(v: f32, gamma: f32) -> f32 {
    let t = ((v - TRIGGER_DEAD) / (TRIGGER_FULL - TRIGGER_DEAD)).clamp(0., 1.);
    t.powf(gamma)
}
/// Lenkstick → Lenkung: kleine Totzone, darüber eine Expo-Kurve (feine Mitte, voller Ausschlag bleibt voll).
/// `sens` 0,5…1,5: unter 1 feiner um die Mitte, über 1 direkter.
pub fn steer_curve(x: f32, sens: f32) -> f32 {
    let a = x.abs();
    if a < STEER_DEAD {
        return 0.;
    }
    let t = ((a - STEER_DEAD) / (1. - STEER_DEAD)).min(1.);
    // Expo: Anteil kubisch; bei sens 1 die Hälfte, bei 0,5 fast ganz, bei 1,5 linear
    let expo = (1.5 - sens).clamp(0., 1.) * 0.9;
    let y = t * (1. - expo) + t * t * t * expo;
    y * x.signum()
}

/// Belegung samt Optionen.
#[derive(Debug, Clone, PartialEq)]
pub struct Bindings {
    keys: BTreeMap<Action, [Option<KeyCode>; 2]>,
    pad: BTreeMap<Action, Option<PadButton>>,
    /// Vibration des Controllers
    pub rumble: bool,
    /// Lenkempfindlichkeit 0,5…1,5 (1 = Standard)
    pub steer_sens: f32,
}
impl Default for Bindings {
    fn default() -> Self {
        Self {
            keys: ACTIONS.iter().map(|i| (i.action, i.keys)).collect(),
            pad: ACTIONS.iter().map(|i| (i.action, i.pad)).collect(),
            rumble: true,
            steer_sens: 1.,
        }
    }
}
impl Bindings {
    pub fn keys_of(&self, a: Action) -> [Option<KeyCode>; 2] {
        self.keys.get(&a).copied().unwrap_or([None, None])
    }
    pub fn pad_of(&self, a: Action) -> Option<PadButton> {
        if info_of(a).pad_fixed.is_some() {
            return None;
        }
        self.pad.get(&a).copied().flatten()
    }
    pub fn set_key(&mut self, a: Action, slot: usize, k: Option<KeyCode>) {
        let e = self.keys.entry(a).or_insert([None, None]);
        e[slot.min(1)] = k;
    }
    pub fn set_pad(&mut self, a: Action, b: Option<PadButton>) {
        if info_of(a).pad_fixed.is_none() {
            self.pad.insert(a, b);
        }
    }
    /// Taste oder Controller-Taste gehalten.
    pub fn held(&self, keys: &Keys, a: Action) -> bool {
        self.key_held(keys, a) || self.pad_of(a).is_some_and(|b| b.held(&keys.pad))
    }
    pub fn key_held(&self, keys: &Keys, a: Action) -> bool {
        self.keys_of(a)
            .iter()
            .flatten()
            .any(|k| keys.held.contains(k))
    }
    /// Nur die Tasten (nicht der Controller): gedrückt seit dem letzten Schritt.
    pub fn key_pressed(&self, keys: &Keys, a: Action) -> bool {
        self.keys_of(a)
            .iter()
            .flatten()
            .any(|k| keys.pressed.contains(k))
    }
    /// Gedrückt seit dem letzten Schritt (Flanke).
    pub fn pressed(&self, keys: &Keys, a: Action) -> bool {
        self.keys_of(a)
            .iter()
            .flatten()
            .any(|k| keys.pressed.contains(k))
            || self.pad_pressed(keys, a)
    }
    pub fn pad_pressed(&self, keys: &Keys, a: Action) -> bool {
        self.pad_of(a).is_some_and(|b| b.pressed(&keys.pad_pressed))
    }
    /// Analogwert einer Controller-Taste (Trigger 0…1, Tasten 0/1).
    pub fn pad_value(&self, keys: &Keys, a: Action) -> f32 {
        self.pad_of(a).map_or(0., |b| b.value(&keys.pad))
    }
    /// Konflikte: Paare von Aktionen mit derselben Taste in überlappenden Bereichen (für die Anzeige).
    pub fn conflicts(&self) -> Vec<(Action, Action)> {
        let mut out = Vec::new();
        for (i, a) in ACTIONS.iter().enumerate() {
            for b in &ACTIONS[i + 1..] {
                if !a.context.overlaps(b.context) || compatible(a.action, b.action) {
                    continue;
                }
                let ka = self.keys_of(a.action);
                let kb = self.keys_of(b.action);
                let key = ka
                    .iter()
                    .flatten()
                    .any(|k| kb.iter().flatten().any(|o| o == k));
                let pad = self.pad_of(a.action).is_some()
                    && self.pad_of(a.action) == self.pad_of(b.action);
                if key || pad {
                    out.push((a.action, b.action));
                }
            }
        }
        out
    }
    #[cfg(test)]
    pub fn in_conflict(&self, a: Action) -> bool {
        self.conflicts().iter().any(|&(x, y)| x == a || y == a)
    }
    /// Nur die Abweichungen vom Standard als JSON (`settings.json` → `"bindings"`).
    pub fn to_json(&self) -> serde_json::Value {
        let d = Bindings::default();
        let mut keys = serde_json::Map::new();
        let mut pad = serde_json::Map::new();
        for i in ACTIONS {
            let name = format!("{:?}", i.action);
            let k = self.keys_of(i.action);
            if k != d.keys_of(i.action) {
                keys.insert(
                    name.clone(),
                    serde_json::json!(k.iter().map(|k| k.and_then(key_id)).collect::<Vec<_>>()),
                );
            }
            if self.pad_of(i.action) != d.pad_of(i.action) {
                pad.insert(
                    name,
                    serde_json::json!(self.pad_of(i.action).map(PadButton::key)),
                );
            }
        }
        serde_json::json!({ "keys": keys, "pad": pad, "rumble": self.rumble, "steer": self.steer_sens })
    }
    /// Aus JSON; Unbekanntes wird übergangen (fehlende Aktionen behalten den Standard).
    pub fn from_json(v: &serde_json::Value) -> Self {
        let mut b = Bindings::default();
        for i in ACTIONS {
            let name = format!("{:?}", i.action);
            if let Some(arr) = v["keys"][&name].as_array() {
                let k: Vec<Option<KeyCode>> =
                    arr.iter().map(|s| s.as_str().and_then(key_from)).collect();
                b.keys.insert(
                    i.action,
                    [k.first().copied().flatten(), k.get(1).copied().flatten()],
                );
            }
            if let Some(p) = v["pad"].get(&name) {
                b.set_pad(i.action, p.as_str().and_then(PadButton::from_key));
            }
        }
        if let Some(r) = v["rumble"].as_bool() {
            b.rumble = r;
        }
        if let Some(s) = v["steer"].as_f64() {
            b.steer_sens = (s as f32).clamp(0.5, 1.5);
        }
        b
    }
    /// Erste neu gedrückte Controller-Taste (für das Belegen).
    pub fn first_pad_press(e: &Pad) -> Option<PadButton> {
        PAD_BUTTONS.iter().copied().find(|b| b.pressed(e))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn both_shoulders_open_the_console_without_side_effects() {
        use super::ShoulderChord;
        let dt = 1. / 60.;
        // gleichzeitig: Befehlszeile, kein LB/RB-Druck – auch nicht später
        let mut c = ShoulderChord::default();
        assert_eq!(c.step(true, true, true, true, dt), (true, false, false));
        for _ in 0..20 {
            assert_eq!(c.step(true, true, false, false, dt), (false, false, false));
        }
        assert_eq!(
            c.step(false, false, false, false, dt),
            (false, false, false)
        );
        // knapp nacheinander (zwei Schritte): ebenfalls Befehlszeile
        let mut c = ShoulderChord::default();
        assert_eq!(c.step(true, false, true, false, dt), (false, false, false));
        assert_eq!(c.step(true, true, false, true, dt), (true, false, false));
        // einzeln gehalten: nach dem Fenster genau ein LB-Druck
        let mut c = ShoulderChord::default();
        let mut lb = 0;
        for i in 0..20 {
            let o = c.step(true, false, i == 0, false, dt);
            assert!(!o.0 && !o.2);
            lb += o.1 as u32;
        }
        assert_eq!(lb, 1);
        // kurz getippt (Druck und Loslassen zwischen zwei Schritten): ein RB-Druck im nächsten Schritt
        let mut c = ShoulderChord::default();
        assert_eq!(c.step(false, false, false, true, dt), (false, false, false));
        assert_eq!(c.step(false, false, false, false, dt), (false, false, true));
    }
    #[test]
    fn long_horn_switches_the_siren_once() {
        use super::{HORN_TAP, HornPress};
        let dt = 1. / 60.;
        // normales Auto: Hupe, solange gehalten
        let mut h = HornPress::default();
        for _ in 0..60 {
            assert_eq!(h.step(true, false, dt), (true, false));
        }
        assert_eq!(h.step(false, false, dt), (false, false));
        // mit Sirene: gehalten hupt nie, schaltet einmal die Sirene, danach still bis zum Loslassen
        let mut h = HornPress::default();
        let (mut toggles, mut horn) = (0, 0);
        for _ in 0..60 {
            let (a, b) = h.step(true, true, dt);
            horn += a as u32;
            toggles += b as u32;
        }
        assert_eq!((toggles, horn), (1, 0));
        let after: u32 = (0..30).map(|_| h.step(false, true, dt).0 as u32).sum();
        assert_eq!(after, 0, "nach der Sirene kein Hupen beim Loslassen");
        // kurzer Druck: erst beim Loslassen ein kurzer Hupton, keine Sirene
        let mut h = HornPress::default();
        let mut toggles = 0;
        for _ in 0..10 {
            let (a, b) = h.step(true, true, dt);
            assert!(!a, "nicht sofort hupen");
            toggles += b as u32;
        }
        assert_eq!(toggles, 0);
        let honk: u32 = (0..60).map(|_| h.step(false, true, dt).0 as u32).sum();
        assert!(
            (honk as f64 * dt - HORN_TAP).abs() < 2. * dt,
            "kurzer Hupton: {honk}"
        );
    }
    #[test]
    fn keyboard_pedal_ramps_up_and_down() {
        use super::{PEDAL_DOWN, PEDAL_UP, pedal_ramp};
        let dt = 1. / 60.;
        let mut p = 0.;
        let mut n = 0;
        while p < 1. {
            p = pedal_ramp(p, 1., dt);
            n += 1;
        }
        assert!((n as f64 * dt - PEDAL_UP).abs() < dt * 1.5, "{n}");
        let mut n = 0;
        while p > 0. {
            p = pedal_ramp(p, 0., dt);
            n += 1;
        }
        assert!((n as f64 * dt - PEDAL_DOWN).abs() < dt * 1.5, "{n}");
    }
    use super::*;
    use std::collections::HashSet;

    fn keys<'a>(
        held: &'a HashSet<KeyCode>,
        pressed: &'a HashSet<KeyCode>,
        pad: Pad,
        edges: Pad,
    ) -> Keys<'a> {
        Keys {
            held,
            pressed,
            pad,
            pad_pressed: edges,
            pad2: Default::default(),
            pad2_pressed: Default::default(),
            mouse: Default::default(),
            typed: "",
        }
    }

    #[test]
    fn defaults_match_the_old_fixed_layout_and_have_no_conflicts() {
        let b = Bindings::default();
        assert_eq!(b.keys_of(A::Throttle)[0], Some(KeyCode::KeyW));
        assert_eq!(b.pad_of(A::Throttle), Some(P::RT));
        assert_eq!(b.pad_of(A::Up), None, "Laufen am Stick ist fest");
        assert!(b.conflicts().is_empty(), "{:?}", b.conflicts());
        // jede Aktion genau einmal
        let set: HashSet<_> = ACTIONS.iter().map(|i| i.action).collect();
        assert_eq!(set.len(), ACTIONS.len());
        // Ziffern 1–6 sind fest
        assert!(!bindable_key(KeyCode::Digit1));
    }

    #[test]
    fn rebinding_conflicts_and_json_roundtrip() {
        let mut b = Bindings::default();
        b.set_key(A::Horn, 0, Some(KeyCode::KeyF));
        assert!(
            b.in_conflict(A::Horn) && b.in_conflict(A::EnterExit),
            "F doppelt im Auto"
        );
        // gleiche Taste in getrennten Bereichen ist erlaubt (Hupe im Auto, Treten zu Fuß)
        b.set_key(A::Horn, 0, Some(KeyCode::KeyV));
        assert!(!b.in_conflict(A::Horn));
        b.set_pad(A::Esp, Some(P::LS));
        b.set_pad(A::Up, Some(P::A));
        assert_eq!(b.pad_of(A::Up), None, "Stick-Aktionen bleiben fest");
        b.rumble = false;
        b.steer_sens = 0.7;
        let j = b.to_json();
        assert!(j["keys"].get("Throttle").is_none(), "nur Abweichungen");
        assert_eq!(Bindings::from_json(&j), b);
        // kaputte Einträge: Standard bleibt
        let k =
            Bindings::from_json(&serde_json::json!({"keys": {"Horn": ["Quatsch"]}, "steer": 9}));
        assert_eq!(k.keys_of(A::Horn), [None, None]);
        assert_eq!(k.steer_sens, 1.5);
        assert_eq!(
            k.keys_of(A::Throttle),
            Bindings::default().keys_of(A::Throttle)
        );
    }

    #[test]
    fn queries_follow_the_binding() {
        let mut b = Bindings::default();
        let held: HashSet<KeyCode> = [KeyCode::KeyJ].into();
        let none = HashSet::new();
        let k = keys(&held, &none, Pad::default(), Pad::default());
        assert!(!b.held(&k, A::Horn));
        b.set_key(A::Horn, 1, Some(KeyCode::KeyJ));
        assert!(b.held(&k, A::Horn));
        let pad = Pad {
            rt: 0.4,
            ..Default::default()
        };
        let k = keys(&none, &none, pad, Pad::default());
        assert!((b.pad_value(&k, A::Throttle) - 0.4).abs() < 1e-6);
        assert!(
            !b.held(&k, A::Throttle),
            "Trigger zählt als Taste erst ab halbem Weg"
        );
        b.set_pad(A::Throttle, Some(P::A));
        assert_eq!(b.pad_value(&k, A::Throttle), 0.);
    }

    #[test]
    fn trigger_curve_is_progressive_with_deadzone_and_full_end() {
        assert_eq!(trigger_curve(0.03, THROTTLE_GAMMA), 0., "Ruherauschen");
        assert_eq!(trigger_curve(1., THROTTLE_GAMMA), 1.);
        assert_eq!(
            trigger_curve(0.98, BRAKE_GAMMA),
            1.,
            "kurz vor dem Anschlag voll"
        );
        let mid = trigger_curve(0.5, THROTTLE_GAMMA);
        assert!(mid > 0.3 && mid < 0.5, "Halbgas feiner als linear: {mid}");
        assert!(trigger_curve(0.5, BRAKE_GAMMA) < mid, "Bremse noch sanfter");
        let mut last = 0.;
        for i in 0..=100 {
            let v = trigger_curve(i as f32 / 100., THROTTLE_GAMMA);
            assert!(v >= last);
            last = v;
        }
    }

    #[test]
    fn steering_has_a_fine_centre_and_full_lock() {
        assert_eq!(steer_curve(0.08, 1.), 0.);
        assert!((steer_curve(1., 1.) - 1.).abs() < 1e-6);
        assert!((steer_curve(-1., 0.5) + 1.).abs() < 1e-6);
        let fine = steer_curve(0.3, 1.);
        assert!(fine > 0. && fine < 0.2, "feine Mitte: {fine}");
        assert!(
            steer_curve(0.3, 0.5) < fine && steer_curve(0.3, 1.5) > fine,
            "Empfindlichkeit wirkt"
        );
    }
}
