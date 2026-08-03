# The Three IR Encodings — Canonical protobuf JSON, Prototext, Binary

| Field      | Value                                                                                                                            |
| ---------- | -------------------------------------------------------------------------------------------------------------------------------- |
| Status     | design, for review — nothing ratified                                                                                            |
| Date       | 2026-08-03                                                                                                                       |
| Origin     | the emitted `.ir.json` is a serde rendering of the Rust structs, not protobuf JSON, so no non-Rust protobuf runtime can parse it |
| Scope      | one crate rewrite (`ridl-ir` serialization surface); two new `--emit` values; one latent emit-filter defect; one ADR             |
| Supersedes | the rendering clause of ADR-0004 §4 — proposed as ADR-0014, not yet written                                                      |

A bare section reference — §4, §14 — is to the **ridl Language Reference**.
References to this document are always marked _above_ or _below_.

## 1. The problem

ADR-0004 §4 fixes the IR as a protobuf schema compiled with `prost`, and calls
the canonical stable form "the plugin-protocol wire format". It keeps a
`serde`→JSON rendering "for debugging and golden tests". Roadmap story E4.5
makes the obligation concrete: _a third-party backend consumes the IR_.

The JSON that ships today cannot serve that obligation. It is serde's rendering
of the generated Rust structs, which is a different encoding from the protobuf
JSON mapping. Compiled from `veh.common`:

```json
{
  "visibility": 1,
  "is_error": false,
  "deprecated": null,
  "kind": { "TypeDef": { "width": { "FloatWidth": 1 } } }
}
```

Canonical protobuf JSON for the same message differs on four counts:

| Aspect        | Emitted today              | Protobuf JSON mapping  |
| ------------- | -------------------------- | ---------------------- |
| enum values   | `1`                        | `"VISIBILITY_PUBLIC"`  |
| field names   | `is_error`                 | `isError`              |
| absent fields | `null`                     | omitted                |
| `oneof`       | wrapped in `{ "TypeDef" }` | flattened to `typeDef` |

`TypeDef` and `FloatWidth` are Rust variant names. They do not exist in the
protobuf JSON mapping at all. A Kotlin or TypeScript backend pointed at this
file with a conformant protobuf JSON parser fails to parse it.

The binary encoding has no such defect — it is schema-faithful by construction —
but no CLI path writes it. So the one interchange form that works is
unreachable, and the one that is reachable does not work.

## 2. What the toolchain can and cannot do

Established against the published APIs rather than assumed:

- **`prost` 0.14 has no JSON and no text format.** `prost::Message` exposes
  `encode`, `decode`, `merge`, `clear`, `encoded_len` — binary only. The crate
  documents that it "does not include support for runtime reflection or message
  descriptors", which is the reason: the JSON mapping needs field names, enum
  value names, and well-known-type handling, none of which survive into the
  generated structs. Both text encodings must come from a companion crate.
- **`prost-reflect` 0.16.5** depends on `prost ^0.14`, matching the workspace
  pin. Its `serde` feature gives canonical JSON via `serialize_with_options` /
  `deserialize_with_options`; its `text-format` feature gives prototext via
  `to_text_format_with_options` / `parse_text_format`. `transcode_from` and
  `transcode_to` convert between the typed message and `DynamicMessage`.
- **`pbjson-build` 0.9.0** also depends on `prost ^0.14` and generates canonical
  JSON serde impls at build time, with no runtime descriptor pool. It is JSON
  only. Rejected — see §8.
- **The descriptor set already exists.** `protox::compile` returns a
  `FileDescriptorSet` in `crates/ridl-ir/build.rs`; it is dropped after codegen
  today.

## 3. Decisions

1. **Canonical protobuf JSON replaces the serde rendering on every surface** —
   the `--emit ir-json` artifact, `.ridl/baseline/*.ir.json`, and the insta
   goldens. One dialect in the tree.
2. **Every field is emitted**, defaults included — `skip_default_fields(false)`,
   in both JSON and prototext. Still conformant on the read side, because a
   conformant parser must accept explicitly present defaults. Keeps goldens
   explicit: a reviewer reads `ordinal: 0` rather than inferring it from
   absence, which matters where ordinals are semantically load-bearing (§11).
3. **Goldens stay JSON.** Prototext is not used for insta snapshots.
4. **All three protobuf encodings are emittable** — binary, canonical JSON,
   prototext.
5. **`ridl diff` and `ridl check --baseline` stay JSON-only.** Baselines remain
   `.ir.json`. A committed baseline must review in a pull request, which binary
   does not.
6. **The serde derives come off the generated types.** The `type_attribute` in
   `build.rs` is removed. serde and serde_json remain underneath `prost-reflect`
   as the JSON writer, but no longer determine the shape.

## 4. The serialization surface

Six functions in `ridl_ir::v2` where there is one today. `to_json_pretty` keeps
its name, so no call site is renamed. Two mechanisms, because binary needs no
descriptors:

| Function                              | Mechanism                      |
| ------------------------------------- | ------------------------------ |
| `to_json_pretty` / `from_json`        | `prost-reflect`, `serde`       |
| `to_text_format` / `from_text_format` | `prost-reflect`, `text-format` |
| `to_binary` / `from_binary`           | `prost`, no descriptors        |

`to_json_pretty` keeps its current signature, including its infallible return.
The new failure modes are the same class as the existing `expect` — they cannot
occur unless the schema and the generated types disagree.

`from_text_format` is required rather than speculative: without it the prototext
emit has no round-trip test, and a write path with no read path is untested by
construction.

**Descriptor pool.** `build.rs` writes the `FileDescriptorSet` it already holds
to `OUT_DIR`; `lib.rs` holds a `LazyLock<DescriptorPool>` over an
`include_bytes!` of that file. No new build step, no system `protoc`, no
vendored blob in the tree — the pool is derived from the same schema compilation
that generates the types, so the two cannot disagree.

**Serializer options**, all confirmed against the 0.16.5 API:

| Format | Option                                | Value             |
| ------ | ------------------------------------- | ----------------- |
| JSON   | `skip_default_fields`                 | `false`           |
| JSON   | `use_enum_numbers`                    | `false` (default) |
| JSON   | `stringify_64_bit_integers`           | `true` (default)  |
| Text   | `pretty`                              | `true`            |
| Text   | `skip_default_fields`                 | `false`           |
| Text   | `print_message_fields_in_index_order` | `true`            |

`stringify_64_bit_integers` has a visible consequence worth recording. The
schema carries real 64-bit fields — `len_min` and `len_max`, the enum
discriminant `value`, and the timing `min` and `max`. Canonical JSON renders
these as strings, so a timing bound emits as `"10000"` rather than `10000`. The
canonical behaviour is kept rather than overridden: JavaScript loses integer
precision above 2^53, and the TypeScript backend already models timing as
`bigint`. Overriding it would break the consumers this change exists to serve.

`print_message_fields_in_index_order` is set for prototext so output ordering is
deterministic rather than incidental.

## 5. The emit surface

Two new values on `ridlc::Emit`, and the existing one keeps its name:

| Flag value  | Artifact          | Encoding                |
| ----------- | ----------------- | ----------------------- |
| `ir-json`   | `<base>.ir.json`  | canonical protobuf JSON |
| `ir-text`   | `<base>.ir.txtpb` | prototext               |
| `ir-binary` | `<base>.ir.binpb` | protobuf binary         |

`ir-json` is kept rather than renamed to `ir-pbjson`. The `.ir.json` suffix is
load-bearing — it drives snapshot detection in `crates/ridl/src/main.rs` and is
cited across ADRs, docs and tests. Flag values stay plain English while
extensions follow the `buf` convention, which is the existing repo precedent:
`c-header` writes `.h`, `typescript` writes `.ts`.

### 5.1 A latent defect the new emits expose

`crates/ridlc/src/lib.rs` filters `ridl.std` out of one emit kind:

```rust
.filter(|emit| !matches!(emit, Emit::IrJson))
```

`ridl.std` is written for every emit _except_ `ir-json`, because a direct IR
dump is not code and `ridl diff` compiles the other side without `ridl.std`. The
comment at that site records the reasoning.

Binary and prototext are direct IR dumps by the identical argument, so both must
join that filter, and the `is_code` check in the same file must widen with it.
Missing this produces a spurious `ridl.std.ir.binpb` on every build — invisible
until someone compares an artifact directory. The filter must become a test over
"is this an IR dump", not an enumeration of one variant, so the next IR encoding
cannot reintroduce the defect.

## 6. Call sites

Every site that touches the rendering, and what happens to it:

| Site                                       | Today                     | After                            |
| ------------------------------------------ | ------------------------- | -------------------------------- |
| `ridlc/src/lib.rs` (emit)                  | `to_json_pretty`          | unchanged call, canonical output |
| `ridl-diff/src/lib.rs`                     | `serde_json::from_str`    | `v2::from_json`                  |
| `ridl-sem/src/check.rs` (two goldens)      | `to_json_pretty`          | unchanged call                   |
| `ridlc/tests/golden.rs`, `corpus.rs`       | `to_json_pretty`          | unchanged call                   |
| `ridl-backend-rust/src/tests.rs` (loader)  | serde parse of `.snap`    | `v2::from_json`                  |
| `ridl-sem/src/check.rs` (substring assert) | `json.contains("\"ok\"")` | unchanged                        |

The last row is worth recording because it looks fragile and is not. That test
asserts the rendered text contains `"ok": "CalReport"`. The proto fields are
`ok` and `err`; both are single words, so lowerCamelCase leaves them identical
and the assertion survives the dialect change untouched.

Golden regeneration covers two IR `.snap` files in `ridl-sem`, plus the ridlc
`ir_package` golden and the corpus snapshots.

## 7. Testing

- The existing binary and JSON round-trip tests stay, re-pointed at the new
  pair, and gain a prototext sibling.
- **A strict-parse conformance test.** Re-read the emitted JSON with
  `DeserializeOptions` configured to reject unknown fields. This tests the claim
  the change actually makes — that a conformant parser accepts this output —
  rather than asserting on output text, which would only restate the
  serializer's behaviour back to itself.
- **No cross-language conformance test.** It would be the strongest available
  evidence, and it needs a non-Rust protobuf runtime in CI. That belongs to
  E4.5's "a third-party backend consumes the IR" criterion, not here. Recorded
  as a known limit rather than left implicit.

## 8. Alternatives considered

**`pbjson-build` instead of `prost-reflect`.** Generates canonical JSON serde
impls at build time, needs no runtime descriptor pool, and leaves every call
site untouched — `to_json_pretty` would stay `serde_json::to_string_pretty`. It
also makes the wrong dialect a compile error, since its generated impls and a
`type_attribute` serde derive would conflict. It was the recommendation until
prototext entered scope. It is JSON only, and prototext requires a runtime
descriptor pool, so pbjson's single advantage disappears and it becomes a second
mechanism doing a subset of what `prost-reflect` already does.

The cost of that choice is real and small. `prost-reflect` renders JSON by
transcoding the typed message — a protobuf encode, then a decode into a
descriptor-keyed `DynamicMessage` — and then walking that tree with a descriptor
lookup per field, where pbjson emits straight-line field writes. Estimated at
3–5× on the serialization step. **That figure is reasoned from what `transcode`
does, not measured**, because `prost-reflect` is not yet a dependency.

Measured baselines put it in proportion. On a two-package workspace with a debug
binary, `--emit ir-json` runs in about 39 ms and `--emit rust` in about 48 ms;
an IR package is 15–18 kB. The JSON path is already faster than the Rust codegen
path, so serialization is not the bottleneck even inside its own command — most
of that time is process startup, parsing, checking and lowering. Serializing 18
kB costs on the order of 100–200 µs; five times that moves a 39 ms command by
well under a millisecond, and a release build shrinks both sides.

The scale argument matters more than the ratio: this is a compiler writing a
handful of files once per build, not a serving path. There is no N to multiply
by — a workspace holds tens of packages, and it would take roughly a thousand
before this reached the noise floor of process startup.

The decision is also reversible without disturbing consumers. The JSON mechanism
sits behind `to_json_pretty` and `from_json` (§4), so substituting pbjson later
is contained to `ridl-ir` and moves no call site. The descriptor pool is paid
for regardless, because prototext needs it. Any such change should follow a
measurement, not this estimate.

**Prototext for goldens.** Prototext reads better than JSON — snake_case field
names as written in the schema, bare enum names, and no 64-bit stringification,
so timing renders as `min: 10000` rather than `"10000"`. Rejected to keep one
dialect across goldens and artifacts; a golden in a format no shipped artifact
uses tests the renderer rather than the artifact.

**Prototext as a consumer-facing interchange format.** Rejected as a
_recommended_ form, though it is emittable. ADR-0004 targets Rust, Kotlin and
TypeScript backends. C++, Java, Python and Go have solid text-format parsers;
TypeScript has essentially none — neither protobuf.js nor ts-proto implements
it. Prototext is for inspection and debugging; JSON is the interchange format.

**Additive only — keep serde JSON, add new emits alongside.** Zero churn and
zero migration, rejected because it leaves the misleading artifact shipped under
the name every consumer reaches for first, and raises the dialect count instead
of lowering it.

**Compatibility shim for existing baselines.** Not needed. Version is `0.0.0`
with no tags; nothing is published, so no baseline exists outside this repo and
the format change is free.

## 9. Records

Proposed as **ADR-0014**, superseding the rendering clause of ADR-0004 §4. A new
record rather than an in-place amendment, following the ADR-0011-supersedes-
ADR-0008 precedent, because it also fixes the canonical-form policy that E4.5
will cite: binary is canonical, JSON is derived and conformance-obliged,
prototext is for inspection.

ADR-0013 (codegen backend scope) is a different subject and does not conflict:
it governs what a wire backend emits **from** the IR, while this design governs
how the IR itself is encoded. The two meet only in vocabulary — ADR-0013's
proto3 target is a backend output, not the IR's own wire format.

ADR-0004 §4's own text stands except for the clause scoping the JSON rendering
to "debugging and golden tests" — after this change the JSON carries an
interchange obligation and a conformance test.

## 10. To establish before implementation

Two behaviours shape golden output and are not worth guessing:

1. How `skip_default_fields(false)` treats proto3 `optional` fields that are
   unset — omitted, or emitted as `null`. The schema uses `optional` for
   `deprecated`, `len_min`, `pattern` and others, so this decides real golden
   shape.
2. Whether the `.boxed()` oneof member configured in `build.rs`
   (`FieldType.kind.inline_scalar`) needs special handling on the reflection
   path.

Both are answered by writing one test against the library before the rewrite,
not by reading its documentation.
