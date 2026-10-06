# Fahrzeugdaten

Die Fahrphysik der Rust-Fassung liest alles hier, nichts davon steht im Code. Geladen und geprüft wird von
`crates/sim/src/vehdata.rs`. Die Dateien sind ins Spiel eingebettet. Wer sie ändert, baut neu.

| Datei | Inhalt |
|-------|--------|
| `vehicles.json` | alle Fahrzeuge (Schema: `vehicle.schema.json`) |
| `classes.json` | Klassen mit sämtlichen Vorgaben, jedes Fahrzeug erbt von seiner Klasse |
| `tires.json` | Reifen: Haftung, Schlupfkurve, Faktor je Untergrund, Rollwiderstand |
| `surfaces.json` | Untergründe der Karte und ihre Reifenkategorie |
| `engine-curves.json` | Drehmomentkurven je Motortyp, Elektro- und Muskelmodell |
| `feel.json` | Spielgefühl-Schicht (Grip, Bremse, Lastwechsel, Aquaplaning, Anfahr-Zuschlag `anfahr_zuschlag` bis `anfahr_bis_kmh` – nur aufs Motormoment, nie auf die Haftung), Simulation = 1 bzw. 0 |
| `vehicles.calibrated.json` | **erzeugt** vom Kalibrierwerkzeug, nicht von Hand bearbeiten |

## Neues Fahrzeug in 2 Minuten

1. Eintrag in `vehicles.json` anhängen. Fünf Felder genügen, alles andere kommt aus der Klasse:

   ```json
   {"id": "kompakt_fuenf_felder", "name": "Spree Ronda Basis", "klasse": "pkw_kompakt",
    "masse": 1300, "kw": 81, "ziel": {"0_100": 10.5, "vmax": 200}}
   ```

   `id` nur aus `a-z`, `0-9` und `_`. Der `name` ist eine Parodie, keine echte Marke. Ohne `nm` wird das
   Drehmoment aus Leistung und Motorkurve abgeleitet. Ohne Getriebeangaben wird übersetzt: Der letzte Gang
   erreicht die Vmax bei der Drehzahl der Spitzenleistung, der erste liegt an der Traktionsgrenze.
2. Weitere Angaben überschreiben die Klasse, Objekte werden tief zusammengeführt. Ein Beispiel:
   `"bremse": {"abs": false}` ändert nur das ABS. Ein Fahrzeug kann mit `"basis": "<id>"` auch von einem
   anderen Fahrzeug erben.
3. Kalibrieren:

   ```bash
   cargo run --release -p physics-calibrate -- --nur kompakt_fuenf_felder
   ```

   Das Werkzeug dreht nur die erlaubten Stellschrauben (cwA ±15 %, Reifen-μ ±10 %, Bremskraft 0,5–1,6,
   Übersetzung, Schaltzeit, Wirkungsgrad ±3 %). Masse, Leistung und Drehmoment bleiben, wie sie sind.
   Ergebnis: `vehicles.calibrated.json` und der Bericht `docs/kalibrierung/bericht.md`.
4. `cargo test -p berlin-sim vehdata` prüft, ob alle Fahrzeuge laden.

Tippfehler fallen sofort auf: Unbekannte Felder, Klassen, Reifen oder Motortypen bricht der Lader mit einer
Meldung ab, die den Eintrag nennt.

## Zielwerte

| Schlüssel | Bedeutung | Toleranz |
|-----------|-----------|----------|
| `0_100` (`0_50`, `0_25`, …) | Sekunden von 0 auf X km/h, Volllast, Start mit gespanntem Lader | ±7 % |
| `vmax` | km/h bis zur Beharrung oder zum Begrenzer | ±3 % |
| `brems_100` (`brems_50`, …) | Bremsweg in m ab Pedal, trocken | ±5 % |
| `quer_g` | stabile Querbeschleunigung auf dem 40-m-Kreis | ±0,05 g |

Lkw und Busse werden voll beladen gemessen, alles andere leer (`kalibrier_zuladung` überschreibt das).
Ist ein Ziel nicht erreichbar, schreibt das Werkzeug den Grund in den Bericht. Beispiele: Die Leistung reicht
physikalisch nicht, ein Begrenzer greift, oder ein Teil des Modells folgt erst in einer späteren Phase.
