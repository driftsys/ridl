# A signal check for `ridl-transport-ws` (TypeScript)

Status: design note, revised 2026-11-09 after the owner's answers. Task C2 of
the TypeScript lane, issue driftsys/ridl#1236. No code is written until the
owner approves this revision.

## The question

The Rust runtimes run a check that the generated binding registers for each
signal (`SignalReader::check_with`, ADR-0021 decision 23). A `Live` payload that
fails the check does not replace the held value, and the sample reads as
`Invalid(Detected(_))` (frame specification §5.1 step 3 and §9.1). The
TypeScript package verifies no payload. Its design record says the caller keeps
the last good value from `onPublish`. Does the package take a check per signal?

## Recommendation

Do it. The change is small: one option, one field on `Sample`, one method, and
one generated function. The TypeScript client then keeps the promise of §9.1
("the malformed value is never delivered; its invalidity always is, on every
read") without code in each caller. Today each caller repeats the verify call
and the last-good bookkeeping, and a `read` cannot return the invalid state.

## Alternatives considered

| Alternative                                                     | Why it was not chosen                                                                                                                            |
| --------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| Do nothing: the caller keeps the value                          | Works for one caller. A `read` has no invalid state, each caller repeats the code, and the two clients of one provider promise different things. |
| The session takes the wasm `Codec` and verifies every signal    | The package would name payload types and type ids. ADR-0020 decision 6 keeps the runtime free of payload types.                                  |
| A check given only to `subscribe`                               | A `read` of a signal that is not subscribed has no check. Registration on the session covers both.                                               |
| Verification in the generated face only (`decode<Name>` throws) | The face sees the sample after the session replaced the held bytes, so the last good value is lost. The session has to hold it.                  |
| Check in the session, last good bytes in the sample (this note) | Matches decision 23: the runtime keeps the bytes, the binding supplies the check.                                                                |

## API shape

Types live in `@driftsys/ridl-runtime`, as `StreamSession` does, so the
generated module imports no transport package (ADR-0020 decision 6):

```ts
export type Detection = "corrupt" | { rule: PayloadRule };
/** Returns `undefined` when the payload passes. */
export type SignalCheck = (bytes: Uint8Array) => Detection | undefined;
export interface SignalCheckEntry {
  readonly iface: number;
  readonly ord: number;
  readonly check: SignalCheck;
}
```

`Detection` mirrors the Rust `Detection` (`InvalidValue(Violation)` or
`Corrupt`) and carries the rule text only. The transport's `Rule` union
(`frame.ts`) and `PayloadRule` have the same members, so a value of one fits the
other.

In `src/session.ts`:

- `ConnectOptions.checks?: readonly SignalCheckEntry[]` registers checks at
  `connect`.
- `Session.checkWith(iface, ord, check)` registers or replaces one check. It
  throws a `RangeError` for an interface number or an ordinal outside 1 to
  4294967295, as `unsubscribe` does (`sample()` validates nothing and keeps
  doing so).
- `Sample` gains a required `lastGood: Uint8Array | null` and its `detected`
  widens from `"corrupt"` to `Detection`. A required field breaks code that
  constructs a `Sample` or compares one with `deepEquals` (for example
  `tests/session_test.ts:1159-1177`, which the task updates). Code that only
  reads a `Sample` stays compatible, and `detected === "corrupt"` keeps its
  meaning.
- `Note` gains
  `{ note: "check-failed"; iface: number; ord: number; error: unknown }`,
  emitted when a check throws. This widens a public union, so an exhaustive
  `switch` over `note` in caller code stops compiling.

`lastGood` is the last payload that passed since the last `subscribe`, and
`null` means the caller uses the init value. Per §5.1:

| `publish` provenance                | `payload` | `lastGood`                                   | `detected`      |
| ----------------------------------- | --------- | -------------------------------------------- | --------------- |
| `Live`, no check or check passes    | the bytes | the bytes                                    | absent          |
| `Live`, check refuses (or oversize) | `null`    | the held `lastGood` (step 3)                 | the `Detection` |
| `Init`                              | `null`    | `null` (the value is the init value, step 4) | absent          |
| `Invalid(Declared)`                 | `null`    | the held `lastGood` (step 5)                 | absent          |

`unsubscribe` drops the held sample with its `lastGood` (§5.1 step 1).

## Behaviour in `Session.#onPublish`

- The check runs after the `seq` test (`session.ts:1198-1205`), so a duplicate
  or stale frame is not checked. It runs after the limit test and before
  `#held.set`.
- A refused payload yields an `invalid` sample with the refused frame's
  envelope. It becomes the held sample, so `sample()` and later reads see it
  until a later `live` payload passes.
- A `read` answered by a refused frame resolves with the invalid sample (§9.1;
  `crates/ridl-rt/src/port.rs:108-123`). It does not reject.
- The over-limit path (D-19) is aligned in the same task: its `read` also
  resolves with the invalid sample (`detected: "corrupt"`, `lastGood` held)
  instead of rejecting `corrupt` (today `session.ts` near 1179 and 1257).
- A check that throws counts as `"corrupt"` and emits the `check-failed` note.
- `checkWith` after `connect` checks a held `live` sample at once. If the check
  refuses it, the held bytes are cleared, the provenance becomes `invalid` with
  `detected`, and the held envelope stays. Registration is not a publication:
  `onPublish` is not called and no `seq` changes (`port.rs:99-102`).

## The generated module

`<base>.codec.ts` exports

```ts
export function signalChecks(codec: Codec): readonly SignalCheckEntry[]
```

It imports only types of `@driftsys/ridl-runtime`. For each signal entry it
calls `codec.typeId(entry.payload.type)` once at registration, as the stream
functions do (`crates/ridl-backend-ts/src/codec.rs`, near 2239), and builds a
check from `codec.verify(typeId, bytes)`:

- `ok` passes;
- `malformed` and `unknown-type` map to `"corrupt"` (Rust `payload::check` maps
  every non-contract verify error to `Corrupt`);
- `rule` with a rule text in the `RULES` list maps to `{ rule }`;
- `rule` with no text (`rule?: string`, `ridl-runtime/src/codec.ts:11-12`), or
  with a text outside `RULES`, throws `CodecError`, as the stream helpers do
  (`codec.rs` near 2186). The session counts the throw as `corrupt` and emits
  `check-failed`.

The caller writes `connect({ ..., checks: signalChecks(codec) })`.

## The end-to-end test

The provider sets the signal to each accepted command's argument
(`e2e/catalog.ts:20`, `tests/support/provider.rs:25`), so the step needs no
server change. In `e2e/deno_test.ts`, register a plain-function check that
refuses a chosen payload, then: send a command with a good argument and see
`live` with those bytes; send a command with the refused argument and assert
`sample()` is `invalid`, `payload` null, `detected` set and `lastGood` equal to
the earlier bytes; read the signal and assert the same invalid sample; send a
good argument and assert `live` with the new bytes. The browser test repeats the
scenario.

## Task split

1. Package, runtime and transport: the `@driftsys/ridl-runtime` types, then
   `Sample.lastGood`, `detected`, the `check-failed` note, `checks`,
   `checkWith`, the `#onPublish` change and the over-limit alignment, with unit
   tests in `tests/` (refused payload keeps bytes, recovery, `read` of a refused
   publish, over-limit `read`, `Init` and `Declared` `lastGood`, a throwing
   check, `checkWith` on a held sample, a stale frame not checked, `unsubscribe`
   drops `lastGood`, bad numbers). Update `tests/session_test.ts:1159-1177` and
   `1359-1377`.
2. End-to-end: the step above in `e2e/deno_test.ts` and the browser test.
3. Backend: emit `signalChecks`, update the fixtures, and run the flatc-based
   `_codec-ts-check`. It depends on the type names of task 1.
4. Documentation, in the same pull request as the code it describes. No
   TypeScript surface is frozen (`freeze/surfaces.toml`), so there are no freeze
   rows. Stale statements to change:
   - `docs/design/ridl-transport-ws-ts.md` lines 61, 237-239 (the `Sample`
     paragraph), 430-431 (D-19 read rejection), 432-436, 839-840 (the e2e last
     step) and "What is not built" lines 1004-1021;
   - `src/session.ts` doc comments at 6-12, 66-71, 130-135, 565-571 and
     1161-1163;
   - the package README and a changelog line.

Tasks 1 and 2 are independent of task 3.
