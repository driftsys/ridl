# Epic 9, stories E9.1 to E9.6 — execution plan

| Field   | Value                                                                                                                 |
| ------- | --------------------------------------------------------------------------------------------------------------------- |
| Status  | working memory — archived to `docs/archive/` when the block closes                                                    |
| Date    | 2026-08-04                                                                                                            |
| Scope   | E9.1 to E9.6 only. E9.7 to E9.12 are not in this block                                                                |
| Records | [ADR-0014](../decisions/ADR-0014-ir-encodings.md), [ADR-0015](../decisions/ADR-0015-qos-absorption-and-rpc-bounds.md) |

This is the first six stories of Epic 9, not the whole epic. The block ends at
E9.6 because E9.6 is the last story that alters the IR, and `docs/ROADMAP.md`
records that E9 and E3 must not run concurrently on the IR. Stopping here leaves
the schema projections (E9.7 to E9.11) and the recorded grammar drift (E9.12)
for a later block, with the IR settled and no half-applied change in the tree.

## Order, and why

One story at a time, in roadmap order. Sequential rather than parallel: E9.4 and
E9.6 both edit `ir.proto` and `classify.rs`, E9.1 rewrites every IR golden that
E9.4 and E9.6 then extend, and E9.2 and E9.3 both edit the emit surface in
`crates/ridlc/src/lib.rs`. Parallel branches would each rebase onto a rewritten
golden set.

```text
E9.1  ──▶  E9.2  ──▶  E9.3  ──▶  E9.4  ──▶  E9.5  ──▶  E9.6
encodings  emits     filter    RPC bounds  prose     composition
└──────── ADR-0014 ────────┘   └────────── ADR-0015 ──────────┘
```

| Story | Depends on | Why                                                                            |
| ----- | ---------- | ------------------------------------------------------------------------------ |
| E9.1  | —          | the descriptor pool and the six-function surface everything after it writes to |
| E9.2  | E9.1       | prototext needs the descriptor pool E9.1 builds                                |
| E9.3  | E9.2       | the predicate is only demonstrable once more than one IR encoding exists       |
| E9.4  | —          | independent of the encodings, but sequenced after them to rebase once          |
| E9.5  | —          | documentation only; sequenced beside the decision it states                    |
| E9.6  | E9.4       | both edit `ir.proto` and `classify.rs`; E9.4 is the smaller of the two         |

**E9.2 and E9.3 are split at the seam the design note draws, not at the
defect.** E9.2 adds the two emits **and** puts them on the correct side of the
existing `ridl.std` filter, so `main` never carries a build that writes a
spurious `ridl.std.ir.binpb`. E9.3 then replaces the enumeration with the
exhaustive classification ADR-0014 decision 10 requires, so the next encoding
cannot reintroduce the defect. Landing E9.2 with a known-wrong filter and fixing
it in the next pull request was rejected: a defect that is understood before it
is written should not be committed.

## The stories

Each row is one branch, one pull request, one review. "Verify" is what must be
green before the pull request is opened, over and above `just verify`.

### E9.1 — `ridl-ir` serialization rewrite (L)

Canonical protobuf JSON replaces the `serde` rendering everywhere. Add
`prost-reflect` with `serde` and `text-format`; write the `FileDescriptorSet`
from `build.rs` to `OUT_DIR` and hold a `LazyLock<DescriptorPool>` over it;
remove the `type_attribute` `serde` derives; move `ridl-diff` and the
`ridl-backend-rust` test loader from `serde_json` to `v2::from_json`; regenerate
every IR golden.

**Before writing the rewrite**, answer ADR-0014's two open items with one test
against the library: how `skip_default_fields(false)` renders an unset proto3
`optional`, and whether the `.boxed()` oneof member needs special handling.
Record both answers in the pull request body — they shape every golden in the
tree.

_Verify:_ round-trip tests for JSON and binary; the strict-parse conformance
test of ADR-0014 decision 11; the regenerated goldens reviewed for one semantic
change only, which is the dialect.

### E9.2 — prototext and binary emits (M)

`ir-text` writing `<base>.ir.txtpb` and `ir-binary` writing `<base>.ir.binpb`,
alongside `ir-json`. Both join the one `ridl.std` filter — the closure building
`code_emits` in `run_build`.

_Verify:_ all three encodings round-trip to the same IR; no `ridl.std` artifact
appears for any of the three; a baseline or a diff pointed at an `.ir.txtpb` or
an `.ir.binpb` is refused, since baselines stay `.ir.json` (ADR-0014 decision
5).

### E9.3 — the emit-filter predicate (S)

Replace the `matches!(emit, Emit::IrJson)` enumeration with an exhaustive
classification over `Emit` — no wildcard arm — so an unclassified new encoding
is a compile error rather than a spurious artifact.

_Verify:_ a test that asserts the classification for every `Emit` variant; the
`clippy::wildcard_enum_match_arm` deny applies at the new site.

### E9.4 — RPC bounds and the response bound (M)

ADR-0015 decisions 2 to 8. Checker: admit the range form on `command` and
`query`, reject strict periodic. Diagnostics: mint RIDL-112, narrow RIDL-106,
widen RIDL-103. IR: `CommandDef.timing = 3`, `QueryDef.timing = 4`. Diff:
`Category::RpcBoundChanged` with the inverted `min` direction. Backends: the
same two microsecond constants they already emit, on two more kinds.
Specification: ridl §9, §9.2, §16.1, Appendix B, Appendix F, and §17.5 open
question 5 — replaced by the absorption principle and its coverage table
(ADR-0015 decision 1).

_Verify:_ a showcase entry per new or moved code; a corpus package declaring
both bounds on both kinds; diff cases for each row of the direction table,
including the two that invert.

### E9.5 — the coherence rule (S)

ADR-0015 decisions 9 and 10 as normative prose in ridl §14, beside the service
definition it keys on, plus the §17.3 open question 3 closure and the Appendix B
coherence rows. Documentation only.

The rule is stated as ADR-0015 decision 9 words it — production coherence, with
consumer observation conditional on the binding. The shorter form that promises
a consumer observes a simultaneous set unconditionally is what decision 10
withdraws, and it must not reach the reference.

_Verify:_ `just check`, `just book-check`, `just link-check`; no code change.

### E9.6 — multi-interface services (L)

ADR-0015 decisions 12 to 20. Grammar change to `ServiceDef` with `ServiceShape`;
regenerate the typed-AST layer with `cargo xtask codegen`; checker moves from
one reference to a list; RIDL-144, RIDL-145, RIDL-146 minted; RIDL-141 and
RIDL-143 become per-element; IR reserves field numbers 10 and 11 and takes fresh
ones; five `ServiceShape*` diff categories and `ServiceChanged` narrowed; both
backends and `ridl-diff`'s walk follow the list. Specification: ridl §11
(interface ids within a service), §14 (the shape list, flat addressing, the
composition rules), §16.4 (the three new codes beside RIDL-140 to RIDL-143, and
RIDL-141 and RIDL-143 becoming per-element), and §17.2 — answered by composition
rather than by the mixins recorded there.

_Verify:_ a service composing two interfaces compiles, generates, and
round-trips; reordering the shape list is invisible to transport identity and
breaking to diff; each new diagnostic has a showcase entry; the grammar drift
test passes.

## Working rules for this block

- **One worktree, one branch, one pull request per story.** The pull request is
  opened ready for review, never as a draft.
- **Every pull request is reviewed before merge.** Critical and Important
  findings are fixed on the branch and re-reviewed. Minor findings are recorded
  on one consolidated debt issue for the block rather than one issue each,
  following the E1 (#135) and E2 (#172) pattern.
- **Merge on a green gate.** `just verify` locally and the CI workflow on the
  pull request. CI was restored to working order on 2026-08-03; if a job fails
  to start again, the local gate is the fallback ADR-0006 decision 8 and
  ADR-0007 decision 16 already describe.
- **Decisions taken while the author is away are recorded in
  `docs/decisions/`**, either as a new record or as an amendment to ADR-0014 or
  ADR-0015, and named in the pull request body.
- **The worktree and branch are removed after the merge**, and local `main` is
  synced to `origin/main`.

## Out of scope, recorded so it is not rediscovered

- **E9.7 to E9.11** — the projection contract, proto3 and FlatBuffers
  projections, the schema hash, and store and dispatcher generation. They read
  the IR this block settles.
- **E9.12** — general form R5's postfix order contradicts the shipped grammar,
  and `InterfaceDef`/`ServiceDef` take no `AttrBlock`. ADR-0015 records the
  first half in its amend table; the second is recorded on the roadmap row and
  in the response-bound note §10. The fix is one grammar edit that also serves
  the deferred `labels`/`deprecated` promotion, and it is not needed by anything
  in this block.
- **Epic 10** — typl value objects. Backend-only, no IR dependency, threads
  independently.
- **The cross-language conformance test** — ADR-0014 open item 3 places it at
  E4.5, because it needs a non-Rust protobuf runtime in CI.
