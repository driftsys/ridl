# ADR-0032 — Byte streams

## Status

Proposed — 2026-10-28. Scope: what a stream whose element is a bare `bytes` or a
bare `string` means (ridl reference §12.2), how the typl length default applies
in that position, and the generated surface intended for such a stream in each
target language. It binds the ridl and typl references, the checkers, the
backends that generate a stream, the stream port of `ridl-rt` when it is built,
and every transport binding that carries a stream.

Written from the owner's decisions of 2026-10-28. It adds no syntax, no
interaction kind and no diagnostic code. The specifications are not changed by
this record while it is proposed; when it is accepted, the documents in
"Documents amended" are changed. Nothing in this record is built: streams have
no port, no transport framing and no generated code (driftsys/ridl#336).

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
- ADR-0030 sets a payload ceiling of 16,256 bytes (decision 2) and states in
  decision 11 that data larger than one transfer is modelled as a stream of
  bounded elements. The open amendment of ADR-0030 (driftsys/ridl#1109) gives
  each interaction kind its own ceiling, in 4 KiB pages (3,840 bytes for
  `signal` and `event`, 16,256 bytes for `fixed`, `command` and `query`), and
  makes the splitting of a payload a concern of each transport binding.
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
   the payload ceiling of the interaction's kind under ADR-0030. The element has
   no bound: typl's `[0..256]` default and TYPL-103 do not apply to a bare
   `bytes` or `string` in stream element position. In every other position
   typl's rules are unchanged: a bare `string` or `bytes` field is still refused
   by §15.3, and a map key still takes the default with TYPL-103. RIDL-202 is
   unchanged.

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

   `ridl-rt` stays sans-IO (ADR-0018 decision 2): the async types belong to the
   generated face and to the async wrapper over the port, not to the port
   itself. A Rust target without `std` has no `AsyncRead` and no `AsyncWrite`;
   it needs a pull or callback API, which is an open question of this record.
   The Kotlin surface is generated by the Kotlin plugin, outside this workspace
   (ADR-0020 decisions 8 to 11).

7. **Framing, buffering, backpressure and chunk size stay transport concerns.**
   This is ridl §12.2 unchanged. This record does not define how the WebSocket
   binding frames a stream; that framing is part of the implementation work for
   streams.

8. **ADR-0030 decision 11 names both forms of a stream.** Data larger than one
   transfer is a stream: a byte stream (decisions 2 and 3) when its chunking is
   not part of the contract, or a stream of bounded elements (decision 4) when
   each element carries meaning.

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
- A transport that carries a byte stream must keep the payload ceiling of the
  interaction's kind for every chunk, and for `<string>` must place chunk
  boundaries between code points.

## Open questions

1. **A target without `std`.** A Rust target without `std` has no `AsyncRead`
   and no `AsyncWrite`. Which pull or callback API the generated face offers
   there, and whether it is the same API the port exposes, is not decided.
2. **Functional failure on a byte stream.** ridl §12.4 expresses a failure the
   producer knows about by making the element type a result union. A byte stream
   has no element type to carry one. Whether such a failure is reported through
   a separate typed value (the `describe` query of decision 5, or a status query
   after the transfer), or through some other means, is not decided.
3. **A normal end and an interrupted transfer.** A consumer of a byte stream
   must tell a complete transfer from one cut short by the transport (Stratum 3,
   ridl §10.3), because a truncated file looks valid. Whether that is a
   requirement on every binding's stream framing, beyond a digest the author
   declares as metadata, is not decided.
4. **Invalid UTF-8 in a `<string>` stream.** Whether content that is not valid
   UTF-8 ends the stream as a Stratum 2 `INVALID_VALUE`, as ridl §12.4 does for
   an element that violates its constraints, is not decided.
5. **The response bound of a streamed reply.** ridl §9.3 makes `max` on a
   `query` the bound on the reply. For a byte stream of unknown length, whether
   the bound covers the first chunk or the whole transfer is not decided.
6. **A `[limits]` entry and the chunk size.** Decision 2 bounds a chunk by the
   payload ceiling of the interaction's kind. Whether a `[limits]` entry for the
   interaction (ADR-0030 decision 12) bounds each chunk as well is not decided.

## Documents amended

When this record is accepted, these documents are changed.

| Document                                                                                       | Change                                                                                                                                                                                                                                                                                                      |
| ---------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [`docs/specification/ridl-language-reference.md`](../specification/ridl-language-reference.md) | §12.2: a bare `bytes` element is a byte stream and a bare `string` element a UTF-8 text stream, with no element bound and no contracted chunk boundary; a named element keeps its bound (decisions 2 to 4). §12 introduction and §12.3 "Maps to": the I/O mapping (decision 6). The glossary entry "stream" |
| [`docs/specification/typl-language-reference.md`](../specification/typl-language-reference.md) | §4.4 and §4.5: the `[0..256]` default and TYPL-103 do not apply to a bare `string` or `bytes` in ridl stream element position (decision 2). §12.2: the map-key paragraph's "the same as every other position". §15.3: the stream exception states that the element has no bound                             |
| [ADR-0030](ADR-0030-transfer-and-bound-limits.md)                                              | decision 11: data larger than one transfer is a byte stream or a stream of bounded elements (decision 8)                                                                                                                                                                                                    |
| [`docs/specification/ridl-family-overview.md`](../specification/ridl-family-overview.md)       | §5: a decision-ledger row for this record. §6: the open questions of this record, under ridl §17, if they are still open                                                                                                                                                                                    |
| [`docs/book/getting-started.md`](../book/getting-started.md)                                   | "Collections and streams": a byte stream example with `<bytes>`, and the distinction from a stream of named elements such as `<FwBlock>`                                                                                                                                                                    |
| [`docs/decisions/README.md`](README.md)                                                        | the entry for this record loses its _Proposed_ mark                                                                                                                                                                                                                                                         |

## References

- driftsys/ridl#336 — `ridl-rt` has no port for a stream.
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
