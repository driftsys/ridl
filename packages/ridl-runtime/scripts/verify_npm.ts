// Builds the npm package and checks what a consumer would receive.
//
// Usage: deno run -A scripts/verify_npm.ts
//
// It builds twice, once for a tag-style version and once for the version in
// deno.json, checks the built manifest and the files `npm pack` would publish,
// then installs the packed tarball into a temporary project and imports the
// package by name from Node, so the `exports` map and the `types` entry are
// the ones read.

import { assert, assertEquals } from "@std/assert";

const denoJson = JSON.parse(await Deno.readTextFile("deno.json"));

async function run(
  cmd: string,
  args: string[],
  cwd?: string,
): Promise<{ code: number; stdout: string; stderr: string }> {
  const out = await new Deno.Command(cmd, {
    args,
    cwd,
    stdout: "piped",
    stderr: "piped",
  })
    .output();
  return {
    code: out.code,
    stdout: new TextDecoder().decode(out.stdout),
    stderr: new TextDecoder().decode(out.stderr),
  };
}

async function build(version?: string) {
  const args = ["run", "-A", "scripts/build_npm.ts"];
  if (version !== undefined) args.push(version);
  return await run(Deno.execPath(), args);
}

const manifest = async () =>
  JSON.parse(await Deno.readTextFile("npm/package.json"));

// A tag version is read with its `v` removed, and a non-version is refused.
const tagged = await build("v9.9.9");
assertEquals(tagged.code, 0, tagged.stderr);
assertEquals((await manifest()).version, "9.9.9");
assert(
  (await build("vfoo")).code !== 0,
  "a non-semver tag must fail the build",
);

const built = await build();
assertEquals(built.code, 0, built.stderr);
const pkg = await manifest();
assertEquals(pkg.name, "@driftsys/ridl-runtime");
assertEquals(pkg.version, denoJson.version);
assertEquals(pkg.license, "MIT");
assertEquals(pkg.sideEffects, false);
assertEquals(pkg.engines, { node: ">=22" });
// The package has no dependency at run time or at build time.
assertEquals(pkg.dependencies, undefined);
assertEquals(pkg.devDependencies, undefined);
assertEquals(pkg.types, "./esm/mod.d.ts");
const entry = pkg.exports["."];
for (const key of ["types", "import", "default"]) {
  assert(entry[key], `exports["."].${key} is missing`);
  await Deno.stat(`npm/${entry[key]}`);
}

// The files `npm pack` would publish.
const dry = await run("npm", ["pack", "--dry-run", "--json"], "npm");
assertEquals(dry.code, 0, dry.stderr);
const listed: string[] = JSON.parse(dry.stdout)[0].files.map(
  (f: { path: string }) => f.path,
);
for (
  const required of [
    "LICENSE",
    "README.md",
    "package.json",
    "esm/mod.js",
    "esm/mod.d.ts",
    "esm/src/codec.js",
    "esm/src/codec.d.ts",
    "esm/src/errors.js",
    "esm/src/errors.d.ts",
  ]
) {
  assert(listed.includes(required), `${required} is not in the package`);
}

for (const path of listed) {
  assert(
    !/(^|\/)(tests?|fixtures?)(\/|\.|_)/.test(path) &&
      !path.endsWith("_test.js"),
    `${path} must not be in the package`,
  );
}

// Install the packed tarball and import it by name from Node.
const tmp = await Deno.makeTempDir({ prefix: "ridl-npm-" });
try {
  const pack = await run(
    "npm",
    ["pack", "--json", "--pack-destination", tmp],
    "npm",
  );
  assertEquals(pack.code, 0, pack.stderr);
  const tarball = `${tmp}/${JSON.parse(pack.stdout)[0].filename}`;
  await Deno.writeTextFile(
    `${tmp}/package.json`,
    '{"name":"consumer","private":true,"type":"module"}',
  );
  const install = await run(
    "npm",
    ["install", "--no-audit", "--no-fund", tarball],
    tmp,
  );
  assertEquals(install.code, 0, install.stderr);
  const names = await run(
    "node",
    [
      "--input-type=module",
      "-e",
      "const m = await import('@driftsys/ridl-runtime');" +
      "console.log(JSON.stringify(Object.keys(m).sort()));",
    ],
    tmp,
  );
  assertEquals(names.code, 0, names.stderr);
  // The names are written out, so a change to the public API is a change here.
  assertEquals(JSON.parse(names.stdout), [
    "CatalogMismatch",
    "CodecError",
    "UnknownPayloadType",
    "loadCodec",
  ]);

  // A TypeScript consumer without the DOM library must be able to read the
  // declarations. The versions are pinned so the check does not drift.
  const tools = await run(
    "npm",
    [
      "install",
      "--no-audit",
      "--no-fund",
      "--save-dev",
      "typescript@5.9.3",
      "@types/node@22.20.5",
    ],
    tmp,
  );
  assertEquals(tools.code, 0, tools.stderr);
  await Deno.writeTextFile(
    `${tmp}/consumer.ts`,
    `import { CatalogMismatch, CodecError, loadCodec } from "@driftsys/ridl-runtime";
import type { Codec, VerifyResult } from "@driftsys/ridl-runtime";
export async function use(bytes: Uint8Array): Promise<VerifyResult> {
  const codec: Codec = await loadCodec(bytes, {
    catalogHash: new Uint8Array(32),
    ruleText: ["rule"],
    imports: { env: { f: () => 0 } },
  });
  void CatalogMismatch;
  void CodecError;
  return codec.verify(0, bytes);
}
`,
  );
  const tsc = await run(
    `${tmp}/node_modules/.bin/tsc`,
    [
      "--noEmit",
      "--strict",
      "--lib",
      "es2022",
      "--types",
      "node",
      "--module",
      "nodenext",
      "--moduleResolution",
      "nodenext",
      "--target",
      "es2022",
      "consumer.ts",
    ],
    tmp,
  );
  assertEquals(tsc.code, 0, tsc.stdout + tsc.stderr);
  await Deno.stat(
    `${tmp}/node_modules/@driftsys/ridl-runtime/esm/mod.d.ts`,
  );
} finally {
  await Deno.remove(tmp, { recursive: true });
}
console.log("verify_npm: the npm package is as expected");
