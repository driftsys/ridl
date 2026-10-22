// A wasm module that follows the codec ABI, built from text for the tests.
//
// - ridl_alloc is a bump allocator that grows the memory when it needs room,
//   and counts its calls in the exported global `allocs`. The option
//   `allocFails` makes it return 0, as an allocator does when out of memory.
// - ridl_free only counts its calls, in the exported global `frees`.
// - ridl_catalog_hash writes the 32 bytes at offset 16 of the data segment.
// - ridl_verify returns -1 for a type_id of 2 or more, 1 for a length of 0,
//   and otherwise the value of byte 0 of the buffer (type_id 0).
//   For type_id 1 it returns 0.

import wabt from "wabt";

export const HASH = Uint8Array.from({ length: 32 }, (_, i) => i + 1);

export interface FixtureOptions {
  /** Export ridl_verify. Default true. */
  verify?: boolean;
  /** Export memory. Default true. */
  memory?: boolean;
  /** Export ridl_free as a global, not a function. Default false. */
  freeIsGlobal?: boolean;
  /** Make type_id 1 return -5, an invalid code. Default false. */
  badCode?: boolean;
  /** Make type_id 1 trap with `unreachable`. Default false. */
  trap?: boolean;
  /**
   * Make ridl_alloc return 0, the failure value. `"always"` fails every call;
   * a number fails only a request of more than that many bytes.
   */
  allocFails?: "always" | number;
  /** Make ridl_alloc return 0 for a request of 0 bytes. Default false. */
  allocNullForEmpty?: boolean;
}

function wat(o: FixtureOptions): string {
  const withVerify = o.verify ?? true;
  const hashData = Array.from(
    HASH,
    (b) => "\\" + b.toString(16).padStart(2, "0"),
  )
    .join("");
  return `(module
  (memory ${o.memory === false ? "" : '(export "memory")'} 1)
  (data (i32.const 16) "${hashData}")
  (global $next (mut i32) (i32.const 1024))
  (global $allocs (export "allocs") (mut i32) (i32.const 0))
  (global $frees (export "frees") (mut i32) (i32.const 0))
  (global $ops (mut i32) (i32.const 0))
  (global $verifySeq (export "verifySeq") (mut i32) (i32.const 0))
  (global $freeSeq (export "freeSeq") (mut i32) (i32.const 0))
  (global $allocPtr (export "allocPtr") (mut i32) (i32.const 0))
  (global $allocLen (export "allocLen") (mut i32) (i32.const 0))
  (global $freePtr (export "freePtr") (mut i32) (i32.const 0))
  (global $freeLen (export "freeLen") (mut i32) (i32.const 0))
  (func (export "ridl_alloc") (param $len i32) (result i32)
    (local $ptr i32) (local $end i32)
    (global.set $allocs (i32.add (global.get $allocs) (i32.const 1)))
    ${
    o.allocFails === "always"
      ? "(return (i32.const 0))"
      : o.allocFails !== undefined
      ? `(if (i32.gt_u (local.get $len) (i32.const ${o.allocFails}))
      (then (return (i32.const 0))))`
      : ""
  }
    ${
    o.allocNullForEmpty
      ? "(if (i32.eqz (local.get $len)) (then (return (i32.const 0))))"
      : ""
  }
    (local.set $ptr (global.get $next))
    (local.set $end (i32.add (local.get $ptr) (local.get $len)))
    ;; Grow by one page when the end passes the memory size.
    (if (i32.gt_u (local.get $end) (i32.mul (memory.size) (i32.const 65536)))
      (then
        (drop (memory.grow
          (i32.add
            (i32.div_u (local.get $len) (i32.const 65536))
            (i32.const 1))))))
    (global.set $next (local.get $end))
    (global.set $allocPtr (local.get $ptr))
    (global.set $allocLen (local.get $len))
    (local.get $ptr))
  ${
    o.freeIsGlobal
      ? `(global (export "ridl_free") i32 (i32.const 0))`
      : `(func (export "ridl_free") (param $p i32) (param $l i32)
    (global.set $frees (i32.add (global.get $frees) (i32.const 1)))
    (global.set $ops (i32.add (global.get $ops) (i32.const 1)))
    (global.set $freeSeq (global.get $ops))
    (global.set $freePtr (local.get $p))
    (global.set $freeLen (local.get $l)))`
  }
  (func (export "ridl_catalog_hash") (param $out i32)
    (memory.copy (local.get $out) (i32.const 16) (i32.const 32)))
  ${
    withVerify
      ? `(func (export "ridl_verify") (param $t i32) (param $p i32) (param $l i32) (result i32)
    (global.set $ops (i32.add (global.get $ops) (i32.const 1)))
    (global.set $verifySeq (global.get $ops))
    ${
        o.trap
          ? "(if (i32.eq (local.get $t) (i32.const 1)) (then unreachable))"
          : ""
      }
    ${
        o.badCode
          ? "(if (i32.eq (local.get $t) (i32.const 1)) (then (return (i32.const -5))))"
          : ""
      }
    (if (i32.ge_u (local.get $t) (i32.const 2)) (then (return (i32.const -1))))
    (if (i32.eqz (local.get $l)) (then (return (i32.const 1))))
    (if (i32.eqz (local.get $t))
      (then (return (i32.load8_u (local.get $p)))))
    (i32.const 0))`
      : ""
  }
)`;
}

const toolkit = await wabt();

/** The bytes of the fixture module; the options remove or break one export. */
export function fixtureBytes(
  options: FixtureOptions = {},
): Uint8Array<ArrayBuffer> {
  const module = toolkit.parseWat("fixture.wat", wat(options), {
    bulk_memory: true,
  });
  try {
    const { buffer } = module.toBinary({});
    return new Uint8Array(buffer);
  } finally {
    module.destroy();
  }
}
