#!/usr/bin/env bash
# Publish the per-target VSIX files of one release to the VS Code Marketplace.
#
# Usage: publish-vsix.sh <vsix-dir>
#
# The token comes from VSCE_PAT. The script fails before it starts the
# publisher when that variable is unset or empty.
#
# Environment overrides (used by the test, which stubs the publisher):
#   PUBLISH_VSCE_BIN          vsce executable
#   PUBLISH_EXPECTED_FILES    number of VSIX files required (default 5)
set -euo pipefail

usage() {
    echo "usage: publish-vsix.sh <vsix-dir>" >&2
    exit 2
}

[ "$#" -eq 1 ] || usage
dir="$1"

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ext_dir="$(cd "$script_dir/.." && pwd)"

token="${VSCE_PAT:-}"
bin="${PUBLISH_VSCE_BIN:-$ext_dir/node_modules/.bin/vsce}"

if [ -z "$token" ]; then
    echo "::error::VSCE_PAT is unset or empty; refusing to publish" >&2
    exit 1
fi

shopt -s nullglob
files=("$dir"/*.vsix)
expected="${PUBLISH_EXPECTED_FILES:-5}"
if [ "${#files[@]}" -ne "$expected" ]; then
    echo "::error::expected $expected VSIX files, found ${#files[@]}" >&2
    exit 1
fi

for f in "${files[@]}"; do
    echo "publishing $f"
    "$bin" publish --packagePath "$f" --pat "$token"
done
