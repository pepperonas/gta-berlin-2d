#!/usr/bin/env python3
"""HUD-Schrift des HD-Modus als SDF-Atlas (Grafik-Überarbeitung Phase 9).

    python3 tools/gfx/build_font.py            # lädt Inter (falls nötig) und baut den Atlas
    python3 tools/gfx/build_font.py --offline  # nur aus tools/gfx/.cache

Quelle: Inter 4.1 (Rasmus Andersson, SIL Open Font License 1.1), Schnitt SemiBold – kräftig genug über der Karte.
Ergebnis unter data/gfx/font/ (nicht von Hand ändern):
  hud_sdf.png   – ein Kanal, Abstandsfeld: 128 = Kante, heller = innen; `spread` Atlas-Pixel bis 0 bzw. 255.
  hud_sdf.json  – je Zeichen Atlas-Rechteck und Metrik (in em), dazu Zeichen, die der Schrift fehlen (das Spiel nimmt
                  dafür das Bitmap-Zeichen von font8x8).
  manifest.json – Quelle, Lizenz, SHA-256 des Pakets.  OFL.txt – Lizenztext.
Das Feld entsteht ohne scipy: Glyphe 4× überabgetastet, je Atlaspixel der Abstand zur nächsten Kante (numpy).
"""
import hashlib
import io
import json
import sys
import urllib.request
import zipfile
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "data" / "gfx" / "font"
CACHE = Path(__file__).resolve().parent / ".cache"
URL = "https://github.com/rsms/inter/releases/download/v4.1/Inter-4.1.zip"
SHA = "9883fdd4a49d4fb66bd8177ba6625ef9a64aa45899767dde3d36aa425756b11e"
TTF = "extras/ttf/Inter-SemiBold.ttf"
EM = 48          # Atlas-Pixel je em
SPREAD = 6       # Atlas-Pixel von der Kante bis zum vollen Wert
SS = 4           # Überabtastung beim Rastern
WIDTH = 1024

# Zeichensatz wie der Bitmap-HUD: ASCII, Latin-1, typografische Zeichen und die Eigenzeichen aus hud.rs
CHARS = [chr(c) for c in range(32, 127)] + [chr(c) for c in range(0xA0, 0x100)] + list(
    "€→✓▣⚠☀☾★▲▼↑↓–—−„“”‚‘’…‹›←"
)


def font_bytes(offline: bool) -> tuple[bytes, bytes, str]:
    CACHE.mkdir(exist_ok=True)
    path = CACHE / "inter.zip"
    if not path.exists():
        if offline:
            sys.exit(f"{path} fehlt (ohne --offline laden)")
        req = urllib.request.Request(URL, headers={"User-Agent": "gta-berlin-build/1.0"})
        with urllib.request.urlopen(req, timeout=120) as r:
            path.write_bytes(r.read())
    data = path.read_bytes()
    sha = hashlib.sha256(data).hexdigest()
    if sha != SHA and "--neu" not in sys.argv:
        sys.exit(f"Inter-Paket hat sich geändert ({sha[:12]}); mit --neu bewusst übernehmen")
    z = zipfile.ZipFile(io.BytesIO(data))
    return z.read(TTF), z.read("LICENSE.txt"), sha


def raster(font: ImageFont.FreeTypeFont, ch: str):
    """Maske der Glyphe (SS-fach), Ursprung links auf der Grundlinie; None bei Leerzeichen."""
    l, t, r, b = font.getbbox(ch, anchor="ls")
    if r <= l or b <= t:
        return None
    pad = SPREAD * SS
    w, h = r - l + 2 * pad, b - t + 2 * pad
    img = Image.new("L", (w, h), 0)
    ImageDraw.Draw(img).text((pad - l, pad - t), ch, font=font, fill=255, anchor="ls")
    return np.asarray(img) >= 128, (l - pad, t - pad)


def sdf(mask: np.ndarray) -> np.ndarray:
    """Abstandsfeld in Atlas-Auflösung (je SS × SS Block ein Wert), 0…255, 128 = Kante."""
    inside = mask
    edge = np.zeros_like(inside)
    edge[:-1] |= inside[:-1] != inside[1:]
    edge[1:] |= inside[:-1] != inside[1:]
    edge[:, :-1] |= inside[:, :-1] != inside[:, 1:]
    edge[:, 1:] |= inside[:, :-1] != inside[:, 1:]
    ey, ex = np.nonzero(edge)
    pts = np.stack([ex + 0.5, ey + 0.5], 1) / SS
    h, w = inside.shape[0] // SS, inside.shape[1] // SS
    gy, gx = np.mgrid[0:h, 0:w]
    centers = np.stack([gx.ravel() + 0.5, gy.ravel() + 0.5], 1)
    d = np.full(len(centers), float(SPREAD))
    for i in range(0, len(centers), 512):
        c = centers[i : i + 512]
        dd = np.sqrt(((c[:, None, :] - pts[None, :, :]) ** 2).sum(-1)).min(1)
        d[i : i + 512] = np.minimum(dd, SPREAD)
    sample = inside[(gy.ravel() * SS + SS // 2), (gx.ravel() * SS + SS // 2)]
    signed = np.where(sample, d, -d)
    return np.clip(np.round(128 + signed / SPREAD * 127), 0, 255).astype(np.uint8).reshape(h, w)


def main() -> None:
    offline = "--offline" in sys.argv
    ttf, lic, sha = font_bytes(offline)
    font = ImageFont.truetype(io.BytesIO(ttf), EM * SS)
    def shape(ch: str):
        m = font.getmask(ch)
        return m.size, bytes(m)

    notdef = shape(chr(0x10FFFD))
    glyphs, missing, cells = {}, [], []
    for ch in CHARS:
        if ch != " " and shape(ch) == notdef:
            missing.append(ch)
            continue
        adv = font.getlength(ch) / SS / EM
        r = raster(font, ch)
        if r is None:
            glyphs[str(ord(ch))] = [0, 0, 0, 0, 0.0, 0.0, round(adv, 4)]
            continue
        mask, (ox, oy) = r
        # Maske auf ein Vielfaches von SS bringen
        h, w = mask.shape
        mask = np.pad(mask, ((0, (-h) % SS), (0, (-w) % SS)))
        cells.append((ch, sdf(mask), ox / SS / EM, oy / SS / EM, adv))
    # Regal-Packen, höchste Glyphen zuerst
    cells.sort(key=lambda c: -c[1].shape[0])
    x = y = row = 0
    placed = []
    for ch, img, bx, by, adv in cells:
        h, w = img.shape
        if x + w > WIDTH:
            x, y, row = 0, y + row + 1, 0
        placed.append((ch, img, x, y, bx, by, adv))
        x += w + 1
        row = max(row, h)
    height = 1 << max(7, int(np.ceil(np.log2(y + row + 1))))
    atlas = np.zeros((height, WIDTH), np.uint8)
    for ch, img, px, py, bx, by, adv in placed:
        h, w = img.shape
        atlas[py : py + h, px : px + w] = img
        glyphs[str(ord(ch))] = [px, py, w, h, round(bx, 4), round(by, 4), round(adv, 4)]
    OUT.mkdir(parents=True, exist_ok=True)
    Image.fromarray(atlas, "L").save(OUT / "hud_sdf.png", optimize=True)
    asc, desc = font.getmetrics()
    meta = {
        "_hinweis": "Erzeugt von tools/gfx/build_font.py – nicht von Hand ändern.",
        "em": EM,
        "spread": SPREAD,
        "atlas": [WIDTH, height],
        "ascender": round(asc / SS / EM, 4),
        "descender": round(desc / SS / EM, 4),
        "cap": round(-font.getbbox("H", anchor="ls")[1] / SS / EM, 4),
        "fehlend": "".join(missing),
        # [x, y, w, h (Atlas-Pixel), links, oben (em, vom Ursprung auf der Grundlinie, y nach unten), Vorschub (em)]
        "zeichen": glyphs,
    }
    (OUT / "hud_sdf.json").write_text(json.dumps(meta, ensure_ascii=False, separators=(",", ":")) + "\n")
    (OUT / "OFL.txt").write_bytes(lic)
    manifest = {
        "_hinweis": "Erzeugt von tools/gfx/build_font.py – nicht von Hand ändern.",
        "schrift": {
            "name": "Inter SemiBold",
            "version": "4.1",
            "urheber": "Rasmus Andersson (The Inter Project Authors)",
            "lizenz": "SIL Open Font License 1.1",
            "seite": "https://rsms.me/inter/",
            "quelle": URL,
            "sha256": sha,
        },
    }
    (OUT / "manifest.json").write_text(json.dumps(manifest, indent=1, ensure_ascii=False) + "\n")
    print(f"{len(glyphs)} Zeichen, Atlas {WIDTH}×{height}, fehlend: {''.join(missing)!r}")


if __name__ == "__main__":
    main()
