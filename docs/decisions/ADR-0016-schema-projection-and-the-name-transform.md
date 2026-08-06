# ADR-0016 — Schema projection and the pinned name transform

## Status

Accepted — 2026-08-05. Scope: the projection from IR identity to a target's
namespace — the four properties every projection must satisfy, the one name
transform, its home, and the collision rule that replaces the injectivity
requirement. It is not epic-scoped: it binds every backend that projects, in the
way ADR-0013 binds what a backend may emit and ADR-0014 binds how the IR is
encoded.

It ratifies the schema-projection note,
[`docs/wip/2026-08-03-schema-projection-design.md`](../wip/2026-08-03-schema-projection-design.md),
and corrects three of its statements. The corrections' reasoning trail is
[`docs/wip/2026-08-05-projection-name-transform-design.md`](../wip/2026-08-05-projection-name-transform-design.md),
which carries the measurements this record summarises. Throughout this record,
"the note" is the schema-projection note, and a reference of the form note §3 is
to it; language-reference sections are named in full (ridl §11, rsdl §13).

It does not supersede [ADR-0013](ADR-0013-codegen-backend-scope.md), which
classifies a backend by what its target can faithfully represent — what a
backend may emit. This record fixes how identity projects into whatever is
emitted. The two bind the same backends on different axes.

This ADR was accepted under the delegated authority recorded in
[ADR-0005](ADR-0005-agent-enablement.md)'s working model — the notes were
written for review, and execution of roadmap story E9.7 needs the decisions
fixed rather than pending.

## Context

The note answers two questions the store-and-dispatcher work raised: what
identity ridl owns, and what a projection from that identity must guarantee so
that a change ridl calls compatible does not silently renumber or rename a
deployed schema. Most of its answers hold and are ratified below as decisions 6
to 10. Executing E9.7 against the workspace disproved three of its statements,
and the corrections are what decisions 1 to 5 make operative.

**The transform stopped being an implementation detail.** Every backend renders
a ridl name into its target's conventions — `currentSpeed` becomes
`current_speed` for a Rust method and for a C function — and today that output
is a local convenience inside one generated file. ridl Appendix B records that
proto RPC identity is **nominal**: the ordinal never reaches the wire, the
method name does. When E9.8 emits the first proto projection, the transform's
output becomes part of the deployed contract, and changing the transform
afterwards renames methods on a live wire. The repository holds two
implementations with different algorithms —
`crates/ridl-backend-rust/src/interact.rs` and
`crates/ridl-backend-rust/src/c_header.rs` — and one of them has to be pinned
before that happens. Note §5 draws that conclusion and E9.7 acts on it.

**Correction 1 — the tie-breaker the note used does not discriminate.** Note
§7.2 pins `interact.rs`'s algorithm by tracing `getVIN` to `get_vin`, "by
inspection rather than by preference". Both implementations produce `get_vin`
for `getVIN`: the capital run reaches the end of the identifier, so no capital
is followed by a lower-case character, and the clause that separates the two
algorithms never fires. The identifiers that do separate them are an acronym
followed by a word:

| name                | `interact.rs`        | `c_header.rs`         |
| ------------------- | -------------------- | --------------------- |
| `currentSpeed`      | `current_speed`      | `current_speed`       |
| `getVIN`            | `get_vin`            | `get_vin`             |
| `HTTPServer`        | `httpserver`         | `http_server`         |
| `IOError`           | `ioerror`            | `io_error`            |
| `parseHTTPResponse` | `parse_httpresponse` | `parse_http_response` |

On the cases that decide, `c_header.rs` produces the better name, so note §7.2
pinned the weaker algorithm on evidence that could not support the choice.
Decision 1 reverses it.

**Correction 2 — the injectivity requirement cannot be satisfied.** Note §7.2
requires the transform to be injective over the names ridl admits, and note §3
property 4 states the same obligation. No case-folding transform has that
property: lowercasing destroys the information that distinguishes two
identifiers, so distinct inputs necessarily share an output. Measured by
enumerating camelCase identifiers up to six characters over a four-character
alphabet, `interact.rs` maps 2730 names onto outputs of which 776 have more than
one preimage; `c_header.rs`, 744. Two realistic collisions: `getVIN` and
`getVIn` both give `get_vin` under `interact.rs`; `parseHTTPResponse` and
`parseHttpResponse` both give `parse_http_response` under `c_header.rs`. The
requirement is unsatisfiable as written, by either candidate or by any
replacement. Injectivity is not a property a transform can carry; it is a
property a **package** either has or does not have under a given transform, and
the only sound way to hold it is to check it and reject a package that fails.
Decision 3 makes that replacement.

**The collision is already a shipped defect, not a future risk.** The lexer
admits `[A-Za-z][A-Za-z0-9_]*` and no diagnostic constrains an interaction
member name further, so an interface declaring both `signal vinNumber` and
`signal vin_number` compiles clean today. `ridl build --emit rust` then writes
`fn vin_number` twice into one trait and `fn publish_vin_number` twice, and
`rustc` rejects the emitted file. Parameter names project through the same
transform, so `command setIt(vinNumber : Speed, vin_number : Speed)` emits
`async fn set_it(&self, vin_number: Speed, vin_number: Speed)`, which binds one
identifier twice in a single parameter list. The note treats the collision as a
risk a nominal-identity target would introduce; it is a defect in the shipped
Rust backend, and decision 3 closes it.

**Correction 3 — the unification note §2.1 proposed did not ship.** Note §2.1
proposed that a service's inline shape occupies slot 1 so that the inline form
becomes a degenerate case of the general one rather than a separate construct.
The shipped design keeps the two forms separate:
[ADR-0015](ADR-0015-qos-absorption-and-rpc-bounds.md) decision 14 admits named
shapes or one inline shape, never both — a two-branch alternation — and the
classifier keeps a switch between the forms breaking (its decision 19 retains
that half of `ServiceChanged`, for the reason its decision 15 gives: extraction
rewrites the transport identity of every fallible query in the shape). What
survives of the proposal is the numbering alone, which its decision 15 records:
in the IR an inline shape carries interface id 1, only ever as the single slot
of an inline-form service. A projection must therefore not treat "extract the
inline shape into a named interface" as identity-preserving.

## Decision

Decisions 1 to 5 are the operative form of the corrections, and are the ones
implementation cites. Decisions 6 to 10 ratify the note unchanged.

1. **The pinned transform is `c_header.rs`'s algorithm.** A separator is
   inserted before an upper-case character that follows a lower-case character
   or a digit, or that follows an upper-case character and is itself followed by
   a lower-case character; the result is lowercased. This reverses note §7.2,
   for the reason correction 1 gives: on the cases that separate the two
   candidates, this algorithm keeps an acronym a word of its own
   (`http_server`), where the other collapses it into the word that follows
   (`httpserver`).

2. **The transform moves to `crates/ridl-ir/src/name.rs` and becomes public.**
   `ridl-ir` is the only crate that `ridl-sem` and both backends already depend
   on, and note §7.2 defines the transform as a pure function from IR identity
   to a target's namespace, which places it with the IR rather than inside one
   backend. E9.8 and E9.9 consume it without a new dependency edge. Both
   existing copies are deleted and every caller uses the pinned function. The
   transform is part of the stability guarantee, so it is specified with the
   projections in E4.5's IR stability policy, not in the attribute registry.

3. **RIDL-149 is minted — two members of one interface whose names collide after
   the pinned transform.** Error. This replaces the injectivity requirement of
   note §3 property 4 and note §7.2: the obligation binds the package,
   discharged by a check, rather than the transform function. RIDL-149 is the
   direct sibling of RIDL-147, which
   [ADR-0015](ADR-0015-qos-absorption-and-rpc-bounds.md) decision 24 minted for
   interface names colliding within a service; this is the same fail-closed rule
   one level down.

4. **The check covers two of the three namespaces the transform is applied to
   today — the members of one interface, and the parameters of one
   interaction.** The first includes a service's inline shape: an inline shape
   is an interface shape (ridl §14.5), so its members are checked the same way.
   Both covered namespaces already emit Rust that does not compile: colliding
   members give one trait two methods of the same name, and colliding parameters
   give one function two identically named arguments. The third namespace stays
   outside the check: `c_header.rs` renders a package's type names through the
   transform when it emits the extern-C header, so a package declaring
   `type HTTPServer` beside `type HttpServer` compiles with no diagnostic today
   and emits a header with two `typedef`s of one name, which no C consumer can
   compile. That defect predates this story, and extending RIDL-149 to type
   names is a known exclusion from E9.7's scope. Struct fields reach the Rust
   backend untransformed, so they stay out until E9.8 extends both the transform
   and this check to them in the commit that starts projecting them, so that the
   rule and its application change in the same commit.

5. **The check runs in `ridl-sem`, not in a backend.** The transform is fixed by
   the family rather than selected by a target, so the collision is a property
   of the package, checkable with no backend knowledge. Checking it in the
   semantic pass leaves `ridlc` a pure source-to-IR function and
   [ADR-0008](ADR-0008-e2-execution.md) decision 9 holds.

6. **Ratified — the four projection properties of note §3.** Every projection is
   **deterministic**: the same IR yields the same numbers on every run, in every
   backend, with no allocation state. It is **total**: defined for every
   interaction, or the backend fails with a diagnostic rather than emitting a
   number the target rejects. It is **stable under compatible change**: if
   `ridl-diff` returns compatible, no number already assigned may move — the
   property that needs a test rather than an argument, driven from the
   classifier. And it is **injective in scope**, restated per decision 3: no two
   interactions of a checked package collide within one message or table,
   because RIDL-149 rejected the package otherwise. Projections are part of the
   stability guarantee, recorded with E4.5's IR stability policy, not backend
   implementation detail.

7. **Ratified — note §6.1, the schema hash answers "is this the same
   contract".** It does not answer "are these two contracts compatible":
   `ridl-diff` calls several changes compatible that still alter the IR and
   therefore the hash. Anything gating attach on hash equality is choosing
   lockstep deployment and should say so, rather than discover it.

8. **Ratified — note §7.1, the service number has no derivation available.**
   Hashing the name was studied and rejected (ridl Appendix E — renames silently
   break the wire, IDs are unreadable from source), and declaration order does
   not exist to be counted, because the service catalog is a flat global
   namespace spanning packages (ridl §14.5, RIDL-140). The number is a
   deployment fact. It is recorded as an open question in rsdl §13, the
   mechanism — allocation-and-record, a registry pinned in a lockfile-shaped
   artifact — is deferred to E6 with the rest of deployment, and the question
   binds tag-based transports only, because proto and gRPC identity is nominal.

9. **Ratified — note §7.3, a `fixed` interaction gets a real field in the store
   table, not a placeholder.** A `fixed` is a value a consumer reads, and a
   field in the store table is the plain accessor ridl §8 already promises. It
   is immutable for the lifetime of the instance, so it costs the coherence
   machinery nothing.

10. **Ratified — note §7.4, the dispatcher is a routing table keyed by ordinal,
    not a nested message.** Nesting exists for coherence, and calls are not read
    together: one service definition per provided interface, its members routed
    by the interface's single ordinal sequence.

## Alternatives considered

| Candidate                                      | Verdict  | Reason                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| ---------------------------------------------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `interact.rs`'s algorithm — the note's own pin | rejected | the trace that chose it (`getVIN`) does not separate the two candidates; on the cases that do, it collapses an acronym into the word that follows (`parse_httpresponse`)                                                                                                                                                                                                                                                                                                                    |
| An injective transform                         | rejected | no case-folding transform can be injective; over the six-character enumeration both candidates leave hundreds of output classes with more than one preimage, so the obligation moves to the package and is checked (decision 3)                                                                                                                                                                                                                                                             |
| A member-name form rule                        | rejected | typl §2.3 says an identifier takes an underscore only in `SCREAMING_SNAKE`, which would remove the easiest collision class at its source — but the collision rule already covers the safety property, typl §15.1 presents naming as a convention where §2.3 reads as a rule, and the lexer emits one `Ident` kind for all three forms, so it is not lexically checkable; resolving that ambiguity is a specification decision that must not be taken as a side effect of a projection story |

## Consequences

- **Positive — a nominal-identity backend has a specified transform to build
  on.** E9.8 and E9.9 consume one public, tested function instead of choosing
  between two divergent private copies.
- **Positive — the shipped member and parameter defect closes.** A package whose
  member or parameter names collide after the transform is rejected with
  RIDL-149 instead of emitting Rust that `rustc` rejects.
- **Positive — both corrections are free today, measured.** Over the corpus, the
  book, the tests, and `docs/`: no identifier anywhere has the
  acronym-followed-by-word shape, so decision 1 changes no generated output and
  churns no snapshot; and of the 166 distinct interaction member names declared,
  no two share an output under the pinned transform, so decision 3 rejects
  nothing that exists. Neither change is free once E9.8 stamps a name onto a
  wire, which is why E9.7 lands before the first projection rather than beside
  it.
- **Negative — a new error can reject source that compiled yesterday.** The
  remedy is renaming one member, and everything such a package could emit today
  is already broken.
- **Negative — struct fields stay outside the transform and the check until E9.8
  projects them** (decision 4). Stated here rather than discovered there.
- **Negative — the C header's type names stay inside the transform but outside
  the check** (decision 4). A package whose type names collide after the
  transform still compiles with no diagnostic and emits a header no C consumer
  can compile. The defect predates this story, and extending RIDL-149 to type
  names is excluded from E9.7's scope. Recorded on driftsys/ridl#236.
- **Negative — a second transform in the Rust backend has the same defect shape
  and stays open.** `crates/ridl-backend-rust/src/lib.rs` camel-cases union arm
  names, so arms `foo_bar` and `fooBar` both emit the Rust variant `FooBar` and
  the emitted file fails to compile (E0428). That transform is not the pinned
  `snake_case` and is outside this record's scope, so RIDL-149 does not make
  "the backend never emits non-compiling output on a name collision" true.
  Recorded on driftsys/ridl#237.

## Documents amended

| Document          | Change                                                                                                                |
| ----------------- | --------------------------------------------------------------------------------------------------------------------- |
| rsdl §13          | gains the service-number open question (decision 8)                                                                   |
| `docs/ROADMAP.md` | the E9.7 row restated per decisions 1 to 3; the Epic 9 status paragraph records this ratification and the corrections |

## Open

1. **The identifier-form ambiguity.** typl §2.3 states the underscore rule
   normatively while typl §15.1 presents naming as a convention, and the lexer
   cannot tell the forms apart. Resolving it is a specification decision in its
   own right — see Alternatives considered.
2. **The transport-binding question note §7.4 leaves open:** whether a
   byte-channel binding multiplexes an interface's calls over one envelope keyed
   by ordinal, or gives each interaction its own channel.

## References

- [`docs/wip/2026-08-03-schema-projection-design.md`](../wip/2026-08-03-schema-projection-design.md)
  — the note this record ratifies and corrects
- [`docs/wip/2026-08-05-projection-name-transform-design.md`](../wip/2026-08-05-projection-name-transform-design.md)
  — the corrections' reasoning trail, with the measurements
- [ADR-0008](ADR-0008-e2-execution.md) — decision 4 (fallible transport
  identity), decision 9 (`ridlc` is a pure source-to-IR function)
- [ADR-0013](ADR-0013-codegen-backend-scope.md) — the backend classification
  this record does not supersede
- [ADR-0014](ADR-0014-ir-encodings.md) — the encodings of the IR this projection
  reads
- [ADR-0015](ADR-0015-qos-absorption-and-rpc-bounds.md) — decisions 14, 15, 17,
  19 (service shapes and their diff), decision 24 (RIDL-147, the sibling rule
  one level up)
- [`docs/specification/ridl-language-reference.md`](../specification/ridl-language-reference.md)
  — §11, §14.5, §16.4, Appendix B (nominal proto identity), Appendix E (the
  hashed-identity rejection)
- [`docs/specification/rsdl-language-reference.md`](../specification/rsdl-language-reference.md)
  — §8 (transport IDs derive from ordinals), §13 (the open question decision 8
  records)
- [`docs/ROADMAP.md`](../ROADMAP.md) — E9.7 (the story this record binds), E9.8
  and E9.9 (the projections that consume it), E4.5 (the stability policy it is
  specified with)
- `crates/ridl-backend-rust/src/interact.rs` and
  `crates/ridl-backend-rust/src/c_header.rs` — the two divergent copies decision
  2 replaces
