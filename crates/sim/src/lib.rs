//! Simulation von GTA Berlin (Phase 3 der nativen Portierung): DOM- und GPU-freie, deterministische Spiellogik.
//! Port der JS-Module `collision.js`, `grid.js`, `car.js`, `dynamics.js`, `carmodels.js`, `traction.js`,
//! `levels.js`, `roadgraph.js`, `traffic.js`, `pedestrians.js`, `mission.js`, `save.js` und der Kernschleife von
//! `world.js`.
pub mod car;
pub mod carmodels;
pub mod city;
pub mod collision;
pub mod daylight;
pub mod dynamics;
pub mod events;
pub mod lamps;
pub mod levels;
pub mod math;
pub mod mission;
pub mod pedestrians;
pub mod roadgraph;
pub mod save;
pub mod traction;
pub mod traffic;
pub mod world;
