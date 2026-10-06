#!/usr/bin/env python3
"""App-Icon von GTA Berlin: abgerundete Kachel mit Abendhimmel, Berliner Dachlinie und dem Fernsehturm in den
Farben des Titel-Logos (Gold → Orange), Kugel mit Glanz und Lichtschein. Gezeichnet mit Pillow in 4-facher Auflösung
und verkleinert (glatte Kanten). Ausgaben (alle erzeugt, nie von Hand ändern):

  data/gfx/icon/icon-1024.png        Vorlage
  data/gfx/icon/icon-128.rgba        rohe RGBA-Bytes 128 × 128 fürs Fenster-/Dock-Icon (winit, ohne PNG-Decoder)
  data/gfx/icon/GtaBerlin.icns       macOS-App (iconutil)
  web/assets/icon.png, icon-180.png  Browser-Favicon und Apple-Touch-Icon
  xbox/GtaBerlin/Assets/*.png        Paketlogos (Kacheln, Store, Splash)

Aufruf: python3 tools/gfx/build_icon.py
"""
import math
import os
import shutil
import subprocess
import tempfile

from PIL import Image, ImageDraw, ImageFilter

ROOT = os.path.normpath(os.path.join(os.path.dirname(__file__), "..", ".."))
SS = 4  # Überabtastung

GOLD = (255, 214, 64)
ORANGE = (255, 122, 26)
NIGHT = (16, 18, 34)
DUSK = (92, 36, 88)
GLOW = (255, 150, 60)


def lerp(a, b, t):
    return tuple(round(a[i] + (b[i] - a[i]) * t) for i in range(len(a)))


def vertical(size, top, bottom, mid=None, mid_at=0.6):
    """Senkrechter Verlauf (optional mit Zwischenfarbe)."""
    w, h = size
    img = Image.new("RGBA", size)
    px = img.load()
    for y in range(h):
        t = y / max(1, h - 1)
        if mid is None:
            c = lerp(top, bottom, t)
        elif t < mid_at:
            c = lerp(top, mid, t / mid_at)
        else:
            c = lerp(mid, bottom, (t - mid_at) / (1 - mid_at))
        for x in range(w):
            px[x, y] = c + (255,)
    return img


def tower(size, s):
    """Fernsehturm (Maske und Farbbild), Höhe nach `s` (px), waagerecht mittig, unten bündig."""
    mask = Image.new("L", size, 0)
    d = ImageDraw.Draw(mask)
    cx = size[0] / 2
    oy = size[1] - s  # bei breiten Flächen sitzt der Turm unten
    ground = oy + s * 0.84
    ball_y, ball_r = oy + s * 0.33, s * 0.105
    # Schaft: unten breiter, zur Kugel schmal
    d.polygon(
        [
            (cx - s * 0.052, ground),
            (cx + s * 0.052, ground),
            (cx + s * 0.026, ball_y),
            (cx - s * 0.026, ball_y),
        ],
        fill=255,
    )
    # Kugel, Telecafé-Ring darunter, Antenne mit rot-weißer Spitze (Spitze extra)
    d.ellipse((cx - ball_r, ball_y - ball_r, cx + ball_r, ball_y + ball_r), fill=255)
    d.rectangle((cx - s * 0.03, ball_y + ball_r * 0.85, cx + s * 0.03, ball_y + ball_r * 1.25), fill=255)
    d.rectangle((cx - s * 0.016, ball_y - ball_r - s * 0.07, cx + s * 0.016, ball_y - ball_r), fill=255)
    d.rectangle((cx - s * 0.008, oy + s * 0.08, cx + s * 0.008, ball_y - ball_r - s * 0.07), fill=255)
    color = vertical(size, GOLD, ORANGE)
    return mask, color, (cx, ball_y, ball_r)


def skyline(size, s, seed=7):
    """Dachlinie: Altbauten mit Dachgauben, ein Hochhaus, Kirchturm – deterministisch."""
    mask = Image.new("L", size, 0)
    d = ImageDraw.Draw(mask)
    w = size[0]
    oy = size[1] - s
    base = oy + s * 0.86
    x = 0.0
    k = seed
    while x < w:
        k = (k * 1103515245 + 12345) & 0x7FFFFFFF
        bw = s * (0.07 + (k % 100) / 100 * 0.08)
        bh = s * (0.07 + ((k >> 8) % 100) / 100 * 0.1)
        d.rectangle((x, base - bh, x + bw, size[1]), fill=255)
        if (k >> 16) % 3 == 0:  # Satteldach
            d.polygon([(x, base - bh), (x + bw / 2, base - bh - s * 0.04), (x + bw, base - bh)], fill=255)
        x += bw - s * 0.004
    # Hochhaus links, Kirchturm rechts (am Turm ausgerichtet)
    ox = (w - s) / 2
    d.rectangle((ox + s * 0.12, base - s * 0.26, ox + s * 0.22, size[1]), fill=255)
    d.rectangle((ox + s * 0.74, base - s * 0.2, ox + s * 0.8, size[1]), fill=255)
    d.polygon(
        [(ox + s * 0.74, base - s * 0.2), (ox + s * 0.77, base - s * 0.31), (ox + s * 0.8, base - s * 0.2)],
        fill=255,
    )
    d.rectangle((0, base, size[0], size[1]), fill=255)
    return mask


def windows(size, s, sky_mask):
    """Erleuchtete Fenster in der Dachlinie (warm, nur wo Gebäude sind)."""
    img = Image.new("RGBA", size, (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    k = 3
    step = s * 0.03
    oy = size[1] - s
    y = oy + s * 0.62
    while y < oy + s * 0.95:
        x = step
        while x < size[0] - step:
            k = (k * 1103515245 + 12345) & 0x7FFFFFFF
            if k % 5 == 0 and sky_mask.getpixel((int(x), int(y))) > 128:
                d.rectangle((x, y, x + s * 0.012, y + s * 0.016), fill=(255, 196, 110, 210))
            x += step
        y += step * 1.4
    return img


def master(px=1024, w=None, rounded=True):
    """Das Icon in `px` Höhe (Breite `w`, sonst quadratisch); `rounded`: abgerundete Kachel, außen transparent."""
    s = px * SS
    size = ((w or px) * SS, s)
    img = vertical(size, NIGHT, DUSK, mid=(40, 30, 70), mid_at=0.55)
    # Lichtschein hinter der Kugel
    tmask, tcolor, (cx, by, br) = tower(size, s)
    glow = Image.new("RGBA", size, (0, 0, 0, 0))
    gd = ImageDraw.Draw(glow)
    for i in range(10, 0, -1):
        r = br * (1 + i * 0.32)
        a = int(16 + (10 - i) * 6)
        gd.ellipse((cx - r, by - r, cx + r, by + r), fill=GLOW + (a,))
    glow = glow.filter(ImageFilter.GaussianBlur(s * 0.03))
    img = Image.alpha_composite(img, glow)
    # Dachlinie (dunkel) mit Fenstern
    sky = skyline(size, s)
    city = Image.new("RGBA", size, (8, 9, 16, 255))
    img.paste(city, (0, 0), sky)
    img = Image.alpha_composite(img, windows(size, s, sky))
    # Turm mit Gold-Orange-Verlauf und dunkler Kante
    edge = tmask.filter(ImageFilter.MaxFilter(int(s * 0.012) | 1))
    img.paste(Image.new("RGBA", size, (30, 14, 6, 255)), (0, 0), edge)
    img.paste(tcolor, (0, 0), tmask)
    # Glanz auf der Kugel, rotes Licht an der Spitze
    hl = Image.new("RGBA", size, (0, 0, 0, 0))
    hd = ImageDraw.Draw(hl)
    hd.ellipse((cx - br * 0.6, by - br * 0.75, cx - br * 0.05, by - br * 0.25), fill=(255, 255, 230, 170))
    hl = hl.filter(ImageFilter.GaussianBlur(s * 0.006))
    img = Image.alpha_composite(img, hl)
    d = ImageDraw.Draw(img)
    r = s * 0.014
    d.ellipse((cx - r, s * 0.08 - r, cx + r, s * 0.08 + r), fill=(255, 60, 40, 255))
    out_size = ((w or px), px)
    if not rounded:
        return img.resize(out_size, Image.LANCZOS)
    # abgerundete Kachel (macOS-Raster: ~22 % Eckradius)
    corner = Image.new("L", size, 0)
    ImageDraw.Draw(corner).rounded_rectangle((0, 0, size[0] - 1, s - 1), radius=int(s * 0.22), fill=255)
    out = Image.new("RGBA", size, (0, 0, 0, 0))
    out.paste(img, (0, 0), corner)
    return out.resize(out_size, Image.LANCZOS)


def tile(w, h):
    """Xbox-Kachel: das Motiv ohne Rundung über die ganze Fläche (die Kachel rundet das System)."""
    return master(h, w=w, rounded=False)


def main():
    out = os.path.join(ROOT, "data", "gfx", "icon")
    os.makedirs(out, exist_ok=True)
    big = master(1024)
    big.save(os.path.join(out, "icon-1024.png"))
    with open(os.path.join(out, "icon-128.rgba"), "wb") as f:
        f.write(big.resize((128, 128), Image.LANCZOS).tobytes())
    # macOS: .iconset → .icns
    with tempfile.TemporaryDirectory() as tmp:
        iconset = os.path.join(tmp, "GtaBerlin.iconset")
        os.makedirs(iconset)
        for base in (16, 32, 128, 256, 512):
            for scale in (1, 2):
                n = base * scale
                name = f"icon_{base}x{base}{'@2x' if scale == 2 else ''}.png"
                big.resize((n, n), Image.LANCZOS).save(os.path.join(iconset, name))
        if shutil.which("iconutil"):
            subprocess.run(
                ["iconutil", "-c", "icns", iconset, "-o", os.path.join(out, "GtaBerlin.icns")], check=True
            )
        else:
            print("iconutil fehlt (nur macOS): GtaBerlin.icns nicht erneuert")
    # Browser
    web = os.path.join(ROOT, "web", "assets")
    big.resize((256, 256), Image.LANCZOS).save(os.path.join(web, "icon.png"))
    big.resize((180, 180), Image.LANCZOS).save(os.path.join(web, "icon-180.png"))
    # Xbox-Paket
    xb = os.path.join(ROOT, "xbox", "GtaBerlin", "Assets")
    for name, (w, h) in {
        "StoreLogo.png": (50, 50),
        "Square44x44Logo.png": (44, 44),
        "Square150x150Logo.png": (150, 150),
        "Wide310x150Logo.png": (310, 150),
        "SplashScreen.png": (620, 300),
    }.items():
        tile(w, h).save(os.path.join(xb, name))
    print("Icon erzeugt:", out)


if __name__ == "__main__":
    main()
