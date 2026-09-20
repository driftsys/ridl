#!/bin/bash
# SessionStart hook for Claude Code on the web.
#
# A web session starts from a base image that carries rustup and cargo but not
# the tools `just build` needs beside them. This hook installs what is missing
# so a session can run every gate: just, prim, mdbook, the pinned toolchain
# with its wasm target, and the `ridl-rt` minimum toolchain for compat-check.
# It mirrors the tool installs in .github/workflows/ci.yml, including the two
# version pins there and the source-build fallback for prim, and it runs
# `./bootstrap`'s git-std and hook steps where they apply.
#
# It runs only in a remote session (CLAUDE_CODE_REMOTE=true). A local checkout
# uses ./bootstrap. Every step is idempotent: a tool already on PATH is kept.
set -euo pipefail

if [ "${CLAUDE_CODE_REMOTE:-}" != "true" ]; then
    exit 0
fi

cd "${CLAUDE_PROJECT_DIR:-$(git rev-parse --show-toplevel)}"

# The first two are the pins of .github/workflows/ci.yml `env:`. Change them
# together.
JUST_VERSION=1.38.0
MDBOOK_VERSION=0.4.40
# Not pinned in CI, which installs prim through its install.sh; pinned here so
# the download needs no API call. Bump it with prim.
PRIM_VERSION=0.9.0

bin="$HOME/.local/bin"
mkdir -p "$bin"
export PATH="$bin:$HOME/.cargo/bin:$PATH"

log() { printf '[session-start] %s\n' "$*" >&2; }

# just — the recipe runner every gate goes through.
if ! command -v just >/dev/null 2>&1; then
    log "installing just $JUST_VERSION"
    curl -fsSL "https://github.com/casey/just/releases/download/${JUST_VERSION}/just-${JUST_VERSION}-x86_64-unknown-linux-musl.tar.gz" \
        | tar -xz -C "$bin" just
fi
just --version >&2

# git-std — commit lint (`just lint-commits`, `just verify`) and the git hooks.
# Best effort: a failed download must not stop the session, because no member
# of `just build` needs it. The summary at the end names it when it is absent.
if ! command -v git-std >/dev/null 2>&1; then
    log "installing git-std"
    GIT_STD_INSTALL_DIR="$bin" bash -c \
        'set -o pipefail; curl -fsSL https://raw.githubusercontent.com/driftsys/git-std/main/install.sh | bash' \
        || log "warning: git-std did not install; just lint-commits and just verify will not run"
fi

# prim — `just check`. The release asset is fetched directly under a pinned
# version, as just and mdbook are above: prim's install.sh asks the GitHub API
# for the latest release first, and that call is rate limited for
# unauthenticated callers on a shared address, which is the 403 CI's comment
# describes. The source build is the last resort, as in CI; the crate in that
# repository is `prim-cli`.
if ! command -v prim >/dev/null 2>&1; then
    log "installing prim $PRIM_VERSION"
    bash -c "set -o pipefail; curl -fsSL 'https://github.com/driftsys/prim/releases/download/v${PRIM_VERSION}/prim-x86_64-unknown-linux-musl.tar.gz' | tar -xz -C '$bin' prim" \
        || {
            log "prim release download failed; building prim-cli from source"
            cargo install --git https://github.com/driftsys/prim prim-cli --locked
        }
fi
prim --version >&2

# mdbook — `just book-check`. Release first, cargo second, for the same reason.
if ! command -v mdbook >/dev/null 2>&1; then
    log "installing mdbook $MDBOOK_VERSION"
    bash -c "set -o pipefail; curl -fsSL 'https://github.com/rust-lang/mdBook/releases/download/v${MDBOOK_VERSION}/mdbook-v${MDBOOK_VERSION}-x86_64-unknown-linux-gnu.tar.gz' | tar -xz -C '$bin'" \
        || {
            log "mdbook release download failed; building from source"
            cargo install mdbook --version "$MDBOOK_VERSION" --locked
        }
fi
mdbook --version >&2

# The pinned toolchain, its components and its wasm target come from
# rust-toolchain.toml; `rustup show` installs whatever that file names and is
# what CI runs. The `ridl-rt` minimum toolchain is what `just compat-check`
# builds the packaged crate with (ADR-0021 decision 10).
log "installing the pinned toolchain from rust-toolchain.toml"
rustup show >/dev/null
minimum="$(sed -n 's/^rust-version[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' crates/ridl-rt/Cargo.toml)"
if [ -n "$minimum" ] && ! rustup run "$minimum" rustc --version >/dev/null 2>&1; then
    log "installing the ridl-rt minimum toolchain $minimum"
    rustup toolchain install "$minimum" --profile minimal
fi
cargo --version >&2

# The git hooks git-std wires (commit-msg lint). Skipped when git-std is absent.
if command -v git-std >/dev/null 2>&1; then
    git std bootstrap >/dev/null 2>&1 || log "warning: git std bootstrap failed; commit hooks are not wired"
fi

# Every crate the lockfile names, so `--locked` builds and tests do not fetch.
log "fetching the workspace's dependencies"
cargo fetch --locked >/dev/null

# The session's shell does not inherit this script's PATH.
if [ -n "${CLAUDE_ENV_FILE:-}" ]; then
    printf 'export PATH="%s:$HOME/.cargo/bin:$PATH"\n' "$bin" >> "$CLAUDE_ENV_FILE"
fi

missing=""
for tool in just prim mdbook git-std; do
    command -v "$tool" >/dev/null 2>&1 || missing="$missing $tool"
done
if [ -n "$missing" ]; then
    log "done; not installed:$missing"
else
    log "done; every gate tool is on PATH"
fi
