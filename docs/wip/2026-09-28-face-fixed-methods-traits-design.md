# The face's fixed methods behind traits — design note

**Status:** a design note for driftsys/ridl#580, awaiting the maintainer's
review. driftsys/ridl#580 was split from driftsys/ridl#570, where the direction
was decided. It settles the shape of the generated API; it does not implement
it. The `dispatch` locals, the other half of driftsys/ridl#570, merged
separately as driftsys/ridl#581. driftsys/ridl#568, driftsys/ridl#569 and the
rest of driftsys/ridl#571 are out of scope.

**Date:** 2026-09-28.

**Trace:** driftsys/ridl#580 (tracking); driftsys/ridl#570 (origin);
[ADR-0023](../decisions/ADR-0023-interaction-face-generation.md) decision 6 (the
two clients, `serve` and the poll face, which this note amends as decision 7);
[ADR-0016](../decisions/ADR-0016-schema-projection-and-the-name-transform.md)
decisions 1 and 4 (the name transform and the namespaces RIDL-149 checks);
[ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) decisions 8 and
10 (the `std` feature and what a `ridl-rt` release is);
[ADR-0018](../decisions/ADR-0018-runtime-core-and-generated-surface.md) decision
15 and
[ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
decision 7 (what a backend emits, and that a later language backend follows the
Rust face's precedent);
[the interaction-face design record](../design/interaction-face.md), section
"The consumer face"; [the `ridl-rt` design record](../design/ridl-rt.md),
section "Module layout".

## 1. The problem

Every fixed method of the generated face is an inherent method: `new` and
`next_event` on the async `Client`, those two plus `with_timeout` and
`set_timeout` on `blocking::Client`, `new` and `commit` on `Publisher`, and the
derived `subscribe_<event>` and `invalidate_<signal>`. The member methods are
inherent methods of the same types. Rust gives one type one inherent namespace,
so a member whose snake case is `new`, `commit` or `set_timeout`, or a member
`subscribeWarning` beside an event `warning`, is rustc E0592 (appendix, `e0`),
and `ridl check` accepts the source.

The maintainer decided the direction: fix the shape of the generated API, not
the set of accepted names. The fixed methods move into trait impls, the derived
methods into one generated trait per interface, and the member methods stay
inherent. A consumer's call sites do not change, apart from one `use` line.

## 2. The inventory

Every generated item, read from
`crates/ridl-backend-rust/tests/generated/interaction_face.rs` (emitted by
`crates/ridl-backend-rust/src/face.rs` and `src/face/*.rs`, and by the codec and
descriptor emitters for the items above the face modules). Two facts decide most
rows:

- A member, parameter or field name is projected through the pinned `snake_case`
  (ADR-0016 decision 1) for a method or a function, and through `camel_case` for
  a type or a variant (ADR-0016, consequences). A method name is therefore
  `[a-z][a-z0-9_]*`, and a type name `[A-Z][A-Za-z0-9]*`.
- A ridl identifier starts with a letter (`[A-Za-z][A-Za-z0-9_]*`,
  `crates/ridl-syntax/src/lexer.rs`; typl reference §2.3). No derived name
  starts with `_`, so an emitter-owned name that does — `__arg`, `__port`,
  `__deadline_after`, `__ridl_fb_encode_<type>` — cannot collide with one.

A derived name that carries a fixed prefix or suffix (`send_<m>`,
`<Member>Call`) cannot equal a bare fixed name (`serve`, `Client`), and two
derived names of one shape collide only when their sources collide, which
RIDL-149 refuses (ADR-0016 decision 3). The rows marked **collides** are the
subject of this note.

| Namespace                               | Item                                                                                                   | Fixed or derived  | Can it collide, and with what                                                                                                                                                                                                                                                                                                   |
| --------------------------------------- | ------------------------------------------------------------------------------------------------------ | ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| package module, types                   | `Wire` (type alias)                                                                                    | fixed             | Yes, with a typl type or an interface named `Wire`. Pre-existing, not a method, not this note's; ADR-0016 decision 4 leaves the type namespace unchecked. Listed in §8.                                                                                                                                                         |
| package module, types                   | `<Type>`, `<Type>FbView`, `<Iface>`, `<Iface><Member>`, `<iface>` (module)                             | derived           | Only with each other (a type named `TemperatureFbView`, an interface named like a type, a type named `CabinWarning`): derived against derived, the class driftsys/ridl#455 records. No fixed name is involved. Adding a fixed name here would add a collision (appendix, `e7`), which is why nothing in this design lives here. |
| package module, values                  | `__ridl_fb_encode_<type>`, `__ridl_fb_verify_<type>`, `__ridl_fb_decode_<type>`                        | fixed prefix `__` | No: the prefix starts with `_`.                                                                                                                                                                                                                                                                                                 |
| `<Type>` (newtype) inherent             | `new`, `check`, `new_unchecked`, `get`                                                                 | fixed             | No: a typl type declares no member, so the type has no derived method.                                                                                                                                                                                                                                                          |
| `<Type>`, trait impls                   | `TryFrom`, `From`, `Default`, `Payload` (`MAX_SIZE`, `View`, `encode`, `verify`, `decode`)             | fixed             | No: trait items live in their trait's namespace, and no derived item is in it.                                                                                                                                                                                                                                                  |
| `<Struct>FbView` inherent               | `bytes` (fixed); one accessor per field (derived)                                                      | mixed             | **Yes**, a struct field named `bytes` is E0592 — the same class as the face's, on the codec side. Not in driftsys/ridl#580's scope; the same fix shape (a `ridl_rt::payload` trait carrying `bytes`) applies. Listed in §8 for a new issue.                                                                                     |
| `<Iface>` inherent                      | `MAX_BUFFER_SIZE`, `EVENT_SOURCE_BUFFER_SIZE`                                                          | fixed             | No: the interface struct has no derived item; the `Interface` trait's constants are in the trait's namespace.                                                                                                                                                                                                                   |
| `<Iface><Member>`, trait impls          | `Interaction`, `Signal`, `Event`, `Command`, `Query` items                                             | fixed             | No: trait namespace, no inherent impl.                                                                                                                                                                                                                                                                                          |
| `<iface>` module, types                 | `Client`, `Event`, `NextEvent`, `Publisher`, `Provider`, `Serve`, `ServeState`, `blocking`             | fixed             | No: every derived type in the module carries a fixed suffix — `<Member>Call`, `<Member>Phase`, `<Member>Correlation` — so a bare fixed name equals none of them. This is the property that makes the module the home of every new fixed name in this design (`Subscribe`, `Invalidate`, `prelude`).                             |
| `<iface>::Event` variants               | `<Member>`                                                                                             | derived           | No fixed variant exists.                                                                                                                                                                                                                                                                                                        |
| `<iface>` module, values                | `serve`, `dispatch`, `poll_next_event` (fixed); `send_<m>`, `poll_<m>_ack`, `poll_<m>_reply` (derived) | mixed             | No: the derived names have a fixed prefix and (for `poll_`) a fixed suffix `_ack`/`_reply`; `poll_next_event` has neither suffix, so no member spells it.                                                                                                                                                                       |
| `<iface>::Client<P>` inherent           | `new`, `next_event` (fixed); `subscribe_<event>` (derived); `<member>` (derived)                       | mixed             | **Collides.** A member `new` or `nextEvent`; a member `subscribeWarning` beside an event `warning` (RIDL-149 does not refuse it: `subscribe_warning` and `warning` are distinct under `snake_case`).                                                                                                                            |
| `<iface>::blocking::Client<P>` inherent | `new`, `with_timeout`, `set_timeout`, `next_event` (fixed); `subscribe_<event>`; `<member>`            | mixed             | **Collides**, as the async client, plus a member `withTimeout` or `setTimeout`.                                                                                                                                                                                                                                                 |
| `<iface>::Publisher<W>` inherent        | `new`, `commit` (fixed); `invalidate_<signal>` (derived); `<member>` (derived)                         | mixed             | **Collides.** A member `new` or `commit`; a member `invalidateTemperature` beside a signal `temperature`.                                                                                                                                                                                                                       |
| `<iface>::Provider` trait               | `<member>`                                                                                             | derived           | No: the trait has no fixed method.                                                                                                                                                                                                                                                                                              |
| `<Member>Call`, `NextEvent`, `Serve`    | `expired`, `sent` (private inherent); `Future::poll`, `Drop::drop`                                     | fixed             | No: these types have no derived method.                                                                                                                                                                                                                                                                                         |
| `<iface>::blocking` module, values      | `serve` (fixed), `__deadline_after`                                                                    | fixed             | No: no derived value lives in the module.                                                                                                                                                                                                                                                                                       |
| struct fields                           | `port`, `inner`, `timeout`, `handler`, `provider`, `buf`, `state`, `phase`, `deadline`                 | fixed             | No: a field and a method are different namespaces in Rust, and every field is private.                                                                                                                                                                                                                                          |
| method locals                           | `dispatch`'s `claim`, `h`, `p`, `accepted`, `buf`, `reply`                                             | fixed             | Yes, with a parameter of that name — merged as driftsys/ridl#581, not this note.                                                                                                                                                                                                                                                |

Headline: three types collide — the two clients and `Publisher` — over five
fixed names (`new`, `next_event`, `with_timeout`, `set_timeout`, `commit`) and
two derived shapes (`subscribe_<event>`, `invalidate_<signal>`). One more
collision of the same class sits outside the face, `<Struct>FbView::bytes`.

## 3. The design

### 3.1 The fixed traits, in `ridl-rt`

A new module `ridl_rt::face` — "the traits a generated face implements" — beside
`contract`, `port` and `task` in `crates/ridl-rt/src/lib.rs`. It is `no_std`
like the rest of the crate and carries no dependency. Its names were checked
against everything `ridl-rt` exports today (`contract::Event`,
`sample::Duration`, `port::Attached` and the port traits,
`error::Transport::Timeout`): none is reused. `Bind` follows the crate's own
vocabulary, where the generated face is "the consumer's binding"
(`crates/ridl-rt/src/sample.rs`) and `new` "binds the face to a port".

```rust,ignore
/// Binds a face to the port it holds.
pub trait Bind {
    type Port;
    fn new(port: Self::Port) -> Self;
}
/// Takes the next occurrence of a subscribed event.
pub trait Events {
    type Next<'a> where Self: 'a;
    fn next_event(&mut self) -> Self::Next<'_>;
}
/// Bounds every waiting method of a blocking face by a wall-clock timeout.
#[cfg(feature = "std")]
pub trait Timeout: Sized {
    fn with_timeout(self, timeout: core::time::Duration) -> Self;
    fn set_timeout(&mut self, timeout: Option<core::time::Duration>);
}
/// Publishes every staged signal change.
pub trait Publish {
    fn commit(&mut self);
}
```

Who implements what: `Bind` — the async `Client<P>` (`Port = P`),
`blocking::Client<P>`, `Publisher<W>`; `Events` — both clients of an interface
that declares an event (`Next<'a> = NextEvent<'a, P>` for the async client,
`Result<Option<Event>, ReadError>` for the blocking one — one trait serves both
because the associated type is generic over the borrow); `Timeout` —
`blocking::Client<P>`; `Publish` — `Publisher<W>`. The generic associated type
is stable since Rust 1.65, below the crate's `rust-version` 1.83; appendix `e6`
compiles these four traits and both implementations of `Events` as a `no_std`
library at 1.83 and at the pin.

Gating: `std::time::Duration` is a re-export of `core::time::Duration`, so
`Timeout` needs no `std` to compile. It is gated under `std` anyway, because its
only implementor is the `blocking` module, which the emitted crate's `std`
feature enables together with `ridl-rt/std` (`crates/ridlc/src/lib.rs`, the
manifest template), and a trait with no possible implementor in a `no_std` build
is surface without a use. `Bind`, `Events` and `Publish` are ungated.

### 3.2 The per-interface traits, generated

Inside each interface module, with fixed names — the module is the one namespace
where a fixed name cannot meet a derived one (§2):

```rust,ignore
pub mod cabin {
    /// Starts delivery of one event of interface `Cabin`.
    pub trait Subscribe {
        fn subscribe_warning(&mut self) -> Result<(), SubscribeError>;
    }
    /// Stages the invalid state of one signal of interface `Cabin`.
    pub trait Invalidate {
        fn invalidate_temperature(&mut self) -> Result<(), WriteError>;
    }
    impl<P: ...> Subscribe for Client<P> { ... }
    impl<P: ...> Invalidate for Publisher<P> { ... }
    pub mod blocking { impl<P: ...> super::Subscribe for Client<P> { ... } }
}
```

`Subscribe` is emitted when the interface declares an event, `Invalidate` when
it declares a signal. A crate-root name such as `CabinEvents` was rejected:
`<Iface><Member>` descriptor structs live at the same level, so a member named
`events` gives E0428 (appendix, `e7`) — a fixed name at the root creates the
class of collision this note removes.

### 3.3 How a consumer gets the traits in scope

Recommended: **a generated `prelude` module per interface**, and no
`ridl_rt::prelude`.

```rust,ignore
pub mod cabin {
    pub mod prelude {
        pub use ::ridl_rt::face::{Bind, Events, Publish};
        #[cfg(feature = "std")]
        pub use ::ridl_rt::face::Timeout;
        pub use super::{Invalidate as _, Subscribe as _};
    }
}
```

The prelude re-exports the fixed traits the module's types implement: no
`Events` in a module with no event, no `Publish` in one with no `Publisher`, and
`Timeout` only where the `blocking` module is emitted. A consumer writes
`use api::cabin::prelude::*;` once per interface it uses and every call site
stays as it is: `cabin::Client::new(&mut port)`, `client.subscribe_warning()`,
`client.next_event()`, `publisher.commit()`,
`blocking::Client::new(&mut port).with_timeout(t)`. The reasons:

- The per-interface traits are generated, so `ridl_rt::prelude` cannot hold
  them; a `ridl_rt::prelude` would still leave a second `use` line per
  interface, and the maintainer's target is one line.
- The re-export `as _` puts a trait in scope for method resolution without
  binding its name, so two interfaces' `Subscribe` traits glob-imported together
  do not conflict, and `Bind` re-exported by both preludes is one item and does
  not conflict either (appendix, `e5a`). Importing the two `Subscribe` traits by
  name is E0252 (appendix, `e5b`), which is what the anonymous re-export avoids.
- The prelude lives in the interface module for the reason §2 gives: at the
  package module, `prelude` would collide with an interface named `Prelude`.
- The fixed traits are re-exported by name so that `<Client<_> as Bind>::new`
  (§4, fact c) can be written from the prelude alone.

### 3.4 The emitter's own calls

The blocking client calls the async one — `super::Client::new(port)`,
`self.inner.next_event()`, `self.inner.subscribe_warning()`
(`crates/ridl-backend-rust/src/face/blocking.rs`). Under this design those calls
are written through the trait:
`<super::Client<P> as ::ridl_rt::face::Bind>::new(port)`,
`::ridl_rt::face::Events::next_event(&mut self.inner)`,
`super::Subscribe::subscribe_warning(&mut self.inner)`. Otherwise a member named
`new` captures the emitter's own call (§4, fact c). Appendix `e6` shows the
first form.

## 4. The Rust resolution facts the design depends on

Each is proven by a file in the appendix, compiled at 1.83.0/edition 2021 and at
1.98.1/edition 2024. No result differs between the two; only the wording of one
E0599 message does.

| Fact                                                                                                                                                                                                                                                  | Experiment   | Result                                                                                              |
| ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------ | --------------------------------------------------------------------------------------------------- |
| The defect: a fixed and a member inherent method of one name.                                                                                                                                                                                         | `e0`         | E0592.                                                                                              |
| (a) An inherent member method wins a dot call over a trait method of the same name. `p.commit()` reaches the member; so does `c.subscribe_warning()` against the per-interface trait.                                                                 | `e1`         | Runs; prints the inherent one.                                                                      |
| (b) The trait method stays reachable: `Publish::commit(&mut p)`, `<Publisher as Publish>::commit(&mut p)`, `Subscribe::subscribe_warning(&mut c)`.                                                                                                    | `e1`         | Runs; prints the trait one.                                                                         |
| (c) With a member named `new`, `Client::new(port)` resolves to the inherent member and does not compile. Confirmed. The consumer writes `<Client<_> as Bind>::new(port)` or `let c: Client<_> = Bind::new(port)`.                                     | `e2a`, `e2b` | E0061 ("this function takes 2 arguments but 1 argument was supplied"); the qualified forms compile. |
| (c') With no member named `new`, `Client::new(port)` resolves through the trait when it is in scope — the call site is unchanged. Without the trait in scope it is E0599 with the help "items from traits can only be used if the trait is in scope". | `e3a`, `e3b` | Runs; E0599.                                                                                        |
| (d) Two traits in scope with one method name, both implemented for the receiver, make a dot call ambiguous; the path call is not.                                                                                                                     | `e4`         | E0034.                                                                                              |
| (e) A glob import of a `prelude` holding `pub use Trait as _` puts the trait in scope for method resolution; two interfaces' preludes together do not conflict; a fixed trait re-exported by both is one item.                                        | `e5a`        | Runs.                                                                                               |
| (e') Importing two interfaces' `Subscribe` by name is refused.                                                                                                                                                                                        | `e5b`        | E0252.                                                                                              |
| (f) The four traits, with `Events` implemented for a borrowing future and for an owned result, compile as a `no_std` library at 1.83.0 and at the pin.                                                                                                | `e6`         | Compiles.                                                                                           |
| (g) A per-interface trait named at the package module as `<Iface>Events` collides with the descriptor of a member named `events`.                                                                                                                     | `e7`         | E0428.                                                                                              |

Can the design produce (d)? No. The fixed traits' methods are `new`,
`next_event`, `with_timeout`, `set_timeout` and `commit`; the per-interface
traits' methods all start with `subscribe_` or `invalidate_`; the sets are
disjoint. A member method never takes part: (a) says the inherent method wins
before the trait candidates are compared. Two interfaces' `Subscribe` traits are
implemented for different types, so neither is a candidate for the other's
receiver (`e5a`).

The implementation PR turns these experiments into permanent tests:
`crates/ridl-backend-rust/tests/face_compile.rs` gains one case per colliding
name over an inline interface whose members are named `new`, `commit`,
`nextEvent`, `withTimeout`, `setTimeout`, `subscribeWarning` beside an event
`warning`, and `invalidateTemperature` beside a signal `temperature`, compiled
with `std` on and off as the existing case is; and one case that compiles a
consumer of two interfaces' preludes with the qualified calls of fact (c). The
round trips in `crates/ridl-backend-rust/tests/interaction_face.rs` prove the
trait methods behave as the inherent ones did.

## 5. Every place that changes

Found by grep for `Client::new(`, `Publisher::new(`, `subscribe_`,
`invalidate_`, `next_event(`, `with_timeout(`, `set_timeout(` and `commit()`
over `docs/`, `README.md`, `CONTRIBUTING.md`, `crates/` and `examples/`.

- `crates/ridl-rt/src/lib.rs` — `pub mod face;`; `crates/ridl-rt/src/face.rs` —
  new; `crates/ridl-rt/README.md` — its module paragraph names `face`;
  `docs/design/ridl-rt.md` — the "Module layout" table gains a row and a short
  section; ADR-0021 — an amendment adding decision 19 (the `face` module, and
  the release that carries it). `crates/ridl-rt-conformance/` is unchanged: it
  tests the port traits a runtime implements, and the face traits are
  implemented by generated code.
- `crates/ridl-backend-rust/src/face.rs` (the async client's `new`, `next_event`
  and `subscribe_<event>`, the publisher's `new`, `invalidate_<signal>` and
  `commit`, the two per-interface traits, the `prelude` module),
  `src/face/blocking.rs` (the blocking client's four fixed methods and its
  `subscribe_<event>`, and its calls into the async client, §3.4), and the doc
  comments in `src/face/futures.rs` and `src/face/poll.rs` that say
  `Client::next_event`.
- `crates/ridl-backend-rust/tests/generated/interaction_face.rs` — regenerated
  with `RIDL_UPDATE_GENERATED=1` through
  `tests/interaction_face_regeneration.rs`; `tests/interaction_face.rs` — one
  `use` line per interface module it drives; `tests/face_generation.rs` — the
  exact-text assertions on `pubfnsubscribe_warning`, `pubfnnext_event` and
  `pubfninvalidate_temperature` move to the trait impls; `tests/face_compile.rs`
  — the cases §4 names.
- `crates/ridlc/src/lib.rs` — the manifest template's `ridl-rt = "0.3"` literal
  (§6). Its guard, `crates/ridlc/tests/rust_crate_emit.rs` (near line 1113),
  derives the expected `major.minor` from the root workspace version, so the
  literal follows the release and the guard fails until it does.
- `examples/cabin/consumer/src/main.rs` — one `use api::cabin::prelude::*;`; the
  generated crate is written by `just demo` and is not checked in.
- `docs/decisions/ADR-0023-interaction-face-generation.md` — a dated amendment
  in `## Status` and a decision 7: "the fixed methods of the face are trait
  methods; the member methods stay inherent", naming the four `ridl-rt` traits,
  the two per-interface traits, the prelude, and fact (c) as the rule a consumer
  follows on a collision; decision 6's bullets for `<iface>::Client<P>`,
  `<iface>::blocking::Client<P>` and `next_event` are marked as superseded on
  those points. `docs/decisions/README.md` — the ADR-0023 summary line.
- `docs/design/interaction-face.md` — "The consumer face" (the `Client` listing,
  `Client::new(&mut port)` at line 159, `next_event` at 207), "The blocking
  module" (the listing at lines 386 to 390), the paragraph "The ridl-named
  argument is rebound to an emitter-owned name" (the two sentences that record
  this collision), and the "What is provisional" table.
- `docs/technotes/ridl-rt-by-example.md` — the `use` lines of its examples, and
  the call sites at lines 109, 240, 242, 267, 367 to 373, 556 and 775.
- `README.md` line 135 names `blocking::Client` and `blocking::serve` with no
  call site: no change. `docs/book/` has no face call site
  (`docs/book/cli-reference.md` names the face only): no change, and no
  `{{#include}}` of the two records above exists.
- `just compat-check` covers the change with no recipe change: it builds
  `ridl-rt` at 1.83 in both editions and the emitted cabin crate at 1.83 as
  edition 2021, which is what appendix `e5a` and `e6` anticipate.
  `just wasm-check` covers `ridl-rt` without `std`, so `face` must stay
  `no_std`, which it is.

## 6. Version impact

- **The generated API breaks**: a fixed method moves from an inherent impl to a
  trait impl, so a consumer with no `use` of the prelude gets E0599 (fact c').
  The backend commit is `feat(ridl-backend-rust)!:`. At 0.x a breaking commit is
  a minor bump of the workspace version, as 0.3.0 followed
  `feat(ridl-backend-rust)!` (1604b5c, E11.21's first half): the next release is
  0.4.0.
- **`ridl-rt` gains only additions** (a module and four traits): not a breaking
  change under ADR-0021 decision 10. `crates/ridl-rt/Cargo.toml` takes
  `version.workspace = true`, so the crate is released as 0.4.0 with the
  workspace, and that is the first published version with a `face` module.
- **The pin the emitted crate carries.** `crates/ridlc/src/lib.rs` writes
  `ridl-rt = { version = "0.3", ... }`, a caret requirement that accepts any
  published 0.3.x, none of which has `face`. Its guard
  (`crates/ridlc/tests/rust_crate_emit.rs`) expects the workspace's
  `major.minor`, so the release to 0.4.0 forces the literal to `"0.4"`, and
  every 0.4.x has `face`; the two move in the release commit, as the release
  procedure already bumps the pins. This is why the change must ride a minor
  release and not a patch: under a patch, `"0.3"` would still accept the
  published 0.3.0. Inside this workspace the gate is self-contained —
  `examples/cabin/Cargo.toml` patches `ridl-rt` to `crates/ridl-rt`, and
  `just compat-check` links the packaged crate by path — so the emitter change
  and the `ridl-rt` change can land as one pull request before the release. For
  a consumer outside the workspace, `ridl build` from the released toolchain
  emits a crate that resolves only once `ridl-rt` 0.4.0 is on crates.io:
  `cargo publish` of `ridl-rt` follows the tag at once, the order ADR-0021
  decision 18 used for E11.21's second half.
- **The Kotlin mirror** (driftsys/ridlc-gen-kotlin, which follows this face per
  ADR-0020 decision 7 and the roadmap's "Kotlin mirror" note) gets an issue
  carrying: the inventory of §2 (which names are fixed, which derived, and the
  rule that a member name must not be restricted by the backend); the decision
  that fixed operations are trait members and member operations are the type's
  own; the four `ridl-rt` trait names and the two per-interface names, so the
  two backends use one vocabulary; the prelude and the one `use` line; fact (a)
  and its Kotlin counterpart (a member function wins over an extension function
  of the same name, so the Kotlin equivalent is an extension or an interface
  default, decided there); and the test names of §4 as the cases its own compile
  tests cover.

## 7. Alternatives considered

| Alternative                                                                                     | Verdict  | Reason                                                                                                                                                                                                                                                                     |
| ----------------------------------------------------------------------------------------------- | -------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| (a) The Rust backend refuses a colliding member at `ridl build`                                 | rejected | A package that builds for TypeScript or proto would fail on Rust, and every future fixed method becomes a new refusal.                                                                                                                                                     |
| (b) A `ridl check` rule like RIDL-149                                                           | rejected | It puts one backend's API into the language, against the rule that an asymmetry between backends justifies a backend strategy, not a language rule.                                                                                                                        |
| A builder, or a separate type for the member methods                                            | rejected | An extra step at every call site.                                                                                                                                                                                                                                          |
| Renaming a colliding member with a suffix (`new_`), as prost does for keywords                  | rejected | A silent rename that depends on a list that grows with each release.                                                                                                                                                                                                       |
| Per-interface traits at the package module (`CabinEvents`, `CabinSignals`)                      | rejected | The package module holds the bare derived names `<Iface><Member>`; a member named `events` collides (appendix, `e7`). Inside the interface module every derived type carries a suffix, so no fixed name can collide there.                                                 |
| A `ridl_rt::prelude` instead of, or beside, a generated prelude                                 | rejected | It cannot carry the generated traits, so a consumer needs a second `use` line per interface anyway; one generated prelude per interface carries both, and the fixed traits are still nameable through it.                                                                  |
| The fixed methods on the per-interface trait too (one trait per interface, no `ridl-rt` change) | rejected | It works, but each interface would restate `new`, `next_event` and `commit` under its own trait, and a generic consumer (a test double, a harness over any `Client`) could not name the operation. The `ridl-rt` traits cost one release, which the pin bump needs anyway. |
| `Events` and `Timeout` as one trait, or `Timeout` ungated                                       | rejected | A signal-only or async client has no timeout, so one trait would force an implementation with no meaning; an ungated `Timeout` compiles (`core::time::Duration`) but has no implementor in a `no_std` build.                                                               |

## 8. Open questions for the maintainer

Every choice below was taken here and should be confirmed or changed.

1. The `ridl-rt` trait names: `Bind`, `Events`, `Timeout`, `Publish`, in a new
   module `ridl_rt::face`. (`Connect` was the example given; `Bind` follows
   `ridl-rt`'s "binding" vocabulary, and no existing export is reused.)
2. `Bind` with an associated type `Port` rather than a type parameter `Bind<P>`.
3. `Events` with a generic associated type `Next<'a>`, one trait for the async
   and the blocking client, rather than two traits.
4. The per-interface trait names `Subscribe` and `Invalidate`, inside the
   interface module.
5. The scope mechanism: a generated `prelude` module per interface, and no
   `ridl_rt::prelude`.
6. `Timeout` gated under `ridl-rt`'s `std` feature although
   `core::time::Duration` would let it compile without.
7. The consumer's rule on a real collision: the qualified form
   `<Client<_> as Bind>::new(port)` (fact c) is documented, and nothing is
   refused or renamed.
8. The `ridl-rt` change and the emitter change as one pull request before the
   release; the release is 0.4.0 (a minor, not a patch, §6), and `ridl-rt` 0.4.0
   is published to crates.io at the tag.
9. Two findings outside this note's scope: `<Struct>FbView::bytes` against a
   field named `bytes` (E0592, codec side), and the package module's fixed
   `Wire` against a type or interface named `Wire` (E0428). Neither is recorded
   on an issue that I found; the implementation PR could file both.

## 9. Implementation outline

1. `ridl-rt`: add `face.rs` with the four traits and their rustdoc,
   `pub mod face`, the README line, the `ridl-rt` design record's row. Proof:
   `just test`, `just wasm-check`, `just compat-check` (the crate builds without
   `std` and at 1.83); a doc test per trait.
2. The emitter, async side: move `new`, `next_event` and `subscribe_<event>`
   into `impl Bind`, `impl Events` and `impl Subscribe`; `Publisher`'s `new`,
   `commit` and `invalidate_<signal>` into `impl Bind`, `impl Publish` and
   `impl Invalidate`; emit `Subscribe`, `Invalidate` and `prelude`. Proof: the
   new `face_compile.rs` cases with the colliding members (they fail before,
   pass after), `face_generation.rs` assertions moved, the fixture regenerated,
   `interaction_face.rs` round trips green.
3. The emitter, blocking side: the same for `blocking::Client` and its calls
   into the async client (§3.4). Proof: the `std`-on half of the compile cases,
   and the blocking round trips.
4. The pin: the release commit raises the workspace version to 0.4.0 and the
   manifest literal to `"0.4"`. Proof: `crates/ridlc/tests/rust_crate_emit.rs`,
   `just demo`.
5. The consumers and the records: `examples/cabin/consumer`, the round-trip
   tests, `ridl-rt-by-example`, the interaction-face record, ADR-0023 decision
   7, ADR-0021 decision 19, `docs/decisions/README.md`. Proof: `just build`
   (`link-check`, `doc-path-check`, `demo`).
6. The Kotlin heads-up issue on driftsys/ridlc-gen-kotlin with §6's content.
7. Archive this note under `docs/archive/` when the records above are written.

## Appendix — the experiments

Every file below was compiled twice: with
`rustup run 1.83.0 rustc --edition 2021` (the `rust-version` of
`crates/ridl-rt/Cargo.toml`, edition 2021) and with
`rustup run 1.98.1 rustc --edition 2024` (the toolchain pin, edition 2024). A
binary that compiled was run. The results are the same on both, so one output is
shown per file, from 1.98.1; where a diagnostic message is worded differently on
1.83.0 the difference is noted. Lines starting with `-->`, `|`, `+`, `-` and the
"aborting" line are cut from the output.

### `e0_baseline_e0592.rs`

```rust,ignore
// The defect: a fixed inherent method and a member method of one name.
pub struct Publisher;
impl Publisher {
    pub fn commit(&mut self) {}           // the face's fixed `commit`
    pub fn commit(&mut self, _level: u8) {} // the member `commit`
}
fn main() {}
```

Output:

```text
error[E0592]: duplicate definitions with name `commit`
For more information about this error, try `rustc --explain E0592`.
did not compile
```

### `e1_inherent_wins.rs`

```rust,ignore
// (a) an inherent member method wins a dot call over a trait method of the
// same name; (b) the trait method is still reachable through a path call.
pub trait Publish { fn commit(&mut self) -> &'static str; }
pub trait Subscribe { fn subscribe_warning(&mut self) -> &'static str; }
pub struct Publisher;
pub struct Client;
impl Publisher {
    // the member `commit`, a command
    pub fn commit(&mut self) -> &'static str { "inherent Publisher::commit" }
}
impl Publish for Publisher {
    fn commit(&mut self) -> &'static str { "trait Publish::commit" }
}
impl Client {
    // the member `subscribeWarning`, a command
    pub fn subscribe_warning(&mut self) -> &'static str { "inherent Client::subscribe_warning" }
}
impl Subscribe for Client {
    fn subscribe_warning(&mut self) -> &'static str { "trait Subscribe::subscribe_warning" }
}
fn main() {
    let mut p = Publisher;
    let mut c = Client;
    println!("p.commit() -> {}", p.commit());
    println!("Publish::commit(&mut p) -> {}", Publish::commit(&mut p));
    println!("<Publisher as Publish>::commit(&mut p) -> {}", <Publisher as Publish>::commit(&mut p));
    println!("c.subscribe_warning() -> {}", c.subscribe_warning());
    println!("Subscribe::subscribe_warning(&mut c) -> {}", Subscribe::subscribe_warning(&mut c));
}
```

Output:

```text
compiled
p.commit() -> inherent Publisher::commit
Publish::commit(&mut p) -> trait Publish::commit
<Publisher as Publish>::commit(&mut p) -> trait Publish::commit
c.subscribe_warning() -> inherent Client::subscribe_warning
Subscribe::subscribe_warning(&mut c) -> trait Subscribe::subscribe_warning
```

### `e2a_new_member_path.rs`

```rust,ignore
// (c) `Client::new(port)` when a member method named `new` exists: the type
// path resolves to the inherent item, so the call does not compile.
pub trait Bind { type Port; fn new(port: Self::Port) -> Self; }
pub struct Client<P> { port: P }
impl<P> Client<P> {
    // the member `new`, a command taking one argument
    pub fn new(&mut self, level: u8) -> u8 { let _ = &self.port; level }
}
impl<P> Bind for Client<P> {
    type Port = P;
    fn new(port: P) -> Self { Client { port } }
}
fn main() {
    let port = 7u32;
    let _c = Client::new(port);
}
```

Output:

```text
error[E0061]: this function takes 2 arguments but 1 argument was supplied
note: expected `&mut Client<_>`, found `u32`
   = note: expected mutable reference `&mut Client<_>`
                           found type `u32`
note: method defined here
help: provide the argument
For more information about this error, try `rustc --explain E0061`.
did not compile
```

### `e2b_new_member_qualified.rs`

```rust,ignore
// (c) the same shape, with the consumer writing the qualified path.
pub trait Bind { type Port; fn new(port: Self::Port) -> Self; }
pub struct Client<P> { port: P }
impl<P> Client<P> {
    pub fn new(&mut self, level: u8) -> u8 { let _ = &self.port; level }
}
impl<P> Bind for Client<P> {
    type Port = P;
    fn new(port: P) -> Self { Client { port } }
}
fn main() {
    let port = 7u32;
    let mut c = <Client<_> as Bind>::new(port);
    let d: Client<u32> = Bind::new(port);
    println!("c.new(3) -> {} (the member); d.port = {}", c.new(3), d.port);
}
```

Output:

```text
compiled
c.new(3) -> 3 (the member); d.port = 7
```

### `e3a_new_via_trait_in_scope.rs`

```rust,ignore
// (c') with no member named `new`, `Client::new(port)` resolves through the
// trait when it is in scope: the call site is unchanged.
mod rt { pub trait Bind { type Port; fn new(port: Self::Port) -> Self; } }
mod cabin {
    pub struct Client<P> { pub port: P }
    impl<P> super::rt::Bind for Client<P> {
        type Port = P;
        fn new(port: P) -> Self { Client { port } }
    }
}
use rt::Bind;
fn main() {
    let c = cabin::Client::new(5u32);
    println!("cabin::Client::new(5u32).port -> {}", c.port);
}
```

Output:

```text
compiled
cabin::Client::new(5u32).port -> 5
```

### `e3b_new_trait_not_in_scope.rs`

```rust,ignore
// (c') the same call with the trait not in scope.
mod rt { pub trait Bind { type Port; fn new(port: Self::Port) -> Self; } }
mod cabin {
    pub struct Client<P> { pub port: P }
    impl<P> super::rt::Bind for Client<P> {
        type Port = P;
        fn new(port: P) -> Self { Client { port } }
    }
}
fn main() {
    let c = cabin::Client::new(5u32);
    println!("{}", c.port);
}
```

Output:

```text
error[E0599]: no associated function or constant named `new` found for struct `Client<P>` in the current scope
...
   = help: items from traits can only be used if the trait is in scope
help: trait `Bind` which provides `new` is implemented but not in scope; perhaps you want to import it
For more information about this error, try `rustc --explain E0599`.
did not compile
```

### `e4_two_traits_ambiguous.rs`

```rust,ignore
// (d) two traits in scope with one method name, both implemented for the
// type, make a dot call ambiguous.
pub trait Events { fn next_event(&mut self) -> &'static str; }
pub trait Other { fn next_event(&mut self) -> &'static str; }
pub struct Client;
impl Events for Client { fn next_event(&mut self) -> &'static str { "Events" } }
impl Other for Client { fn next_event(&mut self) -> &'static str { "Other" } }
fn main() {
    let mut c = Client;
    println!("{}", Events::next_event(&mut c)); // the path call is unambiguous
    println!("{}", c.next_event());
}
```

Output:

```text
error[E0034]: multiple applicable items in scope
note: candidate #1 is defined in an impl of the trait `Events` for the type `Client`
note: candidate #2 is defined in an impl of the trait `Other` for the type `Client`
help: disambiguate the method for candidate #1
help: disambiguate the method for candidate #2
For more information about this error, try `rustc --explain E0034`.
did not compile
```

### `e5a_prelude_glob.rs`

```rust,ignore
// (e) a glob import of a generated `prelude` module that re-exports the
// per-interface traits as `_` and the fixed traits by name puts every trait
// in scope for method resolution; two interface preludes glob-imported
// together do not conflict, because each `Subscribe` is anonymous and the
// fixed trait re-exported twice is one item.
pub mod rt {
    pub mod face {
        pub trait Bind { type Port; fn new(port: Self::Port) -> Self; }
        pub trait Events { fn next_event(&mut self) -> &'static str; }
    }
}
pub mod cabin {
    pub struct Client<P> { pub port: P }
    pub trait Subscribe { fn subscribe_warning(&mut self) -> &'static str; }
    impl<P> super::rt::face::Bind for Client<P> {
        type Port = P;
        fn new(port: P) -> Self { Client { port } }
    }
    impl<P> super::rt::face::Events for Client<P> {
        fn next_event(&mut self) -> &'static str { "cabin next_event" }
    }
    impl<P> Subscribe for Client<P> {
        fn subscribe_warning(&mut self) -> &'static str { "cabin subscribe_warning" }
    }
    pub mod prelude {
        pub use super::super::rt::face::{Bind, Events};
        pub use super::Subscribe as _;
    }
}
pub mod siren {
    pub struct Client<P> { pub port: P }
    pub trait Subscribe { fn subscribe_tripped(&mut self) -> &'static str; }
    impl<P> super::rt::face::Bind for Client<P> {
        type Port = P;
        fn new(port: P) -> Self { Client { port } }
    }
    impl<P> Subscribe for Client<P> {
        fn subscribe_tripped(&mut self) -> &'static str { "siren subscribe_tripped" }
    }
    pub mod prelude {
        pub use super::super::rt::face::Bind;
        pub use super::Subscribe as _;
    }
}
use cabin::prelude::*;
use siren::prelude::*;
fn main() {
    let mut c = cabin::Client::new(1u8);
    let mut s = siren::Client::new(2u8);
    println!("{} / {} / {}", c.subscribe_warning(), c.next_event(), s.subscribe_tripped());
    let c2: cabin::Client<u8> = bind(3); // `Bind` is nameable after both globs
    println!("bind::<cabin::Client<u8>>(3).port -> {}", c2.port);
}
fn bind<T: Bind<Port = u8>>(port: u8) -> T { T::new(port) }
```

Output:

```text
compiled
cabin subscribe_warning / cabin next_event / siren subscribe_tripped
bind::<cabin::Client<u8>>(3).port -> 3
```

### `e5b_two_named_imports.rs`

```rust,ignore
// (e) importing two interfaces' `Subscribe` traits by name is refused; this
// is why the prelude re-exports them as `_`.
mod cabin { pub trait Subscribe { fn subscribe_warning(&mut self); } }
mod siren { pub trait Subscribe { fn subscribe_tripped(&mut self); } }
use cabin::Subscribe;
use siren::Subscribe;
fn main() {}
```

Output:

```text
error[E0252]: the name `Subscribe` is defined multiple times
  = note: `Subscribe` must be defined only once in the type namespace of this module
help: you can use `as` to change the binding name of the import
warning: unused import: `cabin::Subscribe`
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default
warning: unused import: `siren::Subscribe`
For more information about this error, try `rustc --explain E0252`.
did not compile
```

### `e6_trait_shapes_no_std.rs`

```rust,ignore
// (f) the three fixed traits in the shapes the note proposes compile as a
// `no_std` library at the ridl-rt rust-version: `Bind` with an associated
// port type, `Events` with a generic associated type that is a borrowing
// future for the async client and an owned result for the blocking one, and
// `Timeout` over `core::time::Duration`, which is what `std::time::Duration`
// re-exports.
#![no_std]
pub mod face {
    pub trait Bind { type Port; fn new(port: Self::Port) -> Self; }
    pub trait Events {
        type Next<'a> where Self: 'a;
        fn next_event(&mut self) -> Self::Next<'_>;
    }
    pub trait Timeout: Sized {
        fn with_timeout(self, timeout: core::time::Duration) -> Self;
        fn set_timeout(&mut self, timeout: Option<core::time::Duration>);
    }
    pub trait Publish { fn commit(&mut self); }
}
pub mod cabin {
    use core::future::Future;
    use core::pin::Pin;
    use core::task::{Context, Poll};
    pub struct Event;
    pub struct Client<P> { port: P }
    pub struct NextEvent<'a, P> { port: &'a mut P }
    impl<'a, P> Future for NextEvent<'a, P> {
        type Output = Result<Event, ()>;
        fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
            let _ = &self.port; Poll::Pending
        }
    }
    impl<P> super::face::Bind for Client<P> {
        type Port = P;
        fn new(port: P) -> Self { Client { port } }
    }
    impl<P> super::face::Events for Client<P> {
        type Next<'a> = NextEvent<'a, P> where P: 'a;
        fn next_event(&mut self) -> NextEvent<'_, P> { NextEvent { port: &mut self.port } }
    }
    pub struct Publisher<W> { port: W }
    impl<W> super::face::Publish for Publisher<W> { fn commit(&mut self) { let _ = &self.port; } }
    pub mod blocking {
        pub struct Client<P> { inner: super::Client<P>, timeout: Option<core::time::Duration> }
        impl<P> super::super::face::Bind for Client<P> {
            type Port = P;
            fn new(port: P) -> Self {
                // the emitter's own call goes through the trait path, so a
                // member named `new` cannot capture it
                Client { inner: <super::Client<P> as super::super::face::Bind>::new(port), timeout: None }
            }
        }
        impl<P> super::super::face::Timeout for Client<P> {
            fn with_timeout(mut self, timeout: core::time::Duration) -> Self { self.timeout = Some(timeout); self }
            fn set_timeout(&mut self, timeout: Option<core::time::Duration>) { self.timeout = timeout; }
        }
        impl<P> super::super::face::Events for Client<P> {
            type Next<'a> = Result<Option<super::Event>, ()> where P: 'a;
            fn next_event(&mut self) -> Result<Option<super::Event>, ()> {
                let _ = super::super::face::Events::next_event(&mut self.inner);
                Ok(None)
            }
        }
    }
}
```

Output:

```text
compiled
```

### `e7_root_trait_name_collides.rs`

```rust,ignore
// A per-interface trait named at the crate root as `<Iface>Events` shares
// the root type namespace with the `<Iface><Member>` descriptor structs, so
// a member named `events` collides with it.
pub struct Cabin;
pub struct CabinEvents; // the descriptor of member `events` of interface `Cabin`
pub trait CabinEvents { fn subscribe_warning(&mut self); }
fn main() {}
```

Output:

```text
error[E0428]: the name `CabinEvents` is defined multiple times
  = note: `CabinEvents` must be defined only once in the type namespace of this module
For more information about this error, try `rustc --explain E0428`.
did not compile
```
