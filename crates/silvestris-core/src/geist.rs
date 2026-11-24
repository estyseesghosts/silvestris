//! Geist tokens.
//!
//! Geist Dark keeps Lotus's pre-Silvestris palette exactly (lotus
//! crates/desktop/src/theme.rs, from the Geist dark scale in workspace
//! vercel.md: black, gray 100 #1a1a1a ... gray 1000 #ededed, blue #0095ff,
//! green #279447, amber #d99327, red #e34646). Geist Light is a new design
//! (#126) using the same 100-1000 role scale from vercel.md §4 (100 default
//! background ... 1000 primary text); Vercel's light values are not
//! published in the reference material, so docs/themes.md lists them as
//! designed. Geist surfaces are flat: bevels have equal stops and no emboss.

use crate::color::{Gradient, Rgba8};
use crate::theme::ThemeId;
use crate::tokens::*;

const fn c(rgb: u32) -> Rgba8 {
    Rgba8::hex(rgb)
}
const T: Rgba8 = Rgba8::TRANSPARENT;

fn flat(border: Rgba8, fill: Rgba8, text: Rgba8) -> Bevel {
    Bevel::plain(Gradient::flat(border), Gradient::flat(fill), T, text)
}

fn chrome(highlight: Rgba8, fill: Rgba8, separator: Rgba8) -> Chrome {
    Chrome {
        highlight,
        fill: Gradient::flat(fill),
        separator,
    }
}

fn selection(fill: Rgba8, text: Rgba8) -> Selection {
    Selection {
        line: fill,
        fill: Gradient::flat(fill),
        text,
    }
}

fn typography() -> Typography {
    Typography {
        ui: crate::fonts::INTER,
        mono: crate::fonts::GEIST_MONO,
        // Same roles as Aqua so layouts do not change between families.
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

/// Geist geometry: flatter and slightly roomier; the shell layout is shared.
fn metrics() -> Metrics {
    Metrics {
        window_radius: 8.0,
        titlebar_height: 32.0,
        toolbar_height: 40.0,
        toolbar_labelled_height: 52.0,
        bottom_bar_height: 26.0,
        traffic_diameter: 12.0,
        traffic_left: 12.0,
        traffic_top: 10.0,
        traffic_pitch: 20.0,
        push_height: 28.0,
        push_small_height: 24.0,
        push_mini_height: 20.0,
        push_radius: 6.0,
        // designed: cap scales with the 28 px Geist control height.
        popup_cap: [18.0, 16.0, 14.0],
        // designed: 12/10/8 px, 4 px grid.
        push_padding: [12.0, 10.0, 8.0],
        // designed: flex centring.
        push_label_dy: [0.0, 0.0, 0.0],
        textured_height: 28.0,
        textured_radius: 6.0,
        segment_height: 28.0,
        field_height: 28.0,
        field_small_height: 24.0,
        // designed: 8 px text inset, 1 px caret with 6 px clear above and below.
        field_text_inset: 8.0,
        caret_width: 1.0,
        caret_inset_top: 6.0,
        caret_inset_bottom: 6.0,
        checkbox_size: 16.0,
        checkbox_small_size: 14.0,
        checkbox_mini_size: 12.0,
        checkbox_label_gap: 6.0,
        checkbox_radius: 4.0,
        radio_size: [16.0, 14.0, 12.0],
        radio_dot_fraction: 0.3,
        radio_label_gap: 8.0,
        radio_row_gap: 4.0,
        source_row_height: 28.0,
        source_indent: 14.0,
        table_row_height: 24.0,
        table_header_height: 26.0,
        menu_item_height: 28.0,
        menu_separator_height: 9.0,
        menu_padding: 4.0,
        menu_radius: 8.0,
        // designed: Geist's scroller and its small and mini steps.
        scroller_width: [10.0, 8.0, 6.0],
        // designed: 2 px solid accent ring just outside the edge.
        focus_ring: &[(2.0, 1.0), (1.0, 1.0)],
        disclosure_size: 13.0,
        disclosure_width: 4.5,
        disclosure_height: 6.0,
        slider_height: 20.0,
        slider_track_height: [6.0, 5.0, 4.0],
        slider_thumb_width: [18.0, 16.0, 14.0],
        slider_thumb_height: [20.0, 18.0, 16.0],
        slider_tick_height: 4.0,
        slider_tick_gap: 4.0,
        slider_tick_label_gap: 6.0,
        progress_height: [16.0, 12.0, 10.0],
        spinner_size: [16.0, 14.0, 12.0],
        menu_shadow_offset: [0.0, 4.0],
        menu_shadow_blur: 16.0,
        menu_shadow_spread: 0.0,
        menu_label_dy: 0.0,
        well_inset: 1.0,
        splitter_hot_zone: 3.0,
        control_spacing: 8.0,
        tab_label_margin: 12.0,
        panel_radius: 8.0,
    }
}

const GRIP_NONE: GripStyle = GripStyle {
    inset: 0.0,
    shade: T,
    line: T,
    highlight: T,
};

fn traffic(idle_ring: Rgba8, idle_body: Rgba8) -> TrafficLights {
    let solid = |ring: u32, body: u32| TrafficLight {
        ring_top: c(ring),
        ring_bottom: c(ring),
        deep: c(body),
        glow: c(body),
        specular: c(body),
        glyph: c(0x000000).alpha(150),
    };
    TrafficLights {
        close: solid(0xc4352e, 0xff5f57),
        minimize: solid(0xc28a1b, 0xfebc2e),
        zoom: solid(0x1d9a2c, 0x28c840),
        idle: TrafficLight {
            ring_top: idle_ring,
            ring_bottom: idle_ring,
            deep: idle_body,
            glow: idle_body,
            specular: idle_body,
            glyph: c(0x000000).alpha(120),
        },
        emboss: T,
        // designed: flat discs (Geist has no gloss).
        shape: SphereShape::FLAT,
    }
}

/// Geist Dark: Lotus's existing look.
pub fn geist_dark() -> Theme {
    // lotus theme.rs names: background 000, panel 1a1a1a, surface_hover 1f1f1f,
    // surface_active 292929, border 2e2e2e, border_strong 454545, text ededed,
    // text_secondary a1a1a1, text_muted 878787.
    let normal = flat(c(0x2e2e2e), c(0x1f1f1f), c(0xededed));
    let pressed = flat(c(0x454545), c(0x292929), c(0xededed));
    let disabled = flat(c(0x2e2e2e), c(0x1a1a1a), c(0x878787));
    let selected = flat(c(0x0095ff), c(0x0b3a5c), c(0xededed));
    let field = Field {
        border_top: c(0x2e2e2e),
        border: c(0x2e2e2e),
        border_bottom: c(0x2e2e2e),
        inner_shadow: c(0x0a0a0a),
        fill: c(0x0a0a0a),
        fill_disabled: c(0x1a1a1a),
        text: c(0xededed),
        placeholder: c(0x878787),
        caret: c(0xededed),
        selection: c(0x0b3a5c),
    };
    // designed: Geist is flat; the pop-up keeps the same neutral body and a
    // blue accent cap so the affordance stays legible.
    let popup = PopUp {
        body: BevelStates {
            normal,
            pressed,
            disabled,
            selected: normal,
        },
        cap: BevelStates {
            normal: flat(c(0x0095ff), c(0x0b3a5c), c(0xededed)),
            pressed: flat(c(0x0070f3), c(0x0b3a5c), c(0xededed)),
            disabled,
            selected: flat(c(0x0095ff), c(0x0b3a5c), c(0xededed)),
        },
        divider: c(0x0b3a5c),
        arrow: c(0xededed),
    };
    let radio = Radio {
        states: BevelStates {
            normal: flat(c(0x454545), c(0x0a0a0a), c(0xededed)),
            pressed,
            disabled,
            selected: flat(c(0x0095ff), c(0x0b3a5c), c(0xededed)),
        },
        dot: c(0x0a0a0a),
    };
    let slider = Slider {
        track: flat(c(0x2e2e2e), c(0x292929), c(0xededed)),
        track_disabled: flat(c(0x2e2e2e), c(0x1a1a1a), c(0x878787)),
        round: BevelStates {
            normal: flat(c(0x454545), c(0xededed), c(0x000000)),
            pressed,
            disabled,
            selected: pressed,
        },
        pointed: BevelStates {
            normal: flat(c(0x0095ff), c(0x0b3a5c), c(0xededed)),
            pressed,
            disabled,
            selected: pressed,
        },
        tick: c(0x878787),
    };
    let progress = Progress {
        track: flat(c(0x2e2e2e), c(0x1a1a1a), c(0xededed)),
        fill: flat(c(0x0095ff), c(0x0095ff), c(0xededed)),
        barber_light: flat(c(0x3a3a3a), c(0x3a3a3a), c(0xededed)),
        barber_dark: flat(c(0x4f9df0), c(0x4f9df0), c(0xededed)),
        radius: f32::MAX,
        spinner: c(0xd0d0d0),
        spinner_dim: c(0xffffff).alpha(40),
    };
    let help = Help {
        states: BevelStates {
            normal,
            pressed,
            disabled,
            selected: pressed,
        },
        glyph: c(0xededed),
    };
    Theme {
        id: ThemeId::GEIST_DARK,
        provenance: Provenance::Inherited,
        type_: typography(),
        colors: Colors {
            window: c(0x1a1a1a),
            content: c(0x0a0a0a),
            canvas: c(0x000000),
            text: c(0xededed),
            text_secondary: c(0xa1a1a1),
            text_disabled: c(0x878787),
            text_on_accent: c(0xffffff),
            title: c(0xededed),
            title_inactive: c(0x878787),
            title_engrave: T,
            separator: c(0x2e2e2e),
            divider: c(0x2e2e2e),
            focus_ring: c(0x0095ff),
            accent: c(0x0095ff),
            link: c(0x0095ff),
            success: c(0x279447),
            warning: c(0xd99327),
            error: c(0xe34646),
            info: c(0x0095ff),
            sidebar: c(0x1a1a1a),
            sidebar_inactive: c(0x1a1a1a),
            sidebar_heading: c(0x878787),
            source_secondary: c(0x111111),
            source_secondary_heading: c(0x878787),
            row_alternate: c(0x111111),
            group_fill: c(0x1a1a1a),
            group_border: c(0x2e2e2e),
            group_border_top: c(0x2e2e2e),
            // Geist boxes are flat: one border colour, no inner shading.
            group_border_bottom: c(0x2e2e2e),
            group_highlight: c(0x1a1a1a),
            group_highlight_bottom: c(0x1a1a1a),
            group_shadow: [c(0x1a1a1a); 3],
            well_fill: c(0x111111),
            well_border: c(0x2e2e2e),
            well_shadow: c(0x0a0a0a),
            band_rule: c(0x2e2e2e),
            menu: c(0x1a1a1a),
            menu_text: c(0xededed),
            menu_text_disabled: c(0x878787),
            menu_separator: c(0x2e2e2e),
            menu_separator_highlight: T,
            menu_border: c(0x2e2e2e),
            menu_shadow: c(0x000000).alpha(160),
            tooltip: c(0x292929),
            tooltip_border: c(0x454545),
            tooltip_text: c(0xededed),
            shadow: c(0x000000).alpha(160),
            scrim: c(0x000000).alpha(120),
        },
        metrics: metrics(),
        chrome: ChromeSet {
            active: chrome(c(0x1a1a1a), c(0x1a1a1a), c(0x2e2e2e)),
            inactive: chrome(c(0x141414), c(0x141414), c(0x2e2e2e)),
            bottom_active: chrome(c(0x1a1a1a), c(0x1a1a1a), c(0x2e2e2e)),
            bottom_inactive: chrome(c(0x141414), c(0x141414), c(0x2e2e2e)),
        },
        controls: Controls {
            push: BevelStates {
                normal,
                pressed,
                disabled,
                selected,
            },
            push_default: flat(c(0xededed), c(0xededed), c(0x000000)),
            textured: BevelStates {
                normal,
                pressed,
                disabled,
                selected: pressed,
            },
            gradient: BevelStates {
                normal,
                pressed,
                disabled,
                selected: pressed,
            },
            round: BevelStates {
                normal,
                pressed,
                disabled,
                selected: pressed,
            },
            recessed: BevelStates {
                normal,
                pressed,
                disabled,
                selected: pressed,
            },
            segment: BevelStates {
                normal,
                pressed,
                disabled,
                selected: flat(c(0x454545), c(0x292929), c(0xededed)),
            },
            checkbox: BevelStates {
                normal: flat(c(0x454545), c(0x0a0a0a), c(0xededed)),
                pressed,
                disabled,
                selected: flat(c(0xededed), c(0xededed), c(0x000000)),
            },
            radio,
            field,
            search: field,
            scroller: Scroller {
                track: Gradient::flat(T),
                track_border: T,
                thumb: flat(c(0x454545), c(0x454545), c(0xededed)),
                thumb_inactive: flat(c(0x2e2e2e), c(0x2e2e2e), c(0xededed)),
                arrow: c(0x878787),
                track_rows: &[],
                arrow_rows: &[],
                // designed: Geist's flat track has no cap shading.
                cap_shadow: T,
            },
            table_header: flat(c(0x2e2e2e), c(0x1a1a1a), c(0xa1a1a1)),
            popup,
            popup_textured: popup,
            progress,
            slider,
            help,
        },
        lists: Lists {
            source_active: selection(c(0x292929), c(0xededed)),
            source_inactive: selection(c(0x1f1f1f), c(0xa1a1a1)),
            source_unfocused: selection(c(0x232323), c(0xededed)),
            source_secondary_active: selection(c(0x292929), c(0xededed)),
            source_secondary_inactive: selection(c(0x1f1f1f), c(0xa1a1a1)),
            table_active: selection(c(0x0b3a5c), c(0xededed)),
            table_inactive: selection(c(0x292929), c(0xededed)),
            menu: selection(c(0x292929), c(0xededed)),
        },
        traffic: traffic(c(0x454545), c(0x2e2e2e)),
        // designed: no drawn grip (flat chrome); the corner still resizes.
        grip: ResizeGrip {
            visible: false,
            bar: GRIP_NONE,
            body: GRIP_NONE,
            hit: 15.0,
        },
    }
}

/// Geist Light: designed on the same role scale.
pub fn geist_light() -> Theme {
    // Role scale (vercel.md §4): 100 #ffffff bg, 200 #fafafa hover, 300 #f2f2f2
    // active, 400 #e6e6e6 border, 500 #c9c9c9 strong border, 600 #8f8f8f
    // muted text, 900 #666666 secondary text, 1000 #171717 primary text.
    let normal = flat(c(0xe6e6e6), c(0xffffff), c(0x171717));
    let pressed = flat(c(0xc9c9c9), c(0xf2f2f2), c(0x171717));
    let disabled = flat(c(0xebebeb), c(0xfafafa), c(0x8f8f8f));
    let selected = flat(c(0x0070f3), c(0xe6f1fe), c(0x171717));
    let field = Field {
        border_top: c(0xe6e6e6),
        border: c(0xe6e6e6),
        border_bottom: c(0xe6e6e6),
        inner_shadow: c(0xffffff),
        fill: c(0xffffff),
        fill_disabled: c(0xfafafa),
        text: c(0x171717),
        placeholder: c(0x8f8f8f),
        caret: c(0x171717),
        selection: c(0xcce3fd),
    };
    let popup = PopUp {
        body: BevelStates {
            normal,
            pressed,
            disabled,
            selected: normal,
        },
        cap: BevelStates {
            normal: flat(c(0x0070f3), c(0xe6f1fe), c(0x171717)),
            pressed: flat(c(0x005bb5), c(0xe6f1fe), c(0x171717)),
            disabled,
            selected: flat(c(0x0070f3), c(0xe6f1fe), c(0x171717)),
        },
        divider: c(0xc9c9c9),
        arrow: c(0x171717),
    };
    let radio = Radio {
        states: BevelStates {
            normal: flat(c(0xc9c9c9), c(0xffffff), c(0x171717)),
            pressed,
            disabled,
            selected: flat(c(0x0070f3), c(0xe6f1fe), c(0x171717)),
        },
        dot: c(0x171717),
    };
    let slider = Slider {
        track: flat(c(0xe6e6e6), c(0xebebeb), c(0x171717)),
        track_disabled: flat(c(0xebebeb), c(0xfafafa), c(0x8f8f8f)),
        round: BevelStates {
            normal: flat(c(0xc9c9c9), c(0xffffff), c(0x171717)),
            pressed,
            disabled,
            selected: pressed,
        },
        pointed: BevelStates {
            normal: flat(c(0x0070f3), c(0xe6f1fe), c(0x171717)),
            pressed,
            disabled,
            selected: pressed,
        },
        tick: c(0x8f8f8f),
    };
    let progress = Progress {
        track: flat(c(0xe6e6e6), c(0xf2f2f2), c(0x171717)),
        fill: flat(c(0x0070f3), c(0x0070f3), c(0xffffff)),
        barber_light: flat(c(0xf2f2f2), c(0xf2f2f2), c(0x171717)),
        barber_dark: flat(c(0x0070f3), c(0x0070f3), c(0xffffff)),
        radius: f32::MAX,
        spinner: c(0x666666),
        spinner_dim: c(0xc9c9c9),
    };
    let help = Help {
        states: BevelStates {
            normal,
            pressed,
            disabled,
            selected: pressed,
        },
        glyph: c(0x171717),
    };
    Theme {
        id: ThemeId::GEIST_LIGHT,
        provenance: Provenance::Designed,
        type_: typography(),
        colors: Colors {
            window: c(0xfafafa),
            content: c(0xffffff),
            canvas: c(0x000000),
            text: c(0x171717),
            text_secondary: c(0x666666),
            text_disabled: c(0x8f8f8f),
            text_on_accent: c(0xffffff),
            title: c(0x171717),
            title_inactive: c(0x8f8f8f),
            title_engrave: T,
            separator: c(0xebebeb),
            divider: c(0xe6e6e6),
            focus_ring: c(0x0070f3),
            accent: c(0x0070f3),
            link: c(0x0062d1),
            success: c(0x1a7f37),
            warning: c(0xa35200),
            error: c(0xcb2a2f),
            info: c(0x0062d1),
            sidebar: c(0xfafafa),
            sidebar_inactive: c(0xfafafa),
            sidebar_heading: c(0x666666),
            source_secondary: c(0xfafafa),
            source_secondary_heading: c(0x666666),
            row_alternate: c(0xfafafa),
            group_fill: c(0xffffff),
            group_border: c(0xe6e6e6),
            group_border_top: c(0xe6e6e6),
            // Geist boxes are flat: one border colour, no inner shading.
            group_border_bottom: c(0xe6e6e6),
            group_highlight: c(0xffffff),
            group_highlight_bottom: c(0xffffff),
            group_shadow: [c(0xffffff); 3],
            well_fill: c(0xfafafa),
            well_border: c(0xe6e6e6),
            well_shadow: c(0xc9c9c9),
            band_rule: c(0xebebeb),
            menu: c(0xffffff),
            menu_text: c(0x171717),
            menu_text_disabled: c(0x8f8f8f),
            menu_separator: c(0xebebeb),
            menu_separator_highlight: T,
            menu_border: c(0xc9c9c9),
            menu_shadow: c(0x000000).alpha(40),
            tooltip: c(0x171717),
            tooltip_border: c(0x171717),
            tooltip_text: c(0xfafafa),
            shadow: c(0x000000).alpha(40),
            scrim: c(0x000000).alpha(50),
        },
        metrics: metrics(),
        chrome: ChromeSet {
            active: chrome(c(0xffffff), c(0xffffff), c(0xe6e6e6)),
            inactive: chrome(c(0xfafafa), c(0xfafafa), c(0xebebeb)),
            bottom_active: chrome(c(0xffffff), c(0xffffff), c(0xe6e6e6)),
            bottom_inactive: chrome(c(0xfafafa), c(0xfafafa), c(0xebebeb)),
        },
        controls: Controls {
            push: BevelStates {
                normal,
                pressed,
                disabled,
                selected,
            },
            push_default: flat(c(0x171717), c(0x171717), c(0xffffff)),
            textured: BevelStates {
                normal,
                pressed,
                disabled,
                selected: pressed,
            },
            gradient: BevelStates {
                normal,
                pressed,
                disabled,
                selected: pressed,
            },
            round: BevelStates {
                normal,
                pressed,
                disabled,
                selected: pressed,
            },
            recessed: BevelStates {
                normal,
                pressed,
                disabled,
                selected: pressed,
            },
            segment: BevelStates {
                normal,
                pressed,
                disabled,
                selected: flat(c(0xc9c9c9), c(0xf2f2f2), c(0x171717)),
            },
            checkbox: BevelStates {
                normal: flat(c(0xc9c9c9), c(0xffffff), c(0xffffff)),
                pressed,
                disabled,
                selected: flat(c(0x171717), c(0x171717), c(0xffffff)),
            },
            radio,
            field,
            search: field,
            scroller: Scroller {
                track: Gradient::flat(T),
                track_border: T,
                thumb: flat(c(0xc9c9c9), c(0xc9c9c9), c(0x171717)),
                thumb_inactive: flat(c(0xe6e6e6), c(0xe6e6e6), c(0x171717)),
                arrow: c(0x8f8f8f),
                track_rows: &[],
                arrow_rows: &[],
                // designed: Geist's flat track has no cap shading.
                cap_shadow: T,
            },
            table_header: flat(c(0xebebeb), c(0xfafafa), c(0x666666)),
            popup,
            popup_textured: popup,
            progress,
            slider,
            help,
        },
        lists: Lists {
            source_active: selection(c(0xebebeb), c(0x171717)),
            source_inactive: selection(c(0xf2f2f2), c(0x666666)),
            source_unfocused: selection(c(0xe6e6e6), c(0x171717)),
            source_secondary_active: selection(c(0xebebeb), c(0x171717)),
            source_secondary_inactive: selection(c(0xf2f2f2), c(0x666666)),
            table_active: selection(c(0xe6f1fe), c(0x171717)),
            table_inactive: selection(c(0xf2f2f2), c(0x171717)),
            menu: selection(c(0xf2f2f2), c(0x171717)),
        },
        traffic: traffic(c(0xc9c9c9), c(0xe6e6e6)),
        // designed: no drawn grip (flat chrome); the corner still resizes.
        grip: ResizeGrip {
            visible: false,
            bar: GRIP_NONE,
            body: GRIP_NONE,
            hit: 15.0,
        },
    }
}
