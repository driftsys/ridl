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
assertEquals(pkg.name, "@driftsys/ridl-transport-ws");
assertEquals(pkg.version, denoJson.version);
assertEquals(pkg.license, "MIT");
assertEquals(pkg.sideEffects, false);
assertEquals(pkg.engines, { node: ">=22" });
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
    "esm/src/frame.js",
    "esm/src/frame.d.ts",
    "esm/src/errors.d.ts",
    "esm/src/session.d.ts",
    "esm/src/calls.d.ts",
  ]
) {
  assert(listed.includes(required), `${required} is not in the package`);
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
      "const m = await import('@driftsys/ridl-transport-ws');" +
      "console.log(JSON.stringify(Object.keys(m).sort()));",
    ],
    tmp,
  );
  assertEquals(names.code, 0, names.stderr);
  const expected = Object.keys(await import("../mod.ts")).sort();
  assertEquals(JSON.parse(names.stdout), expected);
  await Deno.stat(
    `${tmp}/node_modules/@driftsys/ridl-transport-ws/esm/mod.d.ts`,
  );
} finally {
  await Deno.remove(tmp, { recursive: true });
}
console.log("verify_npm: the npm package is as expected");
