//! The one platform-text adapter shared by every Silvestris text client (#58).
//!
//! `text` owns what is edited; this module owns the GPUI side every client
//! repeats: the `EntityInputHandler` methods (UTF-16 ranges, marked text,
//! secure queries), selection notifications and the caret blink timer.
//! A client implements [`TextClient`] for its layout-specific parts and gets
//! the input handler from [`impl_text_input_handler!`], so there is no second
//! implementation to drift.
//!
//! Blink policy: the timer runs only while the client is focused and its
//! window is active. An inactive window draws a steady caret and schedules no
//! wakeups (#58 regression trap; lotus measured idle redraw costs). Activation
//! restarts it through [`observe_activation`].

use std::ops::Range;

use gpui::{Bounds, Context, FocusHandle, Pixels, Point, Subscription, UTF16Selection, Window};
use unicode_segmentation::UnicodeSegmentation;

use crate::text::{CaretBlink, Change, EditSession, Selection};

/// Bullet used for secure text (U+2022).
pub const BULLET: &str = "\u{2022}";

/// A view that edits an [`EditSession`]. The layout-specific parts (where a
/// range is on screen, which offset is under a point) stay with the view.
pub trait TextClient: Sized + 'static {
    fn session(&self) -> &EditSession;
    fn session_mut(&mut self) -> &mut EditSession;
    fn text_focus(&self) -> &FocusHandle;
    fn caret(&mut self) -> &mut CaretBlink;
    fn is_disabled(&self) -> bool;
    /// Secure text: platform queries see one bullet per grapheme.
    fn is_secure(&self) -> bool {
        false
    }
    /// Record a user edit in the client's history and emit its Changed event.
    /// Composing changes arrive here too; history skips them.
    fn did_change(&mut self, change: &Change, cx: &mut Context<Self>);
    /// Screen bounds of a byte range, from the last layout.
    fn range_bounds(&self, range: Range<usize>) -> Option<Bounds<Pixels>>;
    /// Byte offset under `point`, or None outside the text box.
    fn offset_at(&self, point: Point<Pixels>) -> Option<usize>;
}

// --- caret -----------------------------------------------------------------

/// Show the caret and start a blink timer for the current focus epoch. No
/// timer starts while the window is inactive, and a running timer ends with a
/// steady caret when the window deactivates.
pub fn restart_blink<T: TextClient>(this: &mut T, window: &mut Window, cx: &mut Context<T>) {
    let epoch = this.caret().restart();
    cx.notify();
    if !window.is_window_active() {
        return;
    }
    let Some(interval) = crate::native::caret_blink() else {
        return;
    };
    cx.spawn_in(window, async move |this, cx| loop {
        cx.background_executor().timer(interval).await;
        let keep = this
            .update_in(cx, |this, window, cx| {
                if !window.is_window_active() {
                    // Leave the caret drawn; activation restarts the timer.
                    let caret = this.caret();
                    if caret.epoch == epoch {
                        caret.on = true;
                        caret.stop();
                    }
                    cx.notify();
                    return false;
                }
                let running = this.caret().tick(epoch);
                if running {
                    cx.notify();
                }
                running
            })
            .unwrap_or(false);
        if !keep {
            break;
        }
    })
    .detach();
}

/// End the blink timer (blur).
pub fn stop_blink<T: TextClient>(this: &mut T, cx: &mut Context<T>) {
    this.caret().stop();
    cx.notify();
}

/// Restart the caret when the client's window becomes active while it has
/// focus. Keep the subscription with the client.
pub fn observe_activation<T: TextClient>(window: &mut Window, cx: &mut Context<T>) -> Subscription {
    cx.observe_window_activation(window, |this, window, cx| {
        if window.is_window_active() && this.text_focus().is_focused(window) {
            restart_blink(this, window, cx);
        } else {
            this.caret().on = true;
            this.caret().stop();
            cx.notify();
        }
    })
}

// --- selection -------------------------------------------------------------

/// Select `range`, tell the platform, and show the caret.
pub fn select_range<T: TextClient>(
    this: &mut T,
    range: Range<usize>,
    window: &mut Window,
    cx: &mut Context<T>,
) {
    this.session_mut().select_range(range);
    window.notify_text_selection_changed(this.text_focus());
    restart_blink(this, window, cx);
}

/// After an edit: record it, tell the platform, show the caret.
fn edited<T: TextClient>(
    this: &mut T,
    change: Option<Change>,
    window: &mut Window,
    cx: &mut Context<T>,
) {
    let Some(change) = change else {
        return;
    };
    this.did_change(&change, cx);
    window.notify_text_input_changed(this.text_focus());
    restart_blink(this, window, cx);
}

// --- EntityInputHandler ----------------------------------------------------

pub fn text_for_range<T: TextClient>(
    this: &mut T,
    range_utf16: Range<usize>,
    actual_range: &mut Option<Range<usize>>,
) -> Option<String> {
    let session = this.session();
    let range = session.range_from_utf16(&range_utf16);
    actual_range.replace(session.range_to_utf16(&range));
    let text = &session.text()[range];
    // Secure text is never handed to the platform.
    if this.is_secure() {
        return Some(BULLET.repeat(text.graphemes(true).count()));
    }
    Some(text.to_string())
}

pub fn selected_text_range<T: TextClient>(this: &T) -> Option<UTF16Selection> {
    let session = this.session();
    Some(UTF16Selection {
        range: session.range_to_utf16(&session.selected_range()),
        reversed: session.selection().is_reversed(),
    })
}

pub fn set_selected_text_range<T: TextClient>(
    this: &mut T,
    range_utf16: Range<usize>,
    window: &mut Window,
    cx: &mut Context<T>,
) {
    let range = this.session().range_from_utf16(&range_utf16);
    select_range(this, range, window, cx);
}

pub fn text_length_utf16<T: TextClient>(this: &T) -> Option<usize> {
    Some(this.session().text().encode_utf16().count())
}

pub fn marked_text_range<T: TextClient>(this: &T) -> Option<Range<usize>> {
    let session = this.session();
    session.marked_range().map(|r| session.range_to_utf16(&r))
}

pub fn unmark_text<T: TextClient>(this: &mut T, window: &mut Window, cx: &mut Context<T>) {
    this.session_mut().clear_marked();
    window.notify_text_input_changed(this.text_focus());
    cx.notify();
}

/// The byte range a platform edit targets: the given UTF-16 range, else the
/// composition, else the selection.
fn target<T: TextClient>(this: &T, range_utf16: Option<&Range<usize>>) -> Range<usize> {
    let session = this.session();
    range_utf16
        .map(|r| session.range_from_utf16(r))
        .or(session.marked_range())
        .unwrap_or(session.selected_range())
}

pub fn replace_text_in_range<T: TextClient>(
    this: &mut T,
    range_utf16: Option<Range<usize>>,
    text: &str,
    window: &mut Window,
    cx: &mut Context<T>,
) {
    if this.is_disabled() {
        return;
    }
    let range = target(this, range_utf16.as_ref());
    let change = this.session_mut().replace_range(range, text, false);
    edited(this, change, window, cx);
}

pub fn replace_and_mark_text_in_range<T: TextClient>(
    this: &mut T,
    range_utf16: Option<Range<usize>>,
    text: &str,
    new_selected_utf16: Option<Range<usize>>,
    window: &mut Window,
    cx: &mut Context<T>,
) {
    if this.is_disabled() {
        return;
    }
    let range = target(this, range_utf16.as_ref());
    let session = this.session_mut();
    let change = session.replace_range(range.clone(), text, true);
    if text.is_empty() {
        session.clear_marked();
    } else {
        session.set_marked(Some(range.start..range.start + text.len()));
    }
    if let Some(selected) = new_selected_utf16 {
        let start16 = session.offset_to_utf16(range.start);
        let len16 = text.encode_utf16().count();
        let selected = session.range_from_utf16(
            &(start16 + selected.start.min(len16)..start16 + selected.end.min(len16)),
        );
        session.set_selection(Selection::new(selected.start, selected.end));
    }
    if change.is_none() {
        // An empty composition over an empty range edits nothing, but the
        // marked state may have changed.
        window.notify_text_input_changed(this.text_focus());
        cx.notify();
        return;
    }
    edited(this, change, window, cx);
}

pub fn bounds_for_range<T: TextClient>(
    this: &T,
    range_utf16: Range<usize>,
) -> Option<Bounds<Pixels>> {
    let range = this.session().range_from_utf16(&range_utf16);
    this.range_bounds(range)
}

pub fn character_index_for_point<T: TextClient>(this: &T, point: Point<Pixels>) -> Option<usize> {
    let offset = this.offset_at(point)?;
    Some(this.session().offset_to_utf16(offset))
}

/// Implement `gpui::EntityInputHandler` for a [`TextClient`] by delegating to
/// this module. Every text client uses it; none writes its own.
#[macro_export]
macro_rules! impl_text_input_handler {
    ($client:ty) => {
        impl ::gpui::EntityInputHandler for $client {
            fn text_for_range(
                &mut self,
                range_utf16: ::std::ops::Range<usize>,
                actual_range: &mut Option<::std::ops::Range<usize>>,
                _: &mut ::gpui::Window,
                _: &mut ::gpui::Context<Self>,
            ) -> Option<String> {
                $crate::text_client::text_for_range(self, range_utf16, actual_range)
            }

            fn selected_text_range(
                &mut self,
                _: bool,
                _: &mut ::gpui::Window,
                _: &mut ::gpui::Context<Self>,
            ) -> Option<::gpui::UTF16Selection> {
                $crate::text_client::selected_text_range(self)
            }

            fn set_selected_text_range(
                &mut self,
                range_utf16: ::std::ops::Range<usize>,
                window: &mut ::gpui::Window,
                cx: &mut ::gpui::Context<Self>,
            ) {
                $crate::text_client::set_selected_text_range(self, range_utf16, window, cx);
            }

            fn text_length_utf16(
                &mut self,
                _: &mut ::gpui::Window,
                _: &mut ::gpui::Context<Self>,
            ) -> Option<usize> {
                $crate::text_client::text_length_utf16(self)
            }

            fn marked_text_range(
                &self,
                _: &mut ::gpui::Window,
                _: &mut ::gpui::Context<Self>,
            ) -> Option<::std::ops::Range<usize>> {
                $crate::text_client::marked_text_range(self)
            }

            fn unmark_text(&mut self, window: &mut ::gpui::Window, cx: &mut ::gpui::Context<Self>) {
                $crate::text_client::unmark_text(self, window, cx);
            }

            fn replace_text_in_range(
                &mut self,
                range_utf16: Option<::std::ops::Range<usize>>,
                text: &str,
                window: &mut ::gpui::Window,
                cx: &mut ::gpui::Context<Self>,
            ) {
                $crate::text_client::replace_text_in_range(self, range_utf16, text, window, cx);
            }

            fn replace_and_mark_text_in_range(
                &mut self,
                range_utf16: Option<::std::ops::Range<usize>>,
                text: &str,
                new_selected_utf16: Option<::std::ops::Range<usize>>,
                window: &mut ::gpui::Window,
                cx: &mut ::gpui::Context<Self>,
            ) {
                $crate::text_client::replace_and_mark_text_in_range(
                    self,
                    range_utf16,
                    text,
                    new_selected_utf16,
                    window,
                    cx,
                );
            }

            fn bounds_for_range(
                &mut self,
                range_utf16: ::std::ops::Range<usize>,
                _: ::gpui::Bounds<::gpui::Pixels>,
                _: &mut ::gpui::Window,
                _: &mut ::gpui::Context<Self>,
            ) -> Option<::gpui::Bounds<::gpui::Pixels>> {
                $crate::text_client::bounds_for_range(self, range_utf16)
            }

            fn character_index_for_point(
                &mut self,
                point: ::gpui::Point<::gpui::Pixels>,
                _: &mut ::gpui::Window,
                _: &mut ::gpui::Context<Self>,
            ) -> Option<usize> {
                $crate::text_client::character_index_for_point(self, point)
            }
        }
    };
}

/// The shared conformance suite (#58): the same platform-text scenarios run
/// against every client. Add a line to `conformance!` for each new client.
#[cfg(test)]
mod conformance {
    use std::time::Duration;

    use gpui::{
        div, prelude::*, px, Context, Entity, EntityInputHandler, FocusHandle, Focusable,
        TestAppContext, VisualTestContext, Window,
    };

    use super::TextClient;

    pub(super) struct Host<T> {
        pub client: Entity<T>,
        pub focus: FocusHandle,
    }

    impl<T: Render> Render for Host<T> {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .track_focus(&self.focus)
                .child(div().w(px(240.)).child(self.client.clone()))
        }
    }

    pub(super) fn draw(cx: &mut VisualTestContext) {
        cx.run_until_parked();
        cx.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        cx.run_until_parked();
    }

    pub(super) fn app() -> TestAppContext {
        let app = TestAppContext::single();
        app.update(|cx| {
            crate::theme::install(crate::ThemePreference::default(), cx);
            crate::controls::text_field::bind_keys(cx);
        });
        app
    }

    pub(super) fn focus_client<T: Focusable>(
        host: &Entity<Host<T>>,
        cx: &mut VisualTestContext,
    ) -> Entity<T> {
        let client = host.read_with(cx, |host, _| host.client.clone());
        let focus = client.read_with(cx, |client, cx| client.focus_handle(cx));
        cx.update(|window, cx| window.focus(&focus, cx));
        draw(cx);
        client
    }

    /// Compose, then commit: Undo restores what was there before composing
    /// (nothing), never the raw composition, and Redo brings back the commit.
    pub(super) fn ime_commit_is_one_undo_step<T>(
        client: &Entity<T>,
        history: impl Fn(&T) -> Entity<crate::undo::UndoManager>,
        text: impl Fn(&T) -> String,
        cx: &mut VisualTestContext,
    ) where
        T: TextClient + EntityInputHandler,
    {
        cx.update(|window, cx| {
            client.update(cx, |c, cx| {
                c.replace_and_mark_text_in_range(None, "\u{304b}", None, window, cx);
                c.replace_and_mark_text_in_range(None, "\u{304b}\u{3093}", None, window, cx);
            });
        });
        draw(cx);
        assert_eq!(
            client.read_with(cx, |c, _| c.session().marked_range()),
            Some(0..6)
        );
        cx.update(|window, cx| {
            client.update(cx, |c, cx| {
                c.replace_text_in_range(None, "\u{6f22}", window, cx);
            });
        });
        draw(cx);
        assert_eq!(client.read_with(cx, |c, _| text(c)), "\u{6f22}");
        assert_eq!(
            client.read_with(cx, |c, _| c.session().marked_range()),
            None
        );
        let history = client.read_with(cx, |c, _| history(c));
        cx.update(|_, cx| assert!(history.update(cx, |h, cx| h.undo(cx))));
        draw(cx);
        assert_eq!(
            client.read_with(cx, |c, _| text(c)),
            "",
            "undo removes the committed text, not back to the raw composition"
        );
        cx.update(|_, cx| assert!(history.update(cx, |h, cx| h.redo(cx))));
        draw(cx);
        assert_eq!(client.read_with(cx, |c, _| text(c)), "\u{6f22}");
    }

    /// The caret blinks while the window is active and holds still, with no
    /// timer, while it is inactive; activation resumes it.
    pub(super) fn caret_blink_stops_while_the_window_is_inactive<T: TextClient>(
        client: &Entity<T>,
        cx: &mut VisualTestContext,
    ) {
        let Some(interval) = crate::native::caret_blink() else {
            return; // blinking is off for this user; nothing to stop.
        };
        let caret = |cx: &mut VisualTestContext| client.update(cx, |c, _| *c.caret());
        let before = caret(cx);
        cx.executor()
            .advance_clock(interval + Duration::from_millis(1));
        draw(cx);
        assert_ne!(caret(cx).on, before.on, "the caret blinks while active");

        cx.deactivate_window();
        draw(cx);
        let held = caret(cx);
        assert!(held.on, "an inactive window draws a steady caret");
        for _ in 0..4 {
            cx.executor()
                .advance_clock(interval + Duration::from_millis(1));
            draw(cx);
            assert_eq!(caret(cx), held, "no timer runs while inactive");
        }

        cx.update(|window, _| window.activate_window());
        draw(cx);
        let resumed = caret(cx);
        cx.executor()
            .advance_clock(interval + Duration::from_millis(1));
        draw(cx);
        assert_ne!(caret(cx).on, resumed.on, "activation resumes the blink");
    }

    macro_rules! conformance {
        ($name:ident, $client:ty) => {
            mod $name {
                use super::*;

                fn open(
                    app: &mut TestAppContext,
                ) -> (Entity<Host<$client>>, &mut VisualTestContext) {
                    let (host, cx) = app.add_window_view(|window, cx| {
                        let client = cx.new(|cx| <$client>::new(window, cx));
                        let focus = cx.focus_handle();
                        window.focus(&focus, cx);
                        Host { client, focus }
                    });
                    cx.update(|window, _| window.activate_window());
                    draw(cx);
                    (host, cx)
                }

                #[test]
                fn ime_commit_is_one_undo_step() {
                    let mut app = app();
                    let (host, cx) = open(&mut app);
                    let client = focus_client(&host, cx);
                    super::ime_commit_is_one_undo_step(
                        &client,
                        |c| c.history(),
                        |c| c.text().to_string(),
                        cx,
                    );
                }

                #[test]
                fn caret_blink_stops_while_the_window_is_inactive() {
                    let mut app = app();
                    let (host, cx) = open(&mut app);
                    let client = focus_client(&host, cx);
                    super::caret_blink_stops_while_the_window_is_inactive(&client, cx);
                }
            }
        };
    }

    conformance!(text_field, crate::controls::TextField);
    conformance!(text_area, crate::controls::TextArea);

    /// Secure text is never handed to the platform: queries see bullets.
    #[test]
    fn a_secure_field_answers_platform_queries_with_bullets() {
        let mut app = app();
        let (host, cx) = app.add_window_view(|window, cx| {
            let client = cx.new(|cx| crate::controls::TextField::new(window, cx));
            let focus = cx.focus_handle();
            window.focus(&focus, cx);
            Host { client, focus }
        });
        draw(cx);
        let client = focus_client(&host, cx);
        cx.update(|_, cx| client.update(cx, |c, cx| c.set_masked(true, cx)));
        cx.simulate_input("p\u{e9}w");
        draw(cx);
        let text = cx.update(|window, cx| {
            client.update(cx, |c, cx| {
                let mut actual = None;
                c.text_for_range(0..3, &mut actual, window, cx)
            })
        });
        assert_eq!(text.as_deref(), Some("\u{2022}\u{2022}\u{2022}"));
    }
}
