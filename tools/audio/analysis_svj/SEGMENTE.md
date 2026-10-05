# Referenz V12 (Prüfstand): Segmente und Bewertung

Quelle: `data/audio/raw/engine_reference_svj.m4a` (unveränderte Kopie, SHA-256 `5acf1d6d…9208`), ein
Twin-Turbo-V12 (1000+ PS) am Rollenprüfstand. Erzeugt mit
`tools/audio/analyze_reference.py data/audio/raw/engine_reference_svj.m4a tools/audio/analysis_svj`; die
Drehzahl-Spalten in `frames.csv` und `pitch.png` rechnen mit V10-Annahmen und gelten hier **nicht** – für den V12
gilt die Linienverfolgung im Build-Skript (`line_track`).

## Basisdaten

| | |
|---|---|
| Länge | 168,6 s (Klang bis ~167,5 s) |
| Format | AAC, 48 kHz, Stereo (Korrelation 0,99, praktisch Mono) |
| Lautheit | −14,9 LUFS integriert |
| Peak | −0,1 dBFS – die Fehlzündungen sind im Original übersteuert (dekodiert bis 1,3) |

## Drehzahl-Annahme

Viertakt-V12: Zündfrequenz = Drehzahl/60 × 6. Die hellste Linie eines Zugs steigt von ~250 auf ~870 Hz; als
Zündfrequenz gelesen sind das **2500 → 8700 1/min**, genau der Drehzahlbereich eines V12-Supersportwagens bis zum
Begrenzer. Drehzahl = f0 × 10. Das Mikrofon steht (Prüfstand), es gibt keinen Doppler.

## Segmente (Auswahl)

| von s | bis s | Linie Hz | Drehzahl | Art | verwendet |
|---|---|---|---|---|---|
| 0,0 | 1,3 | flach | – | Leerlauf mit Schnittkanten (Video) | – |
| 15,0 | 16,9 | 129 → 120 | ~1235 | Leerlauf, ruhig; danach läuft die Linie weg | `on_idle` (+ abgeleitet `off_idle`) |
| 21,2 | 27,4 | 252 → 870 | 2500 → 8700 | **Volllast-Zug**, sauber | Last-Loops 2700, 3300, 4100, 5600, 6800, 8400 |
| 27,6 | 31,1 | 800 → 550 | | Ausrollen, Linie wechselt mehrfach | – |
| 48,6 | 51,3 | – | – | Schnitt: anderer Inhalt | – |
| 70,8 | 75,1 | 150 → 450 → | | Gasstöße im Stand | `blip_1`, `blip_2` (hinter dem Schnitt bei 74,2 s) |
| 80,0 | 83,2 | 520 → 860 | 5200 → 8600 | Volllast-Zug | – (Reserve) |
| 83,4 | 89,3 | 810 → 310 | 8100 → 3100 | **echtes Ausrollen (Schub)**; Nachbarlinie kreuzt bei 85,4 und 88,6 s | Schub-Loops 4000, 4600, 5800, 7700 |
| 95,0 | 99,0 | ~370–400 | ~3800 | konstante Teillast | – |
| 99,5 | 139 | | | **Fehlzündungen** (die „Flames“), einzeln und in Salven | `pop_1…7` |
| 120 | 140 | | | Begrenzer-Hüpfen (Linie zittert) | – (Begrenzer bleibt synthetisch) |

## Bewertung

- **Besser als die V10-Aufnahme:** lange, saubere Züge, echter Schub, kein Doppler, viele Fehlzündungen.
- **Lücke 1235 → 2700:** kein Last-Material dazwischen (der Zug beginnt bei 2500). Die Tonhöhengrenzen werden
  an dieser Stelle bis zur Mitte der Lücke erweitert (`pitch_bounds`, bis × 1,53 bzw. × 0,65).
- **Schub unter 4000** ist aus Leerlauf und 2700er-Loop abgeleitet; unter ~3100 gibt es kein sauberes Ausrollen.
- **Kein Anlasser, keine Schaltvorgänge** (Prüfstand im festen Gang): Start- und Schaltgeräusch fehlen in dieser
  Bank, das Spiel spielt dann keins.
- **Störgeräusche:** Prüfstandslüfter und Rollen liegen als Rauschen unter allem; im Spiel fällt das neben Reifen-
  und Fahrtwindgeräusch kaum auf.
