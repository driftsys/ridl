//! The proof of the `wasm` module that `[backend.rust] wasm = true` adds to
//! the generated crate: the cabin example, built with the option, compiled for
//! `wasm32-unknown-unknown` and run in a Node host.
//!
//! The test copies `examples/cabin` into a temporary directory. The example's
//! manifest sets `wasm = true` itself; the tests of the option turned off
//! remove that line from the copy. It then builds two things from one
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

/// The manifest of `examples/cabin`, which sets `wasm = true` in its
/// `[backend.rust]` table for its demos.
fn cabin_manifest() -> String {
    let manifest =
        std::fs::read_to_string(example_dir().join("ridl.toml")).expect("the manifest is read");
    assert!(
        manifest.lines().any(|line| line.trim() == "wasm = true"),
        "the tests expect `wasm = true` in the example's manifest:\n{manifest}"
    );
    manifest
}

/// The manifest of `examples/cabin` without its `wasm = true` line.
fn cabin_manifest_without_wasm() -> String {
    cabin_manifest()
        .lines()
        .filter(|line| line.trim() != "wasm = true")
        .map(|line| format!("{line}\n"))
        .collect()
}

/// A copy of `examples/cabin`'s sources and manifest, with `wasm = true`.
fn cabin_with_wasm(dir: &Path) {
    for file in ["cabin.ridl", "system.rsdl"] {
        std::fs::copy(example_dir().join(file), dir.join(file)).expect("the source is copied");
    }
    std::fs::write(dir.join("ridl.toml"), cabin_manifest()).expect("the manifest copy is written");
}

/// Writes the copy's manifest without `wasm = true`, which turns the option
/// off.
fn turn_wasm_off(dir: &Path) {
    std::fs::write(dir.join("ridl.toml"), cabin_manifest_without_wasm())
        .expect("the manifest without the option is written");
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
use ridl_rt::payload::{Payload, Rule};
use veh_cabin::veh::cabin::{
    Average, Cabin, Diagnostic, Health, Level, Subsystems, Temperature, Warning, Window,
};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn encode(value: &Temperature) -> String {
    let mut buf = <Temperature as Payload<FlatBuffers>>::BUFFER;
    let encoded = <Temperature as Payload<FlatBuffers>>::encode(value, &mut buf)
        .expect("the value encodes");
    hex(encoded.bytes)
}

/// The FlatBuffers bytes of one value of `$ty`, as hex, then the codes the
/// native `verify_by_id` gives those bytes and their first half.
macro_rules! bytes_of {
    ($name:literal, $ty:ty, $value:expr) => {{
        let mut buf = <$ty as Payload<FlatBuffers>>::BUFFER;
        let bytes = <$ty as Payload<FlatBuffers>>::encode(&$value, &mut buf)
            .expect("the value encodes")
            .bytes;
        let id = veh_cabin::wasm::WASM_PAYLOAD_TYPES
            .iter()
            .position(|name| *name == $name)
            .expect("a listed type") as u32;
        format!(
            "{} {} {}",
            hex(bytes),
            veh_cabin::wasm::verify_by_id(id, bytes),
            veh_cabin::wasm::verify_by_id(id, &bytes[..bytes.len() / 2])
        )
    }};
}

fn main() {
    // One valid value of every payload type of the package.
    println!("payload veh.cabin.Average {}", bytes_of!("veh.cabin.Average", Average, Average::new(7).unwrap()));
    println!(
        "payload veh.cabin.Diagnostic {}",
        bytes_of!("veh.cabin.Diagnostic", 
            Diagnostic,
            Diagnostic { covers: Subsystems::HORN, health: Health::Ok, r#loop: Level::new(3).unwrap() }
        )
    );
    println!("payload veh.cabin.Health {}", bytes_of!("veh.cabin.Health", Health, Health::Warn));
    println!("payload veh.cabin.Level {}", bytes_of!("veh.cabin.Level", Level, Level::new(42).unwrap()));
    println!("payload veh.cabin.Subsystems {}", bytes_of!("veh.cabin.Subsystems", Subsystems, Subsystems::CLIMATE));
    println!("payload veh.cabin.Temperature {}", bytes_of!("veh.cabin.Temperature", Temperature, Temperature::new(-3).unwrap()));
    println!(
        "payload veh.cabin.Warning {}",
        bytes_of!("veh.cabin.Warning", Warning, Warning { code: Level::new(5).unwrap(), health: Health::Fail })
    );
    println!("payload veh.cabin.Window {}", bytes_of!("veh.cabin.Window", Window, Window::new(250).unwrap()));
    // The code each rule maps to, paired here with the variant by hand.
    for (text, rule) in [
        ("length", Rule::Length),
        ("pattern", Rule::Pattern),
        ("range", Rule::Range),
        ("step", Rule::Step),
        ("unique", Rule::Unique),
        ("variant", Rule::Variant),
    ] {
        println!("rulecode {text} {}", veh_cabin::wasm::rule_code(rule));
    }
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
  if (testCase.startsWith("at0:")) {
    // Valid bytes written at address 0, then passed as a null pointer.
    const [, typeId, hex] = testCase.split(":");
    const bytes = Uint8Array.from(hex.match(/../g), (pair) => parseInt(pair, 16));
    const saved = new Uint8Array(memory.buffer, 0, bytes.length).slice();
    new Uint8Array(memory.buffer, 0, bytes.length).set(bytes);
    const code = ridl_verify(Number(typeId), 0, bytes.length);
    new Uint8Array(memory.buffer, 0, bytes.length).set(saved);
    return code;
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

// The export table: exactly the memory and the four functions.
const exports = WebAssembly.Module.exports(new WebAssembly.Module(readFileSync(wasmPath)))
  .map((entry) => entry.name)
  .sort();

// Every buffer is aligned to 8, a length of 0 allocates nothing, and freeing
// a null pointer does not trap.
const aligned = [1, 3, 5, 7].every((len) => {
  const ptr = ridl_alloc(len);
  const ok = ptr !== 0 && ptr % 8 === 0;
  ridl_free(ptr, len);
  return ok;
});
const zeroAlloc = ridl_alloc(0);
ridl_free(0, 0);
ridl_free(0, 5);

// A freed buffer is reused: memory does not grow over many cycles.
const grownBefore = memory.buffer.byteLength;
for (let i = 0; i < 5000; i++) {
  const ptr = ridl_alloc(4096);
  ridl_free(ptr, 4096);
}
const grows = memory.buffer.byteLength !== grownBefore;

console.log(JSON.stringify({
  results,
  hash,
  nullHashWrites: before !== after,
  exports,
  aligned,
  zeroAlloc,
  grows,
}));
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
    // Name, bytes, and the native `verify_by_id` codes of the bytes and of
    // their first half.
    let payloads: Vec<(String, String, String, String)> = stdout
        .lines()
        .filter_map(|line| line.strip_prefix("payload "))
        .map(|rest| {
            let mut fields = rest.split(' ');
            let mut next = || fields.next().expect("four fields").to_string();
            (next(), next(), next(), next())
        })
        .collect();
    assert_eq!(
        types,
        payloads
            .iter()
            .map(|(name, ..)| name.clone())
            .collect::<Vec<_>>(),
        "WASM_PAYLOAD_TYPES is every payload type of the package, sorted"
    );
    // Each rule maps to 2 plus its index in WASM_RULES.
    for line in stdout
        .lines()
        .filter_map(|line| line.strip_prefix("rulecode "))
    {
        let (text, code) = line.split_once(' ').expect("a rule and a code");
        let index = rules
            .iter()
            .position(|rule| rule == text)
            .expect("a listed rule");
        assert_eq!(code, (2 + index).to_string(), "the code of rule `{text}`");
    }
    assert_eq!(
        stdout
            .lines()
            .filter(|l| l.starts_with("rulecode "))
            .count(),
        6
    );
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
        // A range past the end of the 32-bit address space, with a length
        // below the type's largest buffer, so that only the guard stops it.
        format!("raw:{temperature}:{}:{}", 0xffff_fff8_u32, 0x10),
        // A null pointer with a length.
        format!("raw:{temperature}:0:5"),
        // A length of `u32::MAX`, above `isize::MAX` on wasm32.
        format!("raw:{temperature}:8:{}", 0xffff_ffff_u32),
        // A length of 0.
        format!("raw:{temperature}:8:0"),
        // A `type_id` of `u32::MAX`.
        format!("{}:{valid}", u32::MAX),
        // The valid bytes with four more bytes after them.
        format!("{temperature}:{valid}00000000"),
        // The valid bytes less their last byte, which is alignment padding
        // after the 1-byte value, so the buffer still verifies.
        format!("{temperature}:{}", &valid[..valid.len() - 2]),
        // Valid bytes copied to address 0 and passed as a null pointer: only
        // the null guard returns 1.
        format!("at0:{temperature}:{valid}"),
        // A length of `isize::MAX + 1` on wasm32. The length guard returns
        // 1 before `from_raw_parts`; without it, the codec's own check of
        // `MAX_SIZE` would also return 1, so this case cannot tell the two
        // apart. It pins the result, not the guard.
        format!("raw:{temperature}:8:{}", 0x8000_0000_u32),
    ];
    // Every payload type, with its valid bytes and with its first half only.
    let mut cases = cases.to_vec();
    for (index, (_, hex, ..)) in payloads.iter().enumerate() {
        cases.push(format!("{index}:{hex}"));
        cases.push(format!("{index}:{}", &hex[..hex.len() / 4 * 2]));
    }
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
        results[..13],
        [0, range_code, 1, -1, 1, 1, 1, 1, -1, 0, 0, 1, 1],
        "valid, out of range, truncated, unknown type_id, overflowing range, null \
         pointer, huge length, length 0, type_id u32::MAX, trailing bytes, one byte \
         less, null pointer over valid bytes, isize::MAX + 1; cases: \
         {cases:?}"
    );
    // Each type: its valid bytes verify, and its first half is malformed.
    assert_eq!(
        results[13..],
        payloads.iter().flat_map(|_| [0, 1]).collect::<Vec<i64>>(),
        "cases: {cases:?}"
    );
    // The native dispatch gives the codes the wasm export gives.
    assert_eq!(
        results[13..],
        payloads
            .iter()
            .flat_map(|(_, _, valid, half)| [valid.parse::<i64>().unwrap(), half.parse().unwrap()])
            .collect::<Vec<i64>>(),
        "native verify_by_id against the wasm ridl_verify"
    );
    assert_eq!(
        report["exports"],
        serde_json::json!([
            "memory",
            "ridl_alloc",
            "ridl_catalog_hash",
            "ridl_free",
            "ridl_verify"
        ])
    );
    assert_eq!(report["aligned"], serde_json::Value::Bool(true));
    assert_eq!(report["zeroAlloc"], serde_json::json!(0));
    assert_eq!(report["grows"], serde_json::Value::Bool(false));
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
    turn_wasm_off(&src);
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
              units and no root package. Build one package whose manifest is its own root, or set \
              `wasm = true` in the build of one member"
        ),
        "{messages:?}"
    );
    assert!(!out.join("lib.rs").exists());
}

/// Builds the cabin copy in `src` into `out` and fails on an error.
fn build_clean(src: &Path, out: &Path) {
    let run = ridlc::run_build(src, out, &[Emit::Rust], false.into()).expect("the build runs");
    assert!(!run.has_error(), "{:?}", run.diagnostics);
}

/// With the option off, nothing of the module is written; with it on, the
/// manifest declares the feature as a TOML value, not only as a line.
#[test]
fn the_option_adds_the_feature_the_module_and_nothing_else() {
    let work = tempfile::tempdir().expect("a temp dir is created");
    let src = work.path().join("src");
    std::fs::create_dir_all(&src).expect("the directory is created");
    cabin_with_wasm(&src);
    let on = work.path().join("on");
    build_clean(&src, &on);
    turn_wasm_off(&src);
    let off = work.path().join("off");
    build_clean(&src, &off);

    let cargo_off = std::fs::read_to_string(off.join("Cargo.toml")).expect("Cargo.toml");
    assert!(!cargo_off.contains("wasm"), "{cargo_off}");
    let lib_off = std::fs::read_to_string(off.join("lib.rs")).expect("lib.rs");
    assert!(!lib_off.contains("wasm"), "{lib_off}");
    assert!(!off.join("__ridl_wasm.rs").exists());

    let manifest: toml::Table = std::fs::read_to_string(on.join("Cargo.toml"))
        .expect("Cargo.toml")
        .parse()
        .expect("the generated manifest is TOML");
    assert_eq!(
        manifest["features"]["wasm-exports"],
        toml::Value::Array(vec![toml::Value::String("std".to_string())])
    );
    assert!(manifest.get("profile").is_none());
    assert!(manifest["lib"].get("crate-type").is_none());
    let mut on_features = manifest["features"].as_table().expect("a table").clone();
    on_features.remove("wasm-exports");
    let off_manifest: toml::Table = cargo_off.parse().expect("TOML");
    assert_eq!(
        &on_features,
        off_manifest["features"].as_table().expect("a table"),
        "the option adds one feature and changes no other"
    );

    // The module carries the generated marker, as every generated file does.
    let module = std::fs::read_to_string(on.join("__ridl_wasm.rs")).expect("the module");
    assert!(
        module.starts_with("// @generated by ridl"),
        "{}",
        &module[..80.min(module.len())]
    );
}

/// A hand-written `__ridl_wasm.rs` is not overwritten: the build is refused,
/// as it is for a hand-written `lib.rs`.
#[test]
fn a_hand_written_module_file_is_refused() {
    let work = tempfile::tempdir().expect("a temp dir is created");
    let src = work.path().join("src");
    let out = work.path().join("out");
    std::fs::create_dir_all(&src).expect("the directory is created");
    std::fs::create_dir_all(&out).expect("the directory is created");
    cabin_with_wasm(&src);
    std::fs::write(out.join("__ridl_wasm.rs"), "// mine\n").expect("the file is written");
    let run = ridlc::run_build(&src, &out, &[Emit::Rust], false.into()).expect("the build runs");
    assert!(run.has_error());
    assert!(
        run.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("__ridl_wasm.rs")),
        "{:?}",
        run.diagnostics
    );
    assert_eq!(
        std::fs::read_to_string(out.join("__ridl_wasm.rs")).expect("the file"),
        "// mine\n"
    );
    assert!(!out.join("lib.rs").exists());
}

/// Builds a one-package manifest named `name` with the option `wasm`, and
/// returns the messages of its error diagnostics.
fn build_named(name: &str, wasm: bool) -> Vec<String> {
    let work = tempfile::tempdir().expect("a temp dir is created");
    let src = work.path().join("src");
    std::fs::create_dir_all(&src).expect("the directory is created");
    let manifest_name = name.split('.').next().expect("a segment");
    std::fs::write(
        src.join("ridl.toml"),
        format!(
            "[package]\nname = \"{manifest_name}\"\nversion = \"1.0.0\"\n\n[backend.rust]\n\
             encodings = [\"flatbuffers\"]\nwasm = {wasm}\n"
        ),
    )
    .expect("the manifest is written");
    // A package below the manifest's own lives in the directory its later
    // segments name.
    let dir = name
        .split('.')
        .skip(1)
        .fold(src.clone(), |dir, segment| dir.join(segment));
    std::fs::create_dir_all(&dir).expect("the package directory is created");
    std::fs::write(
        dir.join("p.ridl"),
        format!("package {name}\n\n/// A level.\ntype Level: integer [0..100]\n"),
    )
    .expect("the source is written");
    let run = ridlc::run_build(&src, &work.path().join("out"), &[Emit::Rust], false.into())
        .expect("the build runs");
    run.diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == ridl_core::Severity::Error)
        .map(|diagnostic| diagnostic.message.clone())
        .collect()
}

/// Only a first segment `wasm` claims the module name.
#[test]
fn only_a_first_segment_wasm_refuses_the_option() {
    for name in ["wasm", "wasm.sub"] {
        let errors = build_named(name, true);
        assert!(
            errors.len() == 1 && errors[0].contains("already uses that module name"),
            "{name}: {errors:?}"
        );
        assert_eq!(
            build_named(name, false),
            Vec::<String>::new(),
            "{name} with the option off"
        );
    }
    for name in ["wasmx", "veh.wasm"] {
        assert_eq!(build_named(name, true), Vec::<String>::new(), "{name}");
    }
}

/// With the option off, a hand-written `__ridl_wasm.rs` in the output
/// directory is neither refused nor removed: the build succeeds and the file
/// is unchanged.
#[test]
fn the_option_off_leaves_a_hand_written_module_file_alone() {
    let work = tempfile::tempdir().expect("a temp dir is created");
    let out = work.path().join("out");
    std::fs::create_dir_all(&out).expect("the directory is created");
    std::fs::write(out.join("__ridl_wasm.rs"), "// mine\n").expect("the file is written");
    let src = work.path().join("src");
    std::fs::create_dir_all(&src).expect("the directory is created");
    cabin_with_wasm(&src);
    turn_wasm_off(&src);
    build_clean(&src, &out);
    assert!(out.join("lib.rs").is_file());
    assert_eq!(
        std::fs::read_to_string(out.join("__ridl_wasm.rs")).expect("the file"),
        "// mine\n"
    );
}
