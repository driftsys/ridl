# ADR-0012 — The interaction boundary model: a dispatch core with domain extensions

## Status

Accepted — 2026-08-03. Scope: what the interaction layer describes, how many
languages describe it, and the mechanism by which boundary-specific vocabulary
enters. It binds the language surface until superseded, the way ADR-0009 binds
the gate and ADR-0010 binds the CLI contract.

It **supersedes the uxdl language reference in shape but not in content** — the
uxdl reference v0.1 is retired as a family member and archived, and its content
is absorbed by decisions 2 through 7 and by the rxdl reference. It **closes**
concept note §12 open question 3 (uxdl vocabulary) and general-form §9.4
(attribute-key governance, promoted from open question to precondition). It
**constrains** the not-yet-written ADR-0003, which must now record four family
members rather than five.

Not implemented. Epic E3 is re-cut against it as the boundary-model **core**,
and the domain spellings are descoped to E7 (`docs/ROADMAP.md`).

**Amendment, 2026-08-03 — where the extensions live.** Decision 7 leaves an
extension grammar-less and says nothing about file kinds. The `.rxdl` profile
now carries them: it **absorbs both readings of the wildcard**, lifting the
_layer_ restriction (any layer — its original total-profile meaning, concept
note §4) and the _domain_ restriction (the person and world spellings). One
rule: `.rxdl` is the profile with no restrictions, and a production package
tightens it in `ridl.toml` rather than relying on the file extension. The cost
is that `.rxdl` becomes a production file kind rather than only an adoption one,
which weakens "which files contain executable behaviour?" as a filesystem query;
per-package restriction is the mitigation the concept note already prescribes.

The exploration that produced it is recorded in
`docs/wip/ridl-boundary-model-review.md`; where that record and this ADR
disagree, this ADR is authoritative.

## Context

### The review that started it

A design review of the uxdl language reference stopped at §7 and became a review
of whether uxdl should exist. Three findings drove that.

**First, uxdl is mostly a pointer.** Counting inheritance references (_"ridl §N
applies"_, _"inherited verbatim"_, and the like) across the five language
references:

| Reference | Lines | Inheritance pointers | Per 100 lines |
| --------- | ----- | -------------------- | ------------- |
| typl      | 1747  | 2                    | 0.11          |
| ridl      | 1462  | 5                    | 0.34          |
| **uxdl**  | 703   | 7                    | **1.00**      |
| rmdl      | 1379  | 1                    | 0.07          |
| rsdl      | 806   | 0                    | 0.00          |

uxdl is the shortest reference carrying the most inheritance pointers — three
times the next-highest density, fourteen times rmdl's. rmdl and rsdl do not
share the pattern: rmdl adds genuinely new semantics (memory and the step) and
rsdl adds composition and deployment. The problem is specific to uxdl.

**Second, `fetch` does not belong at the user boundary.** It is absent from
uxil's eleven interaction verbs and was imported from ridl's `query`. Every
scenario tested decomposes into _interaction → displayed state_: autocomplete,
detail panes, pagination, and pull-to-refresh are each an interaction plus a
state change. A person never perceives a request; they perceive its result.

**Third, some proposed verbs name modalities rather than interactions.**
`scroll` and `drag` name gestures — one of several ways to express "show me more
of this sequence" or "put this item elsewhere". `scan` names a sensing
mechanism; a lidar scans, a staring infrared array does not, and both measure
the same quantity. The criterion this yields is normative in decision 4.

### The test that was applied, and its correct form

Every candidate language, profile, and keyword in the review was put to one
test: **what can it express that the existing languages cannot express without
distortion?**

The first form of that test — _can ridl express it?_ — is too weak, and applying
it produced two conclusions that had to be retracted within the same session.
ridl can express almost anything structurally. The correct form is:

> **Is there an obligation the contract must carry for which the existing
> languages have no vocabulary?**

That distinction separates a rename from a semantic, and it is what the rest of
this ADR rests on.

### The finding

`signal rawSpeed` and `display clusterSpeed` are not the same value. Indicated
speed is legally constrained never to read below true speed, is quantised with
hysteresis, is damped deliberately, and is unit-switched per market. Declaring
both as `signal` erases a distinction that matters, and the wrong one eventually
reaches the cluster.

Generalised:

> **Between two software peers the datum is the truth** — both sides agree on a
> number and there is nothing behind it. **At every boundary with the
> non-software world, the datum and its referent come apart.**

There are exactly three counterparties — another piece of software, a person,
the physical world — and the set is closed. Combined with direction, and with
the two software directions collapsing into one symmetric case, that yields
**five families**:

| Family           | Direction       | The system's promise            |
| ---------------- | --------------- | ------------------------------- |
| **dispatch**     | system → system | _transfer, nothing interpreted_ |
| **presentation** | system → person | _I offer this to be perceived_  |
| **intent**       | person → system | _I capture what you meant_      |
| **acquisition**  | world → system  | _I report what is_              |
| **control**      | system → world  | _I cause this to happen_        |

Two are epistemic (presentation, acquisition) and two are volitional (intent,
control), and **the agent swaps sides**: at the person boundary the person is
the agent and the system informs; at the world boundary the system is the agent
and the world informs. That single fact explains the rest — why a presentation
cannot compel, why an actuator can be commanded, why intent can be
_misunderstood_ while a measurement can only be _inaccurate_.

`dispatch` is the odd one and deliberately so: it is the only family with no
correspondence, because both ends are software.

### Why neither ridl alone nor uxdl-as-a-language was right

ridl alone cannot carry the four obligations of decision 3 — nothing in it says
how closely a value must correspond to its referent, how uncertain it is, how
late, or how you detect that it has stopped corresponding. Two are load-bearing:
the envelope is sender-stamped at **publication**, so for a transducer with a
response time every downstream model computing "with the time of the cause"
(family doctrine 9) is silently wrong by the transducer's lag; and a sensor can
report a well-formed, in-range, perfectly fresh value that is **false**, which
invalid-state provenance does not detect because it catches malformed, not
implausible.

uxdl-as-a-language is the wrong remedy because the obligations are the **same
four at every boundary**. A separate language per boundary would reinvent them
per boundary, and rmdl and rsdl could not then reason across them uniformly. A
proposal to add a third language for sensors and actuators was raised during the
review and retracted for exactly this reason (decision 10).

## Decision

1. **uxdl ceases to be a family member.** The family is four languages — typl,
   ridl, rmdl, rsdl — and ridl gains the boundary model. uxdl's content survives
   in decisions 2 through 7; its shape, a parallel language renaming ridl's
   kinds, does not. The reference is archived verbatim at
   `docs/archive/uxdl-language-reference-v0.1.md` rather than edited.

2. **Five families, as tabulated in Context.** The family is a property of the
   declaration, not of the file, the package, or the deployment. `dispatch` is
   the core: the case with no correspondence and therefore no obligations.

3. **Four correspondence obligations, owned by core.** Wherever datum and
   referent come apart, the contract carries:

   | Obligation                    | Presentation                 | Acquisition                          | Control                     |
   | ----------------------------- | ---------------------------- | ------------------------------------ | --------------------------- |
   | **relationship**              | never under-reads; quantised | transfer function; calibration       | authority limits; slew rate |
   | **uncertainty**               | display resolution           | tolerance per reading                | positioning tolerance       |
   | **latency of correspondence** | perception delay             | sample instant ≠ publication instant | actuation lag               |
   | **failure to correspond**     | shown but not perceivable    | plausible but false                  | commanded but not achieved  |

   Two properties of these obligations are normative. They **compose along a
   path**: actual wheel speed → measurement → derivation → indication →
   perception is four hops, and an end-to-end budget is computable only if each
   hop declares its own. And an obligation frequently **relates two
   declarations** rather than qualifying one — commanded versus achieved angle,
   raw versus indicated speed, a toggle versus the state it flips. uxdl §16.2
   parked this as "two-way binding sugar"; it is not sugar, it is where the
   obligation lives, and one mechanism serves every boundary.

4. **Keyword spellings per family, admitted by two tests.** The surface is a
   keyword per (kind, family) combination, not a generic kind plus a boundary
   clause, because the keyword places the discriminating classifier in R1
   position: `grep '^measure'` enumerates every sensor input, and a reader
   learns what a line is from its first word.

   | Family           | continuous | occurrence | operation (no return)                           | operation (returns) |
   | ---------------- | ---------- | ---------- | ----------------------------------------------- | ------------------- |
   | **dispatch**     | `signal`   | `event`    | `command`                                       | `query`             |
   | **presentation** | `present`  | `notify`   | —                                               | —                   |
   | **intent**       | —          | _open_     | `activate` `toggle` `select` `adjust` `dismiss` | —                   |
   | **acquisition**  | `measure`  | `detect`   | —                                               | _open_              |
   | **control**      | `actuate`  | —          | `trigger`                                       | —                   |

   `fixed` is family-neutral: a provisioned constant carries no correspondence
   at any boundary.

   Two admission tests bind every future addition:

   - **Modality independence.** If two realisations afford the same capability
     they must produce the same declaration. A word naming _how_ — `scroll`,
     `drag`, `scan`, `display` — is rejected. `display` is rejected on this
     ground specifically: a speaker does not display, and the family's own
     non-visual surfaces question (uxdl §16.5) was unanswerable only because the
     vocabulary was visual.
   - **Constraint bundling.** A keyword is warranted if and only if it bundles
     constraints that would otherwise be re-declared by hand at every use. A
     spelling adding nothing over its core kind is rejected.

   The empty cells are consequences, not gaps: **presentation has no
   operations** because nothing can be invoked on an agent; **intent has no
   continuous form** because a person's continuous state is knowable only by
   measuring a physical proxy, which is acquisition (a pedal is `measure`);
   **intent has no query** because a system cannot block on a human — "are you
   sure?" is `present` plus `activate`/`dismiss`, two interactions;
   **acquisition has no command** because commanding a device is system → world,
   hence control; **control has no occurrence** because nothing leaves the
   system uninvited; and **control has no query** because effect is observed by
   measuring back, never returned.

5. **The intent operation set is closed, with no generic.** A person performs
   one act at a time; a multi-parameter operation is a function call, not a
   gesture. Sending a message is `supply` plus `select` plus `activate`;
   drag-and-drop, stripped of modality, is `select` plus `adjust`. uxil shipped
   a compiler-enforced closed verb set against a real automotive corpus, and
   uxdl §14's own guidance (_"prefer refinements over generic `action` — they
   carry scaffolding and test semantics for free"_) is advisory only while a
   generic exists. Removing the generic makes it structural. The world boundary
   keeps a parameterised operation (`trigger`) because nothing there is an agent
   constrained to single gestures.

6. **Family and operation shape are core IR fields; obligations are language
   attributes.** Two closed enums are added to the interaction node — `family`
   and `shape` — added once by core, never by an extension. Obligations are
   authored, heterogeneous, and mix the assignment and predicate forms, so they
   are ordinary attributes under the general-form §4.2 grammar.

   **Nothing is lowered into an attribute.** The mapping from spelling to (kind,
   family, shape) is **bijective**, which gives three properties: `ridl fmt` and
   IR rendering round-trip to the authored spelling; there is no second way to
   write what a keyword says, so "strict beats flexible" holds; and there is no
   desugaring that could drift and produce a spurious diff against a stored
   baseline. `(command, intent, no shape)` is simply an invalid combination —
   decision 5 expressed structurally rather than as a lint.

7. **An extension is a spelling table plus backends. It has no grammar, no IR
   nodes, and no semantics.** All semantics — the kinds, the families, the
   obligations, availability, the operation-shape taxonomy and its payload rules
   — are core. An extension supplies readable spellings for combinations core
   already understands, code generation for its boundary, and optionally a
   package-level restriction on which spellings are permitted (ADR-0002's
   existing profile-purity mechanism, not a new one).

   Two extensions are anticipated: **hmi** (presentation + intent) and **env**
   (acquisition + control). An extension name is a packaging concept and does
   not appear in the IR; codegen selects on `family`.

   Consequences: an extension cannot break `ridl-diff`, because everything
   lowers to a core kind and the walk, name-pairing, and ordinal comparison are
   core's. It cannot break a backend, because an unaware backend sees the core
   kind and emits working code while an aware one reads `family` and emits
   better code. It cannot fragment the ecosystem, because no extension defines
   `present` — core does.

8. **The attribute registry is a precondition, with owner and diff category.**
   General-form §9.4 asked for _name → form → allow-list → consumer_. Two fields
   are added and the whole is promoted from open question to a requirement that
   precedes any extension:

   | Field             | Values                                           |
   | ----------------- | ------------------------------------------------ |
   | name              | namespaced outside core                          |
   | **owner**         | `core` · `extension:<name>` · `user:<namespace>` |
   | form              | flag · assignment · predicate                    |
   | allow-list        | which declaration kinds accept it                |
   | consumer          | required by the general-form §4.1 deletion test  |
   | **diff category** | how a change to it is classified                 |

   Using attributes as the extension mechanism moves the fragmentation risk from
   the keyword namespace, where a registry protects it, into the attribute
   namespace, where none exists. User and extension keys are therefore
   namespaced, so a later core addition cannot collide with a program's private
   key.

9. **Unregistered attribute keys fail closed, in the compiler and in the gate.**
   An unknown key is a compile error, never a pass-through — pass-through turns
   a typo into silent semantics. A key with no diff category is classified
   **breaking**, never compatible. This is not hypothetical: `ridl-diff`'s
   `DocOnly` category covers "doc comment, labels, or deprecation metadata" and
   is classified `Compatible`, there is no general attribute-changed category,
   and tightening a tolerance bound is breaking. The four obligations therefore
   need diff categories of their own before any of them can be authored.

10. **The environment boundary needs no language of its own.** Proposed during
    the review and retracted. Every candidate decomposes: no-acknowledgment-of-
    effect is already ridl `command`; stuck, drift, and disconnection are ridl
    §4.5 invalid-state provenance; plausibility is typl ranges plus `require`;
    tolerance is a typl type property; redundancy and voting are rsdl wiring;
    calibration parameters are `fixed`; transfer functions are rmdl; sampling
    and aliasing are ridl §9 timing. What survives is the obligation set of
    decision 3, which is shared with the person boundary and therefore core.

11. **`fetch` is removed and does not reappear.** Three independent grounds, in
    Context and in decision 4's gap analysis. On-demand data at a user boundary
    is an interaction followed by a state change.

12. **Evolution rules are uniform across boundaries.** `ridl-diff` pairs
    interactions **by name** and then compares ordinals within name-matched
    pairs (`crates/ridl-diff/src/walk.rs`). A rename therefore surfaces as
    `InteractionRemoved` — breaking — plus an append, and a reorder is
    `InteractionReordered`, breaking always. That is already the intersection of
    the two identity disciplines the two boundaries need: the bus carries
    ordinals, while test selectors, telemetry, and generated bindings carry
    names. No boundary-specific evolution rule is warranted, and the question is
    closed unless a real programme produces a counter-case.

## Alternatives considered

| Candidate                                                                                 | Verdict  | Reason                                                                                                                                                                                                                                                                                                                |
| ----------------------------------------------------------------------------------------- | -------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Keep uxdl as a language                                                                   | rejected | mostly renames plus a document that is largely pointers; the obligations it would need are shared with two other boundaries and belong in core                                                                                                                                                                        |
| Keep uxdl as a thin profile                                                               | rejected | the profile's whole content is spellings, which decision 7 delivers without a second grammar, a second reference, or a second compiler path                                                                                                                                                                           |
| Generic kinds plus a boundary clause (`signal … to person`)                               | rejected | semantically identical to decision 4 and reversible, but four consecutive lines open with the same word and the classifier is no longer in R1 position                                                                                                                                                                |
| A third language for sensors and actuators                                                | rejected | decision 10 — everything decomposes into typl, ridl, and rsdl                                                                                                                                                                                                                                                         |
| A new `sidl` covering all boundaries                                                      | rejected | `ridl` already means _Reactive **Interface** Description Language_ and is boundary-neutral as named; SIDL collides with the Babel/LLNL Scientific IDL; renaming unwinds `ridlc`, `ridl.toml`, `ridl.lock`, the cache, and the crates.io reservations                                                                  |
| rsdl as the home for boundary declarations                                                | rejected | rsdl is the lattice apex and composes by import; an interface must be declarable independently of any deployment or it cannot be reused across topologies                                                                                                                                                             |
| Operation shapes as a `gesture` attribute                                                 | rejected | moves the discriminating classifier out of keyword position, violating general-form R1                                                                                                                                                                                                                                |
| Lowering the family into an attribute                                                     | rejected | authorable as a second spelling, and a desugaring that can drift produces spurious diffs against stored baselines; decision 6 makes it a field instead                                                                                                                                                                |
| `boundary` / `domain` as the discriminator name                                           | moot     | decision 6 makes it an IR field, so no attribute key is minted. `domain` would in any case have collided with the value-domain sense used 42 times across the references, including expr-core's normative typing rules                                                                                                |
| `display`, `demand`, `enter`, `apply`, `observe`, `control`, `sample`, `scan` as keywords | rejected | `display` names a modality; `observe` collides with control-theory observers, uxil's verb, and the family's online observers; `control` collides with the HMI sense of a widget; `apply` collides with rsdl's application notation; `sample` and `scan` name mechanisms; `demand` and `enter` were rejected in review |

## Consequences

- **Positive — the family loses a member and gains a model.** Four languages,
  four questions: what data, what boundaries, what computation, what topology.
  The concept note's own §1.2 conceded that user interaction is _"structurally
  the same problem as service contracts, aimed at a different audience"_, and
  justified the split on audience rather than structure. Decision 7 serves the
  audience without the split.
- **Positive — three long-standing questions close.** Non-visual surfaces (uxdl
  §16.5) are expressible because the vocabulary is no longer visual: a spoken
  prompt is `present`, an earcon is `notify`, an utterance is the intent
  occurrence, and uxil's `agent` surface kind becomes unnecessary rather than
  deferred. Two-way binding (uxdl §16.2) is the paired obligation of decision 3.
  The uxil payload bridge (markspec #729, parked with zero code for want of a
  consumer) has its revisit trigger met.
- **Positive — core gains what it needed anyway.** The boundary discriminator
  and the four obligations are useful to ridl with or without a spelling table,
  and the sample-instant obligation fixes a silent error in any model computing
  with the time of the cause.
- **Negative — `signal` and `event` narrow.** They become dispatch-family words
  rather than boundary-agnostic ones, so every existing declaration that
  actually faces a person or a transducer is reclassified. Cheap now: the
  workspace is `0.0.0`, there are no release tags, and the binaries are
  `publish = false`. Not cheap later.
- **Negative — three things must exist before any extension ships**, and none
  was a requirement before this ADR: the attribute registry (decision 8), diff
  categories for the four obligations, and fail-closed classification (decision
  9). All are core work, independent of any extension.
- **Negative — the keyword count rises.** Seven new kind spellings plus five
  operation shapes, against ADR-0002's small-surface philosophy. Accepted on the
  ground that the boundary set is **closed** at three counterparties, so the
  cost is paid once and does not grow, and that no single reader faces all of it
  — an HMI engineer reads four words, a sensor engineer four, a service engineer
  five.
- **Negative — this reopens a question ADR-0003 has not yet answered.** The
  five-language decision was never ratified, which is what makes the change
  cheap, but ADR-0003 must now be written against four.
- **Neutral — the surface choice is reversible.** Decision 4 and the rejected
  boundary-clause alternative lower to the same IR, so a later reversal is a
  surface migration, not a semantic one.

## Open

Recorded rather than decided; none blocks the core work.

1. **The intent occurrence keyword** — the person-supplies-content spelling
   (typing, scanning, dictation). `supply` is the working candidate; `provide`
   and `tell` remain live. `enter` and `submit` were rejected in review — the
   first for modality, the second for implying a commit and an acknowledgment
   that an occurrence does not have.
2. **The acquisition/query cell** — a polled or diagnostic sensor read. Possibly
   `measure` under different timing, possibly `query` crossing a non-software
   boundary unchanged, possibly its own word.
3. **Interaction citation paths** — the projection from a declaration to the
   stable identifier that specifications, journeys, tests, and telemetry cite.
   This is uxil's founding problem, and it is not user-boundary-specific: the
   observability semantic conventions need it for every interaction. Deferred to
   the citation-grammar design, not to an extension.
4. **Availability has five sources and `during` covers one** — mode is covered;
   data, in-progress, policy, and provisioning are not. At a
   presentation-obligated boundary an availability condition must be **evaluable
   by the consumer**, because a person must perceive unavailability before
   attempting rather than discovering it by rejection — which means such a
   predicate may reference only declared, consumer-visible state. Absent versus
   disabled is a further open distinction.
5. **Journeys** — a sequence of interactions across surfaces. Neither ridl nor
   rsdl expresses a graph over interfaces. Likely a
   requirements-and-traceability artifact rather than a language, but
   unconfirmed.

## References

- `docs/wip/ridl-boundary-model-review.md` — the review this decision comes from
- `docs/specification/rxdl-language-reference.md` — the spelling layer this
  decision leaves over
- `docs/archive/uxdl-language-reference-v0.1.md` — retired by decision 1
- `docs/wip/ridl-family-concept.md` §1.2, §2, §3, §12.3 — the five-language
  proposal and the uxdl-vocabulary question this closes
- `docs/wip/family-general-form.md` §4.1–§4.5, §4.8, §9.4 — the attribute model,
  the deletion test, the promotion path, and the registry question decision 8
  promotes
- `docs/specification/ridl-language-reference.md` §3.1, §4.4–4.5, §9, §11 — the
  envelope, init and invalid channels, timing, and evolution the obligations
  extend
- `crates/ridl-diff/src/walk.rs`, `crates/ridl-diff/src/lib.rs` — the
  name-pairing and category vocabulary decisions 9 and 12 rest on
- markspec ADR-034 — uxil, its eleven verbs, its citation grammar, and the
  parked payload bridge
- ADR-0002 — package-level profile purity, reused by decision 7
- ADR-0011 — `fixed`, family-neutral under decision 4
