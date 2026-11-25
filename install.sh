#!/usr/bin/env bash
# Installs a binary from the newest editor-v* GitHub Release (or the one named
# in RIDL_VERSION): downloads <binary>-<target>.tar.gz and its .sha256,
# verifies the checksum, stores pinnable ridl releases in the shared toolchain
# directory, and copies the binary to RIDL_INSTALL_DIR (default ~/.local/bin).
# Older ridl releases and ridlc retain flat installation. Usage:
#   curl -fsSL https://raw.githubusercontent.com/driftsys/ridl/main/install.sh | bash
#
# RIDL_INSTALL_BINARY selects which binary the release carries to install:
# `ridl` (default, the porcelain CLI and what the editor extension bundles) or
# `ridlc` (the plumbing compiler front end, stable flags for CI and build
# systems). Both are built for the same targets and published to the same
# release, one tarball per binary per target.
#
# RIDL_INSTALL_BASE_URL overrides where release assets are fetched from
# (default https://github.com/$REPO/releases/download). It exists so
# `just install-check` can point downloads at a local fixture; it is a test
# hook, not a setting an end user is expected to set.
set -eu

REPO="driftsys/ridl"
TAG_PREFIX="editor-v"
INSTALL_DIR="${RIDL_INSTALL_DIR:-${HOME:-}/.local/bin}"
BINARY="${RIDL_INSTALL_BINARY:-ridl}"
case "$BINARY" in
  ridl | ridlc) ;;
  *)
    echo "error: RIDL_INSTALL_BINARY must be 'ridl' or 'ridlc', got '$BINARY'" >&2
    exit 1
    ;;
esac
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

# Accept decimal release components in the same u32 range as the CLI.
normalise_version() {
  local value="${1#editor-v}" part result="" IFS=.
  [[ "$value" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || return 1
  for part in $value; do
    while [ "${#part}" -gt 1 ] && [ "${part#0}" != "$part" ]; do part="${part#0}"; done
    [ "${#part}" -le 10 ] || return 1
    [ "$part" -le 4294967295 ] || return 1
    result="${result:+$result.}$part"
  done
  printf '%s\n' "$result"
}

toolchain_root() {
  local root
  if [ -n "${RIDL_TOOLCHAIN_DIR:-}" ]; then
    root="$RIDL_TOOLCHAIN_DIR"
  else
    case "$(uname -s)" in
      Linux)
        if [ -n "${XDG_DATA_HOME:-}" ]; then root="$XDG_DATA_HOME/ridl"
        else root="${HOME:?toolchain directory requires HOME}/.local/share/ridl"; fi ;;
      Darwin) root="${HOME:?toolchain directory requires HOME}/Library/Application Support/ridl" ;;
      *) echo "error: unsupported toolchain platform" >&2; return 1 ;;
    esac
  fi
  case "$root" in /*) ;; *) root="$PWD/$root" ;; esac
  printf '%s\n' "$root"
}

# An existing directory is never repaired or replaced.
complete_release() {
  [ -e "$release_dir" ] || [ -L "$release_dir" ] || return 1
  if [ -d "$release_dir" ] && [ ! -L "$release_dir" ] &&
     [ -f "$release_dir/ridl" ] && [ ! -L "$release_dir/ridl" ] &&
     [ -f "$release_dir/.complete" ] && [ ! -L "$release_dir/.complete" ] &&
     [ ! -s "$release_dir/.complete" ]; then
    return 0
  fi
  echo "error: damaged toolchain directory $release_dir; preserve or move it before installing again" >&2
  exit 1
}

download() {
  local url="$1" output="$2" limit="$3" size
  curl -fsSL --connect-timeout 30 --max-time 600 --max-filesize "$limit" "$url" -o "$output"
  size="$(wc -c < "$output" | tr -d ' ')"
  [ "$size" -le "$limit" ] || { echo "error: download exceeds $limit bytes" >&2; return 1; }
}

verify_checksum() {
  local hash tail record
  record="$(cat "$tmpdir/$tarball.sha256"; printf '.')"
  record="${record%.}"
  record="${record%$'\n'}"
  record="${record%$'\r'}"
  hash="${record:0:64}"; tail="${record:64}"
  if ! [[ "$hash" =~ ^[a-fA-F0-9]{64}$ ]] ||
     { [ "$tail" != "  $tarball" ] && [ "$tail" != " *$tarball" ]; }; then
    echo "error: invalid checksum record for $tarball" >&2; return 1
  fi
  (cd "$tmpdir" && if command -v sha256sum >/dev/null; then sha256sum -c "$tarball.sha256"; else shasum -a 256 -c "$tarball.sha256"; fi)
}

extract_release() {
  local archive="$tmpdir/$tarball" destination="$1" name count=0 names="|"
  # Check names and types before extraction. Only three root-level regular
  # files are allowed; duplicates, links and archive markers are refused.
  tar tzf "$archive" > "$tmpdir/names"
  tar tvzf "$archive" > "$tmpdir/types"
  while IFS= read -r name; do
    case "$name" in "$BINARY" | LICENSE | THIRD-PARTY-NOTICES.txt) ;; *) echo "error: unsafe archive entry $name" >&2; return 1 ;; esac
    case "$names" in *"|$name|"*) echo "error: duplicate archive entry $name" >&2; return 1 ;; esac
    names="$names$name|"; count=$((count + 1))
  done < "$tmpdir/names"
  [ "$count" -eq 3 ] || { echo "error: archive missing required files" >&2; return 1; }
  if LC_ALL=C grep -qv '^-' "$tmpdir/types"; then echo "error: unsafe archive entry type" >&2; return 1; fi
  gzip -t "$archive"
  tar xzf "$archive" -C "$destination"
  for name in "$BINARY" LICENSE THIRD-PARTY-NOTICES.txt; do
    [ -f "$destination/$name" ] && [ ! -L "$destination/$name" ] || return 1
  done
  chmod +x "$destination/$BINARY"
}

main() {
  local target version raw normalized="" versioned="" root="" release_dir="" source=""
  target="$(detect_target)"
  raw="$(get_version)"
  version="$raw"
  if normalized="$(normalise_version "$raw")"; then
    version="$TAG_PREFIX$normalized"
    local major minor rest
    major="${normalized%%.*}"; rest="${normalized#*.}"; minor="${rest%%.*}"
    if [ "$BINARY" = ridl ] && { [ "$major" -gt 0 ] || [ "$minor" -ge 23 ]; }; then versioned=1; fi

  fi
  tarball="${BINARY}-${target}.tar.gz"
  url="$RIDL_INSTALL_BASE_URL/$version/$tarball"
  if [ -n "${RIDL_INSTALL_DRY_RUN:-}" ]; then echo "$url"; return; fi

  if [ -z "${RIDL_INSTALL_DIR:-}" ]; then INSTALL_DIR="${HOME:?PATH directory requires HOME}/.local/bin"; fi
  tmpdir=""; tmpbin=""; stage=""
  trap 'rm -rf "${tmpdir:-}" "${tmpbin:-}" "${stage:-}"' EXIT
  trap 'exit 130' INT
  trap 'exit 143' TERM
  if [ -n "$versioned" ]; then
    root="$(toolchain_root)"
    release_dir="$root/versions/$normalized"
    if complete_release; then source="$release_dir/ridl"; fi
  fi
  if [ -z "${source:-}" ]; then
    if [ -n "$versioned" ]; then
      mkdir -p "$root/versions"
      # Assign cleanup ownership only after exclusively creating the stage.
      local candidate="$root/versions/.tmp-$normalized-$$"
      mkdir "$candidate"
      stage="$candidate"
    fi
    tmpdir="$(mktemp -d)"
    download "$url" "$tmpdir/$tarball" 268435456
    download "$url.sha256" "$tmpdir/$tarball.sha256" 4096
    verify_checksum
    if [ -n "$versioned" ]; then
      # A version-named child lets both BSD and GNU mv address the versions
      # parent. They refuse an existing child without nesting the source in it.
      local payload="$stage/$normalized"
      mkdir "$payload"
      extract_release "$payload"
      : > "$payload/.complete"
      if ! complete_release; then
        mv -n "$payload" "$root/versions/"
        if [ -d "$payload" ]; then
          if ! complete_release; then
            echo "error: toolchain directory appeared: $release_dir" >&2
            return 1
          fi
        fi
      fi
      rm -rf "$stage"; stage=""
      source="$release_dir/ridl"
    else
      # Historical archives retain their original flat extraction behavior.
      tar xzf "$tmpdir/$tarball" -C "$tmpdir"
      source="$tmpdir/$BINARY"
    fi
  fi

  # Rename a temporary PATH file on the same filesystem so an executing old
  # binary can be replaced without truncating it or failing with ETXTBSY.
  mkdir -p "$INSTALL_DIR"
  tmpbin="$(mktemp "$INSTALL_DIR/.$BINARY.XXXXXX")"
  cp "$source" "$tmpbin"
  chmod +x "$tmpbin"
  mv "$tmpbin" "$INSTALL_DIR/$BINARY"
  tmpbin=""
  echo "Installed $INSTALL_DIR/$BINARY ($("$INSTALL_DIR/$BINARY" --version))" >&2
  if ! printf '%s\n' "$PATH" | tr ':' '\n' | grep -qxF "$INSTALL_DIR"; then
    echo "Add to your PATH:" >&2
    echo "  export PATH=\"$INSTALL_DIR:\$PATH\"" >&2
  fi
}

main
