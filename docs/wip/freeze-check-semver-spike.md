# cargo-semver-checks spike

- Version: cargo-semver-checks 0.51.0, installed with
  `cargo install cargo-semver-checks --locked`.
- Command:
  `cargo semver-checks --package ridl-rt --baseline-rev 'bc58189e~1' --release-type minor`
  with toolchain 1.98.1 (rust-toolchain.toml). Without `--release-type minor`,
  the tool reads the version change from 0.8.0 to 0.9.0 as a major release and
  exits 0.
- Result: it built both rustdoc JSON files without a format error and exited
  with code 100, reporting 8 major breaks in `ridl-rt` (for example the removed
  `Encoding::max_size` method and `PayloadInfo::max_size` field, and the added
  `Payload::Buffer`, `Payload::BUFFER` and `Interaction::ROW` trait items).
