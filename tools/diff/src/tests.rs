//! Engine tests over constructed IR v2 packages. The engine never reads
//! source, so every fixture is built directly (ADR-0008 decision 14).

use ridl_ir::v2;

use crate::{Category, Verdict, diff_packages, diff_sets, render_json};

// --------------------------------------------------------------------------
// Builders.
// --------------------------------------------------------------------------

fn interaction(name: &str, ordinal: u32, kind: v2::decl::Kind) -> v2::Decl {
    v2::Decl {
        name: name.to_string(),
        visibility: v2::Visibility::Unspecified as i32,
        is_error: false,
        doc: String::new(),
        labels: Vec::new(),
        deprecated: None,
        ordinal,
        kind: Some(kind),
    }
}

fn signal(name: &str, ordinal: u32, payload: &str) -> v2::Decl {
    interaction(
        name,
        ordinal,
        v2::decl::Kind::SignalDef(v2::SignalDef {
            payload: payload.to_string(),
            declared_init: None,
            init: None,
            timing: None,
        }),
    )
}

fn event(name: &str, ordinal: u32, payload: &str) -> v2::Decl {
    interaction(
        name,
        ordinal,
        v2::decl::Kind::EventDef(v2::EventDef {
            payload: payload.to_string(),
            timing: None,
        }),
    )
}

fn reserved(name: &str, ordinal: u32) -> v2::Decl {
    v2::Decl {
        name: String::new(),
        visibility: v2::Visibility::Unspecified as i32,
        is_error: false,
        doc: String::new(),
        labels: Vec::new(),
        deprecated: None,
        ordinal,
        kind: Some(v2::decl::Kind::ReservedSlot(v2::Reserved {
            ordinal,
            name: Some(name.to_string()),
            value: None,
        })),
    }
}

fn interface(name: &str, interactions: Vec<v2::Decl>) -> v2::Interface {
    v2::Interface {
        name: name.to_string(),
        visibility: v2::Visibility::Public as i32,
        doc: String::new(),
        labels: Vec::new(),
        deprecated: None,
        interactions,
    }
}

fn pkg(name: &str, iface: v2::Interface) -> v2::Package {
    v2::Package {
        name: name.to_string(),
        decls: Vec::new(),
        interfaces: vec![iface],
        services: Vec::new(),
    }
}

// --------------------------------------------------------------------------
// Required task-16 cases.
// --------------------------------------------------------------------------

#[test]
fn identical_snapshots_report_identical() {
    let iface = interface(
        "VehicleStatus",
        vec![
            signal("currentSpeed", 1, "Speed"),
            event("doorOpened", 2, "DoorEvent"),
        ],
    );
    let old = pkg("veh.cluster", iface.clone());
    let new = pkg("veh.cluster", iface);

    let report = diff_packages(&old, &new);
    assert_eq!(report.verdict, Verdict::Identical);
    assert!(
        report.changes.is_empty(),
        "no differences, got {:?}",
        report.changes
    );
}

#[test]
fn a_doc_edit_is_doc_only_and_compatible() {
    let old = pkg(
        "veh.cluster",
        interface("VehicleStatus", vec![signal("currentSpeed", 1, "Speed")]),
    );
    let mut new = old.clone();
    new.interfaces[0].interactions[0].doc = "the current road speed".to_string();

    let report = diff_packages(&old, &new);
    assert_eq!(
        report.changes.len(),
        1,
        "one change, got {:?}",
        report.changes
    );
    assert_eq!(report.changes[0].category, Category::DocOnly);
    assert_eq!(
        report.changes[0].path,
        "veh.cluster/VehicleStatus/currentSpeed"
    );
    assert_eq!(report.verdict, Verdict::Compatible);
}

#[test]
fn appending_an_interaction_is_compatible() {
    let old = pkg(
        "veh.cluster",
        interface("VehicleStatus", vec![signal("currentSpeed", 1, "Speed")]),
    );
    let new = pkg(
        "veh.cluster",
        interface(
            "VehicleStatus",
            vec![
                signal("currentSpeed", 1, "Speed"),
                event("doorOpened", 2, "DoorEvent"),
            ],
        ),
    );

    let report = diff_packages(&old, &new);
    assert_eq!(
        report.changes.len(),
        1,
        "one change, got {:?}",
        report.changes
    );
    assert_eq!(report.changes[0].category, Category::InteractionAppended);
    assert_eq!(
        report.changes[0].path,
        "veh.cluster/VehicleStatus/doorOpened"
    );
    assert_eq!(report.verdict, Verdict::Compatible);
}

#[test]
fn a_payload_type_change_is_breaking() {
    let old = pkg(
        "veh.cluster",
        interface("VehicleStatus", vec![event("doorOpened", 1, "DoorEvent")]),
    );
    let new = pkg(
        "veh.cluster",
        interface("VehicleStatus", vec![event("doorOpened", 1, "DoorState")]),
    );

    let report = diff_packages(&old, &new);
    assert_eq!(
        report.changes.len(),
        1,
        "one change, got {:?}",
        report.changes
    );
    let change = &report.changes[0];
    assert_eq!(change.category, Category::PayloadChanged);
    assert_eq!(change.path, "veh.cluster/VehicleStatus/doorOpened");
    assert_eq!(change.before.as_deref(), Some("DoorEvent"));
    assert_eq!(change.after.as_deref(), Some("DoorState"));
    assert_eq!(report.verdict, Verdict::Breaking);
}

#[test]
fn removing_without_a_tombstone_is_breaking() {
    let old = pkg(
        "veh.cluster",
        interface(
            "VehicleStatus",
            vec![
                signal("currentSpeed", 1, "Speed"),
                event("doorOpened", 2, "DoorEvent"),
            ],
        ),
    );
    let new = pkg(
        "veh.cluster",
        interface("VehicleStatus", vec![signal("currentSpeed", 1, "Speed")]),
    );

    let report = diff_packages(&old, &new);
    assert_eq!(
        report.changes.len(),
        1,
        "one change, got {:?}",
        report.changes
    );
    assert_eq!(report.changes[0].category, Category::InteractionRemoved);
    assert_eq!(
        report.changes[0].path,
        "veh.cluster/VehicleStatus/doorOpened"
    );
    assert_eq!(report.verdict, Verdict::Breaking);
}

#[test]
fn removing_with_a_tombstone_is_compatible() {
    let old = pkg(
        "veh.cluster",
        interface(
            "VehicleStatus",
            vec![
                signal("currentSpeed", 1, "Speed"),
                event("doorOpened", 2, "DoorEvent"),
            ],
        ),
    );
    let new = pkg(
        "veh.cluster",
        interface(
            "VehicleStatus",
            vec![
                signal("currentSpeed", 1, "Speed"),
                reserved("doorOpened", 2),
            ],
        ),
    );

    let report = diff_packages(&old, &new);
    assert_eq!(
        report.changes.len(),
        1,
        "one change, got {:?}",
        report.changes
    );
    assert_eq!(report.changes[0].category, Category::InteractionRetired);
    assert_eq!(
        report.changes[0].path,
        "veh.cluster/VehicleStatus/doorOpened"
    );
    assert_eq!(report.verdict, Verdict::Compatible);
}

#[test]
fn a_new_package_is_compatible_and_a_dropped_one_is_breaking() {
    let a = pkg("veh.a", interface("A", vec![signal("s", 1, "Speed")]));
    let b = pkg("veh.b", interface("B", vec![signal("s", 1, "Speed")]));

    let added = diff_sets(std::slice::from_ref(&a), &[a.clone(), b.clone()]);
    assert_eq!(
        added.changes.len(),
        1,
        "one change, got {:?}",
        added.changes
    );
    assert_eq!(added.changes[0].category, Category::DeclAdded);
    assert_eq!(added.changes[0].path, "veh.b");
    assert_eq!(added.verdict, Verdict::Compatible);

    let removed = diff_sets(&[a.clone(), b.clone()], std::slice::from_ref(&a));
    assert_eq!(
        removed.changes.len(),
        1,
        "one change, got {:?}",
        removed.changes
    );
    assert_eq!(removed.changes[0].category, Category::DeclRemoved);
    assert_eq!(removed.changes[0].path, "veh.b");
    assert_eq!(removed.verdict, Verdict::Breaking);
}

// --------------------------------------------------------------------------
// Ordinal analysis — supports the E2.8b classifier (task 17).
// --------------------------------------------------------------------------

#[test]
fn reordering_interactions_is_breaking() {
    let old = pkg(
        "veh.cluster",
        interface(
            "I",
            vec![
                signal("a", 1, "T"),
                signal("b", 2, "T"),
                signal("c", 3, "T"),
            ],
        ),
    );
    // b and c swap places.
    let new = pkg(
        "veh.cluster",
        interface(
            "I",
            vec![
                signal("a", 1, "T"),
                signal("c", 2, "T"),
                signal("b", 3, "T"),
            ],
        ),
    );

    let report = diff_packages(&old, &new);
    let reorders: Vec<_> = report
        .changes
        .iter()
        .filter(|change| change.category == Category::InteractionReordered)
        .collect();
    assert_eq!(
        reorders.len(),
        2,
        "both moved names flagged, got {:?}",
        report.changes
    );
    assert_eq!(report.verdict, Verdict::Breaking);
}

#[test]
fn inserting_before_the_end_is_breaking_without_reorder_noise() {
    let old = pkg(
        "veh.cluster",
        interface("I", vec![signal("a", 1, "T"), signal("b", 2, "T")]),
    );
    // x is inserted between a and b, shifting b from ordinal 2 to 3.
    let new = pkg(
        "veh.cluster",
        interface(
            "I",
            vec![
                signal("a", 1, "T"),
                signal("x", 2, "T"),
                signal("b", 3, "T"),
            ],
        ),
    );

    let report = diff_packages(&old, &new);
    let inserted: Vec<_> = report
        .changes
        .iter()
        .filter(|change| change.category == Category::InteractionInserted)
        .collect();
    assert_eq!(inserted.len(), 1, "one insert, got {:?}", report.changes);
    assert_eq!(inserted[0].path, "veh.cluster/I/x");
    // The shift of b is a consequence of the insert, not an independent reorder.
    assert!(
        report
            .changes
            .iter()
            .all(|change| change.category != Category::InteractionReordered),
        "no reorder noise, got {:?}",
        report.changes
    );
    assert_eq!(report.verdict, Verdict::Breaking);
}

#[test]
fn redeclaring_a_reserved_name_is_breaking() {
    let old = pkg(
        "veh.cluster",
        interface("I", vec![signal("a", 1, "T"), reserved("b", 2)]),
    );
    let new = pkg(
        "veh.cluster",
        interface("I", vec![signal("a", 1, "T"), signal("b", 2, "T")]),
    );

    let report = diff_packages(&old, &new);
    assert!(
        report
            .changes
            .iter()
            .any(|change| change.category == Category::ReservedNameRedeclared),
        "reserved name redeclared, got {:?}",
        report.changes
    );
    assert_eq!(report.verdict, Verdict::Breaking);
}

#[test]
fn a_kind_change_is_breaking() {
    let old = pkg(
        "veh.cluster",
        interface("I", vec![signal("doorOpened", 1, "DoorEvent")]),
    );
    let new = pkg(
        "veh.cluster",
        interface("I", vec![event("doorOpened", 1, "DoorEvent")]),
    );

    let report = diff_packages(&old, &new);
    assert_eq!(
        report.changes.len(),
        1,
        "one change, got {:?}",
        report.changes
    );
    assert_eq!(report.changes[0].category, Category::KindChanged);
    assert_eq!(report.changes[0].before.as_deref(), Some("signal"));
    assert_eq!(report.changes[0].after.as_deref(), Some("event"));
    assert_eq!(report.verdict, Verdict::Breaking);
}

// --------------------------------------------------------------------------
// Rendering — the stable JSON schema.
// --------------------------------------------------------------------------

#[test]
fn render_json_matches_the_stable_schema() {
    let old = pkg(
        "veh.cluster",
        interface("VehicleStatus", vec![event("doorOpened", 1, "DoorEvent")]),
    );
    let new = pkg(
        "veh.cluster",
        interface("VehicleStatus", vec![event("doorOpened", 1, "DoorState")]),
    );

    let report = diff_packages(&old, &new);
    let value: serde_json::Value =
        serde_json::from_str(&render_json(&report)).expect("render_json emits valid JSON");

    assert_eq!(value["verdict"], "breaking");
    let changes = value["changes"].as_array().expect("changes is an array");
    assert_eq!(changes.len(), 1);
    let change = &changes[0];
    assert_eq!(change["path"], "veh.cluster/VehicleStatus/doorOpened");
    assert_eq!(change["category"], "payload_changed");
    assert_eq!(change["verdict"], "breaking");
    assert_eq!(change["before"], "DoorEvent");
    assert_eq!(change["after"], "DoorState");
}

// --------------------------------------------------------------------------
// Tombstone slot integrity (ridl §11).
//
// A tombstone holds the ordinal it retired. A tombstone edit that frees a slot
// lets the surviving interactions slide into it — a wire-identity shift, which
// ADR-0008 decision 14 lists first among breaking changes. Because a straight
// retirement is compatible, these shifts must reach a `Change` here or the
// classifier downstream has nothing to judge.
// --------------------------------------------------------------------------

/// Retiring `b` but writing `reserved b` at the end frees ordinal 2, so the
/// surviving `c` slides 3 -> 2. This is not a compatible retirement.
#[test]
fn a_tombstone_written_out_of_its_slot_is_breaking() {
    let old = pkg(
        "veh.cluster",
        interface(
            "I",
            vec![
                signal("a", 1, "T"),
                signal("b", 2, "T"),
                signal("c", 3, "T"),
            ],
        ),
    );
    let new = pkg(
        "veh.cluster",
        interface(
            "I",
            vec![signal("a", 1, "T"), signal("c", 2, "T"), reserved("b", 3)],
        ),
    );

    let report = diff_packages(&old, &new);
    assert_eq!(
        report.verdict,
        Verdict::Breaking,
        "an out-of-slot tombstone frees a wire slot, got {:?}",
        report.changes
    );
    assert!(
        report
            .changes
            .iter()
            .all(|change| change.category != Category::InteractionRetired),
        "an out-of-slot tombstone is not a compatible retirement, got {:?}",
        report.changes
    );
}

/// Deleting a `reserved b` tombstone releases ordinal 2, so `c` slides 3 -> 2.
/// A wire reservation is permanent (ridl §11).
#[test]
fn dropping_a_tombstone_is_breaking() {
    let old = pkg(
        "veh.cluster",
        interface(
            "I",
            vec![signal("a", 1, "T"), reserved("b", 2), signal("c", 3, "T")],
        ),
    );
    let new = pkg(
        "veh.cluster",
        interface("I", vec![signal("a", 1, "T"), signal("c", 2, "T")]),
    );

    let report = diff_packages(&old, &new);
    assert_eq!(
        report.verdict,
        Verdict::Breaking,
        "a dropped tombstone frees a wire slot, got {:?}",
        report.changes
    );
    assert!(
        report.changes.iter().any(|change| {
            change.category == Category::InteractionRemoved && change.path == "veh.cluster/I/b"
        }),
        "the dropped tombstone is itself a change, got {:?}",
        report.changes
    );
}

/// Moving a tombstone to a different slot shifts the interactions between the
/// two positions.
#[test]
fn moving_a_tombstone_is_breaking() {
    let old = pkg(
        "veh.cluster",
        interface(
            "I",
            vec![signal("a", 1, "T"), reserved("b", 2), signal("c", 3, "T")],
        ),
    );
    let new = pkg(
        "veh.cluster",
        interface(
            "I",
            vec![signal("a", 1, "T"), signal("c", 2, "T"), reserved("b", 3)],
        ),
    );

    let report = diff_packages(&old, &new);
    assert_eq!(
        report.verdict,
        Verdict::Breaking,
        "a moved tombstone shifts wire identities, got {:?}",
        report.changes
    );
}

/// Control: a tombstone written in the retired interaction's own slot keeps
/// every later ordinal, so the retirement stays compatible.
#[test]
fn a_tombstone_written_in_its_slot_is_compatible() {
    let old = pkg(
        "veh.cluster",
        interface(
            "I",
            vec![
                signal("a", 1, "T"),
                signal("b", 2, "T"),
                signal("c", 3, "T"),
            ],
        ),
    );
    let new = pkg(
        "veh.cluster",
        interface(
            "I",
            vec![signal("a", 1, "T"), reserved("b", 2), signal("c", 3, "T")],
        ),
    );

    let report = diff_packages(&old, &new);
    assert_eq!(
        report.changes.len(),
        1,
        "one change, got {:?}",
        report.changes
    );
    assert_eq!(report.changes[0].category, Category::InteractionRetired);
    assert_eq!(report.verdict, Verdict::Compatible);
}

/// Control: retiring the last interaction shifts nothing.
#[test]
fn retiring_the_last_interaction_is_compatible() {
    let old = pkg(
        "veh.cluster",
        interface("I", vec![signal("a", 1, "T"), signal("b", 2, "T")]),
    );
    let new = pkg(
        "veh.cluster",
        interface("I", vec![signal("a", 1, "T"), reserved("b", 2)]),
    );

    let report = diff_packages(&old, &new);
    assert_eq!(
        report.changes.len(),
        1,
        "one change, got {:?}",
        report.changes
    );
    assert_eq!(report.changes[0].category, Category::InteractionRetired);
    assert_eq!(report.verdict, Verdict::Compatible);
}

/// Control: an untouched tombstone is no change at all.
#[test]
fn an_unchanged_tombstone_is_identical() {
    let iface = interface(
        "I",
        vec![signal("a", 1, "T"), reserved("b", 2), signal("c", 3, "T")],
    );
    let old = pkg("veh.cluster", iface.clone());
    let new = pkg("veh.cluster", iface);

    let report = diff_packages(&old, &new);
    assert_eq!(report.verdict, Verdict::Identical);
    assert!(report.changes.is_empty(), "got {:?}", report.changes);
}

/// A tombstone minted mid-body for a name that was never live pushes every
/// later interaction down a slot — the same wire break as an inserted
/// interaction.
#[test]
fn a_fresh_tombstone_before_the_end_is_breaking() {
    let old = pkg(
        "veh.cluster",
        interface("I", vec![signal("a", 1, "T"), signal("b", 2, "T")]),
    );
    let new = pkg(
        "veh.cluster",
        interface(
            "I",
            vec![signal("a", 1, "T"), reserved("z", 2), signal("b", 3, "T")],
        ),
    );

    let report = diff_packages(&old, &new);
    assert_eq!(
        report.verdict,
        Verdict::Breaking,
        "a fresh mid-body tombstone shifts later ordinals, got {:?}",
        report.changes
    );
    assert!(
        report.changes.iter().any(|change| {
            change.category == Category::InteractionInserted && change.path == "veh.cluster/I/z"
        }),
        "the minted tombstone is reported at its own path, got {:?}",
        report.changes
    );
}

/// Control: a fresh tombstone appended at the end reserves an unused slot and
/// shifts nothing.
#[test]
fn a_fresh_tombstone_at_the_end_is_compatible() {
    let old = pkg(
        "veh.cluster",
        interface("I", vec![signal("a", 1, "T"), signal("b", 2, "T")]),
    );
    let new = pkg(
        "veh.cluster",
        interface(
            "I",
            vec![signal("a", 1, "T"), signal("b", 2, "T"), reserved("z", 3)],
        ),
    );

    let report = diff_packages(&old, &new);
    assert_eq!(
        report.changes.len(),
        1,
        "one change, got {:?}",
        report.changes
    );
    assert_eq!(report.changes[0].category, Category::InteractionAppended);
    assert_eq!(report.verdict, Verdict::Compatible);
}
