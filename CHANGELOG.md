# Changelog

## [0.2.1] (2026-09-23)

### Bug Fixes

- **repo:** stop matching sha256sum's localized message in install-check
  ([#503]) ([e53ee4d])
- **ridlc:** bound plugin response collection ([7886ea5])
- **editors:** rename the extension to ridl-lang, its Marketplace name is taken
  ([#499]) ([43e1cf1])

### Documentation

- **ridlc:** describe plugin response timeout ([591c32c])
- **roadmap:** close the Kotlin Binder runtime scope item ([817e0bb])
- **docs:** add the codegen model design note, lane P stage P2a ([fe3ce8e])
- **adr:** make canonical protobuf JSON the canonical IR encoding ([243dc49])
- **docs:** add the frame specification, story E11.1 ([975fae2])
- **docs:** add the IR stability design note, lane P stage P1a ([6644f82])
- **roadmap:** move the plugin protocol ahead of the remaining codecs and
  TypeScript ([bbae0ed])
- **editors:** lead the vscode README with features, not dev commands ([#500])
  ([b7859fc])
- **docs:** add the lane P driver, the codegen plugin system toward Kotlin
  ([#494]) ([3f0a65d])

### Features

- **repo:** merge lane P codegen plugin system ([7fc417e])
- **ridl-ir:** add the backend contract over the codegen model ([86f5fa6])
- **ridl-ir:** add the lowered codegen model, its schema and its lowering
  ([5eb7f8e])
- **editors:** add RIDL extension icon ([#497]) ([c338f9c])
- **ci:** release ridlc alongside ridl on the editor-v* train ([#495])
  ([2632e1d])

### Refactoring

- **ridl-backend-rust:** read the domain types from the lowered model
  ([8d015e5])

[0.2.1]: https://github.com/driftsys/ridl/compare/v0.2.0...v0.2.1
[e53ee4d]: https://github.com/driftsys/ridl/commit/e53ee4d
[#503]: https://github.com/driftsys/ridl/issues/503
[7886ea5]: https://github.com/driftsys/ridl/commit/7886ea5
[43e1cf1]: https://github.com/driftsys/ridl/commit/43e1cf1
[#499]: https://github.com/driftsys/ridl/issues/499
[591c32c]: https://github.com/driftsys/ridl/commit/591c32c
[817e0bb]: https://github.com/driftsys/ridl/commit/817e0bb
[fe3ce8e]: https://github.com/driftsys/ridl/commit/fe3ce8e
[243dc49]: https://github.com/driftsys/ridl/commit/243dc49
[975fae2]: https://github.com/driftsys/ridl/commit/975fae2
[6644f82]: https://github.com/driftsys/ridl/commit/6644f82
[bbae0ed]: https://github.com/driftsys/ridl/commit/bbae0ed
[b7859fc]: https://github.com/driftsys/ridl/commit/b7859fc
[#500]: https://github.com/driftsys/ridl/issues/500
[3f0a65d]: https://github.com/driftsys/ridl/commit/3f0a65d
[#494]: https://github.com/driftsys/ridl/issues/494
[7fc417e]: https://github.com/driftsys/ridl/commit/7fc417e
[86f5fa6]: https://github.com/driftsys/ridl/commit/86f5fa6
[5eb7f8e]: https://github.com/driftsys/ridl/commit/5eb7f8e
[c338f9c]: https://github.com/driftsys/ridl/commit/c338f9c
[#497]: https://github.com/driftsys/ridl/issues/497
[2632e1d]: https://github.com/driftsys/ridl/commit/2632e1d
[#495]: https://github.com/driftsys/ridl/issues/495
[8d015e5]: https://github.com/driftsys/ridl/commit/8d015e5

## 0.2.0 (2026-09-22)

### Bug Fixes

- **ridl-backend-rust:** send the encoded subslice, and settle
  driftsys/ridl[#448] ([#471]) ([83bcc10]), closes [#448]
- **repo:** clear the whole compat-check build directory each run ([#464])
  ([d36e97c]), fixes 442.
- **ridl-sem:** give a bare string or bytes map key the length default ([#459])
  ([86526be]), closes [#457]
- **ridl-backend-rust:** project struct, tuple and union names to their target
  namespaces ([#451]) ([5b5a0ce])
- **ridl-core:** refuse a package named for one the compiler provides ([#407])
  ([f7882b3]), closes 203.
- **ridl-sem:** report an exact duplicate name before RIDL-149 checks a
  collision ([#406]) ([1d43aa4])
- **ci:** give the git-std install a fallback and a post-install check ([#402])
  ([e3a1071]), closes [#395]
- **repo:** close a fence in link-check only at a fence as long as its opener
  ([#359]) ([a7ac5c8])
- **ridl:** refuse to publish an untombstoned removal or an explicit baseline
  with no snapshot ([#330]) ([2a4a47d]), closes 235. Addresses #315 at the
  interaction level. Pass-2 review debt is #339, [#340], #341 and #342.
- **ridl:** describe a snapshot directory whose snapshots are nested ([#233])
  ([21a41f4]), closes [#230]
- **ridl:** refuse unrecognised IR paths instead of compiling them ([#228])
  ([4cf2a51])
- **repo:** clear three recorded debt items ([#227]) ([0f5a712])
- **ridl-ir:** move the IR's JSON encoding onto pbjson-generated impls ([#226])
  ([4762d28])
- **ridl-ir:** contain the prototext reader behind the crate's tests ([#225])
  ([a31bcd2])
- **ci:** stop the pages deploy being skipped by a skipped ancestor ([#214])
  ([ec96247])
- **ridlc:** emit ridl.std so generated code compiles standalone ([#200])
  ([8daf58d]), refs 190.

* fix(ridl-ir): make the reference-walk depth test
  discriminate per path

The recursion test for referenced_packages used the
  same qualifier
(ridl.std) for a direct reference and for the array-element
  reference
it claimed to exercise, so the array arm's body could be
  deleted
without failing the test. Verified: gutting that arm still
  passed.

Give every recursive path through walk_field_type its own
  distinct
qualifier (array, tuple, map key, map value, stream element) plus
  the
direct and bare-reference cases, and assert the exact expected set
instead
  of contains plus a length check, so a missing arm names itself
in the failure
  output.

Proved the fix discriminates: for each of the five recursive arms,
  temporarily replaced its body with an empty block, ran the test, and
confirmed
  it failed naming exactly the qualifier that arm was
responsible for, then
  restored the arm. All five reproduced the
expected single-qualifier omission.,
  190.

* fix(ridlc): emit ridl.std when the workspace references it

`ridlc
  build` writes each checked package's generated code, but
`ridl.std` is
  deliberately absent from `checked` because it is not a
workspace member. A
  consumer's generated code can still reference it
(for example a duration field
  lowers to `ridl.std.Timestamp`), so the
raw build output did not compile
  (issue #190).

`run_build` now checks, after the per-package emit loop,
  whether any
checked package's IR references `ridl.std` (task 1's, 190.

*
  fix(ridlc): keep ridl.std out of the ir-json emit

`run_build` passed the
  caller's full `emits` slice to the standard package's
`write_emits`, so
  `--emit ir-json` wrote `ridl.std.ir.json`. `ridl baseline`
is that call with
  `&[Emit::IrJson]` and publishes every `.ir.json` it finds, while `ridl diff`
  compiles its current side through `compile_workspace`, which
excludes
  `ridl.std` because the standard package is not a workspace member.
The two
  sides were asymmetric, so `ridl baseline` followed by `ridl diff`
over an
  unedited workspace that names a standard type reported

    breaking
     
  [breaking] decl_removed ridl.std: package ridl.std -> (removed)

and exited 1.
  `ridl check --baseline` did not show it: the desk check reports
ordinal drift
  only, and a whole removed package moves no ordinal.

The standard package is
  version-locked to the compiler binary (ADR-0007
decision 15), so it is not
  part of a workspace's contract snapshot — a
baseline holds the packages the
  workspace declares. Issue #190 is about
generated code failing to compile, and
  a `.ir.json` is not code. The three
code targets keep the emit: `rust`,
  `c-header` (its header already names
`ridl_std_*` typedefs) and `typescript`.
- **typl:** scope ridl.std by an inclusion criterion, drop Vin ([#198])
  ([2ba8690])
- close three ridl/ridlc CLI defects, record ADR-0010 ([#195]) ([fa62c08])
- **adr:** discharge ADR-0008 decision 19 and sweep the document ([#192])
  ([c40f65e])
- **repo:** five small items from the E2/E1 debt triage ([#189]) ([499ec32])
- **repo:** make the local gate and CI the same gate ([#186]) ([75c02c2])
- **ridl-backend-ts:** parenthesise an object literal in a concise arrow body
  ([#185]) ([519ad66]), closes [#177]
- **repo:** make the local gate enforce every member decision 11 names ([#184])
  ([dc8cad0]), closes [#182]
- **ridl-sem:** description-first diagnostics across the RIDL namespace ([#178])
  ([da3d246])
- **backends:** two codegen defects — an internal tuple and a constant
  reference ([#176]) ([af7ef7c]), closes 167

* fix(ridl-sem): a constant
  reference lowers as the referenced value

`ConstDef.value` and a declared init
  carried the written text of a constant
reference instead of the constant's
  value. `ridlc check` exits 0 on

    const SECRET : Tick = 7
    const PUB   
  : Tick = SECRET

and the Rust backend then emits `pub const PUB: Tick =
  Tick(SECRET);`, which
rustc rejects.

The repair belongs at lowering, not in a
  backend, and the IR says why:
`ConstDef.value` is a bare string with no
  discriminator, so `const A : string =
"B"` and `const A : string = B` lower to
  identical IR. No consumer can tell a
value from a reference, and each one
  failed differently — the Rust backend
emitted uncompilable code for a named
  scalar, a wrong value for a string or a
boolean, and `PI.0` for a float; the
  TypeScript backend refused the package
outright with a message about
  `Number.MAX_SAFE_INTEGER`. A backend also holds
one package's IR at a time, so
  it could not resolve an imported constant at
all.

The chain is followed
  through the existing `const_value`, which already serves
the range bounds, the
  `match` patterns and `declared_type_init`: one
resolution, cycle-guarded, and
  correct across packages. A reference that does
not resolve still lowers as the
  written text — the diagnostic for an unknown
name belongs to the
  resolver.

The declared-init path had the same rule in a second
  implementation, and the
two diverged where it was hardest to see: a numeric
  reference resolved, so
`type T : integer [0..100] = SEVEN` was right, while
  `type T : string [1..20] =
GREETING` lowered the *name* as the string value
  and the generated `Default`
returned `"GREETING"`. Both now resolve every
  kind, and the resolved value is
checked against the declared bounds exactly as
  a direct literal is.

Fixing it here fixes both backends from one change,
  which is what the E2 exit
criterion asks of the IR., 170

* fix(backends):
  refuse a tuple-name collision instead of deduplicating it

A tuple generates a
  struct named for the CamelCase of the path that reaches it, and nothing
  upstream keeps two paths from spelling one name:

    internal struct A  { bC
  : (y : Tock) }
    struct AB          { c  : (x : Tick) }

both reach `ABC`,
  and `ridlc check` exits 0. The worklist kept the first
discovery, so the
  second declaration got the first one's shape — a module that
compiles and
  states the wrong contract.

Carrying the inducing declaration's visibility
  made that corner worse rather
than better. With one declaration `internal` and
  the other public — and no
`internal` payload type anywhere, so TYPL-005 has
  nothing to see — first-wins
put a `pub(crate)` struct in a `pub` struct's
  field, and rustc reported
`private_interfaces` on a program that built before.
  Keeping the widest
visibility instead would publish the package-private
  declaration's shape, which
is the defect the previous commit fixed.

Neither
  dedup rule is sound under collision; only rejection is. The refusal is
what
  every other generated-name clash already gets from
- **ridl-sem:** three parser and resolver defects from the E2 close-out ([#174])
  ([ebebaf7])
- **ridl-sem:** apply the visibility rule to interfaces and services ([#168])
  ([8705450]), closes 161

* fix(ridl-sem): ask the resolver which names a
  contract clause binds

Review of the previous commit found the exposure check
  for `require`/
`ensure` clauses re-deriving the contract binding order from
  the syntax
tree. It matched a `SignalDef` anywhere under the enclosing
  interface, whatever the clause kind — but `lower_contracts` builds an
  `ensure` scope
with `signals: &[]` (ridl §13), so an interface signal spelled
  like an
`internal` constant suppressed a diagnostic the resolver binds to
  the
constant. `signal MAX_LEVEL` beside `internal const MAX_LEVEL`
  made
`ensure result > MAX_LEVEL` compile with zero diagnostics while the
  IR
published the clause text naming the package-private constant.

The check
  now runs inside `lower_contracts`, reading `collect_refs`
against the very
  scope `check_contract_expr` was run with, so the binding
order has one
  implementation and this reads its answer instead of
guessing it. `ExprRefs`
  gains `enum_types` — the head of an `Enum.MEMBER`
access is not a read, so
  the observer stubs do not want it, but it is a
package declaration the
  published clause names. Re-deriving a rule that
already has an owner is the
  same shape as the defect this branch fixes, where the rule lived in one place
  and its application in another.

Review also found a twentieth exposure
  position: a collection length
`Bound` is a direct child of the
  `ArrayType`/`MapType` rather than a
`Constraint`, so `[Tick; 1..MAXLEN]` never
  matched the walk while
`string [1..MAXLEN]` did — two structurally identical
  positions with one
flagged and one silent, in the typl layer as well as the
  ridl one. typl
§3.3 says "bounds constants", so this was an
  under-implementation of the
section cited as authority for the init-value
  exclusion. Both node kinds
are now named. There is no practical leak — the
  bound resolves to a value
and the constant's name does not survive into the IR
  — but a table
asserting completeness it does not have is how the next person
  stops
looking, which is what produced #161.

The corpus carries both:
  `exposure.ridl` now holds the signal/constant
collision that separates the two
  clause kinds, with the `require` half
silent and the `ensure` half reporting,
  and a length bound in both the
public interface and the `internal` control.
  The legal-direction unit
fixture gains a clause and a bound so that removing
  the clause-level
visibility guard is observable — it was not
  before.

Recorded rather than fixed: duplicate declarations now report
  the
exposure once per duplicate where the lowering loop consulted
`is_winner`;
  the RIDL-143 branch tests for a `ServiceDef` parent with no
`else`, so a later
  interface-naming position inherits TYPL-005 but not
RIDL-143; and an attribute
  value's constant reference is invisible to the
pass, unreachable today only
  because every attribute key draws
FORM-106/107. ADR-0008's allocation ledger
  needs a ninth entry for
RIDL-143 — issue #169, since a PR-body note is what
  #161 said would not
survive the merge.
- **backends:** TypeScript emits interaction ordinals and tombstones ([#164])
  ([3dc6f30])
- **ridl:** resolve contract names the way the checker does ([#162]) ([516dff0])
- **backends:** internal interfaces map to package-private in both backends
  ([#160]) ([4230fd8])
- **ridl-sem:** skip lowering a nameless interaction member ([#158]) ([0c8f3ab])

### Features

- **repo:** give every crate a version, and publish it to crates.io on a tag
  ([#489]) ([8eb3421])
- **repo:** run the cabin example with just demo ([#483]) ([9069d2c])
- **ridlc:** emit the interaction face from ridl build ([#479]) ([330e962])
- **ridl-backend-rust:** move the generated face onto the FlatBuffers codec
  ([#475]) ([5deab17])
- **ridl-backend-flatbuffers:** root every declaration in a table ([#474])
  ([5f05bdc])
- **ridl-backend-rust:** check the typl constraint beside new in verify ([#468])
  ([80773b8]), closes [#463]
- **ridl-loopback:** the in-process reference runtime ([#466]) ([c5c582c])
- **ridl-backend-rust:** emit the FlatBuffers payload codec ([#465]) ([261724f])
- **ridl-backend-rust:** construct vacuous named scalars infallibly ([0ade5b1])
- **ridl-backend-rust:** refuse a struct or union with no finite FlatBuffers
  bound ([#462]) ([20c5f1e])
- **ridl-ir:** the shared FlatBuffers projection facts and the size bound
  ([#458]) ([196989a])
- **ridl-rt:** lane K stage K3 — the FlatBuffers helpers, the subslice, and
  the proof mechanism ([#461]) ([77c5e59])
- **ridl-backend-rust:** derive the sound traits on the typl surface ([#443])
  ([8e5a552])
- **ridl-backend-rust:** hold the port by value and type each correlation
  ([#439]) ([f9b2904])
- **ridl-backend-rust:** check match patterns under validate-pattern ([#436])
  ([61a6c35])
- **ridl-rt:** implement every port trait for a borrow of a port ([#438])
  ([3fdb631])
- **ridl-backend-rust:** validate enum and enum-set conversion from i64 ([#433])
  ([7c9d9c4])
- **ridl-backend-rust:** make constrained named scalars value objects ([#420])
  ([ddcbe85])
- **ridl-backend-rust:** generate the interaction face over ridl-rt ([#418])
  ([2d26b38])
- **rsdl:** lower the system to the IR, with ridl diff and the book chapter
  ([#417]) ([fe4c44b])
- **ridlc:** emit a compiling crate from --emit rust ([#415]) ([564bc14]),
  closes [#252]
- **ridl-backend-rust:** name the runtime by absolute path, not a generated
  error ([#413]) ([529d93e]), closes [#247]
- **ridl-ir:** add the vacuous-constraint classifier ([#412]) ([d8b7a59]),
  closes [#246]
- **ridl:** number every interface from interfaces.lock and add ridl lock
  ([#391]) ([fdcf2ca])
- **rsdl:** parse and check .rsdl, with editor support ([#388]) ([0504220])
- **ridl-rt:** revise the port and error API before 0.1 ([#351]) ([5f24ea7]),
  refs [#350], [#316]
- **ridl-rt:** add ridl-rt 0.1.0, the runtime library ([#348]) ([91e6b62]), refs
  [#316]
- **repo:** add the MCP server and the editor distribution train ([#327])
  ([2fa924c])
- **ridl-diff:** give a composite body reorder its own category ([#331])
  ([f2efb61])
- **ridl-backend-flatbuffers:** project the typl surface and the ordinals onto
  FlatBuffers ([#303]) ([0199406])
- **repo:** retract the interaction layer and record the runtime architecture
  ([#241]) ([7d539bc])
- **ridl-backend-proto:** project the typl surface and the ordinals onto proto3
  ([#242]) ([1e674fe])
- **ridl:** pin one name transform and reject collisions after it ([#238])
  ([b4c420f])
- **ridl:** a service may carry more than one interface ([#223]) ([89b8604])
- **ridl:** give command and query the RPC bounds ([#221]) ([c560e3d])
- **ridlc:** add the ir-text and ir-binary emits ([#219]) ([9a5eb49])
- **ridl-ir:** render canonical protobuf JSON on every IR surface ([#217])
  ([ef3c9a5])
- **ridl:** the interaction boundary model — retire uxdl, add rxdl ([#204])
  ([ded42ef])
- **ridl-syntax:** rename the provisioned-constant keyword to fixed ([#199])
  ([2513e70])
- **ridlc:** emit TypeScript from `ridlc build --emit` ([#188]) ([a3a4700]),
  refs [#172], driftsys/ridl#172
- **ridl:** ridl test — subset evaluator + property runs (E2.11a) ([#157])
  ([96efb74])
- **backends:** Rust backend — interactions and services (E2 exit criterion)
  ([#156]) ([364dc4b])
- **backends:** TypeScript backend — interactions + services (E2.6b) ([#155])
  ([99fe470])
- **ridl:** baseline-aware desk check (E2.9) ([#153]) ([6e6abe5])
- **ridl-sem:** the ridl lint pass (E2.10a) ([#154]) ([3729961])
- **ridl-sem:** observer-stub lowering (E2.5) ([#152]) ([7b3c831])
- **tools:** ridl diff — breaking/compatible classifier (E2.8b) ([#150])
  ([450f2b9])
- **ridl-sem:** expr subset type checking for require/ensure (E2.4) ([#149])
  ([dfcc83b])
- **ridl-lsp:** LSP + editor support for ridl (E2.10b) ([#151]) ([5266ccb])
- **ridl-sem:** services — grammar, resolution, catalog, IR (E2.13) ([#148])
  ([00e666a])
- **tools:** ridl diff — IR-snapshot compare engine + CLI (E2.8a) ([#146])
  ([62bc8ed])
- **ridl-sem:** timing — validity, defaults, resolved IR (E2.2) ([#147])
  ([469047a])

### Refactoring

- **ridlc:** classify emits exhaustively for the ridl.std filter ([#220])
  ([20fba18])
- **repo:** every crate lives at crates/<crate-name>/ ([#181]) ([4cffb74]), refs
  [#180], [#180], [#180]
- **ridl-core:** declare each diagnostic code once (ADR-0008 d21) ([#175])
  ([6f15e77]), refs #172

* fix(ridl-core): correct three guard claims and
  enforce clippy locally

Review of d6f6c9e found three guard doc comments
  asserting a reach the
guards do not have, and one deliverable ADR-0008
  decision 21 binds that
was missing from the repository.

The "invocable
  exactly once" claim was false. A generated `ALL_CATALOGS`
collides only within
  one module, so a second `diag_codes!` inside a child
module of `diag`
  compiles. The safety outcome held, but through a
different guard than the one
  named: the literal scan caught the injected
code, and it did so structurally
  rather than by luck, because, [#172]
- **ridl-ir:** one walk over every interface shape ([#171]) ([2f8ff9b])

### Documentation

- **docs:** correct four lines the face's move onto the codec made false
  ([#478]) ([9684914])
- **docs:** archive the lane K driver, and close the lane ([#477]) ([26214fd])
- **docs:** finish K1 — the plan's task shape and the driver's stages ([#454])
  ([982c8d8])
- **docs:** the FlatBuffers payload codec design and plan ([#446]) ([a7c8051])
- **docs:** lane K stage K0 — split E11.9, file E11.14 and E11.15, add the
  lane prompt ([#447]) ([e44e885])
- **docs:** archive the face and port ergonomics note ([#440]) ([512a478])
- **docs:** design the face and port ergonomics to settle before E11.9 ([#429])
  ([6fb3456])
- **docs:** add the lane R driver prompt ([#435]) ([0b3e69f])
- **adr:** record the face and port ergonomics decisions ([#432]) ([f6b749e])
- **typl:** correct the value objects plan to the code Task 3 landed ([#431])
  ([3504a90])
- **ridl-rt:** document the traits progressively, and garden lane M ([#419])
  ([b6544fe]), closes [#393]
- **docs:** plan the interaction face MVP ([#411]) ([566f024])
- **typl:** take decision D, so a colliding union arm is refused ([#410])
  ([8746585])
- **docs:** give stage C4 its own driver prompt ([#409]) ([3988701])
- **docs:** record the model substitution while Fable is unavailable ([#408])
  ([25db3a3])
- **ridl:** dispose of every ridl §17 open question ([#401]) ([6d21c59]), refs
  319

* docs(docs): archive the lock design and plan

Lane L's work has landed,
  so its working memory moves to `docs/archive/` with
an entry in the archive
  README naming the durable records it gardened into:
ADR-0015's 2026-09-15
  amendment, ADR-0010's `ridl lock` rows, the ridl
reference §11 and §14.5,
  RIDL-409 to RIDL-412, and the CLI reference.

Every inbound link is repointed,
  and the links inside the two moved files now
reach `docs/wip/` from their new
  directory. Two of the repointed links are in
`docs/ROADMAP.md`, which pull
  request #394 also has open; the two lines are far
from its hunks, and the
  coordination issue records that L5 touched the file., 319

* docs(ridl):
  repair what review pass 1 found

Eight findings, three of them Important.

-
  §17.7 said the catalog descriptor and `ridl describe` had landed. They have
 
  not: Epic 16 is unstarted, `ridlc` has no `catalog` emit and `ridl` has no
 
  `describe`. The record now names the descriptor as specified and scheduled,
  which is what the disposition actually rests on, and the family overview says

   the same.
- Appendix G still defined quarantine as withholding an invalid
  stream element
  and continuing, citing §12.4 — the behaviour §12.4 now
  rules out. The entry is
  restated over both §12.4 and §4.5.
- ADR-0015
  decision 9 still quoted the coherence rule with the group identified
  by the
  interface name, and said the text transplants as it stands, while the
 
  reference now identifies the group by its lock number. Decision 9 carries a
 
  dated amendment, the record's amendment header names it, and "Documents to
 
  amend" carries the row.
- §12.4 gave a result-union element type two opposite
  meanings: a terminal error
  element that closes the stream, and a failure the
  consumer reads past. It now
  says what the element type does record — which
  stratum a failure lands in —
  and puts what the producer does after an
  error element where it belongs, in
  behaviour rather than in the contract.
-
  Appendix F's SOME/IP eventgroups row still called the ridl equivalent
 
  interface-level subscription granularity, which §5.1 contradicts.
- §17.14
  named RIDL-409 for a dropped lock entry; RIDL-409 is for an entry left
 
  behind with no declaration, which is the shape a form switch leaves.
- rxdl
  §11's acquisition/query row now names the same version and the same
 
  reopening observation as ridl §17.10, which owns it.
- Three figures of
  speech removed, against the plain-and-literal prose rule., 319

* docs(docs):
  restate the stream clause in the open-question index

Review pass 2 found the
  family overview's §17 summary still carrying the
framing the previous commit
  removed from the ridl reference's §12.4: that
continuing past a failing
  element is what a result-union element type declares.
What it declares is
  which stratum the failure lands in — with a plain element
type a constraint
  violation ends the call, and a failure declared as data is
Stratum 1, which
  does not., [#319]
- **typl:** take the constraint error from ridl-rt, not from codegen ([#404])
  ([b638e63])
- **docs:** list lane M in the step 1 lanes plan ([#405]) ([d3248a1])
- **ridl-rt:** read an absent EncodedSizes field as unsized, not uncarriable
  ([#403]) ([f3b3b14])
- **docs:** design the interaction face MVP ([#394]) ([a720d6c])
- **typl:** dispose of every open question in typl §17 ([#400]) ([f45ea99]),
  closes 245.
- **typl:** refresh the value-objects plan against the code ([#396]) ([72bb9da])
- **docs:** correct the lane C driver and the lanes plan for drift ([#392])
  ([86e10d7])
- **docs:** drive the interaction-face MVP as lane M ([#390]) ([d2e9d41])
- **ridl-rt:** state the port contracts that the signatures cannot ([#389])
  ([4405950])
- **roadmap:** sequence the lock and the catalog descriptor into step 1 ([#383])
  ([cc00580])
- **rsdl:** plan the rsdl parse, check and lowering, and refile Epic 6 ([#360])
  ([f44d475])
- **docs:** plan the lock file and ridl lock ([#358]) ([389e7f0])
- **adr:** state how the toolchain pin follows the latest stable release
  ([#357]) ([4b11633])
- **docs:** plan ridl-rt 0.1 ([#343]) ([bcb2855]), refs [#316]
- **rsdl:** rewrite the rsdl reference ([#333]) ([ab5e0c6])
- **docs:** design the lock file and ridl lock ([#334]) ([05e3100])
- **docs:** design ridl-rt 0.1 ([#332]) ([052ec5f]), refs [#316], [#308],
  [#309], [#328], [#316], [#328], [#316]
- **docs:** plan the step 1 lanes and write a driver prompt per lane ([#329])
  ([c94191a])
- **docs:** settle D-7's two internal tensions and record the rename consequence
  ([#325]) ([b1c43fa])
- **docs:** plan the catalog descriptor implementation ([#324]) ([ee3c57d])
- **docs:** design the runtime descriptors an engine reads ([#323]) ([fe3e918])
- **adr:** record ADR-0020 and amend the records the re-scope changed ([#312])
  ([52952b3])
- **docs:** record the rsdl rewrite decisions of 2026-09-12 ([#313]) ([0228902])
- **roadmap:** rewrite the roadmap as a two-step forward plan ([#310])
  ([69def56])
- **docs:** archive the superseded working notes and record the re-scope
  ([505914e])
- **docs:** garden the E9.7 working memory into the archive ([#240]) ([e1b55db])
- **adr:** field absence is declared once and realised per target ([#239])
  ([8a5b960])
- **roadmap:** put the canonical-encoding question in front of E4.5 ([#232])
  ([851bf99])
- **docs:** repair a mangled heading and the drift the pbjson move left ([#229])
  ([374bdd7])
- **repo:** close out the E9.1 to E9.6 block ([#224]) ([a90962a])
- **ridl:** state the coherence rule beside the service definition ([#222])
  ([899b8da])
- **adr:** ratify ADR-0014 and ADR-0015, and plan E9.1 to E9.6 ([#215])
  ([3d76965])
- **repo:** add status badges and link the published book ([#216]) ([8fae995])
- **docs:** align the book's front matter with ADR-0012 and the README ([#210])
  ([da5ca78])
- **repo:** bring the README and CONTRIBUTING in line with ADR-0012 ([#209])
  ([7e6c4e5])
- **roadmap:** link ADRs and design notes from prose, leave tables plain
  ([#208]) ([361cac0])
- **roadmap:** fold the wip designs into epics 9 and 10 ([#207]) ([c853c27])
- **rmdl:** cite GRust by author and thesis, not by industrial affiliation
  ([#206]) ([4ab9dd9])
- **docs:** add the language reference section to the book ([#197]) ([6440194])
- **docs:** write the CLI reference page ([#193]) ([886f4d7])
- **docs:** build the book foundation with compiled examples ([#187])
  ([58b404b])
- **repo:** garden epic E2 working memory and sync docs as-built ([#183])
  ([b4f7809]), refs 172

* docs(adr): correct four stale sentences found by
  reviewing the fix

Verification of the correction round on this branch found
  four more stale
sentences in ADR-0008's Status section, three of them written
  by that round.

- The fourth extension claimed the falsifying evidence for its
  own miss
  "again" sat inside the correcting commit. It did not:
 
  crates/ridl-syntax/src/keywords.rs is in no commit of this pull request.
  The
  claim inverted the mechanism, asserting the easy case where the
  document had
  just lived the hard one.
- The CI reading said every run completes "in five to
  eight seconds", which is false for 19 of the 247 runs, one of them by 65
  minutes. The
  duration was never load-bearing; the sentence now carries what
  is, which is that no job starts.
- Two sentences said the retired timing
  characterisation was gone from
  "the repository" when docs/archive/ still
  carries it. Both now name the
  live tree.

The running total moves to
  twenty-two, eight of which were found by
reviewing a correction rather than by
  making one.
- **adr:** sweep ADR-0008 against the shipped repository ([#179]) ([06c3f7a]),
  closes [#169]
- **ridl:** reconcile the reference with the shipped toolchain (E2 close-out)
  ([#173]) ([256bb52]), refs ADR-0008 decisions 1, 16-21, #172

* docs(ridl):
  close review findings on the E2 close-out reconciliation

Two blockers and
  three rulings from the review of fa9b731.

**The deferral of two RIDL-108
  sites rested on a false cost.** The PR body
and issue #172 claimed that
  editing the doc comment on
`ridl-diag-showcase/main/timing.ridl` regenerates
  the IR, Rust and TypeScript
snapshots. It does not: that entry carries error
  diagnostics, so all three
snapshots are one-line placeholders and `grep -rl
  "Equal bounds"` over
`crates/ridlc/tests/snapshots/` returns nothing. Editing
  it regenerates zero
snapshots — confirmed by running the suite, which passes
  with no snapshot
written. With the cost gone the deferral has no basis, so
  both sites are
fixed here: the corpus comment ("the same thing as `@250ms`"
  — the worked
example of RIDL-108 asserting the claim decision 17 retires)
  and the
`RIDL_108` doc comment in `crates/ridl-core/src/diag.rs`. Decision
  17's fix
is now four of four sites in code and text; correcting the ADR's
  own
"those are the only sites" sentence is deferred to a dedicated ADR-sweep
  PR, because ADR-0008's Status makes a whole-document sweep a precondition
  for
editing it.

**The new overview §7 opened with a closed enumeration that
  is false.** It
read "Four namespaces are in play. Two belong to a profile"
  while naming five
languages two sentences later; the specification set carries
  five profile
namespaces — `UXDL-` (uxdl §15), `RMDL-` (rmdl §11) and
  `RSDL-` (rsdl §12)
besides `TYPL-` and `RIDL-`. Restated as one namespace per
  profile, with the
two implemented today distinguished from the three specified
  ahead of their
layers.

**ridl §6.1's tuple clause is wrong, and so is typl
  §11's.** ridl §6.1 said
"tuples of parameters as in typl §11"; typl §11
  said higher profiles use
tuples as "interaction parameters and query returns".
  Verified:
`command setRange(w: (min: Speed, max: Speed))` is, ADR-0008
  decisions 17, [#18], [#172]
- **adr:** E2 close-out amendments — ADR-0008 decisions 16 to 21 ([#165])
  ([b32a171])

### BREAKING CHANGES

- an enum set's inner value is now private; read it with
get().
- generated code no longer exposes the inner field. Read it
with get() or From, and construct with new() or new_unchecked().
- --emit rust writes two additional files in package and
workspace mode. Single-file mode is unchanged.
- `ridlc build --emit c-header` is removed, and generated Rust and
TypeScript no longer carry interfaces, services or the five interaction kinds.

[83bcc10]: https://github.com/driftsys/ridl/commit/83bcc10
[#448]: https://github.com/driftsys/ridl/issues/448
[#471]: https://github.com/driftsys/ridl/issues/471
[d36e97c]: https://github.com/driftsys/ridl/commit/d36e97c
[#464]: https://github.com/driftsys/ridl/issues/464
[86526be]: https://github.com/driftsys/ridl/commit/86526be
[#459]: https://github.com/driftsys/ridl/issues/459
[#457]: https://github.com/driftsys/ridl/issues/457
[5b5a0ce]: https://github.com/driftsys/ridl/commit/5b5a0ce
[#451]: https://github.com/driftsys/ridl/issues/451
[f7882b3]: https://github.com/driftsys/ridl/commit/f7882b3
[#407]: https://github.com/driftsys/ridl/issues/407
[1d43aa4]: https://github.com/driftsys/ridl/commit/1d43aa4
[#406]: https://github.com/driftsys/ridl/issues/406
[e3a1071]: https://github.com/driftsys/ridl/commit/e3a1071
[#402]: https://github.com/driftsys/ridl/issues/402
[#395]: https://github.com/driftsys/ridl/issues/395
[a7ac5c8]: https://github.com/driftsys/ridl/commit/a7ac5c8
[#359]: https://github.com/driftsys/ridl/issues/359
[2a4a47d]: https://github.com/driftsys/ridl/commit/2a4a47d
[#330]: https://github.com/driftsys/ridl/issues/330
[#340]: https://github.com/driftsys/ridl/issues/340
[21a41f4]: https://github.com/driftsys/ridl/commit/21a41f4
[#233]: https://github.com/driftsys/ridl/issues/233
[#230]: https://github.com/driftsys/ridl/issues/230
[4cf2a51]: https://github.com/driftsys/ridl/commit/4cf2a51
[#228]: https://github.com/driftsys/ridl/issues/228
[0f5a712]: https://github.com/driftsys/ridl/commit/0f5a712
[#227]: https://github.com/driftsys/ridl/issues/227
[4762d28]: https://github.com/driftsys/ridl/commit/4762d28
[#226]: https://github.com/driftsys/ridl/issues/226
[a31bcd2]: https://github.com/driftsys/ridl/commit/a31bcd2
[#225]: https://github.com/driftsys/ridl/issues/225
[ec96247]: https://github.com/driftsys/ridl/commit/ec96247
[#214]: https://github.com/driftsys/ridl/issues/214
[8daf58d]: https://github.com/driftsys/ridl/commit/8daf58d
[#200]: https://github.com/driftsys/ridl/issues/200
[2ba8690]: https://github.com/driftsys/ridl/commit/2ba8690
[#198]: https://github.com/driftsys/ridl/issues/198
[fa62c08]: https://github.com/driftsys/ridl/commit/fa62c08
[#195]: https://github.com/driftsys/ridl/issues/195
[c40f65e]: https://github.com/driftsys/ridl/commit/c40f65e
[#192]: https://github.com/driftsys/ridl/issues/192
[499ec32]: https://github.com/driftsys/ridl/commit/499ec32
[#189]: https://github.com/driftsys/ridl/issues/189
[75c02c2]: https://github.com/driftsys/ridl/commit/75c02c2
[#186]: https://github.com/driftsys/ridl/issues/186
[519ad66]: https://github.com/driftsys/ridl/commit/519ad66
[#185]: https://github.com/driftsys/ridl/issues/185
[#177]: https://github.com/driftsys/ridl/issues/177
[dc8cad0]: https://github.com/driftsys/ridl/commit/dc8cad0
[#184]: https://github.com/driftsys/ridl/issues/184
[#182]: https://github.com/driftsys/ridl/issues/182
[da3d246]: https://github.com/driftsys/ridl/commit/da3d246
[#178]: https://github.com/driftsys/ridl/issues/178
[af7ef7c]: https://github.com/driftsys/ridl/commit/af7ef7c
[#176]: https://github.com/driftsys/ridl/issues/176
[ebebaf7]: https://github.com/driftsys/ridl/commit/ebebaf7
[#174]: https://github.com/driftsys/ridl/issues/174
[8705450]: https://github.com/driftsys/ridl/commit/8705450
[#168]: https://github.com/driftsys/ridl/issues/168
[3dc6f30]: https://github.com/driftsys/ridl/commit/3dc6f30
[#164]: https://github.com/driftsys/ridl/issues/164
[516dff0]: https://github.com/driftsys/ridl/commit/516dff0
[#162]: https://github.com/driftsys/ridl/issues/162
[4230fd8]: https://github.com/driftsys/ridl/commit/4230fd8
[#160]: https://github.com/driftsys/ridl/issues/160
[0c8f3ab]: https://github.com/driftsys/ridl/commit/0c8f3ab
[#158]: https://github.com/driftsys/ridl/issues/158
[8eb3421]: https://github.com/driftsys/ridl/commit/8eb3421
[#489]: https://github.com/driftsys/ridl/issues/489
[9069d2c]: https://github.com/driftsys/ridl/commit/9069d2c
[#483]: https://github.com/driftsys/ridl/issues/483
[330e962]: https://github.com/driftsys/ridl/commit/330e962
[#479]: https://github.com/driftsys/ridl/issues/479
[5deab17]: https://github.com/driftsys/ridl/commit/5deab17
[#475]: https://github.com/driftsys/ridl/issues/475
[5f05bdc]: https://github.com/driftsys/ridl/commit/5f05bdc
[#474]: https://github.com/driftsys/ridl/issues/474
[80773b8]: https://github.com/driftsys/ridl/commit/80773b8
[#468]: https://github.com/driftsys/ridl/issues/468
[#463]: https://github.com/driftsys/ridl/issues/463
[c5c582c]: https://github.com/driftsys/ridl/commit/c5c582c
[#466]: https://github.com/driftsys/ridl/issues/466
[261724f]: https://github.com/driftsys/ridl/commit/261724f
[#465]: https://github.com/driftsys/ridl/issues/465
[0ade5b1]: https://github.com/driftsys/ridl/commit/0ade5b1
[20c5f1e]: https://github.com/driftsys/ridl/commit/20c5f1e
[#462]: https://github.com/driftsys/ridl/issues/462
[196989a]: https://github.com/driftsys/ridl/commit/196989a
[#458]: https://github.com/driftsys/ridl/issues/458
[77c5e59]: https://github.com/driftsys/ridl/commit/77c5e59
[#461]: https://github.com/driftsys/ridl/issues/461
[8e5a552]: https://github.com/driftsys/ridl/commit/8e5a552
[#443]: https://github.com/driftsys/ridl/issues/443
[f9b2904]: https://github.com/driftsys/ridl/commit/f9b2904
[#439]: https://github.com/driftsys/ridl/issues/439
[61a6c35]: https://github.com/driftsys/ridl/commit/61a6c35
[#436]: https://github.com/driftsys/ridl/issues/436
[3fdb631]: https://github.com/driftsys/ridl/commit/3fdb631
[#438]: https://github.com/driftsys/ridl/issues/438
[7c9d9c4]: https://github.com/driftsys/ridl/commit/7c9d9c4
[#433]: https://github.com/driftsys/ridl/issues/433
[ddcbe85]: https://github.com/driftsys/ridl/commit/ddcbe85
[#420]: https://github.com/driftsys/ridl/issues/420
[2d26b38]: https://github.com/driftsys/ridl/commit/2d26b38
[#418]: https://github.com/driftsys/ridl/issues/418
[fe4c44b]: https://github.com/driftsys/ridl/commit/fe4c44b
[#417]: https://github.com/driftsys/ridl/issues/417
[564bc14]: https://github.com/driftsys/ridl/commit/564bc14
[#415]: https://github.com/driftsys/ridl/issues/415
[#252]: https://github.com/driftsys/ridl/issues/252
[529d93e]: https://github.com/driftsys/ridl/commit/529d93e
[#413]: https://github.com/driftsys/ridl/issues/413
[#247]: https://github.com/driftsys/ridl/issues/247
[d8b7a59]: https://github.com/driftsys/ridl/commit/d8b7a59
[#412]: https://github.com/driftsys/ridl/issues/412
[#246]: https://github.com/driftsys/ridl/issues/246
[fdcf2ca]: https://github.com/driftsys/ridl/commit/fdcf2ca
[#391]: https://github.com/driftsys/ridl/issues/391
[0504220]: https://github.com/driftsys/ridl/commit/0504220
[#388]: https://github.com/driftsys/ridl/issues/388
[5f24ea7]: https://github.com/driftsys/ridl/commit/5f24ea7
[#351]: https://github.com/driftsys/ridl/issues/351
[#350]: https://github.com/driftsys/ridl/issues/350
[#316]: https://github.com/driftsys/ridl/issues/316
[91e6b62]: https://github.com/driftsys/ridl/commit/91e6b62
[#348]: https://github.com/driftsys/ridl/issues/348
[2fa924c]: https://github.com/driftsys/ridl/commit/2fa924c
[#327]: https://github.com/driftsys/ridl/issues/327
[f2efb61]: https://github.com/driftsys/ridl/commit/f2efb61
[#331]: https://github.com/driftsys/ridl/issues/331
[0199406]: https://github.com/driftsys/ridl/commit/0199406
[#303]: https://github.com/driftsys/ridl/issues/303
[7d539bc]: https://github.com/driftsys/ridl/commit/7d539bc
[#241]: https://github.com/driftsys/ridl/issues/241
[1e674fe]: https://github.com/driftsys/ridl/commit/1e674fe
[#242]: https://github.com/driftsys/ridl/issues/242
[b4c420f]: https://github.com/driftsys/ridl/commit/b4c420f
[#238]: https://github.com/driftsys/ridl/issues/238
[89b8604]: https://github.com/driftsys/ridl/commit/89b8604
[#223]: https://github.com/driftsys/ridl/issues/223
[c560e3d]: https://github.com/driftsys/ridl/commit/c560e3d
[#221]: https://github.com/driftsys/ridl/issues/221
[9a5eb49]: https://github.com/driftsys/ridl/commit/9a5eb49
[#219]: https://github.com/driftsys/ridl/issues/219
[ef3c9a5]: https://github.com/driftsys/ridl/commit/ef3c9a5
[#217]: https://github.com/driftsys/ridl/issues/217
[ded42ef]: https://github.com/driftsys/ridl/commit/ded42ef
[#204]: https://github.com/driftsys/ridl/issues/204
[2513e70]: https://github.com/driftsys/ridl/commit/2513e70
[#199]: https://github.com/driftsys/ridl/issues/199
[a3a4700]: https://github.com/driftsys/ridl/commit/a3a4700
[#188]: https://github.com/driftsys/ridl/issues/188
[#172]: https://github.com/driftsys/ridl/issues/172
[96efb74]: https://github.com/driftsys/ridl/commit/96efb74
[#157]: https://github.com/driftsys/ridl/issues/157
[364dc4b]: https://github.com/driftsys/ridl/commit/364dc4b
[#156]: https://github.com/driftsys/ridl/issues/156
[99fe470]: https://github.com/driftsys/ridl/commit/99fe470
[#155]: https://github.com/driftsys/ridl/issues/155
[6e6abe5]: https://github.com/driftsys/ridl/commit/6e6abe5
[#153]: https://github.com/driftsys/ridl/issues/153
[3729961]: https://github.com/driftsys/ridl/commit/3729961
[#154]: https://github.com/driftsys/ridl/issues/154
[7b3c831]: https://github.com/driftsys/ridl/commit/7b3c831
[#152]: https://github.com/driftsys/ridl/issues/152
[450f2b9]: https://github.com/driftsys/ridl/commit/450f2b9
[#150]: https://github.com/driftsys/ridl/issues/150
[dfcc83b]: https://github.com/driftsys/ridl/commit/dfcc83b
[#149]: https://github.com/driftsys/ridl/issues/149
[5266ccb]: https://github.com/driftsys/ridl/commit/5266ccb
[#151]: https://github.com/driftsys/ridl/issues/151
[00e666a]: https://github.com/driftsys/ridl/commit/00e666a
[#148]: https://github.com/driftsys/ridl/issues/148
[62bc8ed]: https://github.com/driftsys/ridl/commit/62bc8ed
[#146]: https://github.com/driftsys/ridl/issues/146
[469047a]: https://github.com/driftsys/ridl/commit/469047a
[#147]: https://github.com/driftsys/ridl/issues/147
[20fba18]: https://github.com/driftsys/ridl/commit/20fba18
[#220]: https://github.com/driftsys/ridl/issues/220
[4cffb74]: https://github.com/driftsys/ridl/commit/4cffb74
[#181]: https://github.com/driftsys/ridl/issues/181
[#180]: https://github.com/driftsys/ridl/issues/180
[6f15e77]: https://github.com/driftsys/ridl/commit/6f15e77
[#175]: https://github.com/driftsys/ridl/issues/175
[2f8ff9b]: https://github.com/driftsys/ridl/commit/2f8ff9b
[#171]: https://github.com/driftsys/ridl/issues/171
[9684914]: https://github.com/driftsys/ridl/commit/9684914
[#478]: https://github.com/driftsys/ridl/issues/478
[26214fd]: https://github.com/driftsys/ridl/commit/26214fd
[#477]: https://github.com/driftsys/ridl/issues/477
[982c8d8]: https://github.com/driftsys/ridl/commit/982c8d8
[#454]: https://github.com/driftsys/ridl/issues/454
[a7c8051]: https://github.com/driftsys/ridl/commit/a7c8051
[#446]: https://github.com/driftsys/ridl/issues/446
[e44e885]: https://github.com/driftsys/ridl/commit/e44e885
[#447]: https://github.com/driftsys/ridl/issues/447
[512a478]: https://github.com/driftsys/ridl/commit/512a478
[#440]: https://github.com/driftsys/ridl/issues/440
[6fb3456]: https://github.com/driftsys/ridl/commit/6fb3456
[#429]: https://github.com/driftsys/ridl/issues/429
[0b3e69f]: https://github.com/driftsys/ridl/commit/0b3e69f
[#435]: https://github.com/driftsys/ridl/issues/435
[f6b749e]: https://github.com/driftsys/ridl/commit/f6b749e
[#432]: https://github.com/driftsys/ridl/issues/432
[3504a90]: https://github.com/driftsys/ridl/commit/3504a90
[#431]: https://github.com/driftsys/ridl/issues/431
[b6544fe]: https://github.com/driftsys/ridl/commit/b6544fe
[#419]: https://github.com/driftsys/ridl/issues/419
[#393]: https://github.com/driftsys/ridl/issues/393
[566f024]: https://github.com/driftsys/ridl/commit/566f024
[#411]: https://github.com/driftsys/ridl/issues/411
[8746585]: https://github.com/driftsys/ridl/commit/8746585
[#410]: https://github.com/driftsys/ridl/issues/410
[3988701]: https://github.com/driftsys/ridl/commit/3988701
[#409]: https://github.com/driftsys/ridl/issues/409
[25db3a3]: https://github.com/driftsys/ridl/commit/25db3a3
[#408]: https://github.com/driftsys/ridl/issues/408
[6d21c59]: https://github.com/driftsys/ridl/commit/6d21c59
[#401]: https://github.com/driftsys/ridl/issues/401
[#319]: https://github.com/driftsys/ridl/issues/319
[b638e63]: https://github.com/driftsys/ridl/commit/b638e63
[#404]: https://github.com/driftsys/ridl/issues/404
[d3248a1]: https://github.com/driftsys/ridl/commit/d3248a1
[#405]: https://github.com/driftsys/ridl/issues/405
[f3b3b14]: https://github.com/driftsys/ridl/commit/f3b3b14
[#403]: https://github.com/driftsys/ridl/issues/403
[a720d6c]: https://github.com/driftsys/ridl/commit/a720d6c
[#394]: https://github.com/driftsys/ridl/issues/394
[f45ea99]: https://github.com/driftsys/ridl/commit/f45ea99
[#400]: https://github.com/driftsys/ridl/issues/400
[72bb9da]: https://github.com/driftsys/ridl/commit/72bb9da
[#396]: https://github.com/driftsys/ridl/issues/396
[86e10d7]: https://github.com/driftsys/ridl/commit/86e10d7
[#392]: https://github.com/driftsys/ridl/issues/392
[d2e9d41]: https://github.com/driftsys/ridl/commit/d2e9d41
[#390]: https://github.com/driftsys/ridl/issues/390
[4405950]: https://github.com/driftsys/ridl/commit/4405950
[#389]: https://github.com/driftsys/ridl/issues/389
[cc00580]: https://github.com/driftsys/ridl/commit/cc00580
[#383]: https://github.com/driftsys/ridl/issues/383
[f44d475]: https://github.com/driftsys/ridl/commit/f44d475
[#360]: https://github.com/driftsys/ridl/issues/360
[389e7f0]: https://github.com/driftsys/ridl/commit/389e7f0
[#358]: https://github.com/driftsys/ridl/issues/358
[4b11633]: https://github.com/driftsys/ridl/commit/4b11633
[#357]: https://github.com/driftsys/ridl/issues/357
[bcb2855]: https://github.com/driftsys/ridl/commit/bcb2855
[#343]: https://github.com/driftsys/ridl/issues/343
[ab5e0c6]: https://github.com/driftsys/ridl/commit/ab5e0c6
[#333]: https://github.com/driftsys/ridl/issues/333
[05e3100]: https://github.com/driftsys/ridl/commit/05e3100
[#334]: https://github.com/driftsys/ridl/issues/334
[052ec5f]: https://github.com/driftsys/ridl/commit/052ec5f
[#332]: https://github.com/driftsys/ridl/issues/332
[#308]: https://github.com/driftsys/ridl/issues/308
[#309]: https://github.com/driftsys/ridl/issues/309
[#328]: https://github.com/driftsys/ridl/issues/328
[c94191a]: https://github.com/driftsys/ridl/commit/c94191a
[#329]: https://github.com/driftsys/ridl/issues/329
[b1c43fa]: https://github.com/driftsys/ridl/commit/b1c43fa
[#325]: https://github.com/driftsys/ridl/issues/325
[ee3c57d]: https://github.com/driftsys/ridl/commit/ee3c57d
[#324]: https://github.com/driftsys/ridl/issues/324
[fe3e918]: https://github.com/driftsys/ridl/commit/fe3e918
[#323]: https://github.com/driftsys/ridl/issues/323
[52952b3]: https://github.com/driftsys/ridl/commit/52952b3
[#312]: https://github.com/driftsys/ridl/issues/312
[0228902]: https://github.com/driftsys/ridl/commit/0228902
[#313]: https://github.com/driftsys/ridl/issues/313
[69def56]: https://github.com/driftsys/ridl/commit/69def56
[#310]: https://github.com/driftsys/ridl/issues/310
[505914e]: https://github.com/driftsys/ridl/commit/505914e
[e1b55db]: https://github.com/driftsys/ridl/commit/e1b55db
[#240]: https://github.com/driftsys/ridl/issues/240
[8a5b960]: https://github.com/driftsys/ridl/commit/8a5b960
[#239]: https://github.com/driftsys/ridl/issues/239
[851bf99]: https://github.com/driftsys/ridl/commit/851bf99
[#232]: https://github.com/driftsys/ridl/issues/232
[374bdd7]: https://github.com/driftsys/ridl/commit/374bdd7
[#229]: https://github.com/driftsys/ridl/issues/229
[a90962a]: https://github.com/driftsys/ridl/commit/a90962a
[#224]: https://github.com/driftsys/ridl/issues/224
[899b8da]: https://github.com/driftsys/ridl/commit/899b8da
[#222]: https://github.com/driftsys/ridl/issues/222
[3d76965]: https://github.com/driftsys/ridl/commit/3d76965
[#215]: https://github.com/driftsys/ridl/issues/215
[8fae995]: https://github.com/driftsys/ridl/commit/8fae995
[#216]: https://github.com/driftsys/ridl/issues/216
[da5ca78]: https://github.com/driftsys/ridl/commit/da5ca78
[#210]: https://github.com/driftsys/ridl/issues/210
[7e6c4e5]: https://github.com/driftsys/ridl/commit/7e6c4e5
[#209]: https://github.com/driftsys/ridl/issues/209
[361cac0]: https://github.com/driftsys/ridl/commit/361cac0
[#208]: https://github.com/driftsys/ridl/issues/208
[c853c27]: https://github.com/driftsys/ridl/commit/c853c27
[#207]: https://github.com/driftsys/ridl/issues/207
[4ab9dd9]: https://github.com/driftsys/ridl/commit/4ab9dd9
[#206]: https://github.com/driftsys/ridl/issues/206
[6440194]: https://github.com/driftsys/ridl/commit/6440194
[#197]: https://github.com/driftsys/ridl/issues/197
[886f4d7]: https://github.com/driftsys/ridl/commit/886f4d7
[#193]: https://github.com/driftsys/ridl/issues/193
[58b404b]: https://github.com/driftsys/ridl/commit/58b404b
[#187]: https://github.com/driftsys/ridl/issues/187
[b4f7809]: https://github.com/driftsys/ridl/commit/b4f7809
[#183]: https://github.com/driftsys/ridl/issues/183
[06c3f7a]: https://github.com/driftsys/ridl/commit/06c3f7a
[#179]: https://github.com/driftsys/ridl/issues/179
[#169]: https://github.com/driftsys/ridl/issues/169
[256bb52]: https://github.com/driftsys/ridl/commit/256bb52
[#173]: https://github.com/driftsys/ridl/issues/173
[#18]: https://github.com/driftsys/ridl/issues/18
[b32a171]: https://github.com/driftsys/ridl/commit/b32a171
[#165]: https://github.com/driftsys/ridl/issues/165
