//! Custom window chrome (#124): unified title/toolbar band with drawn window
//! controls, optional bottom bar, and the platform frame contract
//! (handbook 05, 06, 07).
//!
//! Windows: GPUI's transparent titlebar plus `window_control_area` maps the
//! drawn controls to HTCLOSE / HTMINBUTTON / HTMAXBUTTON and the band to
//! HTCAPTION (gpui_windows events.rs `handle_hit_test_msg`), so minimize,
//! maximize, close, Snap Layouts, Aero Snap and the system menu stay native;
//! DWM draws the shadow and rounds the corners (native.rs).
//! Linux: client-side decorations with a drawn shadow, rounded corners and
//! resize edges, adapted from gpui examples/window_shadow.rs.
//! macOS (not yet a target, handbook 08): the system draws its own buttons
//! at `traffic_light_position`; Silvestris does not draw them.

use gpui::{
    canvas, div, point, prelude::*, px, size, AnyElement, App, Bounds, CursorStyle, Decorations,
    Entity, HitboxBehavior, MouseButton, Pixels, Point, ResizeEdge, SharedString, Size, Window,
    WindowBounds, WindowControlArea, WindowDecorations, WindowHandle, WindowKind, WindowOptions,
};
use silvestris_core::sprites::Glyph;
use silvestris_core::tokens::Theme;

use crate::convert::hsla;
use crate::paint;
use crate::sprites::{self, Light};
use crate::theme::ActiveTheme;
use crate::window_role::WindowRole;

/// Window options for a Silvestris window with custom chrome.
pub fn window_options(
    title: impl Into<SharedString>,
    bounds: Bounds<Pixels>,
    min_size: Size<Pixels>,
    cx: &App,
) -> WindowOptions {
    let theme = cx.theme();
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(gpui::TitlebarOptions {
            title: Some(title.into()),
            appears_transparent: true,
            // macOS places its own buttons here (handbook 08).
            traffic_light_position: Some(point(
                px(theme.metrics.traffic_left),
                px(theme.metrics.traffic_top),
            )),
        }),
        kind: WindowKind::Normal,
        window_min_size: Some(min_size),
        window_decorations: Some(WindowDecorations::Client),
        app_owns_titlebar_drag: true,
        ..Default::default()
    }
}

/// Options for a fixed-size utility window such as a preferences window:
/// HIG p. 244 gives preferences windows no resize control and no zoom. The
/// 10.6 System Preferences capture (hig-09 images) keeps minimize active and
/// shows zoom disabled; GPUI's Windows hit test then refuses HTMAXBUTTON and
/// the resize edges (gpui_windows events.rs, `is_resizable`).
pub fn fixed_window_options(
    title: impl Into<SharedString>,
    bounds: Bounds<Pixels>,
    cx: &App,
) -> WindowOptions {
    WindowOptions {
        is_resizable: false,
        window_min_size: None,
        ..window_options(title, bounds, bounds.size, cx)
    }
}

/// Open a window, apply the native frame and follow OS appearance changes.
pub fn open_window<V: 'static + Render>(
    options: WindowOptions,
    cx: &mut App,
    build: impl FnOnce(&mut Window, &mut App) -> Entity<V>,
) -> anyhow::Result<WindowHandle<V>> {
    cx.open_window(options, |window, cx| {
        crate::native::configure_frame(window);
        window
            .observe_window_appearance(|window, cx| {
                crate::theme::system_appearance_changed(window.appearance(), cx);
            })
            .detach();
        build(window, cx)
    })
}

/// Shadow inset used by Linux client decorations, in logical pixels.
/// Fitted to M: win.shadow.left / top / bottom: the shadow reaches ~25 px at
/// the sides, ~18 px above and >30 px below (a downward offset).
const CLIENT_SHADOW: f32 = 24.0;

/// Draws the window background, chrome and controls around `content`.
#[derive(IntoElement)]
pub struct WindowFrame {
    title: SharedString,
    toolbar: Option<AnyElement>,
    toolbar_height: f32,
    bottom_bar: Option<AnyElement>,
    content: Option<AnyElement>,
    show_title: bool,
    fixed: bool,
    role: Option<WindowRole>,
}

impl WindowFrame {
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            toolbar: None,
            toolbar_height: 0.0,
            bottom_bar: None,
            content: None,
            show_title: true,
            fixed: false,
            role: None,
        }
    }

    /// Apply a window role (#20): its resizability, title visibility and which
    /// title-bar buttons are live. open_role already set the platform side;
    /// this makes the drawn frame agree with it.
    pub fn role(mut self, role: WindowRole) -> Self {
        self.role = Some(role);
        self
    }

    /// Unified toolbar under the title row. `height` is
    /// `Metrics::toolbar_height` or `toolbar_labelled_height`.
    pub fn toolbar(mut self, toolbar: impl IntoElement, height: f32) -> Self {
        self.toolbar = Some(toolbar.into_any_element());
        self.toolbar_height = height;
        self
    }

    /// Bottom bar (HIG "bottom-bar"; M: bottombar.column).
    pub fn bottom_bar(mut self, bar: impl IntoElement) -> Self {
        self.bottom_bar = Some(bar.into_any_element());
        self
    }

    /// Hide the title text (e.g. when a toolbar fills the title row).
    pub fn hide_title(mut self) -> Self {
        self.show_title = false;
        self
    }

    /// Fixed-size window (pair with `fixed_window_options`): no size control
    /// and a disabled zoom button.
    pub fn fixed_size(mut self) -> Self {
        self.fixed = true;
        self
    }

    pub fn child(mut self, content: impl IntoElement) -> Self {
        self.content = Some(content.into_any_element());
        self
    }
}

fn traffic_lights(
    theme: &Theme,
    active: bool,
    enabled: [bool; 3],
    titlebar_height: f32,
) -> impl IntoElement {
    let m = &theme.metrics;
    let d = px(m.traffic_diameter);
    // Hit boxes span the pitch so the tiny targets are easier to hit without
    // overlapping (handbook 12 "larger accessible hit area (not visually
    // larger) that does not overlap"); the visible sphere stays 14 px.
    let hit_w = px(m.traffic_pitch);
    let hit_h = px(titlebar_height);
    // Whole pixels: layout rounds a half-pixel box origin, which moved the
    // sphere off `traffic_left` (14 px sphere in a 21 px pitch leaves 3.5).
    let pad = ((hit_w - d) / 2.0).floor();
    let lights = [
        (
            Light::Close,
            Glyph::Close,
            WindowControlArea::Close,
            "Close",
        ),
        (
            Light::Minimize,
            Glyph::Minimize,
            WindowControlArea::Min,
            "Minimize",
        ),
        (Light::Zoom, Glyph::Zoom, WindowControlArea::Max, "Zoom"),
    ];
    div()
        .id("silvestris-traffic-lights")
        .group("silvestris-traffic-lights")
        // Block the band's Drag hitbox: GPUI returns the first control area
        // whose hitbox is under the mouse (window.rs on_hit_test_window_control)
        // and BlockMouse stops the walk at this group (window.rs hit_test), so
        // the lights inside resolve to Close/Min/Max and the group still hovers.
        .occlude()
        .absolute()
        .top_0()
        .left(px(m.traffic_left) - pad)
        .h(hit_h)
        .flex()
        .flex_row()
        .children(lights.into_iter().enumerate().map(
            move |(index, (light, glyph, area, label))| {
                // A disabled button keeps the idle sphere and shows no glyph.
                let enabled = enabled[index];
                let base = if active && enabled {
                    light
                } else {
                    Light::Idle
                };
                div()
                    .id(label)
                    .role(gpui::Role::Button)
                    .aria_label(label)
                    .w(hit_w)
                    .h(hit_h)
                    .relative()
                    .when(enabled, |el| {
                        el.debug_selector(move || format!("traffic-{label}-enabled"))
                    })
                    .when(!enabled, |el| {
                        el.debug_selector(move || format!("traffic-{label}-disabled"))
                            .aria_label(format!("{label} (unavailable)"))
                    })
                    .when(enabled, |el| el.window_control_area(area))
                    .when(!cfg!(windows) && enabled, |el| {
                        el.on_click(move |_, window, _| match area {
                            WindowControlArea::Close => window.remove_window(),
                            WindowControlArea::Min => window.minimize_window(),
                            _ => window.zoom_window(),
                        })
                    })
                    .child(
                        canvas(
                            |_, _, _| (),
                            move |bounds, _, window, cx| {
                                let sphere = Bounds::new(
                                    point(bounds.origin.x + pad, bounds.origin.y + px(m_top(cx))),
                                    size(d, d),
                                );
                                // Emboss: the disc repeated one hairline lower in
                                // `traffic.emboss`, showing as a crescent under the
                                // sphere (M: traffic.close.column row 18, d8 over c3).
                                let emboss = cx.theme().traffic.emboss;
                                if emboss.a > 0 {
                                    let low =
                                        sphere + point(px(0.), crate::convert::hairline(window));
                                    window.paint_quad(
                                        gpui::fill(low, crate::convert::rgba(emboss))
                                            .corner_radii(d / 2.0),
                                    );
                                }
                                let image = sprites::traffic(base, Glyph::None, window, cx);
                                window
                                    .paint_image(sphere, sphere, 0.0.into(), image, 0, false)
                                    .ok();
                            },
                        )
                        .absolute()
                        .size_full(),
                    )
                    // Hovering the group shows colour and glyphs, also in an
                    // inactive window (10.6 behaviour).
                    .when(enabled, |el| {
                        el.child(
                            div()
                                .absolute()
                                .size_full()
                                .opacity(0.)
                                .group_hover("silvestris-traffic-lights", |s| s.opacity(1.))
                                .child(
                                    canvas(
                                        |_, _, _| (),
                                        move |bounds, _, window, cx| {
                                            let sphere = Bounds::new(
                                                point(
                                                    bounds.origin.x + pad,
                                                    bounds.origin.y + px(m_top(cx)),
                                                ),
                                                size(d, d),
                                            );
                                            let image = sprites::traffic(light, glyph, window, cx);
                                            window
                                                .paint_image(
                                                    sphere,
                                                    sphere,
                                                    0.0.into(),
                                                    image,
                                                    0,
                                                    false,
                                                )
                                                .ok();
                                        },
                                    )
                                    .size_full(),
                                ),
                        )
                    })
            },
        ))
}

/// Tell the platform about the grip's hit area, only when it changes.
fn native_grip(window: &mut Window, cx: &mut App, size: f32) {
    let last = window.use_keyed_state("silvestris-native-grip", cx, |_, _| -1.0f32);
    if *last.read(cx) != size {
        crate::native::set_resize_grip(window, size);
        last.update(cx, |v, _| *v = size);
    }
}

fn m_top(cx: &App) -> f32 {
    cx.theme().metrics.traffic_top
}

impl RenderOnce for WindowFrame {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let m = &theme.metrics;
        let c = &theme.colors;
        let active = window.is_window_active();
        let decorations = window.window_decorations();
        let maximized = window.is_maximized();
        let chrome = if active {
            theme.chrome.active
        } else {
            theme.chrome.inactive
        };
        let bottom = if active {
            theme.chrome.bottom_active
        } else {
            theme.chrome.bottom_inactive
        };
        let spec = self.role.map(WindowRole::spec);
        let titlebar_height = self
            .role
            .map_or(m.titlebar_height, |role| role.titlebar_height(&theme));
        let band_height = titlebar_height + self.toolbar_height;
        let client = matches!(decorations, Decorations::Client { .. })
            && !cfg!(windows)
            && !cfg!(target_os = "macos");
        let radius = if maximized { 0.0 } else { m.window_radius };
        let draws_lights = !cfg!(target_os = "macos");
        let fixed = self.fixed || spec.is_some_and(|spec| !spec.resizable);
        let show_title = self.show_title && spec.is_none_or(|spec| spec.shows_title);
        let (close, minimize, zoom) = spec.map_or((true, true, true), |spec| {
            (spec.close, spec.minimize, spec.zoom)
        });

        let title_text = |color, dy: f32| {
            div()
                .absolute()
                .left_0()
                .right_0()
                .top(px(dy))
                .h(px(titlebar_height))
                .flex()
                .items_center()
                .justify_center()
                .text_color(hsla(color))
                .child(self.title.clone())
        };

        let band = div()
            .id("silvestris-titlebar")
            .debug_selector(|| "silvestris-titlebar".into())
            .relative()
            .flex_none()
            .w_full()
            .h(px(band_height))
            .window_control_area(WindowControlArea::Drag)
            .when(client, |el| {
                el.on_mouse_down(MouseButton::Left, move |e, window, _| {
                    if e.click_count == 2 && !fixed {
                        window.zoom_window();
                    } else {
                        window.start_window_move();
                    }
                })
                .on_mouse_down(MouseButton::Right, |e, window, _| {
                    window.show_window_menu(e.position)
                })
            })
            .child({
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, _| {
                        paint::chrome(window, bounds, &chrome, radius, false);
                    },
                )
                .absolute()
                .size_full()
            })
            .font_family(theme.type_.ui)
            .text_size(px(theme.type_.size_title))
            .when(show_title, |el| {
                el.when(c.title_engrave.a > 0, |el| {
                    el.child(title_text(c.title_engrave, 1.0))
                })
                .child(title_text(
                    if active { c.title } else { c.title_inactive },
                    0.0,
                ))
            })
            .when(draws_lights, |el| {
                el.child(traffic_lights(
                    &theme,
                    active,
                    [close, minimize, zoom],
                    titlebar_height,
                ))
            })
            .when_some(self.toolbar, |el, toolbar| {
                el.child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .top(px(titlebar_height))
                        .h(px(self.toolbar_height))
                        .child(toolbar),
                )
            });

        // Size control (HIG PDF p. 221). Windows resizes through the native
        // hit test; Linux client decorations start a resize on press.
        let resizable = !maximized && !fixed;
        native_grip(window, cx, if resizable { theme.grip.hit } else { 0.0 });
        let grip_style = if self.bottom_bar.is_some() {
            theme.grip.bar
        } else {
            theme.grip.body
        };
        let grip = div()
            .id("silvestris-resize-grip")
            .absolute()
            .right_0()
            .bottom_0()
            .size(px(theme.grip.hit))
            .when(client, |el| {
                el.cursor(CursorStyle::ResizeUpLeftDownRight)
                    .on_mouse_down(MouseButton::Left, |_, window, _| {
                        window.start_window_resize(ResizeEdge::BottomRight)
                    })
            })
            .when(theme.grip.visible, |el| {
                el.child(
                    canvas(
                        |_, _, _| (),
                        move |bounds, _, window, _| {
                            paint::resize_grip(window, bounds.bottom_right(), &grip_style);
                        },
                    )
                    .size_full(),
                )
            });

        let frame = div()
            .id("silvestris-window")
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(hsla(c.window))
            .font_family(theme.type_.ui)
            .text_size(px(theme.type_.size_regular))
            .text_color(hsla(c.text))
            .rounded(px(radius))
            .child(band)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .relative()
                    .children(self.content),
            )
            .when_some(self.bottom_bar, |el, bar| {
                el.child(
                    div()
                        .relative()
                        .flex_none()
                        .w_full()
                        .h(px(m.bottom_bar_height))
                        .child(
                            canvas(
                                |_, _, _| (),
                                move |bounds, _, window, _| {
                                    paint::chrome(window, bounds, &bottom, 0.0, true);
                                },
                            )
                            .absolute()
                            .size_full(),
                        )
                        .child(bar),
                )
            });

        let frame = frame.when(resizable, |el| el.child(grip));

        if client {
            client_decorated(frame, decorations, radius, window)
        } else {
            frame.into_any_element()
        }
    }
}

/// Linux client decorations: transparent inset with resize edges and a drawn
/// shadow (gpui examples/window_shadow.rs).
fn client_decorated(
    frame: gpui::Stateful<gpui::Div>,
    decorations: Decorations,
    radius: f32,
    window: &mut Window,
) -> AnyElement {
    let Decorations::Client { tiling } = decorations else {
        return frame.into_any_element();
    };
    let inset = px(CLIENT_SHADOW);
    window.set_client_inset(inset);
    let r = px(radius);
    div()
        .id("silvestris-client-decorations")
        .size_full()
        .bg(gpui::transparent_black())
        .child(
            canvas(
                |_bounds, window, _| {
                    window.insert_hitbox(
                        Bounds::new(
                            point(px(0.), px(0.)),
                            window.window_bounds().get_bounds().size,
                        ),
                        HitboxBehavior::Normal,
                    )
                },
                move |_bounds, hitbox, window, _| {
                    let size = window.window_bounds().get_bounds().size;
                    if let Some(edge) = resize_edge(window.mouse_position(), inset, size) {
                        window.set_cursor_style(cursor_for(edge), &hitbox);
                    }
                },
            )
            .size_full()
            .absolute(),
        )
        .when(!tiling.top, |d| d.pt(inset))
        .when(!tiling.bottom, |d| d.pb(inset))
        .when(!tiling.left, |d| d.pl(inset))
        .when(!tiling.right, |d| d.pr(inset))
        .on_mouse_move(|_, window, _| window.refresh())
        .on_mouse_down(MouseButton::Left, move |e, window, _| {
            let size = window.window_bounds().get_bounds().size;
            if let Some(edge) = resize_edge(e.position, inset, size) {
                window.start_window_resize(edge);
            }
        })
        .child(
            frame
                .cursor(CursorStyle::Arrow)
                .when(!tiling.is_tiled(), |el| {
                    // Fit to M: win.shadow.*: peak alpha ~0.5, sigma ~17 px,
                    // offset ~10 px down. Unverified on a Linux compositor.
                    el.shadow(vec![gpui::BoxShadow::new(
                        px(0.),
                        px(10.),
                        gpui::Hsla {
                            h: 0.,
                            s: 0.,
                            l: 0.,
                            a: 0.45,
                        },
                    )
                    .blur_radius(px(CLIENT_SHADOW - 10.))])
                })
                .when(tiling.is_tiled(), |el| el.rounded(px(0.)))
                .when(!tiling.is_tiled(), |el| el.rounded(r))
                .on_mouse_move(|_, _, cx| cx.stop_propagation()),
        )
        .into_any_element()
}

fn cursor_for(edge: ResizeEdge) -> CursorStyle {
    match edge {
        ResizeEdge::Top | ResizeEdge::Bottom => CursorStyle::ResizeUpDown,
        ResizeEdge::Left | ResizeEdge::Right => CursorStyle::ResizeLeftRight,
        ResizeEdge::TopLeft | ResizeEdge::BottomRight => CursorStyle::ResizeUpLeftDownRight,
        ResizeEdge::TopRight | ResizeEdge::BottomLeft => CursorStyle::ResizeUpRightDownLeft,
    }
}

fn resize_edge(pos: Point<Pixels>, inset: Pixels, size: Size<Pixels>) -> Option<ResizeEdge> {
    let corner = inset * 1.5;
    let edge = if pos.y < corner && pos.x < corner {
        ResizeEdge::TopLeft
    } else if pos.y < corner && pos.x > size.width - corner {
        ResizeEdge::TopRight
    } else if pos.y < inset {
        ResizeEdge::Top
    } else if pos.y > size.height - corner && pos.x < corner {
        ResizeEdge::BottomLeft
    } else if pos.y > size.height - corner && pos.x > size.width - corner {
        ResizeEdge::BottomRight
    } else if pos.y > size.height - inset {
        ResizeEdge::Bottom
    } else if pos.x < inset {
        ResizeEdge::Left
    } else if pos.x > size.width - inset {
        ResizeEdge::Right
    } else {
        return None;
    };
    Some(edge)
}

#[cfg(test)]
mod tests {
    use gpui::{Context, Render, TestAppContext, VisualTestContext, Window, WindowHandle};

    use super::*;

    struct FrameView {
        role: WindowRole,
    }

    impl Render for FrameView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            WindowFrame::new("Role")
                .role(self.role)
                .child(div().size_full())
        }
    }

    fn test_app() -> TestAppContext {
        let app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        app
    }

    fn draw_frame(role: WindowRole) -> (TestAppContext, WindowHandle<FrameView>) {
        let mut app = test_app();
        let window = app.add_window(|_, _| FrameView { role });
        {
            let mut cx = VisualTestContext::from_window(*window, &app);
            cx.run_until_parked();
            cx.update(|window, cx| {
                window.draw(cx).clear(cx);
            });
            cx.run_until_parked();
        }
        (app, window)
    }

    fn titlebar(window: WindowHandle<FrameView>, app: &TestAppContext) -> Bounds<Pixels> {
        let mut cx = VisualTestContext::from_window(*window, app);
        cx.debug_bounds("silvestris-titlebar")
            .expect("titlebar has a debug selector")
    }

    fn has(window: WindowHandle<FrameView>, app: &TestAppContext, selector: &'static str) -> bool {
        let mut cx = VisualTestContext::from_window(*window, app);
        cx.debug_bounds(selector).is_some()
    }

    #[test]
    fn a_document_frame_draws_all_three_live_buttons() {
        let (app, window) = draw_frame(WindowRole::Document);
        assert!(has(window, &app, "traffic-Close-enabled"));
        assert!(has(window, &app, "traffic-Minimize-enabled"));
        assert!(has(window, &app, "traffic-Zoom-enabled"));
    }

    #[test]
    fn fixed_roles_draw_only_the_close_button() {
        for role in [WindowRole::Dialog, WindowRole::Assistant, WindowRole::About] {
            let (app, window) = draw_frame(role);
            assert!(
                has(window, &app, "traffic-Close-enabled"),
                "{role:?} keeps its close button"
            );
            assert!(
                has(window, &app, "traffic-Minimize-disabled"),
                "{role:?} disables minimize"
            );
            assert!(
                has(window, &app, "traffic-Zoom-disabled"),
                "{role:?} disables zoom"
            );
        }
    }

    #[test]
    fn preferences_keeps_minimize_and_disables_only_zoom() {
        let (app, window) = draw_frame(WindowRole::Preferences);
        assert!(has(window, &app, "traffic-Close-enabled"));
        assert!(has(window, &app, "traffic-Minimize-enabled"));
        assert!(has(window, &app, "traffic-Zoom-disabled"));
    }

    #[test]
    fn a_panel_disables_minimize_and_zoom_without_fixing_its_size() {
        let (app, window) = draw_frame(WindowRole::Panel);
        assert!(has(window, &app, "traffic-Close-enabled"));
        assert!(has(window, &app, "traffic-Minimize-disabled"));
        assert!(has(window, &app, "traffic-Zoom-disabled"));
    }

    #[test]
    fn a_utility_frame_uses_the_smaller_title_band() {
        let (app, document) = draw_frame(WindowRole::Document);
        let document_height = f32::from(titlebar(document, &app).size.height);
        let (utility_app, utility) = draw_frame(WindowRole::Utility);
        let utility_height = f32::from(titlebar(utility, &utility_app).size.height);
        assert!(
            utility_height < document_height,
            "utility band {utility_height} should be shorter than {document_height}"
        );
        assert_eq!(
            utility_height,
            WindowRole::Utility.titlebar_height(&app_theme(&app))
        );
    }

    fn app_theme(app: &TestAppContext) -> silvestris_core::tokens::Theme {
        app.update(|cx| (**cx.theme()).clone())
    }
}
