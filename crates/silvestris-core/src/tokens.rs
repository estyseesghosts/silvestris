//! Resolved theme tokens: semantic colours, layered materials and metrics.
//!
//! Every theme supplies every field; there is no fallback chain, so the type
//! system is the conformance check for completeness (#121). Painters read
//! materials, never theme names.

use crate::color::{Gradient, Rgba8};
use crate::theme::ThemeId;

/// Where a theme's values come from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provenance {
    /// Sampled from Mac OS X 10.6 captures (reference/measurements.json).
    Measured,
    /// Designed for Silvestris; documented in docs/themes.md.
    Designed,
    /// Carried over from an existing product (Lotus's Geist palette).
    Inherited,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub id: ThemeId,
    pub provenance: Provenance,
    pub type_: Typography,
    pub colors: Colors,
    pub metrics: Metrics,
    pub chrome: ChromeSet,
    pub controls: Controls,
    pub lists: Lists,
    pub traffic: TrafficLights,
    pub grip: ResizeGrip,
}

/// Consumer-supplied font family substitutions for a resolved theme.
///
/// Sizes and every other token stay exactly as measured; only the family names
/// change. Applications use this to adopt a house font without forking a theme,
/// and can apply it globally or to one [`ThemeId`]. A specific theme's override
/// wins over the global one.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FontOverrides {
    pub ui: Option<&'static str>,
    pub mono: Option<&'static str>,
}

impl FontOverrides {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn ui(mut self, family: &'static str) -> Self {
        self.ui = Some(family);
        self
    }

    #[must_use]
    pub fn mono(mut self, family: &'static str) -> Self {
        self.mono = Some(family);
        self
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ui.is_none() && self.mono.is_none()
    }
}

impl Theme {
    /// Replace only the UI/mono family names, keeping all measured sizes.
    pub fn apply_font_overrides(&mut self, overrides: FontOverrides) {
        if let Some(ui) = overrides.ui {
            self.type_.ui = ui;
        }
        if let Some(mono) = overrides.mono {
            self.type_.mono = mono;
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Typography {
    /// Family name as registered by silvestris-fonts.
    pub ui: &'static str,
    pub mono: &'static str,
    pub size_regular: f32,
    pub size_small: f32,
    pub size_mini: f32,
    pub size_mono: f32,
    /// Window title.
    pub size_title: f32,
    /// Source-list section headings (upper case, bold).
    pub size_heading: f32,
    pub size_menu: f32,
    /// Lists and tables (HIG p. 129 "view font").
    pub size_view: f32,
    /// Toolbar item labels (HIG p. 129 "label font").
    pub size_label: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Colors {
    /// Window body behind controls (System Preferences, dialogs).
    pub window: Rgba8,
    /// Document/content areas: tables, lists, text views.
    pub content: Rgba8,
    /// Neutral canvas for consumer-owned content (emulator screens, previews).
    pub canvas: Rgba8,
    pub text: Rgba8,
    pub text_secondary: Rgba8,
    pub text_disabled: Rgba8,
    pub text_on_accent: Rgba8,
    pub title: Rgba8,
    pub title_inactive: Rgba8,
    /// One-pixel engraving under chrome text (white in Aqua Light).
    pub title_engrave: Rgba8,
    pub separator: Rgba8,
    /// Divider between the source list and content, and split-view seams.
    pub divider: Rgba8,
    pub focus_ring: Rgba8,
    pub accent: Rgba8,
    pub link: Rgba8,
    pub success: Rgba8,
    pub warning: Rgba8,
    pub error: Rgba8,
    pub info: Rgba8,
    pub sidebar: Rgba8,
    pub sidebar_inactive: Rgba8,
    pub sidebar_heading: Rgba8,
    /// Secondary source list (Mail's mailbox list, Address Book groups).
    pub source_secondary: Rgba8,
    pub source_secondary_heading: Rgba8,
    pub row_alternate: Rgba8,
    pub group_fill: Rgba8,
    pub group_border: Rgba8,
    /// Top edge of a group/tab-view box, darker than its sides in Aqua.
    pub group_border_top: Rgba8,
    /// Bottom edge of a group/tab-view box, lighter than its sides in Aqua.
    pub group_border_bottom: Rgba8,
    /// Inner column beside each side border.
    pub group_highlight: Rgba8,
    /// Inner row above the bottom edge.
    pub group_highlight_bottom: Rgba8,
    /// Inner shadow rows under the top edge, outermost first.
    pub group_shadow: [Rgba8; 3],
    /// Recessed content well (HIG pp. 361-363; M: groupbox.fill/left).
    pub well_fill: Rgba8,
    pub well_border: Rgba8,
    pub well_shadow: Rgba8,
    /// Rule between preferences-overview category bands.
    pub band_rule: Rgba8,
    pub menu: Rgba8,
    pub menu_text: Rgba8,
    pub menu_text_disabled: Rgba8,
    pub menu_separator: Rgba8,
    pub menu_separator_highlight: Rgba8,
    /// Outer hairline around a floating menu, painted outside the body over
    /// the shadow.
    pub menu_border: Rgba8,
    /// Floating-menu drop shadow (offset, blur and spread are metrics).
    pub menu_shadow: Rgba8,
    pub tooltip: Rgba8,
    pub tooltip_border: Rgba8,
    pub tooltip_text: Rgba8,
    /// Popup, menu and overlay drop shadow.
    pub shadow: Rgba8,
    /// Dimming behind sheets and modal alerts.
    pub scrim: Rgba8,
}

/// Unified title/tool/bottom bar surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Chrome {
    /// First row under the window's top edge.
    pub highlight: Rgba8,
    pub fill: Gradient,
    /// Row separating the bar from content.
    pub separator: Rgba8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChromeSet {
    pub active: Chrome,
    pub inactive: Chrome,
    pub bottom_active: Chrome,
    pub bottom_inactive: Chrome,
}

/// How a rounded control derives its corner radius from its height.
///
/// Aqua's capsule controls keep the measured half-height rule; Geist uses
/// fixed radii. Painters clamp both variants to the control's half height so
/// short controls never invert their corners.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CornerRule {
    Capsule,
    Fixed(f32),
}

impl CornerRule {
    #[must_use]
    pub fn radius(self, height: f32) -> f32 {
        let half = (height / 2.0).max(0.0);
        match self {
            CornerRule::Capsule => half,
            CornerRule::Fixed(value) => value.clamp(0.0, half),
        }
    }
}

/// A bezel painted as stacked layers, outer to inner (handbook 10 paint
/// stack): emboss line below, 1 px border ring, then an upper and a lower
/// fill band split at `split` of the inner height. Two bands of two-stop
/// gradients reproduce the measured Aqua gloss (bright top band, step, then a
/// lower band brightening toward the bottom).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bevel {
    pub border: Gradient,
    pub upper: Gradient,
    pub lower: Gradient,
    /// Fraction of the inner height covered by `upper`.
    pub split: f32,
    /// One-pixel line drawn under the bezel; transparent for none.
    pub emboss: Rgba8,
    pub text: Rgba8,
}

impl Bevel {
    /// A bezel with one continuous fill.
    pub fn plain(border: Gradient, fill: Gradient, emboss: Rgba8, text: Rgba8) -> Self {
        Bevel {
            border,
            upper: Gradient::new(fill.from, fill.at(0.5)),
            lower: Gradient::new(fill.at(0.5), fill.to),
            split: 0.5,
            emboss,
            text,
        }
    }
}

/// States of a push-style control. 10.6 push buttons have no hover state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BevelStates {
    pub normal: Bevel,
    pub pressed: Bevel,
    pub disabled: Bevel,
    /// Latched/selected (segments, toggles, the default button).
    pub selected: Bevel,
}

/// Text input well.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Field {
    pub border_top: Rgba8,
    pub border: Rgba8,
    pub border_bottom: Rgba8,
    /// Row under the top border.
    pub inner_shadow: Rgba8,
    pub fill: Rgba8,
    pub fill_disabled: Rgba8,
    pub text: Rgba8,
    pub placeholder: Rgba8,
    /// Caret and text selection.
    pub caret: Rgba8,
    pub selection: Rgba8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scroller {
    /// Track gradient across the bar (left→right for a vertical bar).
    pub track: Gradient,
    pub track_border: Rgba8,
    /// Thumb drawn across the bar; `upper` is the leading half.
    pub thumb: Bevel,
    /// Pointer-hover thumb material. Aqua's painter keeps using `thumb`.
    pub thumb_hover: Bevel,
    pub thumb_inactive: Bevel,
    pub arrow: Rgba8,
    /// Measured track cross-section, leading edge first, painted row by row
    /// when present; otherwise `track` is used. Resampled for other sizes.
    pub track_rows: &'static [Rgba8],
    /// Measured arrow-box cross-section; empty paints the boxes like the
    /// track.
    pub arrow_rows: &'static [Rgba8],
    /// Darkest row of the shading where the track meets the arrow boxes;
    /// the rows before it fall off along a fixed measured profile.
    pub cap_shadow: Rgba8,
}

/// A pop-up button: the body and the trailing arrow cap are separate
/// materials in 10.6 (M: popup-button.regular body rows and cap columns).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PopUp {
    pub body: BevelStates,
    pub cap: BevelStates,
    /// Hairline at the body/cap seam.
    pub divider: Rgba8,
    /// Paired up/down arrow ink.
    pub arrow: Rgba8,
}

/// A radio button's circular bezel and inner dot. The dot is a separate colour
/// because the selected bezel's body is blue (M: radio.selected).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Radio {
    pub states: BevelStates,
    pub dot: Rgba8,
}

/// Slider materials. 10.6 pairs a shallow inset track with either a round
/// thumb (view body) or a pointed shield thumb (Dock/Finder settings).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slider {
    pub track: Bevel,
    pub track_disabled: Bevel,
    pub round: BevelStates,
    pub pointed: BevelStates,
    pub tick: Rgba8,
}

/// Progress materials: the well, the determinate fill, the barber-pole stripe
/// pair and the asynchronous spinner's two tones.
///
/// The barber stripes are two vertical gloss profiles. `border` is the
/// stripe-tinted top row; `upper` and `lower` meet at `split` (fraction of
/// the bar height). The bar alternates them in 45 degree stripes with short
/// anti-aliased edges.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Progress {
    pub track: Bevel,
    pub fill: Bevel,
    pub barber_light: Bevel,
    pub barber_dark: Bevel,
    /// Corner radius of the bar; clamped to half its height, so a large
    /// value draws a capsule.
    pub radius: f32,
    pub spinner: Rgba8,
    pub spinner_dim: Rgba8,
}

/// Help (question-mark) button ring and glyph (M: appearance help button).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Help {
    pub states: BevelStates,
    pub glyph: Rgba8,
}

/// Historical Snow Leopard installer Continue button: a fixed circular
/// control with a built-in two-dot-and-triangle glyph. The material is
/// independent of the standard push button and round toolbar control.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InstallerContinue {
    /// Non-default action.
    pub normal: Bevel,
    /// Default action; this material carries the breathing animation.
    pub default: Bevel,
    pub pressed: Bevel,
    pub disabled: Bevel,
    pub glyph: Rgba8,
    /// Glyph ink for the default-action material (Geist inverts its fill, so
    /// the default glyph is not the normal glyph).
    pub glyph_default: Rgba8,
    pub glyph_pressed: Rgba8,
    pub glyph_disabled: Rgba8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Controls {
    pub push: BevelStates,
    /// Pointer-hover material. Aqua has no hover bevel, so it uses push.normal.
    pub hover: Bevel,
    /// Blue default button (Return).
    pub push_default: Bevel,
    /// Rectangular toolbar/bottom-bar control (NSTexturedRoundedBezelStyle).
    pub textured: BevelStates,
    pub gradient: BevelStates,
    /// Circular textured control for toolbars (HIG p. 261).
    pub round: BevelStates,
    /// Inset/recessed rectangular control (HIG pp. 257-261).
    pub recessed: BevelStates,
    /// Capsule tab/segment (tab views, preference pane switches).
    pub segment: BevelStates,
    pub checkbox: BevelStates,
    pub radio: Radio,
    pub field: Field,
    pub search: Field,
    pub scroller: Scroller,
    pub table_header: Bevel,
    /// Header cell of the sort column. `border.from` is its right divider,
    /// `border.to` its bottom line.
    pub table_header_sorted: Bevel,
    /// Sort direction triangle in the header.
    pub sort_arrow: Rgba8,
    /// Optional table grid lines.
    pub table_grid: Rgba8,
    pub popup: PopUp,
    /// The capsule/textured pop-up used inside unified toolbars (#2).
    pub popup_textured: PopUp,
    pub progress: Progress,
    pub slider: Slider,
    pub help: Help,
    /// Fixed circular installer Continue control (HIG setup assistants);
    /// independent of `push_default`, `round` and `textured`.
    pub installer_continue: InstallerContinue,
}

/// Selection band: a line on top of a gradient.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Selection {
    pub line: Rgba8,
    pub fill: Gradient,
    pub text: Rgba8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lists {
    pub source_active: Selection,
    /// Active window, list not focused (M: Finder/Open sidebar selection).
    pub source_unfocused: Selection,
    pub source_inactive: Selection,
    /// Secondary source lists (Mail, Address Book).
    pub source_secondary_active: Selection,
    pub source_secondary_inactive: Selection,
    pub table_active: Selection,
    pub table_inactive: Selection,
    pub menu: Selection,
}

/// Window-control artwork parameters. The sprite rasterizer
/// (`sprites::traffic_light`) draws original art from these.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrafficLight {
    /// Ring at the top and bottom of the disc; `SphereShape::ring_pow` shapes
    /// the blend between them.
    pub ring_top: Rgba8,
    pub ring_bottom: Rgba8,
    /// Saturated body colour under the specular cap.
    pub deep: Rgba8,
    /// Brightest lower-body colour, centre of the radial glow.
    pub glow: Rgba8,
    /// Specular highlight under the top edge.
    pub specular: Rgba8,
    /// Glyph shown on hover (×, −, +).
    pub glyph: Rgba8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrafficLights {
    pub close: TrafficLight,
    pub minimize: TrafficLight,
    pub zoom: TrafficLight,
    /// Inactive window or disabled control.
    pub idle: TrafficLight,
    /// Line under each button (light emboss in Aqua Light).
    pub emboss: Rgba8,
    pub shape: SphereShape,
}

/// Shared shape of the window-control sphere, in units of the disc radius
/// (`u`, `v` run -1..1 from the centre, `v` downward) unless noted. The model
/// is mirrored exactly by tools/metrology/fit_traffic.py, which fits it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SphereShape {
    /// Body darkens toward the ring above `v = top_end`, by up to `top_k`.
    pub top_end: f32,
    pub top_k: f32,
    /// Elliptical glow toward `glow`: half-axes `glow_x`, `glow_ry`, centre
    /// `v = glow_y`, falloff exponent `glow_pow`.
    pub glow_x: f32,
    pub glow_y: f32,
    pub glow_ry: f32,
    pub glow_pow: f32,
    /// Shading toward the ring from `d/r = edge_start`, by up to `edge_k`.
    pub edge_start: f32,
    pub edge_k: f32,
    /// Elliptical specular cap: half-axes `spec_x`, `spec_ry`, centre `v = spec_y`.
    pub spec_x: f32,
    pub spec_y: f32,
    pub spec_ry: f32,
    pub spec_pow: f32,
    /// Disc radius as a fraction of half the diameter (may exceed 1; the
    /// sprite is clipped to the diameter square).
    pub radius: f32,
    /// Ring width in logical pixels.
    pub ring_width: f32,
    /// Ring colour = mix(ring_top, ring_bottom, (y / diameter) ^ ring_pow).
    pub ring_pow: f32,
}

impl SphereShape {
    /// A flat disc with a ring: zero weights, and the glow and specular
    /// ellipses centred outside the disc (v = 3) with radius 1 never reach it.
    pub const FLAT: SphereShape = SphereShape {
        top_end: 0.0,
        top_k: 0.0,
        glow_x: 1.0,
        glow_y: 3.0,
        glow_ry: 1.0,
        glow_pow: 1.0,
        edge_start: 0.0,
        edge_k: 0.0,
        spec_x: 1.0,
        spec_y: 3.0,
        spec_ry: 1.0,
        spec_pow: 1.0,
        radius: 1.0,
        ring_width: 1.0,
        ring_pow: 1.0,
    };
}

#[derive(Clone, Debug, PartialEq)]
pub struct Metrics {
    pub window_radius: f32,
    pub titlebar_height: f32,
    /// Unified toolbar band under the title for icon-only rectangular items.
    pub toolbar_height: f32,
    /// Unified toolbar band for icon-over-label items.
    pub toolbar_labelled_height: f32,
    /// Regular bottom bar: 4 px above a regular control, the control, then
    /// 6 px below it (HIG p. 365).
    pub bottom_bar_height: f32,
    /// Small bottom bar: 2 px above and below a small control (HIG p. 366).
    pub bottom_bar_small_height: f32,
    pub traffic_diameter: f32,
    pub traffic_left: f32,
    pub traffic_top: f32,
    pub traffic_pitch: f32,
    pub push_height: f32,
    pub push_small_height: f32,
    pub push_mini_height: f32,
    pub push_radius: f32,
    /// Fixed diameter of the installer Continue button (the reference
    /// footprint is 31 x 30 px; the control uses a 30 px square).
    pub installer_continue_diameter: f32,
    /// Arrow-cap width of a pop-up, per control size (M: popup-button.regular
    /// cap is 21 logical px at height 21).
    pub popup_cap: [f32; 3],
    /// Horizontal space from the bezel border to the label ink, per
    /// control size (regular, small, mini). Width = label + 2 * padding.
    pub push_padding: [f32; 3],
    /// Vertical nudge of the label from flex centring, per control size, so
    /// the label ink sits where the reference places it with the bundled font.
    pub push_label_dy: [f32; 3],
    pub textured_height: f32,
    pub textured_radius: f32,
    pub segment_height: f32,
    /// Theme-resolved corner policy for rounded segments.
    pub segment_corner: CornerRule,
    pub field_height: f32,
    pub field_small_height: f32,
    /// Theme-resolved corner policy for ordinary and search fields.
    pub field_corner: CornerRule,
    pub search_corner: CornerRule,
    /// Bezel border to the text origin.
    pub field_text_inset: f32,
    /// Caret: width, and rows kept clear above and below it inside the borders.
    pub caret_width: f32,
    pub caret_inset_top: f32,
    pub caret_inset_bottom: f32,
    pub checkbox_size: f32,
    pub checkbox_small_size: f32,
    pub checkbox_mini_size: f32,
    pub checkbox_label_gap: f32,
    pub checkbox_radius: f32,
    /// Radio bezel diameter per size (M: radio.selected is 14 logical).
    pub radio_size: [f32; 3],
    /// Selected dot diameter as a fraction of the bezel (M: radio.selected).
    pub radio_dot_fraction: f32,
    pub radio_label_gap: f32,
    /// Vertical pitch between radio rows minus the control height.
    pub radio_row_gap: f32,
    pub source_row_height: f32,
    pub source_indent: f32,
    pub table_row_height: f32,
    pub table_header_height: f32,
    pub menu_item_height: f32,
    pub menu_separator_height: f32,
    pub menu_padding: f32,
    pub menu_radius: f32,
    /// Scroll bar thickness per size (regular, small, mini; HIG p. 227).
    pub scroller_width: [f32; 3],
    /// Focus ring as rows around the control edge: `(offset, opacity)`,
    /// where offset 0 is the control's outermost row, positive is outside
    /// and opacity multiplies `Colors::focus_ring`.
    pub focus_ring: &'static [(f32, f32)],
    pub disclosure_size: f32,
    /// Drawn triangle size inside the 13 px hit box (M: disclosure.collapsed).
    pub disclosure_width: f32,
    pub disclosure_height: f32,
    pub slider_height: f32,
    /// Track thickness per size (M: Dock size slider is 5 logical).
    pub slider_track_height: [f32; 3],
    /// Thumb width and height per size. The pointed thumb is taller than the
    /// round one because its point hangs below the track (M: Dock size slider).
    pub slider_thumb_width: [f32; 3],
    pub slider_thumb_height: [f32; 3],
    /// Tick stroke height, its gap past the farther of the track and the
    /// thumb, and the label gap.
    pub slider_tick_height: f32,
    pub slider_tick_gap: f32,
    pub slider_tick_label_gap: f32,
    /// Progress bar height per size (M: installer bar is 16 logical).
    pub progress_height: [f32; 3],
    /// Asynchronous spinner diameter per size.
    pub spinner_size: [f32; 3],
    /// Menu elevation: offset (x, y), Gaussian sigma and spread in logical
    /// pixels. GPUI uses `BoxShadow::blur_radius` as the sigma.
    pub menu_shadow_offset: [f32; 2],
    pub menu_shadow_blur: f32,
    pub menu_shadow_spread: f32,
    /// Vertical nudge for menu row text, negative is up.
    pub menu_label_dy: f32,
    /// Recessed-well border and inner-shadow thickness.
    pub well_inset: f32,
    pub splitter_hot_zone: f32,
    pub control_spacing: f32,
    pub tab_label_margin: f32,
    /// Generic corner radius for panels and popovers.
    pub panel_radius: f32,
}

/// Size control in the window's lower-right corner (HIG PDF p. 221): three
/// 45-degree lines, 10, 6 and 2 px long, 4 px apart, whose upper ends share
/// one column (M: grip.bar, grip.body). Each line pixel has a `shade` pixel
/// on its left and a `highlight` pixel on its right.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GripStyle {
    /// Distance from the window's right and bottom edges to the lines' right
    /// column and bottom row.
    pub inset: f32,
    pub shade: Rgba8,
    pub line: Rgba8,
    pub highlight: Rgba8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResizeGrip {
    /// Drawn at all; the hit area exists either way.
    pub visible: bool,
    /// Over a bottom bar.
    pub bar: GripStyle,
    /// Over the window body (no bottom bar).
    pub body: GripStyle,
    /// Square hit area at the corner, logical pixels.
    pub hit: f32,
}
