"""Visual gate for Silvestris goldens (#18).

Compares a fresh render tree against committed goldens and writes an HTML diff
sheet. The golden comparison is strict: a pixel fails when any channel differs
by more than --tolerance (default 0). The distance to a 10.6 crop from
reference/crops.json is reported for information only and never gates.

    python tools/metrology/gate.py --candidate target/goldens
    python tools/metrology/gate.py --candidate target/goldens \
        --accept --reason "raise the checkbox tick by one pixel"

Accepted goldens record the reason in goldens/reasons.json and refresh
goldens/manifest.json. `cargo test -p silvestris --test goldens` fails when a
golden file does not match that manifest, so an edit without --accept is caught
even when the render step is not run.
"""
import argparse
import hashlib
import html
import json
import os
from pathlib import Path

from PIL import Image, ImageChops, ImageStat

ROOT = Path(__file__).resolve().parents[2]
GOLDENS = ROOT / "goldens" / "renders"
MANIFEST = ROOT / "goldens" / "manifest.json"
REASONS = ROOT / "goldens" / "reasons.json"
CROPS = ROOT / "reference" / "crops.json"
CORPUS = Path(os.environ.get("HIG09_IMAGES", Path.home() / "workdir" / "hig-09" / "vm-screenshots"))


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load_json(path, default):
    if path.exists():
        return json.loads(path.read_text(encoding="utf-8"))
    return default


def difference(golden, candidate, tolerance):
    if golden.size != candidate.size:
        return None
    diff = ImageChops.difference(golden.convert("RGB"), candidate.convert("RGB"))
    red, green, blue = diff.split()
    peak = ImageChops.lighter(ImageChops.lighter(red, green), blue)
    histogram = peak.histogram()
    changed = sum(histogram[tolerance + 1:])
    worst = max((value for value, count in enumerate(histogram) if count), default=0)
    return {
        "changed": changed,
        "total": golden.size[0] * golden.size[1],
        "worst": worst,
        "mean": sum(ImageStat.Stat(diff).mean) / 3.0,
        "diff": diff,
    }


def crop_distance(candidate, crop, corpus_root):
    source = corpus_root / crop["capture"]
    if not source.exists():
        return None
    reference = Image.open(source).convert("RGB")
    x, y, width, height = crop["rect"]
    reference = reference.crop((x, y, x + width, y + height))
    scaled = candidate.convert("RGB").resize(reference.size, Image.LANCZOS)
    delta = ImageChops.difference(reference, scaled)
    return sum(ImageStat.Stat(delta).mean) / 3.0


def rel(path, start):
    return os.path.relpath(path, start).replace("\\", "/")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--candidate", type=Path, default=ROOT / "target" / "goldens")
    parser.add_argument("--goldens", type=Path, default=GOLDENS)
    parser.add_argument("--report", type=Path, default=ROOT / "target" / "golden-report")
    parser.add_argument("--tolerance", type=int, default=0)
    parser.add_argument("--accept", action="store_true")
    parser.add_argument("--reason", default="")
    parser.add_argument("--crops", type=Path, default=CROPS)
    parser.add_argument("--corpus", type=Path, default=CORPUS)
    args = parser.parse_args()

    if args.accept and not args.reason.strip():
        parser.error("--accept requires --reason")

    goldens = sorted(args.goldens.rglob("*.png"))
    if not goldens:
        parser.error(f"no goldens under {args.goldens}")
    manifest = load_json(MANIFEST, {})
    reasons = load_json(REASONS, {})
    crop_data = load_json(args.crops, {"crops": []}) if args.crops.exists() else {"crops": []}
    # A control can have more than one crop (default and regular, for example).
    # Report the regular state against the regular golden.
    by_control = {}
    for crop in crop_data.get("crops", []):
        control = crop["control"]
        if control not in by_control or crop["id"].endswith(".regular"):
            by_control[control] = crop

    args.report.mkdir(parents=True, exist_ok=True)
    failures = []
    rows = []
    accepted = []
    for golden_path in goldens:
        relative = golden_path.relative_to(args.goldens)
        key = relative.as_posix()
        candidate_path = args.candidate / relative
        row = {"key": key, "status": "ok", "detail": "", "changed": 0, "worst": 0,
               "crop": None, "distance": None}
        if not candidate_path.exists():
            row["status"] = "missing"
            row["detail"] = f"no candidate at {candidate_path}"
            failures.append(key)
            rows.append(row)
            continue
        golden = Image.open(golden_path)
        candidate = Image.open(candidate_path)
        result = difference(golden, candidate, args.tolerance)
        if result is None:
            row["status"] = "size"
            row["detail"] = f"golden {golden.size} candidate {candidate.size}"
            failures.append(key)
        elif result["changed"]:
            row["status"] = "diff"
            row["changed"] = result["changed"]
            row["worst"] = result["worst"]
            row["detail"] = (f"{result['changed']} px over tolerance, worst {result['worst']}, "
                             f"mean {result['mean']:.2f}")
            diff_dir = args.report / "diff" / relative.parent
            diff_dir.mkdir(parents=True, exist_ok=True)
            amplified = result["diff"].point(lambda value: min(255, value * 4))
            amplified.save(diff_dir / relative.name)
            failures.append(key)
        if args.accept:
            golden_path.parent.mkdir(parents=True, exist_ok=True)
            golden_path.write_bytes(candidate_path.read_bytes())
            accepted.append(key)
            row["status"] = "accepted"
            row["detail"] = args.reason

        parts = relative.parts
        if len(parts) == 3 and parts[0] == "aqua-light" and parts[1] == "100":
            crop = by_control.get(golden_path.stem)
            if crop:
                row["crop"] = crop["id"]
                row["distance"] = crop_distance(candidate, crop, args.corpus)
        rows.append(row)

    for key in accepted:
        manifest[key] = sha256(args.goldens / key)
        reasons[key] = args.reason
    if args.accept:
        # A new scale or a new control produces candidates with no golden. Accept
        # them too, so the workflow does not need a manual copy.
        known = {path.relative_to(args.goldens).as_posix() for path in goldens}
        for candidate_path in sorted(args.candidate.rglob("*.png")):
            relative = candidate_path.relative_to(args.candidate)
            key = relative.as_posix()
            if key in known:
                continue
            target = args.goldens / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(candidate_path.read_bytes())
            manifest[key] = sha256(target)
            reasons[key] = args.reason
            rows.append({"key": key, "status": "added", "detail": args.reason, "changed": 0,
                         "worst": 0, "crop": None, "distance": None})
        MANIFEST.parent.mkdir(parents=True, exist_ok=True)
        MANIFEST.write_text(json.dumps(dict(sorted(manifest.items())), indent=2) + "\n",
                            encoding="utf-8")
        REASONS.write_text(json.dumps(dict(sorted(reasons.items())), indent=2) + "\n",
                           encoding="utf-8")

    page = ["<!doctype html>", "<meta charset='utf-8'>", "<title>Silvestris golden report</title>",
            "<style>body{font:13px system-ui;margin:24px}table{border-collapse:collapse}"
            "td,th{border:1px solid #ccc;padding:4px;vertical-align:top}"
            "img{image-rendering:pixelated;max-width:320px}</style>",
            f"<h1>Golden report</h1><p>tolerance {args.tolerance} channel value; "
            f"{len(rows)} goldens; {len(failures)} failed</p>"]
    if args.accept:
        page.append(f"<p>Accepted {len(accepted)} goldens: {html.escape(args.reason)}</p>")
    page.append("<table><tr><th>Golden</th><th>Status</th><th>Golden image</th>"
                "<th>Candidate</th><th>Diff</th><th>10.6 crop</th></tr>")
    report_rel = args.report.resolve()
    for row in rows:
        relative = Path(row["key"])
        golden_link = rel((args.goldens / relative).resolve(), report_rel)
        candidate_link = rel((args.candidate / relative).resolve(), report_rel)
        diff_link = rel((args.report / "diff" / relative).resolve(), report_rel)
        crop_cell = "—"
        if row["crop"]:
            crop_cell = html.escape(row["crop"])
            if row["distance"] is not None:
                crop_cell += f"<br>mean |d| {row['distance']:.1f}"
        page.append(
            "<tr>"
            f"<td>{html.escape(row['key'])}</td>"
            f"<td>{html.escape(row['status'])}<br>{html.escape(row['detail'])}</td>"
            f"<td><img src='{golden_link}'></td>"
            f"<td><img src='{candidate_link}'></td>"
            f"<td><img src='{diff_link}'></td>"
            f"<td>{crop_cell}</td>"
            "</tr>")
    page.append("</table>")
    (args.report / "index.html").write_text("\n".join(page), encoding="utf-8")

    print(f"{len(rows)} goldens, {len(failures)} failed; report {args.report / 'index.html'}")
    if failures and not args.accept:
        for key in failures:
            print("FAIL", key)
        raise SystemExit(1)


if __name__ == "__main__":
    main()
