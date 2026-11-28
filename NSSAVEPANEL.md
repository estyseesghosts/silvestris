To do `NSOpenPanel` / `NSSavePanel` properly, the main blockers are **M3: Collections and text views** first, then **M4: Windows, sheets and panels**. In M3, finish **#32 Table View** first, because list mode, selection, sorting, keyboard navigation, inline editing, and large-directory virtualization all depend on it. Then **#40 Scrollers**, because tables and column browsers need complete horizontal/vertical scrolling behavior, followed by **#34 Column Browser**, which is a core part of the Snow Leopard Open/Save panel. **#3 Source List** should also be completed for the sidebar, while **#35 Icon Grid** and **#36 Drag and Drop** are useful for full Finder fidelity but are not hard blockers for the first complete Open/Save panel.

Then move into M4: finish **#22 Document Sheets**, because Save/Open panels need correct sheet presentation when attached to a document, and finally implement **#29 Open, Save and Choose Panels** itself. Alongside that, **#41 Path Control / Path Pop-up** is effectively required for a faithful panel, and **#23 Alerts** matters for Save replacement confirmation. So the clean order I would use is: **#3 → #40 → #32 → #34 → #41 → #22 → #23 → #29**, with **#35/#36** either before `#29` if you want every view mode from day one, or immediately afterward if list + column views are enough for the first complete implementation.

A future Silvestris `SavePanel` should be mostly **composition and policy**, not a giant custom widget. AppKit's `NSSavePanel` itself is a panel that combines filesystem browsing, a current directory, an editable target filename, file-type policy, directory creation, accessory UI, validation, and modal presentation; `NSOpenPanel` then subclasses it and changes selection semantics. [Apple Developer](https://developer.apple.com/documentation/appkit/nssavepanel?changes=__4_5\&utm_source=chatgpt.com) In Silvestris terms, I would expect the eventual structure to look roughly like this:

```text
SavePanel
├── ModalSession / Sheet                # #21 + #22
├── FileBrowserModel
│   ├── FileProvider                    # #29
│   ├── current directory
│   ├── navigation history
│   ├── visible entries
│   ├── selection
│   ├── filtering / allowed types
│   └── async loading
│
├── SourceList                          # #3
├── browser presentation
│   ├── TableView                       # #32
│   ├── ColumnBrowser                   # #34
│   └── IconGrid                        # #35
│
├── ScrollView / ScrollSource           # #40
├── PathControl / PathPopUp             # #41
├── TextField filename                  # #58 already done
├── accessory view slot
├── New Folder                          # #29
├── Cancel / Save buttons
└── replace-existing Alert              # #23
```

The important part is that `SavePanel` should **not implement selection, scrolling, text editing, modality, filesystem listing, or column navigation itself**. It should wire those generic Silvestris systems together and impose Save-panel-specific rules.

### The lower-level dependencies

**#58 Text core and #61 collection controllers are already closed**, and they are important foundations. The filename field in a save panel is not just a painted box: it needs IME, selection, clipboard, Escape/Return semantics, grapheme-safe editing, and validation. That all comes from the shared text system rather than `SavePanel` reimplementing an editor. Likewise, stable item IDs and selection from `ListController` matter because directory contents can be reloaded, filtered, sorted, or updated asynchronously without making the selected file suddenly refer to another row.

**#40 Scrollers is the bottom of the remaining collection stack.** A save panel can encounter directories containing tens of thousands of files, and its list/column presentations need both vertical and horizontal scrolling. `#40` creates a common `ScrollSource` abstraction and `ScrollView`, including `UniformListScrollHandle`. That matters because `#32 TableView` and `#34 ColumnBrowser` should not each invent scrolling behavior. Without `#40`, the save panel could be made to look correct, but every browser mode would have subtly different scrolling, keyboard reveal, thumb geometry, and resizing behavior.

**#32 TableView is probably the most important direct blocker.** The current `controls::table` is deliberately primitive. A real Save panel's list view needs stable selection, icon+filename cells, column headers, sorting, keyboard navigation, type-select, Shift/Cmd selection semantics, virtualization, and ideally contextual actions. `#32` centralizes all of that. Its virtualization requirement is particularly important: `#29` explicitly calls out long directories, and loading 30,000 files cannot mean constructing 30,000 GPUI row trees every frame. Once `#32` exists, `SavePanel` should merely provide a `ListController<FileEntry>` and column definitions such as Name, Date Modified, Size, Kind.

**#34 ColumnBrowser then builds hierarchical navigation on top of those lower-level collection primitives.** This is the classic Finder/Open-panel column mode: selecting `Users` produces another column, selecting a username produces the next one, and so on. It needs horizontal scrolling, left/right keyboard movement, type-select, asynchronous child loading and a preview column. That is why `#34` depends on `#32` and `#40`. The Save panel shouldn't understand column geometry; it should say, effectively, "here is a hierarchy whose children come from `FileProvider`." `ColumnBrowser` handles the UI.

### The panel-specific dependencies

**#3 SourceList supplies the sidebar.** The save panel's Places/Devices-style sidebar is not just decorative. Selecting "Desktop", "Documents", a drive, etc. must change `FileBrowserModel.current_location`; keyboard focus must move between the sidebar and browser; selection appearance must correctly distinguish focused, unfocused and inactive-window states. `SourceList` already has much of the behavior, but `#3` finishes the measured 10.6 presentation. The key architectural point is that the sidebar produces a location ID; it does not perform filesystem navigation itself.

**#41 PathControl is more important than the current dependency graph suggests.** An actual `NSSavePanel` exposes a current directory, and AppKit has a `directoryURL` property for exactly that concept. [Apple Developer](https://developer.apple.com/documentation/appkit/nssavepanel/directoryurl?changes=_8_5%2C_8_5\&utm_source=chatgpt.com) In our implementation, `FileBrowserModel` should own that location, while `PathControl` is simply another view/controller over it:

```text
sidebar click ───────┐
column navigation ───┼──► set_location(...)
path component click ┘
                          │
                          ▼
                   FileBrowserModel
                          │
        ┌─────────────────┼──────────────────┐
        ▼                 ▼                  ▼
    SourceList        TableView        ColumnBrowser
```

That means clicking `/Users/j/Documents` in any representation updates one canonical location rather than each control owning navigation state. `#41` itself correctly says filesystem access must remain outside the control.

**#48 Artwork/Icon API is therefore an indirect dependency.** `#41` depends on it, and `#35` also depends on it. File panels are full of semantic icons: folders, drives, home, back/forward, view modes, action controls. Those should be `Icon::Folder`, `Icon::GoBack`, etc., with the correct selected/inactive rendering, not random SVGs hardcoded into `SavePanel`. This is exactly the kind of reusable Cocoa vocabulary `#48` is supposed to create.

### Why sheets and modality are separate problems

**#21 ModalSession is already done**, and it supplies the behavioral half: block the owning document, constrain focus, make Escape cancel, prevent unrelated commands from firing, restore focus afterward, and return a result. That means a save panel shown for a document can have semantics roughly like:

```rust
SavePanel::new(provider)
    .directory(initial_directory)
    .suggested_name("Untitled.txt")
    .begin_sheet(document_window, |result| {
        ...
    });
```

Underneath, the panel would enter a `ModalSession`. The File menu shouldn't still be usable to open another Save operation against the same document, Tab should remain within the panel, and cancellation should unwind cleanly.

**#22 Sheet supplies the visual/window half.** A Snow Leopard document save panel isn't just a centered overlay; the sheet is visibly attached to the document title bar and has particular geometry, shadow, resize behavior and animation. `ModalSession` answers "what is blocked?" while `Sheet` answers "how is this presented?" That's why `#22` legitimately blocks `#29`.

The same `SavePanel` should also be capable of standalone application-modal presentation. The model and controls remain identical; only the presentation host changes:

```text
                       SavePanel content
                              │
               ┌──────────────┴──────────────┐
               ▼                             ▼
       Sheet(document)                modal Panel/Dialog
       ModalScope::Document           ModalScope::Application
```

That separation is what keeps `SavePanel` from becoming entangled with window management.

### Filename handling is more than a TextField

The shared text core gives us the mechanics, but **`#29` must own Save-specific filename policy**. Apple's panel exposes `nameFieldStringValue`, `allowedContentTypes`, `allowsOtherFileTypes`, hidden-extension behavior, and a current directory. [Apple Developer](https://developer.apple.com/documentation/appkit/nssavepanel?changes=latest_min_4\&language=objc\&utm_source=chatgpt.com) Our equivalent should probably have something like:

```rust
SavePanelOptions {
    suggested_name: String,
    allowed_types: Vec<FileType>,
    allow_other_types: bool,
    show_hidden_files: bool,
    can_create_directories: bool,
    ...
}
```

Then the panel controller derives a candidate:

```text
current directory + filename field
             │
             ▼
     candidate target path
             │
     ┌───────┴─────────┐
     ▼                 ▼
 type validation    provider.exists()
     │                 │
     └───────┬─────────┘
             ▼
        enable Save?
```

This is where the distinction between **generic UI mechanics** and **SavePanel policy** matters. `TextField` should know nothing about extensions or filesystem paths.

### #23 Alerts is required for correct completion

Suppose the user enters `report.txt` and that file already exists. `SavePanel` should **not overwrite it immediately**, and it shouldn't implement its own ad-hoc confirmation card.

That path should be:

```text
Save clicked
    │
    ▼
validate candidate
    │
    ├── nonexistent ──────► ModalResult::Accepted(path)
    │
    └── exists
         │
         ▼
     #23 Alert
     "report.txt already exists.
      Do you want to replace it?"
         │
       ┌─┴─┐
       ▼   ▼
    Cancel Replace
       │     │
       │     └──► Accepted(path)
       └────────► return to SavePanel
```

Issue `#23` specifically includes a **replace-existing-file preset**. So although `#23` is not currently listed in `#29`'s formal `Depends on:` header, `#29`'s own deliverables reference it. For a genuinely complete Save panel, I would treat `#23` as a dependency.

### New Folder depends on the provider, not on arbitrary filesystem calls

Apple exposes `canCreateDirectories`; when enabled, the panel exposes directory-creation UI. [Apple Developer](https://developer.apple.com/documentation/appkit/nssavepanel/cancreatedirectories?changes=__3\&utm_source=chatgpt.com) `#29` already proposes the right abstraction:

```rust
trait FileProvider {
    list(...)
    metadata(...)
    icons(...)
    create_folder(...)
}
```

So pressing New Folder should be approximately:

```text
New Folder
    │
    ▼
FileProvider::create_folder(current_location, name)
    │
    ▼
reload current location
    │
    ▼
select/reveal new folder
```

It should **not** call `std::fs::create_dir()` directly from the button handler. That separation lets us have the local provider, an in-memory provider for deterministic testing, and potentially other providers later without contaminating the UI layer.

It also makes async behavior manageable. A slow provider operation can expose loading state without freezing GPUI.

### Accessory views come almost for free once composition is correct

AppKit lets applications put an arbitrary accessory view above the action buttons, and the panel resizes to accommodate it. [Apple Developer](https://developer.apple.com/documentation/appkit/nssavepanel/accessoryview?utm_source=chatgpt.com) This should map very cleanly into Silvestris:

```rust
SavePanel::new(provider)
    .accessory(
        PopUpButton::new(...)
        // e.g. "Format: PNG / JPEG / TIFF"
    )
```

The important dependency here isn't another filesystem issue. It is having the normal Silvestris controls, layout system and sheet sizing work correctly. `SavePanel` should expose an `AnyElement`/view slot and let the caller compose normal Silvestris controls into it.

### Icon mode changes the dependency graph

The current **#29 formally depends on `#3 #22 #32 #34`**, but its description explicitly calls Open/Save panels a composition of **icon/list/column views**. Meanwhile **#35 IconGrid** explicitly says it is needed for Open-panel icon mode, yet it only declares that it unblocks Finder `#49`.

So if your requirement is now:

> build our NSSavePanel properly and completely from the start

then I think the roadmap dependency graph needs correcting.

`#29` should functionally depend on at least:

```text
#3   SourceList
#22  Sheet
#23  Alerts / replace-existing
#32  TableView
#34  ColumnBrowser
#35  IconGrid
#41  PathControl
```

with these transitive dependencies:

```text
#32 ─► #40 Scrollers
#32 ─► #58 Text core       [done]
#32 ─► #61 ListController  [done]

#34 ─► #32
#34 ─► #40

#35 ─► #36 Drag/drop
#35 ─► #48 Icon API
#35 ─► #61               [done]

#41 ─► #48

#22 ─► #21 ModalSession  [done]
```

I would therefore consider **#48 → #40 → #32 → #36 → #35 → #34 → #41 → #3 → #22 → #23 → #29** a reasonable implementation sequence where dependencies permit parallel work. `#3` doesn't technically need to wait that long; the important part is that all of those primitives land before `#29`.

The payoff is that `#29` then becomes surprisingly small conceptually. It creates **`FileProvider` + `FileBrowserModel` + Save/Open policy**, and composes already-proven Silvestris pieces. That is the architecture that gets us close to Cocoa: the file panel becomes a reusable framework object assembled from the same generic views a future Finder-style application will use, rather than becoming a one-off Finder-looking dialog that later has to be dismantled. [Apple Developer](https://developer.apple.com/documentation/appkit/nssavepanel?changes=__4_5\&utm_source=chatgpt.com)

Current Silvestris references checked: `devel/sil-cocoa-fixture`, issues **#3, #22, #23, #29, #32, #34, #35, #36, #40, #41, #48, #58, #61**. The main roadmap correction I would make is adding **#23, #35 and #41 as explicit `#29` dependencies** if "complete from the start" is now the goal.