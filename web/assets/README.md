# Assets austauschen

Alle Grafiken und Klänge sind Platzhalter, die das Spiel selbst zeichnet bzw. synthetisiert. Nichts stammt aus fremden Quellen.
Eigene Dateien einbinden: Datei hier ablegen und in `manifest.json` eintragen.

```json
{
  "sprites": { "car": "sprites/car.png", "pedestrian": "sprites/ped.png", "player": "sprites/player.png", "tree": "sprites/tree.png" },
  "sounds":  { "crash": "sounds/crash.ogg", "horn": "sounds/horn.ogg" }
}
```

| Sprite | Ausrichtung / Größe im Spiel |
|---|---|
| `car` | Draufsicht, Front nach **rechts**, wird auf 42 × 20 skaliert (ersetzt die prozedurale Pkw-Darstellung; Wagenfarbe, Räder, Licht und Blinker entfallen dann) |
| `pedestrian`, `player` | Draufsicht, Blick nach rechts, 16 × 16 |
| `tree` | Baumkrone von oben, ca. 40 × 40 |

Sound-Schlüssel: `crash`, `hit`, `horn`, `door`, `ui`, `ui-move`, `ui-back`, `tick`, `pickup`, `mission-start`,
`mission-success`, `mission-fail`, `carjack`. Der Motor bleibt synthetisch. `src/enginevoice.js` enthält die modellabhängigen
Klangprofile und Impulsspektren; `src/soundscape.js` berechnet Drehzahl, Gang und Last,
`src/audio.js` erzeugt und mischt die Klänge. Ein Eintrag `sounds.engine` ersetzt
nicht automatisch diese laufende Synthese. Motoraufnahmen erfordern eine eigene
Integration mit Drehzahl-/Lastübergängen. **M** schaltet die komplette Mischung stumm.

Die Stadt wird aus den Kartenkacheln unter `data/berlin/` geladen; `src/map.js`
verwaltet Installation und Streaming. Die Kartenpipeline liegt unter `tools/osm/`
im Repository. Missionstexte und Ablauf liegen in `src/mission.js`.

Fahrzeugdetails werden in `src/vehicleart.js` gezeichnet und über `src/vehicles.js`
als Bilder zwischengespeichert. Vierfache Sprite-Auflösung bedeutet mehr Bilddetails,
keine Änderung der Weltmaße oder Kollisionshüllen. Alle Motorprofile und Zeichnungen
sind selbst erzeugt; es werden keine Originalaufnahmen oder Fahrzeugfotos mitgeliefert.
