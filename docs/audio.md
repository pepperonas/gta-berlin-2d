# Motorsound aus Aufnahmen

Sportwagen, Supercars und Hypercars mit Verbrennungsmotor und mindestens sechs Zylindern klingen nach echten
Aufnahmen statt nach dem Synthesizer. Zwei Sample-Bänke: **v10** (durchstartender V10-Supersportwagen, 8,7 s) für
Sportwagen und Supercars, **v12** (Twin-Turbo-V12 mit 1000+ PS am Prüfstand, 169 s) für Hypercars. Drei
Kategorie-Profile und Überschreibungen je Fahrzeug färben und verstimmen sie. Alle anderen Fahrzeuge (Kleinwagen,
Vierzylinder-Sportwagen, Elektroautos, Lkw, Busse, Zweiräder) behalten ihren Synthese-Klang.

| Teil | Ort |
|---|---|
| Referenzen (unverändert) | `data/audio/raw/engine_reference_lambo.m4a` (v10), `engine_reference_svj.m4a` (v12) |
| Analyse (Phase 1) | `tools/audio/analyze_reference.py [QUELLE [ORDNER]]` → `tools/audio/analysis/` (v10), `analysis_svj/` (v12), je mit `SEGMENTE.md` |
| Build-Skript (Phase 2) | `tools/audio/build_engine_sounds.py [BANK …]` → `data/audio/engine/v10/`, `…/v12/` |
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
.venv-audio/bin/python tools/audio/build_engine_sounds.py   # alle Bänke neu (oder: … v12)
cargo test --workspace                                       # prüft Bank, Manifest, Klicks, Profile
```

Das Build-Skript ist **idempotent**: Gleicher Eingang ergibt bitgleiche Dateien (fester Ablauf ohne Zufall,
16-Bit-PCM-WAV). Es bricht ab, wenn ein Loop die Qualitätsgrenzen verletzt.

### Wie die Loops entstehen

Beide Referenzen bestehen vor allem aus Drehzahl-Sweeps; ein Ausschnitt daraus würde als Loop „jaulen“. Die
v10-Aufnahme ist kurz und fährt vom Mikrofon weg (`tools/audio/analysis/SEGMENTE.md`), die v12-Aufnahme ist ein
Prüfstandslauf mit vielen Zügen, echtem Ausrollen und Fehlzündungen (`tools/audio/analysis_svj/SEGMENTE.md`).
Je Bank steht die Konfiguration oben im Build-Skript (`V10`, `V12`: Quelle, Drehzahl-Annahme, Fenster).

1. **Grundton verfolgen** – v10 über einen Obertonkamm (robuster als pyin, das hier oft eine Oktave springt;
   Annahme V10, Grundton = halbe Zündfrequenz, Drehzahl = f0 × 24, Doppler des wegfahrenden Autos herausgerechnet),
   v12 über eine **Linienverfolgung** von einer Startfrequenz aus (stärkstes Maximum innerhalb ±7 % der vorigen
   Frequenz; der Kamm rutscht beim V12 auf die halbe Frequenz; Annahme Zündfrequenz, Drehzahl = f0 × 10).
2. **Sweep glätten:** Das Quellfenster wird zeitvariabel nachgetastet (ds/dn = f_ziel / f(s)), bis die Tonhöhe
   stillsteht. Der Leerlauf wird nicht geglättet; dort entfernt ein Kerbfilter einen fremden Dauerton (237/454 Hz).
3. **Pegel ausgleichen** innerhalb des Fensters (höchstens ±4 dB).
4. **Loop-Punkt:** Länge = ganze Zahl von Grundtonperioden; der Kopf wird mit gleichstarkem Crossfade (25 ms) mit
   dem Stück hinter dem Ende überblendet. Der Übergang Ende → Anfang ist damit lückenlos.
5. **Lautheit:** alle Loops auf −18 LUFS (gemessen am wiederholten Loop).
6. **Schub-Loops** (Gas weg): v12 hat echte aus dem Ausrollen am Prüfstand (geglättet wie die Züge, im Manifest
   −6 dB). Wo eine Aufnahme keinen Schub hergibt (v10 ganz, v12 unter 4000 1/min), werden sie aus Last-Loops
   abgeleitet: Tiefpass 1,5 kHz, Hochpass 60 Hz, leichte Sättigung, −7 dB. Gefiltert wird über den dreifach
   wiederholten Loop, damit die Naht nahtlos bleibt. Last- und Schub-Loops dürfen auf verschiedenen Drehzahlen
   liegen; jede Liste wird für sich überblendet.

Qualitätsgrenzen im Skript: Restdrift der Tonhöhe ≤ 35 Cent, Sprung am Loop-Punkt nicht größer als der größte
Schritt in seiner Umgebung (±64 Samples).

**v10:**

| Loop | Drehzahl | Länge | Quelle (s) | Restdrift |
|---|---|---|---|---|
| idle | 1362 | 0,56 s | 0,35–0,95 | – (nicht geglättet) |
| r2600 | 2672 | 0,29 s | 1,30–1,62 | 28 ct |
| r3300 | 3313 | 0,49 s | 1,80–2,32 | 10 ct |
| r4900 | 4930 | 0,21 s | 2,64–2,88 | 2 ct |
| r6000 | 5974 | 0,21 s | 2,98–3,22 | 7 ct |
| r7200 | 7175 | 0,31 s | 3,30–3,64 | 22 ct |

**v12:** Last 1235 (Leerlauf, 1,9 s), 2738, 3297, 4122, 5747, 6837, 8383 aus dem Zug 21,2–27,4 s (0,27–0,38 s,
Restdrift 5–32 ct); Schub echt 4047, 4613, 5742, 7725 aus dem Ausrollen 83,4–89,3 s (0,33–0,48 s, 9–29 ct),
abgeleitet 1235 und 2738. Einzelklänge: `blip_1`, `blip_2` (Gasstöße im Stand), `pop_1…7` (Fehlzündungen 99–139 s).
Kein Start, kein Schalten (Prüfstand im festen Gang). Die A/B-Referenz ist ein 30-s-Auszug (Zug, Ausrollen,
Fehlzündungen; `referenz_auszug_s` im Manifest), die ganze Aufnahme wäre als WAV 16 MB.

v10-Einzelklänge: `start` (0,11–0,95 s; ob das ein Motorstart ist, ist unbelegt – die Aufnahme beginnt mit einem harten
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
tiefsten und über dem höchsten Loop hält die Grenze. Tonhöhe = Klang-Drehzahl / Aufnahmedrehzahl. Liegen zwei
Loops mehr als Faktor 2 auseinander (v12: Leerlauf 1235 → 2738), erweitert `pitch_bounds` die Grenzen der beiden bis
zur Mitte der Lücke (dort bis × 1,53 bzw. × 0,65), damit die Tonhöhe in der Überblendung nicht springt.

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
| `sport` | tiefer, weicher (Bank v10) | 0,90 | 7600 | 5,2 kHz | +2,5 / −3 dB | 0,15 | 0,25 |
| `supercar` | nah an der V10-Referenz | 1,00 | 8500 | 9 kHz | 0 / 0 dB | 0,25 | 0,50 |
| `hypercar` | Twin-Turbo-V12 (Bank v12) | 1,00 | 9000 | 12 kHz | 0 / +1,5 dB | 0,35 | 0,85 |

**Zuordnung:** `zuordnung.klassen` bildet die Fahrzeugklasse (`klasse` in `data/vehicles/vehicles.json`) auf ein
Preset ab; Elektromotoren sind immer ausgenommen, Vier- und Dreizylinder stehen in `zuordnung.ausgenommen`
(`roadster`, `leichtcoupe`, `leichtbau`, `rallye`, `drift_coupe`, `hypercar_elektro`). Unter `fahrzeuge` stehen
Überschreibungen je Fahrzeug-id (nur die genannten Felder ändern sich; `preset` wählt eine andere Grundlage).
Beispiel: `"hypercar": { "begrenzer": 7100, "pitch": 0.94 }` (der Bugatti-artige dreht nur bis 6800). Welche Bank
ein Preset spielt, steht in seinem Feld `bank` – Sportwagen auf den V12 umzustellen ist eine Datenänderung.

Pegel gegen den Synthese-Motor desselben Autos (Testfahrt): v10 `supercar_awd` Vollgas +0,3 / Bremsen +1,6 /
Teilgas +2,6 dB, v12 `hypercar` −0,5 / +2,5 / +3,1 dB. Leerlauf- und Schub-Loops sind auf dieselbe Lautheit gebracht
wie die Volllast-Loops; wer es im Teillastbereich leiser will, stellt `lautstaerke` nach.

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
  Motor-Samples schweigen solange), B wieder die Samples. Gespielt wird die Referenz der Bank, die gerade klingt:
  die des gewählten Profils (Regler) bzw. die des gefahrenen Autos.

## Ein neues Fahrzeugsample hinzufügen

1. Aufnahme nach `data/audio/raw/<name>.<ext>` kopieren (Original nie verändern).
2. `analyze_reference.py <datei>` laufen lassen, Plots und `segments.md` ansehen, Drehzahl-Annahme festlegen
   (Zylinderzahl, Grundton = Zündfrequenz oder halbe) und eine Segmenttabelle schreiben.
3. In `build_engine_sounds.py` eine Konfiguration wie `V12` anlegen und in `BANKS` eintragen: Quelle,
   `rpm_per_hz`, Verfolgung (`kamm` oder `linie` mit Startfrequenz je Fenster), Last-Fenster (Abstand benachbarter
   Loops möglichst höchstens Faktor 2), Schub-Fenster aus echtem Ausrollen, abgeleitete Schub-Loops, Einzelklänge,
   Referenz-Auszug. `build_engine_sounds.py <bank>` laufen lassen, bis die Qualitätsgrenzen halten.
4. In `crates/sim/src/enginesound.rs` das Manifest per `include_str!` an `Config::parse` übergeben und in
   `crates/audio/src/sampler.rs` eine Dateiliste per `bank_files!` anlegen und in `files_of` eintragen (der Test
   `banks_match_their_manifests` zeigt, was fehlt). Jede Stimme spielt die Bank, die `SoundOut.bank` nennt.
5. In `engine_profiles.json` Presets mit `"bank": "<bank>"` anlegen und Klassen oder einzelne Fahrzeuge zuordnen.
6. `cargo test --workspace`; im Spiel mit F4/`motorsound` und A/B gegen die Referenz abhören.

## Grenzen

- v10 ist kurz: Loops sind 0,2–0,6 s lang; über 7200 1/min wird der oberste Loop hochgezogen (bis × 1,4). Schub
  ist dort abgeleitet; ob der Einsatz am Anfang ein Motorstart ist, ist unbelegt.
- v12 hat zwischen Leerlauf und 2700 1/min kein Material (Tonhöhe dort bis × 1,53) und unter 4000 keinen echten
  Schub; Prüfstandsrauschen liegt unter allem. Start- und Schaltgeräusch fehlen (Hypercars schalten stumm).
- Die Drehzahl-Beschriftung beruht auf einer Annahme (V10, Ordnung 2,5) und einer geschätzten
  Geschwindigkeit für den Doppler.
- Abgehört wurde nicht von Menschen in dieser Sitzung; verifiziert sind Spektrogramme der Testfahrt, Pegelmessung
  und die Tests (Klicks, Aussetzer, Tonhöhengrenzen, Unterscheidbarkeit der Kategorien, Rechenzeit).

## Schüsse (05.10.2026)

Pistole, Maschinenpistole und Schrotflinte spielen echte Aufnahmen statt des Synthesizers.

| Teil | Ort |
|---|---|
| Quelle | [The Free Firearm Sound Library](https://opengameart.org/content/the-free-firearm-sound-library), „Prepared SFX Library.7z“ (194 MB), **CC0 1.0** – Ben Jaszczak, Brian Nelson, Kevin Heras, Matthew Nanney |
| Build-Skript | `tools/audio/build_weapon_sounds.py` (lädt das Archiv nach `tools/audio/.cache/`, prüft SHA-256, entpackt mit `7z`) |
| Ausgabe | `data/audio/weapons/{pistol_1..3,smg_1..3,shotgun_1..2}.wav` + `manifest.json` (je 1,5 s, Mono, 48 kHz, 16 Bit) |
| Wiedergabe | `crates/audio/src/sampler.rs weapon_bank`, `synth.rs` (`Sfx::Gun`, `SamplePlay`) |

Vorbilder: Walther PPQ (Pistole), Carl Gustav M45 (MP), Benelli Nova (Schrotflinte). Je Variante zwei Schichten
derselben Waffe, am Knall ausgerichtet: die trockene Nahaufnahme und – 15 ms später, 3–6 dB leiser – die
Mittelaufnahme mit Körper und Echo vom Schießstand (der Widerhall, den die Nahaufnahme nicht hat). Bei der MP sind
die Aufnahmen Zweierstöße; genommen wird nur der erste Schuss, den Takt schießt das Spiel selbst. Das Skript ist
idempotent (gleicher Eingang, bitgleiche Dateien).

**Laufzeit:** je Schuss eine zufällige Variante mit ±3 % Tonhöhe. Ein neuer Schuss derselben Waffe blendet den
Nachhall des vorigen in 30 ms aus (sonst türmen sich bei Dauerfeuer bis zu 13 Nachhallfahnen), höchstens 24
gleichzeitig. Entfernung dunkelt ab: Tiefpass 1,2 kHz (fern) bis ~17 kHz (nah), zusätzlich zur Lautstärke. Pegel je
Waffe in `GUN_LEVEL`; ein Test hält die Lautheit mindestens auf dem Stand des Synthesizers und unter Übersteuerung.

## Geräusche aus Aufnahmen (05.10.2026)

Alle übrigen Klänge werden schrittweise von Synthese auf frei lizenzierte Aufnahmen umgestellt (CC0 und CC BY;
CC-BY-Urheber erscheinen im Reiter „Lizenzen“ der Über-Seite). Die Synthese bleibt als Rückfall:
`GTA_SFX_SAMPLES=0` schaltet zum Gegenhören zurück, ebenso fehlt eine Aufnahme nie hörbar.

| Teil | Ort |
|---|---|
| Rezepte (Quellen mit URL, SHA-256, Lizenz, Urheber; Varianten je Klang) | `tools/audio/sfx_recipes.json` |
| Build-Skript | `tools/audio/build_sfx.py [KLANG …]` (Archive nach `tools/audio/.cache/sfx/`, idempotent) |
| Ausgabe | `data/audio/sfx/<klang>_<n>.wav` + `manifest.json` (Mono, 48 kHz, 16 Bit) |
| Dateiliste | `crates/audio/build.rs` erzeugt sie aus dem Ordner (`SFX_FILES`), keine Handliste |
| Wiedergabe | `sampler.rs sfx_bank(name)`, `synth.rs Synth::sample` mit `SfxSpec` (Pegel, Tonhöhenstreuung, Entfernungs-Tiefpass) |
| Lizenzseite | `crates/game/src/about.rs sfx_credits` liest das Manifest |

**Bearbeitung je Variante:** Schichten laden (Mono, 48 kHz), am Einsatz ausrichten (3 ms vor dem ersten Wert über
10 % der Spitze), versetzt mischen, Hochpass, kürzen, ausblenden, Spitze −1 dBFS. **Laufzeit:** zufällige Variante,
Tonhöhe ± `spread`, ferne Klänge (`distance`) zusätzlich dunkler (Tiefpass 1,2–17 kHz wie bei den Schüssen).

**Pegel:** Die Synthese war sehr ungleich laut (Schritt auf Gras fast unhörbar), Gleichheit mit ihr wäre falsch.
Der Test `sfx_samples_match_the_synth_loudness` misst das lauteste 50-ms-Fenster gegen Zielwerte: Schritte so laut
wie der frühere harte Schritt (Gras etwas leiser), alles andere 20 % über der Synthese; nie leiser als die
Synthese, keine Übersteuerung.

**Freesound** (`tools/audio/freesound.py`): Zugangsdaten nur in `~/.config/gta-berlin/freesound.json`
(`client_id`, `client_secret`; nach `freesound.py login` + `freesound.py code CODE` auch die OAuth-Tokens, nötig
für Originaldateien). `freesound.py search "…" [--cc0] [--max-dur S]` listet Kandidaten. Im Rezept ist eine Quelle
dann `{"freesound": ID, "sha256": …, "lizenz": …, "urheber": …, "titel": …, "seite": …}`; Lizenz und Urheber
prüft der Download gegen die API, die Prüfsumme hält den Stand fest. Schichten können eine eigene `quelle` und mit
`von_s`/`bis_s` einen Ausschnitt einer langen Aufnahme tragen.

**Neuer Klang:** Quelle und Rezept in `sfx_recipes.json` eintragen, `build_sfx.py <klang>` laufen lassen, in
`synth.rs` ein `SfxSpec` anlegen und im `play`-Arm `Sfx::X if self.sample(SPEC, k, M) => {}` vor den
Synthese-Arm setzen, Zielwert in den Pegel-Test aufnehmen.

### Phase 1: Schritte und Nahkampf

| Klang | Quelle | Lizenz |
|---|---|---|
| Schritt hart / Gras / Schnee | Kenney „Impact Sounds“ (footstep_concrete/grass/snow) | CC0 |
| Schritt nass | „Footsteps on different surfaces“ (water) von congusbongus, nach EminYILDIRIM und swuing (Freesound) | CC BY 3.0 |
| Ausholen | „Swishes Sound Pack“ von artisticdude | CC0 |
| Faustschlag / Treffer, Blech, Einschlag, Aufprall | Kenney „Impact Sounds“ (impactPunch_medium, impactMetal_heavy, impactMining, impactSoft_heavy) | CC0 |
| Nachladen, eingerastet, Waffenwechsel | Kenney „RPG Audio“ (metalClick, metalLatch, beltHandle, clothBelt, handleSmallLeather) | CC0 |

### Phase 6: Menü und Aufträge (vorgezogen)

| Klang | Quelle | Lizenz |
|---|---|---|
| Menü-Klick, Countdown-Tick | Kenney „Interface Sounds“ (click_002/003, select_001, tick_002) | CC0 |
| Missionsstart, -erfolg, -fehlschlag, Einsammeln | Kenney „Music Jingles“, Saxofon-Reihe | CC0 |

Die Jingles sind nach Tonhöhenverlauf und Tongeschlecht ausgewählt (pyin + Chroma-Abgleich mit Dur/Moll-Profilen):
Erfolg = aufsteigend in Dur (SAX15/10/16), Fehlschlag = absteigend in Moll (SAX07/05/03), Start = kurz
aufsteigend in Moll (SAX04/06), Einsammeln = kurz aufsteigend in Dur (SAX08). Keine Tonhöhenstreuung bei Jingles.

### Phase 2: Fahrzeuge (Freesound, alle CC0)

| Klang | Freesound-Quelle |
|---|---|
| Unfall schwer (Stärke ≥ 0,45) / leicht | craigsmith „S38-24 Big heavy car crash“, „S37-14 Two cars crash foley“ / qubodup „Clank Car Crash Collision“, Logicogonist „car crash long 1“ |
| Hupe | yfjesse „Car Horn“, maciejadach „horn.wav“, DuranBurrus „Car Horn.wav“ |
| Autotür | Frederik_Sunne „Car door close“, Crimsonblaze „Car Door Shutting“, djfigs1 „Car Door Open & Close“ (nur das Zuschlagen) |
| Poller umgefahren | yfjesse „Rest Stop Metal Pole“, Anthousai „hit - metallic - basketball hoop pipe 01“ |
| Spritzwasser | ahill86 „PuddleSplash“ (drei Spritzer), AardsReal „Water Splash“, gis_sweden „Small Splash“ |
| Fahrer herausziehen | avainquin „Skin contact grab“ + Autotür |
| Martinshorn (Schleife) | TitanKaempfer „Martinshorn (Siren) 2“ |

**Martinshorn:** Die Aufnahme wurde per pyin vermessen: exakt 464 und 619 Hz (Quarte, wie das deutsche
Martinshorn), Wechsel alle 0,8 s, keine Tonhöhendrift (stehend aufgenommen, kein Doppler). Die Schleife umfasst vier
Perioden; ihre Länge (6,400 s) ist per Kreuzkorrelation gefunden (Korrelation ≈ 1,0) – mit der geschätzten Periode
sackte der Pegel an der Naht um ein Viertel ab. Wiedergabe als `LoopLayer` im Umgebungskanal (gedämpft im Auto),
Pegel `SIREN_LEVEL` × `Mix::siren`; der Wechsel hoch/tief steckt in der Aufnahme, `Mix::siren_high` wird dann nicht
gebraucht. Test `siren_plays_the_recorded_horn`.

**Build:** `schleife_blende_s` macht aus einem Ausschnitt eine nahtlose Schleife (der Überhang wird mit
gleichleistungs-Blende in den Anfang gemischt), `ausrichten: false` lässt Ausschnitte an ihrer absoluten Zeit.
Freesound drosselt Downloads (HTTP 429); `freesound.py` wartet dann und versucht es erneut.

### Phase 3: Reifen, Fahrtwind, Regen aufs Dach (Schleifen)

| Ebene (`Tires`) | Freesound-Quelle | Lizenz |
|---|---|---|
| Abrollen auf Asphalt (`roll`) | Soundholder „audi a4 b8 20tdi tyres asphalt medium speed mono“ | CC BY 3.0 |
| Kopfstein (`cobble`) | Gustavus „Car on a street with uneven cobblestones-1“ | CC BY 4.0 |
| Schotter, Gras, Erde (`offroad`, **neu hörbar** – die Synthese hatte keine Ebene dafür) | TRP „car tire on gravel rear cu“ | CC0 |
| Nässe (`wet`), Rutschen bei wenig Grip (`slide`, gleiche Aufnahme versetzt) | Zabuhailo „RainAndTireNoise“ | CC BY 4.0 |
| Schnee und Eis (`snow`) | dunebuggy „tires_snow_ice_slow“ | CC0 |
| Quietschen (`skid`) | johnnydekk „screeching tyres / tires“, tonale Strecke um 1,2 kHz | CC0 |
| Fahrtwind (`wind`) | klankbeeld „storm wind room-tone“ | CC BY 4.0 |
| Regen aufs Dach (im Auto) | Nox_Sound „Ambiance_Rain_Inside_Car_Roof_Loop_Stereo“ | CC0 |

**Auswahl:** je Aufnahme das stetigste 6-s-Stück (kleinste Pegelschwankung in 0,25-s-Fenstern, 0,3–2,5 dB);
beim Quietschen die längste tonale Strecke (pyin, Flachheit ≈ 0). Schleifen mit 0,3 s (Quietschen 0,15 s)
Überblendung. **Laufzeit:** `TireLoops` aus `LoopLayer`s mit geglättetem Pegel und Tempo; Tempo 0,75–1,25 mit der
Geschwindigkeit, Quietschen im Drift bis 30 % tiefer (wie die Synthese), stille Ebenen rechnen nicht. Pegel
`LOOP_*` gegen die Rausch-Ebenen der Synthese (Faktor 1,2), Schotter fest knapp über dem Abrollen
(Test `tire_loops_match_the_synth_layers`).

**Falle:** m4a lässt sich nicht über eine Pipe dekodieren (ffmpeg braucht Sprünge) – das ergab still eine leere
Kopfstein-Schleife. Das Build-Skript dekodiert jetzt über eine Zwischendatei und bricht bei stummen Ergebnissen ab.

### Phase 4: Umgebung und Wetter (alle CC0)

| Ebene (`Mix`) | Freesound-Quelle |
|---|---|
| Stadtbrummen (`hum`) | Yakobb1 „City Ambience Distant Stereo“ |
| Verkehr (`traffic`) | Froey_ „Background traffic noise“ |
| Wasser (`water`) | bruno.auzet „bubble lapping wave on concrete pier“ |
| Regen / Starkregen (`rain`, ab 0,6 zusätzlich) | szegvari „City Rain Dark“ / Gustavo_C „heavy_rain_outside“ |
| Wind / Pfeifen bei Böen (`wind`, `gust`) | craigsmith „G56-21-Winter Wind“ / TRP „Wind, window, cold gusts, moan, whistle“ |
| Vögel (`birds`) | logancircle2 „Birds Chirping Bush Urban“ |
| Kneipe (`bar`, mit Richtung) | itinerantmonk108 „Bar room background 2“ |
| Club (`music`, mit Richtung) | CHallSmith „Music, Bass Thumps Through Wall“ |
| Donner nah | Kinoton „Thunder Clap And Rumble #5“, Samot_Ekschotz „Some_Thunders“ (zwei der drei Donner) |
| Donner fern (Tiefpass 500 Hz) | greyfeather „distant rumbling thunderstorm“, Samot_Ekschotz (der mittlere Donner) |

Schleifen 20 s mit 0,5 s Überblendung, je das stetigste Stück der Aufnahme; bei Vögeln war das stetigste Stück
der ersten Wahl fast still (−65 dB) – dort zählt der Pegel, nicht die Stetigkeit. Mit Aufnahmen entfallen die
synthetischen Einzelereignisse (Zwitschern, Regentropfen, Lachen, Gläser, Club-Kick): die Schleifen enthalten sie.
Böen machen den Wind lauter und heller (Tempo 0,9–1,15) und lassen ab mittlerer Stärke das Pfeifen einsetzen.

**Pegel:** gegen die Synthese ×1,2, gemessen wie im Spiel (Mix bei jedem Bild angewendet – die Synthese plant
Zwitschern und Beats pro Bild, ein einmaliges `apply` ergab dort 0). Regen und Starkregen sowie Wind und Pfeifen
überlagern sich und sind gemeinsam aufgelöst (Test `ambience_loops_match_the_synth_layers`).

### Phase 5: Bahn und Glocken

| Klang | Freesound-Quelle | Lizenz |
|---|---|---|
| Fahrgeräusch im Wagen (`TrainLayers::roll`, Tempo aus `roll_f`) | peridactyloptrix „London Underground: full train journey through tunnel“ | CC0 |
| Grollen (`rumble`, Tiefpass 220 Hz; auch `Mix::rumble` draußen) | dieselbe Aufnahme, anderes Stück | CC0 |
| Tunnelwind (`wind`) | Fahrtwind-Schleife aus Phase 3 | CC BY 4.0 |
| Bremsquietschen (`squeal`) | Reifenquietschen aus Phase 3, 2,3-fach schneller (≈ 2,8 kHz) | CC0 |
| Schienenstoß | ahill86 „TravellingOnMetroTrain“ – fünf Stöße, je ~13 dB über dem Fahrgeräusch | CC0 |
| Druckluft | brunoboselli „Air (or steam) pressure release“ | CC0 |
| Abfertigung | uair01 „Berlin metro Brandenburger Tor Zurueckbleiben bitte“: Ansage (1,6–3,2 s), dann Türwarnton (3671 Hz) | CC BY 3.0 |
| Straßenbahnklingel | Profispiesser „Berlin Tram Train Ring Bell Alexanderplatz“ | CC0 |
| Kirchenglocke (je Schlag, Abstand 2,1 s) | theblockofsound235 „12 Noon Hour Bell Strike“ | CC BY 4.0 |

**Bewusst Synthese geblieben:** der **Fahrmotor** (Umrichter-Heulen, `motor_f` 95–1900 Hz – eine Aufnahme lässt sich
über Faktor 20 nicht stimmen, ohne zu brummen oder zu zwitschern; der reine elektrische Ton ist ohnehin ein
Sinus-Sweep) und die **Türgongs** (`GongOpen/GongClose` – elektronische Zweiklänge; es fand sich kein Berliner
Gong unter freier Lizenz).

**Analyse der Abfertigung (nicht abgehört):** pyin zeigt bei 1,75–3,0 s eine Männerstimme mit Sprachmelodie
(f0 164–243 Hz), bei 11,0–13,8 s einen Dauerton bei 3671 Hz (pyin meldet 333 Hz = 3671/11, eine Unterharmonische),
um 16 s das Schließen der Türen, ab 21,7 s das Anfahren. Ob die Stimme wirklich „Zurückbleiben bitte“ sagt, ist
aus Titel und Ablauf geschlossen – bitte gegenhören.

**Bahnsteuerung:** `TrainLoops` je Zug (eigener Zug, Zug am Bahnsteig, versetzt gestartet); mit Aufnahmen bekommt
`TrainVoice` nur noch die Motorschicht. Glockenschläge sind verzögerte Einzelklänge (`Synth::sample_at`).
Test `rail_loops_match_the_synth_layers` (Pegel ×1,2, Fahrmotor unverändert).
