//! Engine tests over constructed IR v2 packages. The engine never reads
//! source, so every fixture is built directly (ADR-0008 decision 14).

use ridl_ir::v2;

use crate::{Category, Change, Verdict, diff_packages, diff_sets, render_json};

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
/// identity is matched by name — the provisional numbers themselves may
/// differ, since a sibling added before it in byte order moves them — and its
/// body is diffed as any matched pair's.
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
    assert_eq!(report.verdict, Verdict::Compatible);
    assert_eq!(
        report.changes,
        vec![
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
