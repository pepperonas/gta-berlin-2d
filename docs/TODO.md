# TODO

Offene Aufgaben, die entschieden, aber noch nicht umgesetzt sind. Erledigtes fliegt raus (Verlauf: CHANGELOG).

## Motorsound: fehlende Bänke (entschieden am 05.10.2026)

Für diese Motoren fand sich keine frei lizenzierte Aufnahme mit gehaltenem Drehzahlbereich (Begründung und
verworfene Kandidaten: [`audio.md`](audio.md), „Was (noch) Synthese bleibt“). Entscheidung:

- [ ] **V8** (Muscle-Cars, `v8_klassik`, Klasse `muscle`): Martin liefert eine Aufnahme – am besten Prüfstand oder
      Hochdrehen im Stand, ohne Publikum/Ansager –, daraus eine eigene Bank wie beim V12 (`build_engine_sounds.py`,
      Analyse mit `analyze_reference.py`, Segmente in einer `SEGMENTE.md`).
- [ ] **Motorrad** (Klasse `zweirad_motor`, Superbike/Naked): ebenso – Aufnahme von Martin, eigene Bank.
- [x] **Zweitakter** (Trabant, Roller) und **luftgekühlter Boxer** (Käfer): bleiben Synthese (`zuordnung.typen` → `null`).
- [ ] Offen, nicht entschieden: **Sechszylinder-Limousinen** (Klasse `pkw_oberklasse`, auch das Spielerauto) – bisher
      Synthese; gleicher Weg möglich, wenn eine Aufnahme vorliegt.

## Xbox

- [ ] Teststrategie: siehe [`NATIVE-RUST.md`](NATIVE-RUST.md), „TODO: Xbox-Teststrategie“.
