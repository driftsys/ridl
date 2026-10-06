# The codegen plugin system

The backend contract `generate(CodegenRequest) → CodegenResponse`, its two
hosts, and the reference plugin — as built. The binding choices are
[ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
decisions 8 to 12 (lower once; one contract; two hosts, the process host in this
release; the parity test, as amended 2026-09-22; the IR stability policy first),
[ADR-0014](../decisions/ADR-0014-ir-encodings.md) decision 9 as amended (the
canonical encoding is canonical protobuf JSON) and
[ADR-0010](../decisions/ADR-0010-cli-conventions.md) (the flags). The rules the
request is written under are
[the IR specification](../specification/ir-specification.md) §3 (what canonical
fixes), §6 (the compatibility rule) and §7 (versioning, and the two fields a
request leads with). The reasoning behind the model the request carries is
[the codegen model design note](../wip/2026-09-22-codegen-model-design.md). This
record implements three rules: the parity test runs over the Rust backend, not
the TypeScript one; the request carries the model, never the raw IR; and the
plugin never touches the filesystem.

**What is built and what is not.** The contract, the in-process host over every
in-tree backend, the process host, the `--plugin` flag and the two reference
plugins are built, and both parity tests run under `just test`. The **Rust
backend** reads the lowered model, so it reads the request's model, options and
artifact base and never the raw IR, and is run as a plugin, `ridlc-gen-rust`;
the parity test over it is the exit test of the plugin split. The other three
in-tree backends — TypeScript, proto3 and FlatBuffers — still read the raw IR
through `RawIr` and are not plugins; each waits to be ported. §6 says what each
parity test proves.

## 1. Where the code is

| What                                                                                                 | Where                                                                                                                                                                                                                                                                                                                                                                                                 |
| ---------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| The schema: `CodegenRequest`, `CodegenResponse`, `GeneratedFile`, `Diagnostic`, `BackendOption`      | `crates/ridl-ir/proto/ridl/codegen/v1/plugin.proto`, with the deployment section's messages in `crates/ridl-ir/proto/ridl/codegen/v1/deployment.proto`, in the package `ridl.codegen.v1` beside `model.proto`, all compiled by the same `build.rs` and registered with the same `pbjson-build` builder                                                                                                |
| The trait `Backend`, the transitional `RawIr`, `ModelBackend`, `SCHEMA`, the encodings, `check_path` | `crates/ridl-ir/src/codegen/contract.rs`, re-exported from `ridl_ir::codegen`                                                                                                                                                                                                                                                                                                                         |
| The four in-tree backends behind the trait                                                           | `crates/ridl-backend-{rust,ts,proto,flatbuffers}/src/contract.rs`. Rust is `pub struct Backend` over the request's model alone; the other three are `pub struct Backend<'a>` over `RawIr`, `generate` (TypeScript) or `generate_with` (proto3, FlatBuffers)                                                                                                                                           |
| The in-process host: one request per package, every emit through the trait, the response written     | `crates/ridlc/src/lib.rs` — `codegen_request`, `write_emits`, `write_response`, `run_build_with`                                                                                                                                                                                                                                                                                                      |
| The process host: `ridlc-gen-<language>`, `PATH` lookup, the pipe, the timeout, the error            | `crates/ridlc/src/plugin.rs` — `PluginSpec`, `resolve`, `run`, `PluginError`                                                                                                                                                                                                                                                                                                                          |
| The flags                                                                                            | `crates/ridlc/src/main.rs` and `crates/ridl/src/main.rs`, `--plugin`, `--plugin-timeout` and `--deployment` on `build`; documented in [`docs/book/cli-reference.md`](../book/cli-reference.md)                                                                                                                                                                                                        |
| The layout test plugin and its fixture                                                               | `crates/ridlc/tests/layout.rs` and `crates/ridlc/tests/fixtures/cabin-layout.json`                                                                                                                                                                                                                                                                                                                    |
| The reference plugins                                                                                | `crates/ridlc-gen-model/`, a binary over `ModelBackend`, and `crates/ridlc-gen-rust/`, a binary over the Rust backend; both `publish = false`                                                                                                                                                                                                                                                         |
| The parity tests                                                                                     | `crates/ridlc-gen-model/tests/parity.rs` and `crates/ridlc-gen-rust/tests/parity.rs`, each over every corpus package at the contract's level and every corpus entry at the command's level; the host's failure modes in `crates/ridlc/src/plugin.rs`'s tests; the flag's behaviour in `crates/ridlc/tests/cli.rs`; the messages' encodings and the path rule in `crates/ridl-ir/src/codegen/tests.rs` |

## 2. The contract

One request per package. In `ridl.codegen.v1`:

```text
CodegenRequest {
  string schema = 1;                  // "ridl.codegen.v1"
  string toolchain = 2;               // the ridlc version that wrote it, "0.2.0"
  Model model = 3;                    // the package's lowered model
  repeated BackendOption options = 4; // {key, value}, sorted by key, keys unique
  string artifact_base = 5;           // what ridlc names this package's files after
  optional Deployment deployment = 6; // one deployment of the system; absent when the build selects none (§7)
  string generated_marker = 7;        // the marker line, without a comment token; the host composes it
  string header = 8;                  // the project's header, normalised, without comment tokens; empty when none
}
CodegenResponse {
  repeated GeneratedFile files = 1;   // {path, oneof content {text | binary}}
  repeated Diagnostic diagnostics = 2;// {severity, message}
}
```

**`version` became `schema` and `toolchain`.** ADR-0020 decision 9 and the
driver write the request as `{version, model, options}`. The IR specification
§7, replaced the one field with two before the plugin design started, with its
reason: a bare number does not say which schema it counts, the package name
does, and a toolchain version is a different fact a consumer must not refuse on.
The request follows the specification. A consumer refuses a `schema` it does not
know with a diagnostic naming both values (the reference plugin does, §5) and
does not refuse on `toolchain`.

**The marker and the header are host facts, fields 7 and 8.** Amended 2026-10-06
(driftsys/ridl#746). `generated_marker` is
`@generated by ridl from package <pkg>. Do not edit.` with the dotted package
name, also in single-file mode, composed by
`ridl_ir::codegen::generated_marker`. `header` is the text of
`[codegen] header-file`, normalised by `ridl_ir::codegen::normalise_header` (no
`\r`, no trailing whitespace, no leading or trailing blank line). Both fields
are additive under the IR specification §6, with no change to `SCHEMA`. The JSON
writer emits both keys in every request (pbjson `emit_fields` is on), so a
project with no header sends `"header": ""`. A backend writes the marker as line
1 of every file that can hold a comment, the header lines next, then one blank
line (`ridl_ir::codegen::comment_preamble`); a backend for a format with no
comments ignores both. A plugin is recommended, not required, to write the
marker; the host does not check that it did.

**`artifact_base` is the one field the driver's shape does not name.** `ridlc`
names a package's artifacts after the package name in package and workspace mode
and after the input file's stem in single-file mode; that is a fact about the
build, not about the package, so the model cannot carry it, and every in-tree
backend needs it to name its one file. It is a typed field rather than an option
because it is the host's fact, not the backend's choice, and a plugin that lays
its output out another way is free to ignore it.

**A text file's content is a `string`.** The proto3 JSON mapping renders `bytes`
as base64, which would make every generated source file unreadable in the
response and in a fixture. Every file the four in-tree backends emit is UTF-8
text, so `GeneratedFile.content` is a `oneof` of `text` (a `string`) and
`binary` (`bytes`), and a backend picks the arm its file is. The IR stability
note §5 had flagged this for this stage; this is the decision.

**Options** are `{key, value}` pairs, not a `map<>`, because a `map<>` has no
canonical order (IR specification §3 item 3). The host passes each in-tree
backend none, and a plugin run from the command line receives an empty list — no
flag sets an option yet; adding one is additive. What a backend does with a key
it does not know is its own contract: every in-tree backend and the reference
plugin refuse it with an error diagnostic, and the Rust backend reads one key,
`wire-encoding`, whose one value today is `flatbuffers`.

**A diagnostic** is a severity and a message and nothing else: a backend has no
source span, because the model carries none. An unset severity is an error, so a
backend that forgets to set it has failed rather than warned.

**The encoding on the pipe** is canonical protobuf JSON, pretty-printed, through
the same generated impls and the same reader as the IR and the model
(`request_to_json`, `request_from_json`, `response_to_json`,
`response_from_json` in `ridl_ir::codegen`). The request's `model` is the same
JSON value as the `--emit codegen-model` artifact, one indentation level deeper,
so the two are equal as JSON but not byte for byte; a plugin author's fixture is
therefore a file `ridlc` wrote, wrapped. The reader keeps the IR's nesting
ceiling of 1,000 JSON levels; the deepest request the front end admits nests to
265 (the model's 264 plus one for the field), and a plugin's parser must
provision that much. The generated reader rejects an unknown key, so a plugin
that answers with a field this schema does not have is reported as malformed
rather than silently accepted; a plugin's own reader should be lenient, per the
IR specification §8.

**Sizes and reservations.** The model states the maximum encoded size of every
payload, of every command and query request, and of every query reply, once per
wire encoding (`PayloadSizes`), as bounded, unbounded (FlatBuffers only) or
absent with a cause. It also sums them: `Interaction.reservation` is the memory
a call table reserves for one member, and `Interface.table_budget` is the memory
for the interface, each as a byte count or as `unsized` with the name of the
first payload or member that has no bounded size. A plugin that sizes storage
reads these two fields and does not add the payload sizes itself. The sums and
the states they read are described in
[the catalog descriptor](catalog-descriptor.md#the-size-states).

**The path rule** every host applies before it writes a file
(`ridl_ir::codegen::check_path`): `/`-separated components, none empty, none `.`
or `..`, no leading `/`, no drive letter, no backslash, no NUL. A response with
one path that breaks it writes nothing, and the refusal names the backend or the
plugin and the path.

## 3. The in-process host

`ridlc::codegen_request(base, package, others, options, deployment)` builds the
one request per package — the model lowered once, over the same `others` every
code emit reads, so the request's model is the same JSON value as the artifact,
and the `deployment` section the build selected once, the same section in every
request — and `write_emits` hands it to each selected backend through the trait:

```rust
pub trait Backend {
    fn language(&self) -> &str;
    fn generate(&self, request: &v1::CodegenRequest) -> v1::CodegenResponse;
}
```

The signature is the contract's own, over the request alone. The Rust backend
reads the request's model, options and artifact base and never the raw IR, so it
is constructed with nothing. The TypeScript, proto3 and FlatBuffers backends are
still readers of the raw IR until their port, so each is constructed with a
`RawIr { package, others }` it keeps beside the request and reads in place of
`request.model`:

```rust
let raw = codegen::RawIr { package: ir, others };
let backend: Box<dyn codegen::Backend + '_> = match emit {
    Emit::Rust => Box::new(ridl_backend_rust::Backend),
    Emit::TypeScript => Box::new(ridl_backend_ts::Backend::new(raw)),
    Emit::Proto => Box::new(ridl_backend_proto::Backend::new(raw)),
    Emit::Flatbuffers => Box::new(ridl_backend_flatbuffers::Backend::new(raw)),
    Emit::CodegenModel => Box::new(codegen::ModelBackend),
    Emit::Catalog => /* the catalog descriptor, written directly, no request */,
    Emit::IrJson | Emit::IrText | Emit::IrBinary => /* a direct IR dump, no request */,
};
let response = backend.generate(request);
write_response(out_dir, Origin::InTree { language: backend.language() }, &response, diagnostics)?;
```

So what changes when a backend is ported is how it is built — `RawIr` leaves its
constructor, as it has left the Rust backend's — never how it is called, and the
request it is handed today is already the request a plugin is handed. The three
IR dumps and the catalog descriptor are not backends and take no request. The
request is built when a plugin runs or when an emit other than an IR dump is
selected, so a build whose only emit is `catalog` builds a request that nothing
reads.

`write_response` records the response's diagnostics — an in-tree backend's
message as it is, since the corpus snapshots pin those messages; a plugin's
prefixed with ``plugin `ridlc-gen-<language>`:`` — and then writes every file,
or none: none when the response carries an error, none when any path fails the
rule. A path with directories in it has them created. The `lib.rs` and
`Cargo.toml` a Rust emit writes are unchanged and still gated on no error
diagnostic being present, which a plugin's error now also is. Both crate files
begin with the marker (`@generated by ridl. Do not edit.`, no package, because a
workspace build's crate root belongs to no one package), then the header, behind
`//` in `lib.rs` and `#` in `Cargo.toml`. A build overwrites either file only
when it begins with the comment token followed by `@generated by ridl`, or with
the marker an earlier release wrote (`// Generated by ridlc. Do not edit.`,
`# Generated by ridlc. Do not edit.`). `lib.rs` carries
`#![allow(clippy::derivable_impls, clippy::module_inception)]` after the
preamble. The allow list is closed: `just demo` swaps `allow` for `expect` and
runs clippy with `-D warnings` on the generated crate for `examples/cabin`.
Single-file mode writes no `lib.rs`.

## 4. The process host

`--plugin <LANGUAGE>[=<PATH>]`, repeatable, and `--plugin-timeout <SECONDS>`
(default 60), the same two flags on `ridl build` and `ridlc build`, spelled and
documented identically (ADR-0010's consistency rule). The precedent for the
value's shape is `protoc --plugin=[NAME=]PATH`: one flag, one naming convention,
one escape from it.

**Lookup.** `<LANGUAGE>` alone names `ridlc-gen-<LANGUAGE>`, found by walking
`PATH` in order and taking the first directory that holds a file of that name
(on Windows, `.exe` is also tried; no other extension). `<LANGUAGE>=<PATH>` runs
the executable at `<PATH>` and skips the lookup; the language still names the
plugin in every message. Every `--plugin` value is resolved before the build
compiles anything, and a value that resolves to nothing is an error diagnostic
that, like a compile error, suppresses every artifact — a misspelled plugin does
not leave every other artifact behind.

**The run** (`ridlc::plugin::run`): the executable is spawned with its standard
input and output piped and its standard error inherited, so a plugin's own
messages reach the terminal as `ridlc`'s do. The request is written on one
thread and the response read on another, so a plugin that writes before it has
read everything cannot block the host on a full pipe in either direction. The
host polls the child's exit every 10 ms until the timeout, then kills it. The
two pipe threads are not joined on the kill path: a plugin that started a child
of its own — a launcher script starting a JVM — leaves that child holding both
pipes after the plugin is killed, and a join would wait for the child rather
than for the plugin. That case was found by the timeout test, whose `sh` script
runs `sleep`.

**The errors**, each a `PluginError` whose message names the plugin and, where
one exists, the executable's path; `ridlc` reports each as an error diagnostic,
so the build exits 1, the same code an in-tree backend's failure exits with:

| Case                                                  | Message                                                                                                          |
| ----------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| no `PATH` directory holds `ridlc-gen-<language>`      | ``plugin `ridlc-gen-kotlin` not found: no directory on PATH holds an executable of that name; install it, or …`` |
| the executable cannot be started                      | ``plugin `ridlc-gen-kotlin` (<path>) cannot be started: <os error>``                                             |
| a non-zero exit, or a signal                          | ``plugin `ridlc-gen-kotlin` (<path>) failed: exit status: 3``                                                    |
| still running at the timeout                          | ``plugin `ridlc-gen-kotlin` (<path>) did not finish within 60 s and was killed; raise `--plugin-timeout` …``     |
| exit 0 but standard output is not a `CodegenResponse` | ``plugin `ridlc-gen-kotlin` (<path>) returned a malformed response: not a `ridl.codegen.v1.CodegenResponse`: …`` |
| a response file path that breaks the rule             | ``plugin `ridlc-gen-kotlin` returned a file path `../x` that ridlc will not write: the path has a `..` …``       |

A response that carries an error-severity diagnostic is not a host failure: it
is a backend that failed, reported through `write_response` like an in-tree
one's.

**Exit codes.** A plugin problem is exit 1, not 2, on both binaries: ADR-0010
decision 1 gives 2 to a run the tool could not complete for a reason outside the
input — an unreadable file, a usage error — and 1 to a run that completed and
found an error. The build completes; the plugin's failure is one of its
findings, beside a compile error, and an in-tree backend's refusal already
exits 1. A `--plugin` value that does not parse (`=/x`, `a/b`) is a usage error
and exits 2 before anything is compiled.

## 5. The reference plugins

`crates/ridlc-gen-model/` builds `ridlc-gen-model`: `--emit codegen-model` as a
process. It reads one `CodegenRequest` from its standard input, refuses a
`schema` other than `ridl.codegen.v1` with a diagnostic naming both values,
generates through the same `ModelBackend` the in-process host calls — one file,
`<artifact_base>.codegen.json`, the model written back through `to_json_pretty`
— and writes the response to its standard output. Input that is not a request is
a message on standard error and exit 1, which the host reports. It opens no
file. It depends on `ridl-ir` alone.

`crates/ridlc-gen-rust/` builds `ridlc-gen-rust`, the same shape over the Rust
backend: `--emit rust` as a process. It reads one `CodegenRequest`, refuses an
unknown `schema` the same way, generates through the same
`ridl_backend_rust::Backend` the in-process host calls — one file,
`<artifact_base>.rs`, from `generate_pipeline` over the request's model — and
writes the response to its standard output. It opens no file. It depends on
`ridl-ir` and `ridl-backend-rust`. It exists because the Rust backend reads the
model: before that the backend read the raw IR, and a plugin has none.

Both are test-only: `publish = false`, not installed by any release, never on a
user's `PATH`. The test suite reaches each as `CARGO_BIN_EXE_<name>`, which
cargo sets for the crate's own integration tests and which is why each parity
test lives in its own crate rather than in `ridlc`: no installation, no `PATH`,
no network, under `just test` as it is. Each adds its own scope to
`.git-std.toml`.

## 6. The parity test, and what it proves

ADR-0020 decision 11, as amended 2026-09-22, makes the parity test the in-tree
Rust backend run as a plugin; that test exists in
`crates/ridlc-gen-rust/tests/parity.rs`, on the two levels below with
`ridl_backend_rust::Backend` and `ridlc-gen-rust` in place of `ModelBackend` and
`ridlc-gen-model`, and `--emit rust` in place of `--emit codegen-model`. Its
command's level compares the per-package `.rs` sources: `lib.rs` and
`Cargo.toml` are what `ridlc` writes for itself, no backend produces them, and a
build that names only a plugin writes neither.

It could not be written before the port. The Rust backend read the raw IR, a
plugin has none, so no test could make it byte-identical through the host. The
two ways to run it anyway — carry the raw IR in the request during the
transition, or reconstruct the IR from the model in the plugin — were rejected:
the first violates the rule that the request carries the model and, under the
compatibility rule, leaves a field in `ridl.codegen.v1` that can never be
removed; the second is a second lowering in reverse, thrown away once the
backend was ported.

The `ridlc-gen-model` test stays beside it, and is the one described below: it
is the narrower proof, over the one backend whose output is a function of the
request alone, and it is what a change to the host itself fails first. It runs
on two levels, in `crates/ridlc-gen-model/tests/parity.rs`:

- **At the contract's level.** For every package of every corpus entry, with the
  scope the CLI gives the backends (every sibling plus `ridl.std`), one
  `codegen_request` is answered by `ModelBackend` in process and by
  `ridlc-gen-model` through `ridlc::plugin::run`, and the two responses are
  equal — every file, every byte.
- **At the command's level.** Every corpus entry is built twice through
  `run_build_with`, once with `--emit codegen-model` and once with
  `--plugin model=<the built binary>` and no emit, and the two output
  directories hold the same files with the same bytes. An entry that does not
  build writes nothing either way, and the test asserts the two builds agree on
  that too.

What the `ridlc-gen-model` test proves: the host, end to end — the request
rendered and parsed, the response parsed and written, the file under `--out-dir`
with the same name and bytes, the process started and reaped — and that the
model survives a parse and a re-render byte for byte, which is the canonical
form's own conformance obligation. What it does not prove: that the model is
sufficient for a language backend. That is what the `ridlc-gen-rust` test
proves, over a backend whose output is a whole Rust crate's source, and what the
unmoved snapshots of `crates/ridl-backend-rust/src/snapshots/`,
`crates/ridlc/tests/snapshots/` and
`crates/ridl-backend-rust/tests/generated/interaction_face.rs` prove beside it:
the model carries every fact that output is a function of.

**The other three backends stay on the raw IR** — TypeScript, proto3 and
FlatBuffers — until their own stories port them, and are not plugins until then.
Each keeps the fact-level drift test of the model design note §8.1
(`crates/ridl-backend-{ts,proto,flatbuffers}/tests/model_drift.rs`), which is
what catches the model and a backend deriving one fact differently. The Rust
backend's is deleted: every fact it compared is a function of the model by
construction now, so the test could not fail.

## 7. The deployment section

`CodegenRequest` has an optional field `deployment` (number 6) of the message
`Deployment`, declared in
`crates/ridl-ir/proto/ridl/codegen/v1/deployment.proto` and imported by
`plugin.proto`. It holds the facts of one concrete deployment of a system that a
plugin needs to lay out memory for it. The emitter is
`crates/ridl-ir/src/codegen/deployment.rs`, `lower_deployment`: an emitter over
the lowered system artifact of the IR, as
[ADR-0022](../decisions/ADR-0022-rsdl-system-in-the-ir.md) decision 1 requires,
and not a second lowering. `ridlc::select_deployment` in
`crates/ridlc/src/lib.rs` chooses the deployment.

The section is a field of the request beside the model and never a part of
`Model`, so a model stays a function of its package and its scope, and
`--emit codegen-model` is unchanged. Every request of one build carries the same
section, whichever package the request is for. A plugin generating package P
finds P's regions and P's messages in the section, and the payload sizes of P's
messages in P's model: the section carries no payload size.

**A request with no deployment is unchanged.** The field is absent, and the
request is byte for byte the request written before the field existed. The field
is additive under the compatibility rule of
[the IR specification](../specification/ir-specification.md) §6. The generated
reader of this toolchain rejects an unknown key, as §2 says, so an in-tree
plugin is rebuilt with the schema; a plugin outside this workspace reads
leniently and ignores a field it does not know.

### What the section carries

In this order:

1. **Identity.** The system's qualified name and the deployment's bare name.
2. **Regions**, in catalog name order. Each has the catalog name, the 32-byte
   catalog hash, and the interfaces of the region in interface number order,
   each with its name, number, `inline` and `provisional` flags and the closure
   service that lists it.
3. **Instances**, in the placement order of the system IR. Each has the
   component, the instance name, the machine and the `external` flag, the
   interfaces the instance offers (the regions whose slots it writes), and the
   catalogs it maps (the catalogs its `requires` lines reach). An instance that
   offers or maps nothing is listed with empty lists.
4. **Channels**, in the order (catalog, interface number, member ordinal,
   producer component, producer instance). A channel is one member of an
   interface from one producer instance, with that member's kind, the producer
   endpoint, and the consumer links in (component, instance) order. A consumer
   link carries its crossing, its encoding and the sizing values of the
   channel's kind. An event channel also carries its ring depth.
5. **Bindings**, in binding name order.

The section states no count that is the length of a list.

**The order rule.** Every repeated field is in the order stated above, so two
builds of one workspace write the same bytes. Reordering the placements in the
source does not change the channels' order, because the order does not come from
the placements. The lowering of the system writes the routes in (catalog,
interface number, member ordinal) order, and the emitter sorts the producers of
each route by (component, instance) and the consumer links of each channel by
(component, instance).

**One channel per producer instance.** A redundant provider set, one route whose
producers are two instances of one component, gives two channels per member,
each with its own consumer links. The links are not merged into one channel. A
producer instance that no link names has a channel with no consumer link.

### The encoding rule

The encoding of a consumer link derives from its crossing and from nothing else,
by the matrix of
[ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
decision 2:

| Crossing            | Encoding    |
| ------------------- | ----------- |
| `same machine`      | FlatBuffers |
| `different machine` | proto3      |
| `off-board`         | proto3      |

A crossing that is unspecified or unknown gives the unspecified encoding. No
link carries the `repr(C)` encoding: the enum value exists in the schema, and
nothing selects it until driftsys/ridl#317. The choice between a shared-memory
store and a local socket on one machine stays the plugin's. Both carry
FlatBuffers, so the choice does not change a size.

### The depth rule

A `Depth` is an optional value and the source of that value. Three sources are
written.

- **Derived.** An event's depth is the contract bound, `ceil(max / min)` over
  the event's resolved timing
  ([ADR-0015](../decisions/ADR-0015-qos-absorption-and-rpc-bounds.md) decision
  21). The resolved timing includes the default timing of
  [the ridl language reference](../specification/ridl-language-reference.md)
  §9.1, so an event written with no timing has both bounds. The quotient is
  computed on the exact-decimal microsecond strings of `Timing` as integers,
  never in floating point.
- **Underivable.** The value is absent when the timing has an explicit half-open
  range, so one bound is missing, and when an event carries no timing at all. It
  is also absent when the lower bound is zero, so there is no quotient; when an
  operand is not `digits[.digits]`, or is too large to scale; when the quotient
  is zero, because a depth is at least 1; and when the quotient does not fit in
  32 bits. The emitter records every one of these the way it records a missing
  bound.
- **Declared.** The value is the `depth` that an rsdl key states, resolved for
  the link by the precedence of
  [the rsdl language reference](../specification/rsdl-language-reference.md) §5:
  the placement line of the consuming instance, then the deployment. A declared
  value replaces the derived one, whether it is above or below the contract
  bound.

Every consumer link of an event channel carries the declared depth when one
resolves for it, and the member's derived depth otherwise. The channel's ring
depth is the maximum over its consumer links, with the source of the link that
supplies it: on an equal value, the declared link supplies the source. It is
absent when any link's depth is absent. An event channel with no consumer link
takes the member's contract bound as its ring depth.

The resolution is per key, so a placement line that writes `slots` alone still
takes the deployment's `depth` and `budget`.

A **command or query channel** carries on each consumer link the declared
`slots` with the declared source, or sixteen slots with the default source when
none is declared. It carries the declared `budget` with the declared source, or
no budget when none is declared, in which case the budget is absent and its
source is unspecified. A **signal or fixed channel** carries no sizing fields.

### The selection

`ridl build` and `ridlc build` take `--deployment NAME`
([CLI reference](../book/cli-reference.md)). The flag names a deployment of the
workspace's system; a workspace has at most one system.

- A workspace whose source declares exactly one deployment needs no flag: the
  request carries it, unless an `RSDL-7xx` error removed that one deployment
  from the system IR, in which case nothing is carried and the build reports
  that error and exits 1.
- A workspace whose source declares several deployments and no flag carries
  none. The count is the count the source declares, so a deployment an
  `RSDL-7xx` error removed from the system IR still counts: a workspace that
  declares two carries neither.
- A workspace with no system and no flag carries none.
- A `NAME` that the system does not declare exits 2 and names the deployments it
  does declare, unless the build has already drawn an error of its own: that
  error takes precedence, so the build reports it and exits 1.
- A workspace with no system, and a workspace whose system declares no
  `deployment` block, built with `--deployment` are the same bad flag value.
  There is no deployment to name in either, so the message states which of the
  two it is rather than listing nothing.
- A deployment that an `RSDL-7xx` error removed from the system is not unknown:
  the build reports that error and exits 1.

### Bindings, and what is not built

The `bindings` list is empty today. The emitter writes one entry per row of the
table of known bindings, in name order, and the table has no row because no
binding's frame layout is specified (driftsys/ridl#265).

The overhead of a binding is not part of any payload size. A plugin that sizes a
socket message adds the frame header and the envelope of the binding the message
crosses to the payload bound, so a socket message size is unavailable for a
binding the table does not list.

Nothing in the shipped toolchain reads the section: the Rust backend and the two
reference plugins ignore it, and no `--emit` value writes the request out. The
request is built in memory, and what a build hands a plugin is checked by
running a plugin that saves each request it is given
(`crates/ridlc/tests/cli.rs`).

### The layout test plugin

`crates/ridlc/tests/layout.rs` holds `LayoutBackend`, a test plugin that
implements `ridl_ir::codegen::Backend` and reads the request alone: no raw IR
and no file. It computes, for the deployment of `examples/cabin`, the byte
layout of each region and the maximum size of each message that crosses a
socket, and its output is compared with
`crates/ridlc/tests/fixtures/cabin-layout.json`, a fixture in which a comment
beside each number states how the number follows from the request. The test
shows that the request carries every input those two layouts need. The layout
rule belongs to the test plugin: it is stated in the plugin's module comment,
and neither the toolchain nor the request prescribes it.

Two things the cabin fixture does not show are tested apart from it. The fixture
states `null` for every socket message's maximum size, because the table of
binding overheads has no row. A second test adds a `websocket` row to cabin's
request with a frame header of 14 bytes and an envelope of 2 bytes, values that
belong to the test and to no binding document, and checks that `Cabin.warning`'s
maximum size is 14 + 2 + 8 = 24. Declared sizing values are not in the
`examples/cabin` source; another test in the same file builds a workspace in a
temporary directory that declares `depth`, `slots` and `budget` and checks that
each declared value reaches the request with the declared source.

## 8. What a plugin author reads

The book chapter [Writing a codegen plugin](../book/codegen-plugins.md) is the
guide for a plugin author, and its "Reading order" section is the reading list.

## 9. Where the records and the code disagree

1. **ADR-0020 decision 11's text before its amendment, the release-scope note
   §3.8's "Proof without a second language", and issue #322's `Done when` all
   named `ridlc-gen-ts`.** Decision 11 is amended in place; the note is a
   reasoning trail and is not edited; #322 is corrected by comment. The roadmap
   row said "both in-tree backends ported onto it" and is corrected to the Rust
   backend alone, the other three following in their own stories.
2. **ADR-0020 decision 9 and the driver write the request as
   `{version, model, options}`.** The IR specification §7 fixed `schema` and
   `toolchain` instead (§2 above), and `artifact_base` is added. Decision 9 is
   not edited: it summarizes, and the specification is the record it defers to
   for the request's fields.
3. **The sequencing plan names the reference plugin as "a binary that wraps the
   in-process Rust backend".** §6 above: not possible before the Rust backend
   read the model without violating the model-only rule, so the first reference
   plugin wraps `ModelBackend`, and `ridlc-gen-rust` was added beside it.
4. **The design note §8.3 says "no second entry point over the model is added:
   `generate_with` keeps its signature".** True; the trait is a second face over
   the same entry points, not a second entry point, and `generate_with` is
   unchanged.
