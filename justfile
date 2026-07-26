# RIDL — task runner.
#
# Recipe set follows the driftsys house style (git-std, prim). The repo
# holds the specifications, ADRs, roadmap, and the Cargo workspace
# (docs/ROADMAP.md, epic E0). `check` gates the docs, `compile` and
# `test` cover the Rust workspace, and `build` runs all three. The
# mdBook docs are served with `just book`.

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

# Compile the Rust workspace (the Cargo.toml guard is a defensive
# fallback for partial checkouts, not a "lands later" gate).
compile:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -f Cargo.toml ]; then
        cargo build --workspace
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
        cargo test --workspace
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

# Full local gate: check Rust formatting, compile the code, run the tests, lint
# the Rust, check the wasm target builds, then run the connective-tissue lint
# checks.
#
# The members are ADR-0008 decision 11's enumeration. Two of them were absent
# until issue #182: `cargo fmt --all --check` was in no recipe at all, and
# `wasm-check` was a recipe that nothing depended on. Anything decision 11 names
# has to be reachable from here, because `just verify` is what the pre-push hook
# runs — a member that is not a dependency of `build` is not enforced.
#
# `fmt-check` runs first on purpose. It needs no compilation and takes under a
# second, so a formatting-only regression — the case issue #182 was filed over —
# reports in a second rather than after a full compile and test run.
build: fmt-check compile test lint wasm-check check

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
