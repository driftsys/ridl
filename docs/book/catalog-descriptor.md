# The catalog descriptor

The catalog descriptor is a small binary file, one per unit, that lists what a
runtime needs to route and check the traffic of that unit's interfaces: the
interface numbers, the members and their ordinals, the maximum encoded size of
each payload, the timing bounds, and the retired interfaces. A program reads it
without the compiler and without decoding the IR. A unit is one `ridl.toml`
with a `[package]` table and every source package in its directory tree; the
unit's name is the manifest's `name`, which is also the name of its root source
package ([the design record][design]).

## What it is for

A process that carries interactions between other processes — a gateway, a
broker, a bridge between two transports, or the engine of a runtime — does not
link the generated crate of every unit it carries. It still has to answer
questions such as:

- Which interface does the number in this frame name, and is that interface
  retired?
- Is this member a signal, an event, a command or a query, and which payload
  does it carry?
- What is the largest payload this member can send, so that a buffer can be
  sized in advance, and so that a larger payload can be refused? The size
  counts the payload alone, without the envelope and the framing a transport
  adds.
- What timing bound does this member declare?
- Do both peers speak the same version of the unit?

The catalog descriptor answers each of these from one file per unit, read at
startup.

**What is built.** The compiler writes the descriptor, the `ridl-descriptor`
crate reads and verifies it, and `ridl describe` prints it. No gateway, broker
or engine that reads it exists in this repository yet; those are the consumers
it is designed for. The generated Rust face does not read the file either: it
carries the catalog name and hash it needs as constants (see
[The catalog and the face](#the-catalog-and-the-face)). Codegen plugins do not
read it; the codegen model carries the same facts (see
[Writing a codegen plugin](codegen-plugins.md)).

## Producing it

`--emit catalog` writes one `<unit>.catalog.binfb` file per unit that declares
at least one interface, or a service with an inline body, in any of its source
packages:

```sh
ridl build examples/cabin --emit catalog --out-dir out
# writes out/veh.cabin.catalog.binfb
```

A unit with no interface gets no file. The flag combines with the others, so
one build can write the Rust crate and the descriptor together. The other emit
targets stay one file per source package; only the catalog is per unit.

## Reading it

`ridl describe` verifies a descriptor and prints it as JSON:

```sh
ridl describe out/veh.cabin.catalog.binfb
```

The top of the output for `examples/cabin`, with the 32 bytes of the hash left
out. `toolchain` is the version of `ridl` that wrote the file, so it changes
with each release; the excerpt shows it as `<version>`:

```json
{
  "hash": [ ... ],
  "interfaces": [
    {
      "members": [ ... ],
      "name": "Cabin",
      "number": 1,
      "provisional": true,
      "reserved_ordinals": []
    },
    ...
  ],
  "name": "veh.cabin",
  "retired": [],
  "toolchain": "<version>",
  "version": 1
}
```

The member `average`, a query, as it appears in `members`:

```json
{
  "kind": "Query",
  "name": "average",
  "ordinal": 4,
  "payloads": [
    {
      "max_sizes": [
        { "bytes": 46, "cause": "Unspecified", "encoding": "FlatBuffers", "state": "Bounded" }
      ],
      "role": "request",
      "type_name": "Window"
    },
    {
      "max_sizes": [
        { "bytes": 44, "cause": "Unspecified", "encoding": "FlatBuffers", "state": "Bounded" }
      ],
      "role": "response",
      "type_name": "Average"
    }
  ],
  "timing": { "max_us": "200000", "min_us": null, "mode": "Range" }
}
```

The keys are in alphabetical order, and an absent timing bound is `null`. A file
that is not a valid descriptor, or that has a schema version this toolchain
does not read, makes `ridl describe` exit with code 2. The
[CLI reference](cli-reference.md#ridl-describe) has the details.

## What it contains

- **The catalog**: the unit `name`; the `hash`, which identifies this version
  of the unit (see below); `toolchain`, the version of `ridl` that wrote the
  file; the schema `version`; the `interfaces`; and the `retired` interfaces,
  each with its name and number, so a reader can refuse a peer that still uses
  one.
- **Each interface**: its `name`, qualified relative to the unit (`Cabin` for
  an interface of the root source package, `cluster.Speed` for one of the
  source package `cluster` below it); its `number`, which is one space per
  unit; and `provisional`, which is true while the number is not yet recorded
  in the unit's `interfaces.lock`, the one lock file beside the unit's
  `ridl.toml`. `Cabin` above is provisional because `examples/cabin` has no
  lock file; a number becomes permanent when
  [`ridl lock`](cli-reference.md#ridl-lock) records it. `reserved_ordinals`
  lists the ordinals that `reserved` tombstones hold. The ridl reference,
  [section 11](reference/ridl.md#11-interaction-identity-and-evolution), is
  the rule for the names and the numbers.
- **Each member**: its `name`, its `ordinal` (its position in the interface
  body), its `kind`, its `payloads`, and its `timing` when the member declares
  one.
- **Each payload**: its `role` (`value` for a signal or a `fixed`, `occurrence`
  for an event, `request` for a command or a query, `response` for a query's
  reply), its `type_name`, and one `max_sizes` row per encoding. A row is
  `Bounded`, with `bytes` the maximum encoded size of the payload, envelope and
  framing excluded; or `Unbounded`, with `bytes` 0 and a `cause` that says why
  the compiler cannot bound it in that encoding. For example, a FlatBuffers row
  is `Unbounded` when the payload's largest encoding would exceed 4 GiB, the
  most that FlatBuffers' 32-bit offsets can address. An encoding with no row
  has no size computed by this toolchain. A stream payload, a request of zero
  or of several parameters, and an inline `T | E` reply have no row in any
  encoding. A `repr(C)` payload has no row, and a proto3 row is present only
  for a payload that is a struct or a union.

The descriptor does not contain type layouts, field lists, constraints other
than the ones that bound a payload's size, contract clauses, or initial values.
A reader that needs a payload's shape reads the schema that the wire backend
emits.

## The catalog and the face

The catalog hash is a SHA-256 hash over the unit's interfaces, from every
source package of the unit, their numbers, and every declaration they reach, in
the unit or in another one. Doc comments, labels and `deprecated` do not change
it; any other change to an interface or to a type it uses does, so a change in
any source package of the unit that an interface reaches changes the one hash.
The design record's [hash section][hash] has the exact input.

The generated Rust face carries the same unit name and hash as its `CATALOG`.
When a face binds to a port, it compares its `CATALOG` with the catalog of
that port, and panics when they differ. So a face generated from a different
version of the unit than the one its runtime serves fails at bind time,
instead of misreading payloads. The face compares itself with its own
port only; it does not compare catalogs with the party at the other end. The
[failures section](generated-code.md#failures) of "Using the generated Rust
code" shows what that check does.

## Stability

The file is a FlatBuffers buffer with the file identifier `RDLC`. Its schema
evolves by appending only: a field is added at the end of its table, and no
field is removed, reordered or retyped. A reader built against an older schema
ignores the fields it does not know. The `version` field is the schema version,
now 1; a reader refuses a version other than the one it was built for.

A Rust program reads the file with the `ridl-descriptor` crate, published on
crates.io. Its `verify` function checks the whole buffer before it returns a
view, so a later read cannot fail on a malformed offset. With its default
features off the crate is `no_std` with `alloc` and keeps `verify`, the
accessors and `finish`; the parts that build a descriptor from the IR need the
standard library and come with the `std` feature.

## Not built yet

The design also defines a system descriptor, one per deployment. It is not
built; the
[design record][not-built] lists it with the other parts that are not built.

## Further reading

- [The catalog descriptor][design] — the design record: the schema, the hash
  input, the size states, and the verifier.

[design]: https://github.com/driftsys/ridl/blob/main/docs/design/catalog-descriptor.md
[hash]: https://github.com/driftsys/ridl/blob/main/docs/design/catalog-descriptor.md#the-catalog-hash
[not-built]: https://github.com/driftsys/ridl/blob/main/docs/design/catalog-descriptor.md#not-built
