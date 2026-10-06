#!/usr/bin/env python3
"""Feuer-, Explosions- und Rauch-Flipbooks der nativen Fassung aus CC0-Quellen (Unity Labs Paris) bauen.

    python3 tools/gfx/build_vfx.py            # lädt fehlende Pakete, baut den Atlas neu
    python3 tools/gfx/build_vfx.py --offline  # nur aus dem Zwischenspeicher (tools/gfx/.cache)

Quelle: „Free VFX Image Sequences & Flipbooks“ von Thomas Iché (Unity Technologies), in Houdini simuliert, CC0
(https://unity.com/blog/engine-platform/free-vfx-image-sequences-flipbooks). Je Paket liegt ein fertiges Flipbook
(TGA, Bilder zeilenweise von links oben) bei. Das Skript schneidet die Bilder aus, verkleinert sie auf die Zellgröße
des Atlas und legt jede Folge als eigenes Raster in data/gfx/vfx/vfx_atlas.png (2048², RGBA, gerade Deckkraft, sRGB;
wo die Deckkraft 0 ist, wird die Farbe 0 – kleinere Datei, kein Farbsaum beim Filtern).

data/gfx/vfx/manifest.json beschreibt Lizenz, Urheber, SHA-256 der Pakete und das Raster je Folge (Lage, Zellgröße,
Spalten, Bilder, Mischart). `crates/engine/src/vfx.rs` liest es und erzeugt daraus die Shader-Konstanten – Atlas und
Shader können so nicht auseinanderlaufen. Nicht von Hand ändern, Skript erneut laufen lassen.

Mischart: `alpha` = über den Hintergrund gelegt (Feuer darin leuchtet selbst, Rauch nimmt das Umgebungslicht an),
`add` = additiv (Flipbook auf Schwarz ohne Deckkraft, z. B. Feuerkern).
"""
import hashlib
import io
import json
import sys
import urllib.request
import zipfile
from pathlib import Path

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "data" / "gfx" / "vfx"
CACHE = Path(__file__).resolve().parent / ".cache"
ATLAS = 2048
BASE = "https://unity3d.com/files/labs/downloads/vfx/assets01"

# Name im Spiel, Paket, Datei im Paket (Spalten × Zeilen der Vorlage), Zweck, Lage im Atlas (x, y), Zellgröße
# (b, h), Mischart, Leuchtanteil (0 = Rauch, 1 = Feuer leuchtet selbst), erwartete SHA-256 des Pakets.
SEQUENCES = [
    ("explosion_a", "Explosion00", "Explosion00_5x5.tga", (5, 5), "Fahrzeug explodiert (Variante A)",
     (0, 0), (200, 200), "alpha", 1.0, "5931820e3e8f5062446b059bf4b0489a8fc56df5de35170721fe9cd4139d6d42"),
    ("explosion_b", "Explosion01", "Explosion01_5x5.tga", (5, 5), "Fahrzeug explodiert (Variante B)",
     (1024, 0), (200, 200), "alpha", 1.0, "496a212733eb4e93c230f654f1fc1460e8a6389d2f1b13f2b945ec124a375742"),
    ("blast", "Explosion02", "Explosion02_5x5.tga", (5, 5), "Handgranate: kompakter Blitz, dann Rauch",
     (0, 1024), (128, 128), "alpha", 1.0, "1426be2dcfc9008b8b8f0c7d54a241da3f611725aa3c84fc33f919f56036f538"),
    ("fireball", "FireBall03", "FireBall03_8x8.tga", (8, 8), "Glutkern eines Brandes (Schleife)",
     (640, 1024), (48, 48), "add", 1.0, "2128c99e763974c83a64d590389398b078aecd5a088287409644fa4a65f3f3c3"),
    ("flame", "Flame02", "Flame02_16x4.tga", (16, 4), "Flammenzunge, groß (Schleife)",
     (1024, 1024), (64, 128), "alpha", 1.0, "f37b12cdae090591d537ed3bb50396f03833a49864362bf665d6817b7282c954"),
    ("flame_small", "Flame03", "Flame03_16x4.tga", (16, 4), "Flammenzunge, klein (Schleife)",
     (1536, 1536), (32, 64), "alpha", 1.0, "248c2d09c6599ded36085633d4c7fdb9a9a05bdf1a98ab34b8f14d282cef87a9"),
    ("smoke", "WispySmoke01", "WispySmoke01_8x8.tga", (8, 8), "Rauchschwaden (Schleife)",
     (1024, 1536), (64, 64), "alpha", 0.0, "f9bdc88948ff18a7d3e6332b68f2f2ff17e9a64c6feb821e05388f4f23dbcff0"),
]


def fetch(package: str, sha: str, offline: bool) -> bytes:
    CACHE.mkdir(exist_ok=True)
    path = CACHE / f"{package}-flipbooks.zip"
    if not path.exists():
        if offline:
            sys.exit(f"{path} fehlt (ohne --offline laden)")
        url = f"{BASE}/{package}/{package}-flipbooks.zip"
        print(f"lade {url}")
        req = urllib.request.Request(url, headers={"User-Agent": "gta-berlin-build"})
        path.write_bytes(urllib.request.urlopen(req).read())
    data = path.read_bytes()
    got = hashlib.sha256(data).hexdigest()
    if got != sha:
        sys.exit(f"{package}: SHA-256 {got} statt {sha} – Quelle geändert?")
    return data


def frames(sheet: Image.Image, cols: int, rows: int) -> list[Image.Image]:
    w, h = sheet.size
    fw, fh = w // cols, h // rows
    return [sheet.crop((c * fw, r * fh, c * fw + fw, r * fh + fh)) for r in range(rows) for c in range(cols)]


def main() -> None:
    offline = "--offline" in sys.argv
    atlas = np.zeros((ATLAS, ATLAS, 4), dtype=np.uint8)
    taken = np.zeros((ATLAS, ATLAS), dtype=bool)
    seqs = []
    for name, package, file, (scols, srows), purpose, (x0, y0), (cw, ch), mode, glow, sha in SEQUENCES:
        z = zipfile.ZipFile(io.BytesIO(fetch(package, sha, offline)))
        sheet = Image.open(io.BytesIO(z.read(file))).convert("RGBA")
        imgs = frames(sheet, scols, srows)
        n = len(imgs)
        # Raster im Atlas: so quadratisch wie möglich in der zugewiesenen Fläche
        cols = scols if cw * scols <= ATLAS - x0 else max(1, (ATLAS - x0) // cw)
        rows = (n + cols - 1) // cols
        w, h = cols * cw, rows * ch
        if x0 + w > ATLAS or y0 + h > ATLAS:
            sys.exit(f"{name}: passt nicht in den Atlas")
        if taken[y0:y0 + h, x0:x0 + w].any():
            sys.exit(f"{name}: überlappt eine andere Folge")
        taken[y0:y0 + h, x0:x0 + w] = True
        for i, im in enumerate(imgs):
            # vorgemultipliziert verkleinern (sonst färben durchsichtige Pixel die Ränder), dann zurück
            a = np.asarray(im, dtype=np.float32) / 255.0
            pm = np.concatenate([a[..., :3] * a[..., 3:4], a[..., 3:4]], axis=2)
            pm8 = Image.fromarray((pm * 255 + 0.5).astype(np.uint8), mode="RGBA")
            small = np.asarray(pm8.resize((cw, ch), Image.LANCZOS), dtype=np.float32) / 255.0
            al = small[..., 3:4]
            rgb = np.where(al > 1e-3, small[..., :3] / np.maximum(al, 1e-3), 0.0)
            out = np.concatenate([np.clip(rgb, 0, 1), al], axis=2)
            if mode == "add":
                out[..., 3] = 1.0  # additiv: Deckkraft spielt keine Rolle, Schwarz ist durchsichtig
            out = (out * 255 + 0.5).astype(np.uint8)
            if mode != "add":
                out[out[..., 3] == 0] = 0
            cx, cy = x0 + (i % cols) * cw, y0 + (i // cols) * ch
            atlas[cy:cy + ch, cx:cx + cw] = out
        seqs.append({
            "name": name, "zweck": purpose, "paket": package, "datei": file, "sha256": sha,
            "x": x0, "y": y0, "zelle": [cw, ch], "spalten": cols, "bilder": n,
            "mischart": mode, "leuchten": glow,
        })
        print(f"{name:12s} {n:3d} Bilder à {cw}×{ch} bei ({x0}, {y0})")
    OUT.mkdir(parents=True, exist_ok=True)
    Image.fromarray(atlas, mode="RGBA").save(OUT / "vfx_atlas.png", optimize=True)
    manifest = {
        "_hinweis": "Erzeugt von tools/gfx/build_vfx.py – nicht von Hand ändern.",
        "quelle": "Free VFX Image Sequences & Flipbooks (Unity Labs Paris)",
        "seite": "https://unity.com/blog/engine-platform/free-vfx-image-sequences-flipbooks",
        "urheber": "Thomas Iché, Unity Technologies",
        "lizenz": "CC0 1.0",
        "atlas": ATLAS,
        "folgen": seqs,
    }
    (OUT / "manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n")
    print(f"{OUT / 'vfx_atlas.png'}: {(OUT / 'vfx_atlas.png').stat().st_size / 1e6:.1f} MB")


if __name__ == "__main__":
    main()
