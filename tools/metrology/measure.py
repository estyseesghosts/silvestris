"""Reproducible measurements of Mac OS X 10.6 captures for Silvestris Aqua tokens.

Run: python tools/metrology/measure.py > reference/measurements.json

Every measurement names its capture (with SHA-256), the sampled pixels in
capture coordinates, and the sampled sRGB values. The Rust tokens in
crates/silvestris-core/src/aqua.rs cite these IDs. Coordinates were chosen by
inspecting 4x nearest-neighbour crops (tools/metrology/zoom.py) for runs of
pixels free of text and icons.

Captures named 10-6-Snow-Leopard-* are 1x window captures with alpha (window
shadow included). "Screenshot 2026-11-18 220259.png" is a scaled (about
1.4875x) photo of a 10.6.8 (10K549) desktop; only flat colours are taken from
it, never geometry.
"""
import json
import sys

from corpus import hexc, load, opaque_bbox

FINDER = "10-6-Snow-Leopard-Finder-Home.png"
ALERT = "10-6-Snow-Leopard-Finder-Copy-Replace.png"
DESKTOP = "10-6-Snow-Leopard-System-Preferences-Desktop-and-Screen-Saver.png"
PREFS = "10-6-Snow-Leopard-System-Preferences-0.png"
ACTIVITY = "10-6-Snow-Leopard-Activity-Monitor.png"
MENU = "10-6-Snow-Leopard-Finder-Menu.png"
FORCE_QUIT = "10-6-Snow-Leopard-Force-Quit.png"
GOTO = "10-6-Snow-Leopard-Finder-Go-To.png"
DESKTOP_1068 = "Screenshot 2026-11-18 220259.png"

out = []


def column(file, x, y0, y1, ident, what, origin=(0, 0)):
    rgb, _, sha = load(file)
    ox, oy = origin
    values = [hexc(rgb[oy + y, ox + x]) for y in range(y0, y1)]
    out.append({
        "id": ident, "what": what, "capture": file, "sha256": sha,
        "kind": "column", "x": ox + x, "y": [oy + y0, oy + y1 - 1], "srgb": values,
    })


def row(file, y, x0, x1, ident, what, origin=(0, 0)):
    rgb, _, sha = load(file)
    ox, oy = origin
    values = [hexc(rgb[oy + y, ox + x]) for x in range(x0, x1)]
    out.append({
        "id": ident, "what": what, "capture": file, "sha256": sha,
        "kind": "row", "y": oy + y, "x": [ox + x0, ox + x1 - 1], "srgb": values,
    })


def alpha_profile(file, ident, what, points):
    _, alpha, sha = load(file)
    values = [[x, y, int(alpha[y, x])] for x, y in points]
    out.append({"id": ident, "what": what, "capture": file, "sha256": sha,
                "kind": "alpha", "samples": values})


def note(ident, what, file, value):
    _, _, sha = load(file)
    out.append({"id": ident, "what": what, "capture": file, "sha256": sha,
                "kind": "geometry", "value": value})


# --- Window silhouette and shadow (Finder, 1x) -----------------------------
_, falpha, _ = load(FINDER)
fx0, fy0, fx1, fy1 = opaque_bbox(falpha)
note("win.finder.bounds", "opaque window bounds inside the shadowed capture", FINDER,
     {"x0": fx0, "y0": fy0, "x1": fx1, "y1": fy1})
alpha_profile(FINDER, "win.corner.top_left", "alpha of the 4x4 top-left corner; opaque from the 5th pixel",
              [(fx0 + x, fy0 + y) for y in range(5) for x in range(5)])
alpha_profile(FINDER, "win.corner.bottom_left", "alpha of the 4x4 bottom-left corner",
              [(fx0 + x, fy1 - y) for y in range(5) for x in range(5)])
alpha_profile(FINDER, "win.shadow.left", "shadow alpha leaving the left edge at mid height, outward",
              [(fx0 - i, (fy0 + fy1) // 2) for i in range(1, 26)])
alpha_profile(FINDER, "win.shadow.top", "shadow alpha above the top edge at mid width, outward",
              [((fx0 + fx1) // 2, fy0 - i) for i in range(1, 26)])
alpha_profile(FINDER, "win.shadow.bottom", "shadow alpha below the bottom edge at mid width, outward",
              [((fx0 + fx1) // 2, fy1 + i) for i in range(1, 41)])

FO = (fx0, fy0)  # window-local origin for Finder samples

# --- Unified title bar + toolbar ------------------------------------------
column(FINDER, 600, 0, 56, "chrome.unified.active",
       "title+toolbar, blank column: row 0 highlight, rows 1-52 gradient, row 53 separator", FO)
column(ALERT, 300, 0, 24, "chrome.titlebar.active",
       "title bar without toolbar (alert): 22 px including the separator row", (40, 25))
column(DESKTOP_1068, 800, 73, 152, "chrome.unified.inactive.scaled",
       "inactive Utilities window title+toolbar, 10.6.8 capture at ~1.4875x; colours only")
column(DESKTOP_1068, 1650, 61, 100, "chrome.titlebar.active.scaled",
       "active Terminal title bar in the same 10.6.8 capture, for cross-checking active colours")

# --- Traffic lights ---------------------------------------------------------
for name, x in (("close", 15), ("minimize", 36), ("zoom", 57)):
    column(FINDER, x, 2, 20, f"traffic.{name}.column", f"{name} button vertical centre line", FO)
row(FINDER, 11, 4, 66, "traffic.row", "horizontal line through all three buttons", FO)

# --- Toolbar controls -------------------------------------------------------
column(FINDER, 13, 22, 48, "toolbar.textured.normal",
       "textured rounded button (Back), clean column: border, fill, border, emboss", FO)
column(FINDER, 74, 22, 48, "toolbar.textured.selected",
       "selected segment of the textured view selector", FO)
row(FINDER, 35, 5, 75, "toolbar.textured.row",
    "row through Back/Forward pair and the view selector: segment seams", FO)
column(FINDER, 800, 18, 50, "toolbar.search.column", "toolbar search field, unfocused", FO)
row(FINDER, 35, 630, 660, "toolbar.search.left", "search field left cap and magnifier", FO)

# --- Source list ------------------------------------------------------------
column(FINDER, 120, 53, 60, "sidebar.background", "sidebar fill under the toolbar separator", FO)
column(FINDER, 125, 306, 336, "sidebar.selection.active", "selected row 'shackett', key window", FO)
row(FINDER, 150, 128, 140, "sidebar.divider", "sidebar / content divider", FO)
row(FINDER, 66, 4, 70, "sidebar.header.text", "DEVICES heading glyph row", FO)
note("sidebar.row_pitch", "source-list row pitch from Desktop (y=318) to shackett (y=338) label baselines",
     FINDER, {"pitch_px": 20})

# --- Bottom bar --------------------------------------------------------------
column(FINDER, 300, 640, 671, "bottombar.column", "content, separator, bottom bar gradient", FO)

# --- Body controls -----------------------------------------------------------
column(ALERT, 270, 112, 138, "button.push.normal",
       "small push button 'Stop' (Copy panel uses small controls, see button.push.label_widths), clean column")
column(FORCE_QUIT, 320, 264, 293, "button.push.default",
       "regular default push button 'Force Quit', text-free column between the words")
note("button.push.label_widths",
     "label ink widths: 'Stop' 22 px (cols 278-299) vs 'Force Quit' 63 px (cols 285-347); "
     "13 pt Lucida Grande sets 'Stop' near 29 px, so the Copy panel's buttons are the small size",
     ALERT, {"stop_px": 22, "force_quit_px": 63})
note("button.push.widths",
     "bezel border-to-border width and label ink inside it, ink = pixels 50 below the row median "
     "in the label band: Stop 246-331 ink 277-299; Replace 342-427 ink 365-404 (both 86 px, an "
     "equal-width pair); Force Quit 267-365 (99 px) ink 284-347, so 16 px left / 17 px right",
     FORCE_QUIT, {"stop": [246, 331, 277, 299], "replace": [342, 427, 365, 404],
                  "force_quit": [267, 365, 284, 347]})
note("button.push.label_rows",
     "label ink rows against the bezel border rows (same ink rule as button.push.widths): "
     "Force Quit (regular) border 268-288, ink 273-284; Stop (small) border 116-133, ink 121-130; "
     "Replace border 116-133, ink 120-130",
     FORCE_QUIT, {"force_quit": [268, 288, 273, 284], "stop": [116, 133, 121, 130],
                  "replace": [116, 133, 120, 130]})
# --- Resize grip (size control, HIG PDF p. 221) ------------------------------
_, falpha_, _ = load(FINDER); gx0, gy0, gx1, gy1 = opaque_bbox(falpha_)
row(FINDER, gy1 - 4, gx1 - 16, gx1 + 1, "grip.bar.row",
    "bottom row of the bottom-bar grip, 4 rows above the window's last row: three [shade, line, highlight] triples")
column(FINDER, gx1 - 4, gy1 - 15, gy1 + 1, "grip.bar.column",
       "right column of the bottom-bar grip, 4 columns in from the window's last column")
_, aalpha_, _ = load(ACTIVITY); ax0, ay0, ax1, ay1 = opaque_bbox(aalpha_)
row(ACTIVITY, ay1 - 1, ax1 - 16, ax1 + 1, "grip.body.row",
    "bottom row of the window-body grip, directly above the window's bottom edge row")
column(ACTIVITY, ax1 - 1, ay1 - 12, ay1 + 1, "grip.body.column", "right column of the window-body grip")
note("grip.geometry", "three 45-degree lines of 10, 6 and 2 line pixels, 4 px apart along the bottom row, "
     "upper ends in one column; bar grip inset 4 from right/bottom, body grip inset 1",
     FINDER, {"lengths": [10, 6, 2], "pitch": 4, "inset_bar": 4, "inset_body": 1})
# --- Text field with text and caret (Go to Folder, window at 40,25) --------
column(GOTO, 117, 64, 92, "field.caret.column",
       "caret column: border row 67, inner rows 68-69, caret 00 on rows 70-86, border 88", origin=(40, 25))
row(GOTO, 79, 14, 30, "field.text.row",
    "left edge through the text: ring 17-19, border 20, fill 21-24, first ink (~) at 25", origin=(40, 25))
note("field.geometry", "focused field 22 rows border to border (67-88); text ink starts 5 px right of the "
     "left border; caret 1 px wide, black, from 3 rows under the top border to 2 rows over the bottom",
     GOTO, {"border_rows": [67, 88], "left_border_x": 20, "first_ink_x": 25, "caret_rows": [70, 86]})
row(ALERT, 125, 240, 262, "button.push.left_cap", "left cap of 'Stop' at its vertical middle")
column(DESKTOP, 60, 90, 100, "window.body", "System Preferences window body")
column(DESKTOP, 70, 195, 205, "groupbox.fill", "group box fill")
column(DESKTOP, 620, 44, 74, "focus.ring.search", "focused search field: ring above and below")
column(DESKTOP, 248, 490, 510, "checkbox.off", "unchecked checkbox, centre column")
row(DESKTOP, 499, 238, 260, "checkbox.off.row", "unchecked checkbox, centre row")
column(DESKTOP, 248, 510, 530, "checkbox.off.disabled", "disabled unchecked checkbox")
column(DESKTOP, 248, 530, 550, "checkbox.on", "checked checkbox, centre column")
row(DESKTOP, 541, 238, 260, "checkbox.on.row", "checked checkbox, row below the mark")
column(DESKTOP, 450, 486, 514, "popup.disabled", "disabled pop-up button body")
row(DESKTOP, 499, 560, 584, "popup.disabled.right", "pop-up right cap with the double arrow")
column(DESKTOP, 320, 100, 126, "segmented.selected", "selected 'Desktop' tab segment")
column(DESKTOP, 420, 100, 126, "segmented.normal", "unselected 'Screen Saver' segment (crosses text)")
row(DESKTOP, 113, 280, 300, "segmented.left_cap", "left cap of the selected segment")

column(DESKTOP, 70, 110, 120, "groupbox.top", "tab-view box top edge: #ababab line, then a soft inner row")
row(DESKTOP, 160, 58, 64, "groupbox.left", "box left edge: #c8c8c8 line then #e2e2e2 inner row")
column(DESKTOP, 70, 576, 586, "groupbox.bottom", "box bottom edge: #e4e4e4 inner row, then a #dbdbdb edge lighter than the sides")
note("prefs.pane.geometry", "pane box 20 px from the window sides and bottom; its top line 34 px below "
     "the toolbar separator; the 22 px tab control is centred on the window and straddles the top line",
     DESKTOP, {"box_x": [60, 687], "box_y": [113, 582], "window_x": [40, 707], "window_bottom": 602,
               "toolbar_separator_y": 78, "tabs_x": [283, 464], "tabs_y": [103, 124]})
column(PREFS, 5, 76, 290, "prefs.overview.bands", "overview category bands: #ededed / #e6e6e6 alternate, "
       "#d8d8d8 separator row between bands", origin=(40, 0))
note("prefs.overview.geometry", "8 icon cells of 81 px (icon 32 px, top 32 px below the band top); "
     "bold heading ink 9-19 px below the band top at x+14; label ink from 68 px below the band top, "
     "two lines when wrapped; bands about 101 px plus a 1 px separator",
     PREFS, {"window_x": [40, 707], "band_tops": [79, 183, 283, 385], "icon_x": [[70, 101], [151, 182], [232, 263], [312, 343]],
             "icon_y": [111, 142], "heading_ink_y": [88, 98], "label_ink_y": [147, 157]})

# --- Scroll bar -----------------------------------------------------------------
row(DESKTOP, 300, 658, 680, "scroller.thumb.row", "vertical thumb, horizontal cross-section")
column(DESKTOP, 669, 226, 250, "scroller.thumb.top", "track top and thumb top cap")
column(DESKTOP, 669, 370, 385, "scroller.thumb.bottom", "thumb bottom cap and track")
row(DESKTOP, 420, 658, 680, "scroller.track.row", "empty track cross-section")
column(DESKTOP, 669, 440, 482, "scroller.arrows", "track end and the paired up/down arrows")

# --- Table -------------------------------------------------------------------------
column(ACTIVITY, 20, 76, 140, "table.header_and_rows",
       "separator, column header, header border, alternating rows", (40, 25))
row(ACTIVITY, 85, 45, 60, "table.header.divider", "column header divider", (40, 25))

# --- Menus ---------------------------------------------------------------------------
column(MENU, 60, 0, 50, "menu.bar_and_selection", "menu bar title selection and first item selection")
column(MENU, 230, 20, 140, "menu.body", "menu body, selected item, separators")

# Contextual and pop-down menus (VM captures, 1x). PopupMenu draws these kinds.
NESTED = "../vm-screenshots/right-click-context-menu-nested.png"
CONTEXT = "../vm-screenshots/expanded-contextual-menu.png"
column(NESTED, 246, 50, 214, "menu.context.rows",
       "pop-down menu: body from row 54, 4 px pad, 18 px items, 12 px separators (line on the 6th row), bottom edge 212")
column(CONTEXT, 200, 58, 140, "menu.context.body",
       "contextual menu over the desktop: translucent body, 4 px pad, separator, 18 px items")
row(NESTED, 99, 70, 256, "menu.context.separator", "separator row: 1 px inset each side, body edge, parent edge")
row(NESTED, 230, 250, 400, "menu.context.edge", "submenu at mid height: left shadow, left edge, body, right edge, right shadow")
column(NESTED, 320, 150, 300, "menu.context.shadow", "submenu at mid width: shadow above, top edge, items, bottom edge, shadow below")
row(NESTED, 167, 250, 390, "menu.context.corners",
    "submenu top edge: square top-left corner beside the parent, rounded top-right")
column(NESTED, 383, 160, 180, "menu.context.corner_column", "submenu right edge entering the top-right curve")
row(NESTED, 181, 225, 256, "menu.context.arrow", "highlighted Arrange By row through the submenu arrow tip and the submenu edge")
row(MENU, 147, 140, 250, "menu.bar.arrow_and_keys", "Services row: submenu arrow 19 px from the body edge")
row(MENU, 66, 140, 250, "menu.bar.key_column", "Preferences row: command glyph 212-221, key 225 onward, body edge 247")

# Finder list view with both scroll bars and the corner (VM capture, 1x).
FINDER_LIST = "../vm-screenshots/finder.png"
column(FINDER_LIST, 715, 404, 426, "scroller.horizontal",
       "horizontal track: the same 15 rows as the vertical track, top to bottom")
row(FINDER_LIST, 415, 690, 785, "scroller.cap",
    "horizontal bar row 7: track, 12 cap rows, arrow boxes, glyphs, divider, 6 end rows, corner")
column(FINDER_LIST, 775, 355, 425, "scroller.cap.vertical",
       "vertical bar column 8: cap rows, boxes, divider, end rows, corner")
row(FINDER_LIST, 388, 760, 785, "scroller.boxes", "arrow box cross-section, no glyph")
column(FINDER_LIST, 750, 404, 426, "scroller.boxes.divider", "divider between the horizontal boxes")
row(FINDER_LIST, 200, 760, 785, "scroller.thumb.finder", "vertical thumb cross-section")
column(FINDER_LIST, 772, 400, 425, "scroller.corner", "corner square between the bars: white")

# Finder list view, sorted by Name (VM capture, 1x).
LIST_SORTED = "../vm-screenshots/Screen shot 2026-11-23 at 3.56.49 AM.png"
column(LIST_SORTED, 420, 74, 130, "table.header.sorted",
       "sort column header: 15 body rows, bottom line, then 19 px rows white / edf3fe")
column(LIST_SORTED, 600, 74, 92, "table.header.plain", "unsorted header column, c0c0c0 bottom line")
row(LIST_SORTED, 82, 528, 566, "table.header.sort_arrow",
    "7 x 6 px triangle centred 7 px before the sorted column's 72a1d4 divider at 546")
column(LIST_SORTED, 727, 74, 92, "table.header.divider", "plain header divider: c0c0c0, full height")

# --- Column browser (Open panel, VM capture, 1x) ---------------------------
OPEN_PANEL = "../vm-screenshots/Screen shot 2026-11-23 at 4.15.47 AM.png"
row(OPEN_PANEL, 120, 377, 563, "browser.columns",
    "first browser column: 171 px white content (377-547), then the 15 px scroller track "
    "(548-562); the next column starts at 563, so the pitch is 186 px")
row(OPEN_PANEL, 120, 548, 563, "browser.divider",
    "column divider is the empty scroller track: dc leading edge, e0..f5 gradient, fd centre, "
    "f7 trailing edge")
column(OPEN_PANEL, 554, 377, 384, "browser.grip",
    "left resize line: 5a for 7 rows, 5 rows above the browser's bottom edge")
row(OPEN_PANEL, 109, 226, 233, "browser.arrow",
    "sidebar branch triangle, row 2 of 6: 5 px wide (227-231), 9da8b3")
note("browser.geometry",
     "Open panel column browser: 186 px pitch = 171 px content + 15 px scroller; rows 19 px; "
     "branch triangle 7 x 6 px (rows 108-113, 7, 5, 5, 3, 3, 1 px wide); resize grip is two "
     "1 px lines 3 px apart (x 554 and 557), 7 rows tall, starting 5 rows above the browser "
     "bottom (377 against bottom 388); column divider dc",
     OPEN_PANEL,
     {"column_px": [377, 548], "pitch_px": 186, "content_px": 171, "scroller_px": 15,
      "arrow_px": [7, 6], "arrow_x": [226, 232], "arrow_y": [108, 113],
      "grip_x": [554, 557], "grip_y": [377, 383], "grip_inset_bottom": 5,
      "divider": "dc"})

json.dump({
    "tool": "tools/metrology/measure.py",
    "colour_space": "sRGB (converted from each capture's embedded ICC profile with LittleCMS)",
    "measurements": out,
}, sys.stdout, indent=1)
sys.stdout.write("\n")
