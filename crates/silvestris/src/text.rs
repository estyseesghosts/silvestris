//! Shared text-editing engine (#58).
//!
//! `TextField`, `TextArea`, token fields, editable table cells and the find bar
//! all need the same mechanics: a buffer, a selection with an anchor and a
//! head, grapheme/word/line boundaries, UTF-8↔UTF-16 mapping for the platform
//! input handler, marked text for IME composition, and a masked policy.
//!
//! The engine deliberately has no GPUI types. The view owns layout, painting,
//! focus, timers and the `EntityInputHandler` trait; this module owns what is
//! edited and where the caret is, so the same conformance tests run against
//! every client.
//!
//! `Buffer` is a trait so a consumer can back the session with a rope. The
//! crate ships `StringBuffer`; chaus keeps its Ropey editor and may implement
//! `Buffer` for its own type without adopting Silvestris widgets.
//!
//! Evidence: HIG pp. 112-120 (selecting and editing text), hig-09
//! `specs/text-input-field.md`, and the duplication inventory from #73.

use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation;

/// The bytes an editor edits. The default is [`StringBuffer`]; a rope-backed
/// implementation can reuse the session.
pub trait Buffer {
    fn text(&self) -> &str;
    fn replace(&mut self, range: Range<usize>, text: &str);
    fn len(&self) -> usize {
        self.text().len()
    }
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// The string-backed buffer every Silvestris control uses.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StringBuffer(String);

impl StringBuffer {
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self(text.into())
    }
}

impl Buffer for StringBuffer {
    fn text(&self) -> &str {
        &self.0
    }

    fn replace(&mut self, range: Range<usize>, text: &str) {
        self.0.replace_range(range, text);
    }
}

/// Anchor and head as byte offsets. The reversed flag is `head < anchor`; the
/// caret is at the head, while Home/End and mouse selection move the anchor.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    pub anchor: usize,
    pub head: usize,
}

impl Selection {
    #[must_use]
    pub fn new(anchor: usize, head: usize) -> Self {
        Self { anchor, head }
    }

    #[must_use]
    pub fn caret(offset: usize) -> Self {
        Self {
            anchor: offset,
            head: offset,
        }
    }

    #[must_use]
    pub fn range(&self) -> Range<usize> {
        self.anchor.min(self.head)..self.anchor.max(self.head)
    }

    #[must_use]
    pub fn start(&self) -> usize {
        self.anchor.min(self.head)
    }

    #[must_use]
    pub fn end(&self) -> usize {
        self.anchor.max(self.head)
    }

    #[must_use]
    pub fn is_reversed(&self) -> bool {
        self.head < self.anchor
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }
}

/// One replacement, for change notification and undo grouping (#39).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    /// The range that was replaced, in the buffer before the edit.
    pub range: Range<usize>,
    /// The text inserted at `range.start`.
    pub inserted: String,
    /// Whether this edit was an IME composition update rather than a commit.
    pub composing: bool,
}

impl Change {
    /// The range the inserted text occupies after the edit.
    #[must_use]
    pub fn inserted_range(&self) -> Range<usize> {
        self.range.start..self.range.start + self.inserted.len()
    }

    /// A one-line label suitable for an undo menu ("Undo Typing").
    #[must_use]
    pub fn kind(&self) -> EditKind {
        if self.composing {
            EditKind::Compose
        } else if self.inserted.is_empty() {
            EditKind::Delete
        } else if self.range.is_empty() {
            EditKind::Insert
        } else {
            EditKind::Replace
        }
    }
}

/// Coarse edit classification for undo grouping and menu titles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditKind {
    Insert,
    Delete,
    Replace,
    Compose,
}

/// Buffer, selection, marked text and the masked policy.
#[derive(Clone, Debug)]
pub struct EditSession<B: Buffer = StringBuffer> {
    buffer: B,
    selection: Selection,
    marked: Option<Range<usize>>,
    masked: bool,
}

impl Default for EditSession<StringBuffer> {
    fn default() -> Self {
        Self::new()
    }
}

impl EditSession<StringBuffer> {
    #[must_use]
    pub fn new() -> Self {
        Self::from_buffer(StringBuffer::default())
    }

    #[must_use]
    pub fn with_text(text: impl Into<String>) -> Self {
        Self::from_buffer(StringBuffer::new(text))
    }
}

impl<B: Buffer> EditSession<B> {
    #[must_use]
    pub fn from_buffer(buffer: B) -> Self {
        Self {
            buffer,
            selection: Selection::default(),
            marked: None,
            masked: false,
        }
    }

    #[must_use]
    pub fn text(&self) -> &str {
        self.buffer.text()
    }

    #[must_use]
    pub fn buffer(&self) -> &B {
        &self.buffer
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    #[must_use]
    pub fn selection(&self) -> Selection {
        self.selection
    }

    #[must_use]
    pub fn selected_range(&self) -> Range<usize> {
        self.selection.range()
    }

    #[must_use]
    pub fn cursor(&self) -> usize {
        self.selection.head
    }

    #[must_use]
    pub fn is_masked(&self) -> bool {
        self.masked
    }

    /// Masked sessions never expose word boundaries, copy text or IME text
    /// beyond their display form.
    pub fn set_masked(&mut self, masked: bool) {
        self.masked = masked;
        if masked {
            self.marked = None;
        }
    }

    /// Replace the whole text and place the caret at the end.
    pub fn set_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        self.buffer.replace(0..self.buffer.len(), &text);
        self.selection = Selection::caret(self.buffer.len());
        self.marked = None;
    }

    pub fn set_selection(&mut self, selection: Selection) {
        let len = self.buffer.len();
        self.selection = Selection::new(
            floor_boundary(self.buffer.text(), selection.anchor.min(len)),
            floor_boundary(self.buffer.text(), selection.head.min(len)),
        );
    }

    pub fn move_to(&mut self, offset: usize) -> bool {
        let offset = floor_boundary(self.buffer.text(), offset.min(self.buffer.len()));
        if self.selection.anchor == offset && self.selection.head == offset {
            return false;
        }
        self.selection = Selection::caret(offset);
        true
    }

    /// Extend the selection to `offset`, keeping the anchor. Used by
    /// Shift-click, Shift-arrow and mouse drag.
    pub fn select_to(&mut self, offset: usize) -> bool {
        let offset = floor_boundary(self.buffer.text(), offset.min(self.buffer.len()));
        if self.selection.head == offset {
            return false;
        }
        self.selection.head = offset;
        true
    }

    pub fn select_range(&mut self, range: Range<usize>) {
        let text = self.buffer.text();
        let start = floor_boundary(text, range.start.min(text.len()));
        let end = floor_boundary(text, range.end.min(text.len()));
        self.selection = Selection::new(start, end);
    }

    pub fn select_all(&mut self) -> bool {
        let len = self.buffer.len();
        if self.selection.range() == (0..len) {
            return false;
        }
        self.selection = Selection::new(0, len);
        true
    }

    /// Replace the selected range. Returns the change for notification and
    /// undo. A non-composing edit clears marked text first.
    pub fn replace_selection(&mut self, text: &str) -> Option<Change> {
        self.replace_range(self.selected_range(), text, false)
    }

    pub fn replace_range(
        &mut self,
        range: Range<usize>,
        text: &str,
        composing: bool,
    ) -> Option<Change> {
        let len = self.buffer.len();
        let start = floor_boundary(self.buffer.text(), range.start.min(len));
        let end = floor_boundary(self.buffer.text(), range.end.min(len));
        let start = start.min(end);
        let end = end.max(start);
        if start == end && text.is_empty() {
            return None;
        }
        self.buffer.replace(start..end, text);
        let head = start + text.len();
        self.selection = Selection::caret(head);
        if !composing {
            self.marked = None;
        } else {
            self.marked = Some(head..head);
        }
        Some(Change {
            range: start..end,
            inserted: text.to_string(),
            composing,
        })
    }

    /// Delete the selection, or one grapheme before the caret.
    pub fn backspace(&mut self) -> Option<Change> {
        if !self.selection.is_empty() {
            return self.replace_range(self.selected_range(), "", false);
        }
        let start = self.previous_boundary(self.cursor());
        self.replace_range(start..self.cursor(), "", false)
    }

    /// Delete the selection, or one grapheme after the caret.
    pub fn delete(&mut self) -> Option<Change> {
        if !self.selection.is_empty() {
            return self.replace_range(self.selected_range(), "", false);
        }
        let end = self.next_boundary(self.cursor());
        self.replace_range(self.cursor()..end, "", false)
    }

    pub fn delete_word_left(&mut self) -> Option<Change> {
        if !self.selection.is_empty() {
            return self.replace_range(self.selected_range(), "", false);
        }
        let start = self.previous_word(self.cursor());
        self.replace_range(start..self.cursor(), "", false)
    }

    pub fn delete_word_right(&mut self) -> Option<Change> {
        if !self.selection.is_empty() {
            return self.replace_range(self.selected_range(), "", false);
        }
        let end = self.next_word(self.cursor());
        self.replace_range(self.cursor()..end, "", false)
    }

    // --- boundaries -------------------------------------------------------

    #[must_use]
    pub fn previous_boundary(&self, offset: usize) -> usize {
        if self.masked {
            return 0;
        }
        self.buffer
            .text()
            .grapheme_indices(true)
            .rev()
            .find_map(|(index, _)| (index < offset).then_some(index))
            .unwrap_or(0)
    }

    #[must_use]
    pub fn next_boundary(&self, offset: usize) -> usize {
        if self.masked {
            return self.buffer.len();
        }
        self.buffer
            .text()
            .grapheme_indices(true)
            .find_map(|(index, _)| (index > offset).then_some(index))
            .unwrap_or(self.buffer.len())
    }

    #[must_use]
    pub fn previous_word(&self, offset: usize) -> usize {
        if self.masked {
            return 0;
        }
        self.buffer
            .text()
            .split_word_bound_indices()
            .rev()
            .find_map(|(index, word)| {
                (index < offset && word.chars().any(char::is_alphanumeric)).then_some(index)
            })
            .unwrap_or(0)
    }

    #[must_use]
    pub fn next_word(&self, offset: usize) -> usize {
        if self.masked {
            return self.buffer.len();
        }
        self.buffer
            .text()
            .split_word_bound_indices()
            .find_map(|(index, word)| {
                let end = index + word.len();
                (end > offset && word.chars().any(char::is_alphanumeric)).then_some(end)
            })
            .unwrap_or(self.buffer.len())
    }

    #[must_use]
    pub fn word_at(&self, offset: usize) -> Range<usize> {
        if self.masked {
            return 0..self.buffer.len();
        }
        self.buffer
            .text()
            .split_word_bound_indices()
            .map(|(index, word)| index..index + word.len())
            .find(|range| {
                range.contains(&offset) || (range.end == offset && offset == self.buffer.len())
            })
            .unwrap_or(offset..offset)
    }

    /// The physical line around `offset`, excluding the newline.
    #[must_use]
    pub fn line_bounds(&self, offset: usize) -> Range<usize> {
        let text = self.buffer.text();
        let start = text[..offset.min(text.len())]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        let end = text[offset.min(text.len())..]
            .find('\n')
            .map_or(text.len(), |index| offset.min(text.len()) + index);
        start..end
    }

    /// The paragraph around `offset`: lines separated by a blank line.
    #[must_use]
    pub fn paragraph_bounds(&self, offset: usize) -> Range<usize> {
        let text = self.buffer.text();
        let start = text[..offset.min(text.len())]
            .rfind("\n\n")
            .map_or(0, |index| index + 2);
        let end = text[offset.min(text.len())..]
            .find("\n\n")
            .map_or(text.len(), |index| offset.min(text.len()) + index);
        start..end
    }

    // --- platform text services -------------------------------------------

    #[must_use]
    pub fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8 = 0;
        let mut utf16 = 0;
        for ch in self.buffer.text().chars() {
            if utf16 >= offset {
                break;
            }
            utf16 += ch.len_utf16();
            utf8 += ch.len_utf8();
        }
        utf8
    }

    #[must_use]
    pub fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf16 = 0;
        let mut utf8 = 0;
        for ch in self.buffer.text().chars() {
            if utf8 >= offset {
                break;
            }
            utf8 += ch.len_utf8();
            utf16 += ch.len_utf16();
        }
        utf16
    }

    #[must_use]
    pub fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    #[must_use]
    pub fn range_from_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range.start)..self.offset_from_utf16(range.end)
    }

    // --- marked text ------------------------------------------------------

    #[must_use]
    pub fn marked_range(&self) -> Option<Range<usize>> {
        self.marked.clone()
    }

    /// Set the composition range. The platform passes a UTF-16 range; the
    /// caller converts it with `range_from_utf16` first.
    pub fn set_marked(&mut self, range: Option<Range<usize>>) {
        self.marked = range;
    }

    pub fn clear_marked(&mut self) {
        self.marked = None;
    }

    /// Replace the marked range as part of an IME composition. The caret goes
    /// to the end of the new composition and the range stays marked.
    pub fn replace_marked(&mut self, text: &str) -> Option<Change> {
        let range = self.marked.clone().unwrap_or_else(|| self.selected_range());
        let Some(change) = self.replace_range(range, text, true) else {
            self.marked = Some(self.cursor()..self.cursor());
            return None;
        };
        self.marked = Some(change.inserted_range());
        Some(change)
    }

    /// Commit any marked text; the composition range becomes ordinary text.
    pub fn unmark(&mut self) {
        self.marked = None;
    }

    // --- display ----------------------------------------------------------

    /// Display bytes for the current masked state. A masked buffer shows one
    /// bullet per grapheme; callers convert offsets with `display_offset` and
    /// `content_offset`.
    #[must_use]
    pub fn display_text(&self) -> String {
        if self.masked {
            "\u{2022}".repeat(self.buffer.text().graphemes(true).count())
        } else {
            self.buffer.text().to_string()
        }
    }

    #[must_use]
    pub fn display_offset(&self, offset: usize) -> usize {
        if !self.masked {
            return offset;
        }
        self.buffer
            .text()
            .grapheme_indices(true)
            .take_while(|(index, _)| *index < offset)
            .count()
            * '\u{2022}'.len_utf8()
    }

    #[must_use]
    pub fn content_offset(&self, display: usize) -> usize {
        if !self.masked {
            return display;
        }
        let count = display / '\u{2022}'.len_utf8();
        self.buffer
            .text()
            .grapheme_indices(true)
            .nth(count)
            .map(|(index, _)| index)
            .unwrap_or(self.buffer.len())
    }

    /// Text a clipboard copy may expose. A masked session exposes nothing.
    #[must_use]
    pub fn copy_text(&self) -> Option<String> {
        if self.masked {
            return None;
        }
        Some(self.buffer.text()[self.selected_range()].to_string())
    }
}

/// Round `offset` down to a grapheme boundary so a caret can never land inside
/// a grapheme (including surrogate-pair emoji).
#[must_use]
pub fn floor_boundary(text: &str, offset: usize) -> usize {
    let offset = offset.min(text.len());
    for (index, grapheme) in text.grapheme_indices(true) {
        if index + grapheme.len() > offset {
            return index;
        }
    }
    text.len()
}

/// Caret blink state. The view owns the timer; this only tracks whether the
/// caret is visible and which focus epoch restarted it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaretBlink {
    pub on: bool,
    pub epoch: u64,
}

impl Default for CaretBlink {
    fn default() -> Self {
        Self { on: true, epoch: 0 }
    }
}

impl CaretBlink {
    pub fn restart(&mut self, epoch: u64) {
        self.on = true;
        self.epoch = epoch;
    }

    pub fn stop(&mut self, epoch: u64) {
        self.on = false;
        self.epoch = epoch;
    }
}

/// Clipboard policy differences between a single-line field and a multiline
/// area. Text widgets read it when they copy, cut and paste.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClipboardPolicy {
    pub flatten_newlines: bool,
    pub masked: bool,
}

impl ClipboardPolicy {
    #[must_use]
    pub const fn field(masked: bool) -> Self {
        Self {
            flatten_newlines: true,
            masked,
        }
    }

    #[must_use]
    pub const fn area() -> Self {
        Self {
            flatten_newlines: false,
            masked: false,
        }
    }

    /// Pasted text after the policy. A field joins lines with a space; an area
    /// keeps them.
    #[must_use]
    pub fn pasted(self, text: &str) -> String {
        if self.flatten_newlines {
            text.replace(['\r', '\n'], " ")
        } else {
            text.replace("\r\n", "\n").replace('\r', "\n")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caret_step_moves_by_grapheme_not_char() {
        let mut session = EditSession::with_text("a👩‍👩‍👧‍👦b");
        session.move_to(0);
        let next = session.next_boundary(0);
        assert_eq!(&session.text()[0..next], "a");
        session.move_to(next);
        let family_end = session.next_boundary(session.cursor());
        assert_eq!(&session.text()[next..family_end], "👩‍👩‍👧‍👦");
        session.move_to(family_end);
        assert_eq!(
            session.next_boundary(session.cursor()),
            session.len(),
            "the caret never lands inside the family emoji"
        );
        assert_eq!(floor_boundary(session.text(), next + 1), next);
        assert_eq!(floor_boundary(session.text(), family_end - 1), next);
        assert_eq!(floor_boundary(session.text(), family_end), family_end);
    }

    #[test]
    fn selection_anchor_and_head_survive_reversal() {
        let mut session = EditSession::with_text("hello world");
        session.select_range(6..11);
        assert_eq!(session.selected_range(), 6..11);
        assert!(!session.selection().is_reversed());
        session.set_selection(Selection::new(11, 6));
        assert!(session.selection().is_reversed());
        assert_eq!(session.cursor(), 6);
        assert_eq!(session.selected_range(), 6..11);
    }

    #[test]
    fn utf16_mapping_handles_surrogate_pairs() {
        let session = EditSession::with_text("a😀b");
        assert_eq!(session.offset_to_utf16(0), 0);
        assert_eq!(session.offset_to_utf16(1), 1);
        assert_eq!(session.offset_to_utf16(5), 3);
        assert_eq!(session.offset_from_utf16(3), 5);
        assert_eq!(session.range_to_utf16(&(1..5)), 1..3);
        assert_eq!(session.range_from_utf16(&(1..3)), 1..5);
    }

    #[test]
    fn replace_selection_reports_the_change_and_the_caret() {
        let mut session = EditSession::with_text("hello");
        session.select_range(1..4);
        let change = session.replace_selection("i").expect("change");
        assert_eq!(
            change,
            Change {
                range: 1..4,
                inserted: "i".into(),
                composing: false
            }
        );
        assert_eq!(session.text(), "hio");
        assert_eq!(session.cursor(), 2);
        assert_eq!(change.inserted_range(), 1..2);
        assert_eq!(change.kind(), EditKind::Replace);
    }

    #[test]
    fn backspace_over_a_line_break_removes_one_grapheme() {
        let mut session = EditSession::with_text("one\ntwo");
        session.move_to(4);
        session.backspace();
        assert_eq!(session.text(), "onetwo");
        assert_eq!(session.cursor(), 3);
    }

    #[test]
    fn marked_text_replaces_the_composition_range() {
        let mut session = EditSession::with_text("ab");
        session.move_to(2);
        let first = session.replace_marked("に").expect("composition");
        assert_eq!(session.text(), "abに");
        assert_eq!(session.marked_range(), Some(2..5));
        assert_eq!(first.kind(), EditKind::Compose);
        session.replace_marked("にほ");
        assert_eq!(session.text(), "abにほ");
        assert_eq!(session.marked_range(), Some(2..8));
        session.unmark();
        assert_eq!(session.marked_range(), None);
        assert_eq!(session.text(), "abにほ");
    }

    #[test]
    fn masked_sessions_hide_words_copy_and_boundaries() {
        let mut session = EditSession::with_text("secret");
        session.set_masked(true);
        assert_eq!(session.display_text(), "••••••");
        assert_eq!(session.copy_text(), None);
        assert_eq!(session.previous_word(4), 0);
        assert_eq!(session.next_word(2), 6);
        assert_eq!(session.word_at(3), 0..6);
        assert_eq!(session.display_offset(3), 9, "three bullets of three bytes");
        assert_eq!(session.content_offset(9), 3);
        assert_eq!(session.content_offset(3), 1);
        session.set_masked(false);
        session.select_all();
        assert_eq!(session.copy_text().as_deref(), Some("secret"));
    }

    #[test]
    fn clipboard_policy_flattens_a_field_and_keeps_an_area() {
        assert_eq!(ClipboardPolicy::field(false).pasted("a\nb"), "a b");
        assert_eq!(ClipboardPolicy::area().pasted("a\r\nb"), "a\nb");
    }

    #[test]
    fn line_and_paragraph_bounds_are_physical() {
        let session = EditSession::with_text("one\ntwo\n\nfour");
        assert_eq!(session.line_bounds(5), 4..7);
        assert_eq!(session.paragraph_bounds(5), 0..7);
        assert_eq!(session.paragraph_bounds(10), 9..13);
    }
}
