import assert from "node:assert/strict";
import { ConstraintError } from "@driftsys/ridl-runtime";
import {
  makeFreeInteger,
  makeLower,
  makeLowerReal,
  makeMaximum,
  makeNarrowReal,
  makeReal,
  makeSmall,
  makeSmallUnchecked,
  makeUnbounded,
  makeUpper,
  makeUpperReal,
  makeWide,
} from "./ctor.ts";
import { makeSafeNumber } from "./number.ts";

function refuses(type: string, build: () => unknown) {
  assert.throws(build, (error: unknown) => {
    assert(error instanceof ConstraintError);
    assert.equal(error.rule, "range");
    assert.equal(error.type, `ctor.${type}`);
    return true;
  });
}

Deno.test("number integer bounds are inclusive", () => {
  assert.equal(makeSmall(-10), -10);
  assert.equal(makeSmall(20), 20);
  refuses("Small", () => makeSmall(-11));
  refuses("Small", () => makeSmall(21));
});

Deno.test("integer construction refuses non-integral and unsafe numbers", () => {
  for (const value of [NaN, Infinity, -Infinity, 1.5, 2 ** 53]) {
    refuses("Small", () => makeSmall(value));
  }
});

Deno.test("number integer construction checks safety without range comparisons", () => {
  assert.equal(makeSafeNumber(2 ** 53 - 1), 2 ** 53 - 1);
  refuses("SafeNumber", () => makeSafeNumber(2 ** 53));
});

Deno.test("open integer ranges check their declared bound", () => {
  assert.equal(makeLower(-10n), -10n);
  assert.equal(makeLower(100n), 100n);
  refuses("Lower", () => makeLower(-11n));
  assert.equal(makeUpper(20n), 20n);
  assert.equal(makeUpper(-100n), -100n);
  refuses("Upper", () => makeUpper(21n));
});

Deno.test("bigint bounds retain their exact values", () => {
  const bound = 9007199254740993n;
  assert.equal(makeWide(-bound), -bound);
  assert.equal(makeWide(bound), bound);
  refuses("Wide", () => makeWide(-bound - 1n));
  refuses("Wide", () => makeWide(bound + 1n));
  assert.equal(makeMaximum(0n), 0n);
  assert.equal(makeMaximum(9223372036854775807n), 9223372036854775807n);
  refuses("Maximum", () => makeMaximum(-1n));
});

Deno.test("bigint construction enforces the language domain", () => {
  const min = -9223372036854775808n;
  const max = 9223372036854775807n;
  assert.equal(makeFreeInteger(min), min);
  assert.equal(makeFreeInteger(max), max);
  assert.equal(makeLower(max), max);
  assert.equal(makeUpper(min), min);
  refuses("Maximum", () => makeMaximum(max + 1n));
  refuses("FreeInteger", () => makeFreeInteger(max + 1n));
  refuses("FreeInteger", () => makeFreeInteger(min - 1n));
  for (const value of [1n << 100n, -(1n << 100n)]) {
    refuses("FreeInteger", () => makeFreeInteger(value));
    refuses("Lower", () => makeLower(value));
    refuses("Upper", () => makeUpper(value));
  }
});

Deno.test("float bounds are inclusive and require finite values", () => {
  assert.equal(makeReal(-1.5), -1.5);
  assert.equal(makeReal(2.5), 2.5);
  refuses("Real", () => makeReal(-2.5));
  refuses("Real", () => makeReal(3.5));
  for (const value of [NaN, Infinity, -Infinity]) {
    refuses("Real", () => makeReal(value));
  }
});

Deno.test("open float ranges check only the declared bound and finiteness", () => {
  assert.equal(makeLowerReal(-1.5), -1.5);
  assert.equal(makeLowerReal(100), 100);
  refuses("LowerReal", () => makeLowerReal(-2.5));
  assert.equal(makeUpperReal(2.5), 2.5);
  assert.equal(makeUpperReal(-100), -100);
  refuses("UpperReal", () => makeUpperReal(3.5));
  for (const value of [NaN, Infinity, -Infinity]) {
    refuses("LowerReal", () => makeLowerReal(value));
    refuses("UpperReal", () => makeUpperReal(value));
  }
});

Deno.test("an unconstrained float is a brand conversion", () => {
  assert(Number.isNaN(makeUnbounded(NaN)));
  assert.equal(makeUnbounded(Infinity), Infinity);
});

Deno.test("unchecked construction preserves an out-of-range value", () => {
  assert.equal(makeSmallUnchecked(21), 21);
});

Deno.test("32-bit float bounds are inclusive and require finite values", () => {
  assert.equal(makeNarrowReal(-1), -1);
  assert.equal(makeNarrowReal(2), 2);
  refuses("NarrowReal", () => makeNarrowReal(-2));
  refuses("NarrowReal", () => makeNarrowReal(3));
  for (const value of [NaN, Infinity, -Infinity]) {
    refuses("NarrowReal", () => makeNarrowReal(value));
  }
});
