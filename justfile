# RIDL — task runner.
#
# Recipe set follows the driftsys house style (git-std, prim). The repo
# holds the specifications, ADRs, roadmap, and the Cargo workspace
# (docs/ROADMAP.md, epic E0). The fmt/check/compile/build shape is in
# place now — `check` gates the docs, and `compile` builds the Rust
# workspace as soon as one exists. The mdBook docs are served with
# `just book`.

set shell := ["bash", "-euo", "pipefail", "-c"]

# List recipes (default — hidden from the listing itself).
[private]
default:
    @just --list

# Reformat the connective tissue (Markdown/JSON/YAML/TOML) in place with prim,
# then auto-fix Markdown style findings.
fmt:
    prim .
    markdownlint '**/*.md' --ignore book --ignore node_modules --fix

# Lint gate — no writes: prim --check (formatting) + markdownlint (style).
check:
    prim --check .
    markdownlint '**/*.md' --ignore book --ignore node_modules

# Compile the Rust workspace. A no-op until the compiler workspace lands
# (docs/ROADMAP.md, epic E0); builds every crate once a Cargo.toml exists.
compile:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -f Cargo.toml ]; then
        cargo build --workspace
    else
        echo "compile: no Rust workspace yet — see docs/ROADMAP.md (epic E0)."
    fi

# Run the Rust workspace test suite. A no-op until the compiler workspace
# lands (docs/ROADMAP.md, epic E0); runs every crate's tests once a
# Cargo.toml exists.
test:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -f Cargo.toml ]; then
        cargo test --workspace
    else
        echo "test: no Rust workspace yet — see docs/ROADMAP.md (epic E0)."
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
