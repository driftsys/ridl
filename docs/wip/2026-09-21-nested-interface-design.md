# Nested interface members — a fixed set of instances under one interface

Status: pre-ADR design note, written 2026-09-21 from a design session against
the crates at the head of `main`. It records a gap the language has, the forms
the current toolchain accepts, the option rejected, the proposal, and the story
list. Nothing here is implemented. It graduates into an ADR that amends
[ADR-0015](../decisions/ADR-0015-qos-absorption-and-rpc-bounds.md) decisions 16
and 17, or is archived if the proposal is not taken.

## 1. The gap

A contract sometimes needs the same interaction set several times under one
service. The example used through this note is a screen that shows up to five
widgets at once. Each widget has a layout, a realtime value, a state, and one
command that dispatches an action to it. The value of one widget changes at its
own rate, so the author wants each widget's members published on their own: a
change to widget 3 must not republish widgets 0, 1, 2 and 4.

The language cannot say this today, and the limit is one consistent decision
rather than several accidental ones:

- A service's interface list is a set. The same interface named twice in one
  service is RIDL-145 (ridl reference §14.5).
- Within one rsdl system each interface is listed by one service. Two closure
  services listing one interface is RSDL-408, because the routing key names the
  package, the interface number and the ordinal, and no service (rsdl reference
  §8).
- A component's instances share its offers and are indistinguishable on the
  wire; redundancy is derived from them, and a left sensor and a right sensor
  are two component declarations (rsdl reference §7).

No interaction, envelope field or address segment can carry "which widget".

## 2. What the toolchain accepts today

Both forms below were type-checked with `ridl check` on 2026-09-21.

### 2.1 The flat form: one signal per member kind, an array in the payload

```ridl
type SlotIndex : integer [0..4]

struct WidgetLayout { slot : SlotIndex, kind : Kind, title : Label }
struct WidgetData   { slot : SlotIndex, value : Percent }
struct WidgetState  { slot : SlotIndex, visibility : Visibility }

struct Layouts { slots : [WidgetLayout; 5] }
struct Data    { slots : [WidgetData; 0..5] }
struct States  { slots : [WidgetState; 5] }

interface Widgets {
  signal  layouts      : Layouts @[1s..]
  signal  data         : Data    @[50ms..200ms]
  signal  widgetStates : States  @[100ms..]
  command dispatch(slot: SlotIndex, act: Action) @[..100ms]
}

service ui.screen : Widgets
```

The array sits in a struct because a signal payload must be a named type. An
exact length says every slot always exists; a bounded length lets a slot be
absent from the payload. The slot index inside each element is what ties the
three signals and the command together.

### 2.2 The unrolled form: one member per slot

```ridl
interface Widgets {
  signal layout0 : WidgetLayout @[1s..]
  signal layout1 : WidgetLayout @[1s..]
  // ... layout2 to layout4
  signal data0 : Percent @[50ms..200ms]
  // ... data1 to data4
  signal widgetState0 : WidgetState @[100ms..]
  // ... widgetState1 to widgetState4
  command dispatch(slot: SlotIndex, act: Action) @[..100ms]
}
```

Fifteen members written by hand for five slots. Each signal is its own channel
with its own last-value cache, sequence number and timing bound. This is the
wire shape the author wants, and it is the baseline the proposal in §4 lowers
to.

Two notes from the session: `state`, `states` and `action` are reserved words
(rmdl's), and a `command` with no `@[..max]` draws warning RIDL-112.

### 2.3 Comparison

| Property                       | Flat (§2.1)                                                                                                     | Unrolled (§2.2), and the proposal (§4)             |
| ------------------------------ | --------------------------------------------------------------------------------------------------------------- | -------------------------------------------------- |
| Channels on the wire           | 4                                                                                                               | 16 (unrolled), 20 (proposal, one command per slot) |
| One widget's value changes     | the whole array is re-encoded and republished                                                                   | one value on one channel                           |
| Coherence                      | across slots: the five layouts are one simultaneous set                                                         | per slot; slots are independent                    |
| Timing bound                   | one deadline for the array                                                                                      | one per slot                                       |
| Sequence number, loss          | per array; a consumer cannot tell which slot it missed                                                          | per slot channel                                   |
| A slot not displayed           | absent from the bounded array                                                                                   | its signals are invalidated                        |
| A consumer that wants one slot | reads the array on every publish                                                                                | reads one channel                                  |
| Adding a sixth slot            | breaking: the array lives in a struct, and `ridl diff` classifies a composite body changed in place as breaking | compatible: a new member, a new ordinal            |
| Slot identity                  | in the data, a field the consumer trusts                                                                        | in the address                                     |
| Cost to have it                | none                                                                                                            | none (unrolled), about six stories (proposal)      |

The flat form is the better contract when the five must be consistent with each
other. Layouts are that case: if a layout change moves several slots at once, a
consumer needs all five from one instant, and only the flat signal gives that.
The unrolled shape is the better contract for what changes per widget at its own
rate. The hybrid in §4.7 keeps both.

## 3. Rejected: a keyed or named instance list in the service

The first idea in the session was to name or key the instances at the service,
which is where the family puts instances (ridl reference §14 reads
`interface : service` as `type : instance`):

```ridl,ignore
service ui.screen : Screen, slots : [Widget; 5]
```

Rejected, for three reasons that hold even after §4.3 removes the runtime cost:

- **rsdl changes.** `requires Widget` would have five providers inside one
  service, so `requires` needs an instance path, and RSDL-403 and RSDL-408 need
  a new rule. That is a change to the apex language for the same wire result.
- **No reuse.** A keyed entry is written again in every service that wants the
  same five slots, and two services can drift. Under §4 the set of slots is a
  type, listed by any service.
- **The hybrid splits.** Layouts flat and data per slot become two service
  entries a consumer has to know belong together, instead of one interface.

A population created and destroyed at runtime, keyed by an id that travels on
the wire, is a different feature. It needs instance lifecycle and disposal in
the runtime and is out of scope here. §4.8 notes how the proposal would
generalise to it.

## 4. Proposal: an interface member that names an interface

### 4.1 Surface

A member of an interface may name another interface, in the `name : Type` line a
struct field uses, in two forms:

```ridl,ignore
interface Widget {
  signal  layout      : WidgetLayout @[1s..]
  signal  data        : Percent      @[50ms..200ms]
  signal  widgetState : WidgetState  @[100ms..]
  command dispatch(act: Action)      @[..100ms]
}

interface Screen {
  slot0 : Widget              // named form
  slot1 : Widget
  slots : [Widget; 5]         // indexed form, exact count
  signal  activeSlot : SlotIndex @[100ms..]
  command resetAll() @[..100ms]
}

service ui.screen : Screen
```

The indexed form uses the family's array spelling, `[T; N]`. A body mixes its
own interactions and nested members freely. Interface members start with a
keyword today, so a line starting with an identifier is unambiguous.

**An inline service shape takes both forms too.** An inline shape is an
interface: ridl reference §14.5 gives it its own `interfaces.lock` entry, keyed
`service:` followed by the service's dotted name (ADR-0015 decision 14). So once
a nested member is legal in an interface body, it is legal in a service body for
the same price, with no rule of its own:

```ridl,ignore
service veh.body.doors {
  left  : DoorControl
  right : DoorControl
  signal  allLocked : boolean @[100ms..]
  command lockAll()           @[..100ms]
}
```

This is `interface Doors { ... }` listed by `service veh.body.doors : Doors`,
with `Doors` unnamed. It is not the rejected form of §3: the instances are
members of an interface body, so each has an interface number and the service's
list is untouched. The trade is the inline form's own — no reuse — and ADR-0015
decision 15 still applies: extracting the body into a named interface later is
breaking for any fallible query in it, so a set of instances that will be shared
is named from the start.

### 4.2 Rules

- **One level, as a limit.** The interface a nested member names may not itself
  have a nested member. The path is at most `service.member.index.member`, which
  keeps the audit argument of ridl reference §14.1 intact: a service's complete
  contract is read from one declaration plus the interfaces it names, with no
  closure walk. The depth is a stated limit, not a property of the design:
  nothing below depends on it being 1, and every cost a deeper nesting adds is
  mechanical (a longer address, a cycle check, more lock entries, a recursive
  grouping in `ridl diff`), none of it in the runtime or the port key. It starts
  at 1 because raising a depth limit is additive — a program legal at depth 1
  stays legal at depth 2 — while starting deeper and retreating is breaking. The
  first contract that needs depth 2 changes a constant in the checker and adds a
  paragraph to the ADR; §4.4 and the cycle rule below are written so that
  nothing else moves.
- **No cycle.** A nested interface may not reach itself through nested members.
  At depth 1 this is unreachable, because the nested interface may not nest at
  all; the rule and its diagnostic (§6) are recorded now so that the day the
  depth is raised, the check already exists and a cycle is not discovered by
  accident.
- **No annotation on the member.** A nested member carries no `@` timing and no
  contract clause; both belong to the interactions inside the named interface.
- **Names.** A nested member shares the body's namespace with the interactions,
  so RIDL-1xx duplicate-member rules apply unchanged. The named interface's
  members do not enter the outer namespace; `Screen.data` does not exist, only
  `Screen.slots.2.data`.
- **Visibility.** A public interface may not nest an `internal` one, mirroring
  RIDL-143 for a service.
- **Not inheritance.** This is composition by value, one level up from a struct
  that holds a struct. `Widget` keeps its own ordinal space, so an insertion in
  `Widget` renumbers nothing in `Screen`, and `Widget` may be listed by a
  service directly, elsewhere, at the same time.

### 4.3 Wire identity: one interface number per instance, no runtime change

Every port trait in `ridl-rt` keys a channel on (interface number, ordinal), in
`crates/ridl-rt/src/port.rs`, and the loopback runtime stores by that pair. A
third key component would be a breaking change to every port trait. The proposal
avoids it: **each instance a nested member creates gets its own interface
number**, allocated in the outer package's `interfaces.lock` under a third key
form, one entry per index:

```text
Screen.slots.0 7
Screen.slots.1 8
Screen.slots.2 9
Screen.slots.3 10
Screen.slots.4 11
Screen.slot0   12
```

An inline shape's instances take the same form under the shape's own key:
`service:veh.body.doors.left` and `service:veh.body.doors.right` for the §4.1
example.

Each entry is an interface on the wire, with `Widget`'s ordinal space. The port
key is unchanged. The per-interface sequence counter and `commit` are then per
slot, which is exactly the coherence §1 asks for. On SOME/IP each slot is its
own eventgroup, which is how a repeated service instance is done there. The
numbers live in `Screen`'s package even when `Widget` is imported, because the
lock is per package and it is `Screen` that creates the instances.

ADR-0015 decision 17 is unchanged in letter: a binding's ordinal spaces stay
keyed on (package, interface number). Only the allocation rule gains a case.
ADR-0021 is untouched.

### 4.4 Addressing

Every address gains at most two segments per nesting level: the member name, and
for the indexed form the index. `ui.screen.slots.2.data` names one channel,
`ui.screen.slot0.data` another, and `ui.screen.activeSlot` is as today. This
amends ADR-0015 decision 16 from "always `service.member`" to a path below the
service, bounded by the nesting depth of §4.2 — four segments at the depth this
note fixes. The amendment should state the bound in terms of the depth rather
than as a count, so that raising the depth does not amend decision 16 a second
time.

### 4.5 Evolution and `ridl diff`

- Raising `[Widget; 5]` to `[Widget; 6]` allocates one lock entry and is
  compatible. Lowering it retires entries and is breaking, with
  `ridl lock <pkg> --retire Screen.slots.5`.
- Adding or removing a named member is a member addition or removal, with the
  existing verdicts. A removed member leaves a `reserved` tombstone, as any
  member does, and its lock entries are retired.
- Renaming a nested member is a lock rename,
  `ridl lock <pkg> --rename
  Screen.slots=Screen.panels`, applied to every
  index.
- Reordering is impossible: the index is the key.
- A change inside `Widget` is reported once, against `Widget`, not once per
  instance. The diff walk and the descriptors group the instances under the
  outer member so a reader sees one member, not five interfaces.

### 4.6 Generated Rust face

`Screen::Client` gets one accessor per nested member returning `Widget::Client`
over the same port with the instance's interface number fixed:

```rust,ignore
let mut screen = ui_screen::Client::new(port);
let value  = screen.slots(2).data()?;
screen.slots(2).dispatch(&Action::Tap)?;
let active = screen.active_slot()?;
```

Today a face names its interface number through the associated constant
`Interface::NUMBER`, emitted by one helper, `interface_number` in
`crates/ridl-backend-rust/src/face.rs`. A nested instance's `Client` and
`Publisher` hold an `InterfaceNo` value instead, set by the outer accessor; a
directly listed interface still uses the constant. `Provider` and `dispatch`
stay per interface, one per instance, and the outer `Provider` composes one
inner `Provider` per index, or a `[P; N]`. The index is checked against the
constant count, or taken as a const generic for a compile-time check.

### 4.7 The hybrid the example wants

```ridl,ignore
interface WidgetLive {
  signal  data        : Percent     @[50ms..200ms]
  signal  widgetState : WidgetState @[100ms..]
  command dispatch(act: Action)     @[..100ms]
}

interface Screen {
  signal layouts : Layouts @[1s..]     // rare, and coherent across slots
  slots : [WidgetLive; 5]              // per slot, at its own rate
}
```

### 4.8 Follow-on: the bounded form

`slots : [Widget; 0..5]` would mean: five instances exist on the wire, and at
most five and at least zero are present, where an absent instance is one whose
signals are all invalidated. ridl already has invalidation, and the generated
`Publisher` has `invalidate_*` per signal, so no wire concept is added. The
minimum is a contract statement, the lowest count the provider promises to keep
present. This form ships after the exact form, once the presence rule is written
into the reference. A later runtime-keyed population would generalise the
indexed form by putting a key type in place of the index, and is not proposed.

## 5. What does not change

- rsdl. A component writes `offers ui.screen` or `requires Screen`. RSDL-408
  keys on `Screen`. A link in the system IR still keys on interface and service.
- `ridl-rt` and `ridl-loopback`: no API change (§4.3).
- The lock file's line format: one new key form, no new line kind.
- The wire backends. Under ADR-0018 they emit payload encodings, so a member
  they do not project costs nothing beyond not rejecting it. To be confirmed per
  backend in the first story.

## 6. Diagnostics

Four new codes in the RIDL-1xx band, to be allocated in the catalogue:

- a nested member whose interface itself has a nested member (depth);
- a nested interface that reaches itself through nested members (cycle) —
  unreachable while the depth is 1, allocated now for the reason §4.2 gives;
- a nested member carrying `@` timing or a contract clause;
- a public interface nesting an `internal` interface.

## 7. Estimate and stories

| Crate                               | Change                                                                                    | Size |
| ----------------------------------- | ----------------------------------------------------------------------------------------- | ---- |
| ridl-syntax                         | one member form, named and indexed, plus AST                                              | S    |
| ridl-sem                            | resolve the reference, ordinal in the outer space, four codes, skip in timing and clauses | M    |
| ridl-core (lock)                    | one key form, allocation per index, retire and rename per index                           | S    |
| ridl-ir                             | one new `Decl` kind on `Interface.interactions`, additive to the v2 schema                | S    |
| ridl-diff                           | walk and classify the kind; count raised compatible, lowered breaking; group instances    | M    |
| ridl-backend-rust                   | descriptors over instances, accessor, `InterfaceNo` by value, `Provider` composition      | M    |
| ridl-backend-ts, proto, flatbuffers | confirm payload-only; refuse or ignore the member                                         | S    |
| ridl-lsp, ridl-fmt                  | hover, inlay, layout of the new line                                                      | S    |
| ridl-rt, ridl-loopback, rsdl        | none                                                                                      | none |
| docs                                | ADR amending ADR-0015 d16 and d17; ridl reference §11, §14, §15; book chapter; roadmap    | M    |

Sequenced so each step ships on its own:

1. Language and IR: syntax, sem, lock, IR, diff, LSP, docs. A backend that meets
   a nested member emits one diagnostic saying it is not generated yet.
   Contracts can be written, checked, locked and diffed. About four stories.
2. The Rust face over the per-instance numbers, proven by the loopback round
   trip. One story, possibly two.
3. The bounded form (§4.8), once presence is specified. One story.

## 8. Open questions

- The lock key spelling for an index: `Screen.slots.0` reads as a path and
  matches the address; `Screen.slots[0]` matches the type syntax. This note uses
  the path form.
- Whether a screen-level command that takes the index as a parameter should be
  preferred over one command per instance in the book's guidance. Both are
  legal; the per-instance one is what the nested form produces.
- Whether `ridl diff` should present the instances of an indexed member as one
  row with a count, or one row per index. This note says one row.
