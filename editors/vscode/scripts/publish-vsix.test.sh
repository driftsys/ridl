#!/usr/bin/env bash
# Test for publish-vsix.sh. No network: the publisher and curl are stubs.
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
# curl stub: prints the status in STUB_STATUS and records the URL.
cat > "$scratch/bin/curl" <<STUB
#!/usr/bin/env bash
echo "\${@: -1}" >> "$scratch/urls"
printf '%s' "\${STUB_STATUS:-200}"
STUB
chmod +x "$scratch/bin/publisher" "$scratch/bin/curl"

failures=0
fail() { echo "FAIL: $*" >&2; failures=$((failures + 1)); }

# run <tool> [VAR=value ...]; leaves the exit status in $rc
run() {
    local tool="$1"
    shift
    rm -f "$scratch/calls" "$scratch/urls"
    env -u VSCE_PAT -u OVSX_PAT \
        PATH="$scratch/bin:$PATH" \
        PUBLISH_VSCE_BIN="$scratch/bin/publisher" \
        PUBLISH_OVSX_BIN="$scratch/bin/publisher" \
        PUBLISH_OVSX_REGISTRY="http://registry.invalid" \
        PUBLISH_VERSION="1.2.3" \
        PUBLISH_VERIFY_ATTEMPTS=2 PUBLISH_VERIFY_DELAY=0 \
        "$@" bash "$script" "$tool" "$scratch/vsix" >/dev/null 2>&1
    rc=$?
}

calls() {
    if [ -f "$scratch/calls" ]; then wc -l < "$scratch/calls" | tr -d ' '; else echo 0; fi
}

for tool in vsce ovsx; do
    if [ "$tool" = vsce ]; then var=VSCE_PAT; else var=OVSX_PAT; fi
    run "$tool"
    [ "$rc" -ne 0 ] || fail "$tool: unset $var exited 0"
    [ "$(calls)" = 0 ] || fail "$tool: publisher ran with $var unset"
    run "$tool" "$var="
    [ "$rc" -ne 0 ] || fail "$tool: empty $var exited 0"
    [ "$(calls)" = 0 ] || fail "$tool: publisher ran with $var empty"
    run "$tool" "$var=secret"
    [ "$rc" -eq 0 ] || fail "$tool: valid token exited $rc"
    [ "$(calls)" = 5 ] || fail "$tool: expected 5 publisher calls, got $(calls)"
    grep -q "secret" "$scratch/calls" || fail "$tool: token was not passed to the publisher"
done

# ovsx checks the registry for the version.
run ovsx OVSX_PAT=secret
grep -qx "http://registry.invalid/api/driftsys/ridl-lang/1.2.3" "$scratch/urls" ||
    fail "ovsx: registry URL was not queried"
run ovsx OVSX_PAT=secret STUB_STATUS=404
[ "$rc" -ne 0 ] || fail "ovsx: registry answering 404 exited 0"

# A wrong file count fails before any publisher runs.
rm "$scratch/vsix/ridl-lang-e.vsix"
run vsce VSCE_PAT=secret
[ "$rc" -ne 0 ] || fail "vsce: 4 files exited 0"
[ "$(calls)" = 0 ] || fail "vsce: publisher ran with 4 files"

if [ "$failures" -ne 0 ]; then
    echo "publish-vsix.test: $failures failure(s)" >&2
    exit 1
fi
echo "publish-vsix.test: ok"
