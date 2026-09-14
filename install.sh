#!/usr/bin/env bash
# Installs the ridl binary from the newest editor-v* GitHub Release (or the one
# named in RIDL_VERSION): downloads ridl-<target>.tar.gz and its .sha256,
# verifies the checksum, and places `ridl` in RIDL_INSTALL_DIR (default
# ~/.local/bin). Usage:
#   curl -fsSL https://raw.githubusercontent.com/driftsys/ridl/main/install.sh | bash
#
# RIDL_INSTALL_BASE_URL overrides where release assets are fetched from
# (default https://github.com/$REPO/releases/download). It exists so
# `just install-check` can point downloads at a local fixture; it is a test
# hook, not a setting an end user is expected to set.
set -eu

REPO="driftsys/ridl"
TAG_PREFIX="editor-v"
INSTALL_DIR="${RIDL_INSTALL_DIR:-$HOME/.local/bin}"
BINARY="ridl"
RIDL_INSTALL_BASE_URL="${RIDL_INSTALL_BASE_URL:-https://github.com/$REPO/releases/download}"

detect_target() {
  case "$(uname -s)/$(uname -m)" in
    Linux/x86_64)   echo "x86_64-unknown-linux-gnu" ;;
    Linux/aarch64)  echo "aarch64-unknown-linux-gnu" ;;
    Darwin/x86_64)  echo "x86_64-apple-darwin" ;;
    Darwin/arm64)   echo "aarch64-apple-darwin" ;;
    *) echo "error: unsupported platform: $(uname -s)/$(uname -m)" >&2; exit 1 ;;
  esac
}

# The newest release whose tag starts with editor-v. Never /releases/latest:
# that is the newest release across every tag, and the workspace's own v*
# releases must not be picked up here.
get_version() {
  if [ -n "${RIDL_VERSION:-}" ]; then
    printf '%s\n' "$RIDL_VERSION"
    return
  fi
  if command -v gh >/dev/null 2>&1; then
    if version=$(gh release list --repo "$REPO" --limit 100 --json tagName \
        -q "[.[].tagName | select(startswith(\"$TAG_PREFIX\"))][0]" 2>/dev/null) \
       && [ -n "$version" ]; then
      printf '%s\n' "$version"
      return
    fi
  fi
  body=$(curl -fsSL "https://api.github.com/repos/$REPO/releases?per_page=100") \
    || { echo "error: could not reach the GitHub API" >&2; exit 1; }
  version=$(printf '%s\n' "$body" | grep '"tag_name"' | cut -d'"' -f4 | grep "^$TAG_PREFIX" | head -1)
  if [ -z "$version" ]; then
    echo "error: no $TAG_PREFIX* release found" >&2
    exit 1
  fi
  printf '%s\n' "$version"
}

main() {
  local target version tarball url
  target="$(detect_target)"
  version="$(get_version)"
  tarball="ridl-${target}.tar.gz"
  url="$RIDL_INSTALL_BASE_URL/$version/$tarball"

  if [ -n "${RIDL_INSTALL_DRY_RUN:-}" ]; then
    echo "$url"
    return
  fi

  echo "Installing ridl $version ($target) to $INSTALL_DIR" >&2
  tmpdir="$(mktemp -d)"
  tmpbin=""
  # Cleans up the download staging directory and, if the atomic install below
  # was interrupted after claiming a temporary name inside INSTALL_DIR, that
  # temporary file too. INT and TERM are trapped as well as EXIT: without
  # them, an interrupt (Ctrl-C) between the mktemp that claims the temporary
  # name and the final rename would kill the script by the signal's default
  # disposition, which bypasses the EXIT trap entirely and leaves the hidden
  # temporary file behind.
  trap 'rm -rf "${tmpdir:-}" "${tmpbin:-}"' EXIT INT TERM
  curl -fsSL "$url" -o "$tmpdir/$tarball"
  curl -fsSL "$url.sha256" -o "$tmpdir/$tarball.sha256"
  (cd "$tmpdir" && if command -v sha256sum >/dev/null; then sha256sum -c "$tarball.sha256"; else shasum -a 256 -c "$tarball.sha256"; fi)
  tar xzf "$tmpdir/$tarball" -C "$tmpdir"

  # Install by renaming into place rather than moving the extracted binary
  # straight onto INSTALL_DIR/BINARY. tmpdir is usually a different
  # filesystem from INSTALL_DIR, so a direct `mv` there is a copy-and-unlink:
  # it fails with ETXTBSY while a previous ridl is still running — the normal
  # case for someone upgrading ridl while `ridl mcp` is running under Claude
  # Code or Codex, not a corner case. Claiming a temporary name inside
  # INSTALL_DIR first, making it executable there, and then renaming it over
  # BINARY keeps both operations within one filesystem, where a rename is
  # atomic and succeeds even while the old inode is still executing.
  mkdir -p "$INSTALL_DIR"
  tmpbin="$(mktemp "$INSTALL_DIR/.$BINARY.XXXXXX")"
  mv "$tmpdir/$BINARY" "$tmpbin"
  chmod +x "$tmpbin"
  mv "$tmpbin" "$INSTALL_DIR/$BINARY"
  tmpbin=""

  echo "Installed $INSTALL_DIR/$BINARY ($("$INSTALL_DIR/$BINARY" --version))" >&2
  if ! printf '%s\n' "$PATH" | tr ':' '\n' | grep -qx "$INSTALL_DIR"; then
    echo "" >&2
    echo "Add to your PATH:" >&2
    echo "  export PATH=\"$INSTALL_DIR:\$PATH\"" >&2
  fi
}

main
