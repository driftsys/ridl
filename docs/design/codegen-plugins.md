# The codegen plugin system

The backend contract `generate(CodegenRequest) → CodegenResponse`, its two
hosts, and the reference plugin — roadmap story E4.5b's first half, as built.
The binding choices are
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
[the codegen model design note](../wip/2026-09-22-codegen-model-design.md), and
the sequencing is [the lane P driver](../wip/2026-09-22-lane-p-driver.md), whose
decisions D-P1 (the parity test runs over the Rust backend, not the TypeScript
one), D-P2 (the request carries the model, never the raw IR) and D-P3 (the
plugin never touches the filesystem) this record implements.

**What is built and what is not.** The contract, the in-process host over every
in-tree backend, the process host, the `--plugin` flag and the reference plugin
are built, and the parity test runs under `just test`. What is not built is the
Rust backend as a plugin: it still reads the raw IR, so it cannot be run from a
request, and its port onto the model is the driver's stage P4. The parity test
therefore runs over the reference plugin today — the one backend whose output is
a function of the request alone — and gains `ridlc-gen-rust` when the port
lands. §6 says why this is the honest form of the test and what it does and does
not prove.

## 1. Where the code is

| What                                                                                                 | Where                                                                                                                                                                                                                                                                                                                                                |
| ---------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| The schema: `CodegenRequest`, `CodegenResponse`, `GeneratedFile`, `Diagnostic`, `BackendOption`      | `crates/ridl-ir/proto/ridl/codegen/v1/plugin.proto`, in the package `ridl.codegen.v1` beside `model.proto`, compiled by the same `build.rs` and registered with the same `pbjson-build` builder                                                                                                                                                      |
| The trait `Backend`, the transitional `RawIr`, `ModelBackend`, `SCHEMA`, the encodings, `check_path` | `crates/ridl-ir/src/codegen/contract.rs`, re-exported from `ridl_ir::codegen`                                                                                                                                                                                                                                                                        |
| The four in-tree backends behind the trait                                                           | `crates/ridl-backend-{rust,ts,proto,flatbuffers}/src/contract.rs`, each `pub struct Backend<'a>` over `generate_pipeline` (Rust), `generate` (TypeScript) or `generate_with` (proto3, FlatBuffers)                                                                                                                                                   |
| The in-process host: one request per package, every emit through the trait, the response written     | `crates/ridlc/src/lib.rs` — `codegen_request`, `write_emits`, `write_response`, `run_build_with`                                                                                                                                                                                                                                                     |
| The process host: `ridlc-gen-<language>`, `PATH` lookup, the pipe, the timeout, the error            | `crates/ridlc/src/plugin.rs` — `PluginSpec`, `resolve`, `run`, `PluginError`                                                                                                                                                                                                                                                                         |
| The flags                                                                                            | `crates/ridlc/src/main.rs` and `crates/ridl/src/main.rs`, `--plugin` and `--plugin-timeout` on `build`; documented in [`docs/book/cli-reference.md`](../book/cli-reference.md)                                                                                                                                                                       |
| The reference plugin                                                                                 | `crates/ridlc-gen-model/`, a binary over `ModelBackend`, `publish = false`                                                                                                                                                                                                                                                                           |
| The parity test                                                                                      | `crates/ridlc-gen-model/tests/parity.rs`, over every corpus package at the contract's level and every corpus entry at the command's level; the host's failure modes in `crates/ridlc/src/plugin.rs`'s tests; the flag's behaviour in `crates/ridlc/tests/cli.rs`; the messages' encodings and the path rule in `crates/ridl-ir/src/codegen/tests.rs` |

## 2. The contract

One request per package. In `ridl.codegen.v1`:

```text
CodegenRequest {
  string schema = 1;                  // "ridl.codegen.v1"
  string toolchain = 2;               // the ridlc version that wrote it, "0.2.0"
  Model model = 3;                    // the package's lowered model
  repeated BackendOption options = 4; // {key, value}, sorted by key, keys unique
  string artifact_base = 5;           // what ridlc names this package's files after
}
CodegenResponse {
  repeated GeneratedFile files = 1;   // {path, oneof content {text | binary}}
  repeated Diagnostic diagnostics = 2;// {severity, message}
}
```

**`version` became `schema` and `toolchain`.** ADR-0020 decision 9 and the
driver write the request as `{version, model, options}`. The IR specification
§7, written by stage P1b, replaced the one field with two before this stage
started, with its reason: a bare number does not say which schema it counts, the
package name does, and a toolchain version is a different fact a consumer must
not refuse on. The request follows the specification. A consumer refuses a
`schema` it does not know with a diagnostic naming both values (the reference
plugin does, §5) and does not refuse on `toolchain`.

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
`response_from_json` in `ridl_ir::codegen`). The request's `model` is the
`--emit codegen-model` artifact byte for byte, one indentation level deeper; a
plugin author's fixture is therefore a file `ridlc` wrote, wrapped. The reader
keeps the IR's nesting ceiling of 1,000 JSON levels; the deepest request the
front end admits nests to 265 (the model's 264 plus one for the field), and a
plugin's parser must provision that much. The generated reader rejects an
unknown key, so a plugin that answers with a field this schema does not have is
reported as malformed rather than silently accepted; a plugin's own reader
should be lenient, per the IR specification §8.

**The path rule** every host applies before it writes a file
(`ridl_ir::codegen::check_path`): `/`-separated components, none empty, none `.`
or `..`, no leading `/`, no drive letter, no backslash, no NUL. A response with
one path that breaks it writes nothing, and the refusal names the backend or the
plugin and the path.

## 3. The in-process host

``ridlc::codegen_request(base, package, others,
options)`builds the one request
per package — the model lowered once, over the same`others`every code emit
reads, so the request's model is the artifact — and`write_emits`
hands it to each selected backend through the trait:

```rust
pub trait Backend {
    fn language(&self) -> &str;
    fn generate(&self, request: &v1::CodegenRequest) -> v1::CodegenResponse;
}
```

The signature is the contract's own, over the request alone. Every in-tree
backend is nevertheless still a reader of the raw IR until its port, so each is
constructed with a `RawIr { package, others }` it keeps beside the request and
reads in place of `request.model`:

```rust
let raw = codegen::RawIr { package: ir, others };
let backend: Box<dyn codegen::Backend + '_> = match emit {
    Emit::Rust => Box::new(ridl_backend_rust::Backend::new(raw)),
    Emit::TypeScript => Box::new(ridl_backend_ts::Backend::new(raw)),
    Emit::Proto => Box::new(ridl_backend_proto::Backend::new(raw)),
    Emit::Flatbuffers => Box::new(ridl_backend_flatbuffers::Backend::new(raw)),
    Emit::CodegenModel => Box::new(codegen::ModelBackend),
    Emit::IrJson | Emit::IrText | Emit::IrBinary => /* a direct IR dump, no request */,
};
let response = backend.generate(request);
write_response(out_dir, Origin::InTree { language: backend.language() }, &response, diagnostics)?;
```

So what changes when a backend is ported is how it is built — `RawIr` leaves its
constructor — never how it is called, and the request it is handed today is
already the request a plugin is handed. The three IR dumps are not backends and
take no request; the request is built only when a code emit or a plugin will
read it.

`write_response` records the response's diagnostics — an in-tree backend's
message as it is, since the corpus snapshots pin those messages; a plugin's
prefixed with ``plugin `ridlc-gen-<language>`:`` — and then writes every file,
or none: none when the response carries an error, none when any path fails the
rule. A path with directories in it has them created. The `lib.rs` and
`Cargo.toml` a Rust emit writes are unchanged and still gated on no error
diagnostic being present, which a plugin's error now also is.

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

## 5. The reference plugin

`crates/ridlc-gen-model/` builds `ridlc-gen-model`: `--emit codegen-model` as a
process. It reads one `CodegenRequest` from its standard input, refuses a
`schema` other than `ridl.codegen.v1` with a diagnostic naming both values,
generates through the same `ModelBackend` the in-process host calls — one file,
`<artifact_base>.codegen.json`, the model written back through `to_json_pretty`
— and writes the response to its standard output. Input that is not a request is
a message on standard error and exit 1, which the host reports. It opens no
file. It depends on `ridl-ir` alone.

It is test-only: `publish = false`, not installed by any release, never on a
user's `PATH`. The test suite reaches it as `CARGO_BIN_EXE_ridlc-gen-model`,
which cargo sets for the crate's own integration tests and which is why the
parity test lives in that crate rather than in `ridlc`: no installation, no
`PATH`, no network, under `just test` as it is.

## 6. The parity test, and what it proves

ADR-0020 decision 11, as amended 2026-09-22, makes the parity test the in-tree
Rust backend run as a plugin; the driver's D-P1 says the same. The Rust backend
reads the raw IR until stage P4 ports it, and a plugin has no raw IR (D-P2), so
the Rust backend cannot be run from a request today, and no test can make it
byte-identical through the host before the port. The two ways to run the test
this stage anyway — carry the raw IR in the request during the transition, or
reconstruct the IR from the model in the plugin — were rejected: the first
violates D-P2 and, under the compatibility rule, leaves a field in
`ridl.codegen.v1` that can never be removed; the second is a second lowering in
reverse, thrown away at P4.

So the test runs over `ridlc-gen-model`, on two levels, in
`crates/ridlc-gen-model/tests/parity.rs`:

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

What it proves: the host, end to end — the request rendered and parsed, the
response parsed and written, the file under `--out-dir` with the same name and
bytes, the process started and reaped — and that the model survives a parse and
a re-render byte for byte, which is the canonical form's own conformance
obligation. What it does not prove: that the model is sufficient for a language
backend. That is P4's proof, and it is the same test with `ridlc-gen-rust` in
place of `ridlc-gen-model`, over `--emit rust`, against the same snapshots. The
other three backends stay on the raw IR until their own stories and are not
plugins until then.

## 7. What a plugin author reads

In order: [the IR specification](../specification/ir-specification.md) §7 and §8
(read `schema` first; parse leniently; a fixture is a file `ridlc` wrote),
`crates/ridl-ir/proto/ridl/codegen/v1/plugin.proto` (the two messages),
`crates/ridl-ir/proto/ridl/codegen/v1/model.proto` (what the model carries,
message by message), `crates/ridlc-gen-model/src/main.rs` (a complete plugin in
sixty lines), and the [CLI reference](../book/cli-reference.md)'s `ridl build`
section (how the plugin is found and what `ridlc` does with the response).

## 8. Where the records and the code disagree

1. **ADR-0020 decision 11's text before its amendment, the release-scope note
   §3.8's "Proof without a second language", and issue #322's `Done when` all
   named `ridlc-gen-ts`.** Decision 11 is amended in place; the note is a
   reasoning trail and is not edited; #322 is corrected by comment. The
   roadmap's E4.5b row said "both in-tree backends ported onto it" and is
   corrected to the Rust backend alone, the other three following in their own
   stories.
2. **ADR-0020 decision 9 and the driver write the request as
   `{version, model, options}`.** The IR specification §7 fixed `schema` and
   `toolchain` instead (§2 above), and `artifact_base` is added. Decision 9 is
   not edited: it summarizes, and the specification is the record it defers to
   for the request's fields.
3. **The driver's P3 bullet names the reference plugin as "a binary that wraps
   the in-process Rust backend".** §6 above: not possible before P4 without
   violating D-P2, so it wraps `ModelBackend`, and the Rust wrapper is P4's.
4. **The design note §8.3 says "no second entry point over the model is added:
   `generate_with` keeps its signature".** True; the trait is a second face over
   the same entry points, not a second entry point, and `generate_with` is
   unchanged.
