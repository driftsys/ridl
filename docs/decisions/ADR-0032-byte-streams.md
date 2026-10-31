# ADR-0032 — Byte streams

## Status

Accepted — 2026-10-31. Scope: what a stream whose element is a bare `bytes` or a
bare `string` means (ridl reference §12.2), how the typl length default applies
in that position, the generated surface intended for such a stream in each
target language, and the rules every stream shares: its end, its failures, its
response bound and the size of its chunks. It binds the ridl and typl
references, the checkers, the backends that generate a stream, the stream port
of `ridl-rt` when it is built, and every transport binding that carries a
stream.

Written from the owner's decisions of 2026-10-28 (decisions 1 to 8) and of
2026-10-31 (decisions 9 to 14, which answer the six open questions the record
was proposed with). It adds no syntax, no interaction kind and no diagnostic
code. The documents in "Documents amended" are changed with the acceptance.
Nothing in this record is built: streams have no port, no transport framing and
no generated code (driftsys/ridl#336, driftsys/ridl#1116).

## Context

- The ridl reference §12 defines the stream container `<T>`, valid only in
  interaction position: a `command` or `query` parameter and a `query` return.
  §12.2 allows a named typl type as the element, or a raw `string` or `bytes`
  "when the element is genuinely unstructured", the one exception to typl §15.3.
  It also says that "the element type's bound describes one logical element",
  and that framing, buffering and backpressure are transport concerns.
- typl §4.4 and §4.5 give a `string` or `bytes` with no bound written the
  default `[0..256]` and the warning TYPL-103. typl §12.2 (map keys) states that
  this default applies "the same as every other position". Read together with
  ridl §12.2, a `<bytes>` stream would be a stream of elements of at most 256
  bytes each, with the boundary of each element part of the contract.
- As built, the checker accepts `<bytes>` and `<string>` with no diagnostic,
  TYPL-103 included, and the IR records the element as `PrimitiveType::BYTES` or
  `PrimitiveType::STRING` in `StreamType.primitive`, with no bound
  (`crates/ridl-ir/proto/ridl/ir/v2/ir.proto`). So the checker and the IR
  already treat a bare element as unbounded; the references do not say so.
- ADR-0030 states in decision 11 that data larger than one transfer is modelled
  as a stream of bounded elements. Its amendment of 2026-10-28
  (driftsys/ridl#1109) gives each interaction kind its own default payload
  ceiling (decision 16: 3,840 bytes for `signal` and `event`, 16,128 bytes for
  `command` and `query`, 65,280 bytes for `fixed`), and makes the splitting of a
  payload a concern of each transport binding.
- The use case that reopens streams is a file or a document that one side
  downloads from or uploads to the other, whose total size the contract author
  does not know and whose chunking the author has no reason to choose. An
  application reads or writes such data as I/O: a reader that it pulls bytes
  from, or a writer that it pushes bytes into.
- `ridl-rt` 0.1 has no stream port, and its descriptors carry no stream element
  type and no stream flag (driftsys/ridl#336, `docs/design/ridl-rt.md` "What 0.1
  leaves out"). `ridl-rt` is sans-IO and `no_std`
  ([ADR-0018](ADR-0018-runtime-core-and-generated-surface.md) decisions 1 and
  2).
- The project adds language syntax only when no backend strategy covers the
  need.

## Decision

1. **The use case is a transfer of unknown size, accessed as I/O.** A byte
   stream exists to download or upload a file or a document whose size and
   chunking the author does not know, and which the application reads or writes
   as a sequence of bytes. A transfer whose units carry meaning to the
   application (a fault record, a log line, a sample) stays a stream of named
   elements (decision 4).

2. **A bare `bytes` element makes a byte stream, with no element bound.** No
   syntax and no interaction kind are added. A stream whose element is a bare
   `bytes` (`<bytes>`), which ridl §12.2 and typl §15.3 already allow, is a byte
   stream: an ordered sequence of bytes whose chunk boundaries are not part of
   the contract. The transport chooses the boundaries, and each chunk is at most
   the interaction's limit (decision 14; "the payload ceiling of the
   interaction's kind" before 2026-10-31). The element has no bound: typl's
   `[0..256]` default and TYPL-103 do not apply to a bare `bytes` or `string` in
   stream element position. In every other position typl's rules are unchanged:
   a bare `string` or `bytes` field is still refused by §15.3, and a map key
   still takes the default with TYPL-103. RIDL-202 is unchanged.

3. **A bare `string` element makes a UTF-8 text stream.** `<string>` is a byte
   stream whose content is UTF-8 text. It follows decision 2, with one more
   rule: a chunk never splits a UTF-8 code point, so each chunk on its own is a
   whole number of code points.

4. **A named element type keeps its meaning.** A stream whose element is a named
   type (`<FaultEvent>`, `<DocChunk>`) is a stream of bounded messages, as ridl
   §12 defines it today: each element is one value of the type, its bound is the
   type's bound, and the boundary between two elements is part of the contract.
   This holds when the named type is itself a `bytes` or `string` type: with
   `type FwBlock: bytes [1..4096]`, `<FwBlock>` is a stream of blocks, not a
   byte stream.

5. **Metadata is an ordinary typed value.** A byte stream carries bytes only. A
   content type, a size or a digest is declared as an ordinary typed value: the
   reply of a separate query (a `describe`), or a parameter beside the stream. A
   declared size is at most a hint; the end of the stream is its end. Resuming
   an interrupted transfer is an ordinary `offset` parameter. For example:

   ```ridl,ignore
   query fetch(id: DocId): <bytes> @[..1s]
   command put(info: DocInfo, data: <bytes>) @[..1s]
   ```

   and, with metadata and resume:

   ```ridl,ignore
   query describe(id: DocId): DocInfo | DocError @[..1s]
   query fetchFrom(id: DocId, offset: ByteOffset): <bytes> @[..1s]
   ```

6. **Each target has an idiomatic generated surface.** This is the intended
   mapping. The backends that generate it are governed by
   [ADR-0013](ADR-0013-codegen-backend-scope.md),
   [ADR-0018](ADR-0018-runtime-core-and-generated-surface.md) and
   [ADR-0020](ADR-0020-third-encoding-runtime-layering-and-plugin-system.md),
   and each backend records its own surface when it is built.

   | Target     | A `<bytes>` the application receives     | A `<bytes>` the application sends       |
   | ---------- | ---------------------------------------- | --------------------------------------- |
   | Rust       | an `AsyncRead`, or a `Stream` of `Bytes` | an `AsyncWrite`, or a `Sink` of `Bytes` |
   | TypeScript | `ReadableStream<Uint8Array>`             | `WritableStream<Uint8Array>`            |
   | Kotlin     | `Flow<ByteArray>`, or an `InputStream`   | `Flow<ByteArray>`, or an `OutputStream` |

   A `<string>` stream (decision 3) has the text form of each row (added
   2026-10-31, decision 12); this too is the intended mapping:

   | Target     | A `<string>` the application receives          | A `<string>` the application sends            |
   | ---------- | ---------------------------------------------- | --------------------------------------------- |
   | Rust       | an `AsyncRead` of UTF-8, or a `Stream<String>` | an `AsyncWrite` of UTF-8, or a `Sink<String>` |
   | TypeScript | `ReadableStream<string>`                       | `WritableStream<string>`                      |
   | Kotlin     | `Flow<String>`, or a `Reader`                  | `Flow<String>`, or a `Writer`                 |

   `ridl-rt` stays sans-IO (ADR-0018 decision 2): the async types belong to the
   generated face and to the async wrapper over the port, not to the port
   itself. A Rust target without `std` has no `AsyncRead` and no `AsyncWrite`;
   the generated face offers the port's own pull API there (decision 9). The
   Kotlin surface is generated by the Kotlin plugin, outside this workspace
   (ADR-0020 decisions 8 to 11).

7. **Framing, buffering, backpressure and chunk size stay transport concerns.**
   This is ridl §12.2 unchanged. This record does not define how the WebSocket
   binding frames a stream; that framing is part of the implementation work for
   streams.

8. **ADR-0030 decision 11 names both forms of a stream.** Data larger than one
   transfer is a stream: a byte stream (decisions 2 and 3) when its chunking is
   not part of the contract, or a stream of bounded elements (decision 4) when
   each element carries meaning.

Decisions 9 to 14 were added on 2026-10-31, from the owner's answers to the six
open questions the record was proposed with, one decision per question, in the
order the questions were listed.

9. **The stream port is a pull API over caller-supplied buffers, and `ridl-rt`
   stays sans-IO.** Like every port method of `ridl-rt` (ADR-0018 decision 2,
   [`docs/design/ridl-rt.md`](../design/ridl-rt.md) "The ports"), a stream port
   method returns without waiting and copies into or out of a buffer the caller
   supplies. A read reports one of four outcomes: the number of bytes copied
   into the caller's buffer, the end of the stream, pending (nothing yet), or an
   error. A write reports how many bytes the port accepted; a port that accepts
   fewer bytes than offered is the backpressure of ridl §12.2, and the producer
   ends its stream explicitly (decision 11). A stream is keyed by its call: by
   the `Correlation` on the calling side and by the `ClaimId` on the providing
   side. On a target without `std` the generated face is this API. With `std`
   the generated face wraps it in `AsyncRead` and `AsyncWrite`, or `Stream` and
   `Sink` (decision 6). The exact signatures, and how the `std` wrapper is
   woken, are settled when streams are built (driftsys/ridl#1116).

10. **No in-band typed error on a byte stream, and no new syntax.** The spelling
    `<bytes> | E` is not added. A failure the provider knows about before the
    transfer is the reply of a preceding query (decision 5's `describe`
    returning `DocInfo | DocError`). A failure the producer meets during the
    transfer reaches the consumer of a byte stream only as an interrupted
    transfer (decision 11). An author who needs the reason inside the stream
    declares a named element type that is a result union
    (`<DocChunk | DocError>`, decision 4, ridl §12.4). In the upload direction,
    a `command` carries no functional result (ridl §6.1), so a provider's
    failure of an upload is state, or the author declares a
    `query put(...): PutResult | PutError` whose reply follows the upload.

11. **The end of a stream is explicit.** A stream ends when its producer ends
    it; the encoding of the end belongs to the frame version that adds streams.
    A stream that stops without its end is a Stratum 3 interruption (ridl
    §10.3), in either direction, never a normal end: a consumer never treats a
    truncated download as complete, and a provider never treats a truncated
    upload as complete. The rule holds for every stream, byte or named element.
    A digest stays optional metadata (decision 5); it is not what tells a
    complete transfer from an interrupted one.

12. **Invalid UTF-8 in a `<string>` stream ends the stream.** Content that is
    not valid UTF-8 is a Stratum 2 `INVALID_VALUE` that ends the stream, as ridl
    §12.4 does for an element that violates its constraints. Because a chunk
    never splits a code point (decision 3), each chunk is checked on its own, at
    the boundary (ridl §10.2), on both ends: the sending side refuses invalid
    text before transmission, and the receiving side refuses it before
    application code. Decision 6's mapping gains its `<string>` rows.

13. **The response bound covers the start of a streamed reply, not the whole
    transfer.** On a query whose reply is a stream, byte or named element, `max`
    (ridl §9.3) bounds the first chunk or element, or the end of the stream,
    whichever comes first, measured from the request's stamp. This holds whether
    or not the query also has a stream parameter: on a bidirectional query (ridl
    §12's `query pipe(...)`) the reply can start before the consumer's stream
    ends, so the streamed-reply rule applies. On a query with a stream parameter
    whose reply is not a stream, `max` is measured from the end of the
    consumer's stream, not from the request's stamp, so it bounds the time to
    the reply after the upload ends; only there can the reply not start before
    the upload ends. On a command with a stream parameter, `max` keeps its
    meaning of acceptance (ridl §6.1, §9.3); the upload itself has no contract
    bound. A stream that stops in the middle is detected by the transport
    (decision 11), not by the response bound. No frozen surface carries a stream
    today (frame version 1 has none), so no existing declaration changes
    meaning.

14. **The interaction's limit bounds each chunk.** The interaction's resolved
    limit (ADR-0030 decisions 12 and 16 to 18) bounds every payload the
    interaction carries: its encoded non-stream arguments or reply, and each
    chunk or element of its stream. Decision 2's "the payload ceiling of the
    interaction's kind" becomes "the interaction's limit". For a command or a
    query, 16,128 bytes is both the default and the largest raise (ADR-0030
    decision 18), so a `[limits.interaction]` entry can only lower the chunk
    size; a limit of 0 makes a stream unusable. The checker does not change: the
    sizer gives a stream no size (`crates/ridl-ir/src/projection/size.rs`), so
    RIDL-157 does not apply to a stream.

## Alternatives considered

- **A new `blob` or `file` kind.** Rejected: it adds a keyword and an IR node to
  express what `<bytes>` already spells, and the language reference already
  allows a bare `bytes` element for unstructured data. A new kind would also
  need its own rules for direction, errors and timing, which §12 already gives a
  stream.
- **An inline bound on the element, `<bytes [0..N]>`.** Rejected: it is new
  syntax, and the bound it states is the size of a chunk, which the author does
  not know and should not choose; the transport and the payload ceiling of the
  kind already bound a chunk. An author who wants chunks of a declared size
  declares a named type and gets a stream of bounded messages (decision 4).
- **A chunked query with the existing kinds, `query read(id, offset): Chunk`.**
  Rejected as the answer to the use case, kept as a pattern an author may write.
  It needs no change, but it turns one transfer into many calls, each with its
  own round trip, response bound and admission; it fixes the chunk size in the
  contract; and the generated surface is a call per chunk, not a reader or a
  writer. The application has to rebuild the I/O view by hand.
- **Keep the `[0..256]` default on a bare element.** Rejected: a bare `<bytes>`
  would then be a stream of messages of at most 256 bytes each, with each
  boundary part of the contract, which no author of a file transfer means. It
  would also make the as-built checker, which applies no default there, wrong
  instead of the references.
- **A required named element type for every stream.** Rejected: it forces the
  author to choose a chunk size for data with no natural unit, which is the
  chunked-query problem inside one call.

## Consequences

- The language surface does not change: every source that compiles today
  compiles after this record, with the same diagnostics. The checker and the IR
  already treat a bare element as unbounded, so the change is to the references,
  which are made to state what the checker does.
- `<bytes>` and `<string>` change meaning in the references, from a stream of
  elements of at most 256 bytes or characters each to a stream of bytes with no
  contracted boundary. No transport carries a stream today, so no deployed
  behaviour changes.
- The stream port of `ridl-rt`, the WebSocket stream framing, and the codegen
  for `<T>` and `<bytes>` in the Rust and TypeScript backends are new work. The
  port adds fields to `Member` or `PayloadInfo`, so it is a breaking change of
  `ridl-rt`, released in a 0.x minor version (driftsys/ridl#336).
- The descriptor and the codegen `Model` must be able to tell a byte stream from
  a stream of named elements. The IR already does, through
  `StreamType.primitive` and `StreamType.named`.
- A transport that carries a byte stream must keep the interaction's limit for
  every chunk (decision 14), must carry the explicit end of a stream (decision
  11), and for `<string>` must place chunk boundaries between code points and
  refuse a chunk that is not valid UTF-8 (decision 12).
- The frame version that adds streams has to encode the end of a stream
  (decision 11), settle how duplicate suppression treats a streamed reply (note
  c), and change where the frame specification measures the response bound of a
  query with a stream parameter and a plain reply: §7 keeps a settled response
  until the bound has passed, measured from the request's stamp, and §8 raises
  the query `Timeout` once now − request stamp > max, where decision 13 measures
  from the end of the upload.

## Open questions

The six open questions this record was proposed with were settled on 2026-10-31
by decisions 9 to 14, one per question in the order they were asked: the API of
a target without `std` (decision 9), functional failure on a byte stream
(decision 10), a normal end against an interrupted transfer (decision 11),
invalid UTF-8 in a `<string>` stream (decision 12), the response bound of a
streamed reply (decision 13), and a `[limits]` entry and the chunk size
(decision 14). None remains open.

## Notes

Observations recorded with the acceptance of 2026-10-31. They are not decisions;
each names the record or the issue that owns it.

- (a) **A command's stream parameter after the acknowledgment.** The delivery
  acknowledgment of ridl §6.1 confirms that the payload present at acceptance is
  valid; it cannot cover a stream, whose chunks arrive after it. A constraint
  failure in a command's stream parameter after the acknowledgment ends the
  stream with the Stratum 2 category, visible to the calling runtime as ridl
  §6.1 says for a negative acknowledgment, and not to the contract.
- (b) **A named element type above the kind's ceiling.** A stream whose named
  element type is larger than the interaction's limit
  (`type Big: bytes
  [1..40000]`, then `<Big>` on a query) passes RIDL-157
  today, because the sizer gives a stream no size (decision 14). Each element of
  such a stream cannot be carried as one chunk. The check belongs to the stream
  work (driftsys/ridl#336).
- (c) **Duplicate suppression and a streamed reply.** The frame's duplicate
  suppression (frame specification §7) keeps a settled response and re-sends it
  to a retransmitted request. A streamed reply is not one response, and a
  provider cannot keep it whole. How a retransmitted request to a query whose
  reply is a stream is answered is an item for the frame version that adds
  streams.

## Documents amended

These documents were changed with the acceptance of 2026-10-31.

| Document                                                                                       | Change                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| ---------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| [`docs/specification/ridl-language-reference.md`](../specification/ridl-language-reference.md) | §12 introduction: the two forms of a stream, a `<bytes>` example and the note that no runtime carries a stream today (decision 1). §12.3 "Maps to": the I/O mapping (decision 6). §12.2: a bare `bytes` element is a byte stream and a bare `string` element a UTF-8 text stream, with no element bound and no contracted chunk boundary, each chunk bounded by the interaction's limit; a named element keeps its bound (decisions 2 to 4, 14). §12.4: the explicit end (decision 11), invalid UTF-8 (decision 12), the failure paths of a byte stream (decision 10). §9.3: the response bound on a stream (decision 13). The glossary entry "stream" |
| [`docs/specification/typl-language-reference.md`](../specification/typl-language-reference.md) | §4.4 and §4.5: the `[0..256]` default and TYPL-103 do not apply to a bare `string` or `bytes` in ridl stream element position (decision 2). §12.2: the map-key paragraph's "the same as every other position". §15.3: the stream exception states that the element has no bound                                                                                                                                                                                                                                                                                                                                                                        |
| [ADR-0030](ADR-0030-transfer-and-bound-limits.md)                                              | decision 11: data larger than one transfer is a byte stream or a stream of bounded elements (decision 8). decision 16: the interaction's limit bounds each chunk or element of a stream (decision 14). Status: a paragraph for the amendment of 2026-10-31                                                                                                                                                                                                                                                                                                                                                                                             |
| [`docs/design/ridl-rt.md`](../design/ridl-rt.md)                                               | "What 0.1 leaves out": the stream port follows decision 9 and is not built (driftsys/ridl#1116)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| [`docs/specification/ridl-family-overview.md`](../specification/ridl-family-overview.md)       | §5: a decision-ledger row for this record                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| [`docs/book/getting-started.md`](../book/getting-started.md)                                   | "Collections and streams": a byte stream example with `<bytes>`, and the distinction from a stream of named elements such as `<FwBlock>`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| [`docs/decisions/README.md`](README.md)                                                        | the entry for this record loses its _Proposed_ mark and summarises decisions 9 to 14                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |

## References

- driftsys/ridl#336 — `ridl-rt` has no port for a stream.
- driftsys/ridl#1116 — the stream work: the port's signatures, the frame version
  that adds streams, and the generated face.
- driftsys/ridl#1109 — the ADR-0030 amendment: payload ceilings by kind, and
  splitting a payload as a transport concern.
- [ADR-0011](ADR-0011-provisioned-constant-keyword.md) and
  [ADR-0015](ADR-0015-qos-absorption-and-rpc-bounds.md) — the language surface
  this record leaves unchanged.
- [ADR-0021](ADR-0021-ridl-rt-0.1-api-and-release.md) and
  [`docs/design/ridl-rt.md`](../design/ridl-rt.md) — the ports, and the stream
  port that 0.1 leaves out.
- [`docs/design/ridl-transport-ws.md`](../design/ridl-transport-ws.md) — the
  WebSocket binding that will carry the stream framing.
