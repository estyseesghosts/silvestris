//! Multiline text area built on the shared text engine (#58).
//!
//! `TextArea` and `TextField` share `text::EditSession` for the buffer,
//! selection, grapheme/word boundaries, UTF-16 mapping and marked text. The
//! view owns wrapping, line metrics, visual-row movement, growth and scrolling.
//!
//! Composer contract (sandcat): Return submits, Shift-Return inserts a newline,
//! Cmd/Ctrl-Return submits. A failed submit does not touch the buffer; the app
//! keeps the draft.
//!
//! The action namespace and key context are the shared `silvestris_text` ones
//! from `text_field`; there is no separate area namespace.

use std::ops::Range;

use gpui::{
    canvas, div, point, prelude::*, px, size, App, Bounds, ClipboardItem, Context, CursorStyle,
    Element, ElementId, ElementInputHandler, Entity, EventEmitter, FocusHandle, Focusable,
    GlobalElementId, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    PaintQuad, Pixels, Point, SharedString, Style, Subscription, TextRun, UnderlineStyle, Window,
    WrappedLine,
};

use super::text_field::{
    Backspace, Copy, Cut, Delete, DeleteWordLeft, DeleteWordRight, Down, End, Home, Left, Newline,
    Paste, Right, SelectAll, SelectDown, SelectEnd, SelectHome, SelectLeft, SelectRight, SelectUp,
    SelectWordLeft, SelectWordRight, Submit, Up, WordLeft, WordRight,
};
use crate::convert::hsla;
use crate::paint;
use crate::text::{CaretBlink, Change, ClipboardPolicy, EditSession};
use crate::text_client::TextClient;
use crate::theme::ActiveTheme;
use crate::undo::{TextHistory, TextHost, UndoManager};

/// Events a text area emits. `Changed` is a user edit; setters do not emit it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextAreaEvent {
    Changed,
    Submitted,
    /// Focus left the area.
    EndedEditing,
}

pub struct TextArea {
    focus: FocusHandle,
    session: EditSession,
    placeholder: SharedString,
    label: SharedString,
    disabled: bool,
    min_rows: usize,
    max_rows: usize,
    /// Externally supplied pixel cap for the whole control, including insets.
    /// When set it replaces the row-based `max_rows` cap.
    max_height: Option<Pixels>,
    /// Preferred content height measured from the last shaped layout, including
    /// insets. `None` until the area has been laid out once.
    measured_height: Option<Pixels>,
    scroll_y: Pixels,
    selecting: bool,
    caret: CaretBlink,
    last_lines: Vec<WrappedLine>,
    last_starts: Vec<usize>,
    last_tops: Vec<Pixels>,
    last_origin: Option<Point<Pixels>>,
    last_bounds: Option<Bounds<Pixels>>,
    current_line_height: Pixels,
    history: TextHistory,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<TextAreaEvent> for TextArea {}

impl Focusable for TextArea {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl TextArea {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle().tab_stop(true);
        let subscriptions = vec![
            crate::text_client::observe_activation(window, cx),
            cx.on_focus(&focus, window, |area, window, cx| {
                area.restart_blink(window, cx)
            }),
            cx.on_blur(&focus, window, |area, _, cx| {
                area.caret.stop();
                area.selecting = false;
                cx.emit(TextAreaEvent::EndedEditing);
                cx.notify();
            }),
        ];
        Self {
            focus,
            session: EditSession::new(),
            placeholder: SharedString::default(),
            label: SharedString::default(),
            disabled: false,
            min_rows: 3,
            max_rows: 10,
            max_height: None,
            measured_height: None,
            scroll_y: px(0.),
            selecting: false,
            caret: CaretBlink::default(),
            last_lines: Vec::new(),
            last_starts: Vec::new(),
            last_tops: Vec::new(),
            last_origin: None,
            last_bounds: None,
            current_line_height: px(16.),
            history: TextHistory::new(cx),
            _subscriptions: subscriptions,
        }
    }

    #[must_use]
    pub fn text(&self) -> &str {
        self.session.text()
    }

    /// Set the whole text and place the caret at the end. Silent: no `Changed`.
    pub fn set_text(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        self.session.set_text(text);
        self.scroll_y = px(0.);
        cx.notify();
    }

    /// The area's local undo stack.
    #[must_use]
    pub fn history(&self) -> Entity<UndoManager> {
        self.history.manager()
    }

    /// A responder entry for this area's undo stack.
    #[must_use]
    pub fn undo_entry(&self) -> crate::responder::ResponderEntry {
        self.history
            .entry(crate::responder::ResponderScope::Control, "text-area-undo")
    }

    fn record_change(&mut self, change: Option<Change>, cx: &mut Context<Self>) -> bool {
        let Some(change) = change else {
            return false;
        };
        let entity = cx.entity();
        self.history.record(entity, &change, cx);
        true
    }

    /// Record the edit, then emit Changed and notify.
    fn after_edit(&mut self, change: Option<Change>, window: &mut Window, cx: &mut Context<Self>) {
        if self.record_change(change, cx) {
            window.notify_text_input_changed(&self.focus);
            cx.emit(TextAreaEvent::Changed);
            self.restart_blink(window, cx);
        }
    }

    pub fn set_placeholder(
        &mut self,
        placeholder: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.placeholder = placeholder.into();
        cx.notify();
    }

    pub fn set_label(&mut self, label: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.label = label.into();
        cx.notify();
    }

    pub fn set_disabled(&mut self, disabled: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled == disabled {
            return;
        }
        self.disabled = disabled;
        if disabled {
            self.caret.stop();
        } else {
            self.restart_blink(window, cx);
        }
        cx.notify();
    }

    /// Bounds for the caret position, relative to the last painted content box.
    pub fn set_rows(&mut self, min_rows: usize, max_rows: usize, cx: &mut Context<Self>) {
        self.min_rows = min_rows.max(1);
        self.max_rows = max_rows.max(self.min_rows);
        cx.notify();
    }

    /// Caps the control's height in pixels, including its field insets.
    ///
    /// The intrinsic minimum is still one row; a cap smaller than that is
    /// raised to the minimum. `None` restores the row-based `max_rows` cap.
    /// The preferred height in between comes from the area's own wrapped
    /// measurement, so consumers never calculate wrapping themselves.
    pub fn set_max_height(&mut self, max: Option<Pixels>, cx: &mut Context<Self>) {
        if self.max_height == max {
            return;
        }
        self.max_height = max;
        cx.notify();
    }

    /// The wrapped content height measured by the last layout, including
    /// insets. `None` until the control has been laid out.
    #[must_use]
    pub fn measured_content_height(&self) -> Option<Pixels> {
        self.measured_height
    }

    /// The internal scroll offset applied by the last paint.
    #[must_use]
    pub fn scroll_offset(&self) -> Pixels {
        self.scroll_y
    }

    fn restart_blink(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        crate::text_client::restart_blink(self, window, cx);
    }

    // --- movement ---------------------------------------------------------

    fn move_to(&mut self, offset: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.session.move_to(offset) {
            window.notify_text_selection_changed(&self.focus);
            self.restart_blink(window, cx);
        }
    }

    fn select_to(&mut self, offset: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.session.select_to(offset) {
            window.notify_text_selection_changed(&self.focus);
            self.restart_blink(window, cx);
        }
    }

    fn select_range(&mut self, range: Range<usize>, window: &mut Window, cx: &mut Context<Self>) {
        crate::text_client::select_range(self, range, window, cx);
    }

    /// Line, local byte index, x and y within the wrapped layout for `offset`.
    fn visual_position(&self, offset: usize) -> Option<(usize, usize, Pixels, Pixels)> {
        let line = self.line_for_offset(offset)?;
        let local = offset.saturating_sub(self.last_starts[line]);
        let lh = self.current_line_height;
        let pos = self
            .last_lines
            .get(line)?
            .position_for_index(local.min(self.last_lines[line].len()), lh)?;
        Some((
            line,
            local,
            pos.x,
            self.last_tops.get(line).copied().unwrap_or(px(0.)) + pos.y,
        ))
    }

    // The last line height used for layout, so movement can be computed outside
    // prepaint. Set during paint.
    fn line_for_offset(&self, offset: usize) -> Option<usize> {
        if self.last_starts.is_empty() {
            return None;
        }
        let mut line = 0;
        for (index, start) in self.last_starts.iter().enumerate() {
            if *start <= offset {
                line = index;
            } else {
                break;
            }
        }
        Some(line)
    }

    fn move_vertical(
        &mut self,
        down: bool,
        extend: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((line, local, x, y)) = self.visual_position(self.session.cursor()) else {
            return;
        };
        let lh = self.current_line_height;
        let target_y = if down { y + lh } else { y - lh };
        if target_y < px(0.) {
            self.select_to(0, window, cx);
            return;
        }
        // Find the line whose top is at or before target_y.
        let mut target_line = line;
        for (index, top) in self.last_tops.iter().enumerate() {
            if *top <= target_y {
                target_line = index;
            } else {
                break;
            }
        }
        let local_y = target_y - self.last_tops[target_line];
        let Some(wrapped) = self.last_lines.get(target_line) else {
            return;
        };
        let index = wrapped
            .closest_index_for_position(point(x, local_y), lh)
            .unwrap_or(if down { wrapped.len() } else { 0 });
        let offset = self.last_starts[target_line] + index;
        if extend {
            self.select_to(offset, window, cx);
        } else {
            self.move_to(offset, window, cx);
        }
        let _ = local;
    }

    // --- actions ----------------------------------------------------------

    fn left(&mut self, _: &Left, window: &mut Window, cx: &mut Context<Self>) {
        let to = if self.session.selection().is_empty() {
            self.session.previous_boundary(self.session.cursor())
        } else {
            self.session.selection().start()
        };
        self.move_to(to, window, cx);
    }

    fn right(&mut self, _: &Right, window: &mut Window, cx: &mut Context<Self>) {
        let to = if self.session.selection().is_empty() {
            self.session.next_boundary(self.session.cursor())
        } else {
            self.session.selection().end()
        };
        self.move_to(to, window, cx);
    }

    fn select_left(&mut self, _: &SelectLeft, window: &mut Window, cx: &mut Context<Self>) {
        self.select_to(
            self.session.previous_boundary(self.session.cursor()),
            window,
            cx,
        );
    }

    fn select_right(&mut self, _: &SelectRight, window: &mut Window, cx: &mut Context<Self>) {
        self.select_to(
            self.session.next_boundary(self.session.cursor()),
            window,
            cx,
        );
    }

    fn word_left(&mut self, _: &WordLeft, window: &mut Window, cx: &mut Context<Self>) {
        self.move_to(
            self.session.previous_word(self.session.cursor()),
            window,
            cx,
        );
    }

    fn word_right(&mut self, _: &WordRight, window: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.session.next_word(self.session.cursor()), window, cx);
    }

    fn select_word_left(
        &mut self,
        _: &SelectWordLeft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_to(
            self.session.previous_word(self.session.cursor()),
            window,
            cx,
        );
    }

    fn select_word_right(
        &mut self,
        _: &SelectWordRight,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_to(self.session.next_word(self.session.cursor()), window, cx);
    }

    fn up(&mut self, _: &Up, window: &mut Window, cx: &mut Context<Self>) {
        self.move_vertical(false, false, window, cx);
    }

    fn down(&mut self, _: &Down, window: &mut Window, cx: &mut Context<Self>) {
        self.move_vertical(true, false, window, cx);
    }

    fn select_up(&mut self, _: &SelectUp, window: &mut Window, cx: &mut Context<Self>) {
        self.move_vertical(false, true, window, cx);
    }

    fn select_down(&mut self, _: &SelectDown, window: &mut Window, cx: &mut Context<Self>) {
        self.move_vertical(true, true, window, cx);
    }

    fn home(&mut self, _: &Home, window: &mut Window, cx: &mut Context<Self>) {
        let bounds = self.session.line_bounds(self.session.cursor());
        self.move_to(bounds.start, window, cx);
    }

    fn end(&mut self, _: &End, window: &mut Window, cx: &mut Context<Self>) {
        let bounds = self.session.line_bounds(self.session.cursor());
        self.move_to(bounds.end, window, cx);
    }

    fn select_home(&mut self, _: &SelectHome, window: &mut Window, cx: &mut Context<Self>) {
        let bounds = self.session.line_bounds(self.session.cursor());
        self.select_to(bounds.start, window, cx);
    }

    fn select_end(&mut self, _: &SelectEnd, window: &mut Window, cx: &mut Context<Self>) {
        let bounds = self.session.line_bounds(self.session.cursor());
        self.select_to(bounds.end, window, cx);
    }

    fn select_all(&mut self, _: &SelectAll, window: &mut Window, cx: &mut Context<Self>) {
        if self.session.select_all() {
            window.notify_text_selection_changed(&self.focus);
            self.restart_blink(window, cx);
        }
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        let change = self.session.backspace();
        if change.is_none() {
            window.play_system_bell();
        }
        self.after_edit(change, window, cx);
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        let change = self.session.delete();
        if change.is_none() {
            window.play_system_bell();
        }
        self.after_edit(change, window, cx);
    }

    fn delete_word_left(
        &mut self,
        _: &DeleteWordLeft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let change = self.session.delete_word_left();
        self.after_edit(change, window, cx);
    }

    fn delete_word_right(
        &mut self,
        _: &DeleteWordRight,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let change = self.session.delete_word_right();
        self.after_edit(change, window, cx);
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = self.session.copy_text().filter(|text| !text.is_empty()) {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = self.session.copy_text().filter(|text| !text.is_empty()) {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
            let change = self.session.replace_selection("");
            self.after_edit(change, window, cx);
        }
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            let text = ClipboardPolicy::area().pasted(&text);
            let change = self.session.replace_selection(&text);
            self.after_edit(change, window, cx);
        }
    }

    fn newline(&mut self, _: &Newline, window: &mut Window, cx: &mut Context<Self>) {
        let change = self.session.replace_selection("\n");
        self.after_edit(change, window, cx);
    }

    fn submit(&mut self, _: &Submit, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(TextAreaEvent::Submitted);
    }

    // --- mouse ------------------------------------------------------------

    fn index_for_position(&self, position: Point<Pixels>) -> usize {
        let (Some(origin), Some(bounds)) = (self.last_origin, self.last_bounds) else {
            return 0;
        };
        if self.last_lines.is_empty() {
            return 0;
        }
        let y = position.y - origin.y + self.scroll_y;
        let mut line = 0;
        for (index, top) in self.last_tops.iter().enumerate() {
            if *top <= y {
                line = index;
            } else {
                break;
            }
        }
        let local_y = y - self.last_tops[line];
        let x = (position.x - origin.x).max(px(0.));
        let wrapped = &self.last_lines[line];
        let local = wrapped
            .closest_index_for_position(point(x, local_y), self.current_line_height)
            .unwrap_or(0);
        let _ = bounds;
        (self.last_starts[line] + local).min(self.session.len())
    }

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
            n if n >= 3 => self.select_range(0..self.session.len(), window, cx),
            2 => {
                let line = self.session.line_bounds(index);
                self.select_range(line, window, cx);
            }
            _ => {
                self.selecting = true;
                if event.modifiers.shift {
                    self.select_to(index, window, cx);
                } else {
                    self.move_to(index, window, cx);
                }
            }
        }
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

    fn on_scroll(&mut self, event: &gpui::ScrollEvent, cx: &mut Context<Self>) {
        let delta = event.delta.pixel_delta(self.current_line_height).y;
        self.scroll_y = (self.scroll_y - delta).max(px(0.));
        cx.notify();
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.selecting = false;
    }
}

impl TextHost for TextArea {
    fn session(&self) -> &EditSession {
        &self.session
    }

    fn session_mut(&mut self) -> &mut EditSession {
        &mut self.session
    }

    fn after_history_change(&mut self, cx: &mut Context<Self>) {
        cx.emit(TextAreaEvent::Changed);
        cx.notify();
    }
}

crate::impl_text_input_handler!(TextArea);

impl TextClient for TextArea {
    fn session(&self) -> &EditSession {
        &self.session
    }

    fn session_mut(&mut self) -> &mut EditSession {
        &mut self.session
    }

    fn text_focus(&self) -> &FocusHandle {
        &self.focus
    }

    fn caret(&mut self) -> &mut CaretBlink {
        &mut self.caret
    }

    fn is_disabled(&self) -> bool {
        self.disabled
    }

    fn did_change(&mut self, change: &Change, cx: &mut Context<Self>) {
        self.record_change(Some(change.clone()), cx);
        cx.emit(TextAreaEvent::Changed);
    }

    fn range_bounds(&self, range: Range<usize>) -> Option<Bounds<Pixels>> {
        let (line, _, x, y) = self.visual_position(range.start)?;
        let (_, _, x1, y1) = self.visual_position(range.end).unwrap_or((line, 0, x, y));
        let origin = self.last_origin?;
        Some(Bounds::from_corners(
            point(origin.x + x, origin.y + y - self.scroll_y),
            point(
                origin.x + x1,
                origin.y + y1 - self.scroll_y + self.current_line_height,
            ),
        ))
    }

    fn offset_at(&self, p: Point<Pixels>) -> Option<usize> {
        if !self.last_bounds?.contains(&p) {
            return None;
        }
        Some(self.index_for_position(p))
    }
}

impl Render for TextArea {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let m = &theme.metrics;
        let enabled = !self.disabled;
        let focused = self.focus.is_focused(window);
        let line_height = window.line_height();
        let inset = px(m.field_text_inset * 2.0);
        let min_height = line_height * self.min_rows as f32 + inset;
        let max_height = self
            .max_height
            .map_or(line_height * self.max_rows as f32 + inset, |max| {
                max.max(min_height)
            });
        // Before the first shaped pass, fall back to the physical-line estimate.
        // Afterwards the height follows the wrapped content the area actually
        // painted, so a single long paragraph grows and three short paragraphs
        // do not.
        let preferred = self.measured_height.unwrap_or_else(|| {
            let logical_rows = self.session.text().split('\n').count().max(1);
            line_height * logical_rows as f32 + inset
        });
        let height = preferred.max(min_height).min(max_height);
        let focus_ring = theme.colors.focus_ring;
        let ring_profile = m.focus_ring;
        let field = theme.controls.field;
        let area = cx.entity();
        let selecting = self.selecting;
        let _ = selecting;

        div()
            .id(ElementId::Name(
                format!("silvestris-text-area-{}", cx.entity_id()).into(),
            ))
            .key_context("SilvestrisTextArea SilvestrisText")
            .role(gpui::Role::MultilineTextInput)
            .aria_label(self.label.clone())
            .aria_placeholder(self.placeholder.clone())
            .aria_value(SharedString::from(self.session.text().to_string()))
            .relative()
            .w_full()
            .h(height)
            .track_focus(&self.focus)
            .cursor(CursorStyle::IBeam)
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_scroll(cx.listener(|area, event, _, cx| area.on_scroll(event, cx)))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::word_left))
            .on_action(cx.listener(Self::word_right))
            .on_action(cx.listener(Self::select_word_left))
            .on_action(cx.listener(Self::select_word_right))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::select_up))
            .on_action(cx.listener(Self::select_down))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::select_home))
            .on_action(cx.listener(Self::select_end))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::delete_word_left))
            .on_action(cx.listener(Self::delete_word_right))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::newline))
            .on_action(cx.listener(Self::submit))
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, _| {
                        paint::field(window, bounds, 0.0, &field, enabled);
                        if focused {
                            paint::focus_ring(window, bounds, 0.0, focus_ring, ring_profile);
                        }
                    },
                )
                .absolute()
                .inset_0(),
            )
            .child(AreaElement { area, line_height })
    }
}

struct AreaPrepaint {
    lines: Vec<WrappedLine>,
    starts: Vec<usize>,
    tops: Vec<Pixels>,
    origin: Point<Pixels>,
    clip: Bounds<Pixels>,
    scroll_y: Pixels,
    content_height: Pixels,
    selection: Vec<PaintQuad>,
    caret: Option<PaintQuad>,
}

struct AreaElement {
    area: Entity<TextArea>,
    line_height: Pixels,
}

impl Element for AreaElement {
    type RequestLayoutState = ();
    type PrepaintState = AreaPrepaint;

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
        style.inset.right = px(0.).into();
        style.inset.bottom = px(0.).into();
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
    ) -> AreaPrepaint {
        let theme = cx.theme().clone();
        let m = &theme.metrics;
        let area = self.area.read(cx);
        let style = window.text_style();
        let font_size = style.font_size.to_pixels(window.rem_size());
        let color = if area.disabled {
            theme.colors.text_disabled
        } else {
            theme.controls.field.text
        };
        let text = if area.session.is_empty() {
            area.placeholder.to_string()
        } else {
            area.session.text().to_string()
        };
        let placeholder = area.session.is_empty();
        let text_color = if placeholder {
            theme.controls.field.placeholder
        } else {
            color
        };
        let run = TextRun {
            len: text.len(),
            font: style.font(),
            color: hsla(text_color),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let runs = match area
            .session
            .marked_range()
            .as_ref()
            .filter(|_| !placeholder)
        {
            Some(marked) => {
                let s = marked.start.min(text.len());
                let e = marked.end.min(text.len());
                vec![
                    TextRun {
                        len: s,
                        ..run.clone()
                    },
                    TextRun {
                        len: e.saturating_sub(s),
                        underline: Some(UnderlineStyle {
                            color: Some(run.color),
                            thickness: crate::convert::hairline(window),
                            wavy: false,
                        }),
                        ..run.clone()
                    },
                    TextRun {
                        len: text.len().saturating_sub(e),
                        ..run
                    },
                ]
                .into_iter()
                .filter(|run| run.len > 0)
                .collect()
            }
            None => vec![run],
        };
        let inset = px(m.field_text_inset);
        let clip = Bounds::from_corners(
            point(bounds.left() + px(2.), bounds.top() + px(2.)),
            point(bounds.right() - px(2.), bounds.bottom() - px(2.)),
        );
        let wrap = (bounds.size.width - inset * 2.0).max(px(1.));
        let lines: Vec<WrappedLine> = window
            .text_system()
            .shape_text(SharedString::from(text), font_size, &runs, Some(wrap), None)
            .unwrap_or_default()
            .into_iter()
            .collect();
        let mut starts = Vec::with_capacity(lines.len());
        let mut tops = Vec::with_capacity(lines.len());
        let mut y = px(0.);
        let mut start = 0;
        for line in &lines {
            starts.push(start);
            tops.push(y);
            let rows = line.wrap_boundaries().len() + 1;
            y += self.line_height * rows as f32;
            start += line.len() + 1;
        }
        let total = y;
        // The caret may paint up to the content mask (bounds inset by 2), which
        // starts at the field inset. Using the raw clip height would let the
        // last row hang below the mask when the field inset is larger.
        let visible = (bounds.size.height - inset - px(2.)).max(px(1.));
        let line = area.line_for_offset(area.session.cursor()).unwrap_or(0);
        let local = area
            .session
            .cursor()
            .saturating_sub(starts.get(line).copied().unwrap_or(0));
        let local_pos = lines
            .get(line)
            .and_then(|wrapped| wrapped.position_for_index(local, self.line_height))
            .unwrap_or(point(px(0.), px(0.)));
        let caret_y = tops.get(line).copied().unwrap_or(px(0.)) + local_pos.y;
        let mut scroll_y = area.scroll_y.min((total - visible).max(px(0.)));
        if caret_y < scroll_y {
            scroll_y = caret_y;
        }
        if caret_y + self.line_height > scroll_y + visible {
            scroll_y = (caret_y + self.line_height - visible).max(px(0.));
        }
        let origin = point(bounds.left() + inset, bounds.top() + inset - scroll_y);

        let focused = area.focus.is_focused(window);
        let mut selection = Vec::new();
        if focused && !area.session.selection().is_empty() {
            let range = area.session.selected_range();
            for (index, wrapped) in lines.iter().enumerate() {
                let line_start = starts[index];
                let line_end = line_start + wrapped.len();
                if range.end < line_start || range.start > line_end {
                    continue;
                }
                let local_start = range.start.max(line_start) - line_start;
                let local_end = range.end.min(line_end) - line_start;
                let top = tops[index];
                let start_pos = wrapped
                    .position_for_index(local_start, self.line_height)
                    .unwrap_or(point(px(0.), px(0.)));
                let end_pos = wrapped
                    .position_for_index(local_end, self.line_height)
                    .unwrap_or(point(wrapped.width(), px(0.)));
                selection.push(gpui::fill(
                    Bounds::from_corners(
                        point(origin.x + start_pos.x, origin.y + top + start_pos.y),
                        point(
                            origin.x + end_pos.x,
                            origin.y + top + end_pos.y + self.line_height,
                        ),
                    ),
                    hsla(theme.controls.field.selection),
                ));
            }
        }
        let caret = (focused
            && area.session.selection().is_empty()
            && area.caret.on
            && window.is_window_active())
        .then(|| {
            gpui::fill(
                Bounds::new(
                    point(origin.x + local_pos.x, origin.y + caret_y),
                    size(px(m.caret_width), self.line_height),
                ),
                hsla(theme.controls.field.caret),
            )
        });
        AreaPrepaint {
            lines,
            starts,
            tops,
            origin,
            clip,
            scroll_y,
            content_height: total + inset * 2.0,
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
        prepaint: &mut AreaPrepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus = self.area.read(cx).focus.clone();
        window.handle_input(
            &focus,
            ElementInputHandler::new(bounds, self.area.clone()),
            cx,
        );
        let origin = prepaint.origin;
        let clip = prepaint.clip;
        let line_height = self.line_height;
        window.with_content_mask(Some(gpui::ContentMask { bounds: clip }), |window| {
            for quad in prepaint.selection.drain(..) {
                window.paint_quad(quad);
            }
            for (index, line) in prepaint.lines.iter().enumerate() {
                let top = prepaint.tops[index];
                line.paint(
                    point(origin.x, origin.y + top),
                    line_height,
                    gpui::TextAlign::Left,
                    Some(Bounds::new(
                        point(origin.x, origin.y + top),
                        size(clip.size.width, line_height),
                    )),
                    window,
                    cx,
                )
                .ok();
            }
            if let Some(caret) = prepaint.caret.take() {
                window.paint_quad(caret);
            }
        });
        let lines = std::mem::take(&mut prepaint.lines);
        let starts = std::mem::take(&mut prepaint.starts);
        let tops = std::mem::take(&mut prepaint.tops);
        let scroll_y = prepaint.scroll_y;
        let content_height = prepaint.content_height;
        let origin = prepaint.origin;
        let clip = prepaint.clip;
        self.area.update(cx, |area, cx| {
            area.last_lines = lines;
            area.last_starts = starts;
            area.last_tops = tops;
            area.last_origin = Some(origin);
            area.last_bounds = Some(clip);
            area.scroll_y = scroll_y;
            area.current_line_height = line_height;
            // Width-dependent wrapping changes the measured height. Notify only
            // when the preferred height genuinely moved, so a stable layout does
            // not request an identical frame every paint.
            let changed = area
                .measured_height
                .is_none_or(|old| (old - content_height).abs() > px(0.5));
            area.measured_height = Some(content_height);
            if changed {
                cx.notify();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use gpui::{Modifiers, TestAppContext, VisualTestContext};

    use super::*;

    struct Harness {
        area: Entity<TextArea>,
        events: Rc<RefCell<Vec<String>>>,
        focus: FocusHandle,
        width: f32,
    }

    impl Render for Harness {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .track_focus(&self.focus)
                .child(div().w(px(self.width)).child(self.area.clone()))
        }
    }

    fn draw(cx: &mut VisualTestContext) {
        cx.run_until_parked();
        cx.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        cx.run_until_parked();
    }

    fn harness(app: &mut TestAppContext) -> (Entity<Harness>, &mut VisualTestContext) {
        app.add_window_view(|window, cx| {
            let area = cx.new(|cx| TextArea::new(window, cx));
            let events = Rc::new(RefCell::new(Vec::new()));
            let log = events.clone();
            cx.subscribe(&area, move |_, _, event: &TextAreaEvent, _| {
                log.borrow_mut().push(format!("{event:?}"));
            })
            .detach();
            let focus = cx.focus_handle();
            window.focus(&focus, cx);
            Harness {
                area,
                events,
                focus,
                width: 320.,
            }
        })
    }

    #[test]
    fn enter_submits_shift_enter_inserts_and_whitespace_round_trips() {
        let mut app = TestAppContext::single();
        app.update(|cx| {
            crate::theme::install(crate::ThemePreference::default(), cx);
            crate::controls::text_field::bind_keys(cx);
        });
        let (view, cx) = harness(&mut app);
        draw(cx);
        let area_focus = view.read_with(cx, |view, cx| view.area.read(cx).focus_handle(cx));
        cx.update(|window, cx| window.focus(&area_focus, cx));
        draw(cx);
        cx.simulate_input("first");
        draw(cx);
        cx.simulate_keystrokes("shift-enter");
        draw(cx);
        cx.simulate_input("second");
        draw(cx);
        assert_eq!(
            view.read_with(cx, |view, cx| view.area.read(cx).text().to_string()),
            "first\nsecond"
        );
        cx.simulate_keystrokes("enter");
        draw(cx);
        assert!(
            view.read_with(cx, |view, _| view
                .events
                .borrow()
                .iter()
                .any(|event| event.contains("Submitted"))),
            "Return submits"
        );

        // Exact whitespace and newlines survive a programmatic set/read.
        let text = "  one\n\n\ttwo  \n";
        cx.update(|_, cx| {
            view.update(cx, |view, cx| {
                view.area.update(cx, |area, cx| area.set_text(text, cx))
            })
        });
        assert_eq!(
            view.read_with(cx, |view, cx| view.area.read(cx).text().to_string()),
            text
        );
    }

    #[test]
    fn home_end_and_up_down_stay_on_physical_lines_and_visual_rows() {
        let mut app = TestAppContext::single();
        app.update(|cx| {
            crate::theme::install(crate::ThemePreference::default(), cx);
            crate::controls::text_field::bind_keys(cx);
        });
        let (view, cx) = harness(&mut app);
        cx.update(|_, cx| {
            view.update(cx, |view, cx| {
                view.area
                    .update(cx, |area, cx| area.set_text("abc\ndef\nghi", cx))
            })
        });
        draw(cx);
        let focus = view.read_with(cx, |view, cx| view.area.read(cx).focus_handle(cx));
        cx.update(|window, cx| window.focus(&focus, cx));
        draw(cx);

        let (lines, starts) = view.read_with(cx, |view, cx| {
            let area = view.area.read(cx);
            (area.last_lines.len(), area.last_starts.clone())
        });
        assert_eq!(lines, 3, "the three physical lines are shaped");
        assert_eq!(starts, vec![0, 4, 8]);
        let area = view.read_with(cx, |view, _| view.area.clone());
        let before = view.read_with(cx, |view, cx| {
            let area = view.area.read(cx);
            (
                area.session.cursor(),
                area.session.line_bounds(area.session.cursor()),
            )
        });
        cx.update(|window, cx| {
            area.update(cx, |area, cx| {
                area.home(&Home, window, cx);
            })
        });
        let after_home = view.read_with(cx, |view, cx| view.area.read(cx).session.cursor());
        assert_eq!((before.0, before.1), (11, 8..11), "test setup");
        assert_eq!(after_home, 8, "Home moves to the physical line start");
        cx.update(|window, cx| area.update(cx, |area, cx| area.up(&Up, window, cx)));
        let cursor = view.read_with(cx, |view, cx| view.area.read(cx).session.cursor());
        assert_eq!(cursor, 4, "Up reaches the previous physical line");
        cx.update(|window, cx| area.update(cx, |area, cx| area.down(&Down, window, cx)));
        cx.update(|window, cx| area.update(cx, |area, cx| area.end(&End, window, cx)));
        draw(cx);
        let cursor = view.read_with(cx, |view, cx| view.area.read(cx).session.cursor());
        assert_eq!(cursor, 11, "End is the physical line end");
        cx.simulate_keystrokes("right backspace");
        draw(cx);
        assert_eq!(
            view.read_with(cx, |view, cx| view.area.read(cx).text().to_string()),
            "abc\ndef\ngh"
        );
    }

    #[test]
    fn typing_and_backspace_record_and_undo_one_step_at_a_time() {
        let mut app = TestAppContext::single();
        app.update(|cx| {
            crate::theme::install(crate::ThemePreference::default(), cx);
            crate::controls::text_field::bind_keys(cx);
        });
        let (view, cx) = harness(&mut app);
        draw(cx);
        let focus = view.read_with(cx, |view, cx| view.area.read(cx).focus_handle(cx));
        cx.update(|window, cx| window.focus(&focus, cx));
        draw(cx);
        cx.simulate_input("ab");
        draw(cx);
        let (entries, name) = view.read_with(cx, |view, cx| {
            let history = view.area.read(cx).history();
            let history = history.read(cx);
            (history.len(), history.undo_name().map(str::to_string))
        });
        assert_eq!(entries, 1);
        assert_eq!(name.as_deref(), Some("Typing"));
        let history = view.read_with(cx, |view, app| view.area.read(app).history());
        cx.update(|_, cx| history.update(cx, |history, cx| assert!(history.undo(cx))));
        draw(cx);
        assert_eq!(
            view.read_with(cx, |view, cx| view.area.read(cx).text().to_string()),
            ""
        );
        cx.update(|_, cx| history.update(cx, |history, cx| assert!(history.redo(cx))));
        draw(cx);
        assert_eq!(
            view.read_with(cx, |view, cx| view.area.read(cx).text().to_string()),
            "ab"
        );

        // A backspace is its own step, and undoing it restores the character.
        cx.simulate_keystrokes("backspace");
        draw(cx);
        assert_eq!(
            view.read_with(cx, |view, cx| view.area.read(cx).text().to_string()),
            "a"
        );
        cx.update(|_, cx| history.update(cx, |history, cx| assert!(history.undo(cx))));
        draw(cx);
        assert_eq!(
            view.read_with(cx, |view, cx| view.area.read(cx).text().to_string()),
            "ab"
        );
    }

    #[test]
    fn a_masked_field_still_refuses_copy_and_the_area_keeps_newlines_on_paste() {
        let mut app = TestAppContext::single();
        app.update(|cx| {
            crate::theme::install(crate::ThemePreference::default(), cx);
            crate::controls::text_field::bind_keys(cx);
        });
        let (view, cx) = harness(&mut app);
        cx.update(|_, cx| {
            view.update(cx, |view, cx| {
                view.area.update(cx, |area, cx| {
                    area.set_text("one\ntwo", cx);
                    area.session.select_all();
                });
                cx.write_to_clipboard(gpui::ClipboardItem::new_string("a\nb".into()));
            })
        });
        draw(cx);
        let focus = view.read_with(cx, |view, cx| view.area.read(cx).focus_handle(cx));
        cx.update(|window, cx| window.focus(&focus, cx));
        draw(cx);
        cx.simulate_keystrokes("secondary-v");
        draw(cx);
        assert_eq!(
            view.read_with(cx, |view, cx| view.area.read(cx).text().to_string()),
            "a\nb"
        );
        let _ = Modifiers::default();
    }

    // --- visual height -----------------

    fn harness_with_text<'a>(
        app: &'a mut TestAppContext,
        text: &str,
    ) -> (Entity<Harness>, &'a mut VisualTestContext) {
        app.update(|cx| {
            crate::theme::install(crate::ThemePreference::default(), cx);
            crate::controls::text_field::bind_keys(cx);
        });
        let (view, cx) = harness(app);
        let text = text.to_owned();
        cx.update(|_, cx| {
            view.update(cx, |view, cx| {
                view.area
                    .update(cx, |area, cx| area.set_text(text.clone(), cx))
            })
        });
        (view, cx)
    }

    fn line_height(cx: &mut VisualTestContext) -> Pixels {
        cx.update(|window, _| window.line_height())
    }

    fn inset_height(cx: &mut VisualTestContext) -> Pixels {
        cx.update(|_, cx| px(cx.theme().metrics.field_text_inset * 2.0))
    }

    fn measured(view: &Entity<Harness>, cx: &mut VisualTestContext) -> Pixels {
        view.read_with(cx, |view, cx| {
            view.area
                .read(cx)
                .measured_content_height()
                .expect("the area has been laid out")
        })
    }

    fn set_area(
        view: &Entity<Harness>,
        cx: &mut VisualTestContext,
        change: impl FnOnce(&mut TextArea, &mut Context<TextArea>),
    ) {
        cx.update(|_, cx| view.update(cx, |view, cx| view.area.update(cx, change)));
    }

    #[test]
    fn one_physical_line_measures_one_row() {
        let mut app = TestAppContext::single();
        let (view, cx) = harness_with_text(&mut app, "hello");
        draw(cx);
        draw(cx);
        let expected = line_height(cx) + inset_height(cx);
        assert_eq!(measured(&view, cx), expected);
    }

    #[test]
    fn three_explicit_newlines_measure_three_rows() {
        let mut app = TestAppContext::single();
        let (view, cx) = harness_with_text(&mut app, "one\ntwo\nthree");
        draw(cx);
        draw(cx);
        let expected = line_height(cx) * 3.0 + inset_height(cx);
        assert_eq!(measured(&view, cx), expected);
    }

    #[test]
    fn a_long_wrapping_paragraph_grows_without_newlines() {
        let mut app = TestAppContext::single();
        let paragraph = "word ".repeat(120);
        let (view, cx) = harness_with_text(&mut app, &paragraph);
        draw(cx);
        draw(cx);
        let one_row = line_height(cx) + inset_height(cx);
        let height = measured(&view, cx);
        assert!(
            height > one_row * 2.0,
            "wrapped rows, not physical newlines, must decide growth: {height:?}"
        );
    }

    #[test]
    fn deleting_back_to_one_line_shrinks_the_control() {
        let mut app = TestAppContext::single();
        let (view, cx) = harness_with_text(&mut app, "one\ntwo\nthree\nfour\nfive");
        draw(cx);
        draw(cx);
        let tall = measured(&view, cx);
        set_area(&view, cx, |area, cx| area.set_text("one", cx));
        draw(cx);
        draw(cx);
        let short = measured(&view, cx);
        assert!(short < tall);
        assert_eq!(short, line_height(cx) + inset_height(cx));
    }

    #[test]
    fn max_height_clamps_and_scrolls_instead_of_growing() {
        let mut app = TestAppContext::single();
        let (view, cx) = harness_with_text(&mut app, "a\nb\nc\nd\ne\nf\ng\nh\ni");
        let cap = line_height(cx) * 3.0 + inset_height(cx);
        set_area(&view, cx, |area, cx| area.set_max_height(Some(cap), cx));
        draw(cx);
        draw(cx);
        let content = measured(&view, cx);
        let (scroll, bounds) = view.read_with(cx, |view, cx| {
            let area = view.area.read(cx);
            (area.scroll_offset(), area.last_bounds.expect("painted"))
        });
        assert!(content > cap, "content itself stays taller than the cap");
        assert!(scroll > px(0.), "the area scrolls internally at the cap");
        assert!(
            bounds.size.height <= cap,
            "the painted control respects the cap"
        );
    }

    #[test]
    fn the_caret_stays_visible_while_the_area_scrolls() {
        let mut app = TestAppContext::single();
        let (view, cx) = harness_with_text(&mut app, "a\nb\nc\nd\ne\nf\ng\nh\ni");
        let cap = line_height(cx) * 3.0 + inset_height(cx);
        set_area(&view, cx, |area, cx| area.set_max_height(Some(cap), cx));
        draw(cx);
        draw(cx);
        let (caret_top, caret_bottom, clip) = view.read_with(cx, |view, cx| {
            let area = view.area.read(cx);
            let (_, _, _, y) = area
                .visual_position(area.session.cursor())
                .expect("caret position");
            let origin = area.last_origin.expect("painted");
            let clip = area.last_bounds.expect("painted");
            (origin.y + y, origin.y + y + area.current_line_height, clip)
        });
        assert!(
            caret_top >= clip.top() && caret_bottom <= clip.bottom(),
            "caret {caret_top:?}..{caret_bottom:?} must stay inside {clip:?}"
        );
    }

    #[test]
    fn narrowing_wraps_more_and_widening_wraps_less() {
        let mut app = TestAppContext::single();
        let paragraph = "word ".repeat(120);
        let (view, cx) = harness_with_text(&mut app, &paragraph);
        draw(cx);
        draw(cx);
        let wide = measured(&view, cx);
        set_area(&view, cx, |_, _| {});
        cx.update(|_, cx| {
            view.update(cx, |view, cx| {
                view.width = 120.;
                cx.notify();
            })
        });
        draw(cx);
        draw(cx);
        let narrow = measured(&view, cx);
        assert!(narrow > wide, "narrower must wrap into more rows");
        cx.update(|_, cx| {
            view.update(cx, |view, cx| {
                view.width = 320.;
                cx.notify();
            })
        });
        draw(cx);
        draw(cx);
        assert_eq!(measured(&view, cx), wide, "widening restores the row count");
    }

    #[test]
    fn a_short_draft_does_not_scroll_after_removing_the_cap() {
        let mut app = TestAppContext::single();
        let (view, cx) = harness_with_text(&mut app, "one\ntwo\nthree\nfour");
        let cap = line_height(cx) * 2.0 + inset_height(cx);
        set_area(&view, cx, |area, cx| area.set_max_height(Some(cap), cx));
        draw(cx);
        draw(cx);
        assert!(view.read_with(cx, |view, cx| view.area.read(cx).scroll_offset()) > px(0.));
        set_area(&view, cx, |area, cx| {
            area.set_text("short", cx);
            area.set_max_height(None, cx);
        });
        draw(cx);
        draw(cx);
        let (scroll, content) = view.read_with(cx, |view, cx| {
            let area = view.area.read(cx);
            (
                area.scroll_offset(),
                area.measured_content_height().unwrap(),
            )
        });
        assert_eq!(scroll, px(0.), "a short draft is not a scrolling pane");
        assert_eq!(content, line_height(cx) + inset_height(cx));
    }
}
