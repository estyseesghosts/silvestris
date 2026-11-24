//! M2 conformance: every shipped fidelity control renders in all four
//! appearances, and every appearance/scale has a committed golden (#70).

use std::fs;

use gpui::{div, prelude::*, Context, Entity, Render, TestAppContext, Window};
use silvestris::controls::{
    Button, ButtonStyle, Checkbox, Disclosure, DisclosureStyle, GroupBox, PopUpButton, PopupStyle,
    ProgressIndicator, RadioGroup, RadioOption, Segment, SegmentedControl, Separator, Slider,
    SourceList, Well,
};
use silvestris::core::source_list::SourceNode;
use silvestris::{ActiveTheme, ThemeId, ThemePreference};

struct Everything {
    list: Entity<SourceList>,
}

impl Render for Everything {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_wrap()
            .gap_2()
            .child(
                Button::new("m2-button", "Save")
                    .default_button(true)
                    .throb(false),
            )
            .child(Button::new("m2-help", "Help").style(ButtonStyle::Help))
            .child(Checkbox::new("m2-check", "Check").checked(true))
            .child(
                RadioGroup::new(
                    "m2-radio",
                    "Radio",
                    vec![RadioOption::new("One"), RadioOption::new("Two")],
                )
                .selected(Some(0)),
            )
            .child(
                PopUpButton::new("m2-popup", "Popup", vec!["One".into(), "Two".into()])
                    .selected(Some(0)),
            )
            .child(
                PopUpButton::new("m2-popup-textured", "Textured", vec!["One".into()])
                    .selected(Some(0))
                    .bezel(PopupStyle::Textured),
            )
            .child(
                SegmentedControl::new(
                    "m2-segments",
                    "Segments",
                    vec![Segment::new("a", "A"), Segment::new("b", "B")],
                )
                .selected("a"),
            )
            .child(Slider::new("m2-slider", "Slider", 0.5))
            .child(ProgressIndicator::new("m2-progress", "Progress", Some(0.4)))
            .child(Disclosure::new("m2-disclosure", "Details"))
            .child(
                Disclosure::new("m2-disclosure-button", "Details button")
                    .style(DisclosureStyle::Button),
            )
            .child(GroupBox::new("m2-group").child(div().child("Group")))
            .child(Well::new("m2-well").child(div().child("Well")))
            .child(Separator::horizontal())
            .child(self.list.clone())
    }
}

#[test]
fn every_fidelity_control_renders_in_all_four_appearances() {
    let ids = [
        "m2-button",
        "m2-help",
        "m2-check",
        "m2-radio",
        "m2-popup",
        "m2-popup-textured",
        "m2-segments",
        "m2-slider",
        "m2-progress",
        "m2-disclosure",
        "m2-disclosure-button",
        "m2-group",
        "m2-well",
    ];
    for theme in ThemeId::ALL {
        let mut app = TestAppContext::single();
        app.update(|cx| silvestris::theme::install(ThemePreference::explicit(theme), cx));
        let (_, cx) = app.add_window_view(|_, cx| Everything {
            list: cx.new(|cx| {
                SourceList::new(vec![SourceNode::new("row", "Row")], cx).expect("source list")
            }),
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        for id in ids {
            assert!(
                cx.debug_bounds(id).is_some(),
                "{theme:?} did not render {id}"
            );
        }
    }
}

#[test]
fn manifest_contains_every_appearance_and_scale_for_every_m2_sample() {
    let manifest: serde_json::Value = serde_json::from_str(
        &fs::read_to_string("../../goldens/manifest.json").expect("read golden manifest"),
    )
    .expect("parse golden manifest");
    let entries = manifest.as_object().expect("manifest map");
    let samples = [
        "button-default",
        "button-regular",
        "checkbox-on",
        "radio-on",
        "popup-button",
        "popup-textured",
        "segmented",
        "segmented-textured",
        "segmented-icon",
        "slider",
        "slider-pointed",
        "slider-vertical",
        "progress",
        "progress-indeterminate",
        "spinner",
        "progress-ring",
        "disclosure",
        "disclosure-expanded",
        "disclosure-button",
        "groupbox",
        "tab-view",
        "well",
        "separator",
        "source-list",
        "source-list-secondary",
    ];
    for appearance in ["aqua-light", "aqua-dark", "geist-light", "geist-dark"] {
        for scale in ["100", "125", "150", "175", "200"] {
            for sample in samples {
                let key = format!("{appearance}/{scale}/{sample}.png");
                assert!(entries.contains_key(&key), "missing golden {key}");
            }
        }
    }
}

#[test]
fn appearances_have_distinct_resolved_surfaces() {
    let mut surfaces = Vec::new();
    for theme in ThemeId::ALL {
        let app = TestAppContext::single();
        app.update(|cx| silvestris::theme::install(ThemePreference::explicit(theme), cx));
        surfaces.push(app.update(|cx| {
            let c = cx.theme().colors.window;
            ((c.r as u32) << 16) | ((c.g as u32) << 8) | c.b as u32
        }));
    }
    for i in 0..surfaces.len() {
        for j in i + 1..surfaces.len() {
            assert_ne!(surfaces[i], surfaces[j], "appearance surfaces must differ");
        }
    }
}
