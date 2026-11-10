# @driftsys/ridl-runtime

Loads the wasm codec module that `ridl build` generates and verifies encoded
payloads with it. A payload is checked against the structure and the typl rules
of its type before the caller uses it.

The package has no dependency. It uses the platform `WebAssembly`, streams and
text decoder APIs, so it runs in Deno, Bun, Node 22 or later, and in a browser.

## API

```ts
import { CatalogMismatch, CodecError, loadCodec } from "@driftsys/ridl-runtime";

const codec = await loadCodec(new URL("./cabin.wasm", import.meta.url), {
  catalogHash, // the 32 bytes that the generated TypeScript expects
  payloadTypes, // optional: payload type names to resolve at load
  ruleText, // optional: the text of rule code n is at index n - 2
});

const typeId = codec.typeId("cabin.Door"); // throws UnknownPayloadType
const result = codec.verify(typeId, bytes);
if (!result.ok) {
  // result.kind is "malformed", "rule" or "unknown-type".
  // result.code is the value that the module returned.
  // result.rule is the text of the failed rule, when ruleText was given.
}
```

- `loadCodec(source, options)` returns a `Promise<Codec>`. `source` is a `URL`,
  a `Response` or a promise of one, or a `BufferSource`. `options` has
  `catalogHash`, and optionally `payloadTypes`, `ruleText` and `imports`.
  `payloadTypes` names the payload types to resolve at load; the promise rejects
  with `UnknownPayloadType` for the first name that the module does not hold.
  `loadCodec` needs the export `ridl_type_id`, so a module built by ridl 0.12 or
  earlier must be rebuilt.
- `Codec.catalogHash` is the hash that the module reports.
- `Codec.typeId(name)` returns the `type_id` of the payload type with that name,
  through the export `ridl_type_id`. The module resolves each name once and the
  package caches the id. It throws `UnknownPayloadType` when the module has no
  such type.
- `Codec.verify(typeId, bytes)` returns `{ ok: true }` or a failure. The code
  `1` is `malformed`, a code of `2` or more is `rule`, and `-1` is
  `unknown-type`.
- `CatalogMismatch` is thrown at load when the hash of the module differs from
  `catalogHash`. Its fields `expected` and `actual` are lower-case hexadecimal.
- `UnknownPayloadType` is thrown by `typeId`, and rejects `loadCodec` for a name
  in `payloadTypes`. Its field `type` is the name.
- `CodecError` is thrown when the module cannot be fetched or compiled, or when
  an export is missing or has the wrong type. Its message and its `exportName`
  name the export. It is also thrown, by `loadCodec`, by `typeId` or by
  `verify`, when `ridl_alloc` returns 0 for a request of more than 0 bytes: the
  module is out of memory, and `exportName` is `ridl_alloc`. `typeId` also
  throws it when `ridl_type_id` returns a code below -1.

## Source of the module

A `Response` is compiled with `WebAssembly.instantiateStreaming` when the
platform has it and the content type is `application/wasm`. Otherwise the body
is read with `arrayBuffer()` and compiled with `WebAssembly.instantiate`. A
`file:` URL is read with `Deno.readFile` when `Deno` exists and is fetched
otherwise. Any other URL is fetched.

## Streams

`StreamSession` holds `{ transport, codec }`. Its `codec` is the loaded wasm
`Codec`; its `transport` implements the structural `StreamTransport` interface,
with `queryStream`, `commandStream`, `queryWithUpload` and `queryDuplex`
byte-stream methods. `StreamOptions` requires `bound`, the response bound in
milliseconds. The runtime has no dependency on a transport package.

A caller of a generated codec module constructs the context explicitly. This
example uses the generated document store module and the WebSocket transport:

```ts
import { loadCodec, type StreamSession } from "@driftsys/ridl-runtime";
import { connect } from "@driftsys/ridl-transport-ws";
import {
  CATALOG_HASH,
  CATALOG_NAME,
  DocStore,
  PAYLOAD_TYPES,
  RULES,
} from "./generated/docstore.api.codec.ts";

const codec = await loadCodec(
  new URL("./generated/docstore_api.wasm", import.meta.url),
  {
    catalogHash: CATALOG_HASH,
    payloadTypes: PAYLOAD_TYPES,
    ruleText: RULES,
  },
);
const transport = await connect("ws://localhost:8080", {
  catalog: { name: CATALOG_NAME, hash: CATALOG_HASH },
  encoding: "flatbuffers",
  interfaces: [DocStore],
});
const context: StreamSession = { transport, codec };
```

Pass `context` first and required `{ bound: milliseconds }` last to a generated
stream function. The [generated-code chapter](../../docs/book/generated-code.md)
shows the supported signatures and document store calls. The transport import
above belongs to the application; this runtime package does not import it.

Resolve named payload types with `codec.typeId` before making a request, and
load the codec with the expected catalog hash. An unknown name throws
`UnknownPayloadType`; a mismatched hash rejects loading with `CatalogMismatch`.
Supply the generated module's `RULES` to `loadCodec` as `ruleText` so that
verification failures include their rule text.

`decodeStream(decode, verify)` returns a `TransformStream` that checks each
whole payload before decoding it. For example, after resolving `typeId`, pipe a
byte stream through
`decodeStream(payload.decode, bytes => codec.verify(typeId, bytes))`. Each chunk
produces one decoded value. Native stream queues preserve backpressure; reader
cancellation and transform errors propagate upstream through `pipeThrough`.
Transport failures propagate unchanged.

`decodeTextStream()` returns a `TransformStream<Uint8Array, string>` that
decodes each chunk independently with fatal UTF-8 checking. A chunk must contain
complete code points. Empty chunks produce empty strings, and a leading BOM
remains content.

`encodeUpload(upload, encode, verify, limit)` returns a typed `WritableStream`.
Each named element becomes one encoded, verified chunk. Its encoded bytes are
never split. Use the generated payload encoder and verify with the resolved
payload type, for example
`encodeUpload(raw.upload, payload.encode, bytes => codec.verify(typeId, bytes), limit)`.

`byteUpload(upload, limit)` splits byte writes into chunks of at most `limit`
bytes. `textUpload(upload, limit)` encodes strings as UTF-8 and splits only
between complete code points. It refuses lone JavaScript surrogates instead of
replacing them. Both adapters preserve empty writes as empty chunks when `limit`
is positive; a zero or negative limit fails as corrupt before an empty chunk can
be written.

Each typed write waits until every raw write it produces has completed, in
order. Closing waits for pending writes before closing the raw upload. Local
failures use the error categories described below, error the typed stream and
abort the raw upload once. Aborting also reaches a raw write waiting for credit.

Remote failures preserve the original error object: pending and later writes,
`writer.closed`, and a close queued before the failure reject with that same
object. A close requested after the writer has errored rejects with the
platform's `TypeError`. An already resolved command acceptance stays resolved.

Local payload failures throw `StreamError`. Its `failure` is
`{ category: "corrupt" }` for malformed payloads, an oversized named element or
code point, or any exception a generated `encode` or `decode` throws without a
rule. Rule failures, including invalid UTF-8, a `RuleError` from a generated
`encode` or `decode`, and the `ConstraintError` with rule `utf8` that a
generated `encode` throws for a string with an unpaired surrogate, use
`{ category: "contract", contract: { category: "invalid-value", rule } }`. A
mapped exception is kept as the error's `cause`. `PayloadRule` names `length`,
`pattern`, `range`, `step`, `unique`, `utf8` and `variant`. A missing or unknown
verification rule text, or an unknown-type verification result, throws
`CodecError`. Local errors do not require a transport-specific error class.

The [payload rule ABI](../../docs/design/ts-codec-table.md#stream-functions)
defines the rule codes, their release classification and the codec regeneration
requirement.

## Constraint errors and helpers

A generated domain module builds a constrained named scalar with `make<Name>`,
which throws a `ConstraintError` when a utf8, range, step, length or pattern
rule breaks, and `make<Name>Unchecked`, which does not check. `ConstraintError`
extends `RangeError`. Its `rule` is `"range"`, `"step"`, `"length"`, `"pattern"`
or `"utf8"`, its `type` is the qualified type name (from a generated
`encode<Name>`, that name and the path of the field), and its message names the
value (a string is cut after 64 code points, and bytes show only their length)
and the constraint.

The generated code also calls five helpers that the package exports:
`checkWellFormed(value, type)` returns a well-formed string and throws a
`ConstraintError` with rule `"utf8"` and the given `type` otherwise; a generated
`encode<Name>` calls it for every string it writes, so that `TextEncoder` never
replaces an unpaired surrogate silently. `isWellFormedString(value)` reports
whether a string holds only Unicode scalar values, that is, no unpaired
surrogate (`TextEncoder` would replace one with U+FFFD without an error), and
`make<Name>` of a string scalar throws a `ConstraintError` with rule `"utf8"`
for one, before it checks the length or the pattern; `codePointLength(value)`
counts the code points of a string, which is the length a string constraint
measures; `offFloatStep(value, wire32, lattice)` and
`offFloatPoints(value, wire32, points)` report a float that is off its step
grid. See [Generated code](../../docs/book/generated-code.md) for what
`make<Name>` checks.

## Limits

- The module must export `memory`, `ridl_alloc`, `ridl_free`,
  `ridl_catalog_hash`, `ridl_type_id` and `ridl_verify`. The package does not
  read other exports.
- `verify` is synchronous and copies the payload into the module memory on each
  call. It frees the copy before it returns, also when the call throws.
- The package verifies named payloads with wasm; stream adapters use the encoder
  and decoder supplied by the caller for named stream elements. For raw byte
  splitting and complete text code points, see the upload adapters above.
- The package is not yet published.
