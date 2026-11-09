# A signal check for `ridl-transport-ws` (TypeScript)

Status: design note for decision, 2026-11-09. Task C2 of the TypeScript lane,
issue driftsys/ridl#1236. No code is written until the owner approves this note.

## The question

The Rust runtimes run a check that the generated binding registers for each
signal (`SignalReader::check_with`, ADR-0021 decision 23). A `Live` payload that
fails the check does not replace the held value, and the sample reads as
`Invalid(Detected(_))` (frame specification §5.1 step 3 and §9.1). The
TypeScript package verifies no payload. Its design record says the caller keeps
the last good value from `onPublish`. Does the package take a check per signal?

## Recommendation

Do it. The cost is small: one option, one field on `Sample`, about 40 lines in
`Session`, and one generated function. The benefit is that the TypeScript client
keeps the same promise as the Rust runtimes (frame specification §9.1: "the
malformed value is never delivered; its invalidity always is, on every read")
without code in each caller. Today every caller must repeat the verify call, the
last-good bookkeeping and the rule that a `read` also returns the invalid state.
A caller that forgets one of these shows a malformed value.

## Alternatives considered

| Alternative                                                     | Why it was not chosen                                                                                                                                                           |
| --------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Do nothing: the caller keeps the value                          | Works for one caller. It does not give `read` the invalid state, and each caller repeats the same code. The two clients of one provider would also differ in what they promise. |
| The session takes the wasm `Codec` and verifies every signal    | The package would name payload types and need a catalog of type ids. ADR-0020 decision 6 and §9 of the frame specification keep the runtime free of payload types.              |
| A check per signal given only to `subscribe`                    | A `read` of a signal that is not subscribed has no registered check, so a rejected reply is delivered as a value. Registration at `connect` covers both.                        |
| Verification in the generated face only (`decode<Name>` throws) | The face sees the sample after the session has replaced the held bytes, so the last good value is lost. The session has to hold it.                                             |
| Check in the session, last good bytes in the sample (this note) | Matches decision 23: the runtime keeps the bytes, the binding supplies the check.                                                                                               |

## API shape

In `src/session.ts`:

```ts
/** What a check returns for a payload it refuses. */
export type Detection = "corrupt" | { rule: string };

/** Returns `undefined` when the payload passes. */
export type SignalCheck = (bytes: Uint8Array) => Detection | undefined;

interface ConnectOptions {
  /** One check per signal, keyed like the interface tables. */
  checks?: readonly { iface: number; ord: number; check: SignalCheck }[];
}
```

`Session.checkWith(iface, ord, check)` registers a check after `connect`, as the
Rust method does. It throws a `RangeError` for a bad number, as `sample` does. A
later registration replaces the earlier one.

`Sample` gains `lastGood: Uint8Array | null` and widens `detected` to
`Detection`. `lastGood` is the last payload that passed (or was not checked)
since the last `subscribe`; `null` means the caller uses the init value. Under
`live` it equals `payload`. `detected: "corrupt"` keeps its meaning, so an
existing caller still compiles.

`Session.#onPublish` change: for a `live` payload with a registered check, run
it after the limit test and before `#held.set`. On a refusal, the sample is
`{ provenance: "invalid", payload: null, detected, lastGood: <held lastGood> }`
with the rejected frame's envelope. The `#held` entry takes this sample, so
`sample()` and every later `read` see it until a later `live` passes. A `read`
answered by the refused frame resolves with the invalid sample (it does not
reject), as in §9.1. A check that throws counts as `"corrupt"` and emits a
`Note`. `unsubscribe` drops `lastGood` with the held sample (§5.1 step 1).

The existing over-limit path (D-19) also fills `lastGood`. Its `read` still
rejects `corrupt`; changing that is not part of this task.

## The generated module

`<base>.codec.ts` already holds a `verify` path
(`session.codec.verify(typeId,
bytes)` returns `VerifyResult`) and maps it to a
category in the stream helpers. The backend emits one function per interface
table:

```ts
export function signalChecks(codec: Codec): ConnectOptions["checks"]
```

Each entry maps `malformed` to `"corrupt"` and `rule` to `{ rule }`, and treats
`unknown-type` as a programming error (throws `CodecError` at registration time,
not at publish time). The caller writes
`connect({ ..., checks: signalChecks(codec) })`. The package imports no payload
type; it receives opaque functions, which preserves decision 6.

## The end-to-end test

`e2e/deno_test.ts` today ends before the Rust scenario's last step. Add that
step: register a check that refuses a chosen payload (the check of the signal in
`e2e/catalog.ts` can be a plain function, so the test needs no wasm), have the
example provider publish a refused payload, and assert that `sample()` reads
`invalid` with `detected`, `payload` null and `lastGood` equal to the bytes of
the earlier good publish. Then publish a good payload and assert `live` with the
new bytes. Whether `loopback_server` can publish a chosen payload on request
must be confirmed first; if it cannot, extend its control channel (one command),
which the task split lists as its own step. The same scenario runs in the
browser test.

## Task split

1. Package: `Sample.lastGood`, `Detection`, `SignalCheck`, `checks` option,
   `checkWith`, the `#onPublish` change, unit tests in `tests/` (refused payload
   keeps bytes, recovery, `read` of a refused publish, check that throws,
   `unsubscribe` drops `lastGood`, bad numbers). Update
   `docs/design/ridl-transport-ws-ts.md` (the `Sample` paragraph, the non-goal
   list and the e2e paragraph that cite #1236).
2. Server and e2e: let `loopback_server` publish a chosen payload if it cannot,
   then add the step to `e2e/deno_test.ts` and the browser test.
3. Backend: emit `signalChecks` in the TypeScript codec module, with fixture
   updates and the flatc-based check of `_codec-ts-check`.
4. Gate: freeze rows if `ridl-transport-ws`'s surface is frozen (check
   `freeze/surfaces.toml`), the README of the package, and a changelog line.

Tasks 1 and 2 are independent of 3. Task 3 depends on task 1's type names.

## Open points for the owner

- Whether a `read` answered by a refused frame resolves with the invalid sample
  (this note, §9.1) or rejects, as the over-limit path does today.
- Whether `Detection` carries only the rule text, or the full `Contract` record
  that `SessionError` uses.
