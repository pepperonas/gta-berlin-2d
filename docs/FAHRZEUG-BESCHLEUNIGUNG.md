# Beschleunigungsabstimmung – 1. Oktober 2026

## Befund und Ziel

Gemessen wurde der tatsächliche Spielerpfad `stepCar → stepDynamics`, nicht die
vereinfachte Verkehrsphysik. 45 Modelle, trockener Asphalt, ESP/ABS aktiv,
Vollgas, geradeaus, 120 Hz, Start aus dem Stand. Geschwindigkeitsüberschreitungen
werden zwischen Messschritten interpoliert. Alle Zeiten sind Simulationssekunden.

Die Ausgangsmessung bestätigt **keinen allgemeinen Anstieg der Beschleunigung mit
Geschwindigkeit**. Bei Heckantrieb nimmt sie während des kurzen Aufbaus der
Hinterachslast zu; danach fällt sie oder bleibt über einen zu großen Bereich fast
konstant. Beim Musclecar: 6,44 m/s² bei 30 km/h und noch 5,88 m/s² bei 100 km/h.
Das Verhältnis zwischen Antritt und späterem Durchzug war damit sehr flach.

Die Motorformel verlangte `min(P / vLow, P / v)` Radkraft: früh Spitzenleistung,
beim Start bei vielen Modellen weit mehr Kraft als die Reifen übertragen können.
Mehr Motorleistung allein hätte hier hauptsächlich die ASR-Begrenzung erhöht.
Die Sound-Gänge sind eine separate Darstellung und bestimmen die Zugkraft nicht.

## Reale Vergleichsfahrzeuge

Die Spielmodelle sind fiktiv; die folgenden Fahrzeuge sind Vergleichsgrößen nach
Bauart und Leistung. Herstellerwerte beziehen sich auf bestimmte Varianten und
Startbedingungen, nicht auf jedes Fahrzeug derselben Baureihe.

| Vergleich | Herstellerangabe | Bezug zur Abstimmung |
|---|---|---|
| [Ford Mustang GT, 2018, 5.0 V8, Automatik](https://media.ford.com/content/fordmedia/feu/gb/en/news/2018/03/12/new-ford-mustang-looks-faster--goes-faster--more-athletic-stylin.html) | 450 PS, 0–100 km/h 4,3 s | Musclecar mit 330 kW; vorher 4,45 s, jetzt 3,71 s als bewusst spritzigere Spielvariante. |
| [VW Golf GTI Clubsport](https://www.volkswagen-newsroom.com/en/press-releases/the-new-golf-gti-clubsport-celebrates-its-world-premiere-at-the-24-hour-race-at-the-nuerburgring-18422/download) | 300 PS, 0–100 km/h 5,6 s | Hot Hatch mit 221 kW; bereits vorher schneller als der reale Vergleich. Keine Behauptung einer originalgetreuen Nachbildung. |
| [Toyota GR Yaris, 2020](https://newsroom.toyota.eu/gr-yaris-born-from-the-world-rally-championship/) | 261 PS, 360 Nm, 0–100 km/h 5,5 s | Rallye-Allradler; startet im Spiel bereits extrem schnell. Erhält keinen weiteren Traktionsbonus. |
| [Porsche Taycan Turbo S, erste Generation](https://newsroom.porsche.com/en_AU/products/taycan/powertrain-18555.html) | 0–100 km/h 2,8 s, 0–200 km/h 9,8 s | Zweiter 100-km/h-Abschnitt dauert 7,0 s: deutlich nachlassender Schub trotz hoher Leistung. Kurzer erster Gang ermöglicht hohe Radzugkraft beim Start. |
| [VW Golf GTI, 2024](https://www.volkswagen-newsroom.com/de/pressemitteilungen/volkswagen-startet-den-vorverkauf-des-neuen-und-staerkeren-golf-gti-18394) | 370 Nm ab 1.600 U/min; 265 PS bei 5.250–6.500 U/min | Kräftiger Antritt braucht nutzbares Drehmoment/Übersetzung. Spitzenleistung steht nicht bei jeder Drehzahl an. |

Die Quellen liefern keine belastbaren 0–50-/0–70-Zwischenzeiten für diese
Gesamtauswahl. Solche Werte in der Tabelle unten sind ausschließlich **eigene
Spielmessungen**. Die Änderung orientiert sich am Verlauf der Beschleunigung;
sie ist keine vollständige Drehmoment-, Kupplungs- und Getriebesimulation.

## Umsetzung

- Bis 20 km/h steht der volle Anfahrbonus bereit; zwischen 20 und 90 km/h läuft
  er mit einer glatten Kurve aus. Ab 90 km/h ist er vollständig weg.
- Maximal 28 % zusätzliche verlangte und übertragbare Längskraft, mit weniger
  Spielraum für Allrad. Die Achslastberechnung begrenzt den Bonus: Modelle, die
  schon ungefähr 10 m/s² Startbeschleunigung schaffen, erhalten keinen weiteren
  Gripbonus. Das ist keine nachträgliche harte Begrenzung der Fahrzeugbewegung.
- Auf glattem Untergrund wird der Bonus reduziert, auf Eis und bei Aquaplaning
  entfällt er. Bei starkem Lenkeinschlag, Handbremse oder aktivem Drift gibt es
  keinen Bonus. Motorräder und Motorroller behalten ihre bisherigen Startgrenzen.
- Zwischen 50 und 120 km/h sinkt die verfügbare Leistungshüllkurve weich auf
  86 %. Diese Zahl ist eine Spielabstimmung für mittlere Radleistung, kein
  recherchierter Wirkungsgrad. Damit bleibt der Antritt relativ zum oberen
  Geschwindigkeitsbereich kräftiger.
- Luftwiderstand wird mit derselben Leistungshüllkurve kalibriert, damit die
  modellabhängigen Höchstgeschwindigkeiten erhalten bleiben.
- Der zusätzliche Längsgrip wird in der Reibellipse berücksichtigt. Er erhöht
  nicht den Seitenhalt und darf beim Beschleunigen nicht plötzlich die gesamte
  Lenkfähigkeit aufbrauchen.

## Messwerte vorher → nachher

Alle Angaben in Sekunden, auf zwei Nachkommastellen gerundet.
„—“ bedeutet: innerhalb des 120-s-Messlaufs nicht erreicht.

| Spielmodell | 0–50 km/h | 0–70 km/h | 0–100 km/h |
|---|---:|---:|---:|
| zweitakter | 3.75 → 3.03 | 7.14 → 5.99 | 20.03 → 19.20 |
| kleinwagen | 2.47 → 2.04 | 4.00 → 3.42 | 7.59 → 7.23 |
| kompakt | 2.36 → 1.96 | 3.38 → 2.90 | 5.60 → 5.27 |
| limousine | 1.83 → 1.50 | 2.58 → 2.19 | 4.14 → 3.85 |
| taxi | 2.01 → 1.57 | 3.19 → 2.66 | 5.85 → 5.49 |
| kombi | 1.23 → 1.23 | 2.06 → 2.06 | 3.88 → 4.03 |
| transporter | 2.87 → 2.38 | 4.64 → 3.98 | 8.91 → 8.50 |
| elektro | 1.15 → 1.15 | 1.76 → 1.76 | 3.09 → 3.20 |
| gelaende | 1.41 → 1.40 | 2.39 → 2.40 | 4.64 → 4.81 |
| sportwagen | 1.33 → 1.33 | 1.85 → 1.85 | 2.66 → 2.69 |
| heckcoupe | 1.27 → 1.27 | 1.76 → 1.76 | 2.62 → 2.68 |
| hothatch | 2.14 → 1.77 | 3.01 → 2.58 | 4.35 → 3.91 |
| roadster | 1.68 → 1.50 | 2.56 → 2.34 | 4.52 → 4.45 |
| musclecar | 2.19 → 1.62 | 3.08 → 2.38 | 4.46 → 3.71 |
| oldtimer | 2.83 → 2.15 | 4.68 → 3.82 | 9.26 → 8.65 |
| pickup | 2.35 → 1.70 | 3.53 → 2.77 | 6.25 → 5.66 |
| kleinbus | 2.90 → 2.87 | 5.47 → 5.40 | 13.61 → 13.88 |
| rallye | 1.06 → 1.06 | 1.59 → 1.60 | 2.78 → 2.87 |
| supersport | 0.95 → 0.95 | 1.33 → 1.33 | 1.91 → 1.93 |
| gtcoupe | 1.45 → 1.45 | 2.02 → 2.02 | 2.88 → 2.88 |
| leichtbau | 1.44 → 1.44 | 2.00 → 2.00 | 3.04 → 3.11 |
| elektrosport | 1.02 → 1.02 | 1.43 → 1.43 | 2.15 → 2.20 |
| sprinter | 2.41 → 2.03 | 4.32 → 3.81 | 8.90 → 8.69 |
| hochdach | 2.74 → 2.28 | 4.26 → 3.66 | 7.77 → 7.40 |
| powerkombi | 1.02 → 1.02 | 1.43 → 1.43 | 2.24 → 2.30 |
| familienkombi | 2.37 → 1.97 | 3.35 → 2.87 | 5.35 → 5.00 |
| business | 1.78 → 1.49 | 2.49 → 2.13 | 3.75 → 3.48 |
| sportlimo | 1.66 → 1.48 | 2.31 → 2.10 | 3.31 → 3.09 |
| luxus | 1.87 → 1.49 | 2.62 → 2.16 | 3.81 → 3.40 |
| coupe | 1.73 → 1.49 | 2.42 → 2.12 | 3.47 → 3.17 |
| leichtcoupe | 1.95 → 1.50 | 2.73 → 2.19 | 4.06 → 3.59 |
| gklasse | 1.25 → 1.25 | 1.93 → 1.94 | 3.43 → 3.54 |
| defender | 1.35 → 1.35 | 2.23 → 2.23 | 4.19 → 4.34 |
| niva | 2.25 → 2.24 | 4.07 → 4.07 | 8.69 → 9.00 |
| kompaktsuv | 2.52 → 2.11 | 3.71 → 3.20 | 6.39 → 6.05 |
| sportsuv | 1.12 → 1.12 | 1.77 → 1.78 | 3.21 → 3.32 |
| grosssuv | 1.16 → 1.16 | 1.86 → 1.87 | 3.41 → 3.53 |
| motorcycle | 1.38 → 1.38 | 1.94 → 1.94 | 2.82 → 2.80 |
| scooter | 2.09 → 2.08 | 4.17 → 4.12 | — → — |
| police | 1.86 → 1.50 | 2.63 → 2.20 | 4.25 → 3.94 |
| ambulance | 3.17 → 2.91 | 5.82 → 5.46 | 12.63 → 12.70 |
| delivery | 3.41 → 2.81 | 5.82 → 4.97 | 11.88 → 11.37 |
| truck | 4.79 → 4.23 | 10.14 → 9.15 | 39.88 → 40.22 |
| garbage | 7.37 → 6.63 | 17.50 → 15.94 | — → — |
| bus | 5.41 → 4.51 | 12.26 → 10.54 | — → — |

## Reproduzieren und Absicherung

```sh
node tools/benchmark-vehicles.mjs
node tools/benchmark-vehicles.mjs musclecar kompakt hothatch
node --test tests/acceleration.test.js tests/dynamics.test.js tests/car.test.js tests/soundscape.test.js
```

Die gezielte Prüfung umfasst fallende Kraftverläufe für sämtliche 45 Modelle,
stetige Übergänge, tatsächliche Beschleunigung bei 30/70 km/h, Vorher-Nachher-Ziele,
30/60/120-Hz-Vergleich, Teillast, Wetter, Aquaplaning, Wiese, Höchsttempo sowie die
bestehenden ESP-, ABS-, Lenk-, Brems- und Handbremsdriftprüfungen.

Der bestehende Drifttest akzeptiert weiterhin mindestens 20 sichtbare
Schlupfbilder (eine Drittelsekunde). Die geänderte Zugkraft verschiebt den
Schwellwert gegenüber der bisherigen strikten Grenze um ein Bild; Driftwinkel,
Gegenlenken und Restgeschwindigkeit bleiben separat abgesichert.
Der Soundtest verwendet jetzt explizit den Benziner `kompakt`: Ein zufälliges
Modell anhand der Fahrzeug-ID darf nicht als garantierter Vierzylinder gelten.

## Prüfergebnis

- 33 gezielte Tests bestanden (Beschleunigung, Fahrdynamik, Fahrzeugbasis, Motordrehzahl).
- 45 Modelle über 120 Simulationssekunden vermessen. Größte Abweichung der
  Endgeschwindigkeit vom Datenwert: 0,02 km/h.
- Alle 37 Pkw mit Vollgas und Lenkeinschlag links/rechts je fünf Sekunden auf
  trockener, nasser und verschneiter Straße: größter Hinterachsschlupf 0,085 rad.
- Zusätzlicher Ablauf für alle Pkw: beschleunigen, bremsen, rückwärtsfahren,
  wieder vorwärtsfahren, jeweils mit und ohne ESP/ABS: alle 74 Abläufe endlich
  und am Ende vorwärts fahrend.
- Gesamtsuite vor Ergänzung des letzten Kurvenstart-Tests: 461 Tests,
  444 bestanden, 17 fehlgeschlagen. Diese 17 Fehler wurden anschließend mit
  der alten `dynamics.js` bei ansonsten identischem Arbeitsstand reproduziert.
  Dazu wurde ausschließlich im temporären Node-Modullader die alte Physik
  geladen; Arbeitsdateien wurden nicht zurückgesetzt. Im Kontrolllauf kam
  zusätzlich ein Verkehrs-/Rotlichttest hinzu, der im Gesamtlauf bestand.
  Es gibt in diesem Vergleich keinen neuen Fehler durch die Physikänderung.
- Die bestehenden Fehler betreffen unter anderem frühere Änderungen an
  Mausinteraktionen, NPC-Stürzen, Fahrzeugklang, Bahnhöfen und Verkehr.
- Ein manueller Fahrgefühlvergleich im Browser wurde nicht durchgeführt.
