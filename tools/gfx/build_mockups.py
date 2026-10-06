#!/usr/bin/env python3
"""README-Mockups: echte Spielaufnahmen (docs/images/screens/*.jpg, aufgenommen mit der nativen Fassung) in selbst
gezeichneten Geräterahmen – ein Laptop und ein Fernseher mit Controller (ohne Markenlogos). Gezeichnet mit Pillow in
2-facher Auflösung.

  python3 tools/gfx/build_mockups.py  →  docs/images/mockups/laptop.jpg, tv.jpg
"""
import os

from PIL import Image, ImageDraw, ImageFilter

ROOT = os.path.normpath(os.path.join(os.path.dirname(__file__), "..", ".."))
SCREENS = os.path.join(ROOT, "docs", "images", "screens")
OUT = os.path.join(ROOT, "docs", "images", "mockups")
SS = 2


def backdrop(w, h, top=(22, 24, 40), bottom=(70, 34, 72)):
    img = Image.new("RGB", (w, h))
    px = img.load()
    for y in range(h):
        t = y / (h - 1)
        c = tuple(round(top[i] + (bottom[i] - top[i]) * t) for i in range(3))
        for x in range(w):
            px[x, y] = c
    # weiches Licht von oben
    glow = Image.new("L", (w, h), 0)
    ImageDraw.Draw(glow).ellipse((w * 0.15, -h * 0.5, w * 0.85, h * 0.55), fill=70)
    glow = glow.filter(ImageFilter.GaussianBlur(w * 0.08))
    img.paste((255, 190, 120), (0, 0), glow)
    return img


def shadow(img, box, radius, blur, alpha=150):
    m = Image.new("L", img.size, 0)
    ImageDraw.Draw(m).rounded_rectangle(box, radius=radius, fill=alpha)
    m = m.filter(ImageFilter.GaussianBlur(blur))
    img.paste((0, 0, 0), (0, 0), m)


def fit(shot, w, h):
    """Aufnahme auf w × h zuschneiden (Mitte) und skalieren."""
    sw, sh = shot.size
    k = max(w / sw, h / sh)
    s = shot.resize((round(sw * k), round(sh * k)), Image.LANCZOS)
    x, y = (s.width - w) // 2, (s.height - h) // 2
    return s.crop((x, y, x + w, y + h))


def glare(img, box):
    """Schräger Glanzstreifen über dem Bildschirm."""
    x0, y0, x1, y1 = box
    m = Image.new("L", img.size, 0)
    d = ImageDraw.Draw(m)
    w = x1 - x0
    d.polygon([(x0 + w * 0.55, y0), (x0 + w * 0.75, y0), (x0 + w * 0.45, y1), (x0 + w * 0.25, y1)], fill=22)
    clip = Image.new("L", img.size, 0)
    ImageDraw.Draw(clip).rectangle(box, fill=255)
    from PIL import ImageChops

    m = ImageChops.multiply(m, clip).filter(ImageFilter.GaussianBlur(6))
    img.paste((255, 255, 255), (0, 0), m)


def laptop(shot, W=1600, H=1000):
    W, H = W * SS, H * SS
    img = backdrop(W, H)
    d = ImageDraw.Draw(img)
    # Deckel
    # Bildschirm im Seitenverhältnis der Aufnahme (16:9), damit der HUD-Rand nicht abgeschnitten wird
    lw = W * 0.76
    lh = (lw - 44 * SS) * 9 / 16 + 22 * SS * 2.15
    lx, ly = (W - lw) / 2, H * 0.08
    shadow(img, (lx, ly + 20 * SS, lx + lw, ly + lh + 60 * SS), 40 * SS, 40 * SS)
    d.rounded_rectangle((lx, ly, lx + lw, ly + lh), radius=30 * SS, fill=(28, 29, 33))
    d.rounded_rectangle((lx + 3 * SS, ly + 3 * SS, lx + lw - 3 * SS, ly + lh - 3 * SS), radius=27 * SS, fill=(10, 10, 12))
    b = 22 * SS
    sx0, sy0, sx1, sy1 = lx + b, ly + b * 1.25, lx + lw - b, ly + lh - b * 0.9
    scr = fit(shot, int(sx1 - sx0), int(sy1 - sy0))
    img.paste(scr, (int(sx0), int(sy0)))
    glare(img, (int(sx0), int(sy0), int(sx1), int(sy1)))
    d = ImageDraw.Draw(img)
    # Kamera
    d.ellipse((W / 2 - 4 * SS, ly + 8 * SS, W / 2 + 4 * SS, ly + 16 * SS), fill=(40, 42, 48))
    # Unterteil
    by = ly + lh
    bw = lw * 1.12
    bx = (W - bw) / 2
    d.polygon(
        [(lx - 4 * SS, by), (lx + lw + 4 * SS, by), (bx + bw, by + 26 * SS), (bx, by + 26 * SS)],
        fill=(176, 179, 186),
    )
    d.rounded_rectangle((bx, by + 18 * SS, bx + bw, by + 34 * SS), radius=10 * SS, fill=(150, 153, 160))
    d.rounded_rectangle((W / 2 - 90 * SS, by, W / 2 + 90 * SS, by + 9 * SS), radius=6 * SS, fill=(120, 123, 130))
    return img.resize((W // SS, H // SS), Image.LANCZOS)


def controller(d, cx, cy, s):
    """Gamepad von oben, stilisiert: Korpus mit zwei Griffen, zwei Sticks, Steuerkreuz, vier Tasten, Schultertasten."""
    body = (34, 36, 42)
    edge = (60, 63, 72)
    # Schultertasten
    d.rounded_rectangle((cx - 1.05 * s, cy - 0.62 * s, cx - 0.35 * s, cy - 0.42 * s), radius=0.1 * s, fill=edge)
    d.rounded_rectangle((cx + 0.35 * s, cy - 0.62 * s, cx + 1.05 * s, cy - 0.42 * s), radius=0.1 * s, fill=edge)
    # Griffe und Korpus
    d.ellipse((cx - 1.25 * s, cy - 0.35 * s, cx - 0.35 * s, cy + 0.85 * s), fill=body)
    d.ellipse((cx + 0.35 * s, cy - 0.35 * s, cx + 1.25 * s, cy + 0.85 * s), fill=body)
    d.rounded_rectangle((cx - 1.1 * s, cy - 0.55 * s, cx + 1.1 * s, cy + 0.35 * s), radius=0.4 * s, fill=body)
    # Sticks
    for x, y in ((-0.62, -0.12), (0.32, 0.18)):
        r = 0.2 * s
        d.ellipse((cx + x * s - r, cy + y * s - r, cx + x * s + r, cy + y * s + r), fill=(18, 18, 22))
        r2 = 0.14 * s
        d.ellipse((cx + x * s - r2, cy + y * s - r2, cx + x * s + r2, cy + y * s + r2), fill=(52, 55, 62))
    # Steuerkreuz
    px, py = cx - 0.32 * s, cy + 0.18 * s
    a, b = 0.05 * s, 0.17 * s
    d.rectangle((px - a, py - b, px + a, py + b), fill=(20, 20, 24))
    d.rectangle((px - b, py - a, px + b, py + a), fill=(20, 20, 24))
    # vier Tasten in Farbe
    bx, by = cx + 0.66 * s, cy - 0.12 * s
    for (dx, dy), col in (
        ((0, 0.15), (70, 190, 90)),
        ((0.15, 0), (220, 70, 60)),
        ((-0.15, 0), (60, 130, 230)),
        ((0, -0.15), (240, 200, 60)),
    ):
        r = 0.065 * s
        d.ellipse((bx + dx * s - r, by + dy * s - r, bx + dx * s + r, by + dy * s + r), fill=col)
    # Menütasten
    for dx in (-0.13, 0.13):
        d.rounded_rectangle(
            (cx + dx * s - 0.05 * s, cy - 0.2 * s, cx + dx * s + 0.05 * s, cy - 0.15 * s),
            radius=0.02 * s,
            fill=(80, 84, 92),
        )


def tv(shot, W=1600, H=1000):
    W, H = W * SS, H * SS
    img = backdrop(W, H, top=(18, 20, 32), bottom=(48, 30, 60))
    d = ImageDraw.Draw(img)
    # Lowboard
    d.rectangle((0, H * 0.82, W, H), fill=(30, 24, 26))
    d.rectangle((0, H * 0.82, W, H * 0.83), fill=(70, 56, 52))
    tw = W * 0.8
    th = tw * 9 / 16
    tx, ty = (W - tw) / 2, H * 0.06
    shadow(img, (tx, ty + 16 * SS, tx + tw, ty + th + 40 * SS), 12 * SS, 40 * SS)
    d.rounded_rectangle((tx, ty, tx + tw, ty + th), radius=10 * SS, fill=(12, 12, 14))
    b = 10 * SS
    sx0, sy0, sx1, sy1 = tx + b, ty + b, tx + tw - b, ty + th - b
    img.paste(fit(shot, int(sx1 - sx0), int(sy1 - sy0)), (int(sx0), int(sy0)))
    glare(img, (int(sx0), int(sy0), int(sx1), int(sy1)))
    d = ImageDraw.Draw(img)
    # Standfuß
    fy = ty + th
    d.polygon([(W / 2 - 60 * SS, fy), (W / 2 + 60 * SS, fy), (W / 2 + 140 * SS, H * 0.82), (W / 2 - 140 * SS, H * 0.82)], fill=(24, 24, 28))
    # Controller vorn, mit Schatten
    cs = 95 * SS
    cx, cy = W * 0.74, H * 0.865
    m = Image.new("L", img.size, 0)
    ImageDraw.Draw(m).ellipse((cx - 1.3 * cs, cy - 0.3 * cs, cx + 1.3 * cs, cy + 1.0 * cs), fill=120)
    img.paste((0, 0, 0), (0, 0), m.filter(ImageFilter.GaussianBlur(20 * SS)))
    controller(ImageDraw.Draw(img), cx, cy, cs)
    return img.resize((W // SS, H // SS), Image.LANCZOS)


def main():
    os.makedirs(OUT, exist_ok=True)
    shot = lambda n: Image.open(os.path.join(SCREENS, n + ".jpg")).convert("RGB")
    laptop(shot("tag")).save(os.path.join(OUT, "laptop.jpg"), quality=86, optimize=True)
    tv(shot("koop")).save(os.path.join(OUT, "tv.jpg"), quality=86, optimize=True)
    print("Mockups:", OUT)


if __name__ == "__main__":
    main()
