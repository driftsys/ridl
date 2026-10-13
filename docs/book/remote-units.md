# Remote units and `ridl vendor`

A package does not have to live in your repository. This chapter shows how to
import a package from a URL, where the compiler reads it from, what
`ridl.lock` pins, how `ridl vendor` copies it into your repository, and how to
build with no network. The [CLI reference](cli-reference.md#ridl-check) lists
what each command writes, and [ADR-0002][adr-0002] records the rules; this
chapter is the guide.

The examples use TOML manifests and shell sessions. A remote import needs a
network, so no block here is compiled, and the URLs are placeholders.

## Import a package from a URL

The `[imports]` table of a manifest names each remote package. The key is the
package name that the remote package declares in its own `ridl.toml`; the value
is the URL of a tar archive that holds the package:

```toml
[package]
name = "veh.cluster"

[imports]
"veh.common" = "https://registry.example.com/veh/common@v1.0.0"
```

A package that is a member of a workspace puts its imports in the root manifest
or in its own manifest. The compiler takes the union of both.

In a source file, import from the remote package as from any other package:

```ridl,ignore
import veh.common.Speed
```

The import resolves by the longest dot-separated prefix that is an `[imports]`
key, so `veh.common.Speed` finds the key `veh.common`. A remote package exposes
the same declarations to you as a package in your repository does, except
`internal` ones.

A remote package, called a remote unit in the diagnostics, follows these rules:

- Its archive holds one package at the top level: a `ridl.toml` with a
  `[package]` table, not a `[workspace]` table, whose `name` equals the
  `[imports]` key. Anything else is `MANI-105`.
- Your `[imports]` lists every remote package the build needs, including the
  ones that your remote packages import. The compiler does not fetch the
  imports of a remote unit. It reads them only to check that you list each key,
  and a key you do not list is `MANI-106`, with a message that names the URL
  the remote unit gave. When the two URLs differ, yours is used.
- A remote unit is a read-only dependency. Its errors fail the command, its
  warnings and infos are not shown, and your `[lints]` table does not apply to
  it. `ridl build` generates code for it beside your own packages, and
  `ridl fmt` never changes it. No catalog, baseline snapshot or `ridl lock`
  entry includes it; its owner publishes those.

## Where the compiler reads a remote unit from

For each URL, the compiler takes the first source that applies:

1. `vendor/<key>/` at the manifest root, when that directory exists and
   `ridl.lock` pins a `tree` for the URL.
2. The cache, `~/.ridl/cache`, for the hash that `ridl.lock` pins.
3. A fetch from the URL, which stores the archive in the cache. Only commands
   that may fetch do this, and never under `--frozen`.

When `ridl.lock` pins no `tree` for the URL, the compiler reads the cache or
fetches first, so that a pin exists. It then verifies a vendored directory, if
there is one, against that pin and reads the directory in place of the cache
entry.

Which commands fetch and which only read matters:

| Commands                                                      | Read from `vendor/`, then the cache | Fetch on a miss | Write `ridl.lock` |
| ------------------------------------------------------------- | ----------------------------------- | --------------- | ----------------- |
| `ridl check`, `ridl build`                                    | yes                                 | yes             | yes               |
| `ridl check --frozen`, `ridl build --frozen`                  | yes                                 | no              | no                |
| `ridl baseline`                                               | yes                                 | yes             | yes               |
| `ridl vendor`                                                 | cache only, never `vendor/`         | yes             | yes               |
| `ridl vendor --check`                                         | `vendor/` only                      | no              | no                |
| `ridl test`, `ridl lock`, `ridl diff`, `ridl lsp`, `ridl mcp` | yes                                 | no              | no                |

The [`ridl check`](cli-reference.md#ridl-check) section of the CLI reference
says when a run writes `ridl.lock`.

## What `ridl.lock` pins

`ridl.lock` sits at the manifest root, beside `ridl.toml`. Each remote import
has one entry, keyed by its URL:

```toml
[entries."https://registry.example.com/veh/common@v1.0.0"]
sha256 = "<the SHA-256 of the fetched archive>"
tree = "<the hash of the unpacked tree>"
```

- `sha256` pins the bytes of the downloaded archive. A fetch whose content
  hashes to another value is `MANI-102`, and the import is dropped.
- `tree` pins the unpacked tree: a SHA-256 over every regular file in it, with
  its path and length. It is what the compiler checks on a cache entry and on a
  vendored directory. A tree that does not match is `MANI-107`. A tree that
  holds a symbolic link or another file that is not a regular file is also
  `MANI-107`, and the message names the file.

Commit `ridl.lock`. It is generated: do not edit it by hand. A `ridl.lock`
written before the `tree` hash existed has no `tree`. A run without `--frozen`
adds it. A run with `--frozen`, and `ridl vendor --check`, report `MANI-108`
instead, because they never write.

## Copy the units into the repository with `ridl vendor`

Run the command from any directory of the project:

```console
$ ridl vendor
vendored veh.common
```

The command acts on the manifest root above the directory, so `vendor/` is at
the workspace root whichever member you run it from. It copies each remote unit
of your `[imports]` to `vendor/<key>/` and updates `ridl.lock`. It also removes
the directories in `vendor/` that no `[imports]` key names. The
[`ridl vendor`](cli-reference.md#ridl-vendor) section of the CLI reference gives
the exact rules for what the command writes, removes and leaves alone.

Commit `vendor/` together with `ridl.lock`. The `tree` hash covers the bytes of
every file, so keep Git from converting line endings in `vendor/`; the CLI
reference gives the `.gitattributes` line.

After a change of URL in `[imports]`, run `ridl vendor` again to replace the
directory.

### Check the directory in CI

`ridl vendor --check` compares `vendor/` with the imports and with `ridl.lock`.
It fetches nothing and writes nothing, so it suits CI:

```console
$ ridl vendor --check
```

It exits 0 when `vendor/` matches. A failure reports `MANI-109`, `MANI-110`,
`MANI-107`, `MANI-103` or `MANI-108`; the
[CLI reference](cli-reference.md#ridl-vendor) gives the meaning of each and the
exit codes. In each case, run `ridl vendor` and commit the result.

## Build with no network

An air-gapped build needs the units in the repository. On a machine with a
network:

```console
$ ridl vendor
$ git add ridl.lock vendor
$ git commit -m "chore(deps): vendor the remote units"
```

On the machine with no network, and no cache:

```console
$ ridl check --frozen
```

Under `--frozen`, the compiler reads `vendor/<key>/` first, verifies it against
the `tree` in `ridl.lock`, and fetches nothing. The same holds for
`ridl build --frozen`.

A project that does not vendor can still build offline, as long as the cache
holds every pin: `ridl check --frozen` reports `MANI-103` for a URL with no
`ridl.lock` entry and `MANI-104` for a pinned unit that is not in the cache. The
message of `MANI-104` names `ridl check` and `ridl vendor` as the fix.

## Fresh clones

`ridl test`, `ridl lock`, `ridl diff`, `ridl lsp` and `ridl mcp` never fetch and
never write `ridl.lock`. In a fresh clone of a project that imports remote
units and does not commit `vendor/`, the cache is empty, so run `ridl check`
once first. It fetches the units into the cache and writes `ridl.lock`.
Otherwise these commands report `MANI-103` or `MANI-104`. A project that
commits `vendor/` and `ridl.lock` needs no such step.

## The `vendor/` directory is reserved

`vendor/` at the manifest root belongs to `ridl vendor`. The loader does not
read source files there as packages of your project:

- In a workspace, a `members` entry that is `vendor` or a path under it is
  `MANI-017`, an error, and the entry is not loaded.
- In a standalone package, a `.typl`, `.ridl` or `.rsdl` file under `vendor/`
  that is not inside a directory named by an `[imports]` key is `MANI-018`, a
  warning with the lint name `vendor-source-not-loaded`, and is not loaded.
  One warning is reported per file.
- The name is reserved at the manifest root only. A directory named `vendor`
  deeper in a package is an ordinary source package. A remote unit's own
  `vendor/` directory is part of its tree and its hash, and the compiler skips
  it when it reads the unit.

This is a breaking change for a project that kept source packages in a
`vendor/` directory at its root. Those packages stop loading. Move them to
another directory, and update the `members` list of the workspace or the
imports that name them.

`ridl fmt` also skips `vendor/` at every manifest root, so it does not rewrite
the units that belong to other owners.

## Where to go next

- [Getting started](getting-started.md#a-workspace) shows the
  workspace that a remote import joins.
- The [`ridl check`](cli-reference.md#ridl-check) and
  [`ridl vendor`](cli-reference.md#ridl-vendor) sections of the CLI reference
  list the exit codes.
- The [lints](lints.md) page lists `vendor-source-not-loaded`.
- [ADR-0002][adr-0002] records the module system, the lockfile and the vendor
  directory.

[adr-0002]: https://github.com/driftsys/ridl/blob/main/docs/decisions/ADR-0002-module-system.md
