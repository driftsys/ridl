//! The proof of the `wasm` module that `[backend.rust] wasm = true` adds to
//! the generated crate: the cabin example, built with the option, compiled for
//! `wasm32-unknown-unknown` and run in a Node host.
//!
//! The test copies `examples/cabin` into a temporary directory and turns the
//! option on in the copy of its manifest. It then builds two things from one
//! generated crate. A native program encodes `Temperature` payloads with the
//! generated FlatBuffers codec and prints the bytes, the catalog hash of
//! `Cabin` and the two lists of the module. A `cdylib` for wasm32 carries the
//! exports, and a Node script calls them on those bytes. `just wasm-check`
//! does not reach generated code, so this test is the guard for the module.
//!
//! The test needs `node` on `PATH` and the `wasm32-unknown-unknown` target of
//! the pinned toolchain. When either is missing it fails; it does not skip.

use std::path::{Path, PathBuf};

use ridlc::Emit;

mod support;

/// The example package whose copy the test builds.
fn example_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("cabin")
}

/// Runs `rustc` with `args` and panics with its diagnostics when it fails.
fn rustc(what: &str, args: &[&std::ffi::OsStr]) {
    let output = std::process::Command::new("rustc")
        .args(args)
        .output()
        .expect("rustc must be installed and runnable for this test to be meaningful");
    assert!(
        output.status.success(),
        "{what} must compile, rustc said:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// A copy of `examples/cabin`'s sources with `wasm = true` added to its
/// `[backend.rust]` table.
fn cabin_with_wasm(dir: &Path) {
    for file in ["cabin.ridl", "system.rsdl"] {
        std::fs::copy(example_dir().join(file), dir.join(file)).expect("the source is copied");
    }
    let manifest =
        std::fs::read_to_string(example_dir().join("ridl.toml")).expect("the manifest is read");
    assert!(
        manifest
            .trim_end()
            .ends_with("encodings = [\"flatbuffers\", \"proto3\"]"),
        "the test appends `wasm` to the `[backend.rust]` table, which must end the manifest:\n\
         {manifest}"
    );
    std::fs::write(
        dir.join("ridl.toml"),
        format!("{}\nwasm = true\n", manifest.trim_end()),
    )
    .expect("the manifest copy is written");
}

/// Writes the cargo workspace around the generated crate: the crate, a
/// consumer that depends on it with its default features off, and the
/// `[patch]` that points `ridl-rt` at this repository's copy, as
/// `examples/cabin/Cargo.toml` does. The lockfile is the cabin example's, so
/// the registry versions are the ones the demo resolves.
fn write_workspace(ws: &Path) {
    let ridl_rt = Path::new(env!("CARGO_MANIFEST_DIR")).join("../ridl-rt");
    std::fs::write(
        ws.join("Cargo.toml"),
        format!(
            "[workspace]\nresolver = \"2\"\nmembers = [\"generated\", \"consumer\"]\n\n\
             [patch.crates-io]\nridl-rt = {{ path = {:?} }}\n",
            ridl_rt.display().to_string()
        ),
    )
    .expect("the workspace manifest is written");
    std::fs::copy(example_dir().join("Cargo.lock"), ws.join("Cargo.lock"))
        .expect("the lockfile is copied");
    std::fs::write(
        ws.join("consumer").join("Cargo.toml"),
        "[package]\nname = \"consumer\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n\
         [dependencies]\nveh_cabin = { path = \"../generated\", default-features = false }\n",
    )
    .expect("the consumer manifest is written");
    std::fs::write(
        ws.join("consumer").join("src").join("lib.rs"),
        "#![no_std]\npub use veh_cabin::wasm::WASM_PAYLOAD_TYPES;\n",
    )
    .expect("the consumer source is written");
}

/// Runs `cargo` with `args` in `dir` and panics with its output when it fails.
fn cargo(what: &str, dir: &Path, target_dir: &Path, args: &[&str]) {
    let output = std::process::Command::new(env!("CARGO"))
        .current_dir(dir)
        .env("CARGO_TARGET_DIR", target_dir)
        .args(args)
        .output()
        .expect("cargo runs");
    assert!(
        output.status.success(),
        "{what} must build, cargo said:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// The native program: one line per fact the wasm host is checked against.
const NATIVE_PROGRAM: &str = r#"
use ridl_rt::contract::Interface;
use ridl_rt::encoding::FlatBuffers;
use ridl_rt::payload::Payload;
use veh_cabin::veh::cabin::{Cabin, Temperature};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn encode(value: &Temperature) -> String {
    let mut buf = <Temperature as Payload<FlatBuffers>>::BUFFER;
    let encoded = <Temperature as Payload<FlatBuffers>>::encode(value, &mut buf)
        .expect("the value encodes");
    hex(encoded.bytes)
}

fn main() {
    println!("valid {}", encode(&Temperature::new(21).expect("21 is in range")));
    println!("range {}", encode(&Temperature::new_unchecked(100)));
    println!("catalog {}", hex(&<Cabin as Interface>::CATALOG.hash.0));
    println!("types {}", veh_cabin::wasm::WASM_PAYLOAD_TYPES.join(","));
    println!("rules {}", veh_cabin::wasm::WASM_RULES.join(","));
}
"#;

/// The Node host: instantiates the module, writes each case's bytes into a
/// buffer from `ridl_alloc`, calls `ridl_verify`, and prints the results and
/// the catalog hash as one JSON object.
const NODE_HOST: &str = r#"
import { readFileSync } from "node:fs";

const [wasmPath, ...cases] = process.argv.slice(2);
const { instance } = await WebAssembly.instantiate(readFileSync(wasmPath), {});
const { memory, ridl_alloc, ridl_free, ridl_verify, ridl_catalog_hash } = instance.exports;

const results = cases.map((testCase) => {
  if (testCase.startsWith("raw:")) {
    // A pointer and a length passed as they are, with no buffer behind them.
    const [, typeId, ptr, len] = testCase.split(":");
    return ridl_verify(Number(typeId), Number(ptr), Number(len));
  }
  const [typeId, hex] = testCase.split(":");
  const bytes = Uint8Array.from(hex.match(/../g) ?? [], (pair) => parseInt(pair, 16));
  const ptr = ridl_alloc(bytes.length);
  new Uint8Array(memory.buffer, ptr, bytes.length).set(bytes);
  const code = ridl_verify(Number(typeId), ptr, bytes.length);
  ridl_free(ptr, bytes.length);
  return code;
});

const out = ridl_alloc(32);
ridl_catalog_hash(out);
const hash = Buffer.from(new Uint8Array(memory.buffer, out, 32)).toString("hex");
ridl_free(out, 32);

// A null `out_ptr` writes nothing.
const before = Buffer.from(new Uint8Array(memory.buffer, 0, 32)).toString("hex");
ridl_catalog_hash(0);
const after = Buffer.from(new Uint8Array(memory.buffer, 0, 32)).toString("hex");

console.log(JSON.stringify({ results, hash, nullHashWrites: before !== after }));
"#;

#[test]
fn the_cabin_wasm_module_verifies_payloads_in_a_node_host() {
    let work = tempfile::tempdir().expect("a temp dir is created");
    let src = work.path().join("src");
    let ws = work.path().join("ws");
    let out = ws.join("generated");
    let libs = work.path().join("libs");
    for dir in [&src, &out, &libs, &ws.join("consumer").join("src")] {
        std::fs::create_dir_all(dir).expect("the directory is created");
    }
    cabin_with_wasm(&src);

    // 1 — the build under test.
    let run = ridlc::run_build(&src, &out, &[Emit::Rust], false.into()).expect("the build runs");
    assert!(
        !run.has_error(),
        "the copy must build clean, diagnostics: {:?}",
        run.diagnostics
            .iter()
            .map(|diagnostic| &diagnostic.message)
            .collect::<Vec<_>>()
    );
    let cargo_toml = std::fs::read_to_string(out.join("Cargo.toml")).expect("Cargo.toml");
    assert!(
        cargo_toml.lines().any(|l| l == "wasm-exports = [\"std\"]"),
        "{cargo_toml}"
    );
    assert!(!cargo_toml.contains("crate-type"), "{cargo_toml}");
    assert!(!cargo_toml.contains("[profile"), "{cargo_toml}");
    write_workspace(&ws);
    let lib_rs = std::fs::read_to_string(out.join("lib.rs")).expect("lib.rs");
    assert!(lib_rs.contains("pub mod wasm;"), "{lib_rs}");

    // 2 — the native crate and program, which produce the bytes and the
    // expected values.
    let ridl_rt = support::ridl_rt_rlib(&libs);
    let veh_cabin = libs.join("libveh_cabin.rlib");
    let dependency = format!("dependency={}", libs.display());
    let extern_rt = format!("ridl_rt={}", ridl_rt.display());
    rustc(
        "the native crate",
        &[
            "--edition".as_ref(),
            "2021".as_ref(),
            "--crate-type".as_ref(),
            "rlib".as_ref(),
            "--crate-name".as_ref(),
            "veh_cabin".as_ref(),
            "--cfg".as_ref(),
            r#"feature="std""#.as_ref(),
            "--extern".as_ref(),
            extern_rt.as_ref(),
            "-L".as_ref(),
            dependency.as_ref(),
            out.join("lib.rs").as_ref(),
            "-o".as_ref(),
            veh_cabin.as_ref(),
        ],
    );
    let program_src = libs.join("native.rs");
    std::fs::write(&program_src, NATIVE_PROGRAM).expect("the program is written");
    let program = libs.join("native");
    let extern_cabin = format!("veh_cabin={}", veh_cabin.display());
    rustc(
        "the native program",
        &[
            "--edition".as_ref(),
            "2021".as_ref(),
            "--extern".as_ref(),
            extern_cabin.as_ref(),
            "--extern".as_ref(),
            extern_rt.as_ref(),
            "-L".as_ref(),
            dependency.as_ref(),
            program_src.as_ref(),
            "-o".as_ref(),
            program.as_ref(),
        ],
    );
    let native = std::process::Command::new(&program)
        .output()
        .expect("the native program runs");
    assert!(native.status.success(), "{native:?}");
    let stdout = String::from_utf8(native.stdout).expect("utf-8 output");
    let fact = |key: &str| -> String {
        stdout
            .lines()
            .find_map(|line| line.strip_prefix(&format!("{key} ")))
            .unwrap_or_else(|| panic!("no `{key}` line in:\n{stdout}"))
            .to_string()
    };
    let valid = fact("valid");
    let out_of_range = fact("range");
    let catalog = fact("catalog");
    let types: Vec<String> = fact("types").split(',').map(str::to_string).collect();
    let rules: Vec<String> = fact("rules").split(',').map(str::to_string).collect();
    let temperature = types
        .iter()
        .position(|name| name == "veh.cabin.Temperature")
        .expect("Temperature is a payload type");
    let range_code = 2 + rules
        .iter()
        .position(|rule| rule == "range")
        .expect("range is a rule") as i64;
    assert_ne!(valid, out_of_range, "the two values encode differently");
    assert_eq!(catalog.len(), 64);

    // 3 — the same crate as a wasm32 cdylib, built the way the generated
    // manifest documents: `cargo rustc` with the `wasm-exports` feature, in a
    // workspace whose `ridl-rt` is this repository's copy.
    let target_dir = work.path().join("target");
    cargo(
        "the wasm module",
        &ws,
        &target_dir,
        &[
            "rustc",
            "-p",
            "veh_cabin",
            "--release",
            "--target",
            "wasm32-unknown-unknown",
            "--crate-type",
            "cdylib",
            "--features",
            "wasm-exports",
            "--",
            "-D",
            "warnings",
        ],
    );
    let module = target_dir.join("wasm32-unknown-unknown/release/veh_cabin.wasm");
    assert!(
        module.is_file(),
        "cargo wrote no module at {}",
        module.display()
    );

    // With the option on, the crate still builds without its default
    // features natively, and a consumer that turns them off still builds.
    cargo(
        "the crate without default features",
        &ws,
        &target_dir,
        &["check", "-p", "veh_cabin", "--no-default-features"],
    );
    cargo(
        "a consumer with default-features = false",
        &ws,
        &target_dir,
        &["check", "-p", "consumer"],
    );

    // 4 — the Node host.
    let host = work.path().join("host.mjs");
    std::fs::write(&host, NODE_HOST).expect("the host script is written");
    let truncated = &valid[..valid.len() / 2];
    let cases = [
        format!("{temperature}:{valid}"),
        format!("{temperature}:{out_of_range}"),
        format!("{temperature}:{truncated}"),
        format!("{}:{valid}", types.len()),
        // A range past the end of the 32-bit address space.
        format!("raw:{temperature}:{}:{}", 0xffff_fff0_u32, 0x40),
        // A null pointer with a length.
        format!("raw:{temperature}:0:5"),
        // A length above `isize::MAX` on wasm32.
        format!("raw:{temperature}:8:{}", 0xffff_ffff_u32),
    ];
    let node = std::process::Command::new("node")
        .arg(&host)
        .arg(&module)
        .args(&cases)
        .output()
        .expect("`node` must be on PATH for this test; it is not skipped without it");
    assert!(
        node.status.success(),
        "the host must run, node said:\n{}",
        String::from_utf8_lossy(&node.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&node.stdout).expect("the host prints one JSON object");
    let results: Vec<i64> = report["results"]
        .as_array()
        .expect("a result list")
        .iter()
        .map(|code| code.as_i64().expect("an integer code"))
        .collect();
    assert_eq!(
        results,
        [0, range_code, 1, -1, 1, 1, 1],
        "valid, out of range, truncated, unknown type_id, overflowing range, null \
         pointer, huge length; cases: {cases:?}"
    );
    assert_eq!(
        report["hash"].as_str(),
        Some(catalog.as_str()),
        "ridl_catalog_hash writes the native Interface::CATALOG hash"
    );
    assert_eq!(report["nullHashWrites"], serde_json::Value::Bool(false));
}

/// A package whose first segment is `wasm` would claim the module name the
/// option adds, so the build is refused and writes no crate files.
#[test]
fn a_package_named_wasm_refuses_the_option() {
    let work = tempfile::tempdir().expect("a temp dir is created");
    let src = work.path().join("src");
    let out = work.path().join("out");
    std::fs::create_dir_all(&src).expect("the directory is created");
    std::fs::write(
        src.join("ridl.toml"),
        "[package]\nname = \"wasm\"\nversion = \"1.0.0\"\n\n[backend.rust]\nencodings = [\"flatbuffers\"]\nwasm = true\n",
    )
    .expect("the manifest is written");
    std::fs::write(
        src.join("wasm.ridl"),
        "package wasm\n\n/// A level.\ntype Level: integer [0..100]\n",
    )
    .expect("the source is written");
    let run = ridlc::run_build(&src, &out, &[Emit::Rust], false.into()).expect("the build runs");
    let messages: Vec<&str> = run
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect();
    assert!(
        messages
            .iter()
            .any(|message| message.contains("package `wasm` already uses that module name")),
        "{messages:?}"
    );
    assert!(run.has_error());
    assert!(!out.join("lib.rs").exists());
}

/// Builds `ridl-rt` for wasm32 with the features the generated crate turns on.
fn ridl_rt_wasm_rlib(dir: &Path) -> PathBuf {
    let rlib = dir.join("libridl_rt.rlib");
    rustc(
        "ridl-rt for wasm32",
        &[
            "--edition".as_ref(),
            "2021".as_ref(),
            "--target".as_ref(),
            "wasm32-unknown-unknown".as_ref(),
            "--crate-type".as_ref(),
            "rlib".as_ref(),
            "--crate-name".as_ref(),
            "ridl_rt".as_ref(),
            "--cfg".as_ref(),
            r#"feature="flatbuffers""#.as_ref(),
            "--cfg".as_ref(),
            r#"feature="std""#.as_ref(),
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../ridl-rt/src/lib.rs")
                .as_ref(),
            "-o".as_ref(),
            rlib.as_ref(),
        ],
    );
    rlib
}

/// A crate with no payload type still gets a module that compiles for wasm32
/// with its exports on and every warning denied: no unused binding and no
/// dead function.
#[test]
fn a_module_with_no_payload_type_builds_without_warnings() {
    let work = tempfile::tempdir().expect("a temp dir is created");
    let source = ridl_backend_rust::generate_wasm_module(&[0; 32], &[])
        .expect("the module generates")
        .rust_source;
    assert!(
        source.contains("WASM_PAYLOAD_TYPES: &[&str] = &[];"),
        "{source}"
    );
    let root = work.path().join("lib.rs");
    std::fs::write(&root, source).expect("the module is written");
    let ridl_rt = ridl_rt_wasm_rlib(work.path());
    let extern_rt = format!("ridl_rt={}", ridl_rt.display());
    rustc(
        "a module with no payload type",
        &[
            "--edition".as_ref(),
            "2021".as_ref(),
            "--target".as_ref(),
            "wasm32-unknown-unknown".as_ref(),
            "--crate-type".as_ref(),
            "cdylib".as_ref(),
            "--crate-name".as_ref(),
            "empty".as_ref(),
            "--cfg".as_ref(),
            r#"feature="wasm-exports""#.as_ref(),
            "-D".as_ref(),
            "warnings".as_ref(),
            "--extern".as_ref(),
            extern_rt.as_ref(),
            root.as_ref(),
            "-o".as_ref(),
            work.path().join("empty.wasm").as_ref(),
        ],
    );
}

/// `WASM_RULES` names every variant of `ridl_rt::payload::Rule`, and no other,
/// so a rule added to or removed from `ridl-rt` fails here until the list
/// that fixes the rule codes is updated. `Rule` is `#[non_exhaustive]`, so an
/// exhaustive match outside `ridl-rt` is not possible; the variants are read
/// from its source instead.
#[test]
fn the_rule_list_matches_the_runtime_rule_enum() {
    let payload = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../ridl-rt/src/payload.rs"),
    )
    .expect("ridl-rt's payload module is read");
    let body = payload
        .split("pub enum Rule {")
        .nth(1)
        .and_then(|rest| rest.split("\n}").next())
        .expect("ridl-rt declares `pub enum Rule`");
    let mut variants: Vec<String> = body
        .lines()
        .map(str::trim)
        .filter(|line| !line.starts_with("//") && !line.starts_with('#') && !line.is_empty())
        .map(|line| line.trim_end_matches(',').to_lowercase())
        .collect();
    variants.sort();
    let source = ridl_backend_rust::generate_wasm_module(&[0; 32], &[])
        .expect("the module generates")
        .rust_source;
    let dense: String = source.chars().filter(|c| !c.is_whitespace()).collect();
    let list = dense
        .split("WASM_RULES:&[&str]=&[")
        .nth(1)
        .and_then(|rest| rest.split(']').next())
        .expect("the module declares WASM_RULES");
    let rules: Vec<String> = list
        .split(',')
        .filter(|rule| !rule.is_empty())
        .map(|rule| rule.trim_matches('"').to_string())
        .collect();
    assert_eq!(rules, variants);
}

/// Turning the option off removes the module an earlier build wrote, and the
/// crate root stops naming it.
#[test]
fn turning_the_option_off_removes_the_module() {
    let work = tempfile::tempdir().expect("a temp dir is created");
    let src = work.path().join("src");
    let out = work.path().join("out");
    std::fs::create_dir_all(&src).expect("the directory is created");
    cabin_with_wasm(&src);
    let build = || {
        let run =
            ridlc::run_build(&src, &out, &[Emit::Rust], false.into()).expect("the build runs");
        assert!(!run.has_error(), "{:?}", run.diagnostics);
    };
    build();
    assert!(out.join("__ridl_wasm.rs").is_file());
    std::fs::copy(example_dir().join("ridl.toml"), src.join("ridl.toml"))
        .expect("the manifest is restored");
    build();
    assert!(!out.join("__ridl_wasm.rs").exists());
    let lib_rs = std::fs::read_to_string(out.join("lib.rs")).expect("lib.rs");
    assert!(!lib_rs.contains("mod wasm"), "{lib_rs}");
}

/// A `[workspace]` manifest names no root package, so a workspace of two
/// units has no one catalog hash for the module, and the build is refused.
#[test]
fn a_workspace_of_two_units_refuses_the_option() {
    let work = tempfile::tempdir().expect("a temp dir is created");
    let root = work.path().join("src");
    for member in ["a", "b"] {
        let dir = root.join(member);
        std::fs::create_dir_all(&dir).expect("the member directory is created");
        std::fs::write(
            dir.join("ridl.toml"),
            format!("[package]\nname = \"{member}\"\nversion = \"1.0.0\"\n"),
        )
        .expect("the member manifest is written");
        std::fs::write(
            dir.join(format!("{member}.ridl")),
            format!("package {member}\n\n/// A level.\ntype Level: integer [0..100]\n"),
        )
        .expect("the member source is written");
    }
    std::fs::write(
        root.join("ridl.toml"),
        "[workspace]\nmembers = [\"a\", \"b\"]\n\n[backend.rust]\nencodings = [\"flatbuffers\"]\nwasm = true\n",
    )
    .expect("the workspace manifest is written");
    let out = work.path().join("out");
    let run = ridlc::run_build(&root, &out, &[Emit::Rust], false.into()).expect("the build runs");
    let messages: Vec<&str> = run
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect();
    assert!(
        messages.contains(
            &"`[backend.rust] wasm = true` writes the catalog hash of one unit; this build has 2 \
              units and no root package"
        ),
        "{messages:?}"
    );
    assert!(!out.join("lib.rs").exists());
}
