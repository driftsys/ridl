//! The seal on `Encoding` and the privacy of `Ref`'s fields, pinned by error
//! code.
//!
//! A `compile_fail` doctest passes for any compile error, and rustdoc does not
//! check the error code written on its fence. So a doctest cannot tell that
//! the seal failed for the right reason. Each case here compiles one snippet
//! with `rustc` against the `ridl_rt` rlib that this test run built, and
//! asserts the error code in the output. A change that opens the seal or makes
//! a field public fails the code check, and so does a rename that moves the
//! failure to an unresolved name.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The directory that holds the rlibs and the other dependency artifacts of
/// this test run: the directory of the running test executable.
fn deps_dir() -> PathBuf {
    let exe = std::env::current_exe().expect("the test executable has a path");
    exe.parent()
        .expect("the executable is in a directory")
        .to_path_buf()
}

/// The newest `libridl_rt-<hash>.rlib` in `deps`.
fn ridl_rt_rlib(deps: &Path) -> PathBuf {
    let mut found: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(deps)
        .expect("the dependency directory is readable")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("libridl_rt-") && name.ends_with(".rlib"))
        })
        .filter_map(|path| {
            let modified = std::fs::metadata(&path).ok()?.modified().ok()?;
            Some((modified, path))
        })
        .collect();
    found.sort();
    found
        .pop()
        .expect("the ridl_rt rlib is next to the test executable")
        .1
}

/// What `rustc` reported for one snippet.
struct Outcome {
    success: bool,
    stderr: String,
}

/// Compiles `source` as an edition 2021 library that links `ridl_rt`.
fn compile(case: &str, source: &str) -> Outcome {
    let deps = deps_dir();
    let rlib = ridl_rt_rlib(&deps);
    let out_dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("seal")
        .join(case);
    std::fs::create_dir_all(&out_dir).expect("the scratch directory can be created");
    let file = out_dir.join("snippet.rs");
    std::fs::write(&file, source).expect("the snippet can be written");

    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let output = Command::new(rustc)
        .args([
            "--edition",
            "2021",
            "--crate-type",
            "lib",
            "--emit",
            "metadata",
        ])
        .arg("--crate-name")
        .arg(case)
        .arg("--out-dir")
        .arg(&out_dir)
        .arg("-L")
        .arg(format!("dependency={}", deps.display()))
        .arg("--extern")
        .arg(format!("ridl_rt={}", rlib.display()))
        .arg(&file)
        .output()
        .expect("rustc can be run");
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
        "{case} must fail with {code}, but rustc said:\n{}",
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
