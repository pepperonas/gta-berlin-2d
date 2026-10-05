# Motorsound aus Aufnahmen

Sportwagen, Supercars und Hypercars mit Verbrennungsmotor und mindestens sechs Zylindern klingen nach einer echten
Aufnahme (V10 eines durchstartenden Supersportwagens) statt nach dem Synthesizer. Alle teilen eine Sample-Bank; drei
Kategorie-Profile und Überschreibungen je Fahrzeug färben und verstimmen sie. Alle anderen Fahrzeuge (Kleinwagen,
Vierzylinder-Sportwagen, Elektroautos, Lkw, Busse, Zweiräder) behalten ihren Synthese-Klang.

| Teil | Ort |
|---|---|
| Referenz (unverändert) | `data/audio/raw/engine_reference_lambo.m4a` |
| Analyse (Phase 1) | `tools/audio/analyze_reference.py` → `tools/audio/analysis/` (Plots, `SEGMENTE.md`) |
| Build-Skript (Phase 2) | `tools/audio/build_engine_sounds.py` → `data/audio/engine/v10/` |
| Profile (Phase 4) | `data/audio/engine_profiles.json` |
| Steuerlogik (rein, getestet) | `crates/sim/src/enginesound.rs` |
| Wiedergabe | `crates/audio/src/sampler.rs`, eingebunden in `synth.rs` |
| Spielanbindung | `crates/game/src/sound.rs` (`Listener`) |
| Debug-Panel (Phase 6) | `crates/game/src/enginedebug.rs` |

## Pipeline

```
m4a ──analyze_reference.py──▶ Plots, Segmenttabelle (Handarbeit: Fenster wählen)
    ──build_engine_sounds.py─▶ on_*.wav / off_*.wav / Einzelklänge / reference.wav / manifest.json
                                      │ include_bytes! (reference.wav nur bei Bedarf von der Platte)
                                      ▼
Fahrzeug ─▶ enginesound::profile_for ─▶ EngineSound::step (Drehzahl, Gas, Gang, Schlupf)
                                      ─▶ SoundOut (Loops mit Gewicht/Tonhöhe, Einzelklänge, Begrenzer, Färbung)
          ─▶ sound::Listener (Raum, Stimmenauswahl) ─▶ Frame.engines ─▶ Synth (Kanal „engine“) ─▶ cpal
```

Werkzeuge einrichten (nur für das Neuerzeugen der Samples; das Spiel braucht nichts davon):

```bash
python3 -m venv .venv-audio && .venv-audio/bin/pip install -r tools/audio/requirements.txt   # dazu ffmpeg
.venv-audio/bin/python tools/audio/analyze_reference.py     # optional: Plots neu
.venv-audio/bin/python tools/audio/build_engine_sounds.py   # Samples + Manifest neu
cargo test --workspace                                       # prüft Bank, Manifest, Klicks, Profile
```

Das Build-Skript ist **idempotent**: Gleicher Eingang ergibt bitgleiche Dateien (fester Ablauf ohne Zufall,
16-Bit-PCM-WAV). Es bricht ab, wenn ein Loop die Qualitätsgrenzen verletzt.

### Wie die Loops entstehen

Die Referenz ist 8,7 s lang und fast nur Hochdrehen unter Last; das Auto fährt dabei vom Mikrofon weg (Details und
Lücken: `tools/audio/analysis/SEGMENTE.md`). Ein Ausschnitt aus einem Hochdrehen würde als Loop „jaulen“, deshalb:

1. **Grundton verfolgen** über einen Obertonkamm (robuster als pyin, das hier oft eine Oktave springt).
   Drehzahl-Annahme: V10, Grundton = halbe Zündfrequenz, also Drehzahl = f0 × 24. Für die Beschriftung wird der
   Doppler des wegfahrenden Autos herausgerechnet (geschätzte Geschwindigkeit).
2. **Sweep glätten:** Das Quellfenster wird zeitvariabel nachgetastet (ds/dn = f_ziel / f(s)), bis die Tonhöhe
   stillsteht. Der Leerlauf wird nicht geglättet; dort entfernt ein Kerbfilter einen fremden Dauerton (237/454 Hz).
3. **Pegel ausgleichen** innerhalb des Fensters (höchstens ±4 dB).
4. **Loop-Punkt:** Länge = ganze Zahl von Grundtonperioden; der Kopf wird mit gleichstarkem Crossfade (25 ms) mit
   dem Stück hinter dem Ende überblendet. Der Übergang Ende → Anfang ist damit lückenlos.
5. **Lautheit:** alle Loops auf −18 LUFS (gemessen am wiederholten Loop).
6. **Schub-Loops** (Gas weg) sind abgeleitet, weil die Aufnahme keinen Schub enthält: Tiefpass 1,5 kHz, Hochpass
   60 Hz, leichte Sättigung, im Manifest −7 dB. Gefiltert wird über den dreifach wiederholten Loop, damit die Naht
   nahtlos bleibt.

Qualitätsgrenzen im Skript: Restdrift der Tonhöhe ≤ 35 Cent, Sprung am Loop-Punkt nicht größer als der größte
Schritt in seiner Umgebung (±64 Samples).

| Loop | Drehzahl | Länge | Quelle (s) | Restdrift |
|---|---|---|---|---|
| idle | 1362 | 0,56 s | 0,35–0,95 | – (nicht geglättet) |
| r2600 | 2672 | 0,29 s | 1,30–1,62 | 28 ct |
| r3300 | 3313 | 0,49 s | 1,80–2,32 | 10 ct |
| r4900 | 4930 | 0,21 s | 2,64–2,88 | 2 ct |
| r6000 | 5974 | 0,21 s | 2,98–3,22 | 7 ct |
| r7200 | 7175 | 0,31 s | 3,30–3,64 | 22 ct |

Einzelklänge: `start` (0,11–0,95 s; ob das ein Motorstart ist, ist unbelegt – die Aufnahme beginnt mit einem harten
Einsatz), `blip` (Gasstoß aus dem Anfahren), `shift_1…3` (die drei Hochschaltvorgänge), `pop_1…5` (Knackser aus dem
Ende und der ersten Schaltsalve). Lautheit: Start/Blip −16 LUFS, Schalten −20 LUFS, Pops Spitze −3 dBFS.

### Manifest (`data/audio/engine/v10/manifest.json`)

Je Loop: `datei`, `rpm` (Aufnahmedrehzahl), `last` (`on`/`off`), `samples`, `loop_start`/`loop_ende` (ganzer
Loop), `grundton_hz`, `quelle_s`, `gain_db`, `drift_cent`. Je Einzelklang: `datei`, `art` (`start`, `blip`,
`schalten`, `pop`), `samples`, `quelle_s`, `gain_db`, `rpm`. Dazu `referenz` (A/B-Datei), Quelle mit SHA-256 und die
Drehzahl-Annahme. Das Manifest wird nur vom Skript geschrieben; ein Test prüft, dass jede Datei im Programm steckt.

## Laufzeit

**Drehzahl:** aus der Fahrphysik (`vphys::State.rpm`/`gear`), wenn das Fahrzeug sie rechnet (Spielerauto, KI im
Physik-LOD-Umkreis), sonst aus einem **virtuellen Getriebe** des Profils: Gangstufen als Tempo am Begrenzer,
Hochschalten bei `schalt_hoch` (Vollgas) bis `schalt_hoch_teillast` (wenig Gas) der Begrenzerdrehzahl, Runterschalten
unter `schalt_runter`; im Stand dreht der Motor mit dem Gas frei hoch, Schlupf lässt ihn über das Rad schießen.
Beides ist reine Klanglogik; die Fahrphysik wird nicht verändert. Der Begrenzer kommt aus den Fahrzeugdaten
(`n_max`), sonst aus dem Profil.

**Überblendung:** Klang-Drehzahl = Drehzahl × `pitch` des Profils. Gleiche Leistung (cos/sin) zwischen den zwei
Loops, die sie einrahmen – aber nur im Bereich, in dem **beide** eine erlaubte Tonhöhe haben (0,7 … 1,4); unter dem
tiefsten und über dem höchsten Loop hält die Grenze. Tonhöhe = Klang-Drehzahl / Aufnahmedrehzahl.

**Last und Schub:** Gas mischt Last- und Schub-Loop mit gleicher Leistung; nahe dem Leerlauf zählt der Leerlauf-Loop
als Last. Beim Hochschalten ist das Gas `SHIFT_CUT` = 0,14 s weg (Schaltgeräusch, unter Volllast manchmal ein Pop).
Beim Runterschalten mit wenig Gas: Zwischengas-Stoß (`blip`).

**Glättung:** Drehzahl und Gas folgen ihrem Ziel mit `drehzahl_glaettung_s` bzw. `gas_glaettung_s` (exponentiell),
im Sampler zusätzlich Gewicht 30 ms, Tonhöhe 15 ms.

**Pops:** Gas weg (vorher > 50 %, jetzt < 15 %) über 55 % der Begrenzerdrehzahl → mit `pop_chance` 1–3 Pops in den
nächsten 0,1–0,5 s, höchstens alle 0,7 s. Zufall nur aus Hashes von Fahrzeug-id und Zähler.

**Begrenzer:** ab 98,5 % der Begrenzerdrehzahl mit Gas > 60 %: Rechteck mit `begrenzer_hz` (45 % der Periode auf
30 % abgesenkt), die Drehzahl zuckt um 3,5 % mit.

**Wiedergabe:** 4-Punkt-Hermite-Interpolation, Startversatz je Loop gegen Kammfilter, Sättigung (`saettigung`, mit
dem Gas stärker), Bass-Shelf 250 Hz, Höhen-Shelf 3 kHz, Tiefpass des Profils, Tiefpass der Entfernung.

### Raum und Leistung (Phase 5)

- Pegel = (1 − d / `hoerweite_px`)², Panorama nach dem seitlichen Versatz (±300 px = ganz links/rechts), Doppler
  aus der Annäherungsgeschwindigkeit (`doppler` = 0,6 der physikalischen Stärke), Tiefpass logarithmisch zwischen
  `tiefpass_nah_hz` und `tiefpass_fern_hz`.
- **Stimmenbudget:** höchstens `stimmen` (6) Sample-Motoren; das Spielerauto zählt zuerst, dann die lautesten.
  Fremde Sportwagen mit Profil fehlen in den Synthese-Stimmen (keine Doppelung); die übrigen bis zu 4 Synthese-
  Stimmen bleiben.
- **LOD:** ab `lod_px` (35 m) spielt ein fremdes Auto nur den nächstgelegenen Last-Loop, ohne Überblendung.
- Das Spielerauto hat immer volle Qualität; im geschlossenen Auto dämpft die Karosserie die Höhen (6,5 kHz).
- **Kanal „engine“:** Alle Sample-Motoren laufen über einen eigenen Pegel (`mix.engine`, dazu `SAMPLE_LEVEL` =
  0,38 im Synthesizer, eingemessen gegen den Synthese-Motor desselben Autos in der Testfahrt `--audio-wav … --audio-fahrzeug supercar_awd`: Vollgas +0,3 dB, Bremsen +1,6 dB, Teilgas +2,6 dB).
  Das eigene Auto geht direkt auf den Fahrzeugbus, fremde Autos auf den Außenbus (im Auto gedämpft).
- Rechenzeit: 16 gleichzeitige Stimmen rendern 1 s Klang in deutlich unter 0,25 s (Test `many_voices_render_fast`).

## Profile (`data/audio/engine_profiles.json`)

| Feld | Bedeutung |
|---|---|
| `bank` | Sample-Bank (Ordner unter `data/audio/engine/`) |
| `leerlauf`, `begrenzer` | Drehzahlen (Begrenzer nur, wenn die Fahrzeugdaten keinen liefern) |
| `gaenge` | virtuelles Getriebe: Tempo (km/h) je Gang am Begrenzer |
| `schalt_hoch`, `schalt_hoch_teillast`, `schalt_runter` | Schaltpunkte als Anteil der Begrenzerdrehzahl |
| `pitch` | Grundverstimmung (multipliziert die Tonhöhe aller Loops) |
| `tiefpass_hz`, `low_shelf_db`, `high_shelf_db`, `saettigung` | Klangfärbung |
| `pop_chance`, `pop_gain` | Wahrscheinlichkeit und Pegel der Pops |
| `lautstaerke` | Gesamtpegel |
| `drehzahl_glaettung_s`, `gas_glaettung_s` | Zeitkonstanten der Glättung |
| `begrenzer_hz` | Takt des Begrenzers |
| `start` | Anlassgeräusch beim Einsteigen in einen stehenden Wagen |

| Preset | Charakter | pitch | Begrenzer | Tiefpass | Shelf Bass/Höhen | Sättigung | Pops |
|---|---|---|---|---|---|---|---|
| `sport` | tiefer, weicher | 0,90 | 7600 | 5,2 kHz | +2,5 / −3 dB | 0,15 | 0,25 |
| `supercar` | nah an der Referenz | 1,00 | 8500 | 9 kHz | 0 / 0 dB | 0,25 | 0,50 |
| `hypercar` | höher, heller, rauer | 1,08 | 9200 | 12 kHz | −1 / +3 dB | 0,50 | 0,85 |

**Zuordnung:** `zuordnung.klassen` bildet die Fahrzeugklasse (`klasse` in `data/vehicles/vehicles.json`) auf ein
Preset ab; Elektromotoren sind immer ausgenommen, Vier- und Dreizylinder stehen in `zuordnung.ausgenommen`
(`roadster`, `leichtcoupe`, `leichtbau`, `rallye`, `drift_coupe`, `hypercar_elektro`). Unter `fahrzeuge` stehen
Überschreibungen je Fahrzeug-id (nur die genannten Felder ändern sich; `preset` wählt eine andere Grundlage).
Beispiel: `"hypercar": { "begrenzer": 7100, "pitch": 0.92 }` (der Bugatti-artige dreht nur bis 6800).

`GTA_ENGINE_SAMPLES=0` schaltet zum Gegenhören auf den Synthese-Klang zurück.

## Debug-Panel (Phase 6)

**F4** (Entwickler-Build: `--dev` oder `GTA_DEV=1`) bzw. Konsole `motorsound [an|aus]` in jedem Build. Rechts im
Bild; die Physik-Anzeige (F3) liegt links, es ist immer nur eine offen (sie teilen sich die Tasten).

- Bild auf/ab wählt eine Zeile, Komma/Punkt verstellt sie: **Quelle** (Fahrzeug oder Regler), **Profil** (alle
  Presets und Fahrzeuge mit Überschreibung), **Drehzahl** (±250), **Gas** (±10 %), **Gang**, **A/B**. Wer an
  Drehzahl, Gas, Gang oder Profil dreht, schaltet auf die Regler – das geht auch zu Fuß.
- Angezeigt: Profil, Drehzahl, Gas, Gang, Begrenzer, Färbung, alle hörbaren Loops mit Gewicht (orange = Last,
  blau = Schub) und Tonhöhe, Zahl der Motorstimmen im Bild.
- **F7** oder die Zeile A/B: A spielt die Referenzaufnahme in Schleife (auf den Pegel der Loops gebracht, die
  Motor-Samples schweigen solange), B wieder die Samples.

## Ein neues Fahrzeugsample hinzufügen

1. Aufnahme nach `data/audio/raw/<name>.<ext>` kopieren (Original nie verändern).
2. `analyze_reference.py <datei>` laufen lassen, Plots und `segments.md` ansehen, Drehzahl-Annahme festlegen
   (Zylinderzahl, Grundton = Zündfrequenz oder halbe) und eine Segmenttabelle schreiben.
3. `build_engine_sounds.py` kopieren oder parametrieren: `SRC`, `OUT` (`data/audio/engine/<bank>/`),
   `RPM_PER_HZ`, die Fenster in `LOOPS` (Abstand benachbarter Loops höchstens Faktor 2, sonst entsteht eine
   Tonhöhenlücke) und `ONESHOTS`. Laufen lassen, bis die Qualitätsgrenzen halten.
4. In `crates/sim/src/enginesound.rs` das Manifest per `include_str!` an `Config::parse` übergeben und in
   `crates/audio/src/sampler.rs` eine Dateiliste wie `V10` samt Ladefunktion anlegen (der Test
   `bank_matches_the_manifest` zeigt, was fehlt). `SamplerVoice` braucht heute eine Bank je Synthesizer; für eine
   zweite Bank bekommt `EngineFrame` den Banknamen und der Synthesizer eine Bank je Stimme.
5. In `engine_profiles.json` Presets mit `"bank": "<bank>"` anlegen und Klassen oder einzelne Fahrzeuge zuordnen.
6. `cargo test --workspace`; im Spiel mit F4/`motorsound` und A/B gegen die Referenz abhören.

## Grenzen

- Die Referenz ist kurz: Loops sind 0,2–0,6 s lang; über 7200 1/min wird der oberste Loop hochgezogen (bis × 1,4).
- Schub ist abgeleitet, nicht aufgenommen; ob der Einsatz am Anfang ein Motorstart ist, ist unbelegt.
- Die Drehzahl-Beschriftung beruht auf einer Annahme (V10, Ordnung 2,5) und einer geschätzten
  Geschwindigkeit für den Doppler.
- Abgehört wurde nicht von Menschen in dieser Sitzung; verifiziert sind Spektrogramme der Testfahrt, Pegelmessung
  und die Tests (Klicks, Aussetzer, Tonhöhengrenzen, Unterscheidbarkeit der Kategorien, Rechenzeit).
