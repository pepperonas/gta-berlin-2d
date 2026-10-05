#!/usr/bin/env python3
"""Baut aus der Referenzaufnahme die Motor-Samples des Spiels (Phase 2 des Motorsound-Auftrags).

Eingang : data/audio/raw/engine_reference_lambo.m4a (wird nur gelesen)
Ausgang : data/audio/engine/v10/  – Loops (Last/Schub), Einzelklänge, Referenz, manifest.json

Aufruf  : python tools/audio/build_engine_sounds.py
Braucht : ffmpeg, Pakete aus tools/audio/requirements.txt

Ablauf je Loop (siehe docs/audio.md):
  1. Grundton über einen Obertonkamm verfolgen (wie analyze_reference.py), geglättet.
  2. „Sweep glätten“: Das Quellfenster wird zeitvariabel nachgetastet, sodass die Tonhöhe auf dem Mittelwert des
     Fensters stillsteht (ds/dn = f_ziel / f(s)). Ohne das würde ein Ausschnitt aus einem Hochdrehen als Loop jaulen.
  3. Pegelschwankungen innerhalb des Fensters ausgleichen (±4 dB höchstens).
  4. Länge = ganze Zahl von Grundtonperioden; Loop-Punkt mit gleichstarkem Crossfade (25 ms) über das, was hinter
     dem Ende kommt – der Übergang Ende → Anfang ist damit lückenlos, ohne Klick.
  5. Lautheit aller Loops auf einen Zielwert (über den mehrfach wiederholten Loop gemessen).
Schub-Loops (Gas weg) sind abgeleitet: Die Aufnahme enthält keinen Schub (das Auto fährt weg), siehe SEGMENTE.md.

Idempotent: Gleicher Eingang ergibt bitgleiche Dateien (kein Zufall, feste Parameter, 16-Bit-PCM-WAV).
"""
import hashlib
import json
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np
import pyloudnorm
import soundfile as sf
from scipy import signal

ROOT = Path(__file__).resolve().parents[2]
SRC = ROOT / "data/audio/raw/engine_reference_lambo.m4a"
OUT = ROOT / "data/audio/engine/v10"
SR = 48000
C = 343.0  # Schallgeschwindigkeit m/s

# Drehzahl-Annahme B (SEGMENTE.md): V10, Grundton = halbe Zündfrequenz → Drehzahl = f0 × 24
RPM_PER_HZ = 24.0
LOOP_LUFS = -18.0
XFADE_S = 0.025

# Last-Loops: Name, Quellfenster (s), Notch-Filter (Hz) gegen den Fremdton am Anfang, Sweep glätten?
# Der Leerlauf wird nicht geglättet: dort ist die Tonhöhe ohnehin fast konstant, die Verfolgung aber unsicher.
LOOPS = [
    ("idle", 0.35, 0.95, [236.9, 453.8], False),
    ("r2700", 1.35, 1.75, [], True),
    ("r3300", 1.80, 2.32, [], True),
    ("r4900", 2.64, 2.88, [], True),
    ("r6000", 2.98, 3.22, [], True),
    ("r7200", 3.30, 3.64, [], True),
]
# Einzelklänge: Name, Art, von, bis (s), Ein-/Ausblendung (s), Zielpegel
ONESHOTS = [
    ("start", "start", 0.105, 0.95, (0.002, 0.25), ("lufs", -16.0)),
    ("blip", "blip", 1.05, 1.70, (0.01, 0.25), ("lufs", -16.0)),
    ("shift_1", "schalten", 3.70, 4.06, (0.005, 0.08), ("lufs", -20.0)),
    ("shift_2", "schalten", 5.30, 5.52, (0.005, 0.06), ("lufs", -20.0)),
    ("shift_3", "schalten", 7.09, 7.27, (0.005, 0.05), ("lufs", -20.0)),
    ("pop_1", "pop", 7.845, 7.965, (0.002, 0.05), ("peak", -3.0)),
    ("pop_2", "pop", 8.015, 8.135, (0.002, 0.05), ("peak", -3.0)),
    ("pop_3", "pop", 8.215, 8.335, (0.002, 0.05), ("peak", -3.0)),
    ("pop_4", "pop", 3.745, 3.865, (0.002, 0.05), ("peak", -3.0)),
    ("pop_5", "pop", 3.88, 4.0, (0.002, 0.05), ("peak", -3.0)),
]


def decode(src: Path) -> np.ndarray:
    with tempfile.TemporaryDirectory() as d:
        wav = Path(d) / "ref.wav"
        subprocess.run(
            ["ffmpeg", "-v", "error", "-y", "-i", str(src), "-ac", "1", "-ar", str(SR), "-c:a", "pcm_f32le", str(wav)],
            check=True,
        )
        y, sr = sf.read(wav, dtype="float64")
    assert sr == SR
    return y


def doppler(t: float) -> float:
    """Faktor, um den das wegfahrende Auto tiefer klingt (c/(c+v)), aus einer geschätzten Geschwindigkeit:
    Start bei 1,05 s, 9,5 m/s² bis 3,7 s, danach 5 m/s². Nur für die Drehzahl-Beschriftung, nicht für den Klang."""
    if t <= 1.05:
        v = 0.0
    elif t <= 3.7:
        v = 9.5 * (t - 1.05)
    else:
        v = 9.5 * 2.65 + 5.0 * (t - 3.7)
    return C / (C + v)


def comb_track(y: np.ndarray, hop: int = 240, n_fft: int = 16384) -> tuple[np.ndarray, np.ndarray]:
    """Grundton je Rahmen über einen Obertonkamm (50–450 Hz, acht Vielfache), Zeitachse in Sekunden."""
    win = np.hanning(n_fft)
    pad = np.pad(y, (n_fft // 2, n_fft // 2))
    freqs = np.fft.rfftfreq(n_fft, 1 / SR)
    cand = np.arange(50.0, 450.0, 0.25)
    idx = [np.clip(np.searchsorted(freqs, cand * k), 0, len(freqs) - 1) for k in range(1, 9)]
    frames = range(0, len(y), hop)
    f0 = np.empty(len(frames))
    for j, s in enumerate(frames):
        col = np.log1p(np.abs(np.fft.rfft(pad[s : s + n_fft] * win)) * 100)
        sc = sum(col[ix] / k**0.3 for k, ix in enumerate(idx, 1))
        f0[j] = cand[int(np.argmax(sc))]
    t = np.arange(len(f0)) * hop / SR
    return t, f0


def smooth_track(t: np.ndarray, f0: np.ndarray, t0: float, t1: float, sweep: bool) -> np.ndarray:
    """Grundton im Fenster als Polynom 2. Grades (Sweeps sind glatt): Median gegen Oktavsprünge, Punkte mehr als
    8 % neben der ersten Anpassung verworfen, neu angepasst. Ohne Sweep: konstanter Median."""
    m = (t >= t0) & (t <= t1)
    tt, ff = t[m], signal.medfilt(f0[m], 15)
    if not sweep:
        return np.array([0.0, 0.0, float(np.median(ff))])
    coef = np.polyfit(tt, ff, 2)
    ok = np.abs(ff / np.polyval(coef, tt) - 1) < 0.08
    return np.polyfit(tt[ok], ff[ok], 2)


def flatten(y: np.ndarray, coef: np.ndarray, t0: float, n_out: int, f_target: float) -> np.ndarray:
    """Zeitvariables Nachtasten: Ausgabe hat konstanten Grundton f_target."""
    s = np.empty(n_out)
    pos = t0 * SR
    for n in range(n_out):
        s[n] = pos
        f = np.polyval(coef, pos / SR)
        pos += f_target / f
    return np.interp(s, np.arange(len(y)), y)


def level_flatten(x: np.ndarray, period: float) -> np.ndarray:
    """Langsame Pegelschwankungen ausgleichen (Hüllkurve über vier Perioden, höchstens ±4 dB)."""
    n = max(16, int(period * 4))
    env = np.sqrt(np.convolve(x**2, np.ones(n) / n, mode="same")) + 1e-9
    env = np.convolve(env, np.ones(n) / n, mode="same")
    g = np.clip(env.mean() / env, 10 ** (-4 / 20), 10 ** (4 / 20))
    return x * g


def loudness(x: np.ndarray, tile: bool) -> float:
    meter = pyloudnorm.Meter(SR)
    z = np.tile(x, int(np.ceil(4 * SR / len(x)))) if tile else np.pad(x, (0, max(0, SR // 2 - len(x))))
    return float(meter.integrated_loudness(z))


def seam_ratio(x: np.ndarray) -> float:
    """Sprung am Loop-Punkt im Verhältnis zum größten Schritt in seiner Umgebung (±64 Samples). ≤ 1 heißt: der
    Übergang Ende → Anfang ist nicht steiler als das Signal dort ohnehin – kein Klick."""
    ring = np.concatenate([x[-64:], x[:64]])
    local = np.abs(np.diff(ring))
    local = np.delete(local, 63)  # der Nahtschritt selbst
    return float(abs(x[0] - x[-1]) / (local.max() + 1e-12))


def residual_cents(x: np.ndarray, f_target: float) -> float:
    """Tonhöhendrift im fertigen Loop (Kamm über den zweimal wiederholten Loop)."""
    t, f = comb_track(np.tile(x, 2), hop=480, n_fft=8192)
    f = f[(t > 0.05) & (t < len(x) * 2 / SR - 0.05)]
    if len(f) < 3:
        return 0.0
    return float(1200 * np.log2(np.percentile(f, 90) / np.percentile(f, 10)))


def circular_filter(x: np.ndarray, b: np.ndarray, a: np.ndarray) -> np.ndarray:
    """Filter über den dreifach wiederholten Loop, Mitte behalten – so bleibt der Loop-Punkt nahtlos."""
    z = signal.lfilter(b, a, np.tile(x, 3))
    return z[len(x) : 2 * len(x)]


def write(path: Path, x: np.ndarray) -> None:
    peak = np.abs(x).max()
    if peak > 0.999:
        raise SystemExit(f"{path.name}: Übersteuerung ({peak:.3f})")
    sf.write(path, x.astype(np.float32), SR, subtype="PCM_16")


def main() -> None:
    if not SRC.exists():
        sys.exit(f"Referenz fehlt: {SRC}")
    OUT.mkdir(parents=True, exist_ok=True)
    for old in OUT.glob("*.wav"):
        old.unlink()
    y = decode(SRC)
    t_tr, f_tr = comb_track(y)
    manifest = {
        "bank": "v10",
        "_hinweis": "Erzeugt von tools/audio/build_engine_sounds.py – nicht von Hand ändern.",
        "quelle": str(SRC.relative_to(ROOT)),
        "quelle_sha256": hashlib.sha256(SRC.read_bytes()).hexdigest(),
        "samplerate": SR,
        "drehzahl_annahme": "V10, Grundton = halbe Zündfrequenz: Drehzahl = f0 × 24, Doppler des wegfahrenden Autos herausgerechnet",
        "loops": [],
        "einzel": [],
        "referenz": "reference.wav",
    }
    report = []

    for name, t0, t1, notches, sweep in LOOPS:
        coef = smooth_track(t_tr, f_tr, t0, t1, sweep)
        tm = (t0 + t1) / 2
        f_target = float(np.polyval(coef, tm))
        period = SR / f_target
        src = y
        if notches:
            src = y.copy()
            for fn in notches:
                b, a = signal.iirnotch(fn, 12.0, SR)
                src = signal.filtfilt(b, a, src)
        # verfügbare Länge in Ausgabesamples (die Tonhöhe ändert die Dauer leicht)
        span = (t1 - t0) * SR * f_target / float(np.mean(np.polyval(coef, np.linspace(t0, t1, 64))))
        xf = int(XFADE_S * SR)
        periods = int((span - xf) // period)
        n_loop = int(round(periods * period))
        flat = flatten(src, coef, t0, n_loop + xf, f_target)
        flat = level_flatten(flat, period)
        # Loop-Punkt: der Kopf wird mit dem Stück hinter dem Ende überblendet (gleiche Leistung)
        u = np.linspace(0, 1, xf, endpoint=False)
        head = flat[:xf] * np.sin(u * np.pi / 2) + flat[n_loop : n_loop + xf] * np.cos(u * np.pi / 2)
        loop = np.concatenate([head, flat[xf:n_loop]])
        # Gleichanteil weg, Lautheit angleichen
        loop -= loop.mean()
        loop *= 10 ** ((LOOP_LUFS - loudness(loop, True)) / 20)
        rpm = f_target * RPM_PER_HZ / doppler(tm)
        drift = residual_cents(loop, f_target) if sweep else float("nan")
        seam = seam_ratio(loop)
        write(OUT / f"on_{name}.wav", loop)
        entry = {
            "datei": f"on_{name}.wav",
            "rpm": round(rpm),
            "last": "on",
            "samples": len(loop),
            "loop_start": 0,
            "loop_ende": len(loop),
            "grundton_hz": round(f_target, 2),
            "quelle_s": [t0, t1],
            "gain_db": 0.0,
            "drift_cent": None if np.isnan(drift) else round(drift, 1),
        }
        manifest["loops"].append(entry)
        report.append((entry["datei"], entry["rpm"], len(loop) / SR, drift, seam))

        # Schub (Gas weg): abgeleitet – Tiefpass, Hochpass gegen Wummern, leichte Sättigung, 7 dB leiser
        b, a = signal.butter(2, 1500 / (SR / 2), "low")
        off = circular_filter(loop, b, a)
        b, a = signal.butter(1, 60 / (SR / 2), "high")
        off = circular_filter(off, b, a)
        off = np.tanh(off * 2.2) / 2.2
        off -= off.mean()
        off *= 10 ** ((LOOP_LUFS - loudness(off, True)) / 20)
        write(OUT / f"off_{name}.wav", off)
        manifest["loops"].append(
            {**entry, "datei": f"off_{name}.wav", "last": "off", "gain_db": -7.0, "drift_cent": entry["drift_cent"]}
        )
        report.append((f"off_{name}.wav", entry["rpm"], len(off) / SR, drift, seam_ratio(off)))

    for name, kind, t0, t1, (fin, fout), (mode, target) in ONESHOTS:
        x = y[int(t0 * SR) : int(t1 * SR)].copy()
        x -= x.mean()
        n_in, n_out = max(1, int(fin * SR)), max(1, int(fout * SR))
        x[:n_in] *= np.linspace(0, 1, n_in)
        x[-n_out:] *= np.linspace(1, 0, n_out) ** 2
        if mode == "lufs":
            x *= 10 ** ((target - loudness(x, False)) / 20)
        else:
            x *= 10 ** (target / 20) / np.abs(x).max()
        write(OUT / f"{name}.wav", x)
        manifest["einzel"].append(
            {
                "datei": f"{name}.wav",
                "art": kind,
                "samples": len(x),
                "quelle_s": [t0, t1],
                "gain_db": 0.0,
                "rpm": round(float(np.median(f_tr[(t_tr >= t0) & (t_tr <= t1)])) * RPM_PER_HZ),
            }
        )

    # Referenz für den A/B-Vergleich (Mono, unverändert im Pegel)
    write(OUT / "reference.wav", y)
    (OUT / "manifest.json").write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + "\n")

    print(f"{'Datei':<16} {'rpm':>6} {'Länge s':>8} {'Drift ct':>9} {'Naht':>6}")
    for f, r, l, d, s in report:
        print(f"{f:<16} {r:>6} {l:>8.3f} {d:>9.1f} {s:>6.2f}")
    bad = [r for r in report if (r[3] == r[3] and r[3] > 35) or r[4] > 1.0]
    if bad:
        sys.exit(f"Qualitätsgrenze verletzt: {bad}")


if __name__ == "__main__":
    main()
