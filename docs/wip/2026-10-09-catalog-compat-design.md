# Catalog compatibility — option A: the compatible catalogs list

Created 2026-10-09, lane H stage H2 (ruling R-1 of the lane H driver,
`2026-10-09-lane-h-driver.md`, on pull request #784). Every decision in this
note was delegated; each one is recorded under [Rulings](#rulings) as
`what — why — cost if wrong`. The plan that builds it is
[`2026-10-09-catalog-compat-plan.md`](2026-10-09-catalog-compat-plan.md).

## Problem

Two tools answer "can this consumer still talk to this provider", and today they
disagree.

- `ridl diff` classifies a change between two snapshots and exits 0 for a
  compatible one: "a consumer built against the old snapshot still works"
  (`crates/ridl-diff/src/lib.rs:45-52`). An appended interaction
  (`InteractionAppended`), a new interface or declaration (`DeclAdded` at the
  package level), a doc change, a tombstoned retirement, an interface rename or
  retirement, and a service's set change are all compatible
  (`crates/ridl-diff/src/classify.rs:62-120`).
- The frame specification §6.1 accepts an `attach` only when the consumer's
  `CatalogRef` equals the provider's, name **and** hash
  (`docs/specification/frame-specification.md` §6.1, as it read before this
  change), and the catalog hash changes with any of the changes above, because
  it covers every interface of the unit and every declaration those interfaces
  reach ([ADR-0014](../decisions/ADR-0014-ir-encodings.md) decision 15,
  `crates/ridl-ir/src/catalog_hash.rs:56-123`).

So a deployed consumer can never attach to a provider that was upgraded by a
compatible change: the change is compatible in `ridl diff` and incompatible on
the wire. Ruling R-1 of the lane driver chose option A: the catalog descriptor
lists the earlier catalog hashes the toolchain judged compatible with the
current catalog, and a provider accepts an `attach` that names one of them.

This note decides the six questions the H2 brief lists. Every claim about
current behaviour cites the code at the commit this note was written from
(`fd374f71`).

## Terms

- **Catalog**: one unit's interfaces, identified by `CatalogRef { name, hash }`
  (`crates/ridl-rt/src/contract.rs:29-40`). The hash is derived on every build
  and never recorded (ADR-0014 decision 15, first paragraph).
- **Baseline**: the published snapshots under `.ridl/baseline/`, one
  `<pkg>.ir.json` per source package, written by `ridl baseline`
  (`crates/ridl/src/main.rs:632-700`) and found by `ridl check` through
  `default_baseline_dir` (`crates/ridl/src/main.rs:1171-1183`).
- **Verdict**: `ridl_diff::Verdict`, `Identical < Compatible < Breaking`, the
  maximum over a report's changes (`crates/ridl-diff/src/lib.rs:42-52`).
- **The compatible catalogs list** (the list): the earlier catalog hashes a
  provider accepts at `attach` beside its own.
- **Consumer, provider**: the two sides of a session (frame specification §3).
  The consuming side opens the session with `attach`.

## Question 1 — where the list comes from

### What exists today

- `ridl baseline` publishes the workspace's snapshots wholesale, after two
  gates: the tombstone gate (`untombstoned_removals`,
  `crates/ridl/src/main.rs:737`) and the lock's interface gate
  (`interface_refusals`, `crates/ridl/src/main.rs:929`). It refuses a
  provisional interface number (RIDL-411), so a published baseline carries only
  frozen numbers. It does not classify the change it publishes, and it records
  no hash.
- `ridl diff` loads each side through `ridlc::load_diff_side` (a snapshot
  directory or a source tree, `crates/ridlc/src/diff_side.rs:9-12`) and
  classifies with `ridl_diff::diff_workspaces`
  (`crates/ridl/src/main.rs:458-494`). The verdict is one value for the whole
  report: the changes of every package of the workspace, with `ridl.std` as
  context (`crates/ridl-diff/src/lib.rs:373-440`).
- `ridl build` computes the catalog hash at every build, in
  `ridlc::run_build_with` (`crates/ridlc/src/lib.rs:795`): the descriptor is
  written by `write_catalogs` over `catalog_scope`
  (`crates/ridlc/src/lib.rs:934` and `:1058`, `:1995-2014`); the codegen model's
  `Catalog.hash` is computed by `ridl_ir::codegen::lower`
  (`crates/ridl-ir/src/codegen/lower.rs:1092-1113`); each region's hash is
  embedded by `embed_catalog_hashes` (`crates/ridlc/src/lib.rs:1500`). The build
  reads no baseline.
- A change's path is `<package>/<declaration>[/<member>…]`
  (`crates/ridl-diff/src/walk.rs:80`, `crates/ridl-diff/src/lib.rs:222-225`), so
  a change can be attributed to the package and the top-level declaration it
  sits in. `ridl_ir::catalog_hash::reachable_decls(unit, packages)` returns
  every declaration a unit's interfaces reach, keyed by canonical name
  (`crates/ridl-ir/src/catalog_hash.rs:30`).

### The decision

The lock and baseline workflow is the source, through `ridl diff`'s classifier.
Two steps record and read the list.

**`ridl baseline` records the chain.** When it publishes, it writes one file per
unit that has an interface shape, `<unit>.catalogs`, beside the snapshots in the
published directory. The file holds hashes, one per line, 64 lowercase hex
characters each, newest first, after a header comment line:

```text
# catalogs of this unit's published baselines, newest first, back to the last breaking change
3f0c…a1   <- the hash of the catalog this publication publishes
9b2e…77   <- the hash of the baseline it replaced, carried over because the change was compatible
```

The first line is the hash of the catalog being published, computed at
publication over the same scope a build uses (`ridlc::catalog_scope` with
`ridl.std` when a package references it, then
`ridl_descriptor::hash::catalog_hash`). The lines after it are carried over from
the replaced baseline's `<unit>.catalogs` file when, and only when, the unit's
verdict between the replaced baseline and the fresh snapshots is `Compatible` or
`Identical`. When the verdict is `Breaking`, when the replaced baseline has no
`<unit>.catalogs` file, or when there is no replaced baseline (a first
publication), the file holds the one new hash. A hash already in the list is not
written twice. The `.catalogs` files are published wholesale with the snapshots:
the directory ends up holding exactly one per unit with a shape, and a stale one
is removed.

**`ridl build` reads the chain, and `ridlc` writes what it is given.** The
`ridl` facade discovers the baseline the way `ridl check` does
(`.ridl/baseline/` under `ridl_core::find_root`, no flag), for every build that
writes a catalog descriptor or lowers a codegen model. When the directory holds
snapshots and a `<unit>.catalogs` file for the unit, `ridl build` compiles the
workspace (as `ridl check` compiles it a second time for its baseline
comparison, `crates/ridl/src/main.rs:1187-1200`), classifies baseline → current
for that unit with `ridl_diff`, and builds the unit's list: when the verdict is
`Compatible` or `Identical`, every hash of the file except the current catalog's
own hash; when it is `Breaking`, nothing. It passes the per-unit lists to
`ridlc::run_build_with` as an input of the build, and `ridlc` writes them into
the descriptor and the codegen model without reading any baseline itself. With
no baseline directory, no snapshot, or no `<unit>.catalogs` file, the list is
empty, which is today's behaviour: the provider accepts its own hash and nothing
else. `ridlc build` always passes an empty list, because the compiler is a pure
source → IR function and reading a workspace-local baseline is workflow, outside
the tool-qualification boundary
([ADR-0008](../decisions/ADR-0008-e2-execution.md) decisions 9 and 14,
`crates/ridl/src/main.rs:23-27`). A baseline directory that is present and
cannot be loaded (a malformed snapshot, a refused encoding) is the error
`ridl check` reports for it, exit 2, because a provider built over a baseline
the toolchain cannot read would silently refuse every deployed consumer.

`ridlc::load_diff_side` (`crates/ridlc/src/diff_side.rs:150`) does read a
snapshot directory, and it sets no precedent for discovery inside `ridlc`: it
loads a path its caller names as an input — the `ridl` facade's `diff` and
`check` — the way the compiler loads a source tree, and `ridlc`'s own CLI never
calls it. The dependency `ridlc` → `ridl-diff` that it needs for `load_ir_json`
(`crates/ridlc/Cargo.toml:17`) does contradict the letter of ADR-0008 decision
9's 2026-07-26 extension, which says `ridl-diff` is a dependency of nothing
`ridlc` compiles; that drift predates this design and is recorded as
driftsys/ridl#786, not changed here.

**The verdict is per unit.** `ridl diff`'s report verdict is workspace-wide. A
new function, `ridl_diff::unit_verdict(report, unit, old, new) -> Verdict`,
takes the maximum over the changes that concern the unit: a change whose package
(the first path segment) is a source package of the unit on either side, and a
change whose top-level declaration (`<package>.<declaration>`, the first two
path segments) is in `reachable_decls(unit, old)` or
`reachable_decls(unit, new)`. A breaking change to a struct in another unit that
one of this unit's interfaces reaches resets this unit's chain, because that
struct is part of this unit's wire contract and of its hash; a breaking change
in a unit this unit does not reach does not.

**The reset on a breaking change.** A breaking verdict at publication writes a
file with one hash; a breaking verdict at build time emits an empty list. A
consumer built against a pre-break catalog is therefore refused with
`catalog_mismatch`, as today. The chain restarts from the first compatible
publication after the break.

**Transitivity.** The list carries hashes judged link by link: each baseline was
judged against the one it replaced, and the current build is judged against the
current baseline. A consumer at the oldest hash in the list attaches to the
newest provider on the strength of the composition of those verdicts. Every
compatible category is monotone — an append stays appended, a tombstone is never
redeclared (`ReservedNameRedeclared` is breaking), a widened constraint or
loosened timing is judged by direction — so the composition of compatible
changes is compatible. The design relies on that property and states it here
rather than keeping every earlier baseline to diff against directly.

**Recorded, not recomputed.** The hash of an earlier catalog is the value the
publishing toolchain computed and wrote; a later build never re-hashes a
snapshot. A deployed consumer carries the hash its own toolchain computed, and a
toolchain upgrade that changes the IR schema moves every hash (ADR-0014 decision
15, the golden-hash test), so a recomputed value would not be the consumer's.
The cost is a migration step: a baseline published before this change has no
`<unit>.catalogs` file, so the chain starts at the next `ridl baseline`.

### What the build does not do

- It does not require a baseline. A workspace that never publishes one gets
  today's exact-match behaviour.
- It does not change `ridl diff`'s output or exit code. `unit_verdict` is a
  library function; the workspace verdict stays what the user sees.
- It does not list a provisional catalog. A baseline never holds a provisional
  number (RIDL-411), so a consumer built from a tree with a provisional number
  carries a hash no file records and is refused. The current build may itself be
  provisional (a new interface with no lock entry yet): its own hash then
  differs from the baseline's, `ridl diff` reports the new interface as
  compatible, and the list carries the baseline's hash, which is correct: the
  deployed consumer knows nothing of the new interface.
- It does not touch driftsys/ridl#700. A `ridl lock` that freezes a provisional
  number changes the hash and `ridl diff` says `identical`; under this design
  that is harmless — the baseline was never provisional, so the frozen current
  build lists the baseline's hash as it should.

### Alternatives considered

| Alternative                                                                                                       | Why not                                                                                                                                                                                                                                                                                                                                                                    |
| ----------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Record the chain in `interfaces.lock`                                                                             | The lock is the number allocator, one line table with a merge driver (`crates/ridl-core/src/interface_lock.rs:10-25`); hashes are not numbers, and the merge driver would have to merge a list it cannot judge. The baseline is the published contract; the chain is a fact about it.                                                                                      |
| Keep every earlier baseline (`.ridl/baseline/<hash>/`) and diff the current tree against each                     | Exact instead of transitive, but the baseline directory grows with every release, `ridl check`'s discovery and the publication gates all assume one published snapshot per package, and the compatible categories are monotone, so the exactness buys nothing today.                                                                                                       |
| Recompute the earlier hash from the snapshot at build time instead of recording it                                | Wrong after a toolchain upgrade that changes the IR schema: the consumer holds the hash its toolchain computed. The golden-hash test shows a snapshot re-hashes stably under one toolchain, not across two.                                                                                                                                                                |
| The workspace-wide verdict decides every unit's chain                                                             | Simpler, and safe, but a breaking change in one unit would refuse the deployed consumers of every other unit in the workspace. The per-unit verdict is a filter over the same report and costs one function.                                                                                                                                                               |
| A hand-written list in `ridl.toml`                                                                                | A second source of truth beside `ridl diff`, the thing ridl §11 rejected for a version block: a hand-maintained list drifts, and a hash is not something a person writes.                                                                                                                                                                                                  |
| Discover the baseline inside `ridlc::run_build_with`, so `ridlc build` and `ridl build` write the same descriptor | Rejected by ruling R-4 of the main session: reading a workspace-local baseline is workflow, and ADR-0008 decisions 9 and 14 keep it in the `ridl` facade so that `ridlc` stays the pure source → IR function the tool-qualification argument covers. `ridlc build` writes an empty list, and the descriptor's bytes depend on which binary wrote them in that field alone. |
| Let `ridl build` list the baseline hash without classifying, and let `ridl baseline` do all judging               | The tree between two publications can be breaking relative to the baseline (a type changed, not yet published); a provider built from it would accept consumers it cannot serve. The build must judge its own tree.                                                                                                                                                        |

## Question 2 — where the list lives

### What exists today

- The catalog descriptor's root table `Catalog` holds `version`, `name`, `hash`,
  `toolchain`, `interfaces` and `retired`
  (`crates/ridl-descriptor/schema/catalog.fbs:96-108`). The schema's rule is
  append-only: a field is added at the end of its table and never removed,
  reordered or retyped, and a reader built against an older schema ignores a
  field it does not know (the schema header). `SCHEMA_VERSION` is 1
  (`crates/ridl-descriptor/src/lib.rs:47`); `verify` rejects any other version
  and then walks every field once (`crates/ridl-descriptor/src/lib.rs:120-181`).
  A planus accessor of a non-required field returns `Option`, `None` when the
  file does not carry it (`crates/ridl-descriptor/src/generated.rs:2386-2390`,
  `timing`), which is what makes the reader lenient.
- The codegen model's `Catalog` holds `package`, `hash` and `retired`
  (`crates/ridl-ir/proto/ridl/codegen/v1/model.proto:734-744`); a plugin reads
  the catalog from it and computes nothing (the archived per-unit design,
  decision 2).
- The generated Rust face carries, per interface, `CATALOG`, `NUMBER`,
  `PROVISIONAL`, `NAME` and `MEMBERS` through `ridl_rt::contract::Interface`
  (`crates/ridl-rt/src/contract.rs:59-71`,
  `crates/ridl-backend-rust/src/descriptors.rs:138-141`). The face does not
  carry the retired list; a runtime's view of retirement is the descriptor's.
- The face's catalog check is `CatalogRef` equality, once per binding, and a
  mismatch panics
  ([ADR-0023](../decisions/ADR-0023-interaction-face-generation.md) decision 8,
  `docs/design/interaction-face.md`, "The catalog check"). It is local to one
  program: the port and the face were built by the same program.
- The one runtime in this workspace, `ridl-loopback`, holds no descriptor and
  compares nothing (`crates/ridl-loopback/src/lib.rs:62-66` and `:99-105`); its
  `attach` makes a second aggregate over the same store for the same catalog
  (`crates/ridl-loopback/src/lib.rs:252`). Nothing in this workspace speaks the
  frame's `attach`.

### The decision

The list lives in the **catalog descriptor** and in the **codegen model**, and
nowhere else.

- **The descriptor.** `Catalog` gains one field appended at its end:
  `compatible: [EarlierCatalog]`, where
  `table EarlierCatalog { hash: [ubyte] (required); }` — a table, because
  FlatBuffers has no vector of vectors. The field is **not** `required`, so a
  file written before this change still verifies, and an engine built against
  the older schema ignores it. `SCHEMA_VERSION` stays 1: the version names the
  set of fields a reader must know, and a reader that does not know this one
  reads the file correctly, as the version rule intends. `verify`'s walk touches
  the new vector and each entry's `hash` when the field is present, as it
  touches each `retired` entry; it checks no length, as it checks none for
  `hash` today (`crates/ridl-descriptor/src/lib.rs:140`). The lowering always
  writes the field, empty when the list is empty. `ridl describe` prints it as
  `"compatible": [[…], …]`, each hash an array of bytes like `hash`, and an
  empty array when the file carries no entry or does not carry the field.
- **The codegen model.** `Catalog` gains `repeated bytes compatible = 4`, in the
  order of the descriptor's list, so that a plugin which generates for a runtime
  that reads no descriptor can emit the list itself.
- **The generated Rust face is unchanged.** The face's `CATALOG` is the identity
  of what the face was generated from, and its check (ADR-0023 decision 8) stays
  `CatalogRef` equality: a face and a port built by one program must agree
  exactly, and an earlier hash has no place in that comparison. The Rust backend
  emits nothing for the list, as it emits nothing for `retired`: both are facts
  about the unit's history that a runtime reads, not facts about the face. When
  a Rust runtime that speaks the frame exists (the WebSocket transport,
  driftsys/ridl#265), it takes the list from the codegen model the way it will
  take `retired`, through whatever the backend then emits for both.

### Alternatives considered

| Alternative                                                                      | Why not                                                                                                                                                                                                                                                                           |
| -------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| A `compatible` constant on `ridl_rt::contract::Interface`, emitted in every face | A new associated constant with no default is a breaking `ridl-rt` change (ADR-0021 decision 10); with a default of `&[]` it is dead in every face today, because no face checks anything but equality, and it would repeat the unit's list in every interface module of the unit. |
| Bump `SCHEMA_VERSION` to 2                                                       | Every engine built against version 1 would refuse every new file, including files whose list is empty. The schema's append-only rule exists so that an appended field does not cost a version.                                                                                    |
| `[ubyte]` of 32×n bytes, one flat vector                                         | Saves one table per entry, but a reader must know the stride, and the entry is not addressable as a value. One table per hash mirrors `retired`: one vector of tables, one entry per row.                                                                                         |
| The list in the lowered system's `Region` beside the region hash                 | The system is per deployment; the list is a fact about one catalog's history, and a system descriptor is not built (`docs/design/catalog-descriptor.md`, "Not built"). The region can gain it when a reader needs it there.                                                       |
| The descriptor only, not the model                                               | A plugin cannot read a descriptor without a FlatBuffers runtime; the model is the plugin's one input (`docs/design/codegen-plugins.md`). `retired` set the precedent: it is in both.                                                                                              |

## Question 3 — what a provider does after it accepts an earlier hash

### What exists today

§6.1 says the catalog check "is the peers' agreement on the whole contract,
taken once": two runtimes attached under one `CatalogRef` hold the same
interfaces, ordinals and kinds, so a frame naming an unknown one is one side's
defect (§6.1 as it read before this change). §6.4 lists what a receiver does
with such a frame, and names a retired interface and a `reserved` ordinal as
unknown for that purpose. §4 makes the receiver check `kind` against its own
catalog ("Why `kind` is on the frame at all").

Every data-plane frame from the provider answers a frame from the consumer:

- a `publish` follows a `subscribe` (§6.2) or a `read` (§6.3) of that
  `(interface, ordinal)`;
- an `occurrence` follows a `subscribe` of that event, and only occurrences
  raised after the `answer` are delivered (§6.2);
- a `response` answers a `request` and carries its `seq` as `correlation` (§5.3,
  §5.4);
- an `answer` answers a `subscribe` or a refused `read` and carries the same
  identity (§6.2, §6.3).

A runtime "may subscribe every signal of an interface at attach" (§6.2, last
paragraph): that is the consuming runtime, over its own catalog.

### The decision

The invariant that replaces §6.1's is:

> The consumer's catalog is the provider's catalog, or an earlier catalog the
> provider's descriptor lists as compatible with it. Every interaction the
> consumer's catalog holds, the provider's catalog holds under the same
> interface number, ordinal and kind, or has retired since. The provider's
> catalog may hold interactions the consumer's does not. The consumer never
> names an interaction its own catalog does not hold, and the provider never
> sends a frame for an interaction the consumer did not name on the session.

Three consequences are written into §6.1:

1. **The provider needs no copy of the consumer's catalog.** It sends a
   data-plane frame only in answer to a `subscribe`, a `read` or a `request` of
   the session, and every one of those names an `(interface, ordinal)` the
   consumer chose from its own catalog. The set of interactions a provider may
   send on a session is the session's subscription set plus its requests in
   flight, which the provider already keeps per session (§3: subscriptions and
   sequence counters are the session's). An event appended after the consumer's
   hash is never subscribed by that consumer, so its occurrences are never sent
   to it. A consuming runtime that subscribes at `attach` subscribes the signals
   of its own catalog.
2. **`UnknownInteraction` from the provider is a designed answer on such a
   session, not a protocol error.** An older consumer can name an interaction
   that a later catalog retired — a `reserved` tombstone or a retired interface,
   both compatible in `ridl diff` (`crates/ridl-diff/src/classify.rs:96` and
   `:104`). The provider answers as §6.4 already states; the consumer reports it
   through its ports as the contract error of ridl §10.2. A consumer attached
   under the provider's own hash still meets it only as one side's defect.
3. **`UnknownInteraction` from the consumer keeps its meaning.** A provider
   frame the consumer cannot place — a `publish` for an ordinal the consumer
   never named — is still a defect, the provider's, and §6.4 discards and
   records it.

The §4 `kind` check is unchanged: a consumer's `kind` for an ordinal it holds
equals the provider's, because a kind change is breaking (`KindChanged`,
`crates/ridl-diff/src/classify.rs:73-85`).

### Alternatives considered

| Alternative                                                                                                                                  | Why not                                                                                                                                                                                                                                                                                                                      |
| -------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| The descriptor carries, per earlier hash, the shape of that catalog (each interface's member count), so the provider can police the consumer | It would let a provider refuse a `subscribe` that a defective consumer sends for an ordinal beyond its catalog. The generated face names only its own members, so the defect it guards against is one the consumer's face already prevents; the cost is a row per interface per earlier hash in every descriptor. Open item. |
| The consumer sends its member set at `attach`                                                                                                | A variable-length `attach`, and a second copy of a fact the provider does not need: the provider learns what the consumer holds from what the consumer names.                                                                                                                                                                |
| The provider pushes every signal's state at `attach`                                                                                         | Not in the frame: §6.2 starts delivery at `subscribe`, and a push at `attach` would send an older consumer a `publish` it cannot place. The design keeps delivery pull-started.                                                                                                                                              |

## Question 4 — direction

Only an **older consumer** attaching to a **newer provider** is accepted. A
newer consumer attaching to an older provider is refused `catalog_mismatch`, as
today.

Why: `ridl diff`'s verdict is one-directional — "a consumer built against the
old snapshot still works" against the new one. The reverse is not judged by any
tool: a consumer at the newer hash may name an appended interaction the older
provider has no row for, and the provider, which holds the older descriptor,
cannot know that the newer hash is compatible with its own. The provider is the
authority on what it serves, and the list is written at the provider's build
from the provider's history. The asymmetry also matches the deployment model the
problem statement describes: a provider is upgraded, and the consumers already
deployed keep running.

### Alternatives considered

| Alternative                                                                                         | Why not                                                                                                                                                                                                                                                                                                                |
| --------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| The consumer also carries its list, and the provider accepts when its own hash is in the consumer's | The consumer's list says what _earlier_ catalogs the consumer's catalog is compatible with as a provider, which is the wrong direction for a consumer: the question is whether the newer consumer names only what the older provider serves, and `ridl diff` does not answer it. It would also put a list on `attach`. |
| Accept both directions whenever the two hashes share any list entry                                 | Two catalogs can each be compatible with a common ancestor and incompatible with each other (one appended `X` at ordinal 4, the other appended `Y` at 4). Only the provider's own list, read in the provider's direction, is sound.                                                                                    |

## Question 5 — the amendments

Both are written in this pull request, as normative text, with the "As built"
caveat the frame specification already carries: nothing in this workspace speaks
the frame, and the descriptor field lands with the H3 plan.

- **ADR-0014 decision 15** gains a dated amendment, "the compatible catalogs":
  the hash is unchanged; the descriptor and the codegen model carry, beside it,
  the earlier hashes the toolchain judged compatible; `ridl baseline` records
  the chain in `<unit>.catalogs` and `ridl build` emits the list from it, both
  through `ridl diff`'s classifier scoped to the unit; a breaking verdict resets
  the chain. The alternatives table gains the rows this note rejects.
- **Frame specification §6.1** replaces the `accepted` rule, the
  `catalog_mismatch` reason's wording and the invariant paragraph; §6.4 gains
  the sentence on a session attached under an earlier hash; conformance item 1
  of §12 names the list.

The records left unchanged, and why:

- `docs/design/catalog-descriptor.md` describes the descriptor **as built**. The
  schema changes under H3, and the record changes with it in the same pull
  request, so that it never claims a field the file does not carry.
- `docs/design/interaction-face.md`, ADR-0023 decision 8: the face's check is
  unchanged.
- `docs/book/`: the book describes the system as built; the H3 plan updates
  `cli-reference.md` and `catalog-descriptor.md` when the behaviour lands, and
  lane H stage H5 writes the evolution chapter.

## Question 6 — what H3 builds

[`2026-10-09-catalog-compat-plan.md`](2026-10-09-catalog-compat-plan.md). In
outline, in dependency order: the per-unit verdict in `ridl-diff`; the
`<unit>.catalogs` file's reader and writer in `ridl-core`; the descriptor schema
field, its lowering, verification and JSON view; the codegen model field;
`ridl baseline` writing the chain; `ridl build` reading it and passing the list
to `ridlc`, which writes it; then the records and the book.

## Migration

A baseline published before this change has no `<unit>.catalogs` file. The first
`ridl baseline` after the upgrade writes one with a single hash; the chain grows
from the next publication. Until then every build emits an empty list, which is
today's behaviour. No `interfaces.lock`, snapshot or generated code changes
shape.

## Rulings

Each ruling: what was decided — why — what it costs if wrong.

- **R-H2-1** The chain is recorded by `ridl baseline` in `<unit>.catalogs`
  beside the snapshots, and read by `ridl build` — the baseline is the published
  contract and the only step that already compares a publication with its
  predecessor; the build is the only step that knows the current hash — if
  wrong, a file format is retired and the chain moves to another carrier; no
  generated artifact changes shape.
- **R-H2-2** The verdict that extends or resets the chain is `ridl diff`'s
  classifier, scoped to the unit by `unit_verdict` (the unit's own packages plus
  the declarations its interfaces reach) — the hash covers exactly that closure,
  so the verdict and the identity cover the same facts — if wrong, a unit's
  chain resets too rarely (unsafe: a consumer is accepted that cannot be served)
  or too often (safe: a consumer is refused that could have been served); the
  filter is one function and its tests, and the workspace-wide verdict is the
  fallback.
- **R-H2-3** A breaking verdict resets the chain to the one new hash, at
  publication and at build time alike — a consumer built against a pre-break
  catalog must be refused — if wrong, nothing: this is the brief's requirement.
- **R-H2-4** Compatibility is carried link by link and assumed transitive —
  every compatible category is monotone, and keeping every earlier baseline
  would change the baseline directory's shape for no gain today — if wrong, a
  category is found whose composition is not compatible, and the build must diff
  against every listed baseline, which needs those baselines kept.
- **R-H2-5** The earlier hash is recorded at publication, never recomputed from
  a snapshot — the consumer holds the hash its toolchain computed — if wrong,
  nothing is lost: a recorded value equals a recomputed one under one toolchain.
- **R-H2-6** A baseline directory that is present but cannot be loaded fails the
  build with exit 2, the way `ridl check` refuses it — a provider built over an
  unreadable baseline would refuse every deployed consumer without a word — if
  wrong, a build fails that could have emitted an empty list, and the user runs
  `ridl baseline` again.
- **R-H2-7** The list lives in the descriptor (`Catalog.compatible`, appended,
  not required, `SCHEMA_VERSION` stays 1) and in the codegen model
  (`Catalog.compatible = 4`), and not in the generated Rust face — the face's
  check is local equality and must stay so; `retired` set the precedent — if
  wrong, a backend later emits a constant from the model field, which already
  carries the list.
- **R-H2-8** The descriptor entry is a table `EarlierCatalog { hash }` per
  earlier catalog, not one flat byte vector — FlatBuffers has no vector of
  vectors, a reader needs no stride, and `retired` is the same shape — if wrong,
  a flat vector saves a few bytes per entry.
- **R-H2-9** The provider learns which interactions a consumer holds from what
  the consumer names on the session, and the descriptor carries no per-hash
  shape — every provider data frame answers a consumer frame, so the provider
  needs nothing else to keep the invariant — if wrong, a per-hash shape is
  appended to the descriptor later, under the same append-only rule.
- **R-H2-10** `UnknownInteraction` from provider to consumer on a session
  attached under an earlier hash is a designed answer for an interaction retired
  since that hash — a tombstoned retirement and a retired interface are
  compatible in `ridl diff`, and §6.4 already answers so — if wrong, the
  retirement categories move to breaking in `ridl diff`, which is a change to
  its output contract.
- **R-H2-11** Only an older consumer attaching to a newer provider is accepted —
  `ridl diff`'s verdict is one-directional and the provider is the authority on
  what it serves — if wrong, a consumer-side list is a second design, and
  `attach` grows a field.
- **R-H2-12** ADR-0014 decision 15 and frame specification §6.1, §6.4 and §12
  are amended now; `docs/design/catalog-descriptor.md`, the interaction-face
  record and the book wait for H3 — normative text states the rule; a design
  record describes what is built — if wrong, a record is stale between two
  merges, which H3's pull request closes.
- **R-H2-13** `ridl diff`'s own output and exit code are unchanged — the
  per-unit verdict is a build-time input, not a report to a person — if wrong, a
  `--unit` flag is added later over the same function.
- **R-H2-14** (withdrawn, replaced by R-4 below) `ridlc build` and `ridl build`
  were to emit the same descriptor by moving the baseline discovery into
  `ridlc::run_build_with`. The pass 1 review found that this crosses the
  boundary ADR-0008 decisions 9 and 14 draw.
- **R-4 (decided by the main session, 2026-10-09)** `ridlc` stays a pure source
  → IR function: the `ridl` facade reads the baseline, computes the per-unit
  lists with `ridl_diff`, and passes them to `ridlc::run_build_with` as an input
  of the build; `ridlc` only writes what it is given, and `ridlc build` writes
  an empty list — ADR-0008 decisions 9 and 14 keep baseline reading outside the
  tool-qualification boundary, and ADR-0008 is not amended — if wrong, a
  `ridlc build` descriptor differs from a `ridl build` one in the `compatible`
  field alone, and a user who builds with `ridlc` directly gets exact-match
  attach behaviour.

## Open items

- The per-hash shape (question 3, first alternative) if a runtime is ever asked
  to police a consumer that names an ordinal beyond its own catalog.
- A `ridl diff --unit <name>` flag over `unit_verdict`, if a user asks for the
  per-unit verdict at the desk.
- driftsys/ridl#700 stands: `ridl diff` says `identical` for a lock freeze. This
  design does not depend on it being fixed.

## Trace

- Lane: the lane H driver, `2026-10-09-lane-h-driver.md` (pull request #784),
  ruling R-1
- Decisions: [ADR-0014](../decisions/ADR-0014-ir-encodings.md) decision 15,
  [ADR-0022](../decisions/ADR-0022-rsdl-system-in-the-ir.md) decision 9,
  [ADR-0023](../decisions/ADR-0023-interaction-face-generation.md) decision 8
- Specification:
  [the frame specification](../specification/frame-specification.md) §3, §4, §6,
  §10, §12; [the ridl reference](../specification/ridl-language-reference.md)
  §10.2, §11
- Design records:
  [`../design/catalog-descriptor.md`](../design/catalog-descriptor.md),
  [`../design/interaction-face.md`](../design/interaction-face.md)
- The archived per-unit design:
  [`../archive/2026-10-08-catalog-per-unit-design.md`](../archive/2026-10-08-catalog-per-unit-design.md)
