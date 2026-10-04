//! `ridl build --emit catalog` and `ridl describe` (spec D-9), through the binary.

use std::path::{Path, PathBuf};
use std::process::Command;

struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("ridl-describe-{label}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create the temporary directory");
        Self(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Runs `ridl` with `args`, returning `(exit_code, stdout, stderr)`.
fn ridl(args: &[&std::ffi::OsStr]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_ridl"))
        .args(args)
        .output()
        .expect("the ridl binary must run");
    let code = output.status.code().expect("the process exits with a code");
    (
        code,
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// The checked-in corpus package with one declared interface and one
/// inline-form service — two interface shapes
/// (`crates/ridl/tests/baseline-corpus`).
fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/baseline-corpus")
}

/// Builds the corpus with `--emit <emits>` into `out`, asserting exit 0.
fn build(out: &Path, emits: &str) {
    let (code, _, stderr) = ridl(&[
        "build".as_ref(),
        corpus().as_os_str(),
        "--out-dir".as_ref(),
        out.as_os_str(),
        "--emit".as_ref(),
        emits.as_ref(),
    ]);
    assert_eq!(code, 0, "`--emit {emits}` failed: {stderr}");
}

/// The one `*.catalog.binfb` in `out`.
fn the_catalog(out: &Path) -> PathBuf {
    let mut found: Vec<PathBuf> = std::fs::read_dir(out)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.to_string_lossy().ends_with(".catalog.binfb"))
        .collect();
    assert_eq!(found.len(), 1, "exactly one catalog descriptor: {found:?}");
    found.pop().unwrap()
}

/// Builds the corpus with `--emit catalog` into `out` and returns the one
/// `*.catalog.binfb` it wrote.
fn build_catalog(out: &Path) -> PathBuf {
    build(out, "catalog");
    the_catalog(out)
}

#[test]
fn build_emits_one_catalog_descriptor_with_the_identifier() {
    let out = TempDir::new("emit");
    let file = build_catalog(out.path());
    let bytes = std::fs::read(&file).unwrap();
    assert_eq!(&bytes[4..8], b"RDLC");
}

#[test]
fn build_emits_the_same_bytes_twice() {
    let first = TempDir::new("stable-1");
    let second = TempDir::new("stable-2");
    let a = std::fs::read(build_catalog(first.path())).unwrap();
    let b = std::fs::read(build_catalog(second.path())).unwrap();
    assert_eq!(a, b);
}

/// driftsys/ridl#275's criterion, as driver §4 answer 11 gives it to #378:
/// the hash — and the whole descriptor — is the same whether a build emits
/// proto3, FlatBuffers or both beside the catalog.
#[test]
fn build_writes_the_same_descriptor_whatever_else_it_emits() {
    let mut descriptors = Vec::new();
    for (label, emits) in [
        ("alone", "catalog"),
        ("proto", "proto,catalog"),
        ("fbs", "flatbuffers,catalog"),
        ("both", "proto,flatbuffers,catalog"),
    ] {
        let out = TempDir::new(label);
        build(out.path(), emits);
        descriptors.push(std::fs::read(the_catalog(out.path())).unwrap());
    }
    assert!(descriptors.windows(2).all(|w| w[0] == w[1]));
}

/// Every `CatalogHash([..])` literal in generated Rust source, each parsed to
/// its bytes. The parse reads the text between `CatalogHash(` and the next
/// `]`, and accepts a `u8` suffix and any whitespace between the elements.
fn rust_catalog_hashes(source: &str) -> Vec<Vec<u8>> {
    source
        .split("CatalogHash(")
        .skip(1)
        .map(|rest| {
            let open = rest.find('[').expect("`CatalogHash(` is followed by `[`");
            let close = rest.find(']').expect("the array closes");
            rest[open + 1..close]
                .split(',')
                .map(str::trim)
                .filter(|element| !element.is_empty())
                .map(|element| {
                    element
                        .trim_end_matches("u8")
                        .parse::<u8>()
                        .unwrap_or_else(|err| panic!("`{element}` is not a byte: {err}"))
                })
                .collect()
        })
        .collect()
}

/// The descriptor and the generated Rust face are the two artifacts a pair is
/// built from, and the port's catalog check compares their hashes. Both come
/// from `ridl_ir::catalog_hash`; this fails if `ridlc` gives the two
/// different `others`.
#[test]
fn descriptor_hash_equals_the_rust_face_hash() {
    let out = TempDir::new("rust-hash");
    build(out.path(), "rust,catalog");

    let bytes = std::fs::read(the_catalog(out.path())).unwrap();
    let catalog = ridl_descriptor::verify(&bytes).expect("the descriptor verifies");
    let descriptor_hash = catalog.hash().expect("the descriptor carries a hash");
    assert_eq!(descriptor_hash.len(), 32);

    let source = std::fs::read_to_string(out.path().join("corpus.baseline.rs"))
        .expect("`--emit rust` writes <package>.rs");
    let face_hashes = rust_catalog_hashes(&source);
    // Every `Interface` impl the face emits carries the package's one hash.
    assert!(!face_hashes.is_empty(), "the face names a catalog hash");
    for face_hash in &face_hashes {
        assert_eq!(face_hash.as_slice(), descriptor_hash);
    }
}

/// A package with no interface shape has no catalog: `--emit catalog`
/// succeeds and writes no descriptor.
#[test]
fn build_writes_no_descriptor_for_a_package_without_an_interface_shape() {
    let src = TempDir::new("no-shape-src");
    let file = src.path().join("types.typl");
    std::fs::write(&file, "package veh.types\ntype Level: integer [0..100]\n").unwrap();
    let out = TempDir::new("no-shape-out");
    let (code, _, stderr) = ridl(&[
        "build".as_ref(),
        file.as_os_str(),
        "--out-dir".as_ref(),
        out.path().as_os_str(),
        "--emit".as_ref(),
        "catalog".as_ref(),
    ]);
    assert_eq!(code, 0, "build failed: {stderr}");
    let written: Vec<PathBuf> = std::fs::read_dir(out.path())
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.to_string_lossy().ends_with(".catalog.binfb"))
        .collect();
    assert!(written.is_empty(), "no catalog descriptor: {written:?}");
}
