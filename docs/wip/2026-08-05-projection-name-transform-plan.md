# E9.7 — The Pinned Name Transform: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> `superpowers:subagent-driven-development` (recommended) or
> `superpowers:executing-plans` to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Pin one name transform for the whole family, in `ridl-ir`, and reject
a package whose names collide under it — before E9.8 makes the transform's
output part of a deployed contract.

**Architecture:** The transform moves out of `ridl-backend-rust` into `ridl-ir`,
the only crate `ridl-sem` and both backends already depend on. Both existing
copies are deleted. The injectivity requirement the design note states is
unsatisfiable for any case-folding transform, so it is replaced by RIDL-149, a
checker rule over two namespaces: the members of one interface, and the
parameters of one interaction.

**Tech stack:** Rust 2024 (workspace toolchain pinned by `rust-toolchain.toml`),
`prost`/`pbjson` IR, `insta` snapshots, `codespan-reporting` diagnostics.

**Design of record:**
[`2026-08-05-projection-name-transform-design.md`](2026-08-05-projection-name-transform-design.md),
which corrects
[`2026-08-03-schema-projection-design.md`](2026-08-03-schema-projection-design.md).

## Global Constraints

- **Conventional Commits**, linted by git-std against `.git-std.toml`. Scopes
  used by this plan: `adr`, `ridl`, `rsdl`, `ridl-ir`, `ridl-core`,
  `ridl-backend-rust`, `ridl-diff`, `docs`.
- **Never push to `main`.** All work lands on `e97-projection-name-transform`
  through a PR.
- **`just build` is the gate.** Run it before every commit that touches Rust;
  `just check` alone is enough for a docs-only commit.
- **`just fmt` before committing** any Markdown, JSON, YAML, or TOML.
- **Prose is plain and literal** — no idioms, no figures of speech, in comments,
  commit messages, docs, and diagnostic text alike.
- **Every diagnostic is coded and carries a fix-it**
  ([ADR-0005](../decisions/ADR-0005-agent-enablement.md) §7 invariant). A new
  code is registered in three places: the `diag.rs` catalogue, the ridl language
  reference §16.4 table, and both lists in `crates/ridlc/tests/corpus.rs`.
- **The transform's output must not change for any name in the corpus.** Task 3
  is expected to churn zero snapshots; a snapshot diff there is a defect in the
  task, not an expected update.

---

### Task 1: ADR-0016 and the rsdl open question

Docs only. Lands first so the implementation tasks have a record to cite.

**Files:**

- Create: `docs/decisions/ADR-0016-schema-projection-and-the-name-transform.md`
- Modify: `docs/specification/rsdl-language-reference.md` — the §13
  open-question list
- Modify: `docs/ROADMAP.md` — the E9.7 row's "Done when" column and the Epic 9
  status paragraph

**Interfaces:**

- Consumes: nothing.
- Produces: the decision numbers every later task cites in a comment or a
  diagnostic message — **decision 1** (the pinned algorithm), **decision 2**
  (the transform lives in `ridl-ir`), **decision 3** (RIDL-149 replaces the
  injectivity requirement), **decision 4** (interaction members and parameters,
  not struct fields), **decision 5** (the check runs in `ridl-sem`).

- [ ] **Step 1: Read the two records this ADR has to agree with**

Read `docs/decisions/ADR-0015-qos-absorption-and-rpc-bounds.md` decisions 14,
17, 19, and 24, and `docs/decisions/ADR-0014-ir-encodings.md` `## Status`.
ADR-0016 must not restate or contradict them: d14 keeps a service's inline shape
separate from its named shapes, d17 keys ordinal spaces on the interface name,
d19 gives the five `ServiceShape*` diff categories, d24 adds the name-uniqueness
rule and RIDL-147.

- [ ] **Step 2: Write ADR-0016**

Follow the house structure, which ADR-0015 shows: `# ADR-NNNN — <title>`, then
`## Status`, then the reasoning, then numbered decisions.

`## Status` must say: Accepted, dated 2026-08-05; scope is the projection from
IR identity to a target's namespace; it binds every backend that projects, not
one epic; it ratifies `docs/wip/2026-08-03-schema-projection-design.md` and
corrects three of its statements; it does not supersede ADR-0013, which
classifies what a backend may emit rather than how identity projects.

Ratify unchanged, each as its own numbered decision:

- the four projection properties of note §3 — deterministic, total, stable under
  compatible change, and injective in scope, with the fourth restated per
  decision 3 below;
- note §6.1 — the schema hash answers "is this the same contract", not "are
  these compatible", so anything gating attach on hash equality is choosing
  lockstep deployment and should say so;
- note §7.1 — the service number has no derivation available, is a deployment
  fact, and is recorded as an open question in rsdl §13;
- note §7.3 — a `fixed` interaction gets a real field in the store table, not a
  placeholder;
- note §7.4 — the dispatcher is a routing table keyed by ordinal, not a nested
  message.

Then the corrections, each stating what the note said and why it does not hold:

- **The algorithm choice is reversed.** Note §7.2 settles it by tracing
  `getVIN`, which yields `get_vin` under both implementations, because the
  capital run reaches the end of the identifier and the clause that separates
  the two algorithms never fires. The cases that decide are an acronym followed
  by a word: `HTTPServer` gives `httpserver` under the algorithm the note pinned
  and `http_server` under the other.
- **The injectivity requirement becomes a checked property.** No case-folding
  transform can be injective, because lowercasing destroys what distinguishes
  two identifiers. Enumerating camelCase identifiers up to six characters over a
  four-character alphabet, one algorithm maps 2730 names onto outputs of which
  776 have more than one preimage, and the other 744. The obligation therefore
  binds the package, discharged by RIDL-149.
- **Note §2.1's "an inline shape is slot 1" did not ship.** ADR-0015 decision 14
  keeps the inline and named forms separate and classifies a switch between them
  as breaking.

Record also that the shipped Rust backend emits duplicate identifiers on
colliding names today — `fn vin_number` twice in one trait for an interface
declaring `vinNumber` and `vin_number`, and two identically named arguments in
one function for a command declaring both as parameters — and that decision 3
closes it.

- [ ] **Step 3: Add the service-number open question to rsdl §13**

Find the §13 open-question list in
`docs/specification/rsdl-language-reference.md` and add one entry, matching the
surrounding entries' shape. It must state: a service's own number has no
derivation in any layer; rsdl §8 derives method and event IDs from ridl §11
ordinals but says nothing about the service number; hashing the name was studied
and rejected in Appendix E; declaration order does not exist to be counted
because the service catalog is a flat global namespace spanning packages (§14.5,
RIDL-140); what remains is allocation-and-record, a registry pinned in a
lockfile-shaped artifact, deferred to E6 with the rest of deployment; and the
question binds tag-based transports only, because proto and gRPC identity is
nominal. Cite ADR-0016.

- [ ] **Step 4: Update the roadmap**

In `docs/ROADMAP.md`, the E9.7 row's "Done when" column currently reads "one
transform, two interactions never collide after it". The second clause is the
unsatisfiable property. Replace the cell with: "one transform, in `ridl-ir`; a
package whose names collide after it is rejected". Add one sentence to the Epic
9 status paragraph recording that ADR-0016 ratified the fourth design note and
corrected three of its statements.

- [ ] **Step 5: Format and check**

```bash
just fmt
just check
just link-check
```

Expected: all three pass. `link-check` is the one that catches a mistyped
relative path in the new ADR.

- [ ] **Step 6: Commit**

```bash
git add docs/decisions/ADR-0016-schema-projection-and-the-name-transform.md \
        docs/specification/rsdl-language-reference.md docs/ROADMAP.md
git commit -m "docs(adr): ADR-0016, the projection contract and the pinned name transform"
```

---

### Task 2: The pinned transform in `ridl-ir`

**Files:**

- Create: `crates/ridl-ir/src/name.rs`
- Modify: `crates/ridl-ir/src/lib.rs` — add the module

**Interfaces:**

- Consumes: ADR-0016 decisions 1 and 2 from Task 1.
- Produces: `ridl_ir::name::snake_case(&str) -> String`. Tasks 3, 4, 5, and 6
  all call it under exactly that path.

- [ ] **Step 1: Write the failing tests**

Create `crates/ridl-ir/src/name.rs` containing only the test module for now:

```rust
#[cfg(test)]
mod tests {
    use super::snake_case;

    #[test]
    fn an_acronym_stays_one_word() {
        assert_eq!(snake_case("getVIN"), "get_vin");
        assert_eq!(snake_case("ABC"), "abc");
    }

    #[test]
    fn an_acronym_followed_by_a_word_splits() {
        assert_eq!(snake_case("HTTPServer"), "http_server");
        assert_eq!(snake_case("IOError"), "io_error");
        assert_eq!(snake_case("parseHTTPResponse"), "parse_http_response");
    }

    #[test]
    fn a_camel_case_name_splits_on_every_boundary() {
        assert_eq!(snake_case("currentSpeed"), "current_speed");
        assert_eq!(snake_case("speed2Target"), "speed2_target");
        assert_eq!(snake_case("aB"), "a_b");
    }

    #[test]
    fn an_underscore_already_present_is_kept() {
        assert_eq!(snake_case("already_snake"), "already_snake");
        assert_eq!(snake_case("mixed_CaseName"), "mixed_case_name");
    }

    #[test]
    fn the_transform_is_idempotent() {
        for name in [
            "getVIN",
            "HTTPServer",
            "currentSpeed",
            "mixed_CaseName",
            "a1B2",
        ] {
            let once = snake_case(name);
            assert_eq!(snake_case(&once), once, "not idempotent on `{name}`");
        }
    }
}
```

Add the module to `crates/ridl-ir/src/lib.rs`, beside the existing `pub mod v2`
block:

```rust
pub mod name;
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cargo test -p ridl-ir --locked name::
```

Expected: FAIL to compile, `cannot find function 'snake_case' in this scope`.

- [ ] **Step 3: Write the transform**

Put this above the test module in `crates/ridl-ir/src/name.rs`:

```rust
//! The pinned name transform (ADR-0016 decisions 1 and 2).
//!
//! One transform serves every backend whose target namespace is snake_case.
//! It lives here rather than in a backend because it is a projection — a pure
//! function from IR identity to a target's namespace — and because `ridl-ir`
//! is the only crate `ridl-sem` and both backends already depend on.

/// snake_case of a ridl name: `currentSpeed` becomes `current_speed`.
///
/// A separator is inserted before an upper-case character that follows a
/// lower-case character or a digit, or that follows an upper-case character
/// and is itself followed by a lower-case character. So an acronym that runs
/// to the end of a name stays one word (`getVIN` gives `get_vin`), while an
/// acronym followed by a word splits (`HTTPServer` gives `http_server`). An
/// underscore already present is kept, and the mapping is stable under
/// repeated application.
///
/// **The transform is not injective, and no case-folding transform can be:**
/// lowercasing destroys what distinguishes two identifiers, so
/// `parseHTTPResponse` and `parseHttpResponse` share an output. A package
/// whose names collide under it is rejected by RIDL-149 (ADR-0016 decision 3),
/// which is where the projection contract's injectivity obligation is
/// discharged.
pub fn snake_case(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::new();
    for (index, &current) in chars.iter().enumerate() {
        if current.is_uppercase() && index > 0 {
            let previous = chars[index - 1];
            let next_lower = chars.get(index + 1).is_some_and(|c| c.is_lowercase());
            if previous.is_lowercase()
                || previous.is_numeric()
                || (previous.is_uppercase() && next_lower)
            {
                out.push('_');
            }
        }
        out.extend(current.to_lowercase());
    }
    out
}
```

- [ ] **Step 4: Run the tests to verify they pass**

```bash
cargo test -p ridl-ir --locked name::
```

Expected: PASS, five tests.

- [ ] **Step 5: Confirm the wasm gate still holds**

```bash
just wasm-check
```

Expected: PASS. The module is pure `core`/`alloc` string work and adds no
dependency, so this should be uneventful; run it because `ridl-ir` is in the
wasm-checked set.

- [ ] **Step 6: Commit**

```bash
git add crates/ridl-ir/src/name.rs crates/ridl-ir/src/lib.rs
git commit -m "feat(ridl-ir): pin one name transform for every backend"
```

---

### Task 3: Adopt the pinned transform, delete both copies

The behaviour change lands here: `interact.rs`'s callers move from the
capital-run algorithm to the acronym-aware one.

**Files:**

- Modify: `crates/ridl-backend-rust/src/interact.rs` — delete `snake_case` at
  lines 1016–1040, keep `screaming_snake`, add the import
- Modify: `crates/ridl-backend-rust/src/c_header.rs` — delete `snake_case` at
  lines 272–292, add the import

**Interfaces:**

- Consumes: `ridl_ir::name::snake_case` from Task 2.
- Produces: nothing new. `screaming_snake` in `interact.rs` keeps its signature
  and now composes the pinned transform.

- [ ] **Step 1: Delete `interact.rs`'s copy and import the pinned one**

Remove the whole `snake_case` function (its doc comment and body, lines
1016–1040). Keep `screaming_snake` exactly as it is — it calls
`snake_case(name)` and now resolves to the import. Add to the crate's import
block at the top of `interact.rs`:

```rust
use ridl_ir::name::snake_case;
```

Replace the deleted function's doc comment with a one-line comment where the
section header sits, so the reader is not left wondering where the transform
went:

```rust
// The name transform is `ridl_ir::name::snake_case` — one pinned function for
// every backend (ADR-0016 decision 2).
```

- [ ] **Step 2: Delete `c_header.rs`'s copy and import the pinned one**

Remove the whole `snake_case` function (lines 272–292). Its two callers,
`c_ident` and `c_ident_for_ref`, are unchanged. Add the same import to
`c_header.rs`. Its algorithm is the one Task 2 pinned, so this call site's
output does not change.

- [ ] **Step 3: Run the workspace tests**

```bash
just test
```

Expected: PASS with **no snapshot changes**. If `insta` reports a pending
snapshot, stop and read it: the design measured that no name in the corpus has
the acronym-followed-by-word shape, so a diff here means either that measurement
missed a name or the wrong algorithm was pinned. Do not accept the snapshot to
make the run green.

- [ ] **Step 4: Confirm no third copy survives**

```bash
grep -rn "fn snake_case" crates/
```

Expected: exactly one hit, `crates/ridl-ir/src/name.rs`.

- [ ] **Step 5: Run the gate**

```bash
just build
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/ridl-backend-rust/src/interact.rs crates/ridl-backend-rust/src/c_header.rs
git commit -m "refactor(ridl-backend-rust): use the pinned name transform, delete both copies"
```

---

### Task 4: RIDL-149 over the members of one interface

**Files:**

- Modify: `crates/ridl-core/src/diag.rs` — add `RIDL_149` after `RIDL_148`
  (around line 612)
- Modify: `crates/ridl-sem/src/check.rs` — `lower_interface` (from line 2808)
  and a new emitter beside `colliding_reserved_shape` (line 3260)
- Modify: `docs/specification/ridl-language-reference.md` — the §16.4 catalogue
  table, after the RIDL-148 row (line 1587)
- Modify: `crates/ridlc/tests/corpus.rs` — both lists, after the RIDL-148
  entries (lines 430 and 695)
- Create: `crates/ridlc/tests/corpus/ridl-diag-showcase/main/projection.ridl`

**Interfaces:**

- Consumes: `ridl_ir::name::snake_case` from Task 2; ADR-0016 decisions 3, 4,
  and 5 from Task 1.
- Produces: `DiagCode::RIDL_149`, and
  `Checker::colliding_projected_name(&mut self, name: &str, first: &str,
  projected: &str, range: TextRange, first_range: TextRange)`
  — Task 5 calls that same emitter for parameters.

- [ ] **Step 1: Write the failing showcase fixture**

Create `crates/ridlc/tests/corpus/ridl-diag-showcase/main/projection.ridl`:

```text
// RIDL-149: two names in one scope that collide after the pinned name
// transform (ADR-0016 decision 3). The names differ in source, so RIDL-402
// (the same name declared twice) does not apply — only the projection
// collides, and a target whose namespace is snake_case would carry one
// identifier twice.

package ridl.showcase

/// Two members whose projections collide. `parseHTTPResponse` and
/// `parseHttpResponse` both become `parse_http_response`.
interface Projection {
  signal parseHTTPResponse : Ratio @[100ms..1s]
  signal parseHttpResponse : Ratio @[100ms..1s]
}
```

Check the package name and the payload type against a sibling showcase file —
`crates/ridlc/tests/corpus/ridl-diag-showcase/main/services.ridl` — and use
whatever `package` line and an already-declared payload type those files use, so
the fixture resolves.

- [ ] **Step 2: Run the corpus test to verify it fails**

```bash
cargo test -p ridlc --locked corpus
```

Expected: FAIL. The showcase harness reports that the package draws no RIDL-149
while the code is registered, or — before registration — that the new file's
diagnostics do not match its snapshot.

- [ ] **Step 3: Add the catalogue entry**

In `crates/ridl-core/src/diag.rs`, after the `RIDL_148` entry:

```rust
/// Two names in one scope that collide after the pinned name
/// transform (ridl §11, §16.4; ADR-0016 decision 3). The transform is
/// not injective and no case-folding transform can be, so
/// `parseHTTPResponse` and `parseHttpResponse` both project to
/// `parse_http_response` and a target whose namespace is snake_case
/// would carry one identifier twice — in Rust, one trait with two
/// methods of the same name, or one function with two identically
/// named arguments. The projection contract's injectivity obligation
/// is discharged here, on the package, because it cannot be carried
/// by the function. Its own code rather than RIDL-402 — that rule is
/// the same name declared twice — because the remedy differs: these
/// names are distinct in source and only their projections collide.
/// Scoped to the members of one interface and the parameters of one
/// interaction (decision 4); struct fields join when E9.8 projects
/// them. Emitted per-package by the checker (E9.7).
RIDL_149 = "RIDL-149", Error,
    "two names in one scope collide after the pinned name transform";
```

- [ ] **Step 4: Add the emitter**

In `crates/ridl-sem/src/check.rs`, beside `colliding_reserved_shape`:

```rust
/// RIDL-149: two names in one scope that collide after the pinned name
/// transform (ADR-0016 decision 3). Shared by the interface-member check
/// and the parameter check — one rule over two namespaces.
fn colliding_projected_name(
    &mut self,
    name: &str,
    first: &str,
    projected: &str,
    range: TextRange,
    first_range: TextRange,
) {
    self.error_with_label(
        DiagCode::RIDL_149,
        range,
        format!(
            "`{name}` and `{first}` both become `{projected}` under the name \
             transform, so a target whose namespace is snake_case would carry \
             one identifier twice. Rename one of them (ridl §11, §16.4; \
             ADR-0016 decision 3)"
        ),
        first_range,
        format!("`{first}` becomes `{projected}` here"),
    );
}
```

- [ ] **Step 5: Add the check to `lower_interface`**

In `crates/ridl-sem/src/check.rs`, beside the `seen` map declared around line
2866, add:

```rust
// RIDL-149: two member names that collide after the pinned name
// transform (ADR-0016 decision 3). Keyed on the projection, holding
// the first member's source name and span for the label.
let mut projected: HashMap<String, (String, TextRange)> = HashMap::new();
```

Then in the member loop, between the RIDL-402 check and `seen.insert`:

```rust
if let Some(first) = seen.get(&name).copied() {
    self.duplicate_interaction(&name, range, first);
    continue;
}
// The offender still lowers and holds its ordinal, as with
// RIDL-146 and RIDL-147: the error blocks emission, and
// dropping the member would shift every later ordinal.
let projection = snake_case(&name);
if let Some((first_name, first)) = projected.get(&projection).cloned() {
    self.colliding_projected_name(&name, &first_name, &projection, range, first);
} else {
    projected.insert(projection, (name.clone(), range));
}
seen.insert(name, range);
```

Add `use ridl_ir::name::snake_case;` to the imports at the top of `check.rs` if
it is not already there.

- [ ] **Step 6: Register the code in the corpus lists**

In `crates/ridlc/tests/corpus.rs`, add `("RIDL-149", Showcase),` after the
RIDL-148 entry near line 430, and `("RIDL-149", Severity::Error),` after the
RIDL-148 entry near line 695.

- [ ] **Step 7: Add the catalogue row to the language reference**

In `docs/specification/ridl-language-reference.md` §16.4, after the RIDL-148
row:

```text
| RIDL-149 | two names in one scope that collide after the pinned name transform — the transform is not injective and no case-folding transform can be, so two names distinct in source can project to one identifier; scoped to the members of one interface and the parameters of one interaction (§11; ADR-0016 decision 3) | error    |
```

- [ ] **Step 8: Run the tests to verify they pass**

```bash
cargo test -p ridlc --locked corpus
cargo test -p ridl-sem --locked
```

Expected: PASS. Review the showcase snapshot `insta` produces and accept it only
after reading that the message names both source names and the shared
projection.

- [ ] **Step 9: Verify against the real reproduction**

```bash
cargo run -q -p ridl -- check crates/ridlc/tests/corpus/ridl-diag-showcase/main
```

Expected: a RIDL-149 error whose primary span sits on `parseHttpResponse` and
whose label points at `parseHTTPResponse`.

- [ ] **Step 10: Run the gate and commit**

```bash
just fmt
just build
git add crates/ridl-core/src/diag.rs crates/ridl-sem/src/check.rs \
        crates/ridlc/tests/corpus.rs crates/ridlc/tests/corpus/ridl-diag-showcase \
        docs/specification/ridl-language-reference.md
git commit -m "feat(ridl): reject two interface members that collide after the name transform"
```

---

### Task 5: RIDL-149 over the parameters of one interaction

**Files:**

- Modify: `crates/ridl-sem/src/check.rs` — `lower_params` (from line 4296)
- Modify: `crates/ridlc/tests/corpus/ridl-diag-showcase/main/projection.ridl` —
  add the parameter case

**Interfaces:**

- Consumes: `Checker::colliding_projected_name` from Task 4.
- Produces: nothing new.

- [ ] **Step 1: Extend the showcase fixture with the failing case**

Append to `projection.ridl`, inside the same package:

```text
/// Two parameters whose projections collide. Rust would emit one function
/// with two identically named arguments, which does not compile.
interface ProjectionParams {
  command setIt(vinNumber : Ratio, vin_number : Ratio) @[..500ms]
}
```

Check the `command` timing spelling against
`crates/ridlc/tests/corpus/ridl-diag-showcase/main/kinds.ridl` and match it, so
the fixture does not also draw RIDL-112 for a missing response bound and muddy
the showcase.

- [ ] **Step 2: Run the corpus test to verify it fails**

```bash
cargo test -p ridlc --locked corpus
```

Expected: FAIL — the new interface draws no RIDL-149, because `lower_params`
does not check yet.

- [ ] **Step 3: Add the pre-pass to `lower_params`**

`lower_params` is a `.map()` over the parameter list, so the check goes in front
of it as a pre-pass. At the top of the function body, before the
`params.params().map(...)` chain:

```rust
// RIDL-149 over one parameter list (ADR-0016 decisions 3 and 4). Two
// parameter names that collide after the transform become one binding
// in a target whose namespace is snake_case — in Rust, one function
// with two identically named arguments, which does not compile. A
// pre-pass rather than a check inside the map, because the map's
// closure returns the lowered `Param` and threading the seen-set
// through it would not read any clearer.
let mut projected: HashMap<String, (String, TextRange)> = HashMap::new();
for param in params.params() {
    let Some(name) = member_name(param.name()) else {
        continue;
    };
    let range = member_name_range(param.name(), param.syntax());
    let projection = snake_case(&name);
    if let Some((first_name, first)) = projected.get(&projection).cloned() {
        self.colliding_projected_name(&name, &first_name, &projection, range, first);
    } else {
        projected.insert(projection, (name, range));
    }
}
```

If `member_name_range` does not accept a `Param` node, use whatever range helper
the sibling `RIDL-304` warning in the same function uses to point at a
parameter, and keep the primary span on the parameter's name.

- [ ] **Step 4: Run the tests to verify they pass**

```bash
cargo test -p ridlc --locked corpus
cargo test -p ridl-sem --locked
```

Expected: PASS, with the showcase snapshot now carrying two RIDL-149 errors.

- [ ] **Step 5: Verify the original reproduction is closed**

```bash
mkdir -p /tmp/e97/pkg
printf 'package probe.names\n\ntype Speed : km/h [0.0..300.0 step 0.5]\n' > /tmp/e97/pkg/probe.typl
printf '[package]\nname = "probe.names"\nversion = "1.0.0"\n' > /tmp/e97/pkg/ridl.toml
printf 'package probe.names\n\ninterface Probe {\n  signal vinNumber : Speed @[100ms..1s]\n  signal vin_number : Speed @[100ms..1s]\n}\n' > /tmp/e97/pkg/probe.ridl
cargo run -q -p ridl -- check /tmp/e97/pkg
```

Expected: a RIDL-149 error. Before this plan, that package compiled clean and
`--emit rust` wrote `fn vin_number` twice into one trait.

- [ ] **Step 6: Run the gate and commit**

```bash
just fmt
just build
git add crates/ridl-sem/src/check.rs crates/ridlc/tests/corpus/ridl-diag-showcase
git commit -m "feat(ridl): reject two parameters that collide after the name transform"
```

---

### Task 6: The classifier-driven name-stability test

Projection contract property 3 says a compatible change moves no assigned
identity. E9.7 assigns names, not numbers, so this is the name-level case — and
it stands up the harness E9.8 extends to numbering.

**Files:**

- Modify: `crates/ridl-diff/src/tests.rs` — one new test at the end

**Interfaces:**

- Consumes: `ridl_ir::name::snake_case` from Task 2; the existing fixture
  builders in that file — `signal(name, ordinal, payload)`,
  `reserved(name, ordinal)`, `interface(name, interactions)`, `pkg(name, iface)`
  — and `diff_packages(old, new)` with `Verdict::Compatible`.
- Produces: nothing.

**Note on the deviation from the design:** the design doc calls this a property
test. It is written as a deterministic table over the compatible deltas the
classifier recognises rather than with `proptest`, because `proptest` is not a
dependency of `ridl-diff` and pulls in `rand`/`getrandom`, which does not
compile for `wasm32-unknown-unknown` — `ridl-sem` already gates it behind an
optional feature for exactly that reason
([ADR-0007](../decisions/ADR-0007-e1-execution.md) decision 5). The property is
still driven from the classifier's own verdict; only the input generation is
enumerated.

- [ ] **Step 1: Write the failing test**

Append to `crates/ridl-diff/src/tests.rs`:

```rust
// --------------------------------------------------------------------------
// Projection contract property 3, at the name level (ADR-0016).
// --------------------------------------------------------------------------

/// For any delta the classifier calls compatible, no surviving member's
/// projected name changes. Names are the only identity E9.7 pins; E9.8
/// extends this test to the numbers a projection assigns.
///
/// The deltas are the compatible ones the classifier recognises: appending an
/// interaction into a never-occupied slot, and retiring one to a tombstone.
#[test]
fn a_compatible_delta_moves_no_projected_name() {
    let base = vec![
        signal("currentSpeed", 1, "Speed"),
        signal("parseHTTPResponse", 2, "Ratio"),
    ];

    let appended = {
        let mut v = base.clone();
        v.push(signal("wheelTicks", 3, "Ratio"));
        v
    };
    let retired = vec![
        signal("currentSpeed", 1, "Speed"),
        reserved("parseHTTPResponse", 2),
    ];

    for (label, new_members) in [("append", appended), ("retire", retired)] {
        let old = pkg("veh.cluster", interface("VehicleStatus", base.clone()));
        let new = pkg("veh.cluster", interface("VehicleStatus", new_members));

        let report = diff_packages(&old, &new);
        assert_eq!(
            report.verdict,
            Verdict::Compatible,
            "{label}: fixture is not a compatible delta"
        );

        // Every member the delta kept must project to the same name it did
        // before. A tombstone carries no name to project.
        for old_member in &old.interfaces[0].interactions {
            let survivor = new.interfaces[0]
                .interactions
                .iter()
                .find(|m| m.name == old_member.name && !m.name.is_empty());
            if let Some(survivor) = survivor {
                assert_eq!(
                    ridl_ir::name::snake_case(&old_member.name),
                    ridl_ir::name::snake_case(&survivor.name),
                    "{label}: `{}` changed projection under a compatible delta",
                    old_member.name
                );
            }
        }
    }
}
```

- [ ] **Step 2: Run it to verify it compiles and passes**

```bash
cargo test -p ridl-diff --locked a_compatible_delta_moves_no_projected_name
```

Expected: PASS. This test passes on the first run by construction — the
transform is a pure function of the name and a rename is breaking, so there is
no way for a compatible delta to move a projection today. That is the point: it
pins the property so E9.8 cannot break it while adding numbering. If it
**fails**, the fixture is wrong — check that the `reserved` helper's ordinal
matches the member it retires, because a mismatched tombstone classifies
breaking and trips the verdict assertion rather than the projection one.

- [ ] **Step 3: Run the gate and commit**

```bash
just build
git add crates/ridl-diff/src/tests.rs
git commit -m "test(ridl-diff): a compatible delta moves no projected name"
```

---

### Task 7: Open the pull request

**Files:** none.

- [ ] **Step 1: Run the full pre-PR gate**

```bash
just verify
```

Expected: PASS — `lint-commits` over every commit on the branch, then `build`.

- [ ] **Step 2: Confirm `docs/wip/` state is understood**

This branch adds two files to `docs/wip/` — the design and this plan. Under the
working-memory lifecycle rule, a `main`-targeting branch should leave
`docs/wip/` gardened. Gardening happens at **E9 close-out**, not here: the four
E9 design notes already sit in `docs/wip/` and are archived as a block. Say so
in the PR description so the wip gate's finding is expected rather than a
surprise to the reviewer.

- [ ] **Step 3: Open the PR**

```bash
git push -u origin e97-projection-name-transform
gh pr create --base main --title "feat(ridl): pin one name transform and reject collisions after it" --body "$(cat <<'BODY'
Implements E9.7. ADR-0016 ratifies the fourth E9 design note and corrects
three of its statements.

Reviewing the two `snake_case` implementations against the note found that
its tie-breaker example, `getVIN`, produces `get_vin` under both, so it
cannot have decided between them; that the injectivity the note requires is
unachievable for any case-folding transform; and that the collision the note
treats as a future risk already makes the Rust backend emit a trait with two
methods of the same name.

- ADR-0016 records the projection contract, the hash position, and the four
  section-7 decisions, with the three corrections stated in place.
- The transform moves to `ridl-ir`, where `ridl-sem` and both backends reach
  it, and both existing copies are deleted.
- RIDL-149 rejects two names in one scope that collide after the transform —
  the members of one interface, and the parameters of one interaction.

No snapshot changed: no name in the corpus has the acronym-followed-by-word
shape, and no two of the 166 declared member names collide under the pinned
transform.

`docs/wip/` is not gardened by this PR. The four E9 design notes are archived
as a block at epic close-out.
BODY
)"
```

---

## Self-Review

**Spec coverage.** Design §3 decision 1 → Task 2 and Task 3. Decision 2 → Task 2
step 3, Task 3. Decision 3 → Task 4. Decision 4 → Task 4 and Task 5 (parameters
added after the design was approved; struct fields remain out). Decision 5 →
Task 4 step 5, which puts the check in `ridl-sem`. Design §3.1, the rejected
form rule → no task, correctly. Design §4, ADR-0016 → Task 1. Design §5
verification → Task 2 step 1 (unit tests), Task 4 step 1 and Task 5 step 1
(showcase), Task 6 (classifier-driven). Design §6 blast radius → Task 3 step 3,
which treats a snapshot diff as a defect.

**Two gaps the design does not name, both now covered.** Parameters (Task 5) —
approved after the design was written, and the design doc's §3 decision 4 and §7
should be amended to say "members and parameters" when this branch lands. And
the `proptest` deviation in Task 6, recorded in the task rather than left
silent.

**Type consistency.** `ridl_ir::name::snake_case(&str) -> String` is the single
name used in Tasks 3, 4, 5, and 6. `colliding_projected_name` takes the same six
parameters in Task 4 where it is defined and Task 5 where it is reused.
`RIDL_149` / `"RIDL-149"` is spelled identically in the catalogue, both corpus
lists, and the reference table.
