# ADR-0014 — IR encodings: canonical protobuf JSON, prototext, and binary

## Status

Accepted — 2026-08-04. Scope: how the IR itself is encoded on every surface that
writes or reads it. It is not epic-scoped: it binds the artifact every future
backend consumes, in the way ADR-0009 binds the gate and ADR-0010 binds the CLI
contract.

**Supersedes the rendering clause of
[ADR-0004](ADR-0004-implementation-sequencing-and-stack.md) §4** — the sentence
scoping the `serde`→JSON rendering to "debugging and golden tests". Everything
else in ADR-0004 §4 stands: protobuf compiled with `prost` remains the canonical
IR, and the rejected alternatives remain rejected.

The reasoning trail is
[`docs/wip/2026-08-03-ir-protobuf-encodings-design.md`](../wip/2026-08-03-ir-protobuf-encodings-design.md),
which carries the measurements and the API confirmations this record summarises.
This ADR was accepted under the delegated authority recorded in
[ADR-0005](ADR-0005-agent-enablement.md)'s working model — the design note was
written for review, and execution of roadmap stories E9.1 to E9.3 needs the
decisions fixed rather than pending.

## Context

ADR-0004 §4 fixes the IR as a protobuf schema compiled with `prost` and calls
the canonical stable form "the plugin-protocol wire format". Roadmap story E4.5
makes the obligation concrete: a third-party backend consumes the IR.

The JSON that ships today cannot serve that obligation. It is `serde`'s
rendering of the generated Rust structs, which is a different encoding from the
protobuf JSON mapping. It differs on four counts: enum values render as numbers
rather than names, field names stay `snake_case` rather than `lowerCamelCase`,
absent fields render as `null` rather than being omitted, and a `oneof` is
wrapped in its Rust variant name rather than flattened to the field name. The
variant names it emits — `TypeDef`, `FloatWidth` — do not exist in the protobuf
JSON mapping at all. A Kotlin or TypeScript backend pointed at this file with a
conformant protobuf JSON parser fails to parse it.

The binary encoding has no such defect, because it is schema-faithful by
construction, but no CLI path writes it. So the one interchange form that works
is unreachable, and the one that is reachable does not work.

Two facts about the toolchain shape the decision, both established against the
published APIs. `prost` 0.14 has no JSON and no text format — it exposes binary
encoding only, and documents that it carries no runtime reflection or message
descriptors, which is precisely what the JSON mapping needs. And the descriptor
set already exists: `protox::compile` returns a `FileDescriptorSet` in
`crates/ridl-ir/build.rs`, which is dropped after codegen today.

## Decision

1. **Canonical protobuf JSON replaces the `serde` rendering on every surface.**
   The `--emit ir-json` artifact, `.ridl/baseline/*.ir.json`, and the `insta`
   goldens all carry one dialect. There is no migration path and none is needed:
   the version is `0.0.0` with no tags, so no baseline exists outside this
   repository.

2. **A field holding its default is emitted rather than skipped** —
   `skip_default_fields(false)`, in both JSON and prototext. This stays
   conformant on the read side, because a conformant parser must accept
   explicitly present defaults. It keeps goldens explicit: a reviewer reads
   `ordinal: 0` rather than inferring it from absence, which matters where
   ordinals are semantically load-bearing (ridl §11). Whether an **unset proto3
   `optional`** is also emitted is a separate question this option may not
   settle — see Open item 1.

3. **Goldens stay JSON.** Prototext is not used for `insta` snapshots. A golden
   in a format no shipped artifact uses would test the renderer rather than the
   artifact.

4. **All three protobuf encodings are emittable**, with these flag values and
   extensions:

   | Flag value  | Artifact          | Encoding                |
   | ----------- | ----------------- | ----------------------- |
   | `ir-json`   | `<base>.ir.json`  | canonical protobuf JSON |
   | `ir-text`   | `<base>.ir.txtpb` | prototext               |
   | `ir-binary` | `<base>.ir.binpb` | protobuf binary         |

   `ir-json` keeps its name rather than becoming `ir-pbjson`. The `.ir.json`
   suffix is load-bearing — it drives snapshot detection in
   `crates/ridl/src/main.rs` and is cited across ADRs, documents, and tests.
   Flag values stay plain English while extensions follow the `buf` convention,
   which is the existing repository precedent: `c-header` writes `.h`,
   `typescript` writes `.ts`.

5. **`ridl diff` and `ridl check --baseline` stay JSON-only.** Baselines remain
   `.ir.json`. A committed baseline must be reviewable in a pull request, which
   binary is not.

6. **The `serde` derives come off the generated types.** The `type_attribute` in
   `build.rs` is removed. `serde` and `serde_json` remain underneath
   `prost-reflect` as the JSON writer, but no longer determine the shape.

7. **The two text encodings go through `prost-reflect` and a build-time
   descriptor pool; binary needs neither.** `build.rs` writes the
   `FileDescriptorSet` it already holds to `OUT_DIR`, and `lib.rs` holds a
   `LazyLock<DescriptorPool>` over an `include_bytes!` of that file. No new
   build step, no system `protoc`, and no vendored blob in the tree — the pool
   is derived from the same schema compilation that generates the types, so the
   two cannot disagree.

   Six functions in `ridl_ir::v2` where there is one today. `to_json_pretty`
   keeps its name, so no call site is renamed. **It does not keep its infallible
   return — see decision 12, which retracts that clause.**

   | Function                              | Mechanism                      |
   | ------------------------------------- | ------------------------------ |
   | `to_json_pretty` / `from_json`        | `prost-reflect`, `serde`       |
   | `to_text_format` / `from_text_format` | `prost-reflect`, `text-format` |
   | `to_binary` / `from_binary`           | `prost`, no descriptors        |

   `from_text_format` is required rather than speculative: without it the
   prototext emit has no round-trip test, and a write path with no read path is
   untested by construction. **It is not public** — see decision 13.

8. **Canonical 64-bit stringification is kept, not overridden.**
   `stringify_64_bit_integers` stays at its default of `true`, so a timing bound
   emits as `"10000"` rather than `10000`. JavaScript loses integer precision
   above 2^53 and the TypeScript backend already models timing as `bigint`;
   overriding the canonical behaviour would break the consumers this change
   exists to serve. Prototext is set to `pretty`, `skip_default_fields(false)`,
   and `print_message_fields_in_index_order`, so its output ordering is
   deterministic rather than incidental.

9. **The canonical-form policy, for E4.5 to cite: binary is canonical, JSON is
   derived and conformance-obliged, prototext is for inspection.** Prototext is
   emittable but is not a recommended interchange form. ADR-0004 targets Rust,
   Kotlin, and TypeScript backends; C++, Java, Python, and Go have solid
   text-format parsers, and TypeScript has essentially none — neither
   protobuf.js nor ts-proto implements it.

10. **The `ridl.std` emit filter becomes a predicate over "is this an IR dump",
    not an enumeration of one variant.** `crates/ridlc/src/lib.rs` filters
    `ridl.std` out of `Emit::IrJson` today, because a direct IR dump is not code
    and `ridl diff` compiles the other side without `ridl.std`. Binary and
    prototext are direct IR dumps by the identical argument, so both must fall
    on the same side of that filter. There is exactly one such site — the
    closure that builds the local `code_emits` in `run_build` — and the
    classification it applies must be exhaustive over `Emit`, so a new encoding
    left unclassified is a compile error rather than a spurious
    `ridl.std.ir.binpb` on every build.

11. **The conformance claim is tested by re-reading, not by asserting on output
    text.** The emitted JSON is re-read with `DeserializeOptions` configured to
    reject unknown fields. Asserting on the rendered text would only restate the
    serializer's behaviour back to itself; re-reading tests the claim the change
    actually makes, which is that a conformant parser accepts this output.

12. **Amendment (2026-08-04) — the serialization surface is fallible on both
    directions, retracting decision 7's infallible return.** Decision 7 kept
    `to_json_pretty` infallible on the reasoning that "the new failure modes are
    the same class as the existing `expect` — they cannot occur unless the
    schema and the generated types disagree." **That reasoning is false**, and
    the E9.1 review demonstrated it.

    `prost-reflect` transcodes by encoding the typed message and decoding it
    into a `DynamicMessage`, and prost's `RECURSION_LIMIT` is a non-configurable
    constant of 100 message levels. Each level of inline composite nesting costs
    **two** message levels for an array or a map (`FieldType` plus `ArrayType`
    or `MapType`) and **three** for a tuple, because `TupleField` is itself a
    message. So the limit is reached at roughly 49 levels of array nesting and
    roughly 32 levels of tuple nesting, and the tuple bound is the one that
    matters, being the tighter of the two. Measured on the E9.1 branch: a
    package nested 45 array levels deep serializes and round-trips correctly and
    at 55 it fails; through the CLI a 30-level tuple source succeeds and a
    40-level one fails. The failure is **input-dependent**, not schema drift, so
    no `expect` on that path is justified.

    This is reachable from legal source. A `.typl` file declaring 55 nested
    inline arrays passes the lexer, the parser, and the checker, and then
    `ridlc build --emit ir-json` panics — while `--emit rust`,
    `--emit c-header`, and `--emit typescript` all emit that same package
    correctly. A panic in a compiler on input it accepted is a defect, and on
    the write path it is a regression against the `serde` rendering this record
    replaces, which had no such limit.

    Therefore: `to_json_pretty` returns a `Result`; `from_json` maps the decode
    failure into its existing error return rather than expecting on it; and the
    one production call site reports the failure as a detached error diagnostic
    and writes no artifact, which is the pattern `ridlc` already uses for the
    TypeScript backend's `Unrepresentable` error. The five remaining call sites
    are tests.

    **A checker-level nesting limit was rejected.** It would restrict input that
    three of the four emits handle correctly, which is a language change made to
    work around a library limit. If the bound ever binds in practice, the escape
    hatch is the one the Alternatives section already records: the JSON
    mechanism sits behind `to_json_pretty` and `from_json`, and `pbjson` emits
    straight-line field writes with no transcode and therefore no recursion
    limit. That reversibility was recorded as a hypothetical; it is now a
    concrete contingency with a known trigger.

13. **Amendment (2026-08-04) — the prototext reader is crate-private, and the
    writer's ceiling is a documented limit rather than a defect.**
    `prost-reflect`'s text parser recurses per message level with frames large
    enough that a debug build exhausts a 2 MiB stack at roughly 45 levels of
    nesting — **below** prost's recursion limit of 100. On that path the error
    return decision 12 relies on is unreachable, and the process aborts. A stack
    overflow cannot be caught in Rust, so this is not fixable the way decision
    12's write-side panic was.

    It is contained by **reach** instead. Nothing in the toolchain reads
    prototext; `ridl diff` and `ridl check --baseline` refuse the encoding by
    name (decision 5); and `from_text_format` is now compiled only for
    `ridl-ir`'s own tests — `#[cfg(test)]` rather than merely `pub(crate)`,
    because outside the tests it has no caller at all and the gate denies dead
    code. No consumer can reach the hazard, and none can link it either. The
    round-trip test decision 7 asked for survives, run on an explicitly sized
    stack. The one test outside the crate that parsed a prototext artifact now
    asserts the artifact is byte-identical to what the writer renders for the IR
    its siblings carry — the same property, and a stricter assertion, because it
    also pins the decision 8 rendering options.

    **The writer is kept as it is.** It shares the transcode and therefore the
    same recursion limit, but it fails closed: `to_text_format` returns an
    error, and the CLI reports a diagnostic and writes no artifact. A ceiling on
    an inspection format that fails cleanly is a documented limit, not a defect
    — unlike the same ceiling on the interchange format, which is what decision
    12 records.

    Making the reader public again means giving it a stack strategy first —
    running the parse on an explicitly sized thread is the obvious candidate,
    because it makes prost's own limit the thing that bites and therefore makes
    the error return reachable. Recorded as debt on driftsys/ridl#218 rather
    than built for a consumer that does not exist.

## Alternatives considered

| Candidate                                             | Verdict  | Reason                                                                                                                                                                                                                    |
| ----------------------------------------------------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `pbjson-build` instead of `prost-reflect`             | rejected | generates canonical JSON at build time with no runtime pool, and was the recommendation until prototext entered scope. It is JSON only, and prototext needs a runtime descriptor pool, so its single advantage disappears |
| Prototext for goldens                                 | rejected | it reads better — `snake_case` names, bare enum names, no 64-bit stringification — but a golden in a format no shipped artifact uses tests the renderer rather than the artifact                                          |
| Prototext as the recommended interchange format       | rejected | TypeScript has no usable text-format parser, and TypeScript is a named target of ADR-0004                                                                                                                                 |
| Additive only — keep `serde` JSON, add new emits      | rejected | zero churn, but it leaves the misleading artifact shipped under the name every consumer reaches for first, and raises the dialect count instead of lowering it                                                            |
| A compatibility shim for existing baselines           | rejected | the version is `0.0.0` with no tags and nothing published, so no baseline exists outside this repository                                                                                                                  |
| Reuse `TimingChanged`-style enumeration in the filter | rejected | see decision 10 — an enumeration of variants is what allowed the defect to be latent, and the next encoding would reintroduce it                                                                                          |

The `prost-reflect` cost is real and small, and is recorded rather than hidden.
It renders JSON by transcoding the typed message and then walking that tree with
a descriptor lookup per field, where `pbjson` emits straight-line field writes —
estimated at three to five times on the serialization step, **reasoned from what
transcoding does rather than measured**. Measured baselines put it in
proportion: on a two-package workspace with a debug binary, `--emit ir-json`
runs in about 39 ms and `--emit rust` in about 48 ms, and an IR package is 15 to
18 kB. Serializing 18 kB costs on the order of 100 to 200 µs, so five times that
moves a 39 ms command by well under a millisecond. This is a compiler writing a
few files once per build, not a serving path. The choice is also reversible
without disturbing consumers, because the JSON mechanism sits behind
`to_json_pretty` and `from_json`; any such change should follow a measurement
rather than this estimate.

## Consequences

- **Positive — the IR becomes consumable by a non-Rust backend.** This is the
  E4.5 obligation, and it was unmet by the artifact that carried its name.
- **Positive — one dialect in the tree.** Artifacts, baselines, and goldens all
  read the same way, so a golden and an emitted file differ only where the IR
  differs.
- **Positive — the descriptor pool is paid for once and serves both text
  encodings**, and it cannot drift from the generated types because both come
  from one schema compilation.
- **Negative — every IR golden is regenerated.** Two IR `.snap` files in
  `ridl-sem`, the `ridlc` `ir_package` golden, and the corpus snapshots. The
  diff is large and mechanical, which makes it a poor place to hide a semantic
  change; the round-trip and conformance tests are what guard it.
- **Negative — `ridl-ir` gains a dependency** (`prost-reflect`, with its `serde`
  and `text-format` features) and a `LazyLock` descriptor pool at run time.
- **Neutral — 64-bit fields render as strings.** Correct per the mapping and
  required by JavaScript consumers, but it is a visible change in every golden
  that carries a timing bound, a length bound, or an enum discriminant.

## Open

1. **Answered during E9.1 (2026-08-04): an unset proto3 `optional` field is
   omitted entirely, and `null` never appears in the output.**
   `skip_default_fields(false)` forces emission only of non-optional fields
   holding their default — `"isError": false`, `"doc": ""`, `"labels": []`,
   `"ordinal": 0`. Unset `optional` fields (`deprecated`, `lenMin`, `pattern`),
   unset message fields, and unset oneofs produce no key at all. Established by
   a probe against `prost-reflect` 0.16.5 over the real schema, not from its
   documentation. Decision 2 is scoped accordingly.
2. **Answered during E9.1 (2026-08-04): the `.boxed()` oneof member
   (`FieldType.kind.inline_scalar`) needs no special handling on the reflection
   path.** `transcode_from` goes through the wire encoding, so the Rust-side
   `Box` is never visible to reflection. A package holding an `inlineScalar`
   round-trips through typed, dynamic, JSON, dynamic, typed and compares equal,
   including under a strict parse.
3. **No cross-language conformance test.** It would be the strongest available
   evidence and it needs a non-Rust protobuf runtime in CI. That belongs to
   E4.5's "a third-party backend consumes the IR" criterion. Recorded as a known
   limit rather than left implicit.

## References

- [`docs/wip/2026-08-03-ir-protobuf-encodings-design.md`](../wip/2026-08-03-ir-protobuf-encodings-design.md)
  — the design note this record ratifies
- [ADR-0004](ADR-0004-implementation-sequencing-and-stack.md) §4 — the IR
  serialization decision whose rendering clause this supersedes
- [ADR-0013](ADR-0013-codegen-backend-scope.md) — a different subject that does
  not conflict: it governs what a wire backend emits _from_ the IR, while this
  record governs how the IR itself is encoded
- [`docs/ROADMAP.md`](../ROADMAP.md) — E9.1, E9.2, E9.3 (the stories this record
  binds) and E4.5 (the IR stability policy that cites the canonical-form policy
  of decision 9)
- `crates/ridl-ir/build.rs`, `crates/ridl-ir/src/lib.rs` — the descriptor set
  and the serialization surface
- `crates/ridlc/src/lib.rs` — the emit filter of decision 10
