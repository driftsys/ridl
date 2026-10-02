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
