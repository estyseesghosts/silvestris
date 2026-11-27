"""Generate prepared Geist pictogram polylines from pinned Lucide SVGs.

The renderer uses cached vector geometry rather than parsing SVG at paint
time. Coordinates are in Lucide's 24x24 viewBox; the caller scales them and
strokes them with a 2 px nominal line at 24 px source size.

    python -I tools/icons/geist_pictograms.py --check
    python -I tools/icons/geist_pictograms.py
"""
import math
import pathlib
import re
import sys
import xml.etree.ElementTree as ET

ROOT = pathlib.Path(__file__).resolve().parents[2]
SOURCE = ROOT / "crates" / "silvestris" / "icons" / "lucide"
OUTPUT = ROOT / "crates" / "silvestris" / "src" / "geist_pictograms.rs"
RELEASE = "0.468.0"
ARCHIVE_SHA256 = "2fe6e35a57404dab89adaddd6df49ec60d13bba3d6caee5d8805f885079a1c57"

ICONS = [
    ("settings", "settings", "General settings"),
    ("database", "database", "Storage"),
    ("message-square", "message-square", "Messages/chat"),
    ("monitor", "monitor", "Display"),
    ("volume-2", "volume-2", "Audio"),
    ("gamepad-2", "gamepad-2", "Controllers/input"),
    ("bug", "bug", "Debugging"),
    ("bot", "bot", "Automation"),
    ("wrench", "wrench", "Advanced tools"),
    ("sliders-horizontal", "sliders-horizontal", "Preferences/options"),
]

NUMBER = r"[-+]?(?:\d*\.\d+|\d+\.?\d*)(?:[eE][-+]?\d+)?"
GROUP = re.compile(r"([MmLlHhVvCcSsQqTtAaZz])([^MmLlHhVvCcSsQqTtAaZz]*)")
VALUES = re.compile(NUMBER)


def numbers(text):
    return [float(match.group()) for match in VALUES.finditer(text)]


def cubic(p0, p1, p2, p3, steps=12):
    out = []
    for step in range(1, steps + 1):
        t = step / steps
        u = 1 - t
        x = (u**3 * p0[0] + 3 * u * u * t * p1[0] + 3 * u * t * t * p2[0] + t**3 * p3[0])
        y = (u**3 * p0[1] + 3 * u * u * t * p1[1] + 3 * u * t * t * p2[1] + t**3 * p3[1])
        out.append((x, y))
    return out


def quadratic(p0, p1, p2, steps=10):
    out = []
    for step in range(1, steps + 1):
        t = step / steps
        u = 1 - t
        out.append((u * u * p0[0] + 2 * u * t * p1[0] + t * t * p2[0],
                    u * u * p0[1] + 2 * u * t * p1[1] + t * t * p2[1]))
    return out


def arc(start, rx, ry, phi, large, sweep, end, steps=12):
    """Endpoint-to-centre conversion, then sampled points (SVG implementation notes)."""
    x1, y1 = start
    x2, y2 = end
    rx, ry = abs(rx), abs(ry)
    if rx == 0 or ry == 0:
        return [end]
    angle = math.radians(phi)
    cos_phi, sin_phi = math.cos(angle), math.sin(angle)
    half_x, half_y = (x1 - x2) / 2.0, (y1 - y2) / 2.0
    x1p = cos_phi * half_x + sin_phi * half_y
    y1p = -sin_phi * half_x + cos_phi * half_y
    scale = (x1p / rx) ** 2 + (y1p / ry) ** 2
    if scale > 1.0:
        root = math.sqrt(scale)
        rx *= root
        ry *= root
    numerator = rx * rx * ry * ry - rx * rx * y1p * y1p - ry * ry * x1p * x1p
    denominator = rx * rx * y1p * y1p + ry * ry * x1p * x1p
    ratio = math.sqrt(max(0.0, numerator / denominator)) if denominator else 0.0
    sign = -1.0 if large == sweep else 1.0
    cxp = sign * ratio * rx * y1p / ry
    cyp = sign * ratio * -ry * x1p / rx
    cx = cos_phi * cxp - sin_phi * cyp + (x1 + x2) / 2.0
    cy = sin_phi * cxp + cos_phi * cyp + (y1 + y2) / 2.0
    theta1 = math.atan2((y1p - cyp) / ry, (x1p - cxp) / rx)
    theta2 = math.atan2((-y1p - cyp) / ry, (-x1p - cxp) / rx)
    delta = theta2 - theta1
    if not sweep and delta > 0:
        delta -= 2 * math.pi
    if sweep and delta < 0:
        delta += 2 * math.pi
    out = []
    for step in range(1, steps + 1):
        theta = theta1 + delta * step / steps
        dx, dy = rx * math.cos(theta), ry * math.sin(theta)
        out.append((cx + cos_phi * dx - sin_phi * dy,
                    cy + sin_phi * dx + cos_phi * dy))
    return out


def path_points(data):
    subpaths = []
    current = (0.0, 0.0)
    start = (0.0, 0.0)
    points = []
    previous_control = None
    previous_command = None

    def flush():
        nonlocal points
        if points:
            subpaths.append(points)
            points = []

    for command, text in GROUP.findall(data):
        values = numbers(text)
        index = 0

        def take(count):
            nonlocal index
            chunk = values[index:index + count]
            index += len(chunk)
            return chunk

        relative = command.islower()
        operation = command.upper()
        while index < len(values):
            if operation == "M":
                x, y = take(2)
                target = (current[0] + x, current[1] + y) if relative else (x, y)
                flush()
                current = start = target
                points.append(target)
                # Subsequent pairs are line-to; `relative` still applies.
                operation = "L"
            elif operation == "L":
                x, y = take(2)
                current = (current[0] + x, current[1] + y) if relative else (x, y)
                points.append(current)
            elif operation == "H":
                (x,) = take(1)
                current = (current[0] + x, current[1]) if relative else (x, current[1])
                points.append(current)
            elif operation == "V":
                (y,) = take(1)
                current = (current[0], current[1] + y) if relative else (current[0], y)
                points.append(current)
            elif operation in ("C", "S"):
                if operation == "C":
                    x1, y1, x2, y2, x, y = take(6)
                    control1 = (current[0] + x1, current[1] + y1) if relative else (x1, y1)
                else:
                    x2, y2, x, y = take(4)
                    control1 = previous_control or current
                    if previous_command in ("C", "S") and previous_control:
                        control1 = (2 * current[0] - previous_control[0],
                                    2 * current[1] - previous_control[1])
                control2 = (current[0] + x2, current[1] + y2) if relative else (x2, y2)
                end = (current[0] + x, current[1] + y) if relative else (x, y)
                points.extend(cubic(current, control1, control2, end))
                previous_control = control2
                current = end
            elif operation in ("Q", "T"):
                if operation == "Q":
                    x1, y1, x, y = take(4)
                    control = (current[0] + x1, current[1] + y1) if relative else (x1, y1)
                else:
                    x, y = take(2)
                    control = previous_control or current
                    if previous_command in ("Q", "T") and previous_control:
                        control = (2 * current[0] - previous_control[0],
                                   2 * current[1] - previous_control[1])
                end = (current[0] + x, current[1] + y) if relative else (x, y)
                points.extend(quadratic(current, control, end))
                previous_control = control
                current = end
            elif operation == "A":
                rx, ry, angle, large, sweep, x, y = take(7)
                end = (current[0] + x, current[1] + y) if relative else (x, y)
                points.extend(arc(current, rx, ry, angle, bool(large), bool(sweep), end))
                previous_control = None
                current = end
            elif operation == "Z":
                flush()
                current = start
                previous_control = None
            previous_command = operation
            if operation == "Z":
                break
    flush()
    return subpaths


def element_points(element):
    tag = element.tag.split("}")[-1]
    if tag == "path":
        return path_points(element.get("d", ""))
    if tag == "line":
        line = [(float(element.get("x1", 0)), float(element.get("y1", 0))),
                (float(element.get("x2", 0)), float(element.get("y2", 0)))]
        return [line]
    if tag in ("polyline", "polygon"):
        tokens = [float(value) for value in element.get("points", "").replace(",", " ").split()]
        points = list(zip(tokens[::2], tokens[1::2]))
        return [points]
    if tag in ("circle", "ellipse"):
        cx = float(element.get("cx", 0))
        cy = float(element.get("cy", 0))
        rx = float(element.get("r", element.get("rx", 0)))
        ry = float(element.get("r", element.get("ry", 0)))
        loop = [(cx + rx * math.cos(2 * math.pi * i / 24),
                 cy + ry * math.sin(2 * math.pi * i / 24)) for i in range(25)]
        return [loop]
    if tag == "rect":
        x, y = float(element.get("x", 0)), float(element.get("y", 0))
        width, height = float(element.get("width", 0)), float(element.get("height", 0))
        return [[(x, y), (x + width, y), (x + width, y + height), (x, y + height), (x, y)]]
    return []


def fmt(value):
    text = f"{value:.2f}".rstrip("0").rstrip(".")
    if text in ("", "-0"):
        return "0.0"
    return text if "." in text else f"{text}.0"


def build():
    lines = [
        "// Generated from pinned Lucide release 0.468.0 by tools/icons/geist_pictograms.py.",
        f"// Archive SHA-256: {ARCHIVE_SHA256}. Do not edit by hand.",
        "// Lucide is ISC licensed; see icons/lucide/LICENSE.",
        "",
        "/// One stroked polyline in the 24x24 Lucide viewBox.",
        "#[derive(Clone, Copy)]",
        "pub struct PictogramPath {",
        "    pub points: &'static [(f32, f32)],",
        "}",
        "",
        "/// Prepared pictogram geometry for the ten generic Geist roles.",
        "// rustfmt must not rewrap the generated point lists: the --check gate",
        "// compares this file byte for byte with the generator output.",
        "#[rustfmt::skip]",
        "#[allow(clippy::approx_constant)]",
        "pub static GEIST_PICTOGRAMS: &[(&str, &str, &[PictogramPath])] = &[",
    ]
    for role, file_name, label in ICONS:
        root = ET.parse(SOURCE / f"{file_name}.svg").getroot()
        paths = []
        for child in root:
            for subpath in element_points(child):
                if len(subpath) >= 2:
                    paths.append(subpath)
        lines.append(f'    // {label} ({file_name}.svg)')
        lines.append(f'    ("{role}", "{label}", &[')
        for points in paths:
            formatted = ", ".join(f"({fmt(x)}, {fmt(y)})" for x, y in points)
            lines.append(f"        PictogramPath {{ points: &[{formatted}] }},")
        lines.append("    ]),")
    lines.append("];")
    lines.append("")
    return "\n".join(lines)


def main():
    text = build()
    if "--check" in sys.argv:
        if not OUTPUT.exists() or OUTPUT.read_text(encoding="utf-8") != text:
            sys.exit("geist_pictograms.rs does not match the pinned SVGs; regenerate it")
        print("ok")
    else:
        OUTPUT.write_text(text, encoding="utf-8", newline="\n")
        print(OUTPUT)


if __name__ == "__main__":
    main()
