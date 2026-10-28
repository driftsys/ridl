#!/usr/bin/env bash
# Publish the per-target VSIX files of one release to one registry.
#
# Usage: publish-vsix.sh <vsce|ovsx> <vsix-dir>
#
# The token comes from VSCE_PAT (vsce) or OVSX_PAT (ovsx). The script fails
# before it starts any publisher when that variable is unset or empty. An
# empty token must fail here: `ovsx publish` prompts for a token when it has
# none, and exits 0 when the prompt reaches end of input, so a job could
# report success without publishing anything (driftsys/ridl#980).
#
# For ovsx, the script then asks the registry for the published version and
# fails unless the registry answers 200.
#
# Environment overrides (used by the test, which stubs the publisher and the
# registry):
#   PUBLISH_VSCE_BIN          vsce executable
#   PUBLISH_OVSX_BIN          ovsx executable
#   PUBLISH_OVSX_REGISTRY     registry base URL (default https://open-vsx.org)
#   PUBLISH_VERIFY_ATTEMPTS   registry checks before failing (default 5)
#   PUBLISH_VERIFY_DELAY      seconds between registry checks (default 10)
#   PUBLISH_EXPECTED_FILES    number of VSIX files required (default 5)
#   PUBLISH_VERSION           version to look up (default: package.json)
set -euo pipefail

usage() {
    echo "usage: publish-vsix.sh <vsce|ovsx> <vsix-dir>" >&2
    exit 2
}

[ "$#" -eq 2 ] || usage
tool="$1"
dir="$2"

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ext_dir="$(cd "$script_dir/.." && pwd)"

case "$tool" in
    vsce)
        token_name=VSCE_PAT
        token="${VSCE_PAT:-}"
        bin="${PUBLISH_VSCE_BIN:-$ext_dir/node_modules/.bin/vsce}"
        ;;
    ovsx)
        token_name=OVSX_PAT
        token="${OVSX_PAT:-}"
        bin="${PUBLISH_OVSX_BIN:-$ext_dir/node_modules/.bin/ovsx}"
        ;;
    *) usage ;;
esac

if [ -z "$token" ]; then
    echo "::error::$token_name is unset or empty; refusing to publish with $tool" >&2
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
    case "$tool" in
        vsce) "$bin" publish --packagePath "$f" --pat "$token" ;;
        ovsx) "$bin" publish "$f" -p "$token" ;;
    esac
done

if [ "$tool" = ovsx ]; then
    version="${PUBLISH_VERSION:-$(sed -n 's/^  "version": "\(.*\)",$/\1/p' "$ext_dir/package.json" | head -n 1)}"
    if [ -z "$version" ]; then
        echo "::error::could not read the extension version" >&2
        exit 1
    fi
    url="${PUBLISH_OVSX_REGISTRY:-https://open-vsx.org}/api/driftsys/ridl-lang/$version"
    attempts="${PUBLISH_VERIFY_ATTEMPTS:-5}"
    delay="${PUBLISH_VERIFY_DELAY:-10}"
    n=1
    while :; do
        status="$(curl -s -o /dev/null -w '%{http_code}' "$url" || true)"
        if [ "$status" = 200 ]; then
            echo "registry lists $url"
            break
        fi
        if [ "$n" -ge "$attempts" ]; then
            echo "::error::registry did not list version $version (last status ${status:-none}): $url" >&2
            exit 1
        fi
        n=$((n + 1))
        sleep "$delay"
    done
fi
