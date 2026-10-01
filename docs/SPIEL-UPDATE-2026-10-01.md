# Spielstand vom 1. Oktober 2026

Dieser Bericht beschreibt den Entwicklungsstand auf Basis von Paketversion 0.46.0.
Die Änderungen stehen im Changelog unter **Unreleased**. Historische Beschreibungen
älterer Versionen sind keine aktuelle Bedienungsanleitung.

## Schnell ausprobieren

1. `npm start` im Projektverzeichnis starten, anschließend `http://localhost:8080/` öffnen.
2. Nach Codeänderungen das Spiel neu laden. Bei einer bereits lange geöffneten Sitzung
   in Chrome auf dem Mac **Cmd+Umschalt+R** verwenden. Ein Neustart des Servers allein
   ersetzt keine bereits geladenen JavaScript-Module im Browser.
3. **Enter**, `auto musclecar`, **Enter**: Fahrzeug neben der Figur erzeugen.
4. Am Fahrzeug **F** drücken; **W/S** Gas/Bremse, **A/D** lenken.
5. **X** schaltet ESP, **Y** ABS. Beide Fahrhilfen sind standardmäßig aktiv.
6. Für den Beschleunigungsvergleich eine gerade, trockene Straße nutzen. Zum gezielten
   Einstellen: `wetter sonnig`, `nass 0`, `schnee 0`, `glaette 0`.
7. **M** schaltet den Gesamtton, **Tab** öffnet den Stadtplan.

Die Enter-Konsole pausiert die Welt. Eine erfolgreiche Texteingabe schließt sie;
**Umschalt+Enter** lässt sie offen. Vorschläge mit **Tab/→** übernehmen, mit **↑/↓**
auswählen; **Esc** leert die Eingabe und schließt beim nächsten Druck.

## Wochentag und Uhrzeit

| Eingabe | Ergebnis |
|---|---|
| `tag` | Aktuellen Wochentag anzeigen |
| `tag Freitag` | Freitag wählen |
| `wochentag Sonntag` | Alias für die Tagesauswahl |
| `day Montag` | Weiterer Alias; Werte bleiben deutsch |
| `tag Mo`, `tag Mo.` | Deutsche Zweibuchstaben-Kürzel |
| `tag 1` … `tag 7` | Montag … Sonntag |
| `tag Sonnabend` | Samstag |
| `Freitag` | Direkte Tagesauswahl ohne Befehlswort |
| `zeit 21:30` | Uhrzeit setzen |
| `tempo 0`, `tempo 1` | Spieluhr anhalten bzw. normal weiterlaufen lassen |

Alle sieben Tagesnamen stehen als Argumentvorschläge bereit. Groß-/Kleinschreibung
ist unerheblich. Unbekannte Tageswerte geben eine Fehlermeldung aus. Die generische
Befehlspalette kann unvollständige Eingaben beim Bestätigen vervollständigen.
Eine Zahl ohne Befehlswort wird weiterhin als Uhrzeit gelesen: `7` = 07:00.

Intern wird ausschließlich `world.day` gesetzt (Montag = 0, Sonntag = 6).
`world.clock`, `world.dayCount`, Wetter und Simulationszeit bleiben unverändert.
Es wird also ein Wochentag gewählt, keine Anzahl kompletter Tage übersprungen.
Der Tagesrhythmus für Verkehr, Passanten und Nachtleben nutzt den neuen Wert
beim Weiterlaufen. Bestehende Personen und Autos werden dabei nicht schlagartig
entfernt; die normale Bevölkerungsverwaltung passt den Bestand an. Mitternacht
schaltet regulär zum Folgetag weiter. Das bestehende Spielstandformat speichert
den Wochentag bereits; eine Formatmigration ist nicht erforderlich.

## Wettertafel

**Enter → `wetter` → Enter** öffnet die Tafel:

| Zeile | Änderung mit ←/→ |
|---|---|
| Wettertyp | Natürlich (`auto`) und die verfügbaren Wetterarten |
| Temperatur | Schrittweise Gradwerte; Rückkehr zur natürlichen Temperatur über `temp auto` möglich |
| Schneedecke | 0–1 in Zehntelschritten |
| Straßennässe | 0–1 in Zehntelschritten |
| Glätte | 0–1 in Zehntelschritten |

**↑/↓** wechselt die Zeile. Änderungen wirken unmittelbar auf die Weltwerte.
**Enter** verlässt die Tafel zurück zur Eingabe; **Esc/Backspace** ebenfalls.
Es gibt keinen getrennten Entwurf und keinen Rückgängig-Schritt für bereits
veränderte Werte. `wetter regen`, `temp -5` oder `schnee 0.5` bleiben als direkte
Befehle verfügbar. Nach dem Fortsetzen entwickelt die Wettersimulation Nässe,
Schnee und Eis wieder weiter; beispielsweise taut Glätte bei Wärme.

## Steuerung und Anzeigen

### Springen und Schwimmen zu Fuß

- **Steuerung:** Leertaste löst am Boden einen Sprung aus. WASD/Pfeiltasten bewegen die Figur dabei weiter. Die
  Leertaste bleibt in Fahrzeugen die Handbremse; der Sprung wird nur bei Fußsteuerung ausgewertet. Im Spiel ist
  außerdem weiterhin Umschalt die Sprinttaste.
- **Sprung:** Vertikale Bewegung wird separat von der Bewegung auf der Karte simuliert und im Renderer als
  Höhenversatz gezeichnet. Der anfängliche Impuls beträgt 5,1 und die Schwerkraft 12 (in den Spielkoordinaten).
  Während die Figur sichtbar über dem Boden ist, werden Zaunkollisionen übergangen. Das erlaubt das Überspringen
  niedriger Zäune, ohne die sonstigen Gebäudewände oder beliebige feste Hindernisse zu entfernen.
- **Wasser:** Der Schwimmzustand kommt aus der Kartenabfrage `surfaceAt` und gilt auf Wasserflächen der geladenen
  Stadtkarte. Beim Betreten wird die Figur als Schwimmer gezeichnet und die Bewegung auf 18 Weltpunkte/s (etwa
  1,8 m/s) begrenzt. Sprinten wird im Wasser nicht angewendet; die normale Ausdauer erholt sich weiter.
- **Ufer:** Die Kartendaten kennzeichnen Uferbarrieren als `quay`. Diese Kollision wird während eines Sprungs oder
  im Schwimmzustand übergangen, damit ein Sprung ins Wasser und das Zurückschwimmen ans Land möglich sind. Andere
  Wandarten wie Gebäude, Gleise und Stadtgrenze behalten ihre Kollision.
- **Controller:** Schwimmen funktioniert mit dem linken Stick. Die Sprungaktion ist derzeit nur auf der Tastatur
  belegt. Im Wasser löst Leertaste keinen Sprung aus.
- **Grenze der Umsetzung:** Es gibt derzeit keinen Ertrinkens-/Atemluft-Timer, keine Schwimm-Ausdauer und keine
  Wasserphysik für Fahrzeuge. Die Mechanik betrifft nur die Spielerfigur zu Fuß.

- Standard-Mausbelegung: links Bewegung, rechts Angriff. Die Option zum Vertauschen
  liegt im Menü **Steuerung** auf **↓** und wird als `gta-mouse-swapped` lokal gespeichert.
- **←/→** im selben Menü schaltet weiterhin Diablo/Klassisch (`gta-controls`).
- Fahrzeugzugang und Einstieg in unterirdische Bahnhöfe verwenden **F** am PC
  bzw. die vorhandene Controller-Aktion **Y**. Reines Hineinlaufen öffnet keinen Bahnhof.
- Das Waffenrad verwendet die Kombination beider Maustasten. Am Controller bleibt
  das Halten von **LB**. Die vollständige Mausinteraktion einschließlich vertauschter
  Belegung ist in diesem Dokument nicht als manuell abgenommen ausgewiesen.
- **M** verändert den Master-Gain für sämtliche Klangquellen mit einem kurzen
  weichen Übergang. Der Zustand gilt für die aktuelle Sitzung; er wird nicht als
  dauerhafte Audioeinstellung gespeichert. Während Texteingaben ist M ein Zeichen.
- Der Fahrzeugbereich im HUD bleibt klein und ohne umschließenden halbtransparenten
  Kasten. Er zeigt Geschwindigkeit, Drehzahl/Gang, Schaden, Antrieb und Fahrhilfen.
  Leistungsangaben erscheinen in **PS**; die Physikdaten verwenden intern weiterhin kW.

## Fahrzeuge: Darstellung

`vehicles.js` organisiert Modelle, Sprite-Cache, Räder und dynamische Leuchten.
`vehicleart.js` zeichnet die detaillierten Pkw-Oberflächen. Modellfamilien teilen
Zeichenbausteine, unterscheiden sich aber in Kontur, Hauben-/Dachproportionen,
Verglasung und Anbauteilen. Dazu kommen Fugen, Griffe, Spiegel, Grill, Stoßfänger,
Leuchten, Lufteinlässe, Streifen oder Reling je Fahrzeug.

Die gespeicherten Fahrzeugbilder werden mit vierfacher Zeichenauflösung erzeugt.
Die dargestellten Weltmaße und Kollisionshüllen werden dadurch nicht größer.
Sonderfahrzeuge und beschädigte Varianten haben eigene Details. Es werden keine
externen Fahrzeugfotos oder Markenmodelle geladen.

## Fahrzeuge: Beschleunigung und Drift

Die Spielerphysik rechnet in SI-Einheiten mit vier Teilschritten. Antrieb, Masse,
Achslastverteilung, Schwerpunkt, Reifenhaftung und Leistungsgrenze bestimmen das
Verhalten; die Welt verwendet 10 Pixel je Meter. Der Verkehr hat weiterhin seine
vereinfachte, auf die Verkehrssteuerung abgestimmte Physik.

Die neue `driveEnvelope` verstärkt den Antritt und lässt den Bonus bis 90 km/h
weich auslaufen. Ein reiner Leistungsaufschlag hätte bei bereits traktionsbegrenzten
Autos keinen entsprechenden Startvorteil bewirkt. Deshalb werden verlangte Kraft
und verfügbare Längshaftung zusammen angepasst und in der Reibellipse berücksichtigt.

Die Hilfe wird durch Untergrund und Lenkeinschlag reduziert. Bei Handbremse,
aktivem Handbremsdrift, Eis oder Aquaplaning entfällt sie. Bereits extrem schnelle
Sport-/Allradstarts werden nicht weiter verstärkt. Zweiräder behalten ihre
Wheelie-/Stoppie-Grenzen. Oberhalb 50 km/h beginnt die Anpassung der mittleren
Radleistung; der Luftwiderstand wird auf dieselbe Grundlage bezogen.

Ein kurzer Handbremsimpuls soll weiterhin einen steuerbaren Drift erlauben;
Gegenlenken baut ihn ab. ESP-Aus erlaubt mehr Übersteuern unter Gas. ESP/ABS sind
bei allen Fahrzeugen verfügbar, also auch beim Musclecar und älteren Modellen.

**Messwerte und Grenzen:** [vollständiger Beschleunigungsbericht](FAHRZEUG-BESCHLEUNIGUNG.md).
Die Abstimmung ist bewusst zugänglicher und teilweise erheblich schneller als
reale Herstellerwerte. Es wird keine originalgetreue Gang-/Kupplungssimulation behauptet.

## Fahrzeuge: Klang

| Modul | Aufgabe |
|---|---|
| `enginevoice.js` | Modellzuordnung, Resonanzen, Pulsbreite, Rauheit, Turbo, Dämmung, Fourierkoeffizienten |
| `soundscape.js` | Drehzahl, Gang, Last, Ladedruck, Rekuperation, Reifen-/Untergrundzustand, Verkehrs-Stimmen |
| `audio.js` | Web-Audio-Oszillatoren, periodische Wellen, Filter, Rauschschichten, Mischung, Richtung, Stummschaltung |

Ein kompletter Viertaktzyklus wird über 720°, ein Zweitaktzyklus über 360° modelliert.
Zylinderzahl, Impulsstärken und Bankunterschiede formen den Auspuffcharakter.
Periodische Wellen werden je Profil zwischengespeichert. Eine getrennte tiefe
Kurbelwellenkomponente und Bassfilter sorgen für mehr Körper; breitere Impulse,
weniger Sättigung und niedrigere Filtergrenzen nehmen scharfe Höhen zurück.

Die Mischung unterscheidet Ansaugung, Auspuff, Dieselnageln, Turbo, Rückwärtsgang,
Reifen und Fahrtwind. Bestimmte sportliche Profile können beim Gaswegnehmen einen
kurzen Auspuffimpuls erzeugen; Turbos bauen Druck hörbar ab. Beide Effekte werden
begrenzt ausgelöst. Elektroautos verwenden leises drehzahlabhängiges Summen mit
Last-/Rekuperationsanteil und keinen Verbrennertakt; im Stand bleibt diese Schicht still.

Der Verkehr verwendet dieselben Modellprofile. Bis zu vier nahe Fahrzeuge werden
mit Entfernung, Stereorichtung und Doppler gemischt. Fahrräder/E-Roller erhalten
keinen künstlichen Verbrennungsmotor. ESP/ABS-geregeltes Bremsen erzeugt bei aktiver
Fahrdynamik nicht allein wegen des Bremspedals ein Reifenquietschen.

Die Klang-Gänge treiben HUD und Synthese an; sie sind nicht die Gangzustände einer
vollständigen Antriebsstrangsimulation. Alle Motoren bleiben prozedurale Synthese,
keine Aufnahmen bestimmter Hersteller. Ein abschließender subjektiver Hörtest
für die dunklere Abstimmung ist noch offen.

## Kollisionen und NPCs

- Umfahrbare Hindernisse: `KNOCK.speed = 8` Weltpixel/s, ungefähr 2,9 km/h entlang
  der Aufprallnormalen. Nach dem Umfahren bleiben 96 % der Geschwindigkeit,
  Fahrzeugschaden pro Hindernis 1,5 Punkte. Das betrifft als Barrieren modellierte
  Objekte; normale Mauern oder Gebäude werden dadurch nicht zerstörbar.
- Zaunsegmente aus der Kollisionsliste erhalten beim Laden eine zusätzliche
  sichtbare Linie mit Saum und Pfosten. Die Änderung greift in jeder geladenen
  Kachel ohne Neubau der Kartendaten. Sie betrifft Zaunkollisionen, nicht sämtliche
  möglichen Unsichtbarkeiten von Gebäuden, Ufern oder der absichtlichen Stadtgrenze.
- Nicht tödlicher NPC-Schaden führt zu Flucht oder Gegenwehr statt zum Umfallen.
  Tödlicher Schaden setzt weiterhin den Todeszustand.
- Über verletzten lebenden NPCs erscheint ein Lebensbalken: grün, gelb oder rot
  nach verbleibendem Leben. Volle Gesundheit und tote Figuren erhalten keinen Balken.
- Fahrzeugkontakt oberhalb der bisherigen Schadensschwelle verursacht Schaden
  proportional zum Tempo. 1,1 s Kontakt-Sperrzeit vermeidet mehrfachen Schaden
  in aufeinanderfolgenden Simulationsbildern desselben Kontakts.

## Prüfstatus und Grenzen

Die Angaben sind Ergebnisse der vorangegangenen Implementierung, kein neuer
Testlauf allein für diese Dokumentationsänderung:

- **33 gezielte Tests bestanden:** Beschleunigung, Fahrdynamik, Fahrzeugbasis,
  Soundscape. Darunter alle 45 Modelle und Kurvenstarts aller 37 Pkw bei trockenem,
  nassem und verschneitem Untergrund, verschiedene Zeitschritte und Teillast.
- **Vollständiger Lauf:** 461 Tests, 444 bestanden, 17 fehlgeschlagen. Dieser Lauf
  lag vor Ergänzung des letzten zusätzlichen Kurvenstart-Tests.
- **Kontrolllauf mit alter Fahrphysik:** sämtliche 17 Fehler reproduziert;
  zusätzlich schwankte ein Verkehrs-/Rotlichttest. Aus dem Vergleich ergibt sich
  keine neue Regression durch die Beschleunigungsänderung.
- Bekannte Fehler betreffen alte Erwartungen an Maus-Einstieg und Bahnhofsbetreten,
  NPC-Stürze, Klanglautstärke, Musclecar-Fahrhilfen sowie Karten-/Verkehrsszenarien.
  Die vollständige Suite ist nicht grün; betroffene Tests und Implementierungen
  müssen bei der nächsten gezielten Bereinigung einzeln abgeglichen werden.
- Der anschließend ergänzte Wochentag-Befehl hat noch keinen eigenen Testlauf.
- Das Spiel wurde nach Serverneustart und Browser-Neuladen sichtbar gestartet.
  Ein manueller Vergleich der neuen Beschleunigung und ein abschließender Hörtest
  sind damit nicht nachgewiesen.

### Nützliche Befehle

```sh
npm start
node tools/benchmark-vehicles.mjs
node tools/benchmark-vehicles.mjs musclecar kompakt hothatch
node --test tests/acceleration.test.js tests/dynamics.test.js tests/car.test.js tests/soundscape.test.js
npm test
```

Das Benchmarkwerkzeug gibt JSON aus. Die Messung ist deterministisch, ohne
Kartenkollisionen, mit Vollgas und aktivierten Fahrhilfen. Sie ersetzt keine Fahrt
auf einer konkreten Straße mit Verkehr, Kurven, Wetter oder abweichender Eingabe.
