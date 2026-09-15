//! The comparison walk over two resolved IR v2 packages.
//!
//! The walk matches package declarations, interfaces, services, and
//! interactions by name within their container, then compares the aspects
//! that carry contract identity — ordinals, payloads, timings, returns,
//! parameters, contracts, widths, constraints, and inits — emitting one
//! [`Change`](crate::Change) per difference with an honest path.
//!
//! Ordinal analysis (ridl §11) is done on the surviving interactions of an
//! interface. A new slot is [`InteractionAppended`](crate::Category::InteractionAppended)
//! when it sits after every pre-existing slot and
//! [`InteractionInserted`](crate::Category::InteractionInserted) otherwise; a
//! surviving interaction whose *relative* order changed is
//! [`InteractionReordered`](crate::Category::InteractionReordered).
//!
//! A surviving interaction whose *absolute* ordinal shifted while its relative
//! order held is not reported a second time. That suppression is sound only
//! because every cause of such a shift is itself reported as breaking, so the
//! report verdict can never come out compatible while a wire identity moved
//! (ADR-0008 decision 14 lists "shifts or reuses a wire identity" first among
//! breaking changes). The causes are exhaustive:
//!
//! 1. an interaction added ahead of it — `InteractionInserted`;
//! 2. an interaction removed ahead of it with no tombstone —
//!    `InteractionRemoved`;
//! 3. an interaction retired to a tombstone in its own slot — no shift at all;
//! 4. an interaction retired to a tombstone written out of its slot —
//!    `InteractionRemoved`, because the freed slot is what the survivors slide
//!    into;
//! 5. a tombstone dropped ahead of it — `InteractionRemoved`, a permanent wire
//!    reservation cannot be released;
//! 6. a tombstone moved — `InteractionReordered`;
//! 7. a tombstone minted ahead of it for a name the old snapshot never held —
//!    `InteractionInserted`.
//!
//! Cases 4 through 7 are why tombstones carry their ordinal through the
//! comparison rather than being compared as a bare set of names: a retirement
//! is compatible, so a tombstone edit that shifts a wire identity would
//! otherwise reach no `Change` at all and the classifier downstream would have
//! nothing left to judge.

use std::collections::{BTreeMap, BTreeSet};

use ridl_ir::v2;

use crate::classify::{struct_slots, union_slots};
use crate::{Category, Change, emit};

/// Walks two matched packages, appending every difference to `changes`.
pub(crate) fn walk_packages(old: &v2::Package, new: &v2::Package, changes: &mut Vec<Change>) {
    let pkg = new.name.as_str();
    diff_decls(pkg, &old.decls, &new.decls, changes);
    diff_interfaces(pkg, &old.interfaces, &new.interfaces, changes);
    diff_services(pkg, &old.services, &new.services, changes);
}

// ==========================================================================
// Package-level typl declarations.
// ==========================================================================

fn diff_decls(pkg: &str, old: &[v2::Decl], new: &[v2::Decl], changes: &mut Vec<Change>) {
    let old_by: BTreeMap<&str, &v2::Decl> = old.iter().map(|d| (d.name.as_str(), d)).collect();
    let new_by: BTreeMap<&str, &v2::Decl> = new.iter().map(|d| (d.name.as_str(), d)).collect();

    for (name, old_decl) in &old_by {
        match new_by.get(name) {
            Some(new_decl) => diff_decl(pkg, name, old_decl, new_decl, changes),
            None => emit(
                changes,
                format!("{pkg}/{name}"),
                Category::DeclRemoved,
                Some(decl_kind_name(old_decl).to_string()),
                None,
            ),
        }
    }
    for (name, new_decl) in &new_by {
        if !old_by.contains_key(name) {
            emit(
                changes,
                format!("{pkg}/{name}"),
                Category::DeclAdded,
                None,
                Some(decl_kind_name(new_decl).to_string()),
            );
        }
    }
}

fn diff_decl(pkg: &str, name: &str, old: &v2::Decl, new: &v2::Decl, changes: &mut Vec<Change>) {
    let path = format!("{pkg}/{name}");
    if envelope_differs(old, new) {
        emit(changes, path.clone(), Category::DocOnly, None, None);
    }
    emit_visibility(changes, path.clone(), old.visibility, new.visibility);

    use v2::decl::Kind;
    match (&old.kind, &new.kind) {
        (Some(Kind::TypeDef(a)), Some(Kind::TypeDef(b))) => diff_type_def(&path, a, b, changes),
        (Some(Kind::ConstDef(a)), Some(Kind::ConstDef(b))) => {
            if a != b {
                emit(
                    changes,
                    path,
                    Category::InitChanged,
                    Some(const_str(a)),
                    Some(const_str(b)),
                );
            }
        }
        (Some(Kind::StructDef(a)), Some(Kind::StructDef(b))) => {
            diff_composite(
                &path,
                struct_slots(a),
                struct_slots(b),
                "ordinal",
                a == b,
                || struct_ignoring_order(a) == struct_ignoring_order(b),
                changes,
            );
        }
        (Some(Kind::EnumDef(a)), Some(Kind::EnumDef(b))) => {
            diff_composite(
                &path,
                positions(&a.values),
                positions(&b.values),
                "position",
                a == b,
                || enum_ignoring_order(a) == enum_ignoring_order(b),
                changes,
            );
        }
        (Some(Kind::EnumSetDef(a)), Some(Kind::EnumSetDef(b))) => {
            diff_composite(
                &path,
                positions(&a.bits),
                positions(&b.bits),
                "position",
                a == b,
                || enum_set_ignoring_order(a) == enum_set_ignoring_order(b),
                changes,
            );
        }
        (Some(Kind::UnionDef(a)), Some(Kind::UnionDef(b))) => {
            diff_composite(
                &path,
                union_slots(a),
                union_slots(b),
                "ordinal",
                a == b,
                || union_ignoring_order(a) == union_ignoring_order(b),
                changes,
            );
        }
        (old_kind, new_kind) => {
            if kind_discriminant(old_kind) != kind_discriminant(new_kind) {
                emit(
                    changes,
                    path,
                    Category::KindChanged,
                    Some(kind_name_opt(old_kind).to_string()),
                    Some(kind_name_opt(new_kind).to_string()),
                );
            }
        }
    }
}

fn diff_type_def(path: &str, a: &v2::TypeDef, b: &v2::TypeDef, changes: &mut Vec<Change>) {
    if a.backing != b.backing || a.width != b.width {
        emit(
            changes,
            path.to_string(),
            Category::WidthChanged,
            Some(type_repr_str(a)),
            Some(type_repr_str(b)),
        );
    }
    if a.constraint != b.constraint {
        emit(
            changes,
            path.to_string(),
            Category::ConstraintChanged,
            Some(constraint_str(a.constraint.as_ref())),
            Some(constraint_str(b.constraint.as_ref())),
        );
    }
    if a.declared_init != b.declared_init || a.init != b.init {
        emit(
            changes,
            path.to_string(),
            Category::InitChanged,
            Some(type_init_str(a)),
            Some(type_init_str(b)),
        );
    }
}

/// A coarse comparison of a composite type body (struct fields, enum values,
/// enum-set bits, union arms) by member name, each member paired with its
/// slot. A member present on one side is an addition or removal, and the
/// comparison stops there: a reorder in the same edit as an addition or a
/// removal reports no `MemberReordered`. The classifier reads the bodies
/// themselves to decide an addition's direction, so the append-only rule of
/// typl §7.4 is judged there.
///
/// With the same member names on both sides, a member whose slot changed is
/// one `MemberReordered` each, carrying the old and new slot — with the
/// container's `ConstraintChanged` as well when the body's content also
/// changed — and a body changed in place with every slot untouched is the
/// container's `ConstraintChanged` alone. The slot is the ordinal for a struct
/// field or union arm, which is the wire identity (typl §7.4), so a field a
/// moved tombstone shifted is reported even though the live names kept their
/// order. An enum value or enum-set bit carries an explicit number instead, and
/// its slot here is its 1-based position in the body: the comparison is over
/// positions, not those numbers, so a textual reorder is reported the same way
/// (typl §17.14). `slot` names the unit in the rendered detail.
///
/// **Known limitation (carried debt).** This comparison is keyed on member
/// names and never reads the body's `reserved` list, so it cannot tell a bare
/// deletion from typl §7.4's sanctioned "delete by tombstone" — both arrive at
/// the classifier as `DeclRemoved`, which classifies breaking. That errs in the
/// safe direction, but it makes a sanctioned retirement gate CI, so a team that
/// retires a struct field correctly still has to override the gate. Closing it
/// means matching removals against the container's `reserved` entries — the IR
/// carries everything needed (`Reserved.name` for struct and union bodies,
/// `Reserved.value` for enum bodies). Recorded for a later epic rather than
/// taken here, because E2.8b's normative table scopes its tombstone row to
/// interactions. Do not close this alone: driftsys/ridl#302 records a
/// FlatBuffers union-discriminant coupling that must move with it.
fn diff_composite(
    path: &str,
    old: Vec<(String, i64)>,
    new: Vec<(String, i64)>,
    slot: &str,
    equal: bool,
    equal_ignoring_order: impl FnOnce() -> bool,
    changes: &mut Vec<Change>,
) {
    if equal {
        return;
    }
    let old_set: BTreeSet<&String> = old.iter().map(|(name, _)| name).collect();
    let new_set: BTreeSet<&String> = new.iter().map(|(name, _)| name).collect();
    let mut structural = false;
    for (name, _) in &new {
        if !old_set.contains(name) {
            emit(
                changes,
                format!("{path}/{name}"),
                Category::DeclAdded,
                None,
                Some(name.clone()),
            );
            structural = true;
        }
    }
    for (name, _) in &old {
        if !new_set.contains(name) {
            emit(
                changes,
                format!("{path}/{name}"),
                Category::DeclRemoved,
                Some(name.clone()),
                None,
            );
            structural = true;
        }
    }
    if structural {
        return;
    }
    // The member names match on both sides, so the difference is a reorder of
    // the body, a member changed in place, or both. A reorder is its own
    // category: for struct fields and union arms the ordinal is wire identity,
    // and an enum or enumset body is reported the same way conservatively,
    // because this walk compares member positions rather than the explicit
    // values. Reporting a reorder as a constraint edit sends the reader looking
    // for a constraint that did not change (driftsys/ridl#314). None of this
    // changes removal matching, so the carried debt above — and
    // driftsys/ridl#302's coupling — stays exactly as it is.
    let mut moved = false;
    for (name, new_slot) in &new {
        let (_, old_slot) = old
            .iter()
            .find(|(old_name, _)| old_name == name)
            .expect("the name sets are equal, so every new name appears in the old body");
        if old_slot != new_slot {
            moved = true;
            emit(
                changes,
                format!("{path}/{name}"),
                Category::MemberReordered,
                Some(format!("{slot} {old_slot}")),
                Some(format!("{slot} {new_slot}")),
            );
        }
    }
    // With no slot moved, the bodies differ in content only: a member changed
    // in place, or a tombstone list changed without shifting a live member. A
    // reorder can also arrive in the same edit as a change to a member's
    // content or to the container itself. Comparing the bodies with member
    // order removed tells the two apart: when they still differ, the
    // container's `ConstraintChanged` is reported as well, after the
    // per-member lines, so the content change is not hidden behind the
    // reorder.
    if !moved || !equal_ignoring_order() {
        emit(
            changes,
            path.to_string(),
            Category::ConstraintChanged,
            None,
            None,
        );
    }
}

// ==========================================================================
// Interfaces and interactions.
// ==========================================================================

fn diff_interfaces(
    pkg: &str,
    old: &[v2::Interface],
    new: &[v2::Interface],
    changes: &mut Vec<Change>,
) {
    let old_by: BTreeMap<&str, &v2::Interface> = old.iter().map(|i| (i.name.as_str(), i)).collect();
    let new_by: BTreeMap<&str, &v2::Interface> = new.iter().map(|i| (i.name.as_str(), i)).collect();

    for (name, old_iface) in &old_by {
        match new_by.get(name) {
            Some(new_iface) => {
                if interface_envelope_differs(old_iface, new_iface) {
                    emit(
                        changes,
                        format!("{pkg}/{name}"),
                        Category::DocOnly,
                        None,
                        None,
                    );
                }
                emit_visibility(
                    changes,
                    format!("{pkg}/{name}"),
                    old_iface.visibility,
                    new_iface.visibility,
                );
                diff_interface(pkg, name, old_iface, new_iface, changes);
            }
            None => emit(
                changes,
                format!("{pkg}/{name}"),
                Category::DeclRemoved,
                Some("interface".to_string()),
                None,
            ),
        }
    }
    for name in new_by.keys() {
        if !old_by.contains_key(name) {
            emit(
                changes,
                format!("{pkg}/{name}"),
                Category::DeclAdded,
                None,
                Some("interface".to_string()),
            );
        }
    }
}

/// The core interaction walk for one matched interface (used for both a
/// top-level interface and a service's inline shape).
fn diff_interface(
    pkg: &str,
    iface: &str,
    old: &v2::Interface,
    new: &v2::Interface,
    changes: &mut Vec<Change>,
) {
    let old_live = live_interactions(old);
    let new_live = live_interactions(new);
    let old_reserved = reserved_names(old);
    let new_reserved = reserved_names(new);

    let old_live_names: BTreeSet<&str> = old_live.iter().map(|(name, ..)| *name).collect();
    let new_live_map: BTreeMap<&str, (u32, &v2::Decl)> = new_live
        .iter()
        .map(|(name, ord, decl)| (*name, (*ord, *decl)))
        .collect();

    // (name, old ordinal, new ordinal) for interactions present on both sides.
    let mut matched: Vec<(&str, u32, u32)> = Vec::new();

    for (name, old_ord, old_decl) in &old_live {
        let member_path = format!("{pkg}/{iface}/{name}");
        if let Some((new_ord, new_decl)) = new_live_map.get(name) {
            matched.push((name, *old_ord, *new_ord));
            diff_interaction(iface, &member_path, old_decl, new_decl, changes);
        } else if let Some(reserved_ord) = new_reserved.get(name) {
            // A tombstone must hold the retired interaction's own ordinal
            // (ridl §11). A tombstone placed elsewhere lets the surviving
            // interactions slide into the freed slot — a wire break that the
            // relative-order check below cannot see.
            if *reserved_ord == *old_ord {
                emit(
                    changes,
                    member_path,
                    Category::InteractionRetired,
                    Some(interaction_desc(old_decl)),
                    Some("reserved".to_string()),
                );
            } else {
                emit(
                    changes,
                    member_path,
                    Category::InteractionRemoved,
                    Some(format!(
                        "{} at ordinal {old_ord}",
                        interaction_desc(old_decl)
                    )),
                    Some(format!("reserved at ordinal {reserved_ord}")),
                );
            }
        } else {
            emit(
                changes,
                member_path,
                Category::InteractionRemoved,
                Some(interaction_desc(old_decl)),
                None,
            );
        }
    }

    // A tombstone is a permanent wire reservation (ridl §11). Dropping one, or
    // moving it, lets every later interaction slide down into the freed slot —
    // a wire break invisible to the relative-order check below.
    for (name, old_ord) in &old_reserved {
        if new_live_map.contains_key(name) {
            continue; // handled as ReservedNameRedeclared below.
        }
        match new_reserved.get(name) {
            Some(new_ord) if new_ord == old_ord => {}
            Some(new_ord) => emit(
                changes,
                format!("{pkg}/{iface}/{name}"),
                Category::InteractionReordered,
                Some(format!("reserved at ordinal {old_ord}")),
                Some(format!("reserved at ordinal {new_ord}")),
            ),
            None => emit(
                changes,
                format!("{pkg}/{iface}/{name}"),
                Category::InteractionRemoved,
                Some(format!("reserved at ordinal {old_ord}")),
                None,
            ),
        }
    }

    let anchor_ordinals = anchor_ordinals(&matched, new, &old_live_names, &old_reserved);

    // A tombstone minted for a name the old snapshot never held reserves a
    // fresh slot. At the end of the body that is harmless; written earlier it
    // pushes every later interaction down one slot — the same wire break as an
    // inserted interaction, and the last shift cause that would otherwise reach
    // no `Change` at all.
    for (name, new_ord) in &new_reserved {
        if old_live_names.contains(name) || old_reserved.contains_key(name) {
            continue;
        }
        let category = if anchor_ordinals.iter().any(|&anchor| anchor > *new_ord) {
            Category::InteractionInserted
        } else {
            Category::InteractionAppended
        };
        emit(
            changes,
            format!("{pkg}/{iface}/{name}"),
            category,
            None,
            Some(format!("reserved at ordinal {new_ord}")),
        );
    }

    for (name, new_ord, new_decl) in &new_live {
        if old_live_names.contains(name) {
            continue;
        }
        let member_path = format!("{pkg}/{iface}/{name}");
        if old_reserved.contains_key(name) {
            emit(
                changes,
                member_path,
                Category::ReservedNameRedeclared,
                Some("reserved".to_string()),
                Some(interaction_desc(new_decl)),
            );
        } else {
            let category = if anchor_ordinals.iter().any(|&anchor| anchor > *new_ord) {
                Category::InteractionInserted
            } else {
                Category::InteractionAppended
            };
            emit(
                changes,
                member_path,
                category,
                None,
                Some(interaction_desc(new_decl)),
            );
        }
    }

    detect_reorders(pkg, iface, &matched, changes);
}

/// Flags surviving interactions whose relative order within the interface
/// changed. An absolute-ordinal change that preserves relative order is a
/// consequence of a slot added or released elsewhere and is not reported here;
/// see the module documentation for why every such cause is independently
/// reported as breaking.
fn detect_reorders(
    pkg: &str,
    iface: &str,
    matched: &[(&str, u32, u32)],
    changes: &mut Vec<Change>,
) {
    let old_rank = rank_by(matched, |(_, old_ord, _)| *old_ord);
    let new_rank = rank_by(matched, |(_, _, new_ord)| *new_ord);
    for (name, old_ord, new_ord) in matched {
        if old_rank[name] != new_rank[name] {
            emit(
                changes,
                format!("{pkg}/{iface}/{name}"),
                Category::InteractionReordered,
                Some(old_ord.to_string()),
                Some(new_ord.to_string()),
            );
        }
    }
}

/// Ranks matched interaction names by a chosen ordinal, returning name → rank.
fn rank_by<'a>(
    matched: &[(&'a str, u32, u32)],
    key: impl Fn(&(&'a str, u32, u32)) -> u32,
) -> BTreeMap<&'a str, usize> {
    let mut order: Vec<&(&'a str, u32, u32)> = matched.iter().collect();
    order.sort_by_key(|entry| key(entry));
    order
        .iter()
        .enumerate()
        .map(|(rank, (name, _, _))| (*name, rank))
        .collect()
}

/// The new-side ordinals of the slots that existed before the change —
/// surviving interactions and the tombstones of names the old snapshot already
/// held. A new slot sitting before any of these was inserted, not appended. A
/// tombstone minted in this change is new content, so it is not itself an
/// anchor.
fn anchor_ordinals(
    matched: &[(&str, u32, u32)],
    new: &v2::Interface,
    old_live_names: &BTreeSet<&str>,
    old_reserved: &BTreeMap<&str, u32>,
) -> Vec<u32> {
    let mut ordinals: Vec<u32> = matched.iter().map(|(_, _, new_ord)| *new_ord).collect();
    for decl in &new.interactions {
        if let Some(v2::decl::Kind::ReservedSlot(reserved)) = &decl.kind
            && let Some(name) = reserved.name.as_deref()
            && (old_live_names.contains(name) || old_reserved.contains_key(name))
        {
            ordinals.push(decl.ordinal);
        }
    }
    ordinals
}

/// Compares two matched interactions of the same name: kind, then the
/// kind-specific contract-carrying fields, then the doc envelope. `iface` is the
/// enclosing interface name, needed to derive an inline `T | E` transport
/// identity (ADR-0008 decision 4).
fn diff_interaction(
    iface: &str,
    path: &str,
    old: &v2::Decl,
    new: &v2::Decl,
    changes: &mut Vec<Change>,
) {
    if envelope_differs(old, new) {
        emit(changes, path.to_string(), Category::DocOnly, None, None);
    }
    emit_visibility(changes, path.to_string(), old.visibility, new.visibility);

    use v2::decl::Kind;
    match (&old.kind, &new.kind) {
        (Some(Kind::SignalDef(a)), Some(Kind::SignalDef(b))) => {
            if a.payload != b.payload {
                emit(
                    changes,
                    path.to_string(),
                    Category::PayloadChanged,
                    Some(a.payload.clone()),
                    Some(b.payload.clone()),
                );
            }
            if a.declared_init != b.declared_init || a.init != b.init {
                emit(
                    changes,
                    path.to_string(),
                    Category::InitChanged,
                    Some(signal_init_str(a)),
                    Some(signal_init_str(b)),
                );
            }
            if a.timing != b.timing {
                emit(
                    changes,
                    path.to_string(),
                    Category::TimingChanged,
                    Some(timing_str(a.timing.as_ref())),
                    Some(timing_str(b.timing.as_ref())),
                );
            }
        }
        (Some(Kind::EventDef(a)), Some(Kind::EventDef(b))) => {
            if a.payload != b.payload {
                emit(
                    changes,
                    path.to_string(),
                    Category::PayloadChanged,
                    Some(a.payload.clone()),
                    Some(b.payload.clone()),
                );
            }
            if a.timing != b.timing {
                emit(
                    changes,
                    path.to_string(),
                    Category::TimingChanged,
                    Some(timing_str(a.timing.as_ref())),
                    Some(timing_str(b.timing.as_ref())),
                );
            }
        }
        (Some(Kind::CommandDef(a)), Some(Kind::CommandDef(b))) => {
            if a.params != b.params {
                emit(
                    changes,
                    path.to_string(),
                    Category::ParamsChanged,
                    Some(params_str(&a.params)),
                    Some(params_str(&b.params)),
                );
            }
            if a.contracts != b.contracts {
                emit(
                    changes,
                    path.to_string(),
                    Category::ContractChanged,
                    Some(contracts_str(&a.contracts)),
                    Some(contracts_str(&b.contracts)),
                );
            }
            // The declared RPC bounds travel as their own category, not as
            // `TimingChanged`: the `min` direction inverts on an RPC
            // (ADR-0015 decision 8).
            if a.timing != b.timing {
                emit(
                    changes,
                    path.to_string(),
                    Category::RpcBoundChanged,
                    Some(timing_str(a.timing.as_ref())),
                    Some(timing_str(b.timing.as_ref())),
                );
            }
        }
        (Some(Kind::QueryDef(a)), Some(Kind::QueryDef(b))) => {
            if a.params != b.params {
                emit(
                    changes,
                    path.to_string(),
                    Category::ParamsChanged,
                    Some(params_str(&a.params)),
                    Some(params_str(&b.params)),
                );
            }
            if a.return_type != b.return_type {
                emit(
                    changes,
                    path.to_string(),
                    Category::ReturnChanged,
                    Some(return_str(iface, old.ordinal, a.return_type.as_ref())),
                    Some(return_str(iface, new.ordinal, b.return_type.as_ref())),
                );
            }
            if a.contracts != b.contracts {
                emit(
                    changes,
                    path.to_string(),
                    Category::ContractChanged,
                    Some(contracts_str(&a.contracts)),
                    Some(contracts_str(&b.contracts)),
                );
            }
            // As on a command: the declared RPC bounds are their own category
            // (ADR-0015 decision 8).
            if a.timing != b.timing {
                emit(
                    changes,
                    path.to_string(),
                    Category::RpcBoundChanged,
                    Some(timing_str(a.timing.as_ref())),
                    Some(timing_str(b.timing.as_ref())),
                );
            }
        }
        (Some(Kind::FixedDef(a)), Some(Kind::FixedDef(b))) => {
            if a.payload != b.payload {
                emit(
                    changes,
                    path.to_string(),
                    Category::PayloadChanged,
                    Some(field_type_opt_str(a.payload.as_ref())),
                    Some(field_type_opt_str(b.payload.as_ref())),
                );
            }
        }
        (old_kind, new_kind) => {
            emit(
                changes,
                path.to_string(),
                Category::KindChanged,
                Some(kind_name_opt(old_kind).to_string()),
                Some(kind_name_opt(new_kind).to_string()),
            );
        }
    }
}

// ==========================================================================
// Services.
// ==========================================================================

fn diff_services(pkg: &str, old: &[v2::Service], new: &[v2::Service], changes: &mut Vec<Change>) {
    let old_by: BTreeMap<&str, &v2::Service> = old.iter().map(|s| (s.name.as_str(), s)).collect();
    let new_by: BTreeMap<&str, &v2::Service> = new.iter().map(|s| (s.name.as_str(), s)).collect();

    for (name, old_svc) in &old_by {
        match new_by.get(name) {
            Some(new_svc) => diff_service(pkg, name, old_svc, new_svc, changes),
            None => emit(
                changes,
                format!("{pkg}/{name}"),
                Category::DeclRemoved,
                Some("service".to_string()),
                None,
            ),
        }
    }
    for name in new_by.keys() {
        if !old_by.contains_key(name) {
            emit(
                changes,
                format!("{pkg}/{name}"),
                Category::DeclAdded,
                None,
                Some("service".to_string()),
            );
        }
    }
}

fn diff_service(
    pkg: &str,
    name: &str,
    old: &v2::Service,
    new: &v2::Service,
    changes: &mut Vec<Change>,
) {
    let path = format!("{pkg}/{name}");
    if service_envelope_differs(old, new) {
        emit(changes, path.clone(), Category::DocOnly, None, None);
    }
    emit_visibility(changes, path.clone(), old.visibility, new.visibility);

    // The `INLINE` slot marks the inline form (ADR-0015 decision 14: one
    // inline shape, never mixed with named shapes). A switch between the two
    // forms stays `ServiceChanged` — extraction rewrites the transport
    // identity of every fallible query in the shape (ADR-0015 decision 15) —
    // while a changed list is read as a set by `diff_service_set` (decision
    // 19 as amended on 2026-09-15, which narrows `ServiceChanged` to the form
    // switch).
    match (inline_shape(old), inline_shape(new)) {
        (Some(a), Some(b)) => diff_interface(pkg, name, a, b, changes),
        (None, None) => diff_service_set(pkg, name, old, new, changes),
        (old_inline, new_inline) => {
            emit(
                changes,
                path,
                Category::ServiceChanged,
                Some(form_desc(old_inline.is_some()).to_string()),
                Some(form_desc(new_inline.is_some()).to_string()),
            );
        }
    }
}

/// The set walk of one matched named-form service (ADR-0015 decision 19 as
/// amended on 2026-09-15): a service's list is a set of interface references,
/// keyed on the canonical reference the checker lowered. An interface in one
/// set and not the other is `ServiceInterfaceRemoved` or
/// `ServiceInterfaceAdded`; the list's order carries nothing, so a reorder is
/// no change. A `reserved` slot the IR may still carry is ignored: a set holds
/// no tombstone. The change path is `<package>/<service>/<reference>`, the
/// reference whole because two references may share a final segment.
fn diff_service_set(
    pkg: &str,
    svc: &str,
    old: &v2::Service,
    new: &v2::Service,
    changes: &mut Vec<Change>,
) {
    let old_refs = interface_refs(old);
    let new_refs = interface_refs(new);
    for reference in old_refs.difference(&new_refs) {
        emit(
            changes,
            format!("{pkg}/{svc}/{reference}"),
            Category::ServiceInterfaceRemoved,
            Some((*reference).to_string()),
            None,
        );
    }
    for reference in new_refs.difference(&old_refs) {
        emit(
            changes,
            format!("{pkg}/{svc}/{reference}"),
            Category::ServiceInterfaceAdded,
            None,
            Some((*reference).to_string()),
        );
    }
}

// ==========================================================================
// Collectors.
// ==========================================================================

/// The live interactions of an interface — every member that is not a reserved
/// tombstone — as (name, ordinal, decl).
fn live_interactions(iface: &v2::Interface) -> Vec<(&str, u32, &v2::Decl)> {
    iface
        .interactions
        .iter()
        .filter_map(|decl| match &decl.kind {
            Some(v2::decl::Kind::ReservedSlot(_)) | None => None,
            Some(_) => Some((decl.name.as_str(), decl.ordinal, decl)),
        })
        .collect()
}

/// The names retired by `reserved` tombstones in an interface body, each with
/// the ordinal its tombstone holds.
fn reserved_names(iface: &v2::Interface) -> BTreeMap<&str, u32> {
    iface
        .interactions
        .iter()
        .filter_map(|decl| match &decl.kind {
            Some(v2::decl::Kind::ReservedSlot(reserved)) => {
                reserved.name.as_deref().map(|name| (name, decl.ordinal))
            }
            _ => None,
        })
        .collect()
}

/// The inline shape of a service, when it carries one — the `INLINE` slot
/// that marks the inline form (ADR-0015 decision 14).
fn inline_shape(service: &v2::Service) -> Option<&v2::Interface> {
    service.shapes.iter().find_map(|slot| match &slot.kind {
        Some(v2::service_shape::Kind::Inline(interface)) => Some(interface),
        _ => None,
    })
}

/// The interface references of a service's set — every `InterfaceRef` slot,
/// by its canonical reference. An inline or `reserved` slot is not a member.
fn interface_refs(service: &v2::Service) -> BTreeSet<&str> {
    service
        .shapes
        .iter()
        .filter_map(|slot| match &slot.kind {
            Some(v2::service_shape::Kind::InterfaceRef(reference)) => Some(reference.as_str()),
            _ => None,
        })
        .collect()
}

/// Enum values or enum-set bits, each with its 1-based position in the body.
/// The explicit number is not read here: the reorder comparison is over
/// positions, conservatively (typl §17.14).
fn positions(values: &[v2::EnumValue]) -> Vec<(String, i64)> {
    values
        .iter()
        .zip(1..)
        .map(|(value, position)| (value.name.clone(), position))
        .collect()
}

// The four `*_ignoring_order` helpers return a body with its member order
// removed, so that `==` over two of them asks whether the bodies are equal
// once order is ignored — the question `diff_composite` needs to tell a pure
// reorder from a reorder combined with a content change. Each removes exactly
// two things from what `a == b` compares: the order of the members, and every
// field whose value is derived from position (the 1-based `ordinal` of a
// struct field, a union arm, and a struct or union tombstone — typl §7.4).
// Everything else stays in the comparison: member names, types, inits, docs,
// labels, deprecations, the explicit value of an enum value or enum-set bit,
// the retired identity of a tombstone, and the container's own fields.

/// A struct body with its member order removed (see above). Fields and
/// tombstones share the `members` list and one ordinal counter, so both have
/// the ordinal cleared and both are sorted, fields before tombstones.
fn struct_ignoring_order(def: &v2::StructDef) -> v2::StructDef {
    use v2::struct_member::Member;
    let mut def = def.clone();
    for member in &mut def.members {
        match &mut member.member {
            Some(Member::Field(field)) => field.ordinal = 0,
            Some(Member::Reserved(reserved)) => reserved.ordinal = 0,
            None => {}
        }
    }
    def.members.sort_by_key(|member| match &member.member {
        Some(Member::Field(field)) => (0, Some(field.name.clone()), None),
        Some(Member::Reserved(reserved)) => (1, reserved.name.clone(), reserved.value),
        None => (2, None, None),
    });
    def
}

/// An enum body with its member order removed (see above). An enum value
/// carries no ordinal — its number is explicit content and stays compared —
/// and an enum tombstone's ordinal is always 0, so only the two lists are
/// sorted.
fn enum_ignoring_order(def: &v2::EnumDef) -> v2::EnumDef {
    let mut def = def.clone();
    def.values.sort_by_key(|value| value.name.clone());
    def.reserved.sort_by_key(tombstone_key);
    def
}

/// An enum-set body with its member order removed (see above). A bit carries
/// no ordinal — its position number is explicit content and stays compared —
/// so only the list is sorted.
fn enum_set_ignoring_order(def: &v2::EnumSetDef) -> v2::EnumSetDef {
    let mut def = def.clone();
    def.bits.sort_by_key(|bit| bit.name.clone());
    def
}

/// A union body with its member order removed (see above). Arms and
/// tombstones are two lists over one ordinal counter, so both have the
/// ordinal cleared and both are sorted.
fn union_ignoring_order(def: &v2::UnionDef) -> v2::UnionDef {
    let mut def = def.clone();
    for arm in &mut def.arms {
        arm.ordinal = 0;
    }
    def.arms.sort_by_key(|arm| arm.name.clone());
    for reserved in &mut def.reserved {
        reserved.ordinal = 0;
    }
    def.reserved.sort_by_key(tombstone_key);
    def
}

/// The retired identity of a tombstone — its name in a struct or union body,
/// its value in an enum body — as a sort key.
fn tombstone_key(reserved: &v2::Reserved) -> (Option<String>, Option<i64>) {
    (reserved.name.clone(), reserved.value)
}

// ==========================================================================
// Envelope comparison.
// ==========================================================================

/// Whether anything in the doc envelope changed.
///
/// Visibility is deliberately **not** part of this comparison. It is metadata
/// in the surface grammar, but it is not metadata to a consumer: `internal`
/// maps to a target's package-private mechanism — Rust `pub(crate)`, a
/// non-exported TypeScript member (ADR-0002 §8) — so narrowing it deletes the
/// declaration from every out-of-package consumer. It travels as its own
/// category so it can be classified by direction.
fn envelope_differs(old: &v2::Decl, new: &v2::Decl) -> bool {
    old.doc != new.doc || old.labels != new.labels || old.deprecated != new.deprecated
}

/// The same comparison as [`envelope_differs`], over an interface. The body is
/// identical but the parameter type is not, and `Decl`, `Interface`, and
/// `Service` are three unrelated generated structs with no shared trait, so none
/// can call another without a trait written only to join them. The third copy is
/// [`service_envelope_differs`].
fn interface_envelope_differs(old: &v2::Interface, new: &v2::Interface) -> bool {
    old.doc != new.doc || old.labels != new.labels || old.deprecated != new.deprecated
}

/// The third copy, over a service — named rather than inlined so all three are
/// greppable together.
fn service_envelope_differs(old: &v2::Service, new: &v2::Service) -> bool {
    old.doc != new.doc || old.labels != new.labels || old.deprecated != new.deprecated
}

/// Emits a [`Category::VisibilityChanged`] when the two sides publish a
/// declaration at different visibilities. The classifier reads the direction.
fn emit_visibility(changes: &mut Vec<Change>, path: String, old: i32, new: i32) {
    if old == new {
        return;
    }
    emit(
        changes,
        path,
        Category::VisibilityChanged,
        Some(visibility_name(old).to_string()),
        Some(visibility_name(new).to_string()),
    );
}

fn visibility_name(visibility: i32) -> &'static str {
    match v2::Visibility::try_from(visibility) {
        Ok(v2::Visibility::Public) => "public",
        Ok(v2::Visibility::Internal) => "internal",
        _ => "unspecified",
    }
}

// ==========================================================================
// Renderers — honest, compact before/after strings.
// ==========================================================================

fn decl_kind_name(decl: &v2::Decl) -> &'static str {
    kind_name_opt(&decl.kind)
}

fn kind_name_opt(kind: &Option<v2::decl::Kind>) -> &'static str {
    use v2::decl::Kind;
    match kind {
        Some(Kind::TypeDef(_)) => "type",
        Some(Kind::ConstDef(_)) => "const",
        Some(Kind::StructDef(_)) => "struct",
        Some(Kind::EnumDef(_)) => "enum",
        Some(Kind::EnumSetDef(_)) => "enum set",
        Some(Kind::UnionDef(_)) => "union",
        Some(Kind::SignalDef(_)) => "signal",
        Some(Kind::EventDef(_)) => "event",
        Some(Kind::CommandDef(_)) => "command",
        Some(Kind::QueryDef(_)) => "query",
        Some(Kind::FixedDef(_)) => "fixed",
        Some(Kind::ReservedSlot(_)) => "reserved",
        None => "declaration",
    }
}

/// A stable discriminant for a decl kind, so a variant switch is a
/// `KindChanged` while a same-variant edit is not.
fn kind_discriminant(kind: &Option<v2::decl::Kind>) -> u8 {
    use v2::decl::Kind;
    match kind {
        None => 0,
        Some(Kind::TypeDef(_)) => 1,
        Some(Kind::ConstDef(_)) => 2,
        Some(Kind::StructDef(_)) => 3,
        Some(Kind::EnumDef(_)) => 4,
        Some(Kind::EnumSetDef(_)) => 5,
        Some(Kind::UnionDef(_)) => 6,
        Some(Kind::SignalDef(_)) => 7,
        Some(Kind::EventDef(_)) => 8,
        Some(Kind::CommandDef(_)) => 9,
        Some(Kind::QueryDef(_)) => 10,
        Some(Kind::FixedDef(_)) => 11,
        Some(Kind::ReservedSlot(_)) => 12,
    }
}

fn interaction_desc(decl: &v2::Decl) -> String {
    format!("{} {}", kind_name_opt(&decl.kind), decl.name)
}

/// The form a service publishes in, for the `ServiceChanged` report: the
/// inline shape, or the named shape list.
fn form_desc(inline: bool) -> &'static str {
    if inline {
        "inline shape"
    } else {
        "interface list"
    }
}

fn timing_str(timing: Option<&v2::Timing>) -> String {
    let Some(timing) = timing else {
        return "(none)".to_string();
    };
    let mode = match v2::TimingMode::try_from(timing.mode) {
        Ok(v2::TimingMode::StrictPeriodic) => "strict",
        Ok(v2::TimingMode::Range) => "range",
        _ => "unspecified",
    };
    let min = timing.min_us.as_deref().unwrap_or("_");
    let max = timing.max_us.as_deref().unwrap_or("_");
    let default = if timing.default_applied {
        " (default)"
    } else {
        ""
    };
    format!("{mode} [{min}us..{max}us]{default}")
}

fn params_str(params: &[v2::Param]) -> String {
    let rendered: Vec<String> = params
        .iter()
        .map(|param| {
            format!(
                "{}: {}",
                param.name,
                field_type_opt_str(param.r#type.as_ref())
            )
        })
        .collect();
    format!("({})", rendered.join(", "))
}

fn contracts_str(contracts: &[v2::Contract]) -> String {
    let rendered: Vec<String> = contracts
        .iter()
        .map(|contract| {
            let kind = match v2::ContractKind::try_from(contract.kind) {
                Ok(v2::ContractKind::Require) => "require",
                Ok(v2::ContractKind::Ensure) => "ensure",
                _ => "clause",
            };
            format!("{kind} {}", contract.source)
        })
        .collect();
    format!("[{}]", rendered.join("; "))
}

/// Renders a query return shape.
///
/// An inline `T | E` renders with its synthesized transport identity alongside
/// the arms (ADR-0008 decision 4: interface + interaction ordinal + ordered arm
/// types). The identity is what a backend keys on, so a report that only showed
/// the arm spelling would hide which of the two changed things actually moved —
/// the same arms at a different ordinal are a different identity.
fn return_str(iface: &str, ordinal: u32, return_type: Option<&v2::ReturnType>) -> String {
    let Some(return_type) = return_type else {
        return "()".to_string();
    };
    match &return_type.kind {
        Some(v2::return_type::Kind::Value(value)) => field_type_str(value),
        Some(v2::return_type::Kind::Fallible(fallible)) => format!(
            "{} | {} (transport identity {})",
            fallible.ok,
            fallible.err,
            v2::fallible_transport_identity(iface, ordinal, fallible)
        ),
        None => "()".to_string(),
    }
}

fn field_type_opt_str(field_type: Option<&v2::FieldType>) -> String {
    field_type
        .map(field_type_str)
        .unwrap_or_else(|| "()".to_string())
}

fn field_type_str(field_type: &v2::FieldType) -> String {
    use v2::field_type::Kind;
    let mut rendered = match &field_type.kind {
        Some(Kind::Named(name)) => name.clone(),
        Some(Kind::Primitive(primitive)) => primitive_name(*primitive).to_string(),
        Some(Kind::InlineScalar(_)) => "<inline scalar>".to_string(),
        Some(Kind::Tuple(tuple)) => {
            let fields: Vec<String> = tuple
                .fields
                .iter()
                .map(|field| {
                    format!(
                        "{}: {}",
                        field.name,
                        field_type_opt_str(field.r#type.as_ref())
                    )
                })
                .collect();
            format!("({})", fields.join(", "))
        }
        Some(Kind::Array(array)) => format!(
            "[{}; {}..{}]",
            field_type_opt_str(array.element.as_deref()),
            array.min,
            array.max
        ),
        Some(Kind::Map(map)) => format!(
            "{{{}: {}}}",
            field_type_opt_str(map.key.as_deref()),
            field_type_opt_str(map.value.as_deref())
        ),
        Some(Kind::Stream(stream)) => match &stream.element {
            Some(v2::stream_type::Element::Named(name)) => format!("<{name}>"),
            Some(v2::stream_type::Element::Primitive(primitive)) => {
                format!("<{}>", primitive_name(*primitive))
            }
            None => "<>".to_string(),
        },
        None => "?".to_string(),
    };
    if field_type.optional {
        rendered.push('?');
    }
    rendered
}

fn primitive_name(primitive: i32) -> &'static str {
    match v2::PrimitiveType::try_from(primitive) {
        Ok(v2::PrimitiveType::Boolean) => "boolean",
        Ok(v2::PrimitiveType::Integer) => "integer",
        Ok(v2::PrimitiveType::Float) => "float",
        Ok(v2::PrimitiveType::String) => "string",
        Ok(v2::PrimitiveType::Bytes) => "bytes",
        _ => "unspecified",
    }
}

fn const_str(def: &v2::ConstDef) -> String {
    if let Some(regex) = &def.regex {
        return format!("regex {regex}");
    }
    match &def.type_ref {
        Some(type_ref) => format!("{}: {}", type_ref, def.value),
        None => def.value.clone(),
    }
}

fn type_repr_str(def: &v2::TypeDef) -> String {
    let backing = match def
        .backing
        .as_ref()
        .and_then(|backing| backing.kind.as_ref())
    {
        Some(v2::backing::Kind::Primitive(primitive)) => primitive_name(*primitive).to_string(),
        Some(v2::backing::Kind::Unit(unit)) => unit.clone(),
        None => "?".to_string(),
    };
    match &def.width {
        Some(v2::type_def::Width::IntWidth(width)) => {
            format!("{backing} {}", int_width_name(*width))
        }
        Some(v2::type_def::Width::FloatWidth(width)) => {
            format!("{backing} {}", float_width_name(*width))
        }
        None => backing,
    }
}

fn int_width_name(width: i32) -> &'static str {
    match v2::IntWidth::try_from(width) {
        Ok(v2::IntWidth::U8) => "u8",
        Ok(v2::IntWidth::I8) => "i8",
        Ok(v2::IntWidth::U16) => "u16",
        Ok(v2::IntWidth::I16) => "i16",
        Ok(v2::IntWidth::U32) => "u32",
        Ok(v2::IntWidth::I32) => "i32",
        Ok(v2::IntWidth::U64) => "u64",
        Ok(v2::IntWidth::I64) => "i64",
        _ => "unspecified",
    }
}

fn float_width_name(width: i32) -> &'static str {
    match v2::FloatWidth::try_from(width) {
        Ok(v2::FloatWidth::F32) => "f32",
        Ok(v2::FloatWidth::F64) => "f64",
        _ => "unspecified",
    }
}

fn constraint_str(constraint: Option<&v2::Constraint>) -> String {
    let Some(constraint) = constraint else {
        return "(none)".to_string();
    };
    let mut parts = Vec::new();
    if let Some(min) = &constraint.min {
        parts.push(format!("min={min}"));
    }
    if let Some(max) = &constraint.max {
        parts.push(format!("max={max}"));
    }
    if let Some(step) = &constraint.step {
        parts.push(format!("step={step}"));
    }
    if let Some(len_min) = constraint.len_min {
        parts.push(format!("len_min={len_min}"));
    }
    if let Some(len_max) = constraint.len_max {
        parts.push(format!("len_max={len_max}"));
    }
    if let Some(pattern) = &constraint.pattern {
        parts.push(format!("pattern={pattern}"));
    }
    format!("[{}]", parts.join(" "))
}

fn type_init_str(def: &v2::TypeDef) -> String {
    if let Some(declared) = &def.declared_init {
        return declared.clone();
    }
    match def.init.as_ref().and_then(|init| init.value.as_ref()) {
        Some(value) => value.clone(),
        None => "(none)".to_string(),
    }
}

fn signal_init_str(def: &v2::SignalDef) -> String {
    if let Some(declared) = &def.declared_init {
        return declared.clone();
    }
    match def.init.as_ref().and_then(|init| init.value.as_ref()) {
        Some(value) => value.clone(),
        None => "(none)".to_string(),
    }
}
