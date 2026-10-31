//! The build-wide check of the C names the podbuf headers of one build claim
//! (driftsys/ridl#1153). The header of each package is written for a C file
//! that includes several of them, so a name two packages derive for two
//! different entities is refused once per build, before any header is
//! written; a head one package declares and another lays out inline with an
//! identical definition is one entity, defined twice behind one guard, and is
//! not refused.

use std::path::Path;

use ridl_core::diag::Diagnostic;
use ridlc::Emit;

/// Writes a workspace whose members are `packages`, each a `(directory,
/// package name, source)` triple, and builds it with `emits`. Answers the
/// build's diagnostics and the output directory.
fn build(packages: &[(&str, &str, &str)], emits: &[Emit]) -> (Vec<Diagnostic>, tempfile::TempDir) {
    let src = tempfile::tempdir().expect("a temp dir");
    let out = tempfile::tempdir().expect("a temp dir");
    let members: Vec<String> = packages
        .iter()
        .map(|(dir, _, _)| format!("\"{dir}\""))
        .collect();
    std::fs::write(
        src.path().join("ridl.toml"),
        format!(
            "[workspace]\nmembers = [{}]\n\n[lints]\nmissing-docs = \"allow\"\n",
            members.join(", ")
        ),
    )
    .unwrap();
    for (dir, name, source) in packages {
        let member = src.path().join(dir);
        std::fs::create_dir(&member).unwrap();
        std::fs::write(
            member.join("ridl.toml"),
            format!("[package]\nname = \"{name}\"\nversion = \"1.0.0\"\n"),
        )
        .unwrap();
        std::fs::write(member.join(format!("{dir}.typl")), source).unwrap();
    }
    let run =
        ridlc::run_build(src.path(), out.path(), emits, false.into()).expect("the build runs");
    (run.diagnostics, out)
}

/// The names of the files in `dir` with the extension `ext`, sorted.
fn files_with(dir: &Path, ext: &str) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .expect("the output directory is read")
        .map(|entry| entry.expect("an entry").path())
        .filter(|path| path.extension().is_some_and(|found| found == ext))
        .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// The one error diagnostic that refuses the C name `name`.
fn error_naming<'a>(diagnostics: &'a [Diagnostic], name: &str) -> &'a Diagnostic {
    let lead = format!("the C name `{name}` is claimed by ");
    let found: Vec<&Diagnostic> = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.message.starts_with(&lead))
        .collect();
    assert_eq!(found.len(), 1, "{name}: {diagnostics:?}");
    found[0]
}

/// `package veh` with `type CommonSpeed` and `package veh.common` with
/// `struct Speed` both derive `veh_common_speed_head` and its guard, with two
/// layouts. The build refuses both headers, names both packages, writes no
/// header, writes the other emits, and exits 1.
#[test]
fn two_packages_deriving_one_head_name_are_refused_naming_both() {
    let (diagnostics, out) = build(
        &[
            (
                "veh",
                "veh",
                "package veh\ntype CommonSpeed : integer [0..100]\n",
            ),
            (
                "common",
                "veh.common",
                "package veh.common\nstruct Speed { v : integer [0..1000] }\n",
            ),
        ],
        &[Emit::Podbuf, Emit::IrJson],
    );
    let error = error_naming(&diagnostics, "veh_common_speed_head");
    assert!(
        error
            .message
            .contains("the head of `CommonSpeed` in the header of `veh`")
            && error
                .message
                .contains("the head of `Speed` in the header of `veh.common`"),
        "{}",
        error.message
    );
    assert_eq!(
        error.severity,
        ridl_core::diag::Severity::Error,
        "{error:?}"
    );
    // The guard macro of the two heads is one name too, and is its own error.
    let guard = error_naming(&diagnostics, "VEH_COMMON_SPEED_HEAD_DEFINED");
    assert!(
        guard
            .message
            .contains("the guard of the head of `CommonSpeed` in the header of `veh`")
            && guard
                .message
                .contains("the guard of the head of `Speed` in the header of `veh.common`"),
        "{}",
        guard.message
    );
    assert_eq!(files_with(out.path(), "h"), Vec::<String>::new());
    assert_eq!(
        files_with(out.path(), "json"),
        vec!["veh.common.ir.json", "veh.ir.json"]
    );
}

/// The tuple at `a.Hub.size` and `struct Size` of `package a.hub` both derive
/// `a_hub_size_head`; the two heads are refused.
#[test]
fn a_tuple_head_and_a_struct_head_of_another_package_are_refused() {
    let (diagnostics, out) = build(
        &[
            (
                "a",
                "a",
                "package a\nstruct Hub { size : (w: integer [0..10], h: integer [0..10]) }\n",
            ),
            (
                "hub",
                "a.hub",
                "package a.hub\nstruct Size { w : integer [0..10] }\n",
            ),
        ],
        &[Emit::Podbuf],
    );
    let error = error_naming(&diagnostics, "a_hub_size_head");
    assert!(
        error
            .message
            .contains("the head of the tuple at `Hub.size` in the header of `a`")
            && error
                .message
                .contains("the head of `Size` in the header of `a.hub`"),
        "{}",
        error.message
    );
    assert_eq!(files_with(out.path(), "h"), Vec::<String>::new());
}

/// The accessor `x_y_z_head_at` of the field `zHead` of `x.Y` and the head
/// `x_y_z_head` of `x.y.Z` are refused together: the whole-buffer accessor
/// `x_y_z_head` of the first is the head type name of the second.
#[test]
fn an_accessor_and_a_head_of_another_package_are_refused() {
    let (diagnostics, out) = build(
        &[
            (
                "x",
                "x",
                "package x\nstruct Y { zHead : integer [0..10] }\n",
            ),
            (
                "xy",
                "x.y",
                "package x.y\nstruct Z { q : integer [0..10] }\n",
            ),
        ],
        &[Emit::Podbuf],
    );
    let error = error_naming(&diagnostics, "x_y_z_head");
    assert!(
        error
            .message
            .contains("the accessor of `z_head` of the head of `Y` in the header of `x`")
            && error
                .message
                .contains("the head of `Z` in the header of `x.y`"),
        "{}",
        error.message
    );
    assert_eq!(files_with(out.path(), "h"), Vec::<String>::new());
}

/// The enum `A` of `package ridl.podbuf` with a value `H` spells
/// `RIDL_PODBUF_A_H`, the include guard of the header of `package a`; a C
/// file that includes both would skip the second header.
#[test]
fn a_constant_equal_to_the_include_guard_of_another_header_is_refused() {
    let (diagnostics, out) = build(
        &[
            ("a", "a", "package a\nstruct S { x : integer [0..10] }\n"),
            (
                "pb",
                "ridl.podbuf",
                "package ridl.podbuf\nenum A { H = 0 }\n",
            ),
        ],
        &[Emit::Podbuf],
    );
    let error = error_naming(&diagnostics, "RIDL_PODBUF_A_H");
    assert!(
        error
            .message
            .contains("the include guard of the header in the header of `a`")
            && error
                .message
                .contains("the enum value `H` in the header of `ridl.podbuf`"),
        "{}",
        error.message
    );
    assert_eq!(files_with(out.path(), "h"), Vec::<String>::new());
}

/// A head one package declares and another lays out inline with the same
/// text is one entity: the two headers define `a_inner_head` behind one guard,
/// and the build writes both.
#[test]
fn an_inline_redefinition_of_a_foreign_head_is_not_a_clash() {
    let (diagnostics, out) = build(
        &[
            (
                "a",
                "a",
                "package a\nstruct Inner { x : integer [0..10] }\n",
            ),
            (
                "b",
                "b",
                "package b\nimport a.Inner\nstruct Outer { i : Inner }\n",
            ),
        ],
        &[Emit::Podbuf],
    );
    assert!(
        !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == ridl_core::diag::Severity::Error),
        "{diagnostics:?}"
    );
    assert_eq!(files_with(out.path(), "h"), vec!["a.h", "b.h"]);
    let b = std::fs::read_to_string(out.path().join("b.h")).unwrap();
    assert!(b.contains("typedef struct a_inner_head {"), "{b}");
}

/// The same for the head of a tuple: `b` lays `a.Hub` out inline and defines
/// the tuple head `a_hub_size_head` that `a.h` defines, under the declaring
/// package's name.
#[test]
fn an_inline_redefinition_of_a_foreign_tuple_head_is_not_a_clash() {
    let (diagnostics, out) = build(
        &[
            (
                "a",
                "a",
                "package a\nstruct Hub { size : (w: integer [0..10], h: integer [0..10]) }\n",
            ),
            (
                "b",
                "b",
                "package b\nimport a.Hub\nstruct Wheel { h : Hub }\n",
            ),
        ],
        &[Emit::Podbuf],
    );
    assert!(
        !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == ridl_core::diag::Severity::Error),
        "{diagnostics:?}"
    );
    assert_eq!(files_with(out.path(), "h"), vec!["a.h", "b.h"]);
    let b = std::fs::read_to_string(out.path().join("b.h")).unwrap();
    assert!(b.contains("typedef struct a_hub_size_head {"), "{b}");
}

/// Three headers claiming one name for three entities give one error that
/// lists all three: the accessor `x_w_ya_z_head` of the field `yaZHead` of
/// `x.W`, the head of the tuple at `x.w.Ya.z` and the head of `x.w.ya.Z`.
#[test]
fn three_claimants_of_one_name_are_one_error_naming_all_three() {
    let (diagnostics, out) = build(
        &[
            (
                "x",
                "x",
                "package x\nstruct W { yaZHead : integer [0..10] }\n",
            ),
            (
                "xw",
                "x.w",
                "package x.w\nstruct Ya { z : (p: integer [0..10], q: integer [0..10]) }\n",
            ),
            (
                "xwya",
                "x.w.ya",
                "package x.w.ya\nstruct Z { r : integer [0..10] }\n",
            ),
        ],
        &[Emit::Podbuf],
    );
    let error = error_naming(&diagnostics, "x_w_ya_z_head");
    assert!(
        error
            .message
            .contains("the accessor of `ya_z_head` of the head of `W` in the header of `x`")
            && error
                .message
                .contains("the head of the tuple at `Ya.z` in the header of `x.w`")
            && error
                .message
                .contains("the head of `Z` in the header of `x.w.ya`"),
        "{}",
        error.message
    );
    // The claimants are in the order of the build's packages, and the errors
    // in the order of the names, so the guard's error comes first.
    let order = |text: &str| error.message.find(text).expect(text);
    assert!(
        order("in the header of `x`") < order("in the header of `x.w`")
            && order("in the header of `x.w`") < order("in the header of `x.w.ya`"),
        "{}",
        error.message
    );
    let names: Vec<&str> = diagnostics
        .iter()
        .filter_map(|diagnostic| diagnostic.message.strip_prefix("the C name `"))
        .map(|rest| rest.split('`').next().unwrap())
        .collect();
    assert_eq!(names, vec!["X_W_YA_Z_HEAD_DEFINED", "x_w_ya_z_head"]);
    assert_eq!(files_with(out.path(), "h"), Vec::<String>::new());
}

/// `ridl.std` is in the scope of the check whenever the build writes its
/// header: `package ridl` with `type StdTimestamp` derives
/// `ridl_std_timestamp_head`, the head of `ridl.std.Timestamp`.
#[test]
fn a_head_name_equal_to_a_ridl_std_head_is_refused() {
    let (diagnostics, out) = build(
        &[(
            "ridl",
            "ridl",
            "package ridl\ntype StdTimestamp : integer [0..10]\nstruct S { t : Timestamp }\n",
        )],
        &[Emit::Podbuf],
    );
    let error = error_naming(&diagnostics, "ridl_std_timestamp_head");
    assert!(
        error
            .message
            .contains("the head of `StdTimestamp` in the header of `ridl`")
            && error
                .message
                .contains("the head of `Timestamp` in the header of `ridl.std`"),
        "{}",
        error.message
    );
    assert_eq!(files_with(out.path(), "h"), Vec::<String>::new());
}

/// Two headers that claim one head name for one identity are refused when
/// their definitions differ. The importing lowering names the tuple at
/// position 1 of `a.A.bX` by that position (`a.A.bX.1`, wire name
/// `ABXField1`), the wire name `a` gives the tuple at `AB.xField1`, so `b.h`
/// defines `a_abx_field1_head` with the members of the first and `a.h` with
/// the member of the second, behind one guard.
#[test]
fn one_identity_with_two_definitions_is_refused() {
    let (diagnostics, out) = build(
        &[
            (
                "a",
                "a",
                "package a\n\
                 struct A { bX : (y : (u: integer [0..10], v: integer [0..10])) }\n\
                 struct AB {\n  xY : (u: integer [0..10], v: integer [0..10])\n  \
                 xField1 : (w: integer [0..200])\n}\n",
            ),
            ("b", "b", "package b\nimport a.A\nstruct B { a : A }\n"),
        ],
        &[Emit::Podbuf],
    );
    let error = error_naming(&diagnostics, "a_abx_field1_head");
    assert!(
        error
            .message
            .contains("the head of the tuple at `AB.xField1` in the header of `a`")
            && error
                .message
                .contains("the head of the tuple at `a.A.bX.1` in the header of `b`"),
        "{}",
        error.message
    );
    error_naming(&diagnostics, "A_ABX_FIELD1_HEAD_DEFINED");
    assert_eq!(files_with(out.path(), "h"), Vec::<String>::new());
}

/// Two entities whose heads render to one text are still two entities:
/// `veh.CommonSpeed` and `veh.common.Speed { value }` both define
/// `veh_common_speed_head` with one `uint8_t value`, and are refused.
#[test]
fn two_identities_with_one_definition_text_are_refused() {
    let (diagnostics, out) = build(
        &[
            (
                "veh",
                "veh",
                "package veh\ntype CommonSpeed : integer [0..100]\n",
            ),
            (
                "common",
                "veh.common",
                "package veh.common\nstruct Speed { value : integer [0..100] }\n",
            ),
        ],
        &[Emit::Podbuf],
    );
    error_naming(&diagnostics, "veh_common_speed_head");
    error_naming(&diagnostics, "VEH_COMMON_SPEED_HEAD_DEFINED");
    assert_eq!(files_with(out.path(), "h"), Vec::<String>::new());
}

/// Three headers claim `veh_common_speed_head`, two of them for one entity:
/// `veh` declares `CommonSpeed`, `b` lays it out inline with the same text,
/// and `veh.common` declares `Speed`. The error names one claimant per
/// entity: `veh` and `veh.common`, not `b`.
#[test]
fn a_claimant_of_an_entity_already_listed_is_not_named_again() {
    let (diagnostics, out) = build(
        &[
            (
                "veh",
                "veh",
                "package veh\nstruct CommonSpeed { v : integer [0..100] }\n",
            ),
            (
                "b",
                "b",
                "package b\nimport veh.CommonSpeed\nstruct Reading { s : CommonSpeed }\n",
            ),
            (
                "common",
                "veh.common",
                "package veh.common\nstruct Speed { w : integer [0..100] }\n",
            ),
        ],
        &[Emit::Podbuf],
    );
    let error = error_naming(&diagnostics, "veh_common_speed_head");
    assert!(
        error
            .message
            .contains("the head of `CommonSpeed` in the header of `veh`")
            && error
                .message
                .contains("the head of `Speed` in the header of `veh.common`")
            && !error.message.contains("in the header of `b`"),
        "{}",
        error.message
    );
    assert_eq!(files_with(out.path(), "h"), Vec::<String>::new());
}

/// A clash together with a deployment no source declares is a usage error:
/// the build exits 2 with the deployment's message, as it does without the
/// clash.
#[test]
fn a_clash_with_an_unknown_deployment_is_a_usage_error() {
    let src = tempfile::tempdir().expect("a temp dir");
    let out = tempfile::tempdir().expect("a temp dir");
    write_clashing_workspace(src.path());
    let run = ridlc::run_build_with(
        src.path(),
        out.path(),
        &[Emit::Podbuf],
        &[],
        std::time::Duration::from_secs(1),
        false.into(),
        ridlc::ApplyLints::Yes,
        Some("Typo"),
        &std::collections::BTreeMap::new(),
    )
    .expect("the build runs");
    assert!(run.usage_error, "{:?}", run.diagnostics);
    assert!(
        run.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.starts_with("no deployment named `Typo`")),
        "{:?}",
        run.diagnostics
    );
}

/// Writes the `veh` + `veh.common` clashing workspace into `dir`.
fn write_clashing_workspace(dir: &Path) {
    std::fs::write(
        dir.join("ridl.toml"),
        "[workspace]\nmembers = [\"veh\", \"common\"]\n\n[lints]\nmissing-docs = \"allow\"\n",
    )
    .unwrap();
    for (member, name, source) in [
        (
            "veh",
            "veh",
            "package veh\ntype CommonSpeed : integer [0..100]\n",
        ),
        (
            "common",
            "veh.common",
            "package veh.common\nstruct Speed { v : integer [0..1000] }\n",
        ),
    ] {
        let path = dir.join(member);
        std::fs::create_dir(&path).unwrap();
        std::fs::write(
            path.join("ridl.toml"),
            format!("[package]\nname = \"{name}\"\nversion = \"1.0.0\"\n"),
        )
        .unwrap();
        std::fs::write(path.join(format!("{member}.typl")), source).unwrap();
    }
}

/// A package whose own header is refused is left out of the check: `veh`
/// has a type with no podbuf size bound, so its `veh_common_speed_head` is
/// not held against `veh.common`, which gets its header; `veh` gets its own
/// refusal.
#[test]
fn a_package_whose_header_is_refused_is_left_out_of_the_check() {
    let (diagnostics, out) = build(
        &[
            (
                "veh",
                "veh",
                "package veh\ntype CommonSpeed : integer [0..100]\ntype Huge : string [0..1100000000]\n",
            ),
            (
                "common",
                "veh.common",
                "package veh.common\nstruct Speed { v : integer [0..1000] }\n",
            ),
        ],
        &[Emit::Podbuf],
    );
    assert!(
        !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.starts_with("the C name ")),
        "{diagnostics:?}"
    );
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("`veh.Huge` has no podbuf size bound")),
        "{diagnostics:?}"
    );
    assert_eq!(files_with(out.path(), "h"), vec!["veh.common.h"]);
}

/// On a clash the per-package files of the other emits are written: the
/// FlatBuffers schemas and the Rust modules. The crate's `lib.rs` and
/// `Cargo.toml` are not, as on any error of a build.
#[test]
fn the_per_package_files_of_the_other_emits_are_written_on_a_clash() {
    let src = tempfile::tempdir().expect("a temp dir");
    let out = tempfile::tempdir().expect("a temp dir");
    write_clashing_workspace(src.path());
    let run = ridlc::run_build(
        src.path(),
        out.path(),
        &[Emit::Podbuf, Emit::Flatbuffers, Emit::Rust],
        false.into(),
    )
    .expect("the build runs");
    error_naming(&run.diagnostics, "veh_common_speed_head");
    // The build exits 1: an error, and not a usage error.
    assert!(run.has_error() && !run.usage_error, "{:?}", run.diagnostics);
    assert_eq!(files_with(out.path(), "h"), Vec::<String>::new());
    assert_eq!(
        files_with(out.path(), "fbs"),
        vec!["veh.common.fbs", "veh.fbs"]
    );
    assert_eq!(
        files_with(out.path(), "rs"),
        vec!["veh.common.rs", "veh.rs"]
    );
    assert_eq!(files_with(out.path(), "toml"), Vec::<String>::new());
}

/// The check runs only when the build writes the headers: a FlatBuffers
/// build over the same workspace succeeds.
#[test]
fn a_build_without_the_podbuf_emit_does_not_run_the_check() {
    let src = tempfile::tempdir().expect("a temp dir");
    let out = tempfile::tempdir().expect("a temp dir");
    write_clashing_workspace(src.path());
    let run = ridlc::run_build(src.path(), out.path(), &[Emit::Flatbuffers], false.into())
        .expect("the build runs");
    assert!(!run.has_error(), "{:?}", run.diagnostics);
    assert_eq!(
        files_with(out.path(), "fbs"),
        vec!["veh.common.fbs", "veh.fbs"]
    );
}
