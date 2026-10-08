#!/usr/bin/env python3
"""Render the pre-masked macOS app icon for the DMG/.app bundle.

desktop/build/icons/mundus.png is full-bleed artwork; a macOS icon needs the
squircle with ~10% transparent margins and a soft drop shadow baked in (sips
can't alpha-mask, so the mask is applied here ahead of time — same approach
as zeron's dist/macos/icon-1024.png).

Output: desktop/build/macos/icon-1024.png — committed so packaging never
needs Pillow. Rerun only when the source artwork changes.

Usage: python3 desktop/scripts/macos-icon.py
"""

import os

from PIL import Image, ImageDraw, ImageFilter

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(ROOT, "build", "icons", "mundus.png")
OUT = os.path.join(ROOT, "build", "macos", "icon-1024.png")

CANVAS = 1024
# Apple's icon grid: the squircle occupies ~80.5% of the canvas (824/1024)
# with a continuous corner approximated by a ~22.5% radius.
ICON = 824
RADIUS = 186
SS = 4  # supersample factor for smooth edges


def squircle_mask(size, radius):
    mask = Image.new("L", (size * SS, size * SS), 0)
    draw = ImageDraw.Draw(mask)
    draw.rounded_rectangle(
        (0, 0, size * SS - 1, size * SS - 1), radius=radius * SS, fill=255
    )
    return mask.resize((size, size), Image.LANCZOS)


def main():
    art = Image.open(SRC).convert("RGBA").resize((ICON, ICON), Image.LANCZOS)

    icon = Image.new("RGBA", (CANVAS * SS, CANVAS * SS), (0, 0, 0, 0))
    off = (CANVAS - ICON) // 2 * SS

    # Drop shadow: same squircle, blurred, hugging the bottom edge.
    mask_ss = squircle_mask(ICON, RADIUS).resize((ICON * SS, ICON * SS), Image.LANCZOS)
    sh = Image.new("RGBA", icon.size, (0, 0, 0, 0))
    sh.paste(Image.new("RGBA", (ICON * SS, ICON * SS), (0, 0, 0, 255)), (off, off + 8 * SS), mask_ss)
    sh = sh.filter(ImageFilter.GaussianBlur(10 * SS))
    icon.alpha_composite(sh)

    masked = Image.new("RGBA", (ICON * SS, ICON * SS), (0, 0, 0, 0))
    masked.paste(art.resize((ICON * SS, ICON * SS), Image.LANCZOS), (0, 0), mask_ss)
    icon.alpha_composite(masked, (off, off))

    icon = icon.resize((CANVAS, CANVAS), Image.LANCZOS)
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    icon.save(OUT)
    print(f"wrote {os.path.relpath(OUT, ROOT)}")


if __name__ == "__main__":
    main()
