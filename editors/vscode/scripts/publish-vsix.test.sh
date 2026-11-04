#!/usr/bin/env bash
# Test for publish-vsix.sh. No network: the publisher is a stub.
set -uo pipefail

script="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/publish-vsix.sh"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT

mkdir -p "$scratch/bin" "$scratch/vsix"
for t in a b c d e; do : > "$scratch/vsix/ridl-lang-$t.vsix"; done

# Publisher stub: records one line per call, and nothing else.
cat > "$scratch/bin/publisher" <<STUB
#!/usr/bin/env bash
echo "\$*" >> "$scratch/calls"
STUB
chmod +x "$scratch/bin/publisher"

failures=0
fail() { echo "FAIL: $*" >&2; failures=$((failures + 1)); }

# run [VAR=value ...]; leaves the exit status in $rc
run() {
    rm -f "$scratch/calls"
    env -u VSCE_PAT \
        PUBLISH_VSCE_BIN="$scratch/bin/publisher" \
        "$@" bash "$script" "$scratch/vsix" >/dev/null 2>&1
    rc=$?
}

calls() {
    if [ -f "$scratch/calls" ]; then wc -l < "$scratch/calls" | tr -d ' '; else echo 0; fi
}

run
[ "$rc" -ne 0 ] || fail "unset VSCE_PAT exited 0"
[ "$(calls)" = 0 ] || fail "publisher ran with VSCE_PAT unset"
run VSCE_PAT=
[ "$rc" -ne 0 ] || fail "empty VSCE_PAT exited 0"
[ "$(calls)" = 0 ] || fail "publisher ran with VSCE_PAT empty"
run VSCE_PAT=secret
[ "$rc" -eq 0 ] || fail "valid token exited $rc"
[ "$(calls)" = 5 ] || fail "expected 5 publisher calls, got $(calls)"
grep -q "secret" "$scratch/calls" || fail "token was not passed to the publisher"

# A wrong file count fails before any publisher runs.
rm "$scratch/vsix/ridl-lang-e.vsix"
run VSCE_PAT=secret
[ "$rc" -ne 0 ] || fail "4 files exited 0"
[ "$(calls)" = 0 ] || fail "publisher ran with 4 files"

if [ "$failures" -ne 0 ]; then
    echo "publish-vsix.test: $failures failure(s)" >&2
    exit 1
fi
echo "publish-vsix.test: ok"
