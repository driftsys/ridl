//! The FlatBuffers projection facts, as ADR-0019 fixes them.
//!
//! Two emitters have to agree on these byte for byte: the `.fbs` schema
//! `ridl-backend-flatbuffers` writes, and the payload codec
//! `ridl-backend-rust` writes (roadmap story E11.7). They share no emission
//! code — one writes a schema, the other writes Rust — so what they share is
//! this module, and a drift test asserts that what each of them puts on the
//! wire is what the other one reads.
//!
//! Three kinds of fact live here:
//!
//! - **the tables and their field slots** ([`TableLayout`]) — which id each
//!   struct field, tuple position, map-entry position and union slot takes;
//! - **a union arm's discriminant** ([`union_arm_discriminant`]);
//! - **the size bound** ([`max_size`]) — the largest buffer any legal value of
//!   a type can encode to, which the codec emits as `MAX_SIZE` and the catalog
//!   descriptor advertises as `EncodedSizes.flatbuffers`.
//!
//! Nothing here names a FlatBuffers *type* or writes a line of schema text.
//! The spelling of a type stays with the emitter that spells it; what is
//! shared is the layout both emitters must produce.

use std::collections::HashMap;

use crate::v2;

/// A fact this module cannot derive from the IR it was handed.
///
/// Every variant is malformed IR rather than a source a user can write — the
/// checker refuses each of them before an emitter is reached — so the message
/// is written for whoever handed the IR over directly, not for a typl author.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionError {
    pub message: String,
}

/// One FlatBuffers table's field slots, in the order they are declared.
///
/// A table is the only shape ADR-0019 decision 3 projects a composite to — a
/// FlatBuffers `struct` is never emitted, whatever `StructDef.fixed_layout`
/// says — so this one type describes every table in the projection: a struct's
/// own, a tuple's positional table, a map's entry table, a union's wrapper and
/// a union arm's box.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableLayout {
    /// The slots, in declaration order. Ids need not be contiguous from the
    /// front of this list only because a union wrapper's discriminant slot is
    /// implicit ([`union_wrapper_table`]).
    pub slots: Vec<FieldSlot>,
}

impl TableLayout {
    /// The number of vtable entries the table needs: one per id up to and
    /// including the highest one used, which is what a FlatBuffers vtable
    /// covers. A table with no slot at all needs none.
    #[must_use]
    pub fn vtable_slots(&self) -> u64 {
        self.slots
            .iter()
            .map(|slot| u64::from(slot.id) + 1)
            .max()
            .unwrap_or(0)
    }
}

/// One field slot of a table: the FlatBuffers `id` it takes, and what put it
/// there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldSlot {
    /// The FlatBuffers `id`. Ids start at 0; typl ordinals start at 1
    /// (typl §7.4).
    pub id: u32,
    pub source: SlotSource,
}

/// What claimed a slot. The emitter needs this to name the field; the drift
/// test needs it to say which field disagreed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlotSource {
    /// A live struct field, by its typl name and 1-based ordinal.
    Field { name: String, ordinal: u32 },
    /// A retired struct ordinal. typl §7.4 is what makes a tombstone hold
    /// its ordinal; holding it with a `deprecated` placeholder field, so a
    /// later id never moves onto it, is this projection's way of doing that
    /// and not a rule ADR-0019 states.
    Retired { ordinal: u32 },
    /// A tuple field, by its 1-based position (typl §11).
    TupleField { position: u32 },
    /// A map entry's key (typl §12.2).
    Key,
    /// A map entry's value.
    Value,
    /// The value slot of a union's wrapper table, or of a union arm's box.
    Wrapped,
}

/// The layout of the table one struct projects to: one slot per member, live
/// or retired, its id the typl ordinal minus one (typl §7.4).
///
/// Ordinal 0 is refused rather than subtracted with a wrapping or panicking
/// underflow, and so is an ordinal used twice. typl assigns neither, so both
/// are totality over IR handed in directly rather than cases reachable
/// through the compiler.
///
/// The repeat matters beyond the schema `flatc` would refuse: [`max_size`]
/// charges one alignment event per vtable slot, and that dominates the real
/// per-field padding only while the fields and the slots are one to one. Two
/// fields sharing an ordinal would be charged one slot's slack and pay two
/// fields' padding.
pub fn struct_table(owner: &str, def: &v2::StructDef) -> Result<TableLayout, ProjectionError> {
    let mut slots: Vec<FieldSlot> = Vec::with_capacity(def.members.len());
    for member in &def.members {
        match &member.member {
            Some(v2::struct_member::Member::Field(field)) => slots.push(FieldSlot {
                id: slot_id(owner, field.ordinal)?,
                source: SlotSource::Field {
                    name: field.name.clone(),
                    ordinal: field.ordinal,
                },
            }),
            Some(v2::struct_member::Member::Reserved(reserved)) => slots.push(FieldSlot {
                id: slot_id(owner, reserved.ordinal)?,
                source: SlotSource::Retired {
                    ordinal: reserved.ordinal,
                },
            }),
            None => {}
        }
        if let Some(last) = slots.last()
            && slots[..slots.len() - 1]
                .iter()
                .any(|slot| slot.id == last.id)
        {
            return Err(ProjectionError {
                message: format!(
                    "`{owner}` carries two struct members with ordinal {}, which FlatBuffers \
                     cannot represent — one id names one field (typl §7.4).",
                    last.id + 1
                ),
            });
        }
    }
    Ok(TableLayout { slots })
}

/// The layout of the positional table a tuple induces: `field_1`, `field_2`, …
/// with ids from 0. A tuple field is always named in typl source (typl §11),
/// but positional access is what a tuple offers, so the position is what
/// reaches the wire.
pub fn tuple_table(name: &str, tuple: &v2::TupleType) -> Result<TableLayout, ProjectionError> {
    let mut slots = Vec::with_capacity(tuple.fields.len());
    for index in 0..tuple.fields.len() {
        // The position is taken before the id, so the last id a FlatBuffers
        // field can carry is refused rather than wrapped to `field_0`.
        let position = u32::try_from(index + 1).map_err(|_| ProjectionError {
            message: format!("{name} has more tuple fields than a FlatBuffers id can carry."),
        })?;
        slots.push(FieldSlot {
            id: position - 1,
            source: SlotSource::TupleField { position },
        });
    }
    Ok(TableLayout { slots })
}

/// The id a map entry's `key` takes.
pub const MAP_ENTRY_KEY_ID: u32 = 0;

/// The id a map entry's `value` takes.
pub const MAP_ENTRY_VALUE_ID: u32 = 1;

/// The id the value field of a union's wrapper table takes — **1**, because
/// the implicit `_type` discriminant takes 0 (ADR-0019 decision 1).
pub const UNION_WRAPPER_VALUE_ID: u32 = 1;

/// The id the value field of a union arm's box table takes
/// (ADR-0019 decision 2).
pub const UNION_ARM_BOX_VALUE_ID: u32 = 0;

/// The layout of the entry table a map induces: `key` at
/// [`MAP_ENTRY_KEY_ID`], `value` at [`MAP_ENTRY_VALUE_ID`]. FlatBuffers has
/// no map type, so a map is a vector of these (typl §12.2), and ADR-0019
/// decision 4 emits no `(key)` attribute on it — which is why the entry table
/// is an ordinary two-field table here and carries no sort obligation.
#[must_use]
pub fn map_entry_table() -> TableLayout {
    TableLayout {
        slots: vec![
            FieldSlot {
                id: MAP_ENTRY_KEY_ID,
                source: SlotSource::Key,
            },
            FieldSlot {
                id: MAP_ENTRY_VALUE_ID,
                source: SlotSource::Value,
            },
        ],
    }
}

/// The layout of the wrapper table a union sits in (ADR-0019 decision 1): the
/// value at **id 1**, not 0.
///
/// A FlatBuffers union field owns two id slots — the hidden `_type`
/// discriminant at `N - 1` and the value at `N` — so the wrapper's value field
/// is declared at id 1 and the discriminant takes id 0 implicitly. The
/// discriminant is not listed in [`TableLayout::slots`] because no emitter
/// writes it as a field; it is charged as a vtable slot and an inline byte by
/// [`max_size`], which is the only place its cost matters.
#[must_use]
pub fn union_wrapper_table() -> TableLayout {
    TableLayout {
        slots: vec![FieldSlot {
            id: UNION_WRAPPER_VALUE_ID,
            source: SlotSource::Wrapped,
        }],
    }
}

/// The layout of the box table a non-table union arm is isolated in
/// (ADR-0019 decision 2): one `value` field at [`UNION_ARM_BOX_VALUE_ID`]. A FlatBuffers union
/// member must be a table, and a named scalar, an enum and an enum set each
/// inline to a bare scalar, so each is wrapped rather than refused.
#[must_use]
pub fn union_arm_box_table() -> TableLayout {
    TableLayout {
        slots: vec![FieldSlot {
            id: UNION_ARM_BOX_VALUE_ID,
            source: SlotSource::Wrapped,
        }],
    }
}

/// The discriminant a union arm carries on the wire: `UnionArm.ordinal`, which
/// is 1-based, follows declaration order, and which a tombstone keeps occupied
/// (typl §7.4).
///
/// **The `.fbs` emitter does not write this number.** It renders the arms in
/// declaration order with no explicit values, so the target numbers them by
/// position and the two disagree after a tombstoned retirement — driftsys/ridl#302.
/// This function is what makes that disagreement visible: the drift test
/// compares it against the schema's implicit numbering on every fixture. E11.7
/// does not close #302; the fix is explicit member values in the schema, and
/// #302 records that `planus` 1.3.0 rejects that form.
#[must_use]
pub fn union_arm_discriminant(arm: &v2::UnionArm) -> u32 {
    arm.ordinal
}

/// Whether a table field typed by `def` needs an explicit `= null` default
/// (ADR-0019 decision 6) — true exactly when the enum declares no zero-valued
/// member.
///
/// FlatBuffers gives every table field a default and cannot mark a scalar or
/// an enum field `required`, so `flatc` refuses a field whose implicit default
/// of 0 is not a member of its enum. `= null` is the rendering that never
/// fabricates a reading, which is also why the codec treats such a field's
/// absence as a missing required field rather than as a value.
#[must_use]
pub fn enum_field_needs_null_default(def: &v2::EnumDef) -> bool {
    !def.values.iter().any(|value| value.value == 0)
}

/// The typl ordinal minus one, refused on 0.
fn slot_id(owner: &str, ordinal: u32) -> Result<u32, ProjectionError> {
    ordinal.checked_sub(1).ok_or_else(|| ProjectionError {
        message: format!(
            "`{owner}` carries a struct member with ordinal 0, which FlatBuffers ids cannot \
             represent — typl ordinals start at 1 (typl §7.4)."
        ),
    })
}

// ---------------------------------------------------------------------
// The size bound
// ---------------------------------------------------------------------

/// The package a bound is computed for, plus every other package it might
/// reference — the same bundle the emitters carry, because a bound follows a
/// reference into the package that declares it.
#[derive(Clone, Copy)]
pub struct Packages<'a> {
    pub package: &'a v2::Package,
    pub others: &'a [&'a v2::Package],
}

/// Whether a declaration gets a root table of its own, and so has a buffer to
/// bound. A struct and a union do; a named scalar, an enum, an enum set and a
/// constant inline at each use site and never become a buffer, so asking
/// [`max_size`] about one is a category error rather than an unbounded type.
#[must_use]
pub fn mints_root_table(decl: &v2::Decl) -> bool {
    matches!(
        decl.kind,
        Some(v2::decl::Kind::StructDef(_)) | Some(v2::decl::Kind::UnionDef(_))
    )
}

/// The largest FlatBuffers buffer any legal value of `decl` can encode to, or
/// `None` when no finite bound can be derived.
///
/// This is the one implementation of the bound (design note D-6). The codec
/// emits it as `<T as Payload<FlatBuffers>>::MAX_SIZE`, the face sizes real
/// buffers from that constant, and Epic 16 advertises the same number as
/// `EncodedSizes.flatbuffers`. A second implementation would be a defect no
/// test either story writes could catch: a codec whose buffer is larger than
/// the size the descriptor advertises.
///
/// **It is an upper bound with the slack charged explicitly, not the exact
/// size of the largest value.** Every FlatBuffers cost the projection can
/// incur is charged at its worst case:
///
/// - the root: one `uoffset` plus [`ALIGN_SLACK`];
/// - each table: one `soffset` back to its vtable, its inline fields, one
///   [`ALIGN_SLACK`] **per slot and one more for the `soffset`**, and its own
///   vtable — four header bytes plus two per slot up to the highest id used
///   ([`TableLayout::vtable_slots`]) plus [`ALIGN_SLACK`]. Vtable sharing is
///   never charged, because sharing only ever makes a buffer smaller. The
///   slack is charged per slot rather than once per table so that the bound
///   holds whatever order the encoder writes a table's fields in: a builder
///   writing them in non-increasing alignment order pre-aligns once, but one
///   writing them in declaration order pre-aligns again at every widening,
///   and nothing here obliges the encoder to either;
/// - a string: its length prefix, four bytes per declared character
///   (typl §5.3 bounds a string in characters, and UTF-8 spends up to four
///   bytes on one), the null terminator, and [`ALIGN_SLACK`];
/// - `bytes`: its length prefix, its declared length, and [`ALIGN_SLACK`];
/// - a vector: its length prefix, the maximum element count times the
///   element's inline width, [`ALIGN_SLACK`], and the maximum element count
///   times whatever each element places out of line;
/// - a union: its wrapper table, whose discriminant costs one vtable slot and
///   one inline byte, plus the largest of its arms.
///
/// An optional field is charged as present, since absence only shrinks a
/// buffer.
///
/// `None` is returned when the bound is not finite or cannot be derived:
///
/// - a `string` or a `bytes` with no length bound. The IR handed over by the
///   compiler always carries one — typl §4.4–§4.5 default it to `[0..256]`
///   with TYPL-103, at a map key position as much as anywhere else
///   (driftsys/ridl#459) — so this is totality over IR handed in directly,
///   the same as the cases below it;
/// - a reference that does not resolve in `packages`, or a declaration kind
///   this projection does not carry;
/// - a composite that reaches itself, directly or transitively — TYPL-206
///   rejects one, so this is totality over IR handed in directly;
/// - an arithmetic overflow of `u64`, which is no representable finite
///   bound;
/// - a bound above [`MAX_ENCODABLE`]. Every offset in the format is 32 bits,
///   so a larger buffer is not addressable by FlatBuffers at all, and the
///   constant the codec emits is a `usize` — which on the `wasm32` target
///   ADR-0020 decision 2 makes this encoding's home is itself 32 bits.
///
/// An array or a map whose `max` is zero is charged as no elements rather
/// than refused. TYPL-202 makes both bounds mandatory, so a zero maximum is a
/// container that carries nothing, not a missing bound.
///
/// Ask only about a declaration [`mints_root_table`] accepts; any other kind
/// answers `None` because it has no buffer of its own, not because it is
/// unbounded.
#[must_use]
pub fn max_size(packages: Packages<'_>, decl: &v2::Decl) -> Option<u64> {
    let mut sizer = Sizer {
        packages,
        visiting: Vec::new(),
        computed: HashMap::new(),
    };
    let body = match &decl.kind {
        Some(v2::decl::Kind::StructDef(def)) => {
            sizer.struct_table_bound(packages.package, &decl.name, def)?
        }
        Some(v2::decl::Kind::UnionDef(def)) => {
            sizer.union_wrapper_bound(packages.package, &decl.name, def)?
        }
        _ => return None,
    };
    match ROOT.checked_add(body) {
        Some(bound) if bound <= MAX_ENCODABLE => Some(bound),
        _ => None,
    }
}

/// The largest buffer a FlatBuffers offset can address. Every offset in the
/// format is 32 bits, so nothing above this is encodable, whatever the
/// arithmetic says.
pub const MAX_ENCODABLE: u64 = u32::MAX as u64;

/// The worst-case padding **one alignment event** can cost: FlatBuffers
/// aligns a scalar to its own width, and eight is the widest this projection
/// emits.
///
/// A table incurs several — one per inline field in the worst write order,
/// plus one for its `soffset` — so a table is charged this per slot and once
/// more for itself, never once for the whole table.
pub const ALIGN_SLACK: u64 = 7;

/// A `uoffset_t` or an `soffset_t`: four bytes.
const OFFSET: u64 = 4;

/// A vtable's two `voffset_t` header entries — its own length and the inline
/// size of the table it describes.
const VTABLE_HEADER: u64 = 4;

/// One vtable entry: a `voffset_t`.
const VTABLE_SLOT: u64 = 2;

/// The buffer header: the `uoffset_t` to the root table, plus the padding the
/// whole buffer's alignment can cost.
const ROOT: u64 = OFFSET + ALIGN_SLACK;

/// What one field costs: the bytes it takes inline in its table, and the bytes
/// it places elsewhere in the buffer.
#[derive(Clone, Copy)]
struct Charge {
    inline: u64,
    out_of_line: u64,
}

impl Charge {
    const fn inline_only(inline: u64) -> Self {
        Self {
            inline,
            out_of_line: 0,
        }
    }
}

/// The walk that charges a bound, carrying the packages a reference resolves
/// in and the declarations already on the stack, so a cycle answers `None`
/// instead of recurring forever.
struct Sizer<'a> {
    packages: Packages<'a>,
    visiting: Vec<String>,
    /// The bound of each named composite already derived, so a type reached
    /// twice is walked once. Without it a diamond — `A { b: B, c: B }`,
    /// `B { d: C, e: C }`, and so on — costs time exponential in its depth,
    /// and typl admits one: TYPL-206 rejects a cycle, not sharing.
    ///
    /// Only a finite bound is cached. A `None` may be the cycle guard
    /// answering for the path that reached it rather than a property of the
    /// type, and that answer does not generalize to another path.
    computed: HashMap<String, u64>,
}

impl<'a> Sizer<'a> {
    /// A same-package bare `Name` or a cross-package fully qualified
    /// `pkg.Name`, resolved to its declaration and the package that holds it —
    /// the package a bare reference *inside* that declaration then resolves
    /// against.
    fn resolve(
        &self,
        home: &'a v2::Package,
        reference: &str,
    ) -> Option<(&'a v2::Decl, &'a v2::Package)> {
        match reference.rsplit_once('.') {
            Some((referenced_package, member)) => std::iter::once(self.packages.package)
                .chain(self.packages.others.iter().copied())
                .find(|candidate| candidate.name == referenced_package)
                .and_then(|candidate| {
                    candidate
                        .decls
                        .iter()
                        .find(|decl| decl.name == member)
                        .map(|decl| (decl, candidate))
                }),
            None => home
                .decls
                .iter()
                .find(|decl| decl.name == reference)
                .map(|decl| (decl, home)),
        }
    }

    /// The bound of the named composite `key`: the one already derived if
    /// there is one, `None` when `key` is already on the visiting stack — a
    /// composite that reaches itself — and otherwise whatever `body` derives,
    /// remembered when it is finite.
    fn guarded(&mut self, key: String, body: impl FnOnce(&mut Self) -> Option<u64>) -> Option<u64> {
        if let Some(bound) = self.computed.get(&key) {
            return Some(*bound);
        }
        if self.visiting.contains(&key) {
            return None;
        }
        self.visiting.push(key.clone());
        let out = body(self);
        self.visiting.pop();
        if let Some(bound) = out {
            self.computed.insert(key, bound);
        }
        out
    }

    /// One table: its soffset, its inline fields, one [`ALIGN_SLACK`] per slot
    /// plus one for the soffset, its own vtable, and everything its fields
    /// place out of line.
    fn table_bound(&self, layout: &TableLayout, inline: u64, out_of_line: u64) -> Option<u64> {
        let slots = layout.vtable_slots();
        let vtable = VTABLE_HEADER
            .checked_add(VTABLE_SLOT.checked_mul(slots)?)?
            .checked_add(ALIGN_SLACK)?;
        let padding = ALIGN_SLACK.checked_mul(slots.checked_add(1)?)?;
        OFFSET
            .checked_add(inline)?
            .checked_add(padding)?
            .checked_add(vtable)?
            .checked_add(out_of_line)
    }

    fn struct_table_bound(
        &mut self,
        home: &'a v2::Package,
        owner: &str,
        def: &v2::StructDef,
    ) -> Option<u64> {
        let layout = struct_table(owner, def).ok()?;
        self.guarded(format!("{}.{owner}", home.name), |sizer| {
            let mut inline = 0u64;
            let mut out_of_line = 0u64;
            for member in &def.members {
                let charge = match &member.member {
                    Some(v2::struct_member::Member::Field(field)) => {
                        sizer.field_charge(home, field.r#type.as_ref()?)?
                    }
                    // A retired ordinal holds its slot with a `deprecated`
                    // placeholder. A deprecated field is never written, so its
                    // inline byte is slack charged rather than a cost paid.
                    Some(v2::struct_member::Member::Reserved(_)) => Charge::inline_only(1),
                    None => continue,
                };
                inline = inline.checked_add(charge.inline)?;
                out_of_line = out_of_line.checked_add(charge.out_of_line)?;
            }
            sizer.table_bound(&layout, inline, out_of_line)
        })
    }

    /// A union's wrapper table plus its largest arm. The discriminant is a
    /// vtable slot and an inline byte the wrapper's own layout does not list,
    /// so both are charged here.
    fn union_wrapper_bound(
        &mut self,
        home: &'a v2::Package,
        owner: &str,
        def: &v2::UnionDef,
    ) -> Option<u64> {
        let layout = union_wrapper_table();
        self.guarded(format!("{}.{owner}", home.name), |sizer| {
            let mut largest_arm = 0u64;
            for arm in &def.arms {
                largest_arm = largest_arm.max(sizer.union_arm_bound(home, arm)?);
            }
            // The discriminant: one `ubyte` inline, at the implicit id 0 the
            // wrapper's layout already counts through the value's id of 1.
            sizer.table_bound(&layout, 1 + OFFSET, largest_arm)
        })
    }

    /// One union arm's out-of-line cost: a struct or a union arm is the
    /// referenced table itself; anything else is isolated in a box table
    /// (ADR-0019 decision 2) and pays that table too.
    fn union_arm_bound(&mut self, home: &'a v2::Package, arm: &v2::UnionArm) -> Option<u64> {
        let (decl, declaring) = self.resolve(home, &arm.type_ref)?;
        match &decl.kind {
            Some(v2::decl::Kind::StructDef(def)) => {
                self.struct_table_bound(declaring, &decl.name, def)
            }
            Some(v2::decl::Kind::UnionDef(def)) => {
                self.union_wrapper_bound(declaring, &decl.name, def)
            }
            Some(v2::decl::Kind::TypeDef(_))
            | Some(v2::decl::Kind::EnumDef(_))
            | Some(v2::decl::Kind::EnumSetDef(_)) => {
                let charge = self.named_charge(home, &arm.type_ref)?;
                self.table_bound(&union_arm_box_table(), charge.inline, charge.out_of_line)
            }
            _ => None,
        }
    }

    fn tuple_table_bound(&mut self, home: &'a v2::Package, tuple: &v2::TupleType) -> Option<u64> {
        let layout = tuple_table("", tuple).ok()?;
        let mut inline = 0u64;
        let mut out_of_line = 0u64;
        for field in &tuple.fields {
            let charge = self.field_charge(home, field.r#type.as_ref()?)?;
            inline = inline.checked_add(charge.inline)?;
            out_of_line = out_of_line.checked_add(charge.out_of_line)?;
        }
        self.table_bound(&layout, inline, out_of_line)
    }

    fn map_entry_bound(&mut self, home: &'a v2::Package, map: &v2::MapType) -> Option<u64> {
        let key = self.field_charge(home, map.key.as_ref()?)?;
        let value = self.field_charge(home, map.value.as_ref()?)?;
        self.table_bound(
            &map_entry_table(),
            key.inline.checked_add(value.inline)?,
            key.out_of_line.checked_add(value.out_of_line)?,
        )
    }

    /// One vector: its length prefix, its elements inline, alignment slack,
    /// and whatever each element places out of line.
    fn vector_charge(count: u64, element: Charge) -> Option<Charge> {
        let out_of_line = OFFSET
            .checked_add(count.checked_mul(element.inline)?)?
            .checked_add(ALIGN_SLACK)?
            .checked_add(count.checked_mul(element.out_of_line)?)?;
        Some(Charge {
            inline: OFFSET,
            out_of_line,
        })
    }

    fn field_charge(&mut self, home: &'a v2::Package, ty: &v2::FieldType) -> Option<Charge> {
        match ty.kind.as_ref()? {
            v2::field_type::Kind::Primitive(primitive) => {
                match v2::PrimitiveType::try_from(*primitive).ok()? {
                    v2::PrimitiveType::Boolean => Some(Charge::inline_only(1)),
                    // A bare `integer` or `float` carries no derived width, so
                    // the full typl domain is on the wire: `long` and
                    // `double` (typl §4).
                    v2::PrimitiveType::Integer | v2::PrimitiveType::Float => {
                        Some(Charge::inline_only(8))
                    }
                    // A bare `string` or `bytes` carries no length bound at
                    // all, and there is nothing to charge without one. The
                    // compiler never hands one over: typl §15.3 keeps it out
                    // of a field position (TYPL-208), and a map key gets
                    // §4.4–§4.5's `[0..256]` default as an inline scalar
                    // (driftsys/ridl#459).
                    _ => None,
                }
            }
            v2::field_type::Kind::InlineScalar(td) => self.scalar_charge(td),
            v2::field_type::Kind::Named(reference) => self.named_charge(home, reference),
            v2::field_type::Kind::Array(array) => {
                let element = self.field_charge(home, array.element.as_ref()?)?;
                Self::vector_charge(array.max, element)
            }
            v2::field_type::Kind::Map(map) => {
                let entry = self.map_entry_bound(home, map)?;
                // A map is a vector of entry tables (typl §12.2): each entry
                // costs one offset inline and its whole table out of line.
                Self::vector_charge(
                    map.max,
                    Charge {
                        inline: OFFSET,
                        out_of_line: entry,
                    },
                )
            }
            v2::field_type::Kind::Tuple(tuple) => Some(Charge {
                inline: OFFSET,
                out_of_line: self.tuple_table_bound(home, tuple)?,
            }),
            // A stream is not part of the typl surface this projection
            // carries (ADR-0013 decision 2).
            _ => None,
        }
    }

    fn named_charge(&mut self, home: &'a v2::Package, reference: &str) -> Option<Charge> {
        let (decl, declaring) = self.resolve(home, reference)?;
        match &decl.kind {
            Some(v2::decl::Kind::TypeDef(td)) => self.scalar_charge(td),
            Some(v2::decl::Kind::StructDef(def)) => Some(Charge {
                inline: OFFSET,
                out_of_line: self.struct_table_bound(declaring, &decl.name, def)?,
            }),
            // Every typl enum is emitted at one underlying width, `long`, so
            // that a later value can never widen it under an otherwise
            // compatible edit.
            Some(v2::decl::Kind::EnumDef(_)) => Some(Charge::inline_only(8)),
            Some(v2::decl::Kind::EnumSetDef(esd)) => {
                Some(Charge::inline_only(int_width_bytes(esd.width)?))
            }
            Some(v2::decl::Kind::UnionDef(def)) => Some(Charge {
                inline: OFFSET,
                out_of_line: self.union_wrapper_bound(declaring, &decl.name, def)?,
            }),
            _ => None,
        }
    }

    /// A named or inline scalar, at its declared width (typl Appendix D).
    /// FlatBuffers stores every scalar at its declared byte width, so the
    /// narrow widths are real bytes and are charged as such.
    fn scalar_charge(&self, td: &v2::TypeDef) -> Option<Charge> {
        match &td.width {
            Some(v2::type_def::Width::IntWidth(width)) => {
                Some(Charge::inline_only(int_width_bytes(*width)?))
            }
            Some(v2::type_def::Width::FloatWidth(width)) => {
                match v2::FloatWidth::try_from(*width).ok()? {
                    v2::FloatWidth::F32 => Some(Charge::inline_only(4)),
                    v2::FloatWidth::F64 => Some(Charge::inline_only(8)),
                    // An unspecified float width is the IR's zero value, not a
                    // width this projection may guess at.
                    v2::FloatWidth::Unspecified => None,
                }
            }
            // No width table: a boolean, string or bytes backing. A unit
            // backing implies the float primitive (typl §5.1), so its width is
            // always derived and never reaches here.
            None => match td.backing.as_ref()?.kind.as_ref()? {
                v2::backing::Kind::Primitive(primitive) => {
                    match v2::PrimitiveType::try_from(*primitive).ok()? {
                        v2::PrimitiveType::Boolean => Some(Charge::inline_only(1)),
                        v2::PrimitiveType::Bytes => {
                            let length = td.constraint.as_ref()?.len_max?;
                            Some(Charge {
                                inline: OFFSET,
                                out_of_line: OFFSET
                                    .checked_add(length)?
                                    .checked_add(ALIGN_SLACK)?,
                            })
                        }
                        v2::PrimitiveType::String => {
                            // typl §5.3 bounds a string in characters; UTF-8
                            // spends up to four bytes on one, and FlatBuffers
                            // writes a null terminator past the length prefix.
                            let characters = td.constraint.as_ref()?.len_max?;
                            Some(Charge {
                                inline: OFFSET,
                                out_of_line: OFFSET
                                    .checked_add(characters.checked_mul(4)?)?
                                    .checked_add(1)?
                                    .checked_add(ALIGN_SLACK)?,
                            })
                        }
                        _ => None,
                    }
                }
                _ => None,
            },
        }
    }
}

/// The byte width of one typl integer width (typl Appendix D).
fn int_width_bytes(width: i32) -> Option<u64> {
    match v2::IntWidth::try_from(width).ok()? {
        v2::IntWidth::U8 | v2::IntWidth::I8 => Some(1),
        v2::IntWidth::U16 | v2::IntWidth::I16 => Some(2),
        v2::IntWidth::U32 | v2::IntWidth::I32 => Some(4),
        v2::IntWidth::U64 | v2::IntWidth::I64 => Some(8),
        v2::IntWidth::Unspecified => None,
    }
}

#[cfg(test)]
mod tests;
