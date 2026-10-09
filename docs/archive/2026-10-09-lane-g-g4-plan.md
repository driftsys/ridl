# Lane G stage G4 plan: formatter, LSP positions, rsdl lowering order

Branch `fix/fmt-lsp-lowering-order`. One task per issue.

## Pre-flight read (against origin/main 376aa927)

- #657: the crate documentation (`crates/ridl-fmt/src/lib.rs`, lines 8-9 and
  82-84) already states the behaviour: a comment after a machine's closing brace
  belongs to the deployment body, with or without a separator comma. The code
  (`can_trail = node.kind() != MachineDef`) and the pinned test agree with it.
  The one statement left that contradicts the behaviour is
  `docs/wip/fmt-ridl-rsdl-layout.md` section 5, "Nothing in this note moves a
  comment to a different member". Defect in the issue text: it quotes crate
  documentation that no longer says "an inline trailing comment stays on its
  line" without the machine exception.
- #728: the issue is stale. `RegionInterface` entries are already sorted with
  `interfaces.sort_by_key(|interface| interface.number)` in
  `crates/ridl-sem/src/rsdl/lower.rs`, and the test
  `the_routing_table_is_sorted_by_interface_number_and_not_by_name` (added by
  29af5733, 2026-10-05) pins a region whose number order is the reverse of its
  name order. Number order is fixed by ADR-0022 (rsdl system in the IR, line
  136), `docs/design/codegen-plugins.md` (line 411) and
  `docs/technotes/rsdl-implementation.md` (line 100).
- #623: the specification (typl reference section 2.2) says "LF and CRLF are
  both accepted" and lists CR as whitespace; it does not say whether a bare CR
  ends a line. ADR-0002 and the codegen header normalisation treat a lone CR as
  a line break for the header file only. Not settled: escalated.

## Task 1: #657 (documentation only)

Files: `docs/wip/fmt-ridl-rsdl-layout.md` section 5. Decision: the code and the
crate documentation are correct; the comment is a between-member comment of the
deployment body, on its own line, in source order. Fix: state the exception in
the sentence that claims otherwise. Red test: none possible (prose). Existing
tests `rsdl_comments_after_machine_separators_stay_between_members` and
`rsdl_machine_gap_comments_ignore_optional_commas` pin the behaviour; the
reviewer applies the mutation "allow trailing after a MachineDef" and confirms
both fail.

## Task 2: #623 (waiting for a ruling)

## Task 3: #728 (no change)

Mutations applied during pre-flight, both caught by the existing test: sort by
name; no sort. No code change; the issue is closed by the main session.
