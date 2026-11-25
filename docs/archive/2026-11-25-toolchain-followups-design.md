# Toolchain follow-ups — design (#1477)

Status: working memory. Decisions were delegated to the agent by the owner
through the session goal ("sdd #1477"), as on #1476. Every decision the agent
made is marked **(call)**. Questions only the owner can answer are listed in
"Owner actions" at the end.

Trace: Satisfies driftsys/ridl#1477. Amends ADR-0036 decisions 6, 8, 9 and 10 in
place (gardening edits the record; no new ADR). Extends
`docs/design/toolchain-directory.md`, `docs/book/release-pin.md` and
`docs/technotes/toolchain-distribution.md`.

## Intent

#1476 made a root `ridl.toml` pin a release and gave
`ridl toolchain
install/list/which`. A terminal user who hits MANI-026 must
still copy a path from `ridl toolchain which` and run it by hand, and an agent
host running `ridl mcp` from the terminal is refused with no recovery. Downloads
are checked against a checksum from the same origin as the archive, so a
replaced release is not detected. This change closes those gaps and the review
leftovers of #1479.

## Goals

1. **Terminal hand-off.** A workspace command that would refuse with MANI-026
   runs a compatible installed release with the same arguments, working
   directory, environment, standard input, standard output and standard error,
   and exits with that release's exit code.
2. **`ridl mcp` hand-off** before it reads `initialize`.
3. **Opt-in terminal automatic install** with `RIDL_TOOLCHAIN_AUTO_INSTALL=1`.
4. **`ridl toolchain uninstall <VERSION>`.**
5. **Signed release archives**, verified by every `ridl toolchain install`
   (explicit, automatic in the terminal, and the editor's).
6. **Workspace Trust install prompt** in the VS Code extension.
7. The six smaller items listed under "Smaller items".

## Non-goals

- Hand-off for `ridl lsp` (the extension already starts the selected binary;
  other editors keep the refusal). Hand-off for `ridlc` (the versions directory
  holds only `ridl`). **(call)**
- Automatic install from `ridl mcp` outside VS Code. An MCP host has no
  Workspace Trust, so `ridl mcp` hands off to an installed release but never
  downloads, even with `RIDL_TOOLCHAIN_AUTO_INSTALL=1`. **(call, follows the
  issue)**
- Signature verification in `install.sh` and `install.ps1`. The script is
  fetched from the same repository as the archives, so verifying inside it adds
  no independent trust; they keep the checksum check. **(call)**
- A user configuration file. "User configuration" in the issue is met by the
  environment variable set in the user's shell profile; no new config file
  surface. **(call)**
- GitHub artifact attestations (see Approaches).
- A mirror or origin override, CI detection, plugin pins, automatic pin updates
  — unchanged from ADR-0036.

## Anti-goals

- A hand-off loop, or a hand-off that runs a release the pin does not accept.
- A hand-off after any workspace or output file was written.
- A workspace (a `ridl.toml` or `.vscode/settings.json`) that can start a
  download or choose the downloaded binary without a user-level opt-in.
- An unsigned pinnable release on the release train.

## Approaches

### A. Where the terminal hand-off happens

- **A1 — at the refusal site (chosen).** Every command that enforces the pin
  already stops on `ridl_core::is_release_refusal` before any write (main.rs
  `run_check`, the `finish_check` path, `lock.rs`, `property.rs`, `vendor.rs`,
  and the others the release-pin chapter lists). A shared helper,
  `handoff::refusal(&diagnostics) -> Option<ExitCode>`, is called at each site
  before the refusal is printed. It uses the pins that the refusal diagnostics
  carry, so it needs no second copy of each command's path-to-root mapping.
  Cost: one call per site, and a test per command that the call exists.
- **A2 — before dispatch.** Discover the root from the command's path arguments
  before running it. Rejected: it duplicates the argument-to-root rules of about
  ten commands (`diff` source versus snapshot, `fmt` over several roots,
  `describe` descriptor roots) and would drift from them.
- **A3 — a rustup-style proxy binary.** Rejected again for the same reason as in
  ADR-0036.

### B. How releases are signed

- **B1 — minisign (Ed25519) signatures with an embedded public key (chosen).**
  The release workflow signs each `ridl-<target>.tar.gz` and publishes
  `ridl-<target>.tar.gz.minisig`. The CLI embeds the public key and verifies
  with the `minisign-verify` crate (pure Rust, no dependencies). Verification is
  offline and small. Cost: a signing key held as a repository secret, and a
  rotation procedure.
- **B2 — GitHub artifact attestations (Sigstore).** No key to manage, but
  verification needs the Sigstore trust root (TUF), the Fulcio certificate chain
  and the Rekor log: either a large dependency tree in `ridl` or a runtime
  dependency on `gh attestation verify` and a GitHub login. Rejected for the
  CLI. It can be added later to the workflow for users of `gh` without changing
  B1.
- **B3 — sign the `.sha256` record instead of the archive.** Equivalent
  security, one more indirection. Rejected for simplicity.

### C. Workspace Trust prompt

- **C1 — limited untrusted support, prompt only (chosen).** Declare
  `untrustedWorkspaces: { supported: "limited", restrictedConfigurations:
  ["ridl.serverPath", "ridl.toolchain.autoInstall"] }`.
  In Restricted Mode the extension starts no language server and no MCP
  definition. For each RIDL root it runs only `ridl toolchain which --json` with
  the bundled binary (selection reads the manifest and never executes a
  candidate). When the result is `missing`, it shows one notification per
  release: "This workspace requires ridl X, which is not installed." with
  **Install** and **Manage Workspace Trust**. Install never starts automatically
  in Restricted Mode. Granting trust runs the normal activation.
- **C2 — run the bundled server in Restricted Mode.** Rejected: the server loads
  the workspace, including remote imports and lockfiles, which is more than a
  prompt needs.

## Design

### 1. Terminal hand-off (goal 1)

- Module `crates/ridl/src/handoff.rs`, called at every refusal site of the
  commands the release-pin chapter lists as enforcing the root pin. MANI-024 and
  MANI-025 never hand off; only MANI-026 does.
- The refusal diagnostics must carry the pin in structured form (today the pin
  is only inside the MANI-026 message). Add the parsed pin to the diagnostic
  data or return it next to the diagnostics; the builder chooses the carrier
  with the smallest change to `ridl-core`'s public surface. **(left to the
  builder, with that criterion)**
- Selection: the highest complete installed release that is compatible with
  **every** pin among the run's MANI-026 refusals. No such release: no hand-off,
  the refusal is printed as today. **(call)** A root that the current release
  accepts and the selected release does not is then refused by the child, which
  does not hand off again (below).
- Guard: the child runs with `RIDL_TOOLCHAIN_HANDOFF=<parent release>` in its
  environment. A process that finds this variable set never hands off and never
  installs; it prints its refusal and exits 2. **(call)**
- Unix: `std::os::unix::process::CommandExt::exec` replaces the process, so
  standard input, signals and the exit status pass through unchanged. If `exec`
  fails, print `error: could not run ridl <version> at <path>: <io
  error>` and
  exit 2.
- Windows: spawn the child with inherited handles, assign it to a Job Object
  created with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` (so killing the parent kills
  the child), ignore console Ctrl-C in the parent while waiting (the child
  receives it), and exit with the child's exit code. Uses `windows-sys` (already
  in `Cargo.lock`), gated to `cfg(windows)`. **(call)**
- Standard input: no enforcing command reads standard input before its refusal.
  The builder verifies this per site; a site that has read input must not hand
  off.
- Output: the hand-off prints nothing on success, so output and exit codes are
  those of the selected release. **(call)**
- `ridl toolchain *`, `init`, `new`, `lock merge`, snapshot-only `diff` and
  standalone descriptors never hand off (they do not enforce the pin).

### 2. `ridl mcp` hand-off (goal 2)

At start, before building the Tokio runtime, `run_mcp` discovers the root from
the current directory. If the root pin is valid and incompatible with the
running release, and a compatible release is installed, it hands off with the
same mechanism and guard as section 1 (argument list `mcp`). Otherwise it serves
as today, and tool calls report the refusal. It never installs. Discovery or
manifest errors at start are ignored here; the server reports them per call as
today. **(call)**

### 3. Opt-in terminal automatic install (goal 3)

- When section 1 finds no compatible installed release, and
  `RIDL_TOOLCHAIN_AUTO_INSTALL` is exactly `1`, and `RIDL_TOOLCHAIN_HANDOFF` is
  unset, the CLI installs the pin of the refusal (only when the run has exactly
  one distinct MANI-026 pin), then hands off. Other values of the variable,
  including empty, are off. **(call)**
- It prints `installing ridl <version>…` and the result line on standard error.
  An install failure prints the install error followed by the original refusal,
  and exits 2.
- Never from `ridl mcp` or `ridl lsp`. A `ridl.toml` key cannot enable it.

### 4. `ridl toolchain uninstall <VERSION>` (goal 4)

- Takes an exact release. Removes `<root>/versions/<version>/`, complete or
  damaged: this is the explicit repair path that ADR-0036 leaves to the user.
- Holds the version's lock (the same `.<version>.lock` as install), renames the
  directory to `.tmp-<version>-<pid>` (a name install already cleans under the
  lock), then removes that stage. A concurrent CLI install waits on the lock and
  then sees the version absent.
- Refuses to remove the directory that contains the running executable
  (canonical comparison), exit 2. **(call)**
- Not installed: message on stderr, exit 1, consistent with `which` reporting
  `missing` with exit 1. I/O errors: exit 2. Success prints the removed
  directory on stdout. **(call)** The builder checks this against ADR-0010.
- Does not acquire anything the scripts respect; a script install racing an
  uninstall of the same version is not serialized (same limit as install).

### 5. Signed releases (goal 5)

- Release workflow (`vscode-release.yaml`, "tarball and checksum" step): sign
  each `ridl-<target>.tar.gz` with minisign using the repository secret
  `RIDL_RELEASE_SIGNING_KEY` (and its password secret), with the trusted comment
  `ridl <version> <target>`, and upload the `.minisig` next to the archive. The
  step fails when the secret is absent, so the train cannot ship an unsigned
  release. `ridlc` archives are not signed (no CLI installs them). **(call)**
- CLI: `ridl toolchain install` downloads `<archive>.minisig` (bounded to 4
  KiB), verifies the archive bytes against the embedded public key, and checks
  that the trusted comment is exactly `ridl <version> <target>` (this stops an
  older or another target's signed archive being served under a new name).
  Verification happens before extraction, after the checksum check. A missing or
  invalid signature is an install error, exit 2.
- The public key lives in `crates/ridl/src/toolchain/mod.rs` next to the origin
  constant, as a minisign public key string. Tests use a separate test key pair
  committed under the test fixtures (its secret key is a test fixture, never
  used for a release).
- Every pinnable release (≥ 0.23.0) is signed, provided the secret exists before
  0.23.0 is tagged (owner action 1), so the CLI has no unsigned path. If 0.23.0
  ships before this change, the builder adds a constant `FIRST_SIGNED_RELEASE`:
  below it, explicit install accepts a checksum-only release with a warning, and
  automatic install (terminal and editor) refuses. **(call)**
- Key rotation: a release that embeds a new public key is signed with the old
  and new keys is out of scope; rotation means a new key for the next release
  and old binaries keep verifying old releases. Documented in the distribution
  technote. **(call)**
- The fixture HTTP and `file://` sources in tests and `just install-check` serve
  a `.minisig` signed with the test key; the CLI's test-only entry point takes
  the public key as a parameter, like `base_url`.

### 6. Workspace Trust prompt (goal 6)

As approach C1. The extension's existing install path is reused for the Install
button. ADR-0036 decision 8 and its "Activate in Restricted Mode" alternative
are amended in place. `ridl.serverPath` and `ridl.toolchain.autoInstall` are
restricted configurations.

### 7. Smaller items

1. **Relative `RIDL_TOOLCHAIN_DIR` is refused**: a nonempty relative value is an
   error, exit 2, in the CLI and in the scripts. 0.23.0 is not released, so no
   released contract changes. **(call)**
2. **Windows PATH fallback**: the extension resolves `ridl` from `PATH` entries
   that are absolute paths only, and never from the working directory.
3. **Stale script stages**: a script removes `.tmp-script-<version>-<digits>`
   directories of its own version whose modification time is older than 24
   hours, in addition to its own exact name. The age bound (the install timeout
   is 10 minutes) avoids deleting a live stage of another process or another PID
   namespace. **(call)**
4. **`install.sh` mv message**: when `mv` fails and no destination exists,
   report the `mv` failure, not "the directory appeared".
5. **`ridl diff` (snapshot directory) and `ridl describe` order**: a release
   refusal of the surrounding workspace is reported before an I/O error of
   listing or reading the input. Both keep their existing messages.
6. (From the issue's scope) The `which` text output for `missing` names the
   automatic install variable as an alternative to `ridl toolchain install`.
   **(call)**

## Tests the builder must add (each must fail when its behaviour is removed)

- Hand-off per enforcing command: a fixture toolchain root holding a fake `ridl`
  script (Unix) that records its argv, cwd and stdin and exits 7; the command's
  exit code is 7 and the record matches. On Windows CI, the same with a small
  compiled test binary.
- Guard: with `RIDL_TOOLCHAIN_HANDOFF` set, the refusal is printed and exit is
  2, and the fake binary did not run.
- Several distinct pins: hand-off only to a release compatible with all.
- `ridl mcp` hands off before reading standard input.
- Automatic install: off by default and for values other than `1`; on with `1`
  against the fixture origin; never from `mcp`.
- Signature: wrong key, wrong trusted comment, missing `.minisig`, tampered
  archive with a matching checksum record — each refused, nothing published.
- Uninstall: complete, damaged, absent, running-executable refusal, concurrent
  install waits.
- Relative `RIDL_TOOLCHAIN_DIR` refused (CLI and both scripts).
- Extension: Restricted Mode starts no client and no MCP definition, shows the
  prompt for `missing`, never installs without the button; PATH lookup skips
  relative entries.
- Script stage cleanup: old stage removed, fresh foreign stage kept.

## Constraints

- ADR-0009/0010 conventions; frozen surfaces extend by addition only (new
  subcommand, new variables, new `.minisig` asset); update `freeze/` dumps.
- No new dependency other than `minisign-verify`, and `windows-sys` features for
  the Job Object.
- The scripts keep their `u32` component limit.

## Builder check resolutions (2026-11-25)

A fresh builder read this design. Its questions are settled as follows; each is
a **(call)** under the delegation.

1. **`ridlc` is excluded.** The refusals in `crates/ridlc/src/lib.rs` and the
   `ridlc` binary keep their behaviour. The hand-off sites are only in the
   `ridl` binary (`crates/ridl/src/`). When `ridl` calls `ridlc` library code
   that returns refusal diagnostics, the hook sits in `ridl` after that call
   returns, before anything is rendered.
2. **Pin carrier: re-parse, no new `Diagnostic` field.** The helper takes the
   diagnostics and their sources, and for each MANI-026 re-parses the pin from
   the diagnostic's file text with `ridl_core::release::parse_release_pin` (the
   same parser that produced it). If that function or `ReleasePin` is not
   public, making them public is the only `ridl-core` API change allowed. No
   message parsing.
3. **One or several pins.** The rule "compatible with every MANI-026 pin of the
   run" stays, because `fmt` checks every discovered root and can report one
   refusal per root. The builder confirms which commands can carry more than one
   refusal. The multi-pin test runs on `fmt` only if `fmt` can produce two
   refusals; otherwise it is dropped, and the rule degenerates to the single
   pin. Automatic install still requires exactly one distinct pin.
4. **Hook position.** The hook runs where the refusal is detected and before any
   rendering, `out::write`, or `finish`/`finish_check` call: at main.rs
   `run_check` (before `finish_check`), at the release-refusal branch that leads
   to `exit_code`, at `lock.rs`, `property.rs` and `vendor.rs` (before the
   `render`), and at the other enforcing sites. `finish` and `exit_code`
   themselves are not hooks. The book says the refusal stops loading before
   remote imports, so `vendor` has fetched nothing when it refuses; the builder
   confirms this with a test that the fetch hook is not called on refusal. A
   site where any write, including a cache write, happens before the refusal
   does not hand off, and the plan lists it.
5. **`ridl mcp` root.** It uses the current directory. MCP hosts that start a
   server in the project directory get the hand-off. Hosts that start elsewhere
   keep the per-call refusal. No `--root` argument is added. The VS Code
   extension is unchanged here, because it already starts the selected release
   for its MCP definition.
6. **Signing branch.** Plan the branch where this change lands before 0.23.0 is
   tagged: no `FIRST_SIGNED_RELEASE`, every install requires a signature. The
   production public key constant holds a placeholder until owner action 1, and
   the pull request is not merged with the placeholder. Tests,
   `just install-check` and the fixture origins use the test key through a
   test-only parameter. If 0.23.0 is tagged first, the `FIRST_SIGNED_RELEASE`
   branch of section 5 is added before merge.

Facts the builder confirmed:

- `windows-sys` is in `Cargo.lock` at 0.52.0 and 0.61.2. Use 0.61 as the direct
  dependency (`cfg(windows)`, features `Win32_Foundation`,
  `Win32_System_JobObjects`, `Win32_System_Threading`, `Win32_System_Console`),
  so that no third version is added.
- `install.ps1` resolves `RIDL_TOOLCHAIN_DIR` with `GetFullPath`, which makes a
  relative value absolute. It must refuse the value instead, as `install.sh` and
  the CLI do.
- No enforcing `ridl` command reads standard input before its refusal.
- `ridl mcp` reads standard input only inside `serve_stdio_with_version`, after
  the hand-off point.

## Left to the builder

- The carrier of the structured pin on MANI-026 (section 1).
- The list of refusal sites, and per-site proof that no input was consumed.
- The minisign tool used in CI (`minisign` package or `rsign2`) and the exact
  step layout.
- Exit-code alignment of `uninstall` with ADR-0010.

## Owner actions (not blocking the build)

1. Generate the release signing key pair and add the secret key and its password
   as repository secrets `RIDL_RELEASE_SIGNING_KEY` and
   `RIDL_RELEASE_SIGNING_KEY_PASSWORD`; give the public key for the constant.
   The agent does not generate a production key, because the secret key would
   pass through the session. Until then the constant holds a placeholder and the
   release workflow fails closed. This must happen before the next `editor-v`
   tag that includes this change.
2. Confirm that this change may land before 0.23.0 is tagged (it sets which
   releases are signed).
