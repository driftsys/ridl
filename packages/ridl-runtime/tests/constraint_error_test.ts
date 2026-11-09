import { assertEquals, assertInstanceOf } from "@std/assert";
import { ConstraintError } from "../mod.ts";

Deno.test("ConstraintError reports its type, rule and numeric value", () => {
  const error = new ConstraintError(
    "range",
    "veh.cabin.Temperature",
    90,
    "[-40..85]",
  );
  assertInstanceOf(error, RangeError);
  assertEquals(error.name, "ConstraintError");
  assertEquals(error.type, "veh.cabin.Temperature");
  assertEquals(error.rule, "range");
  assertEquals(
    error.message,
    "veh.cabin.Temperature: 90 breaks the range rule [-40..85]",
  );
});

Deno.test("ConstraintError formats primitive values with String", () => {
  for (const value of [12n, NaN, Infinity, false]) {
    assertEquals(
      new ConstraintError("step", "test.Value", value, "2").message,
      `test.Value: ${String(value)} breaks the step rule 2`,
    );
  }
});

Deno.test("ConstraintError truncates strings after 64 code points", () => {
  const limit = "😀".repeat(64);
  for (
    const [value, expected] of [[limit, limit], [limit + "x", limit + "…"]]
  ) {
    assertEquals(
      new ConstraintError("pattern", "test.Text", value, "/x/").message,
      `test.Text: ${expected} breaks the pattern rule /x/`,
    );
  }
});

Deno.test("ConstraintError reports only the byte length", () => {
  assertEquals(
    new ConstraintError(
      "length",
      "test.Bytes",
      new Uint8Array([1, 2, 3]),
      "[0..2]",
    ).message,
    "test.Bytes: <3 bytes> breaks the length rule [0..2]",
  );
});
