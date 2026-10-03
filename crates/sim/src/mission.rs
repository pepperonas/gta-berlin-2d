//! Mission „Kisten für den Kiez“ (Port von `mission.js`) als reine Zustandsmaschine:
//! available → briefing → to_pickup → to_dropoff → success, oder failed (Zeit abgelaufen / Ware zerstört).
use crate::car::Car;
use crate::carmodels::is_open_kind;
use crate::city::{Place, Places};
use crate::events::Event;

pub const TIME_LIMIT: f64 = 120.;
pub const LOAD_TIME: f64 = 2.;
pub const REWARD: f64 = 500.;
pub const TIME_BONUS: f64 = 5.;
pub const ZONE_RADIUS: f64 = 46.;
pub const STOP_SPEED: f64 = 35.;
pub const GIVER_RADIUS: f64 = 30.;

pub const BRIEFING: [&str; 4] = [
    "Kalle vom Späti „Zum Kiez“:",
    "„Mein Lieferant hat mich hängen lassen. In der Lagerhalle in Neukölln",
    "stehen meine Kisten. Hol sie ab und bring sie her – aber zackig,",
    "um acht kommen die Stammgäste. Und fahr mir die Ware nicht zu Schrott!“",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Available,
    Briefing,
    ToPickup,
    ToDropoff,
    Success,
    Failed,
}
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Success {
        time: f64,
        reward: f64,
        bonus: f64,
        damage_penalty: f64,
        health: f64,
        new_best: bool,
    },
    Failed {
        reason: String,
    },
}
#[derive(Debug, Clone, PartialEq)]
pub struct Mission {
    pub state: State,
    pub timer: f64,
    pub time_limit: f64,
    pub elapsed: f64,
    pub load: f64,
    pub cargo_car: Option<u32>,
    pub result: Option<Outcome>,
    pub prompt: Option<&'static str>,
}
impl Default for Mission {
    fn default() -> Self {
        Self {
            state: State::Available,
            timer: 0.,
            time_limit: 0.,
            elapsed: 0.,
            load: 0.,
            cargo_car: None,
            result: None,
            prompt: None,
        }
    }
}

/// Spieler aus Sicht der Mission.
#[derive(Debug, Clone, Copy)]
pub struct PlayerView {
    pub x: f64,
    pub y: f64,
    pub in_car: Option<u32>,
}
#[derive(Debug, Clone, Copy, Default)]
pub struct MissionInput {
    /// Taste gedrückt (Flanke)
    pub action: bool,
    /// Taste gehalten
    pub action_held: bool,
}

fn in_zone(x: f64, y: f64, p: &Place, r: f64) -> bool {
    (x - p.x).hypot(y - p.y) <= r
}

impl Mission {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn update(
        &mut self,
        places: &Places,
        player: PlayerView,
        cars: &mut [Car],
        time_limit: Option<f64>,
        input: MissionInput,
        dt: f64,
    ) -> Vec<Event> {
        let mut ev = Vec::new();
        let veh = player
            .in_car
            .and_then(|id| cars.iter().position(|c| c.id == id));
        let car = veh.filter(|&i| !is_open_kind(cars[i].kind)); // auf Rad und Motorrad passen keine Kisten
        self.prompt = None;
        match self.state {
            State::Available => {
                if veh.is_none() && in_zone(player.x, player.y, &places.giver, GIVER_RADIUS) {
                    self.prompt = Some("A: Auftrag annehmen");
                    if input.action {
                        self.state = State::Briefing;
                        ev.push(Event::Ui);
                    }
                }
            }
            State::Briefing => {
                if input.action {
                    self.state = State::ToPickup;
                    self.timer = time_limit.unwrap_or(TIME_LIMIT);
                    self.time_limit = self.timer;
                    self.elapsed = 0.;
                    ev.push(Event::MissionStart);
                }
            }
            State::ToPickup => {
                self.tick(dt, &mut ev);
                if self.state != State::ToPickup {
                    return ev;
                }
                if in_zone(player.x, player.y, &places.pickup, ZONE_RADIUS) {
                    match car {
                        None => self.prompt = Some("Du brauchst ein Auto für die Kisten"),
                        Some(i) if cars[i].wrecked => self.prompt = Some("Dieses Auto ist Schrott"),
                        Some(i) if cars[i].speed() > STOP_SPEED => {
                            self.prompt = Some("Anhalten zum Einladen")
                        }
                        Some(i) => {
                            self.prompt = Some("A gedrückt halten: Kisten einladen");
                            if input.action_held {
                                self.load += dt;
                                if self.load >= LOAD_TIME {
                                    cars[i].cargo = true;
                                    self.cargo_car = Some(cars[i].id);
                                    self.state = State::ToDropoff;
                                    self.load = 0.;
                                    ev.push(Event::Pickup);
                                }
                            } else {
                                self.load = (self.load - dt * 2.).max(0.);
                            }
                        }
                    }
                } else {
                    self.load = 0.;
                }
            }
            State::ToDropoff => {
                let cargo = self
                    .cargo_car
                    .and_then(|id| cars.iter().position(|c| c.id == id));
                let Some(ci) = cargo.filter(|&i| !cars[i].wrecked) else {
                    self.fail("Die Ware ist hinüber – das Auto ist Schrott.", &mut ev);
                    return ev;
                };
                self.tick(dt, &mut ev);
                if self.state != State::ToDropoff {
                    return ev;
                }
                if car == Some(ci) && in_zone(cars[ci].x, cars[ci].y, &places.dropoff, ZONE_RADIUS)
                {
                    if cars[ci].speed() > STOP_SPEED {
                        self.prompt = Some("Anhalten zum Abliefern");
                    } else {
                        self.prompt = Some("A: Kisten abliefern");
                        if input.action {
                            self.succeed(&mut cars[ci], &mut ev);
                        }
                    }
                }
            }
            State::Success | State::Failed => {}
        }
        ev
    }
    fn tick(&mut self, dt: f64, ev: &mut Vec<Event>) {
        self.elapsed += dt;
        let before = self.timer.ceil();
        self.timer = (self.timer - dt).max(0.);
        if self.timer <= 10. && self.timer.ceil() != before && self.timer > 0. {
            ev.push(Event::Tick);
        }
        if self.timer <= 0. {
            self.fail(
                "Zeit abgelaufen – die Stammgäste sitzen auf dem Trockenen.",
                ev,
            );
        }
    }
    pub fn fail(&mut self, reason: &str, ev: &mut Vec<Event>) {
        self.state = State::Failed;
        self.result = Some(Outcome::Failed {
            reason: reason.to_owned(),
        });
        ev.push(Event::MissionFail);
    }
    fn succeed(&mut self, car: &mut Car, ev: &mut Vec<Event>) {
        // Zeitbonus auf das 120-s-Referenzlimit normiert, damit lange Routen nicht mehr Bonus bringen.
        let limit = if self.time_limit > 0. {
            self.time_limit
        } else {
            TIME_LIMIT
        };
        let bonus = (self.timer / limit * 120. * TIME_BONUS).round();
        let damage_penalty = ((100. - car.health) * 2.).round();
        let reward = (REWARD + bonus - damage_penalty).max(100.);
        car.cargo = false;
        self.state = State::Success;
        self.result = Some(Outcome::Success {
            time: self.elapsed,
            reward,
            bonus,
            damage_penalty,
            health: car.health,
            new_best: false,
        });
        ev.push(Event::MissionSuccess);
    }
    /// Ziel für Pfeil/Minikarte und Text fürs HUD.
    pub fn objective(
        &self,
        places: &Places,
        player: PlayerView,
        cars: &[Car],
    ) -> (&'static str, Option<(f64, f64)>) {
        match self.state {
            State::Available => (
                "Kalle am Späti wartet auf dich",
                Some((places.giver.x, places.giver.y)),
            ),
            State::ToPickup => (
                if player.in_car.is_some() {
                    "Fahr zur Lagerhalle in Neukölln"
                } else {
                    "Besorg dir ein Auto und fahr zur Lagerhalle"
                },
                Some((places.pickup.x, places.pickup.y)),
            ),
            State::ToDropoff => match self
                .cargo_car
                .and_then(|id| cars.iter().find(|c| c.id == id))
            {
                Some(c) if player.in_car != Some(c.id) => {
                    ("Zurück zum Wagen mit den Kisten", Some((c.x, c.y)))
                }
                _ => (
                    "Bring die Kisten zum Parkplatz am Späti",
                    Some((places.dropoff.x, places.dropoff.y)),
                ),
            },
            _ => ("", None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::car::Role;

    fn places() -> Places {
        let p = |x: f64, y: f64| Place { x, y, angle: 0. };
        Places {
            giver: p(0., 0.),
            player_spawn: p(0., 10.),
            dropoff: p(100., 0.),
            player_car: p(50., 0.),
            pickup: p(1000., 0.),
            ..Default::default()
        }
    }
    fn press() -> MissionInput {
        MissionInput {
            action: true,
            action_held: true,
        }
    }

    #[test]
    fn full_run_pays_reward_with_bonus_and_damage() {
        let pl = places();
        let mut m = Mission::default();
        let mut cars = vec![Car::new(1, 50., 0., 0., 0, Role::Player, "car")];
        let on_foot = PlayerView {
            x: 5.,
            y: 0.,
            in_car: None,
        };
        assert_eq!(
            m.update(
                &pl,
                on_foot,
                &mut cars,
                Some(200.),
                MissionInput::default(),
                0.1
            ),
            vec![]
        );
        assert_eq!(m.prompt, Some("A: Auftrag annehmen"));
        m.update(&pl, on_foot, &mut cars, Some(200.), press(), 0.1);
        assert_eq!(m.state, State::Briefing);
        let ev = m.update(&pl, on_foot, &mut cars, Some(200.), press(), 0.1);
        assert_eq!((m.state, m.timer), (State::ToPickup, 200.));
        assert_eq!(ev, vec![Event::MissionStart]);
        // zu Fuß an der Lagerhalle: braucht ein Auto
        let at_pickup = PlayerView {
            x: 1000.,
            y: 0.,
            in_car: None,
        };
        m.update(&pl, at_pickup, &mut cars, None, press(), 0.1);
        assert_eq!(m.prompt, Some("Du brauchst ein Auto für die Kisten"));
        cars[0].x = 1000.;
        let driving = PlayerView {
            x: 1000.,
            y: 0.,
            in_car: Some(1),
        };
        cars[0].vx = 100.;
        m.update(&pl, driving, &mut cars, None, press(), 0.1);
        assert_eq!(m.prompt, Some("Anhalten zum Einladen"));
        cars[0].vx = 0.;
        let mut ev = Vec::new();
        for _ in 0..21 {
            ev.extend(m.update(&pl, driving, &mut cars, None, press(), 0.1));
        }
        assert_eq!(m.state, State::ToDropoff);
        assert!(cars[0].cargo && ev.contains(&Event::Pickup));
        cars[0].x = 100.;
        cars[0].health = 80.;
        let at_drop = PlayerView {
            x: 100.,
            y: 0.,
            in_car: Some(1),
        };
        m.update(&pl, at_drop, &mut cars, None, press(), 0.1);
        assert_eq!(m.state, State::Success);
        let Some(Outcome::Success {
            reward,
            bonus,
            damage_penalty,
            ..
        }) = m.result.clone()
        else {
            panic!()
        };
        let expected_bonus = (m.timer / 200. * 120. * 5.).round();
        assert_eq!(
            (bonus, damage_penalty, reward),
            (expected_bonus, 40., 500. + expected_bonus - 40.)
        );
        assert!(!cars[0].cargo);
    }

    #[test]
    fn fails_on_timeout_and_wreck() {
        let pl = places();
        let mut m = Mission {
            state: State::ToPickup,
            timer: 0.5,
            time_limit: 120.,
            ..Default::default()
        };
        let mut cars = vec![Car::new(1, 50., 0., 0., 0, Role::Player, "car")];
        let pv = PlayerView {
            x: 500.,
            y: 0.,
            in_car: None,
        };
        let ev = m.update(&pl, pv, &mut cars, None, MissionInput::default(), 1.);
        assert_eq!(m.state, State::Failed);
        assert!(ev.contains(&Event::MissionFail));
        let mut m = Mission {
            state: State::ToDropoff,
            timer: 50.,
            cargo_car: Some(1),
            ..Default::default()
        };
        cars[0].wrecked = true;
        m.update(&pl, pv, &mut cars, None, MissionInput::default(), 0.1);
        assert_eq!(
            m.result,
            Some(Outcome::Failed {
                reason: "Die Ware ist hinüber – das Auto ist Schrott.".into()
            })
        );
    }

    #[test]
    fn countdown_ticks_in_last_ten_seconds() {
        let pl = places();
        let mut m = Mission {
            state: State::ToPickup,
            timer: 12.,
            time_limit: 120.,
            ..Default::default()
        };
        let mut cars = vec![];
        let pv = PlayerView {
            x: 500.,
            y: 0.,
            in_car: None,
        };
        let mut ticks = 0;
        // exakt darstellbare Schritte: je volle Sekunde unter 10 ein Ticken (JS-Referenz: 10)
        for _ in 0..60 {
            ticks += m
                .update(&pl, pv, &mut cars, None, MissionInput::default(), 0.25)
                .iter()
                .filter(|e| **e == Event::Tick)
                .count();
        }
        assert_eq!(ticks, 10);
        assert_eq!(m.state, State::Failed);
        // auf dem Motorrad passen keine Kisten
        let mut m = Mission {
            state: State::ToPickup,
            timer: 50.,
            ..Default::default()
        };
        let mut bikes = vec![Car::new(2, 1000., 0., 0., 0, Role::Traffic, "motorcycle")];
        m.update(
            &pl,
            PlayerView {
                x: 1000.,
                y: 0.,
                in_car: Some(2),
            },
            &mut bikes,
            None,
            press(),
            0.1,
        );
        assert_eq!(m.prompt, Some("Du brauchst ein Auto für die Kisten"));
    }
}
