#!/usr/bin/env bash
# The pre-push gate, run by .githooks/pre-push.hooks. git writes one line per
# pushed ref on stdin, `<local ref> <local sha> <remote ref> <remote sha>`, and
# git-std forwards them. A push that holds only tags (a release tag, or the
# deletion of one) changes no branch, so the gate is skipped; any other push
# runs `just pre-push`. xtask/tests/pre_push_gate.rs checks this script.
set -euo pipefail

branch_pushed=false
while read -r _local_ref _local_sha remote_ref _remote_sha; do
    case "$remote_ref" in
        refs/tags/*) ;;
        *) branch_pushed=true ;;
    esac
done

if [ "$branch_pushed" = false ]; then
    echo "pre-push: only tags are pushed; the gate is skipped"
    exit 0
fi
exec just pre-push
