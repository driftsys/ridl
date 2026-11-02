# Podbuf: remote structs held inline across import pins

Status: owner-approved design direction for driftsys/ridl#1163, approved on
2026-11-02. Code observations refer to commit `42d04b32`. The owner selected A
and the decisions in section 7. This note is not an implementation plan and
changes no durable decision record or implementation.

## 1. Problem

Consider two packages, `vendor.types` and `app.api`, in separate units.
`vendor.types` is resolved through the consumer's `[imports]`; its artifact and
unpacked tree are pinned in `ridl.lock`. At pin P1, its struct `Reading` has one
unsigned 64-bit field, `value`. At pin P2, a second unsigned 64-bit field,
`stamp`, is appended. Both fields accept zero. The consumer's struct `Sample`
has `reading`, of type `vendor.types.Reading`, followed by an unsigned 64-bit
field `sequence`. Neither the consumer declarations nor their order changes.

The podbuf first-fit law gives `Reading` an 8-byte head at P1 and a 16-byte head
at P2. `Sample.reading` starts at head offset 0 on both pins; `Sample.sequence`
moves from head offset 8 to 16. `Sample` grows from 16 to 24 bytes. There is no
preamble inside the nested `Reading`.

For `value = 1`, `stamp = 2` and `sequence = 3`, the buffer consists of these
8-byte groups, all little-endian:

| Pin | Preamble                  | Head offset 0             | Head offset 8             | Head offset 16            |
| --- | ------------------------- | ------------------------- | ------------------------- | ------------------------- |
| P1  | `10 00 00 00 00 00 00 00` | `01 00 00 00 00 00 00 00` | `03 00 00 00 00 00 00 00` | absent                    |
| P2  | `18 00 00 00 00 00 00 00` | `01 00 00 00 00 00 00 00` | `02 00 00 00 00 00 00 00` | `03 00 00 00 00 00 00 00` |

The old reader takes the stamp bytes as `sequence`. The root preamble supplies
the producer's total head size, but supplies no size for the nested struct. This
is a consumer wire break even though the dependency's own root struct can grow
compatibly.

The current rule in `crates/ridl-diff/src/walk.rs`, `podbuf_inline_appends`,
reads only compatible `decl_added` field lines from the compared packages. It
never considers this pin-only change. Separately, `ridlc::diff_context` chooses
the new remote package before the old one and supplies one context to both
sides. Even a local append can therefore be judged using the wrong old layout.

| Command                                             | Present limitation                                                                                                                                                        |
| --------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ridl diff --encoding podbuf OLD_SOURCE NEW_SOURCE` | Remote declarations are context, not compared declarations; pin-only growth draws no line, and both layouts use the new remote definition.                                |
| `ridl diff --encoding podbuf BASELINE NEW_SOURCE`   | The baseline has no old remote packages; the new remote definition supplies both sides.                                                                                   |
| `ridl build`                                        | Catalog compatibility compares the baseline with the current packages using only current remote context, so the inline break cannot invalidate history through this rule. |
| `ridl baseline`                                     | Publication uses the same comparison for catalog history and stores only local package snapshots, so it cannot establish the old remote layout.                           |

For build and baseline, this concerns the compatible catalog list and chain; a
breaking diff normally restarts history rather than refusing generation or
publication. Existing publication refusal rules still apply independently.
`ridl check --baseline` is an ordinal check, not this compatibility gate.

## 2. What already works

`ridlc::load_diff_side` compiles a source side and partitions its checked IR
into local `packages` and `remotes`. Each source tree can therefore supply its
own old or new remote definitions, using its own pin and locally available
vendor or cache contents. Diff loading fetches nothing. No persistent format
change is required for a source-to-source fix.

The engine would need distinct resolution contexts for old and new sides, and a
consumer-oriented rule independent of local appended-field lines. Keeping
remotes separate from compared packages avoids reporting every unrelated change
in a dependency as a consumer change. The existing shared context API can remain
as a compatibility wrapper; it cannot express two versions of the same remote
package by itself.

The shared podbuf projection already computes struct heads and identifies inline
forms. Its existing holder walk includes struct fields, tuples, exact-length
arrays inside heads and induced interaction tuples. These are the right
semantics to reuse, with old-side holders as the published contract. A direct
named interaction payload or parameter is its own root; a direct struct element
of a variable-length array, a map entry or a union arm has a producer size or
stride. Those cases alone do not create an inline use. An element head can
itself contain an inline struct, which still matters.

Snapshot loading, in contrast, reads each `.ir.json` file as a compared package
and initializes `remotes` empty. Merely placing dependency snapshots next to
consumer snapshots would compare the dependencies as owned packages. The
baseline must distinguish context from ownership.

## 3. Options for the baseline side

### A. Store complete IR snapshots of reached remote packages

Store each remote package in the transitive dependency closure reached from
consumer package declarations and interaction heads, including remote packages
needed to resolve those snapshots. Deduplicate by canonical package identity
within each publication. Store an explicit inventory distinguishing local
packages from remote context, the remote unit and content pin identity, and a
format version. A dedicated context artifact, separate from the flat local
`.ir.json` files, avoids making old readers compare dependencies as local
packages. The exact artifact name is left to the builder.

This extends the baseline directory format, which is frozen by policy under
ADR-0029, even though there is no dedicated baseline dump in
`freeze/surfaces.toml`. The IR package schema need not change: snapshots use
existing `ridl.ir.v2` messages. Changing that schema instead would additionally
affect the frozen codegen protocol and is unnecessary here. Optional storage
does not alone make changed compatibility behavior safe for older readers.

Cost is the serialized IR size of the reached remote packages, including
unrelated declarations within each such package, plus the inventory. This can be
much larger than the consumer snapshots. It is self-contained for offline
comparison; it does not require keeping the old dependency cache.

It detects remote head growth without a consumer append and resolves nested
remote references at their respective pins. Complete IR also provides facts for
future same-size field changes, enum changes and other dependency rules; the
proposed growth rule alone does not detect all those changes. Unreached packages
and unrelated dependency contracts are not compared. It does not add system
snapshots or alter the podbuf layout.

A new reader accepts legacy baselines, but must mark old remote evidence as
unavailable. It must never substitute current definitions as historical facts.
For an affected podbuf unit, incomplete evidence ends carried history and
produces an explicit cannot-judge notice. A first enriched publication starts a
new trusted chain; it cannot certify hashes carried before it. Old tools may
ignore the new artifact, but their continued operation is not proof that they
judge it correctly. Adoption requires a documented minimum tool version for
enriched compatibility decisions. Malformed new evidence is an input error, not
permission to fall back to current context.

### B. Store head sizes or full head layouts

Store one entry per remote struct held inline in an old consumer head,
identified by canonical package and declaration, with its remote unit and pin,
old head size and consumer holder paths. Include format and projection-law
versions and an explicit completeness marker. A full-layout variant also stores
field identities, forms, offsets and sizes. Record outer remote structs that
contain further inline remote structs: their computed size must include the
complete old dependency resolution.

A separate metadata artifact extends the same policy-frozen baseline format; it
can avoid changing the IR protocol. Size-only cost is proportional to the number
of distinct held structs plus their holder paths and identities. Full layouts
add cost proportional to the fields of those structs, usually less than full
package IR. There are no measured size claims in this note.

Sizes detect the requested old-to-new head growth, including growth induced by a
deeper dependency. Equal sizes cannot prove field compatibility, detect moved
fields, changed widths or changed constraints, or reconstruct old layout for the
existing local pad-fit append rule. Full layouts cover more physical changes but
still lack the semantic declarations needed by general diff rules. They
introduce a second versioned representation of projection output. Neither
variant establishes a changed pin unless pin evidence is recorded.

Legacy baselines lack these entries and their completeness marker. They need the
same explicit uncertainty and history reset as A; metadata cannot be
reconstructed from the new pin. Old readers can ignore an artifact but will
retain their old blind spot. Partial or inconsistent enriched metadata must be
rejected or treated as incomplete, never treated as an unchanged layout.

### C. No baseline format change; report cannot judge

Store nothing new. Source-to-source comparisons use separate remote contexts and
judge growth. Baseline comparisons report that the old remote layout is
unavailable, with the consumer holder and remote declaration as evidence.

There is an important limit: current baseline packages do not store import URLs,
lockfile content hashes or old remote declarations. Thus this option cannot
reliably trigger only when the remote pin moved. It must conservatively report
uncertainty whenever an old consumer head holds a remote struct and historical
evidence is absent, even if the pin may have stayed unchanged.
Baseline-to-baseline comparisons can be uncertain on both sides. Looking at
today's `ridl.lock` does not establish yesterday's pin.

Storage cost is zero; the cost is repeated warnings and loss of an affirmative
compatibility judgment for affected baseline comparisons. All published
baselines remain readable. This changes no baseline bytes, but changes the
policy-frozen diff output, and a dedicated category or exit behavior also
requires stability review. `Verdict` currently has only Identical, Compatible
and Breaking; a cannot-judge result cannot silently be added as an existing
verdict or as an rsdl system heading.

A warning alone with a successful compatible result leaves the build and
publication history problem unresolved. A safe variant must stop carrying
catalog history for affected podbuf units and explain that the result is
uncertain rather than a proven wire break. An exit-2 refusal for an explicitly
requested podbuf baseline judgment is a possible CLI contract, subject to owner
approval. Neither behavior proves compatibility of old pins.

## 4. The new diff rule

Proposed category: `PodbufRemoteInlineGrowth`, serialized as
`podbuf_remote_inline_growth`. This is a new category; the present `Category`
enum in `crates/ridl-diff/src/lib.rs` has `PodbufInlineAppend` but no category
for dependency growth or missing historical evidence. Reusing
`podbuf_inline_append` would imply an appended consumer field and its path,
although no such field exists. A distinct category gives an honest explanation
and isolates the stability decision about adding category vocabulary.

Enumerate published consumer inline uses on the old side. Follow named
references through their own side's package scope, including nested remote
structs that an outer remote struct contains inline. Compare each held remote
struct's old and new computed heads; emit when its size grows. Do not require a
local `decl_added` change or rely only on pin inequality: source IR already
supplies the actual old and new declarations. Deduplicate the same remote cause
at the same consumer use; report in deterministic consumer-path order. New
holders with no old published copy do not qualify.

The change belongs to the consumer holder, not to a remote package. Proposed
text line shape, following the existing path, category and verdict fields:
`app.api/Sample/reading [breaking] podbuf_remote_inline_growth: vendor.types.Reading head 8 -> 16 bytes (P1 -> P2)`.
The pin suffix is included only when known. The before and after values carry
the sizes and remote identity within the existing report structure. Consumer
paths also ensure `UnitVerdicts` applies the break to the owning consumer unit
without needing to treat remote packages as owned packages.

Verdict: Breaking when `podbuf` is selected, Compatible otherwise, retaining the
existing “breaking under podbuf” grouping. Other independently detected breaks
keep their normal verdicts. Without podbuf, this line does not certify that a
dependency change is safe for every other encoding.

The `--explain podbuf_remote_inline_growth` entry should state the lack of a
nested producer-size boundary, why a pin-only change affects consumer bytes, why
only old inline holders count, why a pad-fit append with equal size draws no
growth line, and why root and descriptor boundaries differ. It must state that
shrinkage, equal-size layout changes and dependency semantic changes remain
outside this growth rule. Missing, removed or unresolvable definitions produce
explicit uncertainty or input errors; they must not be reported as proved growth
or silently interpreted with new-side definitions.

Catalog-history callers must treat an affected unit's proved podbuf break or
missing historical evidence as a reason to end its chain. The current
unchanged-hash shortcut must not override that result. The existing report
verdict and hash shortcuts need separate consideration; fixing diff alone would
not suffice for build and publication.

## 5. Recommendation

Recommend A, complete snapshots of reached remote packages stored as distinct
baseline context. It preserves the original semantic input, supports the
existing layout law without a second layout schema, and makes enriched baseline
judgments work offline with the same information as source diffs.

For legacy baselines, use C's explicit uncertainty as a migration behavior and
restart affected podbuf histories at the first enriched publication. This is not
a claim to repair historical chains. Source-to-source judgments can be correct
without waiting for baseline enrichment during development. The owner approved
R2: release source comparisons, baseline enrichment and migration behavior
together, so diff, build and publication use consistent evidence.

Defer same-size remote layout changes, shrinkage, general dependency semantic
comparison, historical chain certification, encoding-history storage and rsdl
system snapshots. The approved direction does not perform an implementation,
migration or durable record amendment; this deliverable remains the design note
only.

## 6. Alternatives considered

B with sizes alone is smaller but cannot resolve old consumer heads for existing
pad-fit checks; it solves only a narrower part of the missing-input problem. B
with full layouts duplicates projection state and still cannot support semantic
classification as complete IR can.

C alone avoids storage changes but cannot identify pin moves from current
baseline data. It would make many unchanged workspaces uncertain and provides no
definitive baseline judgment. It remains necessary for legacy evidence.

Fetching old pins or relying on a surviving cache is rejected as the baseline
contract: a published baseline has no pin identity, source diff loading is
read-only and offline, and caches may be removed. A copy of `ridl.lock` alone
would identify pins but would not make the old IR available offline.

Comparing every imported package as a consumer package would report unrelated
dependency changes and confuse unit ownership. Putting new `.ir.json` remote
files into the flat baseline directory would cause exactly that behavior in
current loaders. Reusing `podbuf_inline_append` avoids a new category but
misstates the evidence and its path; a category addition must instead receive
explicit stability review.

## 7. Owner questions and approved decisions

The owner approved these answers on 2026-11-02. No owner question remains open
for this design direction.

1. Do you choose A (complete remote IR snapshots), B (sizes or layouts), or C
   (no format change) for the baseline contract?

   **A.** Store complete reached remote IR as distinct baseline context.

2. If you choose B, do you choose B1 (head sizes only) or B2 (full head
   layouts)?

   **Not applicable with A.** B2 would provide more evidence than B1 if B were
   selected, but neither is added alongside A. Derive layouts from the stored IR
   using the shared projection law.

3. Do you approve treating the baseline extension and new diff category as
   changes to policy-frozen surfaces requiring explicit stability review?

   **Yes.** Users can depend on baseline formats and CLI output. Review the
   compatibility effects and document migration and release impact. Protection
   by policy applies even where no automated freeze dump covers the surface; it
   does not prohibit an explicitly declared change in an eligible release.

4. Do you approve ending affected podbuf catalog histories when legacy baselines
   lack old remote evidence, even when the current catalog hash is unchanged?

   **Yes.** End the affected compatibility chain on a proved break or missing
   historical evidence. Preserve the previous baseline for inspection and
   explain why the chain ended. Start a fresh chain at the current catalog;
   resetting history must never assert compatibility with earlier peers.

5. Do you approve exit code 2 for an explicit podbuf baseline comparison that
   cannot judge remote inline compatibility because historical evidence is
   absent?

   **Yes, provided recovery is available.** Exit 2 means compatibility could not
   be determined; exit 1 remains a proved break. Provide documented, explicit
   repair and reset operations with actionable diagnostics:

   - Repair reconstructs historical evidence from the original consumer sources
     and exact old dependency pins. The evidence must correspond to the baseline
     being repaired; current sources or pins are not substitutes. Repair is
     conditional on those inputs being available and does not certify earlier
     carried hashes without their own evidence.
   - Reset preserves the previous baseline for inspection, publishes an enriched
     current baseline and starts fresh compatible catalog history. It does not
     repair or certify earlier history. Missing evidence alone must not prevent
     this explicit recovery operation, although ordinary compilation and
     publication refusal rules still apply.

   Neither operation is an existing command promised by this note. Exact CLI
   spelling, preservation location and artifact naming are left to the builder;
   the operations must be explicit and the diagnostics must identify the
   applicable recovery command. Normal diff remains read-only.

6. Do you choose R1 (release source-to-source judgment first) or R2 (release it
   together with baseline enrichment and migration behavior)?

   **R2.** Release source comparison, baseline enrichment and recovery together.
   Internal development can proceed separately. Released diff, build and
   baseline behavior must use consistent evidence and history decisions.

7. Do you approve scheduling any incompatible baseline or CLI contract change
   for a declared minor release rather than assuming it can ship in a patch?

   **Yes.** Schedule the compatibility changes for a declared pre-1.0 minor
   release, 0.15.0 if this follows the current 0.14 series. Do not ship them in
   a patch. Release notes must state the changed baseline evidence, category,
   failure behavior, minimum tool version and repair/reset procedure. Follow
   ADR-0029's stability review; do not infer permission to alter unrelated
   frozen surfaces. After 1.0, incompatible public-contract changes require a
   major release under Semantic Versioning.

8. Do you approve the distinct category `podbuf_remote_inline_growth` instead of
   reusing `podbuf_inline_append`?

   **Yes.** The category identifies a consumer break caused by remote inline
   growth without implying that the consumer appended a field.

## 8. Trace and record/code differences

The task source is driftsys/ridl#1163. No network lookup was performed; all
observations above are from this checkout and the supplied issue brief.

Records the approved design would amend or review in subsequent work:

- [ADR-0031](../decisions/ADR-0031-podbuf-projection-rules.md), decisions 10 and
  13: add the consumer effect of dependency growth, separate contexts, category,
  baseline evidence and history behavior; supersede the bounded remote limit.
- [Podbuf codec record](../design/podbuf-codec.md), inline evolution and known
  gaps: describe the selected detection scope and migration limitation.
- [ADR-0002](../decisions/ADR-0002-module-system.md), imports and the resolver
  and lockfile sections: review how remote pin identity is retained as evidence,
  without changing dependency ownership or resolution precedence.
- [ADR-0029](../decisions/ADR-0029-stability-policy.md), decisions 1, 3 and 4;
  [Stability](../book/stability.md); and
  [freeze guidance](../../freeze/README.md): record the baseline and diff-output
  compatibility assessment. The format and categories are frozen by policy only;
  passing dump checks is insufficient.
- [ADR-0022](../decisions/ADR-0022-rsdl-system-in-the-ir.md), decisions 8 and 9,
  and [rsdl implementation](../technotes/rsdl-implementation.md): review the
  contract boundary. This proposal does not add system snapshots, change rsdl
  headings or change which compilation errors permit artifact emission.
- CLI baseline and diff documentation, and the
  [workspace map](../technotes/walking-skeleton-architecture.md): update
  ownership and publication descriptions once a format is selected.

Two qualifications to the current records follow directly from code:

- ADR-0031 decision 13 and the codec known-gaps row describe a remote change
  occurring in the same step as a consumer append. The rule also misses a
  remote-only growth with unchanged consumer declarations: it only visits local
  compatible field additions. The records describe a narrower example than the
  actual detection gap.
- ADR-0031 decision 13 says a podbuf break ends compatible history. In
  `crates/ridl/src/catalogs.rs`, both `compatible_catalogs` and
  `write_catalog_histories` can carry history when its first hash equals the
  current hash, independently of the diff verdict. Their hash scopes use local
  packages plus standard context, excluding remote packages, so a pin-only
  change can leave that history-helper hash unchanged. This is distinct from the
  emitted catalog hash: the build supplies remote packages to its codegen and
  descriptor scope in `crates/ridlc/src/lib.rs`. The history helpers therefore
  also lack the full scope used by the emitted catalog, despite their comments
  saying the hashes agree. A selected design must preserve the canonical emitted
  hash inputs while making history evidence consistent with them; it must not
  redefine the wire hash. A new breaking report must take precedence for
  affected units. These are code-reading observations, not evidence from an
  executed runtime test.

The snapshot ownership and shared-context behavior recorded in ADR-0031 match
`DiffSide`, `load_diff_side`, `diff_context`, `run_baseline` and
`publish_baseline`. The frozen-by-policy status matches ADR-0029 and the book;
there is no contradiction simply because a baseline-format dump is absent.
