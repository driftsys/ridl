# The Pinned Name Transform — correcting the projection note before E9.8

| Field     | Value                                                                                                                             |
| --------- | --------------------------------------------------------------------------------------------------------------------------------- |
| Status    | design, approved — to be ratified as ADR-0016                                                                                     |
| Date      | 2026-08-05                                                                                                                        |
| Origin    | executing E9.7, which pins one name transform before a nominal-identity backend makes the transform's output part of the contract |
| Scope     | the transform's algorithm, its home, the collision rule that replaces the injectivity requirement, and what ADR-0016 records      |
| Companion | `2026-08-03-schema-projection-design.md`, whose §3, §5, and §7.2 this document corrects and whose remaining decisions it ratifies |

A bare section reference — §3, §7.2 — is to the companion note. References to
this document are marked _above_ or _below_. Section references to a language
reference name it in full.

## 1. Why the transform stopped being an implementation detail

Every backend renders a ridl name into its target's conventions. `currentSpeed`
becomes `current_speed` for a Rust method and for a C function, and today that
output is a local convenience: it appears inside one generated file and nothing
outside depends on it.

Appendix B records that proto RPC identity is **nominal** — the ordinal never
reaches the wire, the method name does. When E9.8 emits the first proto
projection, the transform's output becomes part of the deployed contract, and
changing the transform afterwards renames methods on a live wire. §5 of the
companion note draws that conclusion and E9.7 acts on it.

The repository holds two implementations with different algorithms,
`crates/ridl-backend-rust/src/interact.rs` and
`crates/ridl-backend-rust/src/c_header.rs`. One of them has to be pinned.

## 2. Three findings that change what E9.7 must do

### 2.1 §7.2's tie-breaker does not discriminate

§7.2 records the choice as settled "by inspection rather than by preference",
tracing `getVIN` to `get_vin`.

Both implementations produce `get_vin` for `getVIN`. The capital run reaches the
end of the identifier, so no capital is followed by a lower-case character, so
the clause that distinguishes the two algorithms never fires. The example was
chosen to separate them and cannot.

The identifiers that do separate them are an acronym followed by a word:

| name                | `interact.rs`        | `c_header.rs`         |
| ------------------- | -------------------- | --------------------- |
| `currentSpeed`      | `current_speed`      | `current_speed`       |
| `getVIN`            | `get_vin`            | `get_vin`             |
| `HTTPServer`        | `httpserver`         | `http_server`         |
| `IOError`           | `ioerror`            | `io_error`            |
| `parseHTTPResponse` | `parse_httpresponse` | `parse_http_response` |

On the cases that decide, `c_header.rs` produces the better name, so §7.2 pinned
the weaker algorithm on evidence that could not support the choice.

### 2.2 §7.2's injectivity requirement cannot be satisfied

§7.2 requires the transform to be "injective over the names ridl admits", so
that two interactions cannot collide after transformation. Property 4 of §3
states the same obligation.

No case-folding transform has that property. Lowercasing destroys the
information that distinguishes two identifiers, so distinct inputs necessarily
share an output. Enumerating camelCase identifiers up to six characters over a
four-character alphabet, `interact.rs` maps 2730 names onto outputs of which 776
have more than one preimage; `c_header.rs`, 744. Two realistic collisions:

```text
interact.rs   getVIN, getVIn                       -> get_vin
c_header.rs   parseHTTPResponse, parseHttpResponse -> parse_http_response
```

The requirement is therefore unsatisfiable as written, by either candidate or by
any replacement. Injectivity is not a property a transform can carry. It is a
property a **package** either has or does not have under a given transform, and
the only sound way to hold it is to check it and reject a package that fails.

### 2.3 The collision already emits Rust that does not compile

The lexer admits `[A-Za-z][A-Za-z0-9_]*` and no diagnostic constrains an
interaction member name further, so this package compiles clean today:

```text
interface Probe {
  signal vinNumber  : Speed @[100ms..1s]
  signal vin_number : Speed @[100ms..1s]
}
```

`ridl build --emit rust` writes `fn vin_number` twice into one trait and
`fn publish_vin_number` twice, and `rustc` rejects the emitted file with two
errors. Parameter names project through the same transform, so
`command setIt(vinNumber : Speed, vin_number : Speed)` emits
`async fn set_it(&self, vin_number: Speed, vin_number: Speed)`, which binds one
identifier twice in a single parameter list.

The companion note treats the collision as a risk a nominal-identity target
would introduce. It is a defect in the shipped Rust backend.

## 3. Decisions

1. **The pinned transform is `c_header.rs`'s algorithm.** A separator is
   inserted before an upper-case character that follows a lower-case character
   or a digit, or that follows an upper-case character and is itself followed by
   a lower-case character. This reverses §7.2, for the reason §2.1 above gives.

2. **The transform moves to `crates/ridl-ir/src/name.rs` and becomes public.**
   `ridl-ir` is the only crate that `ridl-sem` and both backends already depend
   on, and §7.2 defines the transform as a pure function from IR identity to a
   target's namespace, which places it with the IR rather than inside one
   backend. E9.8 and E9.9 consume it without a new dependency edge. Both
   existing copies are deleted and every caller uses the pinned function.

3. **RIDL-149 is minted — two members of one interface whose names collide after
   the pinned transform.** Error. This replaces the injectivity requirement of
   §3 property 4 and §7.2: the obligation binds the package, discharged by a
   check, rather than the function. RIDL-149 is the direct sibling of RIDL-147,
   which [ADR-0015](../decisions/ADR-0015-qos-absorption-and-rpc-bounds.md)
   decision 24 minted for interface names colliding within a service; this is
   the same fail-closed rule one level down.

4. **The check covers two namespaces — the members of one interface, and the
   parameters of one interaction.** Both are where the transform is applied
   today, and both already emit Rust that does not compile: colliding members
   give one trait two methods of the same name, and colliding parameters give
   one function two identically named arguments. Struct fields reach the Rust
   backend untransformed, so they stay out until E9.8 extends both the transform
   and this check to them in the commit that starts projecting them, which keeps
   the rule and its application in step.

5. **The check runs in `ridl-sem`, not in a backend.** The transform is fixed by
   the family rather than selected by a target, so checking it leaves `ridlc` a
   pure source-to-IR function and
   [ADR-0008](../decisions/ADR-0008-e2-execution.md) decision 9 holds.

### 3.1 Considered and rejected — a member-name form rule

typl §2.3 states that an identifier takes an underscore "only in
`SCREAMING_SNAKE`", which would make `signal vin_number` invalid and would
remove the easiest collision class at its source.

It is not adopted here, for two reasons. The collision rule already covers the
safety property: `vinNumber` beside `vin_number` collides and is rejected, and a
`vin_number` with no colliding sibling projects to itself harmlessly. And the
specification is not settled — §2.3 states the rule normatively while §15.1
presents naming as a convention, and the rule is not lexically checkable in any
case, because the lexer emits one `Ident` kind for all three identifier forms.
Resolving that ambiguity is a specification decision that should not ride along
on a projection story.

## 4. What ADR-0016 records

ADR-0016 ratifies the companion note and carries the corrections above.

**Ratified unchanged:** §3's four projection properties, with property 4
restated per decision 3 above; §6.1, that the schema hash answers "is this the
same contract" and that anything gating on it is choosing lockstep deployment;
§7.1, that the service number has no available derivation and is recorded as an
open question in rsdl §13; §7.3, that a `fixed` interaction gets a real field in
the store table; §7.4, that the dispatcher is a routing table keyed by ordinal.

**Corrected:**

- §7.2's algorithm choice, reversed — §2.1 above.
- §7.2's and §3 property 4's injectivity requirement, restated as a checked
  property of a package — §2.2 above.
- §2.1's proposal that a service's inline shape occupies slot 1, which did not
  ship: [ADR-0015](../decisions/ADR-0015-qos-absorption-and-rpc-bounds.md)
  decision 14 keeps the inline and named forms separate and classifies a switch
  between them as breaking.

**Recorded:** that the shipped Rust backend emits duplicate identifiers on
colliding names, which decision 3 closes.

## 5. Verification

- Unit tests over the pinned transform: the acronym table of §2.1 above,
  idempotence under repeated application, and preservation of an underscore
  already present.
- One `ridl-diag-showcase` corpus entry for RIDL-149.
- A property test driven from the `ridl-diff` classifier: for any delta the
  classifier calls compatible, no surviving member's projected name changes.
  Names are the only identity E9.7 pins — numbers arrive with E9.8 — so this is
  the name-level case of §3 property 3, and it stands up the harness the
  numbering case needs there.

## 6. Blast radius

Measured over the corpus, the book, the tests, and `docs/`:

- No identifier anywhere has the acronym-followed-by-word shape, so decision 1
  changes no generated output and churns no snapshot.
- Of the 166 distinct interaction member names declared, no two share an output
  under the pinned transform, so decision 3 rejects nothing that exists.

Both changes are free today. Neither is free once E9.8 stamps a name onto a
wire, which is the argument for taking E9.7 before the first projection rather
than alongside it.

## 7. Out of scope

- Struct fields, until E9.8 projects them — decision 4 above. Interaction
  parameters are **in** scope, for the reason decision 4 gives.
- Numbering stability, which needs numbers E9.7 does not assign.
- The member-name form rule and the typl §2.3 ambiguity — §3.1 above.
- The transport-binding question §7.4 leaves open: whether a byte-channel
  binding multiplexes an interface's calls over one envelope keyed by ordinal or
  gives each interaction its own channel.
