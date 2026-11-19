"""Compare a region of a Silvestris capture with the same region of a 10.6 capture.

    python tools/metrology/compare.py OURS.png OX,OY REF_NAME RX,RY X,Y,W,H OUT.png [--zoom 8]

OX,OY / RX,RY are the window's top-left in each image (for ours, the
capture margin plus any frame border; for the reference, the window bounds
from measurements.json). X,Y,W,H is the region relative to the window. Writes
ours | reference | |difference|×4 side by side, upscaled with nearest
neighbour, and prints mean and max absolute channel difference.
The reference is read in place (corpus.py); OUT stays outside the repo.
"""
import argparse

import numpy as np
from PIL import Image

import corpus


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("ours")
    ap.add_argument("ours_origin")
    ap.add_argument("ref")
    ap.add_argument("ref_origin")
    ap.add_argument("region")
    ap.add_argument("out")
    ap.add_argument("--zoom", type=int, default=8)
    a = ap.parse_args()
    ox, oy = map(int, a.ours_origin.split(","))
    rx, ry = map(int, a.ref_origin.split(","))
    x, y, w, h = map(int, a.region.split(","))
    ours = np.array(Image.open(a.ours).convert("RGB")).astype(int)[oy + y: oy + y + h, ox + x: ox + x + w]
    ref_rgb, _, digest = corpus.load(a.ref)
    ref = ref_rgb[ry + y: ry + y + h, rx + x: rx + x + w]
    diff = np.abs(ours - ref)
    print(f"ref {a.ref} sha256 {digest[:16]}  region {x},{y},{w},{h}")
    print(f"mean |d| {diff.mean():.1f}  max |d| {diff.max()}  pixels with max channel |d|>16: "
          f"{int((diff.max(axis=2) > 16).sum())}/{w * h}")
    gap = np.full((h, 2, 3), 255)
    strip = np.concatenate([ours, gap, ref, gap, np.clip(diff * 4, 0, 255)], axis=1).astype(np.uint8)
    img = Image.fromarray(strip)
    img.resize((img.width * a.zoom, img.height * a.zoom), Image.NEAREST).save(a.out)


if __name__ == "__main__":
    main()
