Use this as the implementation order. I would **not** let the implementer jump ahead to the Finder reference app, or start drawing new controls ad hoc inside Lotus/Sandcat. Each task closes a framework gap that the next tasks depend on.

Baseline checked against current trees: Silvestris `devel/sil-cocoa-fixture` at `f1c2a010`, Lotus `main` tree `bfdf8576`, Sandcat `master` tree `6557c6b7`, Chaus `master` tree `05663843`, and Pain `master` tree `a6f597e6`. The roadmap is somewhat behind the Cocoa-fixture implementation, so part of this work is bringing the tracker back into agreement with what actually exists.

1. **Reconcile the roadmap and coverage matrix before adding more UI.**  
   **Deliverables.** Audit every open roadmap item touched below against the current Cocoa-fixture branch. Specifically update #14, #26, #33, #34, #35, #36, #46, #48, #53, #54, #56, #75 and #49 so each issue distinguishes **missing**, **provisional**, **functional but incomplete**, and **measured/finished** work. `docs/coverage.md` already correctly calls Toolbar, Bottom Bar, Outline View and Drag and Drop missing, Split View provisional, Preferences/Form provisional, and Column/Icon views functional; preserve that distinction. Do not close #34/#35 merely because usable implementations exist: check their promised behaviors against the issue before closure. Add Pain to the consumer documentation alongside Lotus, Chaus and Sandcat, because it is now a real Silvestris consumer with a significantly different architecture. Update the gap register to describe Pain as a terminal-domain-view consumer rather than a Shell/sidebar application. Finally, make the definition-of-done in `ROADMAP.md` mandatory for every task below: scenario tests, goldens, coverage docs, public API docs, and consumer validation.

   **Avoid.** Do not start by rewriting working Cocoa-fixture code because the old roadmap says it is missing. Do not use “the reference app looks okay” as completion criteria. Do not put proposed Sandcat/Finder behavior directly into generic Silvestris issues without identifying the reusable mechanism first.

   **Regression traps.** The biggest risk is duplicating existing primitives: `PreferencesToolbar`, `PreferencesOverview`, `ColumnBrowser`, `IconGrid`, `PathControl`, `OpenPanel`, `SavePanel`, `FileBrowserModel`, tables and source lists already exist. The second risk is treating a consumer workaround as the desired Silvestris API. Lotus's `fit_size`, for example, demonstrates a missing preferences coordinator; its hard-coded 66% display cap is Lotus policy, not a HIG requirement.

   **Sources.** `silvestris/ROADMAP.md`, especially its definition-of-done and reference-app requirements; `silvestris/docs/coverage.md:98-150`; `silvestris/docs/consumers.md:399-460`. Consumer evidence: `lotus/crates/desktop/src/settings_window/shared.rs:1-100`; `sandcat/apps/desktop/src/app.rs:10650-10845`; `chaus/crates/chaus-app/src/preferences.rs:1-60`; `pain/src/preferences.rs:1-40`.

2. **Turn #48 into a real typed artwork vocabulary instead of continuing to grow one mixed `Icon` enum.**  
   **Deliverables.** Preserve the existing file-panel artwork but separate the API concepts of **file/object artwork**, **monochrome template images**, and **pictorial toolbar artwork**. The current `Icon` enum mixes `Folder`, `Home`, `Document`, etc. with `Back`, `Forward`, `List`, `Columns`, `Eject` and `Search`; consumers should not have to know which rendering method is appropriate for which member. Introduce explicit public roles such as a template/system-image vocabulary for Back, Forward, Icon View, List View, Column View, Search, Stop, Refresh, Add, Remove, Action, Info, Sidebar and Inspector. Keep full-colour file/object artwork separately able to render at 16/32px and retain selected/inactive/disabled behavior. Add a public consumer-artwork adapter so Sandcat can supply original “New Chat” artwork, Lotus can supply Lotus-specific commands, and System Preferences panes can supply their own category artwork without adding application concepts to Silvestris.

   Enforce size families in the API rather than letting consumers type arbitrary constants: standalone toolbar pictorial artwork should target the 32×32 family; rectangular toolbar template artwork should target the ≤19×19 family; segmented-control glyphs use the appropriate small template slot. Add gallery examples showing each artwork class on its correct host control. Create goldens for active, inactive and disabled states.

   **Avoid.** Do not trace or redistribute Apple's actual 10.6 icons. Do not use colourful pictorial icons inside every little textured control. Do not use a toolbar template icon for a different meaning because it “looks close.” Do not bake Sandcat names such as `NewChat` or Lotus names such as `RunGuest` into Silvestris. Do not apply the Aqua UI colour palette to application/file artwork; template glyphs and full-colour artwork have different rendering rules.

   **Regression traps.** The existing file-panel artwork deliberately remains coloured when selected and desaturates when inactive/disabled. Preserve that behavior. `SegmentedControl` already accepts icon roles and applies control-state ink; do not accidentally convert those into fixed-colour SVGs. Ensure Aqua Dark and Geist still receive suitable glyph ink even though the fidelity reference is Aqua 10.6.

   **Sources.** Current mixed vocabulary: `silvestris/crates/silvestris/src/icons.rs:1-180`. Current segmented icon support: `silvestris/crates/silvestris/src/controls/segmented.rs`, especially the icon renderer and `SegmentStyle::Textured`. HIG requirements: `hig-09/03-aqua-interface/icons.md:90-140` — 32×32 standalone toolbar icons, ≤19×19 black/gray rectangular toolbar images, consistent perspective/weight, and never repurpose a system image. `hig-09/specs/toolbar-item.md:59-69`.

3. **Build the non-customizable core of the generic toolbar for #26.**  
   **Deliverables.** Add a first-class `toolbar` module. Do not make applications assemble a toolbar out of arbitrary `div()` calls anymore. Define a small explicit item model covering command buttons, toggles, segmented controls, pop-down/action items, search fields, fixed spaces, and flexible spaces. Every item must have a stable ID, an external descriptive label, accessibility metadata, standard size, and a host-appropriate presentation. Command-backed items must resolve enabled/checked/title state through the existing `CommandRegistry`/responder chain at activation time.

   Extend `CommandButton` rather than bypassing it. Right now `CommandButton` resolves labels, enabled state and checked state correctly, but ultimately constructs a textual `Button`; add a command-bound icon/template presentation so Toolbar does not need a second command system. Underlying `Button` already supports `.icon(...)`, so the missing piece is command presentation, not another button implementation.

   Support at least `IconOnly`, `IconAndLabel`, and `TextOnly` display modes in the model even if user customization lands later. The toolbar chooses `theme.metrics.toolbar_height` or `toolbar_labelled_height`; consumers must never hard-code their own vertical stretch. Use 8px inter-control spacing for rectangular/capsule controls. Allow groups and flexible spaces, but do not allow arbitrary children to violate toolbar rules.

   Sandcat's eventual declaration should be able to express New Chat, Stop, Conversation/Changes, Details, Sidebar, Inspector and Search without its current width-threshold logic. Lotus should likewise be able to replace `render_shared_toolbar()` without losing command-registry behavior.

   **Avoid.** Do not copy Sandcat's current `show_identity >= 820`, `show_segmented >= 980`, `show_effort >= 1180` logic into Silvestris. Responsive presentation belongs to toolbar layout, not application-specific pixel thresholds. Do not allow arbitrary `TextField`, checkbox, slider, etc. into the toolbar just because GPUI can render them there. Do not automatically turn every command into a toolbar item. Do not make Preferences use toolbar customization; its toolbar is specifically fixed.

   **Regression traps.** Preserve responder-chain revalidation, checked toggles, disabled commands, keyboard focus, inactive-window appearance and help tags. Toolbar controls must occlude the draggable title-bar hit region just as current window-frame controls do. Opening Inspector from its toolbar toggle must retain the Shell focus-transfer behavior. Test at narrow widths without silently making a command unreachable.

   **Sources.** Existing host only: `silvestris/crates/silvestris/src/chrome.rs:95-150` (`WindowFrame::toolbar`). Command limitation: `silvestris/crates/silvestris/src/command.rs:359-418`. Existing icon-capable button: `controls/button.rs`, `.icon(...)`. Toolbar rules: `hig-09/specs/toolbar-item.md:8-97,145-156`; permitted control families, external labels, 8px spacing, toggle state and accessibility are explicit there. Consumer evidence: `sandcat/apps/desktop/src/app.rs:10650-10845`; `lotus/crates/desktop/src/lotus_window/shared_shell.rs:35-120`.

4. **Build the real bottom-bar abstraction for #75.**  
   **Deliverables.** Create `BottomBar` as a standard window-frame composition primitive rather than another arbitrary `div`. Support status values, fixed/flexible spaces, small rectangular toolbar controls, and gradient add/remove/action controls where the bar is specifically serving a source/list view. Implement the standard regular/small bar heights from the HIG. Establish truncation behavior: status text may truncate; controls do not become vertically distorted or arbitrarily compressed. Add correct active/inactive materials and ensure the resize grip/window resize region remains usable.

   Add a simple status-value model so Sandcat and Lotus can migrate their hand-built status rows without Silvestris learning what “subagent”, “MCP”, “host” or “telemetry” means. Let values optionally expose an activation callback, because Sandcat's “requests waiting” opens a sheet and Lotus's status details may expose additional UI.

   **Avoid.** A bottom bar is not a second toolbar. Do not move Sandcat's model/agent/variant pickers into it. Do not use capsule-style toolbar controls there. Do not put primary frequent commands there. Do not make the bottom bar own application status computation.

   **Regression traps.** Sandcat currently hides zero-value status entries and truncates notices; preserve the ability to do that in the consumer. Lotus currently has a flexible host-status area plus telemetry values and MCP state; ensure flexible status text does not squeeze controls to zero. Fixed-size preferences and modal windows must not suddenly acquire a bottom bar. Source-list add/remove/action controls must remain attached to the list/bar relationship, not float elsewhere.

   **Sources.** `silvestris/docs/coverage.md:114-123` marks Bottom Bar missing. `sandcat/apps/desktop/src/app.rs:11170-11270` is a concrete bespoke bottom bar. `lotus/crates/desktop/src/lotus_window/shared_shell.rs:120-190` is another. HIG: `hig-09/03-aqua-interface/windows.md` Bottom Bars, pp. 212-214; `hig-09/specs/source-list.md:33-39,72-84`; `hig-09/specs/toolbar-item.md:33-40` explicitly prohibits capsule controls in bottom bars.

5. **Finish #14 by adding a Preferences-window coordinator; do not merely add more preference widgets.**  
   **Deliverables.** Keep `PreferencesNavigator`, `PreferencesToolbar`, `PreferencesOverview`, `PreferencePane`, `PreferenceCategory`, `Form` and `TabView`, but put the missing window behavior around them. Add a `PreferencesWindow`/`PreferencesController`-level abstraction that owns current pane navigation, title projection, natural content measurement, resize scheduling, resize animation, focus return and persistence of the last selected pane.

   The coordinator should measure a pane's required content size **after layout**, compute a legal target inside the display's usable region, and resize the fixed preferences window to that target. If a pane cannot fit the available display, only that pane's content becomes scrollable; do not make the entire preferences window permanently oversized. When switching A→B→C rapidly, cancel/re-target the outstanding size animation rather than letting old deferred resize callbacks win later. The pane's title must become the window title. Back/forward and Show All remain navigation history.

   Before choosing an animation duration/easing, measure an actual 10.6 transition if the corpus has one. If timing cannot be measured, document the estimate in `docs/exceptions.md`; do not invent a modern spring animation.

   Move Lotus's generic `fit_size`/`fit_pending` mechanism into this Silvestris coordinator. **Do not** move Lotus's `MAX_SCREEN_FRACTION = 0.66` into Silvestris; that is an application decision. Give consumers either normal “fit to usable display” behavior or an explicit sizing policy hook if one proves necessary.

   Preserve the simple path. Chaus and Pain have straightforward form-based Preferences windows and should not have to create a `PreferencesOverview` or categories merely to use `WindowRole::Preferences`.

   **Avoid.** Do not make `PreferencesToolbar` customizable or hideable. Do not put a user toolbar show/hide button in preferences. Do not make preferences windows generally resizable. Do not make every settings consumer look like System Preferences. Do not embed Lotus group enums or scope concepts in Silvestris.

   **Regression traps.** Lotus's search/scope changes can alter which categories exist; `PreferencesNavigator::retain()` must continue to handle disappearing panes. Chaus applies settings without recreating editor document/selection/undo state; framework changes must not encourage view reconstruction. Pain applies theme and terminal palette live; changing preferences chrome must not mutate terminal ANSI colors. Window title, focus and navigation history must survive theme changes.

   **Sources.** Existing provisional implementation: `silvestris/crates/silvestris/src/preferences.rs:1-260`. Lotus's framework workaround: `lotus/crates/desktop/src/settings_window/shared.rs:1-100`. Simple-consumer examples: `chaus/crates/chaus-app/src/preferences.rs:1-240`; `pain/src/preferences.rs:1-320`. HIG: `hig-09/03-aqua-interface/windows.md:224-245` — fixed/modeless preferences window, fixed toolbar, pane title and remembered selection.

6. **Finish #54 so Forms carry layout/validation behavior rather than every consumer hand-assembling it.**  
   **Deliverables.** Stabilize `Form`/`FormRow` around the HIG two-column form model. Add explicit row states for normal, disabled/dependent, informational note, warning and validation error. Add a standard inline validation mark/message placement rather than consumers inserting red text wherever it fits. Add dependency helpers that bind enabled/disabled state to a model value without rebuilding the whole form. Ensure `Binding<T>` and `TextBinding<T>` can be used directly with common form controls.

   Retain the current ability to provide an arbitrary control element in a row—Lotus controller assignment is a good example—but the framework should own label alignment, inter-row spacing, notes and validation geometry. Add a section/group helper only if it is backed by HIG group-box/separator patterns; do not invent dashboard cards.

   Validate changes when appropriate to the field, not as a blanket “validate whole preferences pane before navigation” rule. Expose errors without destroying the user's current value.

   **Avoid.** Do not add an Apply button as the default settings pattern. Do not use placeholder text as a substitute for visible labels. Do not make validation layout jump horizontally when an error appears. Do not force all controls to identical widths when their HIG control family says otherwise.

   **Regression traps.** Lotus already uses `FormRow::note()` for controller-assignment explanations and dynamically disables assignments; those cases must remain clean. Chaus's invalid font-family entry retains the previously committed font and shows a warning; Form validation must be able to model that without clearing the valid value. Pain's simple theme/palette/profile window must remain concise rather than inheriting Lotus-sized label widths.

   **Sources.** Lotus form usage: `lotus/crates/desktop/src/settings_window/controllers.rs:70-170`. Chaus validation behavior: `chaus/crates/chaus-app/src/preferences.rs:60-175`. Pain's intentionally simple form: `pain/src/preferences.rs:1-320`. Silvestris coverage explicitly marks Forms provisional: `docs/coverage.md:114-123`. HIG layout rules are in `hig-09/03-aqua-interface/layout-guidelines.md`; Preferences-specific layout is HIG pp. 347-350, already cited by `silvestris/preferences.rs`.

7. **Implement #36 as generic drag-and-drop infrastructure before toolbar customization or full Finder behavior.**  
   **Deliverables.** Implement one window-aware drag session model with a payload, source, current operation, drag image, modifier state, candidate destination and drop result. Separate mechanics from application semantics. Provide drop-target registration/negotiation, accepted/rejected feedback, copy/move/link operation indication, insertion indicators for ordered views, transparent drag images, cancellation, pointer capture and autoscroll hooks.

   Apply the HIG's three-pixel drag threshold. Evaluate Option/Alt copy override from the **current modifier state at drop time**, not at drag start. A destination highlights only if it can accept every item in the drag. Lists/source lists need insertion feedback; icon views need destination/position feedback. Autoscroll must be restricted according to the HIG rather than scrolling every window under the cursor.

   Add integration adapters to `SourceList`, `TableView`, `IconGrid` and eventually `OutlineView`. Add host-file-drop support as a platform adapter, not as a filesystem operation embedded in the control layer.

   Keep meaning in the consumer/provider. Finder decides whether a filesystem drop means copy/move/trash based on provider/location. Lotus decides whether dropping a debugger tile means retile/rearrange. Sandcat decides whether a host file drop adds an attachment. Chaus decides whether a dropped file opens a document. Silvestris only performs the interaction.

   **Avoid.** Do not hard-code “same volume means move” inside generic DnD. That belongs to Finder/file-provider policy even though the HIG gives Finder as an example. Do not put `std::fs::rename` or file copying in DnD. Do not make Lotus tiling types generic framework types. Do not start toolbar drag customization until the base drag contract is tested.

   **Regression traps.** A mouse-down on an unselected discrete item may implicitly select it and begin a drag in one gesture. Multi-selection must not collapse incorrectly when dragging an already-selected item. Inactive-window selections must remain available for dragging. Dragging must not steal text selection or terminal mouse-reporting behavior. Menus/transient surfaces need deterministic dismissal/cancellation during drag.

   **Sources.** Silvestris currently has no DnD implementation: `docs/coverage.md:140-149`. Detailed behavior: `hig-09/03-aqua-interface/drag-and-drop.md:70-180` — drop-time Option behavior, three-pixel threshold, transparent drag image, destination validation, insertion indicators and autoscroll. Consumer gap evidence is already recorded in `silvestris/docs/consumers.md`, including Lotus tiling.

8. **Implement #53 as a standalone `SplitView`; do not turn `Shell` into a generic splitter.**  
   **Deliverables.** Add a reusable `SplitView` supporting horizontal and vertical orientation, two or more panes, thin/thick Aqua divider styles where documented, min/max constraints, user resizing, persistence, optional pane collapse and keyboard-accessible divider focus. The layout result should expose exact measured pane bounds so domain views can adapt to their allocation.

   Keep `Shell` as the specific sidebar/workspace/inspector layout it already is. A consumer should be able to put a `SplitView` **inside the workspace** without Shell learning recursive tile policy.

   Lotus should use SplitView primitives for its actual multi-debugger workspace while keeping tile/session selection and emulator-screen policy in Lotus. Pain can later use it for terminal splits, but PTY/session ownership, terminal resize calculations and Ghostty state remain in Pain. Sandcat should generally continue using `Shell` for sidebar/workspace/inspector rather than replacing that with SplitView.

   **Avoid.** Do not implement an IDE layout manager, tiling tree, docking system or tab manager in #53. Do not make SplitView decide which child collapses. Do not persist arbitrary application state; persist divider positions/IDs only. Do not calculate terminal rows/columns inside Silvestris.

   **Regression traps.** Resizing a divider must not trigger recursive layout oscillation. Min-size enforcement must not push a window past its actual viewport. Focus must remain in the child being resized. Shell's sidebar/inspector divider and its narrow-width overlay behavior must remain unchanged. Pain's terminal must continue deriving cell geometry from its actual GPUI allocation rather than a second “planned” size.

   **Sources.** `silvestris/docs/coverage.md:98-109` currently lists Split View as only `Shell` divider/provisional. Lotus's current measured shell: `lotus/crates/desktop/src/lotus_window/shared_shell.rs:1-60`. Pain explicitly documents that its terminal is a domain view and measured GPUI allocation must drive terminal geometry: `pain/docs/SILVESTRIS.md`, especially §§4, 7, 13 and 17.

9. **Extract the generic file-browser core from `file_panel` before building the Finder reference app.**  
   **Deliverables.** File a new roadmap issue for this; it is a gap exposed by #49 rather than something I would silently bury in #29. Move or logically promote `FileProvider`, `FileEntry`, `FileBrowserModel`, `FileViewMode`, sort policy and navigation events into a neutral `file_browser` layer. `file_panel` should consume that layer for Open/Save/Choose. Preserve transitional re-exports so Chaus does not have to migrate simultaneously with the refactor.

   Keep the current important invariants: one owner of current location/history/filter/view state; asynchronous cancellable navigation; generation IDs dropping stale responses; stable opaque provider identities; display names never used to reconstruct paths; and one shared sort policy across icon/list/column presentation.

   Extend provider capabilities only where a general browser/file-manager genuinely needs them: enumerate, metadata, rename/new-folder, resolve location, supported file operations, trash capability, etc. File operations should return results/errors; UI confirmation and progress presentation remain higher-level concerns.

   This is the core the future file manager should instantiate. It must **not** instantiate an `OpenPanel` and pretend the dialog is Finder.

   **Avoid.** Do not let `file_browser` depend on sheets, Cancel/Open/Save buttons, extension policy or modal state. Do not let it call `std::fs` directly; `StdFsProvider` remains one adapter. Do not use display text as identity. Do not make local-disk assumptions that prevent network/virtual providers later.

   **Regression traps.** Chaus already drives the real Open panel through this provider/model boundary and tests Unicode filenames. Those tests must keep passing unchanged through the transitional re-export. Slow listing A must never overwrite navigation to B. Selection and sorting must survive view switches by stable ID.

   **Sources.** The current model is already almost generic: `silvestris/crates/silvestris/src/file_panel/model.rs:1-160` explicitly says one model owns location/history/listing/filter/view and drops stale async generations. Provider identity contract: `file_panel/provider.rs:1-220`. Consumer integration test: `chaus/crates/chaus-app/tests/file_panels.rs:1-220`. HIG Open-dialog requirements establishing the shared browser concepts: `hig-09/03-aqua-interface/windows.md:245-260`.

10. **Complete the three Finder-style content views around one selection/navigation contract: #33, #34 and #35.**  
   **Deliverables.** Implement `OutlineView` as the hierarchical sibling of `TableView`, not as another source list. It should use table columns, disclosure state, stable IDs, selection, inline rename, type-select, virtualization and scrolling. Source lists are navigation sidebars with a maximum two-level hierarchy; OutlineView is the deeper hierarchical content view.

   Finish `IconGrid` rather than replacing it. It already has virtualized rows, stable controller selection, Shift/Command selection, arrow navigation, activation, context-menu events and rename. Add marquee selection, DnD integration, arrangement/sort behavior, accurate label-selection rendering and resize-safe column recomputation. Preserve virtualization—its existing design explicitly targets 30,000 entries.

   Harden `ColumnBrowser` against the same shared browser model and DnD contract. A location selected in columns and then switched to List/Icon must resolve to the same location/selection where meaningful.

   Introduce one file-view host/controller that switches `Icons/List/Columns` without creating three competing navigation models. `FileBrowserModel::FileViewMode` already gives the correct state boundary. View changes must preserve current location, sort policy and stable selection.

   Use a three-icon equal-width segmented control for the view switch. Do not use text in one segment and an icon in another.

   **Avoid.** Do not repurpose `SourceList` as OutlineView. Do not duplicate `FileBrowserModel` per presentation. Do not reset to the home folder when switching views. Do not let each view define its own sort order. Do not remove virtualization in order to implement marquee selection.

   **Regression traps.** Inline rename must keep focus/undo interaction sane. A directory refresh must preserve selection if the stable item still exists. Keyboard navigation after a responsive column-count change must use the newly measured grid. Column-browser late results must obey the browser generation/cancellation rules. SourceList remains capped at its documented hierarchy and keeps its blue/white navigation semantics.

   **Sources.** Existing IconGrid contract: `silvestris/crates/silvestris/src/controls/icon_grid.rs:1-220`. Shared view state/sort: `file_panel/model.rs:1-160`. Coverage: `docs/coverage.md:98-109`. Source-list boundary: `hig-09/specs/source-list.md:8-84` — primary navigation purpose and maximum two levels. Segmented view-switch rules: `hig-09/03-aqua-interface/controls.md:252-288`.

11. **Build #46 and then finish #26's customization/overflow behavior on top of DnD.**  
   **Deliverables.** Implement an HIG-correct `ActionMenu`, command pop-down, and `ScopeBar`. `ActionMenu` must use the standard Action template image plus arrow and expose commands already present in the command system. Command pop-downs contain commands; normal `PopUpButton` remains for mutually exclusive values. ScopeBar exists specifically for narrowing/filtering/search scope.

   Then finish the customizable side of Toolbar: persisted item ordering, optional items, default-set restoration, Icon & Text/Icon Only/Text Only modes, customization palette, drag reorder, and drag-out removal feedback. Overflow must preserve command reachability; when there is insufficient width, lower-priority items go through a coherent overflow affordance rather than disappearing because a consumer crossed an arbitrary pixel threshold.

   Mark certain toolbars explicitly non-customizable. `PreferencesToolbar` is the canonical example. Dialog/file-panel toolbars may likewise expose fixed composition where appropriate.

   Standardize the search-field toolbar host while doing this. Search stays a real search field with magnifying-glass affordance; it is not a generic text box painted into the toolbar.

   **Avoid.** Do not put verbs in normal value pop-ups. Do not display shortcuts in an Action menu. Do not use an Action menu as a dumping ground for every command. Do not add scope bars to ordinary navigation simply to make the UI look “Mac-like.” Do not make preference-category toolbars customizable.

   **Regression traps.** `CommandRegistry` checked/enabled/title state must remain identical between toolbar, menu and shortcut. Opening a pop-down must use the shared transient surface manager and restore focus correctly. Search fields must keep text-editing shortcut isolation. Toolbar customization DnD must not interfere with normal item activation or titlebar dragging.

   **Sources.** HIG Action/Pop-up distinction: `hig-09/03-aqua-interface/controls.md:288-375`; normal pop-ups are values/nouns, Action menus contain commands and do not print keyboard shortcuts. Toolbar customization requirements: `hig-09/specs/toolbar-item.md:8-156`. Chaus's current app-owned find bar is a useful counterexample that should remain a **find** surface, not become a ScopeBar: `chaus/crates/chaus-editor/src/editor_find.rs:1-240`.

12. **Use System Preferences #56 as the first full acceptance application before declaring the preferences stack finished.**  
   **Deliverables.** Build a reference application using **only public Silvestris APIs**. It should contain a Show All overview with several original 32px category icons, Back/Forward/Show All, a real search field, multiple panes of different natural heights, at least one tabbed pane, a form-heavy pane, radio/checkbox/pop-up/slider controls, a validation example and one pane whose content exceeds the available display and therefore scrolls.

   Exercise pane title changes, remembered pane, back/forward history, resize transitions, keyboard navigation, active/inactive appearance, theme changes and all four Silvestris appearances. Add scripted interaction scenarios, not just screenshots.

   Use Lotus as the real consumer validation immediately afterward: remove its generic `fit_size`/`fit_pending` workaround and adopt the new coordinator. Do **not** migrate Lotus's registry/group semantics into Silvestris.

   Also run Chaus and Pain preferences after this change specifically to prove that a simple one-page `WindowRole::Preferences + Form` remains supported. A successful #56 that makes simple settings windows awkward is not a successful framework design.

   **Avoid.** Do not recreate Apple's preference categories/content verbatim. Do not use the reference app to sneak framework logic into the app. Do not accept fixed pixel heights that happen to fit the test machine. Do not require every Silvestris consumer to use Show All.

   **Regression traps.** Lotus category filtering/history, Chaus's live setting application, Chaus font validation, Pain's live theme/palette update, fixed-window controls, focus return and window-title updates all need explicit tests.

   **Sources.** Roadmap acceptance target: `silvestris/ROADMAP.md` #56. Existing composition: `silvestris/preferences.rs:1-260`. Lotus consumer: `settings_window/shared.rs:1-220`. Chaus: `preferences.rs:1-240`. Pain: `preferences.rs:1-320`. HIG preferences contract: `hig-09/03-aqua-interface/windows.md:224-245`.

13. **Build #49 as a genuine Finder-style reference application, then use it as the gate for future file-manager work.**  
   **Deliverables.** Build a separate reference app using only public Silvestris APIs and the generic file-browser layer. It must contain a primary blue SourceList, resizable source-list splitter, Back/Forward navigation, path control, Icon/List/Column segmented view switch, Action menu, toolbar Search, content view, bottom status bar, keyboard navigation, inline rename, multi-selection and DnD. Use `StdFsProvider` for a real local filesystem smoke test and `MemoryProvider` for deterministic automated scenarios.

   Selection/location/history must survive view switches. Back/Forward/path/sidebar navigation must all drive the same browser model. Toolbar state must be command-backed. The bottom bar should show things such as selection/item count through generic status-value APIs. DnD must exercise same-container and different-container decisions through provider policy. Include New Folder, rename and trash only when the provider advertises those capabilities; unavailable operations are disabled rather than failing after click.

   Exercise a Get Info/utility-panel path if #27 is ready, but do not hold the entire browser hostage to every future Finder feature. The reference app's purpose is to prove the public framework architecture. Once this app can be built without private controls, the future production file manager can add policy/features on top.

   **Avoid.** Do not put filesystem logic directly in the reference view. Do not use an Open/Save panel as the browser root. Do not let source-list selection, path selection and content selection become three unrelated states. Do not add Cover Flow just because Snow Leopard Finder had it; it is not required to prove the current architecture. Do not hide missing framework functionality behind reference-app hand painting—if the reference app needs a generic control that Silvestris lacks, stop and file the Silvestris gap.

   **Regression traps.** Large directories must remain virtualized. Slow provider results cannot overwrite newer navigation. Unicode/non-UTF-8-capable provider identity must not be reconstructed from labels. Sidebar hierarchy stays within source-list rules. Inactive selection state must remain visually correct. DnD must not accidentally mutate files until the provider operation has been chosen and accepted.

   **Sources.** Roadmap explicitly defines #49 as the Finder-style public-API acceptance app and lists #26/#27/#29/#32/#33/#34/#35/#36/#41/#46/#75 as dependencies: `silvestris/ROADMAP.md` #49. Browser model: `file_panel/model.rs:1-160`. Provider identity: `file_panel/provider.rs:1-220`. Icon-grid implementation: `controls/icon_grid.rs:1-220`. Source-list design rules: `hig-09/specs/source-list.md:8-113`. File-dialog/browser precedent: `hig-09/03-aqua-interface/windows.md:245-260`. DnD: `hig-09/03-aqua-interface/drag-and-drop.md:70-180`.

14. **Finish with consumer-specific migrations; use them to prove that Silvestris stayed generic.**  
   **Deliverables.** Migrate each application only to the primitives it actually needs. **Sandcat:** replace its hand-built unified toolbar with `Toolbar`; use pictorial New Chat/Stop plus proper icon view/toggle controls; keep Agent/Model/Variant next to the composer because they configure prompt execution rather than the window; replace `render_bottom_bar()` with `BottomBar`. **Lotus:** replace its hand-built shared toolbar/status chrome; move generic preference sizing/forms to Silvestris; adopt SplitView primitives, but retain emulator/session/tiling policy and gauge contents in Lotus. **Chaus:** keep its editor, Vim engine, syntax rendering and document model app-owned; continue using Open/Save panels; adopt improved Form and standard find/search components only where they actually reduce bespoke UI. **Pain:** keep the terminal as one app-owned domain canvas; continue using simple Preferences; do not force Shell, Finder browser APIs or System Preferences overview onto it; use SplitView only when Pain actually grows terminal splits. **Future file manager:** consume Toolbar, SourceList, generic file browser, all three views, DnD, Action/Scope controls, BottomBar and utility panels without patching Silvestris.

   For every migration, add a consumer-facing smoke or scenario test before removing the old code. Only remove a bespoke implementation once the new public Silvestris API reproduces its necessary behavior. Update `docs/consumers.md` immediately as each gap is eliminated.

   **Avoid.** Do not make an API “generic” by adding an enum with `Lotus`, `Sandcat`, `Chaus`, `Pain` variants. Do not force every consumer to use every high-level shell. Do not migrate domain views into the framework. Do not leave parallel old/new implementations indefinitely.

   **Regression traps.** Sandcat must retain command/shortcut behavior and composer focus. Lotus must retain emulator input isolation, measured screen allocation, settings scope, inspectors and multi-session behavior. Chaus must retain document state, undo history, Vim behavior and real Open/Save integration. Pain must retain terminal keyboard ownership, PTY resize correctness and terminal palette independence from Aqua colours.

   **Sources.** Sandcat toolbar/bottom-bar/composer boundaries: `apps/desktop/src/app.rs:10650-11035,11170-11270,11665-11750`. Lotus shared frame: `crates/desktop/src/lotus_window/shared_shell.rs:1-190`; Lotus settings: `settings_window/shared.rs`. Chaus file-panel consumer boundary: `crates/chaus-app/tests/file_panels.rs:1-220`; preferences: `crates/chaus-app/src/preferences.rs`. Pain's domain-view boundaries are explicitly documented in `pain/docs/SILVESTRIS.md`, especially §§4-17, and its simple settings implementation is `pain/src/preferences.rs`.

The key ordering constraint is **#48 artwork → toolbar core → preferences/forms → DnD → SplitView/file-browser extraction → Finder views → action/customization chrome → reference apps → consumer migrations**. Do not close #26 after only producing a nice Sandcat toolbar, and do not close #14 after only making Lotus Settings resize correctly. The acceptance criterion is that the same public APIs make sense in all five shapes of application without any one of them dictating Silvestris's architecture.