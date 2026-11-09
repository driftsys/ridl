import { assertEquals } from "@std/assert";
import { offFloatPoints, offFloatStep } from "../mod.ts";
import type { FloatLattice } from "../mod.ts";

// The grid 0.5 * k, which binary64 and binary32 both hold exactly.
const half: FloatLattice = [1, 0.5, 0.5, 0, 0, 0];

Deno.test("offFloatStep accepts the points of an exact lattice", () => {
  for (const wire32 of [false, true]) {
    for (const value of [-1.5, 0, 0.5, 3, 1e6]) {
      assertEquals(offFloatStep(value, wire32, half), false, `${value}`);
    }
  }
});

Deno.test("offFloatStep refuses a value between two points", () => {
  for (const wire32 of [false, true]) {
    for (const value of [0.25, -0.75, 1.1, 1e6 + 0.25]) {
      assertEquals(offFloatStep(value, wire32, half), true, `${value}`);
    }
  }
});

Deno.test("offFloatStep refuses a value that is not finite", () => {
  for (const value of [NaN, Infinity, -Infinity]) {
    assertEquals(offFloatStep(value, false, half), true);
    assertEquals(offFloatStep(value, true, half), true);
  }
});

Deno.test("offFloatStep projects to 32 bits before it checks", () => {
  // 1 + 2^-40 is not a 32-bit value; it rounds to 1, a point of the lattice.
  const value = 1 + 2 ** -40;
  assertEquals(offFloatStep(value, true, half), false);
  assertEquals(offFloatStep(value, false, half), true);
  // A value beyond the 32-bit range projects to infinity.
  assertEquals(offFloatStep(1e39, true, half), true);
});

Deno.test("offFloatStep counts from the phase", () => {
  const shifted: FloatLattice = [1, 0.5, 0.5, 0, 0.25, 0];
  assertEquals(offFloatStep(0.25, false, shifted), false);
  assertEquals(offFloatStep(-0.25, false, shifted), false);
  assertEquals(offFloatStep(0.5, false, shifted), true);
});

Deno.test("offFloatPoints accepts a point within a few units of rounding", () => {
  assertEquals(offFloatPoints(1e300, false, [1e300, -1e300]), false);
  assertEquals(offFloatPoints(1e300 * (1 + 2 ** -51), false, [1e300]), false);
  assertEquals(offFloatPoints(1e300 * (1 + 2 ** -48), false, [1e300]), true);
  assertEquals(offFloatPoints(0, false, [0]), false);
});

Deno.test("offFloatPoints refuses every value when it has no point", () => {
  assertEquals(offFloatPoints(0, false, []), true);
  assertEquals(offFloatPoints(NaN, false, [0]), true);
  assertEquals(offFloatPoints(Infinity, true, [0]), true);
});
