#!/usr/bin/env bash
# Usage: scripts/verify-deno-zip.sh <zip> <target> [pin-file]
#
# Fails unless the SHA-256 of <zip> equals the one that the pin file (default:
# .deno-sha256 at the repository root) records for deno-<target>.zip. A missing
# or malformed pin, an unreadable zip and a different hash all fail.
set -euo pipefail

zip="${1:?usage: verify-deno-zip.sh <zip> <target> [pin-file]}"
target="${2:?usage: verify-deno-zip.sh <zip> <target> [pin-file]}"
pins="${3:-$(dirname "$0")/../.deno-sha256}"
name="deno-${target}.zip"

if [ ! -f "$pins" ]; then
  echo "deno checksum: the pin file $pins does not exist" >&2
  exit 1
fi
expected="$(awk -v f="$name" '$2 == f { print $1 }' "$pins")"
if ! printf '%s' "$expected" | grep -Eqx '[0-9a-f]{64}'; then
  echo "deno checksum: no valid SHA-256 for $name in $pins" >&2
  exit 1
fi

if command -v sha256sum >/dev/null 2>&1; then
  actual="$(sha256sum "$zip" | cut -d' ' -f1)"
else
  actual="$(shasum -a 256 "$zip" | cut -d' ' -f1)"
fi

if [ "$actual" != "$expected" ]; then
  echo "deno checksum: $name does not match the pinned SHA-256" >&2
  echo "  expected: $expected" >&2
  echo "  actual:   $actual" >&2
  exit 1
fi
echo "deno checksum: $name matches the pinned SHA-256"
