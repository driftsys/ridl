# Task 9 evidence

Branch: `feat/387-fmt-rsdl`, stacked on `fix/387-fmt-annotation-comments` at
`0249fd1d2fce5970fc71c775eeba5bedb13b2f38`. Planned PR group: Tasks 9–10, with
distinct commits. D-H27–D-H29 record the grouping, layout and test choices.

## Tests first

Before production changes:

- `cargo test -p ridl-fmt --lib rsdl_ --locked`: exit 101; four new layout tests
  fail because system/component/distribution dispatch remains verbatim and
  machine-body attributes are not formatted. Log: `task-09-red-unit.log`.
  Broken-input and the pre-existing attribute renderer control pass.
- `cargo test -p ridl-cli --test facade fmt_ --locked`: exit 101; the canonical
  RSDL facade expectation and valid RSDL brace-glob test fail. Twelve other
  selected caller tests pass. Log: `task-09-red-cli.log`.
- `cargo test -p ridl-lsp --test server formatting_replaces_the_document --locked`:
  exit 101; request 18 still returns the raw component body. Log:
  `task-09-red-lsp.log`.
- `cargo test -p ridl-fmt --test corpus --test rsdl --locked`: exit 101;
  formatter input does not reach its new RSDL goldens. Log:
  `task-09-red-corpus.log`.

These are valid syntax and intended layout failures, not fixture parse errors.
The parser fixture is copied into formatter test data and remains unchanged.

## Acceptance

- `cargo test -p ridl-fmt --locked`: exit 0, 96 unit and 20 integration tests.
  Log: `task-09-green-fmt.log`. The three-profile parser corpus invariants run
  at widths 100, 60 and 40; all seven reference RSDL examples also compare
  syntax structure, content and fixed points at those widths.
- `cargo test -p ridl-cli --test facade --locked`: exit 0, 19 tests. Log:
  `task-09-green-cli.log`. The valid `.rsdl` brace-glob fixture distinguishes
  widths 100 and 40 and completes a second `--check` pass at each width.
- `cargo test -p ridl-lsp --locked`: exit 0, 17 unit and 65 server tests. Log:
  `task-09-green-lsp.log`. Request 18 returns the canonical component body;
  client indentation options remain ignored.

The header has 144 columns with `{`: 143 breaks and 144 stays inline. An empty
body adds the closing brace: 144 breaks and 145 stays inline. Tests also check
empty blocks, dotted references/keys, opening-brace comments excluded from
width, between-member comments, direct commented headers and commented attribute
or reference subtrees. Every exact-output assertion reparses, compares structure
and content/comment identity and checks the second pass. The private machine
renderer is checked at indent one and all three widths without adding public
machine or deployment dispatch.

An initial green run found one test expectation error: its commented reference
made the line exceed 40 columns, correctly breaking attributes. The placement
fixture was shortened rather than changing production width behavior. Log:
`task-09-first-green-width-expectation.log`. Final acceptance passes.

Logs are local and ignored. Task commit, QUICK results and publication gates are
recorded after completion of the change.

## Task commit and QUICK review

Task commit: `c2602dc126474fa6d29871da41bec2f390a08452`. `just test` exited 0 on
that exact head; log: `task-09-workspace-test.log`. `just fmt-check`,
`just lint` and `just check` also passed; lint log: `task-09-lint.log`.

All three required QUICK seats reviewed only `0249fd1` through `c2602dc` and
matched all 13 paths. Native docs/tests ran on actual Terra/high, verified by
rollout `turn_context` records. The fresh bug wrapper invoked the native
built-in at actual Terra/medium, verified at startup, and reported no findings;
its log does not show explicit reads of the requirement documents. The tests
seat also reported no findings. Native logs and review reports remain temporary.

The restricted docs seat found one Important stale statement: the CLI reference
said all RSDL declarations remain verbatim. D-H30 applies the factual correction
now for the three implemented declarations while retaining the current verbatim
deployment/machine status. This is a QUICK correction and receives no recursive
QUICK review. Full PR reviews follow the consecutive Task 10 commit.
