//! Test-only observers for line inspection and continuation copying.
//! Keep the underlying collections private: read and copy operations must
//! contribute to the same bound as emitted text and executed instructions.

use super::Continuation;
use crate::record_render_work;

#[derive(Default)]
pub(super) struct LineBuffer {
    text: String,
}

impl LineBuffer {
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn len(&self) -> usize {
        self.text.len()
    }

    pub(super) fn push_str(&mut self, text: &str) {
        // Renderer::append already observes these characters.
        self.text.push_str(text);
    }

    pub(super) fn truncate(&mut self, bytes: usize) {
        self.text.truncate(bytes);
    }

    pub(super) fn chars(&self) -> impl DoubleEndedIterator<Item = char> + '_ {
        InspectedChars(self.text.chars())
    }
}

impl Clone for LineBuffer {
    fn clone(&self) -> Self {
        record_render_work(self.text.len());
        Self {
            text: self.text.clone(),
        }
    }
}

impl From<LineBuffer> for String {
    fn from(line: LineBuffer) -> Self {
        // Moving a completed line does not copy or inspect its contents.
        line.text
    }
}

struct InspectedChars<'a>(std::str::Chars<'a>);

impl Iterator for InspectedChars<'_> {
    type Item = char;

    fn next(&mut self) -> Option<char> {
        let c = self.0.next()?;
        record_render_work(1);
        Some(c)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl DoubleEndedIterator for InspectedChars<'_> {
    fn next_back(&mut self) -> Option<char> {
        let c = self.0.next_back()?;
        record_render_work(1);
        Some(c)
    }
}

#[derive(Default)]
pub(super) struct Continuations<'a> {
    entries: Vec<Continuation<'a>>,
}

impl<'a> Continuations<'a> {
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(super) fn push(&mut self, entry: Continuation<'a>) {
        record_render_work(1);
        self.entries.push(entry);
    }

    pub(super) fn iter(&self) -> impl DoubleEndedIterator<Item = &Continuation<'a>> {
        self.entries.iter().inspect(|_| record_render_work(1))
    }
}

impl Clone for Continuations<'_> {
    fn clone(&self) -> Self {
        record_render_work(self.entries.len());
        Self {
            entries: self.entries.clone(),
        }
    }
}

impl<'a> std::ops::Index<usize> for Continuations<'a> {
    type Output = Continuation<'a>;

    fn index(&self, index: usize) -> &Self::Output {
        record_render_work(1);
        &self.entries[index]
    }
}

#[cfg(test)]
mod tests {
    use super::super::Instruction;
    use super::*;

    fn work() -> usize {
        crate::RENDER_WORK.with(std::cell::Cell::get)
    }

    fn reset_work() {
        crate::RENDER_WORK.with(|work| work.set(0));
    }

    fn continuations(count: usize) -> Continuations<'static> {
        let mut entries = Continuations::new();
        for i in 0..count {
            entries.push(Continuation {
                instruction: Instruction::Text("entry"),
                next: i.checked_sub(1),
            });
        }
        entries
    }

    fn line(text: &str) -> LineBuffer {
        let mut line = LineBuffer::new();
        line.push_str(text);
        line
    }

    #[test]
    fn arena_copies_charge_one_unit_per_entry_for_every_copy() {
        for count in [0, 1, 2, 5, 17, 100] {
            let entries = continuations(count);
            reset_work();
            let first = entries.clone();
            assert_eq!(work(), count, "one copy of {count} entries");
            assert_eq!(first.len(), count);
            let second = entries.clone();
            let third = entries.clone();
            assert_eq!(work(), 3 * count, "three copies of {count} entries");
            assert_eq!((second.len(), third.len()), (count, count));
        }
    }

    #[test]
    fn forward_traversal_charges_each_entry_as_it_is_read() {
        for count in [1, 2, 5, 17] {
            let entries = continuations(count);
            reset_work();
            let mut entries_iter = entries.iter();
            assert_eq!(work(), 0, "creating the traversal reads no entry");
            for read in 1..=count {
                assert!(entries_iter.next().is_some());
                assert_eq!(work(), read, "after {read} of {count} forward reads");
            }
            assert!(entries_iter.next().is_none());
            assert_eq!(work(), count, "the end of the traversal reads no entry");
        }
    }

    #[test]
    fn backward_traversal_charges_each_entry_as_it_is_read() {
        for count in [1, 2, 5, 17] {
            let entries = continuations(count);
            reset_work();
            let mut entries_iter = entries.iter();
            for read in 1..=count {
                let entry = entries_iter.next_back().expect("an entry remains");
                assert_eq!(entry.next, (count - read).checked_sub(1));
                assert_eq!(work(), read, "after {read} of {count} backward reads");
            }
            assert!(entries_iter.next_back().is_none());
            assert_eq!(work(), count, "the end of the traversal reads no entry");
        }
    }

    #[test]
    fn partial_and_mixed_traversal_charge_only_the_entries_read() {
        let entries = continuations(10);
        reset_work();
        assert_eq!(entries.iter().take(3).count(), 3);
        assert_eq!(work(), 3, "a partial forward traversal");
        reset_work();
        assert_eq!(entries.iter().rev().take(4).count(), 4);
        assert_eq!(work(), 4, "a partial backward traversal");
        reset_work();
        let mut entries_iter = entries.iter();
        entries_iter.next();
        entries_iter.next_back();
        entries_iter.next();
        assert_eq!(work(), 3, "alternating reads from both ends");
    }

    #[test]
    fn line_copies_charge_bytes_for_every_copy() {
        for text in ["", "a", "abc", "aé界", "界界界界界界", &"x".repeat(1000)] {
            let buffer = line(text);
            reset_work();
            let first = buffer.clone();
            assert_eq!(work(), text.len(), "one copy of {text:?}");
            let second = buffer.clone();
            let third = buffer.clone();
            assert_eq!(work(), 3 * text.len(), "three copies of {text:?}");
            for copy in [first, second, third] {
                assert_eq!(copy.text, text);
            }
        }
    }

    #[test]
    fn line_traversal_keeps_character_identity_and_order() {
        let text = "aé界😀z";
        let buffer = line(text);
        reset_work();
        assert_eq!(buffer.chars().collect::<String>(), text);
        assert_eq!(work(), 5);
        reset_work();
        assert_eq!(
            buffer.chars().rev().collect::<Vec<_>>(),
            ['z', '😀', '界', 'é', 'a']
        );
        assert_eq!(work(), 5);
        reset_work();
        let mut chars = buffer.chars();
        assert_eq!(chars.next_back(), Some('z'));
        assert_eq!(chars.next(), Some('a'));
        assert_eq!(chars.next_back(), Some('😀'));
        assert_eq!(chars.next(), Some('é'));
        assert_eq!(chars.next_back(), Some('界'));
        assert_eq!((chars.next(), chars.next_back()), (None, None));
        assert_eq!(work(), 5, "each character is charged once");
    }

    #[test]
    fn converting_a_completed_line_moves_its_allocation() {
        let mut text = String::with_capacity(64);
        text.push_str("aé界");
        let pointer = text.as_ptr();
        let capacity = text.capacity();
        let buffer = LineBuffer { text };
        reset_work();
        let moved = String::from(buffer);
        assert_eq!(moved, "aé界");
        assert_eq!(moved.as_ptr(), pointer, "the conversion copied the line");
        assert_eq!(moved.capacity(), capacity);
        assert_eq!(work(), 0, "a move neither copies nor inspects the line");
    }
}
