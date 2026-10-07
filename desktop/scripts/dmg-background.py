#!/usr/bin/env python3
"""Render the Mundus DMG background.

Composed for the 660x400 pt drag-to-Applications window produced by
desktop/scripts/package-macos-dmg.mjs (app icon at x=165, Applications at
x=495, both at y=195 — the geometry below must match the dmgbuild settings
there). Near-black field with a sparse indigo dot grid and a chevron run
guiding the drag toward Applications.

Outputs desktop/build/macos/dmg-background.png (1x) and
dmg-background@2x.png; the packager pairs them into a hidpi tiff with
tiffutil. Both outputs are committed so CI never needs Pillow — rerun this
only when changing the artwork.

Usage: python3 desktop/scripts/dmg-background.py
"""

import math
import os

from PIL import Image, ImageDraw, ImageFilter

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(ROOT, "build", "macos")

# Window geometry in points; must match the dmgbuild settings in
# desktop/scripts/package-macos-dmg.mjs.
W, H = 660, 400

BG = (10, 9, 14)  # near-black, matching the icon's dark field
INDIGO = (118, 111, 252)  # brand accent sampled from build/icons/mundus.png

# Icon anchors (center points in points).
APP_X, APP_Y = 165, 195
APPS_X = 495


def draw_dots(draw, s):
    """Sparse dot grid in brand indigo, fading toward the window edges and
    denser behind the two icons — a quiet field, not wallpaper noise."""
    step = 22
    for gy in range(step, H, step):
        for gx in range(step, W, step):
            edge = min(gx, W - gx, gy, H - gy) / 160.0
            edge = min(1.0, edge)
            d_app = math.hypot(gx - APP_X, gy - APP_Y)
            d_apps = math.hypot(gx - APPS_X, gy - APP_Y)
            near = max(0.0, 1.0 - min(d_app, d_apps) / 200.0)
            alpha = int((0.05 + 0.16 * near) * edge * 255)
            if alpha <= 4:
                continue
            r = 1.1 * s
            draw.ellipse(
                (gx * s - r, gy * s - r, gx * s + r, gy * s + r),
                fill=INDIGO + (alpha,),
            )


def draw_arrow(draw, s):
    """Chevron run between the icons, brightening toward Applications —
    the same drag hint as zeron's background, drawn as polygons so no font
    is needed."""
    y = APP_Y - 1
    for i, cx in enumerate(range(285, 380, 24)):
        alpha = int((0.30 + i * 0.16) * 255)
        w, h, t = 16 * s, 22 * s, 4.5 * s  # extent, height, stroke thickness
        x0, y0 = cx * s, (y - h / 2) * s
        pts = [
            (x0, y0),
            (x0 + t, y0),
            (x0 + w, y0 + h / 2),
            (x0 + t, y0 + h),
            (x0, y0 + h),
            (x0 + w - t, y0 + h / 2),
        ]
        draw.polygon(pts, fill=INDIGO + (alpha,))


def render(s, path):
    """Render the full background at pixel scale s (1 or 2)."""
    img = Image.new("RGB", (W * s, H * s), BG)

    # Soft indigo glow centered on the drag path, blurred wide.
    glow = Image.new("RGBA", img.size, (0, 0, 0, 0))
    g = ImageDraw.Draw(glow)
    cx, cy = (W / 2) * s, APP_Y * s
    g.ellipse((cx - 260 * s, cy - 150 * s, cx + 260 * s, cy + 150 * s),
              fill=INDIGO + (26,))
    glow = glow.filter(ImageFilter.GaussianBlur(60 * s))
    img.paste(glow, (0, 0), glow)

    layer = Image.new("RGBA", img.size, (0, 0, 0, 0))
    draw = ImageDraw.Draw(layer)
    draw_dots(draw, s)
    draw_arrow(draw, s)
    img.paste(layer, (0, 0), layer)

    img.save(path)
    print(f"wrote {os.path.relpath(path, ROOT)} ({img.size[0]}x{img.size[1]})")


render(1, os.path.join(OUT, "dmg-background.png"))
render(2, os.path.join(OUT, "dmg-background@2x.png"))
