import { strict as assert } from "node:assert";
import * as fs from "node:fs";
import * as path from "node:path";
import { test } from "node:test";

// The compiled test runs from out/, so the extension root is one level up.
const ROOT = path.join(__dirname, "..");

function readJson(relative: string): any {
  return JSON.parse(fs.readFileSync(path.join(ROOT, relative), "utf8"));
}

// rsdl reference §2: the eight rsdl keywords, and the rsdl v0.1 words it retires.
const RSDL_KEYWORDS = ["system", "component", "distribution", "deployment", "machine", "offers", "requires", "for"];
const RETIRED_WORDS = [
  "provides",
  "instance",
  "assurance",
  "target",
  "place",
  "on",
  "transport",
  "bundle",
  "time",
  "base",
  "redundant",
  "supervise",
  "degraded",
];

test("the manifest contributes the rsdl language with its grammar", () => {
  const manifest = readJson("package.json");
  const language = manifest.contributes.languages.find((entry: { id: string }) => entry.id === "rsdl");
  assert.ok(language, "an rsdl language entry");
  assert.deepEqual(language.extensions, [".rsdl"]);
  const grammar = manifest.contributes.grammars.find((entry: { language: string }) => entry.language === "rsdl");
  assert.ok(grammar, "an rsdl grammar entry");
  assert.equal(grammar.scopeName, "source.rsdl");
  assert.equal(readJson(grammar.path).scopeName, "source.rsdl");
});

test("the rsdl grammar scopes the keys of an attribute block", () => {
  const grammar = readJson("syntaxes/rsdl.tmLanguage.json");
  const includes = (patterns: { include?: string }[]) => patterns.map((pattern) => pattern.include);
  assert.ok(includes(grammar.patterns).includes("#attributes"), "the file includes the attribute block");
  const attributes = grammar.repository.attributes;
  assert.deepEqual([attributes.begin, attributes.end], ["\\[", "\\]"]);
  assert.ok(includes(attributes.patterns).includes("#backend-key"), "a block holds backend keys");
  assert.ok(includes(attributes.patterns).includes("#owned-key"), "a block holds rsdl-owned keys");
  assert.ok(new RegExp(grammar.repository["backend-key"].match).test("linux.cpuset"), "`linux.cpuset`");
  const owned = new RegExp(`^(?:${grammar.repository["owned-key"].match})$`);
  for (const key of ["instances", "external", "tier", "labels", "deprecated"]) {
    assert.ok(owned.test(key), `\`${key}\` is an rsdl-owned key`);
  }
});

test("the rsdl grammar highlights the rsdl keywords and no retired word", () => {
  const grammar = readJson("syntaxes/rsdl.tmLanguage.json");
  const keywordRules = Object.entries(grammar.repository)
    .filter(([name]) => name.startsWith("keywords-"))
    .map(([, rule]) => new RegExp(`^(?:${(rule as { match: string }).match})$`));
  const highlighted = (word: string) => keywordRules.some((rule) => rule.test(word));
  for (const word of RSDL_KEYWORDS) {
    assert.ok(highlighted(word), `\`${word}\` is highlighted as a keyword`);
  }
  for (const word of RETIRED_WORDS) {
    assert.ok(!highlighted(word), `\`${word}\` is not highlighted as a keyword`);
  }
});
