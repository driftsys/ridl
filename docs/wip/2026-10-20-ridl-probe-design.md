# A generated probe: one client of every interface — design

Status: draft for review, 2026-10-20. Tracking issue: driftsys/ridl#1007.

## Goal

Give a debug build and an integration test one component that can read every
signal, receive every event and call every command and query of a system,
without listing each interface by hand, and without any of it reaching a release
build.

## What the user asked for

- A debug or test component that can reach every service.
- Enabled only in debug builds.
- As simple as possible. A reserved identity (for example a maximum number) that
  a release runtime rejects was considered first and set aside (see
  "Alternatives considered").
- Spying on calls between other components, and mocking a provider, are left out
  of this design. The section "Out of scope" records what each one needs.

## What exists

- **Reaching an interface needs no wiring at the port level.** A `Caller` sends
  to an `InterfaceNo` and an `Ordinal`, never to a provider. `ridl-loopback`
  routes a call to the handler that serves the member, and checks no link: a
  handle may read any signal, subscribe to any event and call any member of its
  catalog ([`ridl-loopback.md`](../design/ridl-loopback.md), "What it cannot
  report").
- **One loopback aggregate implements every port role.** `Loopback<E>`
  implements the twelve port traits, among them `SignalReader`, `EventSource`,
  `Caller`, `Clock` and `Wakeable` (`crates/ridl-loopback/src/lib.rs`).
- **Each port trait is implemented for `&mut P`**
  (`crates/ridl-rt/src/port.rs`), so a generated `Client` can be built over a
  borrowed port.
- **The generated crate already carries a typed client for every interface.**
  Each face module has `Client<P>`, built through `Bind::new(port)`, which
  checks the port's catalog and panics on a mismatch. The bounds on `P` are only
  the roles the interface's own interactions need.
- **The member table already exists.** Each interface's descriptor type
  implements `ridl_rt::contract::Interface`, with `CATALOG`, `NUMBER`, `NAME`
  and `MEMBERS` (ordinal, kind, name, timing, payloads). No new table is needed.
- **A per-interface `Client` cannot share an event source with another
  interface.** Its event step reads the next occurrence from the port and, when
  the occurrence's `iface` is not its own, returns
  `ReadError::Contract(UnknownInteraction)`. The occurrence has then been taken
  from the port. A component that subscribes to the events of several interfaces
  on one source therefore needs one step that reads an occurrence and routes it
  by interface number. That step is what the probe adds beyond a set of clients.

## Decisions

### D-1. The Rust backend emits the probe

The probe is emitted by `ridl-backend-rust`, in the generated crate, and not by
a codegen plugin.

- The probe is a thin layer over items the backend already emits in the same
  crate: the face modules, the descriptor types and the event decoding.
- A plugin would emit code that names the backend's items, so it would have to
  reproduce the name transform (ADR-0016) and the item layout, and would fail to
  compile when either changes. Inside the backend one generator owns both sides.
- It is emitted by the same `ridl build`, so a probe cannot be out of step with
  the faces of the build.

The cost is a larger generated surface, frozen once released (D-7), and one
probe per language backend. The TypeScript backend is not in scope.

### D-2. The names: module `ridl_probe`, struct `Probe`, feature `ridl-probe`

- **Module `ridl_probe` at the crate root.** The crate root holds one module per
  first package segment (`veh`, `ridl`), so the probe's module needs a name that
  no package can produce:
  - `debug` and `probe` can be produced by a package named `debug.x` or
    `probe.x`.
  - `ridl::probe` can be produced by a package named `ridl.probe`. Only
    `ridl.std` is reserved (TYPL-010), and the typl reference states that
    `ridl.stdlib` is an ordinary package name.
  - `ridl_probe` cannot be produced in a crate build. A crate build reads a
    manifest; a package name is the manifest's `[package] name` followed by the
    directory path (the manifest `veh.common` gives the package `veh.common`);
    and MANI-006 restricts each segment of a manifest name to `[a-z][a-z0-9]*`.
    Checked on 2026-10-20: a manifest with `name = "ridl_probe"` is MANI-006.
    Single-file mode accepts `package ridl_probe`, but it emits one
    `ridl_probe.rs` and no `lib.rs`, so there is no crate root to share.
  - The backend still rejects, as an internal error, a build whose top-level
    package module would be `ridl_probe`, so that a later change to the naming
    rules cannot produce two modules with one name.
- **Struct `Probe<P>`.** `Debug` would be read as `core::fmt::Debug`, which the
  generated code derives, and `dbg` as the `dbg!` macro.
- **Cargo feature `ridl-probe`.** Feature names are separate from module names;
  the prefix keeps it apart from `std` and `validate-pattern`.
- **The word.** The probe attaches to a running system, reads and calls. It does
  not start components or replace them, which is what "harness" usually means,
  and "harness" also names libtest and Cargo's `harness = false`.

### D-3. Off by default, absent from a release build

`ridl-probe` is not in the generated crate's `default` features. Every probe
item is under `#[cfg(feature = "ridl-probe")]`. With the feature off, the
compiled crate is the same as one emitted without the probe. A release build
does not turn the feature on; nothing at run time checks for it.

The feature implies nothing about `std`. The probe allocates nothing and
compiles under `no_std` like the faces.

### D-4. One probe per catalog

A port is bound to one catalog, and one generated crate can hold the packages of
several units, each with its own catalog. The backend emits one `Probe<P>` per
catalog in the build, in a submodule of `ridl_probe` named by the unit's name
through the same segment spelling as the package modules
(`ridl_probe::veh::cluster::Probe`). A build with one catalog still uses the
submodule, so that adding a second unit does not move the first probe.

`Probe::new(port)` checks the port's catalog against the catalog's `CatalogRef`
once and panics on a mismatch, as `Bind::new` does.

### D-5. What a probe holds and offers

`Probe<P>` holds the port by value. `P` must implement the union of the roles
the catalog's faces need. A `Loopback` aggregate, or `&mut` one, satisfies any
union.

- **A typed client per interface.** One method per interface that has a face,
  named by the face module's name, returns that interface's `Client<&mut P>`:
  `probe.wheel_history().read_speed()`. The client borrows the probe's port for
  as long as it lives. Its catalog check runs when it is built, which a debug
  build can afford.
- **Every event, routed.** `subscribe_all()` subscribes to every event member of
  every interface in the catalog. `next_event()` reads one occurrence and
  returns `Event`, an enum with one variant per interface that declares an
  event, each wrapping that interface's own `Event`. An occurrence whose `iface`
  matches no interface is `ReadError::Contract(UnknownInteraction)`, as in the
  face. To make this possible, the face's event step is split in two: reading
  the occurrence from the port, and decoding an occurrence already read. The
  decoding half becomes a `pub(crate)` function the probe calls.
- **The catalog's members.** `Probe::INTERFACES` lists, for each interface, its
  `NAME`, `NUMBER` and `MEMBERS`, taken from the descriptor types. It is what a
  tool prints to show what it can reach.

Signals and calls need nothing beyond the typed clients. An interface for which
the backend emits no face (`__RIDL_NO_FACE_<Iface>`) gets no method and no
variant, and is still listed in `INTERFACES`.

### D-6. No change to the runtime, the IR or rsdl

The probe is not declared in rsdl and adds no `requires`. Nothing in `ridl-rt`,
`ridl-loopback`, the IR or the codegen protocol changes. A runtime that enforces
the links rsdl derives would refuse the probe's calls; giving it a grant behind
the same feature is left until such a runtime exists.

### D-7. Gates and stability

- **Compiled and linted.** `just compat-check` builds the corpus crate with
  every feature on, so it compiles the probe at the frozen minimum Rust version.
  `just demo` checks and lints the generated crates with their default features
  and with them off, so it does not compile the probe today; its clippy and
  `cargo doc` lines for the corpus crate gain `--features ridl-probe`, and the
  `thumbv7em-none-eabihf` check gains one run with `ridl-probe` and the default
  features off.
- **Frozen dump.** The `generated-crate` check of `just freeze-check` compares
  the emitted files byte for byte, so the probe's source is in the dump whether
  or not the feature is on. Adding it passes, because only removed lines fail.
- **Semver check.** The `generated-crate` semver check runs
  `cargo semver-checks` with the default features, so a non-default feature's
  items are not compared. Once released, the probe is a frozen surface by policy
  (ADR-0029 decision 1, "Generated Rust crate"), but no tool would detect a
  break. The implementation adds the probe's feature to that check, or the
  ADR-0029 table records the probe as checked by review only. Open item 2.

### D-8. Verification

- `crates/ridlc/tests/rust_crate_emit.rs`: the emitted `Cargo.toml` declares
  `ridl-probe` outside `default`; the emitted `lib.rs` declares `ridl_probe`
  under the feature's `cfg`.
- A round trip over `ridl-loopback` with the `examples/cabin` catalog: a probe
  reads a signal, receives events of two interfaces in the order raised through
  one `next_event`, and completes a command and a query.
- The internal-error check of D-2, with a test.

## Out of scope

- **Spying on calls between other components.** A third party cannot see a call
  between a consumer and its provider. It needs a port that wraps the provider's
  or the consumer's port and records each call, inserted by the generated wiring
  in a debug build.
- **Mocking a provider.** The port API has no way to withdraw a member a handler
  serves (`serve` only adds), and two handlers serving one member race for each
  call. A mock therefore replaces the provider: either the provider is not
  started, or a wrapping port answers the mocked members before they reach it.
- **Calls chosen at run time from text.** A console that takes "interface,
  member, arguments" as text needs a text form of every payload. The generated
  crate has no text format and no `serde` dependency. Open item 1.
- **The TypeScript backend.**

## Alternatives considered

- **A reserved component or service identity (for example `u32::MAX`) that a
  release runtime rejects.** No component or service identity travels: rsdl
  instances are indistinguishable on the wire (V-02), and there is no
  `ServiceId` (ADR-0021 decision 1). A reserved value needs a sender identity in
  the frame first, which every release build would carry. A run-time refusal
  also keeps the debug code in the release binary and stops only a caller that
  uses the reserved value. Removing the code at build time gives the same
  guarantee with no wire change.
- **A debug component declared in rsdl.** It can `requires` every interface, one
  line each, with no wildcard. A component in the `system` must be placed in
  every deployment (RSDL-701), so the model cannot leave it out of production.
  Declaring it `external` keeps it out of every distribution, but uses
  `external` for something other than its meaning.
- **A codegen plugin.** See D-1.
- **A probe driven by the catalog descriptor at run time.** The descriptor has
  no payload layouts (D-7 of the catalog descriptor design, not built), so a
  generic client could not encode or decode a payload.
- **`ridl::probe`, with `ridl.probe` reserved by TYPL-010.** It gives a shorter
  path at the cost of a language change; `ridl_probe` needs none.
- **The names `debug`, `dbg` and `harness`.** See D-2.

## Open items

1. **A text form for calls.** Whether a later version adds `call_by_name` with a
   text payload format, and which format, given that the generated crate has no
   dependency for one.
2. **The semver check of a non-default feature.** Whether `cargo semver-checks`
   for the `generated-crate` surface runs with `--all-features`, which would
   also check `validate-pattern` and `std`, or with `--features ridl-probe`.

## Records to amend

- [`docs/design/interaction-face.md`](../design/interaction-face.md): a section
  on the probe, and the split of the event step (D-5).
- ADR-0029 decision 1: the "Generated Rust crate" row names the probe, and
  decision 6's gate table records how it is checked (open item 2).
- `docs/book/generated-code.md`: the `ridl-probe` feature and an example.
- The `justfile` recipes `compat-check` and `demo` (D-7), and `AGENTS.md`'s
  description of `just demo`.

## Trace

- Conversation of 2026-10-20 with the maintainer: the request, the rejection of
  a reserved identity, the choice of the Rust backend over a plugin, the names,
  and leaving spying and mocking out.
- Evidence: `freeze/generated/cabin/veh.cabin.rs` (a face's event step and its
  `UnknownInteraction` return), `freeze/generated/veh-cluster/veh.cluster.rs`
  (`Client`, `Bind::new`, the descriptor type),
  `crates/ridl-core/src/manifest.rs` (`is_valid_package_name`),
  `crates/ridl-rt/src/port.rs` (the `&mut P` implementations),
  `crates/ridl-rt/src/contract.rs` (`Interface`).
