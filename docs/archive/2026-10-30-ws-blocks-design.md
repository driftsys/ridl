# WebSocket blocks — design note

Status: design note, working memory (driftsys/ridl#1129). Not implemented. The
frame specification, the two design records of the WebSocket binding and the
binding rationale technote are amended when the code lands, not before (see
"Records to amend when the code lands").

Satisfies: [ADR-0030](../decisions/ADR-0030-transfer-and-bound-limits.md)
decision 15 ("WebSocket: blocks", as amended on 2026-10-30), driftsys/ridl#1129.

Related: ADR-0030 decisions 10, 16, 19 and 20;
[the frame specification](../specification/frame-specification.md) §4, §5.5,
§6.1, §6.3, §6.4, §9.4, §10, §11.1 and §12;
[`../design/ridl-transport-ws.md`](../design/ridl-transport-ws.md);
[`../design/ridl-transport-ws-ts.md`](../design/ridl-transport-ws-ts.md);
[the binding rationale technote](../technotes/websocket-binding-rationale.md).

## 1. The problem

The WebSocket binding (frame specification §11.1, version 1) carries one frame
in one WebSocket message, and the payload is every byte after the header.
ADR-0030 as amended on 2026-10-28 and 2026-10-30 fixes the numbers this note
works with:

| Fact                                                                      | Value                      | Source              |
| ------------------------------------------------------------------------- | -------------------------- | ------------------- |
| largest message of the Rust provider (default and cap)                    | 36 + 16,128 = 16,164 bytes | decision 10         |
| default payload ceiling of a signal and an event                          | 3,840 bytes                | decision 16         |
| default payload ceiling of a command and a query                          | 16,128 bytes               | decision 16         |
| largest raised limit of a signal, event, command, query                   | 16,128 bytes               | decision 18         |
| default payload ceiling of a fixed, and its largest limit                 | 65,280 bytes               | decisions 16 and 18 |
| a binding never refuses an interaction the catalog allows                 | —                          | decision 20         |
| a transport should copy at most 4,096 bytes between two scheduling points | —                          | decision 16         |

Every signal, event, command and query fits one message: the largest is a
`response` under `reply` with a 16,128-byte reply, 36 + 16,128 = 16,164 bytes. A
`fixed` does not: a `publish` header is 27 bytes, so a fixed payload of more
than 16,164 − 27 = 16,137 bytes cannot cross in one message. Decision 20 forbids
refusing such a catalog, and decision 15 settles that the binding splits that
payload into **blocks**. A fixed crosses only in a `publish` that answers a
`read` (frame specification §5.5, §6.3); it never crosses in a `response`. So
the only frame that is split is a `publish` of a `fixed`, in one direction,
provider to consumer.

The owner's decisions of 2026-10-30 that this note follows:

- The catalog is the single source of a limit. The sender, the transport and the
  receiver all take the limit of an interaction from it.
- Blocks are used only for a payload larger than one message. Today that is a
  fixed above 16,137 bytes, of up to 65,280 bytes, provider to consumer. The
  provider never reassembles. The consumer reassembles into a buffer sized from
  the catalog limit, then verifies.
- Rejected: every payload above 4 KiB in 4 KiB blocks, in both directions. It
  can be added later if a measurement asks for it.
- Blocks are a transport concern. Nothing about blocks goes into the catalog,
  the catalog descriptor or the codecs of `ridl-rt`.

## 2. Decisions

### B-1. Frame version 2, binding version 2, subprotocol `ridl-ws-v2`

Blocks add a form and two header fields. The header's field set is closed (§4)
and "a new value in any of these tables is a new frame version" (§4), so blocks
are **frame version 2**, carried by **binding version 2** under the subprotocol
**`ridl-ws-v2`**, as §11.1 "Later versions" prescribes. Frame version 2 is frame
version 1 plus the `block` form of B-2. Every layout, code and vector of version
1 is unchanged, including `attach`, whose `frame_version` field now carries 2.

**Negotiation.**

- The **consumer** (the TypeScript package) offers `ridl-ws-v2, ridl-ws-v1`,
  newest first, as §11.1 item 1 already says a client that speaks several
  versions does. It sends `attach` with the `frame_version` of the subprotocol
  the server selected: 2 on `ridl-ws-v2`, 1 on `ridl-ws-v1`. On a version 1
  session it receives no `block`, and a `block` there is an unknown form, a
  protocol violation as today. Speaking version 1 costs the consumer one
  variable, so a version 2 consumer keeps working against a version 1 provider
  for every catalog that version 1 can carry.
- The **provider** (the Rust crate) serves frame version 2 only. At the opening
  handshake it selects `ridl-ws-v2` when the client offers it. When the client
  offers `ridl-ws-v1` and not `ridl-ws-v2`, it selects `ridl-ws-v1`, reads the
  `attach`, and answers `attached(refused(frame_version_unsupported))`, then
  closes with 1000, as it does today for any refusal. A request that offers
  neither is answered `400 Bad Request`, as today.

**How a version 1 peer is refused:** with the existing refusal reason
`frame_version_unsupported` (code 4), in the existing `attached` frame. No new
code is needed, and §11.1 version 1 already allows a provider to refuse a frame
version it does not serve.

Why the provider does not also serve version 1: a version 1 session cannot carry
a fixed above 16,137 bytes, and decision 20 forbids refusing it. Serving version
1 only for a catalog whose fixed limits all fit one message would make the
accepted versions depend on the catalog, and with an absent limit a fixed's
limit is its default of 65,280 bytes (ADR-0030 decision 13), so almost every
catalog with a fixed would be refused anyway. One rule, "this provider speaks
frame version 2", is simpler to state, to test and to diagnose. The crate is
before 1.0 and its next release is already marked breaking (driftsys/ridl#1108).

**Streams move to frame version 3.** The frame specification §13 and the binding
rationale technote ("Streams arrive as frame version 2 and binding version 2")
reserve version 2 for streams. Blocks take version 2; streams (ADR-0032,
driftsys/ridl#336) arrive as version 3 by the same mechanism. Both records are
amended when the code lands.

### B-2. A new form `block`, not fields on `publish`

Frame version 2 adds one form:

| Form    | Plane | Direction           | Meaning                                                                                    |
| ------- | ----- | ------------------- | ------------------------------------------------------------------------------------------ |
| `block` | data  | provider → consumer | one part of a `publish` of a `fixed` whose payload does not fit one message of the binding |

`form` code: **`block` 11**, the next value after `response` 10. No other code
table changes: no new outcome, refusal reason, provenance or `Rule`.

Two header fields are added to §4's table, both present on `block` only:

| Field   | Domain                                                          | Present on |
| ------- | --------------------------------------------------------------- | ---------- |
| `total` | `u32`: the length in bytes of the whole payload, at least 1     | `block`    |
| `index` | `u32`: the position of the block in the payload, counted from 0 | `block`    |

A `block` also carries `interface`, `ordinal`, `kind` and `envelope`, as a
`publish` does, and a payload: the bytes of that part. It carries no
`provenance`: only a `publish` under `Live` has a payload (§5.1, §5.5), so every
`block` is a part of a `publish` under `Live`, and the reassembled frame is that
`publish`. It carries no `correlation` and no `outcome`.

Every block of one payload carries the same `interface`, `ordinal`, `kind`,
`envelope` and `total`. The header stays canonical (§4): every `block` carries
every field, and none has a default.

In frame version 2 the `kind` of a `block` is `Fixed`. Another kind is a value
outside the form's table, a protocol violation. A later version that needs to
split another kind, or a `response`, adds it then.

Why a new form: the field set of `publish` is closed and has no optional field,
so `total` and `index` on `publish` would be present on every `publish`, every
signal publication included, at 8 bytes each, to serve one read per session of
one fixed. A second, separate form leaves `publish` and all its vectors as they
are, and a receiver tells a whole frame from a part by its `form` code alone.

### B-3. Byte layout

Little-endian, no padding, as every form of §11.1:

| Form    | Bytes                                                                                                         | Header size |
| ------- | ------------------------------------------------------------------------------------------------------------- | ----------- |
| `block` | the common 10 (`form`, `kind`, `interface`, `ordinal`), then `stamp` i64, `seq` u64, `total` u32, `index` u32 | 34          |

The block's bytes are every byte after the header, as for every other form.

The largest data-plane header of version 2 is still the `response`'s: 20 bytes
without the envelope. The `block`'s is 18 bytes without it. So the `websocket`
row of the codegen model (`crates/ridl-ir/src/codegen/bindings.rs`) keeps
`frame_header_max_bytes` 20 and `envelope_bytes` 16, and its `version` becomes
`"2"`.

### B-4. Block size: 4,096 bytes of payload

Every block but the last carries exactly **4,096 bytes** of the payload. The
last block carries the rest: `total − 4,096 × index` bytes, from 1 to 4,096. A
payload of `total` bytes has `n = ceil(total / 4,096)` blocks, with `index` from
0 to n − 1. Each block message is at most 34 + 4,096 = 4,130 bytes.

The block size is a constant of binding version 2, not a configuration value, so
the two ends never disagree about it. Changing it is a new binding version.

| Fixed payload           | Blocks | Last block  | Bytes on the wire |
| ----------------------- | ------ | ----------- | ----------------- |
| 16,138 bytes (smallest) | 4      | 3,850 bytes | 16,274            |
| 32,768 bytes            | 8      | 4,096 bytes | 33,040            |
| 65,280 bytes (largest)  | 16     | 3,840 bytes | 65,824            |

Why 4,096 and not a whole message:

- **The copy rule of decision 16.** A binding should copy at most 4,096 bytes
  between two scheduling points. With 4,096-byte blocks, the receiver copies one
  block, at most 4,096 bytes, per message it handles, and a browser, Deno or an
  embedded executor reaches a scheduling point between two messages. A sender
  that writes one block per step can do the same. With blocks the size of a
  whole message, every copy is up to 16,130 bytes, four times the rule.
- **Page alignment.** Block `i` lands at offset `4,096 × i` of the reassembly
  buffer, so a buffer aligned on a page is filled one page per block, and the
  fixed ceiling itself is 16 pages less 256 bytes (decision 16). With blocks of
  16,130 bytes, the offsets fall in the middle of pages.
- **Independent of the message limit.** A block of 4,130 bytes fits any message
  limit from 4,130 bytes up, so a provider whose limit is lowered (decision 10)
  still splits the same way, and the consumer, which does not know the
  provider's limit, never has to.
- **Cost.** 16 messages and 544 bytes of headers for the largest fixed, against
  5 messages and 170 bytes with whole-message blocks. A fixed is read once per
  session (§5.5), so the 374 extra bytes are paid once per session.

**When the provider splits.** A provider sends a fixed whole, in one `publish`,
when the `publish` fits its message limit: payload ≤ limit − 27, which is 16,137
bytes at the limit of 16,164. Otherwise it sends blocks. This is a rule for the
sender only. The consumer does not know the provider's message limit, so it
accepts blocks for any `total` from 1 up and checks `total` against the catalog
limit only (B-6).

A provider whose message limit is below 4,130 bytes cannot send a block. Under
ADR-0030 decision 10 as amended on 2026-10-30, a `Config` whose message limit
cannot carry a payload the catalog allows, whole or in blocks, is refused at
attach as a configuration error. For a fixed, "in blocks" means a message limit
of at least 4,130 bytes.

### B-5. `index` and `total` in every block, and no last-block flag

The receiver needs three facts: where a block goes, when the payload is
complete, and whether the payload is above the limit before it copies a byte.

- `total` in every block answers the third at the first block, before any
  allocation or copy, and with the fixed block size it also gives `n`, so the
  last block is the one whose `index` is n − 1. It repeats in every block
  because the header is closed; the cost is 4 bytes per block.
- `index` makes a missing, repeated or reordered block detectable by a
  comparison with the next expected index.

How the consumer sizes its buffer: from the catalog limit of the interaction,
never from the sender. The limit is the member's limit in the catalog
descriptor's limit column, or the default ceiling of the `fixed` kind, 65,280
bytes, when the column holds none (ADR-0030 decisions 13 and 16). A `block` with
`index` 0 whose `total` is above that limit is refused before a byte is copied
(B-6). A consumer may allocate one buffer per session with the capacity of the
largest fixed limit of its catalog and reuse it, or allocate `total` bytes per
payload; both are bounded by the limit.

**Ruling R-B1 (the coordinator, 2026-10-30, for the owner's review): the
TypeScript consumer takes each member's limit from the generated codec module.**
The generated `<base>.codec.ts` module carries the catalog limits, so that the
catalog stays the single source of a limit. `ConnectOptions` gains no limit
field. For a member with no limit entry, the default ceiling of its kind from
the catalog rules applies: 65,280 bytes for a fixed. The TypeScript backend
(`crates/ridl-backend-ts`) emits the member limits into that module (task 6a).

A consumer attached under a compatible earlier hash (§6.1) has the same limits
as the provider: a change of a limit is breaking (ADR-0030 decision 13), so it
ends the compatible chain.

### B-6. Reassembly rules and what each failure does

**One reassembly at a time per session.** The consumer holds at most one
reassembly: one `(interface, ordinal)`, one envelope, one `total`, and the next
expected `index`. Its memory is one buffer of at most the fixed limit, 65,280
bytes at most. The provider sends the blocks of one payload before it starts the
blocks of another.

**Other frames may come between blocks.** Any frame of another form, and a
`block` of the same payload, may follow a `block`. A frame of another form is
handled as if no reassembly were in progress, with one exception below. The Rust
provider as first built sends the blocks of one payload one after the other with
nothing between them; the rule lets a later sender, or this one, write one block
per step and send a `response` or an `occurrence` in between, which is what the
copy rule of decision 16 asks of a sender.

| Case                                                                                                                                                                                                                                                  | What the consumer does                                                                                                                                                                                                                                                                                                                                        |
| ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `block` with `index` 0, no reassembly in progress, the member known (or no member table), `total` within the limit                                                                                                                                    | starts a reassembly and copies the block                                                                                                                                                                                                                                                                                                                      |
| next `block` with the expected `index` and the same identity, envelope and `total`                                                                                                                                                                    | copies it; after the last block, the reassembled `publish` (`Live`, the blocks' envelope, the `total` bytes) is handled by §5.5, §6.3 and §6.4 as a `publish` that arrived whole: verified by the generated binding (§9.4), delivered to the waiting `read`                                                                                                   |
| `block` with `index` 0 and `total` above the member's limit                                                                                                                                                                                           | refuses the payload (ADR-0030 decision 19): every waiting `read` of that `(interface, ordinal)` reports the structure failure of §9.4 (`Detected(Corrupt)`; `corrupt` in the TypeScript package), and the event is recorded; the consumer then skips the remaining blocks of that payload, checking their sequence but copying nothing. The session continues |
| `block` with `index` 0 naming an `(interface, ordinal)` that the consumer's member table does not hold as a `fixed`                                                                                                                                   | discarded and recorded (§6.4); the remaining blocks are skipped as above. A consumer with no member table cannot tell, reassembles, and handles the result as any unsolicited `publish` (discarded and recorded)                                                                                                                                              |
| `block` whose `index` is not the next expected: a missing block (greater), a repeated block (smaller), a reordered block                                                                                                                              | protocol violation (§11.1 item 4): recorded, the connection closed with 1002 (the TypeScript package closes with no code, its D-10), the session ends as §3 states, and every waiting `read` reports `Down`                                                                                                                                                   |
| `block` with `index` above 0 and no reassembly in progress                                                                                                                                                                                            | protocol violation, as above                                                                                                                                                                                                                                                                                                                                  |
| `block` with `index` 0 while another reassembly is in progress (two payloads interleaved)                                                                                                                                                             | protocol violation, as above                                                                                                                                                                                                                                                                                                                                  |
| `block` whose `interface`, `ordinal`, `envelope` or `total` differ from the reassembly in progress                                                                                                                                                    | protocol violation, as above                                                                                                                                                                                                                                                                                                                                  |
| a whole `publish` naming the `(interface, ordinal)` of the reassembly in progress                                                                                                                                                                     | protocol violation, as above: the provider answers one `read` with one `publish` or with its blocks, never both                                                                                                                                                                                                                                               |
| one `block` that is wrong by itself: `kind` not `Fixed`, `total` 0, no bytes after the header, more than 4,096 bytes, `index` ≥ n, a block that is not the last with fewer than 4,096 bytes, a last block whose length is not `total − 4,096 × index` | protocol violation, found by the header decoder with no session state                                                                                                                                                                                                                                                                                         |
| a message above the receiver's message size limit                                                                                                                                                                                                     | closed with 1009, as today (§11.1 item 4)                                                                                                                                                                                                                                                                                                                     |
| the session ends with a reassembly in progress                                                                                                                                                                                                        | the reassembly is dropped with the rest of the session's state (§3); every waiting `read` reports `Down`                                                                                                                                                                                                                                                      |
| the provider receives a `block`                                                                                                                                                                                                                       | protocol violation: `block` is a provider-to-consumer form, and the provider never reassembles; closed with 1002, as for any provider-to-consumer form it receives today                                                                                                                                                                                      |

Why a sequence error ends the session rather than dropping the payload: the
transport is reliable and ordered (§11.1 item 5), so a missing, repeated or
reordered block can only be a defect of the sender, and §11.1 item 4 already
ends the session on a frame the receiver cannot read. Dropping the payload and
going on would leave the `read` waiting, because a `read` of the TypeScript
package has no bound of its own (the Rust crate is the provider and has no
`read`); ending the session reports `Down` at once.

Why a payload above the limit does not end the session: decision 19 makes the
refusal of a size a structure failure of the kind, which for a fixed is the
detection §9.4 reports on the read, and every other interaction of the session
stays usable. The skipped blocks are still checked for their sequence, so a
defect in them is still found.

The categories of §9 are unchanged: a reassembled payload that fails
`Payload::verify` is §9.4, a refusal for size is §9.4 through decision 19, and a
frame the binding cannot read is §9.5.

## 3. Conformance tests to add (frame specification §12)

Numbered after the 16 items of §12. Each holds on a version 2 session.

17. A `read` of a `fixed` whose payload fits one `publish` within the provider's
    message limit is answered with one `publish`, and with no `block`.
18. A `read` of a `fixed` whose payload does not fit one `publish` is answered
    with `ceil(total / 4,096)` `block` frames, `index` 0 to n − 1 in order, each
    with `kind` `Fixed`, the same `interface`, `ordinal`, envelope (`seq` 0) and
    `total`, every block but the last holding 4,096 bytes; the consumer delivers
    one sample whose payload is the provisioned value, byte for byte.
19. A frame of another form sent between two blocks of one payload is applied,
    and the payload is still delivered.
20. Each sequence error of B-6 — a missing, a repeated and a reordered block, an
    `index` above 0 with no reassembly, an `index` 0 during another reassembly,
    a block whose identity, envelope or `total` differs, and a whole `publish`
    of the interaction being reassembled — ends the session as a protocol
    violation.
21. Each single-block error of B-6 — `kind` not `Fixed`, `total` 0, no bytes,
    more than 4,096 bytes, `index` ≥ n, a short block that is not the last, a
    last block of the wrong length — is a protocol violation.
22. A `block` with `index` 0 whose `total` is above the member's limit makes the
    waiting `read` report the structure failure of §9.4, no byte of it is
    delivered, and a later `read` on the same session is answered.
23. A `block` received by a provider ends the session as a protocol violation.
24. An `attach` with `frame_version` 1 to a provider that serves version 2 only
    is refused `frame_version_unsupported`; a consumer that offers
    `ridl-ws-v2, ridl-ws-v1` to a version 1 provider attaches with
    `frame_version` 1.

The vector table of binding version 2 adds, beside the version 1 table, which is
never edited:

| Id           | Vector                                                                             | Hex                                                                                                     |
| ------------ | ---------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------- |
| `attach-2`   | `attach` frame 2, FlatBuffers, `veh.cabin`                                         | `01 02000000 01 000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f 09 7665682e636162696e` |
| `block-last` | `block` fixed, interface 1, ordinal 5, seq 0, total 4,098, index 1, payload `abcd` | `0b 05 01000000 05000000 ab89674523010000 0000000000000000 02100000 01000000 abcd`                      |

The stamp is the version 1 table's `0x0123456789AB`. A first block holds 4,096
bytes, too long for a table row, so it is tested by the round trip of item 18
and by a test that builds it, not by a vector. The vectors are frozen through
the `wire` dump as rows of their own, so the version 1 rows do not change.

**The end-to-end test.** `examples/loopback_server.rs` of `ridl-transport-ws`
provisions a fixed of 65,280 bytes, the largest a fixed may be, with a known
pattern (byte `i` is `i mod 251`, so that a block placed at the wrong offset
changes the bytes). The Deno end-to-end test of the package (`just ts-e2e`) and
the browser one (`just ts-browser-e2e`) read it and compare the result byte for
byte, and record through `onNote` that no block was discarded. A second fixed of
16,137 bytes, and a third of 16,138 bytes, check the boundary of B-4: the first
crosses whole, the second in 4 blocks. A Rust test in `tests/serve.rs` reads the
same 65,280-byte fixed through a `tungstenite` client and checks the 16 block
messages and their lengths (15 × 4,130 bytes and one of 3,874).

## 4. Implementation tasks (for a later plan)

The code depends on driftsys/ridl#1108 (the per-kind ceilings, the message limit
of 16,164 bytes, and the three default ceilings exported by `ridl-rt`) and on
the receiver side of driftsys/ridl#1128 (a consumer checks a payload's length
against its member's limit). It is a breaking release of both packages, which
#1108's release already is.

| #  | Task                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          | Estimate |
| -- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------- |
| 1  | Rust codec (`crates/ridl-transport-ws/src/frame.rs`): `FRAME_VERSION` 2, `SUBPROTOCOL` `ridl-ws-v2` and a constant for `ridl-ws-v1`, `BLOCK_BYTES` 4,096, `Frame::Block` (`Frame` is `#[non_exhaustive]`), its encoder and decoder with the single-block checks of B-6, and the version 2 vectors in `tests/vectors.rs`                                                                                                                                                                                                                                       | 0.5 day  |
| 2  | Rust handshake (`src/ws.rs`): select `ridl-ws-v2` when offered, else `ridl-ws-v1`, else 400; the session refuses `frame_version` 1 with the existing code path; a received `block` is a consumer-side form and closes with 1002                                                                                                                                                                                                                                                                                                                               | 0.5 day  |
| 3  | Rust provider (`src/server/session.rs`): `read_fixed` reads the value into a buffer bounded by the member's limit, or by 65,280 bytes with no member table, in place of the one-message `payload_cap`; sends one `publish` when it fits the message limit and blocks otherwise; the `Config` check at attach of decision 10 for a message limit below 4,130 bytes with a fixed that needs blocks                                                                                                                                                              | 1 day    |
| 4  | Rust tests: unit tests in `session.rs` (the boundary of B-4, the block sequence, a `block` received), `tests/serve.rs` (the 65,280-byte round trip through `tungstenite`), and the freeze dumps (`wire` gains the version 2 rows, `runtime-crates` the new constants and variant)                                                                                                                                                                                                                                                                             | 1 day    |
| 5  | TypeScript codec (`packages/ridl-transport-ws/src/frame.ts`): `FRAME_VERSION` 2, both subprotocol names, the `block` form and its single-block checks; `tests/vectors_test.ts` and `tests/decode_errors_test.ts`                                                                                                                                                                                                                                                                                                                                              | 0.5 day  |
| 6  | TypeScript session (`src/session.ts`): offer `ridl-ws-v2, ridl-ws-v1` and attach with the selected version; the reassembly slot (collecting or skipping) and the rules of B-6; each fixed's limit taken from the generated codec module (R-B1), with the default ceiling of the kind for a member with no entry; a `read` that rejects with `corrupt` for a payload above the limit                                                                                                                                                                           | 1 day    |
| 6a | TypeScript backend (`crates/ridl-backend-ts`): emit each member's payload limit from the catalog into the generated `<base>.codec.ts` module (R-B1), with its tests. If the size-rules lane (driftsys/ridl#1128) lands first and already emits the limits, this task is done there                                                                                                                                                                                                                                                                            | 0.5 day  |
| 7  | TypeScript tests: `tests/session_test.ts` over `tests/fake_socket.ts` (every row of B-6), `tests/version_test.ts` (both subprotocols)                                                                                                                                                                                                                                                                                                                                                                                                                         | 1 day    |
| 8  | End-to-end: the three fixed values in `examples/loopback_server.rs` and `e2e/catalog.ts`, read by `e2e/deno_test.ts` and `e2e/browser_test.ts`                                                                                                                                                                                                                                                                                                                                                                                                                | 0.5 day  |
| 9  | Records: amend the frame specification as "Records to amend when the code lands" places each part (§4 the form, its code and its fields; §5.5, §6.3 and §9.4 the receiver rules of B-5 and B-6; §10 a tenth item for the block size and when the sender splits only; §11.1 the version 2 section, its values and its vectors; §12 items 17 to 24; §13 streams at version 3), ADR-0030 decision 15's threshold sentence, both design records, the binding rationale technote, the `websocket` row of the codegen model (`version` `"2"`), and garden this note | 1 day    |

Total: about 7.5 days of implementer time, in three pull requests: the Rust side
(tasks 1 to 4), the TypeScript side (tasks 5 to 7, with 6a), and the end-to-end
test with the records (tasks 8 and 9). The TypeScript side can be written in
parallel with the Rust side from this note; the end-to-end test needs both.

## 5. Alternatives considered

**Blocks for every payload above 4 KiB, in both directions.** Rejected by the
owner on 2026-10-30 (ADR-0030 decision 15). Every signal, event, command and
query fits one message, so splitting them adds a reassembly on the provider for
no payload that needs it. It can be added in a later binding version if a
measurement shows that the copy of a 16 KiB message is too long.

**Blocks as binding framing, with frame version 1 unchanged.** The binding could
split any encoded message, header included, into fragments, as WebSocket
fragmentation does below it, and leave the logical frame alone. Rejected: a
fragment does not name its interaction, so the receiver cannot check the size
against the interaction's limit until the inner header is reassembled, and the
catalog would not be the source of the bound. ADR-0030 decision 15 also settles
that blocks are a new frame version.

**`total` and `index` as fields on `publish`.** Rejected (B-2): the closed field
set puts them on every `publish`.

**A first block that carries the header and later blocks that carry only an
index.** Two forms in place of one, and a later block that cannot be attributed
to its interaction without session state. Repeating the identity, the envelope
and `total` in every block costs under 30 bytes per block after the first, under
500 bytes for the largest fixed, and lets the decoder check each block by
itself.

**A last-block flag in place of `total`.** The receiver would learn the size
only at the end, so it could not refuse a payload above the limit before copying
it, and could not size the buffer from anything but the limit. A missing last
block would be found only by a timer.

**A byte offset in place of `index`.** With a fixed block size the two say the
same thing. An index keeps the block size in one constant and makes a reordering
a comparison with the next expected number.

**Whole-message blocks (16,130 bytes of payload).** Rejected in B-4: four times
the copy rule of decision 16, offsets that are not page-aligned, and a split
that depends on the sender's message limit.

**Several reassemblies at once per session.** Rejected: the memory per session
would be a multiple of 65,280 bytes, and nothing needs it, because the provider
answers reads one after the other.

**No interleaving between blocks.** Simpler to state, but it forbids a sender
from answering a call between two blocks, which is what the copy rule asks of a
sender on a single-threaded executor. The receiver's cost of allowing it is
nothing: it holds one reassembly either way.

**Dropping a payload with a sequence error and continuing.** Rejected in B-6:
the transport is reliable and ordered, so the error is a sender defect, and the
`read` would wait with no bound.

**The provider serves version 1 and version 2.** Rejected in B-1: on version 1
it would have to refuse a large fixed, which decision 20 forbids, or serve
version 1 only for some catalogs.

**A version 1 peer refused at the handshake (`400`).** Rejected: a browser
reports a failed handshake as a close with 1006 and no reason, whereas
`attached(refused(frame_version_unsupported))` tells the consumer why.

## 6. Dependencies and open points

- Dependency on the size-rules lane (driftsys/ridl#1128): a provisioned fixed
  longer than its member's limit is refused by the provider's port when the
  provider sets the value. Until that lands, the Rust provider records
  `ValueTooLarge` for such a value and sends nothing, which leaves the
  consumer's `read` waiting with no bound.

- The Rust provider reads the whole fixed from its backend in one call
  (`FixedReader::read_fixed` into one buffer), up to 65,280 bytes. That copy is
  the port's, not the binding's, and this note does not change the port.

## Records to amend when the code lands

Which record owns each part of this note (the coordinator's ruling, 2026-10-30).
§10 says a binding does not decide what a receiver does with a frame, so the
receiver rules belong to the frame, not to the binding:

| Part                                                                             | Owner                                                           |
| -------------------------------------------------------------------------------- | --------------------------------------------------------------- |
| the `block` form, its code 11, the fields `total` and `index`                    | frame specification §4 (and the table of §5.6)                  |
| the receiver behaviour: reassembly, B-5 and the failure table of B-6             | frame specification §5.5, §6.3 and §9.4                         |
| what a binding decides: the block size, and when the sender splits               | frame specification §10, a tenth item that names these two only |
| the WebSocket values: 4,096 bytes, the 34-byte layout, `ridl-ws-v2`, the vectors | frame specification §11.1, a binding version 2 section          |
| the conformance items 17 to 24                                                   | frame specification §12                                         |

ADR-0030 decision 15, "WebSocket: blocks", says that "only a fixed above 16,128
bytes, of up to 65,280 bytes, is sent in blocks". The precise threshold is
16,137 bytes (B-4). When the code lands, that sentence is corrected to "only a
fixed above what one `publish` carries, 16,137 bytes at the 16,164-byte message
limit, of up to 65,280 bytes, is sent in blocks".

The frame specification (§4, §5.5, §5.6, §6.3, §9.4, §10, §11.1, §12, §13),
[`../design/ridl-transport-ws.md`](../design/ridl-transport-ws.md),
[`../design/ridl-transport-ws-ts.md`](../design/ridl-transport-ws-ts.md),
[the binding rationale technote](../technotes/websocket-binding-rationale.md)
("Streams arrive as frame version 2 and binding version 2" becomes version 3,
and a section on blocks), the `websocket` row of the codegen model, and the
`wire` and `runtime-crates` freeze dumps. ADR-0030's Status names the binding's
blocks as not built (driftsys/ridl#1129); that sentence changes when they are.
