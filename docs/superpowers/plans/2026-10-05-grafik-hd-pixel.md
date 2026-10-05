# Grafik HD + Pixel-Modus (native Fassung) – Umsetzungsplan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking. Nach jeder Phase **STOPP** und Rückmeldung an den Nutzer.

**Goal:** HD-Look als Standard, Pixel-Modus als eigener Render-Pfad, zur Laufzeit umschaltbar; Simulation unberührt.

**Architecture:** Renderer zeichnet die Szene in ein Offscreen-Ziel (HD: `Rgba16Float` + MSAA, Pixel: kleines 1×-Ziel mit
lesbarer Tiefe) und bringt sie per Post-Pass ins Swapchain-Bild; HUD/Minikarte danach unverändert. Modus und Qualität
kommen vom Spiel über `Game::graphics()`.

**Tech Stack:** Rust 2024, wgpu 29 (Metal/DX12/Vulkan, nur Kernfunktionen), WGSL; Python-Build-Skripte für Assets
(Muster `tools/audio/`); keine neuen Crates ohne Rückfrage.

**Spec:** `docs/superpowers/specs/2026-10-05-grafik-hd-pixel-design.md`

## Global Constraints

- Nach jedem Teilschritt: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace` – Commit nur bei beidem grün.
- Deutsche UI und Doku; Eingaben nur über `game/bindings.rs`; nie Zustand in `interp.rs`/Sim ändern (Einrasten nur im
  Uniform); kein WGSL-Bezeichner `half`; nur wgpu-Kernfunktionen oder sauberer Fallback.
- Assets nur CC0/OFL, über Build-Skript + Manifest + Test; `about.rs` liest die Credits.
- Nach jeder Phase: `tools/gfx/captures.sh <NN-name>` (ab Phase 1 mit `MODI="hd pixel"` bzw. nur `hd`, bis es Pixel gibt),
  Bildvergleich, Messwerte in `metrics.json`; CHANGELOG + NATIVE-RUST.md + CLAUDE.md knapp nachziehen; Commit + Push.
- Budget: HD Hoch < 8 ms, Mittel < 5 ms GPU (Boulevard, 2560 × 1440, M1 Pro); Pixel ≤ Baseline.
- Messung: `--fenster 2560x1440 --messung X.json` (Werkzeug aus Phase 0); `--zoom`/`--geo` wirken auch im Spiel.

## Phase 0 – Bestandsaufnahme ✅ (05.10.2026)

- [x] Befund am Code geprüft (Spec, Abschnitt „Befund“)
- [x] Messwerkzeug: `--fenster`, `--messung` (GPU-Zeitstempel), `--geo`/`--zoom` im Spiel
- [x] `tools/gfx/captures.sh`, Baseline `docs/images/native/grafik/00-basis/` + `metrics.json`
- [x] Spec und Plan

## Phase 1 – Render-Architektur und Umschalter ✅ (05.10.2026)

**Dateien:** `engine/src/{lib.rs,renderer.rs,lightpass.rs,lighting.wgsl}`, neu `engine/src/graphics.rs` (Modus,
Qualität, Ziele), `game/src/{play.rs,menu.rs,bindings.rs,console.rs,main.rs}`, `tools/gfx/diff.sh`.

- [x] **1.1 Typen:** `berlin_engine::graphics::{GraphicsMode, Quality, GraphicsSettings}` (Hd/Pixel, Niedrig/Mittel/
      Hoch, `msaa()` = 1/4/4, Standard Hd + Hoch); `Game::graphics() -> GraphicsSettings` mit Standard-Rumpf.
      Test: Standardwerte, `msaa()`.
- [x] **1.2 Einstellungen:** `settings.json` bekommt `"grafik": "hd"|"pixel"` und `"qualitaet": "niedrig"|"mittel"|"hoch"`;
      fehlend/unbekannt → Standard. Tests: Roundtrip, alte Datei ohne Felder, Unsinnswerte.
- [x] **1.3 Bedienung:** Aktion `Action::GraphicsMode` (Standard **F8**, Pad ohne), Konsole `grafik [hd|pixel]`
      (ohne Argument: umschalten) und `qualitaet [niedrig|mittel|hoch]`, CLI `--grafik hd|pixel` und
      `--qualitaet …` (überschreibt die Datei nicht), Menüpunkt „Grafik: HD / Pixel“ in Titel- und Pausenmenü
      (← → bzw. Klick schaltet um, wie das Steuerschema). Speichern bei jeder Änderung. Tests: Belegungstabelle ohne
      Doppelbelegung, Konsole, Menü-Eintrag vorhanden.
- [x] **1.4 HD-Ziele:** `Rgba16Float`-Farbziel mit `sample_count = msaa`, multisampled `Depth32Float`, Resolve-Ziel
      `Rgba16Float`; alle Szenen-Pipelines (`pipeline`, `window_pipeline`, `sprite`, `body`, `silhouette`, `effect`,
      Composites aus `lightpass`) mit Zielformat `Rgba16Float` und `MultisampleState { count: msaa }`; bei Wechsel der
      Qualität neu bauen. Schattenmaske/Lichtkarte bleiben 1× (werden nur gesampelt).
- [x] **1.5 Post-Pass:** `post_fs` (Vollbild-Dreieck, Quelle = Resolve-Ziel): `grade_fs`-Rechnung hierher verlegen
      (statt MULTIPLY-Blend jetzt Multiplikation im Shader), Ergebnis auf 0…1 klemmen; schreibt ins Swapchain-Bild.
      Kritische Szenen-Shader (Bloom/SCREEN) klemmen ihre Ausgabe, damit Phase 1 bildgleich bleibt.
- [x] **1.6 HUD + Minikarte:** HUD-Durchgang aufs Swapchain-Bild mit eigener 1×-Tiefe; Kachel-Pipeline-Variante
      `map_pipeline` (Swapchain-Format, 1×) für die Minikarte.
- [x] **1.7 Aufnahme:** `capture()` zeichnet Szene + Post + HUD wie `render()` (gemeinsame Funktion), liest das finale
      Bild. `--fenster` nutzt dasselbe Offscreen-Ziel.
- [x] **1.8 Pixel-Platzhalter:** bis Phase 8 zeichnet `Pixel` wie HD mit MSAA 1 (Modus schaltbar, kein Absturz) –
      ausdrücklich als Platzhalter dokumentiert.
- [x] **1.9 Bildvergleich:** `tools/gfx/diff.sh A B` (ImageMagick `compare`, Fuzz 2 %: Anteil abweichender Pixel und
      mittlere Abweichung); Prüfung: Qualität Niedrig (MSAA 1) gegen Baseline < 0,5 % Pixel, Hoch nur an Kanten
      abweichend (Sichtprüfung der Differenzbilder aller 9 Szenen).
- [x] Abweichungen vom Entwurf: Ergebnis siehe NATIVE-RUST.md „Grafik HD/Pixel: Phase 1“ (Pipelines/Ziele in
      `engine/src/scenepass.rs`; die Composites hängen am Szenenziel, Schatten-/Lichtkarte bleiben 1×; Aufnahmen
      laufen schrittgebunden; Mittel = Hoch bis Phase 7).
- [x] **1.10** Messung `MODI="hd" tools/gfx/captures.sh 01-architektur` (+ `QUALITAET=mittel`), Doku, Commit, Push.
      **STOPP.**

## Phase 2 – Materialsystem Boden ✅ (05.10.2026)

Umgesetzt (Abweichungen vom Entwurf fett): Build-Skript mit SHA-256-Prüfung und Manifest; je Material **zwei** Arrays
(Detail = Farbe/periodisch geglättete Farbe, **Hochpass gegen wiederkehrende Flecken**; Normale + Rauheit +
**Umgebungsverdeckung** gepackt) mit CPU-Mips; Parameter je Material-ID als Uniform (Gruppe 2 der Kachel-Pipelines);
Abtastung mit `textureSampleGrad` (Ableitungen vor den Verzweigungen); **Hintergrundboden** (`ground_fs`, ID 14) für
Stellen ohne Fläche, nach den Kacheln mit Tiefentest; Nässe über `Lighting.wet`. Anisotropie aus (bringt in der
Draufsicht nichts). Pixel-Modus und Qualitätsstufen nutzen die Texturen noch unverändert (Phase 7/8).

Ursprünglicher Entwurf:

- `tools/gfx/build_materials.py` (Quelle ambientCG/Poly Haven, CC0, SHA-256 je Download, Ausgabe 512² Albedo/Normal/
  Roughness je Material als PNG unter `data/gfx/materials/` + `manifest.json` mit Kachelgröße in m, Quelle, Lizenz).
- Engine: `texture_2d_array` je Kanal mit CPU-Mips; Material-Index aus JSON `data/gfx/material_map.json`
  (Material-ID → Arrayschicht, Maßstäbe), Test: jede in `mesh.rs` erzeugte ID hat einen Eintrag.
- `fs`: Abtastung `uv/px_per_m/kachel`, Anti-Kachelung (Zellrotation/-versatz über `vnoise`, zwei Maßstäbe überblendet),
  Normal Map gegen `camera.sun`, Roughness → Glanz bei Nässe (Wetter-Uniform). Gebrauchsspuren/Moos bleiben Overlay.
  Minikarte (`params.y > 0.5`) unverändert flach. Pixel-Modus: Albedo-Mittelwert statt Textur (entscheidet Phase 8).

## Phase 3 – Kanten, Übergänge, Kontaktschatten ✅ (05.10.2026)

Umgesetzt: Bordstein + Fuge (`mesh.rs curb`, auch als Ringe an Kreuzungen), Randstreifen außen am Gehweg, Rasenrand
(`fringe`, Material 16, Lage quer als eigene interpolierte Größe `across`), Markierungen als neues Merkmal
`Feature::Marks` (aus `crossings`/`signals`/`vertices.trim` der Kacheln, `format.rs markings`, gezeichnet mit der weißen
Atlaszelle), Fahrradpiktogramme, Laub; Atlas 256 px × 4 × 4 mit Mips, `atlas::shader_constants`. Kontaktschatten
gab es schon (Atlaszelle 9 am Fuß jeder Wand) – mit dem größeren Atlas jetzt glatter, sonst unverändert.

Ursprünglicher Entwurf:

- Bordstein als eigener Streifen in `mesh.rs road_mesh` (zwischen Gehweg-Strich und Fahrbahn, heller, dunkle Fuge).
- Randverlauf Gras/Erde ↔ befestigt (Alpha/Mischband an Polygonkanten), Kontaktschatten am Gebäudefuß aus den
  Wand-Quads, Markierungen (Haltelinien, Zebrastreifen, Radwege) soweit Tags vorhanden.
- Decal-Atlas 256-px-Zellen mit Mips, Raster als Uniform/Konstante; `sprite_vs` und `tree_shadow_vs` mitziehen; neue
  Decals (Gullis, Flicken, Risse, Ölflecken, Laub).

## Phase 4 – Dächer und Fassaden ✅ (05.10.2026)

Umgesetzt: sieben weitere CC0-Texturen; neue Fassaden-IDs (Klinker 18/19, Beton 20/23 neben Putz 11/12) aus
`mesh.rs facade_material`; Fassaden und Dächer mit einem Maßstab (`surface_sample`), Wandrelief in der Tangentenbasis
der Wand; Fensterraster `engine/facade.rs` als WGSL-Konstanten, `window_cell`/`in_glass`/`glass_bar` für `fs` und
`window_fs`; Ausblenden der Fenster nach Bildpunkten je Zelle statt nach `detail`; Tür 13, Schaufenster 21 und
Ladenband 22 mit Ortskoordinaten (`local`). Roh-Vorlagen, die nicht quadratisch sind (Klinker 2:1), stapelt das
Build-Skript zum Quadrat.

Ursprünglicher Entwurf:

- Dachmaterialien 6–9 als Texturen (aus Phase 2), Moos/Ruß bleibt; Fassaden 11/12 Putz/Klinker, Rahmen, Simse;
  **ein** Fensterraster (Rust-Konstante → WGSL), Test Gleichheit `fs`/`window_fs`; Material 13 aufwerten.

## Phase 5 – Fahrzeuge ✅ (05.10.2026)

Umgesetzt: Atlas 512 × 256, Raster aus `engine/vehatlas.rs` (WGSL-Konstanten, Zellgröße aus der Atlasbreite statt
Uniform); Glanzmaske G/B in der Lackzelle; `body_gloss` mit Sonnenglanz und Himmelsreflex nach Fahrzeugwinkel;
Motorräder mit Glanzformen 6/7. Lichtkarten-Reflex nachts → Phase 7.

Ursprünglicher Entwurf:

- Zellen 512 × 256, Zellgröße/Raster als Uniform in `vehicle()`/`sprite_cover()` (Silhouetten-Test: gleiche Deckung
  wie vorher); Glanzmaske in G/B der Lack-Zelle (Spec, Entscheidung 6); Sonnenglanz + Umgebungsreflex mit dem
  Fahrzeugwinkel, nachts Lichtkarten-Reflex; `motoart.rs` analog. Abgestimmt auf die Fahrzeugüberarbeitung vom
  05.10.2026 (`paint_car`, `proportions`).

## Phase 6 – Bäume und Figuren ✅ Bäume (05.10.2026), Figuren nur Vorschlag

Umgesetzt: Kronenzellen Linde (12), Platane (13), Kastanie (14), Kiefer (15) im Decal-Atlas, die beiden alten Kronen
(0 Laub, 6 Nadel) mit demselben Erzeuger neu (`atlas.rs crown`: Blattballen als Kugeln auf einer Kuppel, Kern ohne
Ballen gefüllt, Blattkörnung aus Wertrauschen, Nadeln radial gestreckt, weiche Lücken bei Platane und Kiefer). Die
Zellen tragen kein Licht mehr: R = Helligkeit ohne Sonne, G/B = Normale; `sprite_fs crown_light` dreht die Normale mit
dem Baum und hellt die Sonnenseite auf (bei Bedeckung und nachts gleichmäßig). Gattung → Zelle und Laubfarbe in
`mesh.rs tree_look` (Nadelbäume zu 65 % Kiefer); `is_crown` kommt als WGSL-Funktion aus `atlas.rs` und gilt auch für
den Baumschatten.

### Figuren-Atlas – Variante B umgesetzt ✅ (06.10.2026)

Freigegeben („mach wie du es empfiehlst“): Teile als Teilzellen eines Zellenpaars im Fahrzeugatlas
(`game/figart.rs`, Form `FIG_BASE + …`), Details in NATIVE-RUST.md. Ursprünglicher Vorschlag:

Heute: `game/figure.rs` setzt jede Person aus 8–14 Bodies zusammen (Ellipsen und Rechtecke mit flacher Farbe: Rumpf,
Arme, Kopf, Haare, Tasche, Zubehör), 615 Zeilen, Gangbild aus `pose()`. Von oben bei Zoom 1,2 trägt das; bei 2,6 und
im HD-Modus wirken die Figuren neben Autos und Bäumen flach.

- **Variante A – Glanz/Licht auf den bestehenden Bodies (≈ 0,5 Tag):** Kopf, Schultern, Taschen als gewölbte Formen
  (wie Motorradteile, `shape` 6), Licht von der Sonne. Keine neuen Daten, kein Atlas; Figuren bleiben schematisch.
- **Variante B – Teil-Atlas (≈ 2 Tage, empfohlen):** gemalte Draufsicht-Teile im Stil von `carart` (Lack/Detail mit
  Normale): Schultern/Rumpf in 3 Statur-Formen, 6 Frisuren, 5 Kopfbedeckungen, Tasche/Rucksack, Kinderwagen, Hund
  (2 Größen). Farbe weiter je Person aus `look_of`, das Gangbild bleibt (Teile werden wie heute verschoben). Etwa
  24 Zellen à 128², ≈ 1,6 MB mit Mips. Pixel-Modus nutzt dieselben Zellen (Phase 8 quantisiert).
- **Variante C – Ganzkörper-Sprites je Typ und Gangphase (≈ 4–5 Tage):** 12 Typen × 8 Phasen × Aktionen; schönstes
  Ergebnis, aber Farbe und Zubehör ließen sich nicht mehr frei kombinieren, und der Atlas wächst auf ≈ 25 MB.

Ursprünglicher Entwurf:

- Linde, Platane, Kastanie, Kiefer als 256-px-Zellen, Licht-/Schattenseite zur Sonne. Figuren: nur **Vorschlag** mit
  Aufwand für einen Figuren-Atlas (nicht umsetzen ohne Freigabe).

## Phase 7 – Licht und Post ✅ (05.10.2026)

Umgesetzt: AgX im Post (Belichtung abgeglichen, Sättigung 1,2), HDR-Bloom ½/¼ ab Mittel, Lichtkarte ab Mittel bis 1,8,
gefilterte Schattenkante, alles über `post_level`. Budget wegen laufender VM nicht belastbar gemessen (s.
NATIVE-RUST.md); Lichtkarten-Reflex im Lack zurückgestellt.

Ursprünglicher Entwurf:

- Tonemapping (Vorschlag AgX) im Post-Pass, Grade neu abgestimmt; Bloom aus der HDR-Szene (Downsample-Kette ½/¼);
  Schattenmaske gefiltert; nasse Straßen über Roughness; alles hinter Qualitätsstufen; Niederschlag-Kosten prüfen.

## Phase 8 – Pixel-Modus ✅ (06.10.2026)

Umgesetzt wie entworfen (kleines Ziel, Tiefe als Textur, Kamera-Einrasten, Kontur, 43-Farben-Palette mit 32³-LUT,
Bayer 4×4, ganzzahliges Vergrößern mit Rand, HD-Effekte aus); Palette/LUT/Quantisierung in `engine/palette.rs`
getestet. Zusätzlich: Quantisieren in Szenengröße, nur das Vergrößern in voller Größe (sonst langsamer als vorher).

Ursprünglicher Entwurf:

- Kleines Ziel (Standard: Bildpunkt = 3–4 Bildschirmpunkte bei 1080p), Tiefe als Textur, Kamera-Einrasten im Uniform,
  Kontur-Pass über Tiefensprünge, LUT 32–48 Farben (handkuratiert aus `buildcolors.rs` und `mesh.rs`, Datei
  `data/gfx/palette.json`), Bayer 4×4 (Stärke einstellbar), Nearest-Upscale ganzzahlig + Letterbox, HD-Effekte aus.
  Palette/LUT/Quantisierung als reine, getestete Rust-Funktionen. Bildvergleich aller Szenen. **STOPP.**

## Phase 9 – HUD-Schrift im HD-Modus ✅ (06.10.2026)

Umgesetzt mit Inter SemiBold (OFL) als SDF-Atlas aus `tools/gfx/build_font.py`, gemeinsamer Atlas mit der Bitmap,
Schrift-Abstraktion über `Hud.sdf` (Metriken aus der Schrift), Kontur als eigene SDF-Form, Credits aus dem Manifest.
Pixel behält font8x8; alle Bildschirmseiten in beiden Modi geprüft. Straßenschilder in der Welt: offen (Notiz).

Ursprünglicher Entwurf:

- OFL-Schrift (Vorschlag Inter / IBM Plex Sans / Atkinson Hyperlegible) als SDF-Atlas, erzeugt von
  `tools/gfx/build_font.py` (Pillow, eingebettet + Manifest), Laufweiten aus dem Font; Schrift-Abstraktion in
  `engine/hud.rs` (Metriken statt `advances()`), Pixel behält font8x8; alle `--bildschirm`-Seiten in beiden Modi prüfen.
  Notiz: Straßenschilder zurück in die Welt.
