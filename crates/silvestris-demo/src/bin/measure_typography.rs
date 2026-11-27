//! Record Geist label metrics at every supported scale.
//!
//! This tool runs on a real GPUI platform because headless tests use a no-op
//! text system. It writes the measurements that docs/geist-refinement.md cites.

use std::fs;
use std::path::PathBuf;

use gpui::{
    font, point, px, size, App, AppContext, Bounds, EmptyView, FontStyle, FontWeight, TextRun,
    WindowBounds, WindowDecorations, WindowOptions,
};
use serde_json::json;
use silvestris::{ThemeId, ThemePreference};

struct Role {
    name: &'static str,
    size: f32,
    mono: bool,
    weight: FontWeight,
}

const STRINGS: [&str; 7] = [
    "Cancel",
    "0123456789",
    "Settings",
    "Storage & Displays",
    "Load more…",
    "/Users/terezi/Documents/Finder.txt",
    "Éléphant – naïve façade: résumé",
];

fn main() {
    let out = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "docs/geist-typography/measurements.json".to_string()),
    );
    gpui_platform::application().run(move |cx: &mut App| {
        silvestris::init(ThemePreference::explicit(ThemeId::GEIST_DARK), cx)
            .expect("initialise Silvestris");
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                point(px(0.), px(0.)),
                size(px(800.), px(600.)),
            ))),
            titlebar: None,
            window_decorations: Some(WindowDecorations::Client),
            focus: false,
            show: false,
            ..Default::default()
        };
        cx.open_window(options, |window, cx| {
            let theme = silvestris::core::theme(ThemeId::GEIST_DARK);
            let roles = [
                Role { name: "regular", size: theme.type_.size_regular, mono: false, weight: FontWeight::NORMAL },
                Role { name: "small", size: theme.type_.size_small, mono: false, weight: FontWeight::NORMAL },
                Role { name: "mini", size: theme.type_.size_mini, mono: false, weight: FontWeight::NORMAL },
                Role { name: "title", size: theme.type_.size_title, mono: false, weight: FontWeight::NORMAL },
                Role { name: "heading", size: theme.type_.size_heading, mono: false, weight: FontWeight::SEMIBOLD },
                Role { name: "menu", size: theme.type_.size_menu, mono: false, weight: FontWeight::NORMAL },
                Role { name: "view", size: theme.type_.size_view, mono: false, weight: FontWeight::NORMAL },
                Role { name: "toolbar-label", size: theme.type_.size_label, mono: false, weight: FontWeight::NORMAL },
                Role { name: "mono", size: theme.type_.size_mono, mono: true, weight: FontWeight::NORMAL },
            ];
            let mut scales = serde_json::Map::new();
            for scale in [1.0_f32, 1.25, 1.5, 1.75, 2.0] {
                window.set_scale_factor(scale);
                let mut roles_out = Vec::new();
                for role in &roles {
                    let mut face = if role.mono {
                        font(theme.type_.mono)
                    } else {
                        font(theme.type_.ui)
                    };
                    face.weight = role.weight;
                    face.style = FontStyle::Normal;
                    let mut strings = Vec::new();
                    for text in STRINGS {
                        let run = TextRun {
                            len: text.len(),
                            font: face.clone(),
                            color: silvestris::convert::hsla(silvestris::core::Rgba8::hex(0xededed)),
                            background_color: None,
                            underline: None,
                            strikethrough: None,
                        };
                        let line = window.text_system().shape_line(
                            text.to_string().into(),
                            px(role.size),
                            &[run],
                            None,
                        );
                        let width = line.width.as_f32();
                        let height = (line.ascent + line.descent).as_f32();
                        strings.push(json!({
                            "text": text,
                            "logical_width": width,
                            "logical_height": height,
                            "device_width": width * scale,
                            "device_height": height * scale,
                        }));
                    }
                    roles_out.push(json!({
                        "role": role.name,
                        "font_size": role.size,
                        "weight": role.weight.0,
                        "family": if role.mono { theme.type_.mono } else { theme.type_.ui },
                        "strings": strings,
                    }));
                }
                scales.insert(
                    format!("{}%", (scale * 100.0) as u32),
                    json!({ "scale": scale, "roles": roles_out }),
                );
            }
            let report = json!({
                "theme": "geist-dark",
                "source": "GPUI TextSystem::shape_line on the pinned GPUI revision",
                "note": "logical sizes are scale-independent; device values are logical * scale",
                "control_heights": {
                    "push": theme.metrics.push_height,
                    "push_small": theme.metrics.push_small_height,
                    "push_mini": theme.metrics.push_mini_height,
                    "field": theme.metrics.field_height,
                    "field_small": theme.metrics.field_small_height,
                },
                "scales": scales,
            });
            if let Some(parent) = out.parent() {
                fs::create_dir_all(parent).expect("create measurement output directory");
            }
            fs::write(
                &out,
                serde_json::to_string_pretty(&report).expect("serialise measurements"),
            )
            .expect("write measurements");
            eprintln!("wrote {}", out.display());
            cx.quit();
            cx.new(|_| EmptyView)
        })
        .expect("open measurement window");
    });
}
