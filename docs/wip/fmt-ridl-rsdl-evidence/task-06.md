# Task 6 evidence

Date: 2026-10-01, Europe/Paris. Base:
`8b8587106e164a8b2b7ffb5a42fbe2463f0da42f`, Task 5 PR #632. Branch
`feat/387-fmt-ridl`; Tasks 6–8 will be one consecutive grouped PR, based on
`feat/387-fmt-callers`. Each task has a distinct commit and quick review.

## Expected behavior

Canonical interface bodies and every value/callable parser slot without
attributes; tight streams in every type position; parameter and tuple-return
breaking through the existing engine; comments and modifiers preserved.
Attribute members remain verbatim until Task 7. Acceptance:
`cargo test -p ridl-fmt --locked`, `cargo test -p ridl-lsp --locked` and the
formatter invariants at widths 100, 60 and 40.

## RED

A read-only CLI reproduction at `d31f4b9` exited 0 and produced:

```text
struct Streams {
  a:
  b: [; 1..2]
}
```

The `a:` line actually ends with one space. Input fields were `a: <T>` and
`b: [<T>; 1..2]`; both streams disappeared. Local log:
`task-06-stream-before.log`.

Tests were added before production changes. The profile-aware assertion helper
checks exact output, successful reparsing, a second formatting pass, node entry
and exit structure, and non-whitespace/non-comma content including comments.
Local `task-06-red-tests.diff` records the test diff on the base.

- `cargo test -p ridl-fmt ridl_struct_preserves_direct_and_array_stream_types
  --locked`:
  exit 101, expected output assertion failure. Actual text dropped both streams;
  expected text preserves `<T>` and `[<T>; 1..2]`.
- `cargo test -p ridl-fmt ridl_ --locked`: exit 101, 16 expected layout/type
  failures and one passing malformed-interface control. No fixture parse error
  was counted as a regression. Local logs: `task-06-red-stream.log` and
  `task-06-red-members.log`.

## GREEN

- `cargo test -p ridl-fmt --locked`: exit 0, 49 unit tests, the paired golden
  corpus, nine EditorConfig reader tests, the three-profile invariants at widths
  100/60/40 and the current rsdl reference tests passed.
- New fixtures cover every member, four return shapes, stream payload/parameter/
  return, direct/array/map/tuple/optional streams, four timing spellings, event
  and fixed initializers, fixed timing, command return, internal empty bodies,
  body/header/parameter/type/timing comments, and default/60 width boundaries.
  The width-60 worked example breaks the tuple return first and stops; at 40 it
  also breaks the parameter list. Each fixture checks structure and fixed point.

`cargo test -p ridl-lsp --locked` exited 0: 17 library and 65 server tests
passed, including the unchanged canonical ridl rendering. Quick review,
committed-head checks and the grouped PR are forthcoming. Full local logs are
excluded from commits and retained in this directory.

Initializer comment correction: a new fixture exposed loss of
`/* initializer */` in the first interaction renderer.
`cargo test -p ridl-fmt ridl_initializer_comment_is_preserved --locked` exited
101 with `signal s: T = DEFAULT` instead of
`signal s: T = /* initializer */ DEFAULT`. The renderer now keeps a commented
initializer verbatim; the full formatter acceptance and invariants passed. This
was a formatter behavior failure, not a fixture or environment error.

CLI after evidence: `cargo build -p ridl-cli --locked` exited 0. The same stream
fixture formatted with both `<T>` and `[<T>; 1..2]` retained, exit 0; a second
`ridl fmt --check <fixture>` exited 0. Local logs: `task-06-cli-build.log` and
`task-06-stream-after.log`.

The first initializer-preservation attempt exited 101 with
`T= /* initializer */
DEFAULT`: the parser excludes trivia before `=` from
`InitValue`. Adding the canonical space before the verbatim initializer repairs
that separate spacing assertion. Intermediate log:
`task-06-intermediate-init-spacing.log`.
