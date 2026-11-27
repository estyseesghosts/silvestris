"""Generate or check crates/silvestris-fonts/fonts/manifest.json.

The manifest records, per vendored face, the facts handbook 11 asks for:
release, archive URL and SHA-256, file SHA-256, internal names, weight,
style, license path, copyright and Reserved Font Name.

    python -I tools/fonts/manifest.py          # rewrite manifest.json
    python -I tools/fonts/manifest.py --check  # fail if files drifted
"""
import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
FONTS = os.path.join(ROOT, "crates", "silvestris-fonts", "fonts")
sys.path.insert(0, HERE)
import sfnt_names  # noqa: E402

RELEASES = {
    "lato": {
        "project": "https://github.com/latofonts/lato-source",
        "release": "Lato 2.015",
        "archive": "https://www.latofonts.com/files/Lato2OFL.zip",
        "archive_sha256": "42b54e96c07e299d967fc3227c7bd63a20d6cfb1dc8fd6dae83628091e20a5b8",
        "license": "lato/OFL.txt",
        "reserved_font_name": "Lato",
        "themes": ["aqua-light", "aqua-dark"], "role": "ui",
    },
    "source-code-pro": {
        "project": "https://github.com/adobe-fonts/source-code-pro",
        "release": "2.042R-u/1.062R-i/1.026R-vf",
        "archive": "https://github.com/adobe-fonts/source-code-pro/releases/download/2.042R-u/1.062R-i/1.026R-vf/TTF-source-code-pro-2.042R-u_1.062R-i.zip",
        "archive_sha256": "0c85bac90d15c040b82939aa92bc8404420fccc02e37bbcb9c93a7f21abb52c6",
        "license": "source-code-pro/LICENSE.md",
        "reserved_font_name": "Source",
        "themes": ["aqua-light", "aqua-dark"], "role": "mono",
    },
    "inter": {
        "project": "https://github.com/rsms/inter",
        "release": "v4.1",
        "archive": "https://github.com/rsms/inter/releases/download/v4.1/Inter-4.1.zip",
        "archive_sha256": "9883fdd4a49d4fb66bd8177ba6625ef9a64aa45899767dde3d36aa425756b11e",
        "license": "inter/LICENSE.txt",
        "reserved_font_name": None,
        "themes": ["geist-light", "geist-dark"], "role": "ui",
    },
    "geist-sans": {
        "project": "https://github.com/vercel/geist-font",
        "release": "v1.9.2",
        "archive": "https://github.com/vercel/geist-font/releases/download/v1.9.2/geist-font-v1.9.2.zip",
        "archive_sha256": "15f066d58dcb99fb10014a49913009fdaf41e464cfb8851a44b44e593c95d3d8",
        "license": "geist-sans/OFL.txt",
        "reserved_font_name": None,
        "themes": ["geist-light", "geist-dark"], "role": "ui",
    },
    "geist-mono": {
        "project": "https://github.com/vercel/geist-font",
        "release": "v1.9.2",
        "archive": "https://github.com/vercel/geist-font/releases/download/v1.9.2/geist-font-v1.9.2.zip",
        "archive_sha256": "15f066d58dcb99fb10014a49913009fdaf41e464cfb8851a44b44e593c95d3d8",
        "license": "geist-mono/OFL.txt",
        "reserved_font_name": None,
        "themes": ["geist-light", "geist-dark"], "role": "mono",
    },
}


def build():
    out = {"policy": "Fonts are compiled into the binary and registered with "
                     "GPUI TextSystem::add_fonts; nothing is fetched at runtime.",
           "families": []}
    for d, rel in RELEASES.items():
        faces = []
        for f in sorted(os.listdir(os.path.join(FONTS, d))):
            if not f.endswith(".ttf"):
                continue
            path = os.path.join(FONTS, d, f)
            rec = json.loads(_names(path))
            faces.append({
                "file": f"{d}/{f}",
                "sha256": rec["sha256"],
                "bytes": rec["bytes"],
                "family": rec.get("typographic_family", rec["family"]),
                "legacy_family": rec["family"],
                "style": rec.get("typographic_subfamily", rec["subfamily"]),
                "postscript": rec["postscript"],
                "version": rec["version"],
                "weight": rec["weight"],
                "italic": rec["italic"],
                "variable": rec["variable"],
                "copyright": rec.get("copyright", "").strip(),
            })
        out["families"].append({"dir": d, **rel, "faces": faces})
    return json.dumps(out, indent=2, ensure_ascii=False) + "\n"


def _names(path):
    import io
    import contextlib
    buf = io.StringIO()
    with contextlib.redirect_stdout(buf):
        sfnt_names.main([path])
    return buf.getvalue()


def main():
    target = os.path.join(FONTS, "manifest.json")
    text = build()
    if "--check" in sys.argv:
        current = open(target, encoding="utf-8").read()
        if current != text:
            sys.exit("manifest.json does not match the vendored fonts; rerun tools/fonts/manifest.py")
        print("ok")
    else:
        open(target, "w", encoding="utf-8", newline="\n").write(text)
        print(target)


if __name__ == "__main__":
    main()
