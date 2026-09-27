# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

A top-down open-world game (HTML5 Canvas 2D + Web Audio, plain ES modules, **zero dependencies**) set in **Kreuzberg and
Nord-Neukölln at 1:1 scale, built from OpenStreetMap**. It runs in the browser on the Mac and inside a C# UWP/WebView2 shell for an Xbox Series X|S in Developer Mode (private sideloading).
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
npm run map:fetch                      # LOR boundaries (Geoportal WFS) + OSM (Overpass) → data/raw/ (gitignored, needs network)
npm run map:build                      # data/raw/ + data/places.json → web/data/city.json (deterministic, ~3 s)
npm run map:preview -- out.svg [x y w h]   # SVG of the map (or a px window) for visual checks
```

`web/data/city.json` is committed; only rebuild it when the data or `data/places.json` (mission spots) should change.
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
- **Xbox shell bridge:** the Web Gamepad API is broken in UWP WebView2, so the C# shell reads `Windows.Gaming.Input`
  and posts a reading every 8 ms via `PostWebMessageAsJson` (`{type:'gamepad', pads}`); `fromHostReading` converts it
  to a standard gamepad. The page sends `{type:'ready'}` and `{type:'quit'}` (menu "Beenden", only shown when
  `chrome.webview` exists → `canQuit`). The shell also swallows `BackRequested` and `VirtualKey.Gamepad*` so B doesn't
  close the app and focus stays in the WebView. Changing the message format means changing both sides.
- **Map is data, not code.** `tools/osm/build.mjs` projects OSM + LOR boundaries (transverse Mercator, 10 px = 1 m,
  north up) into a compact delta-encoded JSON (road graph with class/width/name/oneway/bridge, buildings with holes and
  heights, water, areas, walls, trees, border, districts, mission places + route-based `timeLimit`). Codes shared with the
  game live in `web/src/citycodes.js`, geometry helpers in `web/src/geom.js` (imported by the build too).
  `map.js` `decodeCity()` builds `SpatialHash`es: `render` (culling), `edgeSegs` (road queries), `polys` (surface /
  in-building), `solids` (collision). Queries: `surfaceAt` (replaces the old tile lookup; `car.js` uses it for friction),
  `inBuilding`, `insideBorder`, `districtAt`, `nearestEdge`, `locationName`. The decoded city is shared read-only by the
  title demo world and the game world; `createWorld({ city })` / `createGame({ city })` / `setCity()` take it explicitly
  (the browser loads it async in `main.js`).
- **Invariant: no tree trunk on a carriageway** (crowns may overhang). `keepTreesOffRoads` in the build pushes trunks to
  the curb or drops them and throws if any violation remains; `decodeCity` drops violators as a safety net
  (`city.droppedTrees` must stay 0); tests check the shipped map, the fixture and the safety net. Constants
  (`TREE_TRUNK_M`, `TREE_FREE_MAX_CLASS`) live in `citycodes.js`, shared by build and game.
- **POIs and house numbers** come from the same Overpass fetch: `city.pois` (`POI_CATS` in `citycodes.js`, stations
  deduped by name), `city.addresses`; queries `nearestPoi` / `nearestAddress`; `locationName` appends the house number.
  Labels are drawn in screen space by `Renderer.drawPois` (capped, overlap-culled).
- **Collision** is wall *segments* (building rings, quays cut open at bridges, rail lines, bridge railings, the district
  border) plus tree circles and crate rects, via `circleVsSegment` / `obbVsSegment` in `collision.js`. The SAT depth is the
  shortest escape distance (`min(a1-b0, b1-a0)`), which matters for zero-thickness walls.
- **Traffic:** `roadgraph.js` turns drivable in-area edges (`cls <= TRAFFIC_MAX_CLASS`) into one lane per direction
  (offset right by w/4, trimmed at junctions, Bezier connectors, no U-turns except dead ends); `traffic.js` follows the lane
  polyline with pure pursuit, slows for turns/obstacles, replans via the lane hash. **Population lives around the camera**
  (`TRAFFIC.spawnMin/spawnMax/despawn` in `config.js`, `managePopulation` in `world.js`).
- **Pedestrians** walk along road edges at a per-side sidewalk offset (cached, shrunk if it would hit a building), pick the
  next edge at nodes, cross streets, and wait for approaching cars.
- **Mission** (`mission.js`) is a state machine; save (`save.js`) is one `localStorage` slot, auto-written after a
  completed mission, with `memoryStorage()` for tests and a corrupt-save path.
- **Assets:** all graphics/sounds are self-generated placeholders; real files can be swapped in via
  `web/assets/manifest.json` (keys, sizes and orientation in `web/assets/README.md`). Never use names/art/music from
  other games.

## Tests

`node:test` only. Tests load the real map once per process via `tests/helpers/city.js` (`realCity()`, ~0.5 s).
`tests/helpers/bot.js` is an **autopilot** that plays the full mission through the same abstract inputs a player uses
(A* over the real road graph); there are also soak tests with traffic/pedestrians, full-throttle ram tests against walls,
quays and the border (checked every step), and `tests/osm-build.test.js`, which runs the build on a tiny synthetic OSM
fixture. Keep the simulation free of DOM access so these keep running in Node. When the test count or verified
behaviour changes, update the "Stand und Prüfumfang" section of `README.md` accordingly.
