#!/usr/bin/env python3
"""Analyse der Motor-Referenzaufnahme (Phase 1 des Motorsound-Auftrags).

Erzeugt in tools/audio/analysis/:
  basis.json        Länge, Samplerate, Kanäle, Lautheit (LUFS integriert), Sample-/True-Peak
  waveform.png      Wellenform + RMS-Hüllkurve
  spectrogram.png   Spektrogramm (log-Frequenz) über die ganze Länge
  pitch.png         Grundfrequenz (pyin und Obertonkamm) und daraus geschätzte Drehzahl (Annahmen A und B)
  frames.csv        je 21 ms: Zeit, RMS (dBFS), f0 pyin, Stimmhaftigkeit, Schwerpunkt, f0 Kamm, Drehzahl A/B
  transients.json   Zeitpunkte und Stärke der Knackser (Schaltvorgänge, Pops)
  segments.md       Rohtabelle je 250 ms (die kuratierte Segmenttabelle steht in SEGMENTE.md)

Drehzahl-Annahmen (Viertakt-V10, Vorbild des Fahrzeugs supercar_awd):
  A  Grundton = Zündfrequenz          Drehzahl = f0 × 60 × 2 / 10 = f0 × 12
  B  Grundton = halbe Zündfrequenz    Drehzahl = f0 × 24  (Ordnung 2,5 – eine Zylinderbank je Auspuffstrang)
  Nur B ergibt einen plausiblen Verlauf (Leerlauf ~1300, Schaltpunkt ~7000, n_max 8500); A läge bei 3500 1/min.

Aufruf: python tools/audio/analyze_reference.py [QUELLE [AUSGABEORDNER]]
        (Vorgabe: die V10-Referenz nach tools/audio/analysis/)
Braucht ffmpeg (Dekodieren) und librosa, numpy, soundfile, matplotlib, pyloudnorm.
Die Quelldatei wird nur gelesen.
"""
import json
import subprocess
import sys
import tempfile
from pathlib import Path

import librosa
import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np
import pyloudnorm
import soundfile as sf

ROOT = Path(__file__).resolve().parents[2]
SRC = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "data/audio/raw/engine_reference_lambo.m4a"
OUT = Path(sys.argv[2]) if len(sys.argv) > 2 else ROOT / "tools/audio/analysis"
HOP = 1024  # ~21 ms bei 48 kHz
FMIN, FMAX = 30.0, 1200.0


def decode(src: Path) -> tuple[np.ndarray, int]:
    with tempfile.TemporaryDirectory() as d:
        wav = Path(d) / "ref.wav"
        subprocess.run(
            ["ffmpeg", "-v", "error", "-y", "-i", str(src), "-c:a", "pcm_f32le", str(wav)], check=True
        )
        y, sr = sf.read(wav, always_2d=True)
    return y.T.astype(np.float32), sr


def true_peak_db(src: Path) -> float | None:
    r = subprocess.run(
        ["ffmpeg", "-hide_banner", "-i", str(src), "-af", "ebur128=peak=true", "-f", "null", "-"],
        capture_output=True,
        text=True,
    )
    lines = r.stderr.splitlines()
    for i, l in enumerate(lines):
        if "True peak:" in l:
            for nxt in lines[i + 1 : i + 3]:
                if "Peak:" in nxt:
                    return float(nxt.split()[1])
    return None


def tick_step(dur: float) -> float:
    """Achsenteilung: etwa 20 Marken über die Länge (0,5 s bei kurzen, 10 s bei langen Aufnahmen)."""
    for step in (0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 30.0):
        if dur / step <= 24:
            return step
    return 60.0


def rpm(f0: np.ndarray, cyl: int) -> np.ndarray:
    # Viertakt: Zündfrequenz = Drehzahl/60 × Zylinder/2  →  Drehzahl = f0 × 120 / Zylinder
    return f0 * 120.0 / cyl


def comb_pitch(mono: np.ndarray, sr: int) -> tuple[np.ndarray, np.ndarray]:
    """Grundton je Rahmen über einen Obertonkamm: die Frequenz, deren erste acht Vielfache zusammen die meiste
    (logarithmische) Energie tragen. Robuster als pyin bei rauem, geräuschhaftem Motorklang (pyin springt dort
    oft eine Oktave nach unten)."""
    n_fft = 16384
    X = np.abs(librosa.stft(mono, n_fft=n_fft, hop_length=HOP))
    freqs = np.fft.rfftfreq(n_fft, 1 / sr)
    cand = np.arange(50.0, 450.0, 0.5)
    idx = [np.clip(np.searchsorted(freqs, cand * k), 0, len(freqs) - 1) for k in range(1, 9)]
    f0 = np.empty(X.shape[1])
    conf = np.empty(X.shape[1])
    for j in range(X.shape[1]):
        col = np.log1p(X[:, j] * 100)
        s = sum(col[ix] / k**0.3 for k, ix in enumerate(idx, 1))
        i = int(np.argmax(s))
        f0[j] = cand[i]
        conf[j] = (s[i] - np.median(s)) / (np.std(s) + 1e-9)
    return f0, conf


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    y2, sr = decode(SRC)
    mono = y2.mean(axis=0)
    dur = len(mono) / sr
    meter = pyloudnorm.Meter(sr)
    lufs = meter.integrated_loudness(y2.T)
    basis = {
        "datei": str(SRC.relative_to(ROOT)) if SRC.is_relative_to(ROOT) else str(SRC),
        "laenge_s": round(dur, 3),
        "samplerate": sr,
        "kanaele": int(y2.shape[0]),
        "lufs_integriert": round(float(lufs), 2),
        "sample_peak_dbfs": round(float(20 * np.log10(np.abs(y2).max() + 1e-12)), 2),
        "true_peak_dbtp": true_peak_db(SRC),
        "stereo_korrelation": round(float(np.corrcoef(y2[0], y2[1])[0, 1]), 3) if y2.shape[0] == 2 else None,
    }

    # Merkmale je Rahmen
    rms = librosa.feature.rms(y=mono, frame_length=2048, hop_length=HOP)[0]
    rms_db = 20 * np.log10(rms + 1e-9)
    cent = librosa.feature.spectral_centroid(y=mono, sr=sr, n_fft=4096, hop_length=HOP)[0]
    f0, voiced, vprob = librosa.pyin(
        mono, fmin=FMIN, fmax=FMAX, sr=sr, frame_length=4096, hop_length=HOP
    )
    t = librosa.frames_to_time(np.arange(len(f0)), sr=sr, hop_length=HOP)
    kf0, kconf = comb_pitch(mono, sr)
    n = min(len(t), len(rms_db), len(cent), len(kf0))
    t, f0, voiced, vprob, rms_db, cent = t[:n], f0[:n], voiced[:n], vprob[:n], rms_db[:n], cent[:n]
    kf0, kconf = kf0[:n], kconf[:n]
    # unsichere Kammwerte (Pausen, Knackser) nicht als Ton werten
    kf0 = np.where((kconf > 2.6) & (rms_db > -30), kf0, np.nan)

    on = librosa.onset.onset_strength(y=mono, sr=sr, hop_length=512)
    ot = librosa.frames_to_time(np.arange(len(on)), sr=sr, hop_length=512)
    pk = librosa.util.peak_pick(on, pre_max=8, post_max=8, pre_avg=20, post_avg=20, delta=1.5, wait=10)
    (OUT / "transients.json").write_text(
        json.dumps([{"t_s": round(float(ot[p]), 3), "staerke": round(float(on[p]), 2)} for p in pk], indent=2)
        + "\n"
    )

    with open(OUT / "frames.csv", "w") as f:
        f.write("t_s,rms_dbfs,f0_pyin_hz,voiced,voiced_prob,centroid_hz,f0_kamm_hz,rpm_a,rpm_b\n")
        for i in range(n):
            ff, kk = f0[i], kf0[i]
            fmt = lambda v: "" if np.isnan(v) else f"{v:.1f}"
            f.write(
                f"{t[i]:.3f},{rms_db[i]:.2f},{fmt(ff)},{int(voiced[i])},{vprob[i]:.2f},{cent[i]:.0f},"
                f"{fmt(kk)},{'' if np.isnan(kk) else f'{kk * 12:.0f}'},{'' if np.isnan(kk) else f'{kk * 24:.0f}'}\n"
            )

    # Wellenform
    fig, ax = plt.subplots(figsize=(14, 4))
    tt = np.arange(len(mono)) / sr
    ax.plot(tt, y2[0], lw=0.3, color="#4a7bd0", label="links")
    if y2.shape[0] > 1:
        ax.plot(tt, y2[1], lw=0.3, color="#d06a4a", alpha=0.6, label="rechts")
    ax2 = ax.twinx()
    ax2.plot(t, rms_db, color="k", lw=1.2, label="RMS (dBFS)")
    ax2.set_ylabel("RMS dBFS")
    ax.set_xlabel("Zeit (s)")
    ax.set_ylabel("Amplitude")
    ax.set_xlim(0, dur)
    ax.set_xticks(np.arange(0, dur + 0.01, tick_step(dur)))
    ax.grid(alpha=0.3)
    ax.set_title("Wellenform und RMS-Hüllkurve")
    fig.tight_layout()
    fig.savefig(OUT / "waveform.png", dpi=110)
    plt.close(fig)

    # Spektrogramm
    S = librosa.amplitude_to_db(np.abs(librosa.stft(mono, n_fft=4096, hop_length=256)), ref=np.max)
    fig, ax = plt.subplots(figsize=(14, 6))
    img = librosa.display.specshow(S, sr=sr, hop_length=256, x_axis="time", y_axis="log", ax=ax, cmap="magma")
    ax.plot(t, f0, color="cyan", lw=1.2, label="f0 (pyin)")
    ax.set_ylim(25, 16000)
    ax.set_xticks(np.arange(0, dur + 0.01, tick_step(dur)))
    ax.legend(loc="upper right")
    fig.colorbar(img, ax=ax, format="%+2.0f dB")
    ax.set_title("Spektrogramm (log-Frequenz) mit Grundfrequenz")
    fig.tight_layout()
    fig.savefig(OUT / "spectrogram.png", dpi=110)
    plt.close(fig)

    # Tonhöhe und Drehzahl
    fig, (a1, a2) = plt.subplots(2, 1, figsize=(14, 7), sharex=True)
    a1.plot(t, f0, ".", ms=3, color="#bbb", label="pyin (springt oft eine Oktave)")
    a1.plot(t, kf0, ".", ms=4, color="#2a6", label="Obertonkamm")
    a1.set_ylabel("Grundton (Hz)")
    a1.grid(alpha=0.3)
    a1.legend()
    a1.set_title("Grundton")
    a2.plot(t, kf0 * 24, ".", ms=4, color="#c33", label="B: V10, Grundton = halbe Zündfrequenz (× 24)")
    a2.plot(t, kf0 * 12, ".", ms=3, color="#36c", label="A: V10, Grundton = Zündfrequenz (× 12)")
    a2.plot(t, kf0 * 10, ".", ms=2, color="#999", label="V12, Grundton = Zündfrequenz (× 10)")
    a2.axhline(8500, color="k", lw=0.8, ls="--")
    a2.text(0.05, 8650, "n_max supercar_awd 8500", fontsize=8)
    a2.set_ylabel("Drehzahl (1/min)")
    a2.set_xlabel("Zeit (s)")
    a2.set_xticks(np.arange(0, dur + 0.01, tick_step(dur)))
    a2.grid(alpha=0.3)
    a2.legend(loc="upper left")
    for tr in json.loads((OUT / "transients.json").read_text()):
        for a in (a1, a2):
            a.axvline(tr["t_s"], color="orange", lw=0.8, alpha=0.7)
    fig.tight_layout()
    fig.savefig(OUT / "pitch.png", dpi=110)
    plt.close(fig)

    # grobe Segmentvorschläge: f0-Steigung und Pegel je 250 ms
    win = max(1, int(0.25 * sr / HOP))
    rows = []
    for s in range(0, n, win):
        sl = slice(s, min(n, s + win))
        ff = kf0[sl]
        ok = ~np.isnan(ff)
        med = float(np.nanmedian(ff)) if ok.any() else float("nan")
        slope = 0.0
        if ok.sum() >= 3:
            slope = float(np.polyfit(t[sl][ok], ff[ok], 1)[0])
        rows.append((t[sl][0], t[sl][-1], med, slope, float(np.mean(rms_db[sl])), float(np.mean(vprob[sl]))))
    with open(OUT / "segments.md", "w") as f:
        f.write("| von s | bis s | Grundton Hz (Kamm) | Steigung Hz/s | RMS dBFS | pyin stimmhaft p |\n|---|---|---|---|---|---|\n")
        for r in rows:
            f.write(f"| {r[0]:.2f} | {r[1]:.2f} | {r[2]:.0f} | {r[3]:+.0f} | {r[4]:.1f} | {r[5]:.2f} |\n")

    (OUT / "basis.json").write_text(json.dumps(basis, indent=2, ensure_ascii=False) + "\n")
    print(json.dumps(basis, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    main()
