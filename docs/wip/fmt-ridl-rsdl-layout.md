# `ridl fmt` layout for the ridl and rsdl declarations

Status: design note, written 2026-10-01 for driftsys/ridl#387. It fixes the
canonical layout `ridl fmt` gives the seven declarations it currently emits as
written — ridl `interface` and `service`, and rsdl `system`, `component`,
`distribution`, `deployment` and `machine`. Nothing here is implemented. The
decisions in §7 belong to the maintainer; implementation starts after they are
taken.

Part of #387.

## 1. Scope and the model this extends

`crates/ridl-fmt/src/lib.rs` formats the typl declarations and lays out every
file: the header block, one blank line between declarations, comments
re-anchored to what they precede. A node it has no rule for takes the fallback
arm of `format_element` (`_ => line(node.text().to_string())`) and is emitted
exactly as written. Seven declarations reach that arm:

| Node kind                                                                  | Body holds (grammar: `crates/ridl-syntax/family.ungram`)                       |
| -------------------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| `InterfaceDef` — `'internal'? 'error'? 'interface' Name '{' … '}'`         | `SignalDef`, `EventDef`, `CommandDef`, `QueryDef`, `FixedDef`, `ReservedEntry` |
| `ServiceDef` — `'service' DottedName (':' shapes \| '{' … '}')`            | a comma-separated `PathType` list, or the interface members above              |
| `SystemDef` — `'system' Name AttrBlock? '{' … '}'`                         | `MemberLine` = `Reference AttrBlock?`                                          |
| `ComponentDef` — `'component' Name AttrBlock? '{' … '}'`                   | `ComponentLine` = `('offers' \| 'requires') Reference AttrBlock?`              |
| `DistributionDef` — `'distribution' Name AttrBlock? '{' … '}'`             | `MemberLine`                                                                   |
| `DeploymentDef` — `'deployment' Name 'for' Reference AttrBlock? '{' … '}'` | `MachineDef`                                                                   |
| `MachineDef` — `'machine' Name AttrBlock? '{' … '}'` (inside a deployment) | `MemberLine`                                                                   |

Inside the interface members the grammar allows these pieces, and each one needs
a rendering: a payload `FieldType` (named type, primitive with constraint,
tuple, array, map, optional), an `InitValue` (`= value`, signals only in the
checker, every value interaction in the parser), a `Timing` (`@` duration, or
`@[` `TimingRange` `]` with one or both bounds), a `ParamList` of
`Param = Name ':' (FieldType | StreamType)`, a `ReturnType` (`PathType`,
`TupleType`, `StreamType`, `FallibleType = PathType '|' PathType`), and an
`AttrBlock` of `Attribute`s — flag `key`, assignment `key = AttrValue`, or
predicate `require`/`ensure` `Expr` — where an `AttrValue` is a `Literal` or a
parenthesised list of `AttrValue`s and an `Expr` is one of `BinaryExpr`,
`PrefixExpr`, `MemberExpr`, `PathExpr`, `ParenExpr`, `LiteralExpr`. The parser
accepts at most one `Timing` and at most one `AttrBlock` on every interaction,
in either order; which kinds take which is checker scope.

In rsdl the pieces are a `Reference` (a `QualifiedName`), the `for` clause, and
the same `AttrBlock`, where a key may be dotted (`linux.cpuset`) and a value or
a list item may be a bare name (`instances = (primary, backup)`).

The note extends the model the typl rules use — one `layout_container` pass over
a body, `format_block_def` for a brace block, token stitching for a single-line
node, verbatim fallback for a node that carries a comment among its own tokens —
rather than adding a second layout engine. Every rule below is a rendering
function for one node kind, or a reuse of one that exists.

## 2. Rules that carry over unchanged

These rules are already in force for the typl declarations and apply to the
seven declarations with no change. The citation is the paragraph of
`docs/wip/family-general-form.md` §5 (general form §5 below) or the module
documentation of `crates/ridl-fmt/src/lib.rs` (the crate doc below) that states
the rule.

- **Tight colon** — `name: Type`, no space before, one after, in every position:
  declarations, parameters, tuple fields, map types, returns and relations
  (general form §5, opening paragraph).
- **No column alignment** anywhere (general form §5, heading sentence; item 4,
  the diff argument).
- **One blank line between top-level declarations**, none at the start of the
  file, one trailing newline, the `package` and `import` lines contiguous (crate
  doc).
- **Inside braces, newline separators are canonical**: separator commas are
  removed, one member per line, two-space indentation per level, no trailing
  comma (crate doc; typl reference §15.2 and general form R8 make newline and
  comma interchangeable, so removing a comma changes nothing).
- **A blank line inside a body appears only where the source had one**, and a
  run of blank lines collapses to one (crate doc, `blanks_between`). The §5
  example groups the signals, the event, the callables and the fixed with blank
  lines, which this rule preserves.
- **An empty body renders as `{}`** on the header line (`format_block_def`).
- **Atoms are tight**: a qualified name, a literal, a duration, a length bound
  (`tight_text`).
- **Collections and tuples**: `[T; 8]`, `[K: V; 0..32]`, `(min: A, max: B)` —
  semicolon then space, comma then space, tight brackets and parentheses (crate
  doc). A constraint on a primitive payload is `[0.0..250.0 step 0.5]` (crate
  doc).
- **An initialiser** `= value` has one space on each side of `=` (crate doc).
- **Modifiers** (`internal`, `error`) are kept in source order, one space after
  each (`modifiers_prefix`).
- **Source order is never changed**: no declaration, member, parameter,
  attribute or list item moves (crate doc, "What order is not changed"). The one
  exception this note proposes is the annotation pair of §3.2, decision D-4.
- **Comments are never dropped**; the placement rules are in §5 of this note
  (crate doc, "Comments are never dropped").

## 3. The ridl declarations

### 3.1 `interface`

An `interface` is a brace block and takes `format_block_def`: the header
`modifiers interface Name`, one space before `{`, the members at the next
indent, `}` at the header's indent. A member is one line, except a member whose
attribute block takes the block form (§3.2). One space follows the member
keyword — the book's `signal  temperature` and `command setLevel` padding is
column alignment and goes.

Before (the style of `docs/book/getting-started.md` and
`examples/cabin/cabin.ridl` today):

```ridl,ignore
interface VehicleStatus {
  signal  currentSpeed : Speed @10ms
  signal  targetSpeed  : Speed = SPEED_LIMIT_EU @[20ms..500ms]
  event   doorOpened   : DoorPayload @[50ms..500ms]

  command setGear(position : GearPosition) [
    require position != GearPosition.PARK || currentSpeed == 0.0
  ] @[..50ms]
  query   getSpeedHistory(window: Duration) : (min: Speed, max: Speed) @[..100ms]
  query   calibrate(axle: Axle): CalReport|CalError @[..5s]
  query   streamFaults(filter: DiagFilter): <FaultCode> @[..1s]
  command uploadFirmware(data: <FwBlock>) @[..1s]
  fixed   capabilities : [Label; 0..32]
  reserved legacyChecksum      // was ordinal 2
}
```

After:

```ridl,ignore
interface VehicleStatus {
  signal currentSpeed: Speed @10ms
  signal targetSpeed: Speed = SPEED_LIMIT_EU @[20ms..500ms]
  event doorOpened: DoorPayload @[50ms..500ms]

  command setGear(position: GearPosition) @[..50ms] [
    require position != GearPosition.PARK || currentSpeed == 0.0
  ]
  query getSpeedHistory(window: Duration): (min: Speed, max: Speed) @[..100ms]
  query calibrate(axle: Axle): CalReport | CalError @[..5s]
  query streamFaults(filter: DiagFilter): <FaultCode> @[..1s]
  command uploadFirmware(data: <FwBlock>) @[..1s]
  fixed capabilities: [Label; 0..32]
  reserved legacyChecksum // was ordinal 2
}
```

The member forms and their canonical line:

| Member     | Grammar                                                  | Canonical rendering                                 |
| ---------- | -------------------------------------------------------- | --------------------------------------------------- |
| `signal`   | `'signal' Name ':' FieldType InitValue? annotations`     | `signal name: Type = INIT @timing [ attrs ]`        |
| `event`    | `'event' Name ':' FieldType annotations`                 | `event name: Type @timing [ attrs ]`                |
| `fixed`    | `'fixed' Name ':' FieldType annotations`                 | `fixed name: Type @timing [ attrs ]`                |
| `command`  | `'command' Name ParamList (':' ReturnType)? annotations` | `command name(p: P, q: Q) @timing [ attrs ]`        |
| `query`    | `'query' Name ParamList ':' ReturnType annotations`      | `query name(p: P): Return @timing [ attrs ]`        |
| `reserved` | `'reserved' (Name \| Literal)`                           | `reserved name` — `format_reserved_entry`, as today |

Every slot after the name is optional except where the grammar requires it, and
absent slots are skipped with no extra space. An empty interface renders as
`interface Empty {}`; `internal interface Hidden {}` keeps its modifier.
Separator commas between members are removed, as in a `struct`.

### 3.2 The interaction pieces

**Payload and parameter types.** A `FieldType` renders through
`format_field_type`, unchanged: `Speed`, `integer [0..100]`, `(min: A, max: B)`,
`[Label; 0..32]`, `[Label: Speed; 1..8]`, `Speed?`. A stream is tight:
`<FaultCode>`, `<bytes>`.

**Parameter list.** `(` tight to the name, parameters joined by a comma and a
space, `)` tight, `()` when empty: `setRange(min: Speed, max: Speed)`. The list
always renders on one line, trailing comma removed, separator commas normalised.

**Return type.** A colon and a space, then one of the four shapes, with one
space on each side of `|` (D-8):

```ridl,ignore
query a(): Speed
query b(): (min: Speed, max: Speed)
query c(): <LogLine>
query d(): CalReport | CalError
```

**Init value.** `= SPEED_LIMIT_EU`, one space on each side of `=`, between the
type and the annotations — the ADR-0008 decision 2 position the parser fixes.

**Timing.** One space before `@`, then the annotation with no internal spacing:
`@10ms`, `@[20ms..100ms]`, `@[..5s]`, `@[20ms..]`. A range written with spaces
around `..` or inside the brackets normalises to that, as a constraint's `..`
does today.

**Annotation order.** The parser accepts the timing and the attribute block in
either order. The canonical order is timing first, then the attribute block —
general form R5's sentence order, `@timing → [ attrs ]` (D-4). With the block
form of the attribute block that puts `]` last on the member, so a member ends
where its contract ends.

**Attribute block, inline form.** One space, the opening bracket, a space, the
attributes joined by a comma and a space, a space, the closing bracket. The
padding inside the brackets is D-3. A block whose attributes are all flags or
assignments takes this form:

```ridl,ignore
signal targetSpeed: Speed [ seed = SPEED_LIMIT_EU, persist ]
query getState(): DriveState [ labels = (SIL_2, PRIVATE) ]
```

**Attribute block, block form.** A block that holds at least one predicate
(`require` or `ensure`) takes the block form (D-2): `[` ends the member's line,
one attribute per line at the next indent, no separator commas, `]` on its own
line at the member's indent — the general form §4.4 rendering, "ordinary block
formatting".

```ridl,ignore
query getAverageSpeed(window: Duration): Speed @[..100ms] [
  require window > 0ms
  ensure result >= 0.0
]
```

One space follows `require` and `ensure` (D-5). The §5 and §4.4 examples write
`ensure  result` with two spaces, which aligns the two expressions and
contradicts the heading rule of §5; this note takes the rule.

**Attribute forms.** A flag is its key: `persist`, `linux.realtime` (a dotted
key is tight). An assignment is `key = value`, spaced. A value is a tight
literal or name, or a list: `(` items joined by a comma and a space `)`, nested
lists the same, `()` kept as written, trailing comma removed:
`instances = (primary, backup)`, `linux.cpuset = (2, 3)`.

**Expressions** (the predicate operand) take a canonical spacing (D-6): one
space on each side of a binary operator (`||`, `&&`, `==`, `!=`, `<`, `<=`, `>`,
`>=`, `+`, `-`, `*`, `/`, `%`); a prefix operator tight to its operand
(`!engaged`, `-1.0`); a member access tight (`result.min`, `GearPosition.PARK`);
parentheses tight to what they enclose and kept exactly where the author wrote
them — the formatter never adds or removes a pair; literals and durations as
written. An expression never breaks across lines.
`require position!=GearPosition.PARK||currentSpeed==0.0` renders as
`require position != GearPosition.PARK || currentSpeed == 0.0`.

### 3.3 `service`

**Named form.** `service` then the dotted name tight, then the tight colon of
the relation (general form §5, "relations"), then the shapes joined by a comma
and a space on one line, the optional trailing comma removed (D-7). The commas
are required by the grammar (ADR-0015 decision 13), so they stay — this is the
one list whose commas the formatter writes rather than removes.

```ridl,ignore
service veh.adas.cruise      : CruiseControl
service veh.body.doors       : DoorControl, DiagBlock
service veh.body.composite :
  DoorControl,
  MotorControl,
```

```ridl,ignore
service veh.adas.cruise: CruiseControl
service veh.body.doors: DoorControl, DiagBlock
service veh.body.composite: DoorControl, MotorControl
```

**Inline form.** The body is an interface body and takes §3.1 unchanged:
`service veh.hvac.cabin {`, the members at the next indent, `}`. A service takes
no modifier.

```ridl,ignore
service veh.hvac.cabin {
  signal temperature: Temperature @[1s..10s]
  command setTarget(t: Temperature)
}
```

A trailing comment after a named form's list (`service a.b: X // note`) is a
token outside the `ServiceDef` node, so the container pass keeps it on the line,
as it does for any top-level declaration.

## 4. The rsdl declarations

### 4.1 `system`, `distribution` and `machine`

The three are brace blocks of bare member lines and take `format_block_def`:
`system Vehicle [ attrs ] {`, one member per line at the next indent, `}`. The
attribute block sits between the name and the brace, one space on each side
(general form §4.4). A member line is its reference, tight, then one space and
the inline attribute block when it has one. Separator commas are removed and the
body is always one member per line (D-1) — the typl rule for every brace body,
applied to a body the reference writes on one line today.

Before (rsdl reference §3.1, §3.3, Appendix A):

```rsdl,ignore
system Vehicle { Cruise, Lane, Panel, Backend, veh.diag.access }

distribution Adas [ tier = PLATFORM ]    { Cruise, Lane, veh.diag.access }
distribution Hmi  [ tier = APPLICATION ] { Panel }
```

After:

```rsdl,ignore
system Vehicle {
  Cruise
  Lane
  Panel
  Backend
  veh.diag.access
}

distribution Adas [ tier = PLATFORM ] {
  Cruise
  Lane
  veh.diag.access
}

distribution Hmi [ tier = APPLICATION ] {
  Panel
}
```

A member line with attributes: `Panel [ linux.cpuset = (2, 3) ]`,
`veh.topology.Cruise.primary [ linux.cpuset = (2) ]`. An empty body is `{}`. A
`machine` body is laid out the same way; the machine's own position inside a
deployment is §4.3.

### 4.2 `component`

A `component` is the same brace block with keyword-plus-reference lines: the
keyword, one space, the reference, then the inline attribute block when the line
has one. `offers` and `requires` are not padded to a common width — the
alignment the issue shows is the one general form §5 forbids.

Before (the issue's example, rsdl reference §3.2):

```rsdl,ignore
component Cruise [ instances = (primary, backup) ] {
  offers   veh.adas.cruise
  requires LaneAssist
}
component Lane    { offers veh.adas.lane }
component Panel   { requires CruiseControl, requires LaneAssist }
component Backend [ external ] { requires CruiseControl }
```

After:

```rsdl,ignore
component Cruise [ instances = (primary, backup) ] {
  offers veh.adas.cruise
  requires LaneAssist
}

component Lane {
  offers veh.adas.lane
}

component Panel {
  requires CruiseControl
  requires LaneAssist
}

component Backend [ external ] {
  requires CruiseControl
}
```

A line with attributes: `offers veh.adas.cruise [ someip.instanceId = 1 ]`,
`requires veh.adas.LaneAssist [ dds.reliable ]`. A component with no lines
renders `component Solo [ instances = solo ] {}`. The blank line between the
declarations is the file rule of §2, not a body rule.

### 4.3 `deployment`

A `deployment` header is `deployment Name for System [ attrs ] {` — the clause
before the attributes (general form R5), single spaces throughout, the system
reference tight (`for veh.topology.Vehicle`). Its body holds `machine` blocks,
each a nested brace block at the next indent: the header
`machine Name [ attrs ] {`, the member lines one further indent in, `}` at the
machine's indent. This is the first nested block the formatter lays out;
`format_block_def` already takes an indent, so a machine is the same call one
level down. Blank lines between machines are preserved where the source had one,
as between any two members (D-11).

Before (rsdl reference §3.4, Appendix A):

```rsdl,ignore
deployment Production for Vehicle {
  machine AdasHpc [ labels = (ASIL_B) ] { Cruise.primary, Lane, veh.diag.access }
  machine Cockpit { Cruise.backup, Panel [ linux.cpuset = (2, 3) ] }
  machine Cloud   [ external ] { Backend }
}
```

After:

```rsdl,ignore
deployment Production for Vehicle {
  machine AdasHpc [ labels = (ASIL_B) ] {
    Cruise.primary
    Lane
    veh.diag.access
  }
  machine Cockpit {
    Cruise.backup
    Panel [ linux.cpuset = (2, 3) ]
  }
  machine Cloud [ external ] {
    Backend
  }
}
```

### 4.4 The attribute block

An rsdl attribute block holds flags and assignments only — no rsdl key takes the
predicate form (rsdl reference §5) — so under D-2 it always takes the inline
form, on a declaration and on a line alike, whatever its length. A multi-line
block in the source is joined; its separator commas become a comma and a space
and the trailing comma goes. The dotted backend key is tight.

Before (`crates/ridl-syntax/test_data/parser/ok/rsdl_attribute_positions.rsdl`):

```rsdl,ignore
component Cruise [
  instances = (primary, backup,),
  deprecated = "use Cruise2",
  rust.crate = "cruise",
  someip.serviceId = 4660,
  linux.realtime,
] {
  /// The service this component offers.
  offers veh.adas.cruise [ someip.instanceId = 1 ],
  requires veh.adas.LaneAssist [ dds.reliable ],
}
```

After:

```rsdl,ignore
component Cruise [ instances = (primary, backup), deprecated = "use Cruise2", rust.crate = "cruise", someip.serviceId = 4660, linux.realtime ] {
  /// The service this component offers.
  offers veh.adas.cruise [ someip.instanceId = 1 ]
  requires veh.adas.LaneAssist [ dds.reliable ]
}
```

The 134-column header is the cost of D-2's recommendation; D-2's alternative (b)
keeps that block multi-line because the author broke it, and D-9 is where a
column limit would be decided. A declaration's block in the block form would
read `component Cruise [`, one attribute per line, `] {` — the `{` on the
closing bracket's line — if (b) is taken.

## 5. Comments

The existing placement rules apply as they are; the new node kinds add two
positions.

- A comment or doc comment before a declaration or a member leads it, at the
  member's indent; a blank line before the comment is kept (the container pass).
- An inline trailing comment stays on its line, one space after the member; a
  comment on the opening-brace line stays on that line
  (`split_brace_line_comment`).
- A comment inside the header region of a block (between the keyword and `{`)
  emits the header verbatim (`block_header_prefix`) — the `for` clause and the
  attribute block of an rsdl declaration included.
- A comment among the tokens of a one-line construct — a parameter list, a tuple
  return, an inline attribute block, an attribute value list, a timing range, an
  expression, a service's shape list — emits that construct verbatim, as a
  constraint or a collection type is today. A line comment there keeps its
  newline, so the construct spans two lines and the output is still a fixed
  point.
- **New:** inside a block-form attribute block, a comment between two attributes
  is laid out like a comment between two members, at the attributes' indent; a
  comment inside one attribute's tokens emits that attribute verbatim.
- **New:** a comment between a `machine` block's `}` and the next `machine` is a
  between-member comment of the deployment body.
- A comment run with no member after it (the last thing in a body) stays at the
  end of the body (`BlockKind::CommentOnly`).

Nothing in this note moves a comment to a different member, and no rule drops
one; §8 states the test.

## 6. Where each rule comes from

| Rule                                                                | Source                                                                        |
| ------------------------------------------------------------------- | ----------------------------------------------------------------------------- |
| tight colon on members, parameters, returns, the service list       | general form §5, opening paragraph ("every position … relations")             |
| one space after a member keyword, `offers`, `requires`              | general form §5, heading ("no column alignment"); the §5 example              |
| one member per line, commas removed, two-space indent               | crate doc; general form R8 makes the rewrite meaning-preserving; D-1 for rsdl |
| blank lines inside a body preserved, collapsed to one               | crate doc (`blanks_between`); the §5 example's grouping                       |
| one blank line between declarations                                 | crate doc                                                                     |
| `{}` for an empty body                                              | `format_block_def`                                                            |
| `= INIT` spaced, before the timing                                  | crate doc; ADR-0008 decision 2 for the position                               |
| `@10ms`, `@[20ms..100ms]` tight, one space before `@`               | the §5 example; the constraint rule of the crate doc for `..`                 |
| timing before the attribute block                                   | general form R5 (`@timing → [ attrs ]`); **D-4**                              |
| inline block `[ a, b ]`, padded                                     | the §5 example and every example of general form §4; **D-3**                  |
| block form for a predicate, `[` on the member line, `]` alone       | general form §4.4 ("ordinary block formatting"); the §5 example; **D-2**      |
| one space after `require` / `ensure`                                | general form §5 heading over its own example; **D-5**                         |
| attribute value list `(a, b)`, nested, trailing comma removed       | the tuple rule of the crate doc                                               |
| expression spacing                                                  | **D-6** (new)                                                                 |
| `T \| E` spaced                                                     | general form §2 Shape 2 and §6.1 examples; **D-8**                            |
| `<T>` tight                                                         | atoms tight (`tight_text`); every ridl reference §12 example                  |
| parameter list `(p: P, q: Q)`, one line                             | the tuple rule of the crate doc; **D-9** (no column limit)                    |
| service list on one line, comma-space separators, no trailing comma | ADR-0015 decision 13 for the commas; **D-7** for the line                     |
| `deployment Name for System [ attrs ] {`                            | general form R5 (clause before attributes), §4.4 (attributes before `{`)      |
| nested `machine` blocks, blank lines preserved                      | `format_block_def` one level down; **D-11**                                   |
| comment placement                                                   | crate doc ("Comments are never dropped"); the two new positions in §5         |

## 7. Decisions for the maintainer

General form §5 settles the colon and forbids alignment; it says nothing about
the choices below. Each one is open until the maintainer takes it, and the
implementation starts from the taken set.

**D-1 — rsdl bodies: one member per line, always?** Options: (a) always one
member per line, as every typl brace body is today; (b) a body the source wrote
on one line (`{` and `}` on the same line) stays on one line, with comma-space
separators, and a body the source broke takes one member per line; (c) a column
limit decides. Recommendation: (a). It is the rule the formatter already has,
general form §5 item 4 argues for it (one line per member gives the smallest
diff when a member is added), and rustfmt expands a struct definition the same
way. The cost is visible: every one-line `system`, `distribution`, `component`
and `machine` in the references, the book and the parser corpus expands, and the
existing test
`an_rsdl_file_takes_the_file_layout_and_keeps_its_declarations_as_written` is
rewritten to the new layout.

**D-2 — when does an attribute block take the block form?** Options: (a) by kind
— a block with at least one predicate is block form, every other block is
inline; (b) by source — a line break between `[` and the first attribute makes
it block form; (c) by column limit. Recommendation: (a). It matches every
example in the general form, the two references and the book, needs no
source-sensitivity beyond the blank-line rule the formatter already has, and
gives one rendering per declaration. The cost is §4.4's 134-column header for a
block of five backend keys.

**D-3 — inline attribute block padding: `[ a, b ]` or `[a, b]`?**
Recommendation: padded. Every example in the family writes it so, and the
padding is what tells an attribute block apart from a typl constraint `[0..1]`
at a glance (general form R3 distinguishes them by position only).

**D-4 — the timing and the attribute block: normalise to the R5 order, or keep
the source order?** Options: (a) emit the timing first, then the attribute
block, whatever the source order; (b) keep the source order. Recommendation:
(a). General form R5 makes `@timing → [ attrs ]` normative and the parser is
lenient, so the formatter would emit the conforming order rather than invent
one, and the block form reads better with `]` last. Two costs: the crate doc's
"whitespace and separators only" sentence gains an exception, and the structural
test of §8 compares the two annotation nodes as a set rather than in order.
Erratum either way: ridl reference Appendix C writes `attr_block? timing?` for
`command` and `query`, the opposite of R5, and `docs/book/getting-started.md`
has two members in that order.

**D-5 — one space after `require` and `ensure`, or the two-space alignment of
the §5 and §4.4 examples?** Recommendation: one space. The heading rule of §5
forbids alignment; the two examples are amended when the sweep (D-10) reaches
them.

**D-6 — expression layout: canonical spacing, or the source text with whitespace
runs collapsed?** Recommendation: canonical spacing as §3.2 states — binary
operators spaced, prefix and member access tight, parentheses kept. A verbatim
rule would leave `a<b` and `a < b` as two renderings of one expression. The
renderer is one function over the six expression node kinds; it never
reassociates and never touches parentheses.

**D-7 — the service shape list: always one line?** Options: (a) one line,
trailing comma removed; (b) keep a list the source broke after `:` on one shape
per line, each with its required comma. Recommendation: (a). A list is a short
set of interface names, and (b) would be the only place the formatter keeps a
trailing comma.

**D-8 — the fallible return: `T | E` or `T|E`?** Recommendation: spaced. The
general form §2 Shape 2 and §6.1 examples write it spaced; the `T|E` spelling
appears only in ADR-0008's prose.

**D-9 — a column limit?** Options: (a) none — a parameter list, a tuple, a shape
list, an inline attribute block and an attribute value list always render on one
line; (b) a limit such as 100 columns, with a breaking rule per construct.
Recommendation: (a) now. The formatter has no width logic, and a width rule is a
design of its own (where each construct breaks, and how it re-joins). Reopen
when a real file produces a line a reviewer objects to.

**D-10 — the scope of the reformatting sweep.** General form §5's errata note
says the examples are reformatted "mechanically once `ridl fmt` exists".
Options: (a) the test fixtures and `examples/` only; (b) also every verified
fence in `docs/book/`; (c) also the fences of the two language references and
the general form. Recommendation: (b) in the implementation pull request, (c) as
a follow-up pull request of its own. The book is what a reader copies, and the
fixed-point test of §10 then holds it to the canonical style; the references are
long and the §5 note already allows them to be touched opportunistically.

**D-11 — blank lines between `machine` blocks inside a deployment.** Options:
(a) preserved where the source had one, as between any two members; (b) always
one, as between top-level declarations. Recommendation: (a), the existing body
rule; a `machine` is a member.

## 8. Invariants and how the tests check them

The four invariants are the crate's existing ones; the tests extend the existing
harnesses to the two new profiles.

1. **Total.** Every input that parses with no error formats; an input with a
   parse error is returned as `FormatOutcome::ParseErrors`, untouched. Test: the
   `broken_input_returns_parse_errors_untouched` pattern over one `.ridl` input
   (an unclosed interface body) and one `.rsdl` input (two references on one
   `requires` line, FORM-102), plus the whole `err` parser corpus of both
   profiles — every file yields `ParseErrors`.
2. **Idempotent.** `format(format(x)) == format(x)`. Test:
   `formatting_is_idempotent_over_the_ok_corpus` in
   `crates/ridl-fmt/tests/properties.rs`, extended from `.typl` files to the
   `.ridl` and `.rsdl` files of `crates/ridl-syntax/test_data/parser/ok`, each
   parsed under the profile of its extension; the same check over every fixture
   §10 lists.
3. **No comment lost.** Every comment token of the input is in the output, in
   order, with the same text (trailing whitespace trimmed). Test:
   `content_tokens` in `properties.rs` already compares every non-whitespace,
   non-comma token including comments; it runs over the extended corpus. Under
   D-4 the comparison is made after moving a `Timing` node's tokens in front of
   its sibling `AttrBlock`'s in both streams, so a reordered pair compares equal
   and a dropped comment still fails.
4. **The parse tree's meaning is unchanged.** Test: parse the input and the
   output, walk both trees with `descendants_with_tokens`, and compare the event
   streams — node kind on entry and exit, token kind and text — ignoring trivia
   and `Comma` tokens. That stream is the structure the checker reads: every
   node, every non-separator token, in order. Under D-4 the `Timing`/`AttrBlock`
   pair is normalised to R5 order in both streams before the comparison, as in
   invariant 3. The existing `source_order_is_never_changed` unit test gains an
   interface and an rsdl variant.

A mutation check closes the loop on 3 and 4: a test that deletes one comment
token and one member from the formatted output must fail the comparison
(`content_tokens_detect_a_dropped_comment_and_a_mutated_literal` is the model).

## 9. Alternatives considered

- **Keep the fallback arm.** Rejected: it is what #387 reports, and the issue's
  example shows the alignment general form §5 forbids surviving a format.
- **A width-driven layout, as rustfmt.** Rejected for now (D-9): no width logic
  exists in the crate, every construct would need a breaking rule, and the
  family's lines are short in every example. It can be added later without
  changing any rule here, because every rule here is a one-line rendering that a
  width rule would break at defined points.
- **Source-sensitive line breaking, as prettier does for object literals.**
  Rejected as the default (D-1 (b), D-2 (b)): the typl declarations are not
  formatted that way, and two authors would then produce two canonical forms of
  one declaration. Kept as the named alternative of both decisions.
- **A separate layout engine for rsdl.** Rejected: the five rsdl declarations
  are the family's container shape (general form §2, Shape 3), and
  `format_block_def` plus `layout_container` already lay out that shape; only
  the header and the line renderers are new.
- **Verbatim expressions.** Rejected (D-6): a formatter that leaves `a<b` alone
  is not canonical for the one construct where authors vary the most.
- **Reformatting nothing outside the fixtures.** Rejected (D-10): the book would
  then show the style the formatter rewrites, and the §5 errata note asks for
  the sweep.

## 10. Test plan

**Unit tests** in `crates/ridl-fmt/src/lib.rs`, one per rule, each a
before/after string pair taken from §3 and §4 of this note, including: every
member kind of an interface; each of the four return shapes; a stream parameter;
an init value; the four timing spellings; the inline and the block-form
attribute block; the annotation pair in both source orders (D-4); an expression
with every operator class; the two service forms, with and without a trailing
comma; each of the five rsdl declarations; a line with attributes; a dotted key;
nested value lists; a nested `machine` with and without blank lines; an empty
body of each kind; a comment in each of the §5 positions; `internal interface`.

**Golden corpus** (`crates/ridl-fmt/tests/corpus.rs`): `test_data/input` and
`test_data/formatted` gain `.ridl` and `.rsdl` pairs — the §3.1 interface in the
book's aligned style, the two service forms, the issue's `component`, Appendix A
of the rsdl reference, and `rsdl_attribute_positions.rsdl`. The harness picks
the profile from the extension instead of listing `.typl` only. Every golden is
a fixed point.

**Properties** (`crates/ridl-fmt/tests/properties.rs`): both tests run over the
`.ridl` and `.rsdl` files of the parser `ok` corpus, and the §8 structure
comparison is added beside `content_tokens`.

**Reference examples** (`crates/ridl-fmt/tests/rsdl.rs`): the existing
idempotence-and-tokens test over the rsdl reference's §3 and Appendix A fences
stays; the as-written test is replaced by the §4 renderings. The ridl
reference's Appendix A gets the same treatment in a `ridl.rs` sibling.

**Round trips.** Each of these formats idempotently and keeps its structure (§8,
invariants 2 to 4), and after the D-10 sweep each is a fixed point:
`examples/cabin/cabin.ridl`; `crates/ridl/tests/baseline-corpus/cluster.ridl`
(the desk check compares `.ir.json` snapshots, so a whitespace change there is
safe); every verified `ridl`, `typl` and `rsdl` fence of `docs/book/`, extracted
with the `fenced_blocks` function of `crates/ridl/tests/book_examples.rs` — the
same `pulldown-cmark` options mdBook uses, so the set is exactly the set the
compile harness verifies. A fence marked `ignore` is skipped; a fence with an
`allow=` marker still round trips, because the formatter does not read
diagnostics.

**The book harness still passes.** `book_examples_compile` runs `ridl check`
over the staged fences; the sweep changes whitespace, separators and (under D-4)
the annotation order only, none of which a diagnostic reads, and the `allow=`
markers are untouched. `just demo` compiles `examples/cabin` the same way. Both
run in `just build`.

**The gate.** No new `just` recipe: the fixed-point tests over `examples/` and
the book fences are the gate that keeps the examples canonical, and `just test`
already runs them.

## Trace links

- Satisfies: driftsys/ridl#387 (this note is the design; the implementation
  closes the issue).
- Extends: `family-general-form.md` §5 (the formatting decision), §4.4
  (attribute placement and the block form), R5 (the sentence order), R8
  (separator discipline); `crates/ridl-fmt/src/lib.rs` module documentation (the
  rules in force).
- Related: ADR-0015 decision 13 (required commas in a service list); ADR-0008
  decision 2 (the init position); typl reference §15.2 (separators); rsdl
  reference §3 to §5 and Appendix A; ridl reference §14 and Appendix C (its
  annotation order is the erratum D-4 names); roadmap story E6.15, where #387
  was found.
- Disposition: when the implementation lands, the decisions of §7 are gardened
  into the crate documentation and a dated amendment of general form §5, and
  this note moves to `docs/archive/`.
