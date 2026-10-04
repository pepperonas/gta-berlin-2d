//! Simulation von GTA Berlin (Phase 3 der nativen Portierung): DOM- und GPU-freie, deterministische Spiellogik.
//! Port der JS-Module `collision.js`, `grid.js`, `car.js`, `dynamics.js`, `carmodels.js`, `traction.js`,
//! `levels.js`, `roadgraph.js`, `traffic.js`, `pedestrians.js`, `mission.js`, `save.js` und der Kernschleife von
//! `world.js`.
pub mod ambience;
pub mod animals;
pub mod bikes;
pub mod calibrate;
pub mod car;
pub mod carmodels;
pub mod city;
pub mod collision;
pub mod combat;
pub mod daylight;
pub mod dynamics;
pub mod enginevoice;
pub mod events;
pub mod figure;
pub mod fleet;
pub mod footpath;
pub mod lamps;
pub mod levels;
pub mod life;
pub mod math;
pub mod mission;
pub mod nightlife;
pub mod pedestrians;
pub mod railsound;
pub mod rhythm;
pub mod ride;
pub mod roadgraph;
pub mod routing;
pub mod save;
pub mod services;
pub mod soundscape;
pub mod station;
pub mod stationlevels;
pub mod stats;
pub mod surface;
pub mod traction;
pub mod traffic;
pub mod transit;
pub mod transitlive;
pub mod tunnel;
pub mod vehdata;
pub mod vphys;
pub mod weather;
pub mod world;
