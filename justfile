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
        # `ridl-descriptor` with its `std` feature off: the tests of the
        # reader half, in the configuration an engine builds.
        cargo test --locked -p ridl-descriptor --no-default-features
        # The workspace build resolves `ridl-rt` with default features, so the
        # encoding features gate code it never compiles: the helpers a
        # generated codec calls, and their tests. `just compat-check` builds
        # them, but only at rust-version 1.83 and only against the packaged
        # crate, so without this line a break in them reaches CI green. The
        # same line is what builds the `task` module under the `std` feature
        # and runs crates/ridl-rt/tests/task.rs.
        cargo test -p ridl-rt --all-features --locked
    else
        echo "test: no Rust workspace yet — see docs/ROADMAP.md (epic E0)."
    fi

# Check the compiler crates build for wasm32-unknown-unknown with fs/fetch
# off (ADR-0007 decision 5) — the E4.4 browser playground guard — and that
# `ridl-descriptor` builds with its `std` feature off for a target with no
# standard library, and with its features on for wasm32. The backend
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
#
# GENERATED CODE IS NOT ON THE `-p` LIST, AND CANNOT BE. ADR-0020 decision 2
# makes the generated Rust compiled to wasm32 the codec a TypeScript consumer
# loads, so the obligation reaches what the backend emits and not only the
# crates that emit it — but a generated package is text with no manifest, and
# `cargo check -p` takes packages. The chosen shape is the other one:
# `the_generated_codec_checks_for_wasm32` in
# `crates/ridl-backend-rust/tests/flatbuffers_conformance.rs` runs the same
# check (`rustc --target wasm32-unknown-unknown --emit=metadata`, which is
# the unit of work `cargo check` performs) over the emitted source, through
# the bare-`rustc` proof mechanism. It runs under `just test`. The
# alternative, an example crate under `crates/` holding a checked-in
# generated fixture so it could join the `-p` list, was rejected: it would
# add a workspace member and a second copy of the fixture to keep in step,
# for a check the test already performs over the emitter's live output.
wasm-check:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -f Cargo.toml ]; then
        if ! command -v rustup >/dev/null 2>&1; then
            echo "wasm-check: rustup is required to add the wasm32 and thumbv7em targets." >&2
            echo "wasm-check: install it, or install the target another way." >&2
            exit 1
        fi
        rustup target add wasm32-unknown-unknown
        cargo check --target wasm32-unknown-unknown \
            -p ridl-syntax -p ridl-core -p ridl-sem -p ridl-ir -p ridl-descriptor \
            -p ridl-backend-proto -p ridl-backend-flatbuffers \
            -p ridl-backend-rust -p ridl-backend-ts \
            -p ridl-rt -p ridl-fmt \
            --no-default-features
        # wasm32-unknown-unknown has a standard library, so the check above
        # cannot show that a crate builds without one. `ridl-descriptor` with
        # its `std` feature off is the reader an engine links, and a target
        # with no standard library is the proof that it links none.
        rustup target add thumbv7em-none-eabihf
        cargo check --target thumbv7em-none-eabihf -p ridl-descriptor --no-default-features
        # `ridl-descriptor` with its `std` feature on, so that the half of it
        # that builds a descriptor (`lower`, `describe`) is still checked for
        # the browser target, which the first check no longer does now that
        # `std` is a default feature of that crate.
        cargo check --target wasm32-unknown-unknown -p ridl-descriptor --all-features
        # And once more with the encoding features on. ADR-0020 decision 2
        # makes the generated Rust compiled to wasm32 the codec a TypeScript
        # consumer loads, so the helpers that codec calls must build for
        # wasm32 too — which the line above, with the features off, does not
        # show. The `std` feature is on here as well, so the `task` module
        # must build for wasm32 too, although `block_on` cannot run there.
        cargo check --target wasm32-unknown-unknown -p ridl-rt --all-features
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
# target/compat-check is removed at the start of the recipe, so both the
# extracted package (target/compat-check/pkg) and the build it feeds
# (CARGO_TARGET_DIR=target/compat-check/build) start clean every run. This
# gate exists to build the packaged crate from scratch under a different
# toolchain, so it must reuse no cache: a fixed build directory kept across
# runs let cargo's fingerprints for that path outlive the source they once
# described, so a passing tree could read as failing or the reverse (issue
# #442). ridl-rt has no dependency of its own in any feature combination
# (ADR-0021 decision 8), so the rebuild this costs is small.
#
# `toolchain-check` is a dependency, and has already proven the running
# toolchain matches the rust-toolchain.toml pin, so this recipe reads the pin
# straight from `rustc --version` rather than re-parsing and re-validating the
# channel line itself. The minimum lives in crates/ridl-rt/Cargo.toml's
# rust-version line, which this recipe still reads and validates on its own.
#
# The crate `ridl build --emit rust` writes for examples/cabin is checked
# here too, as edition 2021 with the minimum toolchain, against the packaged
# ridl-rt built the same way: the first of the three cells ADR-0021 decision
# 10 sets for generated code (edition 2021 at the minimum, edition 2021 at the
# pin, edition 2024 at the pin — the other two follow from the first, and
# `just demo` and `crates/ridlc/tests/cabin_example.rs` build the crate at the
# pin). A bare `rustc` with `--emit=metadata`, the way the compile proofs in
# crates/ridl-backend-rust build generated code, because the emitted crate has
# a manifest that names a registry version of ridl-rt and this check must
# link the packaged source instead.
#
# Fails on: a missing or malformed rust-version line; rustup missing; a
# packaged manifest the minimum toolchain cannot read (for instance, a
# workspace resolver it cannot parse); a packaged LICENSE that differs from
# the root LICENSE (crates/ridl-rt/LICENSE is a symlink to it — this catches a
# checkout where the symlink became a text file); the emitted cabin crate
# failing to check as edition 2021 with the minimum toolchain; or ridl-rt's
# library, tests, doctests, or examples failing to build or pass as edition
# 2021 with the minimum toolchain, or as edition 2024 with the pin.
#
# ridl-rt-conformance is packaged together with ridl-rt and held to the same
# terms, against the packaged ridl-rt. The recipe also fails on: a manifest of
# it that does not set the same rust-version as ridl-rt and edition 2021; a
# packaged copy without its README.md or without `readme = "README.md"`; a
# packaged LICENSE of either crate that differs from the root LICENSE; its
# tests failing to pass as edition 2021 with the minimum toolchain or as
# edition 2024 with the pin; or `suite!`, expanded with every extension flag
# over the source of ridl-loopback as edition 2021 (tests/conformance.rs),
# failing to compile with the minimum toolchain (which also holds that source
# to the minimum). The last one is needed
# because the crate's own tests never expand the macro.
compat-check: toolchain-check
    #!/usr/bin/env bash
    set -euo pipefail
    rm -rf "$PWD/target/compat-check"
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

    # Not a `sed` read of `$manifest`: `ridl-rt` shares the workspace version
    # (ADR-0007 decision 14's 2026-09-21 amendment) via `version.workspace =
    # true`, so the manifest itself names no literal to extract. `cargo pkgid`
    # resolves the inheritance the same way `cargo package` below does.
    pkgid="$(cargo pkgid -p ridl-rt)"
    version="${pkgid##*#}"
    if [ -z "$version" ]; then
        echo "compat-check: could not read ridl-rt's version from 'cargo pkgid'." >&2
        exit 1
    fi

    # ridl-rt-conformance is held to the same minimum: a port author on
    # ridl-rt's minimum must be able to build the suite.
    conformance_manifest="crates/ridl-rt-conformance/Cargo.toml"
    if ! grep -qx "rust-version = \"$minimum\"" "$conformance_manifest" \
        || ! grep -qx 'edition = "2021"' "$conformance_manifest"; then
        echo "compat-check: $conformance_manifest must set edition = \"2021\" and" >&2
        echo "compat-check: rust-version = \"$minimum\", the same as $manifest." >&2
        exit 1
    fi

    # Both crates in one command: the packaged ridl-rt-conformance depends on
    # ridl-rt at this version, which is not on the registry until the release
    # publishes it, so the single-crate form fails while the version is
    # unpublished.
    echo "compat-check: packaging ridl-rt and ridl-rt-conformance $version"
    cargo package -p ridl-rt -p ridl-rt-conformance --no-verify --allow-dirty

    pkg="$PWD/target/compat-check/pkg"
    pkg_conformance="$PWD/target/compat-check/pkg-conformance"
    mkdir -p "$pkg" "$pkg_conformance"
    tar -xzf "${CARGO_TARGET_DIR:-target}/package/ridl-rt-$version.crate" -C "$pkg" --strip-components=1
    tar -xzf "${CARGO_TARGET_DIR:-target}/package/ridl-rt-conformance-$version.crate" -C "$pkg_conformance" --strip-components=1

    for dir in "$pkg" "$pkg_conformance"; do
        if ! cmp -s "$dir/LICENSE" LICENSE; then
            echo "compat-check: $dir/LICENSE differs from the root LICENSE." >&2
            echo "compat-check: the crate's LICENSE is meant to be a symlink to it —" >&2
            echo "compat-check: check whether the checkout turned the symlink into a text file." >&2
            exit 1
        fi
    done
    if [ ! -f "$pkg_conformance/README.md" ] || ! grep -qx 'readme = "README.md"' "$pkg_conformance/Cargo.toml"; then
        echo "compat-check: the packaged ridl-rt-conformance has no README.md or does not" >&2
        echo "compat-check: name it with readme = \"README.md\"." >&2
        exit 1
    fi

    # The extracted manifest has no [workspace] table of its own, but it sits
    # under this repository's root workspace (target/ is inside it), so
    # without this table cargo reports that it believes the package is part
    # of that workspace and refuses to build it standalone.
    printf '\n[workspace]\n' >> "$pkg/Cargo.toml"
    # The conformance crate takes the packaged ridl-rt instead of the registry
    # copy, which may not exist at this version yet.
    printf '\n[workspace]\n\n[patch.crates-io]\nridl-rt = { path = "%s" }\n' "$pkg" >> "$pkg_conformance/Cargo.toml"

    # The emitted cabin crate as edition 2021 at the minimum. The CLI is
    # built with the pin, in the workspace's own target directory, before
    # CARGO_TARGET_DIR is pointed at the compat build below.
    cargo build --locked -p ridl-cli
    cabin="$PWD/target/compat-check/cabin"
    mkdir -p "$cabin"
    "${CARGO_TARGET_DIR:-target}/debug/ridl" build examples/cabin --emit rust --out-dir "$cabin/generated"
    echo "compat-check: $minimum, edition 2021 (the emitted cabin crate)"
    # `std` is on for both: the emitted crate's `blocking` module is under its
    # `std` feature and names `ridl_rt::task::block_on`, which `ridl-rt`'s
    # `std` feature gates, so the cell covers that module too.
    rustup run "$minimum" rustc --edition 2021 --crate-type rlib --crate-name ridl_rt \
        --cfg 'feature="flatbuffers"' --cfg 'feature="std"' \
        "$pkg/src/lib.rs" -o "$cabin/libridl_rt.rlib"
    rustup run "$minimum" rustc --edition 2021 --crate-type lib --crate-name veh_cabin \
        --emit=metadata -D warnings --cfg 'feature="std"' \
        --extern "ridl_rt=$cabin/libridl_rt.rlib" \
        "$cabin/generated/lib.rs" -o "$cabin/libveh_cabin.rmeta"

    export CARGO_TARGET_DIR="$PWD/target/compat-check/build"

    echo "compat-check: $minimum, edition 2021 (packaged)"
    cargo "+$minimum" test --all-features --offline --manifest-path "$pkg/Cargo.toml"

    echo "compat-check: $minimum, edition 2021 (packaged ridl-rt-conformance)"
    cargo "+$minimum" test --all-features --offline --manifest-path "$pkg_conformance/Cargo.toml"

    # The crate's own tests never expand `suite!`, so the macro is compiled
    # here: ridl-loopback's source and its tests/conformance.rs, which expand
    # it with every extension flag, as edition 2021 against the two packaged
    # crates. Compile only: the pin runs this test in the workspace. The
    # loopback source therefore has to build as edition 2021 with the minimum
    # toolchain, so it uses no let chain.
    loopback="$PWD/target/compat-check/loopback"
    mkdir -p "$loopback/tests"
    cp -R crates/ridl-loopback/src "$loopback/src"
    cp crates/ridl-loopback/tests/conformance.rs "$loopback/tests/conformance.rs"
    {
        printf '[package]\nname = "ridl-loopback"\nversion = "0.0.0"\nedition = "2021"\n'
        printf 'rust-version = "%s"\n\n' "$minimum"
        printf '[dependencies]\nridl-rt = { path = "%s" }\n\n' "$pkg"
        printf '[dev-dependencies]\nridl-rt-conformance = { path = "%s" }\n\n' "$pkg_conformance"
        printf '[patch.crates-io]\nridl-rt = { path = "%s" }\n\n[workspace]\n' "$pkg"
    } > "$loopback/Cargo.toml"
    echo "compat-check: $minimum, edition 2021 (suite! expanded over ridl-loopback)"
    cargo "+$minimum" test --no-run --offline --manifest-path "$loopback/Cargo.toml"

    # Edition 2024 requires rust-version >= 1.85 (cargo refuses to parse the
    # manifest otherwise); the pin already satisfies that, so this run's
    # rust-version becomes the pin rather than the minimum.
    for dir in "$pkg" "$pkg_conformance"; do
        sed -i.bak \
            -e 's/^edition = "2021"$/edition = "2024"/' \
            -e "s/^rust-version = \"$minimum\"\$/rust-version = \"$pin\"/" \
            "$dir/Cargo.toml"
        rm -f "$dir/Cargo.toml.bak"
        if ! grep -qx 'edition = "2024"' "$dir/Cargo.toml" || ! grep -qx "rust-version = \"$pin\"" "$dir/Cargo.toml"; then
            echo "compat-check: could not set edition 2024 and rust-version $pin in $dir/Cargo.toml;" >&2
            echo "compat-check: its edition or rust-version line no longer has the form this recipe edits." >&2
            exit 1
        fi
    done

    echo "compat-check: $pin, edition 2024 (packaged)"
    cargo "+$pin" test --all-features --offline --manifest-path "$pkg/Cargo.toml"

    echo "compat-check: $pin, edition 2024 (packaged ridl-rt-conformance)"
    cargo "+$pin" test --all-features --offline --manifest-path "$pkg_conformance/Cargo.toml"

# Generate the cabin example's crate and run the program that links it.
#
# This is the one place the whole chain runs as a person runs it: `ridl build`
# writes a crate from `examples/cabin/cabin.ridl`, and `cargo` builds a program
# against it and executes it. The program prints one line per round trip — a
# signal, an event, a command and a query — and exits non-zero if any of them
# does not hold, so a green run is the demo working rather than merely
# compiling.
#
# `examples/cabin` is its own cargo workspace, outside this repository's. Its
# `generated/` and `generated-corpus/` members are written here and are not in
# git, so nothing in the repository's own workspace depends on a build output.
# `--locked` holds the committed `examples/cabin/Cargo.lock`; a dependency
# change that the lock does not carry fails rather than silently resolving.
#
# The format and lint checks are here rather than in `fmt-check` and `lint`,
# which run `--all` over this repository's workspace and so cannot see a crate
# outside it. The consumer is linted with `--no-deps`, whose absence would lint
# a path dependency built from source.
#
# The generated crate is linted as well, under a closed list. Without any
# allowance it draws one `clippy::module_inception`, for the module a package
# named `veh.cabin` and an interface named `Cabin` give (`veh::cabin::cabin`),
# and two `clippy::derivable_impls`, for a `Default` the emitter writes out
# rather than derives. Both are the emitter's own shape, so the emitted
# `lib.rs` allows exactly those two lints. This recipe turns each `allow` in
# that list into an `expect` on a temporary edit of `lib.rs`, restored on exit.
# A lint that is not on the list then fails the run as a new defect in the
# emitter, and a listed lint that no longer fires fails through
# `unfulfilled_lint_expectations`, so the list cannot go stale.
#
# The crate emitted for the veh-cluster corpus is linted too, with its default
# features and with them off, and with no command-line allowance except
# `dead_code`, because the corpus declares `internal` items that nothing uses;
# the lints the emitted `lib.rs` allows stay allowed. A `dead_code` diagnostic
# passes only when the item it belongs to is one of the corpus's internal
# items (the recipe lists them). Unlike the run above, this one keeps `allow`:
# `clippy::module_inception` does not fire on the corpus crate, so a stale
# entry there does not fail, but the run above fires it and holds it. Any other
# warning is a defect in the emitter.
#
# The binary is reached through `CARGO_TARGET_DIR` where it is set, the way
# `compat-check` reads it, rather than through a hardcoded `./target`: a
# contributor who exports that variable would otherwise get a missing-file
# failure from a member of `just build` rather than from this recipe's own
# work — or, worse, run a stale binary left at the default path. The variable
# is the only spelling handled: `build.target-dir` in a cargo config, and a
# `--target` triple, both move the binary somewhere this does not look, which
# `compat-check` shares and neither closes.
#
# Fails on: the build drawing an error; the emitted crate or the consumer
# failing to compile; the emitted crate failing to check with its default
# features off for `thumbv7em-none-eabihf`; the crate emitted for the
# veh-cluster corpus failing to check the same way, or with only
# `validate-pattern` on; the program exiting non-zero or not reporting all four
# round trips; the lock being out of date; the consumer being unformatted or
# drawing a clippy warning; the generated crate drawing a clippy warning that
# its `lib.rs` does not allow, or an allow that no longer fires; a `lib.rs`
# with no `#![allow(` line to rewrite; the crate emitted for the veh-cluster
# corpus drawing a clippy warning other than `dead_code`, with default
# features or without them, or a `dead_code` warning for an item that is not
# an internal one of the corpus; a planus crate in the resolved graph of
# `examples/cabin`; the planus check running no test or more than one, which is what a renamed test or a changed filter does.
#
# The planus check is `xtask/tests/oracle_boundary.rs`'s
# `the_generated_crate_reaches_no_planus_crate`, which is ignored for a plain
# `cargo test` because it needs the generated crate this recipe writes. A
# generated package must not depend on planus (ADR-0014 decision 15).
#
# The lock pins `ridl-rt`, whose version this recipe does not own, and the
# generated crate, whose dependencies come from the emitter. A version bump or
# a schema change that moves either one fails here with cargo's own `--locked`
# message, which does not name its remedy: rerun without `--locked` once and
# commit the new `examples/cabin/Cargo.lock`.
demo:
    #!/usr/bin/env bash
    set -euo pipefail
    target="${CARGO_TARGET_DIR:-target}"
    cargo build --locked -p ridl-cli
    # Cleared first: `--out-dir` writes over what it writes and leaves
    # everything else, so a file the emitter stops writing would survive here
    # and keep this green while a fresh clone failed.
    rm -rf examples/cabin/generated examples/cabin/generated-corpus
    "$target/debug/ridl" build examples/cabin --emit rust --out-dir examples/cabin/generated
    # For the `no_std` check and the clippy run below: cabin's schema has no
    # string, bytes, array, map or pattern, and this workspace has each of them.
    "$target/debug/ridl" build crates/ridlc/tests/corpus/veh-cluster --emit rust \
        --out-dir examples/cabin/generated-corpus
    # This line is the only one that runs the generated crate's planus check,
    # which is ignored for a plain `cargo test`. A filter that matches no test
    # exits 0, so a renamed test would pass here unseen: the result line must
    # say that exactly one test ran and passed.
    if ! planus_check="$(cargo test --locked -p xtask --test oracle_boundary -- --ignored --exact the_generated_crate_reaches_no_planus_crate 2>&1)"; then
        printf '%s\n' "$planus_check"
        exit 1
    fi
    printf '%s\n' "$planus_check"
    if ! printf '%s\n' "$planus_check" | grep -q '^test result: ok\. 1 passed; 0 failed'; then
        echo "demo: the generated crate's planus check did not run exactly one test" >&2
        exit 1
    fi
    # The emitted `lib.rs` allows the emitter's own lints; `expect` makes both a
    # missing and a stale entry fail. The trap restores the file on any exit.
    trap 'mv examples/cabin/generated/lib.rs.bak examples/cabin/generated/lib.rs' EXIT
    sed -i.bak 's/^#!\[allow(/#![expect(/' examples/cabin/generated/lib.rs
    if ! grep -q '^#!\[expect(' examples/cabin/generated/lib.rs; then
        echo "demo: examples/cabin/generated/lib.rs has no #![allow( line to rewrite to #![expect(" >&2
        exit 1
    fi
    cargo clippy --manifest-path examples/cabin/Cargo.toml -p veh_cabin --locked --no-deps -- -D warnings
    mv examples/cabin/generated/lib.rs.bak examples/cabin/generated/lib.rs
    trap - EXIT
    # The generated crate with its default features off, for a target that
    # has no standard library, which is the proof that it links none: a target
    # that has one, `wasm32-unknown-unknown` included, builds a crate that is
    # missing `no_std` without an error. Through cargo and the emitted
    # manifest rather than a bare `rustc`, so that what the features forward
    # to `ridl-rt` is checked as well. The target comes from
    # rust-toolchain.toml.
    cargo check --manifest-path examples/cabin/Cargo.toml -p veh_cabin --locked \
        --no-default-features --target thumbv7em-none-eabihf
    # The same check over the crate for the veh-cluster corpus, whose string,
    # bytes, array and map types reach `String` and `Vec` through the
    # `alloc` that `lib.rs` links as `std`; then on this machine with only
    # `validate-pattern` on, for the `ridl.std` pattern checks, under which
    # the crate links the standard library while `std` stays off.
    cargo check --manifest-path examples/cabin/Cargo.toml -p ridl_generated --locked \
        --no-default-features --target thumbv7em-none-eabihf
    cargo check --manifest-path examples/cabin/Cargo.toml -p ridl_generated --locked \
        --no-default-features --features validate-pattern
    # The corpus crate is linted as well, with its default features and with
    # them off for the target that has no standard library, so the code that
    # compiles only in the second case is linted too. `dead_code` is the one
    # lint left out of the strict run: the corpus declares `internal` items
    # that nothing uses, and those draw it. A second run reports `dead_code`
    # as JSON and fails on every diagnostic whose owner is not one of the
    # corpus's internal items. The owner is the type in the header of the
    # `impl` block for a method, and the backticked name otherwise. It must
    # match, as a whole, `RawTickCount`, `RawWheelFrame`, `RawWheelSpan` or
    # `WheelDiagnostics`, followed by any of the generated suffixes `Span`,
    # `BurstsElement` and `FbView`. Dead code that the emitter writes for any
    # other owner fails the run. Dead code inside one of those owners does
    # not: the filter reads the owner, not the member. The two runs stay
    # separate because a single `-D warnings` JSON run also reports cargo's
    # own "aborting due to" summary as an error, which the filter would have
    # to tell apart from a real failure.
    corpus_lint() {
        cargo clippy --manifest-path examples/cabin/Cargo.toml -p ridl_generated --locked --no-deps \
            "$@" -- -D warnings -A dead_code
        stray="$(cargo clippy --manifest-path examples/cabin/Cargo.toml -p ridl_generated --locked --no-deps \
            --message-format=json "$@" -- -W dead_code |
            jq -r 'select(.reason == "compiler-message" and .message.code.code == "dead_code")
                | .message
                | (([.spans[].text[0].text | capture("^\\s*impl(<[^>]*>)? (?<n>[A-Za-z0-9_]+)")? | .n][0])
                    // (.message | capture("`(?<n>[^`]+)`").n)) as $owner
                | select($owner | test("^(RawTickCount|RawWheelFrame|RawWheelSpan|WheelDiagnostics)(Span|BurstsElement)*(FbView)?$") | not)
                | .rendered')"
        if [ -n "$stray" ]; then
            printf '%s\n' "$stray" >&2
            echo "demo: the corpus crate has dead code that is not an internal item" >&2
            exit 1
        fi
    }
    corpus_lint
    corpus_lint --no-default-features --target thumbv7em-none-eabihf
    cargo fmt --manifest-path examples/cabin/consumer/Cargo.toml --check
    cargo clippy --manifest-path examples/cabin/Cargo.toml -p consumer --locked --all-targets --no-deps -- -D warnings
    # The output is checked, not just the status, and each line carries the
    # value its round trip carried rather than the bare word `ok`. So the
    # match is on what travelled: a codec or a face returning a wrong value
    # fails here even if the consumer's own `assert` were weakened, and a
    # round trip that aborts part-way takes its line with it. Checking the
    # status alone caught neither, and `cabin_example` reads this same source,
    # so nothing else in the tree would have noticed.
    #
    # What it does not catch, because nothing can: a round trip deleted and
    # its line replaced by the literal this loop looks for. That is editing
    # the proof rather than the code, the same as deleting a test.
    # Captured rather than streamed so the lines can be matched, and printed
    # on both paths: a panic part-way through would otherwise take the round
    # trips that did complete with it, which is what tells a reader how far
    # the demo got.
    if ! output="$(cargo run --manifest-path examples/cabin/Cargo.toml -p consumer --locked)"; then
        printf '%s\n' "$output"
        echo "demo: the consumer did not run to completion" >&2
        exit 1
    fi
    printf '%s\n' "$output"
    for round_trip in "signal ok 21" "event ok 5" "command ok 42" "query ok 7" \
            "blocking command ok 43" "blocking query ok 9"; do
        if ! printf '%s\n' "$output" | grep -qxF "$round_trip"; then
            echo "demo: the consumer did not report \"$round_trip\"" >&2
            exit 1
        fi
    done

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
        # As in `just test`: the workspace resolves `ridl-rt` with default
        # features, so its feature-gated modules are otherwise never linted.
        cargo clippy -p ridl-rt --all-features --all-targets -- -D warnings
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
# bad nesting. This recipe adds checks for the first two of those, and a third
# for an include by anchor that does not resolve, described below. So it is a
# SUMMARY.md parse check plus those three checks, not a proof that the
# rendered book is whole.
#
# It builds a copy, because `mdbook build` writes into its own source: a
# SUMMARY.md naming a chapter file that does not exist makes mdBook **create
# that file** in `docs/book/` and exit 0. A check that mutates the tree it is
# checking is not a check. Copying into a temporary directory keeps the
# repository read-only for the duration. `just book` still writes ./book, which
# is gitignored. The recipe detects the created file by comparing the copy's
# file list taken right after the copy, before the build runs, against the
# copy's file list taken after the build: a file present only in the second
# list is one mdBook created. It reads the tree's `docs/`, and the `.rs` and
# `.ridl` sources under `examples/`, once, to make the copy, and never again, so
# a change to the tree while the build runs cannot affect the result.
#
# The whole of `docs/` is copied, not `docs/book` alone. The eight "Language
# reference" chapters are thin wrappers that `{{#include}}` a normative document
# from `docs/specification/`, so a copy holding only `docs/book` would break
# every one of those includes and check a book no reader ever sees. `docs/` is
# about a megabyte; copying it is cheaper than maintaining a list of the
# directories includes are allowed to reach. A chapter may also include a
# source from `examples/` by anchor, so that the code it shows is the code
# `just demo` builds; the recipe copies every `.rs` and `.ridl` file under
# `examples/`, and nothing under a `target/`, a `generated/` or a
# `generated-corpus/` directory, which hold build output and the generated
# crates.
#
# **mdBook exits 0 on a broken `{{#include}}`.** It logs `ERROR Error updating
# ...`, leaves the directive in the page as literal text, renders the rest, and
# reports success. `mdbook build` alone therefore cannot see that failure, so
# checks 1 and 2 below run to catch it. Check 3 catches the other failure this
# recipe exists for: the chapter file mdBook creates when SUMMARY.md names one
# that does not exist, described above. **mdBook also exits 0, and logs
# nothing, on an include by anchor that does not resolve**: an anchor the file
# does not have renders an empty block, and an anchor with no `ANCHOR_END`
# renders the rest of the file. Check 4 catches that. Any one of the four
# checks that fails fails the recipe:
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
# 3. Every file in the copy after the build was already there before it. It
#    compares every file, not only `.md` files, so it also catches anything
#    else a future mdBook version creates there, not only a missing chapter.
# 4. Every `{{#include <path>:<anchor>}}` in `docs/book` names a file that
#    holds both `ANCHOR: <anchor>` and `ANCHOR_END: <anchor>`. An include by
#    line numbers (`:10`, `:10:20`) is not an anchor and is not checked here.
#
# The fixture below verifies all four checks independently: a chapter that
# includes a file that does not exist, for check 1; a chapter carrying a
# directive mdBook does not recognise, for check 2 (mdBook logs no `ERROR` for
# this one, so check 1 cannot also catch it); a chapter SUMMARY.md names
# but that has no file, both at the top level and nested in a subdirectory,
# for check 3; and an include naming an anchor the file does not have, and one
# naming an anchor with no `ANCHOR_END`, for check 4.
#
# The mdBook guard is deliberate. Making this a `build` dependency makes mdBook
# a hard requirement for every local build, and a missing binary would otherwise
# fail with exit 127 and no explanation (the reasoning that put a
# `command -v rustup` guard on `wasm-check`). The build is a small fraction of a
# second on this book, and `./bootstrap` names mdBook among the tools the gate
# requires.
#
# Given no argument, the recipe runs its fixture first, then the gate over
# this repository. Given a directory, it runs the gate alone, over the book
# at that path. That second form is what the fixture invokes as a child
# process, and it is what stops the fixture from running itself again.
book-check root="":
    #!/usr/bin/env bash
    set -euo pipefail
    if ! command -v mdbook >/dev/null 2>&1; then
        echo "book-check: mdbook is required to build the docs book." >&2
        echo "book-check: install it with 'cargo install mdbook --locked'," >&2
        echo "book-check: or from https://github.com/rust-lang/mdBook/releases." >&2
        exit 1
    fi
    # The gate over the book whose book.toml and docs/ sit in $1. A subshell,
    # so its trap removes its own scratch tree when it returns.
    run_gate() (
        scratch="$(mktemp -d)"
        trap 'rm -rf "$scratch"' EXIT
        cp "$1/book.toml" "$scratch/"
        cp -R "$1/docs" "$scratch/docs"
        # A chapter may include a source from examples/ by anchor, so the
        # code it shows is the code `just demo` builds. Copy the `.rs` and
        # `.ridl` sources too, and nothing else from examples/: no build
        # output, no generated crates.
        if [ -d "$1/examples" ]; then
            (cd "$1" && find examples \( -name '*.rs' -o -name '*.ridl' \) -not -path '*/target/*' -not -path '*/generated/*' -not -path '*/generated-corpus/*') |
                while IFS= read -r source; do
                    mkdir -p "$scratch/$(dirname "$source")"
                    cp "$1/$source" "$scratch/$source"
                done
        fi
        root_docs="$scratch/root-docs.txt"
        (cd "$scratch/docs" && find . -type f | LC_ALL=C sort) >"$root_docs"
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
        scratch_docs="$scratch/scratch-docs.txt"
        (cd "$scratch/docs" && find . -type f | LC_ALL=C sort) >"$scratch_docs"
        created="$scratch/created-docs.txt"
        comm -13 "$root_docs" "$scratch_docs" >"$created"
        if [ -s "$created" ]; then
            echo "book-check: mdBook created a file SUMMARY.md names but the tree does not have:" >&2
            sed 's#^\./#docs/#' "$created" >&2
            exit 1
        fi
        # Check 4: an include by anchor resolves to a whole anchor pair.
        # mdBook renders an empty block, or the rest of the file, and logs
        # nothing, so checks 1 to 3 cannot see it.
        unanchored="$scratch/unanchored.txt"
        : >"$unanchored"
        { grep -roE '[{][{]#include [^}]+[}][}]' "$scratch/docs/book" || true; } |
            while IFS= read -r hit; do
                chapter="${hit%%:*}"
                argument="$(printf '%s\n' "${hit#*:}" | sed -E 's/^[{][{]#include +//; s/ *[}][}]$//')"
                case "$argument" in
                    *:*) ;;
                    *) continue ;;
                esac
                path="${argument%%:*}"
                anchor="${argument#*:}"
                # A line range (`:10`, `:10:20`, `::20`) is not an anchor.
                if printf '%s\n' "$anchor" | grep -qE '^[0-9]*(:[0-9]*)?$'; then
                    continue
                fi
                source="$(dirname "$chapter")/$path"
                # A file that does not exist is check 1's to report.
                [ -f "$source" ] || continue
                if ! grep -qE "ANCHOR:[[:space:]]*${anchor}([^A-Za-z0-9_-]|$)" "$source" ||
                    ! grep -qE "ANCHOR_END:[[:space:]]*${anchor}([^A-Za-z0-9_-]|$)" "$source"; then
                    printf '%s: %s: anchor %s\n' "${chapter#"$scratch"/}" "$path" "$anchor" >>"$unanchored"
                fi
            done
        if [ -s "$unanchored" ]; then
            echo "book-check: an include names an anchor its file does not open with ANCHOR and close with ANCHOR_END:" >&2
            cat "$unanchored" >&2
            exit 1
        fi
    )
    # The fixture. It builds books of its own and runs this recipe over each
    # as a child process, given a root, which is the form that runs the gate
    # and nothing else. It runs no git command, so the git environment a hook
    # exports does not reach it. Twelve cases:
    #
    # 1. A whole book, whose chapters all exist. The gate has to pass.
    # 2. A chapter that includes a file that does not exist. mdBook logs an
    #    `ERROR` line and leaves the directive as literal text; the gate has
    #    to fail and report the logged error (check 1).
    # 3. A chapter carrying a directive mdBook does not recognise. mdBook
    #    leaves it as literal text too, but logs no `ERROR`, so this pins
    #    check 2 on its own: the gate has to fail and report the surviving
    #    directive.
    # 4. The book from case 1, with a SUMMARY.md that also names a chapter
    #    file that does not exist, once at the top level and once nested in a
    #    subdirectory. The gate has to fail, name both files, and leave the
    #    tree unchanged (check 3).
    # 5. The book from case 1 again, built with a stand-in `mdbook` that
    #    deletes a tree file and then runs the real mdbook. The gate has to
    #    pass, because the pre-build file list check 3 compares against comes
    #    from the copy, not from reading the tree again after the build.
    # 6. A chapter that includes a Rust source from examples/ by anchor. The
    #    gate has to pass, because it copies the examples/ sources beside
    #    docs/.
    # 7. The book from case 6, with a chapter that includes an anchor the
    #    source does not have. mdBook renders an empty block and logs nothing;
    #    the gate has to fail and name the chapter, the path and the anchor
    #    (check 4).
    # 8. The book from case 6, with a source whose anchor has no
    #    `ANCHOR_END`. mdBook renders the rest of the file and logs nothing;
    #    the gate has to fail and name the anchor (check 4).
    # 9. The book from case 6, with a `.rs` under examples/'s `target/`, one
    #    under its `generated/` and one under its `generated-corpus/`, each a
    #    symbolic link to a file that does not exist. Copying any one fails,
    #    so the gate has to pass, which pins that the copy leaves the three
    #    directories out.
    # 10. The book from case 6, with a source that closes the anchor with
    #     `ANCHOR_END` but never opens it with `ANCHOR`. The gate has to fail
    #     and name the anchor (check 4).
    # 11. The book from case 6, with a source whose only anchor is `part-x`
    #     while the chapter includes `part`. `part` is a prefix of `part-x`,
    #     not the same name, so the gate has to fail and name the anchor
    #     (check 4).
    # 12. The book from case 6, with a chapter that includes a line range of
    #     the Rust source (`:1:2`), the whole Rust source, and an anchor of a
    #     `.ridl` source under examples/. The gate has to pass: a line range
    #     and a whole-file include name no anchor, and the copy carries the
    #     `.ridl` source.
    fixtures() (
        work="$(mktemp -d)"
        trap 'rm -rf "$work"' EXIT
        run="$work/run"

        # Case 1: a whole book.
        book="$work/fixture"
        mkdir -p "$book/docs/book"
        printf '%s\n' '[book]' 'title = "fixture"' 'src = "docs/book"' > "$book/book.toml"
        printf '%s\n' '# Summary' '' '- [Present](present.md)' > "$book/docs/book/SUMMARY.md"
        printf '%s\n' '# Present' > "$book/docs/book/present.md"
        if ! "{{just_executable()}}" book-check "$book" >"$run" 2>&1; then
            echo "book-check: the gate did not pass over a fixture book whose chapters all exist:" >&2
            cat "$run" >&2
            exit 1
        fi

        # Case 2: a chapter that includes a file that does not exist.
        broken="$work/broken"
        mkdir -p "$broken/docs/book"
        printf '%s\n' '[book]' 'title = "broken"' 'src = "docs/book"' > "$broken/book.toml"
        printf '%s\n' '# Summary' '' '- [Broken](broken.md)' > "$broken/docs/book/SUMMARY.md"
        printf '%s\n' '# Broken' '' '{{{{#include missing-target.md}}' > "$broken/docs/book/broken.md"
        if "{{just_executable()}}" book-check "$broken" >"$run" 2>&1; then
            echo "book-check: the gate returned 0 over a fixture whose chapter includes a file that does not exist:" >&2
            cat "$run" >&2
            exit 1
        fi
        if ! grep -q 'mdBook reported the error above' "$run"; then
            echo "book-check: the gate did not report the broken include's logged error:" >&2
            cat "$run" >&2
            exit 1
        fi

        # Case 3: a chapter carrying a directive mdBook does not recognise.
        # mdBook logs no `ERROR` for this one, so it pins check 2 on its own.
        unresolved="$work/unresolved"
        mkdir -p "$unresolved/docs/book"
        printf '%s\n' '[book]' 'title = "unresolved"' 'src = "docs/book"' > "$unresolved/book.toml"
        printf '%s\n' '# Summary' '' '- [Unresolved](unresolved.md)' > "$unresolved/docs/book/SUMMARY.md"
        printf '%s\n' '# Unresolved' '' '{{{{#nosuchdirective foo}}' > "$unresolved/docs/book/unresolved.md"
        if "{{just_executable()}}" book-check "$unresolved" >"$run" 2>&1; then
            echo "book-check: the gate returned 0 over a fixture whose chapter carries a directive mdBook does not recognise:" >&2
            cat "$run" >&2
            exit 1
        fi
        if grep -q 'mdBook reported the error above' "$run"; then
            echo "book-check: the gate reported a logged error where mdBook logged none, so this case no longer pins check 2 on its own:" >&2
            cat "$run" >&2
            exit 1
        fi
        if ! grep -q 'directive survived into the rendered output' "$run"; then
            echo "book-check: the gate did not report the surviving directive:" >&2
            cat "$run" >&2
            exit 1
        fi

        # Case 4: SUMMARY.md names chapter files that do not exist, one at
        # the top level and one nested in a subdirectory.
        printf '%s\n' '- [Ghost](ghost.md)' '- [Nested ghost](sub/nested.md)' >> "$book/docs/book/SUMMARY.md"
        if "{{just_executable()}}" book-check "$book" >"$run" 2>&1; then
            echo "book-check: the gate returned 0 over a fixture whose SUMMARY.md names chapter files that do not exist:" >&2
            cat "$run" >&2
            exit 1
        fi
        # Built from $d rather than written out, because this file is itself
        # scanned by doc-path-check: a literal docs/… string that resolves
        # nowhere would be reported against this recipe's own line.
        d=docs
        if ! grep -qx "$d/book/ghost.md" "$run"; then
            echo "book-check: the gate did not name the top-level chapter file its fixture is missing:" >&2
            cat "$run" >&2
            exit 1
        fi
        if ! grep -qx "$d/book/sub/nested.md" "$run"; then
            echo "book-check: the gate did not name the nested chapter file its fixture is missing:" >&2
            cat "$run" >&2
            exit 1
        fi
        if [ -e "$book/docs/book/ghost.md" ] || [ -e "$book/docs/book/sub/nested.md" ]; then
            echo "book-check: the gate wrote into the tree it was checking." >&2
            exit 1
        fi

        # Case 5: the tree loses a file while the build runs, after the copy
        # is made. This pins the fix that takes the pre-build file list from
        # the copy instead of re-reading the tree: reading the tree again
        # after the build would see the file gone and wrongly report it as
        # one mdBook created. A stand-in `mdbook`, placed first on PATH,
        # removes the file from the tree and then runs the real mdbook.
        race="$work/race"
        mkdir -p "$race/docs/book"
        printf '%s\n' '[book]' 'title = "race"' 'src = "docs/book"' > "$race/book.toml"
        printf '%s\n' '# Summary' '' '- [Present](present.md)' > "$race/docs/book/SUMMARY.md"
        printf '%s\n' '# Present' > "$race/docs/book/present.md"
        real_mdbook="$(command -v mdbook)"
        stand_in="$work/bin"
        mkdir -p "$stand_in"
        printf '%s\n' \
            '#!/usr/bin/env bash' \
            "rm -f '$race/docs/book/present.md'" \
            "exec '$real_mdbook' \"\$@\"" \
            > "$stand_in/mdbook"
        chmod +x "$stand_in/mdbook"
        if ! PATH="$stand_in:$PATH" "{{just_executable()}}" book-check "$race" >"$run" 2>&1; then
            echo "book-check: the gate reported a file the tree lost during the build as one mdBook created, so it read the tree again after the build instead of using the pre-build copy:" >&2
            cat "$run" >&2
            exit 1
        fi

        # Case 6: a chapter that includes a Rust source from examples/ by
        # anchor. The gate copies only docs/ and the examples/ sources, so this
        # pins the second copy: without it mdBook logs an `ERROR` for the
        # missing file, and the gate fails.
        example="$work/example"
        mkdir -p "$example/docs/book" "$example/examples/demo/src"
        printf '%s\n' '[book]' 'title = "example"' 'src = "docs/book"' > "$example/book.toml"
        printf '%s\n' '# Summary' '' '- [Example](example.md)' > "$example/docs/book/SUMMARY.md"
        printf '%s\n' '# Example' '' '```rust' '{{{{#include ../../examples/demo/src/main.rs:part}}' '```' > "$example/docs/book/example.md"
        printf '%s\n' 'fn main() {' '    // ANCHOR: part' '    let shown = 1;' '    // ANCHOR_END: part' '}' > "$example/examples/demo/src/main.rs"
        if ! "{{just_executable()}}" book-check "$example" >"$run" 2>&1; then
            echo "book-check: the gate did not pass over a fixture whose chapter includes a source from examples/:" >&2
            cat "$run" >&2
            exit 1
        fi

        # Case 7: an include naming an anchor the source does not have.
        printf '%s\n' '# Example' '' '```rust' '{{{{#include ../../examples/demo/src/main.rs:absent}}' '```' > "$example/docs/book/example.md"
        if "{{just_executable()}}" book-check "$example" >"$run" 2>&1; then
            echo "book-check: the gate returned 0 over a fixture whose chapter includes an anchor its source does not have:" >&2
            cat "$run" >&2
            exit 1
        fi
        if ! grep -q 'example.md: ../../examples/demo/src/main.rs: anchor absent$' "$run"; then
            echo "book-check: the gate did not name the chapter, the path and the anchor that does not resolve:" >&2
            cat "$run" >&2
            exit 1
        fi

        # Case 8: an anchor with no ANCHOR_END.
        printf '%s\n' '# Example' '' '```rust' '{{{{#include ../../examples/demo/src/main.rs:part}}' '```' > "$example/docs/book/example.md"
        printf '%s\n' 'fn main() {' '    // ANCHOR: part' '    let shown = 1;' '}' > "$example/examples/demo/src/main.rs"
        if "{{just_executable()}}" book-check "$example" >"$run" 2>&1; then
            echo "book-check: the gate returned 0 over a fixture whose source opens an anchor and never closes it:" >&2
            cat "$run" >&2
            exit 1
        fi
        if ! grep -q 'anchor part$' "$run"; then
            echo "book-check: the gate did not name the anchor that has no ANCHOR_END:" >&2
            cat "$run" >&2
            exit 1
        fi

        # Case 9: the copy leaves target/, generated/ and generated-corpus/
        # out. A symbolic link to a file that does not exist cannot be copied,
        # so the gate fails if the copy reaches any one of them.
        printf '%s\n' 'fn main() {' '    // ANCHOR: part' '    let shown = 1;' '    // ANCHOR_END: part' '}' > "$example/examples/demo/src/main.rs"
        mkdir -p "$example/examples/demo/target/debug" "$example/examples/demo/generated" \
            "$example/examples/demo/generated-corpus"
        ln -s "$work/nowhere.rs" "$example/examples/demo/target/debug/build.rs"
        ln -s "$work/nowhere.rs" "$example/examples/demo/generated/lib.rs"
        ln -s "$work/nowhere.rs" "$example/examples/demo/generated-corpus/lib.rs"
        if ! "{{just_executable()}}" book-check "$example" >"$run" 2>&1; then
            echo "book-check: the gate did not pass over a fixture whose examples/ holds a target/, a generated/ and a generated-corpus/ directory, so it copied one of them:" >&2
            cat "$run" >&2
            exit 1
        fi

        # Case 10: an anchor closed with ANCHOR_END and never opened.
        printf '%s\n' 'fn main() {' '    let shown = 1;' '    // ANCHOR_END: part' '}' > "$example/examples/demo/src/main.rs"
        if "{{just_executable()}}" book-check "$example" >"$run" 2>&1; then
            echo "book-check: the gate returned 0 over a fixture whose source closes an anchor it never opens:" >&2
            cat "$run" >&2
            exit 1
        fi
        if ! grep -q 'anchor part$' "$run"; then
            echo "book-check: the gate did not name the anchor that has no ANCHOR:" >&2
            cat "$run" >&2
            exit 1
        fi

        # Case 11: the source has only an anchor whose name starts with the
        # included name.
        printf '%s\n' 'fn main() {' '    // ANCHOR: part-x' '    let shown = 1;' '    // ANCHOR_END: part-x' '}' > "$example/examples/demo/src/main.rs"
        if "{{just_executable()}}" book-check "$example" >"$run" 2>&1; then
            echo "book-check: the gate returned 0 over a fixture whose chapter includes an anchor that only prefixes the source's anchor:" >&2
            cat "$run" >&2
            exit 1
        fi
        if ! grep -q 'anchor part$' "$run"; then
            echo "book-check: the gate did not name the anchor that only prefixes the source's anchor:" >&2
            cat "$run" >&2
            exit 1
        fi

        # Case 12: a line range, a whole file, and a `.ridl` anchor.
        printf '%s\n' 'fn main() {' '    // ANCHOR: part' '    let shown = 1;' '    // ANCHOR_END: part' '}' > "$example/examples/demo/src/main.rs"
        printf '%s\n' '// ANCHOR: schema' 'package demo' '// ANCHOR_END: schema' > "$example/examples/demo/demo.ridl"
        printf '%s\n' '# Example' '' \
            '```rust' '{{{{#include ../../examples/demo/src/main.rs:1:2}}' '```' '' \
            '```rust' '{{{{#include ../../examples/demo/src/main.rs}}' '```' '' \
            '```text' '{{{{#include ../../examples/demo/demo.ridl:schema}}' '```' \
            > "$example/docs/book/example.md"
        if ! "{{just_executable()}}" book-check "$example" >"$run" 2>&1; then
            echo "book-check: the gate did not pass over a fixture whose chapter includes a line range, a whole file and a .ridl anchor:" >&2
            cat "$run" >&2
            exit 1
        fi
    )
    # One call site, so the fixture above and the real run take the same line.
    root=.
    if [ -n "{{root}}" ]; then
        root="{{root}}"
    else
        fixtures
    fi
    run_gate "$root"

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
#
# Given no argument, the gate runs over the repository this justfile is in, and
# runs its own fixtures first, because a gate that cannot be shown to fail is
# not a gate. Given a directory, it runs over the repository there and runs no
# fixture: that is the form the fixtures invoke as a child process, and it is
# what stops the recursion.
link-check root="":
    #!/usr/bin/env bash
    set -euo pipefail
    # Read the file first and separately from parsing it for links, so a read
    # failure (permission denied, or the file no longer exists) cannot look
    # like "this file cites no links." `-e ''` matches every line of a
    # readable file; `-I` treats a binary file the same as no match, so only a
    # genuine read error exits above 1.
    extract_links() {
        local rq_status=0
        grep -Iq -e '' "$1" >/dev/null 2>&1 || rq_status=$?
        if [ "$rq_status" -gt 1 ]; then
            return 1
        fi
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
    # Report every extracted link that does not resolve under $1, over the
    # file list on stdin. Returns 1 when any did not resolve, or when a file
    # could not be read, and 0 only when every link resolved.
    scan_links() {
        local root="$1" broken=0 file dir target targets
        while IFS= read -r file; do
            if ! targets="$(extract_links "$root/$file")"; then
                echo "link-check: cannot read '$file'." >&2
                return 1
            fi
            dir="$(dirname "$file")"
            if [ -n "$targets" ]; then
                while IFS= read -r target; do
                    [ -e "$root/$dir/$target" ] && continue
                    echo "link-check: $file -> $target" >&2
                    broken=$((broken + 1))
                done <<<"$targets"
            fi
        done
        if [ "$broken" -ne 0 ]; then
            echo "link-check: $broken link(s) above do not resolve." >&2
            return 1
        fi
        return 0
    }
    # A git call that ignores an inherited git environment.
    #
    # This recipe runs from inside a git hook: the pre-push hook invokes `just
    # pre-push`, and a hook exports GIT_DIR. Under that environment `git -C <dir>`
    # changes directory but still reads and writes the repository GIT_DIR names,
    # so the fixture below would build its repository in this one's index rather
    # than its own. Clearing the inherited variables makes every call here act
    # on the directory it is given.
    git_at() {
        env -u GIT_DIR -u GIT_INDEX_FILE -u GIT_WORK_TREE -u GIT_OBJECT_DIRECTORY \
            -u GIT_COMMON_DIR -u GIT_NAMESPACE -u GIT_ALTERNATE_OBJECT_DIRECTORIES \
            git -C "$@"
    }
    # `core.quotePath` is on by default, and it spells a path holding a byte
    # outside ASCII as e.g. "na\303\257ve.md" rather than the bytes as they
    # are on disk. Under that default the pathspec-filtered listing below and
    # the whole-listing-plus-suffix-match expectation would disagree on any
    # such file, and the gate would fail on a file with no broken link at
    # all.
    list_files() {
        git_at "$1" -c core.quotePath=false ls-files "${@:2}"
    }
    # The gate itself, over the repository at $1: every tracked `.md` file has
    # to resolve every relative link it writes.
    run_gate() (
        cd "$1"
        if ! files="$(list_files . '*.md')"; then
            echo "link-check: git ls-files failed; the file list cannot be trusted." >&2
            exit 1
        fi
        # The scanned list has to be exactly the tracked `.md` files. The
        # expectation is derived from the whole listing and a suffix match
        # done in this shell, rather than trusted from the pathspec the call
        # above used, so a `git ls-files` that silently returns less — an
        # empty listing, or an invalid flag accepted without error — cannot
        # pass having checked nothing.
        if ! all_files="$(list_files .)"; then
            echo "link-check: git ls-files failed; the file list cannot be trusted." >&2
            exit 1
        fi
        expected="$(printf '%s\n' "$all_files" | grep -E '\.md$' || true)"
        if [ -z "$expected" ]; then
            echo "link-check: no tracked Markdown files were found; the file list cannot be trusted." >&2
            exit 1
        fi
        if [ "$files" != "$expected" ]; then
            echo "link-check: the file list is not every tracked '*.md' file." >&2
            echo "link-check: '<' would be scanned and should not be; '>' should be and would not:" >&2
            diff <(printf '%s\n' "$files") <(printf '%s\n' "$expected") >&2 || true
            exit 1
        fi
        tracked="$(printf '%s\n' "$files" | grep -c . || true)"
        printf '%s\n' "$files" | scan_links .
        echo "link-check: every relative Markdown link resolves, over $tracked tracked file(s)."
    )
    # The fixtures. Each one builds a case the gate has to pass or fail and
    # fails this recipe when the gate does not. They run in a subshell, so the
    # temporary tree and the git environment set below go no further.
    fixtures() (
        work="$(mktemp -d)"
        trap 'rm -rf "$work"' EXIT
        # The extraction half, unchanged from before this fix: the fence
        # rules the awk script implements.
        sample="$work/sample"
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
        # A file the list names but cannot be read fails the scan rather than
        # being treated as "no links": a read failure and an empty result must
        # not look the same.
        report="$work/report"
        if printf '%s\n' missing.md | scan_links "$work" 2>"$report" \
            || ! grep -q -- "cannot read 'missing.md'" "$report"; then
            echo "link-check: the scan no longer fails on a file it cannot read:" >&2
            cat "$report" >&2
            exit 1
        fi
        # From here on the fixtures use git, and they run under a git
        # environment that names a repository of their own. That is what the
        # clearing in git_at is for, and it is pinned here: without it every
        # call below acts on the decoy rather than on the directory it is
        # given, the listings come back empty and the assertions below fail.
        # It also keeps a call that escapes the clearing away from this
        # repository, which is the accident that has to be made impossible —
        # `just pre-push` runs from the pre-push hook, so an inherited
        # GIT_DIR here is this repository's own.
        decoy="$work/decoy"
        mkdir -p "$decoy"
        # The environment is set before the decoy repository is created, not
        # after, so that the call creating it is covered as well: without the
        # clearing that call reads GIT_DIR, and GIT_DIR has to name the decoy
        # by then rather than this repository.
        export GIT_DIR="$decoy/.git" GIT_WORK_TREE="$decoy"
        git_at "$decoy" -c init.defaultBranch=main -c init.templateDir= init -q
        # A tracked file holding a byte outside ASCII must come back from
        # `list_files` in the spelling it has on disk, not `core.quotePath`'s
        # escaped form — otherwise the pathspec-filtered listing and the
        # whole-listing-plus-suffix-match expectation in `run_gate` disagree
        # on that file and the gate fails it having found no broken link. The
        # repository needs no commit, because git lists the index. Run under
        # the hostile GIT_DIR set above, so a clearing that regresses fails
        # this assertion rather than passing it against the decoy.
        quoting="$work/quoting"
        mkdir -p "$quoting"
        git_at "$quoting" -c init.defaultBranch=main -c init.templateDir= init -q
        name="na$(printf '\303\257')ve.md"
        printf 'no link is cited here\n' > "$quoting/$name"
        git_at "$quoting" -c core.excludesFile=/dev/null add -A
        listed="$(list_files "$quoting")"
        if [ "$listed" != "$name" ]; then
            echo "link-check: the listing did not give the fixture's name as it is spelled on disk:" >&2
            printf '%s\n' "$listed" >&2
            exit 1
        fi
        # The enforcement half. Driven as a child process — this recipe,
        # given a root, which is the form that runs the gate and nothing else
        # — over a repository built for it: first with its one link
        # resolving, then with a second file added whose link does not.
        # The directory is named "sub" rather than the name of this
        # repository's own documentation tree: this justfile is itself
        # scanned by `doc-path-check`, and a literal citation of a path under
        # that name in a grep pattern below would be reported as a broken
        # citation against this recipe's own line.
        #
        # The tracked file with a byte outside ASCII in its name goes into
        # this repository rather than only the isolated `list_files` check
        # above: the bug the quoting fix corrects is in `run_gate`'s file-list
        # comparison, not in `list_files` alone, so only running the whole
        # recipe against such a filename proves that comparison still agrees.
        gate="$work/gate"
        mkdir -p "$gate/sub"
        printf '%s\n' '[present](present.md)' > "$gate/sub/a.md"
        : > "$gate/sub/present.md"
        : > "$gate/sub/na$(printf '\303\257')ve.md"
        git_at "$gate" -c init.defaultBranch=main -c init.templateDir= init -q
        git_at "$gate" -c core.excludesFile=/dev/null add -A
        run="$work/run"
        if ! "{{just_executable()}}" link-check "$gate" >"$run" 2>&1; then
            echo "link-check: the gate did not pass over a fixture whose link resolves:" >&2
            cat "$run" >&2
            exit 1
        fi
        printf '%s\n' '[gone](gone.md)' > "$gate/sub/b.md"
        git_at "$gate" -c core.excludesFile=/dev/null add -A
        if "{{just_executable()}}" link-check "$gate" >"$run" 2>&1; then
            echo "link-check: the gate returned 0 over a fixture citing a link that does not resolve:" >&2
            cat "$run" >&2
            exit 1
        fi
        if ! grep -q -- "sub/b.md -> gone.md" "$run"; then
            echo "link-check: the gate did not report the broken link in its fixture:" >&2
            cat "$run" >&2
            exit 1
        fi
    )
    # One call site, so the fixture above and the real run take the same
    # line: a root argument here skips fixtures for both the top-level call
    # and the recursive call the fixture above makes, which is how that
    # recursive call avoids running fixtures again.
    root=.
    if [ -n "{{root}}" ]; then
        root="{{root}}"
    else
        fixtures
    fi
    run_gate "$root"

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
    # pre-push`, and a hook exports GIT_DIR. Under that environment `git -C <dir>`
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
        # which is the accident that has to be made impossible — `just pre-push`
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

# Check that no story id or plan name is named in shipped text.
#
# Shipped text describes the system as built. A story id such as `E16.5` in a
# rustdoc comment, a book chapter or a design record is a reference to status,
# and status lives in docs/ROADMAP.md and the issue tracker. The reference goes
# stale when the story lands, and nothing else reads it. State the fact, or
# link the tracking issue (`driftsys/ridl#N`) when a gap is real.
#
# Two patterns, each with no letter, digit or underscore directly before or
# after the match, so a number such as `1E5.0` and a name such as `TYPE1.2` do
# not match:
#
# - A story id: an `E`, one or more digits, a dot, one or more digits and an
#   optional lowercase letter (`E2.8b`).
# - A plan name: the word `epic` followed by an epic (`epic E11`, `Epic 10`),
#   the word `stage` followed by a letter, digits and an optional lowercase
#   letter (`stage K3`, `stage K9b`), the word `lane` followed by one capital
#   letter (`lane M`), or an epic id, the word `task` and a task number with an
#   optional lowercase letter (`E2 task 9`, `E2 task 11b`). The word and the
#   letter or number are separated by one space. The word matches in any case;
#   the letter does not, nor does the `E` of an epic id in the last form, so
#   prose such as "a lane a vehicle takes" is not a plan name. A plural such as
#   "E2 tasks 9" is not matched.
#
# The check is a plain text match: it does not read the context, so a match
# that is not a story id or a plan name is reworded rather than exempted. It
# reads one line at a time, so a name wrapped across two lines is not caught;
# keep a name on one line when it is written, and reword one found in a wrapped
# comment.
#
# Scanned, as tracked files: crates/, xtask/, examples/, editors/vscode/src/,
# the docs/book/, docs/design/ and docs/technotes/ trees, and each file of
# docs/specification/ that a docs/book/ file names in an `{{#include}}`, because
# the book renders it. Not scanned, because an id there is the record's own
# subject: docs/ROADMAP.md, docs/BACKLOG.md, docs/decisions/ (a decision traces
# to the story it serves), the rest of docs/specification/ (its overview is the
# ledger of the plan), docs/archive/, docs/wip/, CHANGELOG.md and AGENTS.md.
# Files at the repository root, such as Cargo.toml, are outside the scanned
# trees on purpose.
#
# Given no argument, the gate runs over the repository this justfile is in, and
# runs its own fixtures first, because a gate that cannot be shown to fail is
# not a gate. Given a directory, it runs over the repository there and runs no
# fixture: that is the form the fixtures invoke as a child process, and it is
# what stops the recursion.
story-id-check root="":
    #!/usr/bin/env bash
    set -euo pipefail
    export LC_ALL=C
    # The scanned trees. Each must match a tracked file, so a renamed tree
    # fails the gate instead of leaving it scanning nothing.
    scanned=(crates xtask examples editors/vscode/src docs/book docs/design docs/technotes)
    # Printed by the fixtures when every case passed. The no-argument form
    # fails unless it saw this line, so removing or skipping the fixtures call
    # fails the recipe instead of passing unnoticed.
    fixtures_marker="story-id-check: fixtures passed."
    id_re='(^|[^A-Za-z0-9_])E[0-9]+\.[0-9]+[a-z]?([^A-Za-z0-9_]|$)'
    plan_re='(^|[^A-Za-z0-9_])([Ee][Pp][Ii][Cc] E?[0-9]+|[Ss][Tt][Aa][Gg][Ee] [A-Z][0-9]+[a-z]?|[Ll][Aa][Nn][Ee] [A-Z]|E[0-9]+ [Tt][Aa][Ss][Kk] [0-9]+[a-z]?)([^A-Za-z0-9_]|$)'
    # A git call that ignores an inherited git environment. A hook exports
    # GIT_DIR, which `git -C` does not override; see doc-path-check.
    git_at() {
        env -u GIT_DIR -u GIT_INDEX_FILE -u GIT_WORK_TREE -u GIT_OBJECT_DIRECTORY \
            -u GIT_COMMON_DIR -u GIT_NAMESPACE -u GIT_ALTERNATE_OBJECT_DIRECTORIES \
            git -C "$@"
    }
    # Reads a NUL-separated file list on standard input and prints each match as
    # file:line:text. -H keeps the file name when the list holds one file.
    scan() {
        xargs -0 grep -HInE -e "$id_re" -e "$plan_re" || true
    }
    # The files of docs/specification/ that a docs/book/ file includes.
    included_specs() {
        git_at . -c core.quotePath=off ls-files -z -- docs/book \
            | { xargs -0 grep -hoE '[{][{]#include [^}]*specification/[^} ]*[}][}]' || true; } \
            | sed -n 's#.*specification/\([^} ]*\)[}][}]#docs/specification/\1#p' \
            | sort -u
    }
    run_gate() (
        root="$1"
        cd "$root"
        for tree in "${scanned[@]}"; do
            if [ -z "$(git_at . -c core.quotePath=off ls-files -- "$tree")" ]; then
                echo "story-id-check: the scanned tree '$tree' matches no tracked file; correct the list, or drop the tree if it is gone." >&2
                exit 1
            fi
        done
        while IFS= read -r spec; do
            if [ -z "$(git_at . -c core.quotePath=off ls-files -- "$spec")" ]; then
                echo "story-id-check: a book file includes '$spec', which is not a tracked file; correct the include." >&2
                exit 1
            fi
        done < <(included_specs)
        found="$({ git_at . -c core.quotePath=off ls-files -z -- "${scanned[@]}"
                included_specs | tr '\n' '\0'; } | scan)"
        if [ -n "$found" ]; then
            printf '%s\n' "$found" | sed 's/^\([^:]*:[0-9]*\):/story-id-check: \1: /' >&2
            count="$(printf '%s\n' "$found" | grep -c . || true)"
            echo "story-id-check: $count line(s) above name a story id or a plan name. Delete it, or link the tracking issue as driftsys/ridl#N." >&2
            exit 1
        fi
        echo "story-id-check: no story id in the scanned trees."
    )
    fixtures() (
        work="$(mktemp -d)"
        trap 'rm -rf "$work"' EXIT
        # Ids are assembled with printf so the fixture text does not depend on
        # a literal one.
        # The scanned list is stated again here, so removing a tree from it
        # fails this recipe: the cases below build their trees from the list.
        if [ "${scanned[*]}" != "crates xtask examples editors/vscode/src docs/book docs/design docs/technotes" ]; then
            echo "story-id-check: the scanned trees changed; update the expectation in the fixtures with the list." >&2
            exit 1
        fi
        id="$(printf 'E%s.%s' 16 5)"
        lettered="$(printf 'E%s.%sb' 2 8)"
        # Built from $d: this file is itself scanned by doc-path-check, and a
        # literal docs/… path that resolves nowhere would be reported here.
        d=docs
        # Phrases that read like a plan name and are not one: no match for the
        # letter or digit class, a lowercase letter (including the `e` of an
        # epic id), a bare number, a letter directly before the word or after
        # the name, and the `E<n> task` form with no space, a missing or
        # non-digit number, a missing epic number, a plural word, or a letter
        # glued to the epic id or to the task number.
        plan_clean="a lane a vehicle takes, lane m, stage 2, stage K, stage k3, stage K3xy, lane  M, epic poem, epic e1, upstage K3, plane M, lane Mx, task 9, E2 task, e2 task 9, E2 task x, xE2 task 9, E2 tasks 9, E2 task9, E task 9, 2 task 9, E2 task 9bc"
        # Phrases that are a plan name, in the cases the pattern states.
        plan_names=("epic E11" "Epic 10" "EPIC E1" "stage K3" "stage K3x" "stage K9b" "Stage P4" "STAGE M3" "lane M" "Lane P" "(lane Q's" "E2 task 9" "E2 task 11b" "(E2 task 9)" "E16 task 3" "E2 Task 9")
        root="$work/fixture"
        report="$work/report"
        clean_tree() {
            for tree in "${scanned[@]}"; do
                mkdir -p "$root/$tree"
                printf '%s\n' "// driftsys/ridl#12, 1E5.0, TYPE1.2, $(printf 'E%s.%s' 2 8)bc, $(printf 'E%s.%s' 2 8)B, $(printf 'E%s.%s' 2 8)_1 and _$(printf 'E%s.%s' 1 2) are not story ids; nor are $plan_clean" > "$root/$tree/clean.txt"
            done
            git_at "$root" -c core.excludesFile=/dev/null add -A
        }
        mkdir -p "$root"
        git_at "$root" -c init.defaultBranch=main -c init.templateDir= init -q
        clean_tree
        # An id in a tree that is allowed to hold one.
        mkdir -p "$root/$d/decisions"
        printf '%s\n' "traces to $id" > "$root/$d/decisions/adr.md"
        # A file at the repository root is outside the scanned trees.
        printf '%s\n' "traces to $id, ${plan_names[0]}" > "$root/notes.txt"
        # A specification file that no book file includes is outside them too.
        mkdir -p "$root/$d/specification"
        printf '%s\n' "traces to $id, ${plan_names[3]}" > "$root/$d/specification/other.md"
        git_at "$root" -c core.excludesFile=/dev/null add -A
        if ! "{{just_executable()}}" story-id-check "$root" >"$report" 2>&1; then
            echo "story-id-check: the gate did not pass over a fixture with no id in a scanned tree:" >&2
            cat "$report" >&2
            exit 1
        fi
        # One id in every scanned tree, each reported with its own file and line.
        for tree in "${scanned[@]}"; do
            printf '%s\n' "ok" "// lands with $id, until $lettered" > "$root/$tree/clean.txt"
        done
        git_at "$root" -c core.excludesFile=/dev/null add -A
        if "{{just_executable()}}" story-id-check "$root" >"$report" 2>&1; then
            echo "story-id-check: the gate returned 0 over a fixture naming story ids:" >&2
            cat "$report" >&2
            exit 1
        fi
        for tree in "${scanned[@]}"; do
            if ! grep -q -- "^story-id-check: $tree/clean.txt:2: .*$lettered" "$report"; then
                echo "story-id-check: the gate no longer names file:line, with a lettered id, for $tree:" >&2
                cat "$report" >&2
                exit 1
            fi
        done
        if ! grep -q -- "^story-id-check: ${#scanned[@]} line(s) above" "$report"; then
            echo "story-id-check: the gate no longer counts one line per scanned tree:" >&2
            cat "$report" >&2
            exit 1
        fi
        # A lettered id alone is enough to fail the gate.
        clean_tree
        printf '%s\n' "// see $lettered" > "$root/crates/clean.txt"
        git_at "$root" -c core.excludesFile=/dev/null add -A
        if "{{just_executable()}}" story-id-check "$root" >"$report" 2>&1; then
            echo "story-id-check: the gate returned 0 over a fixture naming only a lettered id:" >&2
            cat "$report" >&2
            exit 1
        fi
        # Each plan name alone is enough to fail the gate.
        for phrase in "${plan_names[@]}"; do
            clean_tree
            printf '%s\n' "// see $phrase" > "$root/crates/clean.txt"
            git_at "$root" -c core.excludesFile=/dev/null add -A
            if "{{just_executable()}}" story-id-check "$root" >"$report" 2>&1 \
                || ! grep -q -- "^story-id-check: crates/clean.txt:1: // see $phrase" "$report"; then
                echo "story-id-check: the gate no longer names the plan name '$phrase':" >&2
                cat "$report" >&2
                exit 1
            fi
        done
        # A specification file that a book file includes is scanned, and one
        # that no book file includes is not.
        clean_tree
        open='{'
        printf '%s\n' "$open$open#include ../specification/inc.md}}" > "$root/$d/book/inc.md"
        printf '%s\n' "traces to $id" > "$root/$d/specification/inc.md"
        git_at "$root" -c core.excludesFile=/dev/null add -A
        if "{{just_executable()}}" story-id-check "$root" >"$report" 2>&1 \
            || ! grep -q -- "^story-id-check: $d/specification/inc.md:1: traces to $id" "$report"; then
            echo "story-id-check: the gate no longer scans a specification file that the book includes:" >&2
            cat "$report" >&2
            exit 1
        fi
        # A book include that names no tracked file fails the gate.
        printf '%s\n' "$open$open#include ../specification/gone.md}}" > "$root/$d/book/gone.md"
        git_at "$root" -c core.excludesFile=/dev/null add -A
        if "{{just_executable()}}" story-id-check "$root" >"$report" 2>&1 \
            || ! grep -q -- "includes '$d/specification/gone.md', which is not a tracked file" "$report"; then
            echo "story-id-check: the gate no longer fails on a book include that resolves to no file:" >&2
            cat "$report" >&2
            exit 1
        fi
        rm "$root/$d/book/inc.md" "$root/$d/specification/inc.md" "$root/$d/book/gone.md"
        # With one file in the list, the match still names the file.
        single="$work/single"
        mkdir -p "$single"
        printf '%s\n' "// $id" > "$single/one.txt"
        if [ "$(cd "$single" && printf 'one.txt\0' | scan)" != "one.txt:1:// $id" ]; then
            echo "story-id-check: a match over a single file no longer names the file." >&2
            exit 1
        fi
        # The no-argument form scans the repository it runs in. The recipe is
        # copied into a fixture repository that holds an id, and run there with
        # no argument and with the nested-run variable set, which skips this
        # function; the scan alone must then fail.
        selfroot="$work/self"
        root="$selfroot"
        mkdir -p "$selfroot"
        git_at "$selfroot" -c init.defaultBranch=main -c init.templateDir= init -q
        clean_tree
        printf '%s\n' "// $id" > "$selfroot/crates/clean.txt"
        cp "{{justfile()}}" "$selfroot/justfile"
        git_at "$selfroot" -c core.excludesFile=/dev/null add -A
        if RIDL_STORY_ID_NESTED=1 "{{just_executable()}}" --justfile "$selfroot/justfile" \
            --working-directory "$selfroot" story-id-check >"$report" 2>&1 \
            || ! grep -q -- "^story-id-check: crates/clean.txt:1: " "$report"; then
            echo "story-id-check: the no-argument form no longer scans the repository it runs in:" >&2
            cat "$report" >&2
            exit 1
        fi
        root="$work/fixture"
        # A scanned tree that has no tracked file fails the gate, with the ids
        # gone so that the status can only come from the missing tree.
        clean_tree
        git_at "$root" rm -q -r -f --cached "$d/technotes"
        if "{{just_executable()}}" story-id-check "$root" >"$report" 2>&1 \
            || ! grep -q -- "the scanned tree '$d/technotes' matches no tracked file" "$report"; then
            echo "story-id-check: the gate no longer fails when a scanned tree has no tracked file:" >&2
            cat "$report" >&2
            exit 1
        fi
        echo "$fixtures_marker"
    )
    if [ -n "{{root}}" ]; then
        run_gate "{{root}}"
    else
        seen=""
        if [ -z "${RIDL_STORY_ID_NESTED:-}" ]; then
            seen="$(fixtures)"
        fi
        if [ -z "${RIDL_STORY_ID_NESTED:-}" ] && [ "$seen" != "$fixtures_marker" ]; then
            echo "story-id-check: the fixtures did not run to the end." >&2
            exit 1
        fi
        run_gate .
    fi

# Check that CI still invokes every recipe the local gate is made of.
#
# The other half of gate parity. CI runs these recipes rather than its own copy
# of their commands, so the commands cannot drift — but a member can still be
# dropped from CI, or added to `build` and never wired into CI, which is how
# `mdbook build` came to run in CI and nowhere else. This compares the two
# lists: every dependency of `build` must appear in .github/workflows/ci.yml as
# a `run:` step invoking `just <recipe>`. It also checks the members of `build`
# against the `expected` list in this recipe, so a member added to or removed
# from `build` without a matching edit here fails.
#
# What it checks is narrow, and the narrowness is the point of this paragraph.
# It checks that the text of a `run: just <member>` step is present in the
# workflow file. It does not check that the step is reached: dropping a job from
# the `ci` aggregate's `needs:`, narrowing `on:`, or adding an `if:` that never
# holds all leave this green while the two gates genuinely diverge. It also says
# nothing about `verify`, `lint-commits`, and `pre-push`, none of which are
# dependencies of `build`.
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
    # The members `build` is expected to have. A member is stated here and on
    # the `build:` line, so removing or adding one has to be done twice. Without
    # this list the check would read its expectation from the line under test,
    # and a member removed from that line would only shrink what is checked.
    expected="toolchain-check gate-parity install-check fmt-check book-check link-check doc-path-check story-id-check compile test lint wasm-check compat-check demo check"
    removed="$(comm -23 <(printf '%s\n' $expected | sort) <(printf '%s\n' $members | sort) | paste -sd' ' -)"
    added="$(comm -13 <(printf '%s\n' $expected | sort) <(printf '%s\n' $members | sort -u) | paste -sd' ' -)"
    duplicated="$(printf '%s\n' $members | sort | uniq -d | paste -sd' ' -)"
    if [ -n "$removed" ] || [ -n "$added" ] || [ -n "$duplicated" ]; then
        [ -z "$removed" ] || echo "gate-parity: removed from 'build': $removed" >&2
        [ -z "$added" ] || echo "gate-parity: added to 'build': $added" >&2
        [ -z "$duplicated" ] || echo "gate-parity: listed twice in 'build': $duplicated" >&2
        echo "gate-parity: update the expected list in the gate-parity recipe to match 'build'." >&2
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
    # install.sh runs under `set -eu`, and the tampered tarball is still a
    # well-formed archive (see above) so nothing before the checksum step can
    # fail: the non-zero exit above and the no-binary check below together
    # already pin the rejection to the checksum step, with no need to also
    # match sha256sum's own message — which driftsys/ridl#501 found is
    # localized by GNU coreutils (e.g. French's "ne correspond pas" instead
    # of "did not match"), making that match fail outside the C locale CI
    # runs in.
    if [ -e "$install2/ridl" ]; then
        echo "install-check: a corrupted download still installed a binary" >&2
        exit 1
    fi
    echo "install-check: tamper case correctly rejected by the checksum and installed nothing"

    # RIDL_INSTALL_BINARY=ridlc: a dry run is enough to prove the wiring —
    # the download/verify/install path past URL construction is the same
    # code already exercised above for ridl.
    ridlc_expected="${url/ridl-/ridlc-}"
    ridlc_url="$(RIDL_VERSION="$version" RIDL_INSTALL_BINARY=ridlc RIDL_INSTALL_DRY_RUN=1 bash install.sh)"
    if [ "$ridlc_url" != "$ridlc_expected" ]; then
        echo "install-check: RIDL_INSTALL_BINARY=ridlc dry run printed '$ridlc_url', expected '$ridlc_expected'" >&2
        exit 1
    fi
    echo "install-check: install.sh RIDL_INSTALL_BINARY=ridlc dry run verified ($ridlc_url)"

    if RIDL_INSTALL_BINARY=bogus bash install.sh 2>"$scratch/bad-binary.err"; then
        echo "install-check: install.sh accepted RIDL_INSTALL_BINARY=bogus" >&2
        exit 1
    fi
    if ! grep -q "RIDL_INSTALL_BINARY must be 'ridl' or 'ridlc'" "$scratch/bad-binary.err"; then
        echo "install-check: install.sh rejected RIDL_INSTALL_BINARY=bogus, but not with the expected message" >&2
        exit 1
    fi
    echo "install-check: install.sh rejects an unknown RIDL_INSTALL_BINARY"

    if command -v pwsh >/dev/null 2>&1; then
        ridlc_ps1_url="$(pwsh -NoProfile -Command '$env:RIDL_VERSION="editor-v0.1.0"; $env:RIDL_INSTALL_BINARY="ridlc"; $env:RIDL_INSTALL_DRY_RUN="1"; ./install.ps1')"
        ridlc_ps1_expected="https://github.com/driftsys/ridl/releases/download/editor-v0.1.0/ridlc-x86_64-pc-windows-msvc.tar.gz"
        if [ "$ridlc_ps1_url" != "$ridlc_ps1_expected" ]; then
            echo "install-check: install.ps1 RIDL_INSTALL_BINARY=ridlc dry run printed '$ridlc_ps1_url', expected '$ridlc_ps1_expected'" >&2
            exit 1
        fi
        echo "install-check: install.ps1 RIDL_INSTALL_BINARY=ridlc dry run verified ($ridlc_ps1_url)"

        if pwsh -NoProfile -Command '$env:RIDL_INSTALL_BINARY="bogus"; ./install.ps1' 2>"$scratch/bad-binary-ps1.err"; then
            echo "install-check: install.ps1 accepted RIDL_INSTALL_BINARY=bogus" >&2
            exit 1
        fi
        if ! grep -q "RIDL_INSTALL_BINARY must be 'ridl' or 'ridlc'" "$scratch/bad-binary-ps1.err"; then
            echo "install-check: install.ps1 rejected RIDL_INSTALL_BINARY=bogus, but not with the expected message" >&2
            exit 1
        fi
        echo "install-check: install.ps1 rejects an unknown RIDL_INSTALL_BINARY"
    fi

# Full local gate: confirm the toolchain and CI wiring, check Rust formatting,
# build the docs book, compile the code, run the tests, lint the Rust, check the
# wasm target builds, then run the connective-tissue lint checks.
#
# The members cover ADR-0008 decision 11's enumeration and the four CI checks
# ADR-0009 brought back to this side. Two of decision 11's were absent until
# issue #182: `cargo fmt --all --check` was in no recipe at all, and `wasm-check`
# was a recipe that nothing depended on. Anything the gate names has to be
# reachable from here, because a member that is not a dependency of `build` is
# not covered by `gate-parity`'s CI-parity check.
#
# The four members that need no compilation run first, so a wrong toolchain, an
# unwired CI job, a formatting regression, or an unparseable SUMMARY.md all
# report before a compile starts rather than after a full compile and test run.
build: toolchain-check gate-parity install-check fmt-check book-check link-check doc-path-check story-id-check compile test lint wasm-check compat-check demo check

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

# Lint and static-check gate before pushing — skips `compile`, `lint`,
# `test`, `wasm-check`, `compat-check`, `demo`, and `install-check`. Wired as the
# pre-push hook (.githooks/pre-push.hooks). CI (ci.yml) still runs the full
# `just build` gate on the PR; `just verify` runs that same gate locally before
# opening one.
pre-push: lint-commits toolchain-check gate-parity fmt-check check doc-path-check story-id-check link-check book-check

# Commit-message lint over commits not yet on origin/main, then build.
# Run before opening a PR.
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

# Verify generic and target-specific VSIX archives: compile, unit tests,
# packaged notices, requested output paths, and the target manifest. A
# placeholder bin/ridl is staged just for this check, so a
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
    for vsce_target in "" linux-x64; do
        archive="$scratch/ridl-lang${vsce_target:+-$vsce_target}.vsix"
        just package-vsix "$vsce_target" "$archive"
        if [ ! -f "$archive" ]; then
            echo "vscode-verify: requested archive was not created: $archive" >&2
            exit 1
        fi
        listing="$(unzip -Z1 "$archive")"
        if ! grep -qx 'extension/bin/ridl' <<<"$listing"; then
            echo "vscode-verify: bin/ridl is missing from the VSIX — check .vscodeignore" >&2
            exit 1
        fi
        unzip -p "$archive" extension/bin/THIRD-PARTY-NOTICES.txt > "$scratch/notices.txt"
        if ! cmp -s ../../THIRD-PARTY-NOTICES.txt "$scratch/notices.txt"; then
            echo "vscode-verify: packaged third-party notices differ from the root notices" >&2
            exit 1
        fi
        if [ -n "$vsce_target" ]; then
            unzip -p "$archive" extension.vsixmanifest > "$scratch/manifest.xml"
            if ! grep -q "TargetPlatform=\"$vsce_target\"" "$scratch/manifest.xml"; then
                echo "vscode-verify: incorrect VSIX target platform" >&2
                exit 1
            fi
        fi
        if grep -q '^extension/src/' <<<"$listing"; then
            echo "vscode-verify: src/ would ship in the VSIX — check .vscodeignore" >&2
            exit 1
        fi
        if grep -q '\.test\.js$' <<<"$listing"; then
            echo "vscode-verify: a compiled test file would ship in the VSIX — check .vscodeignore" >&2
            exit 1
        fi
    done
    echo "vscode-verify: packaged ok"

# Package the extension: compile, then `vsce package`. Assumes the caller has
# already populated editors/vscode/bin/ — this recipe does not build ridl
# itself. With no argument, a plain `vsce package`. With a vsce-target (a
# vsce platform identifier, e.g. darwin-arm64), `vsce package --target
# <vsce-target> --out ridl-lang-<vsce-target>.vsix`, which is what the
# release workflow's package-vsix job runs per target after staging that
# target's binary. An optional output path lets verification use the same
# notice staging and packaging. Not a member of `build`.
package-vsix vsce-target="" output="":
    #!/usr/bin/env bash
    set -euo pipefail
    mkdir -p editors/vscode/bin
    cp THIRD-PARTY-NOTICES.txt editors/vscode/bin/
    cd editors/vscode
    npm ci
    npm run compile
    set --
    if [ -n "{{ vsce-target }}" ]; then set -- "$@" --target "{{ vsce-target }}"; fi
    if [ -n "{{ output }}" ]; then
        set -- "$@" --out "{{ output }}"
    elif [ -n "{{ vsce-target }}" ]; then
        set -- "$@" --out "ridl-lang-{{ vsce-target }}.vsix"
    fi
    npx vsce package "$@"

# Build the extension for this machine: a release build of ridl copied into
# editors/vscode/bin/, then `just package-vsix` for the packaging half. Local
# testing only — the release workflow builds ridl per target in its own job
# and then runs `just package-vsix` with that target's vsce-target. Not a
# member of `build`.
package-vscode:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo build --release --locked -p ridl-cli
    bin=target/release/ridl
    if [ -f target/release/ridl.exe ]; then bin=target/release/ridl.exe; fi
    mkdir -p editors/vscode/bin
    cp "$bin" editors/vscode/bin/
    just package-vsix
