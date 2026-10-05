# Referenzaufnahme: Segmente und Bewertung (Phase 1)

Quelle: `data/audio/raw/engine_reference_lambo.m4a` (unveränderte Kopie, SHA-256 `7dd39ebb…5f26`).
Erzeugt mit `tools/audio/analyze_reference.py`; die Tabelle unten ist von Hand aus `pitch.png`,
`spectrogram.png`, `frames.csv` und `transients.json` zusammengestellt.

## Basisdaten

| | |
|---|---|
| Länge | 8,75 s (Klang 0,12–8,45 s) |
| Format | AAC, 48 kHz, 2 Kanäle – **beide Kanäle identisch** (Korrelation 1,0), also Mono |
| Lautheit | −12,2 LUFS integriert, Lautheitsumfang 3,1 LU |
| Peak | −5,25 dBFS Sample-Peak, −5,2 dBTP True Peak |

## Aufnahmesituation (Angabe des Nutzers: das Auto startet durch und fährt weg)

Das Mikrofon steht, das Auto beschleunigt davon. Folgen für die Auswertung:

- **Doppler:** Eine sich entfernende Quelle klingt um den Faktor c / (c + v) tiefer. Bei geschätzt ~90 km/h am
  ersten Schaltpunkt (3,7 s) sind das ~7 %, bei ~160 km/h am Ende ~12 %. Die gemessene Drehzahl liegt dort also zu
  niedrig: Der erste Schaltpunkt eher bei ~7700 statt 7200 1/min, der Konstant-Abschnitt bei 7,2–7,8 s eher bei
  ~5400 statt 4800 (dort steigt die Drehzahl vermutlich weiter, der wachsende Doppler gleicht es aus). Die
  Schaltsprung-Verhältnisse bleiben davon unberührt. Die Geschwindigkeiten sind geschätzt, nicht gemessen.
- **Entfernung:** Der Pegel fällt von 2,0 s bis 7,4 s nur um ~4 dB, danach in 0,6 s um ~20 dB. Für freies Wegfahren
  wäre das zu wenig, die Aufnahme ist also vermutlich stark komprimiert. Hörbar wird das Wegfahren vor allem am
  Ende. Der spektrale Schwerpunkt sinkt nach dem ersten Zug von ~1650 Hz (3,5 s) auf ~1070 Hz (7,4 s): Spätere
  Abschnitte klingen ferner und dumpfer.
- **Daraus:** Loops bevorzugt aus dem nahen ersten Zug (1,0–3,7 s); spätere Abschnitte nur mit Höhenanhebung und
  Doppler-Korrektur (Resampling um c / (c + v) zurück). Das Ende ab 7,8 s ist **Entfernung bzw. Ausblendung, kein
  Gas-weg**. Die Pops dort bleiben als One-Shots brauchbar.

## Drehzahl-Annahme

Grundton aus einem Obertonkamm (pyin springt beim rauen Motorklang oft eine Oktave tiefer, siehe `pitch.png`).
Annahme **Viertakt-V10** (Vorbild von `supercar_awd`, `n_max` 8500):

- **A** Grundton = Zündfrequenz → Drehzahl = f0 × 12 → Spitze 3600 1/min. Für einen Zug bis zum Schaltpunkt
  unplausibel.
- **B** Grundton = halbe Zündfrequenz (Ordnung 2,5, je Zylinderbank ein Auspuffstrang) → Drehzahl = f0 × 24 →
  Leerlauf ~1300, Schaltpunkt ~7200, danach 5200–6500. **Plausibel, wird verwendet.**

Für das Spiel zählt vor allem das Verhältnis (Pitch = Drehzahl / Aufnahmedrehzahl); die absolute Zuordnung legt nur
fest, welcher Loop bei welcher Spieldrehzahl spielt.

## Segmente

| von s | bis s | Grundton Hz | Drehzahl (B) | Art | verwertbar |
|---|---|---|---|---|---|
| 0,00 | 0,12 | – | – | Stille | – |
| 0,12 | 0,15 | – | – | harter Einsatz (stärkster Knackser der Datei), Beginn eines Start-Barks **oder** nur der Schnitt der Aufnahme | One-Shot nur nach Anhören |
| 0,15 | 1,05 | 50–58 | 1200–1400 | tiefes Brabbeln, schwacher Kamm; darüber ein fremder gleichbleibender Ton bei ~240 Hz | Leerlauf-Loop mit Vorbehalt |
| 1,05 | 2,35 | 58 → 140 | 1400 → 3400 | Anfahren, Drehzahl steigt langsam (Kupplung/Traktion) | Last-Loops 2000 und 3000 |
| 2,35 | 2,60 | unsicher | ~3500–4300 | Übergang, Kamm kurz unscharf | – |
| 2,60 | 3,70 | 180 → 300 | 4300 → 7200 | Zug unter Volllast, steil | Last-Loops 5000, 6000, 7000 |
| 3,77 | 4,05 | – | – | **Hochschalten**: Zündunterbrechung, Knatter-Salve | Schalt-One-Shot, Pops |
| 4,05 | 5,25 | 215 → 275 | 5200 → 6600 | Zug im nächsten Gang | Last-Loops 5500–6500 |
| 5,35 | 5,45 | – | – | **Hochschalten** | Schalt-One-Shot |
| 5,45 | 7,05 | 215 → 250 | 5200 → 6000 | Zug im nächsten Gang, flach | Last-Loops 5500 |
| 7,14 | 7,20 | – | – | **Hochschalten** oder Gas-weg (Drehzahl −20 %) | Schalt-One-Shot |
| 7,20 | 7,80 | 200 konstant | 4800 (Doppler-korrigiert ~5400) | scheinbar konstant: Drehzahl steigt, wachsender Doppler gleicht aus; schon fern | Loop nur mit Korrektur |
| 7,86 | 8,45 | 200 → ? | – | Auto entfernt sich bzw. Ausblendung (−18 → −37 dB), Knackser bei 7,86, 8,03, 8,23 s | Pop-One-Shots (3 Varianten), **kein Schub** |

Schaltsprünge: 300 → 215 Hz (× 0,72), 275 → 213 Hz (× 0,78), 250 → 200 Hz (× 0,80) – passt zu den
enger werdenden Stufen eines Doppelkupplungsgetriebes.

## Was fehlt (Lücken)

1. **Kein stabiler Leerlauf.** Das tiefe Stück am Anfang ist kurz, leise im Kamm und von einem fremden
   Dauerton (~240 Hz) überlagert.
2. **Kein Schub-Sound.** Es gibt keinen Abschnitt mit fallender Drehzahl bei geschlossener Drosselklappe; das
   leiser werdende Ende ist Entfernung bzw. Ausblendung.
3. **Nichts über ~7200 1/min**, also weder Begrenzer noch die obersten 1300 1/min bis `n_max` 8500.
4. **Kein Anlasser.** Ob der Einsatz bei 0,12 s ein Start-Bark ist, lässt sich ohne Anhören nicht entscheiden.
5. **Fast nur Sweeps.** Außer 7,2–7,8 s ändert sich die Drehzahl ständig; ein naiver Ausschnitt würde als Loop
   „jaulen“.
6. **Kein Gasstoß (Blip)** im Stand.

## Vorschläge, wie die Lücken gefüllt werden

- **Loops aus Sweeps:** Fenster von 150–300 ms aus den Zügen schneiden und mit dem gemessenen Grundtonverlauf
  **zeitvariabel nachresamplen**, bis die Tonhöhe stillsteht („Glätten des Sweeps“). Danach Schnitt an
  Nulldurchgängen auf ganze Perioden des Grundtons und Crossfade am Loop-Punkt. Damit sind Last-Loops bei etwa
  1300, 2500, 4000, 5000, 6000 und 7000 1/min möglich.
- **Hohe Drehzahl (7200–8500):** den 7000er-Loop mit Pitch bis × 1,22 hochziehen (innerhalb der Grenze 0,7–1,4).
  Für Hypercars zusätzlich ein leichter Synthese-Layer mit hellem Obertonanteil.
- **Schub (off-load):** abgeleitet statt aufgenommen. Last-Loop mit Tiefpass (~1,5 kHz), weniger Pegel
  (−8 bis −10 dB) und mehr Rauheit; einen echten Schubausschnitt gibt es nicht. Der bestehende
  Synthesizer des Spiels (`synth.rs`) kann einen Schub-Layer mit Knistern liefern.
- **Leerlauf:** Loop aus 0,4–0,9 s, den Fremdton mit einem schmalen Notch-Filter bei ~240 Hz (und 480 Hz)
  herausnehmen; reicht das nicht, Leerlauf per Pitch-Shift aus dem 2500er-Loop (× 0,55 liegt unter der Grenze 0,7,
  also eher aus dem Anfahrstück bei ~1700) oder per Synthese.
- **Begrenzer:** kein Sample nötig, rhythmisches Abschneiden (Gain-Gating ~12–18 Hz) des obersten Loops.
- **Blip/Gasstoß:** aus 1,05–1,6 s (Anstieg 1400 → 2700) mit kurzem Abfall per Hüllkurve.
- **Pops:** 3 Varianten aus 7,86 / 8,03 / 8,23 s, 2 weitere aus der Schaltsalve 3,77–4,0 s.
- **Start:** Einsatz 0,12–0,8 s als „Start-Bark“ verwenden, falls er beim Anhören so klingt; sonst synthetisch.
