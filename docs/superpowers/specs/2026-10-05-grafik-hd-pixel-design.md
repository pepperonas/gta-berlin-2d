# Grafik: HD als Standard, Pixel-Modus als zuschaltbarer Filter (Rust-Fassung) – Design

Stand 05.10.2026, Phase 0 (Bestandsaufnahme) abgeschlossen. Plan: `docs/superpowers/plans/2026-10-05-grafik-hd-pixel.md`.
Gilt nur für die native Fassung (`crates/`); `web/src` bleibt unverändert, `web/data` wird nur gelesen.

## Ziel

- Standard ist ein deutlich hochwertigerer, detaillierter **HD-Look** (Materialtexturen, Kantenglättung, HDR-Licht,
  Tonemapping), eigenständig gestaltet – die JS-Fassung ist nur Logik-Referenz, keine Optik-Vorlage.
- **Pixel-Modus**: echter Retro-Look als eigener Render-Pfad über dieselbe Szene (niedriges internes Raster, Konturen,
  feste Palette, Dithering, ganzzahliges Hochskalieren), HUD mit der bestehenden font8x8-Schrift.
- Umschalten zur Laufzeit ohne Neustart: Menü, frei belegbare Aktion, Konsole `grafik`, CLI `--grafik hd|pixel`;
  Einstellung bleibt in `settings.json` erhalten.
- Simulation, Kollision, Physik, Zufallsfolge, Spielstandformat: **unberührt**. Nur Darstellung.

## Befund (in Phase 0 am Code geprüft)

Alle Punkte der Vorab-Analyse treffen zu. Ergänzungen und Präzisierungen sind mit **neu** markiert.

### Render-Ablauf (`crates/engine/src/renderer.rs` `draw_scene`)
1. Schattenmaske (R8, volle Auflösung, Blend MAX): Hauswände (`shadow_vs`, Extrusion entlang der Sonne) und
   Baumkronen (`tree_shadow_vs`, nur Atlaszellen 0 und 6) – nur wenn `shadow_strength > 0.02`.
2. Lichtkarte (RGBA16F, halbe Auflösung, Blend ADD), mit Umgebungslicht gelöscht – nur nachts (`dark > 0`).
3. **Ein** Hauptdurchgang direkt ins Swapchain-Bild (sRGB-Format, `sample_count 1`): Kacheln (`fs`), Schatten auftragen
   (Vollbild-Dreieck in Tiefe 0,5, Tiefe `Less` = nur Boden), Sprites (Bäume/Decals), Bodies (Autos/Figuren),
   nachts Lichtkarte (MULTIPLY, `Less`) + Umgebungslicht auf Dächern (MULTIPLY, `GreaterEqual`) + Bloom (SCREEN),
   erleuchtete Fenster (`window_fs`, zweiter Durchgang über die Kachelmeshes), Silhouetten (`Greater`), Effekte,
   zuletzt `grade_fs` (MULTIPLY).
4. HUD-Durchgang auf dasselbe Bild, Tiefe neu gelöscht; dazwischen die Minikarte per Viewport/Scissor mit
   **derselben** Kachel-Pipeline (`params.y = 1`) bzw. dem Stadtplan (`overlay.wgsl`).

- **neu:** Die „Tiefe“ ist eine **Schichtnummer** (Vertexattribut, z. B. Boden 0,85–0,98, Brücken 0,68…, Dächer
  `0.45 − center.y/500000·0.2`), keine geometrische Tiefe. Alle Composites hängen daran. Für die Pixel-Konturen ist
  das gut: Sprünge zwischen Schichten sind genau die gewünschten Kanten.
- **neu:** Das sRGB-Ziel klemmt heute jedes Zwischenergebnis auf 0…1. Ein Float-Ziel klemmt nicht – `SCREEN`
  (`OneMinusDst`) wird bei Werten > 1 negativ. Siehe Risiken.
- **neu:** Der Baum-/Decal-Atlas hat `mip_level_count 1`, obwohl der Sampler Mip-Filter setzt → Kronen flimmern beim
  Herauszoomen. Der Fahrzeugatlas hat 4 Mip-Stufen (CPU-Kastenfilter, nach Deckkraft gewichtet).
- **neu:** `capture()` zeichnet die Szene ein zweites Mal in eine eigene Textur und liest sie zurück; das bleibt der Weg,
  er muss nur künftig den Post-Pass mitnehmen.

### Flächen (`scene.wgsl fs`) und Zuordnung (`map_loader/src/mesh.rs`)
| ID | Bedeutung (geprüft) | Quelle |
|---|---|---|
| 0 | Linien/Markierungen, Glasdach | `mesh.rs` Fahrbahnränder, `roof_mat::GLASS` |
| 1 | Asphalt | Straße `surface 0` |
| 2 | Kopfsteinpflaster | Straße `surface 1` |
| 3 | Gehwegplatten, Platz, Brückenfläche | Gehweg-Streifen unter jeder Straße (`class ≤ 9`), `PLAZA`, `BRIDGE` |
| 4 | Gras (auch Kleingärten, Friedhof, Wald, Sportplatz, **Gründach**) | Flächen + `roof_mat::GREEN` |
| 5 | Wasser | Wasserflächen |
| 6 / 7 / 8 / 9 | Ziegel / Schiefer / Blech / Flachdach (auch Berliner Dach und Mansarde) | `roof_mat`, `Style` |
| 10 | Gleis, Sand, unbefestigte Straße (`surface 3`) | `RAIL`, `SAND` |
| 11 / 12 | Fassade Wohnen / Arbeit | `building_kind` |
| 13 | Tür, Ladenfront | Fassadendetails, eigene Höhenprojektion im `vs` |

- **neu:** Es gibt **keine Bordsteinkante**: der Gehweg ist ein breiterer Strich (`width + 2·2 m`, Material 3) unter der
  Fahrbahn; die Kante ist der harte Wechsel zwischen den beiden Strichen.
- Licht: `0.60 + 0.40·max(0, n·sun)` je Fragment aus der Vertexnormale; `in.uv` = Weltpixel, `params.x` = px/m.
- Fensterraster: Zelle 2,5 × 3,0 m, Scheibe x 0,35–0,72 / y 0,28–0,80, **wörtlich doppelt** in `fs` und `window_fs`.

### Atlanten, Fahrzeuge, Figuren, HUD
- Baum-/Decal-Atlas `engine/atlas.rs`: 4 × 3 Zellen à 64 px, 10 belegt; `/64` und `4 × 3` stehen hart in
  `sprite_vs` **und** `tree_shadow_vs` (`lighting.wgsl`).
- Fahrzeugatlas (`game/carart.rs` + `raster.rs`): je Modell zwei Zellen (Lack-Schattierung in R + Deckkraft, Details
  RGBA), 256 × 128, 8 Zellen je Zeile – live gemessen **2048 × 1408 px** (44 Einträge). Zellgröße und „8 je Zeile“ hart
  in `vehicle()` und `sprite_cover()`. **neu:** Der Lack-Layer nutzt nur R und A – G und B sind frei.
- Motorräder `game/motoart.rs` (Teile als Bodies), Figuren `game/figure.rs` (Ellipsen/Rechtecke als Bodies).
- HUD `engine/hud.rs`: font8x8 (Basic + Latin-1 + Eigenzeichen) im 128²-R8-Atlas, Laufweiten `advances()`/`lefts()`
  aus den Bitmaps, Layout in 720er-Basiseinheiten (`game/hud.rs`); Straßenschilder als HUD-Text (`streetfurn.rs`).

### Eingaben und Einstellungen
- **neu:** F6 ist belegt (`Action::DebugExport`, „Physik-Änderungen ausgeben“). Frei sind F1, F2, F8–F12.
  **Vorschlag: F8** für „Grafik: HD / Pixel“.
- `settings.json` (`play.rs`, neben dem Spielstand): Steuerschema + abweichende Belegung; bekommt `"grafik"` und
  `"qualitaet"`.

### Messwerkzeug (in Phase 0 gebaut)
- `--fenster BxH`: feste Zeichengröße, das Bild entsteht abseits des Fensters (macOS begrenzt Fenster auf den
  Bildschirm – auf dem 1080p-Monitor kamen sonst 1920 × 1018 heraus). Das Fenster bleibt dabei dunkel.
- `--messung DATEI.json`: CPU-Arbeit je Bild (ohne Warten auf das Swapchain-Bild) und GPU-Zeit per Zeitstempel
  (Anfang erster bis Ende letzter Durchgang; `TIMESTAMP_QUERY` nur angefordert, wenn gemessen wird), Median und P95
  nach 60 Aufwärmbildern.
- `--geo LAT LON` und `--zoom Z` wirken jetzt auch im Spiel (Sprung wie `tp`, Kamera fest) – nötig für feste Szenen.
- `tools/gfx/captures.sh PHASE` (Variablen `MODI`, `SZENEN`, `ZOOMS`, `FRAMES`): 9 Szenen × Zoom 1,2/2,6, Seed 7,
  feste Uhr und festes Wetter, 2560 × 1440. Verlustfreie PNG nur lokal (`png/`, gitignored, je ~4 MB), eingecheckt
  WebP q85 (~0,3 MB). Messwerte in `docs/images/native/grafik/metrics.json` (Phase → Modus → Szene → Zoom).

## Baseline (Phase 0, Apple M1 Pro, Metal, 2560 × 1440, 300 Bilder)

| Szene | Ort, Uhr, Wetter | CPU Median/P95 ms (z 1,2) | GPU Median/P95 ms (z 1,2) | GPU (z 2,6) |
|---|---|---|---|---|
| block | Oranienstraße, 12:30, klar | 1,66 / 1,96 | 4,28 / 4,48 | 4,01 / 5,34 |
| boulevard | Kurfürstendamm, 19:30, klar | 1,95 / 2,23 | 3,72 / 5,08 | 3,51 / 4,46 |
| park | Neuer See, 13:00, klar | 1,27 / 1,49 | 3,36 / 3,88 | 3,13 / 3,77 |
| spree | Oberbaumbrücke, 21:00, klar | 1,51 / 1,88 | 4,83 / 5,21 | 4,83 / 5,77 |
| regennacht | Reuterkiez, 23:30, Starkregen | 1,93 / 2,14 | 6,04 / 8,80 | 6,07 / 7,80 |
| schnee | Kollwitzplatz, 10:00, Schnee | 1,92 / 2,20 | 6,20 / 7,32 | 6,06 / 7,38 |
| bahnhof | Hermannplatz U8, 13:00 | 1,63 / 2,15 | 4,79 / 5,39 | 4,52 / 5,01 |
| autos | Tempelhofer Feld, `--bildschirm autos` | 1,16 / 1,29 | 2,68 / 4,03 | 2,78 / 3,80 |
| leute | Spielerstart, `--bildschirm leute` | 1,55 / 1,85 | 3,44 / 4,19 | 3,45 / 3,71 |

Bilder: `docs/images/native/grafik/00-basis/`. Beobachtungen: Zoom kostet kaum (die Arbeit ist Füllrate je Bildpunkt,
nicht Geometrie); Regen und Schnee sind die teuersten Szenen (Niederschlag als viele halbtransparente Bodies), ihr P95
liegt schon heute nahe 8 ms. Vorbehalt: Bei 60 Bildern/s ist die GPU kaum ausgelastet und taktet herunter – die
Zeitstempel sind dann eher zu hoch als zu niedrig; Vergleiche immer unter gleichen Bedingungen (gleiche Szene, gleiche
Bildrate). Wiederholungsläufe streuen um etwa ±5 %.

## Architektur-Entscheidungen

1. **`GraphicsMode { Hd, Pixel }` + `Quality { Niedrig, Mittel, Hoch }`** leben im Spiel (`settings.json`), die Engine
   bekommt sie über eine neue `Game`-Methode `graphics() -> GraphicsSettings` (einmal je Bild abgefragt, wie
   `lighting()`); der Renderer baut Ziele/Pipelines nur bei Änderung neu.
2. **HD-Pfad:** Szene in ein Offscreen-Ziel `Rgba16Float` mit MSAA (Hoch/Mittel 4×, Niedrig 1×), Tiefe ebenfalls
   multisampled; Resolve in ein einfaches `Rgba16Float`; **Post-Pass** (Vollbild-Dreieck) ins Swapchain-Bild: heute
   nur `grade_fs` + Klemmen auf 0…1 (Phase 1: bildgleich), ab Phase 7 Tonemapping und HDR-Bloom. HUD und Minikarte
   danach wie bisher direkt aufs Swapchain-Bild – die Minikarte braucht dafür eine **eigene Pipeline-Variante**
   (Swapchain-Format, 1×) und eine eigene einfache Tiefe (heute teilt sie Pipeline und Tiefe mit der Szene).
3. **Pixel-Pfad:** Szene ohne MSAA in ein kleines Ziel (Bildpunktgröße wählbar, Standard ganzzahlig so, dass bei 1080p
   ein Pixel 3–4 Bildschirmpunkte ist), Tiefe als lesbare Textur; Kontur → Palette (LUT) → Bayer-Dithering → Nearest-
   Upscale (ganzzahlig, Rest Letterbox). Kamera im Uniform aufs interne Raster eingerastet. HD-Effekte, die nach dem
   Filter nicht sichtbar sind, entfallen (Bloom-Kette, MSAA, Normal Maps je nach Messung).
4. **Gemeinsame Konstanten** (Fensterraster, Atlasraster, Zellgrößen) aus Rust in WGSL über das Uniform bzw. einen
   generierten Konstantenblock, mit Test, dass beide Seiten übereinstimmen.
5. **Assets:** Muster der Audio-Pipeline – Python-Build-Skript erzeugt aus CC0/OFL-Quellen die eingebetteten Dateien plus
   Manifest (Quelle, Lizenz, Autor, Kachelgröße in m), `about.rs` liest die Credits daraus, ein Test vergleicht Manifest
   und eingebettete Liste. PNG-Dekodierung mit dem schon vorhandenen `png`-Crate, Mips auf der CPU (wie beim
   Fahrzeugatlas) → **keine neue Abhängigkeit** geplant.
6. **Fahrzeug-Glanzmaske ohne dritte Atlaslage:** G und B der Lack-Zelle sind frei → G = Glanz/Spiegelung, B =
   Materialklasse (Lack/Glas/Chrom). Spart ein Drittel Speicher gegenüber einer dritten Zelle (bei 512 × 256 je Zelle
   wären es sonst ~95 MB statt ~63 MB mit Mips).

## Risiken

| Risiko | Wirkung | Gegenmaßnahme |
|---|---|---|
| Float-Ziel klemmt nicht | SCREEN-Bloom und Mehrfach-Mischung weichen vom sRGB-Verhalten ab | Phase 1: kritische Shader-Ausgaben klemmen, Bildvergleich gegen Baseline mit Schwelle; HDR erst in Phase 7 bewusst öffnen |
| MSAA vs. Vollbild-Composites | Tiefentest läuft je Sample, der Fragment-Shader je Pixel – Kanten Dach/Boden könnten Mischpixel falsch belichten | Bildvergleich auf Kantenmaske; notfalls `@interpolate(…, sample)` bzw. Sample-Shading nur im Composite |
| Pixel-Modus braucht lesbare Tiefe, MSAA-Tiefe lässt sich in wgpu nicht auflösen | Konturen nur ohne MSAA | Pixel-Pfad rendert ohnehin 1× – Tiefe direkt als Textur |
| Minikarte im HUD-Pass | bricht, sobald die Szenen-Pipeline MSAA/Float bekommt | eigene Pipeline-Variante, Test „beide Modi zeichnen Minikarte“ (Aufnahme der Szene mit sichtbarer Karte) |
| Speicher | Fahrzeugatlas 512 × 256: ~63 MB mit Mips; Materialarrays 5 × 3 × 512² ≈ 21 MB | Glanz in freie Kanäle (s. o.), Qualitätsstufe Niedrig mit halber Auflösung |
| Budget | Regen/Schnee schon heute 6 ms Median, P95 8,8 ms | Niederschlag in Phase 7 hinter Qualitätsstufen; Messung je Phase |
| Asset-Download | ambientCG/Poly Haven brauchen Netz; Quellen ändern sich | Build-Skript lädt nur bei Bedarf, prüft SHA-256 je Quelle, eingebettete Dateien sind eingecheckt |
| Xbox/DX12 | nie gebaut | nur Kernfunktionen (`Rgba16Float`-Rendertarget, 4× MSAA, `texture_2d_array`) – alle in WebGPU garantiert; Zeitstempel nur optional |
| GPU taktet bei Teillast herunter | Messwerte schwanken | gleiche Bedingungen, Median + P95, Wiederholung bei Zweifel |

## Aufwand (Schätzung, Arbeitstage)

| Phase | Inhalt | Tage |
|---|---|---|
| 1 | Umschalter, Einstellungen, Offscreen-Ziel + MSAA + Post-Pass, Qualitätsstufen, Bildvergleich | 1,5 |
| 2 | Materialsystem Boden (Build-Skript, Texturarrays, Anti-Kachelung, Normal/Roughness, JSON-Zuordnung) | 2 |
| 3 | Bordstein, Übergänge, Kontaktschatten, Markierungen, Decal-Atlas 256 px mit Mips | 1,5 |
| 4 | Dächer und Fassaden (gemeinsames Fensterraster) | 1,5 |
| 5 | Fahrzeuge 512 × 256, Glanzmaske, Reflexe, Motorräder | 1,5 |
| 6 | Bäume (4 Arten, 256 px), Vorschlag Figuren-Atlas | 1 |
| 7 | Tonemapping, HDR-Bloom, weiche Schatten, nasse Straßen | 1,5 |
| 8 | Pixel-Modus (Raster, Einrasten, Kontur, Palette, Dithering, Upscale) | 2 |
| 9 | SDF-Schrift (OFL) im HD-Modus, Schrift-Abstraktion | 2 |
| | **Summe** | **≈ 14,5** |

## Offene Vorschläge (Entscheidung bei der jeweiligen Phase)

- Taste für den Umschalter: **F8** (F6 ist belegt).
- Schrift Phase 9: **Inter**, **IBM Plex Sans** oder **Atkinson Hyperlegible** (alle OFL).
- Tonemapping Phase 7: **AgX** (verträgt gesättigte Neonfarben besser als ACES).
- Straßenschilder zurück in die Welt (SDF-Schrift macht es möglich) – nur notiert.

## Definition of Done

Siehe Auftrag: HD Standard, Pixel per Menü/Taste/Konsole/CLI umschaltbar und gespeichert; alle Szenen in beiden Modi
ohne Nähte, Kachelmuster, Flimmern, Treppenkanten (HD); Pixel-Modus mit klaren Konturen, fester Palette, ruhigem
Scrollen; Minikarte, Stadtplan, Silhouetten, Fenster, Wetter in beiden Modi; Budget HD Hoch < 8 ms / Mittel < 5 ms
(Boulevard, 2560 × 1440, M1 Pro), Pixel nicht langsamer als die Baseline; Tests (Settings-Roundtrip, Materialzuordnung,
Fensterraster, Manifeste, Palette) grün; Doku (Spec, Plan, NATIVE-RUST.md, Vorher/Nachher-Tabelle, CHANGELOG).
