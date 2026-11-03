# The debug socket — design

**Status:** draft, 2026-11-03, for the owner's approval. **Satisfies:** E19.7
(driftsys/ridl#1191), the "MVP 1 in detail" section of Epic 19 in
`docs/ROADMAP.md`. **Related:** driftsys/ridl#988 (the stand-in ports of
`ridl-transport-ws`), the deployment descriptor record
(`docs/design/deployment-descriptor.md`), the `ridl-transport-ws` record
(`docs/design/ridl-transport-ws.md`), the frame specification §3 and §11.1.

This note is the design that the roadmap asks for before code starts. A separate
Codex session writes the implementation plan from it and builds it (see
`2026-11-03-debug-socket-codex-handoff.md`). Each choice that the roadmap does
not state is marked **(call)**: a decision taken in this note that the owner can
change while reading it.

## 1. Intent

A developer debugs a machine of a deployment from a browser or a test program,
over a WebSocket:

- **Listening:** see every signal and every event of every interface on the
  machine, as they change, without a change to any component.
- **Injection:** take the place of one instance N: stop N, and from the
  WebSocket client publish N's signals, raise N's events and answer the calls
  that the machine's consumers send to N.

The production build cannot do either. The identities that make both possible
(the twin −N and the listener `i32::MIN`) exist only behind a `debug` feature.

## 2. Goals, non-goals, anti-goals

Goals — the four parts of the roadmap, in order, each with the roadmap's "Done
when":

1. The identities in `ridl-rt`, behind a `debug` feature.
2. The attach rules: an attach carries an instance number, and a node grants it
   what the deployment descriptor says. The rules are in the port contract
   (`ridl-rt-conformance`) and in `ridl-loopback`.
3. Listening over the WebSocket, through a debug gateway.
4. Injection over the WebSocket, through a subprotocol with the roles reversed.

Non-goals:

- The node's checks of a production attach (E19.9): a production node that
  refuses a number not placed on its machine, or an interface the client claims.
  This design checks only what the debug rules need. **(call)** Write ownership
  per interface (`WriteError::NotOwner` for an instance that writes an interface
  it does not offer) is also E19.9; the only write refusal built here is the
  listener's.
- A runtime other than `ridl-loopback`. The gateway runs in the same process as
  the loopback node, because the loopback is the only runtime in the workspace.
  The book says so.
- Streams on the reversed role. The reversed subprotocol carries frame version 2
  forms only (see §6.2).
- A `ridl` subcommand that runs a gateway. The gateway is an example program and
  a test (see §5.2).
- A generated constant for the twin or the listener. ridl generates neither (the
  roadmap's identity rules).

Anti-goals — the work fails, even if every test passes, when:

- a production build (the `debug` feature off) can write a negative instance
  number, or attach as one;
- any of it is a breaking change to a published crate or to the wire (see §8):
  MVP 1 is meant to be additive, so that it can ship in any release;
- the frame changes. Only the binding's item 1 gains a subprotocol.

## 3. Part 1 — the identities in `ridl-rt`

- `ridl-rt` gains `InstanceNo` in `contract.rs`, beside `InterfaceNo`: a newtype
  over `i32` with a private field. **(call)** The name follows `InterfaceNo`.
  - `InstanceNo::new(n: i32) -> Option<InstanceNo>` returns `Some` for 1 to
    `i32::MAX` and `None` otherwise. `get(self) -> i32`.
  - Behind `debug` only: `InstanceNo::twin(self) -> InstanceNo` (−N for a
    positive N; the twin of a twin is not defined, so `twin` takes only a
    positive number — the builder chooses between a `Option` return and a type
    that cannot hold a negative number), `InstanceNo::LISTENER` (`i32::MIN`),
    and predicates `is_twin`, `is_listener`.
- The feature is named `debug`, off by default, and implies nothing else. It
  works without `std`.
- The generated constants stay `pub const NAME: i32` (MVP 0). **(call)** They
  are not changed to `InstanceNo`, because that changes generated code that
  consumers compile (a breaking change). A program writes
  `InstanceNo::new(instances::pkg::comp::PRIMARY)`.
- The crate that `ridl build` writes gains a `debug` feature that turns on
  `ridl-rt/debug` and nothing else. The generated freeze dumps under
  `freeze/generated/` gain that line.
- Done when (the roadmap's): a crate that names the twin or the listener does
  not compile without the feature, and compiles with it. **(call)** A
  `compile_fail` doctest under `cfg(not(feature = "debug"))` plus a normal test
  under `cfg(feature = "debug")`. Because `cargo test --workspace` unifies
  features, a crate that turns on `ridl-loopback/debug` turns on `ridl-rt/debug`
  for the whole run and compiles the doctest out. So the `test` recipe of the
  justfile gains a run of `-p ridl-rt` alone without `debug` (the only
  `-p
  ridl-rt` run today uses `--all-features`), and the plan shows that the
  doctest runs and fails as expected in that line.

## 4. Part 2 — the attach rules

### 4.1 The node

Today a `Loopback` is bound to one catalog, reads no descriptor, and gives write
access to any handle. A machine of a deployment carries several catalogs.

`ridl-loopback` gains a **node**: the runtime of one machine. **(call)** The
type is `Node`:

- `Node::from_descriptor(bytes: &[u8], machine: &str) -> Result<Node, NodeError>`
  verifies the deployment descriptor (`ridl_descriptor::deployment::verify`),
  keeps the instances placed on `machine`, and creates one `Loopback` per
  catalog that those instances offer or map. A catalog's `CatalogRef` is built
  from the descriptor's embedded catalog (name and hash). A machine name that
  the descriptor does not have is an error. **(call)** `bytes` is
  `&'static [u8]`, because `CatalogRef.name` is `&'static str`
  (`ridl-rt/src/contract.rs`); a generated crate includes its descriptor with
  `include_bytes!`, and a program that reads the file at run time leaks the
  buffer once (`Box::leak`), as the gateway example does.
- `Node::attach(&self, who: InstanceNo, catalog: &CatalogRef) -> Result<Attachment, AttachError>`
  returns an attachment: the `Loopback` handles of that catalog, plus the
  identity, released when the attachment is dropped.
- `Node::catalogs()` lists the machine's catalogs, so that a gateway can serve
  them.
- `ridl-loopback` gains a `debug` feature that turns on `ridl-rt/debug` and,
  with it, the twin and listener rules below. It needs `ridl-descriptor` for
  `Node`; the builder decides whether `Node` sits behind a feature of its own,
  as long as `Loopback` alone keeps its current dependencies.

### 4.2 The grant

- **N (positive).** N must be placed on the node's machine. N may attach to a
  catalog that N maps, or that holds an interface that N offers. Any other
  catalog is refused.
- **−N, the twin (debug).** N must be placed on the machine. −N receives N's
  grant. −N is refused while an attachment of N is alive, and N is refused while
  an attachment of −N is alive, in both orders. Two attachments of the same
  number at the same time are allowed. **(call)** A number is "attached" while
  at least one of its attachments is alive, across catalogs.
- **The listener (debug).** May attach to every catalog of the machine. Its
  attachment refuses every write: `SignalWriter::write` returns
  `WriteError::NotOwner`, `EventSink` returns `RaiseError::NotOwner`, and
  `Handler` serving returns `ServeError::NotOwner`. It may read, subscribe, and
  call (**(call)**, see open question Q2). Several listener attachments may be
  alive at once. `ridl-loopback` documents today that it never returns
  `NotOwner`; that sentence changes for the listener. How the refusal is built
  (an identity on the handles, or wrapper types returned by the attachment) is
  the builder's choice; the plan states it, because it is the `Runtime` type of
  the conformance hook.
- **Several instances that offer the same interface** (a redundant provider set)
  are all attached at once; only one of them is active, and the rule that
  chooses it belongs to the runtime (E19.12, not built). So the twin −N replaces
  N only: the other instances of the set stay attached, and −N is excluded with
  N alone, as the roadmap states. **(owner, 2026-11-03)** Until the active rule
  exists, the loopback node does not arbitrate: every attached instance of the
  set can write. The end-to-end tests use a component with one instance; the
  book page says that injecting on a redundant set before E19.12 needs the other
  instances stopped.
- Without the `debug` feature, a negative `InstanceNo` cannot be built, so no
  path accepts one.

### 4.3 The conformance tests

`ridl-rt-conformance` gains an extension group, **(call)** `identity`, so that a
runtime outside this repository opts in and nothing changes for one that does
not (additive). The group needs a factory hook that attaches as a given number
to a given catalog over a deployment the suite provides; the builder designs the
hook as an extension trait over `Factory`, following the existing `@ext` arms of
`suite!`. The suite carries its own small deployment descriptor fixture (one
machine, two instances, two catalogs, one map). The debug tests sit behind a
`debug` feature of `ridl-rt-conformance`.

Done when (the roadmap's): the conformance tests pass on `ridl-loopback` for the
twin's grant, the exclusion of N and −N in both orders, the listener's read of
every catalog, and the refusal of a write by the listener. **(call)** Add: N's
refusal of a catalog it neither offers nor maps, and the refusal of a number not
placed on the machine.

## 5. Part 3 — listening over the WebSocket

### 5.1 The transport: a backend chosen by the client's attach

`serve` takes one backend, already bound to a catalog. A gateway serves every
catalog of a machine on one port, and the client names the catalog in its
`attach`. `ridl-transport-ws` gains, beside `serve` (additive):

```rust
pub fn serve_selected<C, B, S>(conn: C, select: S, config: &Config) -> Ended
where C: Connection, B: Backend, S: FnOnce(&CatalogRef) -> Option<B>
```

**(call)** The name `serve_selected`. The session reads the client's `attach`,
calls `select` with the catalog it names, and answers `unknown_catalog` when
`select` returns `None`; the encoding and catalog checks of D-11 apply unchanged
after that. `serve` may become `serve_selected` with a selector that compares
the one backend's catalog; the builder decides.

`serve` chooses the highest frame version from the backend (`streams()`,
`members()`) and runs the `ConfigRefused` limit check before the handshake, and
with a selector the backend is not known then. **(call)** `serve_selected` takes
the capability in advance: the caller states the highest binding version it
serves (the gateway knows that every backend it builds is the same type), the
handshake selects with it, and a selected backend that cannot serve the selected
version is refused as `ConfigRefused` on that connection.

### 5.2 The gateway program

**(call)** The gateway is an example of `ridl-transport-ws`,
`examples/debug_gateway.rs`, built with the `debug` feature of `ridl-loopback`
(a dev-dependency there already). It:

1. reads a deployment descriptor and a machine name from its arguments;
2. builds the `Node` of that machine;
3. binds the **listening** listener and, when asked, the **injection** listener,
   each on its own port, and prints both ports;
4. on each listening connection, calls `serve_selected` with a selector that
   attaches as `InstanceNo::LISTENER` to the named catalog.

The gateway's access policy is the embedder's (D-7). The example applies none
beyond binding to `127.0.0.1`; its doc comment says that.

**(call)** The gateway logic that the tests and the example share (build the
node, select the backend) may live in a test support module; it is not a public
API in this MVP.

Done when (the roadmap's): an end-to-end test publishes a value on every
interface of a loopback machine, and a client attached through the gateway
receives each of them. **(call)** The test is Rust (`tungstenite` is already the
test client) and runs a machine with at least two catalogs. The listener's write
refusal is tested by the conformance suite, not here: a listening connection has
no `SignalWriter`, and a client `publish` already ends the session with `1002`
(`UnexpectedDirection`).

## 6. Part 4 — injection with the roles reversed

### 6.1 What the roadmap decided (2026-11-02)

- A subprotocol with the roles reversed: the client provides and the server
  consumes. The server sends `attach` first, the client answers `attached`, and
  the server still sends the pings. The frame does not change. The client
  publishes with `publish` and `occurrence` under their rules for provenance,
  `invalidate` and sequence numbers, and answers `request` with `response`.
- Injection has its own listener and port. A gateway that does not bind it
  cannot inject. The listening listener refuses the reversed subprotocol.
- Alternatives considered and rejected there: a debug control channel of the
  gateway; a consumer that publishes, as a new frame form.

### 6.2 The subprotocol

- **(call)** Name: `ridl-ws-v2-provider`. It reads as "frame version 2, the
  client is the provider". It carries the frame version 2 forms: no `chunk`,
  `stream_end` or `stream_credit`. A later version that adds streams to the
  reversed role is `ridl-ws-v3-provider`; that is not built.
- The client offers only reversed names, or only consuming names, never both:
  **(call)** a connection's role is fixed by what the client offers. A server
  that serves only one role selects only that role's names; when none matches,
  the handshake fails as §11.1 already says.
- §11.1 gains: item 1's reversed variant, the name, the order of the first two
  messages, and test vectors for the first messages of a reversed session (the
  server's `attach`, the client's `attached`, a client `publish`). The vectors
  go in a new "Reversed role test vectors" table, read by
  `crates/ridl-transport-ws/tests/vectors.rs`, by
  `packages/ridl-transport-ws/tests/vectors_test.ts`, and by the `wire` freeze
  dump. Existing rows do not change.
- Each side's checks mirror the existing ones with the roles swapped: the client
  checks the `attach` (frame version, catalog, encoding) and answers `attached`
  with the outcome; the server checks what the client sends as the existing
  client checks what a provider sends.

### 6.3 The Rust side (`ridl-transport-ws`)

As `docs/design/ridl-transport-ws.md` ("Adding the stand-in ports later")
foresees:

- D-20's selection becomes a table from subprotocol name to session role.
- The consuming role gets its own backend trait over
  `SignalWriter + EventSink + Handler` (plus what the session needs from
  `Attached`, `Encoded` and `Clock`). **(call)** The trait is `ProvidedBackend`
  and the entry point is `serve_provided(conn, backend,
  config)`, with a
  selector form like §5.1. The builder may choose better names; the plan states
  them.
- **What the server asks for.** **(call)** Right after `attached`, the server
  subscribes to every signal and every event of the interfaces that N offers in
  the session's catalog, as a consumer of those interfaces would; the client
  sends no `publish` or `occurrence` before that `subscribe`, as frame §5.1 and
  §5.2 require. The gateway reads the ordinals from the member table of the
  embedded catalog (`MemberTable::from_descriptor`, the `descriptor` feature).
- **How a client `publish` reaches `SignalWriter`.** **(call)**
  - A frame with provenance `Init` or `Update` is a `set` (or an `invalidate`
    for an absent value); `Init` exists only because the frame requires the
    first value after `subscribe`, and the node does not distinguish it.
  - The client's `stamp` is not used: the node stamps with its own clock at
    `commit`, as for every writer on the machine.
  - Consecutive frames that carry the same client stamp form one commit (frame
    §5.1, "a commit is a set of frames with one stamp"); the server calls
    `commit` when the stamp changes or when no frame arrives within one tick.
- The server forwards an `occurrence` to `EventSink`, and serves calls that
  consumers on the machine send to N by sending `request` to the client and
  settling with its `response`. `Handler::serve` is given the command and query
  ordinals of N's offered interfaces in the catalog.
- This closes driftsys/ridl#988.

### 6.4 The TypeScript side (`@driftsys/ridl-transport-ws`)

The package gains the providing side, additive to `connect`: **(call)**
`provide(url, options): Promise<Provision>`, which offers `ridl-ws-v2-provider`,
waits for the server's `attach`, answers `attached`, and exposes `publish`,
`raise`, `invalidate` and a handler for requests. It reuses the existing frame
encoders. Deno unit tests cover it; the end-to-end test of §6.6 drives it, or a
Rust `tungstenite` client does when the TypeScript e2e setup makes that cheaper
(the builder decides, the plan states which).

### 6.5 How the gateway chooses the instance and the catalog

With the roles reversed the server sends `attach` first, so the gateway must
know, before the client sends anything, which instance N the client replaces and
which catalog the session carries. **This was question Q1** (§9). The
recommendation, taken:

- the injection listener reads the request path,
  `/<component>/<instance>/<catalog>` (the qualified component name, the
  instance name, the catalog name). The binding still places nothing in the URL:
  the path is the deployment's, as §11.1 item 1 allows, and only the gateway
  reads it.
- `ridl-transport-ws` passes the request path to the embedder's selector of the
  provided role (the handshake already parses it, D-19, and ignores it).
- The gateway looks the instance up in the descriptor, attaches as its twin −N
  to that catalog, and refuses the handshake (HTTP `409`) when −N is refused (N
  attached), or `404` when the path names nothing on the machine.

### 6.6 Done when

The roadmap's: the binding section states the reversed subprotocol and its test
vectors; an end-to-end test with N detached injects a signal value and an event
and serves a call through the gateway's injection listener, a consumer on the
machine receives each of them, and an attach of N while −N writes is refused;
and the listening listener refuses the reversed subprotocol. **(call)** "Serves
a call" replaces the roadmap's "injects a command": see Q2.

## 7. Delivery

**(call)** Three PRs, in this order, each with a per-task review gate and a
branch review:

| PR | Content           | Main crates                                                                |
| -- | ----------------- | -------------------------------------------------------------------------- |
| A  | Parts 1 and 2     | `ridl-rt`, `ridl-loopback`, `ridl-rt-conformance`, `ridl-backend-rust`     |
| B  | Part 3            | `ridl-transport-ws`                                                        |
| C  | Part 4, and §11.1 | `ridl-transport-ws`, `packages/ridl-transport-ws`, the frame specification |

PR C changes the same files as the parameter streams over the WebSocket (streams
stage 5b, not on the roadmap yet). The two must not be built at the same time;
PR C goes after stage 5b merges, or stage 5b after PR C.

The records gardened when the work lands: the deployment descriptor record
(attach rules, the node), the `ridl-transport-ws` record (D-20 table, the
provided role, `serve_selected`; the "Adding the stand-in ports later" section
becomes as-built), the frame specification §11.1, the `ridl-rt` record
(`InstanceNo`, `debug`), and a book page on debugging a machine. The roadmap's
E19.7 row gets its landed note.

## 8. Compatibility

Every change is meant to be additive:

- `ridl-rt`: a new type and a new feature.
- `ridl-loopback`: a new type and a new feature; `Loopback` unchanged.
- `ridl-rt-conformance`: a new opt-in group; existing groups unchanged.
- `ridl-transport-ws`: new functions and a trait; `serve` keeps its signature
  and behaviour.
- The generated crate: one new feature.
- The wire: a new subprotocol name; the `wire` dump gains rows and loses none.

These crates are in the `runtime-crates` freeze surface, checked with
`cargo semver-checks`. A step that the builder cannot make additive stops and
reports it; it is not done as a breaking change without the owner's word.

## 9. Owner answers (2026-11-03)

- **Q1 — how the injection gateway knows N and the catalog** (§6.5): the request
  path `/<component>/<instance>/<catalog>`, read by the gateway only.
  Alternative kept on record: one injection port per (instance, catalog), chosen
  when the gateway starts.
- **Q2 — one debug client listens and injects.** A test-harness client opens a
  listening connection, which observes and also sends commands and queries, and
  an injection connection, which publishes, raises and answers the calls sent to
  N. The listener refuses every write (signals, events, serving). The injection
  end-to-end test "serves a call" (the client answers a call sent to N); the
  listening end-to-end test also sends a command.
- **Q3 — a redundant provider set:** the instances of a set are all attached and
  one is active (§4.2). −N is excluded with N only; the node does not arbitrate
  between the instances of a set until the active rule (E19.12) exists.

## 10. Alternatives considered

| Alternative                                    | Verdict  | Why                                                                                                         |
| ---------------------------------------------- | -------- | ----------------------------------------------------------------------------------------------------------- |
| Generated constants typed `InstanceNo`         | rejected | changes the generated code that consumers compile; the plain `i32` constant converts with `InstanceNo::new` |
| Instance identity on `Loopback` itself         | rejected | a `Loopback` is one catalog; the grant is about the machine's catalogs, so it belongs to the node           |
| One listening port per catalog                 | rejected | the client already names the catalog in `attach`; `serve_selected` uses it                                  |
| The reversed role at frame version 3           | deferred | streams on the reversed role are not needed to debug and would collide with the parameter streams work      |
| A `ridl gateway` subcommand                    | deferred | the only runtime is in process; a subcommand would need a runtime it can reach                              |
| Write ownership per interface for every attach | deferred | E19.9, the node's checks                                                                                    |
