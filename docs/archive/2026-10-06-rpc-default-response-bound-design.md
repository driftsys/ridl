# Default response bound for untimed commands and queries

Status: design note for driftsys/ridl#741, written 2026-10-06 against `main` at
7fe24437. Sebastien approved the design in conversation on 2026-10-06. Nothing
here is implemented. It graduates into an in-place amendment of ADR-0015
decisions 4 and 7 and of the ridl reference §9.1, §9.3 and §16, and is archived
with its plan when the story lands.

## 1. The problem

A `command` or `query` with no timing annotation has no response bound in the IR
today (ADR-0015 decision 4, "warned, never defaulted"). The checker emits
RIDL-112 (`missing-response-bound`, warn) and lowers `CommandDef.timing` or
`QueryDef.timing` as absent.

A runtime that enforces call deadlines reads them from the catalog only: every
timing bound comes from the catalog, so that the provider node and its clients
read the same number. With no bound in the catalog, an untimed call has no
deadline on either side. A lost outcome holds a client call slot until detach,
and a handler that never settles holds a provider permit for as long as the
caller's session lives. A runtime cannot fix this by choosing a number itself,
because the provider and the clients could then disagree.

A deployment that motivated this change intends a response bound of 1 s for
every untimed command and 3 s for every untimed query.

## 2. Decision summary

1. Every `command` and `query` resolves to a response bound. No call is
   unbounded after this change.
2. The bound comes from the first of these that is present:
   1. the member's own annotation;
   2. the package manifest's `[defaults] command_timing` (for a `command`) or
      `query_timing` (for a `query`);
   3. the workspace manifest's key of the same name;
   4. the built-in default: `[..1s]` for a `command`, `[..3s]` for a `query`.
3. RIDL-112 stays a warn-level lint and now means "this `command` or `query`
   takes the default response bound", the same two-step that RIDL-100 gives an
   untimed `signal` or `event`. A package sets `missing-response-bound = "deny"`
   to require an explicit bound on every call.
4. The ceiling and floor lints proposed in #741 are out of scope. They need a
   threshold syntax that the `[lints]` table does not have (ADR-0024), and are
   filed as a follow-up issue.

## 3. Resolution

`ridl-sem/src/timing.rs`, `resolve_timing`, for `InteractionKind::Command` and
`InteractionKind::Query`. `D` is the resolved default for the member's kind (§2,
item 2).

| Annotation       | Resolved `Timing`                                    | `default_applied` | Diagnostic |
| ---------------- | ---------------------------------------------------- | ----------------- | ---------- |
| none             | `D`                                                  | true              | RIDL-112   |
| `@[min..max]`    | as written                                           | false             | none new   |
| `@[..max]`       | as written; `min` stays absent                       | false             | none new   |
| `@[min..]`       | `min` as written, `max` from `D`                     | true              | RIDL-112   |
| malformed timing | `D` (the fallback signals and events already follow) | true              | as today   |

Rules behind the table:

- The default fills `max` only. A written `min` is kept. A default's `min` is
  used only when the member has no annotation at all. On an RPC, `min` is a call
  throttle that constrains the caller (ADR-0015 decision 3), so it applies to an
  annotated member only when the author writes it.
- `@[..max]` is complete for an RPC and is not touched by the default.
- RIDL-112 still fires on `@[min..]`, because ADR-0015 aims the warning at
  `max`, and here `max` came from the default.
- An annotation may set a bound above or below the default. Nothing compares the
  two.
- RIDL-101, RIDL-102, RIDL-103, RIDL-106 and RIDL-108 are unchanged.
- `signal`, `event` and `fixed` resolution is unchanged, including the half-open
  signal range that leaves its absent side unset.

## 4. Manifest

`crates/ridl-core/src/manifest.rs` and `crates/ridl-core/src/workspace.rs`.

- `[defaults]` gains two optional string keys, `command_timing` and
  `query_timing`, beside `timing`. They are added to `RawDefaults`, to the
  `Manifest` struct (as `default_command_timing` and `default_query_timing`, raw
  strings like `default_timing`), and to the `[defaults]` allowed-keys list used
  by MANI-005.
- The keys merge per key: a package's `command_timing` shadows the workspace's
  `command_timing`, independently of `timing` and `query_timing`. This is the
  same `or_else` merge `workspace.rs` applies to `timing`, repeated for each
  key, and is carried through `package.rs` like `default_timing`.
- An RPC default accepts `[..max]` and `[min..max]`. It rejects `[min..]`,
  because a default must bound the call. The other rejections of
  `parse_default_timing` (no brackets, no `..`, fractional, zero or negative
  bound, `min > max`) apply unchanged. This is a new parse function beside
  `parse_default_timing`, which keeps requiring both bounds for `timing`.
- An invalid value draws MANI-009 (error), with the message changed to name the
  key: ``invalid `[defaults].<key>` in the package manifest: <reason>``. The
  checker then uses the built-in default for that kind, the same fallback
  `timing` follows.
- `Checker.default_timing` becomes three resolved values: the signal and event
  default, the command default and the query default.

Example:

```toml
[defaults]
timing = "[100ms..1s]"      # signal and event
command_timing = "[..1s]"   # command
query_timing = "[..3s]"     # query
```

## 5. Contract consequences

### 5.1 ADR-0015, amended in place

- **Decision 4** changes from "warned, never defaulted" to "warned, and
  defaulted". The new rationale: the original objection was that no generic
  value is plausible and that an invented bound is a promise nobody made. A
  package default answers this, because it is a value the package declares, in
  its manifest, and both sides read it from the catalog. The built-in default is
  documented in the reference, so it is declared too, not invented. An author
  who wants every call bound explicitly sets `missing-response-bound = "deny"`.
- **Decision 7** drops "`default_applied` always false, since RPC bounds are
  never defaulted". `default_applied` on an RPC now means the same as on a
  signal: at least one bound came from a default.
- The ADR's `## Status` records the amendment and its date.

### 5.2 `ridl-diff`

`crates/ridl-diff/src/classify.rs`, `rpc_bound`:

- A `default_applied` flip over identical bounds becomes compatible: a default
  made explicit, or an explicit bound replaced by an equal default. This is the
  rule `timing` already gives a signal. The comment that calls a flip "erroneous
  IR" is removed.
- Every other `rpc_bound` rule is unchanged. Comparing a catalog built before
  this change (no bound) with one built after it (a defaulted bound) reports a
  bound added, which is breaking. That verdict is correct: a call that could not
  time out now can.
- Changing a package's `command_timing` or `query_timing` changes the resolved
  bound of every untimed call in that package, and `ridl diff` classifies it by
  the existing `rpc_bound` direction rules. The ridl reference already states
  that changing a configured default is a contract change. That statement now
  covers RPCs too.

### 5.3 Catalog hash

`crates/ridl-ir/src/catalog_hash.rs` hashes the `Timing` message, including
`default_applied`. Every catalog with an untimed `command` or `query` gets a new
hash. The provider and its clients must be rebuilt together; a runtime's
attach-time hash check enforces this. The commit message and the changelog state
it.

### 5.4 Runtime behaviour

`ridl-rt` code is unchanged. `Member::call_deadline()` already reads the
resolved `max`. The behaviour changes all the same: a call that previously
waited without limit now lapses after 1 s (command) or 3 s (query) unless its
package or its annotation says otherwise. The implementation checks
`ridl-loopback`'s tests, `ridl-rt-conformance` and `just demo` for a call that
legitimately takes longer than the new default.

### 5.5 Release

The change is breaking for the IR content, the catalog hash and runtime
behaviour, so its commit carries a `BREAKING CHANGE` footer.

## 6. Documents to update

- `docs/decisions/ADR-0015-qos-absorption-and-rpc-bounds.md` — decisions 4 and
  7, `## Status` (§5.1).
- `docs/specification/ridl-language-reference.md` — §9.1 (the "never defaulted"
  paragraph and the resolution precedence), §9.3, the §16 table row for
  RIDL-112, the glossary entry.
- `docs/book/lints.md` — the RIDL-112 description (checked by
  `crates/ridl/tests/book_lints.rs`).
- `docs/book/getting-started.md` — the `[defaults]` example.
- `crates/ridl-core/src/diag.rs` — RIDL-112 and MANI-009 messages.
- `crates/ridl-rt/src/contract.rs` — the `call_deadline` rustdoc text only.
- `crates/ridl-diff/src/classify.rs` and `crates/ridl-ir/src/rules.rs` — the
  comments that state the old invariant.

## 7. Testing

- `timing.rs` unit tests: one per row of the §3 table, for `command` and for
  `query`; a default with a `min` applied to an unannotated member; a default's
  `min` not applied to `@[..max]`.
- RPC default parsing: `[..1s]` and `[10ms..1s]` accepted; `[10ms..]` and each
  existing rejection reason refused.
- `manifest.rs`: the two keys parse; an unknown `[defaults]` key still draws
  MANI-005.
- `workspace.rs`: per-key precedence (package shadows workspace for each key,
  independently).
- `check.rs`: extend `rpc_bounds_lower_into_the_ir_and_absent_stays_absent` so
  an unannotated member lowers with the built-in default and
  `default_applied = true`; MANI-009 tests for each new key.
- Generated descriptors (`ridl-backend-rust`) and `ridl-descriptor` output show
  the defaulted bound.
- `ridl-diff`: a flip over identical bounds is compatible; old catalog (absent)
  against new catalog (defaulted) is breaking.
- `crates/ridl/tests/lints.rs`: `missing-response-bound = "deny"` turns an
  untimed call into an error.
- Corpus snapshots and `golden_hash.rs` re-blessed; the hash change is named in
  the commit message.

## 8. Alternatives considered

| Alternative                                                                                 | Why it was not chosen                                                                                                                                                                   |
| ------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| No built-in default; an untimed call with no default is a hard error (the issue's proposal) | Sebastien chose a built-in default instead: a call always resolves, like a signal or an event, and requiring explicit bounds stays available through `missing-response-bound = "deny"`. |
| Make RIDL-112 default to `deny`                                                             | ADR-0024 states that no lint defaults to `deny`. With a built-in default nothing is unbounded, so an error by default is not needed.                                                    |
| Retire RIDL-112                                                                             | Removes the only way to require an explicit bound on every call.                                                                                                                        |
| One `rpc_timing` key for commands and queries                                               | The intended values differ by kind (1 s and 3 s).                                                                                                                                       |
| Let the default fill an absent `min` on `@[..max]`                                          | On an RPC, `min` is a throttle on the caller. Applying it to a member whose author wrote only `max` imposes a constraint the author did not write.                                      |
| Ceiling and floor lints in this change                                                      | The `[lints]` table holds levels only. A threshold needs new manifest syntax and an ADR-0024 amendment, which is a separate design. Filed as a follow-up.                               |

Satisfies: driftsys/ridl#741
