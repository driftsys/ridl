# @driftsys/ridl-runtime

Loads the wasm codec module that `ridl build` generates and verifies encoded
payloads with it. A payload is checked against the structure and the typl rules
of its type before the caller uses it.

The package has no dependency. It uses only the platform `WebAssembly`, so it
runs in Deno, Bun, Node 22 or later, and in a browser.

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
  name the export. It is also thrown, by `loadCodec` or by `verify`, when
  `ridl_alloc` returns 0 for a request of more than 0 bytes: the module is out
  of memory, and `exportName` is `ridl_alloc`.

## Source of the module

A `Response` is compiled with `WebAssembly.instantiateStreaming` when the
platform has it and the content type is `application/wasm`. Otherwise the body
is read with `arrayBuffer()` and compiled with `WebAssembly.instantiate`. A
`file:` URL is read with `Deno.readFile` when `Deno` exists and is fetched
otherwise. Any other URL is fetched.

## Limits

- The module must export `memory`, `ridl_alloc`, `ridl_free`,
  `ridl_catalog_hash`, `ridl_type_id` and `ridl_verify`. The package does not
  read other exports.
- `verify` is synchronous and copies the payload into the module memory on each
  call. It frees the copy before it returns, also when the call throws.
- The package does not decode a payload. It reports whether the payload is
  valid.
- The package is not yet published.
