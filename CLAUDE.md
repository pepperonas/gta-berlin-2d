# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

A small top-down open-world game (HTML5 Canvas 2D + Web Audio, plain ES modules, **zero dependencies**) that runs in the
browser on the Mac and inside a C# UWP/WebView2 shell for an Xbox Series X|S in Developer Mode (private sideloading).
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
```

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
- **Map:** `map.js` generates the city from a replaceable `LAYOUT` (block grid sizes in `config.js`); `traffic.js`
  owns lane/intersection logic (right-hand traffic) and exports the grid helpers (`NV`, `NH`, `neighbor`, `DIRS`) that
  the test autopilot reuses. Collision: `collision.js` (circle/rect/SAT OBB + `SpatialHash`).
- **Mission** (`mission.js`) is a state machine; save (`save.js`) is one `localStorage` slot, auto-written after a
  completed mission, with `memoryStorage()` for tests and a corrupt-save path.
- **Assets:** all graphics/sounds are self-generated placeholders; real files can be swapped in via
  `web/assets/manifest.json` (keys, sizes and orientation in `web/assets/README.md`). Never use names/art/music from
  other games.

## Tests

`node:test` only. `tests/helpers/bot.js` is an **autopilot** that plays the full mission through the same abstract
inputs a player uses (BFS route over intersection centers); there is also a long soak test with traffic and
pedestrians. Keep the simulation free of DOM access so these keep running in Node. When the test count or verified
behaviour changes, update the "Stand und Prüfumfang" section of `README.md` accordingly.
