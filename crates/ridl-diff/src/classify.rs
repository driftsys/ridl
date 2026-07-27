//! The breaking/compatible classifier (docs/ROADMAP.md epic E2.8b).
//!
//! [`classify`] turns one structural [`Change`] from the walk into a directional
//! [`Verdict`]. Direction is judged from the consumer's side (ADR-0008 decision
//! 14): a change is breaking when it shifts or reuses a wire identity or narrows
//! a consumer-visible guarantee, and compatible when it only relaxes or appends.
//!
//! The classifier reads the resolved IR of both sides, never source, so every
//! bound it compares is the bound a backend sees. That is what makes the
//! `[defaults].timing` rule need no special case: the manifest default is
//! already resolved into every untimed interaction (ADR-0008 decision 12), so
//! editing it arrives here as an ordinary [`Category::TimingChanged`] on each
//! defaulted interaction and classifies by the bound rules below (ridl §9.1).
//!
//! Two properties are deliberate:
//!
//! - **No implication proving.** Contract clauses are carried as canonical
//!   source text (ADR-0008 decision 14), so the classifier compares text. Any
//!   `require` text change is breaking, and any `ensure` text change is
//!   breaking; it never tries to show that one clause implies another.
//! - **Unlisted is breaking.** A shape the table does not name is classified
//!   breaking. A false "breaking" costs a maintainer one review; a false
//!   "compatible" ships a wire break.

use ridl_ir::v2;

use crate::{Category, Change, Verdict};

#[cfg(test)]
mod classify_tests;

/// Classifies one change against the two snapshots it was drawn from.
///
/// `old` and `new` are the matched packages the change belongs to; the change's
/// path is resolved against them to recover the typed IR the direction is read
/// from. For a package present on only one side the caller passes that package
/// as both arguments — those changes classify on their category alone.
// A new variant must be given a real arm here, not swept into a
// catch-all: rustc forces *an* arm, and the arm its `help:` text
// proposes is `_ =>`, which classifies the new variant silently. The
// two lints below reject a wildcard over `Category` — the first when
// it covers several variants, the second when it covers exactly one,
// which is the case a 21st variant creates.
#[deny(
    clippy::wildcard_enum_match_arm,
    clippy::match_wildcard_for_single_variants
)]
pub fn classify(change: &Change, old: &v2::Package, new: &v2::Package) -> Verdict {
    match change.category {
        // Shifts or reuses a wire identity, or replaces a wire-carrying type.
        // Every one of these is breaking in either direction.
        Category::InteractionInserted
        | Category::InteractionReordered
        | Category::InteractionRemoved
        | Category::ReservedNameRedeclared
        | Category::KindChanged
        | Category::PayloadChanged
        | Category::ReturnChanged
        | Category::ParamsChanged
        | Category::WidthChanged
        | Category::ServiceChanged
        | Category::DeclRemoved => Verdict::Breaking,

        // An init is part of the published contract (ridl §9.1): consumers read
        // the pre-publish value. Neither reference states a compatible
        // direction, so the change is breaking.
        Category::InitChanged => Verdict::Breaking,

        // Doc comments, labels, and deprecation notes reach no consumer's build.
        // Visibility is not among them — it has its own category below.
        //
        // A tombstone in the retired interaction's own slot is the sanctioned
        // retirement (ridl §11); the walk only emits `InteractionRetired` when
        // the slot is preserved.
        Category::DocOnly | Category::InteractionRetired => Verdict::Compatible,

        Category::VisibilityChanged => visibility(change, old, new),
        Category::InteractionAppended => appended(change, old, new),
        Category::DeclAdded => added(change, old, new),
        Category::ConstraintChanged => constraint(change, old, new),
        Category::TimingChanged => timing(change, old, new),
        Category::ContractChanged => contract(change, old, new),
    }
}

// ==========================================================================
// Visibility.
// ==========================================================================

/// A change to the visibility a declaration is published at.
///
/// Visibility rides the surface grammar next to doc comments, but it is not
/// metadata to a consumer. `internal` maps to the target's package-private
/// mechanism — Rust `pub(crate)`, a non-exported TypeScript member (ADR-0002
/// §8) — so narrowing `public` to `internal` deletes the declaration from every
/// out-of-package consumer's build. That is the plainest form of "narrows a
/// consumer-visible guarantee" in ADR-0008 decision 14, even though the byte
/// layout on the wire never moves.
///
/// Widening `internal` to `public` only offers more, so it is compatible. Any
/// direction involving an unset visibility is breaking, following the module's
/// unlisted-is-breaking rule.
fn visibility(change: &Change, old: &v2::Package, new: &v2::Package) -> Verdict {
    let (Some(old_visibility), Some(new_visibility)) = (
        find_visibility(old, &change.path),
        find_visibility(new, &change.path),
    ) else {
        return Verdict::Breaking;
    };

    match (
        v2::Visibility::try_from(old_visibility),
        v2::Visibility::try_from(new_visibility),
    ) {
        (Ok(v2::Visibility::Internal), Ok(v2::Visibility::Public)) => Verdict::Compatible,
        _ => Verdict::Breaking,
    }
}

/// The visibility published at a change's path: an interaction inside an
/// interface or service shape, or a package-level declaration, interface, or
/// service.
fn find_visibility(package: &v2::Package, path: &str) -> Option<i32> {
    let mut segments = path.split('/').skip(1);
    let name = segments.next()?;

    if let Some(member) = segments.next() {
        return find_interaction(package, name, member).map(|decl| decl.visibility);
    }
    if let Some(decl) = find_decl(package, name) {
        return Some(decl.visibility);
    }
    // `Package::shapes` answers for a named interface and for a service with an
    // inline shape, and `InterfaceShape::visibility` reads the authoritative
    // field in each case — the owning service's for an inline shape, whose own
    // `Interface.visibility` is `VISIBILITY_UNSPECIFIED` by construction.
    if let Some(shape) = package.shapes().find(|shape| shape.name == name) {
        return Some(shape.visibility());
    }
    // A service naming an interface after `:` carries no shape of its own, so
    // it is not in `shapes()`; its visibility is still the service's.
    package
        .services
        .iter()
        .find(|service| service.name == name)
        .map(|service| service.visibility)
}

// ==========================================================================
// Interaction append — the identity-reuse guard.
// ==========================================================================

/// An interaction (or a freshly minted tombstone) added after every slot that
/// existed before. Appending is compatible, but only when the slot it takes was
/// never occupied: an ordinal freed by an untombstoned removal and handed to a
/// new name **reuses a wire identity**, which ADR-0008 decision 14 lists first
/// among breaking changes.
///
/// The walk labels that case `InteractionAppended` because the new name does sit
/// after every surviving slot; the reuse is only visible by looking back at what
/// the old snapshot held at that ordinal, which is why the check lives here and
/// not in the walk.
fn appended(change: &Change, old: &v2::Package, new: &v2::Package) -> Verdict {
    let Some((container, member)) = member_path(change) else {
        return Verdict::Breaking;
    };
    let (Some(old_iface), Some(new_iface)) = (
        find_interface(old, container),
        find_interface(new, container),
    ) else {
        return Verdict::Breaking;
    };
    let Some(ordinal) = slot_ordinal(new_iface, member) else {
        return Verdict::Breaking;
    };

    for (name, old_ordinal) in slots(old_iface) {
        if old_ordinal == ordinal && name != member {
            return Verdict::Breaking;
        }
    }
    Verdict::Compatible
}

// ==========================================================================
// Additions — package level, and the append-only composite bodies.
// ==========================================================================

/// A declaration present only in the new snapshot.
///
/// A package-level addition — a new decl, interface, or service — is compatible:
/// nothing that existed moved. A composite member addition is compatible only
/// when it is a genuine append: typl §7.4 makes struct fields and union arms one
/// append-only rule ("new fields are added at the end of the struct or union"),
/// and an enum value appends by taking a number above every live and every
/// retired one.
fn added(change: &Change, old: &v2::Package, new: &v2::Package) -> Verdict {
    let Some((container, member)) = member_path(change) else {
        // One or two segments: a whole package, or a package-level decl,
        // interface, or service.
        return Verdict::Compatible;
    };
    let (Some(old_decl), Some(new_decl)) = (find_decl(old, container), find_decl(new, container))
    else {
        return Verdict::Breaking;
    };

    // The member name is not needed: the append test is a property of the whole
    // body, and reading the body is what catches a *surviving* member whose slot
    // moved — which the walk's name-keyed comparison never reports.
    let _ = member;

    use v2::decl::Kind;
    let appended = match (&old_decl.kind, &new_decl.kind) {
        (Some(Kind::StructDef(old_def)), Some(Kind::StructDef(new_def))) => appended_slot(
            &struct_slots(old_def),
            &struct_reserved(old_def),
            &struct_slots(new_def),
        ),
        (Some(Kind::UnionDef(old_def)), Some(Kind::UnionDef(new_def))) => {
            // A result union's arms are its transport identity (ADR-0008
            // decision 4): any arm change flips it.
            !old_def.is_result
                && !new_def.is_result
                && appended_slot(
                    &union_slots(old_def),
                    &union_reserved(old_def),
                    &union_slots(new_def),
                )
        }
        (Some(Kind::EnumDef(old_def)), Some(Kind::EnumDef(new_def))) => appended_slot(
            &value_slots(&old_def.values),
            &reserved_values(&old_def.reserved),
            &value_slots(&new_def.values),
        ),
        (Some(Kind::EnumSetDef(old_def)), Some(Kind::EnumSetDef(new_def))) => appended_slot(
            &value_slots(&old_def.bits),
            &[],
            &value_slots(&new_def.bits),
        ),
        // A shape the table does not name classifies breaking.
        _ => false,
    };

    if appended {
        Verdict::Compatible
    } else {
        Verdict::Breaking
    }
}

/// Whether the new body is the old body with every pre-existing slot untouched
/// and every addition sitting above the highest slot ever used — live or
/// retired.
///
/// Both halves matter. A member whose slot number moved has had its wire
/// identity shifted even though its name survived, and the walk's name-keyed
/// composite comparison cannot see that. A member taking a number at or below
/// the old high-water mark is an insertion, or a reuse of a retired number,
/// which typl §7.4 forbids so a wire value never carries a new meaning.
fn appended_slot(old: &[(String, i64)], old_retired: &[i64], new: &[(String, i64)]) -> bool {
    for (name, old_slot) in old {
        match new.iter().find(|(new_name, _)| new_name == name) {
            Some((_, new_slot)) if new_slot == old_slot => {}
            // A surviving member moved, or vanished alongside the addition.
            _ => return false,
        }
    }

    let high_water = old
        .iter()
        .map(|(_, slot)| *slot)
        .chain(old_retired.iter().copied())
        .max();
    let old_names: Vec<&String> = old.iter().map(|(name, _)| name).collect();
    for (name, slot) in new {
        if old_names.contains(&name) {
            continue;
        }
        match high_water {
            Some(mark) if *slot <= mark => return false,
            _ => {}
        }
    }
    true
}

// ==========================================================================
// Constraints — narrowed versus widened.
// ==========================================================================

/// A scalar constraint change on a named type. Widening keeps every value the
/// old contract admitted legal, so it is compatible; narrowing rejects values a
/// consumer may already be sending.
///
/// A composite body changed in place reaches here with no member path and no
/// rendered values — the walk cannot say what moved inside it — and classifies
/// breaking.
fn constraint(change: &Change, old: &v2::Package, new: &v2::Package) -> Verdict {
    let mut segments = change.path.split('/').skip(1);
    let (Some(name), None) = (segments.next(), segments.next()) else {
        return Verdict::Breaking;
    };
    let (Some(old_decl), Some(new_decl)) = (find_decl(old, name), find_decl(new, name)) else {
        return Verdict::Breaking;
    };

    use v2::decl::Kind;
    let (Some(Kind::TypeDef(old_def)), Some(Kind::TypeDef(new_def))) =
        (&old_decl.kind, &new_decl.kind)
    else {
        return Verdict::Breaking;
    };

    if narrows(old_def.constraint.as_ref(), new_def.constraint.as_ref()) {
        Verdict::Breaking
    } else {
        Verdict::Compatible
    }
}

/// Whether the constraint moved in the narrowing direction on any facet.
///
/// Each facet is judged independently and any narrowing decides the whole
/// change, so a mixed edit — a lowered `min` with a lowered `max` — is breaking
/// on the half that narrows.
fn narrows(old: Option<&v2::Constraint>, new: Option<&v2::Constraint>) -> bool {
    let (Some(old), Some(new)) = (old, new) else {
        // A constraint appearing where there was none bounds a previously
        // unbounded value; dropping one entirely only widens.
        return old.is_none() && new.is_some();
    };

    // A raised floor or a lowered ceiling rejects values that were legal.
    if raised(old.min.as_deref(), new.min.as_deref())
        || lowered(old.max.as_deref(), new.max.as_deref())
    {
        return true;
    }
    // Length bounds are the same rule over character and byte counts.
    if raised_u64(old.len_min, new.len_min) || lowered_u64(old.len_max, new.len_max) {
        return true;
    }
    // A step quantizes: added or changed at all it can exclude values that were
    // legal, and the classifier does not prove divisibility. Removed, it widens.
    if new.step.is_some() && old.step != new.step {
        return true;
    }
    // A pattern added or rewritten can reject strings that matched before.
    // Removed, it widens.
    if (new.pattern.is_some() && old.pattern != new.pattern)
        || (new.pattern_const.is_some() && old.pattern_const != new.pattern_const)
    {
        return true;
    }
    false
}

/// Whether a lower bound was added or moved up.
fn raised(old: Option<&str>, new: Option<&str>) -> bool {
    match (old, new) {
        (None, Some(_)) => true,
        (Some(old), Some(new)) => cmp_decimal(new, old).is_none_or(std::cmp::Ordering::is_gt),
        _ => false,
    }
}

/// Whether an upper bound was added or moved down.
fn lowered(old: Option<&str>, new: Option<&str>) -> bool {
    match (old, new) {
        (None, Some(_)) => true,
        (Some(old), Some(new)) => cmp_decimal(new, old).is_none_or(std::cmp::Ordering::is_lt),
        _ => false,
    }
}

fn raised_u64(old: Option<u64>, new: Option<u64>) -> bool {
    match (old, new) {
        (None, Some(_)) => true,
        (Some(old), Some(new)) => new > old,
        _ => false,
    }
}

fn lowered_u64(old: Option<u64>, new: Option<u64>) -> bool {
    match (old, new) {
        (None, Some(_)) => true,
        (Some(old), Some(new)) => new < old,
        _ => false,
    }
}

/// Orders two canonical decimal strings exactly, without going through a float
/// (the IR carries exact decimals for precisely this reason, ADR-0007 decision
/// 9). Returns `None` when either side is not a plain decimal, which callers
/// read as "cannot prove this relaxes".
///
/// The `None` case is not dead. `ridlc` only ever writes canonical decimals, but
/// [`load_ir_json`](crate::load_ir_json) deserializes a snapshot off disk and a
/// bound is a plain string there, so a hand-edited or foreign `.ir.json` can
/// carry an exponent form or any other spelling this function does not read.
fn cmp_decimal(left: &str, right: &str) -> Option<std::cmp::Ordering> {
    use std::cmp::Ordering;

    let (left_negative, left_digits) = split_sign(left)?;
    let (right_negative, right_digits) = split_sign(right)?;
    if left_negative != right_negative {
        // Zero is signless in canonical form, so a sign disagreement is a real
        // ordering: the negative side is the smaller one.
        return Some(if left_negative {
            Ordering::Less
        } else {
            Ordering::Greater
        });
    }

    let magnitude = cmp_magnitude(&left_digits, &right_digits)?;
    Some(if left_negative {
        magnitude.reverse()
    } else {
        magnitude
    })
}

/// Splits an optional leading sign from a decimal, rejecting anything that is
/// not sign-digits-optional-fraction.
fn split_sign(text: &str) -> Option<(bool, String)> {
    let (negative, rest) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    if rest.is_empty() || !rest.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return None;
    }
    if rest.matches('.').count() > 1 {
        return None;
    }
    Some((negative, rest.to_string()))
}

/// Orders two unsigned decimal magnitudes by integer part then fraction, so
/// `9` < `10` and `1.5` < `1.50001`.
fn cmp_magnitude(left: &str, right: &str) -> Option<std::cmp::Ordering> {
    use std::cmp::Ordering;

    let (left_int, left_frac) = left.split_once('.').unwrap_or((left, ""));
    let (right_int, right_frac) = right.split_once('.').unwrap_or((right, ""));

    let left_int = left_int.trim_start_matches('0');
    let right_int = right_int.trim_start_matches('0');
    let by_length = left_int.len().cmp(&right_int.len());
    if by_length != Ordering::Equal {
        return Some(by_length);
    }
    let by_int = left_int.cmp(right_int);
    if by_int != Ordering::Equal {
        return Some(by_int);
    }

    // Compare fractions digit by digit, padding the shorter with zeros.
    let width = left_frac.len().max(right_frac.len());
    let pad = |frac: &str| format!("{frac:0<width$}");
    Some(pad(left_frac).cmp(&pad(right_frac)))
}

// ==========================================================================
// Timing.
// ==========================================================================

/// A resolved timing change on a signal or event (ADR-0008 decision 12).
///
/// `min` is the rate floor and `max` the staleness bound, so the consumer-facing
/// guarantee strengthens when `min` rises or `max` falls and weakens when either
/// moves the other way. A bound added or removed, or the strict-periodic/range
/// mode flipped, changes what a consumer may assume at all — rmdl clocks key on
/// strict — and is breaking in both directions.
fn timing(change: &Change, old: &v2::Package, new: &v2::Package) -> Verdict {
    let Some((container, member)) = member_path(change) else {
        return Verdict::Breaking;
    };
    let (Some(old_decl), Some(new_decl)) = (
        find_interaction(old, container, member),
        find_interaction(new, container, member),
    ) else {
        return Verdict::Breaking;
    };
    let (Some(old_timing), Some(new_timing)) =
        (interaction_timing(old_decl), interaction_timing(new_decl))
    else {
        // An interaction kind that carries no timing at all. The walk cannot
        // produce this: it emits `TimingChanged` only inside its signal and
        // event arms, so both sides are already a timed kind by the time a
        // change reaches here. It is still reachable, because [`classify`] is
        // public and takes any hand-built `Change` — so the arm stays, follows
        // the module's unlisted-is-breaking rule, and carries a test of its own.
        // It is deliberately not a `debug_assert!`: a caller passing a category
        // the walk would not have emitted is asking a question, not committing
        // a bug, and aborting a debug build over it would be wrong.
        return Verdict::Breaking;
    };

    let (Some(old_timing), Some(new_timing)) = (old_timing, new_timing) else {
        // Timing appearing where there was none, or dropped, is a bound added
        // or removed.
        return Verdict::Breaking;
    };

    if old_timing.mode != new_timing.mode {
        return Verdict::Breaking;
    }
    // A floor lowered, a ceiling raised, or either bound added or removed.
    if lowered(old_timing.min_us.as_deref(), new_timing.min_us.as_deref())
        || dropped(old_timing.min_us.as_deref(), new_timing.min_us.as_deref())
        || raised(old_timing.max_us.as_deref(), new_timing.max_us.as_deref())
        || dropped(old_timing.max_us.as_deref(), new_timing.max_us.as_deref())
    {
        return Verdict::Breaking;
    }
    // What remains is a floor raised, a ceiling lowered, or only
    // `default_applied` flipped over identical bounds — a default made explicit.
    Verdict::Compatible
}

/// Whether a bound present before is absent now.
fn dropped(old: Option<&str>, new: Option<&str>) -> bool {
    old.is_some() && new.is_none()
}

/// The resolved timing of a signal or event; `None` for interaction kinds that
/// carry none, and `Some(None)` for a timed kind with the field unset.
fn interaction_timing(decl: &v2::Decl) -> Option<Option<&v2::Timing>> {
    use v2::decl::Kind;
    match &decl.kind {
        Some(Kind::SignalDef(def)) => Some(def.timing.as_ref()),
        Some(Kind::EventDef(def)) => Some(def.timing.as_ref()),
        _ => None,
    }
}

// ==========================================================================
// Contracts.
// ==========================================================================

/// A `require`/`ensure` clause-set change on a command or query.
///
/// A `require` is a precondition the caller must meet: adding one, or rewriting
/// one, rejects callers that were legal. An `ensure` is a postcondition the
/// caller may rely on: removing one, or rewriting one, withdraws a guarantee.
/// Clause text is compared verbatim — the classifier never tries to prove that
/// one clause implies another (ADR-0008 decision 14), so a rewrite reads as an
/// addition on the `require` side and as a removal on the `ensure` side, and
/// both are breaking.
fn contract(change: &Change, old: &v2::Package, new: &v2::Package) -> Verdict {
    let Some((container, member)) = member_path(change) else {
        return Verdict::Breaking;
    };
    let (Some(old_decl), Some(new_decl)) = (
        find_interaction(old, container, member),
        find_interaction(new, container, member),
    ) else {
        return Verdict::Breaking;
    };
    let (Some(old_clauses), Some(new_clauses)) = (contracts(old_decl), contracts(new_decl)) else {
        return Verdict::Breaking;
    };

    let kind = |want: v2::ContractKind| {
        move |clause: &&v2::Contract| v2::ContractKind::try_from(clause.kind) == Ok(want)
    };
    let sources = |clauses: &[v2::Contract], want: v2::ContractKind| -> Vec<String> {
        let mut out: Vec<String> = clauses
            .iter()
            .filter(kind(want))
            .map(|clause| clause.source.clone())
            .collect();
        out.sort();
        out
    };

    let old_require = sources(old_clauses, v2::ContractKind::Require);
    let new_require = sources(new_clauses, v2::ContractKind::Require);
    let old_ensure = sources(old_clauses, v2::ContractKind::Ensure);
    let new_ensure = sources(new_clauses, v2::ContractKind::Ensure);

    if !covers(&old_require, &new_require) || !covers(&new_ensure, &old_ensure) {
        return Verdict::Breaking;
    }
    Verdict::Compatible
}

/// Whether every clause in `subset` appears in `superset`, counting duplicates.
fn covers(superset: &[String], subset: &[String]) -> bool {
    let mut remaining: Vec<&String> = superset.iter().collect();
    for clause in subset {
        match remaining.iter().position(|held| *held == clause) {
            Some(index) => {
                remaining.swap_remove(index);
            }
            None => return false,
        }
    }
    true
}

/// The contract clauses of a command or query; `None` for kinds that carry none.
fn contracts(decl: &v2::Decl) -> Option<&[v2::Contract]> {
    use v2::decl::Kind;
    match &decl.kind {
        Some(Kind::CommandDef(def)) => Some(&def.contracts),
        Some(Kind::QueryDef(def)) => Some(&def.contracts),
        _ => None,
    }
}

// ==========================================================================
// Path resolution and IR lookups.
// ==========================================================================

/// Splits a `package/container/member` path into its container and member.
/// Returns `None` for the shorter package-level paths.
fn member_path(change: &Change) -> Option<(&str, &str)> {
    let mut segments = change.path.split('/').skip(1);
    let container = segments.next()?;
    let member = segments.next()?;
    if segments.next().is_some() {
        return None;
    }
    Some((container, member))
}

fn find_decl<'a>(package: &'a v2::Package, name: &str) -> Option<&'a v2::Decl> {
    package.decls.iter().find(|decl| decl.name == name)
}

/// The interface a shape name refers to — a top-level interface, or the inline
/// shape of a service, which the walk descends into under the service's own
/// dotted name. `Package::shapes` keys both on exactly that identity, so one
/// lookup covers them.
fn find_interface<'a>(package: &'a v2::Package, name: &str) -> Option<&'a v2::Interface> {
    package
        .shapes()
        .find(|shape| shape.name == name)
        .map(|shape| shape.interface)
}

fn find_interaction<'a>(
    package: &'a v2::Package,
    container: &str,
    member: &str,
) -> Option<&'a v2::Decl> {
    find_interface(package, container)?
        .interactions
        .iter()
        .find(|decl| decl.name == member)
}

/// Every slot of an interface body as (name, ordinal), tombstones included — a
/// tombstone occupies its ordinal exactly so the slot is never reused
/// (ridl §11).
///
/// A tombstone whose name is unset still holds its slot. The interface-body
/// `reserved` form accepts a bare ordinal or a string literal as well as a
/// name, and both lower to `Reserved { name: None }`; dropping those from the
/// slot list would leave their ordinals looking free, and a new interaction
/// taking one would classify as a clean append. The empty name never equals a
/// real interaction name, so the slot is held against every reuse without
/// matching anything.
fn slots(interface: &v2::Interface) -> Vec<(&str, u32)> {
    interface
        .interactions
        .iter()
        .filter_map(|decl| match &decl.kind {
            Some(v2::decl::Kind::ReservedSlot(reserved)) => {
                Some((reserved.name.as_deref().unwrap_or(""), decl.ordinal))
            }
            Some(_) => Some((decl.name.as_str(), decl.ordinal)),
            None => None,
        })
        .collect()
}

fn slot_ordinal(interface: &v2::Interface, member: &str) -> Option<u32> {
    slots(interface)
        .into_iter()
        .find(|(name, _)| *name == member)
        .map(|(_, ordinal)| ordinal)
}

fn struct_slots(def: &v2::StructDef) -> Vec<(String, i64)> {
    def.members
        .iter()
        .filter_map(|member| match &member.member {
            Some(v2::struct_member::Member::Field(field)) => {
                Some((field.name.clone(), i64::from(field.ordinal)))
            }
            _ => None,
        })
        .collect()
}

fn struct_reserved(def: &v2::StructDef) -> Vec<i64> {
    def.members
        .iter()
        .filter_map(|member| match &member.member {
            Some(v2::struct_member::Member::Reserved(reserved)) => {
                Some(i64::from(reserved.ordinal))
            }
            _ => None,
        })
        .collect()
}

fn union_slots(def: &v2::UnionDef) -> Vec<(String, i64)> {
    def.arms
        .iter()
        .map(|arm| (arm.name.clone(), i64::from(arm.ordinal)))
        .collect()
}

fn union_reserved(def: &v2::UnionDef) -> Vec<i64> {
    def.reserved
        .iter()
        .map(|reserved| i64::from(reserved.ordinal))
        .collect()
}

/// Enum values and enum-set bits key on their integer value, not a declaration
/// ordinal: the number is the wire identity (typl §7.4, §8).
fn value_slots(values: &[v2::EnumValue]) -> Vec<(String, i64)> {
    values
        .iter()
        .map(|value| (value.name.clone(), value.value))
        .collect()
}

fn reserved_values(reserved: &[v2::Reserved]) -> Vec<i64> {
    reserved.iter().filter_map(|entry| entry.value).collect()
}

// ==========================================================================
// `--explain` — the rule row for one category.
// ==========================================================================

/// Parses the snake_case word a report prints back into its category, so
/// `ridl diff --explain <category>` takes exactly what the report shows.
pub fn category_from_word(word: &str) -> Option<Category> {
    crate::CATEGORIES
        .into_iter()
        .find(|category| crate::category_word(*category) == word)
}

/// The rule row for a category: the classification table of ADR-0008 decision
/// 14 as text. This is the CI-facing documentation of record until the E4 error
/// index publishes it.
// A new variant must be given a real arm here, not swept into a
// catch-all: rustc forces *an* arm, and the arm its `help:` text
// proposes is `_ =>`, which classifies the new variant silently. The
// two lints below reject a wildcard over `Category` — the first when
// it covers several variants, the second when it covers exactly one,
// which is the case a 21st variant creates.
#[deny(
    clippy::wildcard_enum_match_arm,
    clippy::match_wildcard_for_single_variants
)]
pub fn explain(category: Category) -> &'static str {
    match category {
        Category::DeclAdded => concat!(
            "A declaration, interface, or service present only in the new snapshot.\n",
            "  compatible  a new package-level decl, interface, or service; an enum\n",
            "              value appended above every live and retired number; a\n",
            "              struct field or union arm appended at the end of the body\n",
            "              (typl 7.4, append-only)\n",
            "  breaking    a member inserted below the highest slot ever used, a member\n",
            "              taking a retired number, any addition that moves a surviving\n",
            "              member's slot, or any arm of a result union (ADR-0008 d4)"
        ),
        Category::DeclRemoved => concat!(
            "A declaration, interface, or service present only in the old snapshot.\n",
            "  breaking    a removed service, interface, decl, enum value, struct\n",
            "              field, or union arm withdraws something a consumer compiled\n",
            "              against\n",
            "  caveat      a composite member retired the sanctioned way — replaced by\n",
            "              a `reserved` tombstone in its own slot (typl 7.4) — is also\n",
            "              reported breaking today. The body comparison is keyed on\n",
            "              member names and does not read the `reserved` list, so it\n",
            "              cannot yet tell that retirement from a bare deletion. This\n",
            "              errs on the safe side; carried as debt, see the note on\n",
            "              `diff_composite`. The interaction-level tombstone IS\n",
            "              recognised — see interaction_retired"
        ),
        Category::InteractionAppended => concat!(
            "An interaction added after every slot that existed before.\n",
            "  compatible  the slot it takes was never occupied\n",
            "  breaking    the slot was freed by an untombstoned removal and is now\n",
            "              reused by a new name — a reused wire identity (ADR-0008 d14)"
        ),
        Category::InteractionInserted => concat!(
            "An interaction added before the end of the interface body.\n",
            "  breaking    always — every later ordinal shifts, and the ordinal is the\n",
            "              transport identity (ridl 11)"
        ),
        Category::InteractionReordered => concat!(
            "A surviving interaction whose relative order in the body changed.\n",
            "  breaking    always — a reorder shifts wire identities (ridl 11)"
        ),
        Category::InteractionRemoved => concat!(
            "An interaction removed without a `reserved` tombstone holding its slot.\n",
            "  breaking    always — the freed ordinal is reusable, so the wire identity\n",
            "              is no longer permanent (ridl 11)"
        ),
        Category::InteractionRetired => concat!(
            "An interaction retired to a `reserved` tombstone in its own ordinal slot.\n",
            "  compatible  always — the sanctioned retirement: the slot stays occupied\n",
            "              and every later ordinal holds (ridl 11)"
        ),
        Category::KindChanged => concat!(
            "An interaction whose kind changed (signal, event, command, query, fixed).\n",
            "  breaking    any direction — the kind selects the transport shape"
        ),
        Category::PayloadChanged => concat!(
            "A signal, event, or fixed payload type changed.\n",
            "  breaking    any direction, a stream added or removed included"
        ),
        Category::ReturnChanged => concat!(
            "A query return shape changed.\n",
            "  breaking    any direction — an ok-arm change, an error arm added, removed,\n",
            "              or retyped, a stream added or removed, or any other change to\n",
            "              the synthesized inline `T | E` transport identity (ADR-0008 d4:\n",
            "              interface + interaction ordinal + ordered arm types)"
        ),
        Category::ParamsChanged => concat!(
            "A command or query parameter list changed.\n",
            "  breaking    any direction — a parameter added, removed, renamed, retyped,\n",
            "              or a stream added or removed on one"
        ),
        Category::TimingChanged => concat!(
            "A signal or event resolved timing changed (ADR-0008 d12).\n",
            "  compatible  min raised (a higher rate floor) or max lowered (a tighter\n",
            "              staleness bound) with the mode unchanged; default_applied\n",
            "              flipped over identical resolved bounds — a default made\n",
            "              explicit\n",
            "  breaking    min lowered, max raised, a bound added where none was, a bound\n",
            "              removed, or the strict-periodic/range mode flipped\n",
            "  note        editing `[defaults].timing` needs no special rule: diff\n",
            "              compares resolved bounds, so it surfaces here on every\n",
            "              defaulted interaction (ridl 9.1)"
        ),
        Category::ContractChanged => concat!(
            "A command or query require/ensure clause set changed (ridl 13).\n",
            "  compatible  a require removed, or an ensure added\n",
            "  breaking    a require added or its text changed, or an ensure removed or\n",
            "              its text changed. Clause text is compared verbatim: the\n",
            "              classifier does not prove that one clause implies another\n",
            "              (ADR-0008 d14)"
        ),
        Category::WidthChanged => concat!(
            "A derived wire width or scalar backing changed.\n",
            "  breaking    any IntWidth or FloatWidth change, uint64 versus int64\n",
            "              included — the resolved width is part of the contract\n",
            "              (typl 4.2, 5.6)"
        ),
        Category::ConstraintChanged => concat!(
            "A scalar constraint changed, or a composite body changed in place.\n",
            "  compatible  widened — min lowered, max raised, a length bound loosened,\n",
            "              a step removed, a match pattern removed (by literal or by\n",
            "              named constant), or the whole constraint dropped so the\n",
            "              value is unbounded again\n",
            "  breaking    narrowed — min raised, max lowered, a length bound\n",
            "              tightened, a step added or changed (divisibility is never\n",
            "              proved), a match pattern added or rewritten (by literal or\n",
            "              by named constant), or a constraint appearing where there\n",
            "              was none, which bounds a previously unbounded value. A\n",
            "              composite body changed in place is breaking: the walk\n",
            "              cannot say what moved inside it\n",
            "  note        each facet is judged on its own and any one narrowing\n",
            "              decides the change, so a mixed edit is breaking on the half\n",
            "              that narrows. A widening that flips the resolved wire width\n",
            "              is separately reported as width_changed, which is always\n",
            "              breaking (typl 5.6)"
        ),
        Category::InitChanged => concat!(
            "A declared or resolved init value changed.\n",
            "  breaking    any direction — the init is the value a consumer reads before\n",
            "              the first publish, so it is part of the contract (ridl 9.1)"
        ),
        Category::ReservedNameRedeclared => concat!(
            "A name retired by a `reserved` tombstone is a live interaction again.\n",
            "  breaking    always — a retired identity is never reused (ridl 11,\n",
            "              RIDL-401)"
        ),
        Category::ServiceChanged => concat!(
            "A service's published shape or interface reference changed.\n",
            "  breaking    always — a changed interface_ref, or a switch between an\n",
            "              interface reference and an inline shape, republishes a\n",
            "              different contract at the same service name"
        ),
        Category::DocOnly => concat!(
            "Only doc comment, labels, or deprecation metadata changed.\n",
            "  compatible  always — none of it reaches a consumer's build.\n",
            "              Visibility is NOT in this category: see\n",
            "              visibility_changed"
        ),
        Category::VisibilityChanged => concat!(
            "The visibility a declaration is published at changed.\n",
            "  compatible  internal -> public: the declaration is offered to more\n",
            "              consumers than before\n",
            "  breaking    public -> internal: `internal` maps to the target's\n",
            "              package-private mechanism — Rust `pub(crate)`, a\n",
            "              non-exported TypeScript member (ADR-0002 8) — so the\n",
            "              declaration disappears from every out-of-package\n",
            "              consumer's build. The wire layout does not move, but the\n",
            "              consumer-visible guarantee narrows (ADR-0008 d14). Any\n",
            "              direction involving an unset visibility is breaking"
        ),
    }
}
