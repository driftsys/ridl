# B2 — a typed TypeScript client face over port interfaces (design note)

Status: approved by the owner on 2026-11-11 (questions 1, 2 and 5 answered by
the owner; questions 3 and 4 decided by the author as recommended in section 9).
Implementation starts after this note merges.

Issue: driftsys/ridl#1262. Lane: `docs/wip/2026-11-08-typescript-lane-driver.md`
(task B2; B0 was settled on 2026-11-09 as option 1, port interfaces in
`@driftsys/ridl-runtime`). Depends on A1 (#1252) and A2 (#923), both merged.
Blocks D1 (#290); D2 (#291) and D3 (#288) stay out of the lane until B2 merges.

Records read for this note: ADR-0020 decision 7, ADR-0021, ADR-0023,
`docs/design/interaction-face.md`, `docs/design/ts-codec-table.md`,
`docs/design/ridl-transport-ws-ts.md`, `crates/ridl-rt/src/port.rs`,
`packages/ridl-runtime/`, `packages/ridl-transport-ws/`, `examples/cabin/web/`
and `examples/docstore/web/`.

## 1. The problem

ADR-0020 decision 7 says `@driftsys/ridl-runtime` carries "the port interfaces
spelled in TypeScript, and the loader", and that the generated faces sit on it.
Today the package holds the loader (`loadCodec`), the errors, the constructor
helpers, the signal check types and the stream adapters. The only port-shaped
type is `StreamTransport`, a structural interface for the four byte-stream
methods. Nothing in TypeScript corresponds to the Rust `Caller`, `SignalReader`
or `EventSource` ports.

The generated `<base>.codec.ts` module gives, per interface, a table
(`export const Cabin`) with each member's `iface`, `ord`, `kind`, payload codecs
and `limit`, plus `signalChecks(codec)`. Typed functions exist only for stream
calls (`DocStore_fetch(session, ...)`, taking a `StreamSession`).

What a TypeScript program writes by hand today to use the cabin package
(`examples/cabin/web/main.ts` and `check.ts` do all of it):

- Compare the server's catalog hash with `CATALOG_HASH`.
- Load the codec and pass `signalChecks(codec)` and the tables to `connect`.
- Send a command: `Cabin.setLevel.request.encode(value)`, then
  `session.command(Cabin.setLevel.iface, Cabin.setLevel.ord, bytes, { bound })`.
  Nothing verifies the request bytes with the wasm codec before sending.
- Call a query: the same, then `codec.verify(...)` and
  `Cabin.average.reply.decode(bytes)` on the reply.
- Read a signal: `session.subscribe(...)`, then filter the single global
  `onPublish(target, sample)` callback by `iface` and `ord`, verify, decode, and
  handle `init` and `invalid` samples (whose `payload` is `null`) itself.
- Receive an event: filter the single global `onOccurrence` callback by ordinal
  and decode.
- Choose a bound for every call, although the catalog declares one
  (`setLevel @[..50ms]`, `average @[..200ms]`).

Each of D1, D2 and D3 would write this same code again. The Rust face
(`Client<P>` in `docs/design/interaction-face.md`) already removes all of it for
Rust programs.

## 2. Decision summary

1. `@driftsys/ridl-runtime` gets consumer port interfaces: `Caller`,
   `SignalReader`, `EventSource` and `StreamCaller` (which replaces
   `StreamTransport`), and the call error type `CallError`. They are
   Promise-based, not poll-based, but have the same split by interaction kind as
   the Rust ports.
2. `ridl-backend-ts` generates one client class per interface,
   `<Interface>Client`, in `<base>.codec.ts`. It is built with
   `<Interface>Client.bind(port, codec)` and has one method per command, query,
   signal, event and supported stream member.
3. `Session` of `@driftsys/ridl-transport-ws` implements the four ports. The
   package takes `@driftsys/ridl-runtime` as a dependency (ADR-0020 decision 6
   already permits it).
4. The free stream functions `<Interface>_<member>` and `StreamSession` are
   removed; streams become methods of the client class.
5. B2 is a breaking change of generated TypeScript and of both packages, and
   ships alone as the next minor release after the release that carries A1 to
   A3.
6. The provider side (typed `provide`, the `Handler`, `SignalWriter` and
   `EventSink` ports) is not part of B2. D1 adds it, on the same pattern.

## 3. The port interfaces (`packages/ridl-runtime/src/ports.ts`)

The ports carry bytes and addresses only. They never name a payload type, a
codec or a transport. The interface type parameter of each Rust port has the
same role as the number pair `(iface, ord)` here.

```ts,ignore
export interface CallOptions {
  /** Milliseconds: finite, > 0, <= 2147483647. */
  readonly bound: number;
  /** Aborting rejects the call with the signal's reason. */
  readonly signal?: AbortSignal;
}

export interface Attached {
  /** The 32-byte hash of the catalog the port is attached to. */
  readonly catalogHash: Uint8Array;
}

export interface Caller extends Attached {
  command(iface: number, ord: number, args: Uint8Array, opts: CallOptions): Promise<void>;
  query(iface: number, ord: number, args: Uint8Array, opts: CallOptions): Promise<Uint8Array>;
}

export interface SignalReader extends Attached {
  checkWith(iface: number, ord: number, check: SignalCheck): void;
  subscribe(iface: number, ord: number, opts?: WaitOptions): Promise<void>;
  unsubscribe(iface: number, ord: number): void;
  /** The held sample, or undefined before the first publish. */
  sample(iface: number, ord: number): RawSample | undefined;
  /** Waits for the next publish of the signal. */
  read(iface: number, ord: number, opts?: WaitOptions): Promise<RawSample>;
  /** Called for every accepted publish of the signal; returns the removal. */
  watch(iface: number, ord: number, listener: (s: RawSample) => void): () => void;
}

export interface EventSource extends Attached {
  subscribe(iface: number, ord: number, opts?: WaitOptions): Promise<void>;
  unsubscribe(iface: number, ord: number): void;
  /** Called for every accepted occurrence of the interface, in order. */
  listen(iface: number, listener: (o: RawOccurrence) => void): () => void;
}

export interface StreamCaller extends Caller {
  // The four methods of today's StreamTransport, unchanged except that
  // StreamOptions becomes CallOptions.
}
```

`RawSample` and `RawOccurrence` move from the transport (`Sample`, `Occurrence`,
`Envelope`) into the runtime unchanged in shape; the transport re-exports them
under their current names. `WaitOptions` moves the same way.

**Why these methods.** They are the methods `Session` already has, with the
target spelled as `(iface, ord)`, plus three additions: `catalogHash`, `watch`
and `listen`. `watch` and `listen` replace filtering the one global `onPublish`
and `onOccurrence` callback of `connect`; those callbacks stay, because a tool
such as the observability server (D3) wants every publish. A runtime may
implement only the ports its consumers need, as in Rust (RA-19).

**What is not a port.** `connect`, `close` and `closed` stay transport methods:
how a port is made and ended is the transport's concern, as `Loopback::attach`
is in Rust.

### The call error

```ts,ignore
export type CallFailure =
  | { category: "contract"; contract: Contract }
  | { category: "corrupt" } | { category: "busy" }
  | { category: "undelivered" } | { category: "timeout" }
  | { category: "down" } | { category: "interrupted" }
  | { category: "unsupported" };

export class CallError extends Error { readonly failure: CallFailure; }
```

`CallFailure` and `Contract` move from the transport into the runtime unchanged.
The transport's `SessionError` becomes a subclass of `CallError`, so an existing
`instanceof SessionError` test still works and a face or a test can test
`instanceof CallError` without importing the transport. `StreamError` becomes
the same class as `CallError` (its two categories are a subset); see section 9,
decision 3.

## 4. The generated client

Generated into `<base>.codec.ts`, beside the table, for every interface that is
not `internal` and has at least one member the table carries. For cabin:

```ts,ignore
export class CabinClient {
  static bind<P extends $rt.Caller & $rt.SignalReader & $rt.EventSource>(
    port: P, codec: $rt.Codec,
  ): CabinClient;

  // command  setLevel(level: Level) @[..50ms]
  setLevel(level: t.Level, opts?: Partial<$rt.CallOptions>): Promise<void>;
  // query    average(window: Window): Average @[..200ms]
  average(window: t.Window, opts?: Partial<$rt.CallOptions>): Promise<t.Average>;

  // signal   temperature: Temperature
  subscribeTemperature(opts?: $rt.WaitOptions): Promise<void>;
  temperature(): $rt.Sample<t.Temperature>;            // held value, at once
  readTemperature(opts?: $rt.WaitOptions): Promise<$rt.Sample<t.Temperature>>;
  watchTemperature(listener: (s: $rt.Sample<t.Temperature>) => void): () => void;

  // event    warning: Warning
  subscribeWarning(opts?: $rt.WaitOptions): Promise<void>;
  onEvent(listener: (e: CabinEvent) => void): () => void;
}

export type CabinEvent = { readonly event: "warning"; readonly occurrence: $rt.Occurrence<t.Warning> };
```

Rules:

- **Parameters.** A call takes its declared parameters in order, typed with the
  types of `<base>.ts`, as the stream functions do today (D-18 of
  `ts-codec-table.md`). The face builds the request object (the single named
  parameter, or the induced tuple) itself.
- **The port bound.** `bind`'s type parameter names only the ports the
  interface's kinds need: `Caller` for a command or a query, `SignalReader` for
  a signal, `EventSource` for an event, `StreamCaller` for a stream member. This
  is RA-19 of the Rust face, and a test constructs the signal-only `HornClient`
  over an object that implements only `SignalReader`.
- **`bind`** compares `port.catalogHash` and `codec.catalogHash` with
  `CATALOG_HASH` and throws `CatalogMismatch` on a difference (Rust panics in
  `Bind::new`; TypeScript throws). It resolves every payload type id once with
  `codec.typeId`, and registers each signal's check with `port.checkWith`, which
  replaces passing `signalChecks(codec)` to `connect`. `signalChecks` stays
  exported for a program that uses the transport without a face.
- **The bound of a call.** When the member declares a maximum response time,
  `opts.bound` defaults to it (the Rust face uses `Member::call_deadline()` the
  same way). When the member declares none, `opts` and `opts.bound` are required
  in the generated signature, so a missing bound is a type error, not a run-time
  error.
- **Signals.** `Sample<T>` is `{ provenance, envelope, value: T, detected? }`.
  `value` is the decoded live value; under `init` and `invalid` it is the last
  good value when there is one, else the type's `init<Name>()` from `<base>.ts`.
  This is the Rust face's rule (the init value when the runtime holds zero
  bytes), and it keeps `value` non-null so that a program never writes the
  `payload === null` branch. `temperature()` throws a `CallError` `down` when
  the session has ended; before the first publish it returns the init sample.
- **Events.** One `onEvent` per interface, with a discriminated union routed by
  ordinal, parallel to the Rust `Event` enum and `next_event`. An occurrence the
  session refused (`detected: "corrupt"`, empty payload) is delivered with
  `occurrence.detected` set and no `value`, so nothing fails silently.
- **Streams.** The four stream forms of D-18 become methods with the same
  parameters and return values; the first parameter (`session`) goes away. For
  docstore, `DocStore_fetch(context, id, opts)` becomes `docs.fetch(id, opts)`.
- **Names.** The class is `<Interface>Client` and the union is
  `<Interface>Event`. A declaration whose name equals one of them, and a member
  whose generated method name equals another method of the same class
  (`subscribeX` beside a member named `subscribeX`, or a member named `onEvent`
  or `bind`), is refused with an error that names both sources, as the existing
  collision checks of `ts-codec-table.md` do.

## 5. The error model

| Where                                         | What the face does                                                                                  |
| --------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| `bind`, catalog differs                       | throws `CatalogMismatch`                                                                            |
| a constrained scalar argument                 | already refused by `make<Name>` (A2), with `ConstraintError`, when the program built it             |
| the encoded request fails `codec.verify`      | rejects with `CallError` `contract` / `invalid-value` and the rule; nothing is sent                 |
| the request is longer than the member's limit | rejects with `CallError` `corrupt`; nothing is sent (the transport does this already)               |
| the provider or the transport fails the call  | the port's `CallError`, unchanged                                                                   |
| the reply fails `codec.verify`                | rejects with `CallError` `contract` / `invalid-value` for a rule, `corrupt` for a malformed payload |
| a rule text not in `RULES`                    | rejects with `CodecError`, as the stream adapters do                                                |

The reply rules are the rules `decodeStream` already applies to a stream
element, so a single call and a stream element fail the same way.

`require` clauses are not checked by the TypeScript face; the provider checks
them and answers `contract`. The Rust face checks them before sending with the
clause translator. Translating the clauses to TypeScript is a separate story
(decision 2 of section 9).

## 6. Cancellation and bounds

- Every call has a bound: the declared one, or `opts.bound`. The transport
  checks the range (`RangeError` before sending, as today).
- `opts.signal` aborts a call. Abort before sending sends nothing. Abort after
  sending rejects the promise with the signal's reason and forgets the call; a
  command that the provider already received may still run, which the TSDoc says
  and which matches `ridl-transport-ws-ts.md` for an aborted upload. `Session`
  gains `signal` on `command` and `query`; `subscribe` and `read` have it
  already (C1, #994).
- Stream methods keep their present cancellation: cancelling the reader or
  aborting the upload.

## 7. Relation to #989 and D1

- **#989** asks for a public Rust client of `ridl-transport-ws` that implements
  the Rust consumer ports, so that the generated Rust face of cabin runs over a
  WebSocket. B2 does the same for TypeScript: the generated face runs over any
  object that implements the ports, and `Session` is one such object. The two
  are independent and do not share code; after both land, the cabin demo can run
  the Rust face and the TypeScript face against the same server. #989 is not a
  dependency of B2.
- **D1 (the emulator)** needs the provider ports (`Handler`, `SignalWriter`,
  `EventSink`) in the runtime, `Provision` of the transport implementing them,
  and a typed `provide` face. B2 fixes the pattern D1 follows: ports in the
  runtime, implemented by the transport, a generated class bound to a port. B2
  also adds a test-only in-process fake of the consumer ports
  (`packages/ridl-runtime/tests/fake_port.ts`), which tests the face without a
  socket. D1 decides whether that fake becomes a public in-process loopback that
  connects a consumer face to a provider face (decision 4 of section 9).

## 8. Compatibility, release and records

**Breaking changes against the released surface** (the release that carries A1
to A3; `v0.18.0` is the latest tag on 2026-11-11, so this is the minor after
it):

- Generated: `<Interface>_<member>` stream functions are removed; their callers
  move to methods. `StreamSession` is removed.
- `@driftsys/ridl-runtime`: `StreamTransport` and `StreamOptions` are replaced
  by `StreamCaller` and `CallOptions`; `StreamError` is merged into `CallError`
  (decision 3 of section 9).
- `@driftsys/ridl-transport-ws`: gains a dependency on the runtime; `Sample`,
  `Occurrence`, `CallFailure`, `WaitOptions` become re-exports (no change of
  shape); `SessionError` gains a superclass. `connect` keeps its options.
- The tables, `encode<Name>`, `decode<Name>`, `signalChecks`, `CATALOG_*`,
  `PAYLOAD_TYPES` and `RULES` do not change.

Keeping the free functions as deprecated wrappers was considered (section 10)
and rejected: there is no published consumer of them outside this repository's
examples, and two APIs for one call double the tests.

**Release.** B2 ships alone as the next minor (0.x: a minor carries breaking
changes), as section 2, decision 3 of the lane driver already decided. The
release notes carry a hand-written breaking note with the before and after of
`main.ts`. The freeze dumps of both packages and of the generated cabin module
change (ADR-0029); `freeze/surfaces.toml` is updated in the same pull request as
each change.

**Records.**

- ADR-0020 decision 7: an amendment, not a new ADR. It records that the port
  interfaces are `Caller`, `SignalReader`, `EventSource` and `StreamCaller` (the
  provider ports to follow with D1), that they are Promise-based, and that the
  transport package implements them and depends on the runtime; it updates the
  "Depends on" cell of the transport row in the decision 6 table.
- ADR-0023: no amendment. It governs the Rust face; this note states the
  parallel. A sentence in its Consequences pointing to the TypeScript face is
  optional.
- `docs/design/ts-codec-table.md`: new decisions for the client class (continue
  the D-numbering), and D-17 and D-18 rewritten for methods.
- `docs/design/ridl-transport-ws-ts.md`: the ports `Session` implements,
  `watch`, `listen`, `signal` on calls.
- `docs/book/generated-code.md` (TypeScript section) and the two package
  READMEs: the new usage.

## 9. Decisions on the open questions

1. **One class per interface.** Approved by the owner. `<Interface>Client.bind`
   is parallel to the Rust `Client<P>`; free functions with a context are
   alternative B of section 10.
2. **`require` clauses.** Approved by the owner: the provider checks them in B2,
   and a follow-up issue tracks a TypeScript clause translator.
3. **`StreamError` merges into `CallError`.** Decided by the author, as
   recommended: a program then catches one class for any call failure. It is one
   more breaking item in the release.
4. **The in-process loopback.** Decided by the author: B2 ships only the
   test-only fake. D1 decides whether it becomes a public runtime module,
   because D1 is the first task with a provider face to connect to it.
5. **The release.** Approved by the owner: B2 is the next minor release. The
   release that carries A1 to A3 is v0.18.0 and is out, so B2 ships in the minor
   after it.

## 10. Alternatives considered

- **A. A small structural interface per face (B0 option 2).** Each generated
  module would declare the structural shape it calls, as `StreamTransport` is.
  Smaller, and no change to the transport. Rejected by the owner on 2026-11-09:
  it gives D1 nothing to implement and no in-process path for tests, and every
  consumer tool (D1, D2, D3) would still bind to `Session`.
- **B. Free functions with a context, `Cabin_setLevel(ctx, level)`.** Extends
  what stream functions do and needs no class. Rejected: the catalog check, the
  type-id resolution and the signal check registration would run on every call
  or need a separate setup step the program can forget, and the per-signal
  listeners would have no object to hold their registrations. A class makes
  `bind` the one place for all three, as `Bind::new` is in Rust.
- **C. Methods on the existing table object (`Cabin.setLevel.call(...)`).**
  Keeps one exported name. Rejected: the table is plain data that `connect`
  reads (`InterfaceTable`), and mixing functions into it changes that type and
  makes the table depend on a port.
- **D. Poll-based ports, as in Rust.** A literal port of `Caller::command` /
  `ack` / `forget`. Rejected: TypeScript has a Promise and an `AbortSignal`, and
  `Session` already exposes calls as Promises; a poll API would add a loop in
  every face for no gain. The split by kind and the byte-only boundary are what
  is kept parallel.
- **E. Keep the stream free functions as deprecated wrappers for one release.**
  Softer migration. Rejected: no published consumer exists, and two APIs double
  the fixture tests (the release decision can reopen this if a consumer
  appears).
- **F. Nullable `value` for `init` and `invalid` samples.** Simpler to generate.
  Rejected: it is what the hand-written code does today and it differs from the
  Rust face, which returns the init value.

## 11. Task breakdown (one pull request each, reviewed per task)

| Task | Scope                                                                                                                                                                                                                                   | Depends on |
| ---- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------- |
| B2.1 | Runtime: `ports.ts` (the four ports, `CallOptions`, `RawSample`, `RawOccurrence`, `Sample<T>`, `Occurrence<T>`), `CallError`/`CallFailure`/`Contract`, the test fake `tests/fake_port.ts` and its tests. `StreamTransport` kept for now | none       |
| B2.2 | Transport: depend on the runtime, re-export the moved types, `SessionError extends CallError`, `catalogHash`, `watch`, `listen`, `signal` on `command` and `query`; a test that a `Session` is assignable to each port                  | B2.1       |
| B2.3 | Backend: generate `<Interface>Client` for commands and queries (`bind`, catalog check, default bound, request verify, reply verify and decode); collision checks; fixtures `codec`, `codec-names`, `codec-single` regenerated           | B2.1       |
| B2.4 | Backend: signals and events (`subscribeX`, `x()`, `readX`, `watchX`, `onEvent`, `<Interface>Event`, `checkWith` in `bind`); `codec-bare-signal` and the RA-19 test over a signal-only fake                                              | B2.3       |
| B2.5 | Backend and runtime: stream members as methods; remove `<Interface>_<member>`, `StreamSession`, `StreamTransport`, `StreamOptions`; `codec-streams`, `codec-bare-stream` and `examples/docstore/web` moved                              | B2.4, B2.2 |
| B2.6 | Demo and records: `examples/cabin/web` uses the face (section 12); ADR-0020 decision 7 amendment; `ts-codec-table.md`, `ridl-transport-ws-ts.md`, the book chapter and READMEs; freeze dumps                                            | B2.5       |

B2.2 and B2.3 can run in parallel after B2.1. Each task regenerates the fixtures
and freeze dumps it changes in its own pull request. The `/review` branch passes
run once after B2.6, over the whole series.

## 12. Test and demo plan

- **Unit tests of the face (Deno, no socket)**, in the backend's fixtures over
  `fake_port.ts`: a command sends the verified request bytes and resolves on
  `accepted`; a query decodes a verified reply; a reply that fails a rule
  rejects `contract`/`invalid-value` with the rule; a malformed reply rejects
  `corrupt`; a request that fails verify sends nothing; the default bound equals
  the declared maximum and `opts.bound` overrides it; abort before and after
  send; `bind` throws `CatalogMismatch` for the port's hash and for the codec's
  hash; a signal sample under `init`, `live`, `invalid` and `invalid` with
  `detected`; an event union routed by ordinal and a refused occurrence. Each
  test is checked to fail when the face is mutated (the semantic-mutation rule
  of the task review).
- **Type tests** (`deno check` of fixtures that must not compile, via
  `// @ts-expect-error`): a member with no declared maximum requires `bound`;
  `HornClient.bind` accepts a port with only `SignalReader`; a wrong argument
  type is refused.
- **Transport tests** (`packages/ridl-transport-ws/tests`): `watch` and `listen`
  remove their listener; `signal` on `command` and `query`; `SessionError` is
  `instanceof CallError`.
- **End to end.** `examples/cabin/web/main.ts` is rewritten to bind
  `CabinClient` over the session and to send `setLevel`, call `average` and read
  `temperature` through it, with no `encode`, `decode`, `iface`, `ord` or `Kind`
  in the program, which is the done-when of #1262. This needs the cabin server
  (`examples/cabin/server`) to provide `setLevel` and `average`; today it only
  publishes `temperature`, so B2.6 adds both handlers to the server. The
  existing temperature checks stay. `just cabin-ws-demo` and
  `just cabin-browser-demo` run it; `just ts-e2e` runs the docstore program on
  the stream methods.
- **Gates.** `just ts-check`, `_codec-ts-check` (through `cabin-ws-demo`),
  `just freeze-check`, `just ts-npm-check` and `just build` on each pull request
  through CI; implementers run the focused crate and package tests only.
