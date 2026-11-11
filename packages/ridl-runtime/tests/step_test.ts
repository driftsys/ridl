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

Deno.test("offFloatStep accepts any large value when the step is finer than its rounding cell", () => {
  const fine: FloatLattice = [1, 1e-3, 1e-3, 0, 0, 0];
  assertEquals(offFloatStep(1e300, false, fine), false);
  assertEquals(offFloatStep(1e30, true, fine), false);
});

// The cases below sit at the ends of the binary64 and binary32 ranges. Each
// expected value follows from the definition of the lattice, not from running
// the check: a lattice point is `phase + k * step`, and a float of magnitude
// 2^53 or more is an integer.

Deno.test("offFloatStep accepts the largest finite floats on an integer lattice", () => {
  const MAX_32 = 3.4028234663852886e38;
  for (const value of [Number.MAX_VALUE, -Number.MAX_VALUE]) {
    assertEquals(offFloatStep(value, false, half), false, `${value}`);
  }
  for (const value of [MAX_32, -MAX_32]) {
    assertEquals(offFloatStep(value, true, half), false, `${value}`);
  }
});

Deno.test("offFloatStep takes the upper gap of the largest float from the lower gap", () => {
  // The next float above the largest one is infinite. With a step of 1.6e308,
  // which is above the rounding cell of the largest float, the largest float
  // is not a point of the lattice: the nearest point is 1.6e308.
  const step = 1.6e308;
  const huge: FloatLattice = [1, step, step, 0, 0, 0];
  assertEquals(offFloatStep(Number.MAX_VALUE, false, huge), true);
  assertEquals(offFloatStep(step, false, huge), false);
});

Deno.test("offFloatStep accepts a point whose distance to the phase overflows", () => {
  // The lattice is 2^1022 + k * 2^1023. The point for k = -2 is -1.5 * 2^1023,
  // which is 2^1024 below the phase. binary64 cannot hold that distance, nor
  // the double of the step. The point is on the lattice by its definition.
  const step = 2 ** 1023;
  const phase = 2 ** 1022;
  const lattice: FloatLattice = [1, step, step, 0, phase, 0];
  const point = phase - step - step;
  assertEquals(Number.isFinite(point), true);
  assertEquals(Number.isFinite(point - phase), false);
  assertEquals(offFloatStep(point, false, lattice), false);
  assertEquals(offFloatStep(phase + step, false, lattice), false);
  // A neighbour of the point, a quarter of a step away, is off the lattice.
  assertEquals(offFloatStep(point + step / 4, false, lattice), true);
});
