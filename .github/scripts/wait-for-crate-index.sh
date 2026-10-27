#!/usr/bin/env bash
# Wait until a crate version is in the crates.io index that cargo reads.
#
# Usage: wait-for-crate-index.sh <crate> <version>
#
# crates.io records an upload first and adds it to the index in a separate
# step, which usually takes seconds but has taken hours. A crate that depends
# on one not yet in the index cannot publish, so the release workflow calls
# this after each crate instead of sleeping for a fixed time.
#
# Environment: CRATES_INDEX_URL (default https://index.crates.io),
# INDEX_WAIT_TIMEOUT in seconds (default 1800), INDEX_WAIT_INTERVAL in seconds
# (default 15). Exits 0 when the version is in the index, and 1 after the
# timeout. xtask/tests/crate_index_wait.rs checks this script.
set -euo pipefail

crate="$1"
version="$2"
index_url="${CRATES_INDEX_URL:-https://index.crates.io}"
timeout="${INDEX_WAIT_TIMEOUT:-1800}"
interval="${INDEX_WAIT_INTERVAL:-15}"

# The sparse index path of a crate name, which the index lowercases.
name="$(printf '%s' "$crate" | tr '[:upper:]' '[:lower:]')"
case "${#name}" in
    1) path="1/$name" ;;
    2) path="2/$name" ;;
    3) path="3/${name:0:1}/$name" ;;
    *) path="${name:0:2}/${name:2:2}/$name" ;;
esac

deadline=$((SECONDS + timeout))
while :; do
    # A failed fetch (the file does not exist yet, or a network error) counts
    # as "not yet".
    if curl -fsS "$index_url/$path" 2>/dev/null | grep -qF "\"vers\":\"$version\""; then
        echo "$crate $version is in the index"
        exit 0
    fi
    if [ "$SECONDS" -ge "$deadline" ]; then
        echo "$crate $version is not in the index after $timeout s" >&2
        exit 1
    fi
    sleep "$interval"
done
