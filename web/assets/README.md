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
| `car` | Draufsicht, Front nach **rechts**, wird auf 42 × 20 skaliert (ersetzt die fünf eingebauten Modelle; Wagenfarbe, Räder, Licht und Blinker entfallen dann) |
| `pedestrian`, `player` | Draufsicht, Blick nach rechts, 16 × 16 |
| `tree` | Baumkrone von oben, ca. 40 × 40 |

Sound-Schlüssel: `crash`, `hit`, `horn`, `door`, `ui`, `ui-move`, `ui-back`, `tick`, `pickup`, `mission-start`,
`mission-success`, `mission-fail`, `carjack`. Der Motor bleibt synthetisch (er folgt der Geschwindigkeit).

Die Stadt selbst ist in `src/map.js` (`LAYOUT`, Straßennamen) austauschbar, der Missionstext in `src/mission.js` (`BRIEFING`).
