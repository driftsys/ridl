import assert from "node:assert/strict";

Deno.test("the package version is the workspace version", async () => {
  const cargo = await Deno.readTextFile(
    new URL("../../../Cargo.toml", import.meta.url),
  );
  const section = cargo.split(/^\[workspace\.package\]\s*$/m)[1] ?? "";
  const match = section.match(/^version\s*=\s*"([^"]+)"/m);
  assert.ok(match, "no version under [workspace.package]");
  const config = JSON.parse(
    await Deno.readTextFile(new URL("../deno.json", import.meta.url)),
  );
  assert.equal(config.version, match[1]);
});
