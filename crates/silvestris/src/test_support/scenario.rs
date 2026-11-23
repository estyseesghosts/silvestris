//! Scenario driver for interaction tests.
//!
//! A `Scenario` owns a test app, one or more windows and the active window. It
//! routes focus, typing, key chords and clicks through GPUI's normal dispatch
//! path, so the test sees the same focus handles and actions as the app.

use std::time::Duration;

use gpui::{
    AnyWindowHandle, App, Bounds, Context, FocusHandle, Modifiers, Pixels, Render, TestAppContext,
    VisualTestContext, Window, WindowHandle,
};

use crate::ThemePreference;

/// A test app with windows, an active window and helpers for driving it.
pub struct Scenario {
    app: TestAppContext,
    windows: Vec<AnyWindowHandle>,
    active: Option<AnyWindowHandle>,
}

impl Default for Scenario {
    fn default() -> Self {
        Self::new()
    }
}

impl Scenario {
    /// Creates an app with the default theme and text-field key bindings.
    #[must_use]
    pub fn new() -> Self {
        let app = TestAppContext::single();
        app.update(|cx| crate::test_support::initialize(ThemePreference::default(), cx));
        Self {
            app,
            windows: Vec::new(),
            active: None,
        }
    }

    /// Opens a window and makes it active.
    pub fn open<V>(
        &mut self,
        build: impl FnOnce(&mut Window, &mut Context<V>) -> V,
    ) -> WindowHandle<V>
    where
        V: Render + 'static,
    {
        let handle = self.app.add_window(build);
        let any = *handle;
        self.windows.push(any);
        self.active = Some(any);
        let mut cx = VisualTestContext::from_window(any, &self.app);
        cx.update(|window, _| window.activate_window());
        self.draw();
        handle
    }

    /// Makes an existing window active.
    pub fn activate(&mut self, window: AnyWindowHandle) {
        assert!(
            self.windows.contains(&window),
            "scenario has no window {window:?}"
        );
        self.active = Some(window);
        let mut cx = self.cx();
        cx.update(|window, _| window.activate_window());
        self.draw();
    }

    /// Windows in the order they were opened.
    #[must_use]
    pub fn windows(&self) -> &[AnyWindowHandle] {
        &self.windows
    }

    /// The window that `update`, `draw` and the input helpers target.
    #[must_use]
    pub fn active_window(&self) -> AnyWindowHandle {
        self.active.expect("scenario has no active window")
    }

    /// The underlying test app, for reading a window root or entity state.
    #[must_use]
    pub fn app(&self) -> &TestAppContext {
        &self.app
    }

    fn cx(&self) -> VisualTestContext {
        VisualTestContext::from_window(self.active_window(), &self.app)
    }

    /// Runs a closure with the active window and the app.
    pub fn update<R>(&mut self, f: impl FnOnce(&mut Window, &mut App) -> R) -> R {
        self.cx().update(f)
    }

    /// Runs pending work and draws the active window.
    pub fn draw(&mut self) {
        let mut cx = self.cx();
        cx.run_until_parked();
        cx.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        cx.run_until_parked();
    }

    /// Advances GPUI's test clock and processes one frame.
    pub fn advance(&mut self, duration: Duration) {
        self.update(|window, cx| {
            cx.background_executor().advance_clock(duration);
            window.simulate_next_frame(cx);
        });
        self.draw();
    }

    /// Clicks the center of the element with this debug selector.
    ///
    /// # Panics
    ///
    /// Panics when no rendered element uses the selector.
    pub fn click(&mut self, selector: &'static str) {
        let mut cx = self.cx();
        let bounds = cx
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("no element for selector {selector:?}"));
        cx.simulate_click(bounds.center(), Modifiers::default());
        self.draw();
    }

    /// Types text into the focused input handler.
    pub fn type_text(&mut self, text: &str) {
        let mut cx = self.cx();
        cx.simulate_input(text);
        self.draw();
    }

    /// Presses a space-separated key chord.
    pub fn press(&mut self, keystrokes: &str) {
        let mut cx = self.cx();
        cx.simulate_keystrokes(keystrokes);
        self.draw();
    }

    /// Focuses a handle and redraws.
    pub fn focus(&mut self, handle: &FocusHandle) {
        self.update(|window, cx| window.focus(handle, cx));
        self.draw();
    }

    /// The active window's focused handle, if any.
    #[must_use]
    pub fn focused(&mut self) -> Option<FocusHandle> {
        self.update(|window, cx| window.focused(cx))
    }

    /// Asserts that a handle is focused after a redraw.
    pub fn assert_focused(&mut self, handle: &FocusHandle) {
        self.draw();
        let focused = self.update(|window, _| handle.is_focused(window));
        assert!(focused, "expected {handle:?} to be focused");
    }

    /// Asserts that no element is focused after a redraw.
    pub fn assert_no_focus(&mut self) {
        self.draw();
        let focused = self.focused();
        assert!(focused.is_none(), "expected no focus, found {focused:?}");
    }

    /// The bounds of an element with this debug selector.
    ///
    /// # Panics
    ///
    /// Panics when no rendered element uses the selector.
    #[must_use]
    pub fn bounds(&mut self, selector: &'static str) -> Bounds<Pixels> {
        self.cx()
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("no element for selector {selector:?}"))
    }

    /// The active window's title.
    #[must_use]
    pub fn window_title(&mut self) -> Option<String> {
        self.cx().window_title()
    }

    /// Closes the active window and activates the most recent remaining one.
    pub fn close(&mut self) -> bool {
        let active = self.active_window();
        let mut cx = self.cx();
        let closed = cx.simulate_close();
        drop(cx);
        self.app.run_until_parked();
        if closed {
            self.windows.retain(|window| *window != active);
            match self.windows.last().copied() {
                Some(window) => self.activate(window),
                None => self.active = None,
            }
        }
        closed
    }
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;
    use std::time::Duration;

    use gpui::{
        actions, div, prelude::*, px, size, AnyWindowHandle, Context, FocusHandle, KeyBinding,
        Render, Window,
    };

    use super::Scenario;

    actions!(scenario_tests, [Increment]);

    struct Harness {
        focus: FocusHandle,
        clicks: Rc<Cell<usize>>,
    }

    impl Render for Harness {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .track_focus(&self.focus)
                .on_action(cx.listener(|this, _: &Increment, _, _| {
                    this.clicks.set(this.clicks.get() + 1);
                }))
                .child(
                    div()
                        .id("scenario-target")
                        .debug_selector(|| "scenario-target".into())
                        .w(px(40.))
                        .h(px(20.))
                        .on_click(cx.listener(|this, _, _, _| {
                            this.clicks.set(this.clicks.get() + 1);
                        })),
                )
        }
    }

    fn open_harness(
        scenario: &mut Scenario,
        clicks: Rc<Cell<usize>>,
    ) -> (AnyWindowHandle, FocusHandle) {
        let slot: Rc<RefCell<Option<FocusHandle>>> = Rc::new(RefCell::new(None));
        let build_slot = slot.clone();
        let handle = scenario.open(move |window, cx| {
            window.on_window_should_close(cx, |_, _| true);
            cx.bind_keys([KeyBinding::new("space", Increment, None)]);
            let focus = cx.focus_handle();
            *build_slot.borrow_mut() = Some(focus.clone());
            Harness { focus, clicks }
        });
        let focus = slot.borrow().clone().expect("focus handle");
        (*handle, focus)
    }

    #[test]
    fn focus_click_press_advance_and_close() {
        let mut scenario = Scenario::new();
        let clicks = Rc::new(Cell::new(0));
        let (_window, focus) = open_harness(&mut scenario, clicks.clone());

        scenario.focus(&focus);
        scenario.assert_focused(&focus);

        scenario.click("scenario-target");
        assert_eq!(clicks.get(), 1);

        scenario.press("space");
        assert_eq!(clicks.get(), 2);

        scenario.advance(Duration::from_millis(16));
        assert_eq!(
            scenario.bounds("scenario-target").size,
            size(px(40.), px(20.))
        );

        assert!(scenario.close());
        assert!(scenario.windows().is_empty());
    }

    #[test]
    fn opens_activates_and_closes_windows_in_order() {
        let mut scenario = Scenario::new();
        let (first, _) = open_harness(&mut scenario, Rc::new(Cell::new(0)));
        let (second, _) = open_harness(&mut scenario, Rc::new(Cell::new(0)));

        assert_eq!(scenario.windows().len(), 2);
        assert_eq!(scenario.windows()[0], first);
        assert_eq!(scenario.windows()[1], second);
        assert_eq!(scenario.active_window(), second);

        scenario.activate(first);
        assert_eq!(scenario.active_window(), first);

        assert!(scenario.close());
        assert_eq!(scenario.active_window(), second);
        assert!(scenario.close());
        assert!(scenario.windows().is_empty());
    }
}
