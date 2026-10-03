# Migration Prompt: Porting `gta-berlin-2d` from JS/HTML5 Canvas to Native Rust (wgpu)

Dieser Prompt ist für die Ausführung via **Claude Code CLI** oder **Codex** direkt im Wurzelverzeichnis des Repositories konzipiert.

---

## 🎯 Zielsetzung & Kontext
Das Repository enthält derzeit einen Top-down-Open-World-Prototyp (`gta-berlin-2d`) ganz Berlins (1:1 aus OpenStreetMap), der aktuell in JavaScript über die HTML5-2D-Canvas-API und Web-Audio läuft und für die Xbox in einer UWP/WebView2-Hülle verpackt ist. 

**Problem:** 
Auf der Xbox Series X|S (Dev Mode) bricht die Performance im Edge-Container/WebView2 massiv ein, da hunderte Gebäude, 250+ Passanten, Lichtkarten und Spuren per CPU-Software-Rasterizer im JS-Thread gezeichnet werden.

**Ziel:**
Portiere das gesamte Projekt strukturiert und schrittweise in eine **native Rust-Codebasis**, die:
1. **Lokal auf macOS (Apple Silicon)** blitzschnell und nativ mit **Metal** läuft (`cargo run`).
2. Als natives x86_64-Binary mit **DirectX 12** auf der **Xbox Series X|S (Dev Mode)** mit 60–120 FPS performt.
3. Die bestehenden Kartendaten (`web/data/berlin/` mit 2.920 Kacheln) direkt wiederverwendet.
4. Sämtliche Dokumentation (`README.md`, `CLAUDE.md`, `docs/`) an die neue Architektur und den neuen Mac/Xbox-Workflow anpasst.

---

## 🛠 Ziel-Architektur & Tech-Stack
* **Sprache:** Rust (Edition 2024 / stabil)
* **Grafik:** `wgpu` (WGSL-Shader, Metal-Backend auf macOS, DirectX-12-Backend auf Windows/Xbox)
* **Windowing & Input:** `winit` + Low-Latency Gamepad-Support via `gilrs`
* **Vektormathematik:** `glam` (SIMD-optimiert für SAT-Kollision und Projektion)
* **Audio:** `kira` oder `rodio` (für prozedurale/synthetisierte Klänge analog zu Web Audio)
* **Serialisierung:** `serde`, `serde_json` (zum Laden der Kacheln und Konfigurationsdateien)
* **Kompilierung & Toolchain:**
  * macOS: Nativ (`aarch64-apple-darwin`)
  * Xbox: Cross-Compilation direkt vom Mac via `cargo-xwin` (`x86_64-pc-windows-msvc`)

---

## 📋 Aufgaben & Ausführungsschritte für den KI-Assistenten

Arbeite die folgenden Phasen systematisch ab:

### Phase 1: Cargo Workspace & Grundgerüst
1. Erstelle ein modulares Rust-Workspace-Setup (z. B. `crates/engine`, `crates/game`, `crates/map_loader`).
2. Richte `winit` und `wgpu` ein: Fenster öffnen, Render-Loop, Swapchain-Konfiguration für Metal und DX12, klares Frame-Pacing (V-Sync / 60/120 FPS Target).
3. Implementiere eine 2D-Kamera mit schräger Draufsicht und Zoom-Stufen analog zu `src/render.js`.

### Phase 2: Kartendaten & Geometrie-Pipeline
1. Portiere `src/citycodes.js`, `src/geom.js` und `src/projection.js` nach Rust (`glam::Vec2`).
2. Implementiere den Kachel-Manager (`src/map.js`), der die bestehenden JSON-Kacheln aus `web/data/berlin/tiles/<x>_<y>.json` asynchron basierend auf der Kameraposition lädt und entlädt.
3. Ersetze das Canvas-Immediate-Mode-Zeichnen durch **GPU-Instancing & Batch-Meshes**:
   - Straßen- und Gleisflächen als zusammenhängende Vertex-/Index-Buffer.
   - Gebäude und Dächer (`src/roofs.js`, `src/buildcolors.js`) mit Sonnenstand-Schattierung via Vertex-/Fragment-Shader statt 2D-Canvas-Fills.
   - Texturierung / Decals (`src/decals.js`, `src/textures.js`) per Sprite-Atlas und Shader.

### Phase 3: Physik, Kollision & Spiellogik
1. Portiere die SAT-Kollisionserkennung und den Spatial-Grid-Hash (`src/collision.js`) mit Unit-Tests in Rust.
2. Portiere die Fahrzeugphysik (`src/car.js`): Lastverschiebung, ESP/ABS, Drift, Reifenhaftung je Untergrund/Wetter (Trocken, Nässe, Schnee, Glätte).
3. Portiere die Fußgänger- und Verkehrs-KI (`src/traffic.js`, `src/roadgraph.js`, `src/pedestrians.js`).
4. Portiere die Mission „Kisten für den Kiez“ (`src/mission.js`) und das Speichersystem (`src/save.js` – lokal per Datei oder SQLite statt IndexedDB).

### Phase 4: Shader & Beleuchtung (WGSL)
1. Portiere die Logik aus `src/daylight.js`, `src/lighting.js` und `src/lamps.js` in einen 2D-Beleuchtungs-Pass:
   - Sonnenlichtrichtung und Tag/Nacht-Zyklus.
   - Schattenwurf von Gebäuden und Bäumen via Geometrie-Extrusion im Vertex-Shader.
   - Lichtquellen (Laternen, Scheinwerfer, Blaulicht) im Fragment-Shader über ein Offscreen-Render-Target.

### Phase 5: Synthetisiertes Audio
1. Portiere die Web-Audio-Synthesizer aus `src/audio.js`, `src/ambience.js` und `src/soundscape.js` in Rust-Klangquellen (Oszillatoren, Rauschgeneratoren für Reifen/Wind, Motor-Drehzahl-Module mit Doppler-Effekt).

---

## 🚀 Deployment- & Build-Dokumentation für Mac und Xbox

Implementiere Tools und dokumentiere die folgenden Workflows detailliert:

### A. Lokale Entwicklung auf macOS (M1/M5)
Keine externen Windows-Abhängigkeiten. Alles läuft über Cargo:
```bash
# Debug-Build & Start mit nativer Metal-Pipeline
cargo run

# Release-Build für volle Performance
cargo run --release

# Automatische Tests ausführen
cargo test