"""Fit the window-control sphere model to the 10.6 Finder capture.

The model (mirrored exactly by silvestris-core sprites.rs) is original
analytic art: a disc with a 1 px ring, a saturated body darkening toward the
top, a radial glow toward the bottom, edge shading, and a small specular cap.
Per-light colours are sampled from fixed reference pixels below; only the
shared *shape* parameters are fitted, by minimising mean absolute error over
the three 14x14 spheres (M: traffic.*). No reference pixels are embedded.

    python tools/metrology/fit_traffic.py               # fit and print Rust constants
    python tools/metrology/fit_traffic.py --rust-check  # expected pixels for sprites.rs
"""
import json

import numpy as np
from scipy.optimize import minimize

import corpus

CAPTURE = "10-6-Snow-Leopard-Finder-Home.png"
# Window origin (M: win.finder.bounds) + traffic offsets (8, 4), pitch 21.
LIGHTS = {"close": 48, "minimize": 69, "zoom": 90}
TOP = 29
# Disabled close button of the Copy panel (window at x 40, same 8 px offset).
IDLE_CAPTURE = "10-6-Snow-Leopard-Finder-Copy-Replace.png"
IDLE_X = 48
D = 14
SS = 4


def sample_colours(rgb, x0):
    """Colours each light uses, from documented pixels (row, col within sphere)."""
    c = lambda r, k: rgb[TOP + r, x0 + k].astype(float)
    return {
        "ring_top": c(0, 7),       # top ring, centre
        "ring_bottom": c(13, 7),   # bottom ring, centre
        "deep": c(5, 7),           # saturated body, centre column under the cap
        "glow": c(11, 7),          # brightest lower body
        "specular": c(2, 7),       # cap peak
    }


def render(col, p, bg):
    """Model at 1x over per-row background `bg` (14x3). Returns 14x14x3."""
    mean, a = render_straight(col, p)
    return mean * a + bg[:, None, :] * (1 - a)


def render_straight(col, p):
    """Model at 1x: straight colour (14x14x3) and coverage (14x14x1)."""
    (top_end, k_top, gx, gy, gyr, gp, e0, k_e, sx, sy, syr, sp, r, rw, rp) = p
    c0 = D / 2.0
    # Sub-sample centres, shape (D*SS,): pixel p covers samples p*SS..p*SS+SS-1.
    s = (np.arange(D * SS) + 0.5) / SS
    y, x = np.meshgrid(s, s, indexing="ij")
    dx, dy = x - c0, y - c0
    d = np.hypot(dx, dy)
    inside = d <= r
    u, v = dx / r, dy / r
    t = (y / D)[..., None]
    lerp = lambda a, b, k: a + (b - a) * k[..., None]
    # Ring darkness holds through the upper body, lightening late toward the bottom.
    ring = col["ring_top"] + (col["ring_bottom"] - col["ring_top"]) * t ** rp
    c = np.broadcast_to(col["deep"], ring.shape)
    # Darken toward the top ring.
    c = c + (ring - c) * (np.clip((top_end - v) / (top_end + 1.0), 0, 1) ** 2 * k_top)[..., None]
    # Radial glow centred low in the sphere.
    g = np.clip(1 - ((u / gx) ** 2 + ((v - gy) / gyr) ** 2), 0, 1) ** gp
    c = lerp(c, col["glow"], g)
    # Edge shading toward the ring.
    c = c + (ring - c) * (np.clip((d / r - e0) / (1 - e0), 0, 1) ** 2 * k_e)[..., None]
    c = np.where((d > r - rw)[..., None], ring, c)
    # Specular cap.
    sc = np.clip(1 - ((u / sx) ** 2 + ((v - sy) / syr) ** 2), 0, 1) ** sp
    c = lerp(c, col["specular"], sc)
    # Box-filter each pixel's SS*SS samples; outside samples show the background.
    m = inside[..., None].astype(float)
    acc = (c * m).reshape(D, SS, D, SS, 3).sum(axis=(1, 3))
    n = m.reshape(D, SS, D, SS, 1).sum(axis=(1, 3))
    a = n / (SS * SS)
    mean = np.divide(acc, n, out=np.zeros_like(acc), where=n > 0)
    return mean, a


START = [0.212, 1.0, 1.003, 0.676, 0.862, 1.261, 0.617, 0.587, 0.321, -0.599, 0.214, 0.933, 7.073, 1.293, 2.055]
BOUNDS = [(-1, 0.5), (0, 1), (0.2, 1.5), (0, 1), (0.2, 1.5), (0.2, 3),
          (0, 0.95), (0, 1), (0.1, 0.8), (-0.95, 0), (0.05, 0.5), (0.2, 3),
          (6.5, 7.5), (0.5, 2.0), (1.0, 6.0)]


# The shape shipped in silvestris-core aqua.rs (`SphereShape`), from the last fit.
SHIPPED = [0.218, 1.0, 0.997, 0.674, 0.858, 1.259, 0.62, 0.657, 0.318, -0.597, 0.212, 0.907,
           7.1, 1.309, 1.982]


def rust_check():
    """Print expected pixels for sprites.rs `matches_the_fitted_python_model`."""
    rgb, _, _ = corpus.load(CAPTURE)
    mean, a = render_straight(sample_colours(rgb, LIGHTS["close"]), SHIPPED)
    for r, k in [(0, 7), (2, 7), (4, 2), (5, 7), (7, 0), (7, 7), (9, 3), (11, 7), (12, 11), (13, 7)]:
        c = [int(round(v)) for v in mean[r, k]] + [int(round(a[r, k, 0] * 255))]
        print(f"        ({r}, {k}, [{c[0]}, {c[1]}, {c[2]}, {c[3]}]),")


def main():
    if "--rust-check" in __import__("sys").argv:
        return rust_check()
    rgb, _, digest = corpus.load(CAPTURE)
    data = {}
    for name, x0 in LIGHTS.items():
        ref = rgb[TOP:TOP + D, x0:x0 + D].astype(float)
        bg = rgb[TOP:TOP + D, x0 - 1].astype(float)
        data[name] = (sample_colours(rgb, x0), ref, bg)

    def loss(p):
        p = [min(max(v, lo), hi) for v, (lo, hi) in zip(p, BOUNDS)]
        return np.mean([np.abs(render(c, p, bg) - ref).mean() for c, ref, bg in data.values()])

    print("start loss", round(loss(START), 2))
    # Nelder-Mead restarts from its own optimum until the loss stops improving.
    p, best = START, loss(START)
    for _ in range(8):
        res = minimize(loss, p, method="Nelder-Mead", options={"maxiter": 4000, "xatol": 1e-4, "fatol": 1e-4})
        p = [min(max(v, lo), hi) for v, (lo, hi) in zip(res.x, BOUNDS)]
        if best - res.fun < 1e-3:
            break
        best = res.fun
    print("fitted loss", round(loss(p), 2), "capture sha256", digest[:16])
    names = ["top_end", "k_top", "glow_x", "glow_y", "glow_ry", "glow_p",
             "edge_start", "edge_k", "spec_x", "spec_y", "spec_ry", "spec_p",
             "radius", "ring_width", "ring_pow"]
    print(json.dumps({k: round(float(v), 3) for k, v in zip(names, p)}, indent=1))
    for name, (c, ref, bg) in data.items():
        err = np.abs(render(c, p, bg) - ref).mean()
        print(name, "MAE", round(float(err), 2), {k: corpus.hexc(v) for k, v in c.items()})
    # Held-out check: the disabled close button of the Copy panel, same
    # position in its window, coloured from the same sample points.
    rgb, _, digest = corpus.load(IDLE_CAPTURE)
    c = sample_colours(rgb, IDLE_X)
    ref = rgb[TOP:TOP + D, IDLE_X:IDLE_X + D].astype(float)
    err = np.abs(render(c, p, rgb[TOP:TOP + D, IDLE_X - 1].astype(float)) - ref).mean()
    print("idle MAE", round(float(err), 2), "capture sha256", digest[:16],
          {k: corpus.hexc(v) for k, v in c.items()})


if __name__ == "__main__":
    main()
