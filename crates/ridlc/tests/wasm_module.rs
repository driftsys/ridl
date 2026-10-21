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

console.log(JSON.stringify({ results, hash }));
"#;

#[test]
fn the_cabin_wasm_module_verifies_payloads_in_a_node_host() {
    let work = tempfile::tempdir().expect("a temp dir is created");
    let src = work.path().join("src");
    let out = work.path().join("out");
    let libs = work.path().join("libs");
    let wasm_libs = work.path().join("wasm-libs");
    for dir in [&src, &out, &libs, &wasm_libs] {
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
    for line in [
        "crate-type = [\"cdylib\", \"rlib\"]",
        "[profile.release]",
        "opt-level = \"z\"",
        "lto = true",
        "panic = \"abort\"",
    ] {
        assert!(
            cargo_toml.lines().any(|l| l == line),
            "missing `{line}`:\n{cargo_toml}"
        );
    }
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

    // 3 — the same crate as a wasm32 cdylib, over a wasm32 `ridl-rt`.
    let ridl_rt_wasm = wasm_libs.join("libridl_rt.rlib");
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
            r#"feature="proto3""#.as_ref(),
            "--cfg".as_ref(),
            r#"feature="std""#.as_ref(),
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../ridl-rt/src/lib.rs")
                .as_ref(),
            "-o".as_ref(),
            ridl_rt_wasm.as_ref(),
        ],
    );
    let module = wasm_libs.join("veh_cabin.wasm");
    let wasm_dependency = format!("dependency={}", wasm_libs.display());
    let extern_rt_wasm = format!("ridl_rt={}", ridl_rt_wasm.display());
    rustc(
        "the wasm module",
        &[
            "--edition".as_ref(),
            "2021".as_ref(),
            "--target".as_ref(),
            "wasm32-unknown-unknown".as_ref(),
            "--crate-type".as_ref(),
            "cdylib".as_ref(),
            "--crate-name".as_ref(),
            "veh_cabin".as_ref(),
            "-C".as_ref(),
            "opt-level=s".as_ref(),
            "-D".as_ref(),
            "warnings".as_ref(),
            "--cfg".as_ref(),
            r#"feature="std""#.as_ref(),
            "--extern".as_ref(),
            extern_rt_wasm.as_ref(),
            "-L".as_ref(),
            wasm_dependency.as_ref(),
            out.join("lib.rs").as_ref(),
            "-o".as_ref(),
            module.as_ref(),
        ],
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
        [0, range_code, 1, -1],
        "valid, out of range, truncated, unknown type_id; cases: {cases:?}"
    );
    assert_eq!(
        report["hash"].as_str(),
        Some(catalog.as_str()),
        "ridl_catalog_hash writes the native Interface::CATALOG hash"
    );
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
