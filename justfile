# RIDL — task runner.
#
# Recipe set follows the driftsys house style (git-std, prim). The repo
# holds the specifications, ADRs, roadmap, and the Cargo workspace
# (docs/ROADMAP.md, epic E0). `check` gates the docs, `compile` and
# `test` cover the Rust workspace, and `build` runs the whole gate. The
# mdBook docs are served with `just book` and built by `just book-check`.
#
# This file is the single definition of every gate command. CI
# (.github/workflows/ci.yml) installs the tools a runner needs and then invokes
# these recipes; it does not restate their commands. ADR-0009 records why:
# every time the two sides held their own copy of a command, the copies drifted
# and a locally clean tree failed CI. `gate-parity` guards the remaining half of
# that — that CI still invokes every recipe `build` depends on.

set shell := ["bash", "-euo", "pipefail", "-c"]

# List recipes (default — hidden from the listing itself).
[private]
default:
    @just --list

# Reformat the connective tissue (Markdown/JSON/YAML/TOML) in place with prim,
# then auto-fix Markdown style findings. Exclusions live in .markdownlintignore,
# which markdownlint auto-detects — see that file for why not `--ignore`.
fmt:
    prim .
    markdownlint '**/*.md' --fix

# Lint gate — no writes: prim --check (formatting) + markdownlint (style).
check:
    prim --check .
    markdownlint '**/*.md'

# Check that the running Rust toolchain is the one rust-toolchain.toml pins.
#
# rustup applies that file per directory, so with rustup installed every cargo
# invocation below is already the pinned version. Without rustup the file is
# read by nobody and ignored without a word, and `cargo fmt --all --check` then
# measures whatever rustfmt the contributor happens to have — the check reports
# green against a different standard than CI applies. A `RUSTUP_TOOLCHAIN` in
# the environment or a `rustup override set` in this directory does the same
# thing with rustup present. One version comparison catches all three.
toolchain-check:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ ! -f rust-toolchain.toml ]; then
        echo "toolchain-check: rust-toolchain.toml is missing — the pin is the gate." >&2
        exit 1
    fi
    pinned="$(sed -n 's/^channel[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' rust-toolchain.toml)"
    if [ -z "$pinned" ]; then
        echo "toolchain-check: rust-toolchain.toml names no channel." >&2
        exit 1
    fi
    if ! command -v rustc >/dev/null 2>&1; then
        echo "toolchain-check: rustc is required and is not on PATH." >&2
        echo "toolchain-check: install rustup (https://rustup.rs); it applies the pin." >&2
        exit 1
    fi
    running="$(rustc --version | cut -d' ' -f2)"
    if [ "$running" != "$pinned" ]; then
        echo "toolchain-check: rust-toolchain.toml pins $pinned, rustc reports $running." >&2
        echo "toolchain-check: install rustup, or clear the RUSTUP_TOOLCHAIN variable or" >&2
        echo "toolchain-check: the 'rustup override' that is outranking the pin." >&2
        exit 1
    fi
    echo "toolchain-check: rustc $running matches the pin."

# Compile the Rust workspace (the Cargo.toml guard is a defensive
# fallback for partial checkouts, not a "lands later" gate).
#
# `--locked` refuses to rewrite Cargo.lock. Without it a manifest change that
# leaves the lockfile stale is repaired silently on the contributor's machine
# and rejected on a CI runner, which is the shape of failure ADR-0009 exists to
# remove. Run `cargo build --workspace` (no flag) to update the lockfile on
# purpose, then commit it.
compile:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -f Cargo.toml ]; then
        cargo build --workspace --locked
    else
        echo "compile: no Rust workspace yet — see docs/ROADMAP.md (epic E0)."
    fi

# Run the Rust workspace test suite (same defensive guard as `compile`).
#
# This carries the `ridl test` property runs (E2.11a): a `ridl` integration test
# drives `ridl test` over the reviewed corpus workspace and asserts exit 0, so
# the range self-corpora and the contract sampling run in the local gate rather
# than in a separate recipe. Run `ridl test <path>` directly to see the report
# for a workspace of your own.
test:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -f Cargo.toml ]; then
        cargo test --workspace --locked
    else
        echo "test: no Rust workspace yet — see docs/ROADMAP.md (epic E0)."
    fi

# Check the compiler crates build for wasm32-unknown-unknown with fs/fetch
# off (ADR-0007 decision 5) — the E4.4 browser playground guard.
wasm-check:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -f Cargo.toml ]; then
        if ! command -v rustup >/dev/null 2>&1; then
            echo "wasm-check: rustup is required to add the wasm32 target." >&2
            echo "wasm-check: install it, or install the target another way." >&2
            exit 1
        fi
        rustup target add wasm32-unknown-unknown
        cargo check --target wasm32-unknown-unknown \
            -p ridl-syntax -p ridl-core -p ridl-sem -p ridl-ir \
            --no-default-features
    else
        echo "wasm-check: no Rust workspace yet — see docs/ROADMAP.md (epic E0)."
    fi

# Check Rust formatting without writing. Separate from `just fmt`, which owns
# the connective tissue (prim + markdownlint) and does not touch Rust.
# ADR-0008 decision 11 names `cargo fmt --all --check` in the merge gate; until
# issue #182 it sat in no recipe, so the gate a contributor runs did not enforce
# it. Run `cargo fmt --all` to repair what this reports.
fmt-check:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -f Cargo.toml ]; then
        cargo fmt --all --check
    else
        echo "fmt-check: no Rust workspace yet — see docs/ROADMAP.md (epic E0)."
    fi

# Lint the Rust workspace the way CI does (.github/workflows/ci.yml).
#
# Part of the local gate rather than CI's alone: some guards are enforced by a
# clippy lint and by nothing else. `crates/ridl-diff` denies
# `clippy::match_wildcard_for_single_variants` on the three matches over
# `Category`, because a new variant swept into a wildcard arm — the arm rustc's
# own `help:` text proposes — compiles and passes the whole test suite
# (ADR-0008 decision 21). A gate that runs `cargo test` and not `cargo clippy`
# does not enforce that guard at all.
lint:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -f Cargo.toml ]; then
        cargo clippy --workspace --all-targets -- -D warnings
    else
        echo "lint: no Rust workspace yet — see docs/ROADMAP.md (epic E0)."
    fi

# Build the mdBook docs as a gate member (build output: ./book, gitignored).
#
# `just book` serves the book and is for reading it; this recipe builds it and
# is for failing on it. mdBook rejects a SUMMARY.md it cannot parse, which is
# how a docs-only PR breaks the published book. Until this recipe existed the
# only place that check ran was CI, so it could not be run before pushing.
#
# The guard is deliberate. Making this a `build` dependency makes mdBook a hard
# requirement for every local build, and a missing binary would otherwise fail
# with exit 127 and no explanation (the reasoning that put a `command -v rustup`
# guard on `wasm-check`). It costs about 30 ms once mdBook is installed, and
# `./bootstrap` names it among the tools the gate requires.
book-check:
    #!/usr/bin/env bash
    set -euo pipefail
    if ! command -v mdbook >/dev/null 2>&1; then
        echo "book-check: mdbook is required to build the docs book." >&2
        echo "book-check: install it with 'cargo install mdbook --locked'," >&2
        echo "book-check: or from https://github.com/rust-lang/mdBook/releases." >&2
        exit 1
    fi
    mdbook build

# Check that CI still invokes every recipe the local gate is made of.
#
# The other half of gate parity. CI runs these recipes rather than its own copy
# of their commands, so the commands cannot drift — but a member can still be
# dropped from CI, or added to `build` and never wired into CI, which is how
# `mdbook build` came to run in CI and nowhere else. This compares the two
# lists: every dependency of `build` must appear in .github/workflows/ci.yml as
# `just <recipe>`.
#
# It does not prove CI runs nothing else; it proves CI runs no less.
#
# The pattern matches a run step and only a run step. A looser pattern would
# have been satisfied by this file's own name appearing in a YAML comment, which
# is a check that cannot fail.
gate-parity:
    #!/usr/bin/env bash
    set -euo pipefail
    workflow=.github/workflows/ci.yml
    if [ ! -f "$workflow" ]; then
        echo "gate-parity: $workflow is missing." >&2
        exit 1
    fi
    members="$(just --dump | sed -n 's/^build:[[:space:]]*//p')"
    if [ -z "$members" ]; then
        echo "gate-parity: could not read the dependencies of 'build' from the justfile." >&2
        exit 1
    fi
    missing=""
    for member in $members; do
        if ! grep -qE "^[[:space:]]*-[[:space:]]+run:[[:space:]]+just $member[[:space:]]*\$" "$workflow"; then
            missing="$missing $member"
        fi
    done
    if [ -n "$missing" ]; then
        echo "gate-parity: $workflow does not invoke:$missing" >&2
        echo "gate-parity: every member of 'just build' has to run in CI too." >&2
        exit 1
    fi
    echo "gate-parity: ci.yml invokes all $(echo $members | wc -w | tr -d ' ') members of 'just build'."

# Full local gate: confirm the toolchain and CI wiring, check Rust formatting,
# build the docs book, compile the code, run the tests, lint the Rust, check the
# wasm target builds, then run the connective-tissue lint checks.
#
# The members cover ADR-0008 decision 11's enumeration and the four CI checks
# ADR-0009 brought back to this side. Two of decision 11's were absent until
# issue #182: `cargo fmt --all --check` was in no recipe at all, and `wasm-check`
# was a recipe that nothing depended on. Anything the gate names has to be
# reachable from here, because `just verify` is what the pre-push hook runs — a
# member that is not a dependency of `build` is not enforced.
#
# The four cheap members run first, and together take about a second: they
# need no compilation, so a wrong toolchain, an unwired CI job, a formatting
# regression, or an unparseable SUMMARY.md all report before a compile starts
# rather than after a full compile and test run.
build: toolchain-check gate-parity fmt-check book-check compile test lint wasm-check check

# Serve the mdBook docs locally with live reload (build output: ./book).
book:
    mdbook serve

# Commit-message lint over commits not yet on origin/main, then build.
# Run before opening a PR (also wired as the pre-push hook). The range is
# taken against origin/main because local main goes stale relative to the
# remote; an empty range is benign and handled rather than failed.
verify:
    #!/usr/bin/env bash
    set -euo pipefail
    if ! git rev-parse --verify --quiet origin/main >/dev/null; then
        echo "verify: origin/main is missing — run 'git fetch origin'." >&2
        echo "verify: refusing to skip the commit-message lint." >&2
        exit 1
    fi
    if [ -z "$(git rev-list -n 1 origin/main..HEAD)" ]; then
        echo "verify: no commits in origin/main..HEAD — nothing to lint"
    else
        git std lint --range origin/main..HEAD
    fi
    just build

# Cut a release: git-std bumps the version, writes the changelog, tags.
release:
    git std bump

# Install the local toolchain: git-std, prim, and the git hooks.
install:
    ./bootstrap

# Remove build artifacts.
clean:
    rm -rf book target
