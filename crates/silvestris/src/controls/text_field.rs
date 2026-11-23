//! Single-line text field (HIG p. 322-325; handbook 12 "Text/IME").
//!
//! Input plumbing follows GPUI's examples/input.rs (Apache-2.0): an entity
//! implementing `EntityInputHandler`, registered each frame with
//! `Window::handle_input`, so typing, IME composition (marked text) and
//! accessibility text queries all go through one path. On top of that:
//!
//! - Aqua/Geist bezel, text inset and caret geometry from `Metrics`
//!   (M: field.geometry) and colours from `Controls::field`.
//! - Horizontal scrolling that keeps the caret visible.
//! - Word movement and deletion, Home/End, select all, clipboard, and
//!   platform accelerators: Cmd on macOS, Ctrl elsewhere (handbook 12).
//! - Password masking: one bullet per grapheme; copy and cut are refused.
//! - A blinking caret at the OS rate (`native::caret_blink`), hidden while the
//!   window is inactive.
//! - `TextFieldEvent::Changed` on every edit, `Submitted` on Return and
//!   `EndedEditing` when focus leaves.
//!
//! The focus ring is drawn whenever the field has focus, as 10.6 does for
//! text fields (M: field.geometry, focused Go to Folder field).

use std::ops::Range;

use gpui::{
    actions, canvas, div, point, prelude::*, px, size, App, Bounds, ClipboardItem, Context,
    CursorStyle, Element, ElementId, ElementInputHandler, Entity, EntityInputHandler, EventEmitter,
    FocusHandle, Focusable, GlobalElementId, KeyBinding, LayoutId, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point, ShapedLine, SharedString, Style,
    Subscription, TextRun, UTF16Selection, UnderlineStyle, Window,
};
use unicode_segmentation::UnicodeSegmentation;

use super::ControlSize;
use crate::convert::hsla;
use crate::paint;
use crate::theme::ActiveTheme;

actions!(
    silvestris_text_field,
    [
        Backspace,
        Delete,
        DeleteWordLeft,
        DeleteWordRight,
        Left,
        Right,
        SelectLeft,
        SelectRight,
        WordLeft,
        WordRight,
        SelectWordLeft,
        SelectWordRight,
        Home,
        End,
        SelectHome,
        SelectEnd,
        SelectAll,
        Copy,
        Cut,
        Paste,
        Submit,
    ]
);

/// Key context of a focused text field.
pub const KEY_CONTEXT: &str = "SilvestrisTextField";

/// Bind text-field keys. Called by `silvestris::init`.
pub fn bind_keys(cx: &mut App) {
    // KeyBinding::new takes &'static str (gpui keymap/binding.rs:35), so the
    // platform variants are literals: Cmd primary and Option word movement on
    // macOS, Ctrl for both elsewhere (handbook 12, shortcut policy).
    macro_rules! k {
        ($mac:literal, $other:literal) => {
            if cfg!(target_os = "macos") {
                $mac
            } else {
                $other
            }
        };
    }
    let ctx = Some(KEY_CONTEXT);
    let mut bindings = vec![
        KeyBinding::new("backspace", Backspace, ctx),
        KeyBinding::new("delete", Delete, ctx),
        KeyBinding::new(k!("alt-backspace", "ctrl-backspace"), DeleteWordLeft, ctx),
        KeyBinding::new(k!("alt-delete", "ctrl-delete"), DeleteWordRight, ctx),
        KeyBinding::new("left", Left, ctx),
        KeyBinding::new("right", Right, ctx),
        KeyBinding::new("shift-left", SelectLeft, ctx),
        KeyBinding::new("shift-right", SelectRight, ctx),
        KeyBinding::new(k!("alt-left", "ctrl-left"), WordLeft, ctx),
        KeyBinding::new(k!("alt-right", "ctrl-right"), WordRight, ctx),
        KeyBinding::new(k!("alt-shift-left", "ctrl-shift-left"), SelectWordLeft, ctx),
        KeyBinding::new(
            k!("alt-shift-right", "ctrl-shift-right"),
            SelectWordRight,
            ctx,
        ),
        KeyBinding::new("home", Home, ctx),
        KeyBinding::new("end", End, ctx),
        KeyBinding::new("shift-home", SelectHome, ctx),
        KeyBinding::new("shift-end", SelectEnd, ctx),
        KeyBinding::new(k!("cmd-a", "ctrl-a"), SelectAll, ctx),
        KeyBinding::new(k!("cmd-c", "ctrl-c"), Copy, ctx),
        KeyBinding::new(k!("cmd-x", "ctrl-x"), Cut, ctx),
        KeyBinding::new(k!("cmd-v", "ctrl-v"), Paste, ctx),
        KeyBinding::new("enter", Submit, ctx),
    ];
    if cfg!(target_os = "macos") {
        // Line start/end on Cmd-arrows, the Mac convention.
        bindings.extend([
            KeyBinding::new("cmd-left", Home, ctx),
            KeyBinding::new("cmd-right", End, ctx),
            KeyBinding::new("cmd-shift-left", SelectHome, ctx),
            KeyBinding::new("cmd-shift-right", SelectEnd, ctx),
        ]);
    } else {
        // Windows/Linux clipboard aliases.
        bindings.extend([
            KeyBinding::new("shift-delete", Cut, ctx),
            KeyBinding::new("ctrl-insert", Copy, ctx),
            KeyBinding::new("shift-insert", Paste, ctx),
        ]);
    }
    cx.bind_keys(bindings);
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextFieldEvent {
    Changed,
    Submitted,
    /// Focus left the field; consumers commit pending values here as well
    /// as on Submitted (a value is accepted on Return or leaving the field).
    EndedEditing,
}

/// Bullet used for masked text (U+2022).
const BULLET: &str = "\u{2022}";

pub struct TextField {
    focus: FocusHandle,
    content: String,
    placeholder: SharedString,
    label: SharedString,
    selected: Range<usize>,
    reversed: bool,
    marked: Option<Range<usize>>,
    size: ControlSize,
    masked: bool,
    search: bool,
    disabled: bool,
    scroll_x: Pixels,
    last_layout: Option<ShapedLine>,
    /// Where the line was painted: origin of the unscrolled text and the
    /// visible text box.
    last_origin: Option<Point<Pixels>>,
    last_bounds: Option<Bounds<Pixels>>,
    selecting: bool,
    caret_on: bool,
    blink_epoch: u64,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<TextFieldEvent> for TextField {}

impl Focusable for TextField {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl TextField {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle().tab_stop(true);
        let subscriptions = vec![
            cx.on_focus(&focus, window, |this, window, cx| {
                this.restart_blink(window, cx)
            }),
            cx.on_blur(&focus, window, |this, _, cx| {
                this.blink_epoch += 1;
                this.selecting = false;
                cx.emit(TextFieldEvent::EndedEditing);
                cx.notify();
            }),
        ];
        TextField {
            focus,
            content: String::new(),
            placeholder: SharedString::default(),
            label: SharedString::default(),
            selected: 0..0,
            reversed: false,
            marked: None,
            size: ControlSize::Regular,
            masked: false,
            search: false,
            disabled: false,
            scroll_x: px(0.),
            last_layout: None,
            last_origin: None,
            last_bounds: None,
            selecting: false,
            caret_on: true,
            blink_epoch: 0,
            _subscriptions: subscriptions,
        }
    }

    pub fn text(&self) -> &str {
        &self.content
    }

    pub fn set_text(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        self.content = text.into();
        let end = self.content.len();
        self.selected = end..end;
        self.reversed = false;
        self.marked = None;
        // No Changed event: AppKit sends text-change notifications only for
        // user edits (NSControlTextDidChangeNotification), not for setters.
        cx.notify();
    }

    pub(super) fn replace_from_control(
        &mut self,
        text: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_text(text, cx);
        if self.focus.is_focused(window) {
            self.restart_blink(window, cx);
        }
        cx.emit(TextFieldEvent::Changed);
    }

    pub fn set_placeholder(
        &mut self,
        placeholder: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.placeholder = placeholder.into();
        cx.notify();
    }

    /// Accessible name (the field's label text).
    pub fn set_label(&mut self, label: impl Into<SharedString>, cx: &mut Context<Self>) {
        let label = label.into();
        if self.label != label {
            self.label = label;
            cx.notify();
        }
    }

    pub fn set_size(&mut self, size: ControlSize, cx: &mut Context<Self>) {
        if self.size != size {
            self.size = size;
            cx.notify();
        }
    }

    /// Secure entry: bullets on screen, no copy or cut.
    pub fn set_masked(&mut self, masked: bool, cx: &mut Context<Self>) {
        self.masked = masked;
        cx.notify();
    }

    /// A rounded search field with an inline clear action and the same
    /// editing, IME and clipboard handling as a regular text field (HIG p. 325).
    pub fn set_search(&mut self, search: bool, cx: &mut Context<Self>) {
        self.search = search;
        cx.notify();
    }

    fn field_tokens(
        &self,
        theme: &silvestris_core::tokens::Theme,
    ) -> silvestris_core::tokens::Field {
        if self.search && !self.masked {
            theme.controls.search
        } else {
            theme.controls.field
        }
    }

    fn clear_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.set_text("", cx);
        self.restart_blink(window, cx);
        cx.emit(TextFieldEvent::Changed);
    }

    pub fn set_disabled(&mut self, disabled: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled == disabled {
            return;
        }
        self.disabled = disabled;
        if disabled && self.focus.is_focused(window) {
            window.blur(cx);
        }
        cx.notify();
    }

    // --- caret blink -------------------------------------------------------

    fn restart_blink(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.caret_on = true;
        self.blink_epoch += 1;
        cx.notify();
        let Some(interval) = crate::native::caret_blink() else {
            return;
        };
        let epoch = self.blink_epoch;
        cx.spawn_in(window, async move |this, cx| loop {
            cx.background_executor().timer(interval).await;
            let keep = this
                .update(cx, |this, cx| {
                    if this.blink_epoch != epoch {
                        return false;
                    }
                    this.caret_on = !this.caret_on;
                    cx.notify();
                    true
                })
                .unwrap_or(false);
            if !keep {
                break;
            }
        })
        .detach();
    }

    // --- offsets -----------------------------------------------------------

    fn cursor(&self) -> usize {
        if self.reversed {
            self.selected.start
        } else {
            self.selected.end
        }
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .rev()
            .find_map(|(i, _)| (i < offset).then_some(i))
            .unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .find_map(|(i, _)| (i > offset).then_some(i))
            .unwrap_or(self.content.len())
    }

    /// Start of the word before `offset` (words contain an alphanumeric).
    /// Masked fields treat the whole text as one word.
    fn previous_word(&self, offset: usize) -> usize {
        if self.masked {
            return 0;
        }
        self.content
            .split_word_bound_indices()
            .rev()
            .find_map(|(i, w)| (i < offset && w.chars().any(char::is_alphanumeric)).then_some(i))
            .unwrap_or(0)
    }

    fn next_word(&self, offset: usize) -> usize {
        if self.masked {
            return self.content.len();
        }
        self.content
            .split_word_bound_indices()
            .find_map(|(i, w)| {
                let end = i + w.len();
                (end > offset && w.chars().any(char::is_alphanumeric)).then_some(end)
            })
            .unwrap_or(self.content.len())
    }

    fn word_at(&self, offset: usize) -> Range<usize> {
        if self.masked {
            return 0..self.content.len();
        }
        self.content
            .split_word_bound_indices()
            .map(|(i, w)| i..i + w.len())
            .find(|r| r.contains(&offset) || r.end == offset && offset == self.content.len())
            .unwrap_or(offset..offset)
    }

    /// Byte offset in the displayed string for a byte offset in `content`.
    fn display_offset(&self, offset: usize) -> usize {
        if !self.masked {
            return offset;
        }
        self.content
            .grapheme_indices(true)
            .take_while(|(i, _)| *i < offset)
            .count()
            * BULLET.len()
    }

    /// Inverse of `display_offset`.
    fn content_offset(&self, display: usize) -> usize {
        if !self.masked {
            return display;
        }
        let n = display / BULLET.len();
        self.content
            .grapheme_indices(true)
            .nth(n)
            .map(|(i, _)| i)
            .unwrap_or(self.content.len())
    }

    fn display_text(&self) -> String {
        if self.masked {
            BULLET.repeat(self.content.graphemes(true).count())
        } else {
            self.content.clone()
        }
    }

    fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8 = 0;
        let mut utf16 = 0;
        for ch in self.content.chars() {
            if utf16 >= offset {
                break;
            }
            utf16 += ch.len_utf16();
            utf8 += ch.len_utf8();
        }
        utf8
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf16 = 0;
        let mut utf8 = 0;
        for ch in self.content.chars() {
            if utf8 >= offset {
                break;
            }
            utf8 += ch.len_utf8();
            utf16 += ch.len_utf16();
        }
        utf16
    }

    fn range_to_utf16(&self, r: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(r.start)..self.offset_to_utf16(r.end)
    }

    fn range_from_utf16(&self, r: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(r.start)..self.offset_from_utf16(r.end)
    }

    fn index_for_position(&self, position: Point<Pixels>) -> usize {
        let (Some(origin), Some(line)) = (self.last_origin, self.last_layout.as_ref()) else {
            return 0;
        };
        // An empty field's shaped line is the placeholder, whose indices are
        // not content offsets.
        if self.content.is_empty() {
            return 0;
        }
        self.content_offset(line.closest_index_for_x(position.x - origin.x))
            .min(self.content.len())
    }

    // --- selection ---------------------------------------------------------

    fn move_to(&mut self, offset: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.selected = offset..offset;
        self.reversed = false;
        window.notify_text_selection_changed(&self.focus);
        self.restart_blink(window, cx);
    }

    fn select_to(&mut self, offset: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.reversed {
            self.selected.start = offset
        } else {
            self.selected.end = offset
        }
        if self.selected.end < self.selected.start {
            self.reversed = !self.reversed;
            self.selected = self.selected.end..self.selected.start;
        }
        window.notify_text_selection_changed(&self.focus);
        self.restart_blink(window, cx);
    }

    fn select_range(&mut self, range: Range<usize>, window: &mut Window, cx: &mut Context<Self>) {
        self.selected = range;
        self.reversed = false;
        window.notify_text_selection_changed(&self.focus);
        self.restart_blink(window, cx);
    }

    fn replace_selection(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.replace_text_in_range(None, text, window, cx);
    }

    // --- actions -----------------------------------------------------------

    fn left(&mut self, _: &Left, window: &mut Window, cx: &mut Context<Self>) {
        let to = if self.selected.is_empty() {
            self.previous_boundary(self.cursor())
        } else {
            self.selected.start
        };
        self.move_to(to, window, cx);
    }

    fn right(&mut self, _: &Right, window: &mut Window, cx: &mut Context<Self>) {
        let to = if self.selected.is_empty() {
            self.next_boundary(self.cursor())
        } else {
            self.selected.end
        };
        self.move_to(to, window, cx);
    }

    fn select_left(&mut self, _: &SelectLeft, window: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor()), window, cx);
    }

    fn select_right(&mut self, _: &SelectRight, window: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor()), window, cx);
    }

    fn word_left(&mut self, _: &WordLeft, window: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.previous_word(self.cursor()), window, cx);
    }

    fn word_right(&mut self, _: &WordRight, window: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.next_word(self.cursor()), window, cx);
    }

    fn select_word_left(
        &mut self,
        _: &SelectWordLeft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_to(self.previous_word(self.cursor()), window, cx);
    }

    fn select_word_right(
        &mut self,
        _: &SelectWordRight,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_to(self.next_word(self.cursor()), window, cx);
    }

    fn home(&mut self, _: &Home, window: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, window, cx);
    }

    fn end(&mut self, _: &End, window: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.content.len(), window, cx);
    }

    fn select_home(&mut self, _: &SelectHome, window: &mut Window, cx: &mut Context<Self>) {
        self.select_to(0, window, cx);
    }

    fn select_end(&mut self, _: &SelectEnd, window: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.content.len(), window, cx);
    }

    fn select_all(&mut self, _: &SelectAll, window: &mut Window, cx: &mut Context<Self>) {
        self.select_range(0..self.content.len(), window, cx);
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected.is_empty() {
            let prev = self.previous_boundary(self.cursor());
            if prev == self.cursor() {
                window.play_system_bell();
                return;
            }
            self.select_to(prev, window, cx);
        }
        self.replace_selection("", window, cx);
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected.is_empty() {
            let next = self.next_boundary(self.cursor());
            if next == self.cursor() {
                window.play_system_bell();
                return;
            }
            self.select_to(next, window, cx);
        }
        self.replace_selection("", window, cx);
    }

    fn delete_word_left(
        &mut self,
        _: &DeleteWordLeft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selected.is_empty() {
            self.select_to(self.previous_word(self.cursor()), window, cx);
        }
        self.replace_selection("", window, cx);
    }

    fn delete_word_right(
        &mut self,
        _: &DeleteWordRight,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selected.is_empty() {
            self.select_to(self.next_word(self.cursor()), window, cx);
        }
        self.replace_selection("", window, cx);
    }

    fn copy(&mut self, _: &Copy, window: &mut Window, cx: &mut Context<Self>) {
        if self.masked {
            window.play_system_bell();
        } else if !self.selected.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected.clone()].to_string(),
            ));
        }
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if self.masked {
            window.play_system_bell();
        } else if !self.selected.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected.clone()].to_string(),
            ));
            self.replace_selection("", window, cx);
        }
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            // Single line: newlines become spaces.
            let text = text.replace("\r\n", " ").replace(['\n', '\r'], " ");
            self.replace_selection(&text, window, cx);
        }
    }

    fn submit(&mut self, _: &Submit, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(TextFieldEvent::Submitted);
    }

    // --- mouse -------------------------------------------------------------

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }
        window.focus(&self.focus, cx);
        let index = self.index_for_position(event.position);
        match event.click_count {
            2 => self.select_range(self.word_at(index), window, cx),
            n if n >= 3 => self.select_range(0..self.content.len(), window, cx),
            _ => {
                self.selecting = true;
                if event.modifiers.shift {
                    self.select_to(index, window, cx)
                } else {
                    self.move_to(index, window, cx)
                }
            }
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.selecting = false;
    }

    fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selecting {
            self.select_to(self.index_for_position(event.position), window, cx);
        }
    }
}

impl EntityInputHandler for TextField {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        // Secure text is never handed to the platform.
        if self.masked {
            return Some(BULLET.repeat(self.content[range].graphemes(true).count()));
        }
        Some(self.content[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected),
            reversed: self.reversed,
        })
    }

    fn set_selected_text_range(
        &mut self,
        range_utf16: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = self.range_from_utf16(&range_utf16);
        self.select_range(range, window, cx);
    }

    fn text_length_utf16(&mut self, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        Some(self.content.encode_utf16().count())
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked.as_ref().map(|r| self.range_to_utf16(r))
    }

    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.marked = None;
        window.notify_text_input_changed(&self.focus);
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }
        let range = range_utf16
            .as_ref()
            .map(|r| self.range_from_utf16(r))
            .or(self.marked.clone())
            .unwrap_or(self.selected.clone());
        self.content.replace_range(range.clone(), new_text);
        let end = range.start + new_text.len();
        self.selected = end..end;
        self.reversed = false;
        self.marked = None;
        window.notify_text_input_changed(&self.focus);
        cx.emit(TextFieldEvent::Changed);
        self.restart_blink(window, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }
        let range = range_utf16
            .as_ref()
            .map(|r| self.range_from_utf16(r))
            .or(self.marked.clone())
            .unwrap_or(self.selected.clone());
        self.content.replace_range(range.clone(), new_text);
        self.marked = (!new_text.is_empty()).then(|| range.start..range.start + new_text.len());
        let start16 = self.offset_to_utf16(range.start);
        let len16 = new_text.encode_utf16().count();
        self.selected = new_selected_range_utf16
            .map(|s| {
                self.range_from_utf16(&(start16 + s.start.min(len16)..start16 + s.end.min(len16)))
            })
            .unwrap_or_else(|| range.start + new_text.len()..range.start + new_text.len());
        self.reversed = false;
        window.notify_text_input_changed(&self.focus);
        cx.emit(TextFieldEvent::Changed);
        self.restart_blink(window, cx);
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        _element_bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let (line, origin, bounds) = (
            self.last_layout.as_ref()?,
            self.last_origin?,
            self.last_bounds?,
        );
        let range = self.range_from_utf16(&range_utf16);
        let x = |o: usize| origin.x + line.x_for_index(self.display_offset(o));
        Some(Bounds::from_corners(
            point(x(range.start), bounds.top()),
            point(x(range.end), bounds.bottom()),
        ))
    }

    fn character_index_for_point(
        &mut self,
        p: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let bounds = self.last_bounds?;
        if !bounds.contains(&p) {
            return None;
        }
        Some(self.offset_to_utf16(self.index_for_position(p)))
    }
}

impl Render for TextField {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let m = &theme.metrics;
        let field = self.field_tokens(&theme);
        let height = match self.size {
            ControlSize::Regular => m.field_height,
            ControlSize::Small | ControlSize::Mini => m.field_small_height,
        };
        let focused = self.focus.is_focused(window);
        let enabled = !self.disabled;
        let ring_color = theme.colors.focus_ring;
        let ring = m.focus_ring;
        let search = self.search && !self.masked;
        let clearable = search && enabled && !self.content.is_empty();
        let radius = if search { height / 2.0 } else { 0.0 };
        let role = match (self.masked, search) {
            (true, _) => gpui::Role::PasswordInput,
            (_, true) => gpui::Role::SearchInput,
            _ => gpui::Role::TextInput,
        };

        let selector = format!("text-field-{}", cx.entity_id());
        div()
            .id(("silvestris-text-field", cx.entity_id()))
            .debug_selector(move || selector)
            .key_context(KEY_CONTEXT)
            .role(role)
            .aria_label(self.label.clone())
            .aria_placeholder(self.placeholder.clone())
            .when(!self.masked, |el| {
                el.aria_value(SharedString::from(self.content.clone()))
            })
            .relative()
            .w_full()
            .h(px(height))
            .font_family(theme.type_.ui)
            .text_size(px(self.size.label_size(&theme)))
            .when(enabled, |el| {
                el.track_focus(&self.focus)
                    .cursor(CursorStyle::IBeam)
                    .on_action(cx.listener(Self::backspace))
                    .on_action(cx.listener(Self::delete))
                    .on_action(cx.listener(Self::delete_word_left))
                    .on_action(cx.listener(Self::delete_word_right))
                    .on_action(cx.listener(Self::left))
                    .on_action(cx.listener(Self::right))
                    .on_action(cx.listener(Self::select_left))
                    .on_action(cx.listener(Self::select_right))
                    .on_action(cx.listener(Self::word_left))
                    .on_action(cx.listener(Self::word_right))
                    .on_action(cx.listener(Self::select_word_left))
                    .on_action(cx.listener(Self::select_word_right))
                    .on_action(cx.listener(Self::home))
                    .on_action(cx.listener(Self::end))
                    .on_action(cx.listener(Self::select_home))
                    .on_action(cx.listener(Self::select_end))
                    .on_action(cx.listener(Self::select_all))
                    .on_action(cx.listener(Self::copy))
                    .on_action(cx.listener(Self::cut))
                    .on_action(cx.listener(Self::paste))
                    .on_action(cx.listener(Self::submit))
                    .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
                    .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
                    .on_mouse_move(cx.listener(Self::on_mouse_move))
            })
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, _| {
                        paint::field(window, bounds, radius, &field, enabled);
                    },
                )
                .absolute()
                .inset_0(),
            )
            .child(TextElement { field: cx.entity() })
            .when(search, |el| {
                el.child(
                    canvas(
                        |_, _, _| (),
                        move |bounds, _, window, _| {
                            let center = bounds.center();
                            let lens =
                                Bounds::new(center - point(px(3.5), px(3.5)), size(px(6.), px(6.)));
                            let hairline = crate::convert::hairline(window);
                            window.paint_quad(gpui::quad(
                                lens,
                                px(3.),
                                gpui::transparent_black(),
                                hairline,
                                hsla(field.placeholder),
                                gpui::BorderStyle::Solid,
                            ));
                            let mut handle = gpui::PathBuilder::stroke(hairline);
                            handle.move_to(center + point(px(1.5), px(1.5)));
                            handle.line_to(center + point(px(4.), px(4.)));
                            if let Ok(path) = handle.build() {
                                window.paint_path(path, hsla(field.placeholder));
                            }
                        },
                    )
                    .absolute()
                    .left(px(4.))
                    .top_0()
                    .bottom_0()
                    .w(px(14.)),
                )
            })
            .when(clearable, |el| {
                el.on_key_down(
                    cx.listener(|field, event: &gpui::KeyDownEvent, window, cx| {
                        if event.keystroke.key == "escape" && !event.keystroke.modifiers.modified()
                        {
                            cx.stop_propagation();
                            field.clear_search(window, cx);
                        }
                    }),
                )
                .child(
                    div()
                        .id("clear-search")
                        .role(gpui::Role::Button)
                        .aria_label("Clear Search")
                        .absolute()
                        .right(px(3.))
                        .top_0()
                        .bottom_0()
                        .w(px(16.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(hsla(field.placeholder))
                        .cursor_pointer()
                        .occlude()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|field, _, window, cx| {
                                window.focus(&field.focus, cx);
                                cx.stop_propagation();
                            }),
                        )
                        .on_click(
                            cx.listener(|field, _, window, cx| field.clear_search(window, cx)),
                        )
                        .child("×"),
                )
            })
            .when(focused, |el| {
                el.child(
                    canvas(
                        |_, _, _| (),
                        move |bounds, _, window, _| {
                            paint::focus_ring(window, bounds, radius, ring_color, ring);
                        },
                    )
                    .absolute()
                    .inset_0(),
                )
            })
    }
}

/// Paints the text, selection and caret inside the field's border, and
/// registers the field as the window's input handler while focused.
struct TextElement {
    field: Entity<TextField>,
}

struct Prepaint {
    line: ShapedLine,
    origin: Point<Pixels>,
    clip: Bounds<Pixels>,
    scroll_x: Pixels,
    selection: Option<PaintQuad>,
    caret: Option<PaintQuad>,
}

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = Prepaint;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = Style {
            position: gpui::Position::Absolute,
            ..Default::default()
        };
        style.inset.top = px(0.).into();
        style.inset.left = px(0.).into();
        style.size.width = gpui::relative(1.).into();
        style.size.height = gpui::relative(1.).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Prepaint {
        let theme = cx.theme().clone();
        let m = &theme.metrics;
        let field = self.field.read(cx);
        let f = field.field_tokens(&theme);
        let style = window.text_style();
        let focused = field.focus.is_focused(window);

        let display = field.display_text();
        let (text, color) = if display.is_empty() {
            (field.placeholder.to_string(), f.placeholder)
        } else if field.disabled {
            (display, theme.colors.text_disabled)
        } else {
            (display, f.text)
        };
        let run = TextRun {
            len: text.len(),
            font: style.font(),
            color: hsla(color),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let runs = match field.marked.as_ref().filter(|_| !field.content.is_empty()) {
            Some(marked) => {
                let (s, e) = (
                    field.display_offset(marked.start),
                    field.display_offset(marked.end),
                );
                [
                    TextRun {
                        len: s,
                        ..run.clone()
                    },
                    TextRun {
                        len: e - s,
                        underline: Some(UnderlineStyle {
                            color: Some(run.color),
                            thickness: crate::convert::hairline(window),
                            wavy: false,
                        }),
                        ..run.clone()
                    },
                    TextRun {
                        len: text.len() - e,
                        ..run
                    },
                ]
                .into_iter()
                .filter(|r| r.len > 0)
                .collect()
            }
            None => vec![run],
        };
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line = window
            .text_system()
            .shape_line(text.into(), font_size, &runs, None);

        // Text box: inside the border by the measured inset.
        let search = field.search && !field.masked;
        let (left, right) = if search {
            (px(21.), px(21.))
        } else {
            (px(m.field_text_inset), px(m.field_text_inset))
        };
        let clip_inset = if search { left - px(1.) } else { px(1.) };
        let clip_left = bounds.left() + clip_inset;
        let clip = Bounds::from_corners(
            point(clip_left, bounds.top() + px(1.)),
            point(
                (bounds.right() - clip_inset).max(clip_left),
                bounds.bottom() - px(1.),
            ),
        );
        let visible = (bounds.size.width - left - right).max(px(0.));

        // Keep the caret inside the visible box.
        let caret_x = if field.content.is_empty() {
            px(0.)
        } else {
            line.x_for_index(field.display_offset(field.cursor()))
        };
        let mut scroll_x = field.scroll_x;
        if line.width() <= visible || field.content.is_empty() {
            scroll_x = px(0.);
        } else {
            if caret_x - scroll_x > visible {
                scroll_x = caret_x - visible;
            }
            if caret_x < scroll_x {
                scroll_x = caret_x;
            }
            scroll_x = scroll_x.min(line.width() - visible).max(px(0.));
        }

        let line_height = window.line_height();
        let origin = point(
            bounds.left() + left - scroll_x,
            bounds.top() + ((bounds.size.height - line_height) / 2.).round(),
        );
        let caret_top = bounds.top() + px(m.caret_inset_top);
        let caret_bottom = bounds.bottom() - px(m.caret_inset_bottom);

        let selection =
            (focused && !field.selected.is_empty() && !field.content.is_empty()).then(|| {
                let x0 = origin.x + line.x_for_index(field.display_offset(field.selected.start));
                let x1 = origin.x + line.x_for_index(field.display_offset(field.selected.end));
                gpui::fill(
                    Bounds::from_corners(point(x0, caret_top), point(x1, caret_bottom)),
                    hsla(f.selection),
                )
            });
        let caret =
            (focused && field.selected.is_empty() && field.caret_on && window.is_window_active())
                .then(|| {
                    gpui::fill(
                        Bounds::new(
                            point(origin.x + caret_x, caret_top),
                            size(px(m.caret_width), caret_bottom - caret_top),
                        ),
                        hsla(f.caret),
                    )
                });
        Prepaint {
            line,
            origin,
            clip,
            scroll_x,
            selection,
            caret,
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        prepaint: &mut Prepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus = self.field.read(cx).focus.clone();
        window.handle_input(
            &focus,
            ElementInputHandler::new(bounds, self.field.clone()),
            cx,
        );
        let line_height = window.line_height();
        window.with_content_mask(
            Some(gpui::ContentMask {
                bounds: prepaint.clip,
            }),
            |window| {
                if let Some(selection) = prepaint.selection.take() {
                    window.paint_quad(selection);
                }
                prepaint
                    .line
                    .paint(
                        prepaint.origin,
                        line_height,
                        gpui::TextAlign::Left,
                        None,
                        window,
                        cx,
                    )
                    .ok();
                if let Some(caret) = prepaint.caret.take() {
                    window.paint_quad(caret);
                }
            },
        );
        let (line, origin, scroll_x, clip) = (
            prepaint.line.clone(),
            prepaint.origin,
            prepaint.scroll_x,
            prepaint.clip,
        );
        self.field.update(cx, |field, _| {
            field.last_layout = Some(line);
            field.last_origin = Some(origin);
            field.last_bounds = Some(clip);
            field.scroll_x = scroll_x;
        });
    }
}
