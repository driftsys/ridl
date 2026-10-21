# ADR-0002: Module System and Package Management

## Status

Accepted.

Amended 2026-10-04 by the lint foundation design (spec 0,
[ADR-0024](ADR-0024-lint-registry-and-levels.md)): §4 gains the `[lints]` table,
which both manifest kinds accept, and its resolution order.

Amended 2026-10-04 by [ADR-0026](ADR-0026-doc-comments.md) (documentation in the
source, spec 2a): §4 gains root discovery, so an entry inside a workspace member
loads the member's workspace.

Amended 2026-10-06 (driftsys/ridl#741): §4's root discovery names
`[defaults].command_timing` and `[defaults].query_timing` among the defaults a
workspace root applies to a member, beside `[defaults].timing`.

Amended 2026-10-06 by the generated-file marker and header (driftsys/ridl#746):
§4 gains the `[codegen]` table, with the `header-file` key, and the codes
MANI-011 and MANI-012.

Amended 2026-10-09 by the catalog-per-unit design: §1 gains the unit, the tree
of source packages below one `[package]` manifest, and the rule that a source
package belongs to one unit (MANI-014); §4 refuses a `ridl.toml` inside a unit's
tree (MANI-013).

Amended 2026-10-10 by driftsys/ridl#801: §4 gains the `[backend.<name>]` tables,
with the `encodings` key, and the codes MANI-015 and MANI-016.

Amended 2026-10-13 by the remote units design (driftsys/ridl#48, PR 1): §5
states how a remote unit is read and checked (a flat, explicit dependency list;
a fetched artifact is a unit; a remote unit is a read-only dependency), and §7
gains the `tree` hash of the unpacked tree beside the `sha256` of the fetched
artifact, with MANI-105 to MANI-108.

Amended 2026-10-13 by the remote units design (driftsys/ridl#48, PR 2): §4 and
the Positive consequence say that an air-gapped build is `ridl vendor`; §7 gains
`vendor/`, its read order and `ridl vendor --check`, with MANI-017, MANI-018,
MANI-109 and MANI-110, and states that `ridl lsp` and `ridl mcp` read `vendor/`
before the cache; the vendor directory open question is resolved.

Amended 2026-10-22 by the TypeScript wasm codec design (slice 1): §4's
`[backend.<name>]` tables gain the boolean key `wasm`, and MANI-015 covers a
`wasm` that is not a boolean.

## Context

RIDL is an interface description language whose source files declare contracts
that must be shared across teams, components, and tools. Once interface
declarations span more than a single file, the language needs a module system
that answers four questions:

1. How is a name brought into scope from another file?
2. How are files organised on disk and grouped under a logical hierarchy?
3. How are sources distributed, versioned, and reproducibly resolved?
4. How is all of the above mapped to the package or module concepts of every
   target backend (Proto, Rust, Kotlin, AIDL, ARXML, AsyncAPI, DBC, FlatBuffers,
   PlantUML)?

ADR-0001 deferred this decision and identified Deno-style URL imports with a
local cache and a content-hashed lockfile as the distribution model. The
remaining language- and workspace-level decisions are settled here.

The design constraints we held throughout:

- **Strict beats flexible.** RIDL is a contract language for safety-relevant
  systems. One way to do each thing, with hard errors at the boundaries, is
  preferable to multiple equivalent forms.
- **Small surface.** Every keyword added to the module layer is a keyword every
  reader of every `.ridl` file must learn. The bar for adding one is high.
- **Faithful codegen mapping.** The module structure must map cleanly onto every
  supported target backend, so that the package a developer writes in RIDL is
  recognisably the same package that appears in the generated Proto, Kotlin,
  ARXML, and AIDL.
- **Monorepo and standalone repo are both first-class.** Fast Track teams
  legitimately use both shapes. The design must not punish either.

## Decision

The module system is defined by **four keywords**: `package`, `import`, `as`,
`internal`. No others are added. Distribution is handled entirely by manifest,
lockfile, and cache — not by language constructs.

### 1. Package

Every `.ridl` file begins with a single `package` declaration:

```ridl
package veh.common.types;
```

A **package is a directory**. All `.ridl` files in that directory must declare
the same package name, and the package name must mirror the directory path
relative to the manifest root. Mismatch is a hard compile error.

Files within a package are purely organisational — one file may hold many
declarations and one package may span many files. The package is the unit of
visibility, of cycle checking, and of codegen output; the file is not.

**Rationale.** This is the Go, Kotlin, and Java model. It is preferred over the
Rust and OCaml model (file = module) because RIDL is an IDL: real contracts
cluster many related declarations (a struct family, an enum and its companions,
a service and its messages) that naturally belong together but routinely outgrow
a single file. Forcing one declaration per file would impose mechanical
splitting; forcing every file to be its own module would fragment the namespace.
Anchoring the namespace to the directory keeps the grouping unit a level above
the file, where the codegen targets (Proto package, Kotlin package, ARXML
`<AR-PACKAGE>` chain) already operate.

The strict directory-name correspondence is a deliberate Go-ism. It removes an
entire class of confusion — there is exactly one place a package can live, and
exactly one name it can have, given its location.

**The unit** (amended 2026-10-09). A **unit** is one `ridl.toml` with a
`[package]` table plus every source package in its directory tree. A workspace
member is a unit, and so is a standalone manifest. The `name` of the `[package]`
table is the unit name. The manifest's directory is the source package `name`,
and each subdirectory is the source package `name.<path>`, so every source
package of a unit starts with the unit name. The root source package can be
empty: the manifest directory can hold only `ridl.toml` and `interfaces.lock`. A
source package belongs to exactly one unit. Across every unit that one build
loads, a second unit that declares the same source package raises MANI-014
(Error); this covers two unit names where one is a prefix of the other (`veh`
and `veh.cluster`). A member that `members` lists twice is one unit: the loader
skips the repeat before it reads the manifest, so the unit loads once; the
repeated listing raises no diagnostic of its own and duplicates none, so the
member's own diagnostics are raised once. In prose, "package" alone means a
source package and "the package manifest" means the unit. A unit is the unit of
distribution: it produces one catalog when it declares an interface shape (see
[the catalog descriptor](../design/catalog-descriptor.md)), and one
`interfaces.lock` beside its manifest (ridl language reference, §11).

**Alternatives rejected for the unit.** An engine that loads every catalog whose
name starts with a prefix keeps N catalogs and N ports, which is the cost the
unit removes. A wildcard import only brings names into scope and does not move
an interface into another catalog. A separate `[module]` table would keep
"package" to one meaning, but `[package]` already names the unit, as the
`package` attribute of an Android manifest does. A free unit name with a `root`
field is not needed, because the unit name as the root of every source package
makes a clash between two units impossible inside one workspace. The Kotlin rule
(the `package` line decides, the directory is a convention) gives up "one place
a package can live" and lets two units claim one source package. Exclusive unit
kinds were rejected because one unit can hold both contracts and a system, as
`examples/cabin` does. A workspace member nested inside another member's tree is
an error, because `vehicle.cabin.hvac.station` would then belong to two units;
`evals/corpus/vss` is laid out with top-level members for that reason.

### 2. Imports

Imports are qualified and named:

```ridl
import veh.common.Speed
import veh.common.Temperature
```

There are **no wildcards**, **no relative imports**, and **no re-exports**. On
collision, an alias is permitted:

```ridl
import veh.common.Speed
import marine.nav.Speed as MarineSpeed
```

**Rationale — no wildcards.** Wildcards make name resolution non-local: the
reader of a file cannot tell where a name comes from without consulting every
star-imported package. They also make refactoring brittle, because adding a
declaration to an upstream package can silently shadow a name downstream. ES
modules, Rust 2018, and Gleam all banned or strongly discouraged wildcards for
these reasons. RIDL bans them outright.

**Rationale — no relative imports.** Relative paths couple the importer to the
file system layout of the importee. They make moving a file a multi-file edit,
and they make a single declaration's import path different depending on where it
is consumed. Absolute, qualified imports are unambiguous and stable.

**Rationale — no re-exports.** Re-exports hide the true origin of a type behind
an intermediate package, which complicates dependency analysis, audit (where did
this ASIL-D type really come from?), and the codegen targets — Proto and AIDL
would need to materialise re-exports as their own synthetic declarations.
Forcing every consumer to import the type from its defining package keeps the
dependency graph honest and matches what every backend would emit anyway.

**Rationale — `as` only on collision.** The alias keyword exists as a release
valve for the rare case where two upstream packages legitimately expose the same
type name and the consumer controls neither. It is not intended as a stylistic
preference. Linters can flag aliases that are not required by an actual
collision.

### 3. Visibility

Two levels:

```ridl
struct WheelTick { ... }              // public, visible to importers
internal struct RawWheelFrame { ... } // package-private
```

Public is the default; `internal` is the opt-out.

**Rationale.** RIDL is an IDL — the dominant case is "this declaration is a
contract intended to be shared." Requiring `pub` on every public declaration, as
Rust does, would make the common case verbose and the rare case (a private
helper) free, which is backwards for this domain. Kotlin made the same call for
the same reason. Two levels — public and package-private — is enough;
`protected` and `pub(crate)`-style gradations add complexity without paying for
themselves in an interface language.

### 4. Manifest (`ridl.toml`)

The manifest has one file shape and two modes, which are mutually exclusive.

**Standalone package mode** — a single package distributed as a unit:

```toml
[package]
name = "veh.common"
version = "1.2.0"

[imports]
"some.dep" = "https://ridl.example.com/some/dep@v1.0.0"
```

**Workspace mode** — a coordinated set of packages developed together:

```toml
[workspace]
members = ["veh-common", "veh-cluster", "veh-adas"]

[imports]
"third-party.foo" = "https://ridl.example.com/third-party/foo@v1.0.0"
```

Each workspace member directory contains its own `ridl.toml` in
standalone-package mode.

**Root discovery** (amended 2026-10-04, ADR-0026). The toolchain finds the root
to load from the entry — a source file, a package directory or a workspace root
— with one rule, which `ridl check`, `ridl build`, the other commands, the
language server and the MCP server share. The nearest `ridl.toml` at or above
the entry is the start. When it is a `[package]` manifest, the walk continues
upward:

- At the first `[workspace]` manifest the walk stops. That workspace is the root
  when its `members` names the package directory (each member path joined to the
  workspace directory and normalised); otherwise the package is the root and
  stays standalone.
- A `[package]` manifest above the start does not stop the walk.
- A `ridl.toml` that cannot be read or parsed stops the walk and is the root, so
  the load reports why it failed.
- A directory that holds `.git` stops the walk after its own `ridl.toml` is
  checked, and so does the filesystem root. The package is then the root.

An entry inside a workspace member therefore loads the whole workspace: the
root's `[lints]`, `[defaults].timing`, `[defaults].command_timing`,
`[defaults].query_timing` and `[imports]` apply to the member, and its imports
of sibling members resolve. A command that reports diagnostics reports those of
the files under the member only.

**The `[lints]` table** (amended 2026-10-04, lint foundation design, ADR-0024)
sets the level of a lint, in both modes. Each key is a lint name, the name a
Warning or Info row of the diagnostic catalogue carries; each value is one of
`allow`, `info`, `warn` or `deny`:

```toml
[lints]
missing-timing = "deny"
shared-error-type = "allow"
```

The level of a lint for a package is resolved key by key, the later source
winning: the catalogue default, then the `[lints]` table of the workspace root
manifest when the package is a workspace member, then the package's own
`[lints]` table. An entry inside a member loads the member's workspace (root
discovery above), so the root table applies to it. An entry that names no lint,
or whose value is not a level, raises MANI-010 and is ignored.

**The `[codegen]` table** (amended 2026-10-06, driftsys/ridl#746) holds one key,
`header-file`: a path, relative to the manifest that declares it, to a text file
whose lines `ridl build` writes after the generated-file marker in every
generated file that can hold a comment:

```toml
[codegen]
header-file = "LICENSE-HEADER.txt"
```

Only the workspace root's manifest, or a standalone package's manifest, may set
it. The loader reads the file, so every command that loads the manifest reports
a failure, `ridl check` included. It normalises the text: CRLF line ends are
accepted and a lone CR is a line break, each line loses its trailing whitespace,
and leading and trailing blank lines are dropped; a file with no text left means
no header. MANI-011 (Error) is raised when the file cannot be read, when it is
not UTF-8, and when it contains a control character (C0, DEL, C1, U+2028 or
U+2029, other than a tab and a line break); MANI-012 (Error) is raised when the
manifest of a workspace member sets the key. Single-file mode has no manifest
and no header.

**The `[backend.<name>]` tables** (amended 2026-10-10, driftsys/ridl#801;
amended 2026-10-22) hold two keys per backend. `encodings` is an array of the
encoding names the toolchain knows, `flatbuffers`, `proto3` and `repr-c`, which
the backend named by `<name>` emits beside the types. `wasm` is a boolean, false
when absent, that asks the backend for the `wasm` module of the generated crate;
the Rust backend reads it and requires `encodings` to name `flatbuffers`, and a
backend that takes no such option refuses it:

```toml
[package]
name = "veh.cabin"
version = "1.0.0"

[backend.rust]
encodings = ["flatbuffers"]
```

`<name>` is the backend's language name as `ridl build` knows it: `rust` for the
built-in Rust backend, and the `<language>` of a `--plugin <language>=<path>`
for an external plugin. The manifest accepts any `<name>`; a table for a backend
that the current build does not emit is read, validated and otherwise unused. An
absent table, an absent key, and an empty array all mean types only. Only the
workspace root's manifest, or a standalone package's manifest, may hold a
`[backend.<name>]` table. MANI-015 (Error) is raised when `backend` or a
`[backend.<name>]` is not a table, when `encodings` is not an array of strings,
when an element is not a known name, when an element repeats, and when `wasm` is
not a boolean; MANI-016 (Error) is raised when the manifest of a workspace
member holds a `backend` key or `[backend.<name>]` table; any other key under
`[backend.<name>]` is MANI-005. These tables are not the `backend.key` attribute
namespace of rsdl (RSDL-804, which annotates rsdl declarations): the two share
the word and nothing else. Single-file mode has no manifest and so no encodings.

**Rationale — one file shape.** A second file type for workspaces would double
the file count and the file's semantic baggage for no real gain. A section-based
mode flag keeps the vocabulary at one filename. A workspace root `ridl.toml` is
just a `ridl.toml` with `[workspace]` instead of `[package]`.

**Rationale — `[imports]` aliases logical package names.** An earlier sketch
used slash-prefixed aliases (`veh-common/Speed`), which leaked manifest concerns
into source code and conflicted with the `package` naming convention. Aliasing
on logical package names instead means imports in `.ridl` files look identical
whether the package is local, a workspace sibling, or a remote URL — only the
manifest changes. An air-gapped build is `ridl vendor` (§7); rewriting the
manifest to point at a mirror is still possible, because only the manifest
changes.

**No manifest inside a unit** (amended 2026-10-09). A `ridl.toml` in a
subdirectory of a unit's tree raises MANI-013 (Error), and the loader does not
treat that directory as a separate package root. A workspace member's directory
may not sit inside another unit's tree; where it sits otherwise is the
workspace's choice.

**Rationale — workspace nesting forbidden.** Cargo permits nested workspaces;
the feature is rarely used correctly and creates ambiguity when tools try to
find "the" workspace root. One level keeps resolution deterministic. Teams that
genuinely need two levels of grouping can split into two repos.

### 5. Resolver

When `veh.cluster` writes `import veh.common.types.Speed`, the compiler resolves
the reference in this fixed order:

1. Is `veh.common` (or any longer prefix that matches) a workspace member? Use
   it locally from the workspace.
2. Is `veh.common` aliased in this `ridl.toml`'s `[imports]`? Use that URL.
3. Is `veh.common` aliased in the workspace root's `[imports]`? Use that URL.
4. Otherwise, error.

**Rationale.** Locality wins, then narrower scope, then broader scope. Workspace
members are checked first because in a monorepo the local copy is always the
intended target — falling through to a URL when a member exists would be a
silent bug. Per-package `[imports]` shadow workspace `[imports]` so that a
member can pin a specific dependency version without rewriting the whole
workspace, while the workspace `[imports]` provides the shared default for
everything else.

**Remote units** (amended 2026-10-13, driftsys/ridl#48). The consumer's
`[imports]` — the union of the root manifest's table and every member's — lists
every remote unit the build needs, indirect ones included. A remote unit's own
`[imports]` are read only to check that the consumer names each key; a key the
consumer does not name is MANI-106 (Error), and when the two URLs for a key
differ the consumer's URL is used, with no diagnostic. When two members pin one
key to different URLs, both URLs are materialized and read; the second unit is
not loaded and draws MANI-014.

A fetched artifact is a unit (§1): its top level holds a `[package]` `ridl.toml`
whose `name` equals the `[imports]` key that names its URL. Otherwise — no
manifest at the top, a `[workspace]` manifest, or another name — the unit is
MANI-105 (Error) and is not loaded; when two keys name one URL, the key that is
not the name is MANI-105 and binds nothing. The ordinary rules apply to a remote
unit unchanged: MANI-014, the directory-equals-package rule, and the
cross-package cycle check across the boundary between the consumer and the unit.

A remote unit is a read-only dependency. Its Errors are reported with the unit
name and the file path, and fail the command; its Warnings and Infos are dropped
before any `[lints]` level is applied, and the consumer's `[lints]` do not apply
to it. Its packages are given to the code backends and generated like a
member's; no IR snapshot, catalog, `interfaces.lock`, property run or diff side
is produced for it, and `ridl fmt` never touches it. `internal` declarations of
a remote unit are not visible to the consumer. A remote package is reached
through the key that pins it: from a member, through the member's own
`[imports]` (step 2) or the workspace `[imports]` (step 3), so a sibling
member's pin is never visible. A package of a remote unit resolves a reference
through step 1, which finds the consumer's workspace members and the packages of
its own unit, then through its own `[imports]` (step 2); the workspace
`[imports]` (step 3) apply to workspace members only, so a remote unit never
uses a remote unit its own manifest does not declare. Step 1 finds the
consumer's members so that the cycle check across the boundary sees both sides.
A qualified reference and a doc link apply the same visibility as an `import`.

### 6. Cycles

Cycles within a package are permitted. Cycles across packages are a hard error.

**Rationale.** Within a package, mutual references are normal — a struct
referring to an enum referring back to that struct is the reality of any
non-trivial type system, and the package is already loaded as a unit. Across
packages, a cycle is an architectural smell: it means two packages are not
actually separable, and tools (codegen, audit, dependency analysis, version
pinning) would have to treat them as if they were one anyway. Go, Cargo, and
Kotlin all forbid cross-package cycles for the same reason. RIDL adopts the same
rule.

### 7. Lockfile and cache

A `ridl.lock` file lives at the workspace root, or at the package root if the
repo is a standalone package. It pins every remote import to its SHA-256 content
hash. It is regenerated by `ridlc` on every successful resolution and verified
strictly under `ridlc --frozen` for CI.

The local cache lives at `~/.ridl/cache`, indexed by URL and content hash. A
cached entry is never re-fetched as long as the hash on record matches.

**The `tree` hash** (amended 2026-10-13, driftsys/ridl#48). Each `ridl.lock`
entry pins two hashes: `sha256`, over the fetched artifact, and `tree`, over the
unpacked tree. The `tree` hash is defined once, in `ridl-core`: the input is
every regular file under the unit's directory, in ascending byte order of its
relative path written with `/` separators; for each file the hash is fed the
path bytes, one NUL byte, the file length as a little-endian u64, and the file
bytes; a symbolic link or any other non-regular file is an error (MANI-107,
naming the file) and no tree that holds one is loaded; the result is the SHA-256
in lowercase hex. A fetch against an entry that pins `tree` verifies the
unpacked tree against it, as a cache hit does (MANI-107 on a mismatch); a fetch
against an entry with no `tree` computes it after unpacking and pins it. An
entry written before this amendment has no `tree`: the next run without
`--frozen` adds it, and a run with `--frozen` raises MANI-108. `ridl lsp` and
`ridl mcp` never fetch: they read `vendor/` and then the cache as a frozen run
does (the next paragraph gives the order), and a miss is MANI-103, whose message
names `ridl check` as the fix, or MANI-104, whose message names `ridl check` and
`ridl vendor`. The tar format (ADR-0007 decision 12) stays provisional.

**The vendor directory** (amended 2026-10-13, driftsys/ridl#48).
`<manifest root>/vendor/` — beside `ridl.toml` and `ridl.lock`; the workspace
root in workspace mode — holds the source tree of each remote unit under its
`[imports]` key, written by `ridl vendor`. For each URL of the consumer's
`[imports]` the loader takes the first of: `vendor/<key>/` when that directory
exists and `ridl.lock` pins a `tree` for the URL (the tree must match,
MANI-107); the cache entry for the pin; outside `--frozen`, a fetch. Where
`ridl.lock` pins no `tree` for the URL, the cache or the fetch is used first so
that a pin exists, and a vendored directory is then verified against that pin
and read in place of the cache entry. This order holds for `ridl check`,
`ridl build`, `ridl baseline`, `ridl lock`, `ridl test`, `ridl diff`, `ridl lsp`
and `ridl mcp`; `ridl vendor` itself reads the cache, never `vendor/`. The name
`vendor` is reserved at the manifest root only: a `members` entry that is
`vendor` or a path under it is MANI-017 (Error), a source file under a
standalone root's `vendor/` outside a directory that an `[imports]` key names is
MANI-018 (Warning, `vendor-source-not-loaded`) and is not loaded, and a deeper
directory named `vendor` is an ordinary source package. A unit's own `vendor/`
directory is copied with the unit, because it is part of the tree hash, and is
skipped when the unit is read. `ridl vendor [PATH]` brings the directory to
exactly the imported set — each unit written whole into a staging directory and
renamed into place; every directory directly under `vendor/` that no `[imports]`
key names removed, whether or not `ridl vendor` wrote it, while files, symbolic
links and hidden entries are left; `vendor/` left untouched when the run has an
error, while `ridl.lock` may still be updated as any non-frozen load updates it
— and reads the cache, never `vendor/`, so a changed URL replaces the directory.
`ridl vendor --check [PATH]` compares the directory with the imports and the
lock without network or writes: MANI-109 (Error) for an imported unit with no
directory, MANI-110 (Error) for a directory no key names, MANI-107 for a tree
that does not match, and MANI-103 or MANI-108 for a unit whose lock entry is
missing or has no `tree`, because such a directory cannot be verified.
`ridl.lock` and `vendor/` are part of the frozen manifest surface from 0.10.0
([ADR-0029](ADR-0029-stability-policy.md)).

Alternatives considered: a vendor directory holding the tar files, or both tars
and trees (a review would show a binary file, and two copies can drift);
replacing `sha256` with the tree hash, or a second pin file under `vendor/` (the
first loses the verification of a download before it is unpacked, the second can
disagree with `ridl.lock`); a hidden `.ridl/vendor/` (no loader change, but not
the convention users expect); a `vendor` reserved at every depth, or detected by
content (the first takes a package segment from every unit, the second makes the
meaning of a directory depend on its contents); a `--frozen` mode for
`ridl vendor` (no stated need), and no `--check` (CI would have no way to check
`vendor/` without a full compile); and leaving `ridl.lock` and `vendor/`
unfrozen (a 0.10.x release could then break a repository that vendored under
0.10.0).

**Rationale.** This is the model from ADR-0001, with the only refinement being
placement: one lockfile per workspace (not per member) so that
diamond-dependency resolution is coherent across all members. Per-member
lockfiles would let two siblings pin conflicting versions of the same upstream,
which defeats the point of pulling them into one workspace.

### 8. Codegen mapping

The package name maps directly onto each backend's native concept:

- Proto: `package veh.cluster;`
- Rust: `mod veh { mod cluster { ... } }`
- Kotlin: `package veh.cluster`
- AIDL: `package veh.cluster`
- ARXML: nested `<AR-PACKAGE>` elements following the dot-segments
- AsyncAPI: namespace prefix on schema component names
- DBC: namespace prefix on message and signal names (DBC has no native package
  concept)

`internal` declarations are emitted with the target language's package-private
mechanism where one exists (Kotlin `internal`, Rust `pub(crate)`). Where none
exists (Proto, AIDL, ARXML, DBC), `internal` declarations are omitted from
cross-package generated artifacts entirely — they are visible only to backends
targeting the same package.

**Rationale.** A one-to-one mapping is what makes RIDL legible at the boundary:
a developer reading generated Proto sees `package veh.cluster` and recognises it
as the same package they wrote in `.ridl`. Configurable per-target package
mapping was considered and rejected — it would let teams paper over namespace
mismatches across backends, and the resulting artifacts would be harder to audit
against the RIDL source.

## Consequences

### Positive

- The module-layer surface area is four keywords. Every reader of a `.ridl` file
  can hold the entire module model in their head.
- Strict directory-to-package correspondence makes "where does this type live?"
  answerable from the import statement alone.
- Single lockfile per workspace gives coherent diamond resolution across all
  members of a monorepo.
- Air-gap is `ridl vendor`: the vendored trees are verified against `ridl.lock`
  and read before the cache. Rewriting the manifest to point at a mirror stays
  possible.
- Standalone-package and workspace shapes use the same file format, reducing
  tool and documentation overhead.
- The codegen mapping is one-to-one to every supported backend, which preserves
  the legibility of generated artifacts against the RIDL source.

### Negative / accepted trade-offs

- No re-exports means a consumer that uses a transitively-referenced type must
  import its defining package directly. This makes the import list longer in
  practice but is necessary to keep the dependency graph audit-friendly.
- No wildcards means longer import sections in files that consume many types
  from one upstream package. Acceptable; an explicit list is cheap to read and
  far cheaper to refactor than a wildcard.
- `internal` declarations cannot be expressed in backends that lack a
  package-private mechanism (Proto, AIDL, ARXML, DBC). The chosen remediation —
  omit them from those backends' outputs — means a declaration's surface differs
  across targets. This is judged better than emitting them with a synthetic
  naming convention that would leak the privacy boundary into the wire format.
- One level of workspace forbids nested grouping. Teams that hit the limit must
  split repos. Acceptable.
- The strict directory-package correspondence forecloses the case where one
  logical package is split across non-adjacent directories. There is no
  compelling reason to support that case in an IDL.

## Open questions

The following are deferred to subsequent ADRs.

- **Diamond-conflict policy under semver.** The lockfile pins by content hash,
  but when two transitive imports demand different versions of the same
  upstream, the resolver needs a policy. Strict pinning with manual override, or
  semver-aware unification, or fail-and-force-the-user-to- decide. Likely the
  third, with tooling support — but worth its own ADR.
- **Path-based local overrides for development.** A `[replace]` mechanism, à la
  Cargo, that lets a developer point a published dependency at a local checkout
  for the duration of a feature branch, without modifying the lockfile
  permanently. Useful in monorepo-of- monorepos workflows; not yet specified.
- **Behavior of `internal` per backend in detail.** The principle is set; the
  per-backend mechanics need a separate ADR with examples for each target.
- **Cache eviction and integrity verification policies.** The cache is
  content-addressed and effectively immutable, but disk pressure, corrupted
  entries, and CI-environment cache seeding all need a defined story.
- **Vendor directory as an alternative to a remote URL cache** (**Resolved
  2026-10-13**, §7, driftsys/ridl#48) for fully offline or qualified
  environments where `~/.ridl/cache` is not acceptable. Not strictly needed
  given the URL-rewrite air-gap path, but may simplify some certification
  arguments.

## References

- ADR-0001: RIDL compiler stack and three-layer separation.
- Go module system — package = directory, one module per repo, no wildcards.
- Kotlin — package convention, `internal` visibility default model.
- Rust 2018 edition — explicit `use` paths, Cargo workspaces.
- Deno — URL-addressable imports, content-hashed lockfile, per-user cache.
- Franca, AUTOSAR ARXML, SCADE, Lustre — automotive and synchronous language
  precedents for the `package` keyword and dotted naming.
- ES Modules — qualified, named imports as the post-CommonJS norm.
