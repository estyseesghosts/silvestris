Yes. The hardening is less about the visual panel now and more about making `StdFsProvider` a **real filesystem adapter rather than a demo adapter**.

Current head is still `04febbb9`. I would fix these before treating Chaus as serious dogfood.

### 1. Stop using display strings as filesystem identity

This is the biggest correctness issue.

Today `LocationId` is just a `SharedString`, and `StdFsProvider` effectively does:

```rust
path.display().to_string() -> LocationId
LocationId.as_ref() -> PathBuf
```

It does the same kind of thing for file names with `to_string_lossy()`. [path_control.rs](https://github.com/estyseesghosts/silvestris/blob/04febbb9247a67880ce753d7d6727e1ccea8cfcc/crates/silvestris/src/controls/path_control.rs#L19-L24) [provider.rs](https://github.com/estyseesghosts/silvestris/blob/04febbb9247a67880ce753d7d6727e1ccea8cfcc/crates/silvestris/src/file_panel/provider.rs#L646-L725)

That is incorrect because **display text is not necessarily a reversible filesystem path**. On Unix, a filename can contain arbitrary non-NUL bytes. On Windows, `OsString` can contain values that do not round-trip cleanly through Rust UTF-8 strings. `to_string_lossy()` can replace unrepresentable data.

Worse, `FileBrowserModel` reconstructs children using the *display name*:

```rust
let child = path.join(entry.name.as_ref());
```

[model.rs](https://github.com/estyseesghosts/silvestris/blob/04febbb9247a67880ce753d7d6727e1ccea8cfcc/crates/silvestris/src/file_panel/model.rs#L279-L327)

So even if `StdFsProvider` internally knew the original path, that identity is thrown away.

I would change the model so the provider remains authoritative:

```text
FileEntry
    id             opaque stable identity
    location       opaque LocationId
    name           display-only SharedString
    metadata...
```

Then:

```rust
entry.location       // navigate/select this
provider.resolve(&entry.location) // obtain real PathBuf
```

The panel should never do:

```rust
parent_path.join(entry.name)
```

for an existing file.

For `StdFsProvider`, internally map opaque IDs to real `PathBuf`s. Something as simple as:

```text
fs-location-000001 -> PathBuf
fs-location-000002 -> PathBuf
```

is better than encoding paths into UI strings.

That lets the UI display a lossy readable label where unavoidable while retaining the exact path for actual IO.

---

### 2. Make filename validation provider-owned

Current SavePanel considers a filename valid when it isn't empty, `.` or `..`, and doesn't contain:

```text
/
\
:
```

[panel.rs](https://github.com/estyseesghosts/silvestris/blob/04febbb9247a67880ce753d7d6727e1ccea8cfcc/crates/silvestris/src/file_panel/panel.rs#L925-L951)

That's neither Windows validation nor Unix validation.

On Windows, it misses numerous invalid cases: reserved characters, reserved device names such as `CON`, trailing dots/spaces, etc. On Unix, `:` is perfectly valid, so Silvestris rejects names the filesystem accepts.

This belongs in `FileProvider`.

I'd add something like:

```rust
fn validate_name(
    &self,
    parent: &LocationId,
    name: &str,
    cx: &App,
) -> Result<(), FileError>;
```

SavePanel itself should enforce only structural UI requirements such as empty names and `.` / `..`. `StdFsProvider` then applies Windows/Linux/macOS filesystem policy.

That also makes custom providers work correctly. A remote-storage provider may have totally different filename restrictions.

---

### 3. `exists()` must distinguish “missing” from “couldn't determine”

Current API:

```rust
fn exists(...) -> Task<bool>;
```

and `StdFsProvider` does:

```rust
path.exists()
```

[provider.rs](https://github.com/estyseesghosts/silvestris/blob/04febbb9247a67880ce753d7d6727e1ccea8cfcc/crates/silvestris/src/file_panel/provider.rs#L803-L806)

This is dangerous for SavePanel. `Path::exists()` returns `false` when it cannot determine whether the path exists.

So:

```text
permission/error
        ↓
exists() == false
        ↓
SavePanel thinks target doesn't exist
        ↓
no Replace confirmation
```

Change it to:

```rust
fn exists(...) -> Task<Result<bool, FileError>>;
```

and implement local files with `try_exists()`.

Then SavePanel has three real states:

```text
doesn't exist → accept
exists        → ask Replace
error         → stay open and show error
```

Never reinterpret an IO error as “file isn't there.”

---

### 4. Tell the consumer whether replacement was authorized

This becomes particularly important for Chaus.

SavePanel currently checks existence, shows the Replace alert, and eventually returns:

```rust
PanelOutcome {
    result,
    paths,
    locations,
    hide_extension,
}
```

There is no indication whether this was:

```text
new target
```

or:

```text
existing target for which the user explicitly clicked Replace
```

[panel.rs](https://github.com/estyseesghosts/silvestris/blob/04febbb9247a67880ce753d7d6727e1ccea8cfcc/crates/silvestris/src/file_panel/panel.rs#L35-L75) [save flow](https://github.com/estyseesghosts/silvestris/blob/04febbb9247a67880ce753d7d6727e1ccea8cfcc/crates/silvestris/src/file_panel/panel.rs#L953-L1028)

I'd add:

```rust
pub enum SaveDisposition {
    New,
    ReplaceConfirmed,
}
```

to the accepted Save outcome.

That solves an important race for Chaus.

If SavePanel said:

```text
New
```

Chaus can use its existing `refuse_existing` behavior. If another process creates the file between panel acceptance and the write, Chaus refuses instead of silently overwriting it.

If SavePanel said:

```text
ReplaceConfirmed
```

Chaus knows the user explicitly authorized replacement and can take a conflict baseline before its asynchronous write.

The panel still does **zero writing**. It is simply communicating the user's decision accurately.

---

### 5. Remove filesystem capabilities the provider doesn't actually have

The panel currently always inserts context-menu commands for:

```text
Rename
Delete
New Folder
Copy Path
```

[panel.rs](https://github.com/estyseesghosts/silvestris/blob/04febbb9247a67880ce753d7d6727e1ccea8cfcc/crates/silvestris/src/file_panel/panel.rs#L438-L535)

But `FileProvider::rename()` defaults to:

```text
"This provider cannot rename entries"
```

and `StdFsProvider` currently doesn't override it.

That's a UI-contract violation. A control should not advertise an operation that the provider says it can't perform.

Add provider capabilities, for example:

```rust
struct FileProviderCapabilities {
    create_folder: bool,
    rename: bool,
    trash: bool,
    copy_path: bool,
}
```

or individual methods:

```rust
fn can_rename(&self, ...) -> bool;
```

Then construct menus from capabilities.

This becomes useful far beyond local filesystems—SSH, archive browsers, virtual project trees, cloud storage, etc.

---

### 6. Remove permanent recursive Delete from the file picker

This one I'd change before Chaus uses the panel at all.

`StdFsProvider::delete()` currently does:

```rust
if path.is_dir() {
    std::fs::remove_dir_all(&path)
} else {
    std::fs::remove_file(&path)
}
```

[provider.rs](https://github.com/estyseesghosts/silvestris/blob/04febbb9247a67880ce753d7d6727e1ccea8cfcc/crates/silvestris/src/file_panel/provider.rs#L808-L843)

And the context menu invokes it directly.

That means a file-picker context menu can recursively, permanently delete a directory.

For an Aqua/Finder-style UI, the right semantic is much closer to **Move to Trash**, not raw recursive deletion. Even independent of Aqua fidelity, irreversible deletion is too dangerous for a generic picker.

For now I would simply remove Delete from `StdFsProvider`/the panel.

Later add something explicit:

```rust
fn trash(...)
```

backed by a tested platform trash implementation. Don't hand-roll recycle-bin behavior.

---

### 7. Fix the fake places

Current `StdFsProvider::places()` literally contains:

```rust
"terezi"
```

for Home. It also advertises:

```text
Today
Yesterday
Past Week
```

using IDs:

```text
search:today
search:yesterday
search:week
```

[provider.rs](https://github.com/estyseesghosts/silvestris/blob/04febbb9247a67880ce753d7d6727e1ccea8cfcc/crates/silvestris/src/file_panel/provider.rs#L753-L802)

But filesystem search is explicitly listed as deferred in `file-panels.md`.

If the user clicks one of those today, `StdFsProvider` has no special search implementation. That synthetic ID eventually looks like a filesystem location.

Those entries should not exist until they work.

Home should use a real label—probably the final component of the actual home directory, or a generic `"Home"` label depending on the intended UI.

So immediately:

```text
remove SEARCH FOR section
replace "terezi" with dynamic Home label
```

Then implement search later as a real provider concept.

---

### 8. Make Places actually platform-aware

The current implementation assumes:

```text
$HOME/Desktop
$HOME/Documents
$HOME/Downloads
$HOME/Applications
```

and checks those directories with `path.exists()`.

That's not reliable cross-platform.

On Windows, Desktop/Documents/Downloads can be redirected and should come from Known Folder APIs. The machine may have several drives, yet current `"Computer"` effectively identifies only the root of the drive containing the user's home directory.

On Linux, XDG user directories can have custom/localized locations. `~/Documents` is not authoritative.

I'd split this out:

```text
std_fs/
    mod.rs
    windows.rs
    linux.rs
    macos.rs
```

with a common result:

```rust
struct HostPlace {
    path: PathBuf,
    label: SharedString,
    icon: Icon,
    section: PlaceSection,
}
```

The Windows implementation should enumerate actual known folders and volumes. Linux should consult XDG user-dir configuration rather than assuming English folder names.

This should be reusable host-platform plumbing, not sprinkled through `places()`.

---

### 9. Don't perform filesystem probes on the GPUI thread

`children()`, `create_folder()` and `delete()` correctly use the background executor.

But `places()` currently calls:

```rust
path.exists()
```

synchronously while constructing the UI. [provider.rs](https://github.com/estyseesghosts/silvestris/blob/04febbb9247a67880ce753d7d6727e1ccea8cfcc/crates/silvestris/src/file_panel/provider.rs#L762-L777)

Usually that's fast. It isn't guaranteed to be fast.

Redirected profiles, network shares, disconnected mounts or weird filesystems can turn an innocent metadata probe into a stall.

For this particular code, the easiest fix is not necessarily making `places()` asynchronous. Better platform discovery can provide the known paths without probing every one during render. If availability must be tested, populate/cache those results outside the hot rendering path.

The general invariant should be:

> `FileProvider` methods callable synchronously by rendering/model code do no blocking filesystem IO.

---

### 10. Stop silently swallowing directory and metadata errors

Current directory loading uses:

```rust
for item in read.flatten()
```

and then:

```rust
let metadata = item.metadata().ok();
```

[provider.rs](https://github.com/estyseesghosts/silvestris/blob/04febbb9247a67880ce753d7d6727e1ccea8cfcc/crates/silvestris/src/file_panel/provider.rs#L681-L725)

Both discard errors.

An entry whose metadata fails can consequently appear as:

```text
not a directory
0 bytes
modified = 0
```

instead of “metadata unavailable.”

And an error produced while iterating `read_dir()` simply makes a file disappear.

For a real file browser provider, I'd introduce:

```rust
struct FileListing {
    entries: Vec<FileEntry>,
    warnings: Vec<FileError>,
}
```

Then:

```rust
children(...) -> Task<Result<FileListing, FileError>>
```

The distinction is useful:

```text
cannot open directory
    → fatal FileError

one entry disappeared halfway through enumeration
    → directory still works, optional warning

metadata unavailable for one entry
    → entry can still be represented appropriately
```

At minimum, remove `flatten()` and `.ok()` so every ignored failure is an intentional decision.

---

### 11. Define symlink/reparse-point behavior explicitly

Right now `StdFsProvider` has no explicit concept of links. It asks metadata whether an entry is a directory and treats the resulting thing as a folder/document/application.

For a real host filesystem provider we need to decide:

```text
Unix symlink to directory:
    navigable folder?
    displayed as link?
    delete link or target?

Windows junction / reparse point:
    navigate?
    icon?
    recursion behavior?
```

Use `symlink_metadata()` / platform metadata to classify first, then decide policy deliberately.

This becomes particularly important before reintroducing any destructive operation.

---

## The architecture I would aim for

Right now the relationship is partly:

```text
FileEntry display name
        ↓
FileBrowserModel reconstructs a path
        ↓
StdFsProvider
```

That is backwards.

Make it:

```text
                 ┌── opaque LocationId ─────────┐
                 │                              ▼
StdFsProvider ───┼── display label ───────> FilePanel
(real PathBuf)   │
                 ├── exact child identity
                 ├── capabilities
                 ├── validation
                 └── filesystem operations

FilePanel
    never parses paths
    never constructs existing-file paths
    never guesses filesystem rules
    never performs filesystem IO
```

That matches the abstraction the source comments already claim:

> “The panels never touch `std::fs`. A provider answers with stable location ids…”

The current design has the right conceptual boundary; `StdFsProvider` just doesn't fully honor it yet. [provider.rs](https://github.com/estyseesghosts/silvestris/blob/04febbb9247a67880ce753d7d6727e1ccea8cfcc/crates/silvestris/src/file_panel/provider.rs#L1-L11)

For Chaus specifically, I would consider **lossless identity, provider-side validation, fallible existence checking, Save replacement disposition, and removal of unsafe Delete** the hard blockers. The dynamic places/platform cleanup and richer listing-error model can follow immediately afterward, but I would still do them before calling `StdFsProvider` generally production-ready.
