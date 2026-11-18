# The canonical layout of `ridl fmt`

The layout that `ridl fmt` gives the seven declarations of the ridl and rsdl
profiles — ridl `interface` and `service`; rsdl `system`, `component`,
`distribution`, `deployment` and `machine` — as built in `crates/ridl-fmt`: the
shape of each declaration, the line width and the breaking rules, the treatment
of comments, and where the width comes from. The code and its tests are the
source of truth; where this record and the code differ, the record is corrected.

The crate documentation of `crates/ridl-fmt/src/lib.rs` owns the rules that hold
for every declaration — the tight colon, no column alignment, one blank line
between top-level declarations, newline separators inside braces, source order
never changed, comments never dropped, and the lint-directive rule. This record
does not restate them. It owns the layout of the seven declarations, which no
other record states. General form §5
([`../specification/family-general-form.md`](../specification/family-general-form.md))
fixes the tight colon and the ban on alignment that every rule below builds on.
The design note that fixed these rules, with the alternatives it weighed for
each of the thirteen decisions, is archived verbatim in
[`../archive/fmt-ridl-rsdl-layout.md`](../archive/fmt-ridl-rsdl-layout.md); the
decision log and the review of the work that built them are beside it. The
decisions are numbered D-1 to D-13 here as in that note, and each is stated next
to the rule it fixes with its reason in one line. The work is tracked in
driftsys/ridl#387.

## Model

The formatter parses with `ridl_syntax::parse` under the profile of the file
extension and returns `FormatOutcome::ParseErrors`, with the input untouched,
when the parse has any error. Otherwise it lays out the lossless syntax tree: a
brace block is one header line, its members at the next indent and `}` at the
header's indent; a one-line construct is rebuilt from its tokens; a construct
that holds a comment among its own tokens is emitted from the source text (see
Comments). There is one layout engine for the typl, ridl and rsdl profiles; the
seven declarations add header and member-line renderers, not a second engine.
Indentation is two spaces per level and the output never holds a tab.

## The ridl declarations

### `interface`

The header is `modifiers interface Name {`, with `internal` and `error` in
source order and one space after each. An empty body is `interface Name {}`.
Members are one per line, in source order, with one space after the member
keyword. The canonical member is

    signal name: Type = INIT @timing [ attrs ]
    command name(p: P, q: Q): Return @timing [ attrs ]

with every slot after the name optional. The slots follow the parser, which is
wider than the grammar file: it accepts a stream payload, an initialiser, a
timing and an attribute block on every value interaction and leaves the
narrowing to the checker, so the formatter renders every slot on every member
kind (`signal`, `event`, `fixed`, `command`, `query`). `reserved name` and
`reserved 3` render as written.

- **Payload and parameter types** render as in typl: `Speed`,
  `integer [0..100]`, `[Label: Speed; 1..8]`, `Speed?`, tuples
  `(min: Speed, max: Speed)`. A stream type is tight, `<FaultCode>`, in every
  type position, including a struct field and a collection element.
- **Parameter list**: `(` tight to the name, items joined by a comma and a
  space, `()` when empty.
- **Return**: a colon and a space, then a named type, a tuple, a stream, or a
  fallible pair with one space on each side of `|` (D-8: the general form writes
  `T | E` spaced; the unspaced form occurs only in an ADR's prose).
- **Initialiser**: `= value` between the type and the annotations (ADR-0008
  decision 2).
- **Timing**: one space before `@`, then no inner spacing: `@10ms`,
  `@[20ms..100ms]`, `@[..5s]`, `@[20ms..]`.
- **Annotation order** (D-4): the timing comes first and the attribute block
  last, whatever the source order, because general form R5 writes
  `@timing → [ attrs ]` and the parser accepts both orders; with the block form
  the member ends on `]`. This is the only place the formatter reorders tokens.
  Two comment cases keep a member's meaning. An inline comment between the two
  annotations stays with the annotation before it in the source, so
  `query q(): T [persist] /* note */ @10ms` becomes
  `query q(): T @10ms [ persist ] /* note */`. When moving a line comment would
  collide with a trailing comment of the member, the whole member stays as
  written, source order included.

### The attribute block

- **Inline form** (D-3): `[ a, b = 1 ]`, padded by one space inside the
  brackets, attributes joined by a comma and a space. The padding tells it apart
  from a typl constraint such as `[0..1]`.
- **Block form** (D-2): `[` ends the member's line, one attribute per line at
  the next indent with no separator comma, `]` alone at the member's indent. A
  block takes this form when it holds a `require` or `ensure` predicate, or when
  its inline form would exceed the width; every other block is inline. The
  decision is by kind and by width, not by the source's line breaks, so one
  declaration has one rendering.
- **Attributes**: a flag is its key, and a dotted key is tight
  (`linux.realtime`); an assignment is `key = value`; a list value is `(a, b)`,
  nested lists the same, `()` kept, a trailing comma removed.
- **`require` and `ensure`** take one space before the expression (D-5: the
  heading rule forbids the two-space alignment that some older examples show).
- **Expressions** (D-6): one space on each side of a binary operator, a prefix
  operator tight to its operand, a member access tight, and parentheses tight to
  their content and kept exactly where the author wrote them. The formatter
  never adds, removes or reassociates a parenthesis, and an expression never
  breaks across lines, so a predicate longer than the width stays on one line.
  Without a canonical spacing, `a<b` and `a < b` would be two renderings of one
  expression.

### `service`

- **Named form**: `service veh.body.doors: DoorControl, DiagBlock` — the dotted
  name tight, the tight colon, the shapes joined by a comma and a space on one
  line, and the optional trailing comma removed (D-7: a list laid out from the
  tree, not from the source's line breaks, so there is one rendering). The
  commas between shapes are required by the grammar (ADR-0015 decision 13), so
  this is the one list whose separators the formatter keeps. When the line
  exceeds the width, the list breaks after the colon, one shape per line at the
  next indent, each followed by a comma except the last.
- **Inline form**: `service veh.hvac.cabin {` with an interface body, laid out
  as an interface. A service takes no modifier.

## The rsdl declarations

`system`, `distribution`, `component`, `deployment` and `machine` are brace
blocks. Their attribute block sits between the name and the brace, one space on
each side: `system Vehicle [ attrs ] {`.

- **Bodies** (D-1): always one member per line, separator commas removed, as in
  every typl brace body — the smallest diff when a member is added, and the rule
  the formatter already had. The width never reads a body, so a one-line
  `system Vehicle { Cruise, Lane }` always expands. An empty body is `{}`.
- **`system`, `distribution`, `machine`**: a member line is its reference, tight
  (`veh.diag.access`), then the inline attribute block when it has one:
  `Panel [ linux.cpuset = (2, 3) ]`.
- **`component`**: lines are `offers Reference` and `requires Reference` with
  one space after the keyword and no padding to a common width, then the inline
  attribute block when present.
- **`deployment`**: the header is `deployment Name for System [ attrs ] {`, the
  clause before the attributes (general form R5), the system reference tight.
  Its body holds `machine` blocks, each a nested brace block one level down:
  `machine Name [ attrs ] {`, its members two levels in, `}` at the machine's
  indent. A blank line between two machines is kept where the source had one,
  and a run of blank lines collapses to one (D-11: a machine is a member, not a
  top-level declaration).
- **Attribute blocks**: flags and assignments only, so they take the inline form
  when the line fits and the block form when it does not. An overlong header
  keeps its opening brace on the closing bracket's line: `component Cruise [`,
  one attribute per line, `] {`. A source block that spans several lines is
  joined when it fits, and its separator commas are replaced.

## Line width and breaking

- **Width** (D-9): a line of output holds at most 100 characters by default,
  where a character is one Unicode scalar value and the indentation counts.
  `FormatOptions::max_line_length` is `Some(100)` by default and `None` for no
  limit. Only line breaks read the width; spacing, order and comment placement
  are the same at every width.
- **A trailing comment does not count**, and never causes a break, because
  breaking the code would move the comment off what it annotates. An unbreakable
  token longer than the width (a string literal, a qualified name, a duration)
  stays as it is, draws no diagnostic, and `ridl fmt --check` accepts the
  result, which is a fixed point.
- **Algorithm**: every construct is laid out on one line first. When a line
  exceeds the width, its breakable constructs break one at a time, the last on
  the line first, and the lines are measured again after each break; the loop
  stops when every line fits or none is left. A construct nested in another
  breaks only after the outer one. The layout is a function of the syntax tree
  and the width only, so it is idempotent.
- **Breakable constructs**: a parameter list, a tuple type (a return, a field, a
  collection element), an attribute value list, an attribute block (to its block
  form), and the shape list of a named service. A broken `(` … `)` list ends the
  line with its opener and puts one item per line at the next indent with a
  comma after every item but the last (D-13: a parenthesised list is a list of
  items, which Rust, Kotlin and TypeScript write with commas; a brace body and
  an attribute block list declarations, which the family writes without commas).
  A tuple inside an array or map type breaks as a tuple; the brackets around it
  do not break. Everything else is unbreakable: a header, a member with no list,
  a timing annotation, a constraint, a fallible pair, a stream type, and an
  expression.
- **Typl declarations** gain the width and nothing else: a tuple type is the one
  breakable construct, so every other typl line is the same at every width.

For example, at a width of 60:

```ridl,ignore
query getSpeedHistory(window: Duration, mode: Mode): (
  min: Speed,
  max: Speed,
  avg: Speed
) @[..100ms] [
  labels = (A, B)
]
```

The attribute block, the last breakable construct, breaks first; the tuple
return still exceeds the width and breaks second; the parameter list then fits
and stays on one line.

## Where the width comes from

The pure entry point `ridl_fmt::format(text, profile, &FormatOptions)` reads no
file and no environment, so it is the same function on every target and builds
for wasm32 without default features. The default feature `editorconfig` adds
`FormatOptions::for_path`, which reads `.editorconfig` through the `ec4rs`
crate: the search from the file's directory upward, every matching `[glob]`
section with later files and sections winning, and a `root = true` file stopping
the search. It reads one key, `max_line_length`. An integer is the width, `off`
removes the limit, and an absent, `unset` or invalid value gives 100.
`indent_size` and `indent_style` are not read (D-12: a canonical style has one
indent, as it has one colon; the output must not depend on a file setting that
the book fences and the golden files cannot carry). The repository's
`.editorconfig` states `indent_size = 2` and `max_line_length = 100` for the
family's files so that editors agree with the formatter.

`ridl fmt` resolves the options per file, because two files of one run can sit
under different `.editorconfig` files. The language server resolves them from
the document's path and ignores the client's `FormattingOptions`, such as
`tabSize`. The crate documentation states the same contract for the library.

## Comments

The crate documentation owns the placement rules at declaration and member
boundaries. The rules below are specific to the seven declarations.

- A comment in the header region of a block, between the keyword and `{` and
  outside any child node (between the name and `[`, or between `for` and the
  system reference), makes the whole header emit from the source text. Without
  such a comment the header is rendered, including the `for` clause and the
  header's attribute block.
- A comment among the tokens of a one-line construct — a parameter list, a tuple
  return, an inline attribute block, an attribute value list, a timing range, an
  expression, a service shape list — emits that construct from the source text.
  A line comment keeps its newline, so the construct spans two lines and the
  output is still a fixed point.
- An attribute block that holds a comment and no predicate is emitted from the
  source text in full, at every width, with its separator commas, its spacing
  and its indentation as written. In a block that holds a predicate, a comment
  between two attributes is laid out like a comment between two members, at the
  attributes' indent; a comment inside one attribute's own tokens keeps that
  attribute as written.
- A comment between a `machine` block's `}` and the next machine, or after the
  `}` on the same line, is a comment between members of the deployment body: it
  leads the next machine on its own line, or stays at the end of the body when
  no machine follows. The same holds for a comment after a separator comma that
  stands on a line of its own.
- A comment on the opening-brace line stays on that line.

## Tests

The tests in `crates/ridl-fmt/tests/` and the unit tests of
`crates/ridl-fmt/src/lib.rs` pin each rule.

- `corpus.rs` formats each input under `test_data/input/` to its golden under
  `test_data/formatted/` (`interface.ridl`, `attributes.ridl`, `services.ridl`,
  `component.rsdl`, `deployment.rsdl`, `rsdl_attribute_positions.rsdl`) and
  checks that every golden is a fixed point.
- `properties.rs` runs over the `ok` parser corpus of all three profiles at
  widths 100, 60 and 40: idempotence, no comment lost, and an unchanged node and
  token stream once the annotation pair is put in timing-first order. Every file
  of the `err` corpus must come back unformatted. A mutation check shows that a
  dropped comment fails the comparison.
- `rsdl.rs` and `ridl.rs` format the reference examples idempotently without
  losing a token; `editorconfig.rs` pins the width resolution; the unit tests
  hold the width boundaries (one less than, equal to and one more than the
  width) for each breakable construct.
