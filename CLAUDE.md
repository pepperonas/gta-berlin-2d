# CLAUDE.md

## Native migration (phase 5)

The Rust 2024 workspace is now the native migration target. `cargo run` starts
the playable `crates/game` (`--free` = old map viewer); `crates/engine` owns winit/wgpu, camera and pacing
plus the `Game` trait (fixed step, keys, instanced `Body` quads for cars/people/markers);
`crates/sim` (`berlin-sim`) is the DOM/GPU-free deterministic simulation ported from the JS reference:
`collision.rs` (SAT, `SpatialHash`, `Grid`), `city.rs` (own sim decoder of v3 tiles with refcounted
streaming, surfaces, walls, levels data; `DiskSource`/`ThreadedSource`), `levels.rs`, `car.rs` + `dynamics.rs`
+ `carmodels.rs` + `traction.rs`, `roadgraph.rs` (lanes, signals), `traffic.rs` (AI + reservations kept in
`Reservations`, AI reads an `Agent` snapshot), `pedestrians.rs`, `mission.rs`, `save.rs` (JS-compatible JSON
file, atomic write), `world.rs`. Positions are f64. Integration tests on real tiles: `crates/sim/tests/world.rs`;
`cargo run --release -- --check-sim 120` runs it headless. Bar feed over HTTP only on request (`game/barfeed.rs`, `--bars live|URL`). Person kinds: `sim/figure.rs` (`Ped.kind/style`, speed only), look in `game/figure.rs`; torso/head/hair/hats/bags/stroller/dog are painted parts from `game/figart.rs` in 64-px subcells of an extra vehicle-atlas pair (`carart::figure_pair`, body shape `vehatlas::FIG_BASE + pair·32 + part`, size via `figart::half`). Transit: `sim/transit.rs`, `transitlive.rs`, `tunnel.rs`, `ride.rs` (riding/driving), `station.rs` (walkable stations: every S/U stop, grouped per name into a complex with `transfers`; `level` = real level from `stationlevels.rs` ← `data/station-levels.json` (researched, sourced) or estimated, `lvl` = player map level, `open_air` platforms drawn over the visible city; use `World::in_tunnel_station()` where "inside" used to mean underground), drawn by `game/underground.rs`. Throwables: `sim/throw.rs` (`World::thrown`/`flames`, grenade → `World::blast` from `fire.rs`, which also serves wreck explosions; `Weapon::throw`, the thrower's seat via `World::seat_index` spares the coop partner), drawn in `game/firefx.rs` from flipbooks (CC0 Unity Labs; `tools/gfx/build_vfx.py` → `data/gfx/vfx/` atlas + manifest → `engine/vfx.rs`: shader constants, premultiplied linear sRGB texture in body group 2 bindings 5/6; an effect body with negative `shape` = `vfx::shape(seq, frame)` is a flipbook frame, drawn only by `effect_fs` in the effects pass, which blends premultiplied; never write decimal literals like `64.0` into generated WGSL – a test forbids them, use `f32(n)`); fire sound = ambience loop `amb_fire` via `ambience::fire_level`. Car radio: stations in `data/radio.json`, streaming/decoding in `audio/radio.rs` (only after `Synth::enable_radio`, i.e. live output – never in tests/captures/WAV), per-car station logic in `game/radio.rs`; every new `AmbLoops` layer must also be added to the sum in `Synth::render`. Nightlife: `sim/nightlife.rs` (feed from file `web/data/bars.json` by default). Console: `game/console.rs` (pure; text via `Keys::typed`). City life: `sim/rhythm.rs` (population targets), `sim/life.rs` (life spots, ped state `Hang`), `sim/animals.rs`, parked e-scooters (`bikes.rs parked_scooters`); all gated by `World::day_rhythm` (default population only). Phase 4 lighting: `sim/daylight.rs` + `sim/lamps.rs`
(pure, tested), `map_loader` emits per-building `ShadowVertex` wall quads, `engine/lightpass.rs` + `lighting.wgsl`
render a shadow mask (vertex extrusion along the sun, tree crowns from the atlas) and a half-res lightmap, then
apply both with a fullscreen triangle at depth 0.5 (ground/cars behind, roofs/crowns in front get ambient only).
Lit windows: `scene.wgsl window_fs`, a second pass over the tile meshes after the light composites (depth LessEqual, no write; `params.z` = windows lit, `sun.w` = clock minutes). Silhouettes: `Game::silhouettes` bodies drawn with depth `Greater`, no write, after the light, by `scene.wgsl silhouette_fs` (outline in screen px + dark inner band + faint fill; cars use their atlas sprite shape; color = line color, alpha = strength, `SIL_OWN`/`SIL_OTHER`). Body pipelines use `body_layout` (group 2 = HUD font atlas binding 4 for world text, group 3 = lightmap `aux_tex`, set via `body_groups`); body shape 8 = SDF glyph (color = atlas rect px, ink fixed), 9 = triangle; direction signs are world bodies (`streetfurn::sign_bodies`), `lamp_glint` reflects the lightmap in glossy paint at night. Translucent effects (tyre smoke/spray, tracers, muzzle flashes): `Game::effects`, own pass after the silhouettes, depth test but no write — never put translucent bodies in `bodies()` in front of cars (they count as occluders and light up the car's silhouette); they darken themselves with the ambient light. Grade + vignette: `lighting.wgsl grade_fs` (multiply). Graphics mode: `engine/graphics.rs` (`GraphicsMode` Hd/Pixel, `Quality`, `msaa()`) via `Game::graphics()`, saved in settings.json (`grafik`, `qualitaet`), F8 / menu / console `grafik`; scene renders into `Rgba16Float` + MSAA (`engine/scenepass.rs` `ScenePipes`/`SceneTargets`, composites from `lightpass::composites`), then bloom (`scenepass::Bloom`, targets ½/¼, post group 3) and `post_fs` (bloom + grade + AgX tonemap) + HUD/minimap (own 1× depth, `map_pipeline`) into the output; `GraphicsSettings::post_level` (0 Niedrig/Pixel, 1 Mittel, 2 Hoch) goes to `camera.padding2.y` and gates bloom, shadow filtering and the HDR lightmap cap; pixel mode: scene (and shadow mask/lightmap) at size / `graphics::pixel_factor`, own `scene_uniform` (scale / k, position snapped, `padding2.y = −1` so `scene.wgsl surface_detail()` flattens textures/grime to `PIXEL_DETAIL`), `pixel_quant_fs` (grade, saturation/contrast boost, depth outline in darkened own colour, Bayer only in true gradients, 32³ palette LUT from `engine/palette.rs` + `data/gfx/palette.json`) into `SceneTargets.pix`, `pixel_post_fs` upscales by an integer with letterbox; HUD/post keep the full-size `bind`; smoke/capture runs advance exactly one sim step per frame. App icon: `tools/gfx/build_icon.py` generates every icon size (window icon via `engine::app_icon`, `.icns` for `tools/macos-app.sh`, Xbox logos, favicon) – never edit them by hand. Junction plates: `tools/osm/plates.mjs` (osm2streets-style corners, node groups < `MERGE_M`) → tile layer `plates` + edge field 12 trims → `format.rs Feature::Plate`/`Road::trim`, `mesh.rs plate_mesh` (render only; the sim keeps the round discs; `trim_polyline` empty = road dropped inside a group). Ground materials: `tools/gfx/build_materials.py` (ambientCG CC0, sha256-checked) → `data/gfx/materials/` (+manifest, credits in about.rs) and `data/gfx/material_map.json` (material id → texture params; id 14 = background `ground_fs`), loaded by `engine/materials.rs` as bind group 2 of the tile pipelines; sample only via `ground_sample`/`textureSampleGrad` (derivatives before branches); never name a WGSL identifier `patch`. Phase 3: curbs/fringe/markings in `map_loader` (`mesh.rs curb`/`fringe`, `format.rs markings` → `Feature::Marks` from tile `crossings`/`signals`/`vertices.trim`); decal atlas 256 px × 4×4 with mips, grid only via `atlas::shader_constants`/`atlas_uv` (no literals in WGSL); tree crowns (cells `atlas::CROWNS`: 0, 6, 12 Linde, 13 Platane, 14 Kastanie, 15 Kiefer, picked by `mesh.rs tree_look`) store R = unlit shade, G/B = normal and are lit by `sprite_fs crown_light`, test crown cells with WGSL `is_crown`; interpolated varyings only — `center` is flat. Phase 4: facades by `mesh.rs facade_material` (plaster 11/12, brick 18/19, concrete 20/23; door 13, shop window 21, sign 22 with `uv.x` = local offset), one window grid in `engine/facade.rs` (WGSL constants; use `window_cell`/`in_glass`/`glass_bar` in both `fs` and `window_fs`). Xbox probe: `crates/xbox_probe` (cdylib `berlin_probe`, C API in `ffi.rs`, wgpu DX12 into a XAML `SwapChainPanel`, same workload as `--example mac`) + UWP host `xbox/RustProbe` (build DLL with `build-probe.ps1`); runs on the Series X (findings in `docs/NATIVE-RUST.md` „Ergebnis auf der Konsole“). On the Xbox: render on an own thread (not the UI thread), never resize the swapchain (`ResizeBuffers` kills the surface), no `TIMESTAMP_QUERY` (driver loses the device), UWP `ISwapChainPanelNative` GUID is `F92F19D2-…` (not WinUI 3's `63aad0b8-…`). Graphics QA: `--fenster BxH` (window shows a scaled preview) (fixed offscreen render size), `--messung x.json` (CPU + GPU timestamp median/P95, `engine/gputime.rs`), `--geo`/`--zoom` also in game; scenes via `tools/gfx/captures.sh PHASE` → `docs/images/native/grafik/` (HD/pixel plan: `docs/superpowers/plans/2026-10-05-grafik-hd-pixel.md`). Vehicle sprites: `game/raster.rs` + `game/carart.rs` paint a two-layer atlas at startup (paint shade + fixed details), bodies with `shape >= 16` sample it (`scene.wgsl vehicle`/`veh_uv`); cell grid 512×256 from `engine/vehatlas.rs` only (shader derives cell size from atlas width), paint cell G = gloss, B = material (0 paint, ½ glass, 1 chrome) set via `Art::material`/`glass_bx`; `body_gloss` adds sun glint + sky by `BodyOut.rot`; body shapes 6/7 = glossy ellipse (paint/chrome, motorcycles). Each passenger model has its own body form (`carart::proportions` → `Prop`, `paint_car`), size from `carmodels::body_dims` (data, width clamped 1.5–2.0 m; sprite = collision box, set via `Car::set_model`), axles/wheels from `carart::axles`/`wheels`, paint from `carmodels::paint_for` (id hash, no world RNG). Lineup check: `--bildschirm autos`; atlas dump: `GTA_ATLAS_DUMP=x.ppm cargo test -p gta-berlin dump_atlas -- --ignored`. Motorcycles: `motoart::WIDE` widens the drawing laterally. Never name a WGSL identifier `half` (Naga passes it to Metal, where it is a type). Phase 5 audio: pure mixing
rules in `sim/{enginevoice,soundscape,ambience}.rs`; `crates/audio` (`berlin-audio`) has Web-Audio-like DSP
(`dsp.rs`), the synth graph of `audio.js` (`synth.rs`, driven by a per-step `Frame`), cpal output and WAV writer
(`output.rs`); gun shots play CC0 samples (`data/audio/weapons/`, built by `tools/audio/build_weapon_sounds.py`, `sampler.rs weapon_bank`); weapon visuals (flash, smoke, travelling streak, casings, impacts, bullet holes) live in `game/gunfx.rs`, owned by `effects.rs`; other sfx: recipes `tools/audio/sfx_recipes.json` → `tools/audio/build_sfx.py` → `data/audio/sfx/` (file list generated by `crates/audio/build.rs`), played via `Synth::sample(SfxSpec)` as a guard arm before the synth fallback (`GTA_SFX_SAMPLES=0` = synth), credits read from the manifest by `about.rs`; `game/sound.rs` builds frames. Rail sound: pure `sim/railsound.rs` (layers from speed/accel/tunnel, joints per axle, platform mix) → `game/railaudio.rs` (per-frame tracker: odometer, dwell cues) → `Frame.rail` → `synth.rs TrainVoice` ×2 + `dsp::Reverb` (hall); `--audio-wav x.wav --audio-szene ubahn` renders Hermannplatz platform + ride. S/U timetable is time-compressed: `transit::RAIL_PACE` (speed/countdown ×3), `RAIL_TAKT` (×6 departures), `RAIL_DWELL_S` (8 s); `run_profile` = accelerate/cruise/brake within the timetable time. `cargo run --release -- --audio-wav x.wav` renders a measured test
drive offline. HUD: `engine/hud.rs` (screen-space instances; text in HD = Inter SDF atlas `data/gfx/font/` from `tools/gfx/build_font.py`, shapes 6/7 = glyph/outline, `Hud.sdf` set by the engine from `Game::graphics()`; Pixel = font8x8 bitmap, shape 3; both share one atlas, bitmap below `HUD_BITMAP_Y`; font credit from `FONT_MANIFEST`) laid out by `game/hud.rs` in
720-line base units; gamepad: `engine/pad.rs` (gilrs) merged with keys in `game/play.rs input_from`;
weather: `sim/weather.rs` (pure, seed + time) stepped by `World::step_weather`, drawn by `game/weatherfx.rs`;
minimap: `Hud::map_inset` → renderer draws tile meshes with a second camera uniform (`params.y = 1` = flat
schematic) inside a viewport/scissor rect between HUD items `..split` and `split..`;
big map: `map_loader/overview.rs` (overview.json → `OverlayMesh`, line widths in screen px) uploaded once via
`Game::take_overview`, drawn by `overlay.wgsl` through `Hud::overview_inset`; labels in `game/bigmap.rs`;
combat: `sim/combat.rs` (`Player.combat`, `Input.combat`, fighters = `PedState::Fight`, knock-out → hospital; damage =
weapon × `HitZone` (`ray_zone`/`melee_zone`) × ±`DMG_SPREAD` from `world.rng`, `Ped::hurt_t` drives the health bars in
`hud.rs ped_health_bars`; jump = `Input.jump`/`Player.z`, takeoff carries `Player::jump_v` (≥ `JUMP_CARRY`, no air steering; click mode jumps toward `Player::click_goal` and replans after landing), `world::jumpable` lists the low solids that don't block in the air),
effects in `game/effects.rs`; bikes: `sim/bikes.rs` (`World::bikes`, `dismount`, `take_bike`, `combat::Target::Bike`); traffic kinds: `sim/fleet.rs` (`World::rhythm`), work stops `world::update_service`; services: `sim/services.rs` (incidents → `put_npc_car` + `traffic::set_goal` goal field, `Ai.urgent`), siren in `Mix`; Diablo clicks: `World::click_control` + `sim/footpath.rs` (A*), scheme in `settings.json`; PC mouse: left never attacks (`Input::click_attack` stays false) and never enters a vehicle (click on a car = `Click::Approach`, enter only via key), right fires at the cursor, both buttons = weapon wheel; all other inputs go through `game/bindings.rs` (rebindable actions, settings.json `bindings`, trigger/steer curves) — never read `KeyCode`s or pad buttons directly in gameplay code; rebinding UI `game/bindmenu.rs`; rumble `game/rumble.rs` → `Game::rumble`; pedal → force `dynamics.rs pedal_force`; navigation: `sim/routing.rs` (`RouteGraph` from ALL tiles, edge-based A*, cost = travel time × DTV factor + signal/turn penalties) + `game/nav.rs` (graph built on a background thread, waypoint, reroute), set via `bigmap::MapClick`/console `ziel`; smoke/capture runs get no input; screens: `play::Screen` (Title/Playing/Paused/Controls/Stats/Bindings/About; About = `game/about.rs`, version from `package.json`, third-party list `game/src/thirdparty.tsv` from `node tools/thirdparty.mjs`, checked against Cargo.lock by a test) + `game/menu.rs`, stats in `sim/stats.rs` → `stats.json`; Esc belongs to the game (`Game::quit` ends);
`crates/map_loader` decodes v3 tiles, geometry/projection/codes, roof styles and
colors, and tessellates meshes on a dedicated streaming thread. It owns shared
features by global IDs and releases far tiles (maximum 64 resident). The engine
uploads changed tile batches only and instances a procedural tree/decal atlas.
`cargo run -- --check-map` validates every committed tile and polygon;
`cargo run -- --capture /tmp/berlin.png` checks loaded-map rendering on the GPU.
Vehicle physics (phases 1–2, not yet wired into the game): data in `data/vehicles/` (classes, tires, curves,
schema; guide in its README), loader `sim/vehdata.rs`, core `sim/vphys.rs` (120 Hz single-track), test drives
`sim/calibrate.rs`, tool `cargo run --release -p physics-calibrate` (writes `vehicles.calibrated.json` and
`docs/kalibrierung/`). Never change mass/power/torque to hit a target; only the tool's screws. Rotational inertia
reduces engine force only, never the tyre limit. Add new schema fields to `vehdata.rs KEYS` and
`vehicle.schema.json` together (a test checks). Rerun the tool after any physics change. Since phase 3 the player car (four wheels, with data) runs
`car.rs step_vphys` (px/y-down ↔ m/y-left conversion each step, `car.phys`); two-wheelers still `dynamics.rs`.
Holding brake at standstill engages reverse (stop brake tests at vx ≤ 0). Render interpolation: `game/interp.rs`
writes lerped poses before drawing and restores them in `end_frame` — never keep state from inside a frame.
Phase 4: per-wheel grip — `sim/surface.rs` (pure mix of `surfaces.json` ids) + `World::wheel_env` (pavement,
rails, puddles, curbs, winter tyres) → `car.env` → `vphys::Env.wheel` [FL, FR, RL, RR].
Phase 5: two-wheelers run `sim/twowheel.rs` (lean model, falls → `World::throw_rider`); every player vehicle with a
data record uses vphys (`car::vphys_vehicle` maps bicycle/escooter kinds), `dynamics.rs` is only a fallback.
Two-wheel steer = share of `twowheel::kappa_max` (low speed: bar angle `DELTA_LOW`, at speed: lean up to
`LEAN_SKILL` × dry grip) — AI converts curvature with the same function; motorcycles are drawn by `game/motoart.rs`
(parts per `Style`, lean shift `LEAN_SHIFT`), bicycles still by the old branch in `play.rs`.
Phase 6: masses from data in `collide_cars` (`Car::mass`), rollover via `vphys::tip_limit` (min of force and v·r),
rigs via `Vehicle.hitch` + `State.art` (`car::trailer_pose` draws the trailer); data-only vehicles spawn with
`World::spawn_data_vehicle` / console `auto <id>`.
Phase 7: arcade drift layer `sim/drift.rs` (pure state machine, `vphys::State.drift`), off in `Feel::simulation`
so calibration stays pure; a collision voids the running drift (`World` checks `Crash` events and `DRIFT_JOLT`).
Phase 8: tests `sim/tests/{physics_budget,calibration_gate,acceptance}.rs` (known calibration misses go into
`data/vehicles/bekannte_abweichungen.json` with a reason); dev overlay `game/physdebug.rs` (F3, sliders write
`set_game_feel` / `Car::tuned`, never the data files); AI uses `car::Limits` from data and runs full vphys within
`World::ai_full_radius` (`Car::lod_full`, pure-pursuit wheel angle in `traffic.rs`), kinematic beyond.
Engine samples (docs/audio.md): combustion cars play sample banks (`data/audio/engine/
v10/` sport/supercar, `v12/` hypercar, `r4/` four-cylinders, `d4/` diesel cars/vans, `d6/` trucks/buses; preset field `bank`; `zuordnung.typen` maps engine type before class, `null` = synth; a bank may be built from several Freesound `teile`), built by `tools/audio/build_engine_sounds.py
[bank]`, never edit by hand; on/off loops may sit on different rpm grids; pure control logic
`sim/enginesound.rs` (profiles `data/audio/engine_profiles.json`), playback `audio/sampler.rs` (16-bit WAV via
`include_bytes!`, lists `V10`/`V12`/`R4`/`D4`/`D6` must match the manifests), wiring in `game/sound.rs`; such cars are removed from the
synth voices. Panel `game/enginedebug.rs` (F4 / console `motorsound`). `GTA_ENGINE_SAMPLES=0` = synth only.
Local co-op: `sim/coop.rs` (`Seat`, `World::p2`, `swap_seat`/`with_p2`; `World::update_coop` = `player_phase` per seat
+ shared world phase; multi-focus helpers `foci`/`min_dist`/`in_view_any`/`spawn_focus` — anything that lived
"around the camera" must use them) — without P2 the sim must stay bit-identical (`single_player_fingerprint_is_stable`,
never re-record it to make a change pass). Views: `engine/split.rs` (pure Voronoi split) → `Game::camera2`/`set_views`,
`Renderer::render_views` draws each view through the same targets, the second blends in `lighting.wgsl split_out`
(camera field `split`); HUD `hud::Parts` + `Hud::shift_since`/`map2`; culling via `game/coopview.rs Spots`; pads
`engine/pad.rs` slots (`Keys.pad2`). `--koop [METER]` for captures.
Use `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`
and `cargo test --workspace` for native changes. See `docs/NATIVE-RUST.md`.
The Canvas implementation below remains the reference for later porting phases.
Do not claim Xbox Dev Mode support based only on a Windows MSVC/DX12 build.

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

A top-down open-world game (HTML5 Canvas 2D + Web Audio, plain ES modules, **zero dependencies**) set in **all of Berlin at 1:1
scale, built from OpenStreetMap** (streamed in tiles). It runs in the browser on the Mac and inside a C# UWP/WebView2 shell for an Xbox Series X|S in Developer Mode (private sideloading).
UI text, README and docs are German. `README.md` covers controls and the full Xbox install walkthrough;
`docs/TECHNIK.md` explains why UWP+WebView2 was chosen, cites sources, and lists what is only verifiable on real hardware.

## Commands

Node ≥ 20, no `npm install` needed.

```bash
npm start                              # dev server http://localhost:8080 (PORT=9000 npm start); file:// won't work (ES modules)
npm test                               # node --test tests/*.test.js
node --test tests/mission.test.js      # single file
node --test --test-name-pattern="Menü" tests/   # single test by name
node tools/prepare-xbox.mjs            # copy web/ → xbox/GtaBerlin/Web/ and generate package logos (PNG, no libs)
npm run map:fetch                      # Geofabrik Berlin PBF + LOR boundaries + tree cadastre + population density + traffic counts (Geoportal WFS) → data/raw/ (gitignored, needs network)
npm run map:build                      # data/raw/ + data/places.json → web/data/berlin/ (deterministic, ~40 s, ~6 GB RAM)
npm run map:transit                    # data/raw/gtfs.zip (VBB GTFS, fetch with map:fetch -- --gtfs) → web/data/berlin/transit.json (~20 s)
npm run map:preview -- out.svg [x y w h]   # SVG of a px window (default 4×4 km around the mission) for visual checks
npm run bars:fetch                     # gostumblr bar occupancy (app.gostumblr.com/api/v1/bars/busyness + weekly) → web/data/bars.json (gitignored); npm start proxies it live (BARS_URL=aus disables, BARS_URL=… other source)
node tools/check-bridges.mjs           # drives every bridge carriageway both ways on the right lane, reports blocks/wrong levels (~30 s)
node tools/check-bridges.mjs 52.4965 13.4585 1500   # only bridges within 1500 m of a point
```

Design specs and implementation plans for larger features live in `docs/superpowers/specs/` and `docs/superpowers/plans/`
(dated file names). Read the matching spec/plan before continuing a feature that has one.

`web/data/berlin/` (index.json, overview.json, ~2 900 tiles, 138 MB) is committed; only rebuild it when the data or `data/places.json` (mission spots) should change.
The build fails loudly if a mission spot lands in a building, outside the area, or unconnected by road.

The UWP shell (`xbox/`) can only be built/signed on Windows with Visual Studio (Release, x64). It has **never been
compiled** — treat it as unverified. `xbox/GtaBerlin/Web/` is generated and gitignored; rerun `prepare-xbox` after any
change in `web/`.

## Architecture

- **Simulation is DOM-free and deterministic.** `world.js` (`createWorld`/`updateWorld`) is one fixed-step simulation
  (`DT = 1/60` in `config.js`) combining city, player, cars, pedestrians and mission; randomness only via seeded
  `rng.js` (mulberry32). `game.js` is the screen state machine (title → playing ⇄ paused, controls, results) on top of it.
  Both take an **abstract input object** (shape: see `tests/helpers/bot.js` `idle()` / `web/src/idle.js`) and emit
  **events** that the browser layer turns into audio/effects. This is what makes everything testable in Node.
- **`main.js` is the only browser glue:** canvas, fixed-timestep accumulator loop, input sources, renderer/HUD/audio,
  the title-screen demo world, and the Xbox shell bridge. `render.js`/`hud.js`/`audio.js`/`assets.js` are presentation only.
- **Input pipeline (`input.js`):** keyboard + Web Gamepad API + host readings are merged into one raw state, then
  `InputState.frame()` derives the abstract actions. Short key presses are latched until the next sim step.
  Weapon wheel: `weaponwheel.js createRightButton` (pure state machine, two instances in `main.js`: right mouse button
  and gamepad LB). Tap → `tap` (mouse: nothing – entering/exiting is key-only, LB: previous weapon), hold → wheel; the mouse wheel opens at the
  cursor (`place`), the real cursor's direction from its centre selects (`move`, absolute), stick via `aim`, `nudge` (mouse wheel), `choose` (digit keys),
  `cancel` (Esc/B), `sync(held)` resolves a missed release. The right button uses `mousedown`/`mouseup` (pointer events
  don't report a second button on the same pointer, e.g. while firing). `easeTimeScale` fades the slow-motion; the aim
  point is frozen while the wheel is open and after closing until the mouse moves (`aimLock`).
  `hud.drawWeaponWheel(p, hover, {vx, vy, age, pad})` draws it (icons from `drawWeaponIcon`).
- **Xbox shell bridge:** the Web Gamepad API is broken in UWP WebView2, so the C# shell reads `Windows.Gaming.Input`
  and posts a reading every 8 ms via `PostWebMessageAsJson` (`{type:'gamepad', pads}`); `fromHostReading` converts it
  to a standard gamepad. The page sends `{type:'ready'}` and `{type:'quit'}` (menu "Beenden", only shown when
  `chrome.webview` exists → `canQuit`). The shell also swallows `BackRequested` and `VirtualKey.Gamepad*` so B doesn't
  close the app and focus stays in the WebView. Changing the message format means changing both sides.
- **Map is data, not code, and streamed.** `tools/osm/pbf.mjs` (hand-written PBF reader) → `store.mjs` (typed-array
  node coords) → `build.mjs` projects OSM + LOR boundaries (transverse Mercator, 10 px = 1 m, north up) into global
  feature arrays → `tiles.mjs` cuts them into 640 m tiles (lines/buildings/small areas multi-homed with a global `gid`,
  points single-homed, big areas clipped per tile) plus `index.json` (border, Bezirke, Ortsteile, mission places,
  per-Bezirk coverage) and `overview.json` (big map). Codes shared with the game live in `web/src/citycodes.js`, geometry
  helpers in `web/src/geom.js` (imported by the build too). `map.js` `openCity(index, loadTile)` returns a city that loads
  tiles around a focus (`city.focus(key, x, y)` → ready?; `STREAM` radii), refcounts shared objects (`city.reg`) and
  removes their `SpatialHash` entries on unload. `city.edges`/`city.nodes` are **Maps by global id**; whole-layer lists
  only via `city.list(layer)` (loaded area). Hashes: `render`, `edgeSegs`, `polys`, `solids`, `poiHash`, `addrHash`.
  Queries: `surfaceAt`, `inBuilding`, `insideBorder`, `districtAt` (Ortsteil), `bezirkAt`, `nearestEdge`, `locationName`.
  `world.js` calls `streamWorld` each step: if tiles near the camera are missing (browser, async fetch) the world freezes
  (`w.loading`) until they arrive; `findTeleportSpot` returns `{pending}` for unloaded targets; saves resolve lazily.
  Anything cached on edges/nodes (`_walk`, `_slots`, `_marks`) is only computed near the camera, where tiles are complete;
  per-node data needed at the fringe (junction discs, lane trim) comes precomputed from the build.
- **Big map** (`hud.drawBigMap`, data `overview.json`): vector layers as Path2D per class; labels from
  `web/src/maplabels.js` (pure: `mapLabels(data, view, measure, blocked)` picks tiers by metres per HUD pixel, places
  greedily without overlap, street names along chained street runs from `tiles.mjs chainStreets`); cached per view.
- **Street cross-section** (`tools/osm/crosssection.mjs` → `e.cs` at runtime, lane layout in `web/src/street.js`
  `laneOffsets(cs, unit)`): curb-to-curb width, lanes per direction, parking/cycle lanes per side, maxspeed, surface.
  `unit` is 1 in the build (m) and `city.scale` in the game (px). Lanes (`roadgraph.js`), markings (`render.js`),
  parked-car slots (`world.js parkingSlots/manageParked`, role `'curb'`, asleep until hit) and the test autopilot all
  derive positions from it — change the layout in one place.
- **Traffic rules:** `city.signals` (vertex ids) + `web/src/signals.js` (fixed 50 s two-axis cycle); the AI records stop
  lines per route (`ai.stops`) and brakes by stopping distance; `city.turnBans` filters lane successors; blocked edges
  (`e.blocked`, bollards/barrier lines/diagonal diverters) and building passages (`e.passage`) are excluded from the
  AI graph and the autopilot. Junction discs (`city.junctions`, radius = widest half width + 2 m) count as road for
  surface, trees and rendering — the build (`keepTreesOffRoads`) mirrors that rule.
- **Time of day and light:** `world.clock` (minutes, `CLOCK` in `config.js`, saved) → `daylight.js lightAt()` (pure:
  sun direction/length/strength, ambient multiplier, `dark`, `lampsOn`, `windowsLit`). `lighting.js` draws two
  screen-size layers: building/tree shadows (one same-orientation path, filled opaque, composited once at
  `SHADOW_ALPHA × strength`) and, when `dark > 0.02`, a lightmap (ambient fill + additive light sprites, composited with
  `multiply`). Light sources come from `Renderer.collectLights`; a second lightmap pass (`lightOccluders`) redraws
  buildings/tree crowns in depth order with the ambient colour (so ground light doesn't shine on roofs) and adds lit
  windows and lamp heads. `drawBuilding(b, cam, ctx, night)` serves both passes. Street lamps: `lamps.js edgeLamps`
  (pure, cached as `e._lamps`). `Renderer.quality` drops to `'low'` when the median draw time exceeds `RENDER.budgetMs`.
  `?uhr=HH:MM` sets the clock of each new world; `globalThis.__renderer` exposes `stats`/`quality`.
- **Ground detail:** `textures.js texture(ctx, kind)` (world-space patterns, cached per context, `null` without a
  canvas → flat colour); `decals.js edgeDecals` (pure, cached `e._decals`, Path2D per kind in `e._decalPaths`); tree
  crowns are per-genus sprites in `assets.js`. Test stubs for `Path2D`/`OffscreenCanvas` must accept any method.
- **Buildings:** `build.mjs buildingTreatment/mergeParts`: `building:part`s without outline form groups (largest = base unless in water/on a bridge deck, clearly taller parts as own buildings on top), parts clearly taller than their outline become buildings on top, parts starting ≥ 3 m up (`'upper'`) raise the smallest building below them; `render.js buildingDepth` draws a building inside another after it. OSM look per building (`tools/osm/looks.mjs` → tile row fields 7–9: packed `look` bitfield with roof shape/materials/type/Bezirk, roof and facade RGB + 1; trailing zeros dropped; codes in `citycodes.js`, decoded as `b.look`/`b.roofRgb`/`b.wallRgb`). `roofs.js roofOf(b)` (pure, cached `b._roof`: style from OSM shape or estimate, facade style, main axis, `geo` = roof facets with fall direction + tile courses + ridges + dormers, decor list); colours from `buildcolors.js` (pure). `render.js` caches the facet `Path2D`s per building in `b._roofPaths`;
  `render.js drawRoof` draws it, facade patterns per style come from `facadePatterns(ctx)`. Beware: `tests/render.test.js`
  identifies rails by stroke width (`TRACK.rail` = 1.6), so don't reuse that width for other strokes.
- **Cars and people (presentation only):** `vehicles.js` (model from `car.id`, sprite cache per model × colour,
  wheels/lights/blinkers per frame); `ai.blink` comes from `traffic.js blinkFor` (no RNG). Never draw visual variety
  from `world.rng` — it would change the simulation (traffic, tests).
- **People:** `figure.js` (pure): person kinds (`KINDS`: everyday, business, tourist, senior, teen, hipster, worker,
  punk, headscarf, parent, jogger, dogwalker) with weights by Bezirk/hour/weekday/activity; `pickKind(id, ctx)` is a
  hash of the id (no RNG). `world.js assignKind` sets `ped.kind` at spawn and multiplies `ped.speed` by the kind's
  speed (the only simulation effect). `figureLook(p)` derives clothes/hair/hat/accessories (cached `p._fig`);
  `PLAYER_LOOK` is the player. `gait.js` (pure, render-only state `p._anim`): cadence from speed (capped), amplitude
  and run blend eased, facing smoothed; `gaitPose` gives feet/hands/twist/bob/lean; `legFrame` turns the legs to the
  movement direction (`player.move`, set in `updatePlayerOnFoot`; `player.angle` becomes the aim) and walks backwards
  when aiming against it. `people.js drawPerson` draws it all (weapons, `ACT_ARMS`, sitting, lying). Visual check:
  `web/lab/figures.html` (every kind × stand/walk/run/sit/weapon/lying, zoomed, with animation toggle).
  `assets.js personLook` remains for bike riders (`critters.js`).
  0.46.0: `people.js` draws humans anatomically from above (superellipse shoulders `bodyPath`/`torsoDims`, two-segment
  arms `drawArm(ctx, look, side, hx, hy)` with elbow, shoes, head with ears/nose/hairline). Torso and head are painted
  once per look into offscreen canvases (`layer`, key from `keysOf`, LRU `SPR_MAX`, `RES` px/unit, `HEAD_K` head
  scale); without a canvas (Node) it paints directly, so tests see the fills. Never `shade()` a shaded colour (it
  returns `rgb()`); per-frame parts use the memo `tone()`. `ACT_ARMS[act](ctx, look, t, arm)`. `setPeopleDetail`
  (from `Renderer.quality`) drops limb outlines at low quality. Player marker = ground ring.
- **Combat:** `combat.js` (pure: `WEAPONS`, `KICK`, `castRay`, `aimAssist`, `strike`, `shoot`, `hurtPed`, `hurtCar`,
  `updatePlayerCombat`). Input fields `fire/firePressed/kick/reload/weaponNext/weaponPrev/weaponSlot/aimX/aimY/aimWorld`
  (see `idle.js`); keyboard fire is Ctrl/mouse, never W (W is throttle/walk). Peds have `hp` and a `'dead'` state
  (ignored by traffic, removed later out of sight). Renderer turns `shot/impact/blood/kill` events into effects.
  About 15 % of peds fight back (`isFighter`, from the id): state `'fight'` via `updateFight`. Player has `hp`;
  `hurtPlayer` → `dead` → `world.js updateKnockout` respawns at `nearestHospital` (`city.hospitals` from index.json).
- **Levels (`lvl`, never `layer` — that name is the render category):** OSM has no heights, only order
  (`bridge`/`layer`/`tunnel`). `tools/osm/levels.mjs levelOf` gives every edge/path/rail/deck a level (bridge ≥ 1,
  open underpass < 0, tunnels dropped), packed as 3-bit signed values (`citycodes.js packLvl/unpackLvl`). Portals
  (`city.portals`, nodes where different levels meet) are the only place an entity changes level (`levels.js
  stepLevel`, run for all movers in `world.js updateLevels`). `render.js` draws level by level; movers under a higher
  surface (`occlusion.js surfacesOver`) are drawn before it and get a silhouette. Physics/traffic are not level-aware yet.
- **Invariant: no tree trunk on a carriageway** (crowns may overhang). `keepTreesOffRoads` in the build pushes trunks to
  the curb or drops them and throws if any violation remains; `decodeCity` drops violators as a safety net
  (`city.droppedTrees` must stay 0); tests check the shipped map, the fixture and the safety net. Constants
  (`TREE_TRUNK_M`, `TREE_FREE_MAX_CLASS`) live in `citycodes.js`, shared by build and game.
- **POIs and house numbers** come from the same Overpass fetch: `city.pois` (`POI_CATS` in `citycodes.js`, stations
  deduped by name), `city.addresses`; queries `nearestPoi` / `nearestAddress`; `locationName` appends the house number.
  Labels are drawn in screen space by `Renderer.drawPois` (capped, overlap-culled).
- **Day rhythm and city life:** `w.clock` + `w.day` (weekday, 0 = Mon, new game Friday). `rhythm.js` turns time, weekday,
  edge traffic counts (`e.dtv`, from `assignTraffic` in the build) and the density raster (`densityAt`) into population
  targets (`w.carTarget`/`w.pedTarget`, only when `w.rhythm`, i.e. default population). `life.js lifeSpots` derives
  activity spots from POIs, OSM furniture (`city.render` layer `'furn'`, `FURN_KIND`) and big lawns, deterministically
  from place/hour/day; `world.js manageLife` keeps them staffed with peds in state `'hang'` — spawn and despawn only out
  of view. Visual variety comes from id hashes, never from `world.rng`.
- **Vehicle kinds, services, bikes, animals, ambience:** `car.kind` (`fleet.js KINDS`: size/power per kind; collision
  and drawing use per-car `hw`/`hh`, AI gaps are bumper-to-bumper). `services.js`: work stops via `ai.hold`, emergency
  incidents (dead peds → ambulance, shots → police) with goal routing (`traffic.js goalField/setGoal`, `ai.urgent`
  runs reds). `bikes.js`: `w.bikes` on the lane graph (rightmost lane, offset to cycle lane/curb). `animals.js`:
  `w.animals` pigeons/ducks. All four only when `w.rhythm`. `ambience.js` is the pure sound mix for `audio.js`.
- **Cyclists as targets and vehicles:** `combat.js castRay`/`meltargets`/`aimAssist`/`pickTarget` include riding
  `w.bikes` (hit type `'bike'`); `hurtBike` → `bikes.js dismount` (bike `'lying'`, rider becomes a knocked-down ped that
  takes the hit; event `bike-down`). `world.js tryEnter` also grabs bikes within `BIKE_GRAB` → `takeBike` turns the bike
  into a car of kind `'bicycle'`/`'escooter'` (`fleet.js KINDS` with `bike: true`, `top` px/s, `accel`; `isBikeKind`),
  removes it from `w.bikes`, pulls a rider off (`carjack` with `bike: true`). Such cars: drawn with `critters.js drawBike`
  (state `'parked'` = no rider), no engine sound/muffle (`main.js onBike`, `ambience.js inCar`), camera near
  (`updateCamera`), never `playerCarId`, no mission cargo (`mission.js veh` vs `car`). Diablo: click on a riding cyclist
  = attack, double-click = hijack; lying bike like a car (approach/enter).
- **Weather:** `weather.js` (pure: blocks per day from `seed`+`w.dayCount`, `weatherLight` adjusts `lightAt`, `stepWet`),
  world keeps `w.weather`/`w.wet` (only with `w.rhythm`, else clear; `w.forceWeather` / `?wetter=`); `wetfx.js` draws
  clouds, rain, wet roads + puddles (`e._puddles`), fog and neon signs; `render.js facadeLight` shades walls by sun.
  Weather values are `{cloud, rain ≤1.6, fog ≤1.7, snow, storm, thunder}` (11 kinds, day types normal/unsettled/winter).
  Gusts (`gustAt`), lightning (`strikeInSlot/strikesAt/flashAt`) and thunder arrival (`thunderBetween`, half-open
  intervals) are pure functions of seed and `w.time`. Snow cover `w.snow` is sim state like `w.wet` (car grip, save).
  Snow/rain/debris/fog banks are drawn from hashes + time (no particle lists); road slush and the wet-road film go through
  an offscreen layer (`Renderer.overlayLayer(fn, alpha)`, so overlaps don't darken junctions). Cloud shadows, ground fog,
  rain curtains and snowstorm sheets are tileable noise patterns (`wetfx.js noisePattern`, per context, scrolled/scaled
  via `scrolled()` = `pattern.setTransform`; `null` without canvas image data → old fallbacks). Ground fog
  (`drawGroundFog`) and wet ground (`drawWetGround`) are drawn before the depth-sorted objects, so roofs stay above
  them; `rainRipples` = impact rings, `drawPuddles` = rim + sky gradient + rain rings.
- **Weather on the road:** `weather.js temperatureAt` (pure) + `w.ice` (`stepIce`, saved; `w.forceTemp` via console
  `temp`) → `traction.js roadCondition/tractionOf` (grip factors, floor = ice value; covered spots dry, bridges ×1.5
  ice; puddles = `edgePuddles`), written per step to `car.traction` by `world.js applyWeather` (also `car.aqua`
  aquaplaning, `gustPush`); `car.js`, `traffic.js` (slower, gentler braking, longer following distance; standstill gap
  unchanged) and `trainphysics.js` (`input.adhesion` from `playertrain.js trainAdhesion`, tunnels = 1) read only
  factors. Missing `car.traction` = dry (and dry + no storm skips the lookup: `car.traction = DRY`). `under()` counts a
  higher edge as a roof only if it doesn't connect to the own level nearby (bridge approaches aren't covered); viaducts
  (rails of higher level, not at their ends) cover too. HUD: temperature next to the clock, `roadWarning` sign.
- **Sound:** `audio.js` only synthesizes; the mix is pure: `ambience.js ambienceAt` (layers + `bar`/`music`/`barPan`,
  `muffle` = in car/snow cover, `gust`), `soundscape.js` (`stepEngine`: rpm/gears per `ENGINES[kind]`, firing freq;
  `tireState`: roll/cobble/wet/snow/skid/slide/wind; `carVoices`: nearest AI cars with pan + Doppler `rate`;
  `stepsBetween`/`footstepKind` from `player.step`). Graph: everything outside goes through `outside` → `muffleF`
  (lowpass), the own vehicle bus goes straight to `master` → compressor. All loop layers register in `sound.loops`
  (tests check they start silent). `main.js` keeps the engine state per player car and updates voices at 20 Hz.
- **Nightlife:** `nightlife.js` (pure): `typicalLevel(kind, min, day)` per OSM drink kind (bar/pub/biergarten/
  nightclub), `parseBarFeed` (tolerant: list or `{bars}`/GeoJSON, `populartimes`/`week` 7×24 Mon-first, current 0..1 or
  %), `attachBars(city, feed, toPx)` → `city.bars` (`gen`, `byName`), `feedBarFor(city, q)` (name match nearby, cached
  on the POI per `gen`), `barLevel`, `nightlifeAt` (crowd/music/pan/sources; feed bars without OSM POI sound at their
  coordinate). `life.js` scales smokers/club queues by `barLevel` only for feed bars (so tests without a feed are
  unchanged). Feed source in `main.js`: `?bars=` (localStorage `gta-bars-url`), console `bars`, else `data/bars.json`
  (served live by `tools/serve.mjs` via `tools/bars-source.mjs fetchBars`, default gostumblr). gostumblr format: bars with
  `latitude/longitude/occupancy_percent/usual_percent/last_scraped/trend[[epoch,pct]]` + `weekly` (per-dow average of
  all bars, dow 0 = Sunday, Berlin local hours); `parseBarFeed` turns that into a per-bar 7×24 week (Mon first):
  weekly shape × popularity ratio (usual vs. weekly at `berlinSlot(last_scraped)`), overridden by trend hours. `web/src/projection.js` holds the map projection
  (`geoToPx(meta)`), re-exported by `tools/osm/geo.mjs`.
- **Snow tracks:** `snowtracks.js` (pure, presentation only): `render.js drawSnowTrails` records rear-wheel segments of
  all `world.cars` per frame into a ring buffer (world time, reset per world), fades them (`trailAlpha`: age, snowfall).
  Tyre tracks use `car.hw` = half **length**, `car.hh` = half **width** (as everywhere): four wheels as drawn in
  `drawCarBody`, one line for two-wheelers, only at lvl 0.
  Note that `npm run map:build` wipes `web/data/berlin/` — rerun `npm run map:transit` afterwards.
- **Windows:** `windows.js` (pure) decides per window (flat hash + room hash vs. `windowsLit`) whether it is lit and in
  which colour; `render.js drawLitWindows` draws them per face, cached per building and game minute.
- **Public transport:** `transit.json` (built by `tools/osm/transit.mjs` from VBB GTFS, own ZIP reader `zip.mjs`) →
  `transit.js prepareTransit` (patterns: shape, stop arc lengths, run times, departures per day type). The timetable only
  sets the headway at the game clock (vehicles run in real time); `stepTransit` keeps virtual vehicles (fare time τ) per
  pattern near the camera in `w.transit`. `transitlive.js`: buses become AI cars (`kind 'bus'`, `ai.follow` = follow the
  shape via `lane.nextBus`, stop at stops), trams stay kinematic but stop for obstacles and feed `w.railObs` (collision,
  AI obstacles), S/U trains are drawn only on above-ground rails. Bus-only lanes (`lane.busOnly`: busways,
  `cs.busContra` contraflow) never appear in `lane.next`. `city.transit` is set by `main.js` / `tests/helpers/city.js`.
- **Riding and driving transit:** vehicles are referenced, never copied: `{pid,key}` (virtual timetable vehicle),
  `{carId}` (bus as AI car), `{playerTrain:true}`; `ride.js vehicleState(w, ref)` resolves one or returns `null` once it
  is gone (→ `world.js endRide` puts the player on foot at `ride.lastStop`, S/U via `stationExit`; `rideExit` is shared
  with the save). `w.player.ride = { kind: 'passenger'|'driver', ref, lastStop, … }`; passengers board/alight with
  `input.ride` (G / D-pad down). Taking over at the cab (`enterExit`, `playertrain.js takeTrain`) marks the virtual
  vehicle `gone` and continues it as `w.playerTrain`, driven by pure `trainphysics.js` (`stepDrive` with a `limit` =
  free distance: end of line, `trainAhead`, `tramFree` → forced braking along √(2·a·d)); timetable trains of the same
  pattern behind it wait (`behindPlayer` in `transitlive.js`). Underground = S/U with no visible rail running along the
  line within 5 m (`tunnel.js undergroundAtS`, heading-aware; unloaded tile = above ground). `tunnelview.js drawTunnels`
  draws the tunnel view, faded by `w.underground`. Tram tracks exist only in the timetable shapes (`transit.js
  tramTrackNear`); parking slots avoid them (`e._slots` is keyed on `city.transit`, which loads async in the browser).
  Trams have right of way: an AI car with a tram car head-on in front (`traffic.js tramHeadOn`) backs up (`ai.tramYield`),
  the car behind a yielding car too — the timetable shape lies in the oncoming lane on ~1.3 % of tram track.
- **Walkable stations (0.39.0):** `station.js` (pure) builds schematic platforms from the timetable: `stationsNear(city,
  x, y, r)` groups underground U/S stops (`undergroundAtS`) by name + axis, cached in `city._stations` (`stationById`);
  a station has centre, axis (`ax/ay`), half length `HL` (train length), `halts [{pid, i, dir}]` (dir +1 keeps right, v > 0)
  and two `exits` (street entrances above the stair ends). Local frame `toLocal/toWorld` (u along, v across);
  `keepInside`, `stairAt`, `arrivalAt`, `trainsAt` (timetable vehicles as cars), `boardable`, `departures`, `waiting`.
  `world.js updateStationPresence`: entrance → `p.inside = {id}` (lvl −2, city hidden), stairs → exit (+`entryGuard`);
  `boardAtPlatform` / `platformArrival` (alight underground onto the next platform); `stationSaveSpot` for saves.
  `stationview.js drawStation` replaces the whole frame in `render.js` while inside; `drawEntrance` draws the stair
  shafts at lvl 0. Combat skips targets when one side is below lvl −1; `ambienceAt` returns a muffled station mix.
  Stations are built per name from **all** stops of that name (`stopIndex(tr).byKey`), never from a radius query (the
  first approach must not decide which platforms exist); `platformUnderground` requires five points along every halt's
  train to have no same-direction rail within `STATION.probe` (15 m). A third exit `main: true` sits at the OSM station
  POI (`mainPoi`, name matched via `matchKey`, same mode) unless a stair exit is within `poiEntrance`. `entranceNear`
  + `STATION.reach`: E (action) enters, HUD hint, minimap icons; `w._stNear` is refreshed every 0.5 s in `updateWorld`
  (also in cars). `teleportTo` sets `entryGuard` at the landing spot; `restartMission` clears `inside`. Trams
  (`obstacleAt`, `collideRail`), pedestrians and gunshot reactions (scare, police) ignore a player who is inside.
- **On-foot PC controls (0.32.0):** `game.settings.controls` = `'diablo'` (default) | `'classic'` (localStorage
  `gta-controls`, toggled with ←/→ on the controls screen). Diablo input fields `clickWorld/clickPressed/clickHeld/
  clickForce/walkSlow` (see `idle.js`, set by `main.js applyPointer`); `world.js clickControl` turns them into the
  normal inputs before `updatePlayerOnFoot` (path via pure `footpath.js findFootPath`, A* on 8 px cells; attack via
  `combat.js clickIntent` = 'attack' (ped) / 'enter' (intact car within `enterDist + CLICK.nearCar` or `clickDouble` → walk there, stop `CLICK.door` s, `tryEnter(w, car)`) / 'approach' (farther car: walk next to it, don't enter) / 'move' / 'force' (Ctrl), decided only at press: held ground clicks follow the cursor and never attack, held attacks stay on the clicked ped; WASD cancels; Shift = sprint, Ctrl = `clickForce`, right-button tap = kick, hold = wheel). `main.js` shows the intent as cursor (`cursor.js` 'arrow'/'attack'/'enter'/'target') and `renderer.hover` outline only for non-walk intents after the pointer dwells (`HOVER` ms, never while the button is held); `renderer.clickFx` = one-shot ring at a walk click (`CLICK_FX` s), no looping marker (`render.js drawClickMarks`); the in-world crosshair only with Ctrl (`renderer.crosshair`). Mouse weapon wheel opens at the cursor (`rightBtn.place(wheelCenter())`, clamped on screen) and selects by the real cursor's direction from its centre (`move(x, y)` absolute, dead zone `WHEEL.dead`); left click inside picks. Speeds `PLAYER.walk/jog/sprint` + `STAMINA`; foot zoom `FOOT_ZOOM`/
  `setFootZoom` (`w.footZoom`, localStorage `gta-foot-zoom`); render detail level (`DETAIL_ZOOM`, `stats.detail`) only
  at quality high. Mouse aim snaps only via `pickTarget` under the cursor, spread × `spreadFactor(p)`; gamepad keeps
  `aimAssist`. Trams give up yielding after `TRAM_PATIENCE` (20 s) against a strictly persisting obstacle.
- **Access:** fences/walls/hedges/bollard lines open `GATE_M` (4.4 m) at gate nodes and wherever any highway way crosses
  them (build, `accessAndRules`). Barrier posts block AI traffic via `e.blocked`, but the player's car knocks them over
  (`car.js knockOver`, `KNOCK` in config): `world.knocked` (post key → angle, survives tile reloads), every solids loop
  skips them via `isDown(world, s)`. Bollard lines (OSM ways `barrier=bollard/block`, `FENCES.bollard`) are emitted as
  posts every ≤ 1.5 m, not as walls, so they can be knocked over too (`keepPostsOffRoads` moves those on open roads). `tests/helpers/city.js reachability` flood-fills a grid for car/foot radius;
  `tests/access.test.js` checks Tempelhofer Feld and car-vs-foot coverage.
- **Invisible walls:** quays, rail side walls and railings are finally cut with `build.mjs surfaceIndex` + `cutWhere` (exact
  point test of the drivable surface per side via `reachOf`: carriageway + bridge gap + cycle track, junction discs, incl.
  passages/blocked roads), per level (`onSurfaceAt(L)`, `surfCutAt(L)`). `tests/walls.test.js` scans the core area and known
  bridges for leftovers on the same level.
- **Bridges:** railings are cut (`railCutAt(L)(pts, skipId)`) wherever another bridge way of the same level or a filled gap
  lies, so they only stand at the outer edge (they continue over roads below); `bridgeFills` gives close dual carriageways a `fill` (tile edge
  field 12). `cs.left/right.track` = cycle tracks beside the curb (x[13], x[14]). Draw order in `render.js`: ground roads →
  bridge curbs → bridge paths → bridge carriageways (+fill) → tracks → markings.
- **Direction signs:** built by `tools/osm/signs.mjs buildSigns` (pure: junction clusters incl. roundabouts, approaches/
  exits by oneway, destinations from OSM `destination_sign`/`destination:*` or traced Ortsteile + „Zentrum“, placement
  via `build.mjs roadClearance`), tile field `signs`, decoded by `web/src/signs.js decodeSign` into layer `'sign'`;
  `render.js signBoard` caches the board canvas (`sg._board`), `drawSign` draws post + board beside the road.
- **Silhouettes:** `occlusion.js occludersOf` (pure) lists what covers a vehicle/person (drawn later in depth order and
  touching one of its `samplePoints`): tree crowns, buildings (incl. passages), viaducts. `render.js` collects
  `this._covered` for all cars/peds/bikes/tram cars and the player, sets `stats.cover` (player) / `stats.silhouettes`,
  and `drawCovered` masks the outline with the union of occluders on two scratch canvases (`destination-in`) after the lightmap.
- **Car models and driving (0.41.0):** `carmodels.js` (pure): `CAR_MODELS` (11 Pkw), `SPECS` (drive fwd/rwd/awd, engine
  front/mid/rear/floor, mass, front weight share, CG height `h`, wheelbase, track, kW, vmax, tyre `mu`, yaw inertia factor,
  `noAids`), `carModel(car)` (fixed `car.model` or id hash, kind for special vehicles, player car = limousine), `specOf`,
  `specLine`. `dynamics.js stepDynamics` = single-track model (substeps 4, SI inside): slip angles in the wheel frame,
  tyre curve `sin(C·atan(Bα))`, friction circle per axle, longitudinal load transfer (suspension-lagged `dyn.ax`), lateral
  load sensitivity by `h/track`, ASR + ESP (drive cut by slip angle, yaw-rate damping), ABS, handbrake drift, kinematic
  blend below 1–4 m/s. `DYN.fun` is the fun layer (grip, brakes, power, steering, handbrake) – tune there, keep the
  physics honest. Only the player's car (`car.driver === 'player'`, not bikes) uses it; AI/parked cars stay on the
  arcade model in `car.js` (traffic tests depend on it). `car.dyn` = state for HUD/tests (`delta`, `ax/ay`, `alphaF/R`,
  `esp`, `understeer`); `car.esp` set from `w.esp` (console `esp`). `vehicles.js bodyShift` = pitch/roll offset;
  `soundscape.js engineFor` picks `MODEL_ENGINES` when `car.dyn || car.model` (electric = whine in `audio.js`).
  The mission bot (`tests/helpers/bot.js`) drives with pure pursuit (curvature → steering angle), corner look-ahead
  braking (`cornerCap`), loop skipping and stuck/orbit recovery – keep it player-like when tuning the physics.
  0.45.0: 19 more Pkw modelled on real cars (37 total, `SHARE` sums to 1, fictional names only: `NAMES`/`vehicleName`),
  `specLine` in PS (`psOf`) with FWD/RWD/AWD; `vehicles.js SHAPES` `van`/`high` for box vans. Entering sets
  `w.vehInfo = {carId, t}` (cleared after `VEH_INFO_S`), drawn by `hud.drawVehInfo`; `main.js` passes `game.engine`
  (`rpm/gear/red/electric` from the engine sound state) to `hud.drawSpeedo`. Game HUD uses `hud.otext` (outlined text,
  `HUD_FONT`) instead of panels; health bar under the minimap (`drawHealth`, part of `layout.minimap`).
  0.43.0 added 7 more Pkw (`hothatch`, `roadster`, `musclecar`, `oldtimer`, `pickup`, `kleinbus`, `rallye`; `brakeK`
  = weaker brakes, `open` roadster) and two-wheelers: `fleet.js KINDS.motorcycle/scooter` (`moto: true`,
  `isMotoKind`, `isOpenKind` = bike or moto: unmuffled, no cargo, never `playerCarId`), specs with `twoWheel`
  (`stepDynamics` caps drive at the wheelie force `m·g·front·wb/h` and braking at the stoppie force, `dyn.wheelie/
  stoppie/lean`, no lateral load term). `pickKind` spawns them by day. `vehicles.js drawMoto` (rider leans, front
  lifts, `car.fallen` lies); `world.js throwRider` on a player crash with `strength ≥ MOTO.throwAt`; camera closer.
- **Collision** is wall *segments* (building rings, quays cut open at bridges, rail lines, bridge railings, the district
  border) plus tree circles and crate rects, via `circleVsSegment` / `obbVsSegment` in `collision.js`. The SAT depth is the
  shortest escape distance (`min(a1-b0, b1-a0)`), which matters for zero-thickness walls.
- **Traffic:** `roadgraph.js` turns drivable in-area edges (`cls <= TRAFFIC_MAX_CLASS`) into lanes per the cross-section
  as their edges load (hooks `city.hooks.edgeAdd/edgeRemove`; `lane.next` recomputed per `city.gen`) (trimmed at junctions, Bezier connectors, no U-turns except dead ends); `traffic.js` follows the lane
  polyline with pure pursuit, slows for turns/obstacles, replans via the lane hash. **Population lives around the camera**
  (`TRAFFIC.spawnMin/spawnMax/despawn` in `config.js`, `managePopulation` in `world.js`).
- **Spawn rule:** a new AI car born within `GATE_STOP` of its first line would pass it in the first step and be
  claimed unchecked by `entryGate` (meant for platoons), even against oncoming traffic in a narrow. Every spawn
  (traffic, services, buses) therefore calls `spawnAllowed` (Rust: `traffic::spawn_allowed`) after `placeOnLane`.
- **Right of way = reservations, not StVO.** `ai.segs` (lane pieces with `k0`/`kEnd`) drive an entry gate before each lane
  end: `mayEnter` checks space behind the junction, `world.jres` (unsignalled junction: approach + per-car movement
  chord; non-conflicting movements may share) and `world.nres` (a narrow = all connected narrow edges of one street,
  keyed by `narrowKey`, one direction at a time; dead-end narrows one car only). Claims live in `ai.claims` and are
  released by `releaseClaims` — a car can hold several claims for the same key, only drop it from the set when none
  remain. Waiting cars stop `GATE_STOP` before the line: the route index advances within 10 px of a point, and a car
  whose index passes `kEnd` counts as inside. Obstacles are measured along the car's own route (`aheadPath`), not its
  heading. `tests/traffic.test.js` has soak tests at the tightest spots plus invariant tests; the system is chaotic,
  so check changes with those, not with a single run.
- **Pedestrians** walk along road edges at a per-side sidewalk offset (cached, shrunk if it would hit a building), pick the
  next edge at nodes, cross streets, and wait for approaching cars.
- **Console and statistics:** `console.js` (pure: `tokenize`/`suggest`/`execute`/`consoleKey`/`consoleAccept`, `placeIndex` from
  `overview.json` for `tp`; palette behaviour: `smartLine` = time/weather/place without a command word, `rankMatches` tier 3 = typo via `editDistance`, Enter retries with the top suggestion if the line fails, success closes (Shift keeps open), empty line lists recent commands, `sugg.help` usage line, suggestions are HUD hits `kind: 'sugg'`) is opened by Enter in `main.js` (Action is E only); while open, `game.js` freezes the world.
  Console teleports carry `auto` and are confirmed by `game.js` once tiles are loaded. `stats.js trackStep` books world
  events + `game.statQueue` (teleport, cheat) into `game.stats.game`/`total`; `statsdb.js` persists them in IndexedDB
  (`applyStoredStats` merges the async answer). New counters need a row in `STAT_SECTIONS`; events that count must
  carry who caused them (`player`, `weapon`, `carId`).
- **Wear and look (0.40.0):** `grime.js` (presentation): `drawGrime` (dirt/bleach noise patterns via `wetfx.js
  noisePattern/scrolled`, two scales rotated, at lvl 0 after crossings, faded by snow), `laneWear(city, e)` (pure, lane
  centre offsets, cached `e._wear`) + `drawLaneWear` (oil band, high quality), `drawContactShadows` (AO strokes around
  footprints before the depth-sorted objects), `roofGrime`, `waterGlint`, `drawVignette` (screen space). Facade soil
  gradient in `drawBuilding` (`soilGradient`). Pure colour mapping `grimeRGBA(kind, n)`.
- **Performance (0.44.0):** simulation speedups must stay bit-identical (check with a position hash over a few thousand
  steps against the previous commit). `grid.js buildGrid/near` = neighbour grid returning indices in list order (so loop
  order and results don't change); `world.js` builds `_gCars/_gPeds` right before the AI loop (`w._gridOn`, nothing
  moves there; `obstacleAhead` uses it only then), a car grid for the ped loop and for car↔car pairs (`j > i`, radius 170).
  `traffic.js zebrasNear` caches zebra crossings per 200-px cell per `city.gen`; `transit.js departuresPerHour` memo per
  game minute + day; `pointOn` uses `geom.js cumLengths/pointAlongCum` (binary search, `shape._cum`); `circleVsObb`
  rejects by bounding box first. Rendering: lit windows as cached `Path2D`s per face (`wins._paths`, `windowPaths`/
  `fillPath` with a rect-list fallback without Path2D), one facade frame per face with `Renderer.baseGradient(ctx, H)`
  (cached per rounded height), pooled face objects, minimap ground from `hud.miniBase` (offscreen, ±4000 px, rebuilt on
  160 m movement or city.gen after 30 frames; falls back to direct drawing without a canvas). `perf.js stepResolution`
  (pure) picks the internal resolution step from rAF gaps (`RES`), applied in `main.js resize`; console `aufloesung`.
- **Tree shadows:** `lighting.js treeShadowGeom` (pure) + `addTrunkShadow` (trunk strip in the opaque shadow path) +
  `crownShadowSprite` (crown sprite as black silhouette with gaps and blurred edge, stretched along the sun).
- **Mission** (`mission.js`) is a state machine; save (`save.js`) is one `localStorage` slot, auto-written after a
  completed mission, with `memoryStorage()` for tests and a corrupt-save path.
- **Assets:** all graphics/sounds are self-generated placeholders; real files can be swapped in via
  `web/assets/manifest.json` (keys, sizes and orientation in `web/assets/README.md`). Never use names/art/music from
  other games.

## README badges

`docs/badges/*.svg` are generated by `node tools/badges.mjs` (LoC without blank/comment lines over tracked + new
non-ignored files, tests counted statically: Rust `#[test]`, JS `test(`/`it(` in `tests/*.test.js` — matches
`cargo test -- --list` and `npm test`). The pre-commit hook `tools/githooks/pre-commit` regenerates and stages them;
each clone needs `git config core.hooksPath tools/githooks`. The repo is private, so no shields.io endpoints.

## Versioning and releases

**Rust implementation:** its own SemVer number is the Cargo workspace version (`Cargo.toml [workspace.package] version`,
shown as „Rust x.y.z“ on the title screen and in „Über das Spiel“, badge `version-rust`). The pre-commit hook
(`tools/rust-version.mjs`) raises the patch level automatically whenever a commit touches `crates/`,
`tools/physics-calibrate/`, `data/`, `Cargo.toml` or `Cargo.lock`, and updates the own packages in `Cargo.lock`. Never
bump the patch by hand; for a minor/major bump edit `Cargo.toml` yourself (the hook then leaves it alone). Two sessions
committing in parallel will both bump – on a rebase conflict in `Cargo.toml`/`Cargo.lock` take the higher number + 1.
The browser game's version below stays separate.

SemVer, started at 0.0.1 (0.x = prototype, formats may break). The version lives in **three places that must match**:
`package.json`, `web/src/version.js` (shown on the title screen) and `xbox/GtaBerlin/Package.appxmanifest`
(`Identity Version="X.Y.Z.0"`); `CHANGELOG.md` needs a dated `## [X.Y.Z] – YYYY-MM-DD` entry on top.
`tests/version.test.js` enforces all of this. Every commit that changes behaviour bumps the version (patch for fixes,
minor for features), gets a CHANGELOG entry, is tagged `vX.Y.Z` and pushed with `git push --follow-tags`.

## Tests

`node:test` only. Tests load the real map via `tests/helpers/city.js`: `realCity()` pins the Kreuzberg + Neukölln tiles
(~1 s, like the old single-file map), `openRealCity()` gives a fresh streaming city with a synchronous disk loader. A
sweep test loads every tile of Berlin once (tree rule, no leftovers after unload, ~11 s).
`tests/helpers/bot.js` is an **autopilot** that plays the full mission through the same abstract inputs a player uses
(A* over the real road graph); there are also soak tests with traffic/pedestrians, full-throttle ram tests against walls,
quays and the border (checked every step), and `tests/osm-build.test.js`, which runs the build on a tiny synthetic OSM
fixture. Keep the simulation free of DOM access so these keep running in Node. When the test count or verified
behaviour changes, update the "Stand und Prüfumfang" section of `README.md` accordingly.
