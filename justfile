# RIDL — task runner.
#
# Recipe set follows the driftsys house style (git-std, prim). The repo
# holds the specifications, ADRs, roadmap, and the Cargo workspace
# (docs/ROADMAP.md, epic E0). `check` gates the docs, `compile` and
# `test` cover the Rust workspace, and `build` runs the whole gate. The
# mdBook docs are served with `just book`, gated by `just book-check`, and
# rendered for publishing by `just book-build`.
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

# Reformat the connective tissue (Markdown/JSON/YAML/TOML) in place with prim.
fmt:
    prim .

# Lint gate — no writes: prim fmt --check (formatting) + prim lint (content,
# floor tier — the 12 always-on defect rules). See the driftsys/prim
# repository's own decision record 0012 (its "AD-" prefix, not this
# repository's unrelated ADR-0012) for the floor/strict split.
check:
    prim fmt --check .
    prim lint .

# Check that the rustc about to run is the version rust-toolchain.toml pins.
#
# It compares versions, and only versions. It does not detect an override as
# such: `RUSTUP_TOOLCHAIN=stable` passes whenever `stable` is the pinned
# release, which is the right answer — what the gate measures against is the
# compiler version, not which alias selected it. What it does catch is every
# way the version can end up wrong: no rustup at all, in which case this file is
# read by nobody and ignored without a word; a `RUSTUP_TOOLCHAIN` or a
# `rustup override set` naming a different release; or a pin nobody installed.
# Any of those leaves `cargo fmt --all --check` measuring a rustfmt other than
# the one CI applies, and reporting green against it.
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
    if ! printf '%s' "$pinned" | grep -qE '^[0-9]+\.[0-9]+(\.[0-9]+)?$'; then
        echo "toolchain-check: rust-toolchain.toml pins the channel '$pinned'." >&2
        echo "toolchain-check: the pin has to be an exact version, not a moving alias —" >&2
        echo "toolchain-check: an alias is the gap the pin exists to close (ADR-0009)." >&2
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
        echo "toolchain-check: install rustup so the pin is applied, or clear whatever names" >&2
        echo "toolchain-check: another release — a RUSTUP_TOOLCHAIN variable, or a" >&2
        echo "toolchain-check: 'rustup override' set on this directory." >&2
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
# off (ADR-0007 decision 5) — the E4.4 browser playground guard. The backend
# crates are included so this recipe actually exercises them: previously it
# checked a fixed non-backend list, so it never built ridl-backend-proto or
# ridl-backend-flatbuffers and could not have caught a change to either.
# `ridl-rt` is included because a package's generated Rust links it and is
# compiled to wasm32 (ADR-0020 decisions 6 and 7).
# Note: `cargo check` alone is not proof that a dev-only dependency (`protox`,
# `planus-translation`) promoted to a normal one would be caught here — both
# type-check cleanly for this target even as a normal dependency, because
# wasm32-unknown-unknown carries `std::fs` as a compiling (if not
# functioning) stub. This recipe covers the crates; it does not by itself
# guarantee catching that specific mistake. The guarantee lives in
# `xtask/tests/oracle_boundary.rs`, which reads the resolved dependency
# graph's edge kind directly (`cargo test -p xtask`, part of `just test`)
# and is what actually enforces that boundary.
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
            -p ridl-backend-proto -p ridl-backend-flatbuffers \
            -p ridl-backend-rust -p ridl-backend-ts \
            -p ridl-rt \
            --no-default-features
    else
        echo "wasm-check: no Rust workspace yet — see docs/ROADMAP.md (epic E0)."
    fi

# Build and test the crate `cargo publish -p ridl-rt` would upload, as both
# editions it supports, at the toolchains that do not already exercise it
# (ADR-0021 decision 10). Edition 2021 with the rust-toolchain.toml pin is
# `just test`; this recipe covers what that does not: the packaged manifest
# as edition 2021 with the minimum supported Rust version, and the packaged
# manifest edited to edition 2024 with the pin. Edition 2024 did not exist
# before Rust 1.85, so the minimum (older than that) can only test edition
# 2021.
#
# It tests the package, not a hand-copied crate: `cargo package` normalizes
# the manifest the way crates.io would receive it — `license.workspace = true`
# resolved to a literal string, and (until root Cargo.toml's `resolver = "2"`)
# this workspace's resolver written into `[package] resolver` — so this catches
# a packaging mistake a hand copy cannot, such as a resolver value the minimum
# toolchain cannot parse.
#
# The package is extracted to target/compat-check/pkg, a fixed path rather
# than a fresh mktemp each run, so CARGO_TARGET_DIR=target/compat-check/build
# lets cargo reuse the build across repeated runs instead of adding new build
# artifacts every time.
#
# `toolchain-check` is a dependency, and has already proven the running
# toolchain matches the rust-toolchain.toml pin, so this recipe reads the pin
# straight from `rustc --version` rather than re-parsing and re-validating the
# channel line itself. The minimum lives in crates/ridl-rt/Cargo.toml's
# rust-version line, which this recipe still reads and validates on its own.
#
# Fails on: a missing or malformed rust-version line; rustup missing; a
# packaged manifest the minimum toolchain cannot read (for instance, a
# workspace resolver it cannot parse); a packaged LICENSE that differs from
# the root LICENSE (crates/ridl-rt/LICENSE is a symlink to it — this catches a
# checkout where the symlink became a text file); or ridl-rt's library,
# tests, doctests, or examples failing to build or pass as edition 2021 with
# the minimum toolchain, or as edition 2024 with the pin.
compat-check: toolchain-check
    #!/usr/bin/env bash
    set -euo pipefail
    manifest="crates/ridl-rt/Cargo.toml"
    minimum="$(sed -n 's/^rust-version[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "$manifest")"
    if [ -z "$minimum" ]; then
        echo "compat-check: $manifest names no rust-version." >&2
        exit 1
    fi
    if ! printf '%s' "$minimum" | grep -qE '^[0-9]+\.[0-9]+(\.[0-9]+)?$'; then
        echo "compat-check: $manifest sets rust-version = \"$minimum\", which is not a" >&2
        echo "compat-check: MAJOR.MINOR or MAJOR.MINOR.PATCH version." >&2
        exit 1
    fi
    pin="$(rustc --version | cut -d' ' -f2)"

    if ! command -v rustup >/dev/null 2>&1; then
        echo "compat-check: rustup is required to install the $minimum toolchain." >&2
        echo "compat-check: install it from https://rustup.rs." >&2
        exit 1
    fi
    if ! rustup run "$minimum" rustc --version >/dev/null 2>&1; then
        echo "compat-check: installing the $minimum toolchain (not found locally)." >&2
        rustup toolchain install "$minimum" --profile minimal
    fi

    version="$(sed -n 's/^version[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "$manifest")"
    if [ -z "$version" ]; then
        echo "compat-check: $manifest names no version." >&2
        exit 1
    fi

    echo "compat-check: packaging ridl-rt $version"
    cargo package -p ridl-rt --no-verify --allow-dirty

    pkg="$PWD/target/compat-check/pkg"
    rm -rf "$pkg"
    mkdir -p "$pkg"
    tar -xzf "${CARGO_TARGET_DIR:-target}/package/ridl-rt-$version.crate" -C "$pkg" --strip-components=1

    if ! cmp -s "$pkg/LICENSE" LICENSE; then
        echo "compat-check: $pkg/LICENSE differs from the root LICENSE." >&2
        echo "compat-check: crates/ridl-rt/LICENSE is meant to be a symlink to it —" >&2
        echo "compat-check: check whether the checkout turned the symlink into a text file." >&2
        exit 1
    fi

    # The extracted manifest has no [workspace] table of its own, but it sits
    # under this repository's root workspace (target/ is inside it), so
    # without this table cargo reports that it believes the package is part
    # of that workspace and refuses to build it standalone.
    printf '\n[workspace]\n' >> "$pkg/Cargo.toml"

    export CARGO_TARGET_DIR="$PWD/target/compat-check/build"

    echo "compat-check: $minimum, edition 2021 (packaged)"
    cargo "+$minimum" test --all-features --offline --manifest-path "$pkg/Cargo.toml"

    # Edition 2024 requires rust-version >= 1.85 (cargo refuses to parse the
    # manifest otherwise); the pin already satisfies that, so this run's
    # rust-version becomes the pin rather than the minimum.
    sed -i.bak \
        -e 's/^edition = "2021"$/edition = "2024"/' \
        -e "s/^rust-version = \"$minimum\"\$/rust-version = \"$pin\"/" \
        "$pkg/Cargo.toml"
    rm -f "$pkg/Cargo.toml.bak"
    if ! grep -qx 'edition = "2024"' "$pkg/Cargo.toml" || ! grep -qx "rust-version = \"$pin\"" "$pkg/Cargo.toml"; then
        echo "compat-check: could not set edition 2024 and rust-version $pin in $pkg/Cargo.toml;" >&2
        echo "compat-check: its edition or rust-version line no longer has the form this recipe edits." >&2
        exit 1
    fi

    echo "compat-check: $pin, edition 2024 (packaged)"
    cargo "+$pin" test --all-features --offline --manifest-path "$pkg/Cargo.toml"

# Check Rust formatting without writing. Separate from `just fmt`, which owns
# the connective tissue (prim) and does not touch Rust.
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

# Build the mdBook docs as a gate member. `just book` serves the book and is for
# reading it; this recipe builds it and is for failing on it. Until this recipe
# existed the only place that check ran was CI, so it could not be run before
# pushing.
#
# What it catches is less than "the book compiles", and the difference matters.
# Measured against mdBook 0.4.52 on this book, `mdbook build` exits non-zero on
# a SUMMARY.md mdBook cannot parse (a suffix chapter followed by a list, for
# one) and on a missing SUMMARY.md. It exits 0 on a chapter file that does not
# exist, on a broken `{{#include}}` (an ERROR line, then exit 0), on a
# SUMMARY.md holding no list items, on one that is not a summary at all, and on
# bad nesting. So this recipe is a SUMMARY.md parse check, not a proof that the
# rendered book is whole.
#
# It builds a copy, because `mdbook build` writes into its own source: a
# SUMMARY.md naming a chapter file that does not exist makes mdBook **create
# that file** in `docs/book/` and exit 0. A check that mutates the tree it is
# checking is not a check. Copying into a temporary directory keeps the
# repository read-only for the duration. `just book` still writes ./book, which
# is gitignored.
#
# The whole of `docs/` is copied, not `docs/book` alone. The six "Language
# reference" chapters are thin wrappers that `{{#include}}` a normative document
# from `docs/specification/`, so a copy holding only `docs/book` would break
# every one of those includes and check a book no reader ever sees. `docs/` is
# about a megabyte; copying it is cheaper than maintaining a list of the
# directories includes are allowed to reach.
#
# **mdBook exits 0 on a broken `{{#include}}`.** It logs `ERROR Error updating
# ...`, leaves the directive in the page as literal text, renders the rest, and
# reports success. `mdbook build` alone therefore cannot see the failure this
# recipe exists to catch, so two checks run after it, and either one fails the
# recipe:
#
# 1. mdBook's stderr carries no `ERROR`. This catches every preprocessor
#    failure, not only a missing include, and it is not anchored to the start of
#    a line so a change to the log prefix does not silently disarm it.
# 2. No mdBook directive (`{{#...}}`) survives into the rendered output. This one
#    depends on no log format at all — it inspects the artifact — which matters
#    because the mdBook version is not pinned (issue #191). A page that ever
#    needs to show this syntax literally will trip it; that is a deliberate
#    change, and the author can escape the directive as `\{{#...}}` and adjust
#    this check with it.
#
# Both were verified by breaking an include on purpose and confirming each fails
# on its own.
#
# The mdBook guard is deliberate. Making this a `build` dependency makes mdBook
# a hard requirement for every local build, and a missing binary would otherwise
# fail with exit 127 and no explanation (the reasoning that put a
# `command -v rustup` guard on `wasm-check`). The build is a small fraction of a
# second on this book, and `./bootstrap` names mdBook among the tools the gate
# requires.
book-check:
    #!/usr/bin/env bash
    set -euo pipefail
    if ! command -v mdbook >/dev/null 2>&1; then
        echo "book-check: mdbook is required to build the docs book." >&2
        echo "book-check: install it with 'cargo install mdbook --locked'," >&2
        echo "book-check: or from https://github.com/rust-lang/mdBook/releases." >&2
        exit 1
    fi
    scratch="$(mktemp -d)"
    trap 'rm -rf "$scratch"' EXIT
    cp book.toml "$scratch/"
    cp -R docs "$scratch/docs"
    if ! mdbook build "$scratch" 2>"$scratch/mdbook.err"; then
        cat "$scratch/mdbook.err" >&2
        exit 1
    fi
    cat "$scratch/mdbook.err" >&2
    if grep -q 'ERROR' "$scratch/mdbook.err"; then
        echo "book-check: mdBook reported the error above and still exited 0." >&2
        exit 1
    fi
    if grep -rq '{{{{#' "$scratch/book"; then
        echo "book-check: an mdBook directive survived into the rendered output," >&2
        echo "book-check: which means it was not resolved:" >&2
        grep -rho '{{{{#[^}]*}}' "$scratch/book" | sort -u >&2
        exit 1
    fi

# Check that every relative Markdown link resolves.
#
# `book-check` cannot do this. mdBook exits 0 on an unresolved relative link, so
# two links to a reference that had been renamed survived in the book until they
# were found by hand. This resolves each link against the directory of the file
# that writes it, over every tracked `.md` — the book, the specifications, the
# ADRs, and the repository's own front matter alike.
#
# Fenced blocks and inline code spans are stripped first. A fence opens at three
# or more backticks or tildes, indented by at most three spaces, and closes only
# at a fence of the same character that is at least as long and has no info
# string, as in CommonMark. So a four-backtick fence can quote a Markdown file
# that holds three-backtick fences. A trailing carriage return is ignored. Not
# every CommonMark case is followed; among those that are not, a fence inside a
# block quote or on a list item's own line is read as text. A code span
# such as `element[](min..max)` is documentation of another language's syntax,
# not a link. External schemes and bare anchors are skipped; an anchor on a real
# path is trimmed, so the file is checked and the fragment is not.
#
# Before the scan, the recipe runs the extraction over a built-in sample that
# exercises each rule above (nesting, a longer closer, an info string, the other
# fence character, zero to four spaces of indentation, fewer than three
# backticks, trailing blanks and a carriage return) and fails unless exactly the
# expected links come out.
link-check:
    #!/usr/bin/env bash
    set -uo pipefail
    extract_links() {
        awk '{
                line = $0; sub(/\r$/, "", line); sub(/^(   |  | )/, "", line)
                if (line !~ /^(```|~~~)/) { if (!f) print; next }
                c = substr(line, 1, 1); n = 0
                while (substr(line, n + 1, 1) == c) n++
                if (!f) { f = 1; fc = c; fn = n; next }
                if (c == fc && n >= fn && substr(line, n + 1) ~ /^[ \t]*$/) f = 0
            }' "$1" \
            | sed -E 's/`[^`]*`//g' \
            | grep -oE '\]\([^)]+\)' \
            | sed -E 's/^\]\(//; s/\)$//' \
            | grep -vE '^(https?:|mailto:|#)' \
            | sed -E 's/#.*$//' \
            | grep -v '^$' || true
    }
    sample="$(mktemp)"
    trap 'rm -f "$sample"' EXIT
    printf '%s\n' '[a](before.md)' '````markdown' '```rsdl' '[b](nested.md)' '```' '````' \
        ' ```text' '[c](one-space.md)' ' ```' '   ```text' '[d](three-spaces.md)' '   ```' \
        '    ```' '[e](four-spaces.md)' \
        '~~~' '[f](tilde.md)' '```' '[g](tilde-still.md)' '~~~' \
        '````' '```' '[h](short-closer.md)' '`````' '[i](long-closer.md)' \
        $'```crlf\r' $'[j](crlf.md)\r' $'```\r' \
        '```' '[k](info.md)' '```text' '[l](info-still.md)' $'```  \t' '[m](trailing.md)' \
        '``' '[n](two-backticks.md)' \
        '[z](after.md)' > "$sample"
    expected="before.md four-spaces.md long-closer.md trailing.md two-backticks.md after.md "
    if [ "$(extract_links "$sample" | tr '\n' ' ')" != "$expected" ]; then
        echo "link-check: the fence rules no longer give the expected links on the built-in sample:" >&2
        extract_links "$sample" >&2
        exit 1
    fi
    broken=0
    while IFS= read -r file; do
        dir="$(dirname "$file")"
        while IFS= read -r target; do
            [ -z "$target" ] && continue
            [ -e "$dir/$target" ] && continue
            echo "link-check: $file -> $target" >&2
            broken=$((broken+1))
        done < <(extract_links "$file")
    done < <(git ls-files '*.md')
    if [ "$broken" -ne 0 ]; then
        echo "link-check: $broken link(s) above do not resolve." >&2
        exit 1
    fi
    echo "link-check: every relative Markdown link resolves."

# Check that every `docs/…` file path named in a tracked file resolves.
#
# `link-check` covers Markdown links. This covers the other way a document gets
# cited: a bare repository-relative path, written in prose, in an inline code
# span, or in a source comment. `link-check` sees none of those — it reads
# `.md` files only, and it strips inline code spans before it looks for links.
#
# Gardening is what makes this necessary. Moving a spec from `docs/wip/` to
# `docs/archive/` leaves every `//!` header comment that cited it pointing at a
# path that no longer exists, and nothing reported it: six such paths broke that
# way in driftsys/ridl#419, in `.rs`, `.ridl` and `.md` files, and every gate
# passed.
#
# Only the file form is checked. The extractor needs a filename extension to
# know where a path ends, so a directory citation — a `docs/…` path whose last
# segment carries no extension, such as `docs/decisions/` — is not checked at
# all, and renaming a directory is not caught here.
#
# Two trees are skipped, for the same reason in both: a `docs/…` path in them
# records what was true when it was written, not a claim about the tree now.
#   docs/archive/ — an archived plan says "Move: docs/wip/X to docs/archive/".
#                   Rewriting that would falsify the record it is kept for.
#   docs/wip/     — a live plan lists the files it is going to create, which do
#                   not exist yet by definition.
#
# One known false positive, with no mechanism to suppress it because it has not
# happened yet: another repository's `docs/…` path, written as a bare path
# rather than inside a URL, is reported as broken. Write it as a URL and this
# recipe leaves it alone, because a URL puts a `/` in front of it.
#
# Given no argument, the gate runs over the repository this justfile is in, and
# runs its own fixtures first, because a gate that cannot be shown to fail is
# not a gate. Given a directory, it runs over the repository there and runs no
# fixture: that is the form the fixtures invoke as a child process, and it is
# what stops the recursion.
doc-path-check root="":
    #!/usr/bin/env bash
    set -euo pipefail
    # C collation, so the order `sort -u` gives is the same on every machine the
    # sample's expected string is compared against. Under a UTF-8 locale an
    # uppercase segment sorts among the lowercase ones instead of before them,
    # and the sample below would fail on a machine that has one.
    export LC_ALL=C
    # Every `docs/…` string below is assembled from $d rather than written out,
    # because this file is itself scanned: a literal one that resolves nowhere
    # would be reported against this recipe's own line.
    d=docs
    skip_archive="$d/archive/"
    skip_wip="$d/wip/"
    # A path must start at the beginning of a line or after a character that no
    # path segment can contain. `/` is one of those characters, which is what
    # keeps a `docs/…` inside a URL from matching — the URL's own separator sits
    # in front of it, and no URL stripping is needed. A relative citation
    # written with a leading `../` is excluded by that same rule and is
    # therefore never checked. The sample below pins both.
    path_re="(^|[^A-Za-z0-9._/-])$d/[A-Za-z0-9._-]+(/[A-Za-z0-9._-]+)*\.[A-Za-z0-9]+"
    # `-I` states that a file holding a NUL byte is skipped rather than leaving
    # that to whichever grep is installed; no tracked file is binary today, so
    # it pins the behaviour rather than changing it. The two greps this was run
    # against announce a match in a binary file differently: BSD grep prints
    # `Binary file <name> matches` on stdout, which the extractor reads as a
    # path, and GNU grep 3.12 writes `grep: <name>: binary file matches` on
    # stderr. So the fixture below asserts the whole report and not only the
    # status; either one alone would let the mutation through on one of the
    # two. The extension is unbounded: capping its length would truncate a
    # long one rather than skip it, and report a string that is in no file.
    extract_paths() {
        local matches status=0
        matches="$(grep -IoE "$path_re" "$1")" || status=$?
        # grep exits 1 when nothing matched and 2 when it could not read the
        # file. Only the first of those is not an error.
        if [ "$status" -gt 1 ]; then
            return 1
        fi
        if [ -n "$matches" ]; then
            printf '%s\n' "$matches" | sed -E 's#^[^d]*##' | sort -u
        fi
    }
    # Report every extracted path that does not resolve under $1, over the file
    # list on stdin. Returns 1 when any did not resolve, or when a file could
    # not be read, and 0 only when every path resolved.
    #
    # A file that is not text is named instead of being passed over in silence:
    # it is not read for paths, so whatever it cites goes unchecked, and a gate
    # that loses coverage has to say where.
    scan_paths() {
        local root="$1" broken=0 file target targets
        while IFS= read -r file; do
            if ! targets="$(extract_paths "$root/$file")"; then
                echo "doc-path-check: cannot read '$file'." >&2
                return 1
            fi
            if [ -n "$targets" ]; then
                while IFS= read -r target; do
                    if [ ! -e "$root/$target" ]; then
                        echo "doc-path-check: $file -> $target" >&2
                        broken=$((broken + 1))
                    fi
                done <<<"$targets"
            elif [ -s "$root/$file" ] && ! grep -Iq -e '' "$root/$file"; then
                # Nothing came back from a file that is not empty. `-e ''`
                # matches every line of a text file, so only `-I` can hold the
                # match back: grep skipped the file rather than reading it.
                # One line per file, like the report above it, so a name
                # holding a space stays readable.
                echo "doc-path-check: not text, so not scanned: $file" >&2
            fi
        done
        if [ "$broken" -ne 0 ]; then
            echo "doc-path-check: $broken docs/ file path(s) above do not resolve." >&2
            echo "doc-path-check: $skip_archive and $skip_wip are not scanned; a path" >&2
            echo "doc-path-check: belonging to another repository should be written as a URL." >&2
            return 1
        fi
        return 0
    }
    # A git call that ignores an inherited git environment.
    #
    # This recipe runs from inside a git hook: the pre-push hook invokes `just
    # verify`, and a hook exports GIT_DIR. Under that environment `git -C <dir>`
    # changes directory but still reads and writes the repository GIT_DIR names,
    # so the fixture below would build its repository in this one's index rather
    # than its own. Clearing the inherited variables makes every call here act
    # on the directory it is given.
    git_at() {
        env -u GIT_DIR -u GIT_INDEX_FILE -u GIT_WORK_TREE -u GIT_OBJECT_DIRECTORY \
            -u GIT_COMMON_DIR -u GIT_NAMESPACE -u GIT_ALTERNATE_OBJECT_DIRECTORIES \
            git -C "$@"
    }
    # The tracked files of the repository at $1, under the pathspecs after it.
    #
    # core.quotePath is on by default, and it spells a path holding a byte
    # outside ASCII as "na\303\257ve.md" — a name that opens no file, which the
    # read check in extract_paths would turn into a gate failure over a file
    # that exists. One definition, so the fixture below drives the same call
    # the real scan does and dropping the setting fails here.
    list_files() {
        git_at "$1" -c core.quotePath=false ls-files "${@:2}"
    }
    # The gate itself, over the repository at $1: every docs/ file path named
    # in a tracked file outside the two skipped trees has to resolve.
    run_gate() (
        cd "$1"
        # Each skipped pathspec has to match tracked files, so a typo that
        # makes one of them inert fails here rather than widening the scan
        # without reporting it.
        for skipped in "$skip_archive" "$skip_wip"; do
            if [ -z "$(git_at . ls-files "$skipped")" ]; then
                echo "doc-path-check: the skipped tree '$skipped' matches no tracked file; correct the pathspec, or drop the exclusion if that tree is gone." >&2
                exit 1
            fi
        done
        if ! files="$(list_files . ":!$skip_archive" ":!$skip_wip")" \
            || ! all_files="$(list_files .)"; then
            echo "doc-path-check: git ls-files failed; the file list cannot be trusted." >&2
            exit 1
        fi
        # The scanned list has to be exactly the tracked files outside the two
        # skipped trees. The expectation is rebuilt from the whole listing with
        # the two tree names spelled out, rather than taken from the pathspecs
        # the call above used, so a pathspec that drops more than its own tree
        # parts the two lists and fails here. A size check did not: the scan
        # passed with `$d/` skipped in place of `$d/wip/`, and passed again
        # with every .rs file dropped, which is the file type this gate exists
        # for.
        expected="$(printf '%s\n' "$all_files" | grep -Ev "^$d/(archive|wip)/" || true)"
        if [ -z "$expected" ]; then
            echo "doc-path-check: every tracked file is inside $skip_archive or $skip_wip; there is nothing left to scan." >&2
            exit 1
        fi
        if [ "$files" != "$expected" ]; then
            echo "doc-path-check: the file list is not the tracked tree minus $skip_archive and $skip_wip." >&2
            echo "doc-path-check: '<' would be scanned and should not be; '>' should be and would not:" >&2
            diff <(printf '%s\n' "$files") <(printf '%s\n' "$expected") >&2 || true
            exit 1
        fi
        tracked="$(printf '%s\n' "$files" | grep -c . || true)"
        printf '%s\n' "$files" | scan_paths .
        echo "doc-path-check: every docs/ file path named outside $skip_archive and $skip_wip resolves, over $tracked tracked files."
    )
    # The fixtures. Each one builds a case the gate has to pass or fail and
    # fails this recipe when the gate does not. They run in a subshell, so the
    # temporary tree and the git environment set below go no further.
    fixtures() (
        work="$(mktemp -d)"
        trap 'rm -rf "$work"' EXIT
        # The extraction half. Each line pins one rule: a path in prose, in an
        # inline code span, in a source comment, followed by line references,
        # inside a URL, inside a Markdown link, mid-word, nested, an uppercase
        # segment, a segment carrying digits and an underscore, a filename with
        # more than one dot and an extension that is neither two letters nor short,
        # a relative citation, and a repeat of an earlier path for `sort -u`.
        sample="$work/sample"
        printf '%s\n' \
            "$d/plain.md named in prose" \
            "an inline code span \`$d/span.md\` link-check would strip" \
            "//! $d/comment.md §6)." \
            "with line references $d/lines.md:77,285" \
            "a foreign URL https://example.com/$d/url.md is not ours" \
            "a Markdown link [a]($d/link.md)" \
            "not a word boundary: x$d/notaword.md" \
            "nested $d/sub/dir/nested.md" \
            "$d/ROADMAP.md, an uppercase segment" \
            "a digit and an underscore in $d/decisions/ADR-0012-boundary_model.md" \
            "two dots and a five-letter extension in $d/ir/cruise.system.txtpb" \
            "an eight-letter extension in $d/typl-language-reference.markdown" \
            "a relative citation ../$d/design/relative.md" \
            "$d/plain.md a second time" > "$sample"
        expected="$d/ROADMAP.md $d/comment.md $d/decisions/ADR-0012-boundary_model.md"
        expected="$expected $d/ir/cruise.system.txtpb $d/lines.md $d/link.md $d/plain.md"
        expected="$expected $d/span.md $d/sub/dir/nested.md $d/typl-language-reference.markdown "
        if [ "$(extract_paths "$sample" | tr '\n' ' ')" != "$expected" ]; then
            echo "doc-path-check: the extractor no longer gives the expected paths on the built-in sample:" >&2
            extract_paths "$sample" >&2
            exit 1
        fi
        # The enforcement half. A scan that prints its findings and still returns 0
        # is the shape this fixture exists to catch, so the status, every report
        # line and the count in the summary are all asserted. One of the two files
        # cites two paths that do not exist, behind one that does, so a scan that
        # stops at the first breakage in a file fails here as well.
        root="$work/fixture"
        mkdir -p "$root/$d/design" "$root/src"
        : > "$root/$d/design/aa-present.md"
        printf '%s\n' "//! $d/design/aa-present.md" "//! $d/design/bb-gone.md" \
            "//! $d/design/cc-gone.md" > "$root/src/a.rs"
        printf '%s\n' "// $d/design/dd-gone.md" > "$root/src/b.ridl"
        report="$work/report"
        if printf '%s\n' src/a.rs src/b.ridl | scan_paths "$root" 2>"$report"; then
            echo "doc-path-check: the scan returned 0 over a fixture citing three paths that do not exist:" >&2
            cat "$report" >&2
            exit 1
        fi
        if [ "$(grep -c -- '->' "$report" || true)" -ne 3 ] \
            || ! grep -q -- "src/a.rs -> $d/design/bb-gone.md" "$report" \
            || ! grep -q -- "src/a.rs -> $d/design/cc-gone.md" "$report" \
            || ! grep -q -- "src/b.ridl -> $d/design/dd-gone.md" "$report" \
            || ! grep -q -- '^doc-path-check: 3 docs/ file path' "$report"; then
            echo "doc-path-check: the scan no longer reports exactly the three broken paths in its fixture:" >&2
            cat "$report" >&2
            exit 1
        fi
        # A file the list names but grep cannot open fails the scan rather than
        # being skipped: grep's exit 2 is an error, not an empty result.
        if printf '%s\n' src/not-there.rs | scan_paths "$root" 2>"$report" \
            || ! grep -q -- "cannot read 'src/not-there.rs'" "$report"; then
            echo "doc-path-check: the scan no longer fails on a file it cannot read:" >&2
            cat "$report" >&2
            exit 1
        fi
        # A file holding a NUL byte is skipped and named, and nothing else is
        # said about it. The path it cites does not exist, so a grep that reads
        # the file anyway reports a breakage and returns 1; the report is
        # compared whole, so a grep that writes a notice about the binary file
        # instead of a match fails here too.
        printf '//! %s\000\n' "$d/design/ee-gone.md" > "$root/src/blob.bin"
        if ! printf '%s\n' src/blob.bin | scan_paths "$root" 2>"$report" \
            || [ "$(cat "$report")" != "doc-path-check: not text, so not scanned: src/blob.bin" ]; then
            echo "doc-path-check: the scan no longer skips a file that is not text and names it:" >&2
            cat "$report" >&2
            exit 1
        fi
        # From here on the fixtures use git, and they run under a git
        # environment that names a repository of their own. That is what the
        # clearing in git_at is for, and it is pinned here: without it every
        # call below acts on the decoy rather than on the directory it is
        # given, the listings come back empty and the assertions fail. It also
        # keeps a call that escapes the clearing away from this repository,
        # which is the accident that has to be made impossible — `just verify`
        # runs from the pre-push hook, so an inherited GIT_DIR here is this
        # repository's own.
        decoy="$work/decoy"
        mkdir -p "$decoy"
        # The environment is set before the decoy repository is created, not
        # after, so that the call creating it is covered as well: without the
        # clearing that call reads GIT_DIR, and GIT_DIR has to name the decoy
        # by then rather than this repository.
        export GIT_DIR="$decoy/.git" GIT_WORK_TREE="$decoy"
        git_at "$decoy" -c init.defaultBranch=main -c init.templateDir= init -q
        # A tracked path holding a byte outside ASCII must come back in a spelling
        # that opens. The name is built from its two bytes so that this file stays
        # ASCII, and the repository needs no commit, because git lists the index.
        quoting="$work/quoting"
        mkdir -p "$quoting"
        git_at "$quoting" -c init.defaultBranch=main -c init.templateDir= init -q
        name="na$(printf '\303\257')ve.md"
        printf 'no path is cited here\n' > "$quoting/$name"
        # An ignore rule reaching this repository from the machine's own git config
        # would drop the file from the listing, and a scan over an empty listing
        # returns 0 — the assertion would pass having tested nothing. The rule is
        # neutralised, and the listing is compared against the name rather than
        # only being fed to the scan, so the check cannot succeed vacuously.
        git_at "$quoting" -c core.excludesFile=/dev/null add -A
        listed="$(list_files "$quoting")"
        if [ "$listed" != "$name" ]; then
            echo "doc-path-check: the listing did not give the fixture's name as it is spelled on disk:" >&2
            printf '%s\n' "$listed" >&2
            exit 1
        fi
        if ! printf '%s\n' "$listed" | scan_paths "$quoting" 2>"$report"; then
            echo "doc-path-check: the scan could not read a tracked path holding a byte outside ASCII:" >&2
            cat "$report" >&2
            exit 1
        fi
        # The status this recipe exits with, pinned from outside it. From in
        # here it cannot be observed: a `|| true` on the one call to run_gate
        # would print every breakage line and still exit 0, and no assertion
        # above would notice. So the gate is run as a child process — this
        # recipe, given a root, which is the form that runs the gate and
        # nothing else — over a repository built for it: first with every
        # scanned citation resolving, then with one that does not.
        #
        # That repository also cites a path that does not exist from inside
        # each of the two skipped trees, so a run that stops skipping them
        # fails the first of the two invocations.
        gate="$work/gate"
        mkdir -p "$gate/$d/design" "$gate/$d/archive" "$gate/$d/wip" "$gate/src"
        : > "$gate/$d/design/present.md"
        printf '%s\n' "moved to $d/archive/ff-gone.md" > "$gate/$d/archive/old.md"
        printf '%s\n' "will write $d/design/gg-gone.md" > "$gate/$d/wip/plan.md"
        printf '%s\n' "//! $d/design/present.md" > "$gate/src/a.rs"
        git_at "$gate" -c init.defaultBranch=main -c init.templateDir= init -q
        git_at "$gate" -c core.excludesFile=/dev/null add -A
        run="$work/run"
        if ! "{{just_executable()}}" doc-path-check "$gate" >"$run" 2>&1; then
            echo "doc-path-check: the gate did not pass over a fixture whose scanned citations all resolve:" >&2
            cat "$run" >&2
            exit 1
        fi
        printf '%s\n' "//! $d/design/hh-gone.md" > "$gate/src/broken.rs"
        git_at "$gate" -c core.excludesFile=/dev/null add -A
        if "{{just_executable()}}" doc-path-check "$gate" >"$run" 2>&1; then
            echo "doc-path-check: the gate returned 0 over a fixture citing a path that does not exist:" >&2
            cat "$run" >&2
            exit 1
        fi
        if ! grep -q -- "src/broken.rs -> $d/design/hh-gone.md" "$run"; then
            echo "doc-path-check: the gate did not report the broken path in its fixture:" >&2
            cat "$run" >&2
            exit 1
        fi
    )
    # One call site, so the fixture above and the real run take the same line:
    # a clause appended here disarms both, and the child process notices.
    root=.
    if [ -n "{{root}}" ]; then
        root="{{root}}"
    else
        fixtures
    fi
    run_gate "$root"

# Check that CI still invokes every recipe the local gate is made of.
#
# The other half of gate parity. CI runs these recipes rather than its own copy
# of their commands, so the commands cannot drift — but a member can still be
# dropped from CI, or added to `build` and never wired into CI, which is how
# `mdbook build` came to run in CI and nowhere else. This compares the two
# lists: every dependency of `build` must appear in .github/workflows/ci.yml as
# a `run:` step invoking `just <recipe>`.
#
# What it checks is narrow, and the narrowness is the point of this paragraph.
# It checks that the text of a `run: just <member>` step is present in the
# workflow file. It does not check that the step is reached: dropping a job from
# the `ci` aggregate's `needs:`, narrowing `on:`, or adding an `if:` that never
# holds all leave this green while the two gates genuinely diverge. It also says
# nothing about `verify` and `lint-commits`, which are not dependencies of
# `build`.
#
# Whole-line YAML comments are excluded, so a recipe named only in a comment —
# this workflow's own header names several — does not satisfy the check. A
# trailing comment on a real step does not break it, and neither does a `name:`
# on the step, because the pattern is not anchored to the start of the line:
# five steps in that workflow already use the `name:` + `run:` form, so
# requiring gate steps to stay unnamed would have been a rule nothing announced,
# enforced by a check whose remedy degrades the Actions UI.
#
# What does make it go red while CI genuinely runs the member: a block scalar
# (`run: |`), a folded scalar (`run: >-`), a quoted `"just check"`, `just
# $RECIPE` with the name in `env:`, a trailing argument (`just check --quiet`),
# and a backslash continuation. All seven fail closed, so none can hide a
# divergence — but five install steps in that workflow already use `run: |`, so
# wrapping a gate step is a plausible edit, and the message will name the member
# rather than the form that defeated the pattern.
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
    steps="$(grep -vE '^[[:space:]]*#' "$workflow")"
    missing=""
    for member in $members; do
        if ! printf '%s\n' "$steps" | grep -qE "run:[[:space:]]+just $member[[:space:]]*(#.*)?\$"; then
            missing="$missing $member"
        fi
    done
    if [ -n "$missing" ]; then
        echo "gate-parity: $workflow does not invoke:$missing" >&2
        echo "gate-parity: every member of 'just build' has to run in CI too." >&2
        exit 1
    fi
    echo "gate-parity: ci.yml invokes all $(echo $members | wc -w | tr -d ' ') members of 'just build'."

# End-to-end test of install.sh: downloads, checksum-verifies, and extracts a
# fixture release reached through a file:// URL — curl reads file://, so no
# server is needed. A dry run alone (RIDL_INSTALL_DRY_RUN=1) exercises neither
# the checksum nor the extraction, the two steps that matter most in a script
# users pipe straight into `bash`.
#
# Also runs install.ps1 through the same three checks — dry run, good
# install, corrupted download — behind a `command -v pwsh` guard:
# ubuntu-latest, where this recipe runs in CI, ships PowerShell 7 as `pwsh`,
# so this is the one place install.ps1 is exercised at all. The guard skips
# that part, rather than failing the recipe, on a machine — such as the one
# this was developed on — with no pwsh. Invoke-WebRequest does not support
# the file:// scheme install.sh's half of this recipe relies on, so the
# install.ps1 cases serve the same kind of fixture over a local
# `python3 -m http.server` instead, reached through the same
# RIDL_INSTALL_BASE_URL override install.sh's own tests use — install.ps1
# already honours that variable, with the same name and the same default.
install-check:
    #!/usr/bin/env bash
    set -euo pipefail
    scratch="$(mktemp -d)"
    http_pid=""
    trap 'rm -rf "$scratch"; [ -n "$http_pid" ] && kill "$http_pid" 2>/dev/null; true' EXIT

    # Learn this host's tarball name from the dry run rather than duplicating
    # install.sh's own detect_target platform table here.
    version="editor-v0.0.0-install-check"
    url="$(RIDL_VERSION="$version" RIDL_INSTALL_DRY_RUN=1 bash install.sh)"
    tarball="$(basename "$url")"
    reldir="$scratch/release/$version"
    mkdir -p "$reldir"

    if command -v pwsh >/dev/null 2>&1; then
        ps1_url="$(pwsh -NoProfile -Command '$env:RIDL_VERSION="editor-v0.1.0"; $env:RIDL_INSTALL_DRY_RUN="1"; ./install.ps1')"
        expected="https://github.com/driftsys/ridl/releases/download/editor-v0.1.0/ridl-x86_64-pc-windows-msvc.tar.gz"
        if [ "$ps1_url" != "$expected" ]; then
            echo "install-check: install.ps1 dry run printed '$ps1_url', expected '$expected'" >&2
            exit 1
        fi
        echo "install-check: install.ps1 dry run verified ($ps1_url)"

        # install.ps1 always requests the Windows target regardless of this
        # host's own platform, so it gets its own tarball, built the same way
        # as install.sh's and placed in the same version directory.
        ps_tarball="ridl-x86_64-pc-windows-msvc.tar.gz"
        printf 'ridl 0.0.0-install-check\n' > "$reldir/ridl.exe"
        if command -v sha256sum >/dev/null 2>&1; then
            (cd "$reldir" && tar czf "$ps_tarball" ridl.exe && sha256sum "$ps_tarball" > "$ps_tarball.sha256")
        else
            (cd "$reldir" && tar czf "$ps_tarball" ridl.exe && shasum -a 256 "$ps_tarball" > "$ps_tarball.sha256")
        fi
        rm "$reldir/ridl.exe"

        http_port=8971
        python3 -m http.server "$http_port" --bind 127.0.0.1 --directory "$scratch/release" \
            >"$scratch/http.log" 2>&1 &
        http_pid=$!
        for _ in $(seq 1 50); do
            curl -fsS "http://127.0.0.1:$http_port/" >/dev/null 2>&1 && break
            sleep 0.1
        done

        # A good install: same shape as install.sh's own good-install case
        # below, but ridl.exe cannot run on this host, so the check compares
        # the installed file's content against the fixture's instead of
        # executing it.
        ps_install1="$scratch/ridl install ps-good"
        RIDL_VERSION="$version" RIDL_INSTALL_BASE_URL="http://127.0.0.1:$http_port" \
            RIDL_INSTALL_DIR="$ps_install1" pwsh -NoProfile -File ./install.ps1
        if [ ! -f "$ps_install1/ridl.exe" ]; then
            echo "install-check: install.ps1 did not land ridl.exe in $ps_install1" >&2
            exit 1
        fi
        ps_got="$(cat "$ps_install1/ridl.exe")"
        if [ "$ps_got" != "ridl 0.0.0-install-check" ]; then
            echo "install-check: install.ps1 installed a file reading '$ps_got', expected the fixture's line" >&2
            exit 1
        fi
        echo "install-check: install.ps1 good install verified ($ps_install1/ridl.exe)"

        # A corrupted download: same construction as install.sh's own case
        # below — original .sha256, tampered tarball content.
        printf 'ridl tampered\n' > "$reldir/ridl.exe"
        (cd "$reldir" && tar czf "$ps_tarball" ridl.exe)
        rm "$reldir/ridl.exe"
        ps_install2="$scratch/ridl install ps-tamper"
        if RIDL_VERSION="$version" RIDL_INSTALL_BASE_URL="http://127.0.0.1:$http_port" \
            RIDL_INSTALL_DIR="$ps_install2" pwsh -NoProfile -File ./install.ps1 2>"$scratch/ps-tamper.err"; then
            echo "install-check: install.ps1 succeeded against a corrupted tarball" >&2
            exit 1
        fi
        cat "$scratch/ps-tamper.err" >&2
        # Test-Checksum's own Write-Error text, so this pins the rejection to
        # the checksum step rather than trusting that something rejected it.
        if ! grep -qi "checksum mismatch" "$scratch/ps-tamper.err"; then
            echo "install-check: install.ps1 rejected the download, but not visibly because of the checksum" >&2
            exit 1
        fi
        if [ -e "$ps_install2/ridl.exe" ]; then
            echo "install-check: a corrupted download still installed a binary via install.ps1" >&2
            exit 1
        fi
        echo "install-check: install.ps1 tamper case correctly rejected by the checksum and installed nothing"

        kill "$http_pid" 2>/dev/null || true
        http_pid=""
    else
        echo "install-check: pwsh not installed — install.ps1 checks skipped"
    fi

    # Build the fixture release: <version>/<tarball> plus its .sha256. The
    # "binary" is a tiny script that prints a recognisable version line.
    printf '#!/bin/sh\necho "ridl 0.0.0-install-check"\n' > "$reldir/ridl"
    chmod +x "$reldir/ridl"
    if command -v sha256sum >/dev/null 2>&1; then
        (cd "$reldir" && tar czf "$tarball" ridl && sha256sum "$tarball" > "$tarball.sha256")
    else
        (cd "$reldir" && tar czf "$tarball" ridl && shasum -a 256 "$tarball" > "$tarball.sha256")
    fi
    rm "$reldir/ridl"

    # A good install: the binary lands in a fresh directory — its name
    # holding a space, the quoting risk both scripts are most exposed to —
    # executable, and matching the fixture's own output.
    install1="$scratch/ridl install 1"
    RIDL_VERSION="$version" RIDL_INSTALL_BASE_URL="file://$scratch/release" \
        RIDL_INSTALL_DIR="$install1" bash install.sh
    if [ ! -x "$install1/ridl" ]; then
        echo "install-check: ridl did not land executable in $install1" >&2
        exit 1
    fi
    got="$("$install1/ridl")"
    if [ "$got" != "ridl 0.0.0-install-check" ]; then
        echo "install-check: installed binary printed '$got', expected the fixture's line" >&2
        exit 1
    fi
    echo "install-check: good install verified ($install1/ridl)"

    # A second install into the same, already-populated directory: install.sh
    # replaces the existing binary rather than merely writing into an empty
    # one, and the atomic rename it uses to do that (see the comment above
    # the install step in install.sh) leaves no `.ridl.*` temporary file
    # behind, unlike a same-directory copy that stops partway.
    RIDL_VERSION="$version" RIDL_INSTALL_BASE_URL="file://$scratch/release" \
        RIDL_INSTALL_DIR="$install1" bash install.sh
    if [ ! -x "$install1/ridl" ]; then
        echo "install-check: the second install did not leave ridl executable in $install1" >&2
        exit 1
    fi
    if find "$install1" -maxdepth 1 -name '.ridl.*' | grep -q .; then
        echo "install-check: a stray .ridl.* temporary file was left behind in $install1" >&2
        exit 1
    fi
    echo "install-check: second install over an existing binary verified, no stray temp file ($install1/ridl)"

    # A corrupted download: different content than the original .sha256
    # describes, but still a well-formed tarball. Appending garbage bytes
    # instead would also make some `tar` implementations refuse to extract
    # the archive at all, which would let this test pass for the wrong
    # reason — rejected by extraction, not by the checksum, which is exactly
    # the failure mode a checksum test exists to rule out. Keeping the
    # original .sha256 and replacing only the tarball's content isolates the
    # checksum step as the one thing that can reject this.
    printf '#!/bin/sh\necho "ridl tampered"\n' > "$reldir/ridl"
    chmod +x "$reldir/ridl"
    (cd "$reldir" && tar czf "$tarball" ridl)
    rm "$reldir/ridl"
    install2="$scratch/ridl install 2"
    if RIDL_VERSION="$version" RIDL_INSTALL_BASE_URL="file://$scratch/release" \
        RIDL_INSTALL_DIR="$install2" bash install.sh 2>"$scratch/tamper.err"; then
        echo "install-check: installer succeeded against a corrupted tarball" >&2
        exit 1
    fi
    cat "$scratch/tamper.err" >&2
    # Names the step that rejected the download, rather than trusting that
    # something did: sha256sum and shasum both write this line to stderr on
    # a checksum mismatch, and nothing else in install.sh's output can match
    # it, so its presence pins the failure to the checksum step specifically.
    if ! grep -qi "did not match" "$scratch/tamper.err"; then
        echo "install-check: installer rejected the download, but not visibly because of the checksum" >&2
        exit 1
    fi
    if [ -e "$install2/ridl" ]; then
        echo "install-check: a corrupted download still installed a binary" >&2
        exit 1
    fi
    echo "install-check: tamper case correctly rejected by the checksum and installed nothing"

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
# The four members that need no compilation run first, so a wrong toolchain, an
# unwired CI job, a formatting regression, or an unparseable SUMMARY.md all
# report before a compile starts rather than after a full compile and test run.
build: toolchain-check gate-parity install-check fmt-check book-check link-check doc-path-check compile test lint wasm-check compat-check check

# Serve the mdBook docs locally with live reload (build output: ./book).
book:
    mdbook serve

# Render the book to ./book. CI's Pages job runs this to produce what it
# publishes; locally it renders the same output `just book` serves.
#
# Not a member of `build`, and not a gate. `book-check` is the gate, and it
# builds a copy so that checking never writes into the tree — this one does
# write, both ./book (gitignored) and, for a SUMMARY.md that names a file which
# does not exist, that file in docs/book/. In CI the checkout is discarded after
# the run, and `book-check` has already failed the gate before this job starts.
book-build:
    mdbook build

# Commit-message lint over the commits this branch adds on top of a base.
#
# The range is taken against the remote-tracking ref because a local branch goes
# stale relative to the remote; an empty range is benign and handled rather than
# failed. The base is a parameter so that CI's commit-lint job, which lints
# against whatever branch a PR targets, and `verify`, which lints against
# `main`, run this one definition rather than each holding its own `git std
# lint` line. They held two before, and the two already differed: the
# workflow's read `origin/$base_ref..HEAD` and the recipe's read
# `origin/main..HEAD`, so they agreed only for PRs based on `main`.
#
# This is the one gate command `gate-parity` cannot watch, because `verify` is
# not a dependency of `build` — which is exactly why the two copies drifted
# unnoticed. One definition is the fix; the guard is not available here.
lint-commits base="main":
    #!/usr/bin/env bash
    set -euo pipefail
    ref="origin/{{ base }}"
    if ! git rev-parse --verify --quiet "$ref" >/dev/null; then
        echo "lint-commits: $ref is missing — run 'git fetch origin'." >&2
        echo "lint-commits: refusing to skip the commit-message lint." >&2
        exit 1
    fi
    if [ -z "$(git rev-list -n 1 "$ref"..HEAD)" ]; then
        echo "lint-commits: no commits in $ref..HEAD — nothing to lint"
    else
        git std lint --range "$ref"..HEAD
    fi

# Commit-message lint over commits not yet on origin/main, then build.
# Run before opening a PR (also wired as the pre-push hook).
verify: lint-commits build

# Cut a release: git-std bumps the version, writes the changelog, tags.
release:
    git std bump

# Set up a clone or worktree: git-std, prim, the git hooks, the Rust toolchain
# rust-toolchain.toml pins, and a report on any of the three tools the gate
# needs (just, rustup, mdbook) that bootstrap cannot find.
install:
    ./bootstrap

# Remove build artifacts.
clean:
    rm -rf book target

# Verify the VS Code extension packages: compile, unit tests, and a `vsce
# package` against a placeholder bin/ridl staged just for this check, so a
# .vscodeignore mistake that drops the binary is caught here instead of in a
# release, where it would ship five VSIXs with no binary at all. Invoked by
# vscode-verify.yaml on pull requests that touch editors/vscode. Not a member
# of `build`, so gate-parity does not cover it; the workflow is the only
# caller besides a contributor.
vscode-verify:
    #!/usr/bin/env bash
    set -euo pipefail
    cd editors/vscode
    npm ci
    npm test
    scratch="$(mktemp -d)"
    # A placeholder bin/ridl is staged only when none is present, so a real
    # binary that `just package-vscode` put there is neither overwritten nor
    # deleted. The trap removes the scratch directory, and the placeholder
    # only when this recipe staged it, even when a check below fails.
    staged=""
    trap 'rm -rf "$scratch"; if [ -n "$staged" ]; then rm -f bin/ridl; rmdir bin 2>/dev/null || true; fi' EXIT
    if [ ! -e bin/ridl ]; then
        mkdir -p bin
        printf '#!/bin/sh\necho placeholder\n' > bin/ridl
        chmod +x bin/ridl
        staged=1
    fi
    npx vsce package --out "$scratch/ridl-vscode.vsix"
    listing="$(npx vsce ls)"
    if ! grep -qx 'bin/ridl' <<<"$listing"; then
        echo "vscode-verify: bin/ridl is missing from the VSIX — check .vscodeignore" >&2
        exit 1
    fi
    if grep -q '^src/' <<<"$listing"; then
        echo "vscode-verify: src/ would ship in the VSIX — check .vscodeignore" >&2
        exit 1
    fi
    if grep -q '\.test\.js$' <<<"$listing"; then
        echo "vscode-verify: a compiled test file would ship in the VSIX — check .vscodeignore" >&2
        exit 1
    fi
    echo "vscode-verify: packaged ok"

# Package the extension: compile, then `vsce package`. Assumes the caller has
# already populated editors/vscode/bin/ — this recipe does not build ridl
# itself. With no argument, a plain `vsce package`. With a vsce-target (a
# vsce platform identifier, e.g. darwin-arm64), `vsce package --target
# <vsce-target> --out ridl-vscode-<vsce-target>.vsix`, which is what the
# release workflow's package-vsix job runs per target after staging that
# target's binary. Not a member of `build`.
package-vsix vsce-target="":
    #!/usr/bin/env bash
    set -euo pipefail
    cd editors/vscode
    npm ci
    npm run compile
    if [ -z "{{ vsce-target }}" ]; then
        npx vsce package
    else
        npx vsce package --target "{{ vsce-target }}" --out "ridl-vscode-{{ vsce-target }}.vsix"
    fi

# Build the extension for this machine: a release build of ridl copied into
# editors/vscode/bin/, then `just package-vsix` for the packaging half. Local
# testing only — the release workflow builds ridl per target in its own job
# and then runs `just package-vsix` with that target's vsce-target. Not a
# member of `build`.
package-vscode:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo build --release --locked -p ridl
    bin=target/release/ridl
    if [ -f target/release/ridl.exe ]; then bin=target/release/ridl.exe; fi
    mkdir -p editors/vscode/bin
    cp "$bin" editors/vscode/bin/
    just package-vsix
