"""Accept exactly one golden stem across every appearance/scale.

Useful when a change is scoped to one control and the surrounding goldens are
known to drift on this host. Regenerates `manifest.json` and `reasons.json`
entries only for the named files.
"""
import hashlib
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
GOLDENS = ROOT / "goldens" / "renders"
MANIFEST = ROOT / "goldens" / "manifest.json"
REASONS = ROOT / "goldens" / "reasons.json"


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


stems = set(sys.argv[1].split(","))
candidate_root = ROOT / sys.argv[2]
reason = sys.argv[3]

manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
reasons = json.loads(REASONS.read_text(encoding="utf-8"))

accepted = []
for golden in sorted(GOLDENS.rglob("*.png")):
    if golden.stem not in stems:
        continue
    relative = golden.relative_to(GOLDENS).as_posix()
    candidate = candidate_root / relative
    if not candidate.exists():
        print("missing candidate", relative)
        continue
    golden.write_bytes(candidate.read_bytes())
    manifest[relative] = sha256(golden)
    reasons[relative] = reason
    accepted.append(relative)

MANIFEST.write_text(json.dumps(dict(sorted(manifest.items())), indent=2) + "\n", encoding="utf-8")
REASONS.write_text(json.dumps(dict(sorted(reasons.items())), indent=2) + "\n", encoding="utf-8")
print(f"accepted {len(accepted)} files")
for a in accepted:
    print(" ", a)
