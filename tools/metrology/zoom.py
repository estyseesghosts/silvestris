"""Write an upscaled crop for visual inspection: zoom.py FILE x y w h scale OUT"""
import sys
from PIL import Image
from corpus import CORPUS
import os
f, x, y, w, h, s, out = sys.argv[1], *map(int, sys.argv[2:7]), sys.argv[7]
im = Image.open(os.path.join(CORPUS, f)).convert("RGBA")
bg = Image.new("RGBA", im.size, (255, 0, 255, 255))
im = Image.alpha_composite(bg, im).crop((x, y, x + w, y + h))
im.resize((w * s, h * s), Image.NEAREST).save(out)
