# File browser core

`file_browser` is the neutral layer beneath Open/Save/Choose panels (#78). It
owns the provider protocol, `FileEntry`, `FileBrowserModel`, `FileViewMode`,
the one `FileSort` policy and navigation events. A file-manager application
instantiates this layer directly; it does not open an `OpenPanel` and treat the
dialog as Finder.

## Invariants

- **One owner.** `FileBrowserModel` owns the current location, back/forward
  history, listing, hidden filter, view mode and sort. Sidebar, path control and
  content-view navigation all call the same model.
- **Opaque identity.** `LocationId` and `ItemId` are provider-owned handles. A
  display name is never parsed back into a path. `FileEntry::location` is the
  only navigation/IO handle for an entry.
- **Generation-based async.** Every load carries a generation; a result from an
  older generation is dropped. A slow listing cannot overwrite newer
  navigation. Selection survives reloads by stable id.
- **One sort policy.** `FileSort` is used by list, icon and column views, so a
  view switch cannot change the ordering. Folders stay above files.
- **Neutral core.** This layer does not depend on `std::fs`, sheets, dialogs,
  buttons, extension policy or modal state. `StdFsProvider` is one optional
  adapter behind the `std-fs` feature.

## Provider surface

`FileProvider` covers enumeration (`children`), optional metadata enrichment,
ancestors, places, `create_folder`, `rename`, `trash`, `delete_permanently`,
`eject`, name validation, child-location lookup and capability flags. File
operations return `Result`; confirmation and progress presentation belong to a
higher layer. `resolve` is an optional host-path adapter used by Save/Open and
by consumers that need durable paths; virtual providers may return `None`.

## View host

`FileViewHost` is the single switch point for Icons, List and Columns. It wraps
the same `Entity<FileBrowserModel>`, so `set_view` changes only the
presentation: location, history, sort and stable selection stay. A consumer
must not create one model per presentation or reset to the home folder on a
switch. The host also exposes `location`, `selected_ids` and `sort` for
toolbar and status projection.

## Relationship to `file_panel`

`file_panel` consumes this layer for Open, Save and Choose. For the Chaus
migration window, `file_panel::{model, provider}` re-export the generic types
and `file_panel::StdFsProvider` remains available. The panel keeps only dialog
policy: allowed extensions, Save extension disposition, sheets, buttons, search
fields and modal outcomes.
