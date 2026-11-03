//! The seal on `Encoding` and the privacy of `Ref`'s fields, pinned by error
//! code.
//!
//! A `compile_fail` doctest passes for any compile error, and rustdoc does not
//! check the error code written on its fence. So a doctest cannot tell that
//! the seal failed for the right reason. Each case here writes one snippet
//! into a generated crate, builds that crate with cargo against this
//! checkout's `ridl-rt`, and asserts the error code in the output. A change
//! that opens the seal or makes a field public fails the code check, and so
//! does a rename that moves the failure to an unresolved name.
//!
//! The generated crate names `ridl-rt` by path, not by a library file found in
//! the build directory. With one target directory shared by several checkouts,
//! the newest `ridl_rt` library file there can belong to another checkout, and
//! the test would then check the seal of that checkout.

use std::path::PathBuf;
use std::process::Command;

/// What `cargo check` reported for one snippet.
struct Outcome {
    success: bool,
    stderr: String,
}

/// Builds `source` as an edition 2021 library crate that depends on `ridl-rt`.
///
/// The crates of all cases share one target directory, so that `ridl-rt` is
/// built once. Cargo locks that directory, which serialises the cases that the
/// test harness runs in parallel.
fn compile(case: &str, source: &str) -> Outcome {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("seal");
    let crate_dir = root.join(case);
    std::fs::create_dir_all(crate_dir.join("src")).expect("the scratch directory can be created");
    // The empty `[workspace]` table keeps cargo from treating the generated
    // crate as a member of the workspace that encloses the target directory.
    let manifest = format!(
        "[package]\nname = \"{case}\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\
         publish = false\n\n[workspace]\n\n[dependencies]\n\
         ridl-rt = {{ path = {:?} }}\n",
        env!("CARGO_MANIFEST_DIR")
    );
    let manifest_path = crate_dir.join("Cargo.toml");
    std::fs::write(&manifest_path, manifest).expect("the manifest can be written");
    std::fs::write(crate_dir.join("src").join("lib.rs"), source)
        .expect("the snippet can be written");

    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .args(["check", "--offline", "--quiet", "--message-format", "short"])
        .arg("--manifest-path")
        .arg(&manifest_path)
        .arg("--target-dir")
        .arg(root.join("target"))
        .output()
        .expect("cargo can be run");
    Outcome {
        success: output.status.success(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// Asserts that `source` fails to compile with `code` in its output.
fn assert_fails_with(case: &str, code: &str, source: &str) {
    let outcome = compile(case, source);
    assert!(
        !outcome.success,
        "{case} compiled, but it must fail with {code}"
    );
    assert!(
        outcome.stderr.contains(&format!("error[{code}]")),
        "{case} must fail with {code}, but cargo said:\n{}",
        outcome.stderr
    );
}

#[test]
fn a_crate_outside_can_name_an_encoding_and_check_a_ref() {
    let outcome = compile(
        "control",
        r#"
use ridl_rt::encoding::{Encoding, Podbuf};
use ridl_rt::payload::{Payload, Ref, VerifyError};

pub fn name<E: Encoding>() -> &'static str {
    E::NAME
}

pub fn check<'a, T>(bytes: &'a [u8]) -> Result<Ref<'a, T, Podbuf>, VerifyError>
where
    T: Payload<Podbuf, View<'a> = &'a [u8]>,
{
    Ref::verify(bytes)
}
"#,
    );
    assert!(
        outcome.success,
        "the permitted use must compile, so that a failure below is a failure of the \
         snippet under test and not of this harness:\n{}",
        outcome.stderr
    );
}

#[test]
fn a_crate_outside_cannot_implement_encoding_without_the_sealing_trait() {
    assert_fails_with(
        "encoding_impl",
        "E0277",
        r#"
pub struct Json;

impl ridl_rt::encoding::Encoding for Json {
    const NAME: &'static str = "json";
}
"#,
    );
}

#[test]
fn the_sealing_trait_is_not_nameable_outside() {
    assert_fails_with(
        "sealing_trait",
        "E0603",
        r#"
pub struct Json;

impl ridl_rt::encoding::sealed::Sealed for Json {}
"#,
    );
}

#[test]
fn a_ref_cannot_be_built_from_its_fields_outside() {
    assert_fails_with(
        "ref_forge",
        "E0451",
        r#"
use core::marker::PhantomData;
use ridl_rt::encoding::Podbuf;
use ridl_rt::payload::{Payload, Ref};

pub fn forge<'a, T>(bytes: &'a [u8]) -> Ref<'a, T, Podbuf>
where
    T: Payload<Podbuf, View<'a> = &'a [u8]>,
{
    Ref { bytes, view: bytes, _e: PhantomData }
}
"#,
    );
}

#[test]
fn the_bytes_field_of_a_ref_is_private() {
    assert_fails_with(
        "ref_bytes_read",
        "E0616",
        r#"
use ridl_rt::encoding::Podbuf;
use ridl_rt::payload::{Payload, Ref};

pub fn peek<'a, T: Payload<Podbuf>>(r: &Ref<'a, T, Podbuf>) -> &'a [u8] {
    r.bytes
}
"#,
    );
}

#[test]
fn the_view_field_of_a_ref_is_private() {
    assert_fails_with(
        "ref_view_write",
        "E0616",
        r#"
use ridl_rt::encoding::Podbuf;
use ridl_rt::payload::{Payload, Ref};

pub fn overwrite<'a, T: Payload<Podbuf>>(r: &mut Ref<'a, T, Podbuf>, v: T::View<'a>) {
    r.view = v;
}
"#,
    );
}
