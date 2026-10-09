# Writing a codegen plugin

A codegen plugin is a backend that is an executable rather than a part of
`ridl`. `ridl build` runs it once per package, writes one request to its
standard input, reads one response from its standard output, and writes the
files the response carries. A plugin can be written in any language that reads
and writes JSON.

This chapter is a guide. Each fact it summarizes is owned by another record,
and the chapter links to that record:

- [`docs/design/codegen-plugins.md`][design] — the plugin system as built: the
  contract, the process host, the reference plugins and the deployment section.
- [IR — encodings and stability](reference/ir.md) — the IR specification: the
  canonical encoding, the nesting bound, the compatibility rule and versioning.
- [ADR-0020][adr-0020] decisions 8 to 12 — the binding choices behind the
  plugin system.
- [CLI reference](cli-reference.md#ridl-build) — every flag of `ridl build`.

## Reading order

Read these in order before you write a plugin:

1. [The IR specification](reference/ir.md) §7 and §8: read `schema` first,
   parse leniently, and use a file `ridlc` wrote as a fixture.
2. [`plugin.proto`][plugin-proto]: the request and the response.
3. [`model.proto`][model-proto]: what the model carries, message by message.
4. [`deployment.proto`][deployment-proto]: the deployment section, if your
   plugin lays out memory for a deployment.
5. [`crates/ridlc-gen-model/src/main.rs`][gen-model]: a complete plugin.
6. The [`ridl build`](cli-reference.md#ridl-build) section of the CLI
   reference: how the plugin is found and what `ridl` does with the response.

## Running a plugin

A plugin for the language `kotlin` is an executable named `ridlc-gen-kotlin`.
`ridl build` runs it when you pass `--plugin`:

- `--plugin kotlin` looks for `ridlc-gen-kotlin` on `PATH`, in `PATH` order.
  On Windows, `ridlc-gen-kotlin.exe` is also tried.
- `--plugin kotlin=/opt/gen/bin/kt-gen` runs the executable at that path and
  skips the lookup. Messages still name the plugin `ridlc-gen-kotlin`.

The flag repeats, once per plugin, and is the same on `ridlc build`. A plugin
runs beside the `--emit` targets, and `--emit` defaults to `rust`, so a build
with `--plugin` and no `--emit` also writes the Rust files. The plugin runs
once per package that the code emits are written for, `ridl.std` included when
a package uses it. `--plugin-timeout <SECONDS>` sets how long one run may take
before `ridl` kills it; the default is 60 seconds.

The plugin's standard error is passed through to the terminal. The error cases
and their exit codes:

| Case                                                                                                | Exit                                                    |
| --------------------------------------------------------------------------------------------------- | ------------------------------------------------------- |
| a `--plugin` value that does not parse, such as `=/x` or `a/b`                                      | 2, before anything is compiled                          |
| no `PATH` directory holds the executable                                                            | 1, and the build writes no file                         |
| the executable cannot be started, exits non-zero, or is still running at the timeout                | 1                                                       |
| standard output is not a `CodegenResponse`                                                          | 1                                                       |
| a file path in the response breaks the path rule: it is absolute, or holds `..` or `\`, for example | 1, and no file of that response is written              |
| the response carries an error-severity diagnostic                                                   | 1, and no file of that response is written              |
| `--deployment NAME` names no deployment the system declares                                         | 2, unless the build already has an error, which exits 1 |

A plugin's failure exits 1, not 2, because the build completed and the failure
is one of its findings ([ADR-0010][adr-0010] decision 1). The messages of each
case are listed in [the design record, §4][design-host].

## The protocol

There is no framing and no binary form. The request is one JSON document on
the plugin's standard input; the plugin reads until the end of input. The
response is one JSON document on its standard output. Both are canonical
protobuf JSON, pretty-printed: keys are the proto3 JSON names, so the field
`artifact_base` is the key `artifactBase`; enum values are written by name;
64-bit integers are decimal strings; and `bytes` fields are base64.

**A plugin never writes files.** It returns each file in the response as a
`path`, relative to `--out-dir`, and a `text` or `binary` content, and `ridl`
writes it. So `--out-dir` means the same for a plugin as for the `rust` emit.

**The nesting depth.** A request can nest deeply, because a type can nest. The
IR specification §4 and §8 state how many JSON levels a reader must accept: at
least 516, and 1,000 is recommended. Some JSON libraries stop below that by
default, `serde_json` at 128 for example, and need one configuration step.

**The response is read strictly.** This toolchain's reader rejects a key the
schema does not have, so a plugin must not add fields of its own to the
response. A diagnostic is a `severity` and a `message`; there is no source
span. An unset severity counts as an error.

## The schemas

The schema of the request and the response is the protobuf package
`ridl.codegen.v1`, in three files in
[`crates/ridl-ir/proto/ridl/codegen/v1/`][proto-dir]:

- `plugin.proto` — `CodegenRequest`, `CodegenResponse`, `GeneratedFile`,
  `Diagnostic`, `BackendOption`;
- `model.proto` — the lowered model;
- `deployment.proto` — the deployment section.

`plugin.proto` imports the other two. The repository publishes no schema
package of its own. Today an author copies the files from the release tag the
plugin is tested against, as [driftsys/ridlc-gen-kotlin][kotlin] does: it keeps
the files unchanged and runs a check that fails when they differ from the
pinned tag. Copy every file that the tag's `plugin.proto` imports: where the
tag holds `deployment.proto`, that is all three files, and a `plugin.proto`
that imports `deployment.proto` does not compile without it. An older tag holds
only `plugin.proto` and `model.proto`.

## Compatibility

The rules are the IR specification's §6 to §8. In short:

- **Additive changes are made in place.** A new field, a new enum value or a new
  `oneof` member can appear in `ridl.codegen.v1` at any release. A plugin whose
  request reader is built on an older `ridl-ir` still rejects a request that
  carries a new enum value, so that plugin must be rebuilt against the
  `ridl-ir` that adds the value. Any other
  change — a removal, a rename, a renumbering, a change of type or of meaning —
  is a new package, `ridl.codegen.v2`, compiled beside the old one.
- **A plugin reads the request leniently.** It ignores keys it does not know,
  so a request from a newer `ridl` still reads. `ridl_ir::codegen::request_from_json`
  does this at every nesting level of the request, from `ridl-ir` 0.6.0
  on. It still rejects an unknown enum name, because that
  changes the meaning of a known field, so a new enum value is a change an older
  plugin reports as an error. A Rust plugin built against an earlier `ridl-ir`
  rejects a request that carries a key that release does not know, and must be
  rebuilt once.
- **A plugin refuses an unknown `schema`.** `schema` is the first field of the
  request. A plugin that does not know its value answers with an error
  diagnostic naming both values. A plugin never refuses on `toolchain`, which
  only says which version of `ridl` wrote the request.
- **The toolchain reads the response strictly**, as the protocol section says.

## What the request carries

These values are the top-level fields of the request `ridl build` sends for
`examples/cabin`, with `model` and `deployment` left out of the excerpt.
`toolchain` is the version of `ridl` that wrote the request, so it changes with
each release; the excerpt shows it as `<version>`:

```sh
jq '{schema, toolchain, options, artifactBase, generatedMarker, header}' request.json
```

```json
{
  "schema": "ridl.codegen.v1",
  "toolchain": "<version>",
  "options": [],
  "artifactBase": "veh.cabin",
  "generatedMarker": "@generated by ridl from package veh.cabin. Do not edit.",
  "header": ""
}
```

- **`model`** is the lowered model of one package: its declarations, with type
  references resolved and names in every case the name transform defines
  ([ADR-0016][adr-0016]), its interfaces and their members, its services, and
  its catalog. It is the same JSON value that
  `ridl build --emit codegen-model` writes to `<base>.codegen.json`, one
  indentation level deeper, so that file is a fixture for a plugin's tests.
- **`options`** is a list of `{key, value}` pairs sorted by key. No flag sets
  an option yet, so a plugin run from the command line receives an empty list.
- **`artifactBase`** is the name `ridl` gives this package's files: the
  package name, or the file stem in single-file mode. A plugin that lays its
  output out another way can ignore it.
- **`generatedMarker`** is the line that marks a generated file as generated,
  without a comment token. The host composes it. See
  [the marker](#the-marker-and-the-header) below.
- **`header`** is the project's file header from `[codegen] header-file`,
  without a comment token, or an empty string when the project sets none. Both
  keys are always present in the JSON, so an empty header is `"header": ""`.
- **`deployment`** is the deployment section, described below. It is absent
  when the build selects no deployment.

### The marker and the header

A plugin is recommended, not required, to write `generatedMarker` as the first
line of each file it writes that can hold a comment, as a comment in that
file's syntax, and then `header`, one comment line per header line, then one
blank line. The marker comes first because `rustfmt` looks for `@generated`
only in the first 5 lines of a file. A plugin that writes in Rust can call
`ridl_ir::codegen::comment_preamble(marker, header, "//")`, which returns that
block; a plugin in another language writes the same layout, with an empty
header line as the bare comment token and no trailing space. A file that cannot
hold a comment, such as JSON, carries neither. Where the key comes from, and
how a project sets the header, is in the
[CLI reference](cli-reference.md#ridl-build).

### The deployment section

The deployment section holds the facts of one deployment of the workspace's
system that a plugin needs to lay out memory: the regions, one per catalog,
with each catalog's hash and interfaces; the instances; one channel per member
and producer instance, with its consumer links; and the transport bindings.
Every request of one build carries the same section. [The design record,
§7][design-deployment], states what each part holds, its order, and the rules
behind each value.

`--deployment NAME` selects the deployment. Without the flag, the request
carries the deployment when the system declares exactly one, and none when it
declares several or the workspace has no system. The
[CLI reference](cli-reference.md#ridl-build) states every case.

Each consumer link states its crossing and the encoding that follows from the
crossing: FlatBuffers on the same machine, proto3 to a different machine or
off-board. One consumer link of the `warning` event in `examples/cabin`:

```sh
jq '.deployment.channels[1].consumers[0]' request.json
```

```json
{
  "component": "veh.cabin.Panel",
  "instance": "Unit",
  "machine": "Hpc",
  "crossing": "CROSSING_SAME_MACHINE",
  "encoding": "ENCODING_FLATBUFFERS",
  "depth": {
    "value": 10,
    "source": "VALUE_SOURCE_DERIVED"
  },
  "slotsSource": "VALUE_SOURCE_UNSPECIFIED",
  "budgetSource": "VALUE_SOURCE_UNSPECIFIED"
}
```

**Where a sizing value comes from.** An event's `depth`, and a command's or a
query's `slots` and `budget`, each come with a source, written in the JSON as
`VALUE_SOURCE_<NAME>`:

| Source        | Meaning                                                                                                                             |
| ------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| `DERIVED`     | computed by the toolchain: an event's depth is `ceil(max / min)` over its resolved timing                                           |
| `DECLARED`    | written in the rsdl source, on the placement line of the consuming instance or on the deployment                                    |
| `DEFAULT`     | no value was declared and the toolchain applied its default: 16 slots for a command or a query                                      |
| `UNDERIVABLE` | no value was declared and none can be derived, so the value is absent; for a depth, for example, when the timing has only one bound |

A source of `UNSPECIFIED` comes with an absent value: a field the member's kind
does not use, such as `slots` on an event link, or a `budget` that nothing
declares. [The design record,
§7][design-deployment], lists every case in which a depth is underivable. The
[rsdl reference](reference/rsdl.md) §5 states the sizing keys, and [Describing a system](rsdl.md)
shows them in use.

### Sizes

The model states the maximum encoded size of every payload, of every command
and query request, and of every query reply, once per wire encoding, proto3 and
FlatBuffers. Each is bounded with a byte count, unbounded with a cause (for
FlatBuffers only), or absent with a cause. The model also sums them:
`Interaction.reservation` is the memory a call table reserves for one member,
and `Interface.table_budget` (the key `tableBudget`) is the memory for the whole interface, each as a
byte count or as `unsized`, naming the first payload or member that has no
bounded size.

**A plugin reads these sizes; it does not compute them.** The sizes of the
`warning` event's payload in `examples/cabin`, and its reservation:

```sh
jq '.model.interfaces[0].slots[1].interaction.event.payload.sizes' request.json
```

```json
{
  "proto3": {
    "bounded": 8
  },
  "flatbuffers": {
    "bounded": 60
  }
}
```

```sh
jq '.model.interfaces[0].slots[1].interaction.reservation' request.json
```

```json
{
  "proto3": {
    "bytes": "8"
  },
  "flatbuffers": {
    "bytes": "60"
  }
}
```

A reservation is a 64-bit integer, so its byte count is a string. The rules
behind each state and each sum are in
[the catalog descriptor record, "The size states"][size-states]. A plugin that
sizes a socket message adds the frame header and the envelope of the transport
binding to the payload bound; the deployment section's `bindings` list carries
those values, and it is empty today (see below).

## The catalog hash

The model's `catalog` holds the package name and the catalog hash, a 32-byte
SHA-256 over the package's interfaces and every declaration they reach
([ADR-0014][adr-0014] decision 15). In the JSON, the hash is base64. Each region
of the deployment section carries the same hash for its catalog.

Embed the name and the hash in the generated code, and check them when the
generated code binds to a port. The Rust face does this: each generated
interface carries its catalog's name and hash, and binding to a port whose
catalog differs panics with a message that names both catalogs
([ADR-0023][adr-0023] decision 8; [the face's catalog check][face-check]). Code
generated from one contract then refuses a port of another contract when it
binds, instead of exchanging bytes that one side reads wrongly.

**A provisional interface number.** An interface that has no entry in its
unit's `interfaces.lock` has a provisional number, and the model marks it
`provisional`. A provisional number can change: adding an interface before it
in the source can move it, and recording it with
[`ridl lock`](cli-reference.md#ridl-lock) changes the catalog hash, because the
hash covers each interface's number and its `provisional` flag. A provisional
number is fine during development. Never rely on it across parties that are
built separately; run `ridl lock` and commit `interfaces.lock` first.
`ridl diff` does not report the hash change that `ridl lock` causes
([driftsys/ridl#700][i700]).

## The catalog descriptor

`ridl build --emit catalog` writes `<base>.catalog.binfb`, a FlatBuffers file
of a unit's interfaces, their members, their payload size bounds and the
catalog hash. The unit is the set of source packages that one `ridl.toml`
declares, and the hash is computed over that unit. One file is written per unit
that declares an interface or a service with an inline body; a unit that
declares only types writes none. It is for a run-time engine that is not
compiled against the unit and reads the file when a party attaches; no such
engine is in this repository.
[`ridl describe`](cli-reference.md#ridl-describe) prints it as JSON.

A plugin does not read the catalog descriptor: the model carries the same
facts. The chapter [The catalog descriptor](catalog-descriptor.md) describes
the file for its readers, and the [catalog descriptor record][catalog] gives
its byte-level details.

A system descriptor, a file per deployment, is not built
([catalog descriptor record, "Not built"][not-built]). The request's deployment
section carries those facts to a plugin at build time.

## What is not available yet

- **Transport binding overheads.** The deployment section's `bindings` list is
  empty, because no binding document states a frame layout yet. A socket
  message's maximum size cannot be computed until it has rows
  ([driftsys/ridl#718][i718]).
- **`repr(C)`.** The `ENCODING_REPR_C` value exists in the schema, but no
  consumer link carries it and no payload has a `repr(C)` size state
  ([driftsys/ridl#317][i317]).
- **Members whose depth or size cannot be derived.** An event's depth is absent
  with the source `UNDERIVABLE` when its timing gives no bound. The build warns
  with RSDL-806 when a consumer link consumes such an event with no declared
  `depth` (see [Lints](lints.md)); it does not warn when no link consumes the
  event ([driftsys/ridl#735][i735]). A stream payload, a request of zero or of
  several parameters, and an inline `T | E` reply have no size in any encoding,
  so the reservation of such a member is `unsized`. No single issue tracks
  these cases; each is recorded in [the design record, §7][design-deployment],
  and in [the catalog descriptor record][size-states].
- **No command writes out the whole request** ([driftsys/ridl#725][i725]).
  `--emit codegen-model` writes the model, but nothing writes the request with
  its deployment section. To see it, run a plugin that saves its standard input
  and answers with an empty response:

  ```sh
  #!/bin/sh
  # Save the request, then answer with no file and no diagnostic.
  cat > "request-$$.json"
  printf '{"files": [], "diagnostics": []}\n'
  ```

  Save it as `save-request`, make it executable, and run
  `ridl build --plugin save=./save-request`. Each package writes one
  `request-<pid>.json` in the current directory. The excerpts in this chapter
  were taken from such a file, built from `examples/cabin`.

## Examples

- [`crates/ridlc-gen-model/src/main.rs`][gen-model] is a complete plugin in
  one file of under 80 lines, its module comment included. It reads the
  request, refuses an unknown `schema`, writes the model back as one file, and
  writes the response.
- [`crates/ridlc/tests/layout.rs`][layout] holds a test plugin that reads the
  deployment section. It computes the byte layout of each region of the
  `examples/cabin` deployment and the maximum size of each message that crosses
  a socket, from the request alone. Its module comment states the layout rule,
  which belongs to the test and not to the toolchain. It runs in process,
  through the same `Backend` trait the in-tree backends implement.
- [driftsys/ridlc-gen-kotlin][kotlin] is a plugin outside this repository,
  written in Kotlin. It runs through `ridl build --plugin kotlin` and generates
  Kotlin value objects, codecs and faces from the model.

[design]: https://github.com/driftsys/ridl/blob/main/docs/design/codegen-plugins.md
[design-host]: https://github.com/driftsys/ridl/blob/main/docs/design/codegen-plugins.md#4-the-process-host
[design-deployment]: https://github.com/driftsys/ridl/blob/main/docs/design/codegen-plugins.md#7-the-deployment-section
[catalog]: https://github.com/driftsys/ridl/blob/main/docs/design/catalog-descriptor.md
[size-states]: https://github.com/driftsys/ridl/blob/main/docs/design/catalog-descriptor.md#the-size-states
[not-built]: https://github.com/driftsys/ridl/blob/main/docs/design/catalog-descriptor.md#not-built
[face-check]: https://github.com/driftsys/ridl/blob/main/docs/design/interaction-face.md#the-catalog-check
[adr-0010]: https://github.com/driftsys/ridl/blob/main/docs/decisions/ADR-0010-cli-conventions.md
[adr-0014]: https://github.com/driftsys/ridl/blob/main/docs/decisions/ADR-0014-ir-encodings.md
[adr-0016]: https://github.com/driftsys/ridl/blob/main/docs/decisions/ADR-0016-schema-projection-and-the-name-transform.md
[adr-0020]: https://github.com/driftsys/ridl/blob/main/docs/decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md
[adr-0023]: https://github.com/driftsys/ridl/blob/main/docs/decisions/ADR-0023-interaction-face-generation.md
[proto-dir]: https://github.com/driftsys/ridl/tree/main/crates/ridl-ir/proto/ridl/codegen/v1
[plugin-proto]: https://github.com/driftsys/ridl/blob/main/crates/ridl-ir/proto/ridl/codegen/v1/plugin.proto
[model-proto]: https://github.com/driftsys/ridl/blob/main/crates/ridl-ir/proto/ridl/codegen/v1/model.proto
[deployment-proto]: https://github.com/driftsys/ridl/blob/main/crates/ridl-ir/proto/ridl/codegen/v1/deployment.proto
[gen-model]: https://github.com/driftsys/ridl/blob/main/crates/ridlc-gen-model/src/main.rs
[layout]: https://github.com/driftsys/ridl/blob/main/crates/ridlc/tests/layout.rs
[kotlin]: https://github.com/driftsys/ridlc-gen-kotlin
[i317]: https://github.com/driftsys/ridl/issues/317
[i700]: https://github.com/driftsys/ridl/issues/700
[i718]: https://github.com/driftsys/ridl/issues/718
[i725]: https://github.com/driftsys/ridl/issues/725
[i735]: https://github.com/driftsys/ridl/issues/735
