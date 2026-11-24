# Silvestris

A standalone Rust/GPUI application shell with Aqua Light/Dark and Geist
Light/Dark appearances. This is work in progress toward Lotus issues
#118-#138, not a completed Lotus migration.

GPUI is pinned to 6b720973027999bae402df23d469026f4cced988.
Rust is pinned to 1.98.1. Aqua uses bundled Lato and Source Code Pro; Geist
uses bundled Inter and Geist Mono. Font sources, hashes and notices are in
crates/silvestris-fonts/fonts/manifest.json. No fonts are fetched at runtime.

## Build and inspect

On this Windows workspace the GNU Rust host/toolchain is used.

    cargo run -p silvestris-demo --bin gallery --locked
    cargo run -p silvestris-demo --bin explorer --locked
    cargo test --workspace --locked
    cargo clippy --workspace --all-targets --locked -- -D warnings

The gallery exercises individual controls. Document Explorer is a separate
sample consumer: select a document, use Sidebar/Inspector toolbar buttons,
pin/unpin and resize the inspector, resize/collapse the sidebar and narrow
the window. Ctrl+B/Cmd+B toggles Sidebar; Ctrl+I/Cmd+I toggles Inspector;
1-4 select Aqua Light/Dark and Geist Light/Dark. Preferences are in memory.

On Windows, tools/capture/capture-window.ps1 supports -ExactTitle and
-TargetProcessId to select one window. -WindowOnly uses PrintWindow with
PW_RENDERFULLCONTENT to capture GPUI's surface without overlapping windows;
the default screen copy also records DWM shadows and any desktop occlusion.
Use these captures to inspect real application composition, alongside native
input checks. Images under target/ are local development artifacts.

## Roadmap

Silvestris is the GUI layer for lotus, sandcat, chaus and johjoh: apps compose
and extend it rather than drawing their own controls. Milestones, the consumer
gap register, the issue anatomy and the definition of done are in
[ROADMAP.md](ROADMAP.md); the tracking issue is #72.

## Consumer entry points

Depend on crates/silvestris and use the exact same GPUI revision. Call
silvestris::init before opening a window to register fonts, install theme
state and bind text editing keys. Use chrome::window_options/open_window and
WindowFrame for platform chrome; Shell for the content layout.

Applications own domain models, session identity, preference persistence and
command callbacks. The framework owns generic layout and interaction state.
No crate imports Lotus, Azahar or an emulator worker.

- [Coverage matrix](docs/coverage.md)
- [Visual goldens and the render gate](docs/goldens.md)
- [Device-pixel snapping](docs/snapping.md)
- [Composition guide](docs/guide.md)
- [Model bindings and list state](docs/bindings.md)
- [Text editing core](docs/text.md)
- [Consumer pins and integration rules](docs/consumers.md)
- [Public API changes by commit range](CHANGELOG.md)
- [Shell composition](docs/shell.md)
- [Window roles and frame persistence](docs/windows.md)
- [Sidebar and divider](docs/sidebar.md)
- [Source lists](docs/source-list.md)
- [Commands and key contexts](docs/commands.md)
- [Popup menus](docs/menus.md)
- [Transient surfaces](docs/transients.md)
- [Modal sessions](docs/modal.md)
- [Segmented control](docs/segmented-control.md)
- [Checkbox](docs/checkbox.md)
- [Measured deviations and platform gaps](docs/exceptions.md)

The explorer source at crates/silvestris-demo/src/bin/explorer.rs shows public
API composition without gallery internals. Public APIs are still evolving;
no compatibility guarantee or released crate version is established yet.

## Evidence and remaining work

Aqua Light tokens cite local metrology in reference/measurements.json.
Reference images stay in the sibling hig-09 corpus and are not included
here. Dark Aqua and Geist are designed appearances. Estimates and deliberate
exceptions are labelled in source and documentation.

Tests include pure theme/layout/tree checks and rendered GPUI interactions
with mock text/platform services. These do not prove native input delivery,
font rasterization, OS window semantics or screenshot parity. Windows is the
current development platform; macOS/Linux validation remains open.

The complete control suite, native menu integration, final art, visual/DPI
baselines, persisted consumer integration, release licensing/packaging and
Lotus migration are not complete. Framework demos default to Aqua Light;
Lotus must retain Geist Dark until its separate migration/cutover acceptance.


The control catalog also exposes an editable combo box and compact status
values. `Notice` and `notice::error_banner` provide shared workspace empty and
error states. `menu::native_menus` projects the command model into GPUI native
macOS menus, while other platforms can mount the same entries in `PopupMenu`.
