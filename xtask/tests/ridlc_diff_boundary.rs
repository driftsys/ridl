//! The guard on the `ridlc` -> `ridl-diff` dependency edge, which ADR-0008
//! decision 9 forbids (driftsys/ridl#786).
//!
//! This guard reads the resolved dependency graph from
//! `cargo metadata --format-version 1 --locked --all-features`, for the
//! reason `oracle_boundary.rs` gives: only the resolved graph records the
//! kind of an edge. `--all-features` puts every optional dependency in that
//! graph, so a `ridl-diff` behind a feature that no member turns on still
//! fails it. The walk follows normal and build edges, direct or through
//! another crate, because cargo compiles both into every build of `ridlc`; it
//! leaves out dev edges only, so a dev-dependency on `ridl-diff` in a
//! `ridlc` test passes it.

mod dep_graph;

use dep_graph::{cargo_metadata, edges, member_id, package_names, path_to, reach};

/// The crate that must not reach [`FORBIDDEN`].
const PACKAGE: &str = "ridlc";

/// The crate [`PACKAGE`] must not reach through normal or build edges.
const FORBIDDEN: &str = "ridl-diff";

#[test]
fn ridlc_does_not_depend_on_ridl_diff() {
    let metadata = cargo_metadata(None, true);
    let names = package_names(&metadata);
    let shipped = edges(&metadata, |kind| kind != Some("dev"));
    let reached = reach(&shipped, &shipped, member_id(&metadata, PACKAGE));
    let path = path_to(&names, &reached, FORBIDDEN);
    assert!(
        path.is_none(),
        "`{FORBIDDEN}` is in `{PACKAGE}`'s normal and build dependency \
         closure: {path}. ADR-0008 decision 9 keeps `{PACKAGE}` off the diff \
         engine (driftsys/ridl#786).",
        path = path.unwrap_or_default(),
    );
}
