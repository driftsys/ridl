# Release Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** `just release` leaves a release commit that builds and passes CI, and
the tags are pushed only after CI on that commit has passed.

**Architecture:** `git std bump --no-commit` writes the version, the
`[[version_files]]` pins and the changelog. A new `release-sync` recipe then
updates the files that a `version_files` regex cannot express: the lockfiles,
the `ridl-rt` requirement that `ridl build` emits, the cabin example, the editor
manifest and the frozen generated manifests. A new test fails when one of those
files does not follow the workspace version. A new `release-push` recipe pushes
`main`, waits for the `ci` run on the release commit, and pushes the two tags
only if that run passed. The crates.io publish job waits for each crate to
appear in the index instead of sleeping for a fixed 30 seconds, and the pre-push
hook skips the gate on a push that holds only tags.

**Tech Stack:** just, bash, git-std 0.11.19, cargo, `gh`, Rust integration tests
in `xtask/tests/`.

**Spec:** this plan. The analysis it implements is the 0.14.0 post-mortem: the
release commit left these at 0.13 — `Cargo.lock`, `examples/cabin/Cargo.lock`,
`RIDL_RT_REQUIREMENT` in `crates/ridlc/src/lib.rs`, the cabin manifests,
`freeze/generated/*/Cargo.toml`, `examples/cabin/web/deno.lock` and
`editors/vscode/package.json`. The tags were pushed before CI on `main` had run,
so each fix needed the tags moved. crates.io's index stopped updating for about
4 hours and the publish job failed on the first dependent crate.

## Design decisions

- **Sync after the bump, not more `version_files`.** `Cargo.lock` has one entry
  per crate and `version_files` rewrites only the first match, so cargo has to
  update it. `RIDL_RT_REQUIREMENT` holds `<major>.<minor>`, while
  `version_files` writes the full version.
- **Alternatives considered:**
  - (a) Move `RIDL_RT_REQUIREMENT` to the full version so `version_files` covers
    it. Rejected: it changes what every generated crate requires on a patch
    release, and with it the frozen generated manifests.
  - (b) Amend the commit that `git std bump` creates. Rejected: `--no-commit`
    exists, so the files are written, synced and committed once.
  - (c) A release branch with a PR. Rejected: the maintainer pushes the release
    commit to `main` directly. The `main` ruleset allows it for organization
    admins since 2026-10-25.
- **The editor version follows the workspace version.** `editor-v0.13.0` and
  `editor-v0.14.0` matched `v0.13.0` and `v0.14.0`, and `vscode-release.yaml`
  refuses a tag that differs from `editors/vscode/package.json`.
- **Out of scope:** npm and crates.io trusted publishing, the `OVSX_PAT` secret,
  and a credentials preflight. Each one needs the maintainer's registry account.

## Global Constraints

- Prose (comments, commit messages, docs) is plain and literal English, with no
  idioms.
- No story id and no plan name in a tracked file under `xtask/` or
  `docs/technotes/` (`just story-id-check`).
- Conventional Commits with a scope from `.git-std.toml`: `repo`, `ci`, `hooks`,
  `xtask`.
- The workflow holds no gate command (ADR-0009). The index wait is publish
  plumbing, so it lives in a script that the workflow calls.
- The new recipes are not members of `build`, so `gate-parity` does not change.
- Release commit message: `chore(release): <version>`. Tags: annotated
  `v<version>` (message `v<version>`) and annotated `editor-v<version>` (message
  `editor-v<version>`), both on the release commit.

## Review Focus

1. A patch release (0.14.0 → 0.14.1): `RIDL_RT_REQUIREMENT` and the cabin
   requirements stay `0.14`, and `freeze update` changes nothing. Pinned by Task
   1's `patch_release_keeps_the_major_minor_requirements` test.
2. A re-run of the publish job, where a crate is already uploaded but not yet in
   the index: the skip path must also wait for the index. Pinned in Task 3 (the
   wait follows both paths).
3. A push of `main` together with a tag in one `git push`: the hook must run the
   gate. Pinned in Task 4 (`mixed_push_runs_the_gate`).
4. `release-push` run when HEAD is not a release commit, or when the tags are
   missing or point elsewhere: it must stop before pushing anything. The checks
   run before the first push in Task 2.
5. The `ci` run for the release commit does not exist yet when `release-push`
   looks for it: wait for it to appear, do not fail at once. Task 2 polls for up
   to 5 minutes.

---

### Task 1: A test that every derived pin follows the workspace version

**Files:**

- Create: `xtask/tests/release_derived_pins.rs`

**Interfaces:**

- Produces: `fn derived_pin_errors(root: &Path, version: &str) -> Vec<String>`,
  one message per stale pin, each naming the file and the value found. The
  `#[test] fn derived_pins_follow_the_workspace_version()` calls it on the
  repository root with the version of `ridl-core` from
  `cargo metadata --no-deps`, and fails with all the messages joined.

Checks, with `mm` = `<major>.<minor>` of `version`:

| File                                                                           | Pin                                                                              |
| ------------------------------------------------------------------------------ | -------------------------------------------------------------------------------- |
| `Cargo.lock`                                                                   | every `[[package]]` whose name is a workspace member has `version = "<version>"` |
| `examples/cabin/Cargo.lock`                                                    | the entries `ridl-rt`, `ridl-loopback`, `ridl-transport-ws` have `<version>`     |
| `crates/ridlc/src/lib.rs`                                                      | `const RIDL_RT_REQUIREMENT: &str = "<mm>";`                                      |
| `examples/cabin/consumer/Cargo.toml`, `examples/cabin/server/Cargo.toml`       | `ridl-rt = { version = "<mm>"`                                                   |
| `examples/cabin/Cargo.toml`                                                    | the comment `ridl-rt = "<mm>"`                                                   |
| `freeze/generated/cabin/Cargo.toml`, `freeze/generated/veh-cluster/Cargo.toml` | `ridl-rt = { version = "<mm>"`                                                   |
| `examples/cabin/web/deno.lock`                                                 | `jsr:@driftsys/ridl-runtime@<version>`                                           |
| `editors/vscode/package.json`                                                  | top-level `"version": "<version>"`                                               |
| `editors/vscode/package-lock.json`                                             | the root `"version"` and `packages[""].version` are `<version>`                  |

Each check fails also when its pattern is not found, so a renamed file or a
rewritten line cannot pass in silence.

- [ ] **Step 1: Write the unit tests over a synthetic tree** (in the same file,
      as `release_publish_list.rs` does): build a temporary directory with
      minimal versions of the files above.
  - `all_pins_current_gives_no_error`: every pin at `0.15.0`/`0.15` →
    `derived_pin_errors(dir, "0.15.0")` is empty.
  - `stale_pins_are_each_reported`: the editor `package.json` at `0.14.0`, the
    cabin consumer requirement at `0.14`, a workspace crate in `Cargo.lock` at
    `0.14.0` → exactly 3 messages, each containing its file path.
  - `missing_pattern_is_reported`: `deno.lock` with no `ridl-runtime` entry →
    one message naming `examples/cabin/web/deno.lock`.
  - `patch_release_keeps_the_major_minor_requirements`: requirements at `0.14`,
    versions at `0.14.1` → `derived_pin_errors(dir, "0.14.1")` is empty.
- [ ] **Step 2: Run** `cargo test -p xtask --test release_derived_pins`.
      Expected: it fails to compile, because `derived_pin_errors` is not
      defined.
- [ ] **Step 3: Implement `derived_pin_errors`.** Use plain string and regex
      matching (`regex` is already a dependency of `xtask`; check
      `xtask/Cargo.toml` and use `serde_json` for the two editor files if it is
      there). Do not add a TOML parser for `Cargo.lock`: scan `name = "…"`
      followed by `version = "…"` lines.
- [ ] **Step 4: Run** `cargo test -p xtask --test release_derived_pins`.
      Expected: PASS, including `derived_pins_follow_the_workspace_version` on
      the real tree at 0.14.0.
- [ ] **Step 5: Mutation check.** Change `examples/cabin/consumer/Cargo.toml` to
      `0.13`, run the test, confirm that it fails and names that file, then
      restore the file.
- [ ] **Step 6: Commit**
      `test(xtask): check that the derived version pins
  follow the workspace version`.

### Task 2: `release` syncs the derived pins; `release-push` keeps the order

**Files:**

- Modify: `justfile` (the `release` recipe at about line 3233; add
  `release-sync` and `release-push` next to it)
- Modify: `CONTRIBUTING.md` (the `just release` row at about line 291; add a
  `just release-push` row)
- Modify: `AGENTS.md` (the `just release` entry in the command list; add
  `just release-push`)
- Modify: `docs/technotes/toolchain-distribution.md` (add a section
  `## Cutting a release` after the two-trains section)

**Interfaces:**

- Consumes: Task 1's test, run by name.
- Produces: `just release *ARGS`, `just release-sync`, `just release-push`.

`release *ARGS: toolchain-check`, in this order. It stops at the first failure:

1. Refuse unless the branch is `main`, the tree is clean, and `HEAD` equals
   `origin/main` after `git fetch origin main`. Print the open PRs
   (`gh pr list --base main --state open`) for information only.
2. `cargo run --locked --package xtask -- freeze check --release` (unchanged).
3. `git std bump --no-commit {{ARGS}}`. `ARGS` carries `--release-as`. Before
   1.0, `git std bump` picks a patch release unless it is told otherwise.
4. `just release-sync`.
5. `cargo check --workspace --locked`, then
   `cargo test --locked -p xtask --test release_derived_pins`, then
   `cargo test --locked -p ridlc --test prose_version_pins --test rust_crate_emit`.
6. `git add -u`, `git commit -m "chore(release): $version"`,
   `git tag -a "v$version" -m "v$version"`,
   `git tag -a "editor-v$version" -m "editor-v$version"`.
7. Print the next command: `just release-push`.

`release-sync` (bash; `version` read from `cargo metadata --no-deps` for
`ridl-core`, `mm` its `<major>.<minor>`):

1. Rewrite `RIDL_RT_REQUIREMENT` and its doc comment line in
   `crates/ridlc/src/lib.rs`, the `ridl-rt` requirement in the cabin consumer
   and server manifests, and the comment in `examples/cabin/Cargo.toml` to `mm`.
   Use `sed -i.bak` and remove the `.bak` files, so the recipe works with BSD
   sed and GNU sed.
2. Rewrite `jsr:@driftsys/ridl-runtime@<old>` in `examples/cabin/web/deno.lock`
   to `version`.
3. `(cd editors/vscode && npm version "$version" --no-git-tag-version --allow-same-version)`.
4. `cargo update --workspace --offline` at the root.
5. `just _cabin-generate`, then
   `(cd examples/cabin && cargo update --workspace --offline)`. The generated
   manifest has to carry the new requirement first, or cargo looks for the old
   `ridl-rt` on crates.io.
6. `cargo run --locked --package xtask -- freeze update`.

`release-push`:

1. Refuse unless `HEAD` is on `main`, its subject is
   `chore(release): <version>`, and `v<version>` and `editor-v<version>` exist
   locally and resolve to `HEAD`.
2. `git push origin HEAD:main`. The pre-push gate runs here.
3. Poll `gh run list --workflow ci --commit "$sha" --json databaseId` every 10
   seconds, for up to 5 minutes, until the run exists. Then run
   `gh run watch "$id" --exit-status`. On a failure, print the run URL and stop.
   The tags stay local.
4. `git push origin "v$version" "editor-v$version"`. Task 4 makes the hook skip
   the gate for this push.
5. Print the three release workflows to watch, and that the editor's Marketplace
   job waits for approval in the `marketplace` environment.

- [ ] **Step 1: Write `release-sync`, then do a dry run in a throwaway
      worktree:** `git worktree add /tmp/…/rel-dry origin/main`, run
      `./bootstrap` there, then
      `git std bump --no-commit --release-as 0.15.0 && just release-sync`.
- [ ] **Step 2: Verify the dry run:**
      `cargo test --locked -p xtask --test release_derived_pins`,
      `cargo check --workspace --locked`, and
      `cargo test --locked -p ridlc --test prose_version_pins --test rust_crate_emit`
      all pass, and `git diff --stat` lists every file in Task 1's table plus
      `Cargo.toml` and `CHANGELOG.md`. Also run Step 1 with
      `--release-as 0.14.1` on a fresh worktree: `crates/ridlc/src/lib.rs` and
      `freeze/generated/` must not change. Remove both worktrees.
- [ ] **Step 3: Rewrite `release` and add `release-push`** as listed above.
      Check the guards of `release-push` by running it on a branch that is not
      `main` (expected: refused, nothing pushed), and on `main` whose `HEAD` is
      not a release commit (expected: refused). Do not run it to completion.
- [ ] **Step 4: Documentation.** In `toolchain-distribution.md`,
      `## Cutting a release` states the order:
      `just release --release-as X.Y.Z`, review the changelog,
      `just release-push`. It also states that a tag is never moved after a
      publish workflow has started, and that a re-run of a tag uses the tagged
      commit's workflow file. Update the `CONTRIBUTING.md` rows and the
      `AGENTS.md` command list to match.
- [ ] **Step 5:** Run `just fmt`, `just check`, `just link-check`,
      `just doc-path-check` and `just story-id-check`. Expected: all pass.
- [ ] **Step 6: Commit**
      `fix(repo): sync the derived version pins in just
  release and push tags after CI passes`.

### Task 3: The crates.io publish waits for the index

**Files:**

- Create: `.github/scripts/wait-for-crate-index.sh`
- Create: `xtask/tests/crate_index_wait.rs`
- Modify: `.github/workflows/crates-io-release.yml` (the `publish()` function;
  replace `sleep 30`)

**Interfaces:**

- Produces: `wait-for-crate-index.sh <crate> <version>`. Environment:
  `CRATES_INDEX_URL` (default `https://index.crates.io`), `INDEX_WAIT_TIMEOUT`
  (seconds, default `1800`), `INDEX_WAIT_INTERVAL` (seconds, default `15`). Exit
  0 when a line of the crate's index file holds `"vers":"<version>"`. Exit 1
  with `<crate> <version> is not in the index after <timeout> s` otherwise.
- Index path (sparse protocol, name lowercased): length 1 → `1/<n>`; 2 →
  `2/<n>`; 3 → `3/<first char>/<n>`; 4 or more → `<chars 1-2>/<chars 3-4>/<n>`.
  Example: `ridl-ir` → `ri/dl/ridl-ir`.

- [ ] **Step 1: Write the tests.** They run the script with bash, with
      `CRATES_INDEX_URL=file://<tempdir>` and `INDEX_WAIT_INTERVAL=1`:
  - `present_version_returns_at_once`: `ri/dl/ridl-ir` holds a line with
    `"vers":"0.15.0"` → exit 0.
  - `absent_version_times_out`: the file holds only `0.14.0`, with
    `INDEX_WAIT_TIMEOUT=2` → exit 1, and stderr contains
    `ridl-ir 0.15.0 is not in the index`.
  - `short_names_use_the_short_paths`: fixtures at `1/a`, `2/ab`, `3/a/abc` →
    exit 0 for each.
  - `missing_file_is_waited_for_not_fatal`: no file, with `INDEX_WAIT_TIMEOUT=2`
    → exit 1 with the same message, not a curl error.
- [ ] **Step 2: Run** `cargo test -p xtask --test crate_index_wait`. Expected:
      FAIL, because the script does not exist.
- [ ] **Step 3: Write the script** (`set -euo pipefail`; `curl -fsS`, where a
      failure counts as "not yet").
- [ ] **Step 4: Run the tests.** Expected: PASS.
- [ ] **Step 5: Wire it into the workflow.** In `publish()`, call
      `.github/scripts/wait-for-crate-index.sh "$crate" "$pkg_version"` after a
      successful publish, replacing `sleep 30`, and also on the "already
      published, skipping" path. Update the comment above the function: it says
      the function sleeps 30 seconds. Check that
      `cargo test -p xtask --test release_publish_list` still passes.
- [ ] **Step 6: Commit**
      `fix(ci): wait for each crate to appear in the
  crates.io index before publishing the next`.

### Task 4: The pre-push hook skips the gate on a push of tags only

**Files:**

- Create: `.githooks/pre-push-gate.sh` (executable)
- Modify: `.githooks/pre-push.hooks` (replace `! just pre-push` with
  `! .githooks/pre-push-gate.sh`)
- Create: `xtask/tests/pre_push_gate.rs`

**Interfaces:**

- Produces: `pre-push-gate.sh`. It reads git's pre-push lines from stdin
  (`<local ref> <local sha> <remote ref> <remote sha>`; git-std forwards them).
  It exits 0 without running anything when there are no lines, or when every
  remote ref starts with `refs/tags/`. Otherwise it runs `just pre-push` and
  returns its exit code.

- [ ] **Step 1: Write the tests.** They run the script with `PATH` starting with
      a temporary directory that holds a fake `just`, which writes a marker file
      and exits 7:
  - `tag_only_push_skips_the_gate`: stdin
    `refs/tags/v0.15.0 <sha> refs/tags/v0.15.0 <zeros>` → exit 0, no marker.
  - `tag_deletion_skips_the_gate`: stdin
    `(delete) <zeros> refs/tags/v0.15.0 <sha>` → exit 0, no marker.
  - `branch_push_runs_the_gate`: stdin
    `refs/heads/main <sha> refs/heads/main <sha>` → exit 7, marker present.
  - `mixed_push_runs_the_gate`: one tag line and one branch line → exit 7.
  - `empty_push_skips_the_gate`: empty stdin → exit 0, no marker.
- [ ] **Step 2: Run** `cargo test -p xtask --test pre_push_gate`. Expected:
      FAIL.
- [ ] **Step 3: Write the script.**
- [ ] **Step 4: Run the tests.** Expected: PASS.
- [ ] **Step 5: Manual check.** Create a lightweight tag on `HEAD` in the
      worktree and push it to a local bare repository with this hook active
      (`git init --bare /tmp/…/remote.git`, push to its path). The hook must
      return at once. Delete the bare repository.
- [ ] **Step 6: Commit**
      `fix(hooks): skip the pre-push gate when a push holds
  only tags`.

### Finish

- [ ] Run `just verify` once on the branch, or let CI run the full gate (see the
      memory note "Let CI run full gates").
- [ ] Garden: move this plan to `docs/archive/`. The durable record is
      `docs/technotes/toolchain-distribution.md`, section "Cutting a release".
- [ ] Open one PR, `ci/release-hardening` → `main`.
- [ ] File in driftsys/git-std: (a) `bump` updates `Cargo.lock` for a Cargo
      workspace; (b) a post-bump hook, so `release-sync` can run inside `bump`.
