//! Engine tests over constructed IR v2 packages. The engine never reads
//! source, so every fixture is built directly (ADR-0008 decision 14).

use ridl_ir::v2;

use crate::{Category, Change, Verdict, diff_packages, diff_sets, render_json};

// --------------------------------------------------------------------------
// Builders.
// --------------------------------------------------------------------------

fn interaction(name: &str, ordinal: u32, kind: v2::decl::Kind) -> v2::Decl {
    v2::Decl {
        limit: None,
        name: name.to_string(),
        visibility: v2::Visibility::Unspecified as i32,
        is_error: false,
        doc: String::new(),
        labels: Vec::new(),
        deprecated: None,
        ordinal,
        kind: Some(kind),
        links: Vec::new(),
        see: Vec::new(),
        since: Vec::new(),
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
            family: ridl_ir::v2::Family::Dispatch as i32,
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
            family: ridl_ir::v2::Family::Dispatch as i32,
        }),
    )
}

fn reserved(name: &str, ordinal: u32) -> v2::Decl {
    v2::Decl {
        limit: None,
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
        links: Vec::new(),
        see: Vec::new(),
        since: Vec::new(),
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
        number: 0,
        provisional: false,
        links: Vec::new(),
        see: Vec::new(),
        since: Vec::new(),
    }
}

fn pkg(name: &str, iface: v2::Interface) -> v2::Package {
    v2::Package {
        name: name.to_string(),
        decls: Vec::new(),
        interfaces: vec![iface],
        services: Vec::new(),
        retired: Vec::new(),
        unit: String::new(),
    }
}

#[test]
fn a_limit_only_change_reports_exactly_one_breaking_change() {
    let mut old_signal = signal("a", 1, "T");
    old_signal.limit = None;
    let mut new_signal = signal("a", 1, "T");
    new_signal.limit = Some(64);
    let old = pkg("veh.cluster", interface("I", vec![old_signal]));
    let new = pkg("veh.cluster", interface("I", vec![new_signal]));
    let report = diff_packages(&old, &new);
    assert_eq!(report.changes.len(), 1, "{:?}", report.changes);
    let change = &report.changes[0];
    assert_eq!(change.category, Category::LimitChanged);
    assert_eq!(change.verdict, Verdict::Breaking);
    assert_eq!(change.before.as_deref(), Some("none"));
    assert_eq!(change.after.as_deref(), Some("64"));
    assert_eq!(crate::heading(Category::LimitChanged), None);
}

#[test]
fn a_kind_change_that_also_changes_the_limit_reports_only_the_kind_change() {
    let mut old_signal = signal("a", 1, "T");
    old_signal.limit = Some(64);
    let mut new_event = event("a", 1, "T");
    new_event.limit = Some(128);
    let old = pkg("veh.cluster", interface("I", vec![old_signal]));
    let new = pkg("veh.cluster", interface("I", vec![new_event]));
    let report = diff_packages(&old, &new);
    assert_eq!(report.changes.len(), 1, "{:?}", report.changes);
    assert_eq!(report.changes[0].category, Category::KindChanged);
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
// Ordinal analysis — supports the classifier.
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

// --------------------------------------------------------------------------
// Projection contract property 3, at the name level (ADR-0016).
// --------------------------------------------------------------------------

/// For any delta the classifier calls compatible, a surviving member's raw
/// name — and therefore its projection, since the transform is a pure
/// function of the name — is left untouched.
///
/// This does not pin that `snake_case` is applied at all, or applied
/// correctly: a compatible delta cannot move a raw name today, so the
/// equality checked below is vacuous with respect to the transform itself —
/// it would hold under any deterministic function of the name, not
/// `snake_case` specifically. The transform's own behaviour is pinned
/// separately, by the unit tests in `crates/ridl-ir/src/name.rs`.
///
/// Names are the only identity assigned today; when a
/// projection assigns numbers, this arm gains real content: a
/// projection can assign a number that a compatible delta moves
/// independently of the name it numbers.
///
/// The deltas are the compatible ones the classifier recognises: appending an
/// interaction into a never-occupied slot, and retiring one to a tombstone.
#[test]
fn a_compatible_delta_moves_no_projected_name() {
    let base = vec![
        signal("currentSpeed", 1, "Speed"),
        signal("parseHTTPResponse", 2, "Ratio"),
    ];

    let appended = {
        let mut v = base.clone();
        v.push(signal("wheelTicks", 3, "Ratio"));
        v
    };
    let retired = vec![
        signal("currentSpeed", 1, "Speed"),
        reserved("parseHTTPResponse", 2),
    ];

    for (label, new_members) in [("append", appended), ("retire", retired)] {
        let old = pkg("veh.cluster", interface("VehicleStatus", base.clone()));
        let new = pkg("veh.cluster", interface("VehicleStatus", new_members));

        let report = diff_packages(&old, &new);
        assert_eq!(
            report.verdict,
            Verdict::Compatible,
            "{label}: fixture is not a compatible delta"
        );

        // Every member the delta kept must project to the same name it did
        // before. A tombstone carries no name to project.
        for old_member in &old.interfaces[0].interactions {
            let survivor = new.interfaces[0]
                .interactions
                .iter()
                .find(|m| m.name == old_member.name && !m.name.is_empty());
            if let Some(survivor) = survivor {
                assert_eq!(
                    ridl_ir::name::snake_case(&old_member.name),
                    ridl_ir::name::snake_case(&survivor.name),
                    "{label}: `{}` changed projection under a compatible delta",
                    old_member.name
                );
            }
        }
    }

    // The discriminating arm. Without it the loop above could pass because
    // nothing in the fixtures ever moves a projection, rather than because
    // compatibility is what preserves it. A rename moves the projection, and
    // the classifier calls a rename breaking — so the two sides of the
    // invariant are both exercised.
    let renamed = vec![
        signal("vehicleSpeed", 1, "Speed"),
        signal("parseHTTPResponse", 2, "Ratio"),
    ];
    let old = pkg("veh.cluster", interface("VehicleStatus", base.clone()));
    let new = pkg("veh.cluster", interface("VehicleStatus", renamed));

    assert_eq!(
        diff_packages(&old, &new).verdict,
        Verdict::Breaking,
        "a rename must classify breaking, or the invariant above is vacuous"
    );
    assert_ne!(
        ridl_ir::name::snake_case("currentSpeed"),
        ridl_ir::name::snake_case("vehicleSpeed"),
        "the rename must move the projection, or this arm proves nothing"
    );
}

// --------------------------------------------------------------------------
// Interface identity — the number from `interfaces.lock` (lock design §7,
// plan decisions PD-1 and PD-14). One test per row of the design's §7 table,
// rows 1 to 8, then the cases PD-1 adds.
// --------------------------------------------------------------------------

/// An interface with a frozen number from the lock.
fn frozen(name: &str, number: u32, interactions: Vec<v2::Decl>) -> v2::Interface {
    v2::Interface {
        number,
        provisional: false,
        ..interface(name, interactions)
    }
}

/// An interface with a provisional number — a declaration with no lock entry.
fn provisional(name: &str, number: u32, interactions: Vec<v2::Decl>) -> v2::Interface {
    v2::Interface {
        number,
        provisional: true,
        ..interface(name, interactions)
    }
}

/// A package holding `interfaces`, the inline-form `services`, and the
/// lock's retired entries.
fn package(
    interfaces: Vec<v2::Interface>,
    services: Vec<v2::Service>,
    retired: Vec<v2::RetiredInterface>,
) -> v2::Package {
    v2::Package {
        name: "veh.cluster".to_string(),
        decls: Vec::new(),
        interfaces,
        services,
        retired,
        unit: String::new(),
    }
}

/// An inline-form service whose shape carries a frozen number — the lock
/// entry keyed `service:<name>`. The shape's own name is `""` and its own
/// visibility unspecified, both by construction (ridl §14.5).
fn inline_service(name: &str, number: u32, interactions: Vec<v2::Decl>) -> v2::Service {
    v2::Service {
        name: name.to_string(),
        visibility: v2::Visibility::Public as i32,
        doc: String::new(),
        labels: Vec::new(),
        deprecated: None,
        shapes: vec![v2::ServiceShape {
            kind: Some(v2::service_shape::Kind::Inline(v2::Interface {
                visibility: v2::Visibility::Unspecified as i32,
                ..frozen("", number, interactions)
            })),
        }],
        links: Vec::new(),
        see: Vec::new(),
        since: Vec::new(),
    }
}

fn retired(name: &str, number: u32) -> v2::RetiredInterface {
    v2::RetiredInterface {
        name: name.to_string(),
        number,
    }
}

fn change(
    path: &str,
    category: Category,
    verdict: Verdict,
    before: Option<&str>,
    after: Option<&str>,
) -> Change {
    Change {
        path: path.to_string(),
        category,
        verdict,
        before: before.map(str::to_string),
        after: after.map(str::to_string),
    }
}

fn door_opened(payload: &str) -> Vec<v2::Decl> {
    vec![event("doorOpened", 1, payload)]
}

/// Row 1: the same frozen number on both sides is one interface, and its
/// interactions are diffed exactly as before the lock.
#[test]
fn a_matched_number_diffs_the_interactions_as_today() {
    let old = package(
        vec![frozen("Doors", 1, door_opened("DoorEvent"))],
        vec![],
        vec![],
    );
    let new = package(
        vec![frozen("Doors", 1, door_opened("DoorState"))],
        vec![],
        vec![],
    );
    let report = diff_packages(&old, &new);
    assert_eq!(
        report.changes,
        vec![change(
            "veh.cluster/Doors/doorOpened",
            Category::PayloadChanged,
            Verdict::Breaking,
            Some("DoorEvent"),
            Some("DoorState"),
        )]
    );
}

/// Row 2: the same number under a new name is `InterfaceRenamed`, compatible
/// — the number is the routing identity — and listed under the heading; the
/// path carries the new name (PD-14).
#[test]
fn a_rename_on_the_same_number_is_compatible_under_the_heading() {
    let old = package(
        vec![frozen("Doors", 1, door_opened("DoorState"))],
        vec![],
        vec![],
    );
    let new = package(
        vec![frozen("DoorStatus", 1, door_opened("DoorState"))],
        vec![],
        vec![],
    );
    let report = diff_packages(&old, &new);
    assert_eq!(report.verdict, Verdict::Compatible);
    assert_eq!(
        report.changes,
        vec![change(
            "veh.cluster/DoorStatus",
            Category::InterfaceRenamed,
            Verdict::Compatible,
            Some("Doors"),
            Some("DoorStatus"),
        )]
    );
    assert_eq!(
        crate::heading(Category::InterfaceRenamed),
        Some("compatible on the wire, visible in source")
    );
    assert_eq!(crate::heading(Category::InterfaceRetired), None);
}

/// Row 3: a frozen number the old side never held is a new interface.
#[test]
fn a_fresh_frozen_number_is_decl_added() {
    let old = package(
        vec![frozen("Doors", 1, door_opened("DoorState"))],
        vec![],
        vec![],
    );
    let new = package(
        vec![
            frozen("Doors", 1, door_opened("DoorState")),
            frozen("Lights", 2, vec![event("on", 1, "DoorState")]),
        ],
        vec![],
        vec![],
    );
    let report = diff_packages(&old, &new);
    assert_eq!(report.verdict, Verdict::Compatible);
    assert_eq!(
        report.changes,
        vec![change(
            "veh.cluster/Lights",
            Category::DeclAdded,
            Verdict::Compatible,
            None,
            Some("interface"),
        )]
    );
}

/// Row 4, and the second PD-1 case: a provisional number is no identity, so a
/// frozen old number is never matched to a new provisional interface — not
/// even one spelled the same and numbered the same. The old one is
/// `DeclRemoved`, the new one `DeclAdded`, breaking, until `ridl lock` records
/// the number.
#[test]
fn a_provisional_interface_is_decl_added_and_never_matched() {
    let old = package(
        vec![frozen("Doors", 1, door_opened("DoorState"))],
        vec![],
        vec![],
    );
    let new = package(
        vec![provisional("Doors", 1, door_opened("DoorState"))],
        vec![],
        vec![],
    );
    let report = diff_packages(&old, &new);
    assert_eq!(report.verdict, Verdict::Breaking);
    assert_eq!(
        report.changes,
        vec![
            change(
                "veh.cluster/Doors",
                Category::DeclRemoved,
                Verdict::Breaking,
                Some("interface"),
                None,
            ),
            change(
                "veh.cluster/Doors",
                Category::DeclAdded,
                Verdict::Compatible,
                None,
                Some("interface"),
            ),
        ]
    );
}

/// Row 5: a number gone from the new side and listed in its retired entries
/// is the sanctioned removal, `InterfaceRetired`, compatible.
#[test]
fn a_number_in_the_retired_list_is_interface_retired() {
    let lights = || frozen("Lights", 2, vec![event("on", 1, "DoorState")]);
    let old = package(
        vec![frozen("Doors", 1, door_opened("DoorState")), lights()],
        vec![],
        vec![],
    );
    let new = package(vec![lights()], vec![], vec![retired("Doors", 1)]);
    let report = diff_packages(&old, &new);
    assert_eq!(report.verdict, Verdict::Compatible);
    assert_eq!(
        report.changes,
        vec![change(
            "veh.cluster/Doors",
            Category::InterfaceRetired,
            Verdict::Compatible,
            Some("interface"),
            Some("retired"),
        )]
    );
}

/// Row 6: a number gone from the new side and not retired there is
/// `DeclRemoved`, breaking — the number could be allocated again.
#[test]
fn a_number_absent_and_not_retired_is_decl_removed() {
    let lights = || frozen("Lights", 2, vec![event("on", 1, "DoorState")]);
    let old = package(
        vec![frozen("Doors", 1, door_opened("DoorState")), lights()],
        vec![],
        vec![],
    );
    let new = package(vec![lights()], vec![], vec![]);
    let report = diff_packages(&old, &new);
    assert_eq!(report.verdict, Verdict::Breaking);
    assert_eq!(
        report.changes,
        vec![change(
            "veh.cluster/Doors",
            Category::DeclRemoved,
            Verdict::Breaking,
            Some("interface"),
            None,
        )]
    );
}

/// Row 7: the same name on another frozen number is a removal plus an
/// addition — two existing categories, breaking, and no third one.
#[test]
fn a_hand_changed_number_is_removed_plus_added() {
    let old = package(
        vec![frozen("Doors", 1, door_opened("DoorState"))],
        vec![],
        vec![],
    );
    let new = package(
        vec![frozen("Doors", 2, door_opened("DoorState"))],
        vec![],
        vec![],
    );
    let report = diff_packages(&old, &new);
    assert_eq!(report.verdict, Verdict::Breaking);
    assert_eq!(
        report.changes,
        vec![
            change(
                "veh.cluster/Doors",
                Category::DeclRemoved,
                Verdict::Breaking,
                Some("interface"),
                None,
            ),
            change(
                "veh.cluster/Doors",
                Category::DeclAdded,
                Verdict::Compatible,
                None,
                Some("interface"),
            ),
        ]
    );
}

/// Row 8: a snapshot published before the lock existed carries `number` 0,
/// which is never allocated, so its interfaces are matched by name — the one
/// transition case — whatever number the new side froze.
#[test]
fn a_published_number_zero_is_matched_by_name() {
    let old = package(
        vec![interface("Doors", door_opened("DoorEvent"))],
        vec![],
        vec![],
    );
    let new = package(
        vec![frozen("Doors", 3, door_opened("DoorState"))],
        vec![],
        vec![],
    );
    let report = diff_packages(&old, &new);
    assert_eq!(
        report.changes,
        vec![change(
            "veh.cluster/Doors/doorOpened",
            Category::PayloadChanged,
            Verdict::Breaking,
            Some("DoorEvent"),
            Some("DoorState"),
        )]
    );
}

/// The first PD-1 case: two source trees compiled with no lock file lower
/// every interface provisional on both sides. An old interface with no
/// identity is matched by name and its body is diffed as any matched pair's.
/// A sibling added before it in byte order moves its provisional number, and
/// the number is the routing key, so the moved number is
/// `InterfaceNumberChanged`, breaking (driftsys/ridl#700).
#[test]
fn a_provisional_old_side_is_matched_by_name() {
    let old = package(
        vec![provisional("Doors", 1, door_opened("DoorState"))],
        vec![],
        vec![],
    );
    let new = package(
        vec![
            provisional("Aux", 1, vec![event("on", 1, "DoorState")]),
            provisional(
                "Doors",
                2,
                vec![
                    event("doorOpened", 1, "DoorState"),
                    event("doorClosed", 2, "DoorState"),
                ],
            ),
        ],
        vec![],
        vec![],
    );
    let report = diff_packages(&old, &new);
    assert_eq!(report.verdict, Verdict::Breaking);
    assert_eq!(
        report.changes,
        vec![
            change(
                "veh.cluster/Doors",
                Category::InterfaceNumberChanged,
                Verdict::Breaking,
                Some("1 (provisional)"),
                Some("2 (provisional)"),
            ),
            change(
                "veh.cluster/Doors/doorClosed",
                Category::InteractionAppended,
                Verdict::Compatible,
                None,
                Some("event doorClosed"),
            ),
            change(
                "veh.cluster/Aux",
                Category::DeclAdded,
                Verdict::Compatible,
                None,
                Some("interface"),
            ),
        ]
    );
}

/// `ridl lock` freezes a provisional number in place: the pair is matched by
/// name, the number is the same on both sides, and the provisional flag went
/// from true to false. Nothing moves on the wire, but the catalog hash covers
/// the flag, so the change is `InterfaceFrozen`, compatible, under its own
/// heading (driftsys/ridl#700).
#[test]
fn a_provisional_number_frozen_in_place_is_interface_frozen() {
    let old = package(
        vec![provisional("Doors", 1, door_opened("DoorState"))],
        vec![],
        vec![],
    );
    let new = package(
        vec![frozen("Doors", 1, door_opened("DoorState"))],
        vec![],
        vec![],
    );
    let report = diff_packages(&old, &new);
    assert_eq!(report.verdict, Verdict::Compatible);
    assert_eq!(
        report.changes,
        vec![change(
            "veh.cluster/Doors",
            Category::InterfaceFrozen,
            Verdict::Compatible,
            Some("1 (provisional)"),
            Some("1"),
        )]
    );
    assert_eq!(
        crate::heading(Category::InterfaceFrozen),
        Some("compatible on the wire, may change the catalog hash")
    );
    assert_eq!(crate::heading(Category::InterfaceNumberChanged), None);
}

/// A provisional number frozen as another number — the lock allocated a
/// number other than the provisional one — moves the routing key:
/// `InterfaceNumberChanged`, breaking, and no `InterfaceFrozen` beside it.
#[test]
fn a_provisional_number_frozen_as_another_number_is_interface_number_changed() {
    let old = package(
        vec![provisional("Doors", 2, door_opened("DoorState"))],
        vec![],
        vec![],
    );
    let new = package(
        vec![frozen("Doors", 1, door_opened("DoorState"))],
        vec![],
        vec![],
    );
    let report = diff_packages(&old, &new);
    assert_eq!(report.verdict, Verdict::Breaking);
    assert_eq!(
        report.changes,
        vec![change(
            "veh.cluster/Doors",
            Category::InterfaceNumberChanged,
            Verdict::Breaking,
            Some("2 (provisional)"),
            Some("1"),
        )]
    );
}

/// An old number 0 marks a snapshot written before the lock existed. It is
/// never allocated, so it is not compared with the new side's number or flag:
/// a pair matched by name across that transition reports nothing of its own.
#[test]
fn an_old_number_zero_is_not_compared() {
    let old = package(
        vec![interface("Doors", door_opened("DoorState"))],
        vec![],
        vec![],
    );
    for new_interface in [
        provisional("Doors", 2, door_opened("DoorState")),
        frozen("Doors", 3, door_opened("DoorState")),
    ] {
        let new = package(vec![new_interface], vec![], vec![]);
        let report = diff_packages(&old, &new);
        assert_eq!(report.verdict, Verdict::Identical);
        assert_eq!(report.changes, vec![]);
    }
}

/// Two provisional builds with the same number and the same flag report
/// nothing for the interface itself.
#[test]
fn an_unchanged_provisional_number_is_identical() {
    let side = || {
        package(
            vec![provisional("Doors", 1, door_opened("DoorState"))],
            vec![],
            vec![],
        )
    };
    let report = diff_packages(&side(), &side());
    assert_eq!(report.verdict, Verdict::Identical);
    assert_eq!(report.changes, vec![]);
}

/// A new number 0 is the reversed comparison: the new side is a snapshot
/// written before the lock existed. That number was never allocated either, so
/// it is not compared with the old side's number or flag.
#[test]
fn a_new_number_zero_is_not_compared() {
    let new = package(
        vec![interface("Doors", door_opened("DoorState"))],
        vec![],
        vec![],
    );
    let old = package(
        vec![provisional("Doors", 2, door_opened("DoorState"))],
        vec![],
        vec![],
    );
    let report = diff_packages(&old, &new);
    assert_eq!(report.verdict, Verdict::Identical);
    assert_eq!(report.changes, vec![]);
}

/// `inline_service` with its shape's number marked provisional — an inline
/// service with no lock entry.
fn provisional_inline_service(name: &str, number: u32, interactions: Vec<v2::Decl>) -> v2::Service {
    let mut service = inline_service(name, number, interactions);
    if let Some(v2::service_shape::Kind::Inline(shape)) = &mut service.shapes[0].kind {
        shape.provisional = true;
    }
    service
}

/// An inline shape's number is compared as a declared interface's is: the
/// same number frozen in place is `InterfaceFrozen`, compatible, under the
/// service's path.
#[test]
fn an_inline_shape_frozen_in_place_is_interface_frozen() {
    let old = package(
        vec![],
        vec![provisional_inline_service(
            "veh.cluster.hvac",
            5,
            door_opened("DoorState"),
        )],
        vec![],
    );
    let new = package(
        vec![],
        vec![inline_service(
            "veh.cluster.hvac",
            5,
            door_opened("DoorState"),
        )],
        vec![],
    );
    let report = diff_packages(&old, &new);
    assert_eq!(report.verdict, Verdict::Compatible);
    assert_eq!(
        report.changes,
        vec![change(
            "veh.cluster/veh.cluster.hvac",
            Category::InterfaceFrozen,
            Verdict::Compatible,
            Some("5 (provisional)"),
            Some("5"),
        )]
    );
}

/// An inline shape whose provisional number is frozen as another number moves
/// the routing key: `InterfaceNumberChanged`, breaking.
#[test]
fn an_inline_shape_frozen_as_another_number_is_interface_number_changed() {
    let old = package(
        vec![],
        vec![provisional_inline_service(
            "veh.cluster.hvac",
            6,
            door_opened("DoorState"),
        )],
        vec![],
    );
    let new = package(
        vec![],
        vec![inline_service(
            "veh.cluster.hvac",
            5,
            door_opened("DoorState"),
        )],
        vec![],
    );
    let report = diff_packages(&old, &new);
    assert_eq!(report.verdict, Verdict::Breaking);
    assert_eq!(
        report.changes,
        vec![change(
            "veh.cluster/veh.cluster.hvac",
            Category::InterfaceNumberChanged,
            Verdict::Breaking,
            Some("6 (provisional)"),
            Some("5"),
        )]
    );
}

/// An inline shape is an interface with its own number (lock design §3), so
/// it follows that number across a rename of its service: the shape is
/// `InterfaceRenamed` and its body is diffed, while the service itself — which
/// is identified by its dotted name, D-7 — is a removal plus an addition.
#[test]
fn an_inline_shape_follows_its_number_across_a_service_rename() {
    let old = package(
        vec![],
        vec![inline_service(
            "veh.cluster.hvac",
            5,
            vec![event("cabinTemp", 1, "DoorState")],
        )],
        vec![],
    );
    let new = package(
        vec![],
        vec![inline_service(
            "veh.cluster.climate",
            5,
            vec![
                event("cabinTemp", 1, "DoorState"),
                event("filterClogged", 2, "DoorState"),
            ],
        )],
        vec![],
    );
    let report = diff_packages(&old, &new);
    assert_eq!(
        report.changes,
        vec![
            change(
                "veh.cluster/veh.cluster.climate",
                Category::InterfaceRenamed,
                Verdict::Compatible,
                Some("veh.cluster.hvac"),
                Some("veh.cluster.climate"),
            ),
            change(
                "veh.cluster/veh.cluster.climate/filterClogged",
                Category::InteractionAppended,
                Verdict::Compatible,
                None,
                Some("event filterClogged"),
            ),
            change(
                "veh.cluster/veh.cluster.hvac",
                Category::DeclRemoved,
                Verdict::Breaking,
                Some("service"),
                None,
            ),
            change(
                "veh.cluster/veh.cluster.climate",
                Category::DeclAdded,
                Verdict::Compatible,
                None,
                Some("service"),
            ),
        ]
    );
    assert_eq!(
        report.verdict,
        Verdict::Breaking,
        "a service is identified by its dotted name, so the rename of the service is breaking \
         even though its shape kept its number"
    );
}

/// A renamed interface's member changes are classified as any other
/// interface's: the classifier re-finds the old side by number. The append
/// rule answers breaking when it cannot find the old body, so a compatible
/// verdict here proves the lookup.
#[test]
fn a_renamed_interfaces_member_change_classifies_as_before() {
    let old = package(
        vec![frozen("Doors", 1, door_opened("DoorState"))],
        vec![],
        vec![],
    );
    let new = package(
        vec![frozen(
            "DoorStatus",
            1,
            vec![
                event("doorOpened", 1, "DoorState"),
                event("doorClosed", 2, "DoorState"),
            ],
        )],
        vec![],
        vec![],
    );
    let report = diff_packages(&old, &new);
    assert_eq!(report.verdict, Verdict::Compatible, "{:?}", report.changes);
    assert_eq!(
        report.changes,
        vec![
            change(
                "veh.cluster/DoorStatus",
                Category::InterfaceRenamed,
                Verdict::Compatible,
                Some("Doors"),
                Some("DoorStatus"),
            ),
            change(
                "veh.cluster/DoorStatus/doorClosed",
                Category::InteractionAppended,
                Verdict::Compatible,
                None,
                Some("event doorClosed"),
            ),
        ]
    );
}

/// The JSON report is unchanged by the heading: a headed change carries the
/// same five fields as any other, and the report the same two.
#[test]
fn a_headed_change_renders_json_with_no_heading_field() {
    let old = package(
        vec![frozen("Doors", 1, door_opened("DoorState"))],
        vec![],
        vec![],
    );
    let new = package(
        vec![frozen("DoorStatus", 1, door_opened("DoorState"))],
        vec![],
        vec![],
    );
    let report = diff_packages(&old, &new);
    let value: serde_json::Value =
        serde_json::from_str(&render_json(&report)).expect("render_json emits valid JSON");

    let mut report_keys: Vec<&str> = value
        .as_object()
        .expect("the report is an object")
        .keys()
        .map(String::as_str)
        .collect();
    report_keys.sort_unstable();
    assert_eq!(report_keys, ["changes", "verdict"]);

    let change = &value["changes"][0];
    let mut change_keys: Vec<&str> = change
        .as_object()
        .expect("a change is an object")
        .keys()
        .map(String::as_str)
        .collect();
    change_keys.sort_unstable();
    assert_eq!(
        change_keys,
        ["after", "before", "category", "path", "verdict"]
    );
    assert_eq!(change["category"], "interface_renamed");
}

// --------------------------------------------------------------------------
// Doc fields (ADR-0026): a change to `doc`, `links`, `see` or `since` on any
// carrier is `DocOnly`.
// --------------------------------------------------------------------------

/// `set(level: Level)`, a command with one parameter.
fn command_with_param() -> v2::Decl {
    interaction(
        "set",
        1,
        v2::decl::Kind::CommandDef(v2::CommandDef {
            params: vec![v2::Param {
                name: "level".to_string(),
                r#type: Some(v2::FieldType {
                    optional: false,
                    kind: Some(v2::field_type::Kind::Named("Level".to_string())),
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
    )
}

fn only_doc_only(report: &crate::DiffReport, path: &str) {
    assert_eq!(
        report.changes,
        vec![change(
            path,
            Category::DocOnly,
            Verdict::Compatible,
            None,
            None
        )],
    );
    assert_eq!(report.verdict, Verdict::Compatible);
}

#[test]
fn a_parameter_doc_change_is_doc_only() {
    let old = pkg(
        "veh.cluster",
        interface("Control", vec![command_with_param()]),
    );
    let mut new = old.clone();
    let Some(v2::decl::Kind::CommandDef(def)) = &mut new.interfaces[0].interactions[0].kind else {
        panic!("not a command");
    };
    def.params[0].doc = "the requested level".to_string();
    def.params[0].links.push(v2::DocLink {
        text: "Level".to_string(),
        offset: 0,
        len: 5,
        target: "veh.cluster.Level".to_string(),
    });

    only_doc_only(&diff_packages(&old, &new), "veh.cluster/Control/set");
}

#[test]
fn a_since_change_is_doc_only() {
    let old = pkg(
        "veh.cluster",
        interface("Control", vec![command_with_param()]),
    );

    let mut on_interaction = old.clone();
    on_interaction.interfaces[0].interactions[0]
        .since
        .push("1.2".to_string());
    only_doc_only(
        &diff_packages(&old, &on_interaction),
        "veh.cluster/Control/set",
    );

    let mut on_interface = old.clone();
    on_interface.interfaces[0].since.push("1.2".to_string());
    only_doc_only(&diff_packages(&old, &on_interface), "veh.cluster/Control");
}

#[test]
fn a_see_change_on_a_declaration_or_a_service_is_doc_only() {
    let see = v2::DocLink {
        text: "Other".to_string(),
        target: "veh.cluster.Other".to_string(),
        ..Default::default()
    };
    let mut old = pkg(
        "veh.cluster",
        interface("Control", vec![command_with_param()]),
    );
    old.decls.push(v2::Decl {
        name: "Level".to_string(),
        kind: Some(v2::decl::Kind::TypeDef(v2::TypeDef::default())),
        ..Default::default()
    });
    old.services.push(v2::Service {
        name: "veh.cluster.control".to_string(),
        ..Default::default()
    });

    let mut on_decl = old.clone();
    on_decl.decls[0].see.push(see.clone());
    only_doc_only(&diff_packages(&old, &on_decl), "veh.cluster/Level");

    let mut on_service = old.clone();
    on_service.services[0].see.push(see);
    only_doc_only(
        &diff_packages(&old, &on_service),
        "veh.cluster/veh.cluster.control",
    );
}

/// A member of a struct, enum, enum set or union carries the same doc
/// fields. A change to them is `DocOnly` on the declaration, not a
/// `ConstraintChanged` on its body.
#[test]
fn a_member_doc_change_is_doc_only() {
    let field = v2::Field {
        name: "x".to_string(),
        ordinal: 1,
        ..Default::default()
    };
    let value = v2::EnumValue {
        name: "ON".to_string(),
        value: 1,
        ..Default::default()
    };
    let arm = v2::UnionArm {
        name: "a".to_string(),
        ordinal: 1,
        type_ref: "S".to_string(),
        ..Default::default()
    };
    let decl = |name: &str, kind| v2::Decl {
        name: name.to_string(),
        kind: Some(kind),
        ..Default::default()
    };
    let mut old = pkg("veh.cluster", interface("Control", Vec::new()));
    old.decls = vec![
        decl(
            "S",
            v2::decl::Kind::StructDef(v2::StructDef {
                members: vec![v2::StructMember {
                    member: Some(v2::struct_member::Member::Field(Box::new(field))),
                }],
                fixed_layout: false,
            }),
        ),
        decl(
            "E",
            v2::decl::Kind::EnumDef(v2::EnumDef {
                values: vec![value.clone()],
                ..Default::default()
            }),
        ),
        decl(
            "F",
            v2::decl::Kind::EnumSetDef(v2::EnumSetDef {
                bits: vec![value],
                ..Default::default()
            }),
        ),
        decl(
            "U",
            v2::decl::Kind::UnionDef(v2::UnionDef {
                arms: vec![arm],
                ..Default::default()
            }),
        ),
    ];

    let mut on_field = old.clone();
    if let Some(v2::decl::Kind::StructDef(def)) = &mut on_field.decls[0].kind
        && let Some(v2::struct_member::Member::Field(field)) = &mut def.members[0].member
    {
        field.doc = "documented".to_string();
    }
    only_doc_only(&diff_packages(&old, &on_field), "veh.cluster/S");

    let mut on_value = old.clone();
    if let Some(v2::decl::Kind::EnumDef(def)) = &mut on_value.decls[1].kind {
        def.values[0].since.push("1.2".to_string());
    }
    only_doc_only(&diff_packages(&old, &on_value), "veh.cluster/E");

    let mut on_bit = old.clone();
    if let Some(v2::decl::Kind::EnumSetDef(def)) = &mut on_bit.decls[2].kind {
        def.bits[0].doc = "documented".to_string();
    }
    only_doc_only(&diff_packages(&old, &on_bit), "veh.cluster/F");

    let mut on_arm = old.clone();
    if let Some(v2::decl::Kind::UnionDef(def)) = &mut on_arm.decls[3].kind {
        def.arms[0].see.push(v2::DocLink {
            text: "S".to_string(),
            target: "veh.cluster.S".to_string(),
            ..Default::default()
        });
    }
    only_doc_only(&diff_packages(&old, &on_arm), "veh.cluster/U");
}

// --------------------------------------------------------------------------
// Units — a frozen number is matched within the unit of its package, and a
// number the unit retired sanctions the removal whichever package held it.
// --------------------------------------------------------------------------

/// `package`, placed in `unit` under `name`.
fn in_unit(unit: &str, name: &str, package: v2::Package) -> v2::Package {
    v2::Package {
        name: name.to_string(),
        unit: unit.to_string(),
        ..package
    }
}

/// A frozen number that moved from one package of the unit to a sibling is
/// one interface: `InterfaceRenamed`, compatible, under the new package's
/// path, with the catalog names as its sides. `u.climate` sorts before
/// `u.cluster`, so its pair is walked first: the moved shape is claimed
/// before that walk, and is not also `DeclAdded`. `u.climate` is on both
/// sides on purpose: a package on the new side only is a package-level
/// `DeclAdded`, which is not what this test pins.
#[test]
fn a_frozen_number_moved_to_a_sibling_package_is_a_rename() {
    let speed = || frozen("Speed", 1, door_opened("DoorState"));
    let old = [
        in_unit("u", "u.cluster", package(vec![speed()], vec![], vec![])),
        in_unit("u", "u.climate", package(vec![], vec![], vec![])),
    ];
    let new = [
        in_unit("u", "u.cluster", package(vec![], vec![], vec![])),
        in_unit("u", "u.climate", package(vec![speed()], vec![], vec![])),
    ];
    let report = diff_sets(&old, &new);
    assert_eq!(report.verdict, Verdict::Compatible);
    assert_eq!(
        report.changes,
        vec![change(
            "u.climate/Speed",
            Category::InterfaceRenamed,
            Verdict::Compatible,
            Some("cluster.Speed"),
            Some("climate.Speed"),
        )]
    );
}

/// The body of a moved interface is compared as any other pair's, under the
/// new package's path, and classified against the package that holds it: an
/// appended interaction is compatible.
#[test]
fn a_moved_interface_has_its_body_compared_and_classified() {
    let old = [
        in_unit(
            "u",
            "u.cluster",
            package(
                vec![frozen("Speed", 1, door_opened("DoorState"))],
                vec![],
                vec![],
            ),
        ),
        in_unit("u", "u.climate", package(vec![], vec![], vec![])),
    ];
    let new = [
        in_unit("u", "u.cluster", package(vec![], vec![], vec![])),
        in_unit(
            "u",
            "u.climate",
            package(
                vec![frozen(
                    "Speed",
                    1,
                    vec![
                        event("doorOpened", 1, "DoorState"),
                        event("doorClosed", 2, "DoorState"),
                    ],
                )],
                vec![],
                vec![],
            ),
        ),
    ];
    let report = diff_sets(&old, &new);
    assert_eq!(report.verdict, Verdict::Compatible);
    assert_eq!(
        report
            .changes
            .iter()
            .map(|change| (change.path.as_str(), change.category, change.verdict))
            .collect::<Vec<_>>(),
        vec![
            (
                "u.climate/Speed",
                Category::InterfaceRenamed,
                Verdict::Compatible
            ),
            (
                "u.climate/Speed/doorClosed",
                Category::InteractionAppended,
                Verdict::Compatible
            ),
        ]
    );
}

/// A number the unit retired sanctions the removal in whichever package held
/// it: the retired entry is carried by the unit's anchor package under its
/// lock key, `cluster.Old` on `u`.
#[test]
fn a_retired_number_carried_by_the_anchor_sanctions_a_removal_in_a_sibling() {
    let old = [
        in_unit("u", "u", package(vec![], vec![], vec![])),
        in_unit(
            "u",
            "u.cluster",
            package(
                vec![frozen("Old", 3, door_opened("DoorState"))],
                vec![],
                vec![],
            ),
        ),
    ];
    let new = [
        in_unit(
            "u",
            "u",
            package(vec![], vec![], vec![retired("cluster.Old", 3)]),
        ),
        in_unit("u", "u.cluster", package(vec![], vec![], vec![])),
    ];
    let report = diff_sets(&old, &new);
    assert_eq!(report.verdict, Verdict::Compatible);
    assert_eq!(
        report.changes,
        vec![change(
            "u.cluster/Old",
            Category::InterfaceRetired,
            Verdict::Compatible,
            Some("interface"),
            Some("retired"),
        )]
    );
}

/// A package gone from the new side while its unit is still there is read
/// shape by shape, not as one package-level `DeclRemoved`: a number the unit
/// retired is `InterfaceRetired`.
#[test]
fn a_retired_number_sanctions_the_removal_of_a_package_of_a_living_unit() {
    let old = [
        in_unit("u", "u", package(vec![], vec![], vec![])),
        in_unit(
            "u",
            "u.cluster",
            package(
                vec![frozen("Old", 3, door_opened("DoorState"))],
                vec![],
                vec![],
            ),
        ),
    ];
    let new = [in_unit(
        "u",
        "u",
        package(vec![], vec![], vec![retired("cluster.Old", 3)]),
    )];
    let report = diff_sets(&old, &new);
    assert_eq!(report.verdict, Verdict::Compatible);
    assert_eq!(
        report.changes,
        vec![change(
            "u.cluster/Old",
            Category::InterfaceRetired,
            Verdict::Compatible,
            Some("interface"),
            Some("retired"),
        )]
    );
}

/// In a package gone from a living unit, a shape whose number moved to a
/// sibling is `InterfaceRenamed`; a shape that is neither moved nor retired,
/// and every other declaration of the package, is `DeclRemoved` on its own
/// line. A package whose whole unit is gone stays one package-level
/// `DeclRemoved` (`a_new_package_is_compatible_and_a_dropped_one_is_breaking`).
#[test]
fn a_package_gone_from_a_living_unit_is_read_declaration_by_declaration() {
    let speed = || frozen("Speed", 1, door_opened("DoorState"));
    let mut cluster = package(
        vec![speed(), frozen("Gone", 2, door_opened("DoorState"))],
        vec![],
        vec![],
    );
    cluster.decls.push(v2::Decl {
        name: "Level".to_string(),
        ..Default::default()
    });
    let old = [
        in_unit("u", "u", package(vec![], vec![], vec![])),
        in_unit("u", "u.cluster", cluster),
    ];
    let new = [in_unit("u", "u", package(vec![speed()], vec![], vec![]))];
    let report = diff_sets(&old, &new);
    assert_eq!(report.verdict, Verdict::Breaking);
    assert_eq!(
        report.changes,
        vec![
            change(
                "u.cluster/Level",
                Category::DeclRemoved,
                Verdict::Breaking,
                Some("declaration"),
                None,
            ),
            change(
                "u.cluster/Gone",
                Category::DeclRemoved,
                Verdict::Breaking,
                Some("interface"),
                None,
            ),
            change(
                "u/Speed",
                Category::InterfaceRenamed,
                Verdict::Compatible,
                Some("cluster.Speed"),
                Some("Speed"),
            ),
        ]
    );
}

/// A snapshot written before the IR carried `unit` has an empty one, and is
/// its own unit: for the root package, that unit is the one the new side
/// records, so the two match number for number.
#[test]
fn a_snapshot_without_a_unit_is_its_own_unit() {
    let foo = || frozen("Foo", 1, door_opened("DoorState"));
    let old = [in_unit("", "p", package(vec![foo()], vec![], vec![]))];
    let new = [in_unit("p", "p", package(vec![foo()], vec![], vec![]))];
    let report = diff_sets(&old, &new);
    assert_eq!(report.verdict, Verdict::Identical);
    assert_eq!(report.changes, vec![]);
}

/// For a package below the root, the unit an old snapshot without `unit`
/// stands for is the package itself, and the new side's unit is another one:
/// no number is matched across them, which is why the migration publishes a
/// new baseline.
#[test]
fn a_sub_package_snapshot_without_a_unit_is_outside_the_new_unit() {
    let speed = || frozen("Speed", 1, door_opened("DoorState"));
    let old = [in_unit(
        "",
        "u.cluster",
        package(vec![speed()], vec![], vec![]),
    )];
    let new = [in_unit(
        "u",
        "u.cluster",
        package(vec![speed()], vec![], vec![]),
    )];
    let report = diff_sets(&old, &new);
    assert_eq!(report.verdict, Verdict::Breaking);
    assert_eq!(
        report
            .changes
            .iter()
            .map(|change| (change.path.as_str(), change.category))
            .collect::<Vec<_>>(),
        vec![
            ("u.cluster/Speed", Category::DeclRemoved),
            ("u.cluster/Speed", Category::DeclAdded),
        ]
    );
}

// --------------------------------------------------------------------------
// Enum and enum-set slots — the slot is the explicit number, not the position.
// --------------------------------------------------------------------------

/// A package holding one declaration `name` of `kind`.
fn with_decl(name: &str, kind: v2::decl::Kind) -> v2::Package {
    let mut package = package(vec![], vec![], vec![]);
    package.decls = vec![v2::Decl {
        name: name.to_string(),
        kind: Some(kind),
        ..Default::default()
    }];
    package
}

/// Enum values or enum-set bits, each with its explicit number.
fn numbered(members: &[(&str, i64)]) -> Vec<v2::EnumValue> {
    members
        .iter()
        .map(|(name, value)| v2::EnumValue {
            name: name.to_string(),
            value: *value,
            ..Default::default()
        })
        .collect()
}

fn enum_def(members: &[(&str, i64)]) -> v2::decl::Kind {
    v2::decl::Kind::EnumDef(v2::EnumDef {
        values: numbered(members),
        ..Default::default()
    })
}

fn enum_set_def(members: &[(&str, i64)]) -> v2::decl::Kind {
    v2::decl::Kind::EnumSetDef(v2::EnumSetDef {
        bits: numbered(members),
        ..Default::default()
    })
}

/// A textual reorder of an enum body moves no explicit number, so nothing
/// moves on the wire. The catalog hash covers the order of the values when an
/// interface reaches the enum: the change is `EnumReordered` on the container,
/// compatible, with no detail.
#[test]
fn an_enum_pure_reorder_is_enum_reordered() {
    let old = with_decl("Mode", enum_def(&[("OFF", 0), ("ON", 1)]));
    let new = with_decl("Mode", enum_def(&[("ON", 1), ("OFF", 0)]));
    let report = diff_packages(&old, &new);
    assert_eq!(report.verdict, Verdict::Compatible);
    assert_eq!(
        report.changes,
        vec![Change {
            path: "veh.cluster/Mode".to_string(),
            category: Category::EnumReordered,
            verdict: Verdict::Compatible,
            before: None,
            after: None,
        }]
    );
    assert_eq!(
        crate::heading(Category::EnumReordered),
        Some("compatible on the wire, may change the catalog hash"),
    );
}

/// A textual reorder of an enum-set body moves no bit, so it is
/// `EnumReordered` on the container, compatible.
#[test]
fn an_enum_set_pure_reorder_is_enum_reordered() {
    let old = with_decl("Flags", enum_set_def(&[("A", 0), ("B", 1)]));
    let new = with_decl("Flags", enum_set_def(&[("B", 1), ("A", 0)]));
    let report = diff_packages(&old, &new);
    assert_eq!(report.verdict, Verdict::Compatible);
    assert_eq!(
        report.changes,
        vec![Change {
            path: "veh.cluster/Flags".to_string(),
            category: Category::EnumReordered,
            verdict: Verdict::Compatible,
            before: None,
            after: None,
        }]
    );
}

/// Enum values renumbered in place — the text order is kept — are each
/// `MemberReordered` with the old and new explicit number as a `value`
/// detail. Every difference between the two bodies is a changed number, so the
/// container's `ConstraintChanged` is not reported beside them.
#[test]
fn an_enum_renumber_is_member_reordered_with_the_value() {
    let old = with_decl("Mode", enum_def(&[("OFF", 0), ("ON", 1)]));
    let new = with_decl("Mode", enum_def(&[("OFF", 1), ("ON", 0)]));
    let report = diff_packages(&old, &new);
    let summary: Vec<_> = report
        .changes
        .iter()
        .map(|change| {
            (
                change.path.as_str(),
                change.category,
                change.before.as_deref(),
                change.after.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        summary,
        vec![
            (
                "veh.cluster/Mode/OFF",
                Category::MemberReordered,
                Some("value 0"),
                Some("value 1"),
            ),
            (
                "veh.cluster/Mode/ON",
                Category::MemberReordered,
                Some("value 1"),
                Some("value 0"),
            ),
        ]
    );
}

/// An enum body with the given values and the given retired numbers.
fn enum_with_reserved(members: &[(&str, i64)], retired: &[i64]) -> v2::decl::Kind {
    v2::decl::Kind::EnumDef(v2::EnumDef {
        values: numbered(members),
        reserved: retired
            .iter()
            .map(|value| v2::Reserved {
                value: Some(*value),
                ..Default::default()
            })
            .collect(),
    })
}

/// A reserved list reordered with nothing else changed is `EnumReordered`,
/// like a reorder of the values.
#[test]
fn an_enum_reserved_list_reorder_is_enum_reordered() {
    let old = with_decl("Mode", enum_with_reserved(&[("OFF", 0)], &[3, 4]));
    let new = with_decl("Mode", enum_with_reserved(&[("OFF", 0)], &[4, 3]));
    let report = diff_packages(&old, &new);
    assert_eq!(report.verdict, Verdict::Compatible);
    let summary: Vec<_> = report
        .changes
        .iter()
        .map(|change| (change.path.as_str(), change.category))
        .collect();
    assert_eq!(summary, vec![("veh.cluster/Mode", Category::EnumReordered)]);
}

/// Two values that swap their numbers, in an edit that also retires another
/// number, report the changed numbers and the container's `ConstraintChanged`:
/// the reserved list is a difference that is not a changed number, so it must
/// not hide behind the `MemberReordered` lines.
#[test]
fn an_enum_renumber_beside_a_reserved_list_change_keeps_constraint_changed() {
    let old = with_decl("Mode", enum_with_reserved(&[("OFF", 0), ("ON", 1)], &[]));
    let new = with_decl("Mode", enum_with_reserved(&[("OFF", 1), ("ON", 0)], &[5]));
    let report = diff_packages(&old, &new);
    assert_eq!(report.verdict, Verdict::Breaking);
    let summary: Vec<_> = report
        .changes
        .iter()
        .map(|change| (change.path.as_str(), change.category))
        .collect();
    assert_eq!(
        summary,
        vec![
            ("veh.cluster/Mode/OFF", Category::MemberReordered),
            ("veh.cluster/Mode/ON", Category::MemberReordered),
            ("veh.cluster/Mode", Category::ConstraintChanged),
        ]
    );
}

/// Two values that swap their numbers, in an edit that also reorders the
/// reserved list with no entry changed, report the changed numbers alone: a
/// reordered reserved list is order only, so it adds no `ConstraintChanged`
/// on the container.
#[test]
fn an_enum_renumber_beside_a_reserved_list_reorder_reports_only_the_values() {
    let old = with_decl(
        "Mode",
        enum_with_reserved(&[("OFF", 0), ("ON", 1)], &[3, 4]),
    );
    let new = with_decl(
        "Mode",
        enum_with_reserved(&[("OFF", 1), ("ON", 0)], &[4, 3]),
    );
    let report = diff_packages(&old, &new);
    assert_eq!(report.verdict, Verdict::Breaking);
    let summary: Vec<_> = report
        .changes
        .iter()
        .map(|change| (change.path.as_str(), change.category))
        .collect();
    assert_eq!(
        summary,
        vec![
            ("veh.cluster/Mode/OFF", Category::MemberReordered),
            ("veh.cluster/Mode/ON", Category::MemberReordered),
        ]
    );
}

/// An enum-set bit renumbered beside a change to the set's backing enum
/// reports the container's `ConstraintChanged` as well.
#[test]
fn an_enum_set_renumber_beside_another_change_keeps_constraint_changed() {
    let old = with_decl("Flags", enum_set_def(&[("A", 0), ("B", 1)]));
    let mut new = with_decl("Flags", enum_set_def(&[("A", 2), ("B", 1)]));
    if let Some(v2::decl::Kind::EnumSetDef(def)) = &mut new.decls[0].kind {
        def.backing_enum = Some("Mode".to_string());
    }
    let report = diff_packages(&old, &new);
    let summary: Vec<_> = report
        .changes
        .iter()
        .map(|change| (change.path.as_str(), change.category))
        .collect();
    assert_eq!(
        summary,
        vec![
            ("veh.cluster/Flags/A", Category::MemberReordered),
            ("veh.cluster/Flags", Category::ConstraintChanged),
        ]
    );
}

/// An enum-set bit renumbered with nothing else changed reports the bit alone,
/// with no `ConstraintChanged` on the container.
#[test]
fn an_enum_set_renumber_alone_reports_only_the_bit() {
    let old = with_decl("Flags", enum_set_def(&[("A", 0), ("B", 1)]));
    let new = with_decl("Flags", enum_set_def(&[("B", 1), ("A", 2)]));
    let report = diff_packages(&old, &new);
    let summary: Vec<_> = report
        .changes
        .iter()
        .map(|change| {
            (
                change.path.as_str(),
                change.category,
                change.before.as_deref(),
                change.after.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        summary,
        vec![(
            "veh.cluster/Flags/A",
            Category::MemberReordered,
            Some("bit 0"),
            Some("bit 2"),
        )]
    );
}

/// The snapshot loader stays reachable under its earlier path, re-exported
/// from `ridl-ir` (ADR-0008 decision 9): this fails to compile without the
/// re-export.
#[test]
fn the_snapshot_loader_is_re_exported() {
    let load: fn(&std::path::Path) -> Result<v2::Package, crate::LoadError> = crate::load_ir_json;
    let _ = load;
}

// ==========================================================================
// The podbuf break rules: an append to a struct nested inline in a head.
// ==========================================================================

mod podbuf {
    use ridl_ir::v2;

    use crate::{Category, Verdict, diff_refs_with, render_text};

    fn typl_decl(name: &str, kind: v2::decl::Kind) -> v2::Decl {
        v2::Decl {
            limit: None,
            name: name.to_string(),
            visibility: v2::Visibility::Public as i32,
            is_error: false,
            doc: String::new(),
            labels: Vec::new(),
            deprecated: None,
            ordinal: 0,
            kind: Some(kind),
            links: Vec::new(),
            see: Vec::new(),
            since: Vec::new(),
        }
    }

    /// An `integer [min..max]` at `u8` width, so that it has a podbuf form.
    fn int_type(name: &str, min: &str, max: &str) -> v2::Decl {
        typl_decl(
            name,
            v2::decl::Kind::TypeDef(v2::TypeDef {
                backing: Some(v2::Backing {
                    kind: Some(v2::backing::Kind::Primitive(
                        v2::PrimitiveType::Integer as i32,
                    )),
                }),
                constraint: Some(v2::Constraint {
                    min: Some(min.to_string()),
                    max: Some(max.to_string()),
                    ..Default::default()
                }),
                declared_init: None,
                init: None,
                width: Some(v2::type_def::Width::IntWidth(v2::IntWidth::U8 as i32)),
            }),
        )
    }

    /// A `string [0..16]`, the key type of a map.
    fn string_type(name: &str) -> v2::Decl {
        typl_decl(
            name,
            v2::decl::Kind::TypeDef(v2::TypeDef {
                backing: Some(v2::Backing {
                    kind: Some(v2::backing::Kind::Primitive(
                        v2::PrimitiveType::String as i32,
                    )),
                }),
                constraint: Some(v2::Constraint {
                    len_min: Some(0),
                    len_max: Some(16),
                    ..Default::default()
                }),
                declared_init: None,
                init: None,
                width: None,
            }),
        )
    }

    fn named(reference: &str) -> v2::FieldType {
        v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::Named(reference.to_string())),
        }
    }

    fn array(element: v2::FieldType, min: u64, max: u64) -> v2::FieldType {
        v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::Array(Box::new(v2::ArrayType {
                element: Some(Box::new(element)),
                min,
                max,
            }))),
        }
    }

    fn map(key: v2::FieldType, value: v2::FieldType) -> v2::FieldType {
        v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::Map(Box::new(v2::MapType {
                key: Some(Box::new(key)),
                value: Some(Box::new(value)),
                min: 0,
                max: 8,
            }))),
        }
    }

    fn tuple(fields: Vec<(&str, v2::FieldType)>) -> v2::FieldType {
        v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::Tuple(v2::TupleType {
                fields: fields
                    .into_iter()
                    .map(|(name, ty)| v2::TupleField {
                        name: name.to_string(),
                        r#type: Some(ty),
                    })
                    .collect(),
            })),
        }
    }

    fn field(name: &str, ordinal: u32, ty: v2::FieldType) -> v2::StructMember {
        v2::StructMember {
            member: Some(v2::struct_member::Member::Field(Box::new(v2::Field {
                name: name.to_string(),
                ordinal,
                r#type: Some(ty),
                declared_init: None,
                init: None,
                doc: String::new(),
                labels: Vec::new(),
                deprecated: None,
                links: Vec::new(),
                see: Vec::new(),
                since: Vec::new(),
            }))),
        }
    }

    fn reserved_field(ordinal: u32) -> v2::StructMember {
        v2::StructMember {
            member: Some(v2::struct_member::Member::Reserved(v2::Reserved {
                ordinal,
                name: None,
                value: None,
            })),
        }
    }

    fn struct_decl(name: &str, members: Vec<v2::StructMember>) -> v2::Decl {
        typl_decl(
            name,
            v2::decl::Kind::StructDef(v2::StructDef {
                members,
                fixed_layout: false,
            }),
        )
    }

    fn package(name: &str, decls: Vec<v2::Decl>) -> v2::Package {
        v2::Package {
            name: name.to_string(),
            decls,
            interfaces: Vec::new(),
            services: Vec::new(),
            retired: Vec::new(),
            unit: String::new(),
        }
    }

    /// `SensorReading { value: Level }`, the struct every fixture appends to.
    fn reading_v1() -> v2::Decl {
        struct_decl("SensorReading", vec![field("value", 1, named("Level"))])
    }

    /// `SensorReading { value: Level, quality: Level }`: an append whose type
    /// holds 0, so the generic rule says compatible.
    fn reading_v2() -> v2::Decl {
        struct_decl(
            "SensorReading",
            vec![
                field("value", 1, named("Level")),
                field("quality", 2, named("Level")),
            ],
        )
    }

    /// `veh.common` with `SensorReading` as given and the inline uses: a
    /// struct field, a tuple member and an element of an exact-length array.
    fn inline_package(reading: v2::Decl) -> v2::Package {
        package(
            "veh.common",
            vec![
                int_type("Level", "0", "100"),
                reading,
                struct_decl(
                    "SensorBounds",
                    vec![
                        field("range", 1, named("SensorReading")),
                        field("limit", 2, named("Level")),
                        field(
                            "pair",
                            3,
                            tuple(vec![
                                ("low", named("Level")),
                                ("high", named("SensorReading")),
                            ]),
                        ),
                        field("window", 4, array(named("SensorReading"), 2, 2)),
                    ],
                ),
            ],
        )
    }

    /// `veh.common` with `SensorReading` as given and only tail uses: a
    /// bounded array element and a map value, each a tail block with a stride.
    fn tail_package(reading: v2::Decl) -> v2::Package {
        package(
            "veh.common",
            vec![
                int_type("Level", "0", "100"),
                string_type("Label"),
                reading,
                struct_decl(
                    "Log",
                    vec![
                        field("entries", 1, array(named("SensorReading"), 0, 8)),
                        field("by_label", 2, map(named("Label"), named("SensorReading"))),
                    ],
                ),
            ],
        )
    }

    fn diff(old: &v2::Package, new: &v2::Package, encodings: &[&str]) -> crate::DiffReport {
        diff_refs_with(&[old], &[new], &[], encodings)
    }

    #[test]
    fn appending_to_a_struct_nested_inline_is_a_podbuf_break_naming_the_uses() {
        let report = diff(
            &inline_package(reading_v1()),
            &inline_package(reading_v2()),
            &["podbuf"],
        );
        let change = report
            .changes
            .iter()
            .find(|change| change.category == Category::PodbufInlineAppend)
            .expect("the append is reported as a podbuf inline append");
        assert_eq!(change.verdict, Verdict::Breaking);
        assert_eq!(change.path, "veh.common/SensorReading/quality");
        assert_eq!(change.before, None);
        assert_eq!(
            change.after.as_deref(),
            Some(
                "nested inline in veh.common/SensorBounds/pair, \
                 veh.common/SensorBounds/range, veh.common/SensorBounds/window"
            )
        );
        assert_eq!(report.verdict, Verdict::Breaking);
    }

    #[test]
    fn the_same_append_is_compatible_without_podbuf_and_listed_under_its_heading() {
        let report = diff(
            &inline_package(reading_v1()),
            &inline_package(reading_v2()),
            &[],
        );
        assert_eq!(report.verdict, Verdict::Compatible);
        let change = report
            .changes
            .iter()
            .find(|change| change.category == Category::PodbufInlineAppend)
            .expect("the append is still reported, as compatible");
        assert_eq!(change.verdict, Verdict::Compatible);
        let text = render_text(&report);
        assert!(
            text.contains(
                "breaking under podbuf:\n  [compatible] podbuf_inline_append \
                 veh.common/SensorReading/quality: (absent) -> nested inline in"
            ),
            "{text}"
        );
    }

    #[test]
    fn an_encoding_without_podbuf_rules_leaves_the_verdict_compatible() {
        let report = diff(
            &inline_package(reading_v1()),
            &inline_package(reading_v2()),
            &["flatbuffers", "proto3"],
        );
        assert_eq!(report.verdict, Verdict::Compatible);
    }

    /// An inline use in another package of the new side is found and named:
    /// the search covers the whole workspace, not the package of the struct.
    #[test]
    fn an_inline_use_in_another_package_is_named() {
        let old = [
            package(
                "veh.common",
                vec![int_type("Level", "0", "100"), reading_v1()],
            ),
            package(
                "veh.other",
                vec![struct_decl(
                    "Frame",
                    vec![field("reading", 1, named("veh.common.SensorReading"))],
                )],
            ),
        ];
        let new = [
            package(
                "veh.common",
                vec![int_type("Level", "0", "100"), reading_v2()],
            ),
            old[1].clone(),
        ];
        let old: Vec<&v2::Package> = old.iter().collect();
        let new: Vec<&v2::Package> = new.iter().collect();
        let report = diff_refs_with(&old, &new, &[], &["podbuf"]);
        let change = report
            .changes
            .iter()
            .find(|change| change.category == Category::PodbufInlineAppend)
            .expect("the cross-package use is found");
        assert_eq!(change.verdict, Verdict::Breaking);
        assert_eq!(
            change.after.as_deref(),
            Some("nested inline in veh.other/Frame/reading")
        );
    }

    /// A struct used only behind a descriptor, as the element of a bounded
    /// array or the value of a map entry, can grow: the descriptor's stride
    /// gives the producer's size, and the fields past it read as 0.
    #[test]
    fn an_appended_field_to_an_array_element_type_is_compatible_under_podbuf() {
        let report = diff(
            &tail_package(reading_v1()),
            &tail_package(reading_v2()),
            &["podbuf"],
        );
        assert_eq!(report.verdict, Verdict::Compatible, "{:?}", report.changes);
        assert!(
            report
                .changes
                .iter()
                .all(|change| change.category != Category::PodbufInlineAppend),
            "{:?}",
            report.changes
        );
    }

    /// An appended non-optional field whose type excludes 0 is breaking under
    /// the generic rule (driftsys/ridl#598); podbuf adds no second line to it.
    #[test]
    fn an_appended_field_whose_type_excludes_zero_is_breaking_under_every_encoding() {
        let with_nonzero = |reading: v2::Decl| {
            let mut package = inline_package(reading);
            package.decls.push(int_type("Positive", "1", "100"));
            package
        };
        let new = struct_decl(
            "SensorReading",
            vec![
                field("value", 1, named("Level")),
                field("nonzero", 2, named("Positive")),
            ],
        );
        for encodings in [&[][..], &["podbuf"][..]] {
            let report = diff(
                &with_nonzero(reading_v1()),
                &with_nonzero(new.clone()),
                encodings,
            );
            assert_eq!(report.verdict, Verdict::Breaking, "{encodings:?}");
            let appended = report
                .changes
                .iter()
                .find(|change| change.path == "veh.common/SensorReading/nonzero")
                .expect("the append is reported");
            assert_eq!(appended.category, Category::DeclAdded);
            assert_eq!(appended.verdict, Verdict::Breaking);
            assert!(
                report
                    .changes
                    .iter()
                    .all(|change| change.category != Category::PodbufInlineAppend),
                "{encodings:?}: {:?}",
                report.changes
            );
        }
    }

    /// A field removed from a struct, its slot held by a tombstone, is
    /// `decl_removed`, breaking, whatever the encodings: the type is gone from
    /// the source, so its bytes cannot be kept. podbuf adds no second line.
    #[test]
    fn a_retired_field_is_breaking_under_every_encoding() {
        let retired = struct_decl(
            "SensorReading",
            vec![field("value", 1, named("Level")), reserved_field(2)],
        );
        for encodings in [&[][..], &["podbuf"][..]] {
            let report = diff(
                &inline_package(reading_v2()),
                &inline_package(retired.clone()),
                encodings,
            );
            assert_eq!(report.verdict, Verdict::Breaking, "{encodings:?}");
            let removed = report
                .changes
                .iter()
                .find(|change| change.path == "veh.common/SensorReading/quality")
                .expect("the removal is reported");
            assert_eq!(removed.category, Category::DeclRemoved);
            assert_eq!(removed.verdict, Verdict::Breaking);
            assert_eq!(
                report
                    .changes
                    .iter()
                    .filter(|change| change.path == "veh.common/SensorReading/quality")
                    .count(),
                1,
                "{encodings:?}: {:?}",
                report.changes
            );
        }
    }

    /// A struct inside a tuple that is the element of a bounded array sits at
    /// a fixed offset of the tuple's head: the element may grow, the struct
    /// inside it may not.
    #[test]
    fn a_struct_inside_a_tuple_element_of_a_bounded_array_is_an_inline_use() {
        let with_log = |reading: v2::Decl| {
            package(
                "veh.common",
                vec![
                    int_type("Level", "0", "100"),
                    reading,
                    struct_decl(
                        "Log",
                        vec![field(
                            "entries",
                            1,
                            array(
                                tuple(vec![
                                    ("at", named("Level")),
                                    ("reading", named("SensorReading")),
                                ]),
                                0,
                                8,
                            ),
                        )],
                    ),
                ],
            )
        };
        let report = diff(
            &with_log(reading_v1()),
            &with_log(reading_v2()),
            &["podbuf"],
        );
        assert_eq!(report.verdict, Verdict::Breaking, "{:?}", report.changes);
        let change = report
            .changes
            .iter()
            .find(|change| change.category == Category::PodbufInlineAppend)
            .expect("the use inside the tuple is found");
        assert_eq!(
            change.after.as_deref(),
            Some("nested inline in veh.common/Log/entries")
        );
    }

    /// A struct reached only through another declaration held inline is
    /// reported once, at the field that names it; the outer declaration's
    /// own fields are not entered.
    #[test]
    fn a_use_through_another_inline_declaration_is_named_once() {
        let with_frame = |reading: v2::Decl| {
            let mut package = inline_package(reading);
            package.decls.push(struct_decl(
                "Frame",
                vec![field("bounds", 1, named("SensorBounds"))],
            ));
            package
        };
        let report = diff(
            &with_frame(reading_v1()),
            &with_frame(reading_v2()),
            &["podbuf"],
        );
        let change = report
            .changes
            .iter()
            .find(|change| change.category == Category::PodbufInlineAppend)
            .expect("the append is reported");
        assert_eq!(
            change.after.as_deref(),
            Some(
                "nested inline in veh.common/SensorBounds/pair, \
                 veh.common/SensorBounds/range, veh.common/SensorBounds/window"
            )
        );
    }

    /// An arm appended to a union that a struct holds inline is not a podbuf
    /// break: the union's head is its tag and an arm descriptor, and the arms
    /// are tail blocks, so the head does not grow.
    #[test]
    fn an_arm_appended_to_an_inline_union_is_not_a_podbuf_break() {
        let arm = |name: &str, ordinal: u32| v2::UnionArm {
            name: name.to_string(),
            ordinal,
            type_ref: "Level".to_string(),
            doc: String::new(),
            links: Vec::new(),
            see: Vec::new(),
            since: Vec::new(),
        };
        let with_union = |arms: Vec<v2::UnionArm>| {
            package(
                "veh.common",
                vec![
                    int_type("Level", "0", "100"),
                    typl_decl(
                        "Choice",
                        v2::decl::Kind::UnionDef(v2::UnionDef {
                            arms,
                            reserved: Vec::new(),
                            is_result: false,
                        }),
                    ),
                    struct_decl("Holder", vec![field("choice", 1, named("Choice"))]),
                ],
            )
        };
        let report = diff(
            &with_union(vec![arm("low", 1)]),
            &with_union(vec![arm("low", 1), arm("high", 2)]),
            &["podbuf"],
        );
        assert_eq!(report.verdict, Verdict::Compatible, "{:?}", report.changes);
        assert!(
            report
                .changes
                .iter()
                .all(|change| change.category != Category::PodbufInlineAppend),
            "{:?}",
            report.changes
        );
    }

    /// A struct nested in the value of a map entry — inside a tuple there —
    /// sits at a fixed offset of the entry's head.
    #[test]
    fn a_struct_inside_a_tuple_map_value_is_an_inline_use() {
        let with_index = |reading: v2::Decl| {
            package(
                "veh.common",
                vec![
                    int_type("Level", "0", "100"),
                    string_type("Label"),
                    reading,
                    struct_decl(
                        "Index",
                        vec![field(
                            "by_label",
                            1,
                            map(
                                named("Label"),
                                tuple(vec![
                                    ("at", named("Level")),
                                    ("reading", named("SensorReading")),
                                ]),
                            ),
                        )],
                    ),
                ],
            )
        };
        let report = diff(
            &with_index(reading_v1()),
            &with_index(reading_v2()),
            &["podbuf"],
        );
        let change = report
            .changes
            .iter()
            .find(|change| change.category == Category::PodbufInlineAppend)
            .expect("the use inside the map value is found");
        assert_eq!(change.verdict, Verdict::Breaking);
        assert_eq!(
            change.after.as_deref(),
            Some("nested inline in veh.common/Index/by_label")
        );
    }

    /// An exact-length array of the struct as the element of a bounded array:
    /// the element may grow, the struct inside the inline array may not.
    #[test]
    fn a_struct_in_an_exact_array_element_of_a_bounded_array_is_an_inline_use() {
        let with_log = |reading: v2::Decl| {
            package(
                "veh.common",
                vec![
                    int_type("Level", "0", "100"),
                    reading,
                    struct_decl(
                        "Log",
                        vec![field(
                            "windows",
                            1,
                            array(array(named("SensorReading"), 2, 2), 0, 8),
                        )],
                    ),
                ],
            )
        };
        let report = diff(
            &with_log(reading_v1()),
            &with_log(reading_v2()),
            &["podbuf"],
        );
        let change = report
            .changes
            .iter()
            .find(|change| change.category == Category::PodbufInlineAppend)
            .expect("the use inside the inline array is found");
        assert_eq!(change.verdict, Verdict::Breaking);
        assert_eq!(
            change.after.as_deref(),
            Some("nested inline in veh.common/Log/windows")
        );
    }

    // ----------------------------------------------------------------------
    // Interaction-level heads: the induced argument struct, a reply, a fixed
    // payload.
    // ----------------------------------------------------------------------

    fn param(name: &str, ty: v2::FieldType) -> v2::Param {
        v2::Param {
            name: name.to_string(),
            r#type: Some(ty),
            doc: String::new(),
            links: Vec::new(),
            see: Vec::new(),
            since: Vec::new(),
        }
    }

    fn command(name: &str, ordinal: u32, params: Vec<v2::Param>) -> v2::Decl {
        super::interaction(
            name,
            ordinal,
            v2::decl::Kind::CommandDef(v2::CommandDef {
                params,
                contracts: Vec::new(),
                timing: None,
                family: v2::Family::Dispatch as i32,
                shape: v2::Shape::None as i32,
            }),
        )
    }

    fn query(name: &str, ordinal: u32, params: Vec<v2::Param>, reply: v2::FieldType) -> v2::Decl {
        super::interaction(
            name,
            ordinal,
            v2::decl::Kind::QueryDef(v2::QueryDef {
                params,
                return_type: Some(v2::ReturnType {
                    kind: Some(v2::return_type::Kind::Value(reply)),
                }),
                contracts: Vec::new(),
                timing: None,
                family: v2::Family::Dispatch as i32,
            }),
        )
    }

    fn fixed(name: &str, ordinal: u32, payload: v2::FieldType) -> v2::Decl {
        super::interaction(
            name,
            ordinal,
            v2::decl::Kind::FixedDef(v2::FixedDef {
                payload: Some(payload),
            }),
        )
    }

    /// `veh.common` with `SensorReading` as given, no struct use, and the
    /// interface `Ctl` holding `interactions`.
    fn interface_package(reading: v2::Decl, interactions: Vec<v2::Decl>) -> v2::Package {
        let mut package = package("veh.common", vec![int_type("Level", "0", "100"), reading]);
        package.interfaces = vec![super::interface("Ctl", interactions)];
        package
    }

    #[track_caller]
    fn assert_interaction_use(interactions: impl Fn() -> Vec<v2::Decl>, expected: &str) {
        let report = diff(
            &interface_package(reading_v1(), interactions()),
            &interface_package(reading_v2(), interactions()),
            &["podbuf"],
        );
        let change = report
            .changes
            .iter()
            .find(|change| change.category == Category::PodbufInlineAppend)
            .unwrap_or_else(|| panic!("the use is found: {:?}", report.changes));
        assert_eq!(change.verdict, Verdict::Breaking);
        assert_eq!(change.after.as_deref(), Some(expected));
    }

    #[track_caller]
    fn assert_no_interaction_use(interactions: impl Fn() -> Vec<v2::Decl>) {
        let report = diff(
            &interface_package(reading_v1(), interactions()),
            &interface_package(reading_v2(), interactions()),
            &["podbuf"],
        );
        assert_eq!(report.verdict, Verdict::Compatible, "{:?}", report.changes);
    }

    /// A call with several parameters sends the argument struct the lowering
    /// induces: each parameter is a member of that head, at a fixed offset.
    #[test]
    fn a_two_parameter_call_holds_the_struct_in_its_argument_struct() {
        assert_interaction_use(
            || {
                vec![command(
                    "set",
                    1,
                    vec![
                        param("a", named("SensorReading")),
                        param("b", named("Level")),
                    ],
                )]
            },
            "nested inline in veh.common/Ctl/set",
        );
    }

    /// A tuple parameter is a tuple head inside the argument struct.
    #[test]
    fn a_tuple_parameter_holds_the_struct_inline() {
        assert_interaction_use(
            || {
                vec![command(
                    "set",
                    1,
                    vec![param(
                        "p",
                        tuple(vec![
                            ("reading", named("SensorReading")),
                            ("at", named("Level")),
                        ]),
                    )],
                )]
            },
            "nested inline in veh.common/Ctl/set",
        );
    }

    /// A fixed payload that is an exact-length array of the struct has no
    /// head: the encoder lays out a head for a declaration and for a named
    /// induced tuple only, so the struct inside such a root is not a use. The
    /// same holds for a reply of that shape.
    #[test]
    fn an_exact_array_root_has_no_head_and_is_not_a_use() {
        assert_no_interaction_use(|| {
            vec![
                fixed("window", 1, array(named("SensorReading"), 2, 2)),
                query(
                    "read",
                    2,
                    vec![param("at", named("Level"))],
                    array(named("SensorReading"), 2, 2),
                ),
            ]
        });
    }

    /// A tuple inside an exact-length array root is a named head, and the
    /// struct inside that tuple sits at a fixed offset of it.
    #[test]
    fn a_tuple_inside_an_exact_array_fixed_payload_holds_the_struct_inline() {
        assert_interaction_use(
            || {
                vec![fixed(
                    "window",
                    1,
                    array(
                        tuple(vec![
                            ("reading", named("SensorReading")),
                            ("at", named("Level")),
                        ]),
                        2,
                        2,
                    ),
                )]
            },
            "nested inline in veh.common/Ctl/window",
        );
    }

    /// A tuple reply is a head of its own with the struct at a fixed offset.
    #[test]
    fn a_tuple_reply_holds_the_struct_inline() {
        assert_interaction_use(
            || {
                vec![query(
                    "read",
                    1,
                    vec![param("at", named("Level"))],
                    tuple(vec![
                        ("reading", named("SensorReading")),
                        ("at", named("Level")),
                    ]),
                )]
            },
            "nested inline in veh.common/Ctl/read",
        );
    }

    /// A single named parameter is sent as it is, and a named reply is a root
    /// of its own: each may grow, so neither is a use.
    #[test]
    fn a_single_named_parameter_and_a_named_reply_are_roots_not_uses() {
        assert_no_interaction_use(|| {
            vec![
                command("set", 1, vec![param("reading", named("SensorReading"))]),
                query(
                    "read",
                    2,
                    vec![param("at", named("Level"))],
                    named("SensorReading"),
                ),
                fixed("last", 3, named("SensorReading")),
            ]
        });
    }

    /// `diff_refs_in` is `diff_refs_with` under no encoding.
    #[test]
    fn diff_refs_in_judges_under_no_encoding() {
        let old = inline_package(reading_v1());
        let new = inline_package(reading_v2());
        assert_eq!(
            crate::diff_refs_in(&[&old], &[&new], &[]),
            diff_refs_with(&[&old], &[&new], &[], &[])
        );
    }
}

mod podbuf_width {
    use ridl_ir::v2;

    use crate::{Category, Verdict, diff_refs_with, render_text};

    /// An enum whose values are `0..count`, named `V<n>`.
    fn enum_package(count: i64) -> v2::Package {
        let values = (0..count)
            .map(|value| v2::EnumValue {
                name: format!("V{value}"),
                value,
                ..Default::default()
            })
            .collect();
        with_decl(
            "Mode",
            v2::decl::Kind::EnumDef(v2::EnumDef {
                values,
                ..Default::default()
            }),
        )
    }

    /// A union with arms at ordinals `1..=count`, named `a<n>`, each a `Level`.
    fn union_package(count: u32) -> v2::Package {
        let arms = (1..=count)
            .map(|ordinal| v2::UnionArm {
                name: format!("a{ordinal}"),
                ordinal,
                type_ref: "Level".to_string(),
                ..Default::default()
            })
            .collect();
        let mut package = with_decl(
            "Outcome",
            v2::decl::Kind::UnionDef(v2::UnionDef {
                arms,
                ..Default::default()
            }),
        );
        package.decls.push(v2::Decl {
            name: "Level".to_string(),
            kind: Some(v2::decl::Kind::TypeDef(v2::TypeDef {
                backing: Some(v2::Backing {
                    kind: Some(v2::backing::Kind::Primitive(
                        v2::PrimitiveType::Integer as i32,
                    )),
                }),
                constraint: Some(v2::Constraint {
                    min: Some("0".to_string()),
                    max: Some("100".to_string()),
                    ..Default::default()
                }),
                declared_init: None,
                init: None,
                width: Some(v2::type_def::Width::IntWidth(v2::IntWidth::U8 as i32)),
            })),
            ..Default::default()
        });
        package
    }

    fn with_decl(name: &str, kind: v2::decl::Kind) -> v2::Package {
        v2::Package {
            name: "veh.cluster".to_string(),
            decls: vec![v2::Decl {
                name: name.to_string(),
                kind: Some(kind),
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    fn width_change(report: &crate::DiffReport) -> Option<&crate::Change> {
        report
            .changes
            .iter()
            .find(|change| change.category == Category::PodbufWidthChange)
    }

    /// Value 256 appended to an enum of `0..=255` widens its podbuf integer
    /// from `u8` to `u16`: breaking under podbuf, compatible otherwise, and
    /// listed under the podbuf heading either way.
    #[test]
    fn an_appended_enum_value_that_widens_the_integer_is_a_podbuf_break() {
        let (old, new) = (enum_package(256), enum_package(257));
        let report = diff_refs_with(&[&old], &[&new], &[], &["podbuf"]);
        let change = width_change(&report).expect("the width change is reported");
        assert_eq!(change.verdict, Verdict::Breaking);
        assert_eq!(change.path, "veh.cluster/Mode");
        assert_eq!(change.before.as_deref(), Some("u8"));
        assert_eq!(change.after.as_deref(), Some("u16"));
        assert_eq!(report.verdict, Verdict::Breaking);

        let report = diff_refs_with(&[&old], &[&new], &[], &[]);
        assert_eq!(report.verdict, Verdict::Compatible);
        assert_eq!(
            width_change(&report).map(|change| change.verdict),
            Some(Verdict::Compatible)
        );
        assert!(
            render_text(&report).contains(
                "breaking under podbuf:\n  [compatible] podbuf_width_change veh.cluster/Mode: u8 -> u16"
            ),
            "{}",
            render_text(&report)
        );
    }

    /// Value 255 appended to an enum of `0..=254` keeps the `u8`: no width
    /// line; the strict-enum line makes it breaking under podbuf.
    #[test]
    fn an_appended_enum_value_that_keeps_the_width_draws_no_width_line() {
        let (old, new) = (enum_package(255), enum_package(256));
        let report = diff_refs_with(&[&old], &[&new], &[], &["podbuf"]);
        assert_eq!(width_change(&report), None);
        assert_eq!(report.verdict, Verdict::Breaking);
    }

    /// Arm 256 appended to a union of 255 arms widens the tag from `u8` to
    /// `u16`; arm 255 appended to 254 arms keeps it.
    #[test]
    fn an_appended_union_arm_past_ordinal_255_widens_the_tag() {
        let (old, new) = (union_package(255), union_package(256));
        let report = diff_refs_with(&[&old], &[&new], &[], &["podbuf"]);
        let change = width_change(&report).expect("the width change is reported");
        assert_eq!(change.verdict, Verdict::Breaking);
        assert_eq!(change.path, "veh.cluster/Outcome");
        assert_eq!(change.before.as_deref(), Some("u8"));
        assert_eq!(change.after.as_deref(), Some("u16"));
        assert_eq!(report.verdict, Verdict::Breaking);
        assert_eq!(
            diff_refs_with(&[&old], &[&new], &[], &[]).verdict,
            Verdict::Compatible
        );

        let (old, new) = (union_package(254), union_package(255));
        let report = diff_refs_with(&[&old], &[&new], &[], &["podbuf"]);
        assert_eq!(width_change(&report), None);
        assert_eq!(report.verdict, Verdict::Compatible);
    }

    /// Two values appended at once draw one line for the enum.
    #[test]
    fn several_appended_values_draw_one_line() {
        let (old, new) = (enum_package(255), enum_package(258));
        let report = diff_refs_with(&[&old], &[&new], &[], &["podbuf"]);
        assert_eq!(
            report
                .changes
                .iter()
                .filter(|change| change.category == Category::PodbufWidthChange)
                .count(),
            1
        );
    }

    fn enum_set_package(count: i64) -> v2::Package {
        let bits = (0..count)
            .map(|bit| v2::EnumValue {
                name: format!("F{bit}"),
                value: bit,
                ..Default::default()
            })
            .collect();
        with_decl(
            "Flags",
            v2::decl::Kind::EnumSetDef(v2::EnumSetDef {
                bits,
                ..Default::default()
            }),
        )
    }

    fn strict_append(report: &crate::DiffReport) -> Vec<&crate::Change> {
        report
            .changes
            .iter()
            .filter(|change| change.category == Category::PodbufStrictEnumAppend)
            .collect()
    }

    /// A value appended to an enum, with the integer kept, is a strict-enum
    /// break under podbuf and compatible without it, listed under the podbuf
    /// heading either way.
    #[test]
    fn an_appended_enum_value_is_a_strict_enum_break_under_podbuf() {
        let (old, new) = (enum_package(3), enum_package(4));
        let report = diff_refs_with(&[&old], &[&new], &[], &["podbuf"]);
        let changes = strict_append(&report);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].verdict, Verdict::Breaking);
        assert_eq!(changes[0].path, "veh.cluster/Mode");
        assert_eq!(changes[0].after.as_deref(), Some("V3"));
        assert_eq!(report.verdict, Verdict::Breaking);

        let report = diff_refs_with(&[&old], &[&new], &[], &[]);
        assert_eq!(report.verdict, Verdict::Compatible);
        assert_eq!(strict_append(&report).len(), 1);
        assert!(
            render_text(&report).contains(
                "breaking under podbuf:\n  [compatible] podbuf_strict_enum_append veh.cluster/Mode"
            ),
            "{}",
            render_text(&report)
        );
        let report = diff_refs_with(&[&old], &[&new], &[], &["flatbuffers", "proto3"]);
        assert_eq!(report.verdict, Verdict::Compatible);
    }

    /// A flag appended to an enum set is the same break.
    #[test]
    fn an_appended_enum_set_flag_is_a_strict_enum_break_under_podbuf() {
        let (old, new) = (enum_set_package(3), enum_set_package(4));
        let report = diff_refs_with(&[&old], &[&new], &[], &["podbuf"]);
        let changes = strict_append(&report);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].verdict, Verdict::Breaking);
        assert_eq!(changes[0].path, "veh.cluster/Flags");
        assert_eq!(report.verdict, Verdict::Breaking);
        assert_eq!(
            diff_refs_with(&[&old], &[&new], &[], &[]).verdict,
            Verdict::Compatible
        );
    }

    /// An append that widens the integer draws the width line alone.
    #[test]
    fn an_append_that_widens_the_integer_draws_no_second_line() {
        let (old, new) = (enum_package(256), enum_package(257));
        let report = diff_refs_with(&[&old], &[&new], &[], &["podbuf"]);
        assert!(width_change(&report).is_some());
        assert_eq!(strict_append(&report), Vec::<&crate::Change>::new());
    }

    /// An appended union arm is not an enum append.
    #[test]
    fn an_appended_union_arm_draws_no_strict_enum_line() {
        let (old, new) = (union_package(2), union_package(3));
        let report = diff_refs_with(&[&old], &[&new], &[], &["podbuf"]);
        assert_eq!(strict_append(&report), Vec::<&crate::Change>::new());
    }
}
