# ADR-0002: Module System and Package Management

## Status

Accepted.

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

**Rationale — one file shape.** A second file type for workspaces would double
the file count and the file's semantic baggage for no real gain. A section-based
mode flag keeps the vocabulary at one filename. A workspace root `ridl.toml` is
just a `ridl.toml` with `[workspace]` instead of `[package]`.

**Rationale — `[imports]` aliases logical package names.** An earlier sketch
used slash-prefixed aliases (`veh-common/Speed`), which leaked manifest concerns
into source code and conflicted with the `package` naming convention. Aliasing
on logical package names instead means imports in `.ridl` files look identical
whether the package is local, a workspace sibling, or a remote URL — only the
manifest changes. This is also what makes air-gapping work as a pure manifest
rewrite.

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
- Air-gap is a manifest rewrite, not a vendoring exercise.
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
- **Vendor directory as an alternative to a remote URL cache** for fully offline
  or qualified environments where `~/.ridl/cache` is not acceptable. Not
  strictly needed given the URL-rewrite air-gap path, but may simplify some
  certification arguments.

## References

- ADR-0001: RIDL compiler stack and three-layer separation.
- Go module system — package = directory, one module per repo, no wildcards.
- Kotlin — package convention, `internal` visibility default model.
- Rust 2018 edition — explicit `use` paths, Cargo workspaces.
- Deno — URL-addressable imports, content-hashed lockfile, per-user cache.
- Franca, AUTOSAR ARXML, SCADE, Lustre — automotive and synchronous language
  precedents for the `package` keyword and dotted naming.
- ES Modules — qualified, named imports as the post-CommonJS norm.
