//! The test-time dependency-boundary guard.
//!
//! `ridl-backend-proto` and `ridl-backend-flatbuffers` each carry a schema
//! compiler — `protox`, `planus-translation` — as a test-time validity
//! oracle: it compiles a backend's emitted schema to prove the backend
//! produced something the real target toolchain accepts, and it plays no
//! part in emission itself. Both crates' own `Cargo.toml` files say so in a
//! doc comment above `[dev-dependencies]`. Promoting either oracle to
//! `[dependencies]` would drag a parser generator and its whole dependency
//! tree (`lalrpop`, `sha3`, `term`, and the rest `cargo metadata` resolves
//! under it) into every downstream build — a real cost paid by everything
//! that depends on `ridlc`, for a guarantee those callers never asked for.
//!
//! `just wasm-check` was believed to catch this and does not: it runs
//! `cargo check --target wasm32-unknown-unknown`, and that target compiles
//! `std::fs` as a stub whose calls fail only at runtime, never at compile
//! time. `protox` and `planus-translation` both use ordinary `std`
//! filesystem calls, so moving either into `[dependencies]` still passes
//! `wasm-check` — proven directly: temporarily promoting either one and
//! running the check still succeeds (recorded in the task report, not
//! repeated here as an automated check, because reproducing it would mean
//! shipping the very promotion this guard exists to prevent).
//!
//! This guard also covers the test-time edges of `ridl-backend-rust`.
//! `ridl-backend-rust`'s FlatBuffers conformance test drives planus's
//! runtime and code generator, and emits the `.fbs` it feeds them with
//! `ridl-backend-flatbuffers`. All four edges are test-time only, for the
//! same reason and one more: ADR-0020 decision 9 makes a backend an
//! executable rather than a library other crates link.
//!
//! **Which record each rule rests on.** The test-time-only rule for
//! `ridl-backend-rust`'s four edges rests on ADR-0020 decision 9. The FlatBuffers
//! design note's D-12 says that `ridl-rt`'s `flatbuffers` feature takes no
//! external dependency, so the FlatBuffers runtime crate ADR-0020 decision 5
//! permits there is not used and ADR-0020's RA-01 dependency ceiling stays
//! unspent. The planus rule of the next paragraph rests on ADR-0020 decision
//! 5 as amended 2026-10-03, which excludes planus from that permission.
//!
//! Lane E16 (the catalog descriptor) added a second kind of boundary, a
//! stronger one (lane E16 driver, section 4, answer 8). The toolchain may now
//! depend on planus: `ridl-descriptor` on its runtime, `xtask` on its schema
//! compiler and code generator, and `ridlc` and `ridl` may depend on it
//! through `ridl-descriptor`. `ridl-rt` and every package `ridl build` generates must
//! not depend on any planus crate at all, as a normal, a build or a dev
//! dependency (ADR-0020 decision 5, as amended 2026-10-03), because `ridl-rt`
//! is the crate a generated package links and the generated package is what
//! a user ships. [`RUNTIME_PACKAGES`] covers `ridl-rt` in this workspace.
//!
//! The generated package is the second half of that rule, and this workspace
//! cannot see it: `ridl build` writes it into `examples/cabin/generated/`,
//! and a second crate into `examples/cabin/generated-corpus/`, neither of
//! which is in git, and `examples/cabin` is its own cargo workspace
//! (`AGENTS.md`). [`the_generated_crate_reaches_no_planus_crate`] reads that
//! workspace's resolved graph, so it can run only after both crates are
//! generated. It is `#[ignore]`d for a plain `cargo test`, and `just demo`
//! runs it right after `ridl build` writes them, so `just build` and CI
//! run it on every change.
//!
//! This guard reads the resolved dependency graph instead, via
//! `cargo metadata --format-version 1`, because that is the one place the
//! *kind* of a dependency edge — normal, dev, or build — is recorded
//! unambiguously. The `Cargo.toml` text is not enough on its own: a
//! workspace-inherited dependency (`protox.workspace = true`) reads the
//! same whether it sits under `[dependencies]` or `[dev-dependencies]`, so a
//! textual scan in the `shape_walk.rs` style cannot tell the two apart —
//! only the resolved graph can.

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

/// One boundary this guard protects: `package` may depend on `oracle` only
/// as a dev-dependency, never as a normal one.
struct Boundary {
    /// The backend crate under the constraint.
    package: &'static str,
    /// The crate `package` may reach only through `[dev-dependencies]`.
    oracle: &'static str,
}

const BOUNDARIES: &[Boundary] = &[
    Boundary {
        package: "ridl-backend-flatbuffers",
        oracle: "planus-translation",
    },
    Boundary {
        package: "ridl-backend-proto",
        oracle: "protox",
    },
    // `ridl-backend-rust` uses planus's runtime and code generator for the
    // FlatBuffers codec's conformance test. They
    // are the same kind of oracle: this crate emits the codec rather than
    // linking one, so its own use of planus is test-time only. The toolchain
    // may ship planus through `ridl-descriptor`; what must never reach planus
    // is `ridl-rt` and a generated package (ADR-0020 decision 5 as amended
    // 2026-10-03), which `RUNTIME_PACKAGES` and the generated-crate check
    // below cover.
    Boundary {
        package: "ridl-backend-rust",
        oracle: "planus",
    },
    Boundary {
        package: "ridl-backend-rust",
        oracle: "planus-codegen",
    },
    Boundary {
        package: "ridl-backend-rust",
        oracle: "planus-translation",
    },
    // Not an oracle, the same rule: the conformance test emits the `.fbs`
    // with the schema backend so that the schema and the codec come from one
    // IR. ADR-0020 decision 9 makes a backend an executable rather than a
    // library other crates link, so that edge stays inside the test binary.
    Boundary {
        package: "ridl-backend-rust",
        oracle: "ridl-backend-flatbuffers",
    },
];

/// The planus crates: the FlatBuffers runtime, the code generator and the
/// schema compiler.
const PLANUS_CRATES: &[&str] = &["planus", "planus-codegen", "planus-translation"];

/// The packages under the stronger boundary of lane E16 (driver section 4,
/// answer 8): none of them may reach any of [`PLANUS_CRATES`] through any
/// kind of dependency edge, dev included, because a generated package links
/// them.
const RUNTIME_PACKAGES: &[&str] = &["ridl-rt"];

/// Runs `cargo metadata --format-version 1 --locked` and parses its stdout
/// as JSON. `--locked` matches every other cargo invocation this workspace's
/// gate makes (`justfile`): the lockfile is already the resolved graph, and
/// this guard must read that graph, not silently re-resolve a different one.
fn cargo_metadata() -> serde_json::Value {
    cargo_metadata_of(None, false)
}

/// [`cargo_metadata`] for the workspace whose root manifest is `manifest`,
/// or for this workspace when it is `None`. With `all_features`, every
/// feature of every workspace member is on, so the resolved graph holds
/// every optional dependency, including one that no feature in the workspace
/// turns on.
fn cargo_metadata_of(manifest: Option<&Path>, all_features: bool) -> serde_json::Value {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let mut command = Command::new(cargo);
    command.args(["metadata", "--format-version", "1", "--locked"]);
    if all_features {
        command.arg("--all-features");
    }
    if let Some(manifest) = manifest {
        command.arg("--manifest-path").arg(manifest);
    }
    let output = command.output().expect("`cargo metadata` must run");
    assert!(
        output.status.success(),
        "`cargo metadata` failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("`cargo metadata` must print valid JSON")
}

/// A package id (`cargo metadata`'s pkgid string, e.g.
/// `path+file:///.../ridl-ir#0.0.0`) to the plain crate name it resolves to.
fn package_names(metadata: &serde_json::Value) -> HashMap<&str, &str> {
    metadata["packages"]
        .as_array()
        .expect("cargo metadata carries a `packages` array")
        .iter()
        .map(|pkg| {
            let id = pkg["id"].as_str().expect("a package id is a string");
            let name = pkg["name"].as_str().expect("a package name is a string");
            (id, name)
        })
        .collect()
}

/// Every package reachable from `package` by NORMAL dependency edges only,
/// each mapped to the edge that first reached it, so a failure can print the
/// path rather than only the endpoint.
///
/// **A normal edge is one whose `dep_kinds[].kind` is JSON null.** `"dev"`
/// and `"build"` are the other two, and an edge can carry several kinds at
/// once — a crate that is both a dependency and a dev-dependency has a node
/// with both, which is why the check is "does any kind read null" rather
/// than "does the first".
///
/// **Reachability, not a direct edge.** Checking only `package`'s own edges
/// would pass a promotion one hop away: `planus` moved into `ridl-ir`'s
/// `[dependencies]` puts it in `ridl-backend-rust`'s shipped graph, and so
/// behind `ridlc` and the `ridl` CLI, while producing no direct
/// `ridl-backend-rust` → `planus` edge at all. The walk starts at
/// `package`'s normal edges, so its own dev-dependencies are excluded at the
/// first step and every step after that is normal by construction.
///
/// **An optional dependency is caught whether or not its feature is on.**
/// `cargo metadata` resolves a node's `deps` with the features of the
/// current invocation, but an optional dependency that no feature activates
/// still appears in `packages[].dependencies` and, once any feature in the
/// resolve activates it, in `resolve.nodes[].deps`. This walk reads the
/// resolved nodes, so it catches an optional normal dependency that is
/// activated in this workspace's own resolve — which is the case that would
/// actually ship — and does **not** catch one that nothing activates. That
/// second case cannot reach a downstream build either, so the gap is not a
/// hole in the property this guard states.
///
/// **A build-dependency is a real gap, named here rather than closed.**
/// `[build-dependencies] planus` in `ridl-ir` would compile planus in every
/// downstream build — which is what the failure text below warns of — while
/// producing no normal edge, so this walk passes it. It does not reach the
/// linked artefact, which is the narrower property the guard actually
/// enforces. Following build edges too was considered and rejected on a
/// measurement rather than on taste: `ridl-ir` already carries `protox` as
/// a build-dependency, for compiling the IR's own `.proto` files, and
/// `ridl-backend-proto` depends on `ridl-ir` normally — so a walk over
/// normal-and-build edges fails the `ridl-backend-proto` / `protox`
/// boundary today, on a legitimate edge that has nothing to do with that
/// backend's test oracle. Separating the two would need the guard to know
/// which use of a crate it is looking at, which is a distinction the
/// resolved graph does not carry.
fn normal_closure<'a>(
    metadata: &'a serde_json::Value,
    package: &'a str,
) -> HashMap<&'a str, &'a str> {
    let normal = edges(metadata, |kind| kind.is_none());
    reach(&normal, &normal, package)
}

/// Every package reachable from `package` by any kind of dependency edge on
/// the first step, then by normal and build edges only, each mapped to the
/// edge that first reached it.
///
/// The first step takes every kind because [`RUNTIME_PACKAGES`] must not
/// reach a planus crate as a dev-dependency either. The later steps leave out dev edges because
/// cargo never builds a dependency's own dev-dependencies for its dependents,
/// and keep build edges because cargo compiles those for every build of
/// `package`: a planus crate there would be compiled into the build of every
/// generated package.
fn runtime_closure<'a>(
    metadata: &'a serde_json::Value,
    package: &'a str,
) -> HashMap<&'a str, &'a str> {
    let any = edges(metadata, |_| true);
    let shipped = edges(metadata, |kind| kind != Some("dev"));
    reach(&any, &shipped, package)
}

/// Every resolved node's dependency names, by the node's own name, keeping
/// an edge when `keep` accepts any of its kinds. A kind is `None` for a
/// normal edge (JSON null), otherwise `"dev"` or `"build"`.
fn edges(
    metadata: &serde_json::Value,
    keep: impl Fn(Option<&str>) -> bool,
) -> HashMap<&str, Vec<&str>> {
    let names = package_names(metadata);
    let nodes = metadata["resolve"]["nodes"]
        .as_array()
        .expect("cargo metadata carries `resolve.nodes`");

    let mut edges: HashMap<&str, Vec<&str>> = HashMap::new();
    for node in nodes {
        let id = node["id"].as_str().expect("a node id is a string");
        let from = *names.get(id).expect("every node id names a package");
        let deps = node["deps"]
            .as_array()
            .expect("a resolved node carries a `deps` array")
            .iter()
            .filter(|dep| {
                dep["dep_kinds"]
                    .as_array()
                    .expect("a dependency edge carries a `dep_kinds` array")
                    .iter()
                    .any(|entry| keep(entry["kind"].as_str()))
            })
            .map(|dep| {
                let dep_id = dep["pkg"].as_str().expect("a dep's `pkg` is a string");
                *names.get(dep_id).expect("every dep id names a package")
            })
            .collect();
        edges.insert(from, deps);
    }
    edges
}

/// Walks from `package`: its own edges in `first`, every later step's in
/// `rest`. Returns each package reached, mapped to the package it was first
/// reached from.
fn reach<'a>(
    first: &HashMap<&'a str, Vec<&'a str>>,
    rest: &HashMap<&'a str, Vec<&'a str>>,
    package: &'a str,
) -> HashMap<&'a str, &'a str> {
    assert!(
        first.contains_key(package),
        "cargo metadata's resolved graph has no node for `{package}` — is it \
         still a workspace member?"
    );

    let mut reached: HashMap<&str, &str> = HashMap::new();
    let mut queue: Vec<&str> = vec![package];
    while let Some(from) = queue.pop() {
        let step = if from == package { first } else { rest };
        for &to in step.get(from).into_iter().flatten() {
            if reached.contains_key(to) || to == package {
                continue;
            }
            reached.insert(to, from);
            queue.push(to);
        }
    }
    reached
}

/// The chain of edges from `package` to `oracle`, as `a -> b -> oracle`,
/// read back out of the predecessor map [`normal_closure`] or
/// [`runtime_closure`] returns.
fn normal_path(reached: &HashMap<&str, &str>, package: &str, oracle: &str) -> String {
    let mut chain = vec![oracle];
    let mut at = oracle;
    while let Some(&from) = reached.get(at) {
        chain.push(from);
        if from == package {
            break;
        }
        at = from;
    }
    chain.reverse();
    chain.join(" -> ")
}

/// Every [`Boundary`] holds: no oracle is reachable from its backend crate
/// through normal dependency edges, at any distance.
#[test]
fn schema_compilers_stay_dev_dependencies() {
    let metadata = cargo_metadata();
    for boundary in BOUNDARIES {
        let reached = normal_closure(&metadata, boundary.package);
        assert!(
            !reached.contains_key(boundary.oracle),
            "\n\
             `{oracle}` is in `{package}`'s NORMAL dependency closure: \
             {path}\n\
             \n\
             `{oracle}` is test-time only for `{package}` and plays no part \
             in emission — see `{package}`'s own `Cargo.toml` doc comment. A \
             normal edge anywhere on that chain drags `{oracle}` and its \
             whole dependency tree into every downstream build. Move the \
             offending edge back to `[dev-dependencies]`.\n",
            package = boundary.package,
            oracle = boundary.oracle,
            path = normal_path(&reached, boundary.package, boundary.oracle),
        );
    }
}

/// Every package in [`RUNTIME_PACKAGES`] holds: no crate of [`PLANUS_CRATES`]
/// is reachable from it through any dependency edge, dev included (lane E16 driver,
/// section 4, answer 8).
///
/// The graph is resolved with `--all-features`. A user's generated package
/// can turn on any `ridl-rt` feature, so a planus crate behind a feature that
/// nothing in this workspace turns on still reaches a shipped build, and
/// this test must see it.
#[test]
fn the_runtime_reaches_no_planus_crate() {
    let metadata = cargo_metadata_of(None, true);
    for &package in RUNTIME_PACKAGES {
        let reached = runtime_closure(&metadata, package);
        for &forbidden in PLANUS_CRATES {
            assert!(
                !reached.contains_key(forbidden),
                "\n\
                 `{forbidden}` is in `{package}`'s dependency closure: {path}\n\
                 \n\
                 `{package}` is the crate every generated package links, so it \
                 must not depend on planus in any way, not even as a \
                 dev-dependency (lane E16 driver, section 4, answer 8). The \
                 toolchain may reach planus through `ridl-descriptor` instead.\n",
                path = normal_path(&reached, package, forbidden),
            );
        }
    }
}

/// No planus crate is in the resolved graph of `examples/cabin`, the
/// workspace that holds the crate `ridl build` generates for `cabin.ridl` and
/// the program that links it (lane E16 driver, section 4, answer 8).
///
/// `examples/cabin/generated/` and `examples/cabin/generated-corpus/` are
/// written by `just demo` and are not in git, so `cargo metadata` cannot
/// resolve the workspace before that, and this
/// test is ignored for a plain `cargo test`. `just demo` runs it with
/// `--ignored` right after `ridl build` writes both crates.
///
/// The graph is resolved with `--all-features`, so a planus crate behind a
/// feature of the generated crate or of the consumer that nothing turns on
/// still fails this test.
#[test]
#[ignore = "needs examples/cabin/generated and generated-corpus, which `just demo` writes; `just demo` runs it"]
fn the_generated_crate_reaches_no_planus_crate() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/cabin/Cargo.toml");
    let metadata = cargo_metadata_of(Some(&manifest), true);
    let names = package_names(&metadata);
    assert!(
        names.values().any(|&name| name == "veh_cabin"),
        "the resolved graph of `examples/cabin` has no `veh_cabin` package — \
         did `ridl build` write `examples/cabin/generated/` and \
         `examples/cabin/generated-corpus/`?"
    );
    for forbidden in PLANUS_CRATES {
        assert!(
            !names.values().any(|name| name == forbidden),
            "\n\
             `{forbidden}` is in the resolved graph of `examples/cabin`, the \
             workspace of the crate `ridl build` generates. A generated \
             package must not depend on planus in any way (lane E16 driver, \
             section 4, answer 8): look for the edge in the generated \
             manifest, in `ridl-rt`, or in `ridl-loopback`.\n",
        );
    }
}

/// The names of the crates `cargo tree` lists in the normal dependency graph
/// of `ridl-descriptor` with the given extra arguments.
///
/// `cargo metadata` cannot answer this: it takes one feature set for the
/// whole workspace, and other members turn on `ridl-descriptor`'s defaults.
/// `cargo tree -p` resolves the named package with its own feature flags.
fn descriptor_tree(extra: &[&str]) -> String {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let output = Command::new(cargo)
        .args(["tree", "--locked", "-p", "ridl-descriptor", "-e", "normal"])
        .args(["--prefix", "none"])
        .args(extra)
        .output()
        .expect("`cargo tree` must run");
    assert!(
        output.status.success(),
        "`cargo tree` failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("`cargo tree` prints UTF-8")
}

/// `ridl-descriptor` with its default features off reaches neither
/// `hashbrown` nor `foldhash`, so a consumer that only reads descriptors
/// links neither (and needs no Zlib licence exception). With the default
/// features on, `hashbrown` is reachable through the `dedup` feature, which
/// shows that the first half of this test can fail.
#[test]
fn the_descriptor_reader_reaches_no_hashing_crate() {
    let reader = descriptor_tree(&["--no-default-features"]);
    for forbidden in ["hashbrown", "foldhash"] {
        assert!(
            !reader.lines().any(|line| line.starts_with(forbidden)),
            "`{forbidden}` is in `ridl-descriptor`'s normal graph with default \
             features off:\n{reader}\n\
             A planus builder cache must stay behind the `dedup` feature."
        );
    }
    let default = descriptor_tree(&[]);
    assert!(
        default.lines().any(|line| line.starts_with("hashbrown")),
        "`hashbrown` is not in `ridl-descriptor`'s graph with default features \
         on, so the check above cannot fail:\n{default}"
    );
}
