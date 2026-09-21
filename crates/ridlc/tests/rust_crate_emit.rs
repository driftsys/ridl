//! `--emit rust` writes a compiling crate (Task 7,
//! docs/wip/typl-value-objects-plan.md): a `Cargo.toml` and `lib.rs` alongside
//! the per-package `.rs` files, so the flat emitted files resolve at the
//! `crate::…` paths `ridl_backend_rust::type_path` already generates.
//!
//! The property has three exceptions, each tested below rather than assumed
//! away. A build that draws an error-severity diagnostic writes neither file,
//! and a build whose output directory already holds a `lib.rs` or a
//! `Cargo.toml` that ridlc did not write writes nothing at all. Neither is a
//! crate that fails to compile — both are the absence of one. The third is:
//! issue #416 records a legal package naming case whose generated path does
//! not resolve — a package `veh.common` alongside a type named `common` in
//! package `veh` — which rustc reports as E0573.

use std::path::{Path, PathBuf};

use ridlc::Emit;

mod support;

/// Writes a one-package fixture (a `ridl.toml` naming `package_name` and one
/// source file) into `dir` and returns the package directory, for the package
/// shapes no corpus fixture has: a name whose segment is a Rust keyword, a
/// single-segment name, and a package whose Rust generation fails.
fn write_package_fixture(dir: &Path, package_name: &str, source: &str) -> PathBuf {
    let package_dir = dir.join("pkg");
    std::fs::create_dir_all(&package_dir).expect("the package directory is created");
    std::fs::write(
        package_dir.join("ridl.toml"),
        format!("[package]\nname = \"{package_name}\"\nversion = \"1.0.0\"\n"),
    )
    .expect("the manifest is written");
    std::fs::write(package_dir.join("source.typl"), source).expect("the source is written");
    package_dir
}

/// Compiles `<dir>/lib.rs` as a crate root with `rustc`, which is the only
/// thing that proves a module tree resolves. Panics with the crate root's own
/// text when it does not compile.
///
/// The `ridl-rt` rlib is built into its own directory, so a test that reads
/// `dir` afterwards sees only what the build wrote.
fn compile_crate_root(dir: &Path) {
    let rlib_dir = tempfile::tempdir().expect("a temp dir is created");
    let rlib = support::ridl_rt_rlib(rlib_dir.path());
    let status = std::process::Command::new("rustc")
        .current_dir(dir)
        .args([
            "--edition",
            "2024",
            "--crate-type",
            "lib",
            "--emit",
            "metadata",
            "-o",
        ])
        .arg(dir.join("out.rmeta"))
        .arg("--extern")
        .arg(format!("ridl_rt={}", rlib.display()))
        .arg("lib.rs")
        .status()
        .expect("rustc must be installed and runnable for this test to be meaningful");
    let lib = std::fs::read_to_string(dir.join("lib.rs")).unwrap_or_default();
    assert!(
        status.success(),
        "the emitted crate root must compile, lib.rs was:\n{lib}"
    );
}

/// Every file named by a `#[path = "…"]` attribute in a generated crate root,
/// in the order they appear. The inline-module anchor `#[path = "."]` is not
/// one: it names the output directory, not a file.
fn path_attribute_targets(lib: &str) -> Vec<String> {
    const OPEN: &str = "#[path = \"";
    lib.match_indices(OPEN)
        .map(|(index, _)| {
            let rest = &lib[index + OPEN.len()..];
            let end = rest.find('"').expect("a path attribute is closed");
            rest[..end].to_string()
        })
        .filter(|target| target != ".")
        .collect()
}

/// Every per-package `.rs` file in `dir`, sorted — the crate root itself is
/// not one. An empty result is what a build that wrote nothing looks like.
fn emitted_package_sources(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<String> = entries
        .map(|entry| entry.expect("a directory entry is readable").file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".rs") && name != "lib.rs")
        .collect();
    files.sort();
    files
}

/// Writes a two-member workspace into `dir` where one package is the parent of
/// the other — package `veh` alongside package `veh.common` — and returns the
/// workspace root.
///
/// No corpus fixture has this shape: every package name in every one of them
/// is exactly two segments, and each fixture declares one package, so no
/// existing test ever produces a package that is also the parent of another.
/// That is the one branch `render_lib_rs` builds by hand, the one that loads
/// `veh`'s own file as a private module and re-exports it.
///
/// `veh.common` names a type from `veh`, so the crate root must make
/// `crate::veh::Speed` resolve as well as `crate::veh::common::Reading`.
fn write_prefix_package_workspace(dir: &Path) -> PathBuf {
    let root = dir.join("workspace");
    std::fs::create_dir_all(root.join("member-veh")).expect("the parent member directory");
    std::fs::create_dir_all(root.join("member-common")).expect("the child member directory");
    std::fs::write(
        root.join("ridl.toml"),
        "[workspace]\nmembers = [\"member-veh\", \"member-common\"]\n",
    )
    .expect("the workspace manifest is written");

    std::fs::write(
        root.join("member-veh/ridl.toml"),
        "[package]\nname = \"veh\"\nversion = \"1.0.0\"\n",
    )
    .expect("the parent manifest is written");
    std::fs::write(
        root.join("member-veh/veh.typl"),
        "package veh\n\n/// Vehicle speed over ground\ntype Speed : km/h [0.0..250.0 step 0.5]\n",
    )
    .expect("the parent source is written");

    std::fs::write(
        root.join("member-common/ridl.toml"),
        "[package]\nname = \"veh.common\"\nversion = \"1.0.0\"\n",
    )
    .expect("the child manifest is written");
    std::fs::write(
        root.join("member-common/common.typl"),
        "package veh.common\n\nimport veh.Speed\n\n/// A reading carrying a speed from the \
         parent package.\nstruct Reading {\n  speed : Speed\n}\n",
    )
    .expect("the child source is written");

    root
}

/// `run_build` over a multi-package workspace with `&[Emit::Rust]` writes a
/// `Cargo.toml` and a `lib.rs` alongside the per-package sources, and the
/// manifest carries the `validate-pattern`/`std` feature set and the `regex`
/// and `ridl-rt` dependencies the plan specifies.
#[test]
fn rust_emit_writes_a_compiling_crate() {
    let out = tempfile::tempdir().expect("temp dir");
    let run = ridlc::run_build(
        Path::new("tests/corpus/workspace-two-members"),
        out.path(),
        &[Emit::Rust],
        false.into(),
    )
    .expect("build runs");
    assert!(
        !run.has_error(),
        "expected no error, got: {:?}",
        run.diagnostics
    );

    assert!(out.path().join("Cargo.toml").exists());
    assert!(out.path().join("lib.rs").exists());

    let manifest = std::fs::read_to_string(out.path().join("Cargo.toml")).unwrap();
    assert!(manifest.contains("default = [\"validate-pattern\", \"std\"]"));
    assert!(manifest.contains("regex = { version = \"1\", optional = true }"));
    assert!(manifest.contains("ridl-rt = "));

    let lib = std::fs::read_to_string(out.path().join("lib.rs")).unwrap();
    assert!(lib.contains("pub mod veh"), "lib.rs was:\n{lib}");
}

/// Both generated files begin with their own generated-by marker. The two
/// lines are load-bearing rather than decorative: a build refuses to overwrite
/// a `lib.rs` or a `Cargo.toml` that does not begin with them
/// (`refusing_to_overwrite_*` below), so a renderer that stopped writing one
/// would turn every subsequent build of the same output directory into a
/// refusal.
#[test]
fn both_generated_files_carry_their_generated_by_marker() {
    let out = tempfile::tempdir().expect("temp dir");
    let run = ridlc::run_build(
        Path::new("tests/corpus/workspace-two-members"),
        out.path(),
        &[Emit::Rust],
        false.into(),
    )
    .expect("build runs");
    assert!(
        !run.has_error(),
        "expected no error, got: {:?}",
        run.diagnostics
    );

    let lib = std::fs::read_to_string(out.path().join("lib.rs")).unwrap();
    assert!(
        lib.starts_with("// Generated by ridlc. Do not edit.\n"),
        "lib.rs must open with the crate-root marker, was:\n{lib}"
    );
    let manifest = std::fs::read_to_string(out.path().join("Cargo.toml")).unwrap();
    assert!(
        manifest.starts_with("# Generated by ridlc. Do not edit.\n"),
        "Cargo.toml must open with the manifest marker, was:\n{manifest}"
    );
}

/// THE INVARIANT. Every assertion above names one file by hand, so a package
/// silently dropped from the module tree passes all of them — and passes the
/// `rustc` proof too, which happily compiles a crate root that merely omits a
/// module. This states the property instead, in both directions: every
/// emitted `.rs` file is named by exactly one `#[path]` in the crate root, and
/// every `#[path]` in the crate root names a file that exists.
#[test]
fn every_emitted_file_is_named_exactly_once_in_the_crate_root() {
    let out = tempfile::tempdir().expect("temp dir");
    let run = ridlc::run_build(
        Path::new("tests/corpus/workspace-two-members"),
        out.path(),
        &[Emit::Rust],
        false.into(),
    )
    .expect("build runs");
    assert!(
        !run.has_error(),
        "expected no error, got: {:?}",
        run.diagnostics
    );

    let lib = std::fs::read_to_string(out.path().join("lib.rs")).unwrap();
    let targets = path_attribute_targets(&lib);

    let emitted = emitted_package_sources(out.path());
    assert!(
        emitted.len() > 1,
        "the fixture must emit several packages, or this test proves nothing, got: {emitted:?}"
    );

    for file in &emitted {
        let named = targets.iter().filter(|target| *target == file).count();
        assert_eq!(
            named, 1,
            "`{file}` must be named by exactly one #[path] in the crate root, it is named \
             {named} times; lib.rs was:\n{lib}"
        );
    }
    for target in &targets {
        assert!(
            out.path().join(target).exists(),
            "the crate root names `{target}`, which no build wrote; lib.rs was:\n{lib}"
        );
    }
}

/// The invariant again, over the one package shape no corpus fixture has: a
/// package that is also the parent of another (`veh` alongside `veh.common`).
/// That shape is the only thing that reaches the branch `render_lib_rs` builds
/// by hand — the private `__ridl_package` module and its `pub use` — so over
/// the two-segment, one-package-each fixtures every other test uses, deleting
/// that branch outright changes nothing any assertion can see.
///
/// Three things are proved here, because each catches a different way of
/// breaking the branch. The invariant catches `veh.rs` being dropped from the
/// tree, which is what deleting the whole branch does. `rustc` catches
/// `crate::veh::Speed` failing to resolve, which is what deleting the `pub use`
/// does — the crate root still parses and still names every file. The third is
/// the module's own visibility: `__ridl_package` is not a path any generated
/// reference uses, and a `pub` one would put a second public path to every item
/// in package `veh` into the crate's API, which neither of the other two
/// notices.
#[test]
fn a_package_that_is_also_a_parent_is_named_once_and_re_exported() {
    let fixture = tempfile::tempdir().expect("temp fixture dir");
    let entry = write_prefix_package_workspace(fixture.path());

    let out = tempfile::tempdir().expect("temp dir");
    let run =
        ridlc::run_build(&entry, out.path(), &[Emit::Rust], false.into()).expect("build runs");
    assert!(
        !run.has_error(),
        "expected no error, got: {:?}",
        run.diagnostics
    );

    let lib = std::fs::read_to_string(out.path().join("lib.rs")).unwrap();
    let targets = path_attribute_targets(&lib);
    let emitted = emitted_package_sources(out.path());
    assert!(
        emitted.contains(&"veh.rs".to_string()) && emitted.contains(&"veh.common.rs".to_string()),
        "the fixture must emit both the parent and the child package, got: {emitted:?}"
    );

    for file in &emitted {
        let named = targets.iter().filter(|target| *target == file).count();
        assert_eq!(
            named, 1,
            "`{file}` must be named by exactly one #[path] in the crate root, it is named \
             {named} times; lib.rs was:\n{lib}"
        );
    }
    for target in &targets {
        assert!(
            out.path().join(target).exists(),
            "the crate root names `{target}`, which no build wrote; lib.rs was:\n{lib}"
        );
    }

    assert!(
        lib.contains("pub use __ridl_package::*;"),
        "package `veh`'s own items must be re-exported at `crate::veh`, which is the path \
         `veh.common`'s reference to `Speed` uses; lib.rs was:\n{lib}"
    );
    assert!(
        !lib.contains("pub mod __ridl_package;"),
        "the module the parent's file is loaded under is an implementation detail and stays \
         private; lib.rs was:\n{lib}"
    );

    compile_crate_root(out.path());
}

/// The emitted manifest is parsed as TOML and asserted as structure, not as a
/// handful of substrings: a `[lib]` section deleted, an `edition` changed, a
/// `[dependencies]` header deleted so both dependencies become feature keys,
/// a feature deleted while `default` still names it, or a feature list emptied
/// all leave every substring assertion above passing, and each of them breaks
/// the crate the build claims to have written.
#[test]
fn the_emitted_manifest_parses_and_carries_the_declared_structure() {
    use std::collections::BTreeMap;

    #[derive(serde::Deserialize)]
    struct GeneratedManifest {
        package: GeneratedPackage,
        lib: GeneratedLib,
        features: BTreeMap<String, Vec<String>>,
        dependencies: BTreeMap<String, Dependency>,
    }
    #[derive(serde::Deserialize)]
    struct GeneratedPackage {
        name: String,
        version: String,
        edition: String,
    }
    #[derive(serde::Deserialize)]
    struct GeneratedLib {
        path: String,
    }
    #[derive(serde::Deserialize)]
    #[serde(untagged)]
    enum Dependency {
        Plain(String),
        Detailed {
            version: String,
            #[serde(default)]
            optional: bool,
        },
    }
    impl Dependency {
        fn version(&self) -> &str {
            match self {
                Dependency::Plain(version) => version,
                Dependency::Detailed { version, .. } => version,
            }
        }
        fn optional(&self) -> bool {
            match self {
                Dependency::Plain(_) => false,
                Dependency::Detailed { optional, .. } => *optional,
            }
        }
    }

    let out = tempfile::tempdir().expect("temp dir");
    let run = ridlc::run_build(
        Path::new("tests/corpus/veh-common"),
        out.path(),
        &[Emit::Rust],
        false.into(),
    )
    .expect("build runs");
    assert!(
        !run.has_error(),
        "expected no error, got: {:?}",
        run.diagnostics
    );

    let text = std::fs::read_to_string(out.path().join("Cargo.toml")).unwrap();
    let manifest: GeneratedManifest = toml::from_str(&text)
        .unwrap_or_else(|err| panic!("the manifest must parse: {err}\n{text}"));

    assert_eq!(manifest.package.name, "veh_common");
    assert_eq!(
        manifest.package.version, "0.0.0",
        "the generated crate is unpublished and says so; a version cargo rejects (`0`, \
         `0.0.0.0`) is still TOML that parses and Rust that compiles"
    );
    assert_eq!(
        manifest.package.edition, "2024",
        "the generated code is edition 2024"
    );
    assert_eq!(
        manifest.lib.path, "lib.rs",
        "the crate root is the generated lib.rs, not src/lib.rs"
    );

    let features: BTreeMap<String, Vec<String>> = manifest.features;
    assert_eq!(
        features.keys().collect::<Vec<_>>(),
        vec!["default", "std", "validate-pattern"],
        "the feature table must hold exactly these three features, was: {features:?}"
    );
    assert_eq!(
        features["default"],
        vec!["validate-pattern".to_string(), "std".to_string()],
        "both features are on by default"
    );
    assert_eq!(
        features["validate-pattern"],
        vec!["dep:regex".to_string()],
        "the feature must actually enable the optional regex dependency"
    );
    assert!(
        features["std"].is_empty(),
        "`std` gates generated code only, it enables no dependency"
    );

    let dependencies = manifest.dependencies;
    assert_eq!(
        dependencies.keys().collect::<Vec<_>>(),
        vec!["regex", "ridl-rt"],
        "the generated crate depends on exactly these two crates, was: {:?}",
        dependencies.keys().collect::<Vec<_>>()
    );
    assert!(
        dependencies["regex"].optional(),
        "regex is optional — `validate-pattern` is what enables it"
    );
    assert_eq!(
        dependencies["regex"].version(),
        "1",
        "the regex requirement is the major version alone"
    );
    assert!(
        !dependencies["ridl-rt"].optional(),
        "ridl-rt is not optional: every generated constructor names it"
    );
    assert!(
        !dependencies["ridl-rt"].version().is_empty(),
        "ridl-rt carries a version requirement; \
         `ridl_rt_version_requirement_matches_the_crate` pins which one"
    );
}

/// A typl package segment may be a Rust keyword — `is_valid_name_segment`
/// accepts any lowercase segment, so `veh.mod` is a legal package name. The
/// crate root must escape it the same way `ridl_backend_rust::type_path`
/// escapes it in a cross-package reference, which is what
/// `ridl_backend_rust::module_segment` is for. Written raw, the crate root
/// says `pub mod mod;` and does not even parse.
#[test]
fn a_keyword_package_segment_emits_a_crate_root_that_compiles() {
    let fixture = tempfile::tempdir().expect("temp fixture dir");
    let entry = write_package_fixture(
        fixture.path(),
        "veh.mod",
        "package veh.mod\n\ntype Speed : km/h [0.0..250.0 step 0.5]\n",
    );

    let out = tempfile::tempdir().expect("temp dir");
    let run =
        ridlc::run_build(&entry, out.path(), &[Emit::Rust], false.into()).expect("build runs");
    assert!(
        !run.has_error(),
        "expected no error, got: {:?}",
        run.diagnostics
    );

    let lib = std::fs::read_to_string(out.path().join("lib.rs")).unwrap();
    assert!(
        lib.contains("pub mod r#mod;"),
        "the keyword segment must be escaped as a raw identifier, lib.rs was:\n{lib}"
    );
    compile_crate_root(out.path());
}

/// The same rule over every segment that forces a spelling, rather than the
/// single `mod` the test above pins. `mod` alone leaves the crate root's
/// spelling satisfiable by a function hardcoded to that one keyword, which is
/// not what `ridl_backend_rust::module_segment` does and not what a package
/// name may contain: `is_valid_name_segment` accepts any lowercase ASCII
/// segment, so all five of these are legal typl package names.
///
/// The two escapes are different, which is the point of the table. `mod` and
/// `fn` become raw identifiers. `crate`, `self` and `super` cannot be raw
/// identifiers at all — `r#crate` is not valid Rust — so they take a trailing
/// underscore instead. A crate root is compiled for a mangled case as well as
/// a raw one, because only the mangled path leaves the segment spelled
/// differently from the package name.
///
/// `type` is absent, and only because a build cannot reach the crate root with
/// it: `ridl_core::manifest`'s `is_valid_name_segment` accepts `veh.type`, but
/// `type` is a family-reserved word (typl §1.4), so the `package veh.type`
/// declaration the sources must carry does not parse. Its spelling is pinned
/// where it can be, in `ridl_backend_rust`'s own
/// `module_segment_spells_a_segment_the_way_type_path_does`.
#[test]
fn every_keyword_package_segment_is_escaped_the_way_a_reference_spells_it() {
    for (segment, expected, compile) in [
        ("mod", "pub mod r#mod;", false),
        ("fn", "pub mod r#fn;", false),
        ("crate", "pub mod crate_;", true),
        ("self", "pub mod self_;", true),
        ("super", "pub mod super_;", false),
    ] {
        let fixture = tempfile::tempdir().expect("temp fixture dir");
        let entry = write_package_fixture(
            fixture.path(),
            &format!("veh.{segment}"),
            &format!("package veh.{segment}\n\ntype Speed : km/h [0.0..250.0 step 0.5]\n"),
        );

        let out = tempfile::tempdir().expect("temp dir");
        let run =
            ridlc::run_build(&entry, out.path(), &[Emit::Rust], false.into()).expect("build runs");
        assert!(
            !run.has_error(),
            "`veh.{segment}` is a legal package name, got: {:?}",
            run.diagnostics
        );

        let lib = std::fs::read_to_string(out.path().join("lib.rs")).unwrap();
        assert!(
            lib.contains(expected),
            "`veh.{segment}` must reach the crate root as `{expected}`, \
             which is how `ridl_backend_rust::type_path` spells it in a reference; lib.rs was:\n\
             {lib}"
        );
        if compile {
            compile_crate_root(out.path());
        }
    }
}

/// A warning does not stop the crate files. The re-check after the write loop
/// asks for an error-severity diagnostic specifically, and relaxing it to any
/// diagnostic at all passes every other test here, because every other fixture
/// used is warning-free and none of them asserts that its diagnostics are
/// empty.
///
/// The warning is TYPL-103: a `string` with no explicit bounds takes the
/// `[0..256]` default and says so (typl §4.4). The test asserts the warning is
/// present before it asserts the files are written, so a fixture that stopped
/// warning would fail here rather than quietly stop testing the severity.
#[test]
fn a_warning_does_not_stop_the_crate_files() {
    let fixture = tempfile::tempdir().expect("temp fixture dir");
    let entry = write_package_fixture(
        fixture.path(),
        "app",
        "package app\n\n/// A name with no explicit length bound.\ntype Tag : string\n",
    );

    let out = tempfile::tempdir().expect("temp dir");
    let run =
        ridlc::run_build(&entry, out.path(), &[Emit::Rust], false.into()).expect("build runs");
    assert!(
        !run.has_error(),
        "an unbounded string warns, it does not error, got: {:?}",
        run.diagnostics
    );
    assert!(
        run.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.as_str() == "TYPL-103"),
        "the fixture must draw the typl §4.4 default-bound warning, or this test proves \
         nothing about severity, got: {:?}",
        run.diagnostics
    );

    assert!(
        out.path().join("lib.rs").exists(),
        "a build that only warns still writes the crate root"
    );
    assert!(
        out.path().join("Cargo.toml").exists(),
        "a build that only warns still writes the manifest"
    );
    compile_crate_root(out.path());
}
/// common real shape, and the one every other test misses, because every name
/// they reach the crate root with is dotted.
#[test]
fn a_single_segment_package_emits_a_crate_root_that_compiles() {
    let fixture = tempfile::tempdir().expect("temp fixture dir");
    let entry = write_package_fixture(
        fixture.path(),
        "app",
        "package app\n\ntype Speed : km/h [0.0..250.0 step 0.5]\n",
    );

    let out = tempfile::tempdir().expect("temp dir");
    let run =
        ridlc::run_build(&entry, out.path(), &[Emit::Rust], false.into()).expect("build runs");
    assert!(
        !run.has_error(),
        "expected no error, got: {:?}",
        run.diagnostics
    );

    let lib = std::fs::read_to_string(out.path().join("lib.rs")).unwrap();
    assert!(
        lib.contains("#[path = \"app.rs\"]") && lib.contains("pub mod app;"),
        "a depth-0 leaf must be a plain module, lib.rs was:\n{lib}"
    );
    compile_crate_root(out.path());
}

/// A package that checks clean but whose Rust generation fails makes the build
/// skip that package's `.rs` and record a diagnostic instead. The module tree
/// is built from every checked package, so it would name a file that was never
/// written — the crate root would not compile. Neither generated file is
/// written when any error-severity diagnostic is present after the write loop.
///
/// The failure used here is the generated-name collision between two tuple
/// types (`ridl_backend_rust::tuple_collision`): `AB.c` and `A.bC` both reach
/// the induced struct name `ABC`, and the two tuples have different shapes, so
/// the backend refuses the package. Nothing upstream draws a diagnostic for
/// it, which is exactly why the emit gate computed before the write loop
/// cannot see it — and the same fixture is built with `&[Emit::IrJson]` first
/// to establish that. Without that precondition the test asserts only that
/// *something* failed, which a fixture that started failing at check time
/// satisfies too, and then it silently becomes a second test of the pre-loop
/// gate and stops testing the re-check after the loop at all.
#[test]
fn a_package_that_fails_rust_generation_writes_no_crate_files() {
    let fixture = tempfile::tempdir().expect("temp fixture dir");
    let entry = write_package_fixture(
        fixture.path(),
        "app",
        "package app\n\nstruct AB {\n  c : (x: integer)\n}\n\nstruct A {\n  bC : (y: integer)\n}\n",
    );

    let checked_out = tempfile::tempdir().expect("temp dir for the precondition build");
    let checked = ridlc::run_build(&entry, checked_out.path(), &[Emit::IrJson], false.into())
        .expect("the precondition build runs");
    assert!(
        !checked.has_error(),
        "the fixture must check clean and fail only in the Rust backend, or this test is a \
         second test of the pre-loop emit gate, got: {:?}",
        checked.diagnostics
    );

    let out = tempfile::tempdir().expect("temp dir");
    let run =
        ridlc::run_build(&entry, out.path(), &[Emit::Rust], false.into()).expect("build runs");
    assert!(
        run.has_error(),
        "the fixture must fail Rust generation, or this test proves nothing, got: {:?}",
        run.diagnostics
    );

    assert!(
        !out.path().join("app.rs").exists(),
        "the failing package writes no source"
    );
    assert!(
        !out.path().join("lib.rs").exists(),
        "a crate root naming a file no build wrote does not compile, so none is written"
    );
    assert!(
        !out.path().join("Cargo.toml").exists(),
        "a manifest without a crate root is not a crate"
    );
}

/// `lib.rs` and `Cargo.toml` are not package-scoped names and `--out-dir` is
/// any directory the caller names, including the root of a hand-written crate.
/// `std::fs::write` truncates, so a build must check before it writes: a file
/// that does not begin with its generated-by marker is left exactly as it is.
///
/// The check runs before the per-package write loop, so a refused build writes
/// nothing at all — not the other crate file, and not a single `<package>.rs`.
/// A refusal that ran after the loop would have already scattered the
/// per-package sources through the hand-written crate it was protecting.
#[test]
fn a_hand_written_crate_root_is_refused_and_left_untouched() {
    let out = tempfile::tempdir().expect("temp dir");
    let hand_written = "//! A crate a person wrote.\npub fn main_logic() {}\n";
    std::fs::write(out.path().join("lib.rs"), hand_written).expect("the crate root is written");

    let run = ridlc::run_build(
        Path::new("tests/corpus/veh-common"),
        out.path(),
        &[Emit::Rust],
        false.into(),
    )
    .expect("build runs");

    assert!(
        run.has_error(),
        "overwriting a hand-written file is an error, got: {:?}",
        run.diagnostics
    );
    let message = run
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.clone())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        message.contains("lib.rs") && message.contains("ridlc did not write it"),
        "the diagnostic must name the file and say it was not written, got:\n{message}"
    );

    assert_eq!(
        std::fs::read_to_string(out.path().join("lib.rs")).unwrap(),
        hand_written,
        "the hand-written crate root must be byte-identical"
    );
    assert!(
        !out.path().join("Cargo.toml").exists(),
        "a refusal on either file writes neither"
    );
    assert_eq!(
        emitted_package_sources(out.path()),
        Vec::<String>::new(),
        "a refused build writes no package source either: the check runs before the write loop"
    );
}

/// The manifest half of the same rule: a hand-written `Cargo.toml` is refused,
/// no crate root is written next to it, and no package source either.
#[test]
fn a_hand_written_manifest_is_refused_and_left_untouched() {
    let out = tempfile::tempdir().expect("temp dir");
    let hand_written = "[package]\nname = \"written_by_a_person\"\nversion = \"0.1.0\"\n";
    std::fs::write(out.path().join("Cargo.toml"), hand_written).expect("the manifest is written");

    let run = ridlc::run_build(
        Path::new("tests/corpus/veh-common"),
        out.path(),
        &[Emit::Rust],
        false.into(),
    )
    .expect("build runs");

    assert!(
        run.has_error(),
        "overwriting a hand-written file is an error, got: {:?}",
        run.diagnostics
    );
    assert_eq!(
        std::fs::read_to_string(out.path().join("Cargo.toml")).unwrap(),
        hand_written,
        "the hand-written manifest must be byte-identical"
    );
    assert!(
        !out.path().join("lib.rs").exists(),
        "a refusal on either file writes neither"
    );
    assert_eq!(
        emitted_package_sources(out.path()),
        Vec::<String>::new(),
        "a refused build writes no package source either: the check runs before the write loop"
    );
}

/// The refusal reads the first line, not the whole file. A hand-written
/// `lib.rs` that merely quotes the marker somewhere in its body — in a comment
/// explaining the rule, for one — is still hand-written and is still refused.
/// `starts_with` relaxed to `contains` passes every other test here, because
/// no other fixture mentions the marker anywhere but on line 1.
#[test]
fn a_hand_written_crate_root_quoting_the_marker_is_still_refused() {
    let out = tempfile::tempdir().expect("temp dir");
    let hand_written = "//! A crate a person wrote.\n\
         // ridlc refuses a file that does not open with\n\
         // `// Generated by ridlc. Do not edit.`, which this one does not.\n\
         pub fn main_logic() {}\n";
    std::fs::write(out.path().join("lib.rs"), hand_written).expect("the crate root is written");

    let run = ridlc::run_build(
        Path::new("tests/corpus/veh-common"),
        out.path(),
        &[Emit::Rust],
        false.into(),
    )
    .expect("build runs");

    assert!(
        run.has_error(),
        "the marker must be the first line, not merely present, got: {:?}",
        run.diagnostics
    );
    assert_eq!(
        std::fs::read_to_string(out.path().join("lib.rs")).unwrap(),
        hand_written,
        "the hand-written crate root must be byte-identical"
    );
    assert!(
        !out.path().join("Cargo.toml").exists(),
        "a refusal on either file writes neither"
    );
}

/// A destination ridlc cannot read is neither written nor refused: the read
/// error is returned as the `std::io::Error` it is, so `run_build` returns
/// `Err` and the command exits 2, which is ADR-0010's exit code for an I/O
/// failure. A diagnostic would exit 1 and would assert that the file does not
/// begin with the marker — which, for a file whose bytes could not be read, is
/// not something the build established.
#[test]
fn an_unreadable_crate_root_aborts_the_build_rather_than_drawing_a_diagnostic() {
    let out = tempfile::tempdir().expect("temp dir");
    // Not valid UTF-8: a lone continuation byte cannot start a sequence, so
    // `read_to_string` fails with `InvalidData`.
    let raw: &[u8] = &[0x80, 0x00, 0xff, 0xfe];
    std::fs::write(out.path().join("lib.rs"), raw).expect("the unreadable crate root is written");

    let run = ridlc::run_build(
        Path::new("tests/corpus/veh-common"),
        out.path(),
        &[Emit::Rust],
        false.into(),
    );

    let err = run
        .err()
        .expect("an unreadable destination is an I/O failure, not a diagnostic");
    assert_eq!(
        err.kind(),
        std::io::ErrorKind::InvalidData,
        "the read error is propagated as itself, got: {err}"
    );

    assert_eq!(
        std::fs::read(out.path().join("lib.rs")).unwrap(),
        raw,
        "the unreadable file must be byte-identical"
    );
    assert!(
        !out.path().join("Cargo.toml").exists(),
        "an aborted build writes neither crate file"
    );
    assert_eq!(
        emitted_package_sources(out.path()),
        Vec::<String>::new(),
        "an aborted build writes no package source either"
    );
}

/// The other direction: a file a previous build wrote carries the marker, so
/// rebuilding into the same output directory overwrites it. Without this, the
/// refusal above would make a second build of any output directory fail.
#[test]
fn a_previously_generated_crate_root_is_overwritten() {
    let out = tempfile::tempdir().expect("temp dir");
    std::fs::write(
        out.path().join("lib.rs"),
        "// Generated by ridlc. Do not edit.\n\n// stale content from an earlier build\n",
    )
    .expect("the stale crate root is written");
    std::fs::write(
        out.path().join("Cargo.toml"),
        "# Generated by ridlc. Do not edit.\n[package]\nname = \"stale\"\n",
    )
    .expect("the stale manifest is written");

    let run = ridlc::run_build(
        Path::new("tests/corpus/veh-common"),
        out.path(),
        &[Emit::Rust],
        false.into(),
    )
    .expect("build runs");
    assert!(
        !run.has_error(),
        "expected no error, got: {:?}",
        run.diagnostics
    );

    let lib = std::fs::read_to_string(out.path().join("lib.rs")).unwrap();
    assert!(
        !lib.contains("stale content") && lib.contains("#[path = \"veh.common.rs\"]"),
        "a marked crate root is overwritten, was:\n{lib}"
    );
    let manifest = std::fs::read_to_string(out.path().join("Cargo.toml")).unwrap();
    assert!(
        manifest.contains("name = \"veh_common\""),
        "a marked manifest is overwritten, was:\n{manifest}"
    );
}

/// THE REAL PROOF: the emitted crate root actually compiles with `rustc`,
/// which is the only thing that proves the module tree resolves — including
/// every `crate::…` cross-package reference the packages make to each other.
/// `compile_crate_root` passes `--extern ridl_rt`, because every generated
/// named scalar names `::ridl_rt::payload::Violation` from its `new`.
#[test]
fn emitted_lib_rs_compiles_with_rustc() {
    let out = tempfile::tempdir().expect("temp dir");
    let run = ridlc::run_build(
        Path::new("tests/corpus/veh-cluster"),
        out.path(),
        &[Emit::Rust],
        false.into(),
    )
    .expect("build runs");
    assert!(
        !run.has_error(),
        "expected no error, got: {:?}",
        run.diagnostics
    );

    compile_crate_root(out.path());
}

/// A package referencing `ridl.std` makes the build write `ridl.std.rs`
/// (issue #190); `lib.rs` must make it reachable as `crate::ridl::std`, or the
/// crate the other packages reference from does not compile.
#[test]
fn ridl_std_is_reachable_from_lib_rs() {
    let out = tempfile::tempdir().expect("temp dir");
    let run = ridlc::run_build(
        Path::new("tests/corpus/veh-common"),
        out.path(),
        &[Emit::Rust],
        false.into(),
    )
    .expect("build runs");
    assert!(
        !run.has_error(),
        "expected no error, got: {:?}",
        run.diagnostics
    );

    assert!(
        out.path().join("ridl.std.rs").exists(),
        "the fixture references ridl.std, so the build must write ridl.std.rs"
    );

    let lib = std::fs::read_to_string(out.path().join("lib.rs")).unwrap();
    assert!(
        lib.contains("#[path = \"ridl.std.rs\"]") && lib.contains("pub mod std"),
        "lib.rs must make ridl.std.rs reachable as crate::ridl::std, was:\n{lib}"
    );
}

/// Single-file mode (no manifest anywhere up the tree) keeps writing only
/// `<stem>.rs`, matching the documented single-file asymmetry on
/// `Emit::TypeScript` — it never writes `Cargo.toml` or `lib.rs`.
#[test]
fn single_file_mode_writes_neither_manifest_nor_lib_rs() {
    let out = tempfile::tempdir().expect("temp dir");
    let entry_dir = tempfile::tempdir().expect("temp entry dir");
    let entry = entry_dir.path().join("standalone.typl");
    std::fs::write(
        &entry,
        "package standalone\ntype Speed: km/h [0.0..250.0 step 0.5]\n",
    )
    .unwrap();

    let run =
        ridlc::run_build(&entry, out.path(), &[Emit::Rust], false.into()).expect("build runs");
    assert!(
        !run.has_error(),
        "expected no error, got: {:?}",
        run.diagnostics
    );

    assert!(out.path().join("standalone.rs").exists());
    assert!(!out.path().join("Cargo.toml").exists());
    assert!(!out.path().join("lib.rs").exists());
}

/// The generated crate name comes from the manifest's `[package]` name with
/// every `.` replaced by `_`, since a dotted ridl package name (`veh.common`)
/// is not a legal Cargo package name.
#[test]
fn crate_name_replaces_dots_with_underscores() {
    let out = tempfile::tempdir().expect("temp dir");
    let run = ridlc::run_build(
        Path::new("tests/corpus/veh-common"),
        out.path(),
        &[Emit::Rust],
        false.into(),
    )
    .expect("build runs");
    assert!(
        !run.has_error(),
        "expected no error, got: {:?}",
        run.diagnostics
    );

    let manifest = std::fs::read_to_string(out.path().join("Cargo.toml")).unwrap();
    assert!(
        manifest.contains("name = \"veh_common\""),
        "manifest was:\n{manifest}"
    );
}

/// A `[workspace]` manifest names no package, so the generated crate falls
/// back to `ridl_generated`.
#[test]
fn crate_name_falls_back_for_a_workspace_manifest() {
    let out = tempfile::tempdir().expect("temp dir");
    let run = ridlc::run_build(
        Path::new("tests/corpus/workspace-two-members"),
        out.path(),
        &[Emit::Rust],
        false.into(),
    )
    .expect("build runs");
    assert!(
        !run.has_error(),
        "expected no error, got: {:?}",
        run.diagnostics
    );

    let manifest = std::fs::read_to_string(out.path().join("Cargo.toml")).unwrap();
    assert!(
        manifest.contains("name = \"ridl_generated\""),
        "manifest was:\n{manifest}"
    );
}

/// The `ridl-rt` dependency requirement in the emitted manifest and
/// `crates/ridl-rt`'s own version must agree on major and minor, or the
/// generated crate could pin a `ridl-rt` release that predates the API the
/// codegen it emits actually needs. `ridlc` cannot read
/// `crates/ridl-rt/Cargo.toml` at run time — it is an installed binary with no
/// access to this repository's sources — so the manifest string is a literal;
/// this test is what keeps the literal and the crate's real version from
/// drifting apart silently (Task 7's own rationale for reading the version at
/// emit time, redirected into a guard test instead).
#[test]
fn ridl_rt_version_requirement_matches_the_crate() {
    #[derive(serde::Deserialize)]
    struct CargoManifest {
        package: CargoPackage,
    }
    #[derive(serde::Deserialize)]
    struct CargoPackage {
        version: String,
    }

    let ridl_rt_manifest_text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../ridl-rt/Cargo.toml"
    ))
    .expect("crates/ridl-rt/Cargo.toml must exist");
    let ridl_rt_manifest: CargoManifest = toml::from_str(&ridl_rt_manifest_text)
        .expect("crates/ridl-rt/Cargo.toml must be valid TOML");
    let version = ridl_rt_manifest.package.version;
    let mut segments = version.split('.');
    let major = segments
        .next()
        .expect("a semver version has a major segment");
    let minor = segments
        .next()
        .expect("a semver version has a minor segment");
    // The requirement now carries the `flatbuffers` feature, because
    // `generate`'s output includes the payload codec (E11.7 stage K5). Only
    // the version half is what this test guards, so it matches the version
    // string inside the dependency table rather than the whole line.
    let expected = format!("ridl-rt = {{ version = \"{major}.{minor}\"");

    let out = tempfile::tempdir().expect("temp dir");
    let run = ridlc::run_build(
        Path::new("tests/corpus/veh-common"),
        out.path(),
        &[Emit::Rust],
        false.into(),
    )
    .expect("build runs");
    assert!(
        !run.has_error(),
        "expected no error, got: {:?}",
        run.diagnostics
    );
    let manifest = std::fs::read_to_string(out.path().join("Cargo.toml")).unwrap();
    assert!(
        manifest.contains(&expected),
        "expected `{expected}` in the emitted manifest, got:\n{manifest}"
    );
}

/// Writes a two-package workspace into `dir` and returns its root: `px.a`
/// holding `source_a`, `px.b` holding `source_b`. The two package
/// directories and the workspace manifest are what `run_build` needs to see
/// both packages in one build, which is the condition under which a
/// cross-package reference resolves at all.
fn write_two_package_workspace(dir: &Path, source_a: &str, source_b: &str) -> PathBuf {
    std::fs::write(
        dir.join("ridl.toml"),
        "[workspace]\nmembers = [\"a\", \"b\"]\n",
    )
    .expect("the workspace manifest is written");
    for (member, name, source) in [("a", "px.a", source_a), ("b", "px.b", source_b)] {
        let member_dir = dir.join(member);
        std::fs::create_dir_all(&member_dir).expect("the member directory is created");
        std::fs::write(
            member_dir.join("ridl.toml"),
            format!("[package]\nname = \"{name}\"\nversion = \"1.0.0\"\n"),
        )
        .expect("the member manifest is written");
        std::fs::write(member_dir.join("source.ridl"), source)
            .expect("the member source is written");
    }
    dir.to_path_buf()
}

/// A **struct** and a **union arm** that reach into another package of the
/// build emit a crate that compiles (driftsys/ridl#467).
///
/// The codec names a foreign view type by a path and builds it with a struct
/// literal, so the view's fields have to be reachable from the other
/// package's module. They were private, and this was the shape that proved
/// it: the corpus's own cross-package references are all named scalars, enums
/// and enum sets, none of which has a view, so nothing else in the tree
/// reaches the struct literal at all.
///
/// The mutation that proves this test: make either field of the emitted
/// `#view` struct in `codec.rs` private again, and `rustc` reports E0451
/// twice for the struct and once for the union arm.
#[test]
fn a_cross_package_struct_or_union_reference_compiles() {
    let dir = tempfile::tempdir().expect("temp dir");
    let entry = write_two_package_workspace(
        dir.path(),
        "package px.a\n\nstruct Point {\n  x : integer [0..100]\n  y : integer [0..100]\n}\n",
        "package px.b\n\nimport px.a.Point\n\n\
         struct Line {\n  from : Point\n  to : Point\n}\n\n\
         union Shape {\n  point : Point\n  line : Line\n}\n",
    );
    let out = tempfile::tempdir().expect("temp dir");
    let run =
        ridlc::run_build(&entry, out.path(), &[Emit::Rust], false.into()).expect("build runs");
    assert!(
        !run.has_error(),
        "expected no error, got: {:?}",
        run.diagnostics
    );

    let source = std::fs::read_to_string(out.path().join("px.b.rs")).expect("px.b.rs is written");
    assert!(
        !source.contains("__RIDL_FB_NO_CODEC"),
        "no type here may be withheld a codec any more, got:\n{source}"
    );
    assert!(
        source.contains("crate::px::a::PointFbView"),
        "the foreign view is named by a path, got:\n{source}"
    );

    compile_crate_root(out.path());
}

/// An **interface** named `Wire` is refused, the same as a declaration is
/// (E11.14 decision 5).
///
/// The descriptor emitter writes `pub struct <Interface>;` at package scope,
/// which is the scope `pub type Wire` lands at, so an interface collides with
/// the alias exactly as a typl declaration does. Refusing only declarations
/// let this through to a rustc E0428 in the emitted source, which is the
/// failure the refusal exists to replace.
///
/// The mutation that proves this test: drop the `interfaces` half of
/// `refuse_wire_collision`'s scan, and the build succeeds here.
#[test]
fn an_interface_named_wire_is_refused() {
    let dir = tempfile::tempdir().expect("temp dir");
    // Written by hand rather than through `write_package_fixture`, which
    // writes a `.typl` file: an interface is a ridl declaration and draws
    // "interaction declaration in typl context" from a typl source.
    let entry = dir.path().join("pkg");
    std::fs::create_dir_all(&entry).expect("the package directory is created");
    std::fs::write(
        entry.join("ridl.toml"),
        "[package]\nname = \"px.w\"\nversion = \"1.0.0\"\n",
    )
    .expect("the manifest is written");
    std::fs::write(
        entry.join("source.ridl"),
        "package px.w\n\ntype Level : integer [0..100]\n\n\
         interface Wire {\n  signal level : Level @10ms\n}\n",
    )
    .expect("the source is written");
    let out = tempfile::tempdir().expect("temp dir");
    let run =
        ridlc::run_build(&entry, out.path(), &[Emit::Rust], false.into()).expect("build runs");
    assert!(
        run.has_error(),
        "an interface named `Wire` must be refused, diagnostics: {:?}",
        run.diagnostics
    );
    let message = run
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<String>();
    assert!(
        message.contains("Wire") && message.contains("collide"),
        "the refusal must state the collision and name `Wire`, got: {message}"
    );
    assert!(
        message.contains("rename the interface"),
        "the refusal must name the interface as the thing to rename, got: {message}"
    );
}

/// An interface the face cannot carry is skipped with a note, and the rest of
/// the package is still emitted and still compiles (E11.14 decision 2).
///
/// The corpus's `veh-cluster` is the fixture that exercises this without one
/// being written for it: two of its interfaces carry a contract clause the
/// narrow translator does not accept, so a build of it emits two notes.
///
/// The mutation that proves this test: replace the `Err` arm of
/// `generate_pipeline`'s per-interface match with `Err(_) => {}`, emitting
/// nothing on a skip, and the note assertions below fail. Without this test
/// that arm — and `skipped_interface_note`, `face_gap` and `FaceGap` with it —
/// could be deleted whole and every other test would stay green.
#[test]
fn a_skipped_interface_leaves_a_note_and_the_package_still_compiles() {
    let out = tempfile::tempdir().expect("temp dir");
    let run = ridlc::run_build(
        Path::new("tests/corpus/veh-cluster"),
        out.path(),
        &[Emit::Rust],
        false.into(),
    )
    .expect("build runs");
    assert!(
        !run.has_error(),
        "a skipped interface is not an error, got: {:?}",
        run.diagnostics
    );

    let source =
        std::fs::read_to_string(out.path().join("veh.cluster.rs")).expect("the package is written");

    // The note exists, and names the interface it stands for.
    assert!(
        source.contains("__RIDL_NO_FACE_VEHICLE_STATUS"),
        "a skipped interface leaves a note naming it, got:\n{source}"
    );
    assert!(
        source.contains("Interface `VehicleStatus` carries no generated interaction face."),
        "the note names the interface in prose, got:\n{source}"
    );
    // It carries a reason and the story that removes the limit, which is what
    // makes it a note rather than a silent omission.
    assert!(
        source.contains("The emitter refused it:"),
        "the note carries the refusal's own reason, got:\n{source}"
    );
    // The owner line must match the reason above it. `VehicleStatus` has a
    // call-shape gap *and* an untranslatable clause, and the refusal that
    // stopped it is the clause — so a note that reads the interface instead
    // of the refusal names the multi-parameter story under a clause reason,
    // which is what this pins.
    let note = source
        .split("const __RIDL_NO_FACE_VEHICLE_STATUS")
        .next()
        .expect("the note precedes its constant");
    let note = &note[note
        .rfind("Interface `VehicleStatus`")
        .expect("the note's headline")..];
    assert!(
        note.contains("cannot translate contract clause"),
        "this interface is skipped for a clause, got:\n{note}"
    );
    assert!(
        note.contains("Story E5.1") && !note.contains("lane M's"),
        "the owner line must name the clause story, not the multi-parameter \
         one, got:\n{note}"
    );
    // A run of literal spaces inside the note is a broken line continuation
    // in the emitter's string, which reaches the reader's source.
    assert!(
        !note.contains("     "),
        "no note may carry a run of literal spaces from a broken \
         string continuation, got:\n{note}"
    );

    // The skip is per interface: the package's other interfaces keep their
    // face, and its payload types are unaffected.
    assert!(
        source.contains("pub mod cabin_climate") || source.contains("pub struct Client"),
        "an interface the face can carry still gets one, got:\n{source}"
    );

    compile_crate_root(out.path());
}
