"""Load hig-09 reference captures as sRGB arrays with provenance.

Captures are read in place from the hig-09 corpus (rights undocumented, see
hig-09/00-source/README.md); nothing from them is copied into this repository.
Each capture's embedded ICC profile is converted to sRGB with LittleCMS
(Pillow ImageCms) so sampled colours are comparable with GPUI's sRGB colours.
"""
import hashlib
import io
import os
from functools import lru_cache

import numpy as np
from PIL import Image, ImageCms

CORPUS = os.environ.get(
    "HIG09_IMAGES",
    os.path.join(os.path.dirname(__file__), "..", "..", "..", "lotus-0.1.0", "hig-09", "images"),
)


@lru_cache(maxsize=None)
def load(name):
    """Return (rgb int array HxWx3 in sRGB, alpha HxW or None, sha256 hex)."""
    path = os.path.join(CORPUS, name)
    with open(path, "rb") as f:
        raw = f.read()
    digest = hashlib.sha256(raw).hexdigest()
    im = Image.open(io.BytesIO(raw))
    alpha = np.array(im.getchannel("A")).astype(int) if im.mode == "RGBA" else None
    rgb = im.convert("RGB")
    icc = im.info.get("icc_profile")
    if icc:
        src = ImageCms.ImageCmsProfile(io.BytesIO(icc))
        rgb = ImageCms.profileToProfile(rgb, src, ImageCms.createProfile("sRGB"), outputMode="RGB")
    return np.array(rgb).astype(int), alpha, digest


def hexc(px):
    return "#%02x%02x%02x" % tuple(int(v) for v in px)


def opaque_bbox(alpha):
    ys, xs = np.where(alpha == 255)
    return int(xs.min()), int(ys.min()), int(xs.max()), int(ys.max())
