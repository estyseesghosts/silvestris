//! Geist tokens.
//!
//! Both appearances are resolved from one private [`GeistPalette`]. The
//! palette follows the semantic hierarchy in docs/geist.txt: document,
//! window, raised surface, control, border, text, selection and accent. No
//! renderer reads the palette directly; all routes go through `Theme`'s
//! public token structs. Aqua's tokens are defined separately in `aqua.rs`.

use crate::color::{Gradient, Rgba8};
use crate::theme::ThemeId;
use crate::tokens::*;

const fn c(rgb: u32) -> Rgba8 {
    Rgba8::hex(rgb)
}
const T: Rgba8 = Rgba8::TRANSPARENT;

/// The private Geist colour contract. Roles are shared by Dark and Light so
/// both appearances have identical component recipes.
#[derive(Clone, Copy)]
struct GeistPalette {
    /// Document and text-input interior.
    document: Rgba8,
    /// Window, titlebar, toolbar, bottom bar and sidebar.
    window: Rgba8,
    /// Raised preference groups and inset surfaces.
    raised: Rgba8,
    /// Ordinary control fill, and its hover and pressed states.
    control: Rgba8,
    control_hover: Rgba8,
    control_pressed: Rgba8,
    border: Rgba8,
    border_hover: Rgba8,
    border_strong: Rgba8,
    text: Rgba8,
    text_secondary: Rgba8,
    text_disabled: Rgba8,
    /// Focus, links, progress and meaningful selected states.
    focus: Rgba8,
    /// Distinct list/table selection band.
    selection: Rgba8,
    /// High-contrast primary action and its inverted label.
    action: Rgba8,
    action_text: Rgba8,
    success: Rgba8,
    warning: Rgba8,
    error: Rgba8,
    /// Tooltip is a high-contrast floating surface in both appearances.
    tooltip: Rgba8,
    tooltip_text: Rgba8,
    /// Flat scrollbar thumb: active, hover and inactive-window tones.
    thumb: Rgba8,
    thumb_hover: Rgba8,
    thumb_inactive: Rgba8,
    /// Alpha is used only for shadows and scrims.
    shadow_alpha: u8,
    scrim_alpha: u8,
}

impl GeistPalette {
    fn dark() -> Self {
        Self {
            document: c(0x000000),
            window: c(0x0a0a0a),
            raised: c(0x111111),
            control: c(0x171717),
            control_hover: c(0x1f1f1f),
            control_pressed: c(0x292929),
            border: c(0x262626),
            border_hover: c(0x393939),
            border_strong: c(0x505050),
            text: c(0xededed),
            text_secondary: c(0xa1a1a1),
            text_disabled: c(0x878787),
            focus: c(0x0095ff),
            selection: c(0x0b3a5c),
            action: c(0xededed),
            action_text: c(0x000000),
            success: c(0x279447),
            warning: c(0xd99327),
            error: c(0xe34646),
            tooltip: c(0x292929),
            tooltip_text: c(0xededed),
            thumb: c(0x555555),
            thumb_hover: c(0x666666),
            thumb_inactive: c(0x393939),
            shadow_alpha: 160,
            scrim_alpha: 120,
        }
    }

    fn light() -> Self {
        Self {
            document: c(0xffffff),
            window: c(0xfafafa),
            raised: c(0xffffff),
            control: c(0xffffff),
            control_hover: c(0xfafafa),
            control_pressed: c(0xf2f2f2),
            border: c(0xe6e6e6),
            border_hover: c(0xc9c9c9),
            border_strong: c(0x8f8f8f),
            text: c(0x171717),
            text_secondary: c(0x666666),
            text_disabled: c(0x8f8f8f),
            focus: c(0x0070f3),
            selection: c(0xe6f1fe),
            action: c(0x171717),
            action_text: c(0xffffff),
            success: c(0x1a7f37),
            warning: c(0xa35200),
            error: c(0xcb2a2f),
            tooltip: c(0x171717),
            tooltip_text: c(0xfafafa),
            thumb: c(0xa0a0a0),
            thumb_hover: c(0x7b7b7b),
            thumb_inactive: c(0xc9c9c9),
            shadow_alpha: 40,
            scrim_alpha: 50,
        }
    }
}

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

/// Geist typography roles (docs/geist.txt "Typography"). Sizes are shared by
/// both appearances; only Aqua keeps the 2009 HIG small/mini sizes.
fn typography() -> Typography {
    Typography {
        ui: crate::fonts::GEIST_SANS,
        mono: crate::fonts::GEIST_MONO,
        size_regular: 13.0,
        size_small: 12.0,
        size_mini: 11.0,
        size_mono: 12.0,
        size_title: 13.0,
        size_heading: 12.0,
        size_menu: 13.0,
        size_view: 13.0,
        size_label: 11.0,
    }
}

/// Geist geometry: the HIG layout is unchanged; only radii and corner rules
/// differ from the measured Aqua capsule controls.
fn metrics() -> Metrics {
    Metrics {
        window_radius: 6.0,
        titlebar_height: 32.0,
        toolbar_height: 40.0,
        toolbar_labelled_height: 52.0,
        bottom_bar_height: 40.0,
        bottom_bar_small_height: 30.0,
        traffic_diameter: 12.0,
        traffic_left: 12.0,
        traffic_top: 10.0,
        traffic_pitch: 20.0,
        push_height: 28.0,
        push_small_height: 24.0,
        push_mini_height: 20.0,
        push_radius: 4.0,
        installer_continue_diameter: 30.0,
        popup_cap: [18.0, 16.0, 14.0],
        push_padding: [12.0, 10.0, 8.0],
        push_label_dy: [0.0, 0.0, 0.0],
        textured_height: 28.0,
        textured_radius: 4.0,
        segment_height: 28.0,
        segment_corner: CornerRule::Fixed(4.0),
        field_height: 28.0,
        field_small_height: 24.0,
        field_corner: CornerRule::Fixed(4.0),
        search_corner: CornerRule::Fixed(6.0),
        field_text_inset: 8.0,
        caret_width: 1.0,
        caret_inset_top: 6.0,
        caret_inset_bottom: 6.0,
        checkbox_size: 16.0,
        checkbox_small_size: 14.0,
        checkbox_mini_size: 12.0,
        checkbox_label_gap: 6.0,
        checkbox_radius: 3.0,
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
        menu_radius: 6.0,
        scroller_width: [10.0, 8.0, 6.0],
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
        panel_radius: 6.0,
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
        shape: SphereShape::FLAT,
    }
}

/// Resolve one palette into the public token contract shared by every control.
fn resolve(id: ThemeId, p: GeistPalette) -> Theme {
    let normal = flat(p.border, p.control, p.text);
    let pressed = flat(p.border_strong, p.control_pressed, p.text);
    let disabled = flat(p.border, p.raised, p.text_disabled);
    let hover = flat(p.border_hover, p.control_hover, p.text);
    let selected = pressed;
    let field = Field {
        border_top: p.border,
        border: p.border,
        border_bottom: p.border,
        inner_shadow: p.document,
        fill: p.document,
        fill_disabled: p.raised,
        text: p.text,
        placeholder: p.text_disabled,
        caret: p.text,
        selection: p.selection,
    };
    // The popup is one neutral control; its cap is not a blue accent.
    let popup = PopUp {
        body: BevelStates {
            normal,
            pressed,
            disabled,
            selected: normal,
        },
        cap: BevelStates {
            normal,
            pressed,
            disabled,
            selected: normal,
        },
        divider: p.border,
        arrow: p.text,
    };
    let radio = Radio {
        states: BevelStates {
            normal: flat(p.border_strong, p.document, p.text),
            pressed,
            disabled,
            selected: flat(p.focus, p.focus, p.action_text),
        },
        dot: p.action_text,
    };
    let slider = Slider {
        track: flat(p.border, p.control_pressed, p.text),
        track_disabled: flat(p.border, p.raised, p.text_disabled),
        round: BevelStates {
            normal: flat(p.border_strong, p.action, p.action_text),
            pressed,
            disabled,
            selected: pressed,
        },
        pointed: BevelStates {
            normal: flat(p.focus, p.control_pressed, p.text),
            pressed,
            disabled,
            selected: pressed,
        },
        tick: p.text_secondary,
    };
    let progress = Progress {
        track: flat(p.border, p.raised, p.text),
        fill: flat(p.focus, p.focus, p.action_text),
        // Flat presentation: the indeterminate animation uses these two
        // materials but no Aqua barber-pole gloss.
        barber_light: flat(p.control, p.control, p.text),
        barber_dark: flat(p.control_hover, p.control_hover, p.text),
        radius: 4.0,
        spinner: p.text,
        spinner_dim: p.text_secondary,
    };
    let help = Help {
        states: BevelStates {
            normal,
            pressed,
            disabled,
            selected: pressed,
        },
        glyph: p.text,
    };
    Theme {
        id,
        provenance: if id.appearance == crate::Appearance::Dark {
            Provenance::Inherited
        } else {
            Provenance::Designed
        },
        type_: typography(),
        colors: Colors {
            window: p.window,
            content: p.document,
            canvas: p.document,
            text: p.text,
            text_secondary: p.text_secondary,
            text_disabled: p.text_disabled,
            text_on_accent: p.action_text,
            title: p.text,
            title_inactive: p.text_disabled,
            title_engrave: T,
            separator: p.border,
            divider: p.border,
            focus_ring: p.focus,
            accent: p.focus,
            link: p.focus,
            success: p.success,
            warning: p.warning,
            error: p.error,
            info: p.focus,
            sidebar: p.window,
            sidebar_inactive: p.window,
            sidebar_heading: p.text_secondary,
            source_secondary: p.raised,
            source_secondary_heading: p.text_secondary,
            row_alternate: p.raised,
            group_fill: p.raised,
            group_border: p.border,
            group_border_top: p.border,
            group_border_bottom: p.border,
            group_highlight: p.raised,
            group_highlight_bottom: p.raised,
            group_shadow: [p.raised; 3],
            well_fill: p.document,
            well_border: p.border,
            well_shadow: p.document,
            band_rule: p.border,
            menu: p.control,
            menu_text: p.text,
            menu_text_disabled: p.text_disabled,
            menu_separator: p.border,
            menu_separator_highlight: T,
            menu_border: p.border_hover,
            menu_shadow: c(0x000000).alpha(p.shadow_alpha),
            tooltip: p.tooltip,
            tooltip_border: p.border_hover,
            tooltip_text: p.tooltip_text,
            shadow: c(0x000000).alpha(p.shadow_alpha),
            scrim: c(0x000000).alpha(p.scrim_alpha),
        },
        metrics: metrics(),
        chrome: ChromeSet {
            active: chrome(p.window, p.window, p.border),
            inactive: chrome(p.document, p.window, p.border),
            bottom_active: chrome(p.window, p.window, p.border),
            bottom_inactive: chrome(p.document, p.window, p.border),
        },
        controls: Controls {
            push: BevelStates {
                normal,
                pressed,
                disabled,
                selected,
            },
            hover,
            push_default: flat(p.action, p.action, p.action_text),
            installer_continue: InstallerContinue {
                normal,
                default: flat(p.action, p.action, p.action_text),
                pressed,
                disabled,
                glyph: p.text,
                glyph_default: p.action_text,
                glyph_pressed: p.text,
                glyph_disabled: p.text_disabled,
            },
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
                selected,
            },
            checkbox: BevelStates {
                normal: flat(p.border_strong, p.document, p.text),
                pressed,
                disabled,
                selected: flat(p.action, p.action, p.action_text),
            },
            radio,
            field,
            search: field,
            scroller: Scroller {
                track: Gradient::flat(T),
                track_border: T,
                thumb: flat(p.thumb, p.thumb, T),
                thumb_hover: flat(p.thumb_hover, p.thumb_hover, T),
                thumb_inactive: flat(p.thumb_inactive, p.thumb_inactive, T),
                arrow: p.text_disabled,
                track_rows: &[],
                arrow_rows: &[],
                cap_shadow: T,
            },
            table_header: flat(p.border, p.window, p.text_secondary),
            table_header_sorted: flat(p.border, p.window, p.text),
            sort_arrow: p.text,
            table_grid: p.border,
            popup,
            popup_textured: popup,
            progress,
            slider,
            help,
        },
        lists: Lists {
            source_active: selection(p.control_pressed, p.text),
            source_inactive: selection(p.control_hover, p.text_secondary),
            source_unfocused: selection(p.control, p.text),
            source_secondary_active: selection(p.control_pressed, p.text),
            source_secondary_inactive: selection(p.control_hover, p.text_secondary),
            table_active: selection(p.selection, p.text),
            table_inactive: selection(p.control_pressed, p.text),
            menu: selection(p.control_pressed, p.text),
        },
        traffic: traffic(p.border_strong, p.control),
        grip: ResizeGrip {
            visible: false,
            bar: GRIP_NONE,
            body: GRIP_NONE,
            hit: 15.0,
        },
    }
}

/// Geist Dark: a black foundation with subtle surface separation.
pub fn geist_dark() -> Theme {
    resolve(ThemeId::GEIST_DARK, GeistPalette::dark())
}

/// Geist Light: the same component recipes with the light palette.
pub fn geist_light() -> Theme {
    resolve(ThemeId::GEIST_LIGHT, GeistPalette::light())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_and_light_resolve_the_documented_palette_roles() {
        let dark = GeistPalette::dark();
        assert_eq!(dark.document, c(0x000000));
        assert_eq!(dark.window, c(0x0a0a0a));
        assert_eq!(dark.raised, c(0x111111));
        assert_eq!(dark.control, c(0x171717));
        assert_eq!(dark.control_hover, c(0x1f1f1f));
        assert_eq!(dark.control_pressed, c(0x292929));
        assert_eq!(dark.border, c(0x262626));
        assert_eq!(dark.border_hover, c(0x393939));
        assert_eq!(dark.border_strong, c(0x505050));
        assert_eq!(dark.text, c(0xededed));
        assert_eq!(dark.text_secondary, c(0xa1a1a1));
        assert_eq!(dark.text_disabled, c(0x878787));
        assert_eq!(dark.focus, c(0x0095ff));
        assert_eq!(dark.selection, c(0x0b3a5c));
        assert_eq!(dark.action, c(0xededed));
        assert_eq!(dark.action_text, c(0x000000));

        let light = GeistPalette::light();
        assert_eq!(light.document, c(0xffffff));
        assert_eq!(light.window, c(0xfafafa));
        assert_eq!(light.control, c(0xffffff));
        assert_eq!(light.control_hover, c(0xfafafa));
        assert_eq!(light.control_pressed, c(0xf2f2f2));
        assert_eq!(light.border, c(0xe6e6e6));
        assert_eq!(light.border_hover, c(0xc9c9c9));
        assert_eq!(light.text, c(0x171717));
        assert_eq!(light.text_secondary, c(0x666666));
        assert_eq!(light.focus, c(0x0070f3));
        assert_eq!(light.action, c(0x171717));
        assert_eq!(light.action_text, c(0xffffff));
    }

    #[test]
    fn both_appearances_share_geometry_and_component_roles() {
        let dark = geist_dark();
        let light = geist_light();
        assert_eq!(dark.metrics, light.metrics);
        assert_eq!(dark.type_, light.type_);
        assert_eq!(dark.controls.push.normal.upper.from, c(0x171717));
        assert_eq!(light.controls.push.normal.upper.from, c(0xffffff));
        assert_eq!(dark.controls.popup.cap.normal, dark.controls.popup.body.normal);
        assert_eq!(light.controls.popup.cap.normal, light.controls.popup.body.normal);
        assert_eq!(dark.controls.progress.radius, 4.0);
        assert_eq!(light.controls.progress.radius, 4.0);
    }

    #[test]
    fn control_states_and_selection_are_distinct() {
        for theme in [geist_dark(), geist_light()] {
            let states = theme.controls.push;
            assert_ne!(states.normal.upper.from, theme.controls.hover.upper.from);
            assert_ne!(
                theme.controls.hover.upper.from,
                states.pressed.upper.from
            );
            assert_ne!(states.pressed.upper.from, states.disabled.upper.from);
            assert_ne!(states.normal.text, states.disabled.text);
            assert_ne!(
                theme.lists.table_active.fill.from,
                theme.lists.table_inactive.fill.from,
                "selected rows must not look like pressed neutral controls"
            );
            assert_ne!(theme.colors.focus_ring, theme.colors.menu_border);
        }
    }

    #[test]
    fn corner_rules_keep_aqua_capsules_and_geist_fixed_radii() {
        assert_eq!(CornerRule::Capsule.radius(21.0), 10.5);
        assert_eq!(CornerRule::Fixed(4.0).radius(28.0), 4.0);
        assert_eq!(CornerRule::Fixed(6.0).radius(6.0), 3.0);
        let metrics = geist_dark().metrics;
        assert_eq!(metrics.segment_corner, CornerRule::Fixed(4.0));
        assert_eq!(metrics.field_corner, CornerRule::Fixed(4.0));
        assert_eq!(metrics.search_corner, CornerRule::Fixed(6.0));
    }
}
