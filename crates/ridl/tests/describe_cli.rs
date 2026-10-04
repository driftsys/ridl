//! `ridl build --emit catalog` (spec D-9), through the binary.

use std::path::{Path, PathBuf};
use std::process::Command;

struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("ridl-describe-{label}-{}", std::process::id()));
        // A directory left by an earlier process with the same id is
        // emptied, so a test reads only what its own build wrote.
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create the temporary directory");
        Self(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    /// Writes `contents` to `relative`, creating its parent directories.
    fn write(&self, relative: &str, contents: &str) -> PathBuf {
        let path = self.0.join(relative);
        std::fs::create_dir_all(path.parent().expect("a file has a parent"))
            .expect("create the parent directory");
        std::fs::write(&path, contents).expect("write the file");
        path
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
    build_from(&corpus(), out, emits);
}

/// Builds `entry` with `--emit <emits>` into `out`, asserting exit 0.
fn build_from(entry: &Path, out: &Path, emits: &str) {
    let (code, _, stderr) = ridl(&[
        "build".as_ref(),
        entry.as_os_str(),
        "--out-dir".as_ref(),
        out.as_os_str(),
        "--emit".as_ref(),
        emits.as_ref(),
    ]);
    assert_eq!(code, 0, "`--emit {emits}` failed: {stderr}");
}

/// Every `*.catalog.binfb` in `out`, by file name, sorted.
fn catalogs_in(out: &Path) -> Vec<String> {
    let mut found: Vec<String> = std::fs::read_dir(out)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".catalog.binfb"))
        .collect();
    found.sort();
    found
}

/// The one `*.catalog.binfb` in `out`, which must be the corpus package's
/// `corpus.baseline.catalog.binfb`.
fn the_catalog(out: &Path) -> PathBuf {
    assert_eq!(
        catalogs_in(out),
        vec!["corpus.baseline.catalog.binfb".to_owned()],
        "exactly one catalog descriptor, named after the package"
    );
    out.join("corpus.baseline.catalog.binfb")
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
/// from `ridl_ir::catalog_hash`. The corpus package imports nothing, so this
/// test does not see which `others` each artifact is given;
/// `descriptor_hash_equals_the_rust_face_hash_across_packages` does.
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

/// The descriptor is named `<base>.catalog.binfb` with the base the IR dumps
/// use, the package name: `corpus.baseline.ir.binpb` beside it.
#[test]
fn the_descriptor_takes_the_ir_dumps_base_name() {
    let out = TempDir::new("base");
    build(out.path(), "ir-binary,catalog");
    assert!(out.path().join("corpus.baseline.ir.binpb").is_file());
    assert!(the_catalog(out.path()).is_file());
}

/// The hash of `bytes`, a catalog descriptor, and the one `CatalogHash` every
/// `Interface` impl of the face `source` carries.
fn hashes(bytes: &[u8], source: &str) -> (Vec<u8>, Vec<u8>) {
    let catalog = ridl_descriptor::verify(bytes).expect("the descriptor verifies");
    let descriptor_hash = catalog.hash().expect("the descriptor carries a hash");
    let face_hashes = rust_catalog_hashes(source);
    assert!(!face_hashes.is_empty(), "the face names a catalog hash");
    assert!(
        face_hashes.windows(2).all(|w| w[0] == w[1]),
        "one hash per package"
    );
    (descriptor_hash.to_vec(), face_hashes[0].clone())
}

/// A two-package workspace: `veh.common` declares the types, `veh.cluster`
/// imports them and declares the one interface. The catalog hash of
/// `veh.cluster` covers the imported types, so the descriptor and the face
/// agree only if `ridlc` gives both the same `others`.
#[test]
fn descriptor_hash_equals_the_rust_face_hash_across_packages() {
    let src = TempDir::new("two-src");
    src.write(
        "ridl.toml",
        "[workspace]\nmembers = [\"common\", \"cluster\"]\n",
    );
    src.write(
        "common/ridl.toml",
        "[package]\nname = \"veh.common\"\nversion = \"1.0.0\"\n",
    );
    src.write(
        "common/common.typl",
        "package veh.common\n\
type Speed: km/h [0.0..250.0 step 0.5]\n\
struct Reading {\n  speed: Speed\n}\n",
    );
    src.write(
        "cluster/ridl.toml",
        "[package]\nname = \"veh.cluster\"\nversion = \"1.0.0\"\n",
    );
    src.write(
        "cluster/cluster.ridl",
        "package veh.cluster\n\
import veh.common.Speed\n\
import veh.common.Reading\n\
interface Dash {\n\
  signal speed: Speed @10ms\n\
  event reading: Reading @[100ms..1s]\n\
}\n",
    );
    let out = TempDir::new("two-out");
    build_from(src.path(), out.path(), "rust,catalog");

    // `veh.common` declares no interface shape, so it has no descriptor.
    assert_eq!(
        catalogs_in(out.path()),
        vec!["veh.cluster.catalog.binfb".to_owned()]
    );
    let bytes = std::fs::read(out.path().join("veh.cluster.catalog.binfb")).unwrap();
    let source = std::fs::read_to_string(out.path().join("veh.cluster.rs"))
        .expect("`--emit rust` writes veh.cluster.rs");
    let (descriptor_hash, face_hash) = hashes(&bytes, &source);
    assert_eq!(descriptor_hash, face_hash);
}

/// `--emit catalog` alone keeps `ridl.std` in the `others` that `run_build`
/// passes, because `Emit::Catalog` is classed as a code emit. The payload of
/// the one event is `Timestamp`, a `ridl.std` type, so the catalog hash covers
/// it: the descriptor built with `--emit catalog` must equal the one built with
/// `--emit rust,catalog`, and its hash must equal the face's `CatalogHash`.
#[test]
fn the_catalog_alone_hashes_the_standard_package_as_the_rust_face_does() {
    let src = TempDir::new("std-src");
    let file = src.write(
        "clock.ridl",
        "package veh.clock\n\
interface Clock {\n\
  event ticked: Timestamp @[100ms..1s]\n\
}\n",
    );
    let alone = TempDir::new("std-alone");
    build_from(&file, alone.path(), "catalog");
    let with_face = TempDir::new("std-face");
    build_from(&file, with_face.path(), "rust,catalog");

    assert_eq!(
        catalogs_in(alone.path()),
        ["clock.catalog.binfb"],
        "`--emit catalog` writes no catalog for ridl.std"
    );

    let name = "clock.catalog.binfb";
    let alone_bytes = std::fs::read(alone.path().join(name)).unwrap();
    let face_bytes = std::fs::read(with_face.path().join(name)).unwrap();
    assert_eq!(
        alone_bytes, face_bytes,
        "the descriptor does not depend on the other emits"
    );

    let source = std::fs::read_to_string(with_face.path().join("clock.rs"))
        .expect("`--emit rust` writes clock.rs");
    let (descriptor_hash, face_hash) = hashes(&alone_bytes, &source);
    assert_eq!(descriptor_hash, face_hash);
}

/// A package whose one interface shape is a service's inline body still has
/// a catalog: the descriptor is written and names the interface after the
/// service's dotted name. A single-file build takes the file stem as the
/// base, for the descriptor as for the IR dump.
#[test]
fn a_service_inline_body_alone_gets_a_descriptor() {
    let src = TempDir::new("service-src");
    let file = src.write(
        "hvac.ridl",
        "package veh.hvac\n\
type Temp: Cel [-40.0..125.0 step 0.1]\n\
service veh.hvac.cabin {\n\
  signal cabinTemp: Temp @[500ms..5s]\n\
}\n",
    );
    let out = TempDir::new("service-out");
    build_from(&file, out.path(), "ir-binary,catalog");

    assert!(out.path().join("hvac.ir.binpb").is_file());
    assert_eq!(
        catalogs_in(out.path()),
        vec!["hvac.catalog.binfb".to_owned()]
    );
    let bytes = std::fs::read(out.path().join("hvac.catalog.binfb")).unwrap();
    let catalog = ridl_descriptor::verify(&bytes).expect("the descriptor verifies");
    let names: Vec<&str> = catalog
        .interfaces()
        .unwrap()
        .iter()
        .map(|interface| interface.unwrap().name().unwrap())
        .collect();
    assert_eq!(names, vec!["veh.hvac.cabin"]);
}

#[test]
fn describe_prints_the_descriptor_as_json() {
    let out = TempDir::new("describe");
    let file = build_catalog(out.path());
    let (code, stdout, stderr) = ridl(&["describe".as_ref(), file.as_os_str()]);
    assert_eq!(code, 0, "{stderr}");
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("stdout is JSON");
    assert_eq!(json["version"], 1);
    // The descriptor records the workspace version, which every release
    // changes. The test checks it here and the snapshot holds a placeholder,
    // so a release does not make the snapshot stale.
    assert_eq!(json["toolchain"], env!("CARGO_PKG_VERSION"));
    let stdout = stdout.replace(
        &format!("\"toolchain\": \"{}\"", env!("CARGO_PKG_VERSION")),
        "\"toolchain\": \"[version]\"",
    );
    insta::assert_snapshot!("corpus_catalog", stdout);
}

#[test]
fn describe_reports_a_missing_path_with_exit_2() {
    let missing = "/nonexistent/x.catalog.binfb";
    let cause = std::fs::read(missing).expect_err("the path does not exist");
    assert_eq!(cause.kind(), std::io::ErrorKind::NotFound);
    let (code, stdout, stderr) = ridl(&["describe".as_ref(), missing.as_ref()]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert_eq!(stderr, format!("error: {missing}: {cause}\n"));
}

/// Runs `ridl describe` on `path` and asserts that it is rejected as the
/// tool being unable to answer: exit 2, nothing on stdout, and a stderr
/// message that names `path` and contains `cause`.
fn assert_describe_rejects(path: &Path, cause: &str) {
    let (code, stdout, stderr) = ridl(&["describe".as_ref(), path.as_os_str()]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stdout.is_empty(), "{stdout}");
    assert!(
        stderr.starts_with(&format!("error: {}: ", path.display())),
        "{stderr}"
    );
    assert!(stderr.contains(cause), "{stderr}");
}

#[test]
fn describe_rejects_a_foreign_file_before_any_read() {
    let out = TempDir::new("foreign");
    let file = out.path().join("ir.binpb");
    // Twelve bytes, so the header is long enough and the file identifier
    // check is what rejects it: bytes 4..8 are `abcd`, not `RDLC`.
    std::fs::write(&file, b"\x08\x01\x12\x08abcdefgh").unwrap();
    assert_describe_rejects(&file, "not a catalog descriptor");
}

#[test]
fn describe_rejects_a_truncated_and_a_flipped_descriptor() {
    let out = TempDir::new("corrupt");
    let file = build_catalog(out.path());
    let bytes = std::fs::read(&file).unwrap();

    let truncated = out.path().join("truncated.catalog.binfb");
    std::fs::write(&truncated, &bytes[..bytes.len() / 2]).unwrap();
    assert_describe_rejects(&truncated, "malformed");

    // The root offset at bytes 0..4 is overwritten to point past the end of
    // the buffer.
    let mut past_the_end = bytes.clone();
    past_the_end[0..4].copy_from_slice(&(bytes.len() as u32 + 64).to_le_bytes());
    let path = out.path().join("past-the-end.catalog.binfb");
    std::fs::write(&path, &past_the_end).unwrap();
    assert_describe_rejects(&path, "malformed");

    // One byte inside the body is flipped: the most significant byte of the
    // root table's signed offset to its vtable. The vtable offset then points
    // outside the buffer.
    let root = u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as usize;
    let mut flipped = bytes.clone();
    flipped[root + 3] ^= 0x40;
    let path = out.path().join("flipped.catalog.binfb");
    std::fs::write(&path, &flipped).unwrap();
    assert_describe_rejects(&path, "malformed");
}

/// A descriptor whose schema version is one this toolchain does not read is
/// rejected with exit 2 and the version named.
#[test]
fn describe_rejects_a_version_this_toolchain_does_not_read() {
    let out = TempDir::new("next-version");
    let version = ridl_descriptor::SCHEMA_VERSION + 1;
    let bytes = ridl_descriptor::finish(&ridl_descriptor::Catalog {
        version,
        ..Default::default()
    });
    let path = out.path().join("next.catalog.binfb");
    std::fs::write(&path, bytes).unwrap();
    let cause = ridl_descriptor::VerifyError::WrongVersion(version).to_string();
    assert_describe_rejects(&path, &cause);
}

/// A reader that closes its end of the pipe before reading anything: every
/// write of `ridl describe` to stdout then fails. The descriptor has 2000
/// members, so its JSON view is far larger than a pipe buffer, and a write
/// fails even when the process starts writing before the pipe is closed.
/// A write I/O failure is exit 2 (ADR-0010 decision 1), not the exit 101 of
/// a panic.
#[test]
fn describe_exits_2_when_the_stdout_reader_has_gone() {
    let src = TempDir::new("closed-pipe-src");
    let mut source =
        String::from("package veh.wide\ntype Level: integer [0..100]\ninterface Wide {\n");
    for i in 0..2000 {
        source.push_str(&format!("  signal level{i}: Level @10ms\n"));
    }
    source.push_str("}\n");
    let file = src.write("wide.ridl", &source);
    let out = TempDir::new("closed-pipe-out");
    build_from(&file, out.path(), "catalog");
    let catalog = out.path().join("wide.catalog.binfb");

    let mut child = Command::new(env!("CARGO_BIN_EXE_ridl"))
        .args(["describe".as_ref(), catalog.as_os_str()])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("the ridl binary must run");
    drop(child.stdout.take());
    let output = child.wait_with_output().expect("the ridl binary exits");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "{stderr}");
    // EPIPE is 32 on Linux and macOS.
    let cause = std::io::Error::from_raw_os_error(32);
    assert_eq!(stderr, format!("error: {}: {cause}\n", catalog.display()));
}
