"""Print the naming and weight facts the font manifest records.

Reads the SFNT `name` (IDs 1, 2, 4, 5, 6, 16, 17) and `OS/2` (usWeightClass,
fsSelection italic bit) tables directly, so no third-party package is needed.
Usage: python -I sfnt_names.py FILE...   (prints one JSON object per file)
"""
import hashlib
import json
import struct
import sys

WANT = {1: "family", 2: "subfamily", 4: "full", 5: "version", 6: "postscript",
        16: "typographic_family", 17: "typographic_subfamily", 0: "copyright"}


def tables(data):
    num = struct.unpack(">H", data[4:6])[0]
    out = {}
    for i in range(num):
        tag, _, off, length = struct.unpack(">4sIII", data[12 + 16 * i: 28 + 16 * i])
        out[tag.decode("latin-1")] = data[off: off + length]
    return out


def names(t):
    _, count, strings = struct.unpack(">HHH", t[:6])
    found = {}
    for i in range(count):
        pid, eid, lid, nid, length, off = struct.unpack(">HHHHHH", t[6 + 12 * i: 18 + 12 * i])
        if nid not in WANT:
            continue
        raw = t[strings + off: strings + off + length]
        # Prefer Windows Unicode English (3,1,0x409); fall back to Mac Roman.
        if pid == 3 and eid in (1, 10) and lid == 0x409:
            found[WANT[nid]] = raw.decode("utf-16-be")
        elif pid == 1 and eid == 0 and WANT[nid] not in found:
            found[WANT[nid]] = raw.decode("mac-roman")
    return found


def main(paths):
    for p in paths:
        data = open(p, "rb").read()
        t = tables(data)
        rec = {"file": p.replace("\\", "/").split("/")[-1],
               "sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data)}
        rec.update(names(t["name"]))
        os2 = t["OS/2"]
        rec["weight"] = struct.unpack(">H", os2[4:6])[0]
        rec["italic"] = bool(struct.unpack(">H", os2[62:64])[0] & 1)
        rec["variable"] = "fvar" in t
        print(json.dumps(rec, ensure_ascii=False))


if __name__ == "__main__":
    main(sys.argv[1:])
