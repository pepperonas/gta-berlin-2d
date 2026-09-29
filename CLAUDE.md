# CLAUDE.md

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
npm test                               # node --test tests/
node --test tests/mission.test.js      # single file
node --test --test-name-pattern="Menü" tests/   # single test by name
node tools/prepare-xbox.mjs            # copy web/ → xbox/GtaBerlin/Web/ and generate package logos (PNG, no libs)
npm run map:fetch                      # Geofabrik Berlin PBF + LOR boundaries + tree cadastre + population density + traffic counts (Geoportal WFS) → data/raw/ (gitignored, needs network)
npm run map:build                      # data/raw/ + data/places.json → web/data/berlin/ (deterministic, ~40 s, ~6 GB RAM)
npm run map:transit                    # data/raw/gtfs.zip (VBB GTFS, fetch with map:fetch -- --gtfs) → web/data/berlin/transit.json (~20 s)
npm run map:preview -- out.svg [x y w h]   # SVG of a px window (default 4×4 km around the mission) for visual checks
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
  and gamepad LB). Tap → `tap` (mouse: `enterExit`, LB: previous weapon), hold → wheel; mouse movement accumulates
  from the wheel centre, clamped to the radius (`move`), stick via `aim`, `nudge` (mouse wheel), `choose` (digit keys),
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
- **Weather:** `weather.js` (pure: blocks per day from `seed`+`w.dayCount`, `weatherLight` adjusts `lightAt`, `stepWet`),
  world keeps `w.weather`/`w.wet` (only with `w.rhythm`, else clear; `w.forceWeather` / `?wetter=`); `wetfx.js` draws
  clouds, rain, wet roads + puddles (`e._puddles`), fog and neon signs; `render.js facadeLight` shades walls by sun.
  Weather values are `{cloud, rain ≤1.6, fog ≤1.7, snow, storm, thunder}` (11 kinds, day types normal/unsettled/winter).
  Gusts (`gustAt`), lightning (`strikeInSlot/strikesAt/flashAt`) and thunder arrival (`thunderBetween`, half-open
  intervals) are pure functions of seed and `w.time`. Snow cover `w.snow` is sim state like `w.wet` (car grip, save).
  Snow/rain/debris/fog banks are drawn from hashes + time (no particle lists); road slush goes through an offscreen layer.
- **Weather on the road:** `weather.js temperatureAt` (pure) + `w.ice` (`stepIce`, saved; `w.forceTemp` via console
  `temp`) → `traction.js roadCondition/tractionOf` (grip factors, floor = ice value; covered spots dry, bridges ×1.5
  ice; puddles = `edgePuddles`), written per step to `car.traction` by `world.js applyWeather` (also `car.aqua`
  aquaplaning, `gustPush`); `car.js`, `traffic.js` (slower, gentler braking, longer following distance; standstill gap
  unchanged) and `trainphysics.js` (`input.adhesion` from `playertrain.js trainAdhesion`, tunnels = 1) read only
  factors. Missing `car.traction` = dry (and dry + no storm skips the lookup: `car.traction = DRY`). `under()` counts a
  higher edge as a roof only if it doesn't connect to the own level nearby (bridge approaches aren't covered); viaducts
  (rails of higher level, not at their ends) cover too. HUD: temperature next to the clock, `roadWarning` sign.
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
- **On-foot PC controls (0.32.0):** `game.settings.controls` = `'diablo'` (default) | `'classic'` (localStorage
  `gta-controls`, toggled with ←/→ on the controls screen). Diablo input fields `clickWorld/clickPressed/clickHeld/
  clickForce/walkSlow` (see `idle.js`, set by `main.js applyPointer`); `world.js clickControl` turns them into the
  normal inputs before `updatePlayerOnFoot` (path via pure `footpath.js findFootPath`, A* on 8 px cells; attack via
  `combat.js pickTarget`; WASD cancels; Shift = sprint, Ctrl = `clickForce`, right-button tap = kick, hold = wheel). Speeds `PLAYER.walk/jog/sprint` + `STAMINA`; foot zoom `FOOT_ZOOM`/
  `setFootZoom` (`w.footZoom`, localStorage `gta-foot-zoom`); render detail level (`DETAIL_ZOOM`, `stats.detail`) only
  at quality high. Mouse aim snaps only via `pickTarget` under the cursor, spread × `spreadFactor(p)`; gamepad keeps
  `aimAssist`. Trams give up yielding after `TRAM_PATIENCE` (20 s) against a strictly persisting obstacle.
- **Access:** fences/walls/hedges/bollard lines open `GATE_M` (4.4 m) at gate nodes and wherever any highway way crosses
  them (build, `accessAndRules`). Barrier posts block AI traffic via `e.blocked`, but the player's car knocks them over
  (`car.js knockOver`, `KNOCK` in config): `world.knocked` (post key → angle, survives tile reloads), every solids loop
  skips them via `isDown(world, s)`. `tests/helpers/city.js reachability` flood-fills a grid for car/foot radius;
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
- **Collision** is wall *segments* (building rings, quays cut open at bridges, rail lines, bridge railings, the district
  border) plus tree circles and crate rects, via `circleVsSegment` / `obbVsSegment` in `collision.js`. The SAT depth is the
  shortest escape distance (`min(a1-b0, b1-a0)`), which matters for zero-thickness walls.
- **Traffic:** `roadgraph.js` turns drivable in-area edges (`cls <= TRAFFIC_MAX_CLASS`) into lanes per the cross-section
  as their edges load (hooks `city.hooks.edgeAdd/edgeRemove`; `lane.next` recomputed per `city.gen`) (trimmed at junctions, Bezier connectors, no U-turns except dead ends); `traffic.js` follows the lane
  polyline with pure pursuit, slows for turns/obstacles, replans via the lane hash. **Population lives around the camera**
  (`TRAFFIC.spawnMin/spawnMax/despawn` in `config.js`, `managePopulation` in `world.js`).
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
- **Console and statistics:** `console.js` (pure: `tokenize`/`suggest`/`execute`/`consoleKey`, `placeIndex` from
  `overview.json` for `tp`) is opened by Enter in `main.js` (Action is E only); while open, `game.js` freezes the world.
  Console teleports carry `auto` and are confirmed by `game.js` once tiles are loaded. `stats.js trackStep` books world
  events + `game.statQueue` (teleport, cheat) into `game.stats.game`/`total`; `statsdb.js` persists them in IndexedDB
  (`applyStoredStats` merges the async answer). New counters need a row in `STAT_SECTIONS`; events that count must
  carry who caused them (`player`, `weapon`, `carId`).
- **Tree shadows:** `lighting.js treeShadowGeom` (pure) + `addTrunkShadow` (trunk strip in the opaque shadow path) +
  `crownShadowSprite` (crown sprite as black silhouette with gaps and blurred edge, stretched along the sun).
- **Mission** (`mission.js`) is a state machine; save (`save.js`) is one `localStorage` slot, auto-written after a
  completed mission, with `memoryStorage()` for tests and a corrupt-save path.
- **Assets:** all graphics/sounds are self-generated placeholders; real files can be swapped in via
  `web/assets/manifest.json` (keys, sizes and orientation in `web/assets/README.md`). Never use names/art/music from
  other games.

## Versioning and releases

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
