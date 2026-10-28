# `ridl-transport-ws` reads the catalog descriptor — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the `ridl-transport-ws` server an optional member table of the
catalog it serves, so that it accepts a listed earlier hash at `attach`, refuses
an unknown interaction, keeps a settled response until its bound has passed, and
can read the table from a catalog descriptor under a feature.

**Architecture:** A public owned `MemberTable` and a third backend adapter,
`Described<B>`, carry the table into the sans-IO `Session` through a provided
method `Backend::members`. The session consults the table at `attach`, before
the form check of every `subscribe`, `read`, `request` and `unsubscribe`, and
when it settles a response. The optional feature `descriptor` adds
`MemberTable::from_descriptor` over `ridl-descriptor` with its default features
off. Without a table the server behaves as it does today.

**Tech Stack:** Rust (edition 2021, `rust-version = "1.83"`), `ridl-rt`,
`ridl-descriptor` (planus 1.3.0, Rust 1.88) under the feature, `ridl-loopback`
and `tungstenite` in tests, the `justfile` gates.

**Spec:**
[`docs/wip/2026-10-23-transport-ws-descriptor-design.md`](2026-10-23-transport-ws-descriptor-design.md)
(the design note; its "Decisions" section holds the rulings D-1 to D-3 of
2026-10-23). Issues: driftsys/ridl#985 (the work), driftsys/ridl#984 (fixed by
Task 1), driftsys/ridl#986 (settled by Task 3).

## Global Constraints

Every task's requirements include these lines.

- **MSRV.** `crates/ridl-transport-ws/Cargo.toml` keeps `edition = "2021"` and
  `rust-version = "1.83"`. The crate builds with Rust 1.83 with its default
  features and with none. The `descriptor` feature needs Rust 1.88 (planus 1.3.0
  pins it), and the rustdoc, the design record and `docs/book/stability.md` say
  so. `freeze/msrv.txt` does not change (it lists per-crate `rust-version`
  values, read from the manifests).
- **`Option::is_none_or`** (Rust 1.82) is the newest standard-library item the
  crate may use; `config.rs` already uses it.
- **The frozen `runtime-crates` surface** (ADR-0029, `freeze/surfaces.toml`
  baseline `v0.12.0`): every change to the public API of `ridl-transport-ws` is
  additive. `serve`'s signature, `Config`'s existing methods and their return
  types, and `Backend`'s required items do not change. `Note` and `Ended` are
  `#[non_exhaustive]`, so a variant may be added.
- **Gates live in the `justfile`** (ADR-0009). A new check is a recipe or a line
  in an existing recipe, never a command in `.github/workflows/ci.yml`.
  `just compat-check` builds the packaged crate offline; a dependency the
  package gains must be resolved and fetched the way the `ridl-descriptor` cell
  does it (see Task 4).
- **`just story-id-check`**: no story id (`E16.5`) and no plan name in
  `crates/`, `docs/design/`, `docs/technotes/` or `docs/book/`. Cite issues as
  `driftsys/ridl#N`.
- **Prose** (comments, rustdoc, commit messages, docs): plain literal English,
  no idioms. Commits follow Conventional Commits with the scopes of
  `.git-std.toml`: `ridl-transport-ws`, `docs`, `family`, `adr`, `repo`,
  `ridl-descriptor`.
- **Shipped docs describe the system as built.** A sentence in
  `docs/specification/frame-specification.md`,
  `docs/design/ridl-transport-ws.md`,
  `docs/technotes/walking-skeleton-architecture.md`,
  `docs/design/catalog-descriptor.md` or the crate rustdoc that a task makes
  false is changed in that task's PR, not later. Each task below quotes the
  current sentence and the new one.
- **Frame specification amendments land in the same PR as the behaviour.** The
  As-built note, §6.4, §7, §11.1 and §12 change in the task whose code makes the
  change true.
- **Tests run focused.** An implementer runs `cargo test -p ridl-transport-ws`
  (and the other commands a task names); `just test`, `just freeze-check`,
  `just compat-check` and `just build` are CI's job on the PR.
- **Formatting.** `cargo fmt --all` before each commit; `prim fmt` over edited
  Markdown (`just fmt`), then `just check`.

## Review Focus

Inputs the spec implies but no task's test exercises on its own; each line names
the test added to the owning task.

1. **A table that holds an interaction the backend does not serve.** A `request`
   of a command the table lists but no handler serves stays in flight and is
   abandoned after `call_abandon_after`, as today; the lookup must not turn an
   unserved member into `UnknownInteraction`. Task 1 test
   `a_listed_command_no_handler_serves_stays_in_flight`.
2. **The same key under two kinds in one table.** `MemberTable::insert` of
   `(iface, ord)` twice replaces the entry and returns the previous one; the
   lookup then matches the newest kind only. Task 1 test
   `insert_replaces_the_entry_under_the_same_key`.
3. **A stamp far behind the provider's clock.** A request stamped at 0 while the
   backend clock is at one hour keeps its response only until `0 + max`, which
   is already past: the repeat is `DuplicateDropped` on the next tick. The rule
   is what §7 asks ("measured from the request's `stamp`"), and the test pins
   that the server does not extend it. Task 3 test
   `a_request_stamped_far_behind_the_clock_keeps_its_response_until_stamp_plus_max`.
4. **A response exactly at the cap.** Retained bytes equal to
   `max_retained_bytes` do not close the session; one byte more does. Task 3
   test `retained_bytes_equal_to_the_cap_do_not_close`.
5. **A descriptor whose `compatible` list repeats the catalog's own hash, or
   lists a hash twice.** `from_descriptor` keeps each hash once and never lists
   the own hash as compatible; `accepts` is unchanged either way. Task 4 test
   `a_repeated_or_own_hash_in_compatible_is_kept_once`.

---

## Shared vocabulary

Symbols that more than one task uses. A task's Interfaces block repeats what it
consumes.

- `Key = (InterfaceNo, Ordinal)` — the private alias in `server/session.rs`.
- `Target { kind: Kind, iface: InterfaceNo, ord: Ordinal }` —
  `crate::frame::Target`, `Copy + Eq`.
- `Kind` — `ridl_rt::contract::Kind` (`Signal`, `Event`, `Command`, `Query`,
  `Fixed`).
- The scenario catalog (`crates/ridl-transport-ws/tests/support/provider.rs`,
  and the constants of the `tests` module of `server/session.rs`): unit
  `test.ws` / `ws.test`, hash `[7u8; 32]`, interface 1, signal 1, event 2,
  command 3, query 4, fixed 5. Ordinal 6 is the reserved ordinal in every table
  this plan builds; ordinal 7 is a second command with no bound (Task 3).

---

### Task 1: The member table and the §6.4 refusals

Closes driftsys/ridl#984.

**Files:**

- Create: `crates/ridl-transport-ws/src/server/members.rs`
- Modify: `crates/ridl-transport-ws/src/server/backend.rs` (the `Backend` trait,
  lines 25-51; new adapter after `Scanning`, line 101)
- Modify: `crates/ridl-transport-ws/src/server.rs:5-15` (module list and
  re-exports), `:184-191` (the `serve` rustdoc paragraph on session state)
- Modify: `crates/ridl-transport-ws/src/server/config.rs:12-61` (`Note`)
- Modify: `crates/ridl-transport-ws/src/server/session.rs` — `subscribe` (524),
  `unsubscribe` (600), `read` (624), `request` (684); the `tests` module
- Modify: `crates/ridl-transport-ws/src/lib.rs:73-75` (crate rustdoc) and
  `:84-86` (re-exports)
- Modify: `docs/design/ridl-transport-ws.md` — D-10, D-12, D-13 last paragraph,
  D-18 `Note` list, "What is not built"
- Modify: `docs/specification/frame-specification.md:664-668` (§6.4 last
  sentences)

**Interfaces:**

- Consumes: `Backend` (`type Ports`, `ports(&mut self)`, `scannable(&self)`,
  `read_fixed(&self, iface, ord, out)`), `Session::new(backend, &Config)`,
  `Note`, `Target`,
  `ridl_rt::contract::{CatalogHash, CatalogRef, InterfaceNo, Kind, Ordinal}`.
- Produces (Tasks 2 to 5 rely on these exact names):

```rust
// crates/ridl-transport-ws/src/server/members.rs
pub struct MemberTable { /* private */ }
impl MemberTable {
    pub fn new(name: impl Into<String>, hash: CatalogHash) -> Self;
    /// Inserts or replaces the entry under `(iface, ord)`; returns the entry it replaced.
    pub fn insert(&mut self, iface: InterfaceNo, ord: Ordinal, kind: Kind, max: Option<Duration>)
        -> Option<(Kind, Option<Duration>)>;
    pub fn name(&self) -> &str;
    pub fn hash(&self) -> &CatalogHash;
    /// The kind and the bound of a member, or `None` for a key the catalog does not hold.
    pub fn member(&self, iface: InterfaceNo, ord: Ordinal) -> Option<(Kind, Option<Duration>)>;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
}
impl core::fmt::Debug for MemberTable; // derived

// crates/ridl-transport-ws/src/server/backend.rs
pub trait Backend {
    // ... the three existing items, unchanged ...
    /// The member table of the catalog the ports serve, when the embedder supplied one.
    fn members(&self) -> Option<&MemberTable> { None }
}
pub struct Described<B: Backend> { /* private: inner: B, table: Arc<MemberTable> */ }
impl<B: Backend> Described<B> {
    /// Fails when the table's name or hash differs from `backend.ports().catalog()`.
    pub fn new(backend: B, table: Arc<MemberTable>) -> Result<Self, TableMismatch>;
}
impl<B: Backend> Backend for Described<B> { /* forwards ports, scannable, read_fixed; members = Some(&self.table) */ }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableMismatch {
    /// The catalog the ports serve.
    pub ports: CatalogRef,
    /// The table's unit name.
    pub name: String,
    /// The table's catalog hash.
    pub hash: CatalogHash,
}
impl fmt::Display for TableMismatch; impl std::error::Error for TableMismatch;

// crates/ridl-transport-ws/src/server/config.rs
pub enum Note { /* existing variants */
    /// A `subscribe`, `read`, `request` or `unsubscribe` named an interaction the member table
    /// does not hold, or holds under another kind (frame specification §6.4). An `unsubscribe`
    /// is dropped; the other three are answered `contract(UnknownInteraction)`.
    UnknownInteraction { target: Target },
}
```

`crate::{Described, MemberTable, TableMismatch}` are re-exported from `lib.rs`
under the `server` feature, beside `Backend`.

`Duration` is `std::time::Duration`. `MemberTable` stores the members in a
`BTreeMap<(InterfaceNo, Ordinal), (Kind, Option<Duration>)>`; `InterfaceNo` and
`Ordinal` derive `Ord` in `ridl-rt`.

- [ ] **Step 1: Write the failing unit tests of `MemberTable` and `Described`**

In `members.rs`, a `#[cfg(test)] mod tests`:

```rust
#[test]
fn a_new_table_is_empty_and_knows_no_member() {
    let t = MemberTable::new("ws.test", CatalogHash([7; 32]));
    assert_eq!((t.name(), t.hash(), t.len(), t.is_empty()), ("ws.test", &CatalogHash([7; 32]), 0, true));
    assert_eq!(t.member(InterfaceNo(1), Ordinal(1)), None);
}

#[test]
fn insert_replaces_the_entry_under_the_same_key() {
    let mut t = MemberTable::new("ws.test", CatalogHash([7; 32]));
    assert_eq!(t.insert(InterfaceNo(1), Ordinal(1), Kind::Signal, None), None);
    assert_eq!(
        t.insert(InterfaceNo(1), Ordinal(1), Kind::Event, Some(Duration::from_secs(1))),
        Some((Kind::Signal, None))
    );
    assert_eq!(t.member(InterfaceNo(1), Ordinal(1)), Some((Kind::Event, Some(Duration::from_secs(1)))));
    assert_eq!(t.len(), 1);
}
```

In the `tests` module of `session.rs` (test 12 of the note):

```rust
#[test]
fn described_new_refuses_a_table_whose_name_or_hash_differs_from_the_ports() {
    let rt = Loopback::new(CATALOG);
    let other_name = Arc::new(MemberTable::new("other", CATALOG.hash));
    let err = Described::new(Scanning(rt.attach()), other_name).err().expect("refused");
    assert_eq!(err, TableMismatch { ports: CATALOG, name: "other".to_owned(), hash: CATALOG.hash });
    let other_hash = Arc::new(MemberTable::new("ws.test", CatalogHash([8; 32])));
    let err = Described::new(Scanning(rt.attach()), other_hash).err().expect("refused");
    assert_eq!(err.hash, CatalogHash([8; 32]));
    assert!(Described::new(Scanning(rt.attach()), table()).is_ok());
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p ridl-transport-ws --lib members::` Expected: compile error,
`MemberTable` not found.

- [ ] **Step 3: Implement `members.rs`, `Described`, `Backend::members`,
      `Note::UnknownInteraction`, and the re-exports**

`server.rs` gains `mod members;` and
`pub use backend::{Backend, Described, Polling, Scanning, TableMismatch};`,
`pub use members::MemberTable;`. `lib.rs` re-exports the three new names.
`Described::new` takes `mut backend: B`, reads `*backend.ports().catalog()`
once, and compares
`table.name() == catalog.name && *table.hash() == catalog.hash`.
`TableMismatch`'s `Display`:
``the member table describes `{name}` with hash
{hash}, and the ports serve `{ports.name}` with hash {ports.hash}``
(hashes as 64 lowercase hexadecimal digits).

- [ ] **Step 4: Run the tests of Step 1 to verify they pass**

Run: `cargo test -p ridl-transport-ws --lib members:: described_new` Expected:
PASS.

- [ ] **Step 5: Write the failing session tests of the lookup (tests 2 to 6 of
      the note)**

In the `tests` module of `session.rs`, add the helpers and the tests. The
scenario table:

```rust
/// The member table of the test catalog: signal 1, event 2, command 3, query 4,
/// fixed 5; ordinal 6 is reserved and has no entry.
fn table() -> Arc<MemberTable> {
    let mut t = MemberTable::new("ws.test", CatalogHash([7; 32]));
    let bound = Some(std::time::Duration::from_secs(1));
    t.insert(IFACE, Ordinal(1), Kind::Signal, None);
    t.insert(IFACE, Ordinal(2), Kind::Event, None);
    t.insert(IFACE, Ordinal(3), Kind::Command, bound);
    t.insert(IFACE, Ordinal(4), Kind::Query, bound);
    t.insert(IFACE, Ordinal(5), Kind::Fixed, None);
    Arc::new(t)
}

fn described_scanning() -> (Session<Described<Scanning<Loopback>>>, Loopback, Notes) { /* as `scanning()`, wrapped in Described::new(.., table()).unwrap() */ }
fn described_polling() -> (Session<Described<Polling<Loopback>>>, Loopback, Notes) { /* the same over Polling */ }
```

The tests (each names the mutation that turns it red):

```rust
/// Mutation: skip the lookup (the loopback answers `Init` for any key).
#[test]
fn a_subscribe_of_an_ordinal_the_table_lacks_is_unknown_and_a_later_commit_sends_nothing() {
    let (mut s, mut p, notes) = described_scanning();
    open(&mut s);
    let t = target(Kind::Signal, 9);
    let (step, out) = send(&mut s, &msg(Frame::Subscribe(t)));
    assert_eq!(step, Step::Continue);
    assert_eq!(out.len(), 1);
    assert_eq!(decode(&out[0]).unwrap(), unknown(t));
    assert_eq!(*notes.lock().unwrap(), vec![Note::UnknownInteraction { target: t }]);
    p.set(IFACE, Ordinal(9), &[1]).unwrap();
    p.commit();
    let (_, out) = tick(&mut s);
    assert!(out.is_empty());
}

/// Mutation: look the ordinal up without the kind.
#[test]
fn a_subscribe_of_the_event_ordinal_under_kind_signal_is_unknown() {
    let (mut s, _p, _n) = described_scanning();
    open(&mut s);
    let t = target(Kind::Signal, 2);
    let (_, out) = send(&mut s, &msg(Frame::Subscribe(t)));
    assert_eq!(decode(&out[0]).unwrap(), unknown(t));
    // The same ordinal under its own kind is accepted.
    let (_, out) = send(&mut s, &msg(Frame::Subscribe(target(Kind::Event, 2))));
    assert_eq!(decode(&out[0]).unwrap(), accepted(target(Kind::Event, 2)));
}

/// Mutation: route by form first (the reserved ordinal would reach the backend as a
/// signal read; the signal under `Command` would be refused by the form check with the
/// same frame, so the second half checks the note too).
#[test]
fn a_read_of_the_reserved_ordinal_and_a_request_of_the_signal_as_a_command_are_refused_with_the_identity_echoed() {
    let (mut s, _p, notes) = described_scanning();
    open(&mut s);
    let reserved = target(Kind::Signal, 6);
    let (_, out) = send(&mut s, &msg(Frame::Read(reserved)));
    assert_eq!(decode(&out[0]).unwrap(), unknown(reserved));
    let as_command = target(Kind::Command, 1);
    let (_, out) = send(&mut s, &request(as_command, 1, &[]));
    assert_eq!(
        decode(&out[0]).unwrap(),
        response(as_command, 0, 1, 1, ResponseOutcome::Contract(Contract::UnknownInteraction))
    );
    assert_eq!(
        *notes.lock().unwrap(),
        vec![Note::UnknownInteraction { target: reserved }, Note::UnknownInteraction { target: as_command }]
    );
}

/// Mutation: drop without recording.
#[test]
fn an_unsubscribe_of_an_unknown_key_sends_nothing_and_is_recorded() {
    let (mut s, _p, notes) = described_scanning();
    open(&mut s);
    let t = target(Kind::Event, 9);
    let (step, out) = send(&mut s, &msg(Frame::Unsubscribe(t)));
    assert_eq!((step, out.is_empty()), (Step::Continue, true));
    assert_eq!(*notes.lock().unwrap(), vec![Note::UnknownInteraction { target: t }]);
}
```

Test 6 needs a backend whose `read` calls are counted. Model it on the existing
`Counted` backend of the `tests` module (line 2466): a struct `Reads` over
`Loopback` that forwards every port trait and counts `SignalReader::read` in an
`Rc<Cell<usize>>`, implemented for both modes by wrapping in `Polling` and in
`Scanning` (the `Scanning` bound needs `ScannableSignals + FixedReader`, both
forwarded).

```rust
/// Mutation: insert the signal state before the lookup.
#[test]
fn a_thousand_unknown_subscribes_leave_no_signal_state_in_either_mode() {
    for scanning in [false, true] {
        let (mut s, mut p, reads) = reads(scanning); // Session<Described<..>>, provider, Rc<Cell<usize>>
        open(&mut s);
        for ord in 100..1100u32 {
            let (_, out) = send(&mut s, &msg(Frame::Subscribe(target(Kind::Signal, ord))));
            assert_eq!(decode(&out[0]).unwrap(), unknown(target(Kind::Signal, ord)));
            p.set(IFACE, Ordinal(ord), &[1]).unwrap();
        }
        p.commit();
        reads.set(0);
        let (_, out) = tick(&mut s);
        assert!(out.is_empty(), "scanning={scanning}");
        assert_eq!(reads.get(), 0, "scanning={scanning}: a tick read a signal");
    }
}
```

Review Focus item 1:

```rust
#[test]
fn a_listed_command_no_handler_serves_stays_in_flight() {
    let (mut s, _p, notes) = described_scanning();
    open(&mut s);
    let (_, out) = send(&mut s, &request(COMMAND, 1, &[]));
    assert!(out.is_empty());
    let (_, out) = tick(&mut s);
    assert!(out.is_empty());
    assert!(notes.lock().unwrap().is_empty());
}
```

Also keep the existing D-12 tests green: they run without a table and must still
refuse by form.

- [ ] **Step 6: Run the session tests to verify they fail**

Run: `cargo test -p ridl-transport-ws --lib session::tests` Expected: the five
new tests FAIL (`accepted` where `unknown` is expected; the notes list is empty;
the counted reads are 1000 in polling mode).

- [ ] **Step 7: Implement the lookup in `session.rs`**

One private method:

```rust
/// Whether the frame's identity is in the member table under its kind. With no table,
/// every identity is known and only the form check of D-12 applies.
fn known(&self, target: Target) -> bool
```

`self.backend.members().is_none_or(|t| t.member(target.iface, target.ord).is_some_and(|(kind, _)| kind == target.kind))`.
At the top of `subscribe`, `read`, `request` (after the `seen(seq)` placement,
so that a repeated `seq` is still handled first, and before `ports.command`) and
`unsubscribe`: when `!self.known(target)`, record
`Note::UnknownInteraction { target }`, then answer as the note's section 3 says
— `answer(target, Contract(UnknownInteraction))` for `subscribe` and `read`,
`respond(target, seq, ResponseOutcome::Contract(UnknownInteraction), out)` for
`request`, nothing for `unsubscribe` — and return `Step::Continue`. The D-12
form check follows unchanged.

- [ ] **Step 8: Run the whole crate's tests**

Run: `cargo test -p ridl-transport-ws` Expected: PASS, every existing test
included.

- [ ] **Step 9: Docs — rustdoc, design record, §6.4**

`serve` rustdoc (`server.rs:184-191`). Current:

> The session keeps state for each signal the client subscribes, and for each
> signal a `read` found published, for as long as the session lasts: the
> channel's session `seq` never restarts (frame specification §7). No limit in
> the crate bounds the number of these signals, so per-session memory, and in
> polling mode the work of each tick, grows with the distinct signals the client
> names. The embedder's access policy decides which clients may open a session;
> it must trust each one for that growth. A `read` of a signal that was never
> published keeps no state.

New (one paragraph added after it):

> With a member table ([`Backend::members`] returns `Some`, as [`Described`]
> does), a `subscribe`, a `read` or a `request` naming an interaction the table
> does not hold is refused with `contract(UnknownInteraction)` before it reaches
> the backend, so the signal and event state of a session holds at most one
> entry per signal or event member of the catalog. Without a table, nothing
> bounds it, and the paragraph above applies.

Crate rustdoc (`lib.rs:73-75`). Current:

> - It reads no catalog descriptor. An `attach` is accepted on an exact match of
>   the unit name and the catalog hash, and the server checks a frame's `kind`
>   against the form, not against the catalog.

New:

> - It reads no catalog descriptor. An `attach` is accepted on an exact match of
>   the unit name and the catalog hash. Without a [`MemberTable`] the server
>   checks a frame's `kind` against the form, not against the catalog; with one
>   ([`Described`]) it refuses an interaction the catalog does not hold.

(Tasks 2 and 4 rewrite the first two sentences.)

Design record `docs/design/ridl-transport-ws.md`:

- D-10: after "two provided methods: `scannable()`, which returns `None`, and
  `read_fixed(iface, ord, out)`, which answers
  `ReadError::Contract(UnknownInteraction)`" add a third: "and `members()`,
  which returns `None`: the member table of the catalog, when the embedder
  supplied one (`server/members.rs`)". After the two adapters, add the third:
  "`Described<B>` wraps any backend with an `Arc<MemberTable>` and forwards the
  other three methods; `Described::new` compares the table's name and hash with
  `ports().catalog()` and returns `TableMismatch` on a difference, so a wrong
  table is found on the embedder's thread before `serve` runs." Add a paragraph
  on `MemberTable`: the unit name, the hash, and a map from
  `(InterfaceNo, Ordinal)` to `(Kind, Option<Duration>)`, the duration being the
  member's `max`; a retired interface and a reserved ordinal have no entry
  (§6.4); built by hand through `new` and `insert`.
- D-12, first sentence. Current: "It holds no catalog descriptor, so it cannot
  check a `kind` against the catalog (§4 "Why `kind` is on the frame"); that
  check is the backend's when it has a member table. What it does check is that
  the form admits the kind:". New: "Without a member table it cannot check a
  `kind` against the catalog (§4 "Why `kind` is on the frame") and checks only
  that the form admits the kind. With one, every `subscribe`, `read`, `request`
  and `unsubscribe` first looks the frame's `(interface, ordinal)` up in the
  table: absent, or present under another `kind`, a `subscribe` or a `read` is
  answered `answer` with `contract(UnknownInteraction)`, a `request` a
  `response` carrying it and the request's `seq` as `correlation`, and an
  `unsubscribe` is dropped and recorded as `Note::UnknownInteraction` (§6.4).
  Nothing reaches the backend and no session state is created. The form check
  follows in both cases:".
- D-13 last paragraph. Current ends: "A bound needs a decision, because the
  frame has no refusal outcome for it (driftsys/ridl#984)." New: "With a member
  table (D-12) the lookup bounds it: a session holds at most one signal channel
  and one event channel per member of the catalog. Without one the growth above
  stands, and `serve`'s rustdoc says so."
- D-18 `Note` list: add `UnknownInteraction` (an interaction the member table
  does not hold; dropped or answered as D-12 says).
- "What is not built": remove the bullet "A bound on the per-session signal
  state (driftsys/ridl#984)."

Frame specification §6.4, lines 664-668. Current:

> Whether a runtime can tell an unknown ordinal from one it has no value for yet
> depends on its holding the member table. `ridl-loopback` holds none: giving
> the loopback the catalog descriptor is not yet assigned to an issue. Until it
> holds one, it cannot report `UnknownInteraction` at all.

New:

> Whether a runtime can tell an unknown ordinal from one it has no value for yet
> depends on its holding the member table. `ridl-transport-ws` reports
> `UnknownInteraction` when its embedder gives it one (its design record, D-12),
> and not otherwise. `ridl-loopback` holds none: giving the loopback the catalog
> descriptor is not yet assigned to an issue. Until it holds one, it cannot
> report `UnknownInteraction` at all.

- [ ] **Step 10: Format, lint, commit**

Run:
`cargo fmt --all && cargo clippy -p ridl-transport-ws --all-targets -- -D warnings && just fmt && just check && just link-check && just doc-path-check && just story-id-check`
Expected: no diagnostic.

```bash
git add crates/ridl-transport-ws docs/design/ridl-transport-ws.md docs/specification/frame-specification.md
git commit -m "feat(ridl-transport-ws): refuse an unknown interaction through a member table

Fixes driftsys/ridl#984."
```

---

### Task 2: Compatible hashes at `attach`

Closes nothing; part of driftsys/ridl#985.

**Files:**

- Modify: `crates/ridl-transport-ws/src/server/members.rs`
- Modify: `crates/ridl-transport-ws/src/server/session.rs:485-508` (`attach`)
  and its `tests` module
- Modify: `crates/ridl-transport-ws/src/lib.rs:73-74`
- Modify: `docs/specification/frame-specification.md:23-45` (As-built note),
  `:1088-1092` (§12 item 1)
- Modify: `docs/design/ridl-transport-ws.md` D-11
- Modify: `docs/technotes/walking-skeleton-architecture.md:325-326`
- Modify: `docs/design/catalog-descriptor.md:292-298`

**Interfaces:**

- Consumes: `MemberTable::{new, hash}`, `Described`, `Session::attach`,
  `Reason::CatalogMismatch`, the test helpers `attach_msg`, `refused`,
  `described_scanning`, `table` of Task 1.
- Produces:

```rust
impl MemberTable {
    /// Adds an earlier catalog hash this catalog is compatible with (§6.1). The own hash and a
    /// hash already listed are not added.
    pub fn add_compatible(&mut self, hash: CatalogHash);
    /// The compatible hashes, in insertion order.
    pub fn compatible(&self) -> &[CatalogHash];
    /// Whether an `attach` naming `hash` is accepted: the own hash or a compatible one.
    pub fn accepts(&self, hash: &CatalogHash) -> bool;
}
```

- [ ] **Step 1: Write the failing tests (test 1 of the note)**

In `members.rs` tests:

```rust
#[test]
fn accepts_the_own_hash_and_each_compatible_one_and_lists_each_once() {
    let mut t = MemberTable::new("ws.test", CatalogHash([7; 32]));
    assert!(t.accepts(&CatalogHash([7; 32])));
    assert!(!t.accepts(&CatalogHash([6; 32])));
    t.add_compatible(CatalogHash([6; 32]));
    t.add_compatible(CatalogHash([6; 32]));
    t.add_compatible(CatalogHash([7; 32]));
    assert_eq!(t.compatible(), &[CatalogHash([6; 32])]);
    assert!(t.accepts(&CatalogHash([6; 32])));
    assert!(!t.accepts(&CatalogHash([5; 32])));
}
```

In `session.rs` tests (mutation: compare the own hash only):

```rust
#[test]
fn an_attach_naming_a_listed_earlier_hash_is_accepted_and_an_unlisted_one_is_catalog_mismatch() {
    let notes = Notes::default();
    let rt = Loopback::new(CATALOG);
    let mut t = MemberTable::new("ws.test", CatalogHash([7; 32]));
    t.add_compatible(CatalogHash([6; 32]));
    let backend = Described::new(Scanning(rt.attach()), Arc::new(t)).unwrap();
    let mut s = Session::new(backend, &config(&notes));
    let (step, out) = send(&mut s, &attach_msg(1, 1, [6u8; 32], "ws.test"));
    assert_eq!(step, Step::Continue);
    assert_eq!(decode(&out[0]).unwrap(), Frame::Attached(AttachOutcome::Accepted));

    let backend = Described::new(Scanning(rt.attach()), table()).unwrap();
    let mut s = Session::new(backend, &config(&notes));
    refused(&mut s, &attach_msg(1, 1, [5u8; 32], "ws.test"), Reason::CatalogMismatch);
}

/// Without a table the exact-match rule stays: the existing test
/// `the_same_name_with_another_hash_is_refused_catalog_mismatch` covers it and is unchanged.
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p ridl-transport-ws --lib` Expected: compile error on
`add_compatible`; after a stub returning `false`, the attach test fails with
`Refused(CatalogMismatch)`.

- [ ] **Step 3: Implement**

`MemberTable` gains `compatible: Vec<CatalogHash>`. In `Session::attach` the
hash branch becomes: refuse `CatalogMismatch` when
`!self.backend.members().map_or(*attach.hash() == self.catalog.hash, |t| t.accepts(attach.hash()))`.
The order of the four checks does not change (the existing test
`the_attach_checks_run_in_the_design_order` stays green).

- [ ] **Step 4: Run the crate's tests**

Run: `cargo test -p ridl-transport-ws` Expected: PASS.

- [ ] **Step 5: Docs**

Frame specification As-built note, first paragraph. Current (lines 28-30):

> The crate serves the observe and call ports of a provider runtime, applies the
> exact-match rule of §6.1 at `attach`, and keeps the last 64 settled responses
> of a session for duplicate suppression instead of keeping each until its
> response bound has passed (§7).

New:

> The crate serves the observe and call ports of a provider runtime. With a
> member table of its catalog, which the embedder supplies, it accepts an
> `attach` naming a listed earlier hash (§6.1); without one it applies the
> exact-match rule. It keeps the last 64 settled responses of a session for
> duplicate suppression instead of keeping each until its response bound has
> passed (§7).

(Task 3 rewrites the last sentence.)

Second paragraph (lines 42-45). Current:

> The catalog descriptor carries the list of compatible earlier catalog hashes
> that §6.1 names, and no runtime in this workspace reads it at `attach`.
> `ridl-transport-ws` holds no descriptor, so it applies only the exact-match
> rule of §6.1 and does not meet conformance item 1 of §12 until it reads one.

New:

> The catalog descriptor carries the list of compatible earlier catalog hashes
> that §6.1 names. `ridl-transport-ws` applies the list when its embedder gives
> it a member table that holds it, and meets conformance item 1 of §12 then;
> without a table it applies only the exact-match rule. `ridl-loopback` reads no
> descriptor.

§12 item 1, the parenthesis. Current: "(§6.1; the toolchain writes the list, and
no runtime in this workspace accepts a listed hash, per the As-built note)".
New: "(§6.1; the toolchain writes the list, and `ridl-transport-ws` with a
member table accepts a listed hash, per the As-built note)".

Design record D-11, third bullet. Current: "the name matches and the hash
differs: `refused(catalog_mismatch)`. This is the exact-match rule of §6.1,
because the server holds no descriptor;". New: "the name matches and the hash is
neither the catalog's own nor, with a member table (D-10), one of the table's
compatible hashes: `refused(catalog_mismatch)`. Without a table this is the
exact-match rule of §6.1;".

Crate rustdoc `lib.rs`, the sentence "An `attach` is accepted on an exact match
of the unit name and the catalog hash." becomes "An `attach` is accepted on a
match of the unit name and of the catalog hash, or of a compatible earlier hash
the [`MemberTable`] lists; without a table, on an exact match."

`docs/technotes/walking-skeleton-architecture.md:325`: "applies the exact-match
rule at `attach`, and" becomes "applies the exact-match rule at `attach` unless
the embedder gives it a member table with the compatible hashes, and" (Task 3
edits the rest of that sentence).

`docs/design/catalog-descriptor.md:292-298`, the bullet "**No runtime reads the
list.**" Current last sentence: "`ridl-transport-ws`, the one crate here that
speaks the frame, holds no descriptor and applies the exact-match rule
([its design record](../design/ridl-transport-ws.md); driftsys/ridl#985)." New
bullet title "**One runtime reads the list.**" and last sentence:
"`ridl-transport-ws`, the one crate here that speaks the frame, accepts a listed
hash when its embedder gives it a member table of the catalog, and applies the
exact-match rule otherwise ([its design record](../design/ridl-transport-ws.md),
D-11)." The first sentence of the bullet ("The generated face compares only its
own `CATALOG`") stays.

- [ ] **Step 6: Format, lint, commit**

Run:
`cargo fmt --all && cargo clippy -p ridl-transport-ws --all-targets -- -D warnings && just fmt && just check && just link-check && just doc-path-check && just story-id-check`

```bash
git add crates/ridl-transport-ws docs/
git commit -m "feat(ridl-transport-ws): accept a compatible earlier hash at attach"
```

---

### Task 3: Retention by bound, and the retained-bytes cap

Closes driftsys/ridl#986 (ruling D-2: the fallback is a stated deviation of this
crate, and §11.1 item 6 does not change).

**Files:**

- Modify: `crates/ridl-transport-ws/src/server/calls.rs` (whole file)
- Modify: `crates/ridl-transport-ws/src/server/session.rs` — `Session` fields
  (89-125), `new` (129), `poll_calls` (414), `request` (684), `respond` (743);
  the `tests` module
- Modify: `crates/ridl-transport-ws/src/server/config.rs` (`Config`, `Default`,
  a new error type)
- Modify: `crates/ridl-transport-ws/src/server.rs:105-115`
  (`Ended::ServerClosed` rustdoc), `:15` (re-export of the new error)
- Modify: `crates/ridl-transport-ws/src/ws.rs:246-258` (a `CloseCode::POLICY`
  constant, 1008)
- Modify: `crates/ridl-transport-ws/src/lib.rs:84-86` (re-export)
- Modify: `crates/ridl-transport-ws/tests/serve.rs` (test 11)
- Modify: `docs/specification/frame-specification.md` — As-built note (28-33),
  §7 last paragraph (709-714), §11.1 item 4 (1002)
- Modify: `docs/design/ridl-transport-ws.md` — D-16 "Duplicates", D-18 `Note`
  list and `Ended` table, D-23, the `Config` section, "What is not built"
- Modify: `docs/technotes/walking-skeleton-architecture.md:325-326`

**Interfaces:**

- Consumes: `MemberTable::member` (the `Option<Duration>` is the member's
  `max`), `Backend::members`, `Clock::now() -> Timestamp` of the backend
  (`ridl_rt::sample::Timestamp(pub i64)`, microseconds), `InFlight`, `Seen`,
  `Calls`, `Note::DuplicateDropped`.
- Produces:

```rust
// calls.rs
pub(super) struct InFlight {
    pub(super) correlation: Correlation,
    pub(super) target: Target,
    pub(super) seq: u64,
    pub(super) sent: Instant,
    /// The request's `stamp`, which the retention of its response is measured from (§7).
    pub(super) stamp: Timestamp,
    /// The backend's clock when the request arrived; caps the retention when `stamp` is ahead of it.
    pub(super) receipt: Timestamp,
}
impl Calls {
    /// Keeps the response sent under `seq`. With `Some(until)` it is kept until the backend's clock
    /// passes `until`; with `None` it is one of the last `RETAINED` such responses.
    pub(super) fn settle(&mut self, seq: u64, response: &[u8], until: Option<Timestamp>);
    /// Lets go every response kept until an instant before `now`.
    pub(super) fn expire(&mut self, now: Timestamp);
    /// The bytes of every response kept.
    pub(super) fn retained_bytes(&self) -> usize;
}
/// The number of settled responses kept when their bound is not known: a member the table
/// gives no `max`, or a server with no table. A stated deviation from frame specification
/// §11.1 item 6 (driftsys/ridl#986).
const RETAINED: usize = 64;

// config.rs
pub struct Config { /* ..., */ max_retained_bytes: usize /* private, default 16 * 1024 * 1024 */ }
impl Config {
    pub fn max_retained_bytes(&self) -> usize;
    /// Refused when `n` is below `max_message_bytes()`: set the message limit first.
    pub fn with_max_retained_bytes(mut self, n: usize) -> Result<Self, RetainedTooSmall>;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetainedTooSmall { pub given: usize, pub message_limit: usize }
impl fmt::Display for RetainedTooSmall; impl std::error::Error for RetainedTooSmall;

// ws.rs
impl CloseCode { pub(crate) const POLICY: Self = Self(1008); }
```

`RetainedTooSmall` is re-exported beside `LimitTooSmall`. The expiry rule: a
response settled with `Some(until)` is kept while `now <= until` and removed by
`expire(now)` when `now > until`.
`until = Timestamp(min(stamp.0, receipt.0).saturating_add(max_micros))` where
`max_micros = i64::try_from(max.as_micros()).unwrap_or(i64::MAX)`. Responses
settled with `None` are counted separately: when more than `RETAINED` of them
are kept, the oldest `None` entry goes. `settle` is called from
`Session::respond`, which returns `Step` from this task on: `Step::Close(1008)`
when `retained_bytes() > max_retained_bytes` after the settle, else
`Step::Continue`. `poll_calls` calls `expire(self.backend.ports().now())` first.

- [ ] **Step 1: Write the failing session tests (tests 7 to 10 of the note, and
      Review Focus items 3 and 4)**

Helpers in the `tests` module:
`fn request_at(target, seq, stamp: i64, payload) -> Vec<u8>` (as `request`, with
`stamp`), and a second command in the table,
`const COMMAND_UNBOUNDED: Target = Target { kind: Kind::Command, iface: IFACE, ord: Ordinal(7) };`
inserted by `table()` with `None`. `settled` is generalised over `B: Backend`
(it only needs `Session<B>` and the provider). The loopback's clock starts at 0;
`p.advance(Duration(micros))` moves it; the session reads it through the
backend's `Clock::now()`.

```rust
/// Mutations: never expire; expire at once.
#[test]
fn a_repeat_within_the_bound_gets_the_response_and_one_after_it_is_dropped() {
    let (mut s, mut p, notes) = described_scanning();
    open(&mut s);
    // COMMAND's max is 1 s = 1_000_000 us; the request is stamped at the clock, 0.
    let out = settled(&mut s, &mut p, COMMAND, 1, Ok(&[]));
    assert_eq!(out.len(), 1);
    p.advance(Duration(500_000));
    tick(&mut s);
    let (_, again) = send(&mut s, &request_at(COMMAND, 1, 0, &[7]));
    assert_eq!(again, out);
    p.advance(Duration(500_001)); // the clock is at max + 1
    tick(&mut s);
    let (_, again) = send(&mut s, &request_at(COMMAND, 1, 0, &[7]));
    assert!(again.is_empty());
    assert_eq!(*notes.lock().unwrap(), vec![Note::DuplicateDropped { seq: 1 }]);
}

/// Mutation: keep `RETAINED` for a bounded member.
#[test]
fn sixty_five_bounded_responses_are_all_kept_within_their_bound() {
    let (mut s, mut p, _n) = described_scanning();
    open(&mut s);
    let first = settled(&mut s, &mut p, COMMAND, 1, Ok(&[]));
    for seq in 2..=65 {
        settled(&mut s, &mut p, COMMAND, seq, Ok(&[]));
    }
    let (_, again) = send(&mut s, &request_at(COMMAND, 1, 0, &[]));
    assert_eq!(again, first);
}

/// Mutation: measure from the stamp only.
#[test]
fn a_request_stamped_an_hour_ahead_expires_at_receipt_plus_max() {
    let (mut s, mut p, notes) = described_scanning();
    open(&mut s);
    let ahead = 3_600_000_000; // one hour, in microseconds, while the clock is at 0
    let (_, out) = send(&mut s, &request_at(COMMAND, 1, ahead, &[]));
    assert!(out.is_empty());
    let c = claim(&mut p, COMMAND);
    p.settle(c.id, Ok(&[])).unwrap();
    let (_, out) = tick(&mut s);
    assert_eq!(out.len(), 1);
    p.advance(Duration(1_000_001)); // receipt (0) + max + 1
    tick(&mut s);
    let (_, again) = send(&mut s, &request_at(COMMAND, 1, ahead, &[]));
    assert!(again.is_empty());
    assert_eq!(*notes.lock().unwrap(), vec![Note::DuplicateDropped { seq: 1 }]);
}

/// Review Focus 3. Mutation: measure from the receipt only.
#[test]
fn a_request_stamped_far_behind_the_clock_keeps_its_response_until_stamp_plus_max() {
    let (mut s, mut p, notes) = described_scanning();
    open(&mut s);
    p.advance(Duration(3_600_000_000)); // the clock is at one hour
    let out = settled(&mut s, &mut p, COMMAND, 1, Ok(&[])); // stamped 0 by `request`
    assert_eq!(out.len(), 1);
    tick(&mut s); // 0 + max is long past: the next tick expires it
    let (_, again) = send(&mut s, &request_at(COMMAND, 1, 0, &[]));
    assert!(again.is_empty());
    assert_eq!(*notes.lock().unwrap(), vec![Note::DuplicateDropped { seq: 1 }]);
}

/// Mutation: drop a response with no bound at once.
#[test]
fn a_member_with_no_bound_falls_back_to_the_last_64() {
    let (mut s, mut p, notes) = described_scanning();
    open(&mut s);
    let mut responses = Vec::new();
    for seq in 1..=65 {
        responses.push(settled(&mut s, &mut p, COMMAND_UNBOUNDED, seq, Ok(&[])));
    }
    p.advance(Duration(3_600_000_000));
    tick(&mut s);
    let (_, again) = send(&mut s, &request_at(COMMAND_UNBOUNDED, 1, 0, &[]));
    assert!(again.is_empty());
    assert_eq!(*notes.lock().unwrap(), vec![Note::DuplicateDropped { seq: 1 }]);
    let (_, again) = send(&mut s, &request_at(COMMAND_UNBOUNDED, 2, 0, &[]));
    assert_eq!(again, responses[1]);
}
```

The existing test `the_retention_of_settled_responses_holds_the_last_64` (no
table) stays as the proof of the fallback without a table.

The cap, at the session level (the frame level is Step 7):

```rust
/// Review Focus 4. Mutation: close at `>=`.
#[test]
fn retained_bytes_equal_to_the_cap_do_not_close_and_one_more_byte_closes_1008() {
    let notes = Notes::default();
    let rt = Loopback::new(CATALOG);
    // A `response` under `contract(UnknownInteraction)` is 36 bytes. The cap cannot go below
    // the message limit, and the limit cannot go below 294: eight responses are 288 bytes,
    // the ninth passes 294.
    let config = config(&notes)
        .with_max_message_bytes(crate::frame::MIN_MESSAGE_LIMIT).unwrap()
        .with_max_retained_bytes(crate::frame::MIN_MESSAGE_LIMIT).unwrap();
    let mut s = Session::new(Scanning(rt), &config);
    open(&mut s);
    for seq in 1..=8 {
        let (step, out) = send(&mut s, &request(SIGNAL, seq, &[]));
        assert_eq!((step, out[0].len()), (Step::Continue, 36), "response {seq}");
    }
    let (step, out) = send(&mut s, &request(SIGNAL, 9, &[]));
    // The ninth response is still sent, then the session closes.
    assert_eq!((step, out.len()), (Step::Close(1008), 1));
}

#[test]
fn a_retained_cap_below_the_message_limit_is_refused() {
    let config = Config::default().with_max_message_bytes(2048).unwrap();
    assert_eq!(
        config.clone().with_max_retained_bytes(2047).err(),
        Some(RetainedTooSmall { given: 2047, message_limit: 2048 })
    );
    assert_eq!(config.with_max_retained_bytes(2048).unwrap().max_retained_bytes(), 2048);
}

#[test]
fn the_default_retained_cap_is_sixteen_mebibytes() {
    assert_eq!(Config::default().max_retained_bytes(), 16 * 1024 * 1024);
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p ridl-transport-ws --lib session::tests` Expected: compile
errors on `request_at`, `with_max_retained_bytes`, `Duration(..)` arguments;
after stubs, the retention tests fail on `again`.

- [ ] **Step 3: Implement `calls.rs`, the session changes, `Config`,
      `CloseCode::POLICY`**

`Session` gains `max_retained_bytes: usize` from the config. `request` computes
`receipt = self.backend.ports().now()` once, stores `stamp: envelope.stamp` and
`receipt` on `InFlight`, and passes both to `respond` for the at-once outcomes.
`respond(&mut self, target, correlation, outcome, stamp, receipt, out) -> Step`
computes `until` through a private helper
`fn retention(&self, target: Target, stamp: Timestamp, receipt: Timestamp) -> Option<Timestamp>`
(`self.backend.members()?.member(target.iface, target.ord)?.1` gives the
`Duration`), settles, and returns `Step::Close(CloseCode::POLICY.0)` when the
retained bytes pass the cap. Every caller of `respond` propagates a `Close`.

- [ ] **Step 4: Run the crate's unit tests**

Run: `cargo test -p ridl-transport-ws --lib` Expected: PASS.

- [ ] **Step 5: Write the failing `serve` test (test 11 of the note)**

In `tests/serve.rs`, with `tungstenite`'s `CloseCode::Policy`:

```rust
/// Mutation: count frames instead of bytes (1024 frames never arrive within the test limit).
#[test]
fn responses_past_the_retained_bytes_cap_close_1008_and_the_client_sees_it() {
    let (server, client) = pair();
    let config = config()
        .with_max_message_bytes(1024).unwrap()
        .with_max_retained_bytes(1024).unwrap();
    let handle = spawn(server, Scanning(Loopback::new(CATALOG)), config);
    let mut ws = connect(client, "ridl-ws-v1").unwrap();
    attach(&mut ws);
    // Each `request` of a signal kind is answered at once with a 36-byte `response`;
    // 28 of them are 1008 bytes, the 29th passes 1024.
    for seq in 1..=29u64 {
        send(&mut ws, Frame::Request { target: SIGNAL, envelope: Envelope { stamp: Timestamp(0), seq }, payload: &[] });
        assert_eq!(recv(&mut ws).len(), 36, "response {seq}");
    }
    assert_eq!(recv_close(&mut ws), Some(CloseCode::Policy));
    assert_eq!(handle.join().0, Ended::ServerClosed(1008));
}
```

- [ ] **Step 6: Run it to verify it fails, then passes**

Run:
`cargo test -p ridl-transport-ws --test serve responses_past_the_retained_bytes_cap`
Expected: FAIL before Step 3's `flush` path handles `Step::Close(1008)` (it
already does, through `Step::Close(code)`); PASS once Step 3 is complete. If it
passes at once, mutate `respond` to compare `>=` and confirm the 28th response
closes instead; restore.

- [ ] **Step 7: Docs**

`Ended::ServerClosed` rustdoc (`server.rs:106-114`). Current: "1009 on a message
over [`Config::max_message_bytes`]." New: "1009 on a message over
[`Config::max_message_bytes`], 1008 when the responses the session keeps for a
repeated `request` pass [`Config::max_retained_bytes`]."

`calls.rs` module and `RETAINED` comments: the Interfaces block above gives the
new `RETAINED` comment; the module comment becomes "the settled responses kept
for a repeated `request`, each until its bound has passed when the member table
gives one, and the last 64 otherwise".

Frame specification §7 last paragraph (709-714). Current:

> No shipped runtime retransmits. `ridl-loopback` delivers each call once
> because nothing in a process is lost. `ridl-transport-ws` suppresses a
> duplicate `request`: it keeps the last 64 settled responses of a session, and
> sends the kept response again for a `request` whose `seq` is among them. It
> does not know the response bound, because it holds no descriptor, so it fixes
> a count instead of retaining each response until the bound has passed.

New:

> No shipped runtime retransmits. `ridl-loopback` delivers each call once
> because nothing in a process is lost. `ridl-transport-ws` suppresses a
> duplicate `request` and keeps each settled response until the request's bound
> has passed, measured from the request's `stamp` or from the provider's clock
> at receipt, whichever is earlier, when its member table gives the member a
> bound. For a member the table gives no bound, or when the embedder gives it no
> table, it keeps the last 64 settled responses of the session instead. That
> fallback is a deviation of this crate, stated here and in its design record,
> and not a rule of the binding (driftsys/ridl#986).

§11.1 item 4 (line 1002). Current: "The binding defines no close code of its
own." New: "A provider may close with 1008 when a session passes a resource
limit it states, such as a cap on the responses it keeps for §7. The binding
defines no close code of its own." Item 6 does not change.

As-built note, first paragraph: the sentence "It keeps the last 64 settled
responses of a session for duplicate suppression instead of keeping each until
its response bound has passed (§7)." becomes "With a member table it keeps each
settled response until its bound has passed (§7); without one, or for a member
with no bound, it keeps the last 64."

Design record D-16 "Duplicates" bullet. Current (lines 477-488), from "one whose
`seq` is among the last 64 settled responses of the session gets that response
again" to the end. New: "one whose `seq` names a settled response still kept
gets that response again; any other `seq` not greater than the highest seen is
recorded (`DuplicateDropped`) and dropped. A response is kept, with the
responses sent at once (`busy`, a `contract`), until
`min(request.stamp, receipt) + max` by the backend's clock, where `receipt` is
`Clock::now()` when the request arrived and `max` the member's bound from the
member table (D-10); the `min` keeps §7's "measured from the request's `stamp`"
when the client's clock is behind, and caps the retention when a client stamps
ahead, so stamping requests in the future holds no response. Expired responses
go on each tick. For a member the table gives no bound (a catalog built before
the default response bound), or with no table, the server keeps the last 64 as
before: a stated deviation from §11.1 item 6 (driftsys/ridl#986, ruled a
deviation of this crate on 2026-10-23). The retained bytes of a session are
capped by `Config::max_retained_bytes` (16 MiB by default); a settle that would
pass it closes the session with 1008, which `serve` returns as
`Ended::ServerClosed(1008)`. Closing is observable, where dropping the oldest
response would silently break item 6."

D-18: the `Ended` table row `ServerClosed(code)` stays; D-23 "Exactly §11.1 item
4's:" list gains "1008 when the retained responses pass `max_retained_bytes`".
The `Config` section: "four settings behind methods", adding
`with_max_retained_bytes` (16 MiB; a value below `max_message_bytes` is refused
with `RetainedTooSmall`). "What is not built": remove "The retention of a
settled response for a provider without bounds, a question for the frame
specification (driftsys/ridl#986)."

`docs/technotes/walking-skeleton-architecture.md:325-326`: "and keeps the last
64 settled responses of a session." becomes "and keeps each settled response
until its bound when that table gives one, the last 64 otherwise."

- [ ] **Step 8: Format, lint, commit**

Run:
`cargo fmt --all && cargo clippy -p ridl-transport-ws --all-targets -- -D warnings && cargo test -p ridl-transport-ws && just fmt && just check && just link-check && just doc-path-check && just story-id-check`

```bash
git add crates/ridl-transport-ws docs/
git commit -m "feat(ridl-transport-ws): keep a settled response until its bound, under a retained-bytes cap

Closes driftsys/ridl#986: the count fallback is a stated deviation of this crate."
```

---

### Task 4: The `descriptor` feature

Closes driftsys/ridl#985.

**Files:**

- Modify: `Cargo.toml:168` (the workspace `ridl-descriptor` entry)
- Modify: `crates/ridl/Cargo.toml:31`, `crates/ridlc/Cargo.toml:16`
- Modify: `crates/ridl-transport-ws/Cargo.toml` (`[features]`, `[dependencies]`,
  a `[[test]]` table)
- Create: `crates/ridl-transport-ws/src/server/descriptor.rs`
- Create: `crates/ridl-transport-ws/tests/descriptor.rs`
- Modify: `crates/ridl-transport-ws/src/server.rs` (module and re-export under
  the feature), `src/lib.rs:73` (crate rustdoc) and `:84-86`
- Modify: `crates/ridl-transport-ws/tests/constraints.rs`
- Modify: `justfile`, the `compat-check` recipe (the dependency sets, the
  transport cells)
- Modify:
  `docs/decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md`
  (`## Status`, decision 6's table row)
- Modify: `docs/design/ridl-transport-ws.md` — constraints, D-1 table, D-2 table
  and prose, "Gates", "What it does not do", "What is not built", the
  "Alternatives considered" row
- Modify: `docs/book/stability.md:34-36`
- Modify: `docs/technotes/walking-skeleton-architecture.md:323-324`

**Interfaces:**

- Consumes: `MemberTable::{new, insert, add_compatible}` (Tasks 1 and 2),
  `ridl_descriptor::{verify, VerifyError, CatalogRef, InterfaceRef, MemberRef, TimingRef, Kind as DescriptorKind}`
  (the generated accessors return `planus::Result<T>`; `verify` has walked every
  field, so a read after it cannot fail on the structure).
- Produces:

```rust
// crates/ridl-transport-ws/src/server/descriptor.rs  (cfg(feature = "descriptor"))
impl MemberTable {
    /// Builds the table from the bytes of a `<unit>.catalog.binfb` file. The crate does no I/O.
    pub fn from_descriptor(bytes: &[u8]) -> Result<MemberTable, DescriptorError>;
}
#[derive(Debug)]
pub enum DescriptorError {
    /// `ridl_descriptor::verify` refused the buffer.
    Verify(VerifyError),
    /// The catalog hash, or a compatible hash, is not 32 bytes.
    HashLength { len: usize },
    /// An interface's `number` is 0.
    ZeroNumber { interface: String },
    /// A member's `ordinal` is 0.
    ZeroOrdinal { interface: String, member: String },
    /// A `max_us` that is not a decimal `u64`.
    MaxUs { iface: InterfaceNo, ord: Ordinal, text: String },
}
impl fmt::Display for DescriptorError; impl std::error::Error for DescriptorError;
impl From<VerifyError> for DescriptorError;
```

The walk: for each `interface` in `catalog.interfaces()`, for each `member` in
`interface.members()`:
`insert(InterfaceNo(number), Ordinal(ordinal), kind, max)` where `kind` maps the
descriptor's `Kind` to `ridl_rt::contract::Kind` by variant name (`Signal`,
`Event`, `Command`, `Query`, `Fixed`; the two enums have different
discriminants), and `max` is `timing.max_us` parsed with `str::parse::<u64>`
into `Duration::from_micros`, `None` when `timing` or `max_us` is absent.
`reserved_ordinals` and `retired` produce no entry. Each `compatible` hash goes
through `add_compatible`. `HashLength` is an addition to the design note's error
list: the schema types a hash as `[ubyte]`.

Manifest changes:

```toml
# Cargo.toml (workspace)
ridl-descriptor = { path = "crates/ridl-descriptor", version = "0.13.0", default-features = false }

# crates/ridl/Cargo.toml and crates/ridlc/Cargo.toml
ridl-descriptor = { workspace = true, features = ["std", "dedup"] }

# crates/ridl-transport-ws/Cargo.toml
[features]
default = ["server"]
server = []
# Reads a member table from a catalog descriptor. Needs Rust 1.88 (planus 1.3.0).
descriptor = ["server", "dep:ridl-descriptor"]

[dependencies]
ridl-rt.workspace = true
ridl-descriptor = { workspace = true, optional = true }

[[test]]
name = "descriptor"
required-features = ["descriptor"]
```

Cargo ignores `default-features = false` on a member entry when the workspace
entry does not set it (the reason `crates/ridl-descriptor/Cargo.toml` declares
`serde` directly), so the workspace entry turns the defaults off and the two
consumers that need them name `std` and `dedup`. The design note says
"`default-features = false`" without saying where; this is where.

- [ ] **Step 1: Write the failing manifest tests**

`tests/constraints.rs`: replace
`the_only_normal_dependency_is_ridl_rt_with_no_feature` and
`the_server_feature_turns_on_no_ridl_rt_feature` with:

```rust
/// The one unconditional dependency is `ridl-rt`, through a workspace entry that names no
/// feature; the one optional dependency is `ridl-descriptor`, through a workspace entry with
/// its default features off, under the `descriptor` feature.
#[test]
fn the_dependencies_are_ridl_rt_and_an_optional_ridl_descriptor() {
    let manifest = crate_file("Cargo.toml");
    assert_eq!(
        table(&manifest, "dependencies"),
        ["ridl-rt.workspace = true", "ridl-descriptor = { workspace = true, optional = true }"]
    );
    for form in ["[dependencies.", "[target.", "[build-dependencies"] {
        assert!(!manifest.contains(form), "a {form} table in the manifest");
    }
    let workspace = crate_file("../../Cargo.toml");
    let entries = table(&workspace, "workspace.dependencies");
    let rt = entries.iter().find(|l| l.starts_with("ridl-rt ")).expect("ridl-rt");
    assert!(!rt.contains("features"), "the workspace entry: {rt}");
    let descriptor = entries.iter().find(|l| l.starts_with("ridl-descriptor ")).expect("ridl-descriptor");
    assert!(descriptor.contains("default-features = false"), "the workspace entry: {descriptor}");
}

#[test]
fn the_features_are_server_and_descriptor() {
    let manifest = crate_file("Cargo.toml");
    assert_eq!(
        table(&manifest, "features"),
        ["default = [\"server\"]", "server = []", "descriptor = [\"server\", \"dep:ridl-descriptor\"]"]
    );
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p ridl-transport-ws --test constraints` Expected: FAIL on the
dependency table.

- [ ] **Step 3: Change the four manifests, then run**

Run:
`cargo test -p ridl-transport-ws --test constraints && cargo build -p ridl-cli -p ridlc && cargo test -p xtask oracle_boundary`
Expected: PASS; `ridl` and `ridlc` still build with the descriptor's `std` and
`dedup` on; the oracle-boundary graph tests still pass.

- [ ] **Step 4: Write the failing descriptor tests (test 13 of the note, Review
      Focus 5)**

`tests/descriptor.rs`, with a descriptor built in the test through
`ridl_descriptor::{finish, Catalog, Interface, Member, Timing, TimingMode, RetiredInterface, EarlierCatalog, Kind, SCHEMA_VERSION}`
(the owned types; `tests/verify.rs` of `ridl-descriptor`, `full_with`, is the
model). The catalog: name `ws.test`, hash `[7u8; 32]`, toolchain `"0.0.0"`,
interface `I` number 1 with members signal 1, event 2, command 3 (`max_us`
`"1000000"`), query 4 (`max_us` `"250000"`), fixed 5 (no timing), reserved
ordinal 6; a retired interface `R` number 9; compatible `[6u8; 32]`.

```rust
#[test]
fn a_descriptor_gives_the_members_their_kinds_and_bounds_and_no_entry_for_reserved_or_retired() {
    let t = MemberTable::from_descriptor(&descriptor()).unwrap();
    assert_eq!((t.name(), t.hash()), ("ws.test", &CatalogHash([7; 32])));
    assert_eq!(t.member(InterfaceNo(1), Ordinal(1)), Some((Kind::Signal, None)));
    assert_eq!(t.member(InterfaceNo(1), Ordinal(2)), Some((Kind::Event, None)));
    assert_eq!(t.member(InterfaceNo(1), Ordinal(3)), Some((Kind::Command, Some(Duration::from_secs(1)))));
    assert_eq!(t.member(InterfaceNo(1), Ordinal(4)), Some((Kind::Query, Some(Duration::from_millis(250)))));
    assert_eq!(t.member(InterfaceNo(1), Ordinal(5)), Some((Kind::Fixed, None)));
    assert_eq!(t.member(InterfaceNo(1), Ordinal(6)), None);
    assert_eq!(t.member(InterfaceNo(9), Ordinal(1)), None);
    assert_eq!(t.len(), 5);
    assert_eq!(t.compatible(), &[CatalogHash([6; 32])]);
}

#[test]
fn a_malformed_buffer_is_a_verify_error() {
    let mut bytes = descriptor();
    bytes.truncate(12);
    assert!(matches!(MemberTable::from_descriptor(&bytes), Err(DescriptorError::Verify(_))));
    assert!(matches!(MemberTable::from_descriptor(b"RDLC"), Err(DescriptorError::Verify(VerifyError::TooShort(4)))));
}

#[test]
fn a_max_us_that_is_not_a_decimal_u64_is_refused() {
    let bytes = descriptor_with(|c| c.interfaces[0].members[2].timing.as_mut().unwrap().max_us = Some("1s".to_owned()));
    assert!(matches!(
        MemberTable::from_descriptor(&bytes),
        Err(DescriptorError::MaxUs { iface: InterfaceNo(1), ord: Ordinal(3), ref text }) if text == "1s"
    ));
}

#[test]
fn a_zero_interface_number_and_a_zero_ordinal_are_refused() {
    let bytes = descriptor_with(|c| c.interfaces[0].number = 0);
    assert!(matches!(MemberTable::from_descriptor(&bytes), Err(DescriptorError::ZeroNumber { .. })));
    let bytes = descriptor_with(|c| c.interfaces[0].members[0].ordinal = 0);
    assert!(matches!(MemberTable::from_descriptor(&bytes), Err(DescriptorError::ZeroOrdinal { .. })));
}

#[test]
fn a_hash_that_is_not_32_bytes_is_refused() {
    let bytes = descriptor_with(|c| c.hash = vec![7; 31]);
    assert!(matches!(MemberTable::from_descriptor(&bytes), Err(DescriptorError::HashLength { len: 31 })));
}

/// Review Focus 5.
#[test]
fn a_repeated_or_own_hash_in_compatible_is_kept_once() {
    let bytes = descriptor_with(|c| {
        c.compatible = Some(vec![
            EarlierCatalog { hash: vec![6; 32] },
            EarlierCatalog { hash: vec![6; 32] },
            EarlierCatalog { hash: vec![7; 32] },
        ])
    });
    let t = MemberTable::from_descriptor(&bytes).unwrap();
    assert_eq!(t.compatible(), &[CatalogHash([6; 32])]);
    assert!(t.accepts(&CatalogHash([7; 32])) && t.accepts(&CatalogHash([6; 32])));
}

/// Each kind maps by name, not by discriminant (the two enums number them differently).
#[test]
fn each_kind_maps_to_the_same_named_kind() {
    for (d, r) in [
        (ridl_descriptor::Kind::Signal, Kind::Signal), (ridl_descriptor::Kind::Event, Kind::Event),
        (ridl_descriptor::Kind::Command, Kind::Command), (ridl_descriptor::Kind::Query, Kind::Query),
        (ridl_descriptor::Kind::Fixed, Kind::Fixed),
    ] {
        let bytes = descriptor_with(|c| c.interfaces[0].members[0].kind = d);
        assert_eq!(MemberTable::from_descriptor(&bytes).unwrap().member(InterfaceNo(1), Ordinal(1)).unwrap().0, r);
    }
}
```

`descriptor()` returns `finish(&catalog())`; `descriptor_with(f)` applies `f` to
the owned `Catalog` first. Mutation per field: drop the `interfaces` loop body
for one field (the kind, the ordinal, the `max_us`) and the matching assertion
goes red.

- [ ] **Step 5: Run to verify they fail**

Run: `cargo test -p ridl-transport-ws --features descriptor --test descriptor`
Expected: compile error, `from_descriptor` not found.

- [ ] **Step 6: Implement `descriptor.rs`; run**

`server.rs`: `#[cfg(feature = "descriptor")] mod descriptor;` and
`#[cfg(feature = "descriptor")] pub use descriptor::DescriptorError;`; `lib.rs`
re-exports `DescriptorError` under the feature.

Run:
`cargo test -p ridl-transport-ws --features descriptor && cargo test -p ridl-transport-ws && cargo clippy -p ridl-transport-ws --all-targets --features descriptor -- -D warnings`
Expected: PASS.

- [ ] **Step 7: The `compat-check` cells**

In the `justfile` recipe `compat-check`:

1. The dependency-set loop:
   `for set in "ridl-rt ridl-rt-conformance ridl-loopback ridl-transport-ws" "ridl-ir ridl-descriptor"`
   becomes one set,
   `"ridl-rt ridl-rt-conformance ridl-loopback ridl-transport-ws ridl-ir ridl-descriptor"`,
   because the transport now depends on `ridl-descriptor` (optional dependencies
   are normal dependencies to `cargo metadata`). Update the comment above
   `unpackaged_deps` accordingly.
2. The transport's `append_tables` call becomes
   `append_tables "$pkg_transport/Cargo.toml" "ridl-rt=$pkg" "ridl-descriptor=$pkg_descriptor" "ridl-ir=$pkg_ir"`:
   cargo resolves an optional dependency even when its feature is off.
3. The two existing 1.83 cells can no longer run `--offline` against the default
   `CARGO_HOME`, because the resolved graph now holds `planus`, `serde` and
   `ridl-ir`'s dependencies, and cargo 1.83 does not read the index the pin's
   cargo downloaded (the recipe's own comment on the `ridl-descriptor` cell).
   Give the transport its own home, the way the veh-cluster cell does:

   ```bash
   transport_home="$PWD/target/compat-check/cargo-home-transport"
   resolve_for "$transport_home" "$pkg_transport/Cargo.toml" "$minimum" ridl-transport-ws
   CARGO_HOME="$transport_home" cargo "+$minimum" fetch --locked --manifest-path "$pkg_transport/Cargo.toml"
   echo "compat-check: $minimum, edition 2021 (packaged ridl-transport-ws, default features)"
   CARGO_HOME="$transport_home" cargo "+$minimum" build --offline --locked --manifest-path "$pkg_transport/Cargo.toml"
   echo "compat-check: $minimum, edition 2021 (packaged ridl-transport-ws, no default features)"
   CARGO_HOME="$transport_home" cargo "+$minimum" build --offline --locked --no-default-features --manifest-path "$pkg_transport/Cargo.toml"
   ```

   `resolve_for` requires cargo to report "latest Rust 1.83 compatible version";
   `planus` is pinned `=1.3.0`, which has no 1.83-compatible release, so the
   fallback resolver selects 1.3.0 and still prints that line for the other
   dependencies. If `resolve_for` fails on this, the implementer reports it
   instead of weakening the check.
4. The new cell, at the descriptor's minimum, with the sources fetched by cargo
   1.88 into the descriptor's home (cargo 1.88 does not read the home cargo 1.83
   filled):

   ```bash
   CARGO_HOME="$descriptor_home" cargo "+$descriptor_minimum" fetch --locked --manifest-path "$pkg_transport/Cargo.toml"
   echo "compat-check: $descriptor_minimum, edition 2021 (packaged ridl-transport-ws, descriptor feature)"
   CARGO_HOME="$descriptor_home" cargo "+$descriptor_minimum" build --offline --locked --features descriptor --manifest-path "$pkg_transport/Cargo.toml"
   ```

5. The recipe's header comment on `ridl-transport-ws` ("must build with the
   minimum toolchain with its default features and with none") gains "and with
   the `descriptor` feature at `ridl-descriptor`'s minimum".

Run: `just compat-check` (several minutes; it downloads toolchains on first
run). Expected: every cell passes, the new one included.

- [ ] **Step 8: Docs**

ADR-0020, `## Status`: a new paragraph first among the amendments, dated the
PR's date: "**Amended <date> — decision 6, `ridl-transport-ws` reads the catalog
descriptor under a feature.** The `ridl-transport-ws` row said `ridl-rt` alone.
The crate gains the optional feature `descriptor`, off by default, which adds
`ridl-descriptor` with its default features off, the `no_std` reader, so that an
embedder can build the member table it gives the server from a
`<unit>.catalog.binfb` file (driftsys/ridl#985). With the feature on the crate
needs Rust 1.88, the minimum of planus 1.3.0; with it off the minimum stays 1.83
(ADR-0029). The row is changed in place; `ridl-loopback` is unchanged." Decision
6's table, the `ridl-transport-ws` row's last cell: "`ridl-rt`;
`ridl-descriptor` with its default features off, under the `descriptor` feature
(off by default, Rust 1.88 with it)".

Design record `docs/design/ridl-transport-ws.md`:

- Top paragraph, "decision 6 names the crate and fixes `ridl-rt` as its only
  dependency (amended 2026-10-19 ...)" becomes "decision 6 names the crate and
  fixes `ridl-rt` as its only unconditional dependency, with `ridl-descriptor`
  under the `descriptor` feature (amended 2026-10-19 and <date>)".
- "What it does not do": "It reads no catalog descriptor." becomes "It reads a
  catalog descriptor only under the `descriptor` feature, and does no I/O: the
  embedder reads or embeds the file."
- Constraints: after the `rust-version` bullet add "The `descriptor` feature
  needs Rust 1.88, the minimum of planus 1.3.0, which `ridl-descriptor` pins;
  `just compat-check` builds the feature at that version."
- D-1 table: a row `descriptor` (private module) / `std` / `descriptor`, off /
  "`MemberTable::from_descriptor` over `ridl-descriptor`". D-1 prose: the
  re-export list gains `Described`, `MemberTable`, `TableMismatch`,
  `RetainedTooSmall`, and `DescriptorError` under `descriptor`.
- D-2 title "No dependency but `ridl-rt`" becomes "No unconditional dependency
  but `ridl-rt`"; the table gains the row `ridl-descriptor` / "optional,
  `descriptor`" / "the catalog descriptor's verifier and accessors, default
  features off: planus and serde (`alloc`, `derive`) only, no `ridl-ir`, no
  `serde_json`, no `hashbrown`"; a sentence: "The workspace entry for
  `ridl-descriptor` turns its default features off, because Cargo ignores
  `default-features = false` on an inherited entry; `ridl` and `ridlc` name
  `std` and `dedup` themselves."
- "Gates": the `tests/constraints.rs` bullet states the new dependency shape;
  the `compat-check` bullet adds the descriptor-feature cell and the resolved
  lockfile. The "fails when the crate gains a workspace dependency outside
  `ridl-rt`, `ridl-rt-conformance` and `ridl-loopback`" clause adds
  `ridl-descriptor` and `ridl-ir`.
- "What is not built": remove the `driftsys/ridl#985` bullet. "Alternatives
  considered": the row "reading the catalog descriptor to check `kind` and
  bounds" moves from `deferred` to `built` with "under the `descriptor` feature,
  off by default, so an embedder that passes no table links no planus
  (driftsys/ridl#985)". The design note places these two rows in its step 5;
  this plan moves them here so the record does not describe a built feature as
  deferred between two PRs.

Crate rustdoc `lib.rs:73`: "It reads no catalog descriptor." becomes "It reads a
catalog descriptor only with the `descriptor` feature
([`MemberTable::from_descriptor`]), and does no I/O: the embedder reads
`<unit>.catalog.binfb`, written by `ridl build --emit catalog`, or embeds it
with `include_bytes!`. The feature needs Rust 1.88."

`docs/book/stability.md:34-36`: after "Rust 1.88 and edition 2024 for
`ridl-descriptor`" add ", and Rust 1.88 for `ridl-transport-ws` with its
`descriptor` feature on".

`docs/technotes/walking-skeleton-architecture.md:323-324`: "so `ridl-rt` is its
only dependency;" becomes "so `ridl-rt` is its only dependency outside the
optional `descriptor` feature, which adds `ridl-descriptor`;".

- [ ] **Step 9: Format, lint, commit**

Run:
`cargo fmt --all && just fmt && just check && just link-check && just doc-path-check && just story-id-check && cargo test -p ridl-transport-ws --features descriptor`

```bash
git add Cargo.toml Cargo.lock crates/ridl/Cargo.toml crates/ridlc/Cargo.toml crates/ridl-transport-ws justfile docs/
git commit -m "feat(ridl-transport-ws): read the member table from a catalog descriptor under a feature

Closes driftsys/ridl#985."
```

---

### Task 5: The examples read the table

Closes nothing; the last step of driftsys/ridl#985.

**Files:**

- Modify: `crates/ridl-transport-ws/tests/support/provider.rs` (a
  `member_table()` function)
- Modify: `crates/ridl-transport-ws/examples/loopback_server.rs:22,46-47`
- Modify: `crates/ridl-transport-ws/tests/scenario.rs:21,160-162`
- Modify: `examples/cabin/server/Cargo.toml` (the `ridl-transport-ws` entry),
  `examples/cabin/server/src/main.rs`, `examples/cabin/Cargo.lock`
- Modify: `justfile`, recipe `_cabin-generate` (line 781)
- Modify: `docs/design/ridl-transport-ws.md` — "Tests", the done-when and the
  loopback-server bullets; "What is not built" last bullet's second sentence

**Interfaces:**

- Consumes: `MemberTable::{new, insert, from_descriptor}`, `Described::new`,
  `Scanning`, the constants
  `CATALOG, IFACE, SIGNAL, EVENT, COMMAND, QUERY, FIXED` of
  `tests/support/provider.rs`.
- Produces:

```rust
// tests/support/provider.rs
/// The member table of [`CATALOG`]: the five members, the command and the query with a
/// bound of 1 s. Ordinal 6 is reserved and has no entry.
pub fn member_table() -> MemberTable;
```

- [ ] **Step 1: The scenario runs with a table (test 14 of the note)**

`tests/scenario.rs`, `provider_process`:
`let backend = Described::new(Scanning(rt.attach()), Arc::new(provider::member_table())).expect("the table matches the provider's catalog");`
`examples/loopback_server.rs` likewise, with one `Arc<MemberTable>` built before
the accept loop and cloned per connection.

Run: `cargo test -p ridl-transport-ws --test scenario` Expected: PASS, both
rigs. (A mismatch between the table and the scenario's ordinals would fail step
1 to 6 of `support::scenario::run` with `UnknownInteraction`, which is the point
of running it with a table.)

Run: `just ts-e2e` Expected: PASS. The TypeScript end-to-end tests name the same
catalog (`packages/ridl-transport-ws/e2e/catalog.ts`), so none of their frames
is unknown.

- [ ] **Step 2: The cabin server reads the descriptor**

`justfile`, `_cabin-generate`: the cabin build line becomes
`"$target/debug/ridl" build examples/cabin --emit rust --emit catalog --out-dir examples/cabin/generated`,
which writes `examples/cabin/generated/veh.cabin.catalog.binfb` beside the crate
(`examples/cabin/generated/` is gitignored).

`examples/cabin/server/Cargo.toml`: the `ridl-transport-ws` entry's features
become `["server", "descriptor"]`. Run
`cargo update --manifest-path examples/cabin/Cargo.toml -p ridl-transport-ws`
(or the narrowest command that makes `--locked` pass) and commit the lockfile
change, which adds `planus` and `serde` to it.

`examples/cabin/server/src/main.rs`: embed the descriptor, because reading it at
run time would need a path argument through `examples/cabin/web/main.ts` and
`server.ts`:

```rust
/// The catalog descriptor `ridl build --emit catalog` wrote beside the generated crate.
const DESCRIPTOR: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../generated/veh.cabin.catalog.binfb"));
```

In `main`:
`let table = Arc::new(MemberTable::from_descriptor(DESCRIPTOR).expect("the descriptor verifies"));`
and per connection
`let backend = Described::new(Scanning(rt.lock().expect("an unpoisoned runtime").attach()), Arc::clone(&table)).expect("the descriptor is the runtime's catalog");`.
The module comment gains one sentence: the server gives the transport the member
table read from that descriptor, so an `attach` naming a compatible earlier hash
is accepted and an unknown interaction is refused. The design note's step 5 says
the server "reads" the file; `include_bytes!` is the no-I/O form the note's
section 2 also allows, chosen so that the Deno and browser programs need no
change.

Run: `just cabin-ws-demo` Expected: PASS (needs Deno and flatc at the pinned
versions). If `flatc` is not installed locally, run the parts that do not need
it:
`cargo build --locked -p ridl-cli && target/debug/ridl build examples/cabin --emit rust --emit catalog --out-dir examples/cabin/generated && cargo clippy --manifest-path examples/cabin/Cargo.toml -p server --locked --all-targets --no-deps -- -D warnings && cargo fmt --manifest-path examples/cabin/server/Cargo.toml --check`
and leave `cabin-ws-demo` to CI.

- [ ] **Step 3: Docs**

`docs/design/ridl-transport-ws.md`, "Tests": the done-when bullet's
cross-process sentence "the child serves a loopback on a port of `127.0.0.1` it
prints" becomes "the child serves a loopback, wrapped in `Described` with the
hand-built member table of the test catalog, on a port of `127.0.0.1` it
prints"; the loopback-server bullet "Serves the scenario's test catalog and
provider on `127.0.0.1:0`" becomes "Serves the scenario's test catalog and
provider, with its member table, on `127.0.0.1:0`". "What is not built", the
last bullet's second sentence "`examples/cabin/server` already serves the
`temperature` signal to a TypeScript client." becomes "`examples/cabin/server`
already serves the `temperature` signal to a TypeScript client, with the member
table read from the descriptor `ridl build --emit catalog` writes."

`AGENTS.md`'s description of `just cabin-ws-demo` and `just demo` says what
`_cabin-generate` emits only as "generate examples/cabin's crate"; no change.

- [ ] **Step 4: Format, lint, commit**

Run:
`cargo fmt --all && cargo clippy -p ridl-transport-ws --all-targets --features descriptor -- -D warnings && cargo test -p ridl-transport-ws --features descriptor && just fmt && just check && just link-check && just doc-path-check && just story-id-check`

```bash
git add crates/ridl-transport-ws examples/cabin justfile docs/design/ridl-transport-ws.md
git commit -m "feat(ridl-transport-ws): give the example servers a member table"
```

---

## After the last task

- Run `/review` passes 1 and 2 over the whole branch (the per-task gate does not
  replace it).
- Garden: `sdd-gardening` moves the design note and this plan to `docs/archive/`
  once the five PRs are merged; the durable records are
  `docs/design/ridl-transport-ws.md` and the frame specification, both edited in
  place by the tasks above.
- Release: the next release is a minor one (new public items; no break). The
  Kotlin runtime (ridlc-gen-kotlin) is not affected: the member table is the
  Rust server's.

## Self-review notes

- Spec coverage: section 1 (Task 1), section 2 (Task 4), section 3 attach (Task
  2), routing (Task 1), retention and cap (Task 3), section 4 every doc line
  (Tasks 1 to 5, each quoted), section 5 tests 1 to 14 (1: T2; 2 to 6 and 12:
  T1; 7 to 11: T3; 13: T4; 14: T5), section 6 (the five tasks), decisions D-1
  (T3), D-2 (T3), D-3 (T4).
- Where this plan departs from the note: the `#985` "What is not built" bullet
  and the "deferred" alternatives row move from step 5 to Task 4; the cabin
  server embeds the descriptor instead of reading a path; `DescriptorError`
  gains `HashLength`; `default-features = false` is set on the workspace entry,
  with `ridl` and `ridlc` naming the features; `compat-check`'s transport cells
  gain a resolved lockfile and their own `CARGO_HOME`, which the note's "the
  msrv freeze dump does not change" sentence does not cover but the recipe's
  offline build requires.
