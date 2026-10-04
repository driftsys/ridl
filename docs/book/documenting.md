# Documenting your API

A contract is read by more people than the ones who write it: the authors of
each component that offers or requires it, the reviewers of a change, and the
assistant that completes a line in an editor. A doc comment is where the
contract says what a type, a field or an interaction means. This chapter covers
where a doc comment goes, what it can hold, and the lints that check it. The
rules are in the [typl reference](reference/typl.md) §14, and the decisions
behind them in
[ADR-0026](https://github.com/driftsys/ridl/blob/main/docs/decisions/ADR-0026-doc-comments.md).

**What is built.** The checker reads the doc comment of every declaration and
every member, resolves the links in it, and stores the doc, the resolved links
and the tags in the IR. The language server shows them on hover and in
completion. The doc lints of this chapter are reported by `ridl check`,
`ridl build`, the language server and the MCP tool `ridl_check`, with the levels
of the project's `[lints]` table.

**What is not built.** Generated code carries the doc text where a backend
emitted it before, but no backend renders a link, a `@see` or a `@since`. There
is no `ridl doc` command that writes a documentation site.

## A documented package

A doc comment is written on the lines directly above the item it documents.
Here is a small package with every item documented:

```ridl
package docs.cruise

/// A vehicle speed over ground, as the brake controller computes it from the
/// four wheel speed sensors.
/// @since 1.0
type Speed: km/h [0.0..250.0 step 0.5]

/// The position the driver moved the cruise control lever to.
enum LeverCmd {
  /// The lever is at rest.
  NONE = 0
  /// Raise the set speed by one step. The step is the `step` of [Speed].
  UP = 1
  /// Lower the set speed by one step.
  DOWN = 2
  /// Switch cruise control off. The set speed is kept for a later resume.
  CANCEL = 3
}

/// The state that [CruiseControl] publishes after each lever command.
struct CruiseState {
  /// Whether cruise control holds the vehicle at [CruiseState.setSpeed].
  engaged: Engaged
  /// The speed cruise control holds while it is engaged.
  setSpeed: Speed
}

/// Whether cruise control is engaged.
type Engaged: boolean

/// The cruise control function of one vehicle.
///
/// The interface accepts lever commands from the steering column switch and
/// publishes the resulting state.
/// @see docs.cruise.LeverCmd
interface CruiseControl {
  /// The current cruise control state. A consumer that subscribes late reads
  /// the last published value.
  signal status: CruiseState @[50ms..500ms]

  /// Applies one lever command. The outcome is published on
  /// [CruiseControl.status]; the command does not return it.
  command setLever(
    /// The lever position read from the switch.
    cmd: LeverCmd
  ) @[..100ms]
}
```

The rest of this chapter takes the parts of this example one at a time.

## House style

- Write doc comments as `///` lines. The compiler also accepts the block form
  `/** ... */` with the same meaning, and `ridl fmt` does not rewrite one form
  into the other. A project that wants only `///` turns on the
  `doc-comment-style` lint (see [The doc lints](#the-doc-lints)).
- Start with one sentence that says what the item is. Add a blank `///` line and
  more paragraphs when the reader needs them.
- Say what the declaration itself cannot say: what a value means, where it comes from, and what
  happens when it changes. Do not repeat the range, the unit or the timing. The
  editor shows those from the declaration itself, in the **Contract** part of
  the hover (see [In the editor](#in-the-editor)).
- Document an interface by its responsibility, and document each interaction on
  its own line. The interface doc does not list its members.

## Where a doc comment goes

A doc comment documents the next named declaration or member. These are the
positions that take a doc comment, called carriers:

| Carrier                                                                    | Must have a doc                  |
| -------------------------------------------------------------------------- | -------------------------------- |
| `type`, `const`, `struct`, `enum`, `enumset`, `union`                      | yes, unless `internal`           |
| `interface`, `service`                                                     | yes, unless `internal`           |
| struct field, enum value, enumset bit, union arm                           | yes, unless its parent is `internal` |
| `signal`, `event`, `command`, `query`, `fixed`                             | yes, unless its parent is `internal` |
| a parameter of a `command` or a `query`                                    | no                               |
| a `reserved` entry                                                         | no                               |
| rsdl `system`, `component`, `distribution`, `deployment`, `machine`        | yes                              |
| an rsdl body line (`offers`, `requires`, or a bare reference)              | no                               |

"Must have a doc" is what the `missing-docs` lint checks. A package needs no
doc.

A doc comment in any other position draws `misplaced-doc-comment`: before
`package`, before an `import`, before a return type or an attribute block, and
at the end of a file or of a body. Each of these positions either has no name to
document or belongs to the item around it.

No blank line may separate a doc comment from its carrier. A blank line draws
`detached-doc-comment`.

### Parameters

A parameter takes a doc comment, as `cmd` does in `setLever` above, but never
needs one. Every parameter type is a named type, and the named type carries its
own doc and its own rules. When a parameter has no doc of its own, the editor
shows the doc of its type instead, under the line "From `TypeName`:".

### rsdl

The five rsdl declarations are carriers, and so is each line of their bodies:

```rsdl,ignore
/// The cruise control computer in the front zone.
component CruiseEcu {
  /// The function the driver controls with the steering column lever.
  offers docs.cruise.control
}
```

The docs of the declarations, of the `system` and `distribution` member lines,
and of the `offers` and `requires` lines are stored in the system IR. A
placement line in a `machine` takes a doc too, and the editor shows it on hover,
but the system IR has no field for it.

## What a doc comment holds

A doc is [CommonMark](https://commonmark.org): paragraphs, lists, emphasis,
code spans and fenced code blocks.

### Links

A name in square brackets is a link to a declaration or a member. Three forms
are links:

| Form            | Example                        | Text shown          |
| --------------- | ------------------------------ | ------------------- |
| `[Name]`        | `[Speed]`, `[docs.cruise.Speed]` | the name          |
| ``[`Name`]``    | ``[`Speed`]``                  | the name, as code   |
| `[text][Name]`  | `[the set speed][CruiseState.setSpeed]` | the text   |

The bracket content is a link only when it is a name: one or more dotted
segments. `[0..250]` and `[see below]` are prose. A name inside a code span or a
fenced code block is not a link. A `[Name]` that has a CommonMark link reference
definition (`[Name]: https://...`) in the same doc is an ordinary Markdown link.

A link resolves in the scope of the file that holds it, by the rules of a type
reference:

- A bare `Name` is looked up as a type name is: a declaration of the package,
  then an imported name (an import alias leads to the declaration it names),
  then `ridl.std`.
- A qualified `pkg.Name` reaches any package that the current package can depend
  on — another member of the workspace, or a package of `[imports]` — whether or
  not the file imports it. A doc link never makes you add an import that the
  code does not use. A remote package of `[imports]` is not read by the
  compiler, so a link into one does not resolve yet and draws
  `broken-doc-link`.
- One more segment names a member of the declaration: a field
  (`[CruiseState.setSpeed]`), an enum value (`[LeverCmd.UP]`), an enumset bit, a
  union arm, or an interaction (`[CruiseControl.status]`).
- A declaration that is `internal` in another package cannot be linked to,
  because documentation of the other package could not show it. A link to an
  `internal` declaration of the same package resolves.

A link that does not resolve draws `broken-doc-link`, and the IR does not store
it.

### Tags

A tag is a word that starts with `@`, at the start of a line of the doc. Four
tags exist:

| Tag           | Value                                              | Checked                                        |
| ------------- | -------------------------------------------------- | ---------------------------------------------- |
| `@see`        | one name, with an optional member, as a link takes | resolved like a link                           |
| `@since`      | a version: `MAJOR.MINOR` or `MAJOR.MINOR.PATCH`    | its form only                                  |
| `@deprecated` | a reason in double quotes                          | a missing reason draws `deprecated-without-reason` |
| `@labels`     | comma-separated `SCREAMING_SNAKE` labels           | passed through unchecked (typl §14.3)          |

A tag may appear more than once; every value is kept. An `@` that is not at the
start of a line is prose. A tag line is not part of the doc text: a doc made
only of tags has no text, and `missing-docs` treats it as missing.

`@see` and `@since` change only documentation. `@deprecated` and `@labels` are
stored in the IR beside the doc, and a backend that emits deprecation or label
metadata reads them there.

```ridl
package docs.history

/// A request to change the gear.
/// @since 1.2
/// @see docs.history.GearBox.setGear
/// @labels SIL_2
struct GearRequest {
  /// The gear to change to, from 1 to 6.
  gear: Gear
}

/// A gear of the six-speed gearbox.
type Gear: integer [1..6]

/// The gearbox controller.
interface GearBox {
  /// Requests a gear change.
  command setGear(request: GearRequest) @[..20ms]

  /// The previous gear request, kept for clients built against version 1.0.
  /// @deprecated "use setGear"
  command requestGear(request: GearRequest) @[..20ms]
}
```

### Examples

Write a usage example under a `# Examples` heading, with a fenced code block.
This is a convention, not a tag: the compiler does not read it, and a name inside
the code block is not a link.

````ridl
package docs.examples

/// A distance to the vehicle ahead.
///
/// # Examples
///
/// ```ridl,ignore
/// signal gap: Gap @[50ms..200ms]
/// ```
type Gap: m [0.0..250.0 step 0.1]
````

## The doc lints

Each doc lint is a `TYPL-` code, because doc comments are part of typl and every
language of the family shares them. All of them are warnings; `doc-comment-style`
is `allow` by default, so it is reported only when a project sets its level. The
[lints page](lints.md) explains levels and the `[lints]` table.

| Lint                        | Code     | Default | Raised when                                                                      |
| --------------------------- | -------- | ------- | -------------------------------------------------------------------------------- |
| `broken-doc-link`           | TYPL-401 | warn    | a link or `@see` target does not resolve, or is `internal` in another package    |
| `detached-doc-comment`      | TYPL-404 | warn    | a blank line separates a doc comment from its carrier                            |
| `deprecated-without-reason` | TYPL-405 | warn    | `@deprecated` has no reason                                                      |
| `missing-docs`              | TYPL-406 | warn    | a carrier that must have a doc has none                                          |
| `misplaced-doc-comment`     | TYPL-407 | warn    | a doc comment is in a position that is not a carrier                             |
| `unknown-doc-tag`           | TYPL-408 | warn    | a tag other than `@see`, `@since`, `@deprecated` and `@labels`                   |
| `malformed-doc-tag`         | TYPL-409 | warn    | `@see` or `@since` has a missing or malformed value                              |
| `doc-comment-style`         | TYPL-410 | allow   | a doc comment is written as `/** */`                                             |

The examples below each draw one lint on purpose.

### `missing-docs`

`missing-docs` is reported once per item, at the item's name. Its quick fix in
the editor inserts an empty `///` line above the item. Here `Odometer` has no
doc, and `internal` exempts `Raw` and its field:

```ridl,allow=TYPL-406
package docs.missing

type Odometer: km [0.0..1000000.0 step 0.1]

/// A raw sensor sample, for the use of this package only.
internal struct Raw {
  value: Odometer
}
```

When you check from inside a workspace member, `ridl check`, `ridl build`,
`ridl lock` and the MCP tool `ridl_check` report only the diagnostics of files
under that member, so an undocumented item in a sibling member does not appear
in that report. The language server reports every file it loaded. A remote
package of `[imports]` is never checked.

A project that does not want to document every item yet sets the level in its
`ridl.toml`:

```toml
[lints]
missing-docs = "allow"
```

### `broken-doc-link`

```ridl,allow=TYPL-401
package docs.broken

/// The heading in whole degrees, converted from a [Compass] reading.
type Heading: integer [0..359]
```

`Compass` is declared nowhere, so the link does not resolve.

### `detached-doc-comment`

```ridl,allow=TYPL-404
package docs.detached

/// The cabin air temperature.

type CabinTemperature: Cel [-40.0..85.0 step 0.5]
```

### `deprecated-without-reason`

```ridl,allow=TYPL-405
package docs.reason

/// The fan level of the first climate control version.
/// @deprecated
type FanStep: integer [0..7]
```

### `misplaced-doc-comment`

A doc comment at the end of a body has no item after it:

```ridl,allow=TYPL-407
package docs.misplaced

/// The wiper speed.
enum WiperSpeed {
  /// The wipers are off.
  OFF = 0
  /// The wipers run once every few seconds.
  SLOW = 1
  /// The wipers run continuously.
  FAST = 2
  /// A fourth speed is planned.
}
```

### `unknown-doc-tag` and `malformed-doc-tag`

```ridl,allow=TYPL-408,allow=TYPL-409
package docs.tags

/// The tyre pressure of one wheel.
/// @author the chassis team
/// @since next
type TyrePressure: kPa [0.0..500.0 step 1.0]
```

`@author` is not one of the four tags, and `next` is not a version.

### `doc-comment-style`

`doc-comment-style` reports every `/** */` doc comment, so a project that sets
it to `warn` or `deny` requires `///`:

```toml
[lints]
doc-comment-style = "deny"
```

Its quick fix in the editor rewrites the comment as `///` lines. The lint checks
in one direction only: no setting requires `/** */`.

## In the editor

The language server reads the docs from the same compile as the diagnostics, so
it shows what `ridl check` reads:

- **Hover** on a declaration, a member, a parameter, an rsdl declaration or body
  line, or a use of any of them shows the signature, the doc, a **Contract**
  list, and the `@since` versions, the labels and the deprecation reason. The
  Contract list is read from the IR: the range, step, unit, length, pattern and
  collection bound of the type, the timing of a `signal` or `event`, the
  response bound of a `command` or `query`, the error type of a fallible
  `query`, and each `require` and `ensure` clause. A link in the doc is shown as
  a link to the target's file and line. Hover on a member used in a contract
  clause shows that member.
- **Completion** items carry the doc of the item they insert, except the
  member items offered after `.` in a doc link. Inside a doc comment, `[` and
  `.` complete link targets, and `@` at the start of a line completes the four
  tags.
- **Go to definition**, **find references** and **rename** work on doc links and
  `@see` targets in `.typl`, `.ridl` and `.rsdl` files. Renaming a type updates
  every link and `@see` that names it, in every member of the workspace.
- **Quick fixes**: `missing-docs` inserts a `///` line, and `doc-comment-style`
  rewrites a `/** */` comment as `///` lines.

There is no signature help: the language has no call expression for it to
serve.
