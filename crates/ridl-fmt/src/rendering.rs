//! Incremental physical-line rendering with reusable continuations.
//!
//! Once a line is complete, later instructions cannot change its width or its
//! last candidate. Commit it when it needs no break. Otherwise resume at the
//! last inline construct on that line, retaining the prefix and continuation.
//! Continuations are persistent arena entries, so saving one never copies the
//! pending siblings of a large tuple. Output is assembled only for committed
//! lines; rejected inline suffixes are discarded locally.

use std::collections::HashSet;

use super::{FormatOptions, Layout, indent_str};

#[cfg(test)]
mod work;
#[cfg(test)]
use work::{Continuations, LineBuffer};
#[cfg(not(test))]
type Continuations<'a> = Vec<Continuation<'a>>;
#[cfg(not(test))]
type LineBuffer = String;

type Cursor = Option<usize>;

#[derive(Clone, Copy)]
enum Instruction<'a> {
    Layout(&'a Layout, bool),
    Text(&'a str),
    Spaces(usize),
    Comment(&'a str),
    LineBreak,
}

#[cfg_attr(test, derive(Clone))]
struct Continuation<'a> {
    instruction: Instruction<'a>,
    next: Cursor,
}

#[derive(Clone, Copy, Default)]
struct LineState {
    columns: usize,
    code_columns: Option<usize>,
    indent: usize,
    has_code: bool,
}

struct Candidate {
    group: usize,
    cursor: Cursor,
    bytes: usize,
    state: LineState,
    previous: Cursor,
}

struct Renderer<'a> {
    pending: Cursor,
    continuations: Continuations<'a>,
    candidates: Vec<Candidate>,
    last_candidate: Cursor,
    broken: HashSet<usize>,
    line: LineBuffer,
    state: LineState,
    lines: Vec<String>,
    width: Option<usize>,
}

impl<'a> Renderer<'a> {
    fn prepend(&mut self, instruction: Instruction<'a>) {
        let next = self.pending;
        self.pending = Some(self.continuations.len());
        self.continuations.push(Continuation { instruction, next });
    }

    fn prepend_all(&mut self, instructions: impl DoubleEndedIterator<Item = Instruction<'a>>) {
        for instruction in instructions.rev() {
            self.prepend(instruction);
        }
    }

    fn append(&mut self, text: &str) {
        debug_assert!(!text.contains('\n'));
        #[cfg(test)]
        super::record_render_work(text.chars().count());
        for c in text.chars() {
            self.state.columns += 1;
            if !self.state.has_code {
                if c == ' ' {
                    self.state.indent += 1;
                } else {
                    self.state.has_code = true;
                }
            }
        }
        self.line.push_str(text);
    }

    // Return false after rewinding. The caller must discard its speculative
    // instruction and continue from the restored continuation.
    fn finish_line(&mut self) -> bool {
        if let (Some(width), Some(candidate)) = (self.width, self.last_candidate)
            && self.state.code_columns.unwrap_or(self.state.columns) > width
        {
            let candidate = &self.candidates[candidate];
            self.broken.insert(candidate.group);
            self.pending = candidate.cursor;
            self.line.truncate(candidate.bytes);
            self.state = candidate.state;
            self.last_candidate = candidate.previous;
            return false;
        }
        let line = std::mem::take(&mut self.line);
        #[cfg(test)]
        let line = String::from(line);
        self.lines.push(line);
        self.state = LineState::default();
        self.last_candidate = None;
        true
    }

    fn text(&mut self, text: &'a str) {
        if let Some((line, rest)) = text.split_once('\n') {
            self.append(line);
            self.prepend(Instruction::Text(rest));
            self.finish_line();
        } else {
            self.append(text);
        }
    }

    fn layout(&mut self, layout: &'a Layout, inside_inline: bool, cursor: Cursor) {
        use Instruction::{Comment, Layout as Visit, LineBreak, Spaces, Text};
        match layout {
            Layout::Text(text) => self.text(text),
            Layout::TrailingComment(text) => self.prepend(Comment(text)),
            Layout::LineBreak => {
                self.prepend(Spaces(self.state.indent));
                self.prepend(LineBreak);
            }
            Layout::Concat(parts) => {
                self.prepend_all(parts.iter().map(|part| Visit(part, inside_inline)));
            }
            Layout::Tuple(items) => {
                let broken = self.group(layout, !items.is_empty(), inside_inline, cursor);
                let indent = self.state.indent;
                let mut instructions = vec![Text("(")];
                for (i, item) in items.iter().enumerate() {
                    if broken {
                        instructions.extend([LineBreak, Spaces(indent + 2)]);
                    } else if i > 0 {
                        instructions.push(Text(", "));
                    }
                    instructions.push(Visit(item, !broken));
                    if broken && i + 1 < items.len() {
                        instructions.push(Text(","));
                    }
                }
                if broken {
                    instructions.extend([LineBreak, Spaces(indent)]);
                }
                instructions.push(Text(")"));
                self.prepend_all(instructions.into_iter());
            }
            Layout::Shapes(items) => {
                let broken = self.group(layout, !items.is_empty(), inside_inline, cursor);
                let indent = self.state.indent;
                let mut instructions = Vec::new();
                if !broken {
                    instructions.push(Text(" "));
                }
                for (i, item) in items.iter().enumerate() {
                    if broken {
                        instructions.extend([LineBreak, Spaces(indent + 2)]);
                    } else if i > 0 {
                        instructions.push(Text(", "));
                    }
                    instructions.push(Text(item));
                    if broken && i + 1 < items.len() {
                        instructions.push(Text(","));
                    }
                }
                self.prepend_all(instructions.into_iter());
            }
            Layout::Attributes {
                blocks,
                force_block,
            } => {
                // Forced blocks are always expanded, including inside an inline
                // ancestor. Only their member layouts become candidates.
                let broken =
                    *force_block || self.group(layout, !blocks.is_empty(), inside_inline, cursor);
                let indent = self.state.indent;
                let mut instructions = vec![Text(if broken { "[" } else { "[ " })];
                for (index, block) in blocks.iter().enumerate() {
                    if broken {
                        if index > 0 && block.gap_blank {
                            instructions.push(LineBreak);
                        }
                        for (i, comment) in block.leading.iter().enumerate() {
                            if i > 0 && comment.blank_before {
                                instructions.push(LineBreak);
                            }
                            instructions.extend([
                                LineBreak,
                                Spaces(indent + 2),
                                Text(&comment.text),
                            ]);
                        }
                        if block.blank_before_node {
                            instructions.push(LineBreak);
                        }
                        if let Some(item) = &block.layout {
                            instructions.extend([
                                LineBreak,
                                Spaces(indent + 2),
                                Visit(item, false),
                            ]);
                        }
                        instructions.extend(block.trailing.iter().map(|text| Comment(text)));
                    } else {
                        if index > 0 {
                            instructions.push(Text(", "));
                        }
                        if let Some(item) = &block.layout {
                            instructions.push(Visit(item, true));
                        }
                    }
                }
                if broken {
                    instructions.extend([LineBreak, Spaces(indent), Text("]")]);
                } else {
                    instructions.push(Text(" ]"));
                }
                self.prepend_all(instructions.into_iter());
            }
        }
    }

    fn group(
        &mut self,
        layout: &Layout,
        nonempty: bool,
        inside_inline: bool,
        cursor: Cursor,
    ) -> bool {
        // Layout references remain stable for the lifetime of this rendering.
        // The address supplies identity only; it is never dereferenced.
        let group = std::ptr::from_ref(layout) as usize;
        if self.broken.contains(&group) {
            return true;
        }
        if nonempty && !inside_inline && self.width.is_some() {
            let previous = self.last_candidate;
            self.last_candidate = Some(self.candidates.len());
            self.candidates.push(Candidate {
                group,
                cursor,
                bytes: self.line.len(),
                state: self.state,
                previous,
            });
        }
        false
    }

    fn run(mut self) -> Vec<String> {
        loop {
            if let Some(cursor) = self.pending {
                #[cfg(test)]
                super::record_render_work(1);
                let entry = &self.continuations[cursor];
                let instruction = entry.instruction;
                self.pending = entry.next;
                match instruction {
                    Instruction::Layout(layout, inline) => {
                        self.layout(layout, inline, Some(cursor))
                    }
                    Instruction::Text(text) => self.text(text),
                    Instruction::Spaces(count) => self.append(&" ".repeat(count)),
                    Instruction::Comment(comment) => {
                        self.state.code_columns.get_or_insert(self.state.columns);
                        self.append(" ");
                        self.prepend(Instruction::Text(comment));
                    }
                    Instruction::LineBreak => {
                        self.finish_line();
                    }
                }
            } else if self.finish_line() {
                return self.lines;
            }
        }
    }
}

pub(super) fn render(layout: &Layout, indent: usize, options: &FormatOptions) -> Vec<String> {
    let mut renderer = Renderer {
        pending: None,
        continuations: Continuations::new(),
        candidates: Vec::new(),
        last_candidate: None,
        broken: HashSet::new(),
        line: LineBuffer::new(),
        state: LineState::default(),
        lines: Vec::new(),
        width: options.max_line_length,
    };
    renderer.append(&indent_str(indent));
    renderer.prepend(Instruction::Layout(layout, false));
    renderer.run()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_renderer() -> Renderer<'static> {
        Renderer {
            pending: None,
            continuations: Default::default(),
            candidates: Vec::new(),
            last_candidate: None,
            broken: HashSet::new(),
            line: Default::default(),
            state: LineState::default(),
            lines: Vec::new(),
            width: Some(100),
        }
    }

    #[test]
    fn line_inspections_accumulate_rendering_work() {
        let mut renderer = empty_renderer();
        renderer.append("abc");
        super::super::RENDER_WORK.with(|work| work.set(0));
        assert_eq!(renderer.line.chars().count(), 3);
        assert_eq!(renderer.line.chars().count(), 3);
        assert_eq!(renderer.line.chars().rev().count(), 3);
        assert_eq!(renderer.line.chars().nth(2), Some('c'));
        assert!(
            super::super::RENDER_WORK.with(std::cell::Cell::get) >= 12,
            "repeated line inspections must contribute to the work bound"
        );
    }

    #[test]
    fn line_copies_contribute_to_rendering_work() {
        let mut renderer = empty_renderer();
        renderer.append("abc");
        super::super::RENDER_WORK.with(|work| work.set(0));
        let copied = renderer.line.clone();
        assert!(
            super::super::RENDER_WORK.with(std::cell::Cell::get) >= 3,
            "copied line bytes must contribute to the work bound"
        );
        assert_eq!(copied.chars().collect::<String>(), "abc");
    }

    #[test]
    fn indexed_continuation_reads_accumulate_work_and_keep_identity() {
        let mut renderer = empty_renderer();
        renderer.prepend(Instruction::Text("first"));
        renderer.prepend(Instruction::Text("second"));
        super::super::RENDER_WORK.with(|work| work.set(0));
        assert!(matches!(
            renderer.continuations[1].instruction,
            Instruction::Text("second")
        ));
        assert_eq!(renderer.continuations[0].next, None);
        assert_eq!(renderer.continuations[1].next, Some(0));
        assert!(
            super::super::RENDER_WORK.with(std::cell::Cell::get) >= 3,
            "repeated indexed reads must contribute to the work bound"
        );
    }

    #[test]
    fn arena_copies_contribute_to_rendering_work() {
        let mut renderer = empty_renderer();
        renderer.prepend(Instruction::Text("first"));
        renderer.prepend(Instruction::Text("second"));
        super::super::RENDER_WORK.with(|work| work.set(0));
        let copied = renderer.continuations.clone();
        assert!(
            super::super::RENDER_WORK.with(std::cell::Cell::get) >= 2,
            "arena copies must contribute to the work bound"
        );
        assert!(matches!(copied[0].instruction, Instruction::Text("first")));
        assert_eq!(copied[0].next, None);
        assert!(matches!(copied[1].instruction, Instruction::Text("second")));
        assert_eq!(copied[1].next, Some(0));
    }

    #[test]
    fn copying_continuation_entries_contributes_to_rendering_work() {
        let mut renderer = empty_renderer();
        renderer.prepend(Instruction::Text("first"));
        renderer.prepend(Instruction::Text("second"));
        super::super::RENDER_WORK.with(|work| work.set(0));
        let copied: Vec<_> = renderer
            .continuations
            .iter()
            .map(|entry| (entry.instruction, entry.next))
            .collect();
        assert_eq!(copied.len(), 2);
        assert!(matches!(copied[0].0, Instruction::Text("first")));
        assert_eq!(copied[0].1, None);
        assert!(matches!(copied[1].0, Instruction::Text("second")));
        assert_eq!(copied[1].1, Some(0));
        assert!(
            super::super::RENDER_WORK.with(std::cell::Cell::get) >= 2,
            "copied continuation entries must contribute to the work bound"
        );
    }
}
