# The generated TypeScript codec table — design

Status: as built, 2026-10-22 (branch `feat/ts-codec-table`); D-4, D-14 and the
fixtures record what the implementation settled. Written 2026-10-22 as step 5 of
the plan change in
[the TypeScript wasm codec design](../wip/2026-10-21-ts-wasm-codec-design.md).
Decisions marked **(ruling)** were taken by the author; Sebastien accepted the
six recommendations at the end on 2026-10-22, and they are recorded there as
rulings. The implementation plan is
[2026-10-22-ts-codec-table-plan.md](2026-10-22-ts-codec-table-plan.md).

## Problem

The cabin demos (`examples/cabin/web/main.ts` and `page.ts`) read the
`Cabin.temperature` signal over the WebSocket transport, verify each payload
through the cabin wasm codec, and decode it with code that `flatc --ts`
generated from the `.fbs` that `ridl build --emit flatbuffers` wrote. Three
facts that the demos need are written by hand in `examples/cabin/web/check.ts`:

- `TEMPERATURE = { iface: 1, ord: 1 }`, the address of the interaction;
- `TEMPERATURE_TYPE_ID = 5`, the index of `veh.cabin.Temperature` in the
  `WASM_PAYLOAD_TYPES` list of the generated `__ridl_wasm.rs`, which
  `ridl_verify` keys on;
- `encodeTemperature` and `decodeTemperature`, written over the `flatc` class
  `TemperatureBox`.

A user's package would need the same three facts for every interaction, and the
spike of 2026-10-22 found that the `flatc` object API does not give the types
`ridl-backend-ts` emits: it returns class instances, gives an enum as a `bigint`
field and a string-valued TypeScript enum, gives a union as
`{ valueType, value }` with boxed scalar arms, gives a map as an array of
`{ key, value }` entries, loses tuple field names (`field_1`), uses `null` for
an absent value, and wraps a scalar payload in an `XBox` table. So the public
API that the owner fixed — plain objects in, plain objects out,
`encodeX(obj): Uint8Array` and `decodeX(bytes): X` over the types
`ridl-backend-ts` already emits — has to be generated from the IR, which knows
the tuple names and the union arms the `.fbs` loses.

This note designs that generated file, `<base>.codec.ts`, the manifest option
that asks for it, how the TypeScript side gets the wasm type ids without drift,
and what changes in the cabin demos.

## What exists

- `ridl-backend-ts` writes one `<base>.ts` per package: branded scalars,
  interfaces for structs, enums, enum sets, discriminated unions, constants and
  `init` functions. Its backend contract
  (`crates/ridl-backend-ts/src/contract.rs`) refuses every option, and reads doc
  comments from `request.model`; everything else still comes from the raw IR
  (`docs/design/codegen-plugins.md`).
- The request's model (`ridl.codegen.v1.Model`) already carries what the table
  needs: `interfaces` with `number` and the `slots` of each interaction,
  `tuples` with `InducedName`, `flatbuffers.roots` with the bound of each root,
  `catalog` with the unit's name and 32-byte hash, and the `Spellings` of every
  name. `ridlc` builds the model with
  `codegen::lower_with(package, others, compatible)` for every backend, and
  `wasm_module_source` lowers with the same call.
- The generated Rust crate has a `wasm` module when `[backend.rust] wasm = true`
  (`crates/ridl-backend-rust/src/wasm.rs`): `WASM_PAYLOAD_TYPES`, the qualified
  names of every FlatBuffers root with a finite bound over **every package of
  the crate and `ridl.std`**, sorted byte by byte; `WASM_RULES`; and the exports
  `ridl_alloc`, `ridl_free`, `ridl_catalog_hash` and
  `ridl_verify(type_id, ptr, len)`. A `type_id` is an index into
  `WASM_PAYLOAD_TYPES`. An induced tuple is listed by its Rust spelling
  (`InducedName.rust`).
- `ridl-backend-flatbuffers` writes one `.fbs` per package: a `table` per
  struct, an `enum : long` per typl enum, a `union` plus a wrapper
  `table <Name> { value: <Name>Union }` per typl union with `<Union><Arm>Box`
  tables for scalar arms, a `<Name>Box` table per named scalar, enum and enum
  set (the root form; ADR-0019 decision 8), a positional `field_1, field_2, …`
  table per tuple, an `<path>Entry` table per map, `= null` on a field whose
  enum has no zero member, and a `<Interface>Ordinal` enum per interface.
- `@driftsys/ridl-runtime` (`packages/ridl-runtime/`) holds `loadCodec`, which
  checks the catalog hash at load and returns
  `{ catalogHash, verify(typeId, bytes) }`.
- The options of a `[backend.<name>]` table reach that backend only, as
  `BackendOption { key, value }` pairs: `encodings`, and `wasm = "true"`
  (`backend_options` in `crates/ridlc/src/lib.rs`). The manifest parser accepts
  `wasm` on any backend table; the backend that does not take it refuses it.

## Decisions

**D-1. `ridl-backend-ts` writes `<base>.codec.ts` beside `<base>.ts` from the
request's model (ruling).** The option `[backend.typescript] codec = true` turns
it on. No change to the codegen contract and no new lowering: the model that
`ridlc` already hands the backend holds the interfaces, the induced tuples, the
FlatBuffers roots and the catalog. The backend reads `model.interfaces`,
`model.tuples`, `model.flatbuffers` and `model.catalog` for this file, which is
how interface declarations reach it. The module `<base>.ts` is unchanged.

**D-2. The public API is plain objects (owner's rule).** For every payload type
of the package — every FlatBuffers root of the package that has a finite bound,
which is exactly the set with a `Payload<FlatBuffers>` impl and an entry in
`WASM_PAYLOAD_TYPES` — the file exports `encode<Name>(value: Name): Uint8Array`
and `decode<Name>(bytes: Uint8Array): Name`, where `Name` is the TypeScript type
of `<base>.ts`. The `flatc` classes, builders and `XBox` tables never appear in
a signature. An `internal` root is the one exception (controller ruling during
execution, 2026-10-22): `<base>.ts` does not export its type, so the module
writes no `encode`, `decode` or `codec<Name>` for it, but still lists its
qualified name in `PAYLOAD_TYPES`, so that the list equals the package's part of
`WASM_PAYLOAD_TYPES`; an interaction whose payload type is internal is refused
with
`` `<Interface>.<member>` has the payload type `<qualified>`, which is internal; `<base>.ts` does not export it, so the codec module cannot name it ``.

**D-3. The wrappers go through the `flatc` object API, with one private
converter pair per declaration (ruling).** `decode<Name>` reads the root with
`getRootAs<Table>`, calls `unpack()`, and converts the `<Table>T` tree to the
plain object; `encode<Name>` converts the plain object to a `<Table>T` tree and
calls `pack(builder)`. The converters are module-private functions `pack<Name>`
and `unpack<Name>`, emitted for every declaration a payload type reaches, so a
nested struct, enum or union is converted once and reused. The object API needs
`flatc --gen-object-api`, which the cabin recipe already passes. The accessor
API would save one allocation per decode, but it needs a generated reader per
field and a generated builder per table; the object API makes the generator a
tree walk over the model, which is the smaller program.

**D-4. The conversion rules are fixed per typl kind (ruling).** The spike listed
every difference; the table below is the rule for each one.

| typl kind                       | `flatc` object API form                                          | Plain object form (`<base>.ts`)                 | Conversion                                                                                                                                                                                                                                                               |
| ------------------------------- | ---------------------------------------------------------------- | ----------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| named scalar as a payload       | `<Name>Box` table, `value`                                       | the branded `number`, `bigint`, `string`, …     | unbox or box; a `bigint` width stays `bigint`                                                                                                                                                                                                                            |
| named scalar as a field         | the backing scalar                                               | the branded scalar                              | a type assertion, no runtime work                                                                                                                                                                                                                                        |
| enum                            | `bigint` field; string-valued `enum` (`.fbs` width is `long`)    | numeric `export enum`                           | `BigInt(value)` to pack, `Number(value)` to unpack; as a payload, through `<Name>Box`                                                                                                                                                                                    |
| enum set                        | integer field (`bigint` for a 64-bit width)                      | branded `number` or `bigint`                    | a type assertion; as a payload, through `<Name>Box`                                                                                                                                                                                                                      |
| struct                          | `<Name>T` class, fields in `.fbs` order                          | `interface` with named fields                   | field by field; the `fb` member is the lower camel case of `snake_case(declared)`, the name `flatc` gives the accessor (not `Spellings.camel`: `flatc` drops each `_` and upper-cases the character after it, so an acronym in the declared name is spelled differently) |
| optional field                  | `null` when absent (`= null` default, or a missing table/vector) | the field is absent (`name?:`)                  | `null` becomes an absent property; an absent property becomes `null`                                                                                                                                                                                                     |
| union                           | wrapper `<Name>T { valueType, value }`; scalar arms boxed        | `{ kind: '<arm>'; value: T }`                   | `kind` is the arm's declared name; the `flatc` member is its `Spellings.snake`; a boxed arm is unboxed through `value`                                                                                                                                                   |
| tuple                           | positional `field_1, field_2, …`                                 | an inline object with the declared field names  | position to name, from the tuple's `fields` in the model                                                                                                                                                                                                                 |
| array                           | `T[]` (a vector)                                                 | `readonly T[]`                                  | element by element through the element's rule                                                                                                                                                                                                                            |
| map                             | `<path>EntryT[]` with `key` and `value`                          | `ReadonlyArray<readonly [K, V]>`                | entry to pair and pair to entry, in order                                                                                                                                                                                                                                |
| string, bytes, boolean, float   | `string`, `Uint8Array`, `boolean`, `number`                      | the same                                        | none                                                                                                                                                                                                                                                                     |
| integer field of U64 or I64     | `bigint`                                                         | `bigint`                                        | none                                                                                                                                                                                                                                                                     |
| induced tuple of an interaction | positional table named `InducedName.wire`                        | `export interface <InducedName.rust>` (see D-9) | as a tuple                                                                                                                                                                                                                                                               |

The `.fbs` enum width stays `long` for now (ruling, Sebastien, 2026-10-22);
narrowing it is driftsys/ridl#1020, and the wrapper absorbs the change when it
lands.

**D-5. A wasm type id is resolved by name at load, not written as a number into
the TypeScript (ruling).** `WASM_PAYLOAD_TYPES` is sorted over every package of
the crate and `ridl.std` (`wasm_module_source` in `ridlc`), while the TypeScript
backend generates one package at a time from a model that holds only that
package's roots. A per-package file cannot compute the crate-wide index; a rule
that worked for a one-package workspace would give a wrong id for a two-package
one, and `ridl_verify` would then check the bytes against the wrong type and
report a malformed buffer or, worse, accept them. So the table carries the
**qualified name** of each payload type, and the module gains one export:

| Export                                    | Behaviour                                                                                               |
| ----------------------------------------- | ------------------------------------------------------------------------------------------------------- |
| `ridl_type_id(name_ptr, name_len) -> i32` | the index in `WASM_PAYLOAD_TYPES` of the UTF-8 name at `name_ptr`, or -1 when the list does not hold it |

The export is additive: the four existing exports and their codes do not change.
A native `type_id_of(name: &str) -> i32` under `#[doc(hidden)]` is compiled for
every target, as `verify_by_id` is, so a native test checks the same lookup. The
module is the one source of the ids, and a name the module does not know fails
at load (D-7), never silently.

**D-6. The qualified name is a shared rule, pinned by a test (ruling).** The
TypeScript backend spells a payload type as the wasm module does:
`<Scope.package>.<declared name>` for a declaration, and
`<Scope.package>.<InducedName.rust>` for an induced tuple. The rule is small and
has two implementations; the pin test of "Tests" below builds the cabin example
with both backends and compares the two generated lists. A name that drifts is
caught twice: by that test in CI, and at load by D-5.

**D-7. `@driftsys/ridl-runtime` resolves and caches the ids (ruling).**
`loadCodec` takes an optional `payloadTypes: readonly string[]`; when given, it
resolves every name through `ridl_type_id` at load and throws
`UnknownPayloadType` naming the first name the module does not know, so a table
and a module that do not match fail before the first payload. `Codec` gains
`typeId(name: string): number`, which returns the cached id or resolves a name
not given at load on first use, and throws `UnknownPayloadType` for a name the
module does not know. `verify(typeId, bytes)` is unchanged. The name goes into a
buffer from `ridl_alloc`, as the bytes of `verify` do.

**D-8. The member table is one `export const <Interface>` per interface
(ruling).** Each holds `iface` (the interface number the model carries, the same
value the Rust descriptor is generated from) and one entry per live interaction,
keyed by the interaction's declared name, with `iface`, `ord`, `kind` and the
payload codecs of its roles: `payload` for a signal, an event and a `fixed`;
`request` for a command; `request` and `reply` for a query. A tombstone has no
entry. A `kind` is one of the strings `"signal"`, `"event"`, `"command"`,
`"query"`, `"fixed"`; the runtime package does not depend on the transport
package, so it does not reuse the transport's `Kind` enum. The types
`PayloadCodec<T>` and `MemberRef` live in `@driftsys/ridl-runtime` and the
generated entries satisfy them structurally. The interface's declared name is
free as a value name in the codec module, because the module imports `<base>.ts`
under a namespace and a package cannot declare a struct and an interface of one
name.

**D-9. An interaction's induced tuple gets an `export interface` in the codec
file (ruling).** A call with a parameter list that is not exactly one named
type, and a query whose reply is a tuple, name an induced tuple that is a
FlatBuffers root and a payload type; `<base>.ts` emits no type for it today. The
codec file exports `interface <InducedName.rust>` with the tuple's named fields,
so `encode<InducedName.rust>` has a plain-object parameter type. Cabin has no
such call (every call takes one named parameter), so the codec fixture's
`setLimit` and `span` are the proof of it (see "Tests"). A query with a fallible
`T | E` reply has no reply payload type, as in the Rust face, and its entry has
no `reply`.

**D-10. The file imports `flatc`'s output at a fixed relative path, and the user
runs `flatc` (ruling).** `flatc --ts -o <dir> <package>.fbs` writes
`<dir>/<segments>/…`: for `veh.cabin`, the barrel `<dir>/veh/cabin.ts` and the
per-type files under `<dir>/veh/cabin/`. The codec file imports
`./<segments joined by '/'>.js` — `./veh/cabin.js` — so the convention is: run
`flatc` with `-o` set to the directory of the TypeScript emit. The file's header
comment states that convention and the `flatc` version it was written against is
not pinned by the file: the repository pins `.flatc-version` for its own demos,
and a user's package pins its own. `ridlc` does not run `flatc`, for the reason
D-6 of the earlier note gives for `cargo`: a tool that spawns another in a
user's environment is a larger decision. The file also imports the `flatbuffers`
npm package (`import * as flatbuffers from "flatbuffers"`), which `flatc`'s own
output already requires.

**D-11. `decode<Name>` does not verify, and `encode<Name>` does not check a
constraint (ruling).** Checks run in the generated Rust (ADR-0020 decisions 2
and 7); a `decode<Name>` is for bytes the codec has verified, and may throw a
reader error on malformed bytes. `encode<Name>` builds what it is given, so that
a payload the codec rejects can be built, as the demos' self-test needs.
ADR-0013 decision 8's checking constructor for TypeScript stays
driftsys/ridl#923.

**D-12. The option is a boolean key on a backend table, and `codec = true`
implies the wasm codec (ruling, Sebastien, 2026-10-22).** The manifest parser
accepts `codec` as it accepts `wasm` (a non-boolean is MANI-015 with the same
wording). `ridlc` passes it to the TypeScript backend as
`BackendOption { key: "codec", value: "true" }`; the TypeScript backend accepts
`codec` and still refuses every other key; a backend that does not take `codec`
refuses it as the TypeScript backend refuses `wasm` today. The table's hash and
names are meaningful only against the wasm module, so a `codec = true` table
turns the module on, under four rules:

1. `codec = true` and `[backend.rust]` has no `wasm` key: `wasm` is turned on
   implicitly; `ridlc` passes `wasm` to the Rust backend as if the manifest had
   written `wasm = true`, with the value `implied`, so a later message can say
   where it came from.
2. `codec = true` and `[backend.rust]` writes `wasm = false`: MANI-019, an error
   on the `codec` value, whose message names both keys.
3. `codec = true` and no `[backend.rust]` table: MANI-019 with its own message,
   because there is no Rust crate for the module to come from.
4. An implied `wasm` whose `[backend.rust] encodings` does not name
   `flatbuffers`: the existing refusal of `wasm` without `flatbuffers`, worded
   to say that the module was implied by `codec = true`.

The reverse is not required: a crate with the wasm module and no TypeScript
consumer is a valid build (the Node host test is one). The manifest's `wasm` key
keeps its meaning, and `BackendTable.wasm` becomes `Option<bool>` so that an
absent key and a written `false` are told apart.

**D-13. The codec file carries the generated marker and header, and a
one-sentence doc per exported item (ruling).** The preamble is the one
`<base>.ts` gets (`codegen::comment_preamble`), followed by
`/* eslint-disable */`. Each `encode<Name>` and `decode<Name>` carries one
sentence naming the payload type as a code span; the type's rules stay on the
type in `<base>.ts`, where ADR-0028 decision 6 puts them, and are not repeated.
The table constants carry one sentence each.

**D-14. A foreign struct, union or enum is refused; a foreign named scalar or
enum set is converted, and one used as a payload root goes through its own
package's codec module (ruling, controller, 2026-10-22; amends Q-2).** A payload
type of the package that reaches a struct, a union or an enum of another package
needs that declaration's converter and `flatc`'s output for that package, and
the module does not write either yet: it refuses with
``<site> `<package>.<Name>`, a <struct|union|enum> of another package, which the codec module does not convert yet``,
where `<site>` names the payload type and the field, arm or element that reaches
it, or the interaction whose payload type it is. The path when a package needs
it is self-contained converters: `model.foreign` carries the full declaration,
so the converters can be emitted into the reaching package's file, with an
import of `./<foreign segments>.js` for `flatc`'s output, and no exported
converter API between codec files.

A foreign named scalar or enum set at a field, arm or element position needs
neither: the `.fbs` inlines both there, and the conversion is a type assertion
that reads the width from `model.foreign`; the module adds
`import type * as <alias> from "./<package>.js";` for each such package.

A foreign named scalar or enum set used as a payload root — `ridl.std.Name` as
the request of `query profile(name: Name)` — is different: its box table
`<Name>Box` lives only in the foreign package's `.fbs`, so the wrappers for it
exist only in that package's codec module. The table entry therefore uses that
module: the file imports
`import * as <alias>_codec from "./<package>.codec.js";` and declares
`const codec<PascalAlias><Name>: PayloadCodec<<alias>.<Name>> = { type: "<package>.<Name>", encode: <alias>_codec.encode<Name>, decode: <alias>_codec.decode<Name> };`
(`codecRidlStdName` for `ridl.std.Name`). A build writes every package of the
workspace, `ridl.std` included, so the foreign codec module exists beside this
one, and the user runs `flatc` on that package's `.fbs` as well. A foreign root
with no finite bound gets no entry, as a local one does.

`ridl.std` declares named scalars and regex constants only, so a package that
uses `ridl.std.Label` or `ridl.std.Timestamp` in a payload is not refused. The
veh-cluster corpus is refused as a whole, because its `cluster` package takes
`veh.common.GearPosition`, an enum, as a command's payload, so the fixture for
the converters is the corpus's `common` package alone (see "Tests").

**D-15. Every import of the codec file carries the `.js` suffix (ruling, see
Q-3).** `flatc`'s barrels use `.js` (`export { X } from './cabin/x.js'`), and
the Node ESM resolver and `tsc` under `moduleResolution: node16` need it; Deno
resolves it through `sloppy-imports`, which the cabin demo already turns on for
`flatc`'s output. `<base>.ts` imports its siblings without a suffix today; the
codec file is new and has no compatibility to keep, so it uses the form that
works outside a bundler.

## What is generated

One file per package, `<base>.codec.ts`, beside `<base>.ts`, when
`[backend.typescript] codec = true`. Its exports, with their TypeScript
signatures:

```ts
export const CATALOG_NAME: string; // the unit's name, `Catalog.package`
export const CATALOG_HASH: Uint8Array; // 32 bytes, `Catalog.hash`
export const PAYLOAD_TYPES: readonly string[]; // this package's, sorted
export const RULES: readonly string[]; // `WASM_RULES`, text of rule code n at n - 2

export function encode<Name>(value: Name): Uint8Array;
export function decode<Name>(bytes: Uint8Array): Name;
export interface <InducedName.rust> { /* the tuple's named fields */ }

export const <Interface>: {
  readonly iface: number;
  readonly <interaction>: MemberRef & { payload: PayloadCodec<T> }; // signal, event, fixed
  readonly <call>: MemberRef & { request: PayloadCodec<A> }; // command
  readonly <query>: MemberRef & { request: PayloadCodec<A>; reply: PayloadCodec<R> };
};
```

In `@driftsys/ridl-runtime`:

```ts
export interface PayloadCodec<T> {
  readonly type: string; // the qualified name, the key of `Codec.typeId`
  encode(value: T): Uint8Array;
  decode(bytes: Uint8Array): T;
}
export interface MemberRef {
  readonly iface: number;
  readonly ord: number;
  readonly kind: "signal" | "event" | "command" | "query" | "fixed";
}
export interface LoadOptions {
  catalogHash: Uint8Array;
  payloadTypes?: readonly string[]; // resolved at load; a miss throws
  ruleText?: readonly string[];
  imports?: CodecImports;
}
export interface Codec {
  readonly catalogHash: Uint8Array;
  typeId(name: string): number; // throws UnknownPayloadType
  verify(typeId: number, bytes: Uint8Array): VerifyResult;
}
export class UnknownPayloadType extends Error {
  readonly type: string;
}
```

The generated table for cabin, abridged (the hash bytes, the docs and the
converters of `Warning` and `Diagnostic` are left out):

```ts
// @generated by ridl from package veh.cabin. Do not edit.
/* eslint-disable */
import * as flatbuffers from "flatbuffers";
import type { PayloadCodec } from "@driftsys/ridl-runtime";
import type * as t from "./veh.cabin.js";
import * as fb from "./veh/cabin.js";

export const CATALOG_NAME = "veh.cabin";
export const CATALOG_HASH: Uint8Array = Uint8Array.from([/* 32 bytes */]);
export const PAYLOAD_TYPES = [
  "veh.cabin.Average", "veh.cabin.Diagnostic", "veh.cabin.Health",
  "veh.cabin.Level", "veh.cabin.Subsystems", "veh.cabin.Temperature",
  "veh.cabin.Warning", "veh.cabin.Window",
] as const;
export const RULES = ["length", "pattern", "range", "step", "unique", "variant"] as const;

/** Encodes a `veh.cabin.Temperature` payload. */
export function encodeTemperature(value: t.Temperature): Uint8Array {
  const builder = new flatbuffers.Builder();
  builder.finish(new fb.TemperatureBoxT(value).pack(builder));
  return builder.asUint8Array();
}
/** Decodes a `veh.cabin.Temperature` payload the codec has verified. */
export function decodeTemperature(bytes: Uint8Array): t.Temperature {
  return fb.TemperatureBox
    .getRootAsTemperatureBox(new flatbuffers.ByteBuffer(bytes))
    .unpack().value as t.Temperature;
}
export function encodeWarning(value: t.Warning): Uint8Array { /* packWarning, pack */ }
export function decodeWarning(bytes: Uint8Array): t.Warning { /* getRootAsWarning, unpack, unpackWarning */ }
function packWarning(value: t.Warning): fb.WarningT {
  return new fb.WarningT(value.code, BigInt(value.health));
}
function unpackWarning(object: fb.WarningT): t.Warning {
  return { code: object.code as t.Level, health: Number(object.health) as t.Health };
}
// encodeLevel, decodeLevel, encodeWindow, decodeWindow, encodeAverage,
// decodeAverage, encodeHealth, decodeHealth, encodeSubsystems,
// decodeSubsystems, encodeDiagnostic, decodeDiagnostic: as above.

const TEMPERATURE: PayloadCodec<t.Temperature> = {
  type: "veh.cabin.Temperature", encode: encodeTemperature, decode: decodeTemperature,
};
// WARNING, LEVEL, WINDOW, AVERAGE, HEALTH: as above.

export const Cabin = {
  iface: 1,
  temperature: { iface: 1, ord: 1, kind: "signal", payload: TEMPERATURE },
  warning: { iface: 1, ord: 2, kind: "event", payload: WARNING },
  setLevel: { iface: 1, ord: 3, kind: "command", request: LEVEL },
  average: { iface: 1, ord: 4, kind: "query", request: WINDOW, reply: AVERAGE },
} as const;

export const Horn = {
  iface: 2,
  active: { iface: 2, ord: 1, kind: "signal", payload: HEALTH },
} as const;
```

A user's program then reads:

```ts
const codec = await loadCodec(wasm, {
  catalogHash: CATALOG_HASH, payloadTypes: PAYLOAD_TYPES, ruleText: RULES,
});
const session = await connect(url, { catalog: { name: CATALOG_NAME, hash: CATALOG_HASH }, encoding: "flatbuffers", onPublish });
await session.subscribe({ kind: Kind.Signal, iface: Cabin.temperature.iface, ord: Cabin.temperature.ord });
// in onPublish, for a sample of Cabin.temperature:
const result = codec.verify(codec.typeId(Cabin.temperature.payload.type), sample.payload);
if (result.ok) use(Cabin.temperature.payload.decode(sample.payload));
```

Names the file reserves: `CATALOG_NAME`, `CATALOG_HASH`, `PAYLOAD_TYPES`,
`RULES`, `encode<Name>`, `decode<Name>`, the interface names, the induced tuple
names of D-9, the private converters, the private `codec<Name>` constants and
the private `absent` function (written only when a converter reads a required
field the buffer may leave out). The imports are namespaced (`t`, `fb`,
`flatbuffers`, one `<alias>` per foreign package whose types a converter names,
one `<alias>_codec` per foreign payload root), so a declaration of the package
never collides with an import. The file imports only `PayloadCodec` from the
runtime package: the entries satisfy `MemberRef` structurally and do not name
it. A declaration whose name collides with a reserved name — an induced tuple
spelled as a declared type, which the model records as a `tuple_collisions`
fact, or an interface named `RULES` — is refused with a `GenerateError` naming
both, as the Rust backend refuses a claimed name.

## How interface declarations reach `ridl-backend-ts`

They already do: `ridlc` builds one `CodegenRequest` per package with
`model: Some(lower_with(package, others, compatible))`, and `Model.interfaces`
holds every interface of the package — its `number`, and a slot per interaction
with the `ordinal`, the `kind`, the `shape` (the `payload` of a signal or event,
the `request` or `request_tuple` of a call, the `reply_payload` or `reply_tuple`
of a query, the `named` payload of a `fixed`). The backend's `generate` ignores
them today. The codec generator is a new module of the crate,
`crates/ridl-backend-ts/src/codec.rs`, that reads the model only — no raw IR —
so it is written in the direction `docs/design/codegen-plugins.md` sets for the
port of this backend onto the model. The payload type of each role is read by
the rule the Rust descriptors use (`single_param_type`, `query_param_type`,
`query_reply_type` in `crates/ridl-backend-rust/src/descriptors.rs`): the single
named parameter or the induced tuple, the named reply or the induced reply
tuple. A payload whose FlatBuffers root has no finite bound has no
`Payload<FlatBuffers>` impl and no `type_id`; its interaction gets no entry, and
the file states the omission in a comment at the entry's place, as the Rust
crate leaves a `__RIDL_FB_NO_CODEC_<Name>` note.

The public `generate(package)` of the crate stays as it is. The codec file is
produced only through the backend contract, because only the request's model
carries the catalog hash of the unit that `wasm_module_source` writes into the
module; a model lowered with no `others` and no `compatible` could carry another
hash.

## `flatc`: who runs it, where its output lives, import paths

- **Who.** The user, or the repository's recipe, runs
  `flatc --ts --gen-object-api -o <dir> <base>.fbs` on the `.fbs` that
  `ridl build --emit flatbuffers` wrote. `ridlc` runs no external tool (D-10).
- **Where.** `<dir>` is the directory of the TypeScript emit. For cabin, every
  emit goes to `examples/cabin/web/generated/`:
  `ridl build examples/cabin --emit typescript --emit flatbuffers --out-dir examples/cabin/web/generated`
  writes `veh.cabin.ts`, `veh.cabin.codec.ts` and `veh.cabin.fbs`, and
  `flatc … -o examples/cabin/web/generated` writes `veh/cabin.ts` and
  `veh/cabin/*.ts` beside them, with `-I examples/cabin/web/generated` for the
  includes. No `schema/` subdirectory is written. When a codec module imports
  another package's codec module (D-14), `flatc` runs on that package's `.fbs`
  too, as the recipe `_codec-ts-check` does for `veh.common.fbs` and
  `ridl.std.fbs`.
- **Import paths.** `./veh/cabin.js` for the `flatc` barrel, `./veh.cabin.js`
  for the types, `flatbuffers` and `@driftsys/ridl-runtime` as bare specifiers
  that the user's import map or `package.json` resolves (D-15).

## The type-id agreement and its pin test

Three layers hold it:

1. **By construction (D-5, D-7).** The id is read from the module through
   `ridl_type_id`; the TypeScript never states one. A name the module does not
   hold throws `UnknownPayloadType` at load when `payloadTypes` is given, and on
   first use otherwise.
2. **The shared name rule, pinned (D-6).** A test in `crates/ridlc/tests/`
   builds the cabin example with `--emit rust --emit typescript`, reads
   `WASM_PAYLOAD_TYPES` and `WASM_RULES` from `__ridl_wasm.rs` and
   `PAYLOAD_TYPES` and `RULES` from `veh.cabin.codec.ts`, and asserts that the
   two type lists are equal (cabin is a one-package workspace, so the package's
   list is the crate's) and the two rule lists are equal. A second case builds
   `crates/ridlc/tests/fixtures/codec-two-packages`, one root package `pin`
   whose files declare `pin.a` (`type Zed`) and `pin.b` (`type Alpha`) — one
   manifest, two packages, not a workspace — and asserts that the crate list is
   `["pin.a.Zed", "pin.b.Alpha"]`, that each package's `PAYLOAD_TYPES` is a
   subsequence of it in order, and that their concatenation equals it, which is
   what D-5 relies on.
3. **The export, pinned.** The Node host of `crates/ridlc/tests/wasm_module.rs`
   calls `ridl_type_id` for every name of `WASM_PAYLOAD_TYPES` and asserts the
   index, and asserts -1 for `veh.cabin.Nothing`; the native program asserts the
   same through `type_id_of`.

## The manifest option and its validation

```toml
[backend.rust]
encodings = ["flatbuffers", "proto3"]

[backend.typescript]
codec = true
```

- `codec` is a boolean; any other value is MANI-015, as for `wasm`.
- `codec = true` implies `[backend.rust] wasm = true` when the `rust` table
  writes no `wasm` key (D-12 rule 1): the manifest above builds the crate with
  its `wasm` module. A written `wasm = true` beside it is the same build.
- `codec = true` with a written `[backend.rust] wasm = false` is MANI-019:
  `` `[backend.typescript] codec = true` needs the wasm codec, but `[backend.rust]` sets `wasm = false` ``.
  `codec = true` with no `[backend.rust]` table is MANI-019 too:
  `` `[backend.typescript] codec = true` needs the wasm codec, and there is no `[backend.rust]` table to build it from ``.
  Both report on the `codec` value, and `ridl build` writes no artifact, as for
  any manifest error.
- An implied `wasm` still needs `encodings` to name `flatbuffers`; the Rust
  backend's refusal then reads
  `` `wasm` is implied by `codec = true` in the manifest, and `[backend.rust] encodings` does not name `flatbuffers`, which every export of the `wasm` module checks ``.
  So the `.fbs` the wrappers need is always emittable when the build passes.
- The key is passed only to the backend whose table carries it. The TypeScript
  backend takes `codec`; the Rust backend and a plugin that take no such option
  refuse it with the message their unknown-option path already gives.
- Setting `codec = true` and running `ridl build --emit typescript` alone is
  allowed: the file is still generated from the same manifest, and the catalog
  hash check at load catches a module from another compile of the sources.

## What changes in `examples/cabin/web`

- `examples/cabin/ridl.toml` gains `[backend.typescript] codec = true`.
- `_cabin-web-generate` in the `justfile` runs
  `ridl build examples/cabin --emit typescript --emit flatbuffers --out-dir examples/cabin/web/generated`
  and
  `flatc --ts --gen-object-api -o examples/cabin/web/generated examples/cabin/web/generated/veh.cabin.fbs`.
  `generated/` stays a build output outside git.
- `deno.json` maps `@driftsys/ridl-runtime` to
  `../../../packages/ridl-runtime/mod.ts`, which the codec file imports as a
  bare specifier; `main.ts`, `page.ts` and `check_test.ts` keep their relative
  imports of the packages or move to the bare names.
- `check.ts` drops `TEMPERATURE`, `TEMPERATURE_TYPE_ID`, `encodeTemperature`,
  `decodeTemperature`, the `flatbuffers` import and the `TemperatureBox` import.
  It imports `Cabin`, `encodeTemperature`, `decodeTemperature` and `RULES` from
  `./generated/veh.cabin.codec.ts`; `RANGE_RULE` becomes
  `2 + RULES.indexOf("range")`; `verifyPayload` and `selfTest` take the id from
  `codec.typeId(Cabin.temperature.payload.type)`.
- `main.ts` and `page.ts` load the codec with `CATALOG_HASH`, `PAYLOAD_TYPES`
  and `RULES` from the codec file, compare the hash the server announces with
  `CATALOG_HASH` and fail on a difference, attach with `CATALOG_NAME` and
  `CATALOG_HASH`, and address the signal through `Cabin.temperature`.
- `check_test.ts` drops the test that reads `__ridl_wasm.rs` (the pin test of
  `ridlc` replaces it) and the `TemperatureBox` test; it keeps the round trips
  through the generated `encodeTemperature` and `decodeTemperature`, the
  self-test cases, and adds one case: `codec.typeId("veh.cabin.Nothing")` throws
  `UnknownPayloadType`.
- The `server.ts` helper and the Rust server are unchanged.

## Frozen-surface impact (ADR-0029)

| Surface                  | Effect                                                                                                                                          |
| ------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| `manifest`               | one added line, `backend.*.codec boolean (implies backend.rust.wasm)`; an addition passes; the `backend.*.wasm boolean` line is unchanged       |
| `codegen-protocol`       | none: `BackendOption` carries the new key as it carries `wasm`                                                                                  |
| `generated-crate`        | none: the frozen copies do not set `wasm = true`, so the `wasm` module and its new export are not in a dump; the export is additive in any case |
| `runtime-crates`         | none: no Rust runtime crate changes                                                                                                             |
| `wire`, `diagnostics`    | `wire` none; `diagnostics` gains the line `MANI-019 error` (the two `codec` conflicts of D-12), an addition                                     |
| the TypeScript emit      | not a frozen surface (ADR-0029 decision 2); `<base>.ts` is unchanged in any case                                                                |
| `@driftsys/ridl-runtime` | not frozen and not yet published; `typeId`, `payloadTypes`, `UnknownPayloadType`, `PayloadCodec` and `MemberRef` are additions                  |

## Tests

- **`ridl-backend-ts`.** An `insta` snapshot of `veh.cabin.codec.ts` for the
  cabin fixture; a snapshot for a fixture that holds a union with a scalar arm,
  a map, a tuple field, an optional enum field without a zero member, a 64-bit
  enum set and a two-parameter command (D-9); refusals: `codec` with another
  option key, a foreign reference (D-14), a reserved-name collision, a model
  with no catalog hash. The existing `model_drift` test gains the codec file:
  every payload type of the model has an `encode` and a `decode`, and every live
  interaction of every interface has an entry.
- **`ridl-core`.** `codec` parses as a boolean; a non-boolean is MANI-015;
  `codec = true` with a written `wasm = false`, and with no `[backend.rust]`
  table, are the two MANI-019 cases; `codec = true` with no `wasm` key draws no
  diagnostic.
- **`ridlc`.** `codec = true` with no `wasm` key builds the `wasm` module and
  the codec file; an implied `wasm` without `flatbuffers` is refused with the
  implied wording; a MANI-019 manifest writes nothing; the pin test of "The
  type-id agreement" (two cases); `wasm_module.rs` extended with `ridl_type_id`
  and `type_id_of`.
- **`ridl-backend-rust`.** `descriptor_generation.rs` checks that the module's
  source holds `type_id_of` and the export.
- **`packages/ridl-runtime`.** The wabt fixture of `tests/fixture.ts` gains
  `ridl_type_id`; tests: `payloadTypes` resolves at load, a miss throws
  `UnknownPayloadType` with the name, `typeId` caches, a module without the
  export reports `CodecError` naming it.
- **`examples/cabin/web`.** `check_test.ts` as above; `cabin-ws-demo` and
  `cabin-browser-demo` type-check the codec file through `main.ts` and
  `page.ts`, and `deno test` runs the round trips against the real `flatc`
  output and the real module — that is the one place the generated wrappers meet
  `flatc`'s generated classes.
- **Interop beyond cabin.** `_cabin-web-generate` also builds the codec fixture
  (`crates/ridl-backend-ts/tests/fixtures/codec/`: the veh-cluster corpus's
  `common.typl`, copied verbatim and pinned equal by a test, plus an interface
  file in the same package) with `--emit typescript --emit flatbuffers` into a
  temporary directory, runs `flatc` on `veh.common.fbs` and `ridl.std.fbs` (the
  fixture's `query profile(name: Name)` makes `ridl.std.Name` a foreign payload
  root, so the module imports `ridl.std.codec.js`), runs `deno check` on its
  codec file with an import map for `flatbuffers` and `@driftsys/ridl-runtime`,
  and runs a wasm-free round-trip test, `decode(encode(v))` deep-equal to `v`,
  over a union, a map, a tuple, an optional field and a two-parameter call. That
  is the proof for the union, map, tuple and multi-parameter rules of D-4 and
  D-9 against the pinned `flatc`, which cabin does not exercise. A round trip
  through a wasm module of the fixture is a follow-up once it has a wasm build.

## Alternatives considered

| Alternative                                                                  | Rejected because                                                                                                                                                                                            |
| ---------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| The type id written into the TypeScript, by a shared sort rule               | the list spans every package of the crate and `ridl.std`; a per-package file cannot compute it from its model, and a wrong id is a silent wrong check (D-5)                                                 |
| The type id carried in the codegen model (`FbRoot.wasm_type_id`)             | the lowering is per package and does not see the crate's member list; `ridlc` would have to compute the index and patch every model, which is a second lowering step                                        |
| `ridlc` passes the crate's list as a backend option string                   | couples `ridlc` to one backend's needs through a comma-joined option, and a plugin would have to parse it; the module already holds the list, so asking the module is simpler                               |
| An export that enumerates the names (`ridl_payload_type_name(id, out, cap)`) | needs a buffer protocol and a loop in the loader for the same result as one lookup per name; `ridl_type_id` is symmetric with `ridl_verify`                                                                 |
| Wrappers over the accessor API, no object API                                | a generated reader per field and builder per table; the object API makes the generator a tree walk (D-3)                                                                                                    |
| The table typed with the transport's `Kind` enum                             | the runtime package would depend on the transport package; a string kind is enough to pick a transport call (D-8)                                                                                           |
| `PayloadCodec` and `MemberRef` declared in every generated file              | one copy per package of the same two interfaces; the runtime package is the one place they are versioned                                                                                                    |
| `RULES` in the runtime package instead of the codec file                     | the list is the module's, and the codec file is the artifact generated beside the module; the pin test compares the two generated outputs, which a hand-written list would not join                         |
| `ridlc` runs `flatc`                                                         | a tool spawned in the user's environment, the reason the earlier note gave for not running `cargo` (D-6 there)                                                                                              |
| `codec = true` implied by `wasm = true`, no second key                       | a Rust crate with the module and no TypeScript consumer would get a `.ts` file it did not ask for, and the TypeScript backend would read another backend's table; the implication runs the other way (D-12) |
| `codec = true` requires a written `wasm = true`                              | a user who asks for the TypeScript codec has asked for the module it runs on; a second key to write, and an error when it is missing, is work with no decision in it (D-12, ruling of 2026-10-22)           |
| One codec file per workspace instead of per package                          | the backend contract is per package; the module's crate-wide list is reached through D-5 without a crate-wide file                                                                                          |
| A checking `encode<Name>` that refuses an out-of-range value                 | duplicates the Rust check in TypeScript, which ADR-0020 decision 7 rules out; the self-test needs to build a rejected payload (D-11)                                                                        |

## Rulings on the open questions

Each question was put to Sebastien with the author's recommendation; he accepted
all six on 2026-10-22.

**Q-1. Accepted 2026-10-22: the type id is resolved by name through the new
export `ridl_type_id` (D-5), not written as a static id.** A static id needs the
crate-wide list in every package's file, which the model does not give. The
export is one `#[unsafe(no_mangle)]` function and a linear scan, it is additive
to the ABI, and it makes a mismatch a load-time error with the name in it.

**Q-2. Accepted 2026-10-22: a foreign reference is refused in the first slice
(D-14), with the message naming the payload type and the foreign declaration;
the self-contained converters come in the slice after the cabin demos run on the
table.** Cabin reaches no other package. The refusal is loud, and the path is
designed. The refusal's scope is the one D-14 states: a foreign struct, union or
enum; a foreign named scalar or enum set, `ridl.std`'s included, is converted by
a type assertion. Amended by the controller during execution (2026-10-22): a
foreign named scalar or enum set used as a payload root is accepted and its
table entry goes through that package's codec module, because its box table
lives only in that package's schema (D-14).

**Q-3. Accepted 2026-10-22: `.js` import suffixes in the codec file (D-15), and
a follow-up issue, filed with the pull request, to move `<base>.ts` to `.js` as
well.** That change touches a snapshot per test of the backend and is better
reviewed alone.

**Q-4. Accepted 2026-10-22: the table carries `CATALOG_HASH`, and the demo keeps
the server's announced hash as a cross-check.** The generated unit is
self-describing, and `loadCodec` and `connect` read the hash from one place.

**Q-5. Accepted 2026-10-22: the type-level proof of the union, map and tuple
rules runs now, as a `deno check` in `_cabin-web-generate` against the pinned
`flatc`; a round trip through a wasm module of that fixture is a later task.**
Because the `cluster` package of veh-cluster is refused under D-14, the fixture
is the corpus's `common` package, copied verbatim, plus one interface file in
the same package (the plan's Task 5); the plan adds a wasm-free round trip
(`decode(encode(v))`) to it, which pins the values the type check cannot.

**Q-6. Accepted 2026-10-22: `codec = true` does not require `--emit flatbuffers`
in the same build.** `ridl build` cannot see a later `flatc` run, and a build
with `--emit typescript` alone is a valid way to regenerate the wrappers after a
schema change; the header comment of the file states the two steps, and the demo
recipe runs them.
