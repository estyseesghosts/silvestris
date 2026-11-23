//! Deterministic golden renders for the visual gate (#18).
//!
//! One process renders one appearance at one scale. It opens a fixed-size
//! window, draws eight control samples at known logical rectangles, reads the
//! frame back with `Window::render_to_image`, crops each sample and writes
//! PNGs. Text uses grayscale antialiasing and the bundled fonts; no control is
//! focused or animated, so caret blink and the default-button throb cannot
//! affect the pixels.
//!
//! Run through `tools/metrology/render.ps1`; it supplies every appearance and
//! scale and the output directory.

use std::path::{Path, PathBuf};

use gpui::{
    div, point, prelude::*, px, size, App, Bounds, Context, Entity, Render, SharedString, Window,
};
use silvestris::controls::{
    Button, Checkbox, ControlSize, PopUpButton, RadioGroup, RadioOption, Segment, SegmentedControl,
    Slider, SourceList, TextField,
};
use silvestris::core::source_list::SourceNode;
use silvestris::{ActiveTheme, ThemeId, ThemePreference};

const PAD: f32 = 16.;
const GAP: f32 = 12.;
const CELL_WIDTH: f32 = 244.;

struct Sample {
    id: &'static str,
    width: f32,
    height: f32,
}

const SAMPLES: [Sample; 8] = [
    Sample {
        id: "button",
        width: 160.,
        height: 44.,
    },
    Sample {
        id: "checkbox",
        width: 200.,
        height: 24.,
    },
    Sample {
        id: "radio",
        width: 200.,
        height: 24.,
    },
    Sample {
        id: "popup-button",
        width: 180.,
        height: 30.,
    },
    Sample {
        id: "segmented",
        width: 240.,
        height: 32.,
    },
    Sample {
        id: "slider",
        width: 200.,
        height: 28.,
    },
    Sample {
        id: "text-field",
        width: 240.,
        height: 32.,
    },
    Sample {
        id: "source-list",
        width: 200.,
        height: 160.,
    },
];

fn root_size() -> (f32, f32) {
    let mut height = PAD;
    for sample in SAMPLES {
        height += sample.height + GAP;
    }
    height += PAD - GAP;
    (CELL_WIDTH + PAD * 2., height)
}

fn sample_rects() -> Vec<(&'static str, f32, f32, f32, f32)> {
    let mut y = PAD;
    let mut rects = Vec::with_capacity(SAMPLES.len());
    for sample in SAMPLES {
        rects.push((sample.id, PAD, y, sample.width, sample.height));
        y += sample.height + GAP;
    }
    rects
}

struct CaptureView {
    field: Entity<TextField>,
    list: Entity<SourceList>,
}

impl CaptureView {
    fn new(window: &mut Window, cx: &mut Context<Self>, out: PathBuf, scale: f32) -> Self {
        let field = cx.new(|cx| TextField::new(window, cx));
        field.update(cx, |field, cx| field.set_text("Snow Leopard", cx));
        let mut root = SourceNode::new("home", "Home");
        root.children = vec![
            SourceNode::new("desktop", "Desktop"),
            SourceNode::new("documents", "Documents"),
        ];
        let list = cx.new(|cx| SourceList::new(vec![root], cx).expect("source list"));
        list.update(cx, |list, cx| {
            list.set_expanded("home", true, cx);
            list.select("documents", cx);
        });

        window.on_next_frame(move |window, cx| {
            capture(window, cx, &out, scale);
            cx.quit();
        });

        Self { field, list }
    }

    fn sample(&self, id: &str) -> gpui::AnyElement {
        match id {
            "button" => Button::new("golden-button", "Save")
                .size(ControlSize::Regular)
                .into_any_element(),
            "checkbox" => Checkbox::new("golden-checkbox", "Show hidden files")
                .checked(true)
                .into_any_element(),
            "radio" => RadioGroup::new(
                "golden-radio",
                "Check for updates",
                vec![
                    RadioOption::new("Automatically"),
                    RadioOption::new("Manually"),
                ],
            )
            .selected(Some(0))
            .into_any_element(),
            "popup-button" => PopUpButton::new(
                "golden-popup",
                "Appearance",
                vec![SharedString::from("Aqua"), SharedString::from("Graphite")],
            )
            .selected(Some(0))
            .size(ControlSize::Regular)
            .min_width(140.)
            .into_any_element(),
            "segmented" => SegmentedControl::new(
                "golden-segmented",
                "View",
                vec![
                    Segment::new("icon", "Icons"),
                    Segment::new("list", "List"),
                    Segment::new("column", "Columns"),
                ],
            )
            .selected("list")
            .into_any_element(),
            "slider" => Slider::new("golden-slider", "Volume", 0.6).into_any_element(),
            "text-field" => self.field.clone().into_any_element(),
            "source-list" => self.list.clone().into_any_element(),
            other => unreachable!("unknown sample {other}"),
        }
    }
}

impl Render for CaptureView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let mut root = div()
            .relative()
            .w(px(CELL_WIDTH + PAD * 2.))
            .h(px(root_size().1))
            .bg(silvestris::convert::hsla(theme.colors.window));
        for (id, x, y, width, height) in sample_rects() {
            root = root.child(
                div()
                    .absolute()
                    .left(px(x))
                    .top(px(y))
                    .w(px(width))
                    .h(px(height))
                    .child(self.sample(id)),
            );
        }
        root
    }
}

fn capture(window: &mut Window, cx: &mut App, out: &Path, scale: f32) {
    let image = window.render_to_image().expect("frame readback");
    let (width, height) = root_size();
    let expected_width = (width * scale).round() as u32;
    let expected_height = (height * scale).round() as u32;
    assert!(
        image.width() >= expected_width && image.height() >= expected_height,
        "frame {}x{} is smaller than the scaled root {expected_width}x{expected_height}",
        image.width(),
        image.height()
    );
    std::fs::create_dir_all(out).expect("create render directory");
    for (id, x, y, w, h) in sample_rects() {
        let crop = image::imageops::crop_imm(
            &image,
            (x * scale).round() as u32,
            (y * scale).round() as u32,
            (w * scale).round() as u32,
            (h * scale).round() as u32,
        )
        .to_image();
        crop.save(out.join(format!("{id}.png")))
            .expect("write sample png");
    }
    let layout = serde_json::json!({
        "scale": scale,
        "logical_root": [width, height],
        "physical_frame": [image.width(), image.height()],
        "samples": sample_rects().iter().map(|(id, x, y, w, h)| {
            serde_json::json!({
                "id": id,
                "logical": [x, y, w, h],
                "physical": [
                    (x * scale).round() as u32,
                    (y * scale).round() as u32,
                    (w * scale).round() as u32,
                    (h * scale).round() as u32,
                ],
            })
        }).collect::<Vec<_>>(),
    });
    std::fs::write(
        out.join("layout.json"),
        serde_json::to_vec_pretty(&layout).expect("serialize layout"),
    )
    .expect("write layout");
    let _ = cx;
}

fn appearance(name: &str) -> ThemeId {
    match name {
        "aqua-light" => ThemeId::AQUA_LIGHT,
        "aqua-dark" => ThemeId::AQUA_DARK,
        "geist-light" => ThemeId::GEIST_LIGHT,
        "geist-dark" => ThemeId::GEIST_DARK,
        other => panic!("unknown appearance {other}"),
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let appearance_name = args.next().expect("appearance name");
    let scale: f32 = args.next().expect("scale").parse().expect("scale number");
    let out = PathBuf::from(args.next().expect("output directory"));
    let theme = appearance(&appearance_name);
    let (width, height) = root_size();
    let request_width = width * scale + 32.;
    let request_height = height * scale + 80.;

    gpui_platform::application().run(move |cx: &mut App| {
        silvestris::init(ThemePreference::explicit(theme), cx).expect("init Silvestris");
        cx.set_text_rendering_mode(gpui::TextRenderingMode::Grayscale);
        let options = gpui::WindowOptions {
            window_bounds: Some(gpui::WindowBounds::Windowed(Bounds::new(
                point(px(0.), px(0.)),
                size(px(request_width), px(request_height)),
            ))),
            titlebar: None,
            window_decorations: Some(gpui::WindowDecorations::Client),
            focus: true,
            show: true,
            ..Default::default()
        };
        cx.open_window(options, |window, cx| {
            window.set_scale_factor(scale);
            cx.new(|cx| CaptureView::new(window, cx, out.clone(), scale))
        })
        .expect("open capture window");
    });
}
