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
        rustup target add wasm32-unknown-unknown
        cargo check --target wasm32-unknown-unknown \
            -p ridl-syntax -p ridl-core -p ridl-sem -p ridl-ir \
            --no-default-features
    else
        echo "wasm-check: no Rust workspace yet — see docs/ROADMAP.md (epic E0)."
    fi

# Full local gate: compile the code, run the tests, then run the lint checks.
build: compile test check

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
