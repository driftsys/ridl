//! The claim tables: one per Rust namespace a ridl-derived name reaches
//! (`docs/technotes/rust-backend-name-collisions.md`, rule step 2 and the per-namespace table;
//! driftsys/ridl#449, #453, #455).
//!
//! Two names the backend derives from two ridl names can be equal in one Rust
//! namespace although the ridl names differ: a descriptor `<Interface><Member>`
//! and a declaration (`type CabinTemperature` beside member `temperature` of
//! interface `Cabin`), two members whose `camel_case` agrees (`XY`, `x_y`),
//! two tuple fields whose `snake_case` agrees (`minSpeed`, `min_speed`). rustc
//! rejects the emitted crate (E0124, E0428). The language does not refuse
//! these pairs, because of the in-tree backends only this one applies those
//! transforms in those scopes (ADR-0016, 2026-09-29 amendment), so this
//! backend refuses the package at `ridl build`, with one message that names the
//! generated name and both sources.
//!
//! A name the backend chose is not claimed here. Such a name is changed so
//! that no ridl name reaches it (rule step 1): the internal `__ridl_fb_*`
//! and `__RIDL_*` items start with an underscore, which no ridl identifier
//! does, and the face module's fixed names (`Client`, `Event`, `Serve`, …)
//! end with none of the suffixes a member's call types carry.
//!
//! A table claims only what the backend emits. [`check`] is handed the
//! interfaces whose descriptors and face are emitted, so an interface whose
//! face the pipeline skips (interaction-face decision 2) claims nothing, and a pair of
//! its members is refused by the change that makes its face emittable
//! (decision 13; experiments X-8b and X-8c). The views are the
//! ones the codec emits ([`codec::view_owners`]).
//!
//! The tables run before the codec, so a tuple named like a declaration is
//! refused with both sources named rather than by the FlatBuffers projection,
//! whose message named neither (X-11).

use crate::codec;
use crate::descriptors::{self, declared_name, interactions};
use crate::face::{self, CALL_SUFFIXES};
use crate::{Ctx, GenerateError, camel_of, declared, ident, snake_of, tuple_collision, tuple_name};
use proc_macro2::Ident;
use ridl_ir::codegen::v1;
use std::collections::HashMap;

/// One Rust namespace: the names claimed in it, each with the source that
/// claimed it first.
struct Table {
    /// What the namespace is, as the refusal names it.
    namespace: String,
    claimed: HashMap<String, String>,
}

impl Table {
    fn new(namespace: String) -> Self {
        Table {
            namespace,
            claimed: HashMap::new(),
        }
    }

    /// Claims `name` for `source`, and refuses the package when another
    /// source already claimed it.
    ///
    /// The key is the identifier without the raw prefix, because rustc
    /// treats `r#type` and `type` as one name.
    fn claim(&mut self, name: &Ident, source: String) -> Result<(), GenerateError> {
        let text = name.to_string();
        let key = text.strip_prefix("r#").unwrap_or(&text).to_string();
        if let Some(previous) = self.claimed.get(&key) {
            return Err(GenerateError {
                message: format!(
                    "the generated Rust name `{key}` is claimed twice in {}: once by {previous}, \
                     and again by {source}. One Rust namespace cannot hold two items of one \
                     name, and the backend does not choose between them, so rename one of them \
                     so that their generated names differ.",
                    self.namespace
                ),
            });
        }
        self.claimed.insert(key, source);
        Ok(())
    }
}

/// Refuses the package when two emitted items that are derived from ridl
/// names share one name in one Rust namespace.
///
/// `interfaces` are the interfaces whose descriptors and face this call's
/// entry point emits: none for [`crate::generate`] and
/// [`crate::generate_with`], every named interface for
/// [`crate::generate_face_with`], and the ones not skipped for
/// [`crate::generate_pipeline`].
///
/// The lowering's tuple collision (two tuples of different shapes that one
/// path name reaches, X-7a and X-7b) is reported first, with the message
/// [`tuple_collision`] gave it before this table existed.
pub(crate) fn check(ctx: &Ctx, interfaces: &[&v1::Interface]) -> Result<(), GenerateError> {
    if let Some(collision) = ctx.model.tuple_collisions.first() {
        return Err(tuple_collision(collision));
    }
    let package = ctx.package_name();
    let mut types = Table::new(format!("the type namespace of package `{package}`"));
    let mut values = Table::new(format!("the value namespace of package `{package}`"));

    // Every declaration is a type, except a constant, which is a value. A
    // named scalar and an enum set are tuple structs, whose constructor is a
    // value as well.
    for decl in &ctx.model.declarations {
        let name = declared(decl.name.as_ref());
        let id = ident(name);
        let source = || format!("declaration `{name}`");
        match decl.kind.as_ref() {
            Some(v1::declaration::Kind::Constant(_)) => values.claim(&id, source())?,
            Some(v1::declaration::Kind::Scalar(_) | v1::declaration::Kind::EnumSet(_)) => {
                types.claim(&id, source())?;
                values.claim(&id, source())?;
            }
            Some(
                v1::declaration::Kind::Struct(_)
                | v1::declaration::Kind::Enum(_)
                | v1::declaration::Kind::Union(_),
            ) => types.claim(&id, source())?,
            None => {}
        }
    }

    // An induced tuple is a struct with named fields: a type, and one
    // namespace of fields of its own. An unnamed one is not emitted.
    for tuple in &ctx.model.tuples {
        let name = tuple_name(tuple);
        if name.is_empty() {
            continue;
        }
        let source = tuple_source(ctx, tuple);
        types.claim(&ident(name), source.clone())?;
        let mut fields = Table::new(format!("the fields of {source}, the struct `{name}`"));
        for field in &tuple.fields {
            let declared_field = declared(field.name.as_ref());
            fields.claim(
                &ident(snake_of(field.name.as_ref())),
                format!("field `{declared_field}`"),
            )?;
        }
    }

    let owners = codec::view_owners(ctx);
    for index in owners.declarations {
        if let Some(decl) = ctx.declaration(index) {
            let name = declared(decl.name.as_ref());
            types.claim(
                &codec::view_ident(name),
                format!("the FlatBuffers view of declaration `{name}` (the suffix `FbView`)"),
            )?;
        }
    }
    for index in owners.tuples {
        if let Some(tuple) = ctx.tuple(index) {
            types.claim(
                &codec::view_ident(tuple_name(tuple)),
                format!(
                    "the FlatBuffers view of {} (the suffix `FbView`)",
                    tuple_source(ctx, tuple)
                ),
            )?;
        }
    }

    for interface in interfaces {
        let Some(iface_name) = declared_name(interface) else {
            continue;
        };
        let iface = ident(iface_name);
        // An interface's descriptor and each member's are unit structs: a
        // type and a value.
        let source = || format!("the descriptor of interface `{iface_name}`");
        types.claim(&iface, source())?;
        values.claim(&iface, source())?;
        for (_, interaction) in interactions(interface) {
            let member = declared(interaction.name.as_ref());
            let id = descriptors::descriptor_ident(&iface, interaction);
            let source =
                || format!("the descriptor of member `{member}` of interface `{iface_name}`");
            types.claim(&id, source())?;
            values.claim(&id, source())?;
        }
        if face::emits_module(interface) {
            let module = face::module_ident(interface);
            types.claim(
                &module,
                format!("the face module of interface `{iface_name}`"),
            )?;
            face_module(interface, iface_name, &module)?;
        }
    }
    Ok(())
}

/// The two tables inside one face module: its types that are spelled from a
/// member, and the variants of its `Event` enum.
///
/// Two members that meet in either table have one `camel_case`, so their
/// descriptors `<Interface><Member>` met first in the package's table; these
/// two are kept so that each namespace the face writes a member-derived name
/// into has its table (the per-namespace table, last row).
fn face_module(
    interface: &v1::Interface,
    iface_name: &str,
    module: &Ident,
) -> Result<(), GenerateError> {
    let mut types = Table::new(format!("the type namespace of face module `{module}`"));
    let mut variants = Table::new(format!("the variants of `{module}::Event`"));
    for (_, interaction) in interactions(interface) {
        let member = declared(interaction.name.as_ref());
        let camel = camel_of(interaction.name.as_ref());
        match interaction.shape.as_ref() {
            Some(v1::interaction::Shape::Command(_) | v1::interaction::Shape::Query(_)) => {
                for suffix in CALL_SUFFIXES {
                    types.claim(
                        &face::call_type_ident(camel, suffix),
                        format!(
                            "the `{suffix}` type of member `{member}` of interface `{iface_name}`"
                        ),
                    )?;
                }
            }
            Some(v1::interaction::Shape::Event(_)) => {
                variants.claim(
                    &ident(camel),
                    format!("event member `{member}` of interface `{iface_name}`"),
                )?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// How a refusal names an induced tuple: by the field path that reaches it,
/// `<Declaration>.<field>…`. The lowering names only a tuple reached from a
/// declaration, so a named tuple always has this origin; the struct name is
/// the fallback for a model that says otherwise.
fn tuple_source(ctx: &Ctx, tuple: &v1::InducedTuple) -> String {
    match tuple.origin.as_ref() {
        Some(v1::induced_tuple::Origin::Declaration(path)) => {
            let owner = ctx
                .declaration(path.declaration)
                .map(|decl| declared(decl.name.as_ref()))
                .unwrap_or_default();
            let mut segments = vec![owner.to_string()];
            segments.extend(path.segments.iter().cloned());
            format!("the tuple at field path `{}`", segments.join("."))
        }
        _ => format!("the tuple `{}`", tuple_name(tuple)),
    }
}

#[cfg(test)]
mod tests {
    //! The face module's two tables are unreachable through an entry point:
    //! two members that meet in either one have one `camel_case`, so their
    //! descriptors meet first in the package's table. These tests call the
    //! tables directly over a hand-written model, so that the tables stay
    //! correct for the day an entry point reaches them. The same holds for
    //! an interface descriptor's value claim, for the reason its test gives.

    use super::face_module;
    use crate::face;
    use ridl_ir::codegen::v1;

    fn spellings(declared: &str) -> v1::Spellings {
        v1::Spellings {
            declared: declared.to_string(),
            camel: ridl_ir::name::camel_case(declared),
            snake: ridl_ir::name::snake_case(declared),
            ..Default::default()
        }
    }

    fn slot(ordinal: u32, name: &str, shape: v1::interaction::Shape) -> v1::InteractionSlot {
        v1::InteractionSlot {
            ordinal,
            occupant: Some(v1::interaction_slot::Occupant::Interaction(Box::new(
                v1::Interaction {
                    name: Some(spellings(name)),
                    shape: Some(shape),
                    ..Default::default()
                },
            ))),
        }
    }

    fn interface(slots: Vec<v1::InteractionSlot>) -> v1::Interface {
        v1::Interface {
            identity: Some(v1::interface::Identity::Declared(spellings("Cabin"))),
            slots,
            ..Default::default()
        }
    }

    #[test]
    fn two_calls_equal_under_camel_case_meet_in_the_face_module() {
        let iface = interface(vec![
            slot(1, "XY", v1::interaction::Shape::Command(Default::default())),
            slot(2, "x_y", v1::interaction::Shape::Query(Default::default())),
        ]);
        let module = face::module_ident(&iface);
        let err = face_module(&iface, "Cabin", &module).expect_err("the pair is refused");
        for part in [
            "`XYCall`",
            "face module `cabin`",
            "member `XY`",
            "member `x_y`",
            "rename one of them",
        ] {
            assert!(
                err.message.contains(part),
                "must name {part:?}: {}",
                err.message
            );
        }
    }

    #[test]
    fn two_events_equal_under_camel_case_meet_in_the_event_enum() {
        let iface = interface(vec![
            slot(1, "XY", v1::interaction::Shape::Event(Default::default())),
            slot(2, "x_y", v1::interaction::Shape::Event(Default::default())),
        ]);
        let module = face::module_ident(&iface);
        let err = face_module(&iface, "Cabin", &module).expect_err("the pair is refused");
        for part in [
            "`XY`",
            "`cabin::Event`",
            "event member `XY`",
            "event member `x_y`",
        ] {
            assert!(
                err.message.contains(part),
                "must name {part:?}: {}",
                err.message
            );
        }
    }

    #[test]
    fn a_signal_and_a_fixed_claim_nothing_in_the_face_module() {
        let iface = interface(vec![
            slot(1, "XY", v1::interaction::Shape::Signal(Default::default())),
            slot(2, "x_y", v1::interaction::Shape::Fixed(Default::default())),
        ]);
        let module = face::module_ident(&iface);
        assert!(face_module(&iface, "Cabin", &module).is_ok());
    }

    /// An interface's descriptor is a unit struct, a value as well as a type.
    /// The only name that meets it in the value namespace alone is a constant
    /// of the interface's own name, which TYPL-009 refuses at check time, so
    /// no source reaches this claim; the model here states the pair directly.
    #[test]
    fn an_interface_descriptor_claims_its_value() {
        let model = v1::Model {
            declarations: vec![v1::Declaration {
                name: Some(spellings("Cabin")),
                kind: Some(v1::declaration::Kind::Constant(Default::default())),
                ..Default::default()
            }],
            interfaces: vec![interface(Vec::new())],
            ..Default::default()
        };
        let ctx = crate::Ctx::over(&model);
        let err = super::check(&ctx, &[&model.interfaces[0]]).expect_err("the pair is refused");
        for part in [
            "`Cabin`",
            "value namespace",
            "declaration `Cabin`",
            "the descriptor of interface `Cabin`",
        ] {
            assert!(
                err.message.contains(part),
                "must name {part:?}: {}",
                err.message
            );
        }
    }
}
