# Changelog

## [0.7.0] (2026-10-09)

### Features

- **ridl-diff:** match interface numbers within a unit ([fa332aa])
- **ridl-sem:** key the system regions on the unit ([25d80b7])
- **ridl-descriptor:** lower one catalog per unit with qualified interface names
  ([2c06099])
- **ridl-ir:** hash the reduced unit and name the catalog after the unit
  ([36062f6])
- **ridl:** write one interfaces.lock per unit with ridl lock ([36500e8])
- **ridl-sem:** number interfaces once per unit ([618e479])
- **ridl-ir:** record the unit of a package in the IR ([90813aa])
- **ridl-core:** read one interfaces.lock per unit ([839d017])
- **ridl-core:** refuse a source package claimed by two units (MANI-014)
  ([26b31fb])
- **ridl-core:** refuse a manifest inside a unit's tree (MANI-013) ([bbdd43c])
- **ridl-core:** accept a dotted relative name as an interface lock key
  ([83f504d])
- **ridl-core:** record the unit of every loaded package ([455d49f])
- **ridl-descriptor:** add a std feature, off for a no_std reader ([041dc61])
- **ridl-rt-conformance:** publish the port contract suite ([6572d4f])

### Documentation

- **repo:** say unit in the comments the per-unit lock made false ([d48d6ae])
- **docs:** say unit where the catalog is per unit ([eab32cd])
- **docs:** archive the catalog-per-unit spec and plan ([35f9684])
- **docs:** state the gate mechanism in the release note ([d8d5ed9])
- **docs:** draft the Kotlin heads-up and the release note for one catalog per
  unit ([d80bd7e])
- **docs:** qualify the baseline gate's deleted-package refusal ([5d4d44c])
- **docs:** describe one catalog per unit in the book ([7f6689f])
- **ridl:** scope the hash sentence and the number key to the unit ([cd68c1b])
- **adr:** scope the ADR-0015 interface number to the unit ([6e58bad])
- **ridl:** state the unit in the language references and design records
  ([72c1886])
- **adr:** record one catalog per unit in the decision records ([5541663])
- **ridl:** add the catalog-per-unit implementation handoff ([54b6a95])
- **ridl:** apply the plan review ([6475c8c])
- **ridl:** record the author's answers in the catalog-per-unit plan ([24d9ca0])
- **ridl:** plan one catalog per unit ([090539f])
- **ridl:** state that a types-only unit still gets generated types ([2dfacf3])
- **ridl:** note that a future .rxdl file fits a unit ([604aa26])
- **ridl:** fold the design review into the catalog-per-unit spec ([4c04bb5])
- **ridl:** settle baselines, plugins and the empty root package ([95e9ac6])
- **ridl:** design one catalog per unit ([726c1f0])
- **repo:** name the bare-metal target where the pin is quoted ([f8c65c1])
- **adr:** name the decision of the ridl-rt-conformance amendments ([307a9ac])
- **adr:** record that ridl-rt-conformance is published ([827a8c1])
- **docs:** wrap two long lines in the editor setup section ([5bd02f9])
- **docs:** correct install and agent setup details in the book ([d984947])
- **docs:** document installation and editor and agent setup in the book
  ([1cf820d])
- **docs:** correct the Timeout and illustration wording in the book ([e8e015c])
- **docs:** correct the generated-code and catalog-descriptor chapters
  ([ae09bbf])
- **docs:** add book chapters on the generated Rust code and the catalog
  descriptor ([22e70f5])

### Bug Fixes

- **ridl-ir:** keep the full name of a package outside its unit ([fed5f9c])
- **ridl:** compare a legacy baseline snapshot in the unit of its package
  ([04f8dd8])
- **ridl-core:** report MANI-014 for two members with one package name
  ([a84625e])
- **xtask:** count the descriptor round-trip test's three interface reads
  ([b75ba3b])
- **ridl-sem:** spell every retired entry as its lock key in the IR ([9ed0b8a])
- **ridl:** keep a number that moves to another package of its unit ([fe145b9])
- **ridl-descriptor:** pin the no_std surface and name the new checks
  ([b009792])
- **ridl-rt-conformance:** build on the minimum Rust of ridl-rt ([53488c4])
- **repo:** pin both halves of the book-check anchor test with fixtures
  ([e305f35])
- **repo:** make book-check fail on an include anchor that does not resolve
  ([254e5be])
- **repo:** let book-check resolve includes of examples/ sources ([b394dc6])

### BREAKING CHANGES

- ridl diff groups baseline snapshots by unit and matches interface numbers within the unit; a move between sibling packages is InterfaceRenamed.
- a region of the system artifact is one unit; region interface names are catalog names (cluster.Speed), and a plugin finds its region by Catalog.package.
- ridl build --emit catalog writes <unit>.catalog.binfb, one file per unit; interface names in the descriptor are qualified by the source package path relative to the unit.
- the IR's retired entries carry their lock key (catalog name), not the short name.
- every catalog hash changes; the reduced package is now the reduced unit and CATALOG.name is the unit name.
- ridl lock writes one interfaces.lock per unit, in the manifest directory, with catalog-name keys (cluster.Speed); it no longer writes a lock per source package directory.
- interface numbers are one space per unit and a declared interface's lock key is its source package path relative to the unit plus its name (cluster.Speed). Delete the per-package interfaces.lock files, run ridl lock, and publish a new baseline.
- an interfaces.lock in a subdirectory of a unit is no longer read; the unit's lock is the one beside its ridl.toml.

[0.7.0]: https://github.com/driftsys/ridl/compare/v0.6.0...v0.7.0
[fa332aa]: https://github.com/driftsys/ridl/commit/fa332aa
[25d80b7]: https://github.com/driftsys/ridl/commit/25d80b7
[2c06099]: https://github.com/driftsys/ridl/commit/2c06099
[36062f6]: https://github.com/driftsys/ridl/commit/36062f6
[36500e8]: https://github.com/driftsys/ridl/commit/36500e8
[618e479]: https://github.com/driftsys/ridl/commit/618e479
[90813aa]: https://github.com/driftsys/ridl/commit/90813aa
[839d017]: https://github.com/driftsys/ridl/commit/839d017
[26b31fb]: https://github.com/driftsys/ridl/commit/26b31fb
[bbdd43c]: https://github.com/driftsys/ridl/commit/bbdd43c
[83f504d]: https://github.com/driftsys/ridl/commit/83f504d
[455d49f]: https://github.com/driftsys/ridl/commit/455d49f
[041dc61]: https://github.com/driftsys/ridl/commit/041dc61
[6572d4f]: https://github.com/driftsys/ridl/commit/6572d4f
[d48d6ae]: https://github.com/driftsys/ridl/commit/d48d6ae
[eab32cd]: https://github.com/driftsys/ridl/commit/eab32cd
[35f9684]: https://github.com/driftsys/ridl/commit/35f9684
[d8d5ed9]: https://github.com/driftsys/ridl/commit/d8d5ed9
[d80bd7e]: https://github.com/driftsys/ridl/commit/d80bd7e
[5d4d44c]: https://github.com/driftsys/ridl/commit/5d4d44c
[7f6689f]: https://github.com/driftsys/ridl/commit/7f6689f
[cd68c1b]: https://github.com/driftsys/ridl/commit/cd68c1b
[6e58bad]: https://github.com/driftsys/ridl/commit/6e58bad
[72c1886]: https://github.com/driftsys/ridl/commit/72c1886
[5541663]: https://github.com/driftsys/ridl/commit/5541663
[54b6a95]: https://github.com/driftsys/ridl/commit/54b6a95
[6475c8c]: https://github.com/driftsys/ridl/commit/6475c8c
[24d9ca0]: https://github.com/driftsys/ridl/commit/24d9ca0
[090539f]: https://github.com/driftsys/ridl/commit/090539f
[2dfacf3]: https://github.com/driftsys/ridl/commit/2dfacf3
[604aa26]: https://github.com/driftsys/ridl/commit/604aa26
[4c04bb5]: https://github.com/driftsys/ridl/commit/4c04bb5
[95e9ac6]: https://github.com/driftsys/ridl/commit/95e9ac6
[726c1f0]: https://github.com/driftsys/ridl/commit/726c1f0
[f8c65c1]: https://github.com/driftsys/ridl/commit/f8c65c1
[307a9ac]: https://github.com/driftsys/ridl/commit/307a9ac
[827a8c1]: https://github.com/driftsys/ridl/commit/827a8c1
[5bd02f9]: https://github.com/driftsys/ridl/commit/5bd02f9
[d984947]: https://github.com/driftsys/ridl/commit/d984947
[1cf820d]: https://github.com/driftsys/ridl/commit/1cf820d
[e8e015c]: https://github.com/driftsys/ridl/commit/e8e015c
[ae09bbf]: https://github.com/driftsys/ridl/commit/ae09bbf
[22e70f5]: https://github.com/driftsys/ridl/commit/22e70f5
[fed5f9c]: https://github.com/driftsys/ridl/commit/fed5f9c
[04f8dd8]: https://github.com/driftsys/ridl/commit/04f8dd8
[a84625e]: https://github.com/driftsys/ridl/commit/a84625e
[b75ba3b]: https://github.com/driftsys/ridl/commit/b75ba3b
[9ed0b8a]: https://github.com/driftsys/ridl/commit/9ed0b8a
[fe145b9]: https://github.com/driftsys/ridl/commit/fe145b9
[b009792]: https://github.com/driftsys/ridl/commit/b009792
[53488c4]: https://github.com/driftsys/ridl/commit/53488c4
[e305f35]: https://github.com/driftsys/ridl/commit/e305f35
[254e5be]: https://github.com/driftsys/ridl/commit/254e5be
[b394dc6]: https://github.com/driftsys/ridl/commit/b394dc6

## [0.6.0] (2026-10-07)

### Refactoring

- **ridl-core:** carry command and query timing defaults through the manifest
  ([4bcb741])
- **ridl-ir:** move the payload sizer out of ridl-descriptor ([1c7ff96])

### Documentation

- **adr:** record the rejected claim span orders and correct a row ([782e7d7])
- **ridl-rt:** record that trace links std, and state the hook's contract
  ([56dc99c])
- **ridl-rt:** name generated code as the hook's caller ([a7550e1])
- **docs:** add the phase 1 implementation plan for generated observation
  ([92285db]), refs driftsys/ridl#754
- **docs:** record that the case 3 telemetry library consumes tracing spans
  ([3153736]), refs [#754]
- **docs:** resolve the trace context source with an application hook
  ([881000e]), refs [#754]
- **docs:** add the design for observation from the generated Rust face
  ([b7bac2b]), refs [#754]
- **ridl-ir:** state the limit of an in-place enum value in the compatibility
  rule ([48a98fc])
- **ridl-ir:** correct the pascal field comment on which readers are strict
  ([3055c67])
- **ridl-ir:** state that a second-phase request error has no position
  ([210bcf8])
- **ridl-ir:** say a new enum value needs a rebuilt plugin, in every statement
  ([2bce411])
- **docs:** state that an unknown enum name stays an error ([4cefe3d])
- **docs:** record that the codegen request reader ignores unknown keys
  ([855ff18])
- **docs:** garden the generated-file marker and header design and plan
  ([3b63166])
- **docs:** state that a package build names its package in the crate files'
  marker ([137581c])
- **docs:** record the generated-file marker and the codegen header ([234b6e2])
- **docs:** add the generated-file marker and licence header plan ([01d0de1]),
  refs 746.
- **docs:** name the trace field of ShortClaim in the design records
  ([2acef73]), refs [#758]
- **docs:** name the trace context in the loopback, technote and archive records
  ([7844947]), refs [#758]
- **ridl-rt:** list TraceContext under versioning and separate its module
  paragraph ([6398b57]), refs [#758]
- **adr:** record decision 21 as a breaking change under decision 10
  ([e8d482e]), refs [#758]
- **docs:** garden the trace context design and plan into the records
  ([ef59133]), refs [#752]
- **adr:** correct the trace context records after review ([11c8b6c]), refs
  [#752]
- **adr:** record the trace context on calls and events ([c37b8e8]), refs [#752]
- **docs:** add the trace context propagation plan ([74b3fdf])
- **docs:** add the design for trace context propagation in ridl-rt ([3ebe67c])
- **repo:** remove the remaining story references and reflow edited comments
  ([b19c227])
- **repo:** remove story ids of the form "E<n> task <n>" from comments
  ([4bd57fe])
- **docs:** correct stale default-bound comments and record the ADR amendments
  ([20178c7])
- **docs:** garden the default response bound design and plan ([007a72d])
- **docs:** remove the last statements that a command or query has no default
  bound ([3c446ad])
- **ridl-core:** name every [defaults] key in the workspace loader comments
  ([55f7ca5])
- **ridl-rt:** describe call_deadline under the default response bound
  ([dbd2bd4])
- **docs:** describe the default response bound of commands and queries
  ([6d3f4d2])
- **adr:** amend ADR-0015 so commands and queries take a default response bound
  ([a3d6758])
- **ridl-diff:** update RPC bound text for a default that can apply ([cea3440])
- **docs:** add the implementation plan for a default RPC response bound
  ([e01a573])
- **docs:** add the design note for a default RPC response bound ([a21753c])
- **ridlc:** wrap the codegen_request rustdoc at the file's width ([abfd11c])
- **docs:** state what the Rust backend reads in the record's opening and in
  ridlc's rustdoc ([d69c346])
- **docs:** correct what the Rust backend reads and when the request is built
  ([b965add])
- **docs:** match the in-process host's record to the code and state how the
  request nests the model ([836d0ec]), fixes 744., 745.
- **docs:** add the generated-file marker and licence header design ([95ea0c9])
- **docs:** word the schema-copy rule so that it stays true after a release
  ([e129c08])
- **docs:** state which schema files a release tag holds and how the request
  nests the model ([bbae978])
- **docs:** add the chapter "Writing a codegen plugin" to the book ([765cdf4])
- **docs:** correct five book statements that describe an older state
  ([fa8eb36])
- **docs:** bring four records in line with the design lints, the MCP tools and
  evals ([#742]) ([52aac58])
- **docs:** correct the layout-inputs records after the second review
  ([23da72a])
- **docs:** name the layout test plugin as the only plugin that reads the
  deployment section ([b3b4edc])
- **adr:** date the rewrite of ADR-0022's decision 3 note ([dad602c])
- **roadmap:** state that the layout proof's messages carry the proto3 bound
  only ([caba173])
- **docs:** correct the layout-inputs records against the test plugin
  ([8c01bff])
- **docs:** point the archived layout-inputs plan at the archived design
  ([4dd41c7])
- **docs:** garden the layout-inputs design and plan ([fe74aa1])
- **roadmap:** repair the E17.2 row of the landed record ([6ef52bf])
- **roadmap:** record the layout inputs for backend plugins as built ([68647a5])
- **rsdl:** state the depth pass's package dependency and the diff's silence on
  sizing ([fc7bfbb])
- **rsdl:** specify the depth, slots and budget keys ([71614c2])
- **docs:** log the delegated decisions of the two depth warnings ([bd2c62d])
- **docs:** log the delegated decisions of the rsdl sizing keys ([ddaf9cc])
- **docs:** record the deployment section's delegated decisions ([07f6410])
- **docs:** correct the deployment section record after review ([c625070])
- **docs:** record the deployment section of the codegen request ([2617c38])
- **docs:** record the size states' delegated decisions ([ddd9336])
- **docs:** layout inputs for backend plugins, the design and the plan ([#722])
  ([43fbd10])
- **roadmap:** add layout inputs for backend plugins as priority 1 and order the
  P2 devex track ([#721]) ([fac5abc])
- **roadmap:** garden lane E16's working memory into the records and the archive
  ([#702]) ([112da94])
- **roadmap:** point the E16 driver at stage D8 and record D7's decisions
  ([#698]) ([ebad724])
- **docs:** specify design lints and the eval seed ([#694]) ([b49a8c9])
- **roadmap:** point the E16 driver at stage D7 and record D6's decisions
  ([#695]) ([ed6e34d])
- **roadmap:** point the E16 driver at stage D6 and record D5's decisions
  ([#691]) ([c1e6335])
- **adr:** garden the workspace-aware MCP tools (1a) into ADR-0025 ([#689])
  ([d972d68])
- **repo:** list ADR-0024 in the AGENTS.md reading map ([f3f1298])
- **roadmap:** point the E16 driver at stage D5 and record D4's decisions
  ([#685]) ([a622a1f])
- **adr:** garden the lint foundation (spec 0) into ADR-0024 ([#683])
  ([8a57f68])
- **roadmap:** point the E16 driver at stage D4 and record D3's decisions
  ([#680]) ([fffde63])
- **roadmap:** record the E16 decisions taken under delegation ([#674])
  ([4a99c6e])
- **roadmap:** design and plan for the lint foundation (spec 0) ([#671])
  ([69670a8])
- **roadmap:** rsdl uses in the MCP review tools, and the follow-up plan
  ([#673]) ([ac60f62])
- **roadmap:** point the E16 driver at stage D3 ([#672]) ([99cb616])
- **roadmap:** re-baseline the catalog descriptor plan against main (E16 D1)
  ([#667]) ([9995826])
- **roadmap:** record the answers to the E16 driver's eleven questions ([#666])
  ([440dfb5])
- **roadmap:** design and plan for the workspace-aware MCP tools ([#664])
  ([fd05240])
- **roadmap:** add the driver for lane E16, the catalog descriptor ([#662])
  ([472c4f8])
- **roadmap:** add the scope brief for the devex and agent tracks ([#656])
  ([29b221a])

### Features

- **ridl-rt:** add the trace-context propagation hook ([fadd91c])
- **ridlc:** mark the crate files, allow the emitter's two clippy lints, and
  gate the list ([54d1ad9])
- **ridlc:** write the generated-file marker and the project header ([10bbe90])
- **ridl-core:** read the codegen header file named in the manifest ([76c4449])
- **ridl-ir:** carry the generated-file marker and header in the codegen request
  ([7ca995c])
- **ridl-rt:** carry the trace context on ReadError::ShortClaim ([7abf9b0]),
  refs [#758]
- **ridl-loopback:** deliver the sender's trace context on claims and
  occurrences ([3e160c0]), refs [#752]
- **ridl-rt:** carry an optional trace context on calls and events ([85bd8da]),
  refs [#752]
- **ridl-sem:** default the response bound of untimed commands and queries
  ([26c7049])
- **ridl-diff:** classify a defaulted RPC bound made explicit as compatible
  ([9acc91d])
- **ridl-sem:** parse response-bound defaults for commands and queries
  ([35cec3f])
- **ridlc:** calibrate the design lints on the evals corpus and garden piece 1b
  ([#738]) ([54a34bf])
- **ridl-sem:** warn on a depth below the contract bound and on an underivable
  depth ([d99f698])
- **ridl-ir:** resolve the declared sizing values into each consumer link
  ([b7485ba])
- **ridl-ir:** carry the declared sizing values in the system IR ([9dd202d])
- **ridl-sem:** read the depth, slots and budget keys on deployments and
  placement lines ([0cc59b8])
- **ridl-ir:** carry the known transport bindings' overheads in the deployment
  section ([e0c4fb0])
- **ridlc:** select a deployment and carry it in every codegen request
  ([e9000ef])
- **ridl-ir:** emit the deployment section from the lowered system ([36ba6fe])
- **ridl-ir:** derive an event ring depth from its timing bounds ([d4e872f])
- **ridl-ir:** add the deployment section to the codegen request schema
  ([2ac4dd0])
- **repo:** give examples/cabin a system and a deployment ([303fbc0])
- **ridl-ir:** tabulate the reservation and the table budget per encoding
  ([f5d8cb4])
- **ridl-ir:** size every request and reply shape, absent with a cause where no
  codec defines it ([c7b17e4])
- **ridl-ir:** carry a size state per encoding on every payload ([3ffca3f])
- **ridlc:** add workspace design lints and metrics ([#712]) ([8ef2f28])
- **ridl-sem:** documentation in the source (spec 2a) ([#703]) ([5964c8f]),
  closes [#529]
- **repo:** add the design eval corpus and task seed ([#707]) ([d694fba])
- **ridlc:** embed the catalog hash in each region of the lowered system (E6.17)
  ([#699]) ([eaeb3a5]), closes 367.
- **ridl:** ridl describe and the catalog descriptor's JSON view (E16.6)
  ([#696]) ([681e666])
- **ridl-descriptor:** the lowering, --emit catalog and the port's catalog check
  (E16.5) ([#692]) ([a8ad508])
- **ridl-descriptor:** the proto3 and FlatBuffers size state per payload (E16.4)
  ([#686]) ([d317c96])
- **ridl-mcp:** rsdl uses in the review tools, and review follow-ups ([#677])
  ([a87599a])
- **ridl-descriptor:** the size context, the type leaves and the string byte
  capacity (E16.3) ([#681]) ([eb36d16])
- lint foundation — named lints, [lints] in ridl.toml, SARIF (spec 0) ([#678])
  ([fe30c4f])
- **ridl-descriptor:** interface numbers and the catalog hash (E16.2) ([#676])
  ([742c0a3]), refs 378., 378.
- **ridl-descriptor:** the catalog descriptor crate, schema and verifier (E16.1)
  ([#669]) ([004ca06]), refs [#377]
- **ridl-mcp:** workspace-aware MCP tools (1a) ([#668]) ([edeec6e])

### Bug Fixes

- **ridl-ir:** keep an unknown enum name an error in the request reader
  ([5adbe70])
- **ridl-ir:** ignore unknown keys in the codegen request reader ([27c53b1])
- **ridl-ir:** refuse every Unicode control and line-separator character in the
  header file ([b63053d])
- **ridl-core:** refuse a control character in the codegen header file
  ([4b1127a])
- **ridl-lsp:** name only the maximum as defaulted on an RPC hover with a
  minimum ([c7d6f47])
- **ridl-sem:** name the default as the source of a filled maximum ([57d42bd])
- **ridl-sem:** state the RPC default in the IR schema and pin its edges
  ([30e06e9])
- **ridl-sem:** name the deployment in the depth warnings and pin the review
  findings ([1e1e953])
- **ridl-ir:** pin the deployment section's behaviour and select by what the
  source declares ([8982c4d])
- **ridlc:** report a dropped deployment's error and pin the selection
  ([f30826d])
- **ridl-ir:** key a route's interface on its name alone ([31a3a75])
- **ridl-ir:** pin the padded scale and treat a zero depth as not derivable
  ([0f72d7f])
- **repo:** align the cabin service line with the formatter and cite the durable
  record ([bbd6f96])
- **ridl-ir:** pin the size states and mark their breaking changes ([02f632e])

### BREAKING CHANGES

- every ReadError::ShortClaim literal, and every pattern
that names its fields without `..`, gains the `trace` field.
- the three send methods of the port traits take one more
argument, and Claim and RawOccurrence have one more field.
- every command and query now carries a response bound in
the IR. An untimed command lapses after 1 s and an untimed query after 3 s
unless [defaults] command_timing or query_timing says otherwise. Catalogs
with untimed commands or queries get a new catalog hash, so a provider and
its clients must be rebuilt together.
- two public structs of `ridlc` gain a field. `CliRun`
gains `usage_error: bool`, which carries the exit-code-2 outcome a bad
`--deployment` value now has instead of an I/O error, and
`WorkspaceOutput` gains `declared_deployments: Vec<String>`, the
deployment names the source declares. Neither struct is
`#[non_exhaustive]`, so a downstream crate that builds either with a
struct literal, or matches one exhaustively, no longer compiles; adding
the field to the literal is the fix. `UnknownDeployment`, also public,
gains `has_system: bool` for the same reason, and `select_deployment`
takes the declared names as a new second argument — both of those
shipped first in this release series, so only the two structs above
affect code written against `v0.5.1`.
- `ridl_descriptor::size` is gone. Its contents are
- a member entry point now loads its workspace. The
root's `[lints]`, `[defaults].timing` and `[imports]` apply, a member
build fails on another member's error, the generated crate of a member
build is named `ridl_generated`, and the lockfile is the root's.
- a generated Client, blocking Client or Publisher bound
to a port attached to a catalog other than the one the face was
generated from, and a serve over such a handler port, now panics. Until
this change such a face read and wrote another interface's slots with
no error. Attach the runtime to the generated CATALOG, or compare
port.catalog() with <Iface as Interface>::CATALOG before binding.

[0.6.0]: https://github.com/driftsys/ridl/compare/v0.5.1...v0.6.0
[4bcb741]: https://github.com/driftsys/ridl/commit/4bcb741
[1c7ff96]: https://github.com/driftsys/ridl/commit/1c7ff96
[782e7d7]: https://github.com/driftsys/ridl/commit/782e7d7
[56dc99c]: https://github.com/driftsys/ridl/commit/56dc99c
[a7550e1]: https://github.com/driftsys/ridl/commit/a7550e1
[92285db]: https://github.com/driftsys/ridl/commit/92285db
[3153736]: https://github.com/driftsys/ridl/commit/3153736
[#754]: https://github.com/driftsys/ridl/issues/754
[881000e]: https://github.com/driftsys/ridl/commit/881000e
[b7bac2b]: https://github.com/driftsys/ridl/commit/b7bac2b
[48a98fc]: https://github.com/driftsys/ridl/commit/48a98fc
[3055c67]: https://github.com/driftsys/ridl/commit/3055c67
[210bcf8]: https://github.com/driftsys/ridl/commit/210bcf8
[2bce411]: https://github.com/driftsys/ridl/commit/2bce411
[4cefe3d]: https://github.com/driftsys/ridl/commit/4cefe3d
[855ff18]: https://github.com/driftsys/ridl/commit/855ff18
[3b63166]: https://github.com/driftsys/ridl/commit/3b63166
[137581c]: https://github.com/driftsys/ridl/commit/137581c
[234b6e2]: https://github.com/driftsys/ridl/commit/234b6e2
[01d0de1]: https://github.com/driftsys/ridl/commit/01d0de1
[2acef73]: https://github.com/driftsys/ridl/commit/2acef73
[#758]: https://github.com/driftsys/ridl/issues/758
[7844947]: https://github.com/driftsys/ridl/commit/7844947
[6398b57]: https://github.com/driftsys/ridl/commit/6398b57
[e8d482e]: https://github.com/driftsys/ridl/commit/e8d482e
[ef59133]: https://github.com/driftsys/ridl/commit/ef59133
[#752]: https://github.com/driftsys/ridl/issues/752
[11c8b6c]: https://github.com/driftsys/ridl/commit/11c8b6c
[c37b8e8]: https://github.com/driftsys/ridl/commit/c37b8e8
[74b3fdf]: https://github.com/driftsys/ridl/commit/74b3fdf
[3ebe67c]: https://github.com/driftsys/ridl/commit/3ebe67c
[b19c227]: https://github.com/driftsys/ridl/commit/b19c227
[4bd57fe]: https://github.com/driftsys/ridl/commit/4bd57fe
[20178c7]: https://github.com/driftsys/ridl/commit/20178c7
[007a72d]: https://github.com/driftsys/ridl/commit/007a72d
[3c446ad]: https://github.com/driftsys/ridl/commit/3c446ad
[55f7ca5]: https://github.com/driftsys/ridl/commit/55f7ca5
[dbd2bd4]: https://github.com/driftsys/ridl/commit/dbd2bd4
[6d3f4d2]: https://github.com/driftsys/ridl/commit/6d3f4d2
[a3d6758]: https://github.com/driftsys/ridl/commit/a3d6758
[cea3440]: https://github.com/driftsys/ridl/commit/cea3440
[e01a573]: https://github.com/driftsys/ridl/commit/e01a573
[a21753c]: https://github.com/driftsys/ridl/commit/a21753c
[abfd11c]: https://github.com/driftsys/ridl/commit/abfd11c
[d69c346]: https://github.com/driftsys/ridl/commit/d69c346
[b965add]: https://github.com/driftsys/ridl/commit/b965add
[836d0ec]: https://github.com/driftsys/ridl/commit/836d0ec
[95ea0c9]: https://github.com/driftsys/ridl/commit/95ea0c9
[e129c08]: https://github.com/driftsys/ridl/commit/e129c08
[bbae978]: https://github.com/driftsys/ridl/commit/bbae978
[765cdf4]: https://github.com/driftsys/ridl/commit/765cdf4
[fa8eb36]: https://github.com/driftsys/ridl/commit/fa8eb36
[52aac58]: https://github.com/driftsys/ridl/commit/52aac58
[#742]: https://github.com/driftsys/ridl/issues/742
[23da72a]: https://github.com/driftsys/ridl/commit/23da72a
[b3b4edc]: https://github.com/driftsys/ridl/commit/b3b4edc
[dad602c]: https://github.com/driftsys/ridl/commit/dad602c
[caba173]: https://github.com/driftsys/ridl/commit/caba173
[8c01bff]: https://github.com/driftsys/ridl/commit/8c01bff
[4dd41c7]: https://github.com/driftsys/ridl/commit/4dd41c7
[fe74aa1]: https://github.com/driftsys/ridl/commit/fe74aa1
[6ef52bf]: https://github.com/driftsys/ridl/commit/6ef52bf
[68647a5]: https://github.com/driftsys/ridl/commit/68647a5
[fc7bfbb]: https://github.com/driftsys/ridl/commit/fc7bfbb
[71614c2]: https://github.com/driftsys/ridl/commit/71614c2
[bd2c62d]: https://github.com/driftsys/ridl/commit/bd2c62d
[ddaf9cc]: https://github.com/driftsys/ridl/commit/ddaf9cc
[07f6410]: https://github.com/driftsys/ridl/commit/07f6410
[c625070]: https://github.com/driftsys/ridl/commit/c625070
[2617c38]: https://github.com/driftsys/ridl/commit/2617c38
[ddd9336]: https://github.com/driftsys/ridl/commit/ddd9336
[43fbd10]: https://github.com/driftsys/ridl/commit/43fbd10
[#722]: https://github.com/driftsys/ridl/issues/722
[fac5abc]: https://github.com/driftsys/ridl/commit/fac5abc
[#721]: https://github.com/driftsys/ridl/issues/721
[112da94]: https://github.com/driftsys/ridl/commit/112da94
[#702]: https://github.com/driftsys/ridl/issues/702
[ebad724]: https://github.com/driftsys/ridl/commit/ebad724
[#698]: https://github.com/driftsys/ridl/issues/698
[b49a8c9]: https://github.com/driftsys/ridl/commit/b49a8c9
[#694]: https://github.com/driftsys/ridl/issues/694
[ed6e34d]: https://github.com/driftsys/ridl/commit/ed6e34d
[#695]: https://github.com/driftsys/ridl/issues/695
[c1e6335]: https://github.com/driftsys/ridl/commit/c1e6335
[#691]: https://github.com/driftsys/ridl/issues/691
[d972d68]: https://github.com/driftsys/ridl/commit/d972d68
[#689]: https://github.com/driftsys/ridl/issues/689
[f3f1298]: https://github.com/driftsys/ridl/commit/f3f1298
[a622a1f]: https://github.com/driftsys/ridl/commit/a622a1f
[#685]: https://github.com/driftsys/ridl/issues/685
[8a57f68]: https://github.com/driftsys/ridl/commit/8a57f68
[#683]: https://github.com/driftsys/ridl/issues/683
[fffde63]: https://github.com/driftsys/ridl/commit/fffde63
[#680]: https://github.com/driftsys/ridl/issues/680
[4a99c6e]: https://github.com/driftsys/ridl/commit/4a99c6e
[#674]: https://github.com/driftsys/ridl/issues/674
[69670a8]: https://github.com/driftsys/ridl/commit/69670a8
[#671]: https://github.com/driftsys/ridl/issues/671
[ac60f62]: https://github.com/driftsys/ridl/commit/ac60f62
[#673]: https://github.com/driftsys/ridl/issues/673
[99cb616]: https://github.com/driftsys/ridl/commit/99cb616
[#672]: https://github.com/driftsys/ridl/issues/672
[9995826]: https://github.com/driftsys/ridl/commit/9995826
[#667]: https://github.com/driftsys/ridl/issues/667
[440dfb5]: https://github.com/driftsys/ridl/commit/440dfb5
[#666]: https://github.com/driftsys/ridl/issues/666
[fd05240]: https://github.com/driftsys/ridl/commit/fd05240
[#664]: https://github.com/driftsys/ridl/issues/664
[472c4f8]: https://github.com/driftsys/ridl/commit/472c4f8
[#662]: https://github.com/driftsys/ridl/issues/662
[29b221a]: https://github.com/driftsys/ridl/commit/29b221a
[#656]: https://github.com/driftsys/ridl/issues/656
[fadd91c]: https://github.com/driftsys/ridl/commit/fadd91c
[54d1ad9]: https://github.com/driftsys/ridl/commit/54d1ad9
[10bbe90]: https://github.com/driftsys/ridl/commit/10bbe90
[76c4449]: https://github.com/driftsys/ridl/commit/76c4449
[7ca995c]: https://github.com/driftsys/ridl/commit/7ca995c
[7abf9b0]: https://github.com/driftsys/ridl/commit/7abf9b0
[3e160c0]: https://github.com/driftsys/ridl/commit/3e160c0
[85bd8da]: https://github.com/driftsys/ridl/commit/85bd8da
[26c7049]: https://github.com/driftsys/ridl/commit/26c7049
[9acc91d]: https://github.com/driftsys/ridl/commit/9acc91d
[35cec3f]: https://github.com/driftsys/ridl/commit/35cec3f
[54a34bf]: https://github.com/driftsys/ridl/commit/54a34bf
[#738]: https://github.com/driftsys/ridl/issues/738
[d99f698]: https://github.com/driftsys/ridl/commit/d99f698
[b7485ba]: https://github.com/driftsys/ridl/commit/b7485ba
[9dd202d]: https://github.com/driftsys/ridl/commit/9dd202d
[0cc59b8]: https://github.com/driftsys/ridl/commit/0cc59b8
[e0c4fb0]: https://github.com/driftsys/ridl/commit/e0c4fb0
[e9000ef]: https://github.com/driftsys/ridl/commit/e9000ef
[36ba6fe]: https://github.com/driftsys/ridl/commit/36ba6fe
[d4e872f]: https://github.com/driftsys/ridl/commit/d4e872f
[2ac4dd0]: https://github.com/driftsys/ridl/commit/2ac4dd0
[303fbc0]: https://github.com/driftsys/ridl/commit/303fbc0
[f5d8cb4]: https://github.com/driftsys/ridl/commit/f5d8cb4
[c7b17e4]: https://github.com/driftsys/ridl/commit/c7b17e4
[3ffca3f]: https://github.com/driftsys/ridl/commit/3ffca3f
[8ef2f28]: https://github.com/driftsys/ridl/commit/8ef2f28
[#712]: https://github.com/driftsys/ridl/issues/712
[5964c8f]: https://github.com/driftsys/ridl/commit/5964c8f
[#703]: https://github.com/driftsys/ridl/issues/703
[#529]: https://github.com/driftsys/ridl/issues/529
[d694fba]: https://github.com/driftsys/ridl/commit/d694fba
[#707]: https://github.com/driftsys/ridl/issues/707
[eaeb3a5]: https://github.com/driftsys/ridl/commit/eaeb3a5
[#699]: https://github.com/driftsys/ridl/issues/699
[681e666]: https://github.com/driftsys/ridl/commit/681e666
[#696]: https://github.com/driftsys/ridl/issues/696
[a8ad508]: https://github.com/driftsys/ridl/commit/a8ad508
[#692]: https://github.com/driftsys/ridl/issues/692
[d317c96]: https://github.com/driftsys/ridl/commit/d317c96
[#686]: https://github.com/driftsys/ridl/issues/686
[a87599a]: https://github.com/driftsys/ridl/commit/a87599a
[#677]: https://github.com/driftsys/ridl/issues/677
[eb36d16]: https://github.com/driftsys/ridl/commit/eb36d16
[#681]: https://github.com/driftsys/ridl/issues/681
[fe30c4f]: https://github.com/driftsys/ridl/commit/fe30c4f
[#678]: https://github.com/driftsys/ridl/issues/678
[742c0a3]: https://github.com/driftsys/ridl/commit/742c0a3
[#676]: https://github.com/driftsys/ridl/issues/676
[004ca06]: https://github.com/driftsys/ridl/commit/004ca06
[#669]: https://github.com/driftsys/ridl/issues/669
[#377]: https://github.com/driftsys/ridl/issues/377
[edeec6e]: https://github.com/driftsys/ridl/commit/edeec6e
[#668]: https://github.com/driftsys/ridl/issues/668
[5adbe70]: https://github.com/driftsys/ridl/commit/5adbe70
[27c53b1]: https://github.com/driftsys/ridl/commit/27c53b1
[b63053d]: https://github.com/driftsys/ridl/commit/b63053d
[4b1127a]: https://github.com/driftsys/ridl/commit/4b1127a
[c7d6f47]: https://github.com/driftsys/ridl/commit/c7d6f47
[57d42bd]: https://github.com/driftsys/ridl/commit/57d42bd
[30e06e9]: https://github.com/driftsys/ridl/commit/30e06e9
[1e1e953]: https://github.com/driftsys/ridl/commit/1e1e953
[8982c4d]: https://github.com/driftsys/ridl/commit/8982c4d
[f30826d]: https://github.com/driftsys/ridl/commit/f30826d
[31a3a75]: https://github.com/driftsys/ridl/commit/31a3a75
[0f72d7f]: https://github.com/driftsys/ridl/commit/0f72d7f
[bbd6f96]: https://github.com/driftsys/ridl/commit/bbd6f96
[02f632e]: https://github.com/driftsys/ridl/commit/02f632e

## [0.5.1] (2026-10-03)

### Bug Fixes

- **family:** enforce payload constraints and initial value validity ([#654])
  ([6c3def2]), closes [#469], 421
  M2 of #172 is fixed; the remaining roll-up items stay open.
- **ridl-fmt:** preserve colliding annotation comments ([#635]) ([0bc48da])
- **ridl-sem:** cache the regex crate's TYPL-220 verdict by pattern text
  ([#621]) ([d534ed2])
- RIDL-149 citation, Spellings field numbers, typl and ridl
  catalogue-to-reference test ([#618]) ([d4a0007]), closes [#456], refs [#561],
  closes [#575], [#561], refs [#575]
- **ridl-lsp:** treat a bare carriage return as a line break ([#622])
  ([f045b8f])
- **ridl:** accept --stdio and --clientProcessId on ridl lsp ([#620])
  ([5df89ac])

### Performance

- **ridl-fmt:** render tuple breaks incrementally ([#651]) ([d9aa42c])
- **ridl-fmt:** index break candidates by physical line ([#640]) ([85a2d01])

### Features

- **ridl-fmt:** complete RIDL and RSDL canonical formatting ([#638]) ([1caf85a])
- **ridl-fmt:** format RIDL interfaces, attributes and services ([#634])
  ([c9c7c0e])
- **ridl-fmt:** apply per-file EditorConfig width and document layout ([#630])
  ([037256d])
- **ridl-fmt:** resolve width options from EditorConfig ([#628]) ([9f0b953])
- **ridl-fmt:** add formatting options and tuple line width ([#626]) ([43debd6])
- **ridl-lsp:** format a document with the ridl fmt engine ([#617]) ([bf389b4])

### Documentation

- **roadmap:** capture prioritized issue backlog ([#655]) ([4a3d470])
- **repo:** design portable typl match patterns ([#597]) ([#615]) ([aeb46c8]),
  refs [#597]
- **ridl-fmt:** design the layout of the ridl and rsdl declarations ([#624])
  ([169dc4c])

[0.5.1]: https://github.com/driftsys/ridl/compare/v0.5.0...v0.5.1
[6c3def2]: https://github.com/driftsys/ridl/commit/6c3def2
[#654]: https://github.com/driftsys/ridl/issues/654
[#469]: https://github.com/driftsys/ridl/issues/469
[0bc48da]: https://github.com/driftsys/ridl/commit/0bc48da
[#635]: https://github.com/driftsys/ridl/issues/635
[d534ed2]: https://github.com/driftsys/ridl/commit/d534ed2
[#621]: https://github.com/driftsys/ridl/issues/621
[d4a0007]: https://github.com/driftsys/ridl/commit/d4a0007
[#618]: https://github.com/driftsys/ridl/issues/618
[#456]: https://github.com/driftsys/ridl/issues/456
[#561]: https://github.com/driftsys/ridl/issues/561
[#575]: https://github.com/driftsys/ridl/issues/575
[f045b8f]: https://github.com/driftsys/ridl/commit/f045b8f
[#622]: https://github.com/driftsys/ridl/issues/622
[5df89ac]: https://github.com/driftsys/ridl/commit/5df89ac
[#620]: https://github.com/driftsys/ridl/issues/620
[d9aa42c]: https://github.com/driftsys/ridl/commit/d9aa42c
[#651]: https://github.com/driftsys/ridl/issues/651
[85a2d01]: https://github.com/driftsys/ridl/commit/85a2d01
[#640]: https://github.com/driftsys/ridl/issues/640
[1caf85a]: https://github.com/driftsys/ridl/commit/1caf85a
[#638]: https://github.com/driftsys/ridl/issues/638
[c9c7c0e]: https://github.com/driftsys/ridl/commit/c9c7c0e
[#634]: https://github.com/driftsys/ridl/issues/634
[037256d]: https://github.com/driftsys/ridl/commit/037256d
[#630]: https://github.com/driftsys/ridl/issues/630
[9f0b953]: https://github.com/driftsys/ridl/commit/9f0b953
[#628]: https://github.com/driftsys/ridl/issues/628
[43debd6]: https://github.com/driftsys/ridl/commit/43debd6
[#626]: https://github.com/driftsys/ridl/issues/626
[bf389b4]: https://github.com/driftsys/ridl/commit/bf389b4
[#617]: https://github.com/driftsys/ridl/issues/617
[4a3d470]: https://github.com/driftsys/ridl/commit/4a3d470
[#655]: https://github.com/driftsys/ridl/issues/655
[aeb46c8]: https://github.com/driftsys/ridl/commit/aeb46c8
[#597]: https://github.com/driftsys/ridl/issues/597
[#615]: https://github.com/driftsys/ridl/issues/615
[169dc4c]: https://github.com/driftsys/ridl/commit/169dc4c
[#624]: https://github.com/driftsys/ridl/issues/624

## [0.5.0] (2026-10-01)

### Features

- **ridl-backend-rust:** refuse two ridl-derived names that one Rust namespace
  cannot hold ([#612]) ([0df98ff]), closes [#449], [#453], [#455]
- **ridl-backend-rust:** rename backend-chosen names a ridl name could reach
  ([#608]) ([b325a38]), closes [#583], [#587], [#588], [#423]
- **ridl-loopback:** add Loopback::attach, a second aggregate on the same store
  ([#593]) ([c3cb412]), refs [#488], [#488], [#488], [#488], [#488]

### Refactoring

- **repo:** run the cabin example over one runtime with long-lived faces
  ([#599]) ([b859634]), closes [#488], [#484]

### Bug Fixes

- **ridl-backend-rust:** name a type hidden by its child package through
  __ridl_package ([#611]) ([02b3401]), closes 416
Part of #424
- **ridl-diff:** classify an appended field breaking unless an old payload stays
  readable ([#609]) ([1efc4e6]), closes [#598]
- **ridl-sem:** refuse a tuple field name declared twice (TYPL-215) ([#606])
  ([623975c]), refs [#449]
- **ridl-backend-flatbuffers:** read an omitted FlatBuffers default as the
  default and give an optional scalar `= null` ([#604]) ([ea92239])
- **ridl-sem:** refuse a match pattern the Rust regex crate cannot compile
  (TYPL-220) ([#601]) ([a776736]), refs [#437], closes [#437], refs [#437],
  [#437], [#437]
- **ridl-ir:** measure Closure.reaches_foreign from Scope.package ([#600])
  ([c624bd2])
- **ridl-ir:** set TypeRef.foreign from the declaring package, not the reference
  text ([#592]) ([255938e]), closes [#586]
- **ridl-sem:** refuse a non-integer enumset bit position (TYPL-219) ([#591])
  ([68e0c57]), closes [#579]
- **ci:** make link-check fail closed on a broken or empty file listing ([#595])
  ([4045f74]), closes 427.

### Documentation

- **repo:** garden the generated-name collision design ([#613]) ([73e37cc])
- **ridl-backend-rust:** design the generated-name collision fix ([#596])
  ([fd87978]), refs [#583], [#587], [#588], [#423], [#449], [#453], [#455],
  [#416], [#424]

### BREAKING CHANGES

- `ridl diff` now exits 1 on a non-optional struct field
appended whose type does not allow the value 0 (a string, bytes, struct,
union, tuple, array or map field, a scalar whose range or step excludes
0, an enum with no zero member, or an unresolved type). Such a diff
exited 0 before. Declare the new field optional to keep it compatible.
- a consumer that calls `view.bytes()` on a generated
FlatBuffers view adds `use ridl_rt::payload::View;`. The `Wire` alias is
removed; a consumer writes `ridl_rt::encoding::FlatBuffers`. A source name
`self_`, `Self_`, `super_` or `crate_` now emits one more `_` (`self__`).
- `ridl check` and `ridl build` now refuse a tuple that
declares the same field name twice, such as `(a : Speed, a : Speed)`.
The generated Rust (E0124) and TypeScript (TS2300) for such a source
already failed to compile, but `ridl build --emit proto`, `--emit
flatbuffers` and `--emit ir-json` succeeded, because those backends
name a tuple's fields by position. Rename one of the two fields.
- the emitted `.fbs` text changes for every optional
scalar or enum field. Code a foreign consumer generates from the schema
reads such a field as an optional value (planus: `Option<T>`), and
reads an old buffer whose field was omitted at 0 as absent rather than
as 0. The bytes this codec writes do not change.

[0.5.0]: https://github.com/driftsys/ridl/compare/v0.4.0...v0.5.0
[0df98ff]: https://github.com/driftsys/ridl/commit/0df98ff
[#612]: https://github.com/driftsys/ridl/issues/612
[#449]: https://github.com/driftsys/ridl/issues/449
[#453]: https://github.com/driftsys/ridl/issues/453
[#455]: https://github.com/driftsys/ridl/issues/455
[b325a38]: https://github.com/driftsys/ridl/commit/b325a38
[#608]: https://github.com/driftsys/ridl/issues/608
[#583]: https://github.com/driftsys/ridl/issues/583
[#587]: https://github.com/driftsys/ridl/issues/587
[#588]: https://github.com/driftsys/ridl/issues/588
[#423]: https://github.com/driftsys/ridl/issues/423
[c3cb412]: https://github.com/driftsys/ridl/commit/c3cb412
[#593]: https://github.com/driftsys/ridl/issues/593
[#488]: https://github.com/driftsys/ridl/issues/488
[b859634]: https://github.com/driftsys/ridl/commit/b859634
[#599]: https://github.com/driftsys/ridl/issues/599
[#484]: https://github.com/driftsys/ridl/issues/484
[02b3401]: https://github.com/driftsys/ridl/commit/02b3401
[#611]: https://github.com/driftsys/ridl/issues/611
[1efc4e6]: https://github.com/driftsys/ridl/commit/1efc4e6
[#609]: https://github.com/driftsys/ridl/issues/609
[#598]: https://github.com/driftsys/ridl/issues/598
[623975c]: https://github.com/driftsys/ridl/commit/623975c
[#606]: https://github.com/driftsys/ridl/issues/606
[ea92239]: https://github.com/driftsys/ridl/commit/ea92239
[#604]: https://github.com/driftsys/ridl/issues/604
[a776736]: https://github.com/driftsys/ridl/commit/a776736
[#601]: https://github.com/driftsys/ridl/issues/601
[#437]: https://github.com/driftsys/ridl/issues/437
[c624bd2]: https://github.com/driftsys/ridl/commit/c624bd2
[#600]: https://github.com/driftsys/ridl/issues/600
[255938e]: https://github.com/driftsys/ridl/commit/255938e
[#592]: https://github.com/driftsys/ridl/issues/592
[#586]: https://github.com/driftsys/ridl/issues/586
[68e0c57]: https://github.com/driftsys/ridl/commit/68e0c57
[#591]: https://github.com/driftsys/ridl/issues/591
[#579]: https://github.com/driftsys/ridl/issues/579
[4045f74]: https://github.com/driftsys/ridl/commit/4045f74
[#595]: https://github.com/driftsys/ridl/issues/595
[73e37cc]: https://github.com/driftsys/ridl/commit/73e37cc
[#613]: https://github.com/driftsys/ridl/issues/613
[fd87978]: https://github.com/driftsys/ridl/commit/fd87978
[#596]: https://github.com/driftsys/ridl/issues/596
[#416]: https://github.com/driftsys/ridl/issues/416
[#424]: https://github.com/driftsys/ridl/issues/424

## [0.4.0] (2026-09-28)

### Refactoring

- **ridl-backend-rust:** split face.rs into submodules and move the fixture's
  regeneration guard ([#574]) ([6f3f446])

### Features

- **ridl-backend-rust:** move the face's fixed and derived methods behind traits
  ([#582]) ([46df3cf])
- **ridl-backend-rust:** emit the blocking client and blocking::serve (E11.21,
  second half) ([#577]) ([1eb0fba])

### Bug Fixes

- **ridl:** warn RIDL-407 on a struct field or union arm inserted or removed
  ([#589]) ([99adfe7]), closes [#533]
- **ridl-sem:** refuse an enumset bit name declared twice (TYPL-218) ([#578])
  ([82b1e43]), closes [#565]
- **ridl-rt:** report an oversized claim with its id so serve can settle it
  ([#585]) ([14a487a]), closes [#569]
- **ridl-backend-rust:** bound the claims one serve poll takes ([#584])
  ([9b98a6a]), closes [#568]
- **ridl-backend-rust:** bind dispatch's decoded argument as __arg ([#581])
  ([b1fae74])
- **ridl-sem:** refuse a union arm name declared twice (TYPL-217) ([#573])
  ([93da335]), closes [#452]
- **ridl-backend-rust:** emit only the bit constants in an enum set's impl
  ([#572]) ([fced988]), closes [#562]

### BREAKING CHANGES

- a consumer of a generated crate adds
`use <crate>::<iface>::prelude::*;` for each interface whose face it uses;
without it every fixed method is E0599. When a member of the interface
shares a fixed name, the consumer writes the trait's path for the face's
method, `<Client<_> as Bind>::new(port)`. The emitted crate needs
`ridl-rt` with the `face` module, which the 0.4.0 release carries.
- the manifest ridl build emits declares
std = ["ridl-rt/std"] in place of std = [], so a consumer that builds
the emitted crate with its default features now links ridl-rt with its
std feature, and one that disables the emitted crate's std has no
blocking module. The generated face gains items and changes none.
- generated enum sets no longer have the associated
const `DECLARED_MASK` or the method `get`. Use `i64::from(set)` to
read the bits. `i64::from` is not a `const fn`, so the bits can no
longer be read in a const context.

[0.4.0]: https://github.com/driftsys/ridl/compare/v0.3.0...v0.4.0
[6f3f446]: https://github.com/driftsys/ridl/commit/6f3f446
[#574]: https://github.com/driftsys/ridl/issues/574
[46df3cf]: https://github.com/driftsys/ridl/commit/46df3cf
[#582]: https://github.com/driftsys/ridl/issues/582
[1eb0fba]: https://github.com/driftsys/ridl/commit/1eb0fba
[#577]: https://github.com/driftsys/ridl/issues/577
[99adfe7]: https://github.com/driftsys/ridl/commit/99adfe7
[#589]: https://github.com/driftsys/ridl/issues/589
[#533]: https://github.com/driftsys/ridl/issues/533
[82b1e43]: https://github.com/driftsys/ridl/commit/82b1e43
[#578]: https://github.com/driftsys/ridl/issues/578
[#565]: https://github.com/driftsys/ridl/issues/565
[14a487a]: https://github.com/driftsys/ridl/commit/14a487a
[#585]: https://github.com/driftsys/ridl/issues/585
[#569]: https://github.com/driftsys/ridl/issues/569
[9b98a6a]: https://github.com/driftsys/ridl/commit/9b98a6a
[#584]: https://github.com/driftsys/ridl/issues/584
[#568]: https://github.com/driftsys/ridl/issues/568
[b1fae74]: https://github.com/driftsys/ridl/commit/b1fae74
[#581]: https://github.com/driftsys/ridl/issues/581
[93da335]: https://github.com/driftsys/ridl/commit/93da335
[#573]: https://github.com/driftsys/ridl/issues/573
[#452]: https://github.com/driftsys/ridl/issues/452
[fced988]: https://github.com/driftsys/ridl/commit/fced988
[#572]: https://github.com/driftsys/ridl/issues/572
[#562]: https://github.com/driftsys/ridl/issues/562

## [0.3.0] (2026-09-27)

### Documentation

- **docs:** archive the bug lanes driver and close the lanes ([#559])
  ([75e7c28])
- **docs:** plan the PascalCase enum variant change ([#556]) ([9841cdf])
- **docs:** design PascalCase enum variants and amend ADR-0016 ([#555])
  ([2eb0e29])
- **docs:** route AGENTS.md reading by task and keep one ADR index ([#547])
  ([adc7bd3])
- **ridl:** align the call sequence scope and the invalid event with the frame
  specification ([#544]) ([2b8b904]), closes [#308], [#309]
- **adr:** amend ADR-0023 and ADR-0021 for the async face, and add the F3 to F5
  plan ([#541]) ([26f590a])
- **docs:** add the async face design note for lane F stage F2 ([#530])
  ([e4507db])
- **docs:** rename the bug lane from F to D ([#524]) ([479f13e])
- **ridl:** state that ridl specifies no Binder layout ([#521]) ([ddd56fd]),
  refs [#516], [#516], [#516], [#516]
- **docs:** add the driver prompt for bug lanes F and Q ([#508]) ([2202639])
- **roadmap:** add lane F, the async face and the runtime substrate ([#505])
  ([f846f4f])

### Bug Fixes

- **ridl-sem:** refuse an enum value name declared twice (TYPL-216) ([#563])
  ([44d4b28]), closes [#554]
- **ridl-sem:** give an unresolved type path a diagnostic code (TYPL-011)
  ([#564]) ([8231aaa]), closes [#543]
- **ridl-backend-rust:** spell enum variants through pascal_case ([#560])
  ([a32f72c])
- **ridl-loopback:** withdraw a forgotten call no handler has claimed ([#557])
  ([5ac7082])
- **ridl-loopback:** store one waker per kind of key and return a dropped
  handler's claims (E11.16) ([#551]) ([c2543c2])
- **repo:** fail book-check when SUMMARY.md names a chapter file that does not
  exist ([#540]) ([cf7dca5]), closes [#201], [#201], [#201], [#201]
- **editors:** retry a failed language client start and restart safely ([#539])
  ([997a770]), closes [#344], refs [#344]
- **ridl-syntax:** bound the depth of an operator chain in an expression
  ([#536]) ([0a7aecb])
- **ridl-sem:** reject a negative default timing bound and code a primitive
  query return ([#537]) ([aaa4010])
- **ridl:** stop the empty --baseline refusal from suggesting a publish into the
  source tree ([#535]) ([a12e9fc]), closes [#340]
- **ridlc:** run the service catalog in ridl_check and the language server
  ([#532]) ([d54da37]), closes [#345], [#386]
- **ridl:** warn at the desk when a struct field or union arm moves ([#527])
  ([2af7cc9]), closes [#335]
- **ridl:** refuse to publish when the RIDL-408 gate cannot resolve the
  published side ([#528]) ([ae0301a])
- **ridl-lsp:** show the workspace load error and load from the opened file
  ([#525]) ([fd0008a]), closes [#384]
- **ridl-backend-rust:** read a never-published signal as Init, not Corrupt
  ([#519]) ([a5e6e1b]), fixes 517., refs [#517]
- **ridl-backend-rust:** compile the FlatBuffers codec for a vector of booleans
  ([#520]) ([7342dc2]), fixes [#518]
- **repo:** make main green again after the 0.2.1 release ([#504]) ([a2a8fbd])

### Features

- **ridl-backend-rust:** emit the async client, the named futures and serve
  (E11.21, first half) ([#566]) ([1604b5c])
- **ridl-rt:** add the correlation table and the composed errors, and move the
  loopback onto them (E11.18) ([#553]) ([eb41a7a])
- **ridl-rt:** add the Wakeable port extension and Transport::Busy (E11.16)
  ([#545]) ([c0fa57c])
- **ridl-rt-conformance:** add the port contract suite as a crate (E11.20)
  ([#531]) ([83214a1])
- **ridl-rt:** add freshness, event loss, budget and deadline helpers (E11.19)
  ([#523]) ([87de8c6]), refs [#513], [#513], [#513], [#513], [#513], [#513],
  [#513]
- **ridl-rt:** add the std feature with block_on and noop_waker (E11.17)
  ([#522]) ([3f2cfb3]), refs [#511]

### BREAKING CHANGES

- Client::<command>, Client::<query> and
- every generated enum variant whose pascal_case differs
from its typl spelling is renamed (A and X2 keep theirs). Code that
names a variant, as in Health::WARN, must use the new spelling,

[0.3.0]: https://github.com/driftsys/ridl/compare/v0.2.1...v0.3.0
[75e7c28]: https://github.com/driftsys/ridl/commit/75e7c28
[#559]: https://github.com/driftsys/ridl/issues/559
[9841cdf]: https://github.com/driftsys/ridl/commit/9841cdf
[#556]: https://github.com/driftsys/ridl/issues/556
[2eb0e29]: https://github.com/driftsys/ridl/commit/2eb0e29
[#555]: https://github.com/driftsys/ridl/issues/555
[adc7bd3]: https://github.com/driftsys/ridl/commit/adc7bd3
[#547]: https://github.com/driftsys/ridl/issues/547
[2b8b904]: https://github.com/driftsys/ridl/commit/2b8b904
[#544]: https://github.com/driftsys/ridl/issues/544
[#308]: https://github.com/driftsys/ridl/issues/308
[#309]: https://github.com/driftsys/ridl/issues/309
[26f590a]: https://github.com/driftsys/ridl/commit/26f590a
[#541]: https://github.com/driftsys/ridl/issues/541
[e4507db]: https://github.com/driftsys/ridl/commit/e4507db
[#530]: https://github.com/driftsys/ridl/issues/530
[479f13e]: https://github.com/driftsys/ridl/commit/479f13e
[#524]: https://github.com/driftsys/ridl/issues/524
[ddd56fd]: https://github.com/driftsys/ridl/commit/ddd56fd
[#521]: https://github.com/driftsys/ridl/issues/521
[#516]: https://github.com/driftsys/ridl/issues/516
[2202639]: https://github.com/driftsys/ridl/commit/2202639
[#508]: https://github.com/driftsys/ridl/issues/508
[f846f4f]: https://github.com/driftsys/ridl/commit/f846f4f
[#505]: https://github.com/driftsys/ridl/issues/505
[44d4b28]: https://github.com/driftsys/ridl/commit/44d4b28
[#563]: https://github.com/driftsys/ridl/issues/563
[#554]: https://github.com/driftsys/ridl/issues/554
[8231aaa]: https://github.com/driftsys/ridl/commit/8231aaa
[#564]: https://github.com/driftsys/ridl/issues/564
[#543]: https://github.com/driftsys/ridl/issues/543
[a32f72c]: https://github.com/driftsys/ridl/commit/a32f72c
[#560]: https://github.com/driftsys/ridl/issues/560
[5ac7082]: https://github.com/driftsys/ridl/commit/5ac7082
[#557]: https://github.com/driftsys/ridl/issues/557
[c2543c2]: https://github.com/driftsys/ridl/commit/c2543c2
[#551]: https://github.com/driftsys/ridl/issues/551
[cf7dca5]: https://github.com/driftsys/ridl/commit/cf7dca5
[#540]: https://github.com/driftsys/ridl/issues/540
[#201]: https://github.com/driftsys/ridl/issues/201
[997a770]: https://github.com/driftsys/ridl/commit/997a770
[#539]: https://github.com/driftsys/ridl/issues/539
[#344]: https://github.com/driftsys/ridl/issues/344
[0a7aecb]: https://github.com/driftsys/ridl/commit/0a7aecb
[#536]: https://github.com/driftsys/ridl/issues/536
[aaa4010]: https://github.com/driftsys/ridl/commit/aaa4010
[#537]: https://github.com/driftsys/ridl/issues/537
[a12e9fc]: https://github.com/driftsys/ridl/commit/a12e9fc
[#535]: https://github.com/driftsys/ridl/issues/535
[#340]: https://github.com/driftsys/ridl/issues/340
[d54da37]: https://github.com/driftsys/ridl/commit/d54da37
[#532]: https://github.com/driftsys/ridl/issues/532
[#345]: https://github.com/driftsys/ridl/issues/345
[#386]: https://github.com/driftsys/ridl/issues/386
[2af7cc9]: https://github.com/driftsys/ridl/commit/2af7cc9
[#527]: https://github.com/driftsys/ridl/issues/527
[#335]: https://github.com/driftsys/ridl/issues/335
[ae0301a]: https://github.com/driftsys/ridl/commit/ae0301a
[#528]: https://github.com/driftsys/ridl/issues/528
[fd0008a]: https://github.com/driftsys/ridl/commit/fd0008a
[#525]: https://github.com/driftsys/ridl/issues/525
[#384]: https://github.com/driftsys/ridl/issues/384
[a5e6e1b]: https://github.com/driftsys/ridl/commit/a5e6e1b
[#519]: https://github.com/driftsys/ridl/issues/519
[#517]: https://github.com/driftsys/ridl/issues/517
[7342dc2]: https://github.com/driftsys/ridl/commit/7342dc2
[#520]: https://github.com/driftsys/ridl/issues/520
[#518]: https://github.com/driftsys/ridl/issues/518
[a2a8fbd]: https://github.com/driftsys/ridl/commit/a2a8fbd
[#504]: https://github.com/driftsys/ridl/issues/504
[1604b5c]: https://github.com/driftsys/ridl/commit/1604b5c
[#566]: https://github.com/driftsys/ridl/issues/566
[eb41a7a]: https://github.com/driftsys/ridl/commit/eb41a7a
[#553]: https://github.com/driftsys/ridl/issues/553
[c0fa57c]: https://github.com/driftsys/ridl/commit/c0fa57c
[#545]: https://github.com/driftsys/ridl/issues/545
[83214a1]: https://github.com/driftsys/ridl/commit/83214a1
[#531]: https://github.com/driftsys/ridl/issues/531
[87de8c6]: https://github.com/driftsys/ridl/commit/87de8c6
[#523]: https://github.com/driftsys/ridl/issues/523
[#513]: https://github.com/driftsys/ridl/issues/513
[3f2cfb3]: https://github.com/driftsys/ridl/commit/3f2cfb3
[#522]: https://github.com/driftsys/ridl/issues/522
[#511]: https://github.com/driftsys/ridl/issues/511

## [0.2.1] (2026-09-23)

### Bug Fixes

- **repo:** stop matching sha256sum's localized message in install-check
  ([#503]) ([e53ee4d])
- **ridlc:** bound plugin response collection ([7886ea5])
- **editors:** rename the extension to ridl-lang, its Marketplace name is taken
  ([#499]) ([43e1cf1])

### Documentation

- **ridlc:** describe plugin response timeout ([591c32c])
- **roadmap:** close the Kotlin Binder runtime scope item ([817e0bb])
- **docs:** add the codegen model design note, lane P stage P2a ([fe3ce8e])
- **adr:** make canonical protobuf JSON the canonical IR encoding ([243dc49])
- **docs:** add the frame specification, story E11.1 ([975fae2])
- **docs:** add the IR stability design note, lane P stage P1a ([6644f82])
- **roadmap:** move the plugin protocol ahead of the remaining codecs and
  TypeScript ([bbae0ed])
- **editors:** lead the vscode README with features, not dev commands ([#500])
  ([b7859fc])
- **docs:** add the lane P driver, the codegen plugin system toward Kotlin
  ([#494]) ([3f0a65d])

### Features

- **repo:** merge lane P codegen plugin system ([7fc417e])
- **ridl-ir:** add the backend contract over the codegen model ([86f5fa6])
- **ridl-ir:** add the lowered codegen model, its schema and its lowering
  ([5eb7f8e])
- **editors:** add RIDL extension icon ([#497]) ([c338f9c])
- **ci:** release ridlc alongside ridl on the editor-v* train ([#495])
  ([2632e1d])

### Refactoring

- **ridl-backend-rust:** read the domain types from the lowered model
  ([8d015e5])

[0.2.1]: https://github.com/driftsys/ridl/compare/v0.2.0...v0.2.1
[e53ee4d]: https://github.com/driftsys/ridl/commit/e53ee4d
[#503]: https://github.com/driftsys/ridl/issues/503
[7886ea5]: https://github.com/driftsys/ridl/commit/7886ea5
[43e1cf1]: https://github.com/driftsys/ridl/commit/43e1cf1
[#499]: https://github.com/driftsys/ridl/issues/499
[591c32c]: https://github.com/driftsys/ridl/commit/591c32c
[817e0bb]: https://github.com/driftsys/ridl/commit/817e0bb
[fe3ce8e]: https://github.com/driftsys/ridl/commit/fe3ce8e
[243dc49]: https://github.com/driftsys/ridl/commit/243dc49
[975fae2]: https://github.com/driftsys/ridl/commit/975fae2
[6644f82]: https://github.com/driftsys/ridl/commit/6644f82
[bbae0ed]: https://github.com/driftsys/ridl/commit/bbae0ed
[b7859fc]: https://github.com/driftsys/ridl/commit/b7859fc
[#500]: https://github.com/driftsys/ridl/issues/500
[3f0a65d]: https://github.com/driftsys/ridl/commit/3f0a65d
[#494]: https://github.com/driftsys/ridl/issues/494
[7fc417e]: https://github.com/driftsys/ridl/commit/7fc417e
[86f5fa6]: https://github.com/driftsys/ridl/commit/86f5fa6
[5eb7f8e]: https://github.com/driftsys/ridl/commit/5eb7f8e
[c338f9c]: https://github.com/driftsys/ridl/commit/c338f9c
[#497]: https://github.com/driftsys/ridl/issues/497
[2632e1d]: https://github.com/driftsys/ridl/commit/2632e1d
[#495]: https://github.com/driftsys/ridl/issues/495
[8d015e5]: https://github.com/driftsys/ridl/commit/8d015e5

## 0.2.0 (2026-09-22)

### Bug Fixes

- **ridl-backend-rust:** send the encoded subslice, and settle
  driftsys/ridl[#448] ([#471]) ([83bcc10]), closes [#448]
- **repo:** clear the whole compat-check build directory each run ([#464])
  ([d36e97c]), fixes 442.
- **ridl-sem:** give a bare string or bytes map key the length default ([#459])
  ([86526be]), closes [#457]
- **ridl-backend-rust:** project struct, tuple and union names to their target
  namespaces ([#451]) ([5b5a0ce])
- **ridl-core:** refuse a package named for one the compiler provides ([#407])
  ([f7882b3]), closes 203.
- **ridl-sem:** report an exact duplicate name before RIDL-149 checks a
  collision ([#406]) ([1d43aa4])
- **ci:** give the git-std install a fallback and a post-install check ([#402])
  ([e3a1071]), closes [#395]
- **repo:** close a fence in link-check only at a fence as long as its opener
  ([#359]) ([a7ac5c8])
- **ridl:** refuse to publish an untombstoned removal or an explicit baseline
  with no snapshot ([#330]) ([2a4a47d]), closes 235. Addresses #315 at the
  interaction level. Pass-2 review debt is #339, [#340], #341 and #342.
- **ridl:** describe a snapshot directory whose snapshots are nested ([#233])
  ([21a41f4]), closes [#230]
- **ridl:** refuse unrecognised IR paths instead of compiling them ([#228])
  ([4cf2a51])
- **repo:** clear three recorded debt items ([#227]) ([0f5a712])
- **ridl-ir:** move the IR's JSON encoding onto pbjson-generated impls ([#226])
  ([4762d28])
- **ridl-ir:** contain the prototext reader behind the crate's tests ([#225])
  ([a31bcd2])
- **ci:** stop the pages deploy being skipped by a skipped ancestor ([#214])
  ([ec96247])
- **ridlc:** emit ridl.std so generated code compiles standalone ([#200])
  ([8daf58d]), refs 190.

* fix(ridl-ir): make the reference-walk depth test
  discriminate per path

The recursion test for referenced_packages used the
  same qualifier
(ridl.std) for a direct reference and for the array-element
  reference
it claimed to exercise, so the array arm's body could be
  deleted
without failing the test. Verified: gutting that arm still
  passed.

Give every recursive path through walk_field_type its own
  distinct
qualifier (array, tuple, map key, map value, stream element) plus
  the
direct and bare-reference cases, and assert the exact expected set
instead
  of contains plus a length check, so a missing arm names itself
in the failure
  output.

Proved the fix discriminates: for each of the five recursive arms,
  temporarily replaced its body with an empty block, ran the test, and
confirmed
  it failed naming exactly the qualifier that arm was
responsible for, then
  restored the arm. All five reproduced the
expected single-qualifier omission.,
  190.

* fix(ridlc): emit ridl.std when the workspace references it

`ridlc
  build` writes each checked package's generated code, but
`ridl.std` is
  deliberately absent from `checked` because it is not a
workspace member. A
  consumer's generated code can still reference it
(for example a duration field
  lowers to `ridl.std.Timestamp`), so the
raw build output did not compile
  (issue #190).

`run_build` now checks, after the per-package emit loop,
  whether any
checked package's IR references `ridl.std` (task 1's, 190.

*
  fix(ridlc): keep ridl.std out of the ir-json emit

`run_build` passed the
  caller's full `emits` slice to the standard package's
`write_emits`, so
  `--emit ir-json` wrote `ridl.std.ir.json`. `ridl baseline`
is that call with
  `&[Emit::IrJson]` and publishes every `.ir.json` it finds, while `ridl diff`
  compiles its current side through `compile_workspace`, which
excludes
  `ridl.std` because the standard package is not a workspace member.
The two
  sides were asymmetric, so `ridl baseline` followed by `ridl diff`
over an
  unedited workspace that names a standard type reported

    breaking
     
  [breaking] decl_removed ridl.std: package ridl.std -> (removed)

and exited 1.
  `ridl check --baseline` did not show it: the desk check reports
ordinal drift
  only, and a whole removed package moves no ordinal.

The standard package is
  version-locked to the compiler binary (ADR-0007
decision 15), so it is not
  part of a workspace's contract snapshot — a
baseline holds the packages the
  workspace declares. Issue #190 is about
generated code failing to compile, and
  a `.ir.json` is not code. The three
code targets keep the emit: `rust`,
  `c-header` (its header already names
`ridl_std_*` typedefs) and `typescript`.
- **typl:** scope ridl.std by an inclusion criterion, drop Vin ([#198])
  ([2ba8690])
- close three ridl/ridlc CLI defects, record ADR-0010 ([#195]) ([fa62c08])
- **adr:** discharge ADR-0008 decision 19 and sweep the document ([#192])
  ([c40f65e])
- **repo:** five small items from the E2/E1 debt triage ([#189]) ([499ec32])
- **repo:** make the local gate and CI the same gate ([#186]) ([75c02c2])
- **ridl-backend-ts:** parenthesise an object literal in a concise arrow body
  ([#185]) ([519ad66]), closes [#177]
- **repo:** make the local gate enforce every member decision 11 names ([#184])
  ([dc8cad0]), closes [#182]
- **ridl-sem:** description-first diagnostics across the RIDL namespace ([#178])
  ([da3d246])
- **backends:** two codegen defects — an internal tuple and a constant
  reference ([#176]) ([af7ef7c]), closes 167

* fix(ridl-sem): a constant
  reference lowers as the referenced value

`ConstDef.value` and a declared init
  carried the written text of a constant
reference instead of the constant's
  value. `ridlc check` exits 0 on

    const SECRET : Tick = 7
    const PUB   
  : Tick = SECRET

and the Rust backend then emits `pub const PUB: Tick =
  Tick(SECRET);`, which
rustc rejects.

The repair belongs at lowering, not in a
  backend, and the IR says why:
`ConstDef.value` is a bare string with no
  discriminator, so `const A : string =
"B"` and `const A : string = B` lower to
  identical IR. No consumer can tell a
value from a reference, and each one
  failed differently — the Rust backend
emitted uncompilable code for a named
  scalar, a wrong value for a string or a
boolean, and `PI.0` for a float; the
  TypeScript backend refused the package
outright with a message about
  `Number.MAX_SAFE_INTEGER`. A backend also holds
one package's IR at a time, so
  it could not resolve an imported constant at
all.

The chain is followed
  through the existing `const_value`, which already serves
the range bounds, the
  `match` patterns and `declared_type_init`: one
resolution, cycle-guarded, and
  correct across packages. A reference that does
not resolve still lowers as the
  written text — the diagnostic for an unknown
name belongs to the
  resolver.

The declared-init path had the same rule in a second
  implementation, and the
two diverged where it was hardest to see: a numeric
  reference resolved, so
`type T : integer [0..100] = SEVEN` was right, while
  `type T : string [1..20] =
GREETING` lowered the *name* as the string value
  and the generated `Default`
returned `"GREETING"`. Both now resolve every
  kind, and the resolved value is
checked against the declared bounds exactly as
  a direct literal is.

Fixing it here fixes both backends from one change,
  which is what the E2 exit
criterion asks of the IR., 170

* fix(backends):
  refuse a tuple-name collision instead of deduplicating it

A tuple generates a
  struct named for the CamelCase of the path that reaches it, and nothing
  upstream keeps two paths from spelling one name:

    internal struct A  { bC
  : (y : Tock) }
    struct AB          { c  : (x : Tick) }

both reach `ABC`,
  and `ridlc check` exits 0. The worklist kept the first
discovery, so the
  second declaration got the first one's shape — a module that
compiles and
  states the wrong contract.

Carrying the inducing declaration's visibility
  made that corner worse rather
than better. With one declaration `internal` and
  the other public — and no
`internal` payload type anywhere, so TYPL-005 has
  nothing to see — first-wins
put a `pub(crate)` struct in a `pub` struct's
  field, and rustc reported
`private_interfaces` on a program that built before.
  Keeping the widest
visibility instead would publish the package-private
  declaration's shape, which
is the defect the previous commit fixed.

Neither
  dedup rule is sound under collision; only rejection is. The refusal is
what
  every other generated-name clash already gets from
- **ridl-sem:** three parser and resolver defects from the E2 close-out ([#174])
  ([ebebaf7])
- **ridl-sem:** apply the visibility rule to interfaces and services ([#168])
  ([8705450]), closes 161

* fix(ridl-sem): ask the resolver which names a
  contract clause binds

Review of the previous commit found the exposure check
  for `require`/
`ensure` clauses re-deriving the contract binding order from
  the syntax
tree. It matched a `SignalDef` anywhere under the enclosing
  interface, whatever the clause kind — but `lower_contracts` builds an
  `ensure` scope
with `signals: &[]` (ridl §13), so an interface signal spelled
  like an
`internal` constant suppressed a diagnostic the resolver binds to
  the
constant. `signal MAX_LEVEL` beside `internal const MAX_LEVEL`
  made
`ensure result > MAX_LEVEL` compile with zero diagnostics while the
  IR
published the clause text naming the package-private constant.

The check
  now runs inside `lower_contracts`, reading `collect_refs`
against the very
  scope `check_contract_expr` was run with, so the binding
order has one
  implementation and this reads its answer instead of
guessing it. `ExprRefs`
  gains `enum_types` — the head of an `Enum.MEMBER`
access is not a read, so
  the observer stubs do not want it, but it is a
package declaration the
  published clause names. Re-deriving a rule that
already has an owner is the
  same shape as the defect this branch fixes, where the rule lived in one place
  and its application in another.

Review also found a twentieth exposure
  position: a collection length
`Bound` is a direct child of the
  `ArrayType`/`MapType` rather than a
`Constraint`, so `[Tick; 1..MAXLEN]` never
  matched the walk while
`string [1..MAXLEN]` did — two structurally identical
  positions with one
flagged and one silent, in the typl layer as well as the
  ridl one. typl
§3.3 says "bounds constants", so this was an
  under-implementation of the
section cited as authority for the init-value
  exclusion. Both node kinds
are now named. There is no practical leak — the
  bound resolves to a value
and the constant's name does not survive into the IR
  — but a table
asserting completeness it does not have is how the next person
  stops
looking, which is what produced #161.

The corpus carries both:
  `exposure.ridl` now holds the signal/constant
collision that separates the two
  clause kinds, with the `require` half
silent and the `ensure` half reporting,
  and a length bound in both the
public interface and the `internal` control.
  The legal-direction unit
fixture gains a clause and a bound so that removing
  the clause-level
visibility guard is observable — it was not
  before.

Recorded rather than fixed: duplicate declarations now report
  the
exposure once per duplicate where the lowering loop consulted
`is_winner`;
  the RIDL-143 branch tests for a `ServiceDef` parent with no
`else`, so a later
  interface-naming position inherits TYPL-005 but not
RIDL-143; and an attribute
  value's constant reference is invisible to the
pass, unreachable today only
  because every attribute key draws
FORM-106/107. ADR-0008's allocation ledger
  needs a ninth entry for
RIDL-143 — issue #169, since a PR-body note is what
  #161 said would not
survive the merge.
- **backends:** TypeScript emits interaction ordinals and tombstones ([#164])
  ([3dc6f30])
- **ridl:** resolve contract names the way the checker does ([#162]) ([516dff0])
- **backends:** internal interfaces map to package-private in both backends
  ([#160]) ([4230fd8])
- **ridl-sem:** skip lowering a nameless interaction member ([#158]) ([0c8f3ab])

### Features

- **repo:** give every crate a version, and publish it to crates.io on a tag
  ([#489]) ([8eb3421])
- **repo:** run the cabin example with just demo ([#483]) ([9069d2c])
- **ridlc:** emit the interaction face from ridl build ([#479]) ([330e962])
- **ridl-backend-rust:** move the generated face onto the FlatBuffers codec
  ([#475]) ([5deab17])
- **ridl-backend-flatbuffers:** root every declaration in a table ([#474])
  ([5f05bdc])
- **ridl-backend-rust:** check the typl constraint beside new in verify ([#468])
  ([80773b8]), closes [#463]
- **ridl-loopback:** the in-process reference runtime ([#466]) ([c5c582c])
- **ridl-backend-rust:** emit the FlatBuffers payload codec ([#465]) ([261724f])
- **ridl-backend-rust:** construct vacuous named scalars infallibly ([0ade5b1])
- **ridl-backend-rust:** refuse a struct or union with no finite FlatBuffers
  bound ([#462]) ([20c5f1e])
- **ridl-ir:** the shared FlatBuffers projection facts and the size bound
  ([#458]) ([196989a])
- **ridl-rt:** lane K stage K3 — the FlatBuffers helpers, the subslice, and
  the proof mechanism ([#461]) ([77c5e59])
- **ridl-backend-rust:** derive the sound traits on the typl surface ([#443])
  ([8e5a552])
- **ridl-backend-rust:** hold the port by value and type each correlation
  ([#439]) ([f9b2904])
- **ridl-backend-rust:** check match patterns under validate-pattern ([#436])
  ([61a6c35])
- **ridl-rt:** implement every port trait for a borrow of a port ([#438])
  ([3fdb631])
- **ridl-backend-rust:** validate enum and enum-set conversion from i64 ([#433])
  ([7c9d9c4])
- **ridl-backend-rust:** make constrained named scalars value objects ([#420])
  ([ddcbe85])
- **ridl-backend-rust:** generate the interaction face over ridl-rt ([#418])
  ([2d26b38])
- **rsdl:** lower the system to the IR, with ridl diff and the book chapter
  ([#417]) ([fe4c44b])
- **ridlc:** emit a compiling crate from --emit rust ([#415]) ([564bc14]),
  closes [#252]
- **ridl-backend-rust:** name the runtime by absolute path, not a generated
  error ([#413]) ([529d93e]), closes [#247]
- **ridl-ir:** add the vacuous-constraint classifier ([#412]) ([d8b7a59]),
  closes [#246]
- **ridl:** number every interface from interfaces.lock and add ridl lock
  ([#391]) ([fdcf2ca])
- **rsdl:** parse and check .rsdl, with editor support ([#388]) ([0504220])
- **ridl-rt:** revise the port and error API before 0.1 ([#351]) ([5f24ea7]),
  refs [#350], [#316]
- **ridl-rt:** add ridl-rt 0.1.0, the runtime library ([#348]) ([91e6b62]), refs
  [#316]
- **repo:** add the MCP server and the editor distribution train ([#327])
  ([2fa924c])
- **ridl-diff:** give a composite body reorder its own category ([#331])
  ([f2efb61])
- **ridl-backend-flatbuffers:** project the typl surface and the ordinals onto
  FlatBuffers ([#303]) ([0199406])
- **repo:** retract the interaction layer and record the runtime architecture
  ([#241]) ([7d539bc])
- **ridl-backend-proto:** project the typl surface and the ordinals onto proto3
  ([#242]) ([1e674fe])
- **ridl:** pin one name transform and reject collisions after it ([#238])
  ([b4c420f])
- **ridl:** a service may carry more than one interface ([#223]) ([89b8604])
- **ridl:** give command and query the RPC bounds ([#221]) ([c560e3d])
- **ridlc:** add the ir-text and ir-binary emits ([#219]) ([9a5eb49])
- **ridl-ir:** render canonical protobuf JSON on every IR surface ([#217])
  ([ef3c9a5])
- **ridl:** the interaction boundary model — retire uxdl, add rxdl ([#204])
  ([ded42ef])
- **ridl-syntax:** rename the provisioned-constant keyword to fixed ([#199])
  ([2513e70])
- **ridlc:** emit TypeScript from `ridlc build --emit` ([#188]) ([a3a4700]),
  refs [#172], driftsys/ridl#172
- **ridl:** ridl test — subset evaluator + property runs (E2.11a) ([#157])
  ([96efb74])
- **backends:** Rust backend — interactions and services (E2 exit criterion)
  ([#156]) ([364dc4b])
- **backends:** TypeScript backend — interactions + services (E2.6b) ([#155])
  ([99fe470])
- **ridl:** baseline-aware desk check (E2.9) ([#153]) ([6e6abe5])
- **ridl-sem:** the ridl lint pass (E2.10a) ([#154]) ([3729961])
- **ridl-sem:** observer-stub lowering (E2.5) ([#152]) ([7b3c831])
- **tools:** ridl diff — breaking/compatible classifier (E2.8b) ([#150])
  ([450f2b9])
- **ridl-sem:** expr subset type checking for require/ensure (E2.4) ([#149])
  ([dfcc83b])
- **ridl-lsp:** LSP + editor support for ridl (E2.10b) ([#151]) ([5266ccb])
- **ridl-sem:** services — grammar, resolution, catalog, IR (E2.13) ([#148])
  ([00e666a])
- **tools:** ridl diff — IR-snapshot compare engine + CLI (E2.8a) ([#146])
  ([62bc8ed])
- **ridl-sem:** timing — validity, defaults, resolved IR (E2.2) ([#147])
  ([469047a])

### Refactoring

- **ridlc:** classify emits exhaustively for the ridl.std filter ([#220])
  ([20fba18])
- **repo:** every crate lives at crates/<crate-name>/ ([#181]) ([4cffb74]), refs
  [#180], [#180], [#180]
- **ridl-core:** declare each diagnostic code once (ADR-0008 d21) ([#175])
  ([6f15e77]), refs #172

* fix(ridl-core): correct three guard claims and
  enforce clippy locally

Review of d6f6c9e found three guard doc comments
  asserting a reach the
guards do not have, and one deliverable ADR-0008
  decision 21 binds that
was missing from the repository.

The "invocable
  exactly once" claim was false. A generated `ALL_CATALOGS`
collides only within
  one module, so a second `diag_codes!` inside a child
module of `diag`
  compiles. The safety outcome held, but through a
different guard than the one
  named: the literal scan caught the injected
code, and it did so structurally
  rather than by luck, because, [#172]
- **ridl-ir:** one walk over every interface shape ([#171]) ([2f8ff9b])

### Documentation

- **docs:** correct four lines the face's move onto the codec made false
  ([#478]) ([9684914])
- **docs:** archive the lane K driver, and close the lane ([#477]) ([26214fd])
- **docs:** finish K1 — the plan's task shape and the driver's stages ([#454])
  ([982c8d8])
- **docs:** the FlatBuffers payload codec design and plan ([#446]) ([a7c8051])
- **docs:** lane K stage K0 — split E11.9, file E11.14 and E11.15, add the
  lane prompt ([#447]) ([e44e885])
- **docs:** archive the face and port ergonomics note ([#440]) ([512a478])
- **docs:** design the face and port ergonomics to settle before E11.9 ([#429])
  ([6fb3456])
- **docs:** add the lane R driver prompt ([#435]) ([0b3e69f])
- **adr:** record the face and port ergonomics decisions ([#432]) ([f6b749e])
- **typl:** correct the value objects plan to the code Task 3 landed ([#431])
  ([3504a90])
- **ridl-rt:** document the traits progressively, and garden lane M ([#419])
  ([b6544fe]), closes [#393]
- **docs:** plan the interaction face MVP ([#411]) ([566f024])
- **typl:** take decision D, so a colliding union arm is refused ([#410])
  ([8746585])
- **docs:** give stage C4 its own driver prompt ([#409]) ([3988701])
- **docs:** record the model substitution while Fable is unavailable ([#408])
  ([25db3a3])
- **ridl:** dispose of every ridl §17 open question ([#401]) ([6d21c59]), refs
  319

* docs(docs): archive the lock design and plan

Lane L's work has landed,
  so its working memory moves to `docs/archive/` with
an entry in the archive
  README naming the durable records it gardened into:
ADR-0015's 2026-09-15
  amendment, ADR-0010's `ridl lock` rows, the ridl
reference §11 and §14.5,
  RIDL-409 to RIDL-412, and the CLI reference.

Every inbound link is repointed,
  and the links inside the two moved files now
reach `docs/wip/` from their new
  directory. Two of the repointed links are in
`docs/ROADMAP.md`, which pull
  request #394 also has open; the two lines are far
from its hunks, and the
  coordination issue records that L5 touched the file., 319

* docs(ridl):
  repair what review pass 1 found

Eight findings, three of them Important.

-
  §17.7 said the catalog descriptor and `ridl describe` had landed. They have
 
  not: Epic 16 is unstarted, `ridlc` has no `catalog` emit and `ridl` has no
 
  `describe`. The record now names the descriptor as specified and scheduled,
  which is what the disposition actually rests on, and the family overview says

   the same.
- Appendix G still defined quarantine as withholding an invalid
  stream element
  and continuing, citing §12.4 — the behaviour §12.4 now
  rules out. The entry is
  restated over both §12.4 and §4.5.
- ADR-0015
  decision 9 still quoted the coherence rule with the group identified
  by the
  interface name, and said the text transplants as it stands, while the
 
  reference now identifies the group by its lock number. Decision 9 carries a
 
  dated amendment, the record's amendment header names it, and "Documents to
 
  amend" carries the row.
- §12.4 gave a result-union element type two opposite
  meanings: a terminal error
  element that closes the stream, and a failure the
  consumer reads past. It now
  says what the element type does record — which
  stratum a failure lands in —
  and puts what the producer does after an
  error element where it belongs, in
  behaviour rather than in the contract.
-
  Appendix F's SOME/IP eventgroups row still called the ridl equivalent
 
  interface-level subscription granularity, which §5.1 contradicts.
- §17.14
  named RIDL-409 for a dropped lock entry; RIDL-409 is for an entry left
 
  behind with no declaration, which is the shape a form switch leaves.
- rxdl
  §11's acquisition/query row now names the same version and the same
 
  reopening observation as ridl §17.10, which owns it.
- Three figures of
  speech removed, against the plain-and-literal prose rule., 319

* docs(docs):
  restate the stream clause in the open-question index

Review pass 2 found the
  family overview's §17 summary still carrying the
framing the previous commit
  removed from the ridl reference's §12.4: that
continuing past a failing
  element is what a result-union element type declares.
What it declares is
  which stratum the failure lands in — with a plain element
type a constraint
  violation ends the call, and a failure declared as data is
Stratum 1, which
  does not., [#319]
- **typl:** take the constraint error from ridl-rt, not from codegen ([#404])
  ([b638e63])
- **docs:** list lane M in the step 1 lanes plan ([#405]) ([d3248a1])
- **ridl-rt:** read an absent EncodedSizes field as unsized, not uncarriable
  ([#403]) ([f3b3b14])
- **docs:** design the interaction face MVP ([#394]) ([a720d6c])
- **typl:** dispose of every open question in typl §17 ([#400]) ([f45ea99]),
  closes 245.
- **typl:** refresh the value-objects plan against the code ([#396]) ([72bb9da])
- **docs:** correct the lane C driver and the lanes plan for drift ([#392])
  ([86e10d7])
- **docs:** drive the interaction-face MVP as lane M ([#390]) ([d2e9d41])
- **ridl-rt:** state the port contracts that the signatures cannot ([#389])
  ([4405950])
- **roadmap:** sequence the lock and the catalog descriptor into step 1 ([#383])
  ([cc00580])
- **rsdl:** plan the rsdl parse, check and lowering, and refile Epic 6 ([#360])
  ([f44d475])
- **docs:** plan the lock file and ridl lock ([#358]) ([389e7f0])
- **adr:** state how the toolchain pin follows the latest stable release
  ([#357]) ([4b11633])
- **docs:** plan ridl-rt 0.1 ([#343]) ([bcb2855]), refs [#316]
- **rsdl:** rewrite the rsdl reference ([#333]) ([ab5e0c6])
- **docs:** design the lock file and ridl lock ([#334]) ([05e3100])
- **docs:** design ridl-rt 0.1 ([#332]) ([052ec5f]), refs [#316], [#308],
  [#309], [#328], [#316], [#328], [#316]
- **docs:** plan the step 1 lanes and write a driver prompt per lane ([#329])
  ([c94191a])
- **docs:** settle D-7's two internal tensions and record the rename consequence
  ([#325]) ([b1c43fa])
- **docs:** plan the catalog descriptor implementation ([#324]) ([ee3c57d])
- **docs:** design the runtime descriptors an engine reads ([#323]) ([fe3e918])
- **adr:** record ADR-0020 and amend the records the re-scope changed ([#312])
  ([52952b3])
- **docs:** record the rsdl rewrite decisions of 2026-09-12 ([#313]) ([0228902])
- **roadmap:** rewrite the roadmap as a two-step forward plan ([#310])
  ([69def56])
- **docs:** archive the superseded working notes and record the re-scope
  ([505914e])
- **docs:** garden the E9.7 working memory into the archive ([#240]) ([e1b55db])
- **adr:** field absence is declared once and realised per target ([#239])
  ([8a5b960])
- **roadmap:** put the canonical-encoding question in front of E4.5 ([#232])
  ([851bf99])
- **docs:** repair a mangled heading and the drift the pbjson move left ([#229])
  ([374bdd7])
- **repo:** close out the E9.1 to E9.6 block ([#224]) ([a90962a])
- **ridl:** state the coherence rule beside the service definition ([#222])
  ([899b8da])
- **adr:** ratify ADR-0014 and ADR-0015, and plan E9.1 to E9.6 ([#215])
  ([3d76965])
- **repo:** add status badges and link the published book ([#216]) ([8fae995])
- **docs:** align the book's front matter with ADR-0012 and the README ([#210])
  ([da5ca78])
- **repo:** bring the README and CONTRIBUTING in line with ADR-0012 ([#209])
  ([7e6c4e5])
- **roadmap:** link ADRs and design notes from prose, leave tables plain
  ([#208]) ([361cac0])
- **roadmap:** fold the wip designs into epics 9 and 10 ([#207]) ([c853c27])
- **rmdl:** cite GRust by author and thesis, not by industrial affiliation
  ([#206]) ([4ab9dd9])
- **docs:** add the language reference section to the book ([#197]) ([6440194])
- **docs:** write the CLI reference page ([#193]) ([886f4d7])
- **docs:** build the book foundation with compiled examples ([#187])
  ([58b404b])
- **repo:** garden epic E2 working memory and sync docs as-built ([#183])
  ([b4f7809]), refs 172

* docs(adr): correct four stale sentences found by
  reviewing the fix

Verification of the correction round on this branch found
  four more stale
sentences in ADR-0008's Status section, three of them written
  by that round.

- The fourth extension claimed the falsifying evidence for its
  own miss
  "again" sat inside the correcting commit. It did not:
 
  crates/ridl-syntax/src/keywords.rs is in no commit of this pull request.
  The
  claim inverted the mechanism, asserting the easy case where the
  document had
  just lived the hard one.
- The CI reading said every run completes "in five to
  eight seconds", which is false for 19 of the 247 runs, one of them by 65
  minutes. The
  duration was never load-bearing; the sentence now carries what
  is, which is that no job starts.
- Two sentences said the retired timing
  characterisation was gone from
  "the repository" when docs/archive/ still
  carries it. Both now name the
  live tree.

The running total moves to
  twenty-two, eight of which were found by
reviewing a correction rather than by
  making one.
- **adr:** sweep ADR-0008 against the shipped repository ([#179]) ([06c3f7a]),
  closes [#169]
- **ridl:** reconcile the reference with the shipped toolchain (E2 close-out)
  ([#173]) ([256bb52]), refs ADR-0008 decisions 1, 16-21, #172

* docs(ridl):
  close review findings on the E2 close-out reconciliation

Two blockers and
  three rulings from the review of fa9b731.

**The deferral of two RIDL-108
  sites rested on a false cost.** The PR body
and issue #172 claimed that
  editing the doc comment on
`ridl-diag-showcase/main/timing.ridl` regenerates
  the IR, Rust and TypeScript
snapshots. It does not: that entry carries error
  diagnostics, so all three
snapshots are one-line placeholders and `grep -rl
  "Equal bounds"` over
`crates/ridlc/tests/snapshots/` returns nothing. Editing
  it regenerates zero
snapshots — confirmed by running the suite, which passes
  with no snapshot
written. With the cost gone the deferral has no basis, so
  both sites are
fixed here: the corpus comment ("the same thing as `@250ms`"
  — the worked
example of RIDL-108 asserting the claim decision 17 retires)
  and the
`RIDL_108` doc comment in `crates/ridl-core/src/diag.rs`. Decision
  17's fix
is now four of four sites in code and text; correcting the ADR's
  own
"those are the only sites" sentence is deferred to a dedicated ADR-sweep
  PR, because ADR-0008's Status makes a whole-document sweep a precondition
  for
editing it.

**The new overview §7 opened with a closed enumeration that
  is false.** It
read "Four namespaces are in play. Two belong to a profile"
  while naming five
languages two sentences later; the specification set carries
  five profile
namespaces — `UXDL-` (uxdl §15), `RMDL-` (rmdl §11) and
  `RSDL-` (rsdl §12)
besides `TYPL-` and `RIDL-`. Restated as one namespace per
  profile, with the
two implemented today distinguished from the three specified
  ahead of their
layers.

**ridl §6.1's tuple clause is wrong, and so is typl
  §11's.** ridl §6.1 said
"tuples of parameters as in typl §11"; typl §11
  said higher profiles use
tuples as "interaction parameters and query returns".
  Verified:
`command setRange(w: (min: Speed, max: Speed))` is, ADR-0008
  decisions 17, [#18], [#172]
- **adr:** E2 close-out amendments — ADR-0008 decisions 16 to 21 ([#165])
  ([b32a171])

### BREAKING CHANGES

- an enum set's inner value is now private; read it with
get().
- generated code no longer exposes the inner field. Read it
with get() or From, and construct with new() or new_unchecked().
- --emit rust writes two additional files in package and
workspace mode. Single-file mode is unchanged.
- `ridlc build --emit c-header` is removed, and generated Rust and
TypeScript no longer carry interfaces, services or the five interaction kinds.

[83bcc10]: https://github.com/driftsys/ridl/commit/83bcc10
[#448]: https://github.com/driftsys/ridl/issues/448
[#471]: https://github.com/driftsys/ridl/issues/471
[d36e97c]: https://github.com/driftsys/ridl/commit/d36e97c
[#464]: https://github.com/driftsys/ridl/issues/464
[86526be]: https://github.com/driftsys/ridl/commit/86526be
[#459]: https://github.com/driftsys/ridl/issues/459
[#457]: https://github.com/driftsys/ridl/issues/457
[5b5a0ce]: https://github.com/driftsys/ridl/commit/5b5a0ce
[#451]: https://github.com/driftsys/ridl/issues/451
[f7882b3]: https://github.com/driftsys/ridl/commit/f7882b3
[#407]: https://github.com/driftsys/ridl/issues/407
[1d43aa4]: https://github.com/driftsys/ridl/commit/1d43aa4
[#406]: https://github.com/driftsys/ridl/issues/406
[e3a1071]: https://github.com/driftsys/ridl/commit/e3a1071
[#402]: https://github.com/driftsys/ridl/issues/402
[#395]: https://github.com/driftsys/ridl/issues/395
[a7ac5c8]: https://github.com/driftsys/ridl/commit/a7ac5c8
[#359]: https://github.com/driftsys/ridl/issues/359
[2a4a47d]: https://github.com/driftsys/ridl/commit/2a4a47d
[#330]: https://github.com/driftsys/ridl/issues/330
[#340]: https://github.com/driftsys/ridl/issues/340
[21a41f4]: https://github.com/driftsys/ridl/commit/21a41f4
[#233]: https://github.com/driftsys/ridl/issues/233
[#230]: https://github.com/driftsys/ridl/issues/230
[4cf2a51]: https://github.com/driftsys/ridl/commit/4cf2a51
[#228]: https://github.com/driftsys/ridl/issues/228
[0f5a712]: https://github.com/driftsys/ridl/commit/0f5a712
[#227]: https://github.com/driftsys/ridl/issues/227
[4762d28]: https://github.com/driftsys/ridl/commit/4762d28
[#226]: https://github.com/driftsys/ridl/issues/226
[a31bcd2]: https://github.com/driftsys/ridl/commit/a31bcd2
[#225]: https://github.com/driftsys/ridl/issues/225
[ec96247]: https://github.com/driftsys/ridl/commit/ec96247
[#214]: https://github.com/driftsys/ridl/issues/214
[8daf58d]: https://github.com/driftsys/ridl/commit/8daf58d
[#200]: https://github.com/driftsys/ridl/issues/200
[2ba8690]: https://github.com/driftsys/ridl/commit/2ba8690
[#198]: https://github.com/driftsys/ridl/issues/198
[fa62c08]: https://github.com/driftsys/ridl/commit/fa62c08
[#195]: https://github.com/driftsys/ridl/issues/195
[c40f65e]: https://github.com/driftsys/ridl/commit/c40f65e
[#192]: https://github.com/driftsys/ridl/issues/192
[499ec32]: https://github.com/driftsys/ridl/commit/499ec32
[#189]: https://github.com/driftsys/ridl/issues/189
[75c02c2]: https://github.com/driftsys/ridl/commit/75c02c2
[#186]: https://github.com/driftsys/ridl/issues/186
[519ad66]: https://github.com/driftsys/ridl/commit/519ad66
[#185]: https://github.com/driftsys/ridl/issues/185
[#177]: https://github.com/driftsys/ridl/issues/177
[dc8cad0]: https://github.com/driftsys/ridl/commit/dc8cad0
[#184]: https://github.com/driftsys/ridl/issues/184
[#182]: https://github.com/driftsys/ridl/issues/182
[da3d246]: https://github.com/driftsys/ridl/commit/da3d246
[#178]: https://github.com/driftsys/ridl/issues/178
[af7ef7c]: https://github.com/driftsys/ridl/commit/af7ef7c
[#176]: https://github.com/driftsys/ridl/issues/176
[ebebaf7]: https://github.com/driftsys/ridl/commit/ebebaf7
[#174]: https://github.com/driftsys/ridl/issues/174
[8705450]: https://github.com/driftsys/ridl/commit/8705450
[#168]: https://github.com/driftsys/ridl/issues/168
[3dc6f30]: https://github.com/driftsys/ridl/commit/3dc6f30
[#164]: https://github.com/driftsys/ridl/issues/164
[516dff0]: https://github.com/driftsys/ridl/commit/516dff0
[#162]: https://github.com/driftsys/ridl/issues/162
[4230fd8]: https://github.com/driftsys/ridl/commit/4230fd8
[#160]: https://github.com/driftsys/ridl/issues/160
[0c8f3ab]: https://github.com/driftsys/ridl/commit/0c8f3ab
[#158]: https://github.com/driftsys/ridl/issues/158
[8eb3421]: https://github.com/driftsys/ridl/commit/8eb3421
[#489]: https://github.com/driftsys/ridl/issues/489
[9069d2c]: https://github.com/driftsys/ridl/commit/9069d2c
[#483]: https://github.com/driftsys/ridl/issues/483
[330e962]: https://github.com/driftsys/ridl/commit/330e962
[#479]: https://github.com/driftsys/ridl/issues/479
[5deab17]: https://github.com/driftsys/ridl/commit/5deab17
[#475]: https://github.com/driftsys/ridl/issues/475
[5f05bdc]: https://github.com/driftsys/ridl/commit/5f05bdc
[#474]: https://github.com/driftsys/ridl/issues/474
[80773b8]: https://github.com/driftsys/ridl/commit/80773b8
[#468]: https://github.com/driftsys/ridl/issues/468
[#463]: https://github.com/driftsys/ridl/issues/463
[c5c582c]: https://github.com/driftsys/ridl/commit/c5c582c
[#466]: https://github.com/driftsys/ridl/issues/466
[261724f]: https://github.com/driftsys/ridl/commit/261724f
[#465]: https://github.com/driftsys/ridl/issues/465
[0ade5b1]: https://github.com/driftsys/ridl/commit/0ade5b1
[20c5f1e]: https://github.com/driftsys/ridl/commit/20c5f1e
[#462]: https://github.com/driftsys/ridl/issues/462
[196989a]: https://github.com/driftsys/ridl/commit/196989a
[#458]: https://github.com/driftsys/ridl/issues/458
[77c5e59]: https://github.com/driftsys/ridl/commit/77c5e59
[#461]: https://github.com/driftsys/ridl/issues/461
[8e5a552]: https://github.com/driftsys/ridl/commit/8e5a552
[#443]: https://github.com/driftsys/ridl/issues/443
[f9b2904]: https://github.com/driftsys/ridl/commit/f9b2904
[#439]: https://github.com/driftsys/ridl/issues/439
[61a6c35]: https://github.com/driftsys/ridl/commit/61a6c35
[#436]: https://github.com/driftsys/ridl/issues/436
[3fdb631]: https://github.com/driftsys/ridl/commit/3fdb631
[#438]: https://github.com/driftsys/ridl/issues/438
[7c9d9c4]: https://github.com/driftsys/ridl/commit/7c9d9c4
[#433]: https://github.com/driftsys/ridl/issues/433
[ddcbe85]: https://github.com/driftsys/ridl/commit/ddcbe85
[#420]: https://github.com/driftsys/ridl/issues/420
[2d26b38]: https://github.com/driftsys/ridl/commit/2d26b38
[#418]: https://github.com/driftsys/ridl/issues/418
[fe4c44b]: https://github.com/driftsys/ridl/commit/fe4c44b
[#417]: https://github.com/driftsys/ridl/issues/417
[564bc14]: https://github.com/driftsys/ridl/commit/564bc14
[#415]: https://github.com/driftsys/ridl/issues/415
[#252]: https://github.com/driftsys/ridl/issues/252
[529d93e]: https://github.com/driftsys/ridl/commit/529d93e
[#413]: https://github.com/driftsys/ridl/issues/413
[#247]: https://github.com/driftsys/ridl/issues/247
[d8b7a59]: https://github.com/driftsys/ridl/commit/d8b7a59
[#412]: https://github.com/driftsys/ridl/issues/412
[#246]: https://github.com/driftsys/ridl/issues/246
[fdcf2ca]: https://github.com/driftsys/ridl/commit/fdcf2ca
[#391]: https://github.com/driftsys/ridl/issues/391
[0504220]: https://github.com/driftsys/ridl/commit/0504220
[#388]: https://github.com/driftsys/ridl/issues/388
[5f24ea7]: https://github.com/driftsys/ridl/commit/5f24ea7
[#351]: https://github.com/driftsys/ridl/issues/351
[#350]: https://github.com/driftsys/ridl/issues/350
[#316]: https://github.com/driftsys/ridl/issues/316
[91e6b62]: https://github.com/driftsys/ridl/commit/91e6b62
[#348]: https://github.com/driftsys/ridl/issues/348
[2fa924c]: https://github.com/driftsys/ridl/commit/2fa924c
[#327]: https://github.com/driftsys/ridl/issues/327
[f2efb61]: https://github.com/driftsys/ridl/commit/f2efb61
[#331]: https://github.com/driftsys/ridl/issues/331
[0199406]: https://github.com/driftsys/ridl/commit/0199406
[#303]: https://github.com/driftsys/ridl/issues/303
[7d539bc]: https://github.com/driftsys/ridl/commit/7d539bc
[#241]: https://github.com/driftsys/ridl/issues/241
[1e674fe]: https://github.com/driftsys/ridl/commit/1e674fe
[#242]: https://github.com/driftsys/ridl/issues/242
[b4c420f]: https://github.com/driftsys/ridl/commit/b4c420f
[#238]: https://github.com/driftsys/ridl/issues/238
[89b8604]: https://github.com/driftsys/ridl/commit/89b8604
[#223]: https://github.com/driftsys/ridl/issues/223
[c560e3d]: https://github.com/driftsys/ridl/commit/c560e3d
[#221]: https://github.com/driftsys/ridl/issues/221
[9a5eb49]: https://github.com/driftsys/ridl/commit/9a5eb49
[#219]: https://github.com/driftsys/ridl/issues/219
[ef3c9a5]: https://github.com/driftsys/ridl/commit/ef3c9a5
[#217]: https://github.com/driftsys/ridl/issues/217
[ded42ef]: https://github.com/driftsys/ridl/commit/ded42ef
[#204]: https://github.com/driftsys/ridl/issues/204
[2513e70]: https://github.com/driftsys/ridl/commit/2513e70
[#199]: https://github.com/driftsys/ridl/issues/199
[a3a4700]: https://github.com/driftsys/ridl/commit/a3a4700
[#188]: https://github.com/driftsys/ridl/issues/188
[#172]: https://github.com/driftsys/ridl/issues/172
[96efb74]: https://github.com/driftsys/ridl/commit/96efb74
[#157]: https://github.com/driftsys/ridl/issues/157
[364dc4b]: https://github.com/driftsys/ridl/commit/364dc4b
[#156]: https://github.com/driftsys/ridl/issues/156
[99fe470]: https://github.com/driftsys/ridl/commit/99fe470
[#155]: https://github.com/driftsys/ridl/issues/155
[6e6abe5]: https://github.com/driftsys/ridl/commit/6e6abe5
[#153]: https://github.com/driftsys/ridl/issues/153
[3729961]: https://github.com/driftsys/ridl/commit/3729961
[#154]: https://github.com/driftsys/ridl/issues/154
[7b3c831]: https://github.com/driftsys/ridl/commit/7b3c831
[#152]: https://github.com/driftsys/ridl/issues/152
[450f2b9]: https://github.com/driftsys/ridl/commit/450f2b9
[#150]: https://github.com/driftsys/ridl/issues/150
[dfcc83b]: https://github.com/driftsys/ridl/commit/dfcc83b
[#149]: https://github.com/driftsys/ridl/issues/149
[5266ccb]: https://github.com/driftsys/ridl/commit/5266ccb
[#151]: https://github.com/driftsys/ridl/issues/151
[00e666a]: https://github.com/driftsys/ridl/commit/00e666a
[#148]: https://github.com/driftsys/ridl/issues/148
[62bc8ed]: https://github.com/driftsys/ridl/commit/62bc8ed
[#146]: https://github.com/driftsys/ridl/issues/146
[469047a]: https://github.com/driftsys/ridl/commit/469047a
[#147]: https://github.com/driftsys/ridl/issues/147
[20fba18]: https://github.com/driftsys/ridl/commit/20fba18
[#220]: https://github.com/driftsys/ridl/issues/220
[4cffb74]: https://github.com/driftsys/ridl/commit/4cffb74
[#181]: https://github.com/driftsys/ridl/issues/181
[#180]: https://github.com/driftsys/ridl/issues/180
[6f15e77]: https://github.com/driftsys/ridl/commit/6f15e77
[#175]: https://github.com/driftsys/ridl/issues/175
[2f8ff9b]: https://github.com/driftsys/ridl/commit/2f8ff9b
[#171]: https://github.com/driftsys/ridl/issues/171
[9684914]: https://github.com/driftsys/ridl/commit/9684914
[#478]: https://github.com/driftsys/ridl/issues/478
[26214fd]: https://github.com/driftsys/ridl/commit/26214fd
[#477]: https://github.com/driftsys/ridl/issues/477
[982c8d8]: https://github.com/driftsys/ridl/commit/982c8d8
[#454]: https://github.com/driftsys/ridl/issues/454
[a7c8051]: https://github.com/driftsys/ridl/commit/a7c8051
[#446]: https://github.com/driftsys/ridl/issues/446
[e44e885]: https://github.com/driftsys/ridl/commit/e44e885
[#447]: https://github.com/driftsys/ridl/issues/447
[512a478]: https://github.com/driftsys/ridl/commit/512a478
[#440]: https://github.com/driftsys/ridl/issues/440
[6fb3456]: https://github.com/driftsys/ridl/commit/6fb3456
[#429]: https://github.com/driftsys/ridl/issues/429
[0b3e69f]: https://github.com/driftsys/ridl/commit/0b3e69f
[#435]: https://github.com/driftsys/ridl/issues/435
[f6b749e]: https://github.com/driftsys/ridl/commit/f6b749e
[#432]: https://github.com/driftsys/ridl/issues/432
[3504a90]: https://github.com/driftsys/ridl/commit/3504a90
[#431]: https://github.com/driftsys/ridl/issues/431
[b6544fe]: https://github.com/driftsys/ridl/commit/b6544fe
[#419]: https://github.com/driftsys/ridl/issues/419
[#393]: https://github.com/driftsys/ridl/issues/393
[566f024]: https://github.com/driftsys/ridl/commit/566f024
[#411]: https://github.com/driftsys/ridl/issues/411
[8746585]: https://github.com/driftsys/ridl/commit/8746585
[#410]: https://github.com/driftsys/ridl/issues/410
[3988701]: https://github.com/driftsys/ridl/commit/3988701
[#409]: https://github.com/driftsys/ridl/issues/409
[25db3a3]: https://github.com/driftsys/ridl/commit/25db3a3
[#408]: https://github.com/driftsys/ridl/issues/408
[6d21c59]: https://github.com/driftsys/ridl/commit/6d21c59
[#401]: https://github.com/driftsys/ridl/issues/401
[#319]: https://github.com/driftsys/ridl/issues/319
[b638e63]: https://github.com/driftsys/ridl/commit/b638e63
[#404]: https://github.com/driftsys/ridl/issues/404
[d3248a1]: https://github.com/driftsys/ridl/commit/d3248a1
[#405]: https://github.com/driftsys/ridl/issues/405
[f3b3b14]: https://github.com/driftsys/ridl/commit/f3b3b14
[#403]: https://github.com/driftsys/ridl/issues/403
[a720d6c]: https://github.com/driftsys/ridl/commit/a720d6c
[#394]: https://github.com/driftsys/ridl/issues/394
[f45ea99]: https://github.com/driftsys/ridl/commit/f45ea99
[#400]: https://github.com/driftsys/ridl/issues/400
[72bb9da]: https://github.com/driftsys/ridl/commit/72bb9da
[#396]: https://github.com/driftsys/ridl/issues/396
[86e10d7]: https://github.com/driftsys/ridl/commit/86e10d7
[#392]: https://github.com/driftsys/ridl/issues/392
[d2e9d41]: https://github.com/driftsys/ridl/commit/d2e9d41
[#390]: https://github.com/driftsys/ridl/issues/390
[4405950]: https://github.com/driftsys/ridl/commit/4405950
[#389]: https://github.com/driftsys/ridl/issues/389
[cc00580]: https://github.com/driftsys/ridl/commit/cc00580
[#383]: https://github.com/driftsys/ridl/issues/383
[f44d475]: https://github.com/driftsys/ridl/commit/f44d475
[#360]: https://github.com/driftsys/ridl/issues/360
[389e7f0]: https://github.com/driftsys/ridl/commit/389e7f0
[#358]: https://github.com/driftsys/ridl/issues/358
[4b11633]: https://github.com/driftsys/ridl/commit/4b11633
[#357]: https://github.com/driftsys/ridl/issues/357
[bcb2855]: https://github.com/driftsys/ridl/commit/bcb2855
[#343]: https://github.com/driftsys/ridl/issues/343
[ab5e0c6]: https://github.com/driftsys/ridl/commit/ab5e0c6
[#333]: https://github.com/driftsys/ridl/issues/333
[05e3100]: https://github.com/driftsys/ridl/commit/05e3100
[#334]: https://github.com/driftsys/ridl/issues/334
[052ec5f]: https://github.com/driftsys/ridl/commit/052ec5f
[#332]: https://github.com/driftsys/ridl/issues/332
[#308]: https://github.com/driftsys/ridl/issues/308
[#309]: https://github.com/driftsys/ridl/issues/309
[#328]: https://github.com/driftsys/ridl/issues/328
[c94191a]: https://github.com/driftsys/ridl/commit/c94191a
[#329]: https://github.com/driftsys/ridl/issues/329
[b1c43fa]: https://github.com/driftsys/ridl/commit/b1c43fa
[#325]: https://github.com/driftsys/ridl/issues/325
[ee3c57d]: https://github.com/driftsys/ridl/commit/ee3c57d
[#324]: https://github.com/driftsys/ridl/issues/324
[fe3e918]: https://github.com/driftsys/ridl/commit/fe3e918
[#323]: https://github.com/driftsys/ridl/issues/323
[52952b3]: https://github.com/driftsys/ridl/commit/52952b3
[#312]: https://github.com/driftsys/ridl/issues/312
[0228902]: https://github.com/driftsys/ridl/commit/0228902
[#313]: https://github.com/driftsys/ridl/issues/313
[69def56]: https://github.com/driftsys/ridl/commit/69def56
[#310]: https://github.com/driftsys/ridl/issues/310
[505914e]: https://github.com/driftsys/ridl/commit/505914e
[e1b55db]: https://github.com/driftsys/ridl/commit/e1b55db
[#240]: https://github.com/driftsys/ridl/issues/240
[8a5b960]: https://github.com/driftsys/ridl/commit/8a5b960
[#239]: https://github.com/driftsys/ridl/issues/239
[851bf99]: https://github.com/driftsys/ridl/commit/851bf99
[#232]: https://github.com/driftsys/ridl/issues/232
[374bdd7]: https://github.com/driftsys/ridl/commit/374bdd7
[#229]: https://github.com/driftsys/ridl/issues/229
[a90962a]: https://github.com/driftsys/ridl/commit/a90962a
[#224]: https://github.com/driftsys/ridl/issues/224
[899b8da]: https://github.com/driftsys/ridl/commit/899b8da
[#222]: https://github.com/driftsys/ridl/issues/222
[3d76965]: https://github.com/driftsys/ridl/commit/3d76965
[#215]: https://github.com/driftsys/ridl/issues/215
[8fae995]: https://github.com/driftsys/ridl/commit/8fae995
[#216]: https://github.com/driftsys/ridl/issues/216
[da5ca78]: https://github.com/driftsys/ridl/commit/da5ca78
[#210]: https://github.com/driftsys/ridl/issues/210
[7e6c4e5]: https://github.com/driftsys/ridl/commit/7e6c4e5
[#209]: https://github.com/driftsys/ridl/issues/209
[361cac0]: https://github.com/driftsys/ridl/commit/361cac0
[#208]: https://github.com/driftsys/ridl/issues/208
[c853c27]: https://github.com/driftsys/ridl/commit/c853c27
[#207]: https://github.com/driftsys/ridl/issues/207
[4ab9dd9]: https://github.com/driftsys/ridl/commit/4ab9dd9
[#206]: https://github.com/driftsys/ridl/issues/206
[6440194]: https://github.com/driftsys/ridl/commit/6440194
[#197]: https://github.com/driftsys/ridl/issues/197
[886f4d7]: https://github.com/driftsys/ridl/commit/886f4d7
[#193]: https://github.com/driftsys/ridl/issues/193
[58b404b]: https://github.com/driftsys/ridl/commit/58b404b
[#187]: https://github.com/driftsys/ridl/issues/187
[b4f7809]: https://github.com/driftsys/ridl/commit/b4f7809
[#183]: https://github.com/driftsys/ridl/issues/183
[06c3f7a]: https://github.com/driftsys/ridl/commit/06c3f7a
[#179]: https://github.com/driftsys/ridl/issues/179
[#169]: https://github.com/driftsys/ridl/issues/169
[256bb52]: https://github.com/driftsys/ridl/commit/256bb52
[#173]: https://github.com/driftsys/ridl/issues/173
[#18]: https://github.com/driftsys/ridl/issues/18
[b32a171]: https://github.com/driftsys/ridl/commit/b32a171
[#165]: https://github.com/driftsys/ridl/issues/165
