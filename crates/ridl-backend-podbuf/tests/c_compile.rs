//! The generated headers, compiled as C. The host compiler checks that a
//! header is valid C11 with no warning, `clang` checks that the
//! `_Static_assert` layout checks hold on targets whose ABIs differ from the
//! host's, and a small program reads a hand-written buffer through the
//! accessors.
//!
//! A check that has no compiler to run skips with a notice. Set
//! `RIDL_REQUIRE_CC=1` to turn a skip into a failure.

mod support;

use std::path::Path;

use support::{clang, directory, header, host_cc, run, skip};

/// The targets of the layout check: one that aligns a `u64` to 4 inside a
/// struct, a 64-bit one, and the R52 core.
const TARGETS: &[&str] = &["i686-linux-gnu", "aarch64-linux-gnu", "armv8r-none-eabihf"];

const STRICT: &[&str] = &["-std=c11", "-Wall", "-Wextra", "-Werror", "-fsyntax-only"];

/// The header file of a package: `veh.common` is `veh_common.h`.
fn file_name(package: &str) -> String {
    format!("{}.h", package.replace('.', "_"))
}

fn headers_of(packages: &[ridl_ir::v2::Package], names: &[&str]) -> Vec<(String, String)> {
    names
        .iter()
        .map(|name| (file_name(name), header(packages, name)))
        .collect()
}

fn veh_cluster_headers() -> Vec<(String, String)> {
    headers_of(
        &support::corpus("veh-cluster"),
        &["veh.common", "veh.cluster"],
    )
}

fn two_package_headers() -> Vec<(String, String)> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/two-packages");
    headers_of(&support::workspace(&path), &["pod.parts", "pod.vehicle"])
}

/// A translation unit that includes `headers` in order.
fn unit(headers: &[&str]) -> String {
    let mut text: String = headers
        .iter()
        .map(|name| format!("#include \"{name}\"\n"))
        .collect();
    text.push_str("int main(void) { return 0; }\n");
    text
}

/// `headers` and `extra` written into a fresh directory.
fn with_headers(headers: &[(String, String)], extra: &[(&str, &str)]) -> tempfile::TempDir {
    let mut files: Vec<(&str, &str)> = headers
        .iter()
        .map(|(name, text)| (name.as_str(), text.as_str()))
        .collect();
    files.extend_from_slice(extra);
    directory(&files)
}

#[test]
fn the_veh_cluster_headers_compile_as_c11_on_the_host() {
    let Some(cc) = host_cc() else {
        return skip("no C compiler (cc, clang or gcc) on PATH");
    };
    let source = unit(&["veh_common.h", "veh_cluster.h"]);
    let dir = with_headers(&veh_cluster_headers(), &[("unit.c", &source)]);
    let mut args = STRICT.to_vec();
    args.push("unit.c");
    run(cc, &args, dir.path()).unwrap_or_else(|error| panic!("{error}"));
}

#[test]
fn the_static_asserts_hold_on_i686_aarch64_and_armv8r() {
    let Some(clang) = clang() else {
        return skip("no clang on PATH");
    };
    let source = unit(&["veh_common.h", "veh_cluster.h"]);
    let dir = with_headers(&veh_cluster_headers(), &[("unit.c", &source)]);
    for target in TARGETS {
        let triple = format!("--target={target}");
        let mut args = vec![triple.as_str(), "-ffreestanding"];
        args.extend_from_slice(STRICT);
        args.push("unit.c");
        match run(clang, &args, dir.path()) {
            Ok(()) => {}
            Err(error) if error.contains("unknown target triple") => {
                skip(&format!("this clang does not know the target {target}"));
            }
            Err(error) => panic!("{target}: {error}"),
        }
    }
}

/// Two packages, one of which lays out three heads of the other inline, in
/// one translation unit and in either include order: the guard macro of each
/// head keeps a head from being defined twice.
#[test]
fn the_headers_of_two_packages_compile_in_one_unit_in_either_order() {
    let Some(cc) = host_cc() else {
        return skip("no C compiler (cc, clang or gcc) on PATH");
    };
    let headers = two_package_headers();
    let vehicle = &headers[1].1;
    for head in [
        "pod_parts_wheel_head",
        "pod_parts_hub_head",
        "pod_parts_tyre_head",
    ] {
        assert!(
            vehicle.contains(&format!("typedef struct {head} {{")),
            "pod.vehicle defines {head} itself:\n{vehicle}"
        );
    }
    let forward = unit(&["pod_parts.h", "pod_vehicle.h"]);
    let backward = unit(&["pod_vehicle.h", "pod_parts.h"]);
    let dir = with_headers(
        &headers,
        &[("forward.c", &forward), ("backward.c", &backward)],
    );
    for source in ["forward.c", "backward.c"] {
        let mut args = STRICT.to_vec();
        args.push(source);
        run(cc, &args, dir.path()).unwrap_or_else(|error| panic!("{source}: {error}"));
    }
}

/// The accessors read a hand-written `FaultEvent` buffer, answer 0 for a field
/// past the producer's head, and read a string through its descriptor.
#[test]
fn the_accessors_read_a_buffer_and_answer_zero_past_the_head() {
    let Some(cc) = host_cc() else {
        return skip("no C compiler (cc, clang or gcc) on PATH");
    };
    let program = r#"
#include <string.h>
#include "veh_common.h"
#include "veh_cluster.h"

/* head_size 24, code 513, a message of 5 bytes at offset 32, timestamp 7 */
static const uint8_t full[] = {
  24, 0, 0, 0, 0, 0, 0, 0,
  1, 2, 0, 0, 32, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 0,
  7, 0, 0, 0, 0, 0, 0, 0,
  'h', 'e', 'l', 'l', 'o', 0
};
/* a producer whose head ends after the message descriptor */
static const uint8_t older[] = {
  16, 0, 0, 0, 0, 0, 0, 0,
  1, 2, 0, 0, 32, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 0
};

int main(void) {
  uint32_t len = 99;
  const char *text;
  if (veh_cluster_fault_event_code(full) != 513) return 1;
  if (veh_cluster_fault_event_timestamp(full) != 7) return 2;
  text = veh_cluster_fault_event_message(full, &len);
  if (len != 5 || text == NULL || memcmp(text, "hello", 5) != 0) return 3;
  if (veh_cluster_fault_event_timestamp(older) != 0) return 4;
  if (veh_cluster_fault_event_code(older) != 513) return 5;
  return 0;
}
"#;
    let dir = with_headers(&veh_cluster_headers(), &[("program.c", program)]);
    run(
        cc,
        &[
            "-std=c11",
            "-Wall",
            "-Wextra",
            "-Werror",
            "program.c",
            "-o",
            "program",
        ],
        dir.path(),
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let status = std::process::Command::new(dir.path().join("program"))
        .status()
        .expect("run the program");
    assert!(
        status.success(),
        "the accessor program exited with {status}"
    );
}
