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

/// Whichever header defines a shared head first, a member of a tuple head has
/// the type the other header names for it: a pointer to the member assigns to
/// a pointer of the type each header spells, in either include order. A head
/// that named a same-shape head, or a head under another name, would make the
/// assignment an incompatible pointer type, which `-Werror` refuses
/// (driftsys/ridl#1112).
#[test]
fn a_shared_tuple_head_has_one_type_in_either_include_order() {
    let Some(cc) = host_cc() else {
        return skip("no C compiler (cc, clang or gcc) on PATH");
    };
    const BODY: &str = r#"
int main(void) {
  pod_parts_pair_head pair;
  pod_parts_hub_size_head size;
  pod_vehicle_axle_head axle;
  pod_parts_pair_a_head *a = &pair.a;
  pod_parts_pair_b_head *b = &pair.b;
  pod_parts_hub_size_field3_head *inner = &size.inner;
  pod_vehicle_axle_own_head *own = &axle.own;
  pod_parts_pair_a_head *pair_a = &axle.pair.a;
  pod_parts_hub_size_field3_head *hub_inner = &axle.hub.size.inner;
  (void)a;
  (void)b;
  (void)inner;
  (void)own;
  (void)pair_a;
  (void)hub_inner;
  return 0;
}
"#;
    let forward = format!("#include \"pod_parts.h\"\n#include \"pod_vehicle.h\"\n{BODY}");
    let backward = format!("#include \"pod_vehicle.h\"\n#include \"pod_parts.h\"\n{BODY}");
    let dir = with_headers(
        &two_package_headers(),
        &[("forward.c", &forward), ("backward.c", &backward)],
    );
    for source in ["forward.c", "backward.c"] {
        let mut args = STRICT.to_vec();
        args.extend_from_slice(&["-Werror=incompatible-pointer-types", source]);
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

/// The presence test of an optional field reads its own bit: a bit above 0 of
/// the first flags byte, and bit 0 of a second flags byte.
#[test]
fn the_presence_tests_read_a_bit_above_zero_and_a_second_flags_byte() {
    let Some(cc) = host_cc() else {
        return skip("no C compiler (cc, clang or gcc) on PATH");
    };
    let program = r#"
#include "pod_parts.h"

/* Sparse: the flags byte at 0, o1..o8 at 1..9, the second flags byte at 9,
   o9 at 10; head 16. o2 = 2, o4 = 4 and o9 = 9 are present: bits 1 and 3 of
   the first flags byte, bit 0 of the second. */
static const uint8_t sparse[] = {
  16, 0, 0, 0, 0, 0, 0, 0,
  0x0A, 0, 2, 0, 4, 0, 0, 0, 0, 0x01, 9, 0, 0, 0, 0, 0
};

int main(void) {
  if (pod_parts_sparse_has_o1(sparse) != 0) return 1;
  if (pod_parts_sparse_has_o2(sparse) != 1) return 2;
  if (pod_parts_sparse_o2(sparse) != 2) return 3;
  if (pod_parts_sparse_has_o3(sparse) != 0) return 4;
  if (pod_parts_sparse_has_o4(sparse) != 1) return 5;
  if (pod_parts_sparse_o4(sparse) != 4) return 6;
  if (pod_parts_sparse_has_o8(sparse) != 0) return 7;
  if (pod_parts_sparse_has_o9(sparse) != 1) return 8;
  if (pod_parts_sparse_o9(sparse) != 9) return 9;
  return 0;
}
"#;
    let dir = with_headers(&two_package_headers(), &[("program.c", program)]);
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
        "the presence program exited with {status}"
    );
}

/// An accessor over a head of a given size answers 0 for a field past that
/// size, in the root head and in an array element whose stride is smaller than
/// the reader's head, and a signed field comes back signed.
#[test]
fn the_accessors_bound_every_head_by_the_producer_size() {
    let Some(cc) = host_cc() else {
        return skip("no C compiler (cc, clang or gcc) on PATH");
    };
    let program = r#"
#include "pod_parts.h"

/* Reading: delta (i16) at 0, extra (u64) at 8; head 16 bytes. delta = -2,
   extra = 9. */
static const uint8_t reading[] = {
  16, 0, 0, 0, 0, 0, 0, 0,
  0xFE, 0xFF, 0, 0, 0, 0, 0, 0, 9, 0, 0, 0, 0, 0, 0, 0
};
/* A Bag whose producer wrote 8-byte elements: two elements at offsets 16 and
   24, with delta -2 and 5 and no `extra`. */
static const uint8_t bag[] = {
  8, 0, 0, 0, 0, 0, 0, 0,
  16, 0, 0, 0, 2, 0, 8, 0,
  0xFE, 0xFF, 0, 0, 0, 0, 0, 0,
  5, 0, 0, 0, 0, 0, 0, 0
};
/* The same Bag with 16-byte elements: one element, extra = 9. */
static const uint8_t bag16[] = {
  8, 0, 0, 0, 0, 0, 0, 0,
  16, 0, 0, 0, 1, 0, 16, 0,
  7, 0, 0, 0, 0, 0, 0, 0, 9, 0, 0, 0, 0, 0, 0, 0
};

int main(void) {
  int64_t delta;
  uint16_t count = 0, stride = 0;
  const uint8_t *items;

  delta = pod_parts_reading_delta(reading);
  if (delta != -2) return 1;
  if (pod_parts_reading_extra(reading) != 9) return 2;

  items = pod_parts_bag_items(bag, &count, &stride);
  if (items == NULL || count != 2 || stride != 8) return 3;
  delta = pod_parts_reading_delta_at(items, stride);
  if (delta != -2) return 4;
  if (pod_parts_reading_extra_at(items, stride) != 0) return 5;
  delta = pod_parts_reading_delta_at(items + stride, stride);
  if (delta != 5) return 6;
  if (pod_parts_reading_extra_at(items + stride, stride) != 0) return 7;

  items = pod_parts_bag_items(bag16, &count, &stride);
  if (items == NULL || count != 1 || stride != 16) return 8;
  if (pod_parts_reading_extra_at(items, stride) != 9) return 9;
  return 0;
}
"#;
    let dir = with_headers(&two_package_headers(), &[("program.c", program)]);
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
