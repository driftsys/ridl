import assert from "node:assert/strict";
import { ConstraintError } from "@driftsys/ridl-runtime";
import {
  makeBlob,
  makeCapitals,
  makeContains,
  makeDot,
  makeFree,
  makeFreeInteger,
  makeLower,
  makeLowerReal,
  makeMaximum,
  makeNarrowReal,
  makeNegative,
  makePrecise,
  makeQuoted,
  makeRaw,
  makeReal,
  makeShifted,
  makeShort,
  makeSmall,
  makeSmallUnchecked,
  makeTenth,
  makeTriple,
  makeUnbounded,
  makeUpper,
  makeUpperReal,
  makeWide,
  makeWord,
  makeWordUnchecked,
} from "./ctor.ts";
import {
  makeEndless,
  makeFromZero,
  makeHugeStep,
  makePatterned,
  makeSafeNumber,
  makeStepped,
  makeUnbounded32,
  makeWideFromZero,
  makeWideStepped,
} from "./number.ts";

function refuses(
  type: string,
  build: () => unknown,
  rule: ConstraintError["rule"] = "range",
) {
  assert.throws(build, (error: unknown) => {
    assert(error instanceof ConstraintError);
    assert.equal(error.rule, rule);
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

Deno.test("an integer step counts from the minimum", () => {
  for (const value of [-10, -5, 0, 5, 10, 15, 20]) {
    assert.equal(makeStepped(value), value);
  }
  for (const value of [-9, -6, 1, 4, 19]) {
    refuses("Stepped", () => makeStepped(value), "step");
  }
});

Deno.test("an integer step message names the step and the origin", () => {
  assert.throws(
    () => makeStepped(1),
    { message: "ctor.Stepped: 1 breaks the step rule step 5 from -10" },
  );
});

Deno.test("the range is checked before the step", () => {
  // 25 is on the grid and 21 is not; both are out of range.
  refuses("Stepped", () => makeStepped(25));
  refuses("Stepped", () => makeStepped(21));
  refuses("Stepped", () => makeStepped(-15));
  refuses("Stepped", () => makeStepped(1.5));
});

Deno.test("an integer step with no minimum counts from zero", () => {
  for (const value of [-100, -5, 0, 5, 20]) {
    assert.equal(makeFromZero(value), value);
  }
  for (const value of [-7, -3, 3, 19]) {
    refuses("FromZero", () => makeFromZero(value), "step");
  }
});

Deno.test("a step larger than the range admits only the minimum", () => {
  assert.equal(makeHugeStep(-3), -3);
  for (const value of [-2, 0, 100]) {
    refuses("HugeStep", () => makeHugeStep(value), "step");
  }
});

Deno.test("a bigint step is exact beyond the safe number interval", () => {
  const min = -9007199254740993n;
  for (const value of [min, min + 4n, -1n, 3n, 9007199254740991n]) {
    assert.equal(makeWideStepped(value), value);
  }
  for (const value of [min + 1n, min + 2n, 0n, 1n, 9007199254740993n]) {
    refuses("WideStepped", () => makeWideStepped(value), "step");
  }
  refuses("WideStepped", () => makeWideStepped(min - 4n));
});

Deno.test("an unsigned bigint step counts from zero", () => {
  for (const value of [0n, 3n, 9223372036854775806n]) {
    assert.equal(makeWideFromZero(value), value);
  }
  for (const value of [1n, 4n, 9223372036854775807n]) {
    refuses("WideFromZero", () => makeWideFromZero(value), "step");
  }
});

Deno.test("a 32-bit float step accepts the rounded decimal grid", () => {
  for (const value of [0, 0.1, 0.2, 0.3, 0.1 + 0.2, 0.5, 0.1 * 7, 1]) {
    assert.equal(makeTenth(value), value);
  }
  for (const value of [0.05, 0.15, 0.25, 0.35, 0.99]) {
    refuses("Tenth", () => makeTenth(value), "step");
  }
});

Deno.test("a 32-bit float is projected before the step is checked", () => {
  // The offset is below the 32-bit resolution at 0.3, so the value rounds to
  // the grid point. A larger offset is a different 32-bit value.
  assert.equal(makeTenth(0.3 + 1e-9), 0.3 + 1e-9);
  refuses("Tenth", () => makeTenth(0.3 + 1e-6), "step");
});

Deno.test("a float step counts from a minimum between two multiples", () => {
  for (const value of [0.05, 0.15, 0.25, 0.05 + 0.1 * 3, 1.05]) {
    assert.equal(makeShifted(value), value);
  }
  for (const value of [0.1, 0.2, 1]) {
    refuses("Shifted", () => makeShifted(value), "step");
  }
});

Deno.test("a 64-bit float step tolerates only a few units of rounding", () => {
  for (const value of [-1, -1 + 1e-10, 0, 0.5, 1, 0.1 + 2e-17]) {
    assert.equal(makePrecise(value), value);
  }
  for (const value of [5e-11, 1e-11, 0.1 + 1e-16]) {
    refuses("Precise", () => makePrecise(value), "step");
  }
});

Deno.test("an integer step stays exact near the safe limit", () => {
  // The exact differences are 9007199254741001 (off the grid) and
  // 9007199254740995 (on it); a rounded subtraction swaps the two.
  assert.equal(makeUnbounded32(9007199254740985), 9007199254740985);
  refuses("Unbounded32", () => makeUnbounded32(9007199254740991), "step");
});

Deno.test("a float grid counts from a negative minimum between two multiples", () => {
  for (const value of [-1.05, -0.95, -0.05, 0.05, 0.95]) {
    assert.equal(makeNegative(value), value);
  }
  for (const value of [-1, -0.9, 0, 0.1, 1]) {
    refuses("Negative", () => makeNegative(value), "step");
  }
});

Deno.test("string length bounds are inclusive and count code points", () => {
  assert.equal(makeWord("a"), "a");
  assert.equal(makeWord("abcd"), "abcd");
  refuses("Word", () => makeWord(""), "length");
  refuses("Word", () => makeWord("abcde"), "length");
});

Deno.test("string length counts a multi-byte character once", () => {
  // Two bytes in UTF-8, one UTF-16 unit.
  assert.equal(makeWord("éééé"), "éééé");
  refuses("Word", () => makeWord("ééééé"), "length");
  // Three bytes in UTF-8, one UTF-16 unit.
  assert.equal(makeWord("€€€€"), "€€€€");
});

Deno.test("string length counts an astral code point once, not two units", () => {
  // Four code points, eight UTF-16 units.
  assert.equal("😀😀😀😀".length, 8);
  assert.equal(makeWord("😀😀😀😀"), "😀😀😀😀");
  refuses("Word", () => makeWord("😀😀😀😀😀"), "length");
  // The minimum counts code points too: three of them in six units.
  assert.equal(makeTriple("😀😀😀"), "😀😀😀");
  refuses("Triple", () => makeTriple("😀😀"), "length");
  refuses("Triple", () => makeTriple("😀😀😀😀"), "length");
});

Deno.test("string length counts a lone surrogate as one character", () => {
  assert.equal(makeTriple("\ud83dab"), "\ud83dab");
  refuses("Triple", () => makeTriple("\ud83dabc"), "length");
});

Deno.test("an empty string passes a zero minimum and fails a positive one", () => {
  assert.equal(makeShort(""), "");
  assert.equal(makeFree(""), "");
  assert.equal(makeFree("a".repeat(256)).length, 256);
  refuses("Free", () => makeFree("a".repeat(257)), "length");
  refuses("Word", () => makeWord(""), "length");
  refuses("Triple", () => makeTriple(""), "length");
});

Deno.test("a fixed string length refuses both neighbours", () => {
  assert.equal(makeTriple("abc"), "abc");
  refuses("Triple", () => makeTriple("ab"), "length");
  refuses("Triple", () => makeTriple("abcd"), "length");
});

Deno.test("bytes length counts bytes and is inclusive", () => {
  assert.equal(makeBlob(new Uint8Array(2)).length, 2);
  assert.equal(makeBlob(new Uint8Array(4)).length, 4);
  refuses("Blob", () => makeBlob(new Uint8Array(1)), "length");
  refuses("Blob", () => makeBlob(new Uint8Array(5)), "length");
  assert.equal(makeRaw(new Uint8Array(0)).length, 0);
  assert.equal(makeRaw(new Uint8Array(8)).length, 8);
  refuses("Raw", () => makeRaw(new Uint8Array(9)), "length");
});

Deno.test("a pattern with anchors matches the whole value", () => {
  assert.equal(makeCapitals("ABC"), "ABC");
  refuses("Capitals", () => makeCapitals("ABc"), "pattern");
  refuses("Capitals", () => makeCapitals("1ABC"), "pattern");
  refuses("Capitals", () => makeCapitals("ABC1"), "pattern");
});

Deno.test("an anchored pattern refuses a trailing newline", () => {
  // `$` without the multiline flag matches only at the end of the input.
  refuses("Capitals", () => makeCapitals("ABC\n"), "pattern");
  refuses("Capitals", () => makeCapitals("AB\nC"), "pattern");
});

Deno.test("an unanchored pattern accepts a match anywhere", () => {
  assert.equal(makeContains("xxabxx"), "xxabxx");
  assert.equal(makeContains("ab"), "ab");
  refuses("Contains", () => makeContains("ba"), "pattern");
  refuses("Contains", () => makeContains(""), "pattern");
});

Deno.test("a pattern runs without the unicode flag", () => {
  // The dot matches one UTF-16 unit, the semantics of the checker. With the
  // `u` flag it would match the whole astral character.
  assert.equal(makeDot("a"), "a");
  assert.equal(makeDot("é"), "é");
  refuses("Dot", () => makeDot("😀"), "pattern");
  // The dot does not match a line break.
  refuses("Dot", () => makeDot("\n"), "pattern");
});

Deno.test("a pattern checks the same value on every call", () => {
  // A `RegExp` with the `g` or `y` flag keeps a position between calls.
  for (let i = 0; i < 3; i++) {
    assert.equal(makeCapitals("ABC"), "ABC");
    assert.equal(makeContains("ab"), "ab");
  }
});

Deno.test("a pattern with an escape and a quote is emitted literally", () => {
  assert.equal(makeQuoted("12'x"), "12'x");
  refuses("Quoted", () => makeQuoted("1a'x"), "pattern");
  refuses("Quoted", () => makeQuoted("12x"), "pattern");
});

Deno.test("length is checked before the pattern", () => {
  // Nine capitals match the pattern and break the length bound.
  refuses("Capitals", () => makeCapitals("ABCDEFGHI"), "length");
  refuses("Capitals", () => makeCapitals(""), "length");
  // A length-valid value that breaks the pattern is refused for the pattern.
  refuses("Capitals", () => makeCapitals("abc"), "pattern");
});

Deno.test("the error names the length and pattern constraints", () => {
  assert.throws(() => makeWord("abcde"), {
    message: "ctor.Word: abcde breaks the length rule [1..4]",
  });
  assert.throws(() => makeCapitals("abc"), {
    message: "ctor.Capitals: abc breaks the pattern rule /^[A-Z]+$/",
  });
  assert.throws(() => makeBlob(new Uint8Array(5)), {
    message: "ctor.Blob: <5 bytes> breaks the length rule [2..4]",
  });
});

Deno.test("an unchecked string constructor returns the value unchanged", () => {
  assert.equal(makeWordUnchecked("abcdefgh"), "abcdefgh");
  assert.equal(makeWordUnchecked(""), "");
});

Deno.test("a length maximum beyond the safe numbers is never exceeded", () => {
  assert.equal(makeEndless("a"), "a");
  refuses("Endless", () => makeEndless(""), "length");
});

Deno.test("a pattern on bytes is ignored", () => {
  assert.equal(makePatterned(new Uint8Array(2)).length, 2);
  refuses("Patterned", () => makePatterned(new Uint8Array(3)), "length");
});
