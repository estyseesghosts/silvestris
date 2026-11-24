# Source list v0.1 work in progress

Implements the generic tree and virtualized row portion of #128/#130.
The persistent rail and resize splitter are in sidebar.md; the Lotus adapter remains separate work.

Reference: aqua-gpui handbook 09, section C, and handbook 13; selection and
sidebar tokens cite measurements.json (10.6 Finder, Mail and Open panel).
Disclosure direction and 13 px bounds follow hig-09/03-aqua-interface/controls.md,
HIG pp. 329-331; the drawn 4.5 x 6 px triangle is measured from
`disclosure.collapsed`.

SourceNode has stable string ID, label, disabled state, section flag, optional
icon role and badge, and children.
SourceTree::replace atomically rejects duplicate IDs, retains expansion and
selection by identity and removes selection if its node disappears or becomes
disabled. Expansion never selects. Hidden selection survives collapse; the
next navigation starts from the visible boundary if needed.

SourceList::new(nodes, cx) returns a GPUI entity for a bounded-height
container. state, replace, select and set_expanded synchronize presentation
without invoking domain actions. Applications subscribe to SourceListEvent
Selected and Expanded for user actions and own persistence. No emulator
types or worker calls occur in the component.

The gallery composes a collection of 10,000 documents, a disabled item and
an archive item. Event text distinguishes selection from disclosure.

- Input: row click selects; disclosure click expands independently. Tab
  reaches the tree and visible disclosure controls. Tree arrows navigate,
  Left collapses/returns to parent, Right expands/enters descendants;
  Home/End select first/last enabled visible item. Modified keys propagate.
  A focused disclosure uses Left/Right and Enter/Space itself.
- Accessibility: Tree/TreeItem roles, labels, selected/expanded state and
  Button/expanded state for disclosure. Disabled state is still limited by
  pinned GPUI; see exceptions.md. Screen-reader behavior is untested.
- Long labels truncate in the available width. Indentation and row height
  come from theme metrics. RTL behavior is not implemented or verified.
- Rendering: cached immutable row snapshots shared across repaints;
  GPUI uniform_list creates elements for the visible range. Navigation
  reveals the selected row. No image/atlas allocation is added.
- Selection has three states: focused blue gradient, key-window-unfocused
  muted blue gradient, and inactive-window grey. `SourceListStyle::Primary`
  is the Finder blue-grey sidebar; `SourceListStyle::Secondary` is the
  document-integrated list used by Mail and Address Book.
  No per-theme behavioral branch exists.
- Text editing, IME, clipboard, command dispatch, context menus,
  final coloured icon artwork, drag/reorder, custom scrollbar and footer
  remain required for the full source-list milestone where applicable.

Verification: source_list unit tests cover navigation, disclosure isolation,
disabled/empty/edge cases, stable identity, atomic duplicate rejection,
10,000 rows, and immutable snapshot sharing. GPUI interaction tests now
exercise rendered disclosure clicks and Space/Enter activation in all four
themes, tree keyboard navigation and focus through live theme switches,
and Home/End scrolling over 10,000 rows. The icon callback count verifies
viewport rendering rather than constructing an element for every item.
Run them with `cargo test -p silvestris --lib --locked`.

The tests use GPUI TestAppContext/VisualTestContext and its mock text system.
They install the theme directly; bundled font registration is deliberately
outside this harness. They verify event dispatch, hitboxes, focus and layout,
not native input delivery or pixel accuracy. Key activation tests send both
key-down and key-up, matching div.rs's focus-generation check.

Workspace builds and Clippy check GPUI integration. Source-list goldens for
both styles are committed in all four appearances at 100/125/150/175/200%
(`source-list`, `source-list-secondary`). Real mouse/keyboard, native
inactive/focus repaint, screen-reader delivery and DPI review remain separate
platform work (#50).

Pinned GPUI: 6b720973027999bae402df23d469026f4cced988,
uniform_list.rs and div.rs. No new artwork or font files are distributed.

Section and icon extension: SourceNode::section creates a nonselectable,
always-open heading; its children start at the regular row indentation.
Navigation skips headings and disabled items, including parent traversal.
Labels are displayed uppercase with the heading typography/colour tokens.
The 10 px heading inset, 16 px icon slot and existing 4/6 px row padding
are estimates pending visual review. Equal row heights preserve virtualization.

Optional icon and badge strings are presentation descriptors. The icon role
is resolved through SourceList::set_icon_renderer for visible rows only;
the consumer owns loading and caching. The callback receives selected and
disabled states. With a renderer installed, rows without an icon keep an empty
slot. Badges are trailing, noninteractive text; consumers should keep them
short. The gallery's blue folder is an original geometric fixture, not final
period-faithful icon artwork. Section/icon-slot unit coverage is in
source_list::tests::sections_are_open_but_never_selected_or_collapsed.

Rows now distinguish selection eligibility from disabled state. Set
SourceNode.selectable=false for a container that can disclose children but
must not become the workspace target (for example an offline card). Its
disclosure remains enabled. Navigation skips it when choosing a selection.
Identical model replacements keep the shared row snapshot without rebuilding
or issuing a SourceList notification.

set_row_accessory supplies trailing application-owned controls for visible
non-heading rows. Consume accessory activation when row selection would be
an unintended side effect. set_row_decorator wraps the complete row with
application drag/context handlers; preserve its height for uniform_list and
retain its child so framework focus/selection/disclosure stays authoritative.
The application controls each accessory's availability. Use weak entity
captures to avoid ownership cycles.

`set_row_accessories` is the standard model beside the renderer hook. It
returns `RowAccessory` values: `Badge(count)`, `Eject`, `Spinner`,
`StatusDot(RowStatus)`, `Hover(..)` and `Button { .. }`. Badges and dots are
presentation only; action buttons stop row selection and drag, are tab stops,
and activate on Enter/Space. `Hover` reveals its inner accessory only while the
row group is hovered, for optional affordances such as an eject control. The
gallery and the secondary golden show a badge, status dot, spinner and eject.

Tests verify nonselectable containers can expand, identical replacements
share snapshots, accessory clicks do not select another row, context handlers
receive the correct row ID, and disclosure does not arm enclosing row drags.

clear_selection clears a controller-owned selection without dispatching an
action or changing expansion. Handled source-list mouse-down events stop at
the component boundary so an enclosing workspace does not reclaim guest-key
focus after an accessory click.

Downstream GPUI tests can enable the test-support Cargo feature and call
silvestris::test_support::initialize(preference, cx). This installs theme and
text-field action bindings for the mock platform without claiming font
registration. Production applications must still call silvestris::init.
