# Epic 2 — ridl, the Interface Layer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** ridl ships as the family's interface layer — `.ridl` packages with
interfaces, the five interaction kinds, timing, inline `T | E` fallible returns,
`require`/`ensure` contracts, streams, and services compile to IR v2 with full
coded diagnostics; a second backend (TypeScript) proves IR neutrality;
`ridl diff` classifies contract changes as breaking or compatible with the 0/1/2
exit contract; the desk gains baseline-aware ordinal checking, ridl-aware LSP +
lint, and range-derived property tests under `ridl test` (docs/ROADMAP.md, epic
E2).

**Architecture:** the E1 typl toolchain grows the family grammar and the
interaction semantics on top of its existing seams. `ridl-syntax` gains the ridl
keyword activations, a `Profile` parameter on lexing and parsing (`.typl` vs
`.ridl`), the interaction grammar, the single general-form attr_block
production, and the guaranteed expr subset grammar. `ridl-sem` gains the
interaction resolver/checker, timing resolution, fallible-return semantics, expr
type checking, observer-stub lowering, the ridl lint pass, and a subset expr
evaluator. `ridl-ir` bumps to v2 (`proto/ridl/ir/v2/ir.proto`) using the field
numbers v1 earmarked; v1 is retired when the v2 lowering lands, mirroring E1's
v0→v1 flip. A new `backends/typescript` crate is the second backend. A new
`tools/diff` crate holds the IR-snapshot compare engine and classifier, surfaced
as `ridl diff` in the facade — never in `ridlc` (the ISO 26262
tool-qualification boundary). The facade also gains `ridl baseline` and
`ridl test`; `ridl-lsp` and the VS Code extension learn `.ridl`.

**Tech Stack:** unchanged from E1 — Rust edition 2024 · logos · rowan ·
ungrammar · salsa (pinned `=0.28.0`) · prost + protox · num-bigint +
num-rational · codespan-reporting · toml + serde · lsp-server + lsp-types ·
quote + prettyplease · minijinja · clap · insta · proptest. The TypeScript
backend emits source through a plain Rust string emitter (no new dependencies).
All fixed by ADR-0004; E2-specific choices recorded in ADR-0008.

## Global Constraints

- Specification authority: `docs/specification/ridl-language-reference.md` (ridl
  v0.2.0) is normative for everything E2 implements, **except** the four
  general-form §6 supersessions, where `docs/wip/family-general-form.md` wins
  because the roadmap E2 rows cite it (ADR-0008 decision 1): inline `T | E`
  returns (gf §6.1), generic timing min = rate floor / max = staleness bound (gf
  §6.2), ordinal tooling (gf §6.3), and the Stratum-3 wording "infrastructure
  failure — detected, undeclared" (gf §6.4). The ridl reference text absorbs
  these at close-out (doc sync), not mid-epic.
- The fourteen ADR-0008 decisions are fixed and not renegotiable inside a task:
  1 gf §6 authority; 2 signal init is bare `= value` before timing and signals
  carry **no** attr_block; 3 `persist` deferred; 4 inline `T | E` transport
  identity = interface + ordinal + arm types; 5 the `final` spelling is frozen
  for E2; 6 service codes stay RIDL-140/141 as documented; 7 the second backend
  is TypeScript; 8 IR v2 lives at `proto/ridl/ir/v2/ir.proto` and uses the v1
  earmarks exactly (Package 3–15, Decl 7–9 + kind 16–29, FieldType kind 16–19),
  v1 retained until the v2 lowering lands; 9 `ridl diff` lives in the
  facade/tools with exit codes 0 compatible / 1 breaking / 2 error; 10 the
  expr-core spec document lands before or with the E2.4 subset; 11 the local
  gate is the merge gate (CI is still stuck); 12 IR timing carries resolved
  concrete bounds, a mode discriminator, and a default-applied flag; 13 the new
  diagnostic-code allocations (FORM-106/107/108, RIDL-308, RIDL-407, MANI-009);
  14 the `ridl diff` directional classifier rules, the `.ridl/baseline/`
  storage, and contracts carried in IR as canonical source text.
- Module semantics are ADR-0002; stack choices are ADR-0004; the E1 execution
  decisions (ADR-0007) stay in force where E2 does not supersede them.
- Diagnostics are accumulated `Diagnostic` values with stable codes, never
  renumbered or reused. E2 implements the `RIDL-` catalogue (ridl §16) and
  allocates these new codes (ADR-0008 decision 13): FORM-106 unknown attribute
  key, FORM-107 attribute key not allowed on this declaration kind, FORM-108
  duplicate attribute key in one block (gf §4.3); RIDL-308 named result union in
  return position (inline `T | E` is canonical, gf §6.1); RIDL-407 interaction
  ordinal changed against the baseline (gf §6.3); MANI-009 invalid
  `[defaults].timing` value (MANI-008 is already live on `main` for a workspace
  member directory without a manifest — never reuse it).
- Exactness carries over (ADR-0007 decision 9): durations are exact-decimal
  **microsecond** strings in the IR; no float or double field anywhere in IR v2;
  type references are canonical (`pkg.Name` cross-package, bare `Name`
  same-package), never aliases.
- The lexer/parser losslessness invariant holds for both profiles:
  `parse(text, profile).syntax().text() == text` for every input, valid or
  broken. The parser accepts more than the reference allows; the checker narrows
  (the E1 discipline).
- Commit messages are Conventional Commits linted by git-std against
  `.git-std.toml`; use the type and scope named in each task. No new scopes are
  needed: `backends` covers `backends/typescript`, `tools` covers `tools/diff`,
  and the root `Cargo.toml` member globs pick both up.
- Every task ends with the local gate green: `cargo test --workspace`,
  `cargo fmt --all --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `just check` for
  tasks that touch Markdown, `just wasm-check`
  (`cargo check --target wasm32-unknown-unknown --no-default-features` over the
  compiler crates, the E1.19 guarantee) for tasks that touch `ridl-syntax`,
  `ridl-core`, `ridl-sem`, or `ridl-ir`, and `just verify` before opening the
  PR. The local gate is the merge gate (ADR-0008 decision 11).
- Prose in comments and docs follows plain, literal English.
- One PR per task, squash-merged after a recorded review; never push to `main`
  directly; sync local `main` after every merge.

## Debt folding

E2 absorbs the family-grammar debt E1 recorded as out of scope: the deferred
profile codes TYPL-304 (task 2), TYPL-301 (task 3), and TYPL-303 (task 4) ship
as the constructs they reject become parseable (ADR-0007 decision 10); the
single general-form §4 `attr_block` production replaces the per-language
attribute sketches (task 4); `ridl diff` and lint leave the ADR-0004 §10 "later
rings" list (tasks 16–19). The gf §4.7 promotion of `labels`/`deprecated` to
attributes is **not** absorbed — it stays deferred (see Out of scope) because it
is not one of the roadmap-cited supersessions.

## Dependency waves

```text
wave 0   PR 0 (plan + ADR-0008, this document)
wave 1   T1 ir-v2 ∥ T2 profile-lexer → T3 interact-parser → T4 attr+expr
         grammar → T5 interact resolve+check → T6 lower-to-v2 (needs T1)
         T7 expr-core spec (doc-only, parallel, before or with T11)
wave 2   T8 services (after T6) ; T9 timing ∥ T10 fallible (after T6)
         → T11 expr-check (after T4, T7) → T12 observers (after T11)
wave 3   T13 ts-types (after T6) → T14 ts-interact (after T8, T9, T10,
         T12, T13) ∥ T15 rust-interact (after T6, T8, T9, T10, T12)
         T16 diff-engine (after T6) → T17 diff-classifier (after T8, T9,
         T10, T16) → T18 baseline (after T17)
wave 4   T19 lint (after T10) ∥ T20 lsp (after T9) ∥ T21 ridl-test
         (after T11, T12) → T22 corpus (last)
close    whole-epic review → fix wave → gardening + docs sync (the ridl
         reference absorbs the four gf §6 supersessions per ADR-0008 d1)
```

Model tiers: fable = T1, T3, T5, T6, T8, T17, epic review; sonnet = T22; opus =
everything else.

---

### Task 1: E2.7 — IR v2 schema (interaction layer)

Branch `e2-7-ir-v2` · commits `feat(ridl-ir): …` · model: fable.

**Read first:** `crates/ridl-ir/proto/ridl/ir/v1/ir.proto` (the earmark
comments); ridl reference §3–§14 (semantics per message); gf §6.1 (fallible
returns), §6.2 (generic timing); ADR-0008 decisions 4, 8, 12.

**Files:**

- Create: `crates/ridl-ir/proto/ridl/ir/v2/ir.proto`
- Modify: `crates/ridl-ir/build.rs` (compile both packages),
  `crates/ridl-ir/src/lib.rs` (module `v2` re-export beside `v1`; `v1` stays
  until task 6 flips the pipeline)
- Test: round-trip + JSON tests in `ridl-ir`

**Interfaces:**

- Produces (consumed by tasks 6, 8–17, 20): proto package `ridl.ir.v2`, Rust
  path `ridl_ir::v2::*`. The v2 file contains every v1 message **verbatim with
  unchanged field numbers** (the typl surface is untouched) plus the additions
  below, which allocate the earmarked numbers exactly (ADR-0008 decision 8). New
  messages number data fields from 1 and oneof members from 10, the v1 scheme.

```proto
message Package {
  // ... v1 fields 1–2 unchanged ...
  // E2 (earmarked 3–15): interfaces and services, source order.
  repeated Interface interfaces = 3;
  repeated Service services = 4;
  // 5–15 remain open for later profiles.
}

message Decl {
  // ... v1 fields 1–6 unchanged ...
  // E2 (earmarked 7–9): the interaction ordinal — 1-based declaration
  // order across all interactions of the enclosing interface, one
  // sequence regardless of kind, reserved tombstones counted (ridl §11).
  // 0 for package-level declarations. 8–9 remain open.
  uint32 ordinal = 7;
  oneof kind {
    // ... v1 members 10–15 unchanged ...
    // E2 (earmarked 16–29): the five interaction kinds plus the
    // interface-body reserved tombstone. An interaction reuses the Decl
    // envelope (name, doc, labels, deprecated); visibility and is_error
    // stay unset on interactions.
    SignalDef signal_def = 16;
    EventDef event_def = 17;
    CommandDef command_def = 18;
    QueryDef query_def = 19;
    FinalDef final_def = 20;
    Reserved reserved_slot = 21;
  }
}

// The abstract contract shape (ridl §14.0) — identity-less, flat.
message Interface {
  string name = 1;                  // CamelCase; "" for a service's
                                    // inline shape (ridl §14.5)
  Visibility visibility = 2;
  string doc = 3;
  repeated string labels = 4;
  optional string deprecated = 5;
  // Interactions and reserved tombstones, declaration order; every
  // member's Decl.kind is one of members 16–21 above.
  repeated Decl interactions = 6;
}

// A global published declaration of an interface (ridl §14.5).
message Service {
  string name = 1;                  // dotted global name "veh.adas.cruise"
  Visibility visibility = 2;
  string doc = 3;
  repeated string labels = 4;
  optional string deprecated = 5;
  oneof shape {
    string interface_ref = 10;      // canonical interface reference
    Interface inline = 11;          // inline shape; Interface.name == ""
  }
}

// Resolved timing (ridl §9, gf §6.2, ADR-0008 decision 12). min = rate
// floor, max = staleness bound; per-kind behavior is derived from the
// declaring keyword, not stored. Bounds are exact-decimal MICROSECOND
// strings. An untimed signal/event gets the configured default resolved
// at compile time with default_applied = true — "untimed" does not exist
// beyond the parser (ridl §9.1). Strict periodic stores the period in
// both bounds. An explicit half-open range leaves the absent side unset.
enum TimingMode {
  TIMING_MODE_UNSPECIFIED = 0;
  TIMING_MODE_STRICT_PERIODIC = 1;
  TIMING_MODE_RANGE = 2;
}
message Timing {
  TimingMode mode = 1;
  optional string min_us = 2;
  optional string max_us = 3;
  bool default_applied = 4;
}

message SignalDef {
  string payload = 1;               // canonical named-type reference
  // The bare `= value` channel-init override (ridl §4.4), canonical text.
  optional string declared_init = 2;
  InitValue init = 3;               // resolved channel init (RIDL-109)
  Timing timing = 4;
}
message EventDef {
  string payload = 1;
  Timing timing = 2;
}
message CommandDef {                // returns () always — no return field
  repeated Param params = 1;
  repeated Contract contracts = 2;  // require only (RIDL-302)
}
message QueryDef {
  repeated Param params = 1;
  ReturnType return_type = 2;
  repeated Contract contracts = 3;
}
message FinalDef {
  FieldType payload = 1;            // named type or array (ridl §8)
}
message Param {
  string name = 1;                  // camelCase
  FieldType type = 2;               // stream via FieldType.kind.stream
}

// The ridl-owned stream container (ridl §12), earmarked FieldType member.
message StreamType {
  oneof element {
    string named = 10;              // canonical named-type reference
    PrimitiveType primitive = 11;   // STRING or BYTES only (RIDL-202)
  }
}
message FieldType {
  // ... v1 unchanged ...
  oneof kind {
    // ... v1 members 10–15 unchanged ...
    StreamType stream = 16;         // E2 (earmarked 16–19)
  }
}

message ReturnType {
  oneof kind {
    FieldType value = 10;           // named / tuple / stream
    FallibleType fallible = 11;     // inline T | E (gf §6.1)
  }
}
// Transport identity (ADR-0008 decision 4): derived from the enclosing
// interface name + the interaction ordinal + both arm references —
// stable under compatible evolution; see fallible_transport_identity.
message FallibleType {
  string ok = 1;                    // canonical non-error named type
  string err = 2;                   // canonical error-type reference
}

enum ContractKind {
  CONTRACT_KIND_UNSPECIFIED = 0;
  CONTRACT_KIND_REQUIRE = 1;
  CONTRACT_KIND_ENSURE = 2;
}
// One require/ensure clause, lowered as an observer stub (E2.5; full
// expr checking is E5). source is the canonical expr text — E5.1
// replaces it with a structured expr tree.
message Contract {
  ContractKind kind = 1;
  string source = 2;
  repeated string signal_refs = 3;  // interface signals the expr reads
  repeated string param_refs = 4;
  bool uses_result = 5;
  string observer_id = 6;           // "<Interface>.<interaction>.<kind>[n]"
}
```

- Rust helpers in `ridl_ir::v2`:
  `pub fn fallible_transport_identity(interface: &str, ordinal: u32,
  fallible: &FallibleType) -> String`
  returning `"{interface}#{ordinal}:{ok}|{err}"` (e.g.
  `VehicleStatus#9:FaultPage|DiagError`) — the ADR-0008 decision 4 rule, the
  single derivation both the TypeScript backend and the diff classifier call;
  `pub fn to_json_pretty(pkg: &Package) -> String` (serde derives on all v2
  types, as v1).

**Steps:**

- [ ] Write the proto with doc comments citing the ridl reference section for
      every new message (the v1 comment discipline).
- [ ] Failing tests: construct a representative `Package` in Rust — one
      interface holding all five kinds plus a reserved tombstone (ordinals 1–6),
      a strict-periodic and a defaulted range timing, a fallible query, a stream
      query, two services (named-ref and inline shape); prost round-trip; JSON
      round-trip; JSON shows `"min_us": "10000"` as a string and the fallible
      arms; `fallible_transport_identity` matches the format above.
- [ ] Full local gate; commit.

**Done when:** IR v2 compiles beside v1, round-trips, and its JSON dump is exact
and readable; every earmarked number is allocated exactly as the v1 comments
promised.

---

### Task 2: E2.1a — ridl keywords, expr tokens, profile plumbing

Branch `e2-1a-profile-lexer` · commits `feat(ridl-syntax): …` /
`feat(ridl-core): …` · model: opus.

**Read first:** ridl reference §2 (lexical additions, keywords); typl reference
§1.4 (registry), §16.4 (TYPL-304); the walking-skeleton technote (pipeline
seams); `crates/ridl-syntax/src/keywords.rs` as-built.

**Files:**

- Modify: `crates/ridl-syntax/src/keywords.rs`,
  `crates/ridl-syntax/src/syntax_kind.rs`, `crates/ridl-syntax/src/lexer.rs`,
  `crates/ridl-syntax/src/lib.rs`, `crates/ridl-syntax/src/parser.rs`
  (signature + TYPL-304), `crates/ridl-core/src/db.rs` (profile-aware
  `parse_file`), `crates/ridl-core/src/workspace.rs` (`.ridl` discovery),
  `tools/fmt/src/lib.rs` + `crates/ridlc/src/lib.rs` +
  `crates/ridl-lsp/src/server.rs` (call-site signature updates)
- Test: lexer unit tests + `test_data/lexer/*.ridl` corpus; workspace tests

**Interfaces:**

- Produces (consumed by every later task):

```rust
// ridl-syntax
pub enum Profile { Typl, Ridl }               // Copy + Eq + Hash
pub fn lex(input: &str, profile: Profile) -> Vec<Token>
pub fn parse(input: &str, profile: Profile) -> Parse   // signature change
// keywords.rs — FAMILY_RESERVED is unchanged (it already holds every word)
pub fn keyword_in(profile: Profile, text: &str) -> Option<SyntaxKind>
// ridl-core
pub fn profile_of_path(path: &str) -> Profile  // ".ridl" => Ridl, else Typl
```

- New `SyntaxKind` token variants: `InterfaceKw`, `ServiceKw`, `SignalKw`,
  `EventKw`, `CommandKw`, `QueryKw`, `FinalKw`, `RequireKw`, `EnsureKw` (the
  ridl profile's words, ridl §2.3), and the expr operator tokens task 4 needs:
  `Lt`, `Le`, `Gt`, `Ge`, `EqEq`, `Neq`, `AmpAmp`, `PipePipe`, `Bang`, `Plus`,
  `Star` (`Minus`, `Slash`, `Percent`, `Pipe`, `Eq`, `Dot` exist from E1).
- Profile rule in the lexer: under `Profile::Ridl` the nine ridl words lex to
  their keyword variants and durations/`@` are ordinary tokens (no TYPL-302);
  under `Profile::Typl` behavior is byte-identical to E1 (ridl words stay
  `ReservedWord`, durations/`@` still draw TYPL-302 in the parser). Words of the
  other profiles (uxdl/rmdl/rsdl) stay `ReservedWord` in both.
- Parser profile boundary, both directions: in a `.typl` parse, an interaction
  keyword (`ReservedWord` whose text is one of the nine) at declaration-start
  position emits **TYPL-304** (`interaction declaration in
  typl context`)
  instead of the generic FORM-105 — the first ADR-0007 decision-10 debt item
  lands. In a `.ridl` parse, a `ReservedWord` at declaration-start position
  emits **RIDL-403**
  (`behaviour, user-interaction, or architecture declaration in ridl
  context`).
  Both recover exactly as FORM-105 does today (ErrorNode, resync at the next
  top-level keyword).
- `parse_file` derives the profile from `InputFile.path` via `profile_of_path`;
  `load_workspace` accepts `.ridl` files everywhere it accepts `.typl` (same
  package↔directory law, TYPL-001/002 unchanged; a package may mix `.typl` and
  `.ridl` files). Single-file mode accepts a bare `.ridl` entry.
- `tools/fmt` and all other callers pass the profile through; `ridl fmt` formats
  `.ridl` files with unchanged rules (interaction-specific layout is refined in
  task 3's corpus, not here).

**Steps:**

- [ ] Failing tests: (a) `signal` lexes `SignalKw` under Ridl and `ReservedWord`
      under Typl; (b) `10ms` and `@` lex clean under Ridl, still TYPL-302 under
      Typl; (c) `a >= b && !c` lexes `Ident Ge Ident AmpAmp Bang Ident`; (d)
      `T | E` lexes with `Pipe` (unchanged); (e) `interface X {}` in a `.typl`
      parse → TYPL-304 with recovery; (f) `model X {}` in a `.ridl` parse →
      RIDL-403; (g) a package directory holding `a.typl` + `b.ridl` loads both
      files; (h) round-trip totality on a `.ridl` corpus file.
- [ ] Implement; update every `parse(`/`lex(` call site; E1 snapshots stay
      byte-identical (the Typl profile is behavior-preserving).
- [ ] Full local gate; commit.

**Done when:** both profiles lex and load; TYPL-304 and RIDL-403 fire with
recovery; every E1 test is green unchanged.

---

### Task 3: E2.1a — interaction grammar, typed AST, parser

Branch `e2-1b-interact-parser` · commits `feat(ridl-syntax): …` · model: fable.

**Read first:** ridl reference §4–§9, §12, §14.0–14.1, Appendix C; gf §2 (three
shapes), §6.1 (fallible_type); ADR-0008 decisions 1, 2;
`crates/ridl-syntax/typl.ungram` (the node contract task 2 of E1 built).

**Files:**

- Modify: `crates/ridl-syntax/typl.ungram` → rename to
  `crates/ridl-syntax/family.ungram` (it now describes more than typl; update
  the one path reference in `xtask/src/codegen.rs`),
  `crates/ridl-syntax/src/ast/generated.rs` (regenerated),
  `crates/ridl-syntax/src/ast.rs` (new member enums),
  `crates/ridl-syntax/src/syntax_kind.rs` (node kinds),
  `crates/ridl-syntax/src/parser.rs`
- Test: `crates/ridl-syntax/test_data/parser/ok/*.ridl` + `err/*.ridl` + insta
  snapshots

**Interfaces:**

- Grammar added to `family.ungram`, transliterating Appendix C with the two
  decided supersessions baked in (fallible_type per gf §6.1 / ADR-0008 decision
  1; bare init before timing per decision 2):

```ebnf
definition    = [ "internal" ] ( typl_definition | interface_def ) ;
              (* service_def joins in task 8 *)

interface_def = doc_comment? "interface" CamelCase_id
                "{" { interaction sep? } "}" ;
interaction   = signal_def | event_def | command_def | query_def
              | final_def | reserved ;

signal_def    = doc_comment? "signal"  camelCase_id ":" type_ref
                init_value? timing? ;
init_value    = "=" ( literal | SCREAMING_SNAKE_ID ) ;
event_def     = doc_comment? "event"   camelCase_id ":" type_ref timing? ;
command_def   = doc_comment? "command" camelCase_id "(" param_list ")"
                attr_block? ;
query_def     = doc_comment? "query"   camelCase_id "(" param_list ")"
                ":" return_type attr_block? ;
final_def     = doc_comment? "final"   camelCase_id ":" final_type ;

param_list    = "" | param { "," param } ;
param         = camelCase_id ":" param_type ;
param_type    = type_ref | stream_type ;
return_type   = type_ref | tuple_type | stream_type | fallible_type ;
fallible_type = type_ref "|" type_ref ;          (* gf §6.1 *)
final_type    = type_ref | array_type ;
stream_type   = "<" ( type_ref | "string" | "bytes" ) ">" ;

timing        = "@" duration | "@" "[" timing_range "]" ;
timing_range  = duration ".." duration | duration ".." | ".." duration ;
duration      = int_lit ( "us" | "ms" | "s" ) ;
```

- Parser leniency (checker narrows, the E1 discipline): a `":" return_type`
  after a command's params parses (RIDL-104 fires in task 5); timing parses on
  command/query/final (RIDL-106 and friends in task 5); attr_block parses on
  signal/event/final (RIDL-301/-106 in task 5); an init_value parses on
  event/final. `attr_block` content parsing is task 4 — until then the parser
  consumes a balanced `[ … ]` after a signature into an `AttrBlock` node holding
  raw tokens (lossless), with no inner structure.
- New `SyntaxKind` node variants: `InterfaceDef`, `SignalDef`, `EventDef`,
  `CommandDef`, `QueryDef`, `FinalDef`, `Param`, `ParamList`, `ReturnType`,
  `StreamType`, `FallibleType`, `Timing`, `TimingRange`, `AttrBlock`.
- Generated AST accessors (consumed by tasks 5–6, 8–12, 19–20):
  `InterfaceDef::{name(), members()}` where a member is
  `ast::InterfaceMember { Signal(SignalDef), Event(EventDef),
  Command(CommandDef), Query(QueryDef), Final(FinalDef),
  Reserved(ReservedEntry) }`
  (manual enum in `ast.rs`, like `StructMember`);
  `SignalDef::{name(), payload(), init_value(), timing()}`;
  `EventDef::{name(), payload(), timing()}`;
  `CommandDef::{name(), params(), return_type(), attr_block(), timing()}` (the
  lenient extras included so the checker can point at them);
  `QueryDef::{name(), params(), return_type(), attr_block(), timing()}`;
  `FinalDef::{name(), payload(), timing(), attr_block()}`;
  `Param::{name(), param_type()}`;
  `ReturnType::{type_ref(), tuple_type(), stream_type(), fallible_type()}`;
  `FallibleType::{ok(), err()}` (first/second `type_ref` child);
  `StreamType::{element_type(), element_primitive()}`;
  `Timing::{duration(), range()}`; `TimingRange::{min(), max()}` (each an
  `Option` of the duration token). Doc comments ride the existing
  `HasDocComments` trivia mechanism.
- Profile boundary completes the second ADR-0007 decision-10 item: the stream
  grammar now parses in both profiles, and a `stream_type` in a `.typl` parse
  emits **TYPL-301** (`stream type in typl context`), continuing.

**Steps:**

- [ ] Extend `family.ungram`; regenerate; drift test green.
- [ ] Failing `ok` corpus first: (a) the ridl reference Appendix A example
      verbatim (it contains no service declarations — services arrive with task
      8); (b) one file per construct family — signals with init + both timing
      forms, events, commands with params + attr placeholder, queries with all
      four return shapes (named, tuple, stream, fallible), finals with array
      type, reserved tombstones between interactions, doc comments on
      interactions, half-open timing ranges; (c) a mixed file: typl
      declarations + an interface in one `.ridl` file. Snapshots assert
      losslessness and zero errors.
- [ ] Failing `err` corpus: unclosed interface brace; missing payload type;
      `signal x : <T>` (parses — checker rejects later; snapshot the CST);
      garbage inside an interface recovering at the next interaction keyword;
      `<T>` in a `.typl` file → TYPL-301.
- [ ] Implement production by production; recovery points gain the five
      interaction keywords inside interface bodies.
- [ ] Full local gate; commit per production group (interface shell,
      signal/event, command/query, final/reserved, streams/fallible).

**Done when:** the whole `ok` corpus parses lossless with zero errors; Appendix
A parses clean; recovery inside interface bodies proven; TYPL-301 fires.

---

### Task 4: E2.1a/E2.4 — attr_block + guaranteed-subset expr grammar

Branch `e2-1c-attr-expr-grammar` · commits `feat(ridl-syntax): …` · model: opus.

**Read first:** gf §4.2–§4.5 (the single attr_block production, three forms);
ridl reference §13, Appendix C (attribute = require/ensure); the guaranteed
subset list (ridl §13, ADR-0008 backbone); typl §16.4 (TYPL-303).

**Files:**

- Modify: `crates/ridl-syntax/family.ungram`,
  `crates/ridl-syntax/src/ast/generated.rs` (regenerated),
  `crates/ridl-syntax/src/ast.rs`, `crates/ridl-syntax/src/syntax_kind.rs`,
  `crates/ridl-syntax/src/parser.rs`
- Test: parser corpus `ok/attrs_*.ridl`, `err/attrs_*.ridl` + snapshots

**Interfaces:**

- Grammar (gf §4.2 — one production, three forms; the expr grammar is the
  guaranteed subset only, precedence-climbing):

```ebnf
attr_block  = "[" { attribute sep? } "]" ;
attribute   = attr_key
            | attr_key "=" const_value
            | ( "require" | "ensure" ) expr ;
attr_key    = camelCase_id ;
const_value = literal | SCREAMING_SNAKE_ID
            | "(" const_value { "," const_value } ")" ;

expr        = or_expr ;
or_expr     = and_expr { "||" and_expr } ;
and_expr    = cmp_expr { "&&" cmp_expr } ;
cmp_expr    = add_expr [ ( "==" | "!=" | "<" | "<=" | ">" | ">=" )
                         add_expr ] ;
add_expr    = mul_expr { ( "+" | "-" ) mul_expr } ;
mul_expr    = unary_expr { ( "*" | "/" | "%" ) unary_expr } ;
unary_expr  = [ "!" | "-" ] postfix_expr ;
postfix_expr= primary { "." ( camelCase_id | SCREAMING_SNAKE_ID ) } ;
primary     = literal | duration_lit | path_head | "(" expr ")" ;
path_head   = camelCase_id | CamelCase_id | SCREAMING_SNAKE_ID ;
```

- New `SyntaxKind` node variants: `Attribute`, `AttrValue`, `BinaryExpr`,
  `PrefixExpr`, `MemberExpr`, `PathExpr`, `ParenExpr`, `LiteralExpr` (the
  `AttrBlock` node from task 3 gains real children; the raw-token fallback is
  removed).
- Generated accessors (consumed by tasks 5, 11, 12, 21):
  `AttrBlock::attributes()`;
  `Attribute::{key(), value(), predicate_kind(), expr()}` where
  `predicate_kind()` returns `Option<ast::PredicateKind { Require,
  Ensure }>`;
  `BinaryExpr::{lhs(), op_token(), rhs()}`;
  `PrefixExpr::{op_token(), operand()}`; `MemberExpr::{base(), member_token()}`;
  `PathExpr::name_token()`; `ParenExpr::inner()`; `LiteralExpr::token()`. A
  manual `ast::Expr` enum in `ast.rs` casts over the six expr node kinds.
- Non-predicate attributes (flag and assignment forms) parse everywhere an
  attr_block parses — the semantic allow-list is task 5's job (FORM-106/107
  /108); the grammar stays one production (gf §4.3).
- Profile boundary completes the third ADR-0007 decision-10 item: an attr_block
  whose attribute is `require`/`ensure` in a `.typl` parse emits **TYPL-303**
  (`require/ensure attribute in typl context`), continuing.
- `<`/`>` ambiguity rule, stated for the record: `<` opens a `stream_type` only
  in param-type and return-type position (task 3's productions); inside an
  `expr` it is always the comparison operator. The two positions never overlap.
- Empty blocks: gf §4.2 requires at least one attribute per block. The parser
  accepts `[]` losslessly (an empty `AttrBlock` node in the CST) and emits
  **FORM-101** (`expected an attribute`) at the closing bracket — structural
  expectations live in the parser, the E1 FORM-101 discipline.

**Steps:**

- [ ] Extend `family.ungram`; regenerate; drift test green.
- [ ] Failing `ok` corpus: every §13 example verbatim
      (`require position != GearPosition.PARK || currentSpeed == 0.0`,
      `require window > 0ms`, `ensure result >= 0.0`, `require min < max`,
      `require max <= MAX_SPEED`); precedence dumps (`a || b && c` parses
      `a || (b && c)`; `!a && b` parses `(!a) && b`; `a + b * c < d` parses
      `(a + (b*c)) < d`); member chains (`filter.severity`,
      `GearPosition.PARK`); a flag and an assignment attribute (`[ someKey ]`,
      `[ someKey = 3 ]`) parse into `Attribute` nodes.
- [ ] Failing `err` corpus: unterminated attr block; an empty `[]` block →
      FORM-101 with the `AttrBlock` node kept losslessly; `require` with empty
      expr; `a ||` dangling; `require x > 0ms` inside a `.typl` file → TYPL-303.
- [ ] Implement (precedence climbing inside the existing recursive-descent
      parser; one function per precedence level).
- [ ] Full local gate; commit.

**Done when:** every §13 example parses with reviewed CST snapshots; precedence
is proven by snapshot; TYPL-303 fires; the raw-token AttrBlock fallback is gone.

---

### Task 5: E2.1b — interaction resolver + structural checker

Branch `e2-1d-interact-check` · commits `feat(ridl-sem): …` · model: fable.

**Read first:** ridl reference §4.1, §5.1, §6.1, §7.1, §8, §11, §12.3, §14.1,
§16 (the codes below); gf §4.3 (attribute allow-list); ADR-0008 decision 2; the
E1 as-built `resolve.rs`/`check.rs` (first-wins, FileId scheme).

**Files:**

- Modify: `crates/ridl-sem/src/resolve.rs` (interface symbols),
  `crates/ridl-sem/src/check.rs` (interaction structural pass)
- Test: resolver + checker tests over in-memory workspaces

**Interfaces:**

- Consumes: task 3–4 AST; E1 `resolve_package`/`check_package` (signatures
  unchanged: `(db, ws: Workspace, pkg: Package, std: Package)`).
- Produces (consumed by tasks 6, 8–12, 19–20):

```rust
// resolve.rs — one new variant; Symbol/Resolution shapes unchanged
pub enum SymbolKind { Type, Const, Struct, Enum, EnumSet, Union,
                      Interface }          // Interface is new
// check.rs — the structural pass, shared by lowering (task 6)
pub struct CheckedInterface {
    pub def: ast::InterfaceDef,
    // (member, 1-based ordinal) — reserved tombstones occupy slots
    pub members: Vec<(ast::InterfaceMember, u32)>,
}
```

- Interface names enter the package symbol table as `SymbolKind::Interface`; a
  duplicate against any declaration is TYPL-009 first-wins (the E1 tiebreak,
  unchanged).
- Structural rules, each with a test (payload/type references resolve through
  `Resolution.symbols` exactly as struct fields do; an unknown payload name
  keeps the E1 unknown-type message shape):
  - RIDL-104 return type on `command` (error);
  - RIDL-105 `query` returning `()` — an empty tuple return (error);
  - RIDL-106 timing annotation or attribute block on `final` (error);
  - RIDL-107 type declaration inside an `interface` body (error; the parser
    recovers into an ErrorNode there — the checker attaches the code from the
    recovered node's first token when it is a typl definition keyword);
  - RIDL-109 signal payload with no derivable init and no `= value` override
    (error; uses E1 `init` derivation + `const_value` for SCREAMING_SNAKE
    references);
  - RIDL-110 signal `= value` override violating the payload constraints (error;
    E1 `scalar` validation);
  - RIDL-201 stream `<T>` on `signal`/`event` payload (error);
  - RIDL-202 stream element not a named type, `string`, or `bytes` (error);
  - RIDL-301 `require`/`ensure` on `signal`/`event`/`final` (error);
  - RIDL-302 `ensure` on `command` (error);
  - RIDL-401 interaction re-declared under a `reserved` name (error);
  - RIDL-402 duplicate interaction name within an interface (error, first-wins
    for lowering);
  - timing or init parsed on a kind that does not admit them (event init,
    command/query timing): §16.1 scopes RIDL-106 to `final`, and the reference
    grammar simply has no such production — so the checker emits **FORM-102**
    (`unexpected token`) with a pointed message
    (`timing annotation not valid on command`); no new RIDL code is burned;
  - attribute allow-list (gf §4.3): FORM-106 unknown attribute key, FORM-107 key
    not allowed on this declaration kind (every flag/ assignment key in E2 —
    only `require`/`ensure` predicates are consumable), FORM-108 duplicate key
    in one block.
- Ordinal assignment: 1-based, declaration order, one sequence per interface
  across all kinds, reserved tombstones counted (ridl §11) —
  `CheckedInterface.members` carries the assignment; task 6 copies it into IR,
  task 20 renders it as inlay hints.

**Steps:**

- [ ] Failing tests first: one per code above (including
      `reserved
      resetCounters` followed by `query resetCounters(...)` →
      RIDL-401, and `type X: m` inside an interface body → RIDL-107), plus an
      ordinal test on the Appendix A interface — currentSpeed `#1`, engineTemp
      `#2`, warnings `#3`, doorOpened `#4`, setGear `#5`, the reserved tombstone
      `#6`, getAverageSpeed `#7`, streamFaults `#8`, getFaultPage `#9`,
      softwareVersion `#10`, capabilities `#11` — plus a resolution test:
      payload `Speed` resolves through the import, `NoSuchType` draws the
      unknown-type message.
- [ ] Implement the pass inside `check_package` (diagnostics only — IR untouched
      until task 6).
- [ ] Full local gate; commit.

**Done when:** every listed code fires in a test; ordinals match the Appendix A
worked example; E1 behavior is untouched for pure-typl packages.

---

### Task 6: E2.1c — lower interactions to IR v2, flip the pipeline

Branch `e2-1e-lower-v2` · commits `feat(ridl-sem): …` / `feat(ridlc): …` /
`refactor(ridl-ir): …` · model: fable.

**Read first:** task 1's proto (the exact messages); ridl reference §3.1
(envelope never in payloads — nothing to lower for it), §11; ADR-0008 decision
8; E1 task 13's lowering shape in `check.rs`.

**Files:**

- Modify: `crates/ridl-sem/src/check.rs` (lower to v2),
  `crates/ridl-ir/src/lib.rs` + `build.rs` (remove v1),
  `crates/ridlc/src/lib.rs` (v2 in `Emit::IrJson` docs + goldens),
  `backends/rust/src/lib.rs` + `backends/rust/src/defaults.rs` (port the typl
  surface to `ridl_ir::v2`; interfaces/services are accepted and passed through
  untouched in this task — task 15 adds the Rust interaction codegen)
- Test: updated golden snapshots (`.ir.json` now v2), lowering unit tests

**Interfaces:**

- Consumes: `CheckedInterface` (task 5), IR v2 (task 1).
- Produces (consumed by tasks 8–18, 20–22):

```rust
pub struct CheckedPackage {
    pub ir: ridl_ir::v2::Package,     // was v1
    pub diagnostics: Vec<Diagnostic>,
}
```

- Lowering rules: interfaces land in `Package.interfaces` in source order; each
  interaction is a `Decl` with `ordinal` set, `kind` per task 1's members 16–21,
  doc comments in `doc`, `@labels`/`@deprecated` doc tags in
  `labels`/`deprecated` (the E1 doc-tag scanner applied to interactions);
  payload references are canonical (`pkg.Name` cross-package); RIDL-402
  duplicates lower first-wins only. `Timing` is lowered **empty** in this task
  (`mode = UNSPECIFIED`) — task 9 populates it; `Contract.source` lowers as the
  token text of the expr (canonicalization arrives in task 11);
  `QueryDef.return_type` lowers all four shapes including `FallibleType` (arm
  semantics checked in task 10). Streams lower to `FieldType.stream`.
- v1 is deleted (proto file, build entry, module) — decision 8's "retained until
  v2 lowering lands" ends here, mirroring E1's v0 retirement.

**Steps:**

- [ ] Failing tests: the Appendix A package lowers clean; its IR v2 JSON
      snapshot is the new golden and shows ordinals 1–11 (task 5's assignment)
      and the stream return. Appendix A's `getFaultPage` returns the **named**
      union `FaultPageResult` — assert it lowers as a named `ReturnType.value`;
      add a corpus variant returning an inline `FaultPage | DiagError` to assert
      `ReturnType.fallible`. A mixed package (typl + ridl files) lowers both
      surfaces into one `Package`.
- [ ] Port `backends/rust` + `ridlc` goldens to v2; delete v1; full local gate;
      commit.

**Done when:** the pipeline emits IR v2 end to end (`ridlc build --emit
ir-json`
writes v2 JSON); v1 is gone; all E1 goldens are re-reviewed as v2.

---

### Task 7: E2.12 — expr-core specification (document)

Branch `e2-12-expr-core-spec` · commits `docs(family): …` · model: opus.

**Read first:** ridl reference §13 (the guaranteed subset and the four
executions); roadmap E5.1 row (total functions, `let`, `if`/`case`/`match`,
bounded combinators, totality checks, RMDL-1xx); family overview §2 (core
index); gf §4.2 (predicate attribute form); ADR-0008 decision 10.

**Files:**

- Create: `docs/specification/expr-core-specification.md`
- Modify: `docs/specification/ridl-family-overview.md` (index the new document
  in the doctrine/reference map, one line)

**Interfaces:**

- Produces: the normative expr-core document tasks 11, 12, and 21 cite, and the
  E5.1 implementation later extends. Required content (no code — this is a
  specification):
  - The layer model: the **guaranteed subset** (E2, this epic) as a strict
    subset of the **function layer** (E5.1) — one grammar, the subset marked
    normatively.
  - The full grammar (EBNF): the task 4 subset productions verbatim, plus the
    E5.1 layer — `let` bindings, `if`/`case`/`match` expressions, total function
    definitions, bounded combinators (`all`, `any`, `count` over bounded
    collections) — each marked `V2 (E5.1)`.
  - The rejection list, normative: recursion, loops, `last` inside functions,
    side effects, calls to anything but (V2) declared total functions.
  - Typing rules for the subset: operand/result types per operator (comparison →
    boolean over ordered operands of one type; `&&`/`||`/`!` over boolean;
    arithmetic over numeric operands of one named type or literal; `%`
    integer-only; duration compares against duration; enum access `Enum.MEMBER`
    types as its enum; tuple-field access; nominal typing per typl §5.7 — no
    implicit cross-type arithmetic).
  - Reference environment: parameters; `result` (ensure only); package and
    imported constants; enum values; the enclosing interface's own signals
    (require reads the latest published value); nothing else.
  - The evaluation domains: exact rational arithmetic (typl exactness); duration
    in microseconds; evaluation is total on well-typed input.
  - The RIDL-306 boundary: any form outside the subset is RIDL-306 (error) in
    E2, lifted per-form as E5.1 lands.
  - An "Alternatives considered" section (per the working-memory doctrine)
    recording at least: quoted/string predicates (rejected — unchecked), full E5
    grammar now (rejected — sequencing), external assertion language (rejected —
    one grammar doctrine).
- The document is written as implemented-subset-normative: nothing in it
  contradicts what tasks 4/11 build; the V2 layer is explicitly marked
  forward-looking (the one document where that is its purpose, like the
  rmdl/rsdl references).

**Steps:**

- [ ] Draft; cross-check every subset form against the task 4 grammar and the
      ridl §13 examples (each §13 example must type-check on paper under the
      stated rules).
- [ ] `just check` (prim + markdownlint) green; commit.

**Done when:** the document exists, is indexed in the overview, matches the task
4 grammar exactly, and marks the E5.1 layer; task 11 implements against it.

---

### Task 8: E2.13 — services: grammar, resolution, catalog, IR

Branch `e2-13-services` · commits `feat(ridl-syntax): …` / `feat(ridl-sem): …` ·
model: fable.

**Read first:** ridl reference §14.5–§14.6 (service semantics, both declaration
forms), §16.4 (RIDL-140/141); ADR-0008 decision 6; ADR-0002 §1 (dotted-name
shape).

**Files:**

- Modify: `crates/ridl-syntax/family.ungram` + regenerated AST +
  `syntax_kind.rs` + `parser.rs` (service production),
  `crates/ridl-sem/src/resolve.rs` (service symbols),
  `crates/ridl-sem/src/check.rs` (service checks + lowering),
  `crates/ridl-core/src/package.rs` (workspace-level catalog query)
- Test: parser corpus `ok/services.ridl`; checker tests; catalog tests over a
  two-package workspace

**Interfaces:**

- Grammar — `service_def` is absent from Appendix C; this task authors it
  (recorded for the close-out doc sync):

```ebnf
definition    = [ "internal" ] ( typl_definition | interface_def
                               | service_def ) ;
service_def   = doc_comment? "service" dotted_name
                ( ":" type_ref | "{" { interaction sep? } "}" ) ;
dotted_name   = lowercase_id { "." lowercase_id } ;
```

- New `SyntaxKind` nodes: `ServiceDef`, `DottedName`. Accessors:
  `ServiceDef::{name(), interface_ref(), inline_members()}` where
  `inline_members()` yields `ast::InterfaceMember` (task 3's enum);
  `DottedName::segments()`.
- Produces (consumed by tasks 14–15, 17, 22):

```rust
// ridl-core/src/package.rs — the system-wide catalog (ridl §14.5)
pub struct ServiceCatalog {
    // dotted service name -> (declaring package, interface reference or
    // "" for an inline shape)
    pub entries: BTreeMap<String, CatalogEntry>,
    pub diagnostics: Vec<Diagnostic>,        // RIDL-140 duplicates
}
pub struct CatalogEntry { pub package: String, pub interface_ref: String }
#[salsa::tracked(returns(clone))]
pub fn service_catalog(db: &dyn salsa::Database, ws: Workspace,
                       std: Package) -> ServiceCatalog
```

- Semantics with codes: RIDL-140 duplicate service name across the whole
  workspace (error, in `service_catalog` — flat global namespace; both
  declarations labeled); RIDL-141 `service` naming a type that is not an
  `interface` when it has no inline shape (error, per-package in
  `check_package`); an inline shape runs the full task 5 structural pass (own
  ordinal sequence). The codes stay 140/141 as documented even though they sit
  in the 4xx table — ADR-0008 decision 6 accepts the anomaly.
- Lowering: `Package.services` per task 1's `Service` message — `interface_ref`
  canonical, or `inline` as an `Interface` with `name ==
  ""`. Services are
  posture-neutral: nothing else lowers (providing/ requiring is E6).
- `SymbolKind` is **not** extended: a service's dotted name lives in the catalog
  namespace, not the type namespace (`service.member` addressing has no E2
  consumer — rsdl binds in E6).

**Steps:**

- [ ] Failing parser corpus: the three §14.5 named-ref examples and the
      `veh.hvac.cabin` inline example verbatim; service declarations for the
      corpus interfaces are authored after the §14.5 model (Appendix A contains
      none).
- [ ] Failing checker tests: RIDL-141 on `service x.y : Speed`; RIDL-140 across
      two packages declaring `veh.adas.cruise`; inline shape with a duplicate
      interaction name → RIDL-402; catalog resolves a cross-package
      `interface_ref` through imports.
- [ ] Implement grammar → resolve → catalog → lowering; wire `service_catalog`
      into `ridlc::compile_workspace` so its diagnostics render with the rest.
- [ ] Full local gate; commit.

**Done when:** services declare, resolve, and appear in IR v2 (the roadmap exit
line); RIDL-140/141 fire in tests; the catalog is queryable.

---

### Task 9: E2.2 — timing: validity, defaults, resolved IR

Branch `e2-2-timing` · commits `feat(ridl-sem): …` / `feat(ridl-core): …` ·
model: opus.

**Read first:** ridl reference §9 (whole section, incl. §9.1 defaults and §9.2
validity), §16.1; gf §6.2 (generic min/max — the authoritative semantics,
ADR-0008 decision 1); ADR-0008 decision 12; ADR-0002 §4–5 (manifest precedence).

**Files:**

- Create: `crates/ridl-sem/src/timing.rs`
- Modify: `crates/ridl-core/src/manifest.rs` (`[defaults]` section),
  `crates/ridl-core/src/workspace.rs` (precedence merge),
  `crates/ridl-sem/src/check.rs` (populate IR `Timing`)
- Test: `timing.rs` unit tests; manifest tests; checker tests

**Interfaces:**

- Produces (consumed by tasks 14–15, 17, 20):

```rust
// ridl-core: manifest gains the section (ridl §9.1); the raw string is
// stored unparsed — ridl-core cannot depend on ridl-sem, so parsing and
// MANI-009 happen in the checker (below)
pub struct Manifest {
    pub kind: ManifestKind,
    pub imports: BTreeMap<String, String>,
    pub default_timing: Option<String>,   // raw "[100ms..1000ms]" text
}
// ridl-core: the winning raw default rides the Package salsa input
// (package [defaults] shadows workspace [defaults], merged at load)
#[salsa::input] pub struct Package { /* existing fields … */
    default_timing: Option<String> }
// ridl-sem/src/timing.rs
pub enum TimingMode { StrictPeriodic, Range }
pub struct TimingSpec {
    pub mode: TimingMode,
    pub min_us: Option<ExactValue>,   // rate floor (gf §6.2)
    pub max_us: Option<ExactValue>,   // staleness bound
    pub default_applied: bool,
}
pub enum InteractionKind { Signal, Event, Command, Query, Final }
/// Parse + validate one timing annotation, or apply the default.
/// Diagnostics: RIDL-100 (default applied, warning), RIDL-101 (min>max),
/// RIDL-102 (zero/negative duration), RIDL-103 (@Xms on event),
/// RIDL-108 (@[X..X], warning).
pub fn resolve_timing(annot: Option<&ast::Timing>, kind: InteractionKind,
                      default: &TimingSpec, file: FileId)
    -> (Option<TimingSpec>, Vec<Diagnostic>)
/// Parse a `[defaults].timing` string ("[100ms..1000ms]"). Called once
/// per package in check_package; a malformed string is MANI-009 (span:
/// the package's first file, message naming the manifest). The built-in
/// default `[100ms..1000ms]` (ridl §9.1) is the fallback.
pub fn parse_default_timing(text: &str) -> Result<TimingSpec, String>
```

- Rules: durations convert to exact microseconds (`us`/`ms`/`s` ×1/×1000/
  ×1000000, integer only — the lexer guarantees no fractions); the default
  applies only to **untimed signal and event** (never strict-periodic, never
  command/query/final — those return `None`); precedence is package `[defaults]`
  → workspace `[defaults]` → built-in (ADR-0002 §5 idiom, merged in
  `load_workspace`); `default_applied = true` marks the applied default in IR
  (decision 12) — the diff rule "editing `[defaults].timing` is a contract
  change" follows from comparing resolved bounds (task 17). MANI-005 (unknown
  manifest key) no longer fires for `[defaults]`.
- `check.rs` fills `SignalDef.timing`/`EventDef.timing` from `TimingSpec`
  (exact-decimal strings; both bounds set for strict periodic and for the
  applied default; half-open explicit ranges keep the absent side unset) — the
  task 6 `mode = UNSPECIFIED` placeholder dies here.

**Steps:**

- [ ] Failing tests: every §9.2 rule (RIDL-101/102/108 + RIDL-103 on
      `event x : T @10ms`); RIDL-100 warning text names the applied bounds;
      `@10ms` → strict with `min_us == max_us == "10000"`; `@[20ms..]` → range
      with max unset; untimed signal under a package
      `[defaults] timing = "[50ms..2s]"` resolves `"50000"`/`"2000000"` with
      `default_applied`; package default shadows workspace default; MANI-009 on
      `timing = "fast"`; command with no timing → `Timing` absent from IR.
- [ ] Implement; full local gate; commit.

**Done when:** every timed/untimed interaction in the corpus carries resolved
concrete bounds + mode + flag in IR v2 exactly per decision 12.

---

### Task 10: E2.3 — errors-as-data: inline `T | E` semantics

Branch `e2-3-fallible` · commits `feat(ridl-sem): …` · model: opus.

**Read first:** ridl reference §10.1–§10.3, §16.3; gf §6.1 (the decided rules),
§6.4 (Stratum-3 wording); ADR-0008 decisions 1, 4.

**Files:**

- Modify: `crates/ridl-sem/src/check.rs` (fallible-return checks + lowering)
- Test: checker tests + IR snapshot

**Interfaces:**

- Consumes: task 3's `FallibleType` AST, task 1's `ReturnType`/ `FallibleType`
  IR, task 6's lowering.
- Produces: the checked fallible-return semantics tasks 14–15, 17, 19–20 rely on
  —
  - Left arm: any non-error named type; an `error`-modified left arm is the
    checker's arm-order error (message
    ``success arm `X` is an error type — write `T | E` with the error
    arm second``,
    code RIDL-303 family: emitted under **RIDL-303** since a fallible return
    whose success arm is an error type has no success path).
  - Right arm: exactly one **error type** (`Symbol.is_error`); a non-error right
    arm is an error with the gf §6.1 wording
    (`` `Y` is not an error type — compose failure kinds into an error
    union first ``),
    also RIDL-303 family → emitted as RIDL-303.
  - RIDL-303 proper: a query returning a **bare** error type (no success path),
    inline or named.
  - RIDL-304 (warning): an `error`-typed or result-union **parameter** on
    `command`/`query`.
  - RIDL-307 (warning): an `error enum` declaring a Stratum-2 category name —
    any of `INVALID_VALUE`, `PRECONDITION_FAILED`, `CONTRACT_BROKEN`,
    `UNKNOWN_INTERACTION` (checked wherever error enums lower, both profiles —
    the categories are reserved vocabulary).
  - Lowering: `ReturnType.fallible` with canonical arm references; the transport
    identity is **not stored** — consumers derive it via
    `fallible_transport_identity` (task 1, decision 4).
  - A named result union (`UnionDef.is_result`) in return position stays legal
    and lowers as `ReturnType.value` — the canonical-form lint is task 19
    (RIDL-308), not an error here.

**Steps:**

- [ ] Failing tests: `query calibrate(target: Axle): CalReport | CalError`
      checks clean and lowers with both arms canonical; swapped arms → RIDL-303;
      `CalReport | Speed` (right arm not error) → RIDL-303;
      `query f(): CalError` and `query f(): CalError | CalError` → RIDL-303;
      `command c(e: CalError)` → RIDL-304; `error enum X { INVALID_VALUE = 0 }`
      → RIDL-307; the IR JSON snapshot shows
      `"fallible": { "ok": "CalReport", "err":
      "veh.common.CalError" }`
      for a cross-package error arm.
- [ ] Implement; full local gate; commit.

**Done when:** fallible queries check and lower per gf §6.1; all four codes fire
in tests; identity derivation is exercised by a unit test.

---

### Task 11: E2.4 — expr subset type checking for require/ensure

Branch `e2-4-expr-check` · commits `feat(ridl-sem): …` · model: opus.

**Read first:** the task 7 expr-core specification (typing rules, reference
environment — normative); ridl reference §13, §16.3 (RIDL-305/306); typl §5.7
(nominal typing).

**Files:**

- Create: `crates/ridl-sem/src/expr.rs`
- Modify: `crates/ridl-sem/src/lib.rs`, `crates/ridl-sem/src/check.rs`
- Test: `expr.rs` unit tests + checker integration tests

**Interfaces:**

- Consumes: task 4 expr AST, task 5 `CheckedInterface`, E1
  `scalar::ExactValue` + `const_value`.
- Produces (consumed by tasks 12, 21):

```rust
pub enum ExprType {
    Boolean,
    Numeric(String),        // canonical named-type ref, or "" for a bare
                            // numeric literal (unifies with any Numeric)
    Duration,
    EnumType(String),       // canonical enum reference
    Tuple(String),          // canonical named tuple-carrying type
}
pub struct ContractScope<'a> {
    pub params: &'a [(String, ExprType)],
    pub result: Option<ExprType>,     // Some only for ensure on query
    pub signals: &'a [(String, ExprType)],  // the interface's own signals
    pub resolution: &'a Resolution,   // constants + enum values
}
/// Type-check one require/ensure expression against the task 7 spec.
/// RIDL-306 for any form outside the guaranteed subset (unknown
/// reference, cross-named-type arithmetic, non-boolean root, …);
/// RIDL-305 (warning) for an ensure that never references `result`.
pub fn check_contract_expr(expr: &ast::Expr, scope: &ContractScope)
    -> (Option<ExprType>, Vec<Diagnostic>)
/// The canonical one-line rendering lowered into IR Contract.source:
/// single spaces around binary operators, no spaces inside parens or
/// around `.`, minimal parentheses (re-inserted only where precedence
/// requires). Stable across formatting changes so ridl-diff does not
/// see phantom contract edits.
pub fn canonical_expr_text(expr: &ast::Expr) -> String
/// The references the expr reads, resolved — feeds task 12's stubs.
pub struct ExprRefs { pub params: Vec<String>, pub signals: Vec<String>,
                      pub uses_result: bool, pub consts: Vec<String> }
pub fn collect_refs(expr: &ast::Expr, scope: &ContractScope) -> ExprRefs
```

- Typing rules (from the task 7 spec, enforced here): root must be `Boolean`;
  comparison operands unify (same named type, literal-vs-named, or
  duration-vs-duration); `&&`/`||`/`!` boolean-only; arithmetic numeric-only and
  nominally consistent; `%` integer-backed only; `Enum.MEMBER` resolves through
  the resolution and types as its enum; tuple-field access on a tuple-returning
  query's `result` (`result.min >= 0.0` after `: (min: Speed, max: Speed)`);
  duration literals type as `Duration`; references resolve in scope order —
  param, `result` (ensure only), the interface's own signals (require only — the
  §13 table scopes ensure to `result` and parameters), constants, enum types.
  Anything else is RIDL-306 with a message naming the offending form.
- `check.rs` runs this for every require/ensure that task 5's placement rules
  admitted, and replaces the task 6 raw `Contract.source` with
  `canonical_expr_text`.

**Steps:**

- [ ] Failing tests: every §13 example type-checks clean; RIDL-306 on
      `require unknownName > 0`, on `require speed + window > 0` (Speed +
      Duration mixed), on `require 3` (non-boolean root), on
      `ensure currentSpeed >= 0.0` (signal in ensure); RIDL-305 on
      `ensure window > 0ms`; `canonical_expr_text` normalizes `result>=0.0` to
      `result >= 0.0` and strips redundant parens; `collect_refs` on the §14.0
      setGear require returns `params: [position]`, `signals: [currentSpeed]`.
- [ ] Implement; full local gate; commit.

**Done when:** contract clauses type-check per the spec, RIDL-305/306 fire, and
IR carries canonical text.

---

### Task 12: E2.5 — observer-stub lowering

Branch `e2-5-observers` · commits `feat(ridl-sem): …` · model: opus.

**Read first:** ridl reference §13 (one assertion, four executions); concept
note §9.2; roadmap E2.5 row ("observers represented in IR"); task 1's `Contract`
message.

**Files:**

- Modify: `crates/ridl-sem/src/check.rs` (populate the stub fields)
- Test: checker IR snapshot tests

**Interfaces:**

- Consumes: `collect_refs`/`ExprRefs` (task 11), IR `Contract` (task 1).
- Produces (consumed by tasks 14–15, 21): fully populated observer stubs — for
  every lowered `Contract`: `signal_refs` (canonical `Interface.signalName`
  entries), `param_refs`, `uses_result`, and `observer_id` =
  `"{Interface}.{interaction}.{require|ensure}[{i}]"` with `i` the 0-based index
  among the interaction's clauses of that kind (e.g.
  `VehicleStatus.getAverageSpeed.ensure[0]`). The id is the stable handle the E5
  observer runtime and the test plane will address; it never changes when other
  clauses are appended. The clause itself rides the IR as canonical source text
  in `Contract.source` (ADR-0008 decision 14) until E5.1 restructures it.

**Steps:**

- [ ] Failing tests: the Appendix A interface lowers with
      `VehicleStatus.setGear.require[0]` carrying
      `signal_refs: ["VehicleStatus.currentSpeed"]`, `param_refs: ["position"]`;
      the ensure stub carries `uses_result: true`; two requires on one command
      number `[0]`/`[1]`; the IR JSON snapshot is reviewed.
- [ ] Implement (a thin pass over task 11's outputs during lowering).
- [ ] Full local gate; commit.

**Done when:** every contract clause in the corpus is an addressable observer
stub in IR v2 — the roadmap exit line.

---

### Task 13: E2.6a — TypeScript backend: typl surface

Branch `e2-6a-ts-types` · commits `feat(backends): …` · model: opus.

**Read first:** typl reference Appendix D (language-layer mapping table — the TS
column does not exist; this task defines it, mirroring the Rust column's
decisions); `backends/rust/src/lib.rs` (the per-construct structure to mirror);
ADR-0008 decision 7.

**Files:**

- Create: `backends/typescript/Cargo.toml` (crate `ridl-backend-ts`),
  `backends/typescript/src/lib.rs`, `backends/typescript/src/tests.rs` (the root
  `Cargo.toml` `backends/*` glob picks the crate up unchanged)
- Test: per-construct insta snapshots + an optional `tsc` compile check

**Interfaces:**

- Produces (consumed by tasks 14–15, 22):

```rust
pub struct GeneratedTs { pub source: String }   // one module per package
pub enum GenerateError { Unrepresentable(String) }
pub fn generate(pkg: &ridl_ir::v2::Package)
    -> Result<GeneratedTs, GenerateError>
```

- Mapping (the TS language layer, fixed here): named scalar → branded type
  `export type Speed = number & { readonly __ridl: 'veh.common.Speed' };`
  (U64/I64 widths brand `bigint` instead of `number` — exactness beyond 2^53);
  const → `export const MAX_SPEED = 250.0;` (`as const` for strings); struct →
  `export interface DoorPayload { sensorId: number;
  isOpen: boolean; }` with
  optional fields as `name?:`; enum →
  `export enum GearPosition { PARK = 0, … }`; enumset → branded number +
  `export const WarningFlagsBits = { … } as const;`; union → discriminated union
  `export type FaultPageResult = { kind: 'page'; value: FaultPage } |
  { kind: 'err'; value: DiagError };`;
  tuple → inline object type with the named fields; array → `readonly T[]` with
  bounds in a JSDoc `@bounds` tag; map → `ReadonlyArray<readonly [K, V]>`
  (deterministic, the Rust `Vec<(K, V)>` decision carried over); init values →
  one `export function init<TypeName>(): T` per named type and struct, derived
  from IR `InitValue` recursively (the composite-derivation rule from the IR
  comments); doc comments → JSDoc with units and ranges; `internal` → not
  exported (module-local); deprecated → `@deprecated` JSDoc.
- Emission is a plain string emitter (no new dependencies); output is
  deterministic and stable-ordered (source order, like the Rust backend);
  codegen is total — `GenerateError`, never panic.
- The `tsc` check: a test compiles the generated module with
  `npx --no-install tsc --noEmit --strict` **when a `tsc` binary is
  discoverable** (`which tsc` / `npx --no-install`), and is skipped with a
  printed notice otherwise — the snapshot tests are the gate; the compile check
  is best-effort local evidence (network-free, mirroring the E1 rustc-compile
  precedent as far as the toolchain allows).

**Steps:**

- [ ] Failing snapshot tests first, one per construct family (scalar + unit doc,
      U64 bigint brand, const, struct with optional + reserved skip, enum,
      enumset both forms, result union, tuple, arrays, map, init functions incl.
      a recursive struct init).
- [ ] Implement; verify the typl Appendix B example generates and (if tsc
      present) compiles strict.
- [ ] Full local gate; commit.

**Done when:** the typl surface of the corpus generates deterministic,
snapshot-reviewed TS; strict tsc accepts it where tsc exists.

---

### Task 14: E2.6b — TypeScript backend: interactions + services

Branch `e2-6b-ts-interact` · commits `feat(backends): …` · model: opus.

**Read first:** ridl reference §4.4–§4.5 (provenance), §6–§8, §10.1 (binding
split), §12 (streams), Appendix B (codegen targets — the TS column's spirit); gf
§6.4 (Stratum-3 wording for generated comments); task 13's emitter.

**Files:**

- Modify: `backends/typescript/src/lib.rs`
- Create: `backends/typescript/src/interact.rs`
- Test: per-kind insta snapshots; the Appendix A corpus package generates

**Interfaces:**

- Consumes: IR v2 interfaces/services (tasks 6, 8), resolved timing (task 9),
  fallible returns (task 10), observer stubs (task 12),
  `fallible_transport_identity` (task 1).
- Produces (consumed by task 22): the interaction mapping, fixed here — emitted
  once per module, a small runtime-neutral vocabulary:

```ts
export type Provenance = 'init' | 'live' | 'invalid';
export interface SignalHandle<T> {
  read(): { value: T; provenance: Provenance };
  subscribe(fn: (value: T, provenance: Provenance) => void): () => void;
}
export interface EventHandle<T> {
  subscribe(fn: (occurrence: T) => void): () => void;
}
export type Result<T, E> =
  { ok: true; value: T } | { ok: false; error: E };
```

- Per interface `X`: a consumer face `export interface XConsumer` — signals as
  `SignalHandle<T>` properties, events as `EventHandle<T>`, finals as `readonly`
  properties, commands as `name(params): Promise<void>` (the ack is
  runtime-internal — the generated comment uses the §6.1 wording), queries as
  `name(params): Promise<R>` where a fallible return is
  `Promise<Result<Ok, Err>>` and a stream return is `AsyncIterable<T>` (stream
  params take `AsyncIterable<T>`); and a provider face
  `export interface XProvider` — command/query handlers plus `publish`-side
  signal/event emitters (`currentSpeed: { publish(value: Speed): void }`).
  Transport-error comments use the gf §6.4 wording verbatim:
  `infrastructure failure —
  detected, undeclared`.
- Timing metadata: per interface,
  `export const xTiming = { currentSpeed: { mode: 'strict-periodic',
  minUs: 10000n, maxUs: 10000n, defaultApplied: false }, … } as const;`
  (bigint microseconds — exactness preserved).
- Contracts: per interaction, the observer stubs as data —
  `export const xContracts = [{ id: 'VehicleStatus.setGear.require[0]',
  kind: 'require', source: '…', signals: […], params: […] }] as const;`.
- Services:
  `export const services = { 'veh.adas.cruise':
  { interface: 'CruiseControl' } } as const;`
  (inline shapes name their generated anonymous interface
  `Service_veh_adas_cruise`).
- Fallible transport identity: emitted as a JSDoc tag
  `@transportIdentity VehicleStatus#9:FaultPage|DiagError` on the query method,
  from `fallible_transport_identity`.

**Steps:**

- [ ] Failing snapshots: one per kind (signal with init + provenance, event,
      command with require, fallible query, tuple-return query, bidirectional
      stream query, final with array), one interface with all kinds (Appendix
      A), services both forms, the timing and contracts consts.
- [ ] Implement; tsc-strict check as task 13 (skip-if-absent).
- [ ] Full local gate; commit.

**Done when:** the Appendix A package generates a complete, deterministic TS
module — IR-neutrality proven by a second backend (the roadmap exit line).

---

### Task 15: E2 exit criterion — Rust backend: interactions + services

Branch `e2-rust-interact` · commits `feat(backends): …` · model: opus.

This task is the Rust half of the E2 exit criterion "ridl interfaces compile to
Rust **and** a second backend from one IR" — tasks 13–14 (E2.6, the TypeScript
half) and this task jointly satisfy it. It mirrors task 14's mapping decisions
in Rust.

**Read first:** ridl reference §4.4–§4.5 (provenance), §6–§8, §10.1 (binding
split), §10.3 + gf §6.4 (transport wording), §12 (streams), Appendix B; task
14's mapping (the decisions to mirror); `backends/rust/src/lib.rs` as-built
(quote/prettyplease structure, the rustc-compile harness); tasks 1, 9, 10, 12
interfaces.

**Files:**

- Create: `backends/rust/src/interact.rs`
- Modify: `backends/rust/src/lib.rs` (dispatch `Package.interfaces` /
  `Package.services` into the token stream — replacing task 6's pass-through;
  the C header gains one trailing comment line listing interfaces/services as
  not represented in the C ABI)
- Test: per-kind snapshots in `backends/rust/src/tests.rs` + the existing
  rustc-compile harness over the Appendix A corpus package

**Interfaces:**

- Consumes: IR v2 interfaces/services (tasks 6, 8), resolved `Timing` (task 9),
  `ReturnType`/`FallibleType` (task 10), observer stubs (task 12),
  `fallible_transport_identity` (task 1).
- Produces (consumed by task 22): `generate` keeps its E1 signature —
  `pub fn generate(pkg: &ridl_ir::v2::Package) -> Result<Generated,
  GenerateError>`
  — with `rust_source` now covering interactions. The Rust interaction
  vocabulary, emitted once per generated module, dependency-free (the
  rustc-compile harness builds with no external crates):

```rust
pub enum Provenance { Init, Live, Invalid }
pub trait SignalHandle<T> {
    fn read(&self) -> (T, Provenance);
    fn subscribe(&mut self, f: Box<dyn FnMut(&T, Provenance)>);
}
pub trait EventHandle<T> {
    fn subscribe(&mut self, f: Box<dyn FnMut(&T)>);
}
pub trait RidlStream {
    type Item;
    fn poll_next(self: core::pin::Pin<&mut Self>,
                 cx: &mut core::task::Context<'_>)
        -> core::task::Poll<Option<Self::Item>>;
}
pub enum TimingMode { StrictPeriodic, Range }
pub struct TimingConst { pub mode: TimingMode, pub min_us: Option<u64>,
    pub max_us: Option<u64>, pub default_applied: bool }
pub enum ContractKind { Require, Ensure }
pub struct ContractStub { pub id: &'static str, pub kind: ContractKind,
    pub source: &'static str, pub signals: &'static [&'static str],
    pub params: &'static [&'static str] }
```

- Per interface `X` (names converted camelCase → snake_case; each item's doc
  comment records the source name and ordinal): a consumer trait — signals as
  `fn current_speed(&mut self) -> &mut dyn SignalHandle<Speed>`, events as
  `&mut dyn EventHandle<DoorPayload>`, finals as plain accessors
  `fn software_version(&self) -> &Version` (ridl §8), commands as
  `async fn set_gear(&self, position: GearPosition)` (the ack is
  runtime-internal — the doc comment uses the §6.1 wording), queries as
  `async fn get_average_speed(&self, window: Duration) -> Speed`, fallible
  queries as
  `async fn calibrate(&self, target: Axle)
  -> Result<CalReport, CalError>`
  (native `Result`, exhaustive by construction), stream returns as
  `fn stream_faults(&self, filter: DiagFilter)
  -> impl RidlStream<Item = FaultEvent>`
  (stream params take `impl RidlStream<Item = T>`); and a provider trait —
  `fn publish_current_speed(&mut self, value: Speed)`,
  `fn raise_door_opened(&mut self, occurrence: DoorPayload)`,
  `async fn on_set_gear(&mut self, position: GearPosition)`, and the query
  handlers. Transport-failure doc comments use the gf §6.4 sentence verbatim:
  `infrastructure failure — detected, undeclared`.
- Timing metadata:
  `pub const VEHICLE_STATUS_TIMING: &[(&str,
  TimingConst)] = &[("currentSpeed", TimingConst { … }), …];`
  (u64 microseconds — durations are integral, exactness preserved).
- Contracts: `pub const VEHICLE_STATUS_CONTRACTS: &[ContractStub] = …;` from the
  task 12 observer stubs.
- Services:
  `pub const SERVICES: &[(&str, &str)] =
  &[("veh.adas.cruise", "CruiseControl"), …];`
  — an inline shape names its generated anonymous interface
  `ServiceVehHvacCabin` (CamelCase of the dotted name).
- Fallible transport identity: a doc comment
  `/// transport identity: VehicleStatus#9:FaultPage|DiagError` on the query
  method, from `fallible_transport_identity`.

**Steps:**

- [ ] Failing snapshots first, one per kind: signal (init + provenance), event,
      command with require doc, fallible query with identity doc, tuple-return
      query, bidirectional stream query, final with array, services both forms,
      the timing and contract consts.
- [ ] Implement `interact.rs` with quote/prettyplease like the rest of the
      backend; async trait methods and RPITIT compile on edition 2024 without
      external crates.
- [ ] The Appendix A corpus package generates and passes the existing
      rustc-compile test
      (`rustc --edition 2024 --crate-type lib
      --emit metadata`).
- [ ] Full local gate; commit.

**Done when:** generated Rust for the Appendix A `.ridl` package compiles via
the existing rustc-compile harness — with tasks 13–14 this closes the E2 exit
criterion "compile to Rust and a second backend from one IR".

---

### Task 16: E2.8a — `ridl diff`: IR-snapshot compare engine + CLI

Branch `e2-8a-diff-engine` · commits `feat(tools): …` / `feat(ridl): …` · model:
opus.

**Read first:** concept note §9.1 (exit-code contract); ridl reference §11;
ADR-0008 decision 9 (placement — facade/tools, never ridlc); the IR v1 proto
header comments on exactness/canonical refs (they exist for this tool).

**Files:**

- Create: `tools/diff/Cargo.toml` (crate `ridl-diff`), `tools/diff/src/lib.rs`,
  `tools/diff/src/walk.rs`
- Modify: `crates/ridl/src/main.rs` (`ridl diff` subcommand),
  `crates/ridl/Cargo.toml` (dep on `ridl-diff`), `.git-std.toml` (nothing —
  scope `tools` exists)
- Test: engine unit tests over constructed v2 packages; CLI integration tests
  via `CARGO_BIN_EXE_ridl`

**Interfaces:**

- Produces (consumed by tasks 17, 18):

```rust
pub enum Verdict { Identical, Compatible, Breaking }
pub enum Category {           // task 17 completes the classification —
    DeclAdded, DeclRemoved,   // this task emits structural categories
    InteractionAppended, InteractionInserted, InteractionReordered,
    InteractionRemoved, InteractionRetired,      // removed + tombstone
    KindChanged, PayloadChanged, ReturnChanged, ParamsChanged,
    TimingChanged, ContractChanged, WidthChanged, ConstraintChanged,
    InitChanged, ReservedNameRedeclared, ServiceChanged, DocOnly,
}
pub struct Change {
    pub path: String,     // "veh.cluster/VehicleStatus/doorOpened"
    pub category: Category,
    pub verdict: Verdict,
    pub before: Option<String>,   // rendered old value
    pub after: Option<String>,
}
pub struct DiffReport { pub changes: Vec<Change>, pub verdict: Verdict }
pub fn diff_packages(old: &ridl_ir::v2::Package,
                     new: &ridl_ir::v2::Package) -> DiffReport
pub fn diff_sets(old: &[ridl_ir::v2::Package],
                 new: &[ridl_ir::v2::Package]) -> DiffReport
pub fn load_ir_json(path: &Path)
    -> Result<ridl_ir::v2::Package, LoadError>
pub fn render_text(report: &DiffReport) -> String
pub fn render_json(report: &DiffReport) -> String   // stable schema:
    // { "verdict": "breaking", "changes": [{ "path", "category",
    //   "verdict", "before", "after" }] }
```

- The walk matches decls/interactions by **name** within their container, then
  compares ordinals (a matched name with a different ordinal is
  `InteractionReordered`/`InteractionInserted` — inserted when every later
  ordinal shifted by the same amount, reordered otherwise), payloads, timings,
  returns, params, contracts, widths, constraints, inits — producing one
  `Change` per difference with an honest path. Doc/label changes are `DocOnly`.
  Comparison is over **resolved** IR — the engine never reads source.
- Provisional verdicts in this task (task 17 replaces them): `DocOnly` +
  `InteractionAppended` + `DeclAdded` + `InteractionRetired` → `Compatible`;
  everything else → `Breaking`. `DiffReport.verdict` is the max over changes
  (Breaking > Compatible > Identical).
- CLI: `ridl diff <OLD> <NEW> [--format text|json]` where each of OLD/NEW is an
  `.ir.json` file, a `.typl`/`.ridl` file, a package directory, or a workspace
  root (sources are compiled in-process through `ridlc::compile_workspace`;
  compile errors in either side → exit 2 with rendered diagnostics). Exit codes
  (concept §9.1, decision 9): **0** compatible or identical, **1** breaking,
  **2** error (I/O, compile, usage). `ridlc` is untouched — the qualification
  boundary holds.

**Steps:**

- [ ] Failing engine tests: identical → `Identical`; a doc edit → `DocOnly`/0;
      an append → `InteractionAppended`/0; a payload type change →
      `PayloadChanged`/1; a remove-without-tombstone → `InteractionRemoved`/1; a
      remove-with-tombstone → `InteractionRetired`/0; package added/removed at
      set level.
- [ ] Failing CLI tests: ir.json vs ir.json; source-dir vs source-dir; broken
      source → exit 2; `--format json` output parses and matches the schema
      above.
- [ ] Implement; full local gate; commit.

**Done when:** `ridl diff` compares two snapshots or source trees with the 0/1/2
contract and machine-readable output; every category surfaces with an honest
path.

---

### Task 17: E2.8b — the breaking/compatible classifier

Branch `e2-8b-diff-classifier` · commits `feat(tools): …` · model: fable.

**Read first:** ridl reference §11 (evolution), §9.1 (defaults are contract
changes); typl §7.4; the ADR-0008 backbone evolution table; ADR-0008 decisions
4, 12, 14; task 16's `Category`/`Change`.

**Files:**

- Create: `tools/diff/src/classify.rs`, `tools/diff/test_data/gate/base/` +
  `…/breaking/` + `…/compatible/` (three variants of one small source package:
  `breaking` reorders two interactions of `base`; `compatible` appends one),
  `crates/ridl/tests/diff_gate.rs`
- Modify: `tools/diff/src/lib.rs` (verdicts flow through the classifier)
- Test: one test per rule row below + the gating integration test

**Interfaces:**

- Consumes: task 16's engine and task 8's service lowering (the service rows
  below). Produces: the classification table — the normative center of E2.8,
  fixed by ADR-0008 decision 14, recorded here and in the tool's `--explain`
  output:

```rust
pub fn classify(change: &Change, old: &ridl_ir::v2::Package,
                new: &ridl_ir::v2::Package) -> Verdict
```

- **Breaking (exit 1):** insert-not-at-end; reorder; remove without tombstone;
  redeclare under a reserved name (`ReservedNameRedeclared`); interaction kind
  change (any direction — wire identity changes); payload/param/return type
  change; stream added/removed on a param/return; wire-width flip (any
  `IntWidth`/`FloatWidth` change, uint64 vs int64 included); constraint narrowed
  (min raised, max lowered, step added/coarsened, length tightened, pattern
  changed); timing — **min lowered** (rate floor drops: consumers sized for the
  old floor), **max raised** (staleness bound loosens), a bound added where none
  was, a bound removed, or the mode flipped (strict-periodic ↔ range — rmdl
  clocks key on strict); fallible/result **error-arm type change** or
  **error-arm added** (consumers face an unknown failure); ok-arm change; inline
  `T | E` transport-identity change (decision 4 — any arm or ordinal change
  flips the identity); require **added or strengthened** (callers that were
  legal become rejected: any require text change is breaking — the classifier
  does not prove implication); ensure **removed or weakened** (any ensure text
  change is breaking, same reason); service removed, service interface_ref
  changed; interface removed.
- **Compatible (exit 0):** append interaction at end; retire via `reserved`
  tombstone (same ordinal slot); append a new decl, interface, or service;
  constraint widened (min lowered, max raised, step removed); timing — min
  raised, max lowered (both strengthen the consumer-visible guarantee) with mode
  unchanged; require removed; ensure added; `default_applied` flipped with
  **identical resolved bounds** (a default made explicit); doc/label-only
  changes; enum value appended; union arm appended **at the end** of a
  non-result union (result/fallible arms are the breaking rule above).
- The `[defaults].timing` rule needs no special case: the engine compares
  resolved bounds (decision 12), so editing the default surfaces as
  `TimingChanged` on every defaulted interaction and classifies by the bound
  rules — assert this end to end.
- `ridl diff --explain <category>` prints the rule row for a category (the table
  above, as text) — the CI-facing documentation of record until E4's error
  index.

**Steps:**

- [ ] Failing tests: one per rule above (both directions of every directional
      rule — e.g. min raised compatible / min lowered breaking), plus the
      `[defaults].timing` end-to-end case (two source trees differing only in
      `ridl.toml`), plus a transport-identity case (error arm swapped for
      another error type → breaking with the identity named in
      `before`/`after`).
- [ ] Implement `classify`; replace task 16's provisional verdicts; wire
      `--explain`.
- [ ] Failing gating test (`crates/ridl/tests/diff_gate.rs`, via
      `CARGO_BIN_EXE_ridl` — the task 21 `ridl test` precedent):
      `ridl diff
      test_data/gate/base test_data/gate/breaking` exits **1**,
      `ridl diff
      test_data/gate/base test_data/gate/compatible` exits
      **0**. The test runs inside `cargo test --workspace`, so the local merge
      gate itself exercises the CI gating contract (ADR-0008 decision 11).
- [ ] Full local gate; commit.

**Done when:** every row of the table is enforced by a test; the roadmap exit
line — breaking vs compatible classified correctly — holds on the corpus
scenarios; and the gating integration test proves the "`ridl diff` gates
breaking changes in CI" exit line inside the local gate.

---

### Task 18: E2.9 — baseline-aware desk check

Branch `e2-9-baseline` · commits `feat(ridl): …` · model: opus.

**Read first:** gf §6.3 (the decided mitigation — desk-time reorder/insertion
flagging); ADR-0008 decision 9 (the qualification boundary keeps this out of
`ridlc`) and decision 14 (the `.ridl/baseline/` storage); tasks 16–17
interfaces.

**Files:**

- Modify: `crates/ridl/src/main.rs` (`--baseline` on `ridl check`, new
  `ridl baseline` subcommand)
- Test: CLI integration tests via `CARGO_BIN_EXE_ridl`

**Interfaces:**

- Produces:
  - `ridl baseline [PATH] [--out <DIR>]` — compiles the workspace and writes one
    `<pkg-name>.ir.json` per package into `<DIR>` (default `.ridl/baseline/` at
    the workspace root). This is the published snapshot a desk compares against;
    registry-published baselines are E7 territory.
  - `ridl check [PATH] [--baseline <DIR|FILE>]` — after a clean compile, loads
    the baseline snapshots (auto-discovering `.ridl/baseline/` when the flag is
    absent and the directory exists; silently skipped when neither is present),
    runs the task 16/17 engine per matching package, and renders every
    **ordinal-affecting** breaking change (`InteractionInserted`,
    `InteractionReordered`, `InteractionRemoved`, `ReservedNameRedeclared`) as a
    coded **RIDL-407** warning
    (`interaction ordinal changed against the baseline: …`, with the diff path
    in the message and the interaction's declaration as the span). Non-ordinal
    categories stay `ridl diff`'s job in CI — the desk check is the gf §6.3
    mitigation, not a second diff gate. Warnings do not change the exit code
    (`ridl check` keeps its 0/1/2 semantics).
- Implementation sits wholly in the facade over `ridl-diff` and
  `ridlc::run_check` — `ridlc` itself is untouched (decision 9).

**Steps:**

- [ ] Failing CLI tests: `ridl baseline` writes the snapshots; a reorder against
      the baseline draws RIDL-407 with the moved interaction named; an append
      draws nothing; no baseline present → no new output; `--baseline` pointing
      at a single file works.
- [ ] Implement; full local gate; commit.

**Done when:** a reorder is flagged at the desk before CI — the gf §6.3
"unprotected only with no baseline at all" line holds.

---

### Task 19: E2.10a — the ridl lint pass

Branch `e2-10a-ridl-lint` · commits `feat(ridl-sem): …` · model: opus.

**Read first:** ridl reference §7.2, §16.4 (RIDL-404/405/406), §3.1 (envelope
rule); gf §6.1 (canonical-form lint → RIDL-308).

**Files:**

- Create: `crates/ridl-sem/src/lint.rs`
- Modify: `crates/ridl-sem/src/check.rs` (run the pass),
  `crates/ridl-sem/
  src/lib.rs`
- Test: lint tests, one per code

**Interfaces:**

- Produces (surfaced automatically in CLI and LSP through the diagnostic
  pipeline): four lints, emitted from `check_package` after lowering —
  - **RIDL-404** (warning): a query whose name matches
    `^(set|reset|clear|apply|write|update)[A-Z]` — probably a command (ridl
    §7.2).
  - **RIDL-405** (info): one error type appearing as the error arm (inline or
    named result union) of queries in **three or more distinct interfaces**
    across the package set — the "shared across unrelated failure domains"
    heuristic, threshold recorded here.
  - **RIDL-406** (info): a signal/event payload struct carrying a field named
    one of `timestamp`, `time`, `seq`, `seqNo`, `sequence`, `sequenceNumber`,
    `frameCounter`, `frameNo` — envelope duplication (ridl §3.1); the message
    quotes the §3.1 domain-time exception so the legitimate case knows to keep
    its field and ignore the info.
  - **RIDL-308** (warning): a named result union (`UnionDef.is_result`) in query
    return position — `inline \`T | E\` is canonical` (gf §6.1, the new code
    from the Global Constraints allocation).
- Lints are ordinary coded diagnostics (severity warning/info) — no separate
  lint driver, no configuration surface in E2.
- Traceability note: the E2.10 row's "alias-not-required" lint needs no new work
  — TYPL-008 (import alias without a collision, warning) has shipped from the
  resolver since E1; recorded here so the story reads fully covered.

**Steps:**

- [ ] Failing tests: `query setGear(...)` → RIDL-404; a `DiagError` arm used by
      queries in three interfaces → RIDL-405 (two interfaces → silent); a
      `FaultEvent`-style payload with `timestamp` on a **query stream return**
      stays silent but the same struct as a signal payload draws RIDL-406;
      `query getFaultPage(...):
      FaultPageResult` → RIDL-308 while the
      inline spelling is silent.
- [ ] Implement; full local gate; commit.

**Done when:** all four lints fire on a real interface (the roadmap exit line)
and are silent on the idiomatic corpus.

---

### Task 20: E2.10b — LSP + editor support for ridl

Branch `e2-10b-ridl-lsp` · commits `feat(ridl-lsp): …` / `feat(editors): …` ·
model: opus.

**Read first:** gf §6.2 (hover expands the per-kind reading), §6.3 (ordinal
inlays), §6.4 (Stratum-3 wording); ridl reference §9.2, §14; the E1
`hover.rs`/`inlay.rs`/`complete.rs` structure.

**Files:**

- Modify: `crates/ridl-lsp/src/hover.rs`, `crates/ridl-lsp/src/inlay.rs`,
  `crates/ridl-lsp/src/complete.rs`, `crates/ridl-lsp/src/nav.rs` (interaction
  symbols), `editors/vscode/package.json` + `editors/vscode/src/extension.ts`
  (language id `ridl` for `.ridl` files)
- Create: `editors/vscode/syntaxes/ridl.tmLanguage.json` (the ridl grammar —
  includes the typl patterns by reference, adds the nine ridl keywords, `@`
  timing, durations, and `T | E`)
- Test: LSP integration tests over a fixture `.ridl` workspace

**Interfaces:**

- Hover on an interaction: kind, payload type (unit/range via the E1 hover
  data), ordinal, resolved timing with mode + bounds + the derived per-kind
  reading per gf §6.2 (signal:
  `min = rate floor (debounce),
  max = staleness bound (refresh ceiling)`;
  event: `… (throttle)` / `… (TTL: stale occurrences discarded)`), and
  `default [100ms..1000ms]
  applied` when `default_applied`. Hover on a
  fallible return: both arms plus the strata note ending with the gf §6.4
  sentence verbatim: `infrastructure failure — detected, undeclared`. Hover on a
  service: its interface and the posture-neutral note (ridl §14.5).
- Inlay hints: interaction ordinals (`#1`…) beside every interaction and
  reserved tombstone inside interface bodies and service inline shapes — the gf
  §6.3 mitigation's editor half (the E1 struct-field inlays already exist; this
  extends the provider to `InterfaceMember`).
- Goto-definition/find-references work on payload type references inside
  interactions and on `interface_ref` in services (extends `symbol_at`).
- Completion: after `:` in signal/event/final/param/return position → visible
  named types (the E1 context machinery); at interaction-start position inside
  an interface body → the five kind keywords + `reserved`.
- VS Code: `.ridl` files get language id `ridl`, the LSP client attaches, and
  the TextMate grammar covers the nine ridl keywords, `@` timing, durations, and
  `T | E` in return position.

**Steps:**

- [ ] Failing integration tests: hover text for `currentSpeed` (strict periodic)
      and for a defaulted event (default note present); hover for a fallible
      query names both arms + the §6.4 sentence; ordinal inlays on the Appendix
      A interface render `#1`–`#11` with the reserved slot at `#6`; goto-def
      from a payload ref crosses packages; completion inside an interface body
      offers `signal`.
- [ ] Implement; rebuild the vsix locally and record the manual check in the PR
      description (E1 task 26 precedent).
- [ ] Full local gate; commit.

**Done when:** a `.ridl` interface edits live in VS Code with coded diagnostics,
ridl hovers, ordinal inlays, and completion.

---

### Task 21: E2.11a — `ridl test`: subset evaluator + property runs

Branch `e2-11a-ridl-test` · commits `feat(ridl-sem): …` / `feat(ridl): …` ·
model: opus.

**Read first:** ridl reference §13 (one assertion, four executions — this is
execution "property test in CI"); the task 7 spec (evaluation domains); E1
`testgen.rs` (strategies, boundary/violation corpora); roadmap E2.11.

**Files:**

- Create: `crates/ridl-sem/src/expr_eval.rs`
- Modify: `crates/ridl/src/main.rs` (`ridl test` subcommand),
  `crates/ridl-sem/src/lib.rs`, `justfile` (recipe `ridl-test` folded into
  `just test` docs)
- Test: evaluator unit tests; CLI integration test over the corpus

**Interfaces:**

- Produces (consumed by task 22 and the E5 oracle later):

```rust
// ridl-sem/src/expr_eval.rs — total evaluation of the guaranteed subset
pub enum Value {
    Bool(bool),
    Num(ExactValue),          // exact rational — no floats (task 7 spec)
    Dur(ExactValue),          // exact microseconds
    EnumVal(String, i64),     // canonical enum ref + value
    Tuple(Vec<(String, Value)>),
}
pub struct EvalEnv<'a> {
    pub params: &'a [(String, Value)],
    pub result: Option<Value>,
    pub consts: &'a dyn Fn(&str) -> Option<Value>,
}
pub enum EvalError { UnboundRef(String), TypeMismatch(String),
                     DivisionByZero }
pub fn eval_expr(expr: &ast::Expr, env: &EvalEnv)
    -> Result<Value, EvalError>
```

- `ridl test [PATH] [--samples N] [--format text|json]` (default N = 256, seeded
  deterministically per contract so runs are reproducible): compiles the
  workspace, then per package —
  1. **Range self-corpora:** for every constrained named type, the E1 `testgen`
     boundary and violation corpora run against the constraint validator — every
     boundary value must be accepted, every violation value rejected (a failure
     here is a checker/testgen bug surfacing).
  2. **Require satisfiability sampling:** for every `require` whose references
     are all generatable (params with ranged types; clauses reading signals are
     reported `skipped: reads live state — observer
     territory (E5)`), draw
     N param tuples from the `testgen` strategies, evaluate via `eval_expr`, and
     report the satisfied count. Zero satisfied out of N → reported as
     `suspect: no sampled
     input satisfies this precondition` (test-plane
     finding, not a compile diagnostic — no code burned).
  3. `ensure` clauses are listed as observer stubs only (nothing to evaluate
     without a provider — E5's oracle executes them).
- Exit codes: 0 all runs pass; 1 any range self-corpus failure or evaluation
  error; 2 compile/I/O error. `--format json` emits
  `{ package, contracts: [{ id, status, satisfied, samples }],
  ranges: [{ type, status }] }`.
- CI wiring (decision 11 — local gate): a `ridlc` integration test runs
  `ridl test` over the corpus workspace and asserts exit 0, so
  `cargo test --workspace` carries the property runs.

**Steps:**

- [ ] Failing evaluator tests: each operator over exact values
      (`0.1 + 0.2 == 0.3` holds — rationals), duration compare, enum compare,
      tuple member, short-circuit `&&`/`||`, division by zero.
- [ ] Failing CLI tests: the corpus runs green; a rigged package with
      `require speed > 300.0` over `Speed [0.0..250.0]` reports the suspect
      finding; `--format json` parses.
- [ ] Implement; full local gate; commit.

**Done when:** range-derived corpora run as tests under `ridl test` and in the
local gate — the roadmap exit line.

---

### Task 22: E2.11b — the ridl corpus + diagnostic showcase

Branch `e2-11b-ridl-corpus` · commits `test(ridlc): …` · model: sonnet.

**Read first:** E1 task 18's corpus layout (`crates/ridlc/tests/corpus/`, the
insta::glob runner); ridl reference Appendix A; every RIDL code implemented in
tasks 2–19.

**Files:**

- Create: `crates/ridlc/tests/corpus/veh-cluster/` (the ridl Appendix A package
  verbatim — `ridl.toml`, the typl vocabulary file, the `.ridl` contract file
  plus authored service declarations after the §14.5 model (Appendix A has none)
  — plus the `veh.common` import target extended with the types Appendix A
  imports), `crates/ridlc/tests/corpus/ridl-diag-showcase/` (a package whose
  compile emits one instance of every implemented RIDL- and new FORM-/ MANI-
  code: RIDL-100 through 110, 140/141, 201/202, 301 through 308, 401 through
  407, FORM-106/107/108, MANI-009, plus the debt-folded profile codes
  TYPL-301/303/304, each exercised from a `.typl` file in the package — the
  RIDL-140 pair and the RIDL-407 baseline case live in dedicated sub-scenarios
  since they need a second package / a baseline directory),
  `crates/ridlc/tests/corpus/services-workspace/` (two members, a shared
  interface, a cross-package service)
- Modify: `crates/ridlc/tests/corpus.rs` (the runner also snapshots generated
  TypeScript beside Rust/IR JSON for `.ridl`-bearing packages)
- Test: this task **is** tests

**Interfaces:**

- Consumes: everything. Produces: the diag-showcase snapshot as the de-facto
  RIDL diagnostic index (the E1 precedent, until E4.2), and the golden corpus
  every later epic regresses against.

**Steps:**

- [ ] Build the three corpus packages; extend the runner; review every snapshot
      (diagnostics render + IR v2 JSON + generated Rust + generated TS per
      entry).
- [ ] Cross-check: every code listed above appears exactly where intended (grep
      the snapshot); Appendix A compiles clean end to end.
- [ ] Full local gate; commit.

**Done when:** the corpus runs green under `insta::glob`; every E2-implemented
diagnostic has a living example; Appendix A is the golden `.ridl` package.

---

## Close-out (after task 22)

- Whole-epic review on the most capable model; Critical/Important → one fix-wave
  PR; Minor → consolidated debt issue.
- Gardening PR (sdd-gardening): archive this plan to `docs/archive/`; update the
  walking-skeleton technote to the E2 as-built map; sync AGENTS.md (crate map:
  `backends/typescript`, `tools/diff`; commands: `ridl diff`, `ridl baseline`,
  `ridl test`), README, CONTRIBUTING.
- **Doc sync per ADR-0008 decision 1:** the ridl reference absorbs the four gf
  §6 supersessions it has not absorbed — Appendix C gains `fallible_type` and
  the single attr_block production, §7/§10.1 adopt the inline `T | E` canonical
  form (+ RIDL-308), §9 restates generic min/max with the per-kind table as a
  derivation, §10.3/glossary adopt "infrastructure failure — detected,
  undeclared", §14.5 gains the authored `service_def` grammar, §16 gains the new
  codes (RIDL-308/407, FORM-106/107/108, MANI-009). The gf §7 errata list is the
  checklist.
- Roadmap E2 rows checked off; ADR-0008 status flipped to Accepted with any
  mid-epic amendments recorded.

## Out of scope (deferred, recorded)

- `persist` — attribute, semantics, and diff category (ADR-0008 decision 3; gf
  open question 3).
- gf §4.7 promotion of `labels`/`deprecated` from doc tags to attributes (not
  roadmap-cited; the attr_block grammar is ready for it).
- `final` → `fixed`/`provisioned` rename (decision 5 — frozen for E2, the gf
  §6.5 reopening stays open).
- `service.member` cross-references, providing/requiring, posture realization —
  E6 (rsdl).
- Selective broadcasts, signal groups, actions sugar, reflection service,
  mid-stream abort policy — ridl §17 open questions, untouched.
- Registry-published baselines and diff-against-registry — E7.4.
- Structured expr trees in IR (`Contract.source` is canonical text until E5.1
  restructures it).
- uxdl `fetch`/`fixed` fallible surface — E3 (profile-maps onto this epic's
  core).
