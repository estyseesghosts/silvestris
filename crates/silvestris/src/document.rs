//! Document protocol: identity, dirty state and the close decision (#37).
//!
//! Silvestris owns the protocol, never the file format or IO. An application
//! implements [`Document`] for its model; [`DocumentController`] runs the
//! close decision and [`Documents`] numbers untitled windows without reusing a
//! name that is still open.
//!
//! The close flow mirrors HIG p. 246 and p. 221:
//!
//! 1. A close request on a clean document closes it.
//! 2. A dirty document asks for a decision. Cancel keeps it open; Don't Save
//!    closes it; Save runs the document's save.
//! 3. If save needs a location, the app opens its Save panel (#29) and reports
//!    the result back. A failed save keeps the document open.
//!
//! A request while a save is in flight returns `Stay` and does not prompt
//! again, so closing twice cannot double-prompt. The sheet itself is the app's
//! content in a `modal::ModalSession` (#21/#22); conflict and recovery prompts
//! stay app-owned hooks.

use std::collections::BTreeSet;
use std::rc::Rc;

use gpui::{AnyWindowHandle, App, Context, Entity, SharedString, Window};

use crate::file_panel::{FilePanel, FileProvider, OpenPanel, SavePanel};

/// What a save attempt did. `NeedsPanel` means the document has no location
/// and the application must open its Save panel (#29).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SaveResult {
    Saved,
    Failed(String),
    NeedsPanel,
}

/// The document model an application owns. The default `save_as` asks for a
/// panel; `url` is only for the title/proxy display.
pub trait Document: 'static {
    fn display_name(&self) -> SharedString;
    fn url(&self) -> Option<String> {
        None
    }
    fn is_dirty(&self) -> bool;
    fn save(&mut self, cx: &mut Context<Self>) -> SaveResult
    where
        Self: Sized;
    /// Save to a destination chosen by a panel. The default ignores the path
    /// and calls `save`, which is right for a document that already knows its
    /// location. Documents that adopt the Save panel implement this.
    fn save_to(&mut self, _path: &std::path::Path, cx: &mut Context<Self>) -> SaveResult
    where
        Self: Sized,
    {
        self.save(cx)
    }
    fn save_as(&mut self, cx: &mut Context<Self>) -> SaveResult
    where
        Self: Sized,
    {
        self.save(cx)
    }
    fn revert(&mut self, cx: &mut Context<Self>)
    where
        Self: Sized;
}

/// What the user chose in a close prompt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PromptChoice {
    Save,
    DontSave,
    Cancel,
}

/// The next step the application takes after a close request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseStep {
    /// Remove the window.
    Close,
    /// Present the app's save sheet in a modal session.
    Prompt,
    /// A save is running or needs a Save panel; do not prompt again.
    Saving,
    /// Keep the window open.
    Stay,
}

/// Erased access to one document, so a controller is not generic.
pub trait DocumentHandle {
    fn display_name(&self, cx: &App) -> SharedString;
    fn url(&self, cx: &App) -> Option<String>;
    fn is_dirty(&self, cx: &App) -> bool;
    fn save(&self, cx: &mut App) -> SaveResult;
    fn revert(&self, cx: &mut App);
}

impl<T: Document> DocumentHandle for Entity<T> {
    fn display_name(&self, cx: &App) -> SharedString {
        self.read(cx).display_name()
    }

    fn url(&self, cx: &App) -> Option<String> {
        self.read(cx).url()
    }

    fn is_dirty(&self, cx: &App) -> bool {
        self.read(cx).is_dirty()
    }

    fn save(&self, cx: &mut App) -> SaveResult {
        self.update(cx, |document, cx| document.save(cx))
    }

    fn revert(&self, cx: &mut App) {
        self.update(cx, |document, cx| document.revert(cx));
    }
}

/// Title and dirty state for rendering.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentTitle {
    pub name: SharedString,
    pub edited: bool,
    pub url: Option<String>,
}

impl DocumentTitle {
    #[must_use]
    pub fn new(name: impl Into<SharedString>, edited: bool, url: Option<String>) -> Self {
        Self {
            name: name.into(),
            edited,
            url,
        }
    }

    /// The window title. 10.6 shows the document name, not a path and not an
    /// "edited" suffix; the close button carries the dot.
    #[must_use]
    pub fn window_title(&self) -> String {
        self.name.to_string()
    }

    /// Tooltip for the proxy icon.
    #[must_use]
    pub fn proxy_tooltip(&self) -> Option<&str> {
        self.url.as_deref()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Idle,
    Prompt,
    Saving,
}

/// Runs the close decision for one window's document.
pub struct DocumentController {
    document: Box<dyn DocumentHandle>,
    phase: Phase,
}

impl DocumentController {
    #[must_use]
    pub fn new<T: Document>(document: Entity<T>) -> Self {
        Self {
            document: Box::new(document),
            phase: Phase::Idle,
        }
    }

    #[must_use]
    pub fn title(&self, cx: &App) -> DocumentTitle {
        DocumentTitle::new(
            self.document.display_name(cx),
            self.document.is_dirty(cx),
            self.document.url(cx),
        )
    }

    /// A close was requested. Clean documents close; dirty ones prompt; a save
    /// already in flight returns `Saving` without prompting again.
    pub fn request_close(&mut self, cx: &App) -> CloseStep {
        match self.phase {
            Phase::Saving => CloseStep::Saving,
            Phase::Prompt => CloseStep::Prompt,
            Phase::Idle => {
                if self.document.is_dirty(cx) {
                    self.phase = Phase::Prompt;
                    CloseStep::Prompt
                } else {
                    CloseStep::Close
                }
            }
        }
    }

    /// Resolve a prompt. `Save` starts the save and returns what it needs.
    pub fn resolve_prompt(&mut self, choice: PromptChoice, cx: &mut App) -> CloseStep {
        if self.phase != Phase::Prompt {
            return CloseStep::Stay;
        }
        match choice {
            PromptChoice::Cancel => {
                self.phase = Phase::Idle;
                CloseStep::Stay
            }
            PromptChoice::DontSave => {
                self.phase = Phase::Idle;
                CloseStep::Close
            }
            PromptChoice::Save => {
                self.phase = Phase::Saving;
                match self.document.save(cx) {
                    SaveResult::Saved => {
                        self.phase = Phase::Idle;
                        CloseStep::Close
                    }
                    SaveResult::NeedsPanel => CloseStep::Saving,
                    SaveResult::Failed(_) => {
                        self.phase = Phase::Idle;
                        CloseStep::Stay
                    }
                }
            }
        }
    }

    /// Report the result of a Save panel (#29). Only meaningful while saving.
    pub fn resolve_save(&mut self, result: SaveResult) -> CloseStep {
        if self.phase != Phase::Saving {
            return CloseStep::Stay;
        }
        match result {
            SaveResult::Saved => {
                self.phase = Phase::Idle;
                CloseStep::Close
            }
            SaveResult::NeedsPanel => CloseStep::Saving,
            SaveResult::Failed(_) => {
                self.phase = Phase::Idle;
                CloseStep::Stay
            }
        }
    }

    /// Revert through the controller. The app's Revert command calls this.
    pub fn revert(&mut self, cx: &mut App) {
        self.document.revert(cx);
    }

    #[must_use]
    pub fn is_saving(&self) -> bool {
        self.phase == Phase::Saving
    }

    #[must_use]
    pub fn is_prompting(&self) -> bool {
        self.phase == Phase::Prompt
    }
}

/// Open untitled names. A closed name is released, and a new untitled window
/// never takes a number that another open window still uses.
#[derive(Default)]
pub struct Documents {
    open: BTreeSet<SharedString>,
}

impl Documents {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `Untitled`, then `Untitled 2`, `Untitled 3`, skipping any name that is
    /// still open.
    pub fn next_untitled(&mut self) -> SharedString {
        let mut index = 1;
        loop {
            let name: SharedString = if index == 1 {
                "Untitled".into()
            } else {
                format!("Untitled {index}").into()
            };
            if !self.open.contains(&name) {
                self.open.insert(name.clone());
                return name;
            }
            index += 1;
        }
    }

    /// Register a named document (opened from a path).
    pub fn open(&mut self, name: impl Into<SharedString>) -> SharedString {
        let name = name.into();
        self.open.insert(name.clone());
        name
    }

    /// Release a name when its window closes.
    pub fn close(&mut self, name: &str) {
        self.open.remove(name);
    }

    #[must_use]
    pub fn is_open(&self, name: &str) -> bool {
        self.open.contains(name)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.open.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.open.is_empty()
    }
}

/// Standard File-menu command ids. Apps register handlers that call the
/// helpers below; the ids match the HIG menu titles.
pub mod commands {
    pub const SAVE: &str = "file.save";
    pub const SAVE_AS: &str = "file.save-as";
    pub const OPEN: &str = "file.open";
}

/// Present the Save panel for `document` as a sheet over `owner`, saving the
/// chosen path through [`Document::save_to`].
///
/// The suggested name is the document's display name. A document with no
/// `save_to` implementation keeps its old save behaviour.
pub fn save_as_panel<T: Document>(
    document: Entity<T>,
    provider: Rc<dyn FileProvider>,
    owner: AnyWindowHandle,
    window: &mut Window,
    cx: &mut App,
) -> Entity<FilePanel> {
    save_as_panel_with(document, provider, owner, window, cx, |_, _| {})
}

/// Like [`save_as_panel`], and reports the save result so a close flow can
/// call `DocumentController::resolve_save`.
///
/// ```ignore
/// match controller.resolve_prompt(PromptChoice::Save, cx) {
///     CloseStep::Saving => {
///         document::save_as_panel_with(document, provider, owner, window, cx,
///             move |result, cx| { controller.resolve_save(result); });
///     }
///     CloseStep::Close => window.remove_window(),
///     _ => {}
/// }
/// ```
pub fn save_as_panel_with<T: Document>(
    document: Entity<T>,
    provider: Rc<dyn FileProvider>,
    owner: AnyWindowHandle,
    window: &mut Window,
    cx: &mut App,
    report: impl Fn(SaveResult, &mut App) + 'static,
) -> Entity<FilePanel> {
    let name = document.read(cx).display_name().to_string();
    let report = Rc::new(report);
    SavePanel::new(provider)
        .name(name)
        .on_finish(move |outcome, cx| {
            let result = match outcome.first_path() {
                Some(path) => document.update(cx, |document, cx| document.save_to(path, cx)),
                None if outcome.accepted() => SaveResult::NeedsPanel,
                None => SaveResult::Failed("The chosen destination has no host path".into()),
            };
            report(result, cx);
        })
        .present(Some(owner), window, cx)
}

/// Present the Open panel application-modal and call `on_open` for each chosen
/// host path. Returns the panel entity; the callback is the supported result.
pub fn open_panel(
    provider: Rc<dyn FileProvider>,
    window: &mut Window,
    cx: &mut App,
    on_open: impl Fn(&std::path::Path, &mut App) + 'static,
) -> Entity<FilePanel> {
    OpenPanel::new(provider)
        .can_choose_files(true)
        .can_choose_directories(false)
        .on_finish(move |outcome, cx| {
            for path in &outcome.paths {
                on_open(path, cx);
            }
        })
        .present(None, window, cx)
}

/// Keep the title in step with the document. Call it when the document emits a
/// change or after a save/revert.
pub fn sync_title<T: Document>(
    document: &Entity<T>,
    window: &mut Window,
    cx: &App,
) -> DocumentTitle {
    let title = DocumentTitle::new(
        document.read(cx).display_name(),
        document.read(cx).is_dirty(),
        document.read(cx).url(),
    );
    window.set_window_title(&title.window_title());
    title
}

#[cfg(test)]
mod tests {
    use gpui::{AppContext, TestAppContext};

    use super::*;

    struct Demo {
        name: SharedString,
        dirty: bool,
        save: SaveResult,
        saves: usize,
        reverts: usize,
    }

    impl Document for Demo {
        fn display_name(&self) -> SharedString {
            self.name.clone()
        }

        fn is_dirty(&self) -> bool {
            self.dirty
        }

        fn save(&mut self, _: &mut Context<Self>) -> SaveResult {
            self.saves += 1;
            match self.save.clone() {
                SaveResult::Saved => {
                    self.dirty = false;
                    SaveResult::Saved
                }
                other => other,
            }
        }

        fn revert(&mut self, _: &mut Context<Self>) {
            self.reverts += 1;
            self.dirty = false;
        }
    }

    fn demo(app: &mut TestAppContext, save: SaveResult) -> Entity<Demo> {
        app.update(|cx| {
            cx.new(|_| Demo {
                name: "Untitled".into(),
                dirty: true,
                save,
                saves: 0,
                reverts: 0,
            })
        })
    }

    fn close(controller: &mut DocumentController, app: &mut TestAppContext) -> CloseStep {
        app.update(|cx| controller.request_close(cx))
    }

    fn prompt(
        controller: &mut DocumentController,
        app: &mut TestAppContext,
        choice: PromptChoice,
    ) -> CloseStep {
        app.update(|cx| controller.resolve_prompt(choice, cx))
    }

    fn title(controller: &DocumentController, app: &TestAppContext) -> DocumentTitle {
        app.update(|cx| controller.title(cx))
    }

    #[test]
    fn a_clean_document_closes_without_a_prompt() {
        let mut app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        let document = demo(&mut app, SaveResult::Saved);
        document.update(&mut app, |document, _| document.dirty = false);
        let mut controller = DocumentController::new(document);
        assert_eq!(close(&mut controller, &mut app), CloseStep::Close);
    }

    #[test]
    fn a_dirty_close_honours_each_prompt_button() {
        let mut app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        let document = demo(&mut app, SaveResult::Saved);
        let mut controller = DocumentController::new(document.clone());

        assert_eq!(close(&mut controller, &mut app), CloseStep::Prompt);
        assert_eq!(
            prompt(&mut controller, &mut app, PromptChoice::Cancel),
            CloseStep::Stay
        );
        assert!(title(&controller, &app).edited);

        assert_eq!(close(&mut controller, &mut app), CloseStep::Prompt);
        assert_eq!(
            prompt(&mut controller, &mut app, PromptChoice::DontSave),
            CloseStep::Close
        );
        assert!(app.update(|cx| document.read(cx).is_dirty()));

        let mut controller = DocumentController::new(document.clone());
        assert_eq!(close(&mut controller, &mut app), CloseStep::Prompt);
        assert_eq!(
            prompt(&mut controller, &mut app, PromptChoice::Save),
            CloseStep::Close
        );
        assert!(!app.update(|cx| document.read(cx).is_dirty()));
        assert_eq!(app.update(|cx| document.read(cx).saves), 1);
    }

    #[test]
    fn a_save_that_needs_a_panel_stays_saving_and_never_double_prompts() {
        let mut app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        let document = demo(&mut app, SaveResult::NeedsPanel);
        let mut controller = DocumentController::new(document);
        assert_eq!(close(&mut controller, &mut app), CloseStep::Prompt);
        assert_eq!(
            prompt(&mut controller, &mut app, PromptChoice::Save),
            CloseStep::Saving
        );
        assert!(controller.is_saving());
        assert_eq!(
            close(&mut controller, &mut app),
            CloseStep::Saving,
            "a second close request does not prompt while saving"
        );
        assert_eq!(controller.resolve_save(SaveResult::Saved), CloseStep::Close);
    }

    #[test]
    fn a_failed_save_keeps_the_window_open() {
        let mut app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        let document = demo(&mut app, SaveResult::Failed("disk full".into()));
        let mut controller = DocumentController::new(document.clone());
        assert_eq!(close(&mut controller, &mut app), CloseStep::Prompt);
        assert_eq!(
            prompt(&mut controller, &mut app, PromptChoice::Save),
            CloseStep::Stay
        );
        assert!(app.update(|cx| document.read(cx).is_dirty()));
        assert_eq!(
            close(&mut controller, &mut app),
            CloseStep::Prompt,
            "a failure returns to a promptable state"
        );
    }

    #[test]
    fn untitled_numbers_skip_open_windows_and_are_released() {
        let mut documents = Documents::new();
        assert_eq!(documents.next_untitled(), "Untitled");
        assert_eq!(documents.next_untitled(), "Untitled 2");
        assert_eq!(documents.next_untitled(), "Untitled 3");
        documents.close("Untitled 2");
        assert_eq!(documents.next_untitled(), "Untitled 2");
        assert!(documents.is_open("Untitled"));
        assert_eq!(documents.len(), 3);
    }

    #[test]
    fn revert_clears_dirty_and_the_title_carries_it() {
        let mut app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        let document = demo(&mut app, SaveResult::Saved);
        let mut controller = DocumentController::new(document.clone());
        let before = title(&controller, &app);
        assert!(before.edited);
        assert_eq!(before.window_title(), "Untitled");
        app.update(|cx| controller.revert(cx));
        assert!(!title(&controller, &app).edited);
        assert_eq!(app.update(|cx| document.read(cx).reverts), 1);
    }
}
