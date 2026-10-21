// Builds the npm package of @driftsys/ridl-runtime into ./npm with dnt.
//
// Usage: deno run -A scripts/build_npm.ts [version]
//
// The version defaults to the one in deno.json. A leading "v" is removed, so a
// release tag can be passed as it is.

import { build, emptyDir } from "@deno/dnt";

const denoJson = JSON.parse(await Deno.readTextFile("deno.json"));
const version = (Deno.args[0] ?? denoJson.version).replace(/^v/, "");
// A version that is not semver would build and fail only at `npm publish`.
if (!/^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/.test(version)) {
  console.error(`build_npm: "${version}" is not a semantic version`);
  Deno.exit(1);
}

await emptyDir("./npm");

await build({
  entryPoints: ["./mod.ts"],
  outDir: "./npm",
  // The package uses the platform WebAssembly only, so no shim is needed.
  shims: { deno: false },
  typeCheck: false,
  test: false,
  scriptModule: false,
  package: {
    name: denoJson.name,
    version,
    description:
      "Loader and payload verifier of the ridl wasm codec for Deno, Bun, Node and the browser",
    license: "MIT",
    repository: {
      type: "git",
      url: "git+https://github.com/driftsys/ridl.git",
      directory: "packages/ridl-runtime",
    },
    bugs: { url: "https://github.com/driftsys/ridl/issues" },
    homepage:
      "https://github.com/driftsys/ridl/tree/main/packages/ridl-runtime",
    keywords: ["ridl", "wasm", "codec"],
    engines: { node: ">=22" },
    sideEffects: false,
  },
  postBuild() {
    Deno.copyFileSync("LICENSE", "npm/LICENSE");
    Deno.copyFileSync("README.md", "npm/README.md");
    // dnt writes no `types` field, so a consumer would get no declarations.
    const path = "npm/package.json";
    const manifest = JSON.parse(Deno.readTextFileSync(path));
    manifest.types = "./esm/mod.d.ts";
    manifest.exports = {
      // `default` lets a CommonJS `require` reach the ESM entry on a Node
      // that can load ES modules from `require`.
      ".": {
        types: "./esm/mod.d.ts",
        import: "./esm/mod.js",
        default: "./esm/mod.js",
      },
    };
    Deno.writeTextFileSync(path, JSON.stringify(manifest, null, 2) + "\n");
  },
});
