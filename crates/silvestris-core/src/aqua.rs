//! Aqua tokens.
//!
//! Aqua Light values are sampled from Mac OS X 10.6 captures; the comment on
//! each value names the measurement ID in reference/measurements.json (`M:`),
//! the 2009 HIG page via the hig-09 notes (`HIG p.`), or says `designed:`
//! where no capture shows that state yet (docs/themes.md lists them). Dark
//! Aqua is a new design (handbook 10 "Dark Aqua is invented") that keeps
//! Aqua Light's geometry, layer order and hue identities.

use crate::color::{Gradient, Rgba8};
use crate::theme::ThemeId;
use crate::tokens::*;

const fn c(rgb: u32) -> Rgba8 {
    Rgba8::hex(rgb)
}
const fn g(a: u32, b: u32) -> Gradient {
    Gradient::new(Rgba8::hex(a), Rgba8::hex(b))
}
const BLACK: Rgba8 = Rgba8::hex(0x000000);
const WHITE: Rgba8 = Rgba8::hex(0xffffff);

/// M: scroller.track.row: 15 rows across the track, leading edge first.
#[rustfmt::skip]
const SCROLLER_TRACK_ROWS: [Rgba8; 15] = [
    c(0xc5c5c5), c(0xcbcbcb), c(0xd2d2d2), c(0xdbdbdb), c(0xe2e2e2),
    c(0xe9e9e9), c(0xefefef), c(0xf4f4f4), c(0xf8f8f8), c(0xfbfbfb),
    c(0xfcfcfc), c(0xfdfdfd), c(0xfafafa), c(0xf6f6f6), c(0xf2f2f2),
];
/// M: scroller.boxes: 15 rows across an arrow box, leading edge first.
#[rustfmt::skip]
const SCROLLER_ARROW_ROWS: [Rgba8; 15] = [
    c(0xc7c7c7), c(0xdbdbdb), c(0xf5f5f5), c(0xf9f9f9), c(0xfafafa),
    c(0xfbfbfb), c(0xf8f8f8), c(0xe7e7e7), c(0xededed), c(0xf1f1f1),
    c(0xf6f6f6), c(0xfdfdfd), c(0xfefefe), c(0xfbfbfb), c(0xf2f2f2),
];

fn typography() -> Typography {
    Typography {
        // Fonts are the requested substitutes for Lucida Grande / Monaco (#118).
        ui: crate::fonts::LATO,
        mono: crate::fonts::SOURCE_CODE_PRO,
        // HIG p. 129 (hig-09 text.md): system 13, small 11, mini 9, view 12, label 10.
        size_regular: 13.0,
        size_small: 11.0,
        size_mini: 9.0,
        size_mono: 12.0,
        size_title: 13.0,
        size_heading: 11.0,
        size_menu: 13.0,
        size_view: 12.0,
        size_label: 10.0,
    }
}

fn metrics() -> Metrics {
    Metrics {
        // M: win.corner.top_left / bottom_left: alpha reaches 255 at the 5th pixel.
        window_radius: 4.0,
        // M: chrome.titlebar.active: 21 chrome rows + separator.
        titlebar_height: 22.0,
        // M: chrome.unified.active: 53 chrome rows + separator = 54, minus the title.
        toolbar_height: 32.0,
        // Activity Monitor icon-over-label toolbar: separator at row 70 (M: table.header_and_rows context).
        toolbar_labelled_height: 49.0,
        // HIG pp. 365-366: regular is 4 px + a 22 px regular control + 6 px;
        // small is 2 px + an 18 px small control + 2 px. The Finder Home capture
        // (`bottombar.column`) measures a status-only strip of 24 including its
        // separator, so the control-bearing standard bar is taller.
        bottom_bar_height: 32.0,
        bottom_bar_small_height: 22.0,
        // M: traffic.*.column / traffic.row: 14 px, left edge 8, top 4, pitch 21.
        traffic_diameter: 14.0,
        traffic_left: 8.0,
        traffic_top: 4.0,
        traffic_pitch: 21.0,
        // M: button.push.default (Force Quit, regular): 21 rows border to border.
        push_height: 21.0,
        // M: button.push.normal ('Stop' in the Copy panel): 18 rows; small size
        // per M: button.push.label_widths.
        push_small_height: 18.0,
        // designed: 10.6 mini push not captured.
        push_mini_height: 15.0,
        // Capsule ends; painters clamp to half the height.
        push_radius: 11.0,
        // M: installer Continue footprint is 31 x 30 px on the language
        // screen; the control uses a 30 px square.
        installer_continue_diameter: 30.0,
        // M: popup-button.regular: cap is 21 logical wide at 21 high. Small and
        // mini scale with the button height (designed; no 10.6 capture).
        popup_cap: [21.0, 18.0, 15.0],
        // M: button.push.widths: Force Quit (regular) has 16/17 px from border to ink.
        // designed: small and mini scale that by the label sizes (11/13, 9/13 pt,
        // HIG p. 268); the Copy panel's small buttons are equal-width (HIG PDF p. 359)
        // and so do not show a label-fitted padding.
        push_padding: [16.0, 14.0, 11.0],
        // M: button.push.label_rows: Force Quit ink starts 5 rows under the top
        // border and ends 4 above the bottom; centred Lato 13 lands at 6/3, so
        // regular moves up 1. Small (Stop 5/3, Replace 4/3) already matches the
        // reference with Lato 11. designed: mini, not captured, left centred.
        push_label_dy: [-1.0, 0.0, 0.0],
        // M: toolbar.textured.normal: 22 rows; corner from toolbar.textured.row.
        textured_height: 22.0,
        textured_radius: 4.0,
        // M: segmented.selected: 21 rows border to border.
        segment_height: 21.0,
        // M: segment corners are half-height capsules.
        segment_corner: CornerRule::Capsule,
        // HIG p. 322-325 (hig-09 controls.md): text fields 22/19/15; M: toolbar.search.column = 22.
        field_height: 22.0,
        // M: field.geometry and toolbar.search.column are capsule ends.
        field_corner: CornerRule::Capsule,
        search_corner: CornerRule::Capsule,
        field_small_height: 19.0,
        // M: field.geometry: first ink 5 px right of the border; with the bundled
        // font the text origin sits 4 px in (checked against a capture, like
        // push_label_dy). Caret 1 px, rows 3 under / 2 over the borders.
        field_text_inset: 4.0,
        caret_width: 1.0,
        caret_inset_top: 3.0,
        caret_inset_bottom: 2.0,
        // M: checkbox.off: 14 rows.
        checkbox_size: 14.0,
        // Estimated small/mini geometry and label gap; CMP-03 gives no box sizes.
        checkbox_small_size: 12.0,
        checkbox_mini_size: 10.0,
        checkbox_label_gap: 6.0,
        checkbox_radius: 2.0,
        // M: radio.selected: 14 logical bezel; small/mini scale by label size
        // (designed, not captured).
        radio_size: [14.0, 12.0, 10.0],
        // M: radio.selected: the dark dot is about 6 of 28 physical pixels.
        radio_dot_fraction: 0.24,
        radio_label_gap: 6.0,
        // M: Appearance pane radio rows sit 17 logical apart at 14 high.
        radio_row_gap: 3.0,
        // M: sidebar.row_pitch and sidebar.selection.active (20 rows incl. top line).
        source_row_height: 20.0,
        // designed: one disclosure width; Finder's own indent is not measured.
        source_indent: 14.0,
        // M: table.header_and_rows: 18 px rows, 17 px header + 1 px border.
        table_row_height: 18.0,
        table_header_height: 18.0,
        // M: menu.context.rows: 18 px items, 12 px separator blocks, 4 px pad
        // in both contextual captures. Menu-bar menus use 19 px items
        // (M: menu.body); Silvestris does not draw menu-bar menus.
        menu_item_height: 18.0,
        menu_separator_height: 12.0,
        menu_padding: 4.0,
        // M: menu.context.corners: the curve spans 5 rows and columns.
        menu_radius: 5.0,
        // M: scroller.track.row: 15 px bar + 1 px shared border; M:
        // scroller.horizontal: the horizontal bar is the same 15 rows. Small
        // and mini are estimates (no capture; docs/exceptions.md).
        scroller_width: [15.0, 11.0, 9.0],
        // M: focus.ring.search, each row solved against the unfocused field
        // (M: toolbar.search.column): 0.56 two rows out, 0.83 adjacent,
        // 0.55 on the border row, 0.28 one row inside; relative to the 0.83
        // peak in Colors::focus_ring. The capture's extra faint row below the
        // field (0.27) has no counterpart above and is not reproduced.
        focus_ring: &[(2.0, 0.67), (1.0, 1.0), (0.0, 0.66), (-1.0, 0.34)],
        // HIG p. 341-343: disclosure triangle 13x13; slider 19 regular; splitter hot zone 2 px.
        disclosure_size: 13.0,
        // M: disclosure.collapsed: 9x12 physical px -> 4.5x6 logical.
        disclosure_width: 4.5,
        disclosure_height: 6.0,
        slider_height: 19.0,
        // M: Dock size slider: 10 physical px track -> 5 logical.
        slider_track_height: [5.0, 4.0, 3.0],
        // M: Dock pointed thumb 15x18 logical; gallery round thumb 14x14.
        // Small/mini scale by the label sizes (designed).
        slider_thumb_width: [15.0, 13.0, 11.0],
        slider_thumb_height: [18.0, 15.0, 13.0],
        // M: Dock size slider, 2x capture: ticks rows 113-117 logical (5 high),
        // one row past the thumb tip at 111; labels 4 below (designed).
        slider_tick_height: 5.0,
        slider_tick_gap: 1.0,
        slider_tick_label_gap: 4.0,
        // M: installer/Software Update bars are 16 logical high.
        progress_height: [16.0, 12.0, 10.0],
        // designed: spinner sizes scale with the bar.
        spinner_size: [16.0, 13.0, 11.0],
        // M: menu.context.shadow: least-squares Gaussian fit to the submenu
        // side, bottom and top profiles over white, max error 1/255.
        menu_shadow_offset: [0.0, 10.0],
        menu_shadow_blur: 6.5,
        menu_shadow_spread: -2.0,
        // M: menu.context.rows: centred Lato 13 puts the baseline one row
        // below the 10.6 ink (Name: rows 4-12 of the item, Lato 4-13).
        menu_label_dy: -1.0,
        // M: groupbox.top/left: 1 px border plus a 1 px inner shadow row.
        well_inset: 1.0,
        splitter_hot_zone: 2.0,
        // HIG p. 261: 8 px between toolbar controls.
        control_spacing: 8.0,
        // HIG p. 338-341: 12 px label margin in tabs.
        tab_label_margin: 12.0,
        panel_radius: 5.0,
    }
}

fn traffic_light() -> TrafficLights {
    TrafficLights {
        // Colours per light are the sample points of tools/metrology/fit_traffic.py
        // (sphere row, col): ring_top (0,7), ring_bottom (13,7), deep (5,7),
        // glow (11,7), specular (2,7); Finder capture, sha256 b29c5f91ab6deb56.
        close: TrafficLight {
            ring_top: c(0x733a37),
            ring_bottom: c(0xcb9a98),
            deep: c(0xfa483e),
            glow: c(0xfebdbb),
            specular: c(0xf4dddd),
            glyph: c(0x4d0b06), // designed: hover glyph not captured
        },
        // M: traffic.minimize.column.
        minimize: TrafficLight {
            ring_top: c(0x9b3c33),
            ring_bottom: c(0xcdaa68),
            deep: c(0xf6bc60),
            glow: c(0xfee985),
            specular: c(0xf6e9db),
            glyph: c(0x7a4a00), // designed
        },
        // M: traffic.zoom.column.
        zoom: TrafficLight {
            ring_top: c(0x496d34),
            ring_bottom: c(0x95b06d),
            deep: c(0x90cc60),
            glow: c(0xcfe892),
            specular: c(0xd6e5cc),
            glyph: c(0x1f4a0a), // designed
        },
        // Disabled close of the Copy panel, same sample points (sha256 da11b66e3f19fb04);
        // the fitted shape reproduces it at MAE 4.81 without being fitted to it.
        idle: TrafficLight {
            ring_top: c(0x878787),
            ring_bottom: c(0xb5b5b5),
            deep: c(0xc4c4c4),
            glow: c(0xd2d2d2),
            specular: c(0xd5d5d5),
            glyph: c(0x555555),
        },
        // M: traffic.close.column row 18 (d8d8d8 over the c3c3c3 bar).
        emboss: WHITE.alpha(90),
        // fit_traffic.py: Nelder-Mead over close, minimize and zoom; mean
        // absolute error 7.32/255 (close 7.66, minimize 7.63, zoom 6.68).
        // spec_x is a fitted value that happens to be near 1/pi.
        #[allow(clippy::approx_constant)]
        shape: SphereShape {
            top_end: 0.218,
            top_k: 1.0,
            glow_x: 0.997,
            glow_y: 0.674,
            glow_ry: 0.858,
            glow_pow: 1.259,
            edge_start: 0.62,
            edge_k: 0.657,
            spec_x: 0.318,
            spec_y: -0.597,
            spec_ry: 0.212,
            spec_pow: 0.907,
            radius: 7.1 / 7.0,
            ring_width: 1.309,
            ring_pow: 1.982,
        },
    }
}

/// Aqua Light, measured.
/// Map each colour of a bevel (`f` on every gradient stop).
fn map_bevel(b: Bevel, f: impl Fn(Rgba8) -> Rgba8) -> Bevel {
    let gr = |x: Gradient| Gradient::new(f(x.from), f(x.to));
    Bevel {
        border: gr(b.border),
        upper: gr(b.upper),
        lower: gr(b.lower),
        ..b
    }
}

/// estimated: pressed push button. No 10.6 capture shows one; a post-10.6
/// capture (hig-09 "Screenshot 2026-11-19 020811.png", normal/default/pressed
/// push buttons, scaled) fits pressed = a * default + b per channel over the
/// bezel body, mean error 3.0/2.5/1.4 (R/G/B), and that map is applied to the
/// measured 10.6 default bevel.
fn aqua_pressed(c: Rgba8) -> Rgba8 {
    let f = |v: u8, a: f32, b: f32| (v as f32 * a + b).round().clamp(0.0, 255.0) as u8;
    Rgba8::rgba(
        f(c.r, 1.255, -97.0),
        f(c.g, 1.266, -101.4),
        f(c.b, 1.141, -52.1),
        c.a,
    )
}

pub fn aqua_light() -> Theme {
    let push_normal = Bevel {
        // M: button.push.normal: border 5e5e5e/696969, upper fc..ef (6 of 16 rows), lower e9..ff.
        border: g(0x5e5e5e, 0x696969),
        upper: g(0xfcfcfc, 0xefefef),
        lower: g(0xe9e9e9, 0xffffff),
        split: 6.0 / 16.0,
        // c9 over white and b0 over ededed (Force Quit) ≈ black at 24 %.
        emboss: BLACK.alpha(60),
        text: c(0x000000),
    };
    // Force Quit 'Force Quit': border 3540a2/616161, upper d2e5f8..a8d1f9 (7 of 19), lower 88c3f6..d7fdff.
    let push_default = Bevel {
        border: g(0x3540a2, 0x616161),
        upper: g(0xd2e5f8, 0xa8d1f9),
        lower: g(0x88c3f6, 0xd7fdff),
        split: 7.0 / 19.0,
        emboss: BLACK.alpha(60),
        text: c(0x000000),
    };
    // M: popup.disabled body and borders; text from checkbox.off.disabled label tone.
    let push_disabled = Bevel {
        border: g(0xb5b5b5, 0xa2a2a2),
        upper: g(0xf0f0f0, 0xe5e5e5),
        lower: g(0xdedede, 0xf2f2f2),
        split: 6.0 / 16.0,
        emboss: BLACK.alpha(26),
        text: c(0x9a9a9a),
    };
    let textured_normal = Bevel::plain(
        // M: toolbar.textured.normal: border 55/4d, fill fe..b8, emboss b9 over a9.
        g(0x555555, 0x4d4d4d),
        g(0xfefefe, 0xb8b8b8),
        WHITE.alpha(48),
        c(0x2b2b2b),
    );
    // M: toolbar.textured.selected: steep inner shadow (32..68 over 6 rows) then flat 6a..6c.
    let textured_selected = Bevel {
        border: g(0x555555, 0x4d4d4d),
        upper: g(0x323232, 0x686868),
        lower: g(0x6a6a6a, 0x6c6c6c),
        split: 6.0 / 20.0,
        emboss: WHITE.alpha(48),
        text: c(0xf2f2f2),
    };
    let textured_disabled = Bevel::plain(
        g(0x8a8a8a, 0x828282), // designed: borders at ~60 % contrast
        g(0xf2f2f2, 0xcfcfcf),
        WHITE.alpha(32),
        c(0x8f8f8f),
    );
    // M: segmented.normal ('Screen Saver'): border 77/5e, upper f9..f1, lower e6..ff.
    let segment_normal = Bevel {
        border: g(0x777777, 0x5e5e5e),
        upper: g(0xf9f9f9, 0xf1f1f1),
        lower: g(0xe6e6e6, 0xffffff),
        split: 8.0 / 19.0,
        emboss: BLACK.alpha(48),
        text: c(0x000000),
    };
    // M: segmented.selected ('Desktop'): border 353c97/5e5e5e, upper eaf2fb..aed2f1 (8 of 19), lower 7bb8eb..ccf9ff.
    let segment_selected = Bevel {
        border: g(0x353c97, 0x5e5e5e),
        upper: g(0xeaf2fb, 0xaed2f1),
        lower: g(0x7bb8eb, 0xccf9ff),
        split: 8.0 / 19.0,
        emboss: BLACK.alpha(48),
        text: c(0x000000),
    };
    // designed: pressed segment, one step darker along the same hue.
    let segment_pressed = Bevel {
        border: g(0x2a3080, 0x4a4a4a),
        upper: g(0xc3d9ef, 0x8db8e0),
        lower: g(0x5c9bd6, 0xa6dcf6),
        split: 8.0 / 19.0,
        emboss: BLACK.alpha(48),
        text: c(0x000000),
    };
    let checkbox_off = Bevel {
        // M: checkbox.off: border 83/5f, upper ff..f4 (5 of 12), lower e9..ff; emboss c2 over e6.
        border: g(0x838383, 0x5f5f5f),
        upper: g(0xffffff, 0xf4f4f4),
        lower: g(0xe9e9e9, 0xffffff),
        split: 5.0 / 12.0,
        emboss: BLACK.alpha(40),
        text: c(0x1c2d3d),
    };
    let checkbox_on = Bevel {
        // M: checkbox.on / checkbox.on.row: border 2e3893/5f5f5f, upper deebf7..86b4dc, lower 7db9ec..c8f7ff; mark 1c2d3d.
        border: g(0x2e3893, 0x5f5f5f),
        upper: g(0xdeebf7, 0x86b4dc),
        lower: g(0x7db9ec, 0xc8f7ff),
        split: 5.0 / 12.0,
        emboss: BLACK.alpha(40),
        text: c(0x1c2d3d),
    };
    let checkbox_disabled = Bevel {
        // M: checkbox.off.disabled.
        border: g(0xb4b4b4, 0xa2a2a2),
        upper: g(0xf1f1f1, 0xededed),
        lower: g(0xe7e7e7, 0xf1f1f1),
        split: 5.0 / 12.0,
        emboss: BLACK.alpha(20),
        text: c(0x9a9a9a),
    };
    let checkbox_pressed = Bevel {
        // designed: pressed checkbox not captured.
        border: g(0x5f5f5f, 0x4a4a4a),
        upper: g(0xd8d8d8, 0xcbcbcb),
        lower: g(0xc2c2c2, 0xdedede),
        split: 5.0 / 12.0,
        emboss: BLACK.alpha(40),
        text: c(0x1c2d3d),
    };
    // M: popup-button.regular body and cap rows at 2x. The body is a neutral
    // gloss with a taller top border than a push button; the cap is blue with
    // a dark blue top border and the same grey bottom border.
    let popup_body = Bevel {
        border: g(0x868686, 0x616161),
        upper: g(0xfbfbfb, 0xdfdfdf),
        lower: g(0xe0e0e0, 0xffffff),
        split: 9.0 / 19.0,
        emboss: BLACK.alpha(48),
        text: BLACK,
    };
    let popup_cap = Bevel {
        border: g(0x1d1e89, 0x3862b3),
        upper: g(0xdde5f3, 0xadcaec),
        lower: g(0x85b2e4, 0xdafefe),
        split: 8.0 / 19.0,
        emboss: BLACK.alpha(48),
        text: BLACK,
    };
    // M: popup.disabled body and cap; the cap keeps the same neutral material
    // and its arrows are grey (M: popup.disabled.right).
    let popup_disabled = Bevel {
        border: g(0xb5b5b5, 0xa2a2a2),
        upper: g(0xf2f2f2, 0xe5e5e5),
        lower: g(0xdedede, 0xf2f2f2),
        split: 9.0 / 19.0,
        emboss: BLACK.alpha(26),
        text: c(0x9a9a9a),
    };
    // designed: no 10.6 capture of a pressed pop-up; the pressed body darkens
    // the normal gloss, and the cap reuses the measured selected segment blue.
    let popup_body_pressed = Bevel {
        upper: g(0xe7e7e7, 0xc8c8c8),
        lower: g(0xc8c8c8, 0xededed),
        ..popup_body
    };
    let popup_cap_pressed = Bevel {
        border: g(0x2a3080, 0x4a4a4a),
        upper: g(0xc3d9ef, 0x8db8e0),
        lower: g(0x5c9bd6, 0xa6dcf6),
        split: 8.0 / 19.0,
        emboss: BLACK.alpha(48),
        text: BLACK,
    };
    // designed: 10.6's toolbar pop-up is a capsule of the textured toolbar
    // material, so body and cap share it with a seam.
    let popup_textured = PopUp {
        body: BevelStates {
            normal: textured_normal,
            pressed: textured_selected,
            disabled: textured_disabled,
            selected: textured_normal,
        },
        cap: BevelStates {
            normal: textured_normal,
            pressed: textured_selected,
            disabled: textured_disabled,
            selected: textured_normal,
        },
        divider: c(0x4d4d4d),
        arrow: c(0x2b2b2b),
    };
    // M: radio.selected and the unselected Appearance rows. The selected ring
    // is dark blue over a blue gloss, with a near-black dot; the unselected
    // ring is grey over white.
    let radio_normal = Bevel {
        border: g(0x858585, 0x5d5d5d),
        upper: g(0xffffff, 0xe9e9e9),
        lower: g(0xe9e9e9, 0xffffff),
        split: 6.0 / 12.0,
        emboss: BLACK.alpha(40),
        text: BLACK,
    };
    let radio_selected = Bevel {
        border: g(0x172a8a, 0x5d5d5d),
        upper: g(0xdde5f3, 0x86b2e4),
        lower: g(0x86b2e4, 0xdafefe),
        split: 6.0 / 12.0,
        emboss: BLACK.alpha(40),
        text: c(0x212d3b),
    };
    let radio_pressed = Bevel {
        // designed: pressed radio not captured; one step darker.
        upper: g(0xdedede, 0xc9c9c9),
        lower: g(0xc9c9c9, 0xdedede),
        ..radio_normal
    };
    // M: Dock size slider track: top border 68605d, fill 939190..b1b1b1.
    let slider_track = Bevel::plain(
        g(0x68605d, 0xa49e9d),
        g(0x939190, 0xb1b1b1),
        WHITE.alpha(40),
        BLACK,
    );
    // M: disabled Magnification track and thumb in the same Dock capture.
    let slider_track_disabled = Bevel::plain(
        g(0xb9b9b9, 0xd2d2d2),
        g(0xd7d7d7, 0xefefef),
        WHITE.alpha(24),
        c(0x9a9a9a),
    );
    // M: gallery slider round thumb: border 5e5e5e, fbfbfb..e7e7e7 then
    // ececec..fdfdfd. The pointed thumb reuses the measured pop-up cap blue
    // (M: Dock size slider 695d9a / d4e3f7 .. c2e8fd).
    let slider_round = Bevel {
        border: g(0x5e5e5e, 0x696969),
        upper: g(0xfbfbfb, 0xe7e7e7),
        lower: g(0xececec, 0xfdfdfd),
        split: 10.0 / 22.0,
        emboss: BLACK.alpha(60),
        text: BLACK,
    };
    let slider_pointed = Bevel {
        border: g(0x695d9a, 0x7a7a7a),
        upper: g(0xd4e3f7, 0x8bb8ec),
        lower: g(0x93bff0, 0xc2e8fd),
        split: 10.0 / 20.0,
        emboss: BLACK.alpha(48),
        text: BLACK,
    };
    // M: progress.determinate track rows d2..f3 with an a5 bottom border, and
    // the blue fill cap 383f94 / dcebf9..7bb8eb / 91caf0..c0effb.
    let progress_track = Bevel::plain(
        g(0xd2d2d2, 0xa4a4a4),
        g(0xd7d7d7, 0xf3f3f3),
        WHITE.alpha(48),
        BLACK,
    );
    let progress_fill = Bevel {
        border: g(0x383f94, 0x5c5d62),
        upper: g(0xdcebf9, 0x7bb8eb),
        lower: g(0x91caf0, 0xc0effb),
        split: 6.0 / 13.0,
        emboss: Rgba8::TRANSPARENT,
        text: BLACK,
    };
    let help_button = Help {
        // M: Dock pane help button (systempreferences-dock.png, rows
        // 660..701 at x 1306): ring 552b80 top to 9780ae bottom, gloss
        // fdfafd..ded0f7 over ded0f7..fef7fe split at the middle, black
        // glyph, and a drop shadow acacac on the ededed window (alpha 70).
        states: BevelStates {
            normal: Bevel {
                border: g(0x552b80, 0x9780ae),
                upper: g(0xfdfafd, 0xded0f7),
                lower: g(0xded0f7, 0xfef7fe),
                split: 0.5,
                emboss: BLACK.alpha(70),
                text: BLACK,
            },
            pressed: Bevel {
                upper: g(0xe1d3fa, 0xcdbbeb),
                lower: g(0xbdabdd, 0xe1d3fa),
                ..radio_pressed
            },
            disabled: push_disabled,
            selected: radio_selected,
        },
        glyph: BLACK,
    };
    let field = Field {
        // M: toolbar.search.column: 84 top, d8 inner shadow, white, e1 bottom.
        border_top: c(0x848484),
        border: c(0x9e9e9e), // designed: side tone between the measured top and bottom rows
        border_bottom: c(0xc6c6c6),
        inner_shadow: c(0xd8d8d8),
        fill: WHITE,
        fill_disabled: c(0xf4f4f4),
        text: BLACK,
        placeholder: c(0x8c8c8c),
        caret: BLACK,
        selection: c(0xb5d5ff), // designed: no captured text selection yet
    };
    let search = Field {
        border_bottom: c(0xe1e1e1),
        ..field
    };
    Theme {
        id: ThemeId::AQUA_LIGHT,
        provenance: Provenance::Measured,
        type_: typography(),
        colors: Colors {
            window: c(0xededed), // M: window.body; Force Quit body
            content: WHITE,      // M: table.header_and_rows rows
            canvas: c(0x1e1e1e), // designed: neutral dark well for consumer previews
            text: BLACK,
            text_secondary: c(0x4c4c4c), // designed
            text_disabled: c(0x9a9a9a),  // checkbox.off.disabled label tone
            text_on_accent: WHITE,
            title: c(0x262626),          // designed: title glyph core over chrome
            title_inactive: c(0x7a7a7a), // designed
            title_engrave: WHITE.alpha(150),
            separator: c(0xc0c0c0), // M: table.header.divider
            divider: c(0xb4b4b4),   // M: sidebar.divider
            // M: focus.ring.search: peak row solved from 6fa3d4 over c1 and 6b9fd0 over a9
            // (alpha 0.83, colour 5e9dd8); the outer row is 0.68 of that (89accc over c1).
            focus_ring: Rgba8::rgba(0x5e, 0x9d, 0xd8, 211),
            accent: c(0x3b77da), // Force Quit active list selection
            link: c(0x0b47d8),   // designed
            success: c(0x2b9a3f),
            warning: c(0xd28e0b),
            error: c(0xd0302a),
            info: c(0x3b77da),
            sidebar: c(0xdee4ea),          // M: sidebar.background
            sidebar_inactive: c(0xececec), // M: 10.6.8 capture, inactive Utilities sidebar
            sidebar_heading: c(0x738193),  // M: sidebar.header.text glyph core
            // M: Mail mailbox list (SL/mail-inbox.png): #dfe4e9 surface,
            // #738193 upper-case headings.
            source_secondary: c(0xdfe4e9),
            source_secondary_heading: c(0x738193),
            row_alternate: c(0xedf3fe),       // M: table.header_and_rows
            group_fill: c(0xe6e6e6),          // M: groupbox.fill
            group_border: c(0xc8c8c8),        // M: groupbox.left
            group_border_top: c(0xababab),    // M: groupbox.top
            group_border_bottom: c(0xdbdbdb), // M: groupbox.bottom
            group_highlight: c(0xe2e2e2),     // M: groupbox.left
            group_highlight_bottom: c(0xe4e4e4), // M: groupbox.bottom
            group_shadow: [c(0xc5c5c5), c(0xdbdbdb), c(0xe5e5e5)], // M: groupbox.top
            // M: groupbox.fill/left/top: a recessed well uses the same fill and
            // a 1 px inner top shadow one step darker than the border.
            well_fill: c(0xe6e6e6),
            well_border: c(0xc8c8c8),
            well_shadow: c(0xd2d2d2),
            band_rule: c(0xd8d8d8), // M: prefs.overview.bands
            // M: menu.context.body: white at 0.953 reads fbfbfb over the toolbar
            // and faf5f9 over the desktop in the two contextual captures.
            menu: WHITE.alpha(243),
            menu_text: BLACK,
            menu_text_disabled: c(0xa3a3a3),
            // M: menu.context.separator / .rows: one row darkening the body by
            // 0.106 (e4e4e4 over white, dfe0e1 over f9fbfc), 1 px side insets, no
            // highlight row.
            menu_separator: BLACK.alpha(27),
            menu_separator_highlight: WHITE.alpha(0),
            // M: menu.context.edge: black at 0.105 over the shadow, the
            // least-squares value for the top, side and bottom edge rows.
            menu_border: BLACK.alpha(27),
            menu_shadow: BLACK.alpha(84), // M: menu.context.shadow peak 0.33
            tooltip: c(0xffffc6),         // designed: help tags not captured
            tooltip_border: c(0xb5b5a0),
            tooltip_text: BLACK,
            shadow: BLACK.alpha(90), // M: win.shadow.* peak alpha ~94 below the window
            scrim: BLACK.alpha(40),
        },
        metrics: metrics(),
        chrome: ChromeSet {
            // M: chrome.unified.active (Finder) and chrome.titlebar.active (alert) agree.
            active: Chrome {
                highlight: c(0xe2e2e2),
                fill: g(0xd0d0d0, 0xa7a7a7),
                separator: c(0x515151),
            },
            // M: chrome.unified.inactive.scaled (10.6.8 Utilities window).
            inactive: Chrome {
                highlight: c(0xf3f3f3),
                fill: g(0xececec, 0xd8d8d8),
                separator: c(0x9d9d9d),
            },
            // M: bottombar.column.
            bottom_active: Chrome {
                highlight: c(0xcbcbcb),
                fill: g(0xcbcbcb, 0xa7a7a7),
                separator: c(0x515151),
            },
            // designed: inactive bottom bar follows the inactive title ramp.
            bottom_inactive: Chrome {
                highlight: c(0xececec),
                fill: g(0xe8e8e8, 0xd6d6d6),
                separator: c(0x9d9d9d),
            },
        },
        controls: Controls {
            push: BevelStates {
                normal: push_normal,
                // HIG pp. 131-132, 265-269: a pressed button acquires colour.
                pressed: map_bevel(push_default, aqua_pressed),
                disabled: push_disabled,
                selected: push_default,
            },
            // Measured Aqua has no hover bevel; the resting material is kept.
            hover: push_normal,
            push_default,
            // M: Snow Leopard installer Continue. Rim near 676767,
            // upper 5187e2->71abec, split 15 of 27, lower 52aaed->88e9fc,
            // bottom emboss f6f8f8, glyph 1c2737 (close-up capture).
            installer_continue: InstallerContinue {
                normal: Bevel {
                    border: g(0x6a6a6a, 0x646464),
                    upper: g(0x5187e2, 0x71abec),
                    lower: g(0x52aaed, 0x88e9fc),
                    split: 15.0 / 27.0,
                    emboss: c(0xf6f8f8),
                    text: c(0x1c2737),
                },
                default: Bevel {
                    border: g(0x6a6a6a, 0x646464),
                    upper: g(0x5187e2, 0x71abec),
                    lower: g(0x52aaed, 0x88e9fc),
                    split: 15.0 / 27.0,
                    emboss: c(0xf6f8f8),
                    text: c(0x1c2737),
                },
                // designed: no pressed capture; the same blue, deepened.
                pressed: Bevel {
                    border: g(0x6a6a6a, 0x646464),
                    upper: g(0x3f6fbe, 0x5c94d2),
                    lower: g(0x408fce, 0x6cc9e4),
                    split: 15.0 / 27.0,
                    emboss: c(0xf6f8f8),
                    text: c(0x141f2d),
                },
                // designed: desaturated disabled adaptation of the blue.
                disabled: Bevel {
                    border: g(0xa9a9a9, 0x999999),
                    upper: g(0xc9d2dc, 0xd6dde4),
                    lower: g(0xcbd9e3, 0xe2edf2),
                    split: 15.0 / 27.0,
                    emboss: c(0xf6f8f8),
                    text: c(0x8b959f),
                },
                glyph: c(0x1c2737),
                glyph_default: c(0x1c2737),
                glyph_pressed: c(0x141f2d),
                glyph_disabled: c(0x8b959f),
            },
            textured: BevelStates {
                normal: textured_normal,
                pressed: textured_selected,
                disabled: textured_disabled,
                selected: textured_selected,
            },
            gradient: BevelStates {
                // Print & Fax (10.6), add/remove buttons: 21 px square,
                // near-white upper face and light grey lower face.
                normal: Bevel {
                    border: g(0xb3b3b3, 0xa3a3a3),
                    upper: g(0xf9f9f9, 0xfbfbfb),
                    lower: g(0xe9e9e9, 0xededed),
                    split: 0.5,
                    emboss: Rgba8::TRANSPARENT,
                    text: BLACK,
                },
                pressed: textured_selected,
                disabled: textured_disabled,
                selected: textured_selected,
            },
            // M: capsule toolbar controls are the textured material with a
            // circular silhouette (HIG p. 261); pressed/disabled are shared.
            round: BevelStates {
                normal: textured_normal,
                pressed: textured_selected,
                disabled: textured_disabled,
                selected: textured_selected,
            },
            // designed: recessed control shares the measured tool-well material.
            recessed: BevelStates {
                normal: Bevel::plain(
                    g(0x9a9a9a, 0xc6c6c6),
                    g(0xd8d8d8, 0xf4f4f4),
                    BLACK.alpha(30),
                    BLACK,
                ),
                pressed: textured_selected,
                disabled: push_disabled,
                selected: textured_selected,
            },
            segment: BevelStates {
                normal: segment_normal,
                pressed: segment_pressed,
                disabled: push_disabled,
                selected: segment_selected,
            },
            checkbox: BevelStates {
                normal: checkbox_off,
                pressed: checkbox_pressed,
                disabled: checkbox_disabled,
                selected: checkbox_on,
            },
            radio: Radio {
                states: BevelStates {
                    normal: radio_normal,
                    pressed: radio_pressed,
                    disabled: checkbox_disabled,
                    selected: radio_selected,
                },
                dot: c(0x212d3b),
            },
            field,
            search,
            scroller: Scroller {
                // M: scroller.track.row: c5 at the leading edge to fd, border b3.
                track: g(0xc5c5c5, 0xfafafa),
                track_border: c(0xb3b3b3),
                // M: scroller.thumb.row: blue leading edge 0751c0, 4 rows
                // 88b4e5..98c3ef, 7 rows 5da5e9..a4e0ff, grey trailing edge
                // 5e5e5e (finder.png agrees on both axes).
                thumb: Bevel {
                    border: g(0x0751c0, 0x5e5e5e),
                    upper: g(0x88b4e5, 0x98c3ef),
                    lower: g(0x5da5e9, 0xa4e0ff),
                    split: 4.0 / 11.0,
                    emboss: Rgba8::TRANSPARENT,
                    text: BLACK,
                },
                // Aqua has no hover thumb material; the painter keeps `thumb`.
                thumb_hover: Bevel {
                    border: g(0x0751c0, 0x5e5e5e),
                    upper: g(0x88b4e5, 0x98c3ef),
                    lower: g(0x5da5e9, 0xa4e0ff),
                    split: 4.0 / 11.0,
                    emboss: Rgba8::TRANSPARENT,
                    text: BLACK,
                },
                // designed: graphite thumb for inactive windows.
                thumb_inactive: Bevel {
                    border: g(0x8f8f8f, 0x7d7d7d),
                    upper: g(0xe4e4e4, 0xd2d2d2),
                    lower: g(0xbdbdbd, 0xe6e6e6),
                    split: 4.0 / 11.0,
                    emboss: Rgba8::TRANSPARENT,
                    text: BLACK,
                },
                arrow: c(0x595959), // M: scroller.arrows
                // M: scroller.track.row (and the Finder and Console bars,
                // both axes): the curve a two-stop gradient cannot follow.
                track_rows: &SCROLLER_TRACK_ROWS,
                // M: scroller.boxes: a split bevel, identical on both axes.
                arrow_rows: &SCROLLER_ARROW_ROWS,
                // M: scroller.cap: black at 0.35 on the row before the arrows.
                cap_shadow: BLACK.alpha(90),
            },
            // M: table.header_and_rows: ff..e8 / e9..ff, border c0.
            table_header: Bevel {
                border: g(0xc0c0c0, 0xc0c0c0),
                upper: g(0xffffff, 0xe8e8e8),
                lower: g(0xe9e9e9, 0xffffff),
                split: 0.5,
                emboss: Rgba8::TRANSPARENT,
                text: BLACK,
            },
            // M: table.header.sorted: 7 rows cde2f5..88beec, 8 rows
            // 7cb8ea..c2edff, divider 72a1d4, bottom line 94badd (Finder list).
            table_header_sorted: Bevel {
                border: g(0x72a1d4, 0x94badd),
                upper: g(0xcde2f5, 0x88beec),
                lower: g(0x7cb8ea, 0xc2edff),
                split: 7.0 / 15.0,
                emboss: Rgba8::TRANSPARENT,
                text: BLACK,
            },
            sort_arrow: c(0x3d5b74), // M: table.header.sort_arrow
            // designed: the header divider grey for optional grid lines.
            table_grid: c(0xd9d9d9),
            popup: PopUp {
                body: BevelStates {
                    normal: popup_body,
                    pressed: popup_body_pressed,
                    disabled: popup_disabled,
                    selected: popup_body,
                },
                cap: BevelStates {
                    normal: popup_cap,
                    pressed: popup_cap_pressed,
                    disabled: popup_disabled,
                    selected: popup_cap,
                },
                divider: c(0x66a0e2),
                arrow: BLACK,
            },
            popup_textured,
            progress: Progress {
                track: progress_track,
                fill: progress_fill,
                // M: vm-screenshots/indeterminate-barber-pole-installer.png
                // (bar x 245..438, rows
                // 326..342). Blue column: top 2e4cb6, 9ebae5..599ede, then
                // 1583de..59b8ff from row 334. White column: top 8f9091,
                // e1e1e1..d2d3d3, then d7d8d8..feffff.
                barber_light: Bevel {
                    border: g(0x8f9091, 0x8f9091),
                    upper: g(0xe1e1e1, 0xd2d3d3),
                    lower: g(0xd7d8d8, 0xfeffff),
                    split: 0.5,
                    emboss: Rgba8::TRANSPARENT,
                    text: BLACK,
                },
                barber_dark: Bevel {
                    border: g(0x2e4cb6, 0x2e4cb6),
                    upper: g(0x9ebae5, 0x599ede),
                    lower: g(0x1583de, 0x59b8ff),
                    split: 0.5,
                    emboss: Rgba8::TRANSPARENT,
                    text: BLACK,
                },
                // M: one-pixel corners (indeterminate-barber-pole-installer.png, x 245..246 at
                // rows 326 and 342).
                radius: 1.0,
                spinner: c(0x4c4c4c),
                spinner_dim: BLACK.alpha(40),
            },
            slider: Slider {
                track: slider_track,
                track_disabled: slider_track_disabled,
                round: BevelStates {
                    normal: slider_round,
                    pressed: segment_pressed,
                    disabled: Bevel {
                        text: c(0x9a9a9a),
                        ..slider_round
                    },
                    selected: slider_round,
                },
                pointed: BevelStates {
                    normal: slider_pointed,
                    pressed: popup_cap_pressed,
                    disabled: Bevel {
                        text: c(0x9a9a9a),
                        ..slider_pointed
                    },
                    selected: slider_pointed,
                },
                tick: c(0x8a8a8a),
            },
            help: help_button,
        },
        lists: Lists {
            // M: sidebar.selection.active: line 5595d2, 6ca5dd..1a6ab9; labels white.
            source_active: Selection {
                line: c(0x5595d2),
                fill: g(0x6ca5dd, 0x1a6ab9),
                text: WHITE,
            },
            // designed: graphite selection for inactive windows.
            source_inactive: Selection {
                line: c(0x979797),
                fill: g(0xb4b4b4, 0x8a8a8a),
                text: WHITE,
            },
            // M: Finder/Open sidebar selection in an active but unfocused
            // window (Screen shot 4.15.47): abbad5 line, b3bed6..8196b9.
            source_unfocused: Selection {
                line: c(0xabbad5),
                fill: g(0xb3bed6, 0x8196b9),
                text: WHITE,
            },
            // M: Mail mailbox selection gradient matches the primary list's
            // focused band; secondary lists only differ in their background.
            source_secondary_active: Selection {
                line: c(0x5595d2),
                fill: g(0x6ca5dd, 0x1a6ab9),
                text: WHITE,
            },
            source_secondary_inactive: Selection {
                line: c(0xabbad5),
                fill: g(0xb3bed6, 0x8196b9),
                text: WHITE,
            },
            // Force Quit: flat 3b77da.
            table_active: Selection {
                line: c(0x3b77da),
                fill: Gradient::flat(c(0x3b77da)),
                text: WHITE,
            },
            // M: Desktop pane list without focus: flat d4d4d4.
            table_inactive: Selection {
                line: c(0xd4d4d4),
                fill: Gradient::flat(c(0xd4d4d4)),
                text: BLACK,
            },
            // M: menu.body: line 5980ee, 6388f8..215ff6.
            menu: Selection {
                line: c(0x5980ee),
                fill: g(0x6388f8, 0x215ff6),
                text: WHITE,
            },
        },
        traffic: traffic_light(),
        // M: grip.bar (Finder, Console: 93/64/c6 over ac) and grip.body
        // (Activity Monitor, TextEdit: c2/80/e1 over e6), as alpha over the
        // local background: 1 - 147/172 = 0.145, 1 - 100/172 = 0.419,
        // (198-172)/(255-172) = 0.31 white; 0.157, 0.443, 0.022 black.
        grip: ResizeGrip {
            visible: true,
            bar: GripStyle {
                inset: 4.0,
                shade: BLACK.alpha(37),
                line: BLACK.alpha(107),
                highlight: WHITE.alpha(80),
            },
            body: GripStyle {
                inset: 1.0,
                shade: BLACK.alpha(40),
                line: BLACK.alpha(113),
                highlight: BLACK.alpha(6),
            },
            // The bar grip spans 15 px from the corner (inset 4 + 10 + shade 1).
            hit: 15.0,
        },
    }
}

/// Dark Aqua: designed graphite adaptation (not a historical Apple theme).
pub fn aqua_dark() -> Theme {
    let light = aqua_light();
    let push_normal = Bevel {
        border: g(0x0e0e0e, 0x1a1a1a),
        upper: g(0x6a6a6a, 0x575757),
        lower: g(0x4a4a4a, 0x606060),
        split: 6.0 / 16.0,
        emboss: WHITE.alpha(22),
        text: c(0xeeeeee),
    };
    let push_default = Bevel {
        border: g(0x0b2a6b, 0x0e1420),
        upper: g(0x7fb0ea, 0x4a86d6),
        lower: g(0x2a68c8, 0x5aa6ee),
        split: 7.0 / 19.0,
        emboss: WHITE.alpha(22),
        text: WHITE,
    };
    let push_disabled = Bevel {
        border: g(0x262626, 0x2a2a2a),
        upper: g(0x464646, 0x404040),
        lower: g(0x3a3a3a, 0x424242),
        split: 6.0 / 16.0,
        emboss: WHITE.alpha(10),
        text: c(0x7a7a7a),
    };
    let textured_normal = Bevel::plain(
        g(0x101010, 0x0a0a0a),
        g(0x5e5e5e, 0x3a3a3a),
        WHITE.alpha(20),
        c(0xe6e6e6),
    );
    let textured_selected = Bevel {
        border: g(0x0a0a0a, 0x0a0a0a),
        upper: g(0x101010, 0x222222),
        lower: g(0x242424, 0x262626),
        split: 6.0 / 20.0,
        emboss: WHITE.alpha(20),
        text: WHITE,
    };
    let segment_selected = Bevel {
        border: g(0x0b2a6b, 0x0e1420),
        upper: g(0x86b6ee, 0x4f8ad8),
        lower: g(0x2b6cc9, 0x5fa9ef),
        split: 8.0 / 19.0,
        emboss: WHITE.alpha(18),
        text: WHITE,
    };
    let field = Field {
        border_top: c(0x0a0a0a),
        border: c(0x1a1a1a),
        border_bottom: c(0x2c2c2c),
        inner_shadow: c(0x101010),
        fill: c(0x1d1d1d),
        fill_disabled: c(0x262626),
        text: c(0xeeeeee),
        placeholder: c(0x7d7d7d),
        caret: c(0xf2f2f2),
        selection: c(0x2f5a96),
    };
    let checkbox_off = Bevel {
        border: g(0x0e0e0e, 0x1a1a1a),
        upper: g(0x6a6a6a, 0x575757),
        lower: g(0x474747, 0x606060),
        split: 5.0 / 12.0,
        emboss: WHITE.alpha(18),
        text: WHITE,
    };
    let radio_normal = Bevel {
        border: g(0x0e0e0e, 0x1a1a1a),
        upper: g(0x6a6a6a, 0x575757),
        lower: g(0x474747, 0x606060),
        split: 6.0 / 12.0,
        emboss: WHITE.alpha(18),
        text: WHITE,
    };
    let radio_selected = Bevel {
        border: g(0x0b2a6b, 0x0e1420),
        upper: g(0x86b6ee, 0x4f8ad8),
        lower: g(0x2b6cc9, 0x5fa9ef),
        split: 6.0 / 12.0,
        emboss: WHITE.alpha(18),
        text: c(0x101820),
    };
    // designed: Dark Aqua keeps the 10.6 layer order; the cap stays blue and
    // the body graphite.
    let popup_body = Bevel {
        border: g(0x0a0a0a, 0x1a1a1a),
        upper: g(0x6a6a6a, 0x4f4f4f),
        lower: g(0x484848, 0x626262),
        split: 9.0 / 19.0,
        emboss: WHITE.alpha(18),
        text: c(0xeeeeee),
    };
    let popup_cap = Bevel {
        border: g(0x0b2a6b, 0x0e1420),
        upper: g(0x7fb0ea, 0x4a86d6),
        lower: g(0x2a68c8, 0x5aa6ee),
        split: 8.0 / 19.0,
        emboss: WHITE.alpha(22),
        text: WHITE,
    };
    let popup_disabled = Bevel {
        border: g(0x262626, 0x2a2a2a),
        upper: g(0x464646, 0x404040),
        lower: g(0x3a3a3a, 0x424242),
        split: 9.0 / 19.0,
        emboss: WHITE.alpha(10),
        text: c(0x7a7a7a),
    };
    let popup_textured = PopUp {
        body: BevelStates {
            normal: textured_normal,
            pressed: textured_selected,
            disabled: Bevel::plain(
                g(0x1c1c1c, 0x1c1c1c),
                g(0x464646, 0x363636),
                WHITE.alpha(10),
                c(0x777777),
            ),
            selected: textured_normal,
        },
        cap: BevelStates {
            normal: textured_normal,
            pressed: textured_selected,
            disabled: Bevel::plain(
                g(0x1c1c1c, 0x1c1c1c),
                g(0x464646, 0x363636),
                WHITE.alpha(10),
                c(0x777777),
            ),
            selected: textured_normal,
        },
        divider: c(0x0a0a0a),
        arrow: c(0xe6e6e6),
    };
    let slider_track = Bevel::plain(
        g(0x0c0c0c, 0x2a2a2a),
        g(0x202020, 0x343434),
        WHITE.alpha(18),
        WHITE,
    );
    let slider_round = Bevel {
        border: g(0x0e0e0e, 0x1a1a1a),
        upper: g(0x6a6a6a, 0x575757),
        lower: g(0x4a4a4a, 0x606060),
        split: 10.0 / 22.0,
        emboss: WHITE.alpha(18),
        text: WHITE,
    };
    let slider_pointed = popup_cap;
    let help_button = Help {
        states: BevelStates {
            normal: Bevel {
                border: g(0x2a1430, 0x4a2a52),
                upper: g(0x8a6a9a, 0x6a4a7a),
                lower: g(0x5a3a68, 0x7a5a88),
                split: 0.5,
                emboss: WHITE.alpha(18),
                text: c(0xf0e4fd),
            },
            pressed: Bevel {
                upper: g(0x6a4a7a, 0x5a3a68),
                lower: g(0x4a2a52, 0x6a4a7a),
                ..radio_normal
            },
            disabled: push_disabled,
            selected: radio_selected,
        },
        glyph: c(0xf0e4fd),
    };
    Theme {
        id: ThemeId::AQUA_DARK,
        provenance: Provenance::Designed,
        type_: typography(),
        colors: Colors {
            window: c(0x2d2d2d),
            content: c(0x1f1f1f),
            canvas: c(0x101010),
            text: c(0xeeeeee),
            text_secondary: c(0xb0b0b0),
            text_disabled: c(0x767676),
            text_on_accent: WHITE,
            title: c(0xe6e6e6),
            title_inactive: c(0x8c8c8c),
            title_engrave: BLACK.alpha(150),
            separator: c(0x3c3c3c),
            divider: c(0x0c0c0c),
            focus_ring: Rgba8::rgba(0x5a, 0x9c, 0xf0, 0xb4),
            accent: c(0x3f7fe0),
            link: c(0x79b0ff),
            success: c(0x47b85a),
            warning: c(0xe0a33a),
            error: c(0xf05a50),
            info: c(0x5a9cf0),
            // Graphite with the Aqua sidebar's blue cast kept (M: sidebar.background hue).
            sidebar: c(0x2a3038),
            sidebar_inactive: c(0x2b2b2b),
            sidebar_heading: c(0x8d9aab),
            source_secondary: c(0x23282e),
            source_secondary_heading: c(0x8d9aab),
            row_alternate: c(0x252b34),
            group_fill: c(0x333333),
            group_border: c(0x474747),
            group_border_top: c(0x262626), // designed: dark mirror of the Aqua top edge
            // designed: each row sits where its Aqua Light row sits between
            // the edge and the fill (M: groupbox.top, .left, .bottom).
            group_border_bottom: c(0x474747).mix(c(0x333333), 0.63),
            group_highlight: c(0x474747).mix(c(0x333333), 0.87),
            group_highlight_bottom: c(0x474747).mix(c(0x333333), 0.93),
            group_shadow: [
                c(0x262626).mix(c(0x333333), 0.44),
                c(0x262626).mix(c(0x333333), 0.81),
                c(0x262626).mix(c(0x333333), 0.98),
            ],
            well_fill: c(0x333333),
            well_border: c(0x474747),
            well_shadow: c(0x202020),
            band_rule: c(0x2a2a2a), // designed
            // designed: Aqua Light's translucency over the dark body.
            menu: c(0x2c2c2e).alpha(243),
            menu_text: c(0xeeeeee),
            menu_text_disabled: c(0x777777),
            menu_separator: c(0x1a1a1a),
            menu_separator_highlight: c(0x3c3c3e),
            menu_border: c(0x0a0a0a),
            menu_shadow: BLACK.alpha(170), // designed: dark window shadow
            tooltip: c(0x3a3a30),
            tooltip_border: c(0x5a5a48),
            tooltip_text: c(0xf0f0e0),
            shadow: BLACK.alpha(170),
            scrim: BLACK.alpha(90),
        },
        metrics: light.metrics.clone(),
        chrome: ChromeSet {
            active: Chrome {
                highlight: c(0x6a6a6a),
                fill: g(0x4c4c4c, 0x2f2f2f),
                separator: c(0x0a0a0a),
            },
            inactive: Chrome {
                highlight: c(0x4e4e4e),
                fill: g(0x3c3c3c, 0x323232),
                separator: c(0x161616),
            },
            bottom_active: Chrome {
                highlight: c(0x4a4a4a),
                fill: g(0x464646, 0x2f2f2f),
                separator: c(0x0a0a0a),
            },
            bottom_inactive: Chrome {
                highlight: c(0x3c3c3c),
                fill: g(0x3a3a3a, 0x313131),
                separator: c(0x161616),
            },
        },
        controls: Controls {
            push: BevelStates {
                normal: push_normal,
                // designed: the light map clips Dark Aqua's lower stops to pure blue;
                // darken 25% toward black instead.
                pressed: map_bevel(push_default, |c| c.mix(BLACK.alpha(c.a), 0.25)),
                disabled: push_disabled,
                selected: push_default,
            },
            // Measured Aqua has no hover bevel; the resting material is kept.
            hover: push_normal,
            push_default,
            // designed: Dark Aqua adaptation of the installer Continue blue.
            installer_continue: InstallerContinue {
                normal: Bevel {
                    border: g(0x0b2a6b, 0x0e1420),
                    upper: g(0x5f97dd, 0x356fbe),
                    lower: g(0x245cb4, 0x4a95dd),
                    split: 15.0 / 27.0,
                    emboss: WHITE.alpha(22),
                    text: c(0xeaf3ff),
                },
                default: Bevel {
                    border: g(0x0b2a6b, 0x0e1420),
                    upper: g(0x5f97dd, 0x356fbe),
                    lower: g(0x245cb4, 0x4a95dd),
                    split: 15.0 / 27.0,
                    emboss: WHITE.alpha(22),
                    text: c(0xeaf3ff),
                },
                pressed: Bevel {
                    border: g(0x0b2a6b, 0x0e1420),
                    upper: g(0x4b7fca, 0x2a5da6),
                    lower: g(0x1c4a96, 0x3d7fc4),
                    split: 15.0 / 27.0,
                    emboss: WHITE.alpha(22),
                    text: c(0xdbe9f7),
                },
                disabled: Bevel {
                    border: g(0x262626, 0x2a2a2a),
                    upper: g(0x464646, 0x404040),
                    lower: g(0x3a3a3a, 0x424242),
                    split: 15.0 / 27.0,
                    emboss: WHITE.alpha(10),
                    text: c(0x858585),
                },
                glyph: c(0xeaf3ff),
                glyph_default: c(0xeaf3ff),
                glyph_pressed: c(0xdbe9f7),
                glyph_disabled: c(0x858585),
            },
            textured: BevelStates {
                normal: textured_normal,
                pressed: textured_selected,
                disabled: Bevel::plain(
                    g(0x1c1c1c, 0x1c1c1c),
                    g(0x464646, 0x363636),
                    WHITE.alpha(10),
                    c(0x777777),
                ),
                selected: textured_selected,
            },
            gradient: BevelStates {
                normal: textured_normal,
                pressed: textured_selected,
                disabled: Bevel::plain(
                    g(0x1c1c1c, 0x1c1c1c),
                    g(0x464646, 0x363636),
                    WHITE.alpha(10),
                    c(0x777777),
                ),
                selected: textured_selected,
            },
            round: BevelStates {
                normal: textured_normal,
                pressed: textured_selected,
                disabled: Bevel::plain(
                    g(0x1c1c1c, 0x1c1c1c),
                    g(0x464646, 0x363636),
                    WHITE.alpha(10),
                    c(0x777777),
                ),
                selected: textured_selected,
            },
            recessed: BevelStates {
                normal: Bevel::plain(
                    g(0x0a0a0a, 0x2a2a2a),
                    g(0x1e1e1e, 0x343434),
                    WHITE.alpha(12),
                    WHITE,
                ),
                pressed: textured_selected,
                disabled: push_disabled,
                selected: textured_selected,
            },
            segment: BevelStates {
                normal: Bevel {
                    split: 8.0 / 19.0,
                    ..push_normal
                },
                pressed: Bevel {
                    upper: g(0x5f90cf, 0x3a70bd),
                    lower: g(0x1f55a8, 0x4686cc),
                    ..segment_selected
                },
                disabled: push_disabled,
                selected: segment_selected,
            },
            checkbox: BevelStates {
                normal: checkbox_off,
                pressed: Bevel {
                    upper: g(0x4a4a4a, 0x404040),
                    lower: g(0x363636, 0x444444),
                    ..checkbox_off
                },
                disabled: Bevel {
                    text: c(0x777777),
                    ..push_disabled
                },
                selected: Bevel {
                    border: g(0x0b2a6b, 0x0e1420),
                    upper: g(0x86b6ee, 0x4f8ad8),
                    lower: g(0x2b6cc9, 0x5fa9ef),
                    split: 5.0 / 12.0,
                    emboss: WHITE.alpha(18),
                    text: WHITE,
                },
            },
            radio: Radio {
                states: BevelStates {
                    normal: radio_normal,
                    pressed: Bevel {
                        upper: g(0x4a4a4a, 0x404040),
                        lower: g(0x363636, 0x444444),
                        ..radio_normal
                    },
                    disabled: Bevel {
                        text: c(0x777777),
                        ..push_disabled
                    },
                    selected: radio_selected,
                },
                dot: c(0x101820),
            },
            field,
            search: field,
            scroller: Scroller {
                track: g(0x1a1a1a, 0x2a2a2a),
                track_border: c(0x0e0e0e),
                thumb: Bevel {
                    border: g(0x0b2a6b, 0x14365e),
                    upper: g(0x6c9ad6, 0x5786c8),
                    lower: g(0x2f6cc4, 0x5c9be0),
                    split: 5.0 / 12.0,
                    emboss: Rgba8::TRANSPARENT,
                    text: WHITE,
                },
                thumb_hover: Bevel {
                    border: g(0x0b2a6b, 0x14365e),
                    upper: g(0x6c9ad6, 0x5786c8),
                    lower: g(0x2f6cc4, 0x5c9be0),
                    split: 5.0 / 12.0,
                    emboss: Rgba8::TRANSPARENT,
                    text: WHITE,
                },
                thumb_inactive: Bevel {
                    border: g(0x0e0e0e, 0x161616),
                    upper: g(0x6a6a6a, 0x5a5a5a),
                    lower: g(0x484848, 0x606060),
                    split: 5.0 / 12.0,
                    emboss: Rgba8::TRANSPARENT,
                    text: WHITE,
                },
                arrow: c(0xb8b8b8),
                track_rows: &[],
                arrow_rows: &[],
                // designed: the light cap shading carried over to the dark track.
                cap_shadow: BLACK.alpha(90),
            },
            table_header: Bevel {
                border: g(0x141414, 0x141414),
                upper: g(0x464646, 0x3a3a3a),
                lower: g(0x353535, 0x3e3e3e),
                split: 0.5,
                emboss: Rgba8::TRANSPARENT,
                text: c(0xe6e6e6),
            },
            // designed: the sorted header and arrow for Dark Aqua.
            table_header_sorted: Bevel {
                border: g(0x2a4a72, 0x2a4a72),
                upper: g(0x3c5f8c, 0x2c5185),
                lower: g(0x244a80, 0x3a6aa6),
                split: 7.0 / 15.0,
                emboss: Rgba8::TRANSPARENT,
                text: WHITE,
            },
            sort_arrow: c(0xc8d8ea),
            table_grid: c(0x333333),
            popup: PopUp {
                body: BevelStates {
                    normal: popup_body,
                    pressed: Bevel {
                        upper: g(0x575757, 0x3f3f3f),
                        lower: g(0x383838, 0x4e4e4e),
                        ..popup_body
                    },
                    disabled: popup_disabled,
                    selected: popup_body,
                },
                cap: BevelStates {
                    normal: popup_cap,
                    pressed: Bevel {
                        upper: g(0x5f90cf, 0x3a70bd),
                        lower: g(0x1f55a8, 0x4686cc),
                        ..popup_cap
                    },
                    disabled: popup_disabled,
                    selected: popup_cap,
                },
                divider: c(0x0b2a6b),
                arrow: WHITE,
            },
            popup_textured,
            progress: Progress {
                track: Bevel::plain(
                    g(0x0c0c0c, 0x161616),
                    g(0x1e1e1e, 0x2e2e2e),
                    WHITE.alpha(18),
                    WHITE,
                ),
                fill: segment_selected,
                // D: Aqua's measured barber profiles, darkened for the dark
                // well.
                barber_light: Bevel {
                    border: g(0x1a1a1a, 0x1a1a1a),
                    upper: g(0x4a4a4a, 0x3a3a3a),
                    lower: g(0x363636, 0x505050),
                    split: 0.5,
                    emboss: Rgba8::TRANSPARENT,
                    text: WHITE,
                },
                barber_dark: Bevel {
                    border: g(0x1e3f8a, 0x1e3f8a),
                    upper: g(0x7aa6dc, 0x3f86d0),
                    lower: g(0x1667b8, 0x4f9df0),
                    split: 0.5,
                    emboss: Rgba8::TRANSPARENT,
                    text: WHITE,
                },
                radius: 1.0,
                spinner: c(0xd0d0d0),
                spinner_dim: WHITE.alpha(40),
            },
            slider: Slider {
                track: slider_track,
                track_disabled: Bevel::plain(
                    g(0x161616, 0x1a1a1a),
                    g(0x202020, 0x2a2a2a),
                    WHITE.alpha(10),
                    c(0x777777),
                ),
                round: BevelStates {
                    normal: slider_round,
                    pressed: segment_selected,
                    disabled: Bevel {
                        text: c(0x777777),
                        ..slider_round
                    },
                    selected: slider_round,
                },
                pointed: BevelStates {
                    normal: slider_pointed,
                    pressed: Bevel {
                        upper: g(0x5f90cf, 0x3a70bd),
                        lower: g(0x1f55a8, 0x4686cc),
                        ..slider_pointed
                    },
                    disabled: Bevel {
                        text: c(0x777777),
                        ..slider_pointed
                    },
                    selected: slider_pointed,
                },
                tick: c(0xb8b8b8),
            },
            help: help_button,
        },
        lists: Lists {
            source_active: Selection {
                line: c(0x4a86d0),
                fill: g(0x4c8ad0, 0x174f96),
                text: WHITE,
            },
            source_inactive: Selection {
                line: c(0x5c5c5c),
                fill: g(0x555555, 0x3c3c3c),
                text: WHITE,
            },
            source_unfocused: Selection {
                line: c(0x4a4a4a),
                fill: g(0x525252, 0x3a3a3a),
                text: WHITE,
            },
            source_secondary_active: Selection {
                line: c(0x4a86d0),
                fill: g(0x4c8ad0, 0x174f96),
                text: WHITE,
            },
            source_secondary_inactive: Selection {
                line: c(0x3c3c3c),
                fill: g(0x444444, 0x2f2f2f),
                text: WHITE,
            },
            table_active: Selection {
                line: c(0x2f63b8),
                fill: Gradient::flat(c(0x2f63b8)),
                text: WHITE,
            },
            table_inactive: Selection {
                line: c(0x464646),
                fill: Gradient::flat(c(0x464646)),
                text: c(0xeeeeee),
            },
            menu: Selection {
                line: c(0x4f78e6),
                fill: g(0x5a82ee, 0x1d55df),
                text: WHITE,
            },
        },
        // designed: Dark Aqua chrome is darker, so the lines need more
        // weight to show and the white highlight less.
        grip: ResizeGrip {
            visible: true,
            bar: GripStyle {
                inset: 4.0,
                shade: BLACK.alpha(60),
                line: BLACK.alpha(150),
                highlight: WHITE.alpha(40),
            },
            body: GripStyle {
                inset: 1.0,
                shade: BLACK.alpha(60),
                line: BLACK.alpha(150),
                highlight: WHITE.alpha(20),
            },
            hit: 15.0,
        },
        traffic: TrafficLights {
            // Identities kept (handbook 10); the idle state darkens for graphite chrome.
            idle: TrafficLight {
                ring_top: c(0x1c1c1c),
                ring_bottom: c(0x2c2c2c),
                deep: c(0x5a5a5a),
                glow: c(0x6a6a6a),
                specular: c(0x8a8a8a),
                glyph: c(0xd0d0d0),
            },
            emboss: WHITE.alpha(20),
            ..light.traffic
        },
    }
}
