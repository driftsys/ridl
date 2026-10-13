# The WebSocket binding text and the `Violation` change — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task by task. Steps use
> checkbox (`- [ ]`) syntax for tracking. The per-task review gate of
> `~/.claude/rules/per-task-review-gate.md` applies: record the base SHA before
> each task's implementer starts, give the reviewer the diff from that base as a
> file, and require both verdicts (meets its brief; the code is sound) before
> the next task starts.

**Goal:** Apply the accepted WebSocket binding design to the frame
specification, the records, the codegen model's `KNOWN` table and the `wire`
freeze dump (pull request A), then make `ridl_rt::payload::Violation.type_name`
an `Option<&'static str>` (pull request B).

**Architecture:** Pull request A is text, one table row and one `xtask` dumper:
the frame specification §11.1 becomes the binding document, the `websocket` row
enters `crates/ridl-ir/src/codegen/bindings.rs`, and `xtask/src/freeze/wire.rs`
reads the §11.1 vector table into `freeze/wire.txt` with a structural check.
Pull request B is a breaking `ridl-rt` change with every consumer updated and
every generated output regenerated.

**Tech Stack:** Rust 2024 workspace (pinned toolchain), `xtask`, `insta`
snapshots, mdBook, prim, git-std, `just`.

**Spec:**
[`docs/wip/2026-10-12-ws-binding-design.md`](2026-10-12-ws-binding-design.md) —
rulings R-W-7 to R-W-19, the replacement text of section 3.1, the record changes
of section 3.2, the dump of section 4. Read it whole before any task. Its
decisions are settled; this plan does not reopen them.

**The two pull requests, in order:**

| PR | Branch                        | Base                            | Title                                                                    |
| -- | ----------------------------- | ------------------------------- | ------------------------------------------------------------------------ |
| A  | `docs/ws-binding-text`        | `origin/main`                   | `feat: specify the WebSocket binding and freeze its frame vectors`       |
| B  | `feat/violation-peer-no-name` | `origin/main` after A is merged | `feat(ridl-rt)!: report a violation that a peer sends with no type name` |

**Precondition.** The pull request of branch `docs/ws-binding` (the design and
this plan) is merged first, so that both bases hold the design note the issue
comments point at.

## Global Constraints

- Prose — comments, commit messages, docs, PR descriptions — is plain, literal
  English: no idioms, no figures of speech. Technical terms stay as they are.
- Shipped docs (`README`s, `AGENTS.md`, `CONTRIBUTING.md`, everything under
  `docs/` except `docs/wip/` and `docs/archive/`) and rustdoc describe the
  system as built. A sentence about what a future crate will do is not added
  unless the design's text states it verbatim.
- No story id (`E11.9`) and no plan name (`lane W`, `step 2`, `Task A3`, `PR A`)
  in any shipped file or in code. `docs/ROADMAP.md`, the ADRs and `docs/wip/`
  may hold story ids, but no step of this plan adds a plan name to an ADR
  either. Use `driftsys/ridl#N` for a gap.
- The justfile is the single definition of every gate. Run gates with
  `just <recipe>`; never copy a recipe's command.
- Never `--no-verify`, on a commit or a push. A local `prim lint` failure that
  CI does not show is a stale prim: reinstall it, do not skip the hook.
- Commits are Conventional Commits with the scopes of `.git-std.toml`. Every
  commit message ends with
  `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. A pull request
  description ends with
  `🤖 Generated with [Claude Code](https://claude.com/claude-code)`.
- Work in a fresh worktree per pull request, created from `origin/main`, and run
  `./bootstrap` in it before the first edit.
- Exact numbers, used everywhere: envelope **16** bytes; largest data-plane
  header without its envelope **20** bytes (`response`); `attach` header **39 +
  name** bytes, at most **39 + 255 = 294** bytes (see R-P-1); binding version
  `"1"`; frame version `1`; subprotocol `ridl-ws-v1`.

## Rulings taken by this plan

Each is the smallest decision that lets the plan proceed; none reopens a design
ruling. Later steps cite them by their `R-P-<n>` number.

- **R-P-1.** Ruling (plan): the `attach` header is 39 + name bytes and its
  maximum is 294 bytes, not 40 + name and 295 as the design's text says — the
  field list the design fixes (`form` 1 + `frame_version` 4 + `encoding` 1 +
  `hash` 32 + `name_len` 1 = 39) and the frozen vector `attach-1` (48 bytes =
  39 + 9) are the layout, and 40 / 295 are a miscount derived from them; the
  structural check would reject `attach-1` against 40. Every "40 + name", "40 +
  255 = 295" and "295 bytes" of the design's section 3.1 text is applied as
  "39 + name", "39 + 255 = 294" and "294 bytes". The controller reports this to
  Sebastien.
- **R-P-2.** Ruling (plan): the frame specification's §2 `Violation` row and the
  one §9.6 sentence that names `type_name: None` ("The receiving runtime reports
  the violation with no type name, which `ridl_rt::payload::Violation`
  represents as `type_name: None`; a violation the generated binding detects
  locally keeps its name.") land in pull request B, not A — both describe the
  `ridl-rt` type, and between A and B the type is still `&'static str`, so
  applying them in A would make a shipped document describe an API that does not
  exist. Everything else of section 3.1 lands in A, verbatim apart from R-P-1.
- **R-P-3.** Ruling (plan): the Trace line that points at the archived design
  note is not added by either pull request — the design says "once this note is
  archived", and archiving is the gardening of this design and plan, which runs
  after pull request B (outside this plan); pull request A adds only "ADR-0015
  decision 4" to the Trace section's `Binds:` line.
- **R-P-4.** Ruling (plan): the structural check accepts any `outcome` code 1 to
  9 on every form that carries one — which outcomes a form may carry is §5 and
  §6 semantics, not byte layout, and the design asks for lengths and codes only.
- **R-P-5.** Ruling (plan): the test
  `a_websocket_row_adds_its_frame_header_and_envelope_to_the_proto3_bound` in
  `crates/ridlc/tests/layout.rs` is kept, stops pushing a synthetic row, and
  asserts the real row's 20 + 16 + 8 = 44 — the design replaces the synthetic
  row, and the test still names the sum in one place.
- **R-P-6.** Ruling (plan): `the_emitter_writes_the_rendered_table_it_is_given`
  in `crates/ridl-ir/src/codegen/deployment.rs` keeps its two-row synthetic
  table — it tests `render`'s name order, which a one-row `KNOWN` cannot show;
  it does not read `KNOWN`.
- **R-P-7.** Ruling (plan): four stale sentences the design's section 3.2 does
  not list are corrected in pull request A, because each states that the
  `bindings` list is empty or that the binding is unspecified:
  `docs/book/codegen-plugins.md` ("and it is empty today (see below)"),
  `docs/book/writing-a-port.md` ("The WebSocket transport is planned and not
  built"), `docs/technotes/walking-skeleton-architecture.md` ("which has no row
  yet"), and the `KNOWN` rustdoc. One stale sentence is corrected in pull
  request B for the same reason: `docs/technotes/ridl-rt-by-example.md` ("A
  `Violation` names the typl type").
- **R-P-8.** Ruling (plan): the `docs/ROADMAP.md` paragraph also loses its
  parenthetical "(the WebSocket fixed to proto3, encoding tag 2)", because §11.1
  no longer fixes an encoding (R-W-11).
- **R-P-9.** Ruling (plan): the ADR-0021 amendment sits in its `## Status`
  section and as a dated paragraph under decision 9, beside the 2026-10-02
  `Rule::Unique` extension — decision 9 is where `Rule`'s `#[non_exhaustive]` is
  recorded, and decision 10 already lists `Violation` among the structs a field
  change breaks, so it needs no edit.
- **R-P-10.** Ruling (plan): #350 item 5 is ticked after pull request A merges
  and item 12 after pull request B merges — item 12's ruling is text in A but
  its change is code in B.

## Review Focus

The inputs most likely to break what this plan builds, with the test that pins
each (all in Task A2 unless stated):

1. A vector row whose hex has an odd digit count, an uppercase digit, or a
   missing code span — the dump must fail and name the row's `Id`, never write a
   partial line (`a_malformed_vector_row_fails`).
2. A renamed `### 11.1 WebSocket` heading, a renamed column, or an empty table —
   the dump must fail rather than silently drop all nine lines
   (`a_missing_vector_table_fails`).
3. An `attach` whose `name_len` disagrees with the message, whose name is empty,
   or whose name is not UTF-8 (`an_attach_with_a_bad_name_fails`).
4. A data-plane vector one byte short of its form's header, or a payload on a
   form that carries none
   (`a_vector_whose_length_disagrees_with_its_form_fails`).
5. A downstream program that matched `violation.type_name` as a `&str` — it must
   stop compiling, not silently print `None` (Task B1's red step; the `!` commit
   and the Kotlin heads-up of Task B3 tell its authors).

## Pre-flight: the files each task touches

| Task | PR | Files                                                                                                                                                                                                                                                                                                                                                                                                                   |
| ---- | -- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| A1   | A  | `docs/specification/frame-specification.md`                                                                                                                                                                                                                                                                                                                                                                             |
| A2   | A  | `xtask/src/freeze/wire.rs`, `freeze/wire.txt`, `freeze/README.md`, `CONTRIBUTING.md`, `docs/decisions/ADR-0029-stability-policy.md`, `docs/book/stability.md`                                                                                                                                                                                                                                                           |
| A3   | A  | `crates/ridl-ir/src/codegen/bindings.rs`, `crates/ridl-ir/src/codegen/deployment.rs`, `crates/ridlc/tests/layout.rs`, `crates/ridlc/tests/fixtures/cabin-layout.json`, `crates/ridlc/tests/codegen_model.rs`, `docs/design/codegen-plugins.md`, `docs/book/codegen-plugins.md`, `docs/book/writing-a-port.md`, `docs/technotes/walking-skeleton-architecture.md`                                                        |
| A4   | A  | `docs/decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md`, `docs/design/catalog-descriptor.md`, `docs/specification/ridl-family-overview.md`, `docs/ROADMAP.md`, `docs/archive/2026-10-06-ws-and-payload-formats-steering.md`                                                                                                                                                                      |
| A5   | A  | none (verify, push, pull request, issue comment texts)                                                                                                                                                                                                                                                                                                                                                                  |
| B1   | B  | `crates/ridl-rt/src/payload.rs`, `crates/ridl-rt/src/sample.rs`, `crates/ridl-rt/tests/payload.rs`, `crates/ridl-rt/tests/read_sample.rs`, `crates/ridl-rt/examples/read_sample.rs`, `docs/design/ridl-rt.md`, `docs/decisions/ADR-0021-ridl-rt-0.1-api-and-release.md`, `docs/specification/frame-specification.md`, `docs/technotes/ridl-rt-by-example.md`                                                            |
| B2   | B  | `crates/ridl-backend-rust/src/lib.rs`, `crates/ridl-backend-rust/src/codec.rs`, `crates/ridl-backend-rust/src/tests.rs`, `crates/ridl-backend-rust/src/snapshots/*.snap`, `crates/ridl-backend-rust/tests/flatbuffers_roundtrip.rs`, `crates/ridl-backend-rust/tests/interaction_face.rs`, `crates/ridl-backend-rust/tests/generated/interaction_face.rs`, `crates/ridlc/tests/snapshots/*.snap`, `freeze/generated/**` |
| B3   | B  | none (verify, push, pull request, issue comment and heads-up texts)                                                                                                                                                                                                                                                                                                                                                     |

Checked and unchanged: `crates/ridl-loopback` and `crates/ridl-rt-conformance`
neither build nor read a `Violation` (a grep for `Violation` and `type_name`
finds none); `crates/ridl-backend-ts` emits no `Violation`;
`freeze/codegen-protocol.txt` dumps the proto schema's fields, not `KNOWN`'s
values, so the row changes no line of it; `freeze/surfaces.toml` has a row for
`mcp` only, so neither `wire`, `codegen-protocol`, `generated-crate` nor
`runtime-crates` is compared with a baseline, and an added `wire` line passes.
`crates/ridl-ir/src/codegen/tests.rs` builds its own `v1::Binding` (14 + 8) to
test JSON rendering and does not read `KNOWN`; it is not changed.

---

## Pull request A — apply the binding

Set up once:

```bash
cd /Users/sebastientasson/Workspace/driftsys/ridl
git fetch origin
git worktree add -b docs/ws-binding-text .claude/worktrees/lane-w-ws-a origin/main
cd .claude/worktrees/lane-w-ws-a && ./bootstrap
git rev-parse HEAD   # record as BASE_A0
```

### Task A1: The frame specification text

**Files:**

- Modify: `docs/specification/frame-specification.md` (line 9 version; the
  As-built note, lines 23-35; §1 table row and paragraph, lines 70-83; §3
  encoding paragraph, lines 150-155; §4 after the forms table and the `outcome`
  row, lines 186-210; §9.6, lines 780-789; §10 item 3, lines 813-814; §11
  opening and §11.1, lines 834-855; §13 Streams bullet, lines 942-943; Trace
  `Binds:` line, lines 976-980)

**Interfaces:**

- Produces: the §11.1 vector table that Task A2 parses. Its exact shape is the
  design's section 3.1 "Test vectors" table: header row `| Id | Vector | Hex |`,
  nine rows, each `Id` and `Hex` cell one code span, `Hex` lowercase with single
  spaces. It sits under the heading `### 11.1 WebSocket` and before
  `### 11.2 Binder on Android`.

- [ ] **Step 1: Apply the replacements in document order**

  For each item below, the replacement text is the design's section 3.1
  paragraph named in the right column, copied verbatim with the `>` quote prefix
  removed, except where R-P-1 or 2 says otherwise.

  | Where in the specification                                                                                    | Design section 3.1 paragraph                                                                                                                                                                                                                                                                                                                 |
  | ------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
  | line 9                                                                                                        | "The version line": `Version: 0.2.0 — Draft`                                                                                                                                                                                                                                                                                                 |
  | As-built note, after "and no runtime in this workspace reads it at `attach`."                                 | "The As-built note gains" (one sentence)                                                                                                                                                                                                                                                                                                     |
  | §1 table, the `a binding` row's last cell                                                                     | "§1, the table's binding row"                                                                                                                                                                                                                                                                                                                |
  | §1 paragraph after the table, from "and which encoding a path carries" to "within a node."                    | "§1, the paragraph after the table"                                                                                                                                                                                                                                                                                                          |
  | §3, the sentence "The encoding is the one the path takes ... corrupt payloads."                               | "§3, the encoding paragraph"                                                                                                                                                                                                                                                                                                                 |
  | §4, a new paragraph after "The ten forms" table, before "**Why `kind` is on the frame at all.**"              | "§4, after "The ten forms" table" (the block starting **The codes.**)                                                                                                                                                                                                                                                                        |
  | §4 field table, `outcome` row, Domain cell                                                                    | "§4, the `outcome` row": the cell becomes `§5.3 and §5.4; §9.6 for what`contract(InvalidValue)`carries`                                                                                                                                                                                                                                      |
  | §9.6, a new last paragraph                                                                                    | "§9.6, a new last paragraph", **without** its sentence "The receiving runtime reports the violation with no type name, which `ridl_rt::payload::Violation` represents as `type_name: None`; a violation the generated binding detects locally keeps its name." (R-P-2; Task B1 adds it)                                                      |
  | §10 item 3                                                                                                    | "§10 item 3 becomes"                                                                                                                                                                                                                                                                                                                         |
  | §11 opening paragraph, the words "The WebSocket transport is planned in this repository and is not built yet" | "§11, the opening paragraph"                                                                                                                                                                                                                                                                                                                 |
  | §11.1, from its heading to the line before `### 11.2`                                                         | "§11.1, replaced whole" — with R-P-1: the `attach` row's "Header size" cell is `39 + name`; in "The sizes" the sentence reads "The largest header of any message is the `attach` at 39 + 255 = 294 bytes; it carries no payload, so a receiver's message size limit admits at least 294 bytes."; in item 4 "which admits at least 294 bytes" |
  | §13, the **Streams.** bullet                                                                                  | "§13, the Streams bullet"                                                                                                                                                                                                                                                                                                                    |
  | Trace, the `Binds:` bullet                                                                                    | the ADR-0015 clause becomes `decisions 3, 4 (every call has a response bound, §11.1 item 5), 10 and 17` (R-P-3)                                                                                                                                                                                                                              |

  Do not change the §2 `Violation` row (Task B1 does). Do not add the Trace line
  for the archived note.

- [ ] **Step 2: Format and check the structure**

  Run:
  `prim fmt docs/specification/frame-specification.md && prim lint docs/specification/frame-specification.md`
  Expected: exit 0. prim re-pads the tables; the hex cells keep their spaces.

  Run:
  `awk '/^### 11.1 WebSocket/,/^### 11.2/' docs/specification/frame-specification.md | grep -c '^|`[a-z]'`Expected:`16`
  (7 layout-table rows + 9 vector rows).

  Run:
  `grep -n '295\|40 + name\|proto3-framed\|is not built yet' docs/specification/frame-specification.md`
  Expected: only the §11 opening's "its transport crate is not built yet".

- [ ] **Step 3: Run the task gates**

  Run, in order:
  `just freeze-check && just link-check && just doc-path-check && just story-id-check && just book-check && just check`
  Expected: every recipe exits 0 (the book includes this file through
  `docs/book/reference/frame.md`).

- [ ] **Step 4: Commit**

  ```bash
  git add docs/specification/frame-specification.md
  git commit -m "docs(docs): specify the WebSocket binding in the frame specification

  The frame specification becomes version 0.2.0, still a draft. Section 11.1 is
  the WebSocket binding document: one session per connection, the ridl-ws-v1
  subprotocol, a fixed little-endian header layout, no fixed encoding, close
  codes, the caller's identity and nine test vectors. Section 4 and section 9.6
  gain the numeric codes, section 13 states how streams arrive as frame
  version 2.

  Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
  ```

### Task A2: The frame vectors in the `wire` dump

**Files:**

- Modify: `xtask/src/freeze/wire.rs` (module doc; new functions after
  `catalog_hashes`; `dump`; the `tests` module)
- Modify: `freeze/wire.txt` (regenerated, nine lines added)
- Modify: `freeze/README.md` ("The frozen source copies", one sentence)
- Modify: `CONTRIBUTING.md` ("Changing a frozen surface", lines 63-68)
- Modify: `docs/decisions/ADR-0029-stability-policy.md` (Status, Context bullet
  lines 53-56, decision 3 lines 159-163, decision 6 `Wire format` row line 209)
- Modify: `docs/book/stability.md` (lines 63-66)

**Interfaces:**

- Consumes: Task A1's §11.1 vector table.
- Produces: `fn frame_vectors(spec: &str) -> Result<Vec<String>, String>` and
  `fn check_vector(bytes: &[u8]) -> Result<(), String>` in
  `xtask/src/freeze/wire.rs`; dump lines of the form `frame-vector <id> <hex>`,
  `<hex>` lowercase with no spaces.

- [ ] **Step 1: Add stubs so that the tests compile**

  In `xtask/src/freeze/wire.rs`, after `catalog_hashes`, add:

  ```rust
  /// The frame specification, whose §11.1 vector table the dump reads.
  const FRAME_SPEC: &str = "docs/specification/frame-specification.md";

  fn frame_vectors(_spec: &str) -> Result<Vec<String>, String> {
      Ok(Vec::new())
  }

  fn check_vector(_bytes: &[u8]) -> Result<(), String> {
      Ok(())
  }
  ```

- [ ] **Step 2: Write the failing tests**

  Append to the `tests` module of `xtask/src/freeze/wire.rs`:

  ```rust
  /// A frame specification that holds only a §11.1 vector table with `rows`.
  fn spec_with(rows: &[&str]) -> String {
      format!(
          "## 11. The Bindings\n\n### 11.1 WebSocket\n\nText.\n\n| Id | Vector | Hex |\n| --- | --- | --- |\n{}\n\n### 11.2 Binder on Android\n\n| Id | Vector | Hex |\n| --- | --- | --- |\n| `ignored` | not read | `ff` |\n",
          rows.join("\n")
      )
  }

  fn root() -> &'static Path {
      Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap()
  }

  #[test]
  fn the_frame_specification_holds_the_nine_vectors() {
      let text = std::fs::read_to_string(root().join(FRAME_SPEC)).unwrap();
      assert_eq!(
          frame_vectors(&text).unwrap(),
          vec![
              "frame-vector attach-1 010100000001000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f097665682e636162696e",
              "frame-vector attached-accepted 020100",
              "frame-vector attached-mismatch 020902",
              "frame-vector subscribe-signal 03010100000002000000",
              "frame-vector answer-accepted 040101000000020000000100",
              "frame-vector publish-live 07010100000002000000ab89674523010000070000000000000002ff01",
              "frame-vector request-command 09030100000003000000ab89674523010000010000000000000001",
              "frame-vector response-accepted 0a030100000003000000ac89674523010000010000000000000001000000000000000100",
              "frame-vector response-violation 0a040100000004000000ac89674523010000020000000000000002000000000000000301",
          ]
      );
  }

  #[test]
  fn a_vector_line_drops_the_spaces_of_its_hex_cell() {
      let text = spec_with(&["| `ok` | `attached` accepted | `02 01 00` |"]);
      assert_eq!(frame_vectors(&text).unwrap(), vec!["frame-vector ok 020100"]);
  }

  #[test]
  fn a_vector_whose_length_disagrees_with_its_form_fails() {
      for (hex, why) in [
          ("02 01", "attached is 3 bytes"),
          ("03 01 01000000 02000000 ff", "subscribe carries no payload"),
          ("07 01 01000000 02000000 ab89674523010000 0700000000000000", "publish is 27 bytes at least"),
          ("0a 03 01000000 03000000 ac89674523010000 0100000000000000 0100000000000000 01", "response is 36 bytes at least"),
      ] {
          let text = spec_with(&[&format!("| `short` | x | `{hex}` |")]);
          let err = frame_vectors(&text).expect_err(why);
          assert!(err.contains("short"), "{why}: {err}");
      }
  }

  #[test]
  fn a_code_outside_its_table_fails() {
      for (hex, why) in [
          ("0b", "form 11"),
          ("02 0a 00", "outcome 10"),
          ("02 01 01", "detail must be 0 with accepted"),
          ("02 03 07", "Rule code 7"),
          ("02 09 06", "refusal reason 6"),
          ("03 06 01000000 02000000", "kind 6"),
          ("03 01 00000000 02000000", "interface 0"),
          ("03 01 01000000 00000000", "ordinal 0"),
          ("07 01 01000000 02000000 ab89674523010000 0700000000000000 04", "provenance 4"),
      ] {
          let text = spec_with(&[&format!("| `bad` | x | `{hex}` |")]);
          let err = frame_vectors(&text).expect_err(why);
          assert!(err.contains("bad"), "{why}: {err}");
      }
  }

  #[test]
  fn an_attach_with_a_bad_name_fails() {
      let head = "01 01000000 01 000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
      for (tail, why) in [
          ("09 7665682e636162", "name_len 9 with 7 name bytes"),
          ("00", "an empty name"),
          ("02 ff fe", "a name that is not UTF-8"),
      ] {
          let text = spec_with(&[&format!("| `att` | x | `{head} {tail}` |")]);
          let err = frame_vectors(&text).expect_err(why);
          assert!(err.contains("name"), "{why}: {err}");
      }
      let wrong_version = "01 02000000 01 000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f 01 61";
      assert!(frame_vectors(&spec_with(&[&format!("| `v` | x | `{wrong_version}` |")])).is_err());
      let wrong_encoding = "01 01000000 04 000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f 01 61";
      assert!(frame_vectors(&spec_with(&[&format!("| `e` | x | `{wrong_encoding}` |")])).is_err());
  }

  #[test]
  fn a_malformed_vector_row_fails() {
      for (row, why) in [
          ("| `odd` | x | `02 01 0` |", "an odd digit count"),
          ("| `upper` | x | `02 01 0A` |", "an uppercase digit"),
          ("| `plain` | x | 02 01 00 |", "a Hex cell that is not a code span"),
          ("| plain-id | x | `02 01 00` |", "an Id cell that is not a code span"),
          ("| `Bad_Id` | x | `02 01 00` |", "an Id outside [a-z0-9-]"),
          ("| `two` | `02 01 00` |", "a row of two cells"),
      ] {
          assert!(frame_vectors(&spec_with(&[row])).is_err(), "{why}");
      }
      let twice = spec_with(&[
          "| `same` | x | `02 01 00` |",
          "| `same` | x | `02 09 02` |",
      ]);
      let err = frame_vectors(&twice).expect_err("a duplicate Id");
      assert!(err.contains("same"), "{err}");
  }

  #[test]
  fn a_missing_vector_table_fails() {
      assert!(frame_vectors("## 11. The Bindings\n").is_err(), "no §11.1 heading");
      assert!(
          frame_vectors("### 11.1 WebSocket\n\nNo table.\n\n### 11.2 Binder\n").is_err(),
          "no table"
      );
      assert!(frame_vectors(&spec_with(&[])).is_err(), "a table with no row");
      let renamed = spec_with(&["| `ok` | x | `02 01 00` |"]).replacen("| Id |", "| Name |", 1);
      assert!(frame_vectors(&renamed).is_err(), "a renamed column");
  }
  ```

  Also extend `wire_dump_lists_encodings` with:

  ```rust
  assert_eq!(
      lines.iter().filter(|l| l.starts_with("frame-vector ")).count(),
      9,
      "{lines:?}"
  );
  ```

- [ ] **Step 3: Run the tests to verify they fail on their assertions**

  Run: `cargo test -p xtask -- freeze::wire::tests` Expected: compiles; FAIL in
  `the_frame_specification_holds_the_nine_vectors` (left is `[]`),
  `a_vector_line_drops_the_spaces_of_its_hex_cell`, every `expect_err`/`is_err`
  test ("called `Result::expect_err()` on an `Ok` value"), and
  `wire_dump_lists_encodings` (count 0). The other existing tests pass.

- [ ] **Step 4: Implement the reader and the check**

  Replace the two stubs with:

  ```rust
  /// The cells of one Markdown table row, trimmed; `None` for a line that is
  /// not a row.
  fn cells(line: &str) -> Option<Vec<&str>> {
      let inner = line.trim().strip_prefix('|')?.strip_suffix('|')?;
      Some(inner.split('|').map(str::trim).collect())
  }

  /// The text of a cell that is exactly one code span.
  fn code_span(cell: &str) -> Option<&str> {
      cell.strip_prefix('`')?
          .strip_suffix('`')
          .filter(|text| !text.contains('`'))
  }

  /// Lowercase hexadecimal with no spaces, as bytes.
  fn parse_hex(text: &str) -> Result<Vec<u8>, String> {
      if !text.len().is_multiple_of(2) {
          return Err(format!("`{text}` has an odd number of digits"));
      }
      if !text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
          return Err(format!("`{text}` is not lowercase hexadecimal"));
      }
      (0..text.len())
          .step_by(2)
          .map(|i| u8::from_str_radix(&text[i..i + 2], 16).map_err(|e| e.to_string()))
          .collect()
  }

  /// `frame-vector <id> <hex>` for each row of the test-vector table of the
  /// frame specification §11.1: the table under `### 11.1 WebSocket` whose
  /// columns are `Id`, `Vector` and `Hex`. The table is the vectors' source;
  /// no encoder exists. A row whose bytes [`check_vector`] rejects, a
  /// malformed row, a duplicate `Id` and a missing or empty table are errors.
  fn frame_vectors(spec: &str) -> Result<Vec<String>, String> {
      let (_, section) = spec
          .split_once("### 11.1 WebSocket\n")
          .ok_or("the frame specification has no `### 11.1 WebSocket` section")?;
      let section = section
          .split_once("\n### ")
          .map_or(section, |(head, _)| head);
      let mut lines = section.lines();
      lines
          .by_ref()
          .find(|line| cells(line).as_deref() == Some(&["Id", "Vector", "Hex"][..]))
          .ok_or("§11.1 has no table with the columns `Id`, `Vector` and `Hex`")?;
      lines.next();
      let mut ids = std::collections::BTreeSet::new();
      let mut out = Vec::new();
      for line in lines.take_while(|line| line.trim_start().starts_with('|')) {
          let row = cells(line).ok_or_else(|| format!("a malformed vector row: {line}"))?;
          let [id, _, hex] = row.as_slice() else {
              return Err(format!("a vector row without three cells: {line}"));
          };
          let id = code_span(id)
              .filter(|id| {
                  !id.is_empty()
                      && id
                          .bytes()
                          .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
              })
              .ok_or_else(|| format!("the `Id` cell is not one code span of [a-z0-9-]: {line}"))?;
          if !ids.insert(id) {
              return Err(format!("{id}: the `Id` is used twice"));
          }
          let hex = code_span(hex)
              .ok_or_else(|| format!("{id}: the `Hex` cell is not one code span"))?
              .replace(' ', "");
          let bytes = parse_hex(&hex).map_err(|e| format!("{id}: {e}"))?;
          check_vector(&bytes).map_err(|e| format!("{id}: {e}"))?;
          out.push(format!("frame-vector {id} {hex}"));
      }
      if out.is_empty() {
          return Err("the §11.1 vector table has no row".to_string());
      }
      Ok(out)
  }

  /// A little-endian `u32` at `bytes[at..at + 4]`.
  fn u32_le(bytes: &[u8], at: usize) -> u32 {
      u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
  }

  /// `value` is in `range`, or an error naming `field`.
  fn check_in(value: u8, range: std::ops::RangeInclusive<u8>, field: &str) -> Result<(), String> {
      if range.contains(&value) {
          Ok(())
      } else {
          Err(format!("{field} {value} is outside {range:?}"))
      }
  }

  /// An `outcome` code and its `detail`: the `Rule` code with outcome 3, the
  /// refusal reason with outcome 9, and 0 with every other outcome (frame
  /// specification §4, §9.6, §11.1 item 2).
  fn check_outcome(outcome: u8, detail: u8) -> Result<(), String> {
      check_in(outcome, 1..=9, "outcome")?;
      match outcome {
          3 => check_in(detail, 1..=6, "the `Rule` detail"),
          9 => check_in(detail, 1..=5, "the refusal detail"),
          _ if detail == 0 => Ok(()),
          _ => Err(format!("detail {detail} with outcome {outcome}, which takes 0")),
      }
  }

  /// An `attach`: `form` 1, `frame_version` u32 = 1, `encoding` 1 to 3, a
  /// 32-byte hash, `name_len` u8 of at least 1, and exactly `name_len` bytes
  /// of UTF-8 — 39 bytes plus the name.
  fn check_attach(bytes: &[u8]) -> Result<(), String> {
      if bytes.len() < 39 {
          return Err(format!("{} bytes, below the 39-byte `attach` header", bytes.len()));
      }
      if u32_le(bytes, 1) != 1 {
          return Err(format!("frame_version {} is not 1", u32_le(bytes, 1)));
      }
      check_in(bytes[5], 1..=3, "encoding")?;
      let name_len = usize::from(bytes[38]);
      if name_len == 0 {
          return Err("the `attach` name is empty".to_string());
      }
      if bytes.len() != 39 + name_len {
          return Err(format!(
              "name_len {name_len} does not match the {} name bytes",
              bytes.len() - 39
          ));
      }
      std::str::from_utf8(&bytes[39..])
          .map(|_| ())
          .map_err(|e| format!("the `attach` name is not UTF-8: {e}"))
  }

  /// The bytes of one vector against the layout of the frame specification
  /// §11.1 item 2: the header size of its form, whether the form carries a
  /// payload, and every code against its table.
  fn check_vector(bytes: &[u8]) -> Result<(), String> {
      let form = *bytes.first().ok_or("the vector is empty")?;
      let (header, payload) = match form {
          1 => return check_attach(bytes),
          2 => (3, false),
          3 | 5 | 6 => (10, false),
          4 => (12, false),
          7 => (27, true),
          8 | 9 => (26, true),
          10 => (36, true),
          other => return Err(format!("form {other} is outside 1..=10")),
      };
      if bytes.len() < header || (!payload && bytes.len() != header) {
          return Err(format!(
              "{} bytes, but form {form} is a {header}-byte header{}",
              bytes.len(),
              if payload { " plus a payload" } else { " and no payload" }
          ));
      }
      if form == 2 {
          return check_outcome(bytes[1], bytes[2]);
      }
      check_in(bytes[1], 1..=5, "kind")?;
      if u32_le(bytes, 2) == 0 {
          return Err("interface 0".to_string());
      }
      if u32_le(bytes, 6) == 0 {
          return Err("ordinal 0".to_string());
      }
      match form {
          4 => check_outcome(bytes[10], bytes[11]),
          7 => check_in(bytes[26], 1..=3, "provenance"),
          10 => check_outcome(bytes[34], bytes[35]),
          _ => Ok(()),
      }
  }
  ```

  In `dump`, after `lines.extend(catalog_hashes(root, &ridl, &scratch.0)?);`
  add:

  ```rust
  let spec = std::fs::read_to_string(root.join(FRAME_SPEC))
      .map_err(|e| format!("cannot read {FRAME_SPEC}: {e}"))?;
  lines.extend(frame_vectors(&spec)?);
  ```

  Replace the module doc's first sentence with: "The wire mapping: the encoding
  names the manifest accepts, the proto3 and FlatBuffers schemas that
  `ridl build` writes for the corpus, the catalog hash of the cabin example, and
  the test vectors of the WebSocket binding." and append the paragraph: "The
  vectors are read from the frame specification itself
  (`docs/specification/frame-specification.md` §11.1), because that table is
  their source and no encoder exists; each is checked against the binding's
  header layout before it becomes a line."

- [ ] **Step 5: Run the tests to verify they pass**

  Run: `cargo test -p xtask -- freeze::wire::tests` Expected: PASS, every test
  of the module.

- [ ] **Step 6: Regenerate the dumps and check only `wire.txt` changed**

  Run: `cargo xtask freeze update && git diff --stat freeze/` Expected:
  `freeze/wire.txt | 9 +` and nothing else. If any other dump changes, stop and
  report: no other surface may change in this pull request.

  Run: `git diff freeze/wire.txt | grep '^[-+]f' | sort` Expected: nine
  `+frame-vector` lines equal to the list of Step 2, and no `-` line.

- [ ] **Step 7: Update the records that list the policy-only parts**

  - `freeze/README.md`, "The frozen source copies", append to the first
    paragraph: "The `wire` dump also holds the test vectors of the WebSocket
    binding: one `frame-vector <id> <hex>` line per row of the vector table in
    `docs/specification/frame-specification.md` §11.1, read from that document
    by the row's `Id`, so an edit to a vector changes the dump."
  - `CONTRIBUTING.md`, "Changing a frozen surface": the list becomes "the
    formats of `interfaces.lock` and `.ridl/baseline`, the exit codes of every
    subcommand other than `ridl check`, and the output of `ridl diff` and the
    `ridl-diff` categories." (", the frame envelope and the `attach` handshake"
    removed).
  - `docs/book/stability.md`, lines 63-66: the list becomes "the formats of
    `interfaces.lock` and `.ridl/baseline`, the exit codes of every subcommand
    other than `ridl check`, the output of `ridl diff`, and the categories that
    `ridl diff` reports." Line 28 ("The frame envelope, the `attach` handshake,
    ...") stays: it lists what is frozen.
  - `docs/decisions/ADR-0029-stability-policy.md`:
    - `## Status`, a new last paragraph, dated with the commit's day
      (`date +%F`): "**Amended <date> — the wire dump holds the frame vectors.**
      The `wire` dump gains one `frame-vector <id> <hex>` line per row of the
      vector table of the frame specification §11.1, so the frame envelope and
      the `attach` handshake are checked by the dump and leave the policy-only
      list. The Context bullet on the wire dump, decision 3 and decision 6's
      `Wire format` row are changed in place."
    - Context, the bullet at lines 53-56 becomes: "- The wire dump holds the
      encoding names, the `.proto` and `.fbs` lines for the corpus, the catalog
      hash of the cabin example, and the test vectors of the WebSocket binding
      (the frame specification §11.1), which cover the frame envelope and the
      `attach` handshake."
    - Decision 3, lines 159-163: "The output of `ridl diff` and the `ridl-diff`
      categories, the exit codes of every subcommand other than `ridl check`,
      and the formats of `interfaces.lock` and `.ridl/baseline` are in no dump;
      they are frozen by policy only." (the sentence keeps its following "This
      record is the one that states ..." sentence).
    - Decision 6, the `Wire format` row's second cell: "`wire`: encoding names,
      `.proto` and `.fbs` lines, catalog hash, frame vectors".

  Run:
  `prim fmt freeze/README.md CONTRIBUTING.md docs/book/stability.md docs/decisions/ADR-0029-stability-policy.md && prim lint freeze/README.md CONTRIBUTING.md docs/book/stability.md docs/decisions/ADR-0029-stability-policy.md`
  Expected: exit 0.

  Run:
  `grep -rn "frames are policy only\|holds no frame vectors" docs CONTRIBUTING.md freeze | grep -v 'docs/archive\|docs/wip'`
  Expected: no output.

- [ ] **Step 8: Run the task gates**

  Run:
  `cargo fmt --all --check && cargo clippy -p xtask --all-targets -- -D warnings`
  Expected: exit 0.

  Run, in order:
  `just freeze-check && just link-check && just doc-path-check && just story-id-check && just book-check && just check`
  Expected: every recipe exits 0.

- [ ] **Step 9: Commit**

  ```bash
  git add xtask/src/freeze/wire.rs freeze/wire.txt freeze/README.md CONTRIBUTING.md docs/book/stability.md docs/decisions/ADR-0029-stability-policy.md
  git commit -m "feat(xtask): freeze the WebSocket frame vectors in the wire dump

  The wire dump reads the test-vector table of the frame specification section
  11.1 and writes one frame-vector line per row, after checking each vector's
  length against its form's header and each code against its table. The frame
  envelope and the attach handshake leave the list of parts frozen by policy
  only, in ADR-0029, the stability chapter, CONTRIBUTING.md and freeze/README.md.

  Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
  ```

### Task A3: The `websocket` row in `KNOWN`

**Files:**

- Modify: `crates/ridl-ir/src/codegen/bindings.rs:17-27`
- Modify: `crates/ridl-ir/src/codegen/deployment.rs:757-764` (test)
- Modify: `crates/ridlc/tests/layout.rs:30-35` (module doc), `:334-338`,
  `:357-389`
- Modify: `crates/ridlc/tests/fixtures/cabin-layout.json` (header comment,
  messages comment, the `warning` entry)
- Modify: `crates/ridlc/tests/codegen_model.rs:1001`
- Modify: `docs/design/codegen-plugins.md:557-595`,
  `docs/book/codegen-plugins.md:332-335` and `:390-393`,
  `docs/book/writing-a-port.md:208-209`,
  `docs/technotes/walking-skeleton-architecture.md:160-161`

**Interfaces:**

- Produces:
  `KNOWN == [Known { name: "websocket", version: "1",
  frame_header_max_bytes: Some(20), envelope_bytes: Some(16) }]`;
  every deployment section's `bindings` equals
  `[v1::Binding { name: "websocket", version: "1", frame_header_max_bytes: Some(20), envelope_bytes: Some(16) }]`.

- [ ] **Step 1: Write the failing tests**

  In `crates/ridl-ir/src/codegen/deployment.rs`, replace the body of
  `the_binding_list_is_the_known_table_in_name_order` with:

  ```rust
  // The one row is the WebSocket binding of the frame specification
  // §11.1: a data-plane header of at most 20 bytes and a 16-byte
  // envelope.
  assert_eq!(
      bindings(),
      vec![v1::Binding {
          name: "websocket".to_string(),
          version: "1".to_string(),
          frame_header_max_bytes: Some(20),
          envelope_bytes: Some(16),
      }]
  );
  assert_eq!(section().bindings, bindings());
  assert_eq!(bindings().len(), KNOWN.len());
  ```

  In `crates/ridlc/tests/layout.rs`, in
  `a_layout_plugin_computes_cabin_from_the_request_alone`, replace the last
  comment and assertion (lines 334-338) with:

  ```rust
  // The request carries the `websocket` row of the frame specification
  // §11.1, so `Cabin.warning`'s `max_message_bytes` is 20 + 16 + 8.
  let deployment = request.deployment.as_ref().expect("a deployment");
  assert_eq!(
      deployment.bindings,
      vec![v1::Binding {
          name: "websocket".to_string(),
          version: "1".to_string(),
          frame_header_max_bytes: Some(20),
          envelope_bytes: Some(16),
      }]
  );
  ```

  Replace the doc comment and body of
  `a_websocket_row_adds_its_frame_header_and_envelope_to_the_proto3_bound`
  (R-P-5) with:

  ```rust
  /// A message's `max_message_bytes` is the `websocket` row's frame header
  /// bound plus its envelope plus the message's proto3 bound. The row is the
  /// one the toolchain puts in every deployment section.
  #[test]
  fn a_websocket_row_adds_its_frame_header_and_envelope_to_the_proto3_bound() {
      let (request, _) = request_for(&cabin(), "veh.cabin");
      let actual = layout_json(&request);
      let max_message_bytes = |member: &str| {
          actual["messages"]
              .as_array()
              .expect("a list of messages")
              .iter()
              .find(|message| message["member"] == member)
              .unwrap_or_else(|| panic!("a message for {member}"))["max_message_bytes"]
              .clone()
      };
      // `Warning`'s proto3 bound is 8 (the fixture derives it): 20 + 16 + 8.
      assert_eq!(max_message_bytes("warning"), json!(44));
      // `Temperature` has no proto3 bound, so the sum has no value.
      assert_eq!(max_message_bytes("temperature"), Value::Null);
  }
  ```

  In `crates/ridlc/tests/fixtures/cabin-layout.json`: the header line "-
  Bindings: none, so no `websocket` row." becomes "- Bindings: the `websocket`
  row, a frame header of at most 20 bytes and an envelope of 16 bytes (the frame
  specification §11.1)."; the messages comment "With no `websocket` row, every
  `max_message_bytes` is null." becomes "With the `websocket` row,
  `max_message_bytes` is 20 + 16 + `proto3_bound`, and null when `proto3_bound`
  is null."; in the `warning` entry, append the comment line
  `// max_message_bytes: 20 + 16 + 8 = 44.` after "(1 + 5) + (1 + 1) = 8." and
  set `"max_message_bytes": 44`.

  In `crates/ridlc/tests/codegen_model.rs` line 1001, replace `"bindings": []`
  with:

  ```text
  "bindings": [
    {
      "name": "websocket",
      "version": "1",
      "frameHeaderMaxBytes": 20,
      "envelopeBytes": 16
    }
  ]
  ```

  (four spaces before `"bindings"`, as the line it replaces).

- [ ] **Step 2: Run the tests to verify they fail on their assertions**

  Run:
  `cargo test -p ridl-ir --lib codegen::deployment::tests::the_binding_list_is_the_known_table_in_name_order`
  Expected: FAIL, `left: []`.

  Run: `cargo test -p ridlc --test layout --test codegen_model` Expected: FAIL
  in `a_layout_plugin_computes_cabin_from_the_request_alone`,
  `a_websocket_row_adds_its_frame_header_and_envelope_to_the_proto3_bound`
  (`left: Null, right: 44`) and
  `the_cabin_deployment_section_is_rendered_end_to_end`; every other test
  passes.

- [ ] **Step 3: Add the row and its rustdoc**

  In `crates/ridl-ir/src/codegen/bindings.rs`:

  ```rust
  /// The largest frame header the binding adds to a data-plane message, in
  /// bytes. `None` when the binding states no frame header.
  pub frame_header_max_bytes: Option<u32>,
  ```

  ```rust
  /// Every known binding. A row's numbers come from its binding document: the
  /// `websocket` row from the frame specification §11.1
  /// (`docs/specification/frame-specification.md`), a value derived from the
  /// binding's frozen header layout. Adding a row here adds it to every
  /// deployment section.
  pub const KNOWN: &[Known] = &[Known {
      name: "websocket",
      version: "1",
      frame_header_max_bytes: Some(20),
      envelope_bytes: Some(16),
  }];
  ```

- [ ] **Step 4: Run the tests to verify they pass**

  Run:
  `cargo test -p ridl-ir --lib codegen:: && cargo test -p ridlc --test layout --test codegen_model`
  Expected: PASS.

  Run:
  `grep -rn '"bindings": \[\]\|bindings.is_empty()\|KNOWN.is_empty()' crates`
  Expected: no output.

- [ ] **Step 5: Update the records of the row**

  - `docs/design/codegen-plugins.md`, "Bindings, and what is not built", the
    first paragraph becomes:

    ```markdown
    The `bindings` list holds the `websocket` row (binding version 1, frame
    header at most 20 bytes, envelope 16 bytes), from
    [the frame specification](../specification/frame-specification.md) §11.1.
    The emitter writes one entry per row of the table of known bindings
    (`crates/ridl-ir/src/codegen/bindings.rs`), in name order.
    ```

  - Same file, "The layout test plugin", the paragraph that starts "Two things
    the cabin fixture does not show" becomes:

    ```markdown
    The fixture states the socket message sizes the `websocket` row gives:
    20 + 16 + 8 = 44 bytes for `Cabin.warning`, whose proto3 bound is 8, and
    `null` for the members that have no proto3 bound. A second test checks the
    same sum on its own. Declared sizing values are not in the `examples/cabin`
    source; another test in the same file builds a workspace in a temporary
    directory that declares `depth`, `slots` and `budget` and checks that each
    declared value reaches the request with the declared source.
    ```

  In the two book passages below, `<<text>>` stands for a Markdown link whose
  text is `text` and whose target is `reference/frame.md#111-websocket`
  (relative to `docs/book/`); the plan cannot write the link itself, because
  `just link-check` resolves it from `docs/wip/`.

  - `docs/book/codegen-plugins.md` lines 332-335: the clause "and it is empty
    today (see below)." is replaced, so the sentence ends:

    ```markdown
    the deployment section's `bindings` list carries those values: the
    `websocket` row of
    <<the frame specification §11.1>>, a frame
    header of at most 20 bytes and an envelope of 16 bytes.
    ```

  - Same file, "What is not available yet", the "Transport binding overheads"
    bullet becomes (the `i718` reference definition already exists):

    ```markdown
    - **A check of the binding overheads against a frame.** The `websocket`
      row's sum is not yet checked against a frame a transport writes, because
      no transport in this repository writes one ([driftsys/ridl#718][i718]).
    ```

  - `docs/book/writing-a-port.md` lines 208-209: "The WebSocket transport is
    planned and not built, and on Android" becomes:

    ```markdown
    The WebSocket binding is specified in
    <<§11.1>> and its transport is not built,
    and on Android
    ```

  - `docs/technotes/walking-skeleton-architecture.md` lines 160-161: "which has
    no row yet" becomes "whose one row is the `websocket` binding of the frame
    specification §11.1".

  Run:
  `prim fmt docs/design/codegen-plugins.md docs/book/codegen-plugins.md docs/book/writing-a-port.md docs/technotes/walking-skeleton-architecture.md && prim lint docs/design/codegen-plugins.md docs/book/codegen-plugins.md docs/book/writing-a-port.md docs/technotes/walking-skeleton-architecture.md`
  Expected: exit 0.

- [ ] **Step 6: Run the task gates**

  Run:
  `cargo fmt --all --check && cargo clippy -p ridl-ir -p ridlc --all-targets -- -D warnings`
  Expected: exit 0.

  Run, in order:
  `just freeze-check && just link-check && just doc-path-check && just story-id-check && just book-check && just check`
  Expected: every recipe exits 0. `freeze-check` passes with no dump change (the
  protocol dump is the schema, not the row).

- [ ] **Step 7: Commit**

  ```bash
  git add crates/ridl-ir/src/codegen/bindings.rs crates/ridl-ir/src/codegen/deployment.rs crates/ridlc/tests/layout.rs crates/ridlc/tests/fixtures/cabin-layout.json crates/ridlc/tests/codegen_model.rs docs/design/codegen-plugins.md docs/book/codegen-plugins.md docs/book/writing-a-port.md docs/technotes/walking-skeleton-architecture.md
  git commit -m "feat(ridl-ir): add the websocket binding row to the deployment section

  KNOWN gains the row of the frame specification section 11.1: binding version
  1, a data-plane frame header of at most 20 bytes and a 16-byte envelope. Every
  deployment section carries it, and the cabin layout fixture's socket message
  size for Cabin.warning is 20 + 16 + 8 = 44. The synthetic 14 + 2 row of the
  layout test is removed.

  Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
  ```

### Task A4: ADR-0020 and the records that cite the binding

**Files:**

- Modify:
  `docs/decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md`
  (Status, a new last amendment paragraph; decision 6 table line 332)
- Modify: `docs/design/catalog-descriptor.md:277-279`
- Modify: `docs/specification/ridl-family-overview.md:48`
- Modify: `docs/ROADMAP.md:249-251`
- Modify: `docs/archive/2026-10-06-ws-and-payload-formats-steering.md` ("H.
  Recorded, not asked")

**Interfaces:** none; text only.

- [ ] **Step 1: Apply the changes**

  - ADR-0020, decision 6 table, `ridl-transport-ws` row, "Holds" cell: "the
    ports over a WebSocket, proto3-framed; the default of the getting-started
    path" becomes "the ports over a WebSocket, carrying the encoding the session
    names, proto3 by default (frame specification §11.1); the default of the
    getting-started path".
  - ADR-0020 `## Status`, after the last "**Amended ...**" paragraph, dated with
    the commit's day (`date +%F`): "**Amended <date> — decision 6, the WebSocket
    transport carries the session's encoding.** The `ridl-transport-ws` row said
    "proto3-framed". The frame specification §11.1, version 0.2.0, fixes no
    encoding for the WebSocket binding: a session carries the encoding its
    `attach` names, and the transport offers proto3 when the program names none.
    The row is changed in place."
  - `docs/design/catalog-descriptor.md`, the "No runtime reads the list" bullet
    gains, after "on the strength of the list.": "The frame specification §11.1
    names the descriptor as the source a providing runtime reads the list from;
    the first WebSocket provider holds no descriptor and applies the exact-match
    rule."
  - `docs/specification/ridl-family-overview.md` line 48, the Frame
    Specification row: "0.1 draft" becomes "0.2 draft"; "no binding is built"
    becomes "the WebSocket binding is specified (§11.1), none is built".
  - `docs/ROADMAP.md` lines 249-251 (R-P-8): "E11.9 lands over FlatBuffers, and
    `frame-specification.md` section 11.1 (the WebSocket fixed to proto3,
    encoding tag 2) is amended when E11.9 lands; the specification is unchanged
    until then." becomes "E11.9 lands over FlatBuffers, and
    `frame-specification.md` section 11.1 was amended before 0.10.0 by the
    binding document; E11.9 implements it."
  - Steering note, "H. Recorded, not asked", a new paragraph: "**Changed
    <date> (WebSocket binding design, R-W-16).** The `KNOWN` row and #718's
    first half moved from #265 to the pull request that amended the frame
    specification §11.1; #265 keeps the test that checks the sum against a frame
    the transport writes."

- [ ] **Step 2: Format and check**

  Run:
  `prim fmt docs/decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md docs/design/catalog-descriptor.md docs/specification/ridl-family-overview.md docs/ROADMAP.md docs/archive/2026-10-06-ws-and-payload-formats-steering.md`,
  then the same file list with `prim lint`. Expected: exit 0.

  Run:
  `grep -rn "proto3-framed\|0.1 draft\|unchanged until then" docs --include=*.md | grep -v 'docs/archive\|docs/wip'`
  Expected: no output.

- [ ] **Step 3: Run the task gates**

  Run, in order:
  `just freeze-check && just link-check && just doc-path-check && just story-id-check && just book-check && just check`
  Expected: every recipe exits 0.

- [ ] **Step 4: Commit**

  ```bash
  git add docs/decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md docs/design/catalog-descriptor.md docs/specification/ridl-family-overview.md docs/ROADMAP.md docs/archive/2026-10-06-ws-and-payload-formats-steering.md
  git commit -m "docs(adr): record the WebSocket binding in ADR-0020 and the records that cite it

  ADR-0020 decision 6 no longer fixes proto3 for the WebSocket transport. The
  catalog descriptor record names where a providing runtime reads the
  compatible list, the family overview names frame specification 0.2, and the
  roadmap states that section 11.1 is amended.

  Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
  ```

### Task A5: Verify, push and open pull request A

- [ ] **Step 1: Run the whole gate**

  Run: `just verify` Expected: exit 0 (lint-commits, then build).

- [ ] **Step 2: Push and open the pull request**

  ```bash
  git push -u origin docs/ws-binding-text
  gh pr create --base main --title "feat: specify the WebSocket binding and freeze its frame vectors" --body-file <body>
  ```

  The body states: what changed (the four commits' subjects); that the `wire`
  dump gains nine lines and no other dump changes; that no `surfaces.toml` row
  is added (the `wire` row waits for the `.proto`/`.fbs` review and #858); that
  `attach`'s header is 39 + name bytes and its maximum 294 bytes, a correction
  of the design's 40 / 295 (R-P-1); "Refs #350, #718, #336, #265". It must not
  hold a closing keyword for #350, #718 or #336, not even inside a negation
  ("does not close #718" closes it). It ends with
  `🤖 Generated with [Claude Code](https://claude.com/claude-code)`.

  Run:
  `gh pr view --json body -q .body | grep -inE '(close|fix|resolve)[sd]? #'`
  Expected: no output.

- [ ] **Step 3: After the merge, the controller posts these comments**

  Replace `#A` with pull request A's number.

  **#350** (and tick item 5 in the issue body):

  > Item 5 is settled by the WebSocket binding, the frame specification §11.1
  > item 5 (merged in #A). Since ADR-0015 decision 4 as amended (#749) every
  > command and every query has a response bound, so on this binding a call
  > always has an outcome: the `response`; `Transport::Undelivered` (command) or
  > `Transport::Timeout` (query) when the bound passes; or `Transport::Down`
  > when the session ends first. The binding never retransmits a `request`, and
  > the caller's identity is the connection, which does not survive a reconnect.
  > Reasoning: R-W-13 of `docs/wip/2026-10-12-ws-binding-design.md`.
  >
  > Item 12 is ruled (R-W-14 of the same note): the type name does not cross.
  > The frame carries the `Rule` of a `contract(InvalidValue)` outcome as a
  > one-byte code (§9.6), and `Violation.type_name` becomes
  > `Option<&'static str>`, `None` for a violation a peer reports. The `ridl-rt`
  > change is a separate breaking pull request; item 12 is ticked when it
  > merges.

  **#718:**

  > The `websocket` row landed with the frame specification §11.1 amendment
  > (#A): binding version 1, `frame_header_max_bytes` 20 (the `response` header
  > without its envelope), `envelope_bytes` 16 (`stamp` 8 + `seq` 8). Every
  > deployment section now carries it, and the cabin layout fixture's
  > `max_message_bytes` for `Cabin.warning` is 20 + 16 + 8 = 44. That is the
  > first half of this issue. The second half, a test that checks the sum
  > against a frame the transport writes, lands with #265; this issue stays open
  > until then.

  **#336:**

  > Ruling for 0.11 (R-W-18 of `docs/wip/2026-10-12-ws-binding-design.md`,
  > applied in the frame specification §11.1 "Later versions" and §13 by #A):
  > streams arrive as frame version 2, carried by binding version 2. A 0.11
  > client offers `ridl-ws-v2, ridl-ws-v1`; a 0.10 server selects `ridl-ws-v1`,
  > and the client sends `attach` with `frame_version` 1 and uses no stream form
  > on that session. A 0.11 server selects `ridl-ws-v2`; a 0.10 client, which
  > offers `ridl-ws-v1` only, gets `ridl-ws-v1` as before. Version 1's layout,
  > codes and vectors are never edited. A 0.11 consumer of a contract that
  > declares a stream, given version 1, reports a call that uses the stream
  > through a new `Transport` variant (additive: `Transport` is
  > `#[non_exhaustive]`), not as `UnknownInteraction`, and uses the rest of the
  > contract. No 0.10 surface breaks.

---

## Pull request B — `Violation.type_name` becomes optional

Set up after pull request A is merged:

```bash
cd /Users/sebastientasson/Workspace/driftsys/ridl
git fetch origin
git worktree add -b feat/violation-peer-no-name .claude/worktrees/lane-w-ws-b origin/main
cd .claude/worktrees/lane-w-ws-b && ./bootstrap
git rev-parse HEAD   # record as BASE_B0
```

Coordination: ADR-0021 is also rewritten by the language-and-codegen work that
follows this design; that rewrite starts after this pull request merges and
carries this amendment through.

### Task B1: The `ridl-rt` type and its records

**Files:**

- Modify: `crates/ridl-rt/src/payload.rs:240-268`
- Modify: `crates/ridl-rt/src/sample.rs:300-302` (test)
- Modify: `crates/ridl-rt/tests/payload.rs:24-27`,
  `crates/ridl-rt/tests/read_sample.rs:87-89`,
  `crates/ridl-rt/examples/read_sample.rs:64-66`
- Modify: `docs/design/ridl-rt.md:411`
- Modify: `docs/decisions/ADR-0021-ridl-rt-0.1-api-and-release.md` (Status;
  decision 9 after the 2026-10-02 paragraph; "Documents amended" table)
- Modify: `docs/specification/frame-specification.md` (§2 `Violation` row, line
  117; §9.6 last paragraph)
- Modify: `docs/technotes/ridl-rt-by-example.md:361-363`

**Interfaces:**

- Produces:
  `pub struct Violation { pub type_name: Option<&'static str>, pub rule: Rule }`
  in `ridl_rt::payload`. Task B2's emitted code writes
  `type_name: ::core::option::Option::Some(<literal>)`.

- [ ] **Step 1: Write the failing test**

  In `crates/ridl-rt/tests/payload.rs`, change `SPEED_RANGE` to
  `type_name: Some("Speed"),` and add:

  ```rust
  /// A violation a peer reports over a frame carries the rule alone (frame
  /// specification §9.6); one the generated code detects keeps its type name.
  #[test]
  fn a_violation_a_peer_reports_carries_the_rule_alone() {
      let reported = Violation {
          type_name: None,
          rule: Rule::Range,
      };
      assert_eq!(reported.type_name, None);
      assert_eq!(SPEED_RANGE.type_name, Some("Speed"));
      assert_ne!(reported, SPEED_RANGE);
      assert_eq!(reported.rule, SPEED_RANGE.rule);
  }
  ```

- [ ] **Step 2: Run the test to verify it fails**

  Run: `cargo test -p ridl-rt --test payload` Expected: FAIL to compile with
  E0308 "expected `&str`, found `Option<_>`" at `SPEED_RANGE` and in the new
  test. A type change has no assertion-level red; the compile error is the red.

- [ ] **Step 3: Change the type**

  In `crates/ridl-rt/src/payload.rs`:

  ```rust
  /// A value that breaks a typl constraint.
  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  pub struct Violation {
      /// The name of the typl type whose constraint failed: `Some` when the
      /// generated code detected the violation locally, `None` when a peer
      /// reported it, because a frame carries the [`Rule`] alone (the frame
      /// specification §9.6).
      pub type_name: Option<&'static str>,
      /// The kind of constraint that failed.
      pub rule: Rule,
  }
  ```

  On `Rule`'s doc comment, append: "A variant is added only together with a new
  frame version, because a receiver cannot construct a variant it does not know
  (the frame specification §9.6); the enum stays `#[non_exhaustive]`."

  Update the three other `ridl-rt` sites to `type_name: Some("Speed"),`:
  `src/sample.rs` line 301, `tests/read_sample.rs` line 88,
  `examples/read_sample.rs` line 65.

- [ ] **Step 4: Run the tests to verify they pass**

  Run:
  `cargo test -p ridl-rt --all-features && cargo test -p ridl-rt --no-default-features && cargo test -p ridl-loopback -p ridl-rt-conformance`
  Expected: PASS; the loopback and conformance crates compile unchanged.

- [ ] **Step 5: Update the records**

  - `docs/design/ridl-rt.md` line 411:
    `pub struct Violation { pub type_name: Option<&'static str>, pub rule: Rule }`.
  - `docs/specification/frame-specification.md` (R-P-2): the §2 `Violation`
    row's meaning cell becomes the design's section 3.1 "§2, the `Violation`
    row" text; in the §9.6 paragraph **What a violation carries.**, insert the
    sentence "The receiving runtime reports the violation with no type name,
    which `ridl_rt::payload::Violation` represents as `type_name: None`; a
    violation the generated binding detects locally keeps its name." between
    "...a variant it does not know." and "A type identifier was rejected...".
  - `docs/technotes/ridl-rt-by-example.md` lines 361-363 (R-P-7): "A `Violation`
    names the typl type and which rule it broke" becomes "A `Violation` that the
    generated code detects names the typl type and which rule it broke", and
    after "...rather than that something somewhere was wrong." add "A violation
    a peer reports over a frame carries the rule alone, and its `type_name` is
    `None`."
  - ADR-0021 (R-P-9), dated with the commit's day (`date +%F`):
    - `## Status`, after the last "**Amendment (...)**" paragraph: "**Amendment
      (<date>) — decision 9: a violation a peer reports carries no type name.**
      `payload::Violation.type_name` becomes `Option<&'static str>`: `Some` when
      the generated code detects the violation locally, `None` when a peer
      reports it, because the frame carries only the `Rule` of a
      `contract(InvalidValue)` outcome (the frame specification §9.6) and a
      `no_std` runtime cannot make a `&'static str` from received bytes. Under
      decision 10 the change is breaking: `Violation` is a struct whose fields
      are all public. A `Rule` variant is added only together with a new frame
      version, although `Rule` is `#[non_exhaustive]`. driftsys/ridl#350 item
      12."
    - Decision 9, after the "**Extended (2026-10-02, driftsys/ridl#469).**"
      paragraph: "**Amended (<date>, driftsys/ridl#350 item 12).** A `Rule`
      variant is added only together with a new frame version, because a
      receiver cannot construct a variant it does not know (the frame
      specification §9.6). `#[non_exhaustive]` stays, so that a match outside
      the crate keeps its fallback arm. `Violation.type_name` is
      `Option<&'static str>`, `None` for a violation a peer reports."
    - "Documents amended" table, two rows:

      ```markdown
      | [the `ridl-rt` design record](../design/ridl-rt.md), the `payload` item list           | `Violation.type_name` is `Option<&'static str>` (decision 9's <date> amendment)                          |
      | [the frame specification](../specification/frame-specification.md) §2 and §9.6 | a violation a peer reports carries the `Rule` alone and has no type name (decision 9's <date> amendment) |
      ```

  Run:
  `prim fmt docs/design/ridl-rt.md docs/decisions/ADR-0021-ridl-rt-0.1-api-and-release.md docs/specification/frame-specification.md docs/technotes/ridl-rt-by-example.md && prim lint docs/design/ridl-rt.md docs/decisions/ADR-0021-ridl-rt-0.1-api-and-release.md docs/specification/frame-specification.md docs/technotes/ridl-rt-by-example.md`
  Expected: exit 0.

- [ ] **Step 6: Run the task gates**

  Run:
  `cargo fmt --all --check && cargo clippy -p ridl-rt -p ridl-loopback -p ridl-rt-conformance --all-targets --all-features -- -D warnings`
  Expected: exit 0. (`ridl-backend-rust`'s tests do not compile until Task B2;
  do not run the workspace tests here.)

  Run, in order:
  `just freeze-check && just link-check && just doc-path-check && just story-id-check && just book-check && just check`
  Expected: every recipe exits 0. The emitter is unchanged, so the generated
  crates are unchanged.

- [ ] **Step 7: Commit**

  ```bash
  git add crates/ridl-rt docs/design/ridl-rt.md docs/decisions/ADR-0021-ridl-rt-0.1-api-and-release.md docs/specification/frame-specification.md docs/technotes/ridl-rt-by-example.md
  git commit -m "feat(ridl-rt)!: report a violation that a peer sends with no type name

  Violation.type_name becomes Option<&'static str>: Some when the generated
  code detects the violation, None when a peer reports it, because a frame
  carries the Rule of a contract(InvalidValue) outcome alone and a no_std
  runtime cannot make a &'static str from received bytes. A Rule variant is
  added only together with a new frame version. ADR-0021, the ridl-rt design
  record and the frame specification sections 2 and 9.6 record it.

  BREAKING CHANGE: ridl_rt::payload::Violation.type_name is
  Option<&'static str>. Code that builds a Violation wraps the name in Some;
  code that reads it handles None.

  Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
  ```

### Task B2: The Rust backend's emitted code and every regenerated output

**Files:**

- Modify: `crates/ridl-backend-rust/src/lib.rs` lines 1300, 1311, 1332, 1374,
  1401, 1412, 1457, 1766, 1872; `crates/ridl-backend-rust/src/codec.rs` lines
  2055, 2847
- Modify: `crates/ridl-backend-rust/src/tests.rs:3324`,
  `crates/ridl-backend-rust/tests/flatbuffers_roundtrip.rs:640,657,706`,
  `crates/ridl-backend-rust/tests/interaction_face.rs:1045,2728,2740,2769`
- Regenerate: `crates/ridl-backend-rust/src/snapshots/*.snap`,
  `crates/ridl-backend-rust/tests/generated/interaction_face.rs`,
  `crates/ridlc/tests/snapshots/*.snap`, `freeze/generated/cabin/*`,
  `freeze/generated/veh-cluster/*`

**Interfaces:**

- Consumes: Task B1's
  `Violation { type_name: Option<&'static str>, rule: Rule }`.
- Produces: generated code that writes
  `type_name: ::core::option::Option::Some("<TypeName>")`.

- [ ] **Step 1: Write the failing tests**

  - `tests/flatbuffers_roundtrip.rs`: line 640
    `assert_eq!(violation.type_name, Some("Speed"));`, line 657
    `assert_eq!(violation.type_name, Some("Label"));`, line 706
    `assert_eq!(violation.type_name, Some(type_name), "{case}: wrong type");`.
  - `tests/interaction_face.rs`: lines 1045, 2728, 2740, 2769
    `type_name: Some("Level"),`.
  - `src/tests.rs` line 3324, inside the hand-written source string:
    `type_name: ::core::option::Option::Some("Speed"),`.

- [ ] **Step 2: Run the tests to verify they fail**

  Run:
  `cargo test -p ridl-backend-rust --test flatbuffers_roundtrip --test interaction_face`
  Expected: FAIL to compile with E0308 inside the generated code, "expected
  `Option<&str>`, found `&str`" (the emitter still writes a bare literal).

- [ ] **Step 3: Change the emitter**

  ```bash
  sed -i '' 's/type_name: #type_name,/type_name: ::core::option::Option::Some(#type_name),/' crates/ridl-backend-rust/src/lib.rs
  sed -i '' 's/type_name: #owner/type_name: ::core::option::Option::Some(#owner)/' crates/ridl-backend-rust/src/codec.rs
  grep -c 'Option::Some(#type_name)' crates/ridl-backend-rust/src/lib.rs   # expect 9
  grep -c 'Option::Some(#owner)' crates/ridl-backend-rust/src/codec.rs     # expect 2
  ```

  `PayloadInfo { type_name: #type_name }` in `src/descriptors.rs` is a different
  struct and stays.

- [ ] **Step 4: Regenerate the fixtures and snapshots, then review them**

  ```bash
  RIDL_UPDATE_GENERATED=1 cargo test -p ridl-backend-rust --test interaction_face_regeneration
  INSTA_UPDATE=always cargo test -p ridl-backend-rust -p ridlc
  cargo xtask freeze update
  git diff --stat
  ```

  Expected: only `.snap` files, `tests/generated/interaction_face.rs` and
  `freeze/generated/**` change besides the files of Steps 1 and 3. Run
  `git diff -U0 -- '*.snap' crates/ridl-backend-rust/tests/generated freeze/generated | grep '^[-+] ' | grep -v 'type_name' | head`
  Expected: no output — every changed line is a `type_name` line. A
  `PayloadInfo { type_name: "..." }` line must not change; if one does, stop.

- [ ] **Step 5: Run the tests to verify they pass**

  Run: `cargo test --workspace --locked` Expected: PASS.

  Run:
  `grep -rn 'type_name: "' crates/ridl-rt crates/ridl-backend-rust/tests crates/ridl-backend-rust/src/tests.rs | grep -v PayloadInfo`
  Expected: no output outside `PayloadInfo` literals.

- [ ] **Step 6: Run the task gates**

  Run: `cargo fmt --all --check && just lint` Expected: exit 0.

  Run, in order:
  `just freeze-check && just link-check && just doc-path-check && just story-id-check && just book-check && just check && just compat-check && just demo`
  Expected: every recipe exits 0 (`demo` lints the generated crates with clippy
  under `-D warnings`; `compat-check` builds the generated cabin crate at the
  minimum Rust version).

- [ ] **Step 7: Commit**

  ```bash
  git add crates/ridl-backend-rust crates/ridlc/tests/snapshots freeze/generated
  git commit -m "feat(ridl-backend-rust): name the type of a locally detected violation with Some

  The generated constraint checks and codecs write
  type_name: Some(\"<Type>\") into ridl_rt::payload::Violation, which is
  Option<&'static str>. The snapshots, the interaction face fixture and the
  frozen generated crates are regenerated.

  Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
  ```

### Task B3: Verify, push and open pull request B

- [ ] **Step 1: Run the whole gate**

  Run: `just verify` Expected: exit 0.

- [ ] **Step 2: Push and open the pull request**

  ```bash
  git push -u origin feat/violation-peer-no-name
  gh pr create --base main --title "feat(ridl-rt)!: report a violation that a peer sends with no type name" --body-file <body>
  ```

  The body states the breaking change and its migration (wrap a built name in
  `Some`, handle `None` when reading), that `ridl-loopback` and
  `ridl-rt-conformance` needed no change, that no `surfaces.toml` row exists for
  `runtime-crates` or `generated-crate` yet, and "Refs #350". No closing
  keyword. It ends with
  `🤖 Generated with [Claude Code](https://claude.com/claude-code)`.

  Run:
  `gh pr view --json body -q .body | grep -inE '(close|fix|resolve)[sd]? #'`
  Expected: no output.

- [ ] **Step 3: After the merge, the controller posts these**

  Replace `#B` with pull request B's number.

  **#350** (and tick item 12 in the issue body):

  > Item 12 is done in #B: `ridl_rt::payload::Violation.type_name` is
  > `Option<&'static str>`. Generated code writes `Some("<Type>")` for a
  > violation it detects; a runtime reports `None` for a violation a peer sends,
  > because the frame carries the `Rule` code alone (frame specification §9.6).
  > A new `Rule` variant comes only with a new frame version.

  **A heads-up issue on driftsys/ridlc-gen-kotlin:**

  > Title: `ridl-rt`: `Violation.type_name` is now optional
  >
  > driftsys/ridl#B changed `ridl_rt::payload::Violation.type_name` from
  > `&'static str` to `Option<&'static str>` (a breaking change, in the next
  > `ridl-rt` minor release). `None` means a peer reported the violation: over a
  > frame, `contract(InvalidValue)` carries the `Rule` code alone (frame
  > specification §9.6: `Range` 1, `Step` 2, `Length` 3, `Pattern` 4, `Variant`
  > 5, `Unique` 6). A violation that generated code detects locally keeps its
  > type name. If the Kotlin runtime or the generated Kotlin code mirrors
  > `Violation`, a nullable type name keeps the two in parity. A new `Rule`
  > variant now comes only together with a new frame version.

---

## Self-review record

- Spec coverage: section 3.1 → A1 (and B1 for the two `Violation` pieces);
  section 3.2 → ADR-0020 A4, ADR-0021 B1, `ridl-rt.md` B1, §2 row B1, steering H
  A4, `bindings.rs` A3, `codegen-plugins.md` design and book A3,
  `freeze/README.md` A2, `CONTRIBUTING.md` A2, overview A4, ADR-0029 and
  `stability.md` A2, `catalog-descriptor.md` A4, ROADMAP A4, issues A5 and B3;
  section 4 → A2; R-W-14's code → B1 and B2.
- Numbers: 20, 16, 44, 39 + name, 294, the nine vectors and their lengths (48,
  3, 3, 10, 12, 29, 27, 36, 36) were recomputed from the field widths.
- Names used across tasks: `frame_vectors`, `check_vector`, `FRAME_SPEC`,
  `KNOWN`, `Known`, `v1::Binding`, `Violation`, `Rule` — one spelling each.
