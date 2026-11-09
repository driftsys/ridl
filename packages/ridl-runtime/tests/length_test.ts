import { assertEquals } from "@std/assert";
import { codePointLength } from "../mod.ts";

Deno.test("codePointLength counts a code point outside the BMP once", () => {
  assertEquals(codePointLength(""), 0);
  assertEquals(codePointLength("abc"), 3);
  assertEquals(codePointLength("é"), 1);
  assertEquals(codePointLength("😀"), 1);
  assertEquals("😀".length, 2);
  assertEquals(codePointLength("a😀b"), 3);
});

Deno.test("codePointLength counts an unpaired surrogate once", () => {
  assertEquals(codePointLength("\ud83d"), 1);
  assertEquals(codePointLength("\ude00\ud83d"), 2);
});
