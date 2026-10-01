# `ridl fmt` layout for the ridl and rsdl declarations

Status: design note, written 2026-10-01 for driftsys/ridl#387. It fixes the
canonical layout `ridl fmt` gives the seven declarations it currently emits as
written — ridl `interface` and `service`, and rsdl `system`, `component`,
`distribution`, `deployment` and `machine`. Nothing here is implemented. Updated
2026-10-01: D-9 is decided — the formatter gets a line width — and §6 carries
the breaking rules it needs. Updated the same day: the maintainer took the
recommended option of every other decision, so all thirteen decisions of §8 are
decided and the implementation can start. The implementation plan is
[`fmt-ridl-rsdl-plan.md`](fmt-ridl-rsdl-plan.md).

Part of #387.

## 1. Scope and the model this extends

`crates/ridl-fmt/src/lib.rs` formats the typl declarations and lays out every
file: the header block, one blank line between declarations, comments
re-anchored to what they precede. A node it has no rule for takes the fallback
arm of `format_element` (`_ => line(node.text().to_string())`) and is emitted
exactly as written. Six declarations reach that arm as nodes of their own; the
seventh, `machine`, occurs only inside a `deployment`, so the formatter never
dispatches on it and it is emitted as part of its deployment's text:

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
  exception is the annotation pair of §3.2, decision D-4.
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

The grammar column gives what the parser accepts (`value_interaction` and
`callable_interaction` in `crates/ridl-syntax/src/parser.rs`), which is wider
than `family.ungram`: the parser takes a stream payload, an init value, a timing
and an attribute block on every value interaction and leaves the narrowing to
the checker (RIDL-106, RIDL-201, RIDL-301, and FORM-102 for an init value on an
`event` or `fixed`).
`crates/ridl-syntax/test_data/parser/ok/lenient_overapprox.ridl` holds each of
these shapes. A file that draws only a checker diagnostic has no parse error, so
the formatter formats it, and every slot below needs a rendering.

| Member     | Slots the parser accepts                                             | Canonical rendering                                 |
| ---------- | -------------------------------------------------------------------- | --------------------------------------------------- |
| `signal`   | `'signal' Name ':' (FieldType \| StreamType) InitValue? annotations` | `signal name: Type = INIT @timing [ attrs ]`        |
| `event`    | `'event' Name ':' (FieldType \| StreamType) InitValue? annotations`  | `event name: Type = INIT @timing [ attrs ]`         |
| `fixed`    | `'fixed' Name ':' (FieldType \| StreamType) InitValue? annotations`  | `fixed name: Type = INIT @timing [ attrs ]`         |
| `command`  | `'command' Name ParamList (':' ReturnType)? annotations`             | `command name(p: P, q: Q): R @timing [ attrs ]`     |
| `query`    | `'query' Name ParamList ':' ReturnType annotations`                  | `query name(p: P): Return @timing [ attrs ]`        |
| `reserved` | `'reserved' (Name \| Literal)`                                       | `reserved name` — `format_reserved_entry`, as today |

`annotations` is at most one `Timing` and at most one `AttrBlock`, in either
order. Examples of the lenient slots, as the formatter renders them:
`signal rawFeed: <SensorFrame>`, `event calibrated: CalPayload = DEFAULT_CAL`,
`fixed region: Region = REGION_EU`, `fixed buildId: BuildId @1s`,
`command setMode(mode: DriveMode): AckState`.

Every slot after the name is optional except where the parser requires it, and
absent slots are skipped with no extra space.

**The stream type in a payload position.** A stream payload is a `StreamType`
node where a `FieldType` would stand. `field_type()` in
`crates/ridl-fmt/src/lib.rs` finds the first child whose kind `is_field_type`
accepts, and `is_field_type` does not list `StreamType`, so today it returns an
empty string for a stream. The formatter must render a `StreamType` tight
(`<SensorFrame>`, `<bytes>`) wherever the parser builds one: add `StreamType` to
`is_field_type` and a `StreamType` arm to `format_field_type`, which reaches
every type position at once — a value interaction's payload, a parameter, a
return, and an array or map element. The same gap exists today outside the seven
declarations: in a `.ridl` file the parser accepts a stream as a struct field's
type or as a collection element with no parse error (TYPL-301 is emitted in a
`.typl` parse only), and `ridl fmt` currently rewrites the struct field `a: <T>`
to `a:` and a space, and `[<T>; 1..2]` to `[; 1..2]`, dropping the stream
(reproduced 2026-10-01). The same `is_field_type` change fixes that. An empty
interface renders as `interface Empty {}`; `internal interface Hidden {}` keeps
its modifier. Separator commas between members are removed, as in a `struct`.

### 3.2 The interaction pieces

**Payload and parameter types.** A `FieldType` renders through
`format_field_type`, unchanged: `Speed`, `integer [0..100]`, `(min: A, max: B)`,
`[Label; 0..32]`, `[Label: Speed; 1..8]`, `Speed?`. A stream is tight:
`<FaultCode>`, `<bytes>`.

**Parameter list.** `(` tight to the name, parameters joined by a comma and a
space, `)` tight, `()` when empty: `setRange(min: Speed, max: Speed)`. The list
renders on one line when the line fits the width, one parameter per line
otherwise (§6.3); trailing comma removed, separator commas normalised.

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
attributes joined by a comma and a space, a space, the closing bracket (the
padding is D-3). A block whose attributes are all flags or assignments takes
this form when its line fits the width:

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
and a space on one line when the line fits the width, the optional trailing
comma removed (D-7). A list that does not fit breaks after the colon, one shape
per line at the next indent, each but the last followed by its comma (§6.3). The
commas are required by the grammar (ADR-0015 decision 13), so they stay — this
is the one list whose commas the formatter writes rather than removes.

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
predicate form (rsdl reference §5) — so under D-2 it takes the inline form
whenever its line fits the width, on a declaration and on a line alike, and the
block form when it does not (§6.2). A multi-line block in the source is joined
when it fits; its separator commas become a comma and a space and the trailing
comma goes. The dotted backend key is tight.

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
component Cruise [
  instances = (primary, backup)
  deprecated = "use Cruise2"
  rust.crate = "cruise"
  someip.serviceId = 4660
  linux.realtime
] {
  /// The service this component offers.
  offers veh.adas.cruise [ someip.instanceId = 1 ]
  requires veh.adas.LaneAssist [ dds.reliable ]
}
```

The joined header would be 144 columns (measured: the line
`component Cruise [ instances = (primary, backup), deprecated = "use Cruise2", rust.crate = "cruise", someip.serviceId = 4660, linux.realtime ] {`),
over the default width of 100, so the block takes the block form:
`component Cruise [`, one attribute per line with no commas, and `] {` — the
brace on the closing bracket's line (§6.4). At a width of 144 or more the same
declaration is one header line; at 143 it takes the block form. A block that
fits, such as `component Backend [ external ] {`, stays inline.

## 5. Comments

The existing placement rules apply as they are; the new node kinds add two
positions.

- A comment or doc comment before a declaration or a member leads it, at the
  member's indent; a blank line before the comment is kept (the container pass).
- An inline trailing comment stays on its line, one space after the member; a
  comment on the opening-brace line stays on that line
  (`split_brace_line_comment`).
- A comment that is a direct token of the header region of a block (between the
  keyword and `{`, outside any child node — for example between the name and
  `[`, or between `for` and the system's reference) emits the header verbatim,
  as `block_header_prefix` does today for a typl block. Today
  `block_header_prefix` already includes the text of every direct child node
  (such as the `for` clause and the header's attribute block) in the verbatim
  text, but it detects a comment among direct tokens only, and when there is no
  comment it renders nothing but the modifiers, the keyword and the name; for
  the rsdl declarations it is extended to render the `for` clause and the
  header's attribute block in that case. A comment inside the header's attribute
  block is not a direct token of the declaration: it follows the attribute-block
  rules of the next two bullets, as it does on a member line.
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
one; §9 states the test.

## 6. Line width and line breaking

### 6.1 The rule (D-9, decided 2026-10-01)

A physical line of formatted output holds at most `W` characters, where `W` is
100 by default and the value of `max_line_length` in `.editorconfig` when one
applies to the file (§6.6). `max_line_length = off` removes the limit. A
character is one Unicode scalar value; the indentation counts; the output never
holds a tab.

Three qualifications, each a rule rather than a decision:

- **A trailing comment does not count.** The width applies to the code of a
  line. A comment never causes a break — breaking code to make room for a
  comment would move the comment off the code it annotates, which §5 forbids —
  so a line whose code fits and whose comment runs past `W` is canonical.
- **An unbreakable token longer than the width stays as it is.** A long string
  literal, regex, qualified name or duration makes a line that exceeds `W`; the
  formatter emits it, draws no diagnostic, and `ridl fmt --check` accepts the
  file, because the output is a fixed point.
- **Only line breaks read the width.** Spacing, the order of tokens, the blank
  lines and the comment placement are the same at every width.

### 6.2 How a line is broken

Every construct renders on one line first, as §3 and §4 state. When a physical
line exceeds `W`, the breakable constructs on that line (§6.3) are broken one at
a time, starting from the last one on the line and moving toward its start;
after each break the resulting lines are measured again, and the loop stops when
every line fits or no breakable construct is left. A construct nested inside
another is therefore broken only after the outer one — it then sits on a line of
its own that still exceeds.

A broken construct is laid out as a block: its opener ends the line it was on,
one item per line at the next indent level (two spaces more than the line the
opener is on), and its closer starts a line at the opener's line indent,
followed by whatever came after the construct on the original line. The layout
is a function of the parse tree and `W` only, so it is idempotent by
construction: the second run recomputes the same one-line renderings from the
same tree and takes the same decisions.

At a width of 60, for illustration:

```ridl,ignore
query getSpeedHistory(window: Duration, mode: Mode): (min: Speed, max: Speed, avg: Speed) @[..100ms] [ labels = (A, B) ]
```

The last breakable construct is the attribute block; it takes the block form of
§3.2. The first line still exceeds, and its last breakable construct is now the
tuple return:

```ridl,ignore
query getSpeedHistory(window: Duration, mode: Mode): (
  min: Speed,
  max: Speed,
  avg: Speed
) @[..100ms] [
  labels = (A, B)
]
```

Every line fits, so the parameter list stays on one line. Had it not fitted, the
parameter list would have broken the same way, `(` ending the header line and
`): (` opening the next.

### 6.3 The breakable constructs

| Construct                           | Opener and closer           | One item per line | Separators                                        |
| ----------------------------------- | --------------------------- | ----------------- | ------------------------------------------------- |
| parameter list                      | `(` … `)`                   | a parameter       | a comma after every item but the last (D-13)      |
| tuple type (return, field, element) | `(` … `)`                   | a tuple field     | as above (D-13)                                   |
| inline attribute block              | `[` … `]`                   | an attribute      | none — the block form of §3.2                     |
| attribute value list, nested or not | `(` … `)`                   | a value           | a comma after every item but the last (D-13)      |
| service shape list (§3.3)           | none; the break follows `:` | a shape           | the required comma after every shape but the last |

A tuple inside an array or map type breaks as a tuple; the brackets around it
are not breakable:

```typl,ignore
readings: [(
  a: A,
  b: B
); 8]
```

Everything else is unbreakable: a name, a qualified name, a literal, a string, a
regex, a duration, a timing annotation (`@[20ms..100ms]`), a constraint
(`[0..250 step 1]`), a stream type, a fallible pair `T | E`, a declaration
header (`interface Name {`, `deployment Name for System {`), a member with no
list (`signal name: Type @10ms`, `offers X`, `reserved x`, `NAME = 0`), a
`package` or `import` line, and an **expression**. A predicate longer than the
width stays on one line: breaking an expression needs precedence-aware
continuation rules, and no contract in the corpus comes near the width. Reopen
when one does.

### 6.4 The rsdl declarations under the width

A body is never on one line (D-1), so the width reaches only a header and a
member line, and the one breakable construct on either is the attribute block
with its value lists. A declaration's block in the block form puts the brace on
the closer's line — `component Cruise [`, one attribute per line, `] {` — which
is what the §4.4 example renders to at the default width. A `for` clause and a
reference are unbreakable.

### 6.5 The typl declarations

The typl rules of §2 gain the width and nothing else. The one typl construct
that can break is a tuple type (a field's, or an array's or map's element): it
breaks one field per line as §6.3 states. Every other token on a typl line — a
constraint, a literal, a regex, a unit expression, a qualified name, an enum
value, a block header — is unbreakable, so a `type`, `const`, `import`,
`reserved`, enum value, enumset bit or union arm line is the same at every
width. No line in the parser corpus, the fmt goldens,
`examples/cabin/cabin.ridl`, the baseline corpus or the book fences exceeds 100
columns (measured 2026-10-01), so the existing goldens do not change at the
default width.

### 6.6 Where the width comes from

**The formatter stays pure.** `ridl_fmt::format(text, profile)` becomes
`format(text, profile, &FormatOptions)`, with
`FormatOptions { max_line_length: Option<usize> }` — `Some(100)` by `Default`,
`None` for no limit. The function reads no file and no environment, so it is the
same function on every target; the core of the crate takes no new dependency and
keeps building for wasm32 with `--no-default-features`. `ridl-fmt` is not in
`just wasm-check`'s crate list today; the implementation pull request adds it,
so the gate holds the promise.

**One crate reads `.editorconfig`: `ridl-fmt`, behind a default cargo feature
`editorconfig`.** The feature adds a module with one function,
`FormatOptions::for_path(path) -> FormatOptions`, over the `ec4rs` dependency,
release 1.2.0. Checked 2026-10-01 on crates.io: 1.2.0 was released 2025-04-19
under Apache-2.0, a permissive licence compatible with the workspace's MIT; its
`rust-version` is 1.56, below the workspace's floor (`edition = "2024"` needs
Rust 1.85; the workspace sets no `rust-version`, and only `ridl-rt` sets one,
1.83) and below the pinned toolchain 1.98.1 in `rust-toolchain.toml`; it has one
optional dependency, `language-tags`, which no default feature enables and which
stays off. Its API covers what this section needs: `ec4rs::properties_of(path)`
runs the upward search, and the `ec4rs::property::MaxLineLen` property is
`Value(usize)` or `Off`. A later release line exists — 2.0.0-rc.1, 2026-07-21,
`rust-version` 1.79, different optional dependencies — and is not used: it is a
release candidate, and 1.2.0 does what is needed. Decided by the maintainer at
plan Task 4 on 2026-10-01: RIDL remains MIT; `THIRD-PARTY-NOTICES.txt` carries
the dependency licence text and any applicable upstream notices in every binary
release archive, including the bundled binary in the VS Code extension. `ec4rs`
is the Rust core library editorconfig.org links; Task 4 adds version 1.2.0 to
`Cargo.lock`. The alternatives are `editorconfig` 1.0.0 (MIT, last release 2017,
a binding to the C core) and `editorconfig-rs` 0.2.3 (MIT, 2025, also a binding
to the C core, which needs the system library); neither is pure Rust. A small
parser of our own was rejected (§10): the glob language and the precedence rules
are the whole difficulty, and a reimplementation would diverge from the editors
that read the same file.

**What is honoured.** Everything the EditorConfig specification defines, as
`ec4rs` implements it: the search from the file's directory upward to a file
with `root = true` or the filesystem root; every `[glob]` section whose pattern
matches the path — `*`, `**`, `?`, `[…]`, `[!…]`, `{a,b}`, `{1..3}` and a
`/`-anchored pattern — with later files and later sections winning, so `[*]`,
`[*.ridl]` and `[*.{typl,ridl,rsdl}]` all apply when they match. One key is
read: `max_line_length` — an integer is the width, `off` removes the limit,
absent or `unset` gives 100, and any other value is ignored and gives 100.
`indent_size` and `indent_style` are not read (D-12): the indentation is
canonical at two spaces.

**The callers.** `ridl fmt` (`run_fmt` in `crates/ridl/src/main.rs`) resolves
the options per file, because two files of one run may sit under different
`.editorconfig` files. `ridl lsp`'s `formatting` handler resolves them from the
document's path (`convert::uri_to_path`); an untitled document has no path and
the handler already returns `None` for it. The client's `FormattingOptions` —
`tabSize`, `insertSpaces` and the rest — stay ignored, as the handler's
documentation states today.

**The repository's own `.editorconfig`** gains, in the implementation pull
request:

```ini
[*.{typl,ridl,rsdl}]
indent_size = 2
max_line_length = 100
```

so that the file and the default agree, an editor shows the ruler at 100, and
the `[*]` section's `indent_size = 4` no longer describes the family's files.

## 7. Where each rule comes from

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
| parameter list `(p: P, q: Q)`, one line when it fits                | the tuple rule of the crate doc; **D-9** (width 100, §6)                      |
| line width 100, `.editorconfig` override, the breaks of §6          | **D-9**; **D-12** (indent not read), **D-13** (commas in a broken list)       |
| service list on one line, comma-space separators, no trailing comma | ADR-0015 decision 13 for the commas; **D-7** for the line                     |
| `deployment Name for System [ attrs ] {`                            | general form R5 (clause before attributes), §4.4 (attributes before `{`)      |
| nested `machine` blocks, blank lines preserved                      | `format_block_def` one level down; **D-11**                                   |
| comment placement                                                   | crate doc ("Comments are never dropped"); the two new positions in §5         |

## 8. Decisions

General form §5 settles the colon and forbids alignment; it says nothing about
the choices below. The maintainer decided D-9 on 2026-10-01 and, the same day,
took the recommended option of every other decision. Each entry gives the chosen
option, the alternatives that were rejected, and the reason in short.

**D-1 — rsdl bodies. DECIDED 2026-10-01: (a) always one member per line**, as
every typl brace body is today. Rejected: (b) keep a body the source wrote on
one line on one line; (c) one line when the body fits the width. Reason: it is
the rule the formatter already has; general form §5 item 4 argues for it (the
smallest diff when a member is added); and (c) would have to reach `struct` and
`enum` too to stay one rule, which changes the typl goldens. The width never
reads a body; a member line that exceeds breaks on its own (§6.4). Consequence:
every one-line `system`, `distribution`, `component` and `machine` expands, and
four tests that pin today's one-line rendering change with it:
`an_rsdl_file_takes_the_file_layout_and_keeps_its_declarations_as_written` in
`crates/ridl-fmt/tests/rsdl.rs` is rewritten to the new layout; the `tokens()`
helper of the same file stops comparing `Comma` tokens, because the separator
commas of the reference fences it reads (`system Vehicle { Cruise, Lane, … }`)
are removed; `fmt_formats_an_rsdl_file` in `crates/ridl/tests/facade.rs` expects
`system Vehicle {\n  Cruise\n}`; and the `.rsdl` case of
`formatting_replaces_the_document_with_the_ridl_fmt_rendering` in
`crates/ridl-lsp/tests/server.rs` expects
`component Door {\n  offers solo.door\n}`.

**D-2 — the block form of an attribute block. DECIDED 2026-10-01: (a) by kind
and by width** — a block with at least one predicate takes the block form, and
so does a block whose inline form makes its line exceed the width (§6.2); every
other block is inline. Rejected: (b) by source (a line break after `[`); (c) by
width alone, which would put a one-predicate contract on the member's line, as
one example in the family does: the callable-declaration fence of general form
§2, Shape 2 (`docs/wip/family-general-form.md` line 85,
`command setGear(…) [ require … ]` on one line), which the D-10 follow-up
reformats. Reason: it matches every other example in the general form and every
example in the two references and the book, and gives one rendering per
declaration.

**D-3 — inline attribute block padding. DECIDED 2026-10-01: padded,
`[ a, b ]`.** Rejected: `[a, b]`. Reason: every example in the family writes it
so, and the padding tells an attribute block apart from a typl constraint
`[0..1]` (general form R3 distinguishes them by position only).

**D-4 — the timing and the attribute block. DECIDED 2026-10-01: (a) emit the
timing first, then the attribute block, whatever the source order.** Rejected:
(b) keep the source order. Reason: general form R5 makes `@timing → [ attrs ]`
normative and the parser is lenient, so the formatter emits the conforming
order; the block form reads better with `]` last. Costs: the crate doc's
"whitespace and separators only" sentence gains an exception, and the tests of
§9 compare the two annotation nodes after normalising their order. Erratum: ridl
reference Appendix C writes `attr_block? timing?` for `command` and `query`, the
opposite of R5, and `docs/book/getting-started.md` has six members in that order
(`setTargetSpeed`, `getSpeedHistory`, `requestStart`, `setGear`,
`setClimateTarget`, `getDriverState`, each with the attribute block before the
timing); the book members are rewritten by the sweep, Appendix C by the D-10
follow-up.

**D-5 — the space after `require` and `ensure`. DECIDED 2026-10-01: one space.**
Rejected: the two-space alignment of the general form §5 and §4.4 examples.
Reason: the heading rule of §5 forbids alignment. The two examples are corrected
in the D-10 follow-up.

**D-6 — expression layout. DECIDED 2026-10-01: canonical spacing** as §3.2
states — binary operators spaced, prefix and member access tight, parentheses
kept. Rejected: the source text with whitespace runs collapsed, which leaves
`a<b` and `a < b` as two renderings of one expression. The renderer is one
function over the six expression node kinds; it never reassociates and never
adds or removes parentheses.

**D-7 — the service shape list. DECIDED 2026-10-01: (a) one line when the line
fits the width**, trailing comma removed, and one shape per line after the colon
when it does not (§6.3), the required comma after every shape but the last.
Rejected: (b) keep a list the source broke after `:`. Reason: (b) would be the
only place the formatter keeps a trailing comma, and the only list laid out from
the source rather than from the tree.

**D-8 — the fallible return. DECIDED 2026-10-01: spaced, `T | E`.** Rejected:
`T|E`. Reason: the general form §2 Shape 2 and §6.1 examples write it spaced;
`T|E` appears only in ADR-0008's prose.

**D-9 — a column limit. DECIDED 2026-10-01: yes.** The default is 100 columns;
when `.editorconfig` sets `max_line_length` for the file being formatted, that
value replaces the default. §6 holds the rule, the breaking algorithm, the
breakable constructs of every declaration (the typl ones included), and where
the width is resolved. Rejected: no limit, which the earlier draft recommended.

**D-10 — the scope of the reformatting sweep. DECIDED 2026-10-01: (b) in the
implementation, (c) as a follow-up pull request of its own.** The implementation
reformats the test fixtures, `examples/` and every verified fence in
`docs/book/`; the follow-up reformats the fences of the language references and
the general form, and is out of scope for #387's implementation. Rejected: (a)
the fixtures and `examples/` only, which leaves the book showing the style the
formatter rewrites. Reason: the book is what a reader copies, and the
fixed-point test of §11 then holds it to the canonical style; the references are
long, and general form §5's errata note allows them to be touched
opportunistically.

Consequences of (b), all in the implementation's sweep. The book prose that
quotes a reformatted fence or describes the formatter becomes false and is
corrected with the fences: in `docs/book/getting-started.md`, the inline code
`type Speed : km/h [0.0..250.0 step 0.5]` (line 136),
`signal currentSpeed : Speed @10ms` (line 164),
`fixed doorCount : integer [1..8]` and `type DoorCount : integer [1..8]` (lines
204-205), `hasCruise : Enabled` and `hasCruise : boolean` (lines 479-480), all
of which take the tight colon; the `text` fence at lines 182-188, which quotes
`./veh/common/types.ridl:4:19` and the line `type Speed : km/h […]` of the
reformatted fence, and is regenerated by running `ridl check` on the reformatted
source (the column becomes 18); and the paragraph "`ridl fmt` has its own
canonical layout" at lines 818-820, which says the listings use the aligned
layout and is rewritten to say they are in the formatter's layout. In
`docs/book/cli-reference.md`, the `ridl fmt` paragraph at lines 868-872, whose
sentences at lines 871-872 say the formatter has no layout rules for the rsdl
declarations and keeps a ridl `interface` or `service` as written, are rewritten
to the new rules and gains the width and its `.editorconfig` source (§6.6). Not
changed: `docs/book/getting-started.md` line 214 quotes the typl reference's own
`frame : bytes [8]`, which stays true until the follow-up reformats that
reference, and the follow-up updates it then; the `demo.ridl` and `broken.typl`
diagnostics of `cli-reference.md` quote files that the chapter does not hold in
any `ridl`, `typl` or `rsdl` fence (it has none), so the sweep does not reach
them. The fixture `crates/ridl/tests/baseline-corpus/cluster.ridl` is pinned by
two tests of `crates/ridl/tests/baseline_desk.rs` whose expected strings the
sweep changes (§11, "Round trips").

**D-11 — blank lines between `machine` blocks. DECIDED 2026-10-01: (a) preserved
where the source had one**, as between any two members. Rejected: (b) always
one, as between top-level declarations. Reason: a `machine` is a member.

**D-12 — `indent_size` from `.editorconfig`. DECIDED 2026-10-01: (a) not read**
— the indentation stays canonical at two spaces, and `.editorconfig` contributes
the width only. Rejected: (b) `indent_size` sets the indent step and
`indent_style = tab` emits tabs. Reason: a canonical style has one indent, as it
has one colon; the LSP handler already ignores the client's `tabSize`; and (b)
would make the output depend on a file setting the book fences, the fixtures and
the goldens cannot carry. The repository's `.editorconfig` states
`indent_size = 2` for the family's files (§6.6) so that editors agree with the
formatter, not so that the formatter reads it.

**D-13 — commas in a broken parenthesised list. DECIDED 2026-10-01: (a) kept**,
one after every item but the last, when a parameter list, a tuple type or an
attribute value list breaks (§6.3). Rejected: (b) dropped, as in a brace body.
Reason: a brace body and an attribute block list declarations, which the family
writes without commas; a parenthesised list is a list of items, and Rust, Kotlin
and TypeScript write a broken one with commas. The block-form attribute block
keeps its no-comma rendering, settled by the general form §4.4 example.

## 9. Invariants and how the tests check them

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
   §11 lists.
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

## 10. Alternatives considered

- **Keep the fallback arm.** Rejected: it is what #387 reports, and the issue's
  example shows the alignment general form §5 forbids surviving a format.
- **No column limit.** The earlier draft recommended it (every construct on one
  line, no width logic in the crate). Rejected by the maintainer's D-9 decision
  of 2026-10-01; §6 is the width rule that replaces it. The one-line renderings
  of §3 and §4 are unchanged — the width breaks them at the points §6.3 names.
- **A width-driven layout for every body, as rustfmt lays out a call.** Rejected
  (D-1 (c), D-2 (c)): a body stays one member per line at every width, and a
  predicate block stays block form at every width, so that the examples of the
  general form, the references and the book keep their shape.
- **Breaking expressions at operators.** Rejected for now (§6.3): it needs
  precedence-aware continuation rules, and no contract in the corpus comes near
  the width.
- **A small `.editorconfig` parser of our own.** Rejected (§6.6): the glob
  language and the precedence rules are the whole of the problem, and a second
  implementation would disagree with the editors reading the same file; `ec4rs`
  is pure Rust, maintained, and one optional dependency away from none.
- **Reading `.editorconfig` inside `ridl_fmt::format`.** Rejected (§6.6): the
  function would then touch the filesystem, which makes it impure and breaks the
  wasm32 build; the reader is a separate feature-gated function that produces
  the options the pure function takes.
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

## 11. Test plan

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
`.ridl` and `.rsdl` files of the parser `ok` corpus, and the §9 structure
comparison is added beside `content_tokens`.

**Reference examples** (`crates/ridl-fmt/tests/rsdl.rs`): the existing
idempotence-and-tokens test over the rsdl reference's §3 and Appendix A fences
stays, with one change: its `tokens()` helper keeps every non-trivia token,
`Comma` included, and D-1 removes the separator commas of those fences
(`system Vehicle { Cruise, Lane, Panel, Backend, veh.diag.access }` and five
other bodies), so `tokens()` must also skip `Comma` tokens, as `content_tokens`
in `properties.rs` already does. The as-written test is replaced by the §4
renderings, and the module documentation of `rsdl.rs`, which says the formatter
emits each rsdl declaration as written, is rewritten. The ridl reference's
Appendix A gets the same treatment in a `ridl.rs` sibling.

**Tests outside `ridl-fmt` that pin today's rendering.** Two pin the one-line
rsdl body that D-1 expands: `fmt_formats_an_rsdl_file` in
`crates/ridl/tests/facade.rs` (input
`component   Cruise {}\nsystem Vehicle { Cruise }`; the expected output becomes
`component Cruise {}\n\nsystem Vehicle {\n  Cruise\n}`, and its doc comment,
which says each rsdl declaration is kept as written, is rewritten) and the
`.rsdl` case of `formatting_replaces_the_document_with_the_ridl_fmt_rendering`
in `crates/ridl-lsp/tests/server.rs` (input
`component Door { offers solo.door }`; the expected edit becomes
`component Door {\n  offers solo.door\n}`). The `.ridl` case of the same LSP
test (`interface Door {\n  signal open: boolean\n}`) is already canonical and
does not change, and `fmt_check_walks_into_rsdl_files` still exits 1. Two tests
of `crates/ridl/tests/baseline_desk.rs` pin the text of `cluster.ridl`, which
the D-10 sweep reformats (see "Round trips").

**Round trips.** Each of these formats idempotently and keeps its structure (§9,
invariants 2 to 4), and after the D-10 sweep each is a fixed point:
`examples/cabin/cabin.ridl`; `crates/ridl/tests/baseline-corpus/cluster.ridl`;
every verified `ridl`, `typl` and `rsdl` fence of `docs/book/`, extracted with
the `fenced_blocks` function of `crates/ridl/tests/book_examples.rs` — the same
`pulldown-cmark` options mdBook uses, so the set is exactly the set the compile
harness verifies. A fence marked `ignore` is skipped; a fence with an `allow=`
marker still round trips, because the formatter does not read diagnostics.

`cluster.ridl` is not only compared through its `.ir.json` baseline snapshot,
which holds no source position: two tests of
`crates/ridl/tests/baseline_desk.rs` pin its text, and the sweep changes both.
Reformatting it tightens every `name : Type` colon (measured 2026-10-01, today's
formatter tightens only the colons of the three `type` lines, 34-36, and emits
the interface and service members verbatim) and inserts a blank line between the
three `type` declarations, so every later line moves down by two.
`check_reports_ordinal_drift_against_the_committed_baseline` asserts that the
spans of `tyrePressure`, `legacyWheelPhase`, `doorOpened` and `doorClosed` quote
the aligned source lines (`event tyrePressure : DoorState @[100ms..1s]` and
three others); each expected string takes the tight colon.
`inline_shape_removal_spans_the_service_name` asserts `cluster.ridl:46:9`; the
`service` line moves from 46 to 48, and the column stays 9. The `.ir.json`
snapshot does not change.

**The book harness still passes.** `book_examples_compile` runs `ridl check`
over the staged fences; the sweep changes whitespace, separators and (under D-4)
the annotation order only, none of which a diagnostic reads, and the `allow=`
markers are untouched. `just demo` compiles `examples/cabin` the same way. Both
run in `just build`.

**Width** (unit tests in `crates/ridl-fmt/src/lib.rs`, one group per breakable
construct of §6.3): for each construct, three inputs whose one-line rendering is
99, 100 and 101 characters long at the default width — the first two stay on one
line, the third breaks as §6.2 states, so the boundary is pinned on both sides;
the same three at `max_line_length = 60` through `FormatOptions`, which pins
that the option is read rather than the constant; the §6.2 worked example at 60,
which pins the last-to-first order and the re-measure; a line with two breakable
constructs where breaking the last one is enough, which pins that the loop
stops; a tuple inside an array type; a trailing comment past the width that
causes no break; an unbreakable 120-character string literal that stays as it
is; `max_line_length` of `None` leaving a 200-column line alone; a width of 1,
where every breakable construct breaks and the output still parses to the same
tree.

**Idempotence of broken lines**: every width test formats its output a second
time and asserts equality, and `formatting_is_idempotent_over_the_ok_corpus`
runs at widths 100, 60 and 40 over the whole corpus — the small widths force
breaks in files whose lines fit at 100 — with the §9 structure comparison at
each width.

**`.editorconfig` override** (`crates/ridl/tests/facade.rs`, beside the existing
`ridl fmt` tests such as `fmt_formats_an_rsdl_file`, through the CLI): a
temporary directory with `root = true` and `[*.ridl] max_line_length = 60`
breaks a 80-column member that the default leaves alone; `[*.{typl,ridl,rsdl}]`
matches an `.rsdl` file; `max_line_length = off` leaves a 200-column line alone;
a nested directory's `.editorconfig` with `root = true` stops the walk, so the
outer file's value is not seen; a file with no `.editorconfig` above it formats
at 100. The client-options half of D-12 is already pinned:
`formatting_replaces_the_document_with_the_ridl_fmt_rendering` in
`crates/ridl-lsp/tests/server.rs` formats under `tabs()` (`tab_size: 8`,
`insert_spaces: false`) and `four_spaces()` and asserts two-space indentation
under both. One new LSP test pins what the plumbing adds and fails before it: a
document whose path is in a temporary directory with an `.editorconfig` holding
`root = true` and `[*.typl] max_line_length = 60`, `indent_size = 4` and
`indent_style = tab`, whose typl tuple field line is 80 columns, receives an
edit with the tuple broken at 60 and two-space indentation — the width read from
the file, the indent keys ignored (D-12). `just wasm-check` builds `ridl-fmt`
with `--no-default-features`, pinning the purity claim of §6.6.

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
  was found; the EditorConfig specification (editorconfig.org) and `ec4rs` 1.2
  (<https://github.com/TheDaemoness/ec4rs>) for §6.6; ADR-0009 for the
  `just wasm-check` gate the implementation extends.
- Disposition: when the implementation lands, the decisions of §8 are gardened
  into the crate documentation and a dated amendment of general form §5, and
  this note moves to `docs/archive/`.
