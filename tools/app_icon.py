"""Ícone do app (256x256): gato pixel de assets/icons/icons.txt em quadrado com sombra dura."""
import sys
from pathlib import Path
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
lines = (ROOT / "assets/icons/icons.txt").read_text().splitlines()
rows = lines[lines.index("cat_2 mascot") + 1:][:16]
img = Image.new("RGBA", (256, 256), (0, 0, 0, 0))
d = ImageDraw.Draw(img)
d.rectangle([24, 24, 239, 239], fill=(236, 236, 236, 255))
d.rectangle([12, 12, 227, 227], fill=(0, 0, 0, 255), outline=(236, 236, 236, 255), width=4)
s = 12
for y, r in enumerate(rows):
    for x, c in enumerate(r):
        if c == "#":
            d.rectangle([24 + x * s, 24 + y * s, 24 + x * s + s - 1, 24 + y * s + s - 1], fill=(236, 236, 236, 255))
img.save(sys.argv[1] if len(sys.argv) > 1 else ROOT / "packaging/raster.png")
