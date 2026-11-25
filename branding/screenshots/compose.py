#!/usr/bin/env python3
"""Turns the raw screenshots into the published product images.

    python3 branding/screenshots/compose.py

Reads branding/screenshots/raw/*.png (2880x1800, from capture.py) and writes:

  site/assets/screens/<name>-{1600,2400}.webp   framed images for the website
                                                (hero 3200x2000, others 3168x2088)
  docs/assets/screenshots/<name>.png            plain windows for the README
                                                and the AppStream metainfo

The framed images put the window, with rounded corners, a hairline border and
a soft shadow, on the brand's espresso or cream field with a warm glow.
"""
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw, ImageFilter

ROOT = Path(__file__).resolve().parents[2]
RAW = ROOT / "branding/screenshots/raw"
SITE = ROOT / "site/assets/screens"
DOCS = ROOT / "docs/assets/screenshots"

ESPRESSO = (28, 17, 13)
ROAST = (58, 36, 25)
CACAO = (90, 52, 38)
ORANGE = (232, 121, 58)
CREAM = (246, 239, 227)
PAPER = (251, 246, 238)


def rounded(image, radius):
    mask = Image.new("L", image.size, 0)
    ImageDraw.Draw(mask).rounded_rectangle((0, 0, *image.size), radius, fill=255)
    out = image.convert("RGBA")
    out.putalpha(ImageChops.multiply(out.getchannel("A"), mask))
    border = ImageDraw.Draw(out)
    border.rounded_rectangle((0, 0, image.width - 1, image.height - 1), radius,
                             outline=(255, 255, 255, 34), width=3)
    return out


def field(size, dark):
    """The background: a vertical gradient with a warm radial glow."""
    w, h = size
    top, bottom = (CACAO, ESPRESSO) if dark else (PAPER, CREAM)
    base = Image.new("RGB", size)
    draw = ImageDraw.Draw(base)
    for y in range(h):
        t = y / max(h - 1, 1)
        draw.line([(0, y), (w, y)], fill=tuple(round(a + (b - a) * t) for a, b in zip(top, bottom)))
    glow = Image.new("L", size, 0)
    ImageDraw.Draw(glow).ellipse((w * 0.45, -h * 0.35, w * 1.25, h * 0.75), fill=170 if dark else 110)
    glow = glow.filter(ImageFilter.GaussianBlur(w // 9))
    tint = Image.new("RGB", size, ORANGE)
    return Image.composite(tint, base, glow.point(lambda v: v * (0.55 if dark else 0.35)))


def shadow(window, offset, blur, opacity):
    alpha = window.getchannel("A").point(lambda v: v * opacity)
    pad = blur * 3
    layer = Image.new("L", (window.width + pad * 2, window.height + pad * 2), 0)
    layer.paste(alpha, (pad, pad))
    return layer.filter(ImageFilter.GaussianBlur(blur)), pad, offset


def place(canvas, window, x, y, blur=70, opacity=0.6, drop=50):
    mask, pad, _ = shadow(window, drop, blur, opacity)
    black = Image.new("RGB", mask.size, (0, 0, 0))
    canvas.paste(black, (x - pad, y - pad + drop), mask)
    canvas.paste(window, (x, y), window)


def framed(name, dark, back=None):
    """One window, optionally with a second one behind it, on a 16:10 field."""
    w, h = 3200, 2000
    canvas = field((w, h), dark)
    front = rounded(Image.open(RAW / f"{name}.png").convert("RGB"), 36)
    scale = 0.78
    front = front.resize((round(front.width * scale), round(front.height * scale)), Image.LANCZOS)
    if back:
        behind = rounded(Image.open(RAW / f"{back}.png").convert("RGB"), 36)
        behind = behind.resize((round(behind.width * scale * 0.86), round(behind.height * scale * 0.86)),
                               Image.LANCZOS)
        place(canvas, behind, w - behind.width - 120, 110, blur=60, opacity=0.45)
        place(canvas, front, 140, h - front.height - 150)
    else:
        place(canvas, front, (w - front.width) // 2, (h - front.height) // 2 + 10)
    return canvas


def tight(name, dark):
    """One window at full size with a short margin: 3168x2088."""
    front = rounded(Image.open(RAW / f"{name}.png").convert("RGB"), 40)
    margin_x, margin_top, margin_bottom = 144, 120, 168
    canvas = field((front.width + 2 * margin_x, front.height + margin_top + margin_bottom), dark)
    place(canvas, front, margin_x, margin_top, blur=60, opacity=0.55, drop=40)
    return canvas


def save_web(image, name):
    SITE.mkdir(parents=True, exist_ok=True)
    for width in (1600, 2400):
        out = image.resize((width, round(image.height * width / image.width)), Image.LANCZOS)
        out.save(SITE / f"{name}-{width}.webp", "WEBP", quality=86, method=6)


def main():
    compositions = {
        "hero": ("reading-dark", True, "newtab-light"),
        "workspaces": ("workspaces-light", False, None),
        "palette": ("palette-dark", True, None),
        "privacy": ("privacy-light", False, None),
        "newtab": ("newtab-light", False, None),
        "history": ("history-dark", True, None),
    }
    for name, (front, dark, back) in compositions.items():
        save_web(framed(front, dark, back) if back else tight(front, dark), name)
        print("framed", name)
    DOCS.mkdir(parents=True, exist_ok=True)
    for raw in sorted(RAW.glob("*.png")):
        image = Image.open(raw).convert("RGB")
        image.resize((1600, 1000), Image.LANCZOS).save(DOCS / raw.name, optimize=True)
        print("plain", raw.stem)


if __name__ == "__main__":
    main()
