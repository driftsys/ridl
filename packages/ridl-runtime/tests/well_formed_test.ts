import { assertEquals } from "@std/assert";
import { ConstraintError, isWellFormedString } from "../mod.ts";

Deno.test("isWellFormedString accepts text made of scalar values", () => {
  for (const value of ["", "abc", "é", "😀", "a😀b", "😀"]) {
    assertEquals(isWellFormedString(value), true, JSON.stringify(value));
  }
});

Deno.test("isWellFormedString refuses a lone surrogate in any position", () => {
  for (
    const value of [
      "\ud83d",
      "\ude00",
      "a\ud83d",
      "\ud83da",
      "\ude00\ud83d",
      "\ud83d😀",
      "x\ude00y",
    ]
  ) {
    assertEquals(isWellFormedString(value), false, JSON.stringify(value));
  }
});

Deno.test("isWellFormedString agrees with the code-unit scan when isWellFormed is absent", () => {
  const prototype = String.prototype as { isWellFormed?: () => boolean };
  const original = prototype.isWellFormed;
  try {
    delete prototype.isWellFormed;
    assertEquals(isWellFormedString("a😀b"), true);
    assertEquals(isWellFormedString("a\ud83d"), false);
    assertEquals(isWellFormedString("\ude00"), false);
    assertEquals(isWellFormedString("\ude00\ud83d"), false);
    assertEquals(isWellFormedString("\ud83d😀"), false);
  } finally {
    prototype.isWellFormed = original;
  }
});

Deno.test("ConstraintError accepts the utf8 rule", () => {
  const error = new ConstraintError(
    "utf8",
    "test.Text",
    "a\ud83d",
    "well-formed text",
  );
  assertEquals(error.rule, "utf8");
  assertEquals(
    error.message,
    "test.Text: a\ud83d breaks the utf8 rule well-formed text",
  );
});
