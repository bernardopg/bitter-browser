#!/usr/bin/env python3
"""Generates the Bitter Browser brand assets.

Every logo, icon and social image is drawn here from geometry and from the
outlines of the brand typefaces, so the assets never depend on installed
fonts and can be regenerated exactly.

Usage:
    branding/build.py [--fraunces PATH] [--inter PATH]

Needs fontTools and Pillow (Python), rsvg-convert (librsvg) and ImageMagick.
Fraunces is downloaded from the Google Fonts repository when no path is
given; Inter defaults to the system copy. Both are under the SIL Open Font
License.
"""
import argparse
import json
import math
import shutil
import subprocess
import urllib.request
from pathlib import Path

from fontTools.pens.recordingPen import RecordingPen
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.ttLib import TTFont
from fontTools.varLib import instancer

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "branding"
APP_ID = "br.com.bebitterbebetter.browser.Bitter"
HICOLOR = ROOT / "crates/bitter-ui/data/icons/hicolor"
FRAUNCES_URL = ("https://github.com/google/fonts/raw/main/ofl/fraunces/"
                "Fraunces%5BSOFT,WONK,opsz,wght%5D.ttf")

COLORS = {
    "espresso": {"hex": "#1C110D", "use": "Ink, dark backgrounds, text on light"},
    "roast": {"hex": "#3A2419", "use": "Dark surfaces, gradients"},
    "cacao": {"hex": "#5A3426", "use": "Gradient highlight on dark"},
    "bitter-orange": {"hex": "#E8793A", "use": "Accent: the drop, calls to action, links"},
    "amber": {"hex": "#F4A259", "use": "Accent gradient start, highlights on dark"},
    "zest": {"hex": "#D9622B", "use": "Accent gradient end, pressed states"},
    "cream": {"hex": "#F6EFE3", "use": "Light backgrounds"},
    "paper": {"hex": "#FBF6EE", "use": "Lightest surface, text on dark"},
    "gentian": {"hex": "#2F5D50", "use": "Secondary accent for success and privacy cues, sparingly"},
}
C = {name: value["hex"] for name, value in COLORS.items()}
SLOGAN = "The web, unsweetened."
TAGLINE = "Be bitter. Browse better."

# The mark: a lowercase b drawn as a drop of bitters. The stem is the left
# tangent of the bowl, the drop's point is the top of the stem, and the right
# side is the second tangent from that point to the bowl.
BOWL = (280.0, 340.0, 130.0)  # centre x, centre y, radius
TIP = (150.0, 40.0)
COUNTER = 55.0


def mark_path():
    cx, cy, r = BOWL
    px, py = TIP
    d = math.hypot(px - cx, py - cy)
    toward_tip = math.atan2(py - cy, px - cx)
    spread = math.acos(r / d)
    right = toward_tip + spread
    qx, qy = cx + r * math.cos(right), cy + r * math.sin(right)
    return (f"M{px:g} {py:g} L{cx - r:g} {cy:g} "
            f"A{r:g} {r:g} 0 1 0 {qx:.2f} {qy:.2f} Z "
            f"M{cx:g} {cy - COUNTER:g} a{COUNTER:g} {COUNTER:g} 0 1 0 0.01 0 Z")


MARK_BOX = (150.0, 40.0, 410.0, 470.0)  # x0, y0, x1, y1


def mark(x, y, height, fill, extra=""):
    """The mark with its bounding box's top-left at (x, y)."""
    s = height / (MARK_BOX[3] - MARK_BOX[1])
    return (f'<path transform="translate({x:.2f} {y:.2f}) scale({s:.5f}) '
            f'translate({-MARK_BOX[0]:g} {-MARK_BOX[1]:g})" fill="{fill}" '
            f'fill-rule="evenodd" d="{mark_path()}"{extra}/>')


def mark_width(height):
    return height * (MARK_BOX[2] - MARK_BOX[0]) / (MARK_BOX[3] - MARK_BOX[1])


def drop(cx, cy, r, tip=2.25):
    """An upright drop: a circle of radius r with a point tip*r above its centre."""
    spread = math.degrees(math.acos(1 / tip))
    a1, a2 = math.radians(-90 - spread), math.radians(-90 + spread)
    p1 = (cx + r * math.cos(a1), cy + r * math.sin(a1))
    p2 = (cx + r * math.cos(a2), cy + r * math.sin(a2))
    return (f"M{cx:.2f} {cy - tip * r:.2f} L{p2[0]:.2f} {p2[1]:.2f} "
            f"A{r:.2f} {r:.2f} 0 1 1 {p1[0]:.2f} {p1[1]:.2f} Z")


class Face:
    def __init__(self, path, **axes):
        self.font = instancer.instantiateVariableFont(TTFont(path), axes)
        self.glyphs = self.font.getGlyphSet()
        self.cmap = self.font.getBestCmap()
        self.upem = self.font["head"].unitsPerEm

    def width(self, text, size, track=0.0):
        s = size / self.upem
        return sum(self.font["hmtx"][self.cmap[ord(ch)]][0] * s + track for ch in text) - track

    def text(self, text, size, x, baseline, fill, track=0.0, drop_dot=None):
        """Outlines text. drop_dot replaces the dot of each i with a drop of that colour."""
        s = size / self.upem
        parts = []
        for ch in text:
            name = self.cmap[ord(ch)]
            pen = SVGPathPen(self.glyphs)
            if ch == "i" and drop_dot:
                recording = RecordingPen()
                self.glyphs[name].draw(recording)
                contours, current = [], []
                for op, args in recording.value:
                    current.append((op, args))
                    if op in ("closePath", "endPath"):
                        contours.append(current)
                        current = []

                def bounds(contour):
                    points = [p for _, args in contour for p in args]
                    xs, ys = [p[0] for p in points], [p[1] for p in points]
                    return min(xs), min(ys), max(xs), max(ys)

                dot = max(contours, key=lambda c: bounds(c)[1])
                for contour in contours:
                    if contour is not dot:
                        for op, args in contour:
                            getattr(pen, op)(*args)
                x0, y0, x1, y1 = bounds(dot)
                r = (x1 - x0) / 2 * s * 0.95
                cx = x + (x0 + x1) / 2 * s
                cy = baseline - (y0 + y1) / 2 * s + r * 0.35
                parts.append(f'<path fill="{drop_dot}" d="{drop(cx, cy, r)}"/>')
            else:
                self.glyphs[name].draw(pen)
            parts.append(f'<path transform="translate({x:.2f} {baseline:.2f}) '
                         f'scale({s:.5f} {-s:.5f})" fill="{fill}" d="{pen.getCommands()}"/>')
            x += self.font["hmtx"][name][0] * s + track
        return "".join(parts)


def svg(width, height, body, defs=""):
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" '
            f'viewBox="0 0 {width} {height}"><defs>{defs}</defs>{body}</svg>\n')


GRADIENTS = (
    f'<linearGradient id="espresso" x1="0" y1="0" x2="1" y2="1">'
    f'<stop offset="0" stop-color="{C["cacao"]}"/><stop offset="0.55" stop-color="{C["roast"]}"/>'
    f'<stop offset="1" stop-color="{C["espresso"]}"/></linearGradient>'
    f'<linearGradient id="orange" x1="0" y1="0" x2="1" y2="1">'
    f'<stop offset="0" stop-color="{C["amber"]}"/><stop offset="1" stop-color="{C["zest"]}"/></linearGradient>'
    f'<linearGradient id="sheen" x1="0" y1="0" x2="0" y2="1">'
    f'<stop offset="0" stop-color="#fff" stop-opacity="0.14"/>'
    f'<stop offset="0.5" stop-color="#fff" stop-opacity="0"/></linearGradient>'
    f'<radialGradient id="glow" cx="0.5" cy="0.5" r="0.5">'
    f'<stop offset="0" stop-color="{C["bitter-orange"]}" stop-opacity="0.35"/>'
    f'<stop offset="1" stop-color="{C["bitter-orange"]}" stop-opacity="0"/></radialGradient>'
)


def write(path, text):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")
    return path


def png(source, target, width, height=None):
    target.parent.mkdir(parents=True, exist_ok=True)
    args = ["rsvg-convert", "--width", str(width)]
    if height:
        args += ["--height", str(height)]
    subprocess.run(args + ["--output", str(target), str(source)], check=True)


# ---------------------------------------------------------------- the assets

SHOT = ROOT / "docs/assets/history-dark.png"


def product_shot(x, y, width):
    """The real browser, framed as a floating window with a soft shadow."""
    import base64
    height = width * 800 / 1200
    data = base64.b64encode(SHOT.read_bytes()).decode()
    r = width * 0.018
    uid = f"shot{int(x)}{int(y)}"
    return (f'<filter id="{uid}s" x="-20%" y="-20%" width="140%" height="150%">'
            f'<feGaussianBlur stdDeviation="{width * 0.03:.1f}"/></filter>'
            f'<clipPath id="{uid}c"><rect x="{x}" y="{y}" width="{width}" height="{height:.1f}" rx="{r:.1f}"/></clipPath>'
            f'<rect x="{x}" y="{y + width * 0.04:.1f}" width="{width}" height="{height:.1f}" rx="{r:.1f}" '
            f'fill="#000" opacity="0.55" filter="url(#{uid}s)"/>'
            f'<image x="{x}" y="{y}" width="{width}" height="{height:.1f}" clip-path="url(#{uid}c)" '
            f'href="data:image/png;base64,{data}"/>'
            f'<rect x="{x}" y="{y}" width="{width}" height="{height:.1f}" rx="{r:.1f}" fill="none" '
            f'stroke="#fff" stroke-opacity="0.12" stroke-width="1.5"/>')

def app_icon():
    """GNOME-style app icon: espresso tile, orange drop-b, soft top sheen."""
    tile = 'x="16" y="16" width="480" height="480" rx="112"'
    h = 340
    body = (f'<rect {tile} fill="url(#espresso)"/>'
            f'<rect {tile} fill="url(#sheen)"/>'
            f'<rect x="17" y="17" width="478" height="478" rx="111" fill="none" '
            f'stroke="#fff" stroke-opacity="0.08" stroke-width="2"/>'
            + mark(256 - mark_width(h) / 2 + 6, 256 - h / 2 + 4, h, "url(#orange)"))
    return svg(512, 512, body, GRADIENTS)


def symbolic_icon():
    h = 14.0
    return svg(16, 16, mark(8 - mark_width(h) / 2, 1, h, "#2e3436"))


def build(fraunces_path, inter_path):
    display = Face(fraunces_path, wght=600, opsz=144, SOFT=100, WONK=0)
    display_light = Face(fraunces_path, wght=380, opsz=144, SOFT=100, WONK=0)
    display_italic = None
    ui = Face(inter_path, wght=500, opsz=32) if "opsz" in [a.axisTag for a in TTFont(inter_path)["fvar"].axes] else Face(inter_path, wght=500)
    ui_bold = Face(inter_path, wght=650, opsz=32) if "opsz" in [a.axisTag for a in TTFont(inter_path)["fvar"].axes] else Face(inter_path, wght=650)
    del display_italic

    written = []

    # Colour tokens.
    write(OUT / "tokens/colors.json", json.dumps(COLORS, indent=2) + "\n")
    css = [":root {"] + [f"  --bitter-{name}: {value['hex']};" for name, value in COLORS.items()]
    css += ["  --bitter-font-display: 'Fraunces', Georgia, serif;",
            "  --bitter-font-text: 'Inter', system-ui, sans-serif;",
            "  --bitter-font-mono: 'JetBrains Mono', ui-monospace, monospace;", "}"]
    write(OUT / "tokens/brand.css", "\n".join(css) + "\n")
    for name, value in COLORS.items():
        write(OUT / f"tokens/swatches/{name}.svg",
              svg(48, 24, f'<rect width="48" height="24" rx="4" fill="{value['hex']}" '
                          f'stroke="#000" stroke-opacity="0.15"/>'))

    # Mark.
    variants = {
        "mark": ("url(#orange)", None),
        "mark-espresso": (C["espresso"], None),
        "mark-paper": (C["paper"], None),
        "mark-black": ("#000000", None),
    }
    for name, (fill, _) in variants.items():
        written.append(write(OUT / f"logo/{name}.svg",
                             svg(300, 480, mark(20, 20, 440, fill), GRADIENTS)))

    # Wordmark "bitter browser".
    size = 120
    w1 = display.width("bitter", size, -1)
    gap = size * 0.24
    w2 = display_light.width("browser", size, -1)
    for name, ink in (("wordmark", C["espresso"]), ("wordmark-paper", C["paper"])):
        body = (display.text("bitter", size, 0, 110, ink, -1, C["bitter-orange"])
                + display_light.text("browser", size, w1 + gap, 110, ink, -1))
        written.append(write(OUT / f"logo/{name}.svg", svg(round(w1 + gap + w2 + 2), 140, body)))

    # Horizontal lockup: mark + wordmark.
    mh = 138
    mw = mark_width(mh)
    for name, ink, mfill in (("lockup", C["espresso"], "url(#orange)"),
                             ("lockup-paper", C["paper"], "url(#orange)"),
                             ("lockup-espresso", C["espresso"], C["espresso"])):
        x = mw + 44
        body = (mark(0, 144 - mh, mh, mfill)
                + display.text("bitter", size, x, 140, ink, -1, C["bitter-orange"] if mfill != C["espresso"] else C["espresso"])
                + display_light.text("browser", size, x + w1 + gap, 140, ink, -1))
        written.append(write(OUT / f"logo/{name}.svg",
                             svg(round(x + w1 + gap + w2 + 4), 165, body, GRADIENTS)))

    # Stacked lockup: mark above the wordmark, centred.
    total = w1 + gap + w2
    for name, ink in (("lockup-stacked", C["espresso"]), ("lockup-stacked-paper", C["paper"])):
        body = (mark(total / 2 - mark_width(220) / 2 + 10, 0, 220, "url(#orange)")
                + display.text("bitter", size, 0, 360, ink, -1, C["bitter-orange"])
                + display_light.text("browser", size, w1 + gap, 360, ink, -1))
        written.append(write(OUT / f"logo/{name}.svg", svg(round(total + 2), 390, body, GRADIENTS)))

    # App icons.
    icon = write(OUT / "icon/app-icon.svg", app_icon())
    symbolic = write(OUT / "icon/app-icon-symbolic.svg", symbolic_icon())
    shutil.copy(icon, HICOLOR / f"scalable/apps/{APP_ID}.svg")
    (HICOLOR / "symbolic/apps").mkdir(parents=True, exist_ok=True)
    shutil.copy(symbolic, HICOLOR / f"symbolic/apps/{APP_ID}-symbolic.svg")
    for size_px in (16, 24, 32, 48, 64, 128, 256, 512):
        png(icon, HICOLOR / f"{size_px}x{size_px}/apps/{APP_ID}.png", size_px, size_px)
    for size_px, name in ((180, "apple-touch-icon"), (192, "icon-192"), (512, "icon-512"), (1024, "app-icon-1024")):
        png(icon, OUT / f"icon/{name}.png", size_px, size_px)
    shutil.copy(icon, OUT / "icon/favicon.svg")
    for size_px in (16, 32, 48):
        png(icon, OUT / f"icon/.favicon-{size_px}.png", size_px, size_px)
    subprocess.run(["magick"] + [str(OUT / f"icon/.favicon-{s}.png") for s in (16, 32, 48)]
                   + [str(OUT / "icon/favicon.ico")], check=True)
    for s in (16, 32, 48):
        (OUT / f"icon/.favicon-{s}.png").unlink()

    # Social and campaign images: espresso field, glow, lockup, slogan.
    def campaign(width, height, layout):
        defs = GRADIENTS
        body = (f'<rect width="{width}" height="{height}" fill="url(#espresso)"/>'
                f'<circle cx="{width * layout["glow"][0]}" cy="{height * layout["glow"][1]}" '
                f'r="{max(width, height) * layout["glow"][2]}" fill="url(#glow)"/>')
        scale = layout["scale"]
        lx, ly = layout["lockup"]
        mh2 = 138 * scale
        x = lx + mark_width(mh2) + 44 * scale
        size2 = 120 * scale
        body += mark(lx, ly + (144 - 138) * scale, mh2, "url(#orange)")
        body += display.text("bitter", size2, x, ly + 140 * scale, C["paper"], -1 * scale, C["bitter-orange"])
        body += display_light.text("browser", size2, x + display.width("bitter", size2, -scale) + size2 * 0.24,
                                   ly + 140 * scale, C["paper"], -1 * scale)
        sx, sy, ssize = layout["slogan"]
        body += display_light.text(SLOGAN, ssize, sx, sy, C["paper"], -0.5)
        if layout.get("detail"):
            dx, dy, dsize = layout["detail"]
            body += ui.text("Native Linux web browser  ·  GTK 4  ·  WebKitGTK  ·  Rust", dsize, dx, dy,
                            C["amber"], 0.4)
        if layout.get("shot"):
            body += product_shot(*layout["shot"])
        if layout.get("url"):
            ux, uy, usize = layout["url"]
            body += ui.text("browser.bebitterbebetter.com.br", usize, ux, uy, C["cream"], 0.3)
        return svg(width, height, body, defs)

    social = {
        "og-image": (1200, 630, {"glow": (0.75, 0.35, 0.55), "scale": 0.62, "lockup": (70, 170),
                                 "slogan": (72, 340, 46), "detail": (74, 400, 17), "url": (74, 560, 17),
                                 "shot": (600, 140, 640)}),
        "github-social-preview": (1280, 640, {"glow": (0.75, 0.35, 0.55), "scale": 0.66, "lockup": (76, 175),
                                              "slogan": (78, 352, 49), "detail": (80, 414, 18), "url": (80, 570, 18),
                                              "shot": (640, 140, 680)}),
        "x-header": (1500, 500, {"glow": (0.85, 0.3, 0.5), "scale": 0.75, "lockup": (560, 120),
                                 "slogan": (562, 340, 50), "url": (564, 410, 20)}),
        "linkedin-banner": (1584, 396, {"glow": (0.9, 0.3, 0.45), "scale": 0.6, "lockup": (640, 90),
                                        "slogan": (642, 270, 40), "url": (644, 330, 18)}),
        "instagram-post": (1080, 1080, {"glow": (0.7, 0.75, 0.6), "scale": 0.8, "lockup": (100, 110),
                                        "slogan": (102, 360, 62), "detail": (104, 430, 22), "url": (104, 1010, 22),
                                        "shot": (140, 520, 940)}),
        "instagram-story": (1080, 1920, {"glow": (0.6, 0.55, 0.7), "scale": 0.8, "lockup": (100, 330),
                                         "slogan": (102, 590, 62), "detail": (104, 660, 22), "url": (104, 1780, 26),
                                         "shot": (100, 900, 1000)}),
    }
    for name, (width, height, layout) in social.items():
        source = write(OUT / f"social/{name}.svg", campaign(width, height, layout))
        png(source, OUT / f"social/{name}.png", width, height)

    # README banner.
    banner = campaign(1280, 400, {"glow": (0.85, 0.25, 0.5), "scale": 0.72, "lockup": (90, 95),
                                  "slogan": (92, 300, 46), "detail": (94, 352, 19)})
    write(ROOT / "docs/assets/banner.svg", banner)

    # PNG exports of the logos.
    for source in sorted((OUT / "logo").glob("*.svg")):
        png(source, OUT / f"logo/png/{source.stem}.png", 1200)
    return written


SITE_FONTS = ROOT / "site/fonts"
LATIN = ("U+0000-00FF,U+0131,U+0152-0153,U+02BB-02BC,U+02C6,U+02DA,U+02DC,U+2000-206F,"
         "U+20AC,U+2122,U+2191,U+2193,U+2212,U+2215,U+FEFF,U+FFFD")


def site_fonts(fraunces, inter, mono):
    """Self-hosted, Latin-subset WOFF fonts for the website: no third-party font requests."""
    from fontTools import subset
    SITE_FONTS.mkdir(parents=True, exist_ok=True)
    jobs = [
        (fraunces, {"wght": 380, "opsz": 144, "SOFT": 100, "WONK": 0}, "fraunces-380"),
        (fraunces, {"wght": 600, "opsz": 144, "SOFT": 100, "WONK": 0}, "fraunces-600"),
        (inter, {"wght": 400, "opsz": 14}, "inter-400"),
        (inter, {"wght": 550, "opsz": 14}, "inter-550"),
    ]
    for source, axes, name in jobs:
        font = TTFont(source)
        present = {a.axisTag for a in font["fvar"].axes}
        font = instancer.instantiateVariableFont(font, {k: v for k, v in axes.items() if k in present})
        temp = SITE_FONTS / f"{name}.ttf"
        font.save(temp)
        subset.main([str(temp), f"--unicodes={LATIN}", "--layout-features=kern,liga,calt,ccmp,locl,mark,mkmk,onum,lnum,tnum",
                     "--flavor=woff", "--no-hinting",
                     f"--output-file={SITE_FONTS / (name + '.woff')}"])
        temp.unlink()
    subset.main([str(mono), f"--unicodes={LATIN}", "--flavor=woff", "--no-hinting",
                 f"--output-file={SITE_FONTS / 'jetbrains-mono-500.woff'}"])


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--fraunces", type=Path)
    parser.add_argument("--inter", type=Path, default=Path("/usr/share/fonts/inter/InterVariable.ttf"))
    parser.add_argument("--mono", type=Path, default=Path("/usr/share/fonts/TTF/JetBrainsMono-Medium.ttf"))
    args = parser.parse_args()
    fraunces = args.fraunces
    if fraunces is None:
        cache = Path.home() / ".cache/bitter-browser-brand/Fraunces-VF.ttf"
        if not cache.exists():
            cache.parent.mkdir(parents=True, exist_ok=True)
            urllib.request.urlretrieve(FRAUNCES_URL, cache)
        fraunces = cache
    build(fraunces, args.inter)
    site_fonts(fraunces, args.inter, args.mono)
    print(f"Brand assets written to {OUT}")


if __name__ == "__main__":
    main()
