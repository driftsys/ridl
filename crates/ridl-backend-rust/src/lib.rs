//! IR v2 package to Rust source plus an extern-C header (ADR-0004 section 7,
//! ADR-0007 decision 13).
//!
//! The full E1.12 backend over the typl surface. Each declaration in a
//! [`v2::Package`] is realized twice: once as idiomatic Rust (the language
//! layer of typl reference Appendix D — every integer is `i64`, every float is
//! `f64`) and once, where the C ABI admits it, as an entry in a companion C
//! header. Named scalar types become `#[repr(transparent)]` newtypes so unit
//! safety survives into generated code (typl reference §5.7); composites map to
//! structs, enums, and Rust `enum` unions.
//!
//! Rust source is built as a [`proc_macro2::TokenStream`] with `quote` and
//! formatted with `prettyplease`, never by shelling out to `rustfmt`.
//!
//! Default derivation follows the leaf-recursion rule: an `impl Default` is
//! emitted for a type only when every field it transitively contains is
//! derivable. The IR `InitValue.derivable` flag on a composite-typed field is a
//! one-level flag, so same-package composite references are re-checked by
//! recursion rather than trusted (see the `defaults` module).
//!
//! Derive eligibility uses the same recursion over the transitive closure, in
//! the `derives` module: `Debug`, `Clone` and `PartialEq` on every generated
//! type, `Copy`, `Eq`, `Hash` and the ordering pair where the closure permits,
//! and `Default` never, because it comes from the typl init value instead.

use proc_macro2::{Ident, Span, TokenStream};
use quote::{format_ident, quote};
use ridl_ir::codegen::v1;
use ridl_ir::name::{camel_case, snake_case};
use ridl_ir::projection::flatbuffers as fb_projection;
use ridl_ir::v2;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

mod clauses;
mod codec;
mod contract;
mod defaults;
mod derives;
mod descriptors;
mod face;

pub use contract::{Backend, WIRE_ENCODING_OPTION};

/// The generated artifact for one package: Rust source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Generated {
    pub rust_source: String,
}

/// A failure to generate code from a package.
///
/// Carried as a value so codegen stays total: no stage in the pipeline panics
/// (ADR-0004 section 5). The `compile` driver folds `message` into its
/// diagnostic list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerateError {
    pub message: String,
}

/// Generates the Rust source for `package`: the domain types only. The only
/// runtime paths in its output are the `::ridl_rt::payload::Violation` and
/// `::ridl_rt::payload::Rule` that a named scalar's constructor (typl value
/// objects, Task 3) and an enum's or enum set's `TryFrom<i64>` (Task 5) name;
/// it emits no interaction face.
///
/// The compiler corpus runs this. The pipeline ran it too until E11.14, which
/// gave the pipeline [`generate_pipeline`] — `ridl build --emit rust` calls
/// that, and this output is a subset of it. The face is emitted by
/// [`generate_face`] and [`generate_pipeline`], not from here, for the reason the Lane M plan records ("Where the face is
/// emitted from"): the corpus interfaces carry contract clauses the M3 clause
/// translator must refuse. The plan's second reason, that the corpus proofs
/// passed no `--extern ridl_rt`, no longer holds: every compile proof links
/// the runtime, because a generated named scalar, enum and enum set all name
/// it. A package of pure `enum` and `enumset` declarations, with no named
/// scalar at all, still depends on `ridl-rt`.
///
/// The call is total: it returns [`GenerateError`] rather than panicking. Every
/// emitted identifier is produced through `ident`, which escapes Rust
/// keywords as raw identifiers, so a typl name that happens to be a Rust
/// keyword (for example a field named `override`) is emitted as `r#override`
/// rather than panicking `format_ident!`. As a final guard the assembled token
/// stream is parsed with `syn::parse2`; a parse failure (a codegen bug) surfaces
/// as a `GenerateError` instead of unformatted output.
pub fn generate(package: &v2::Package) -> Result<Generated, GenerateError> {
    let model = ridl_ir::codegen::lower(package, &[]);
    let ctx = Ctx::new(package, &model);
    render(package_items(&ctx, package)?)
}

/// Generates the Rust source for `package`: the domain types [`generate`]
/// emits, plus the interaction-face descriptors over the `ridl-rt` runtime
/// crate (Lane M stage M3).
///
/// This is the companion entry point of the M3 design's "companion entry
/// point, not the pipeline" decision. The descriptors it appends name
/// `::ridl_rt` and carry the translated `require`/`ensure` clause bodies, so it
/// is the only caller of the clause translator, and the only entry point whose
/// output names the runtime outside the domain types' own constructors and
/// conversions. The domain types come from the same call because the
/// checked-in fixture is brought in
/// with a single `include!`: the face names those types, and the orphan rule
/// needs them local to the test crate. The codec comes from the same call for
/// the same reason: the face names `Payload<Wire>` for every payload type, and
/// the implementations that satisfy it are the ones [`generate`] emits.
///
/// Total for the same reason [`generate`] is: it returns [`GenerateError`]
/// rather than panicking, and additionally refuses a contract clause outside
/// the accepted form and a call the M3 restriction cannot represent.
pub fn generate_face(package: &v2::Package) -> Result<Generated, GenerateError> {
    generate_face_with(package, WireEncoding::default())
}

/// [`generate_face`] with the package's wire encoding stated rather than
/// defaulted (design note D-11 of
/// `docs/archive/2026-09-20-flatbuffers-codec-design.md`).
///
/// `generate_face(package)` is
/// `generate_face_with(package, WireEncoding::default())`, which is the
/// relation `ridl-backend-proto` and `ridl-backend-flatbuffers` already give
/// their own `generate_with`. The encoding reaches the output as one alias,
/// `pub type Wire`, emitted once per package and named by every buffer the
/// face sizes and every `Ref` it builds.
///
/// The alias is emitted here and not by [`generate`]: it exists so that the
/// face names one thing rather than repeating an encoding at each of its
/// sites, and a package generated with no face names it nowhere. What
/// [`generate`] emits is unchanged by this entry point's existence.
pub fn generate_face_with(
    package: &v2::Package,
    wire: WireEncoding,
) -> Result<Generated, GenerateError> {
    refuse_wire_collision(package)?;
    let model = ridl_ir::codegen::lower(package, &[]);
    let ctx = Ctx::new(package, &model);
    let mut items = vec![wire_alias(wire)];
    items.extend(package_items(&ctx, package)?);
    items.extend(descriptors::interface_items(&ctx, package)?);
    items.extend(face::interface_items(package)?);
    render(items)
}

/// [`generate`] with the other packages of the same build.
///
/// The relation is the one `ridl-backend-proto` and `ridl-backend-flatbuffers`
/// already give their own `generate_with` (ADR-0017 decision 1):
/// `generate(package)` is `generate_with(package, &[])`.
///
/// `others` is what lets the codec size and encode a cross-package reference.
/// Without it a type reaching another package carries no
/// `Payload<FlatBuffers>` implementation and a note saying so
/// (driftsys/ridl#467); with it, such a type is emitted like any other.
pub fn generate_with(
    package: &v2::Package,
    others: &[&v2::Package],
) -> Result<Generated, GenerateError> {
    let model = ridl_ir::codegen::lower(package, others);
    let ctx = Ctx::with_others(package, others, &model);
    render(package_items(&ctx, package)?)
}

/// The pipeline's entry point: everything [`generate_face_with`] emits, over
/// the whole build, with an interface the face cannot carry skipped rather
/// than refused (E11.14 decisions 1, 2 and 4).
///
/// **Why the pipeline calls this and not [`generate`]** (decision 1): a
/// consumer of `ridl build --emit rust` needs the face as much as the domain
/// types, and the face compiles over the codec in the same unit, so the
/// emitted unit is the superset rather than two calls.
/// [`generate`] stays the entry point for every other caller, and it still
/// emits no face and no descriptors. Its output is not byte-for-byte what it
/// was — this story widened the visibility of `check` and the `__ridl_fb_*`
/// functions, which moved every corpus snapshot — but what it emits is
/// unchanged in kind. ADR-0023 decision 2 carries a dated consequence note
/// (2026-09-21) saying the CLI calls this entry point rather than
/// [`generate`]; the decision itself is not amended.
///
/// **Why an interface is skipped and not refused** (decision 2): the clause
/// translator accepts one narrow form, and a multi-parameter call and a stream
/// have no face at all. Refusing would make `--emit rust` reject legal ridl
/// over a gap two named stories own — E5.1 replaces the translator, and the
/// multi-parameter argument struct is lane M's parked follow-up — and would
/// put a codegen error where a source diagnostic belongs. So the package keeps
/// its domain types and its codec, the interface loses its `Client`,
/// `Publisher`, `Provider` and `dispatch`, and a note at that site names the
/// interface, the reason and the story that removes it.
/// [`generate_face_with`] itself still refuses, for a direct caller.
///
/// **What a skipped interface also loses.** Its descriptors. Every cause
/// decision 2 names is detected in the descriptor emitter, not the face
/// emitter — `single_param_type`, `query_reply_type` and the clause translator
/// all live there and serve both — so an interface whose face cannot be built
/// cannot have its descriptors built either. The rest of the package's
/// descriptors are unaffected; only the skipped interface's are absent, and
/// the note says so.
pub fn generate_pipeline(
    package: &v2::Package,
    wire: WireEncoding,
    others: &[&v2::Package],
) -> Result<Generated, GenerateError> {
    refuse_wire_collision(package)?;
    let model = ridl_ir::codegen::lower(package, others);
    let ctx = Ctx::with_others(package, others, &model);
    let mut items = vec![wire_alias(wire)];
    items.extend(package_items(&ctx, package)?);
    for shape in package.shapes() {
        if shape.service.is_some() {
            continue;
        }
        match faced_interface(&ctx, package, shape.name, shape.interface) {
            Ok(produced) => items.extend(produced),
            Err(err) => items.push(skipped_interface_note(shape.name, shape.interface, &err)),
        }
    }
    render(items)
}

/// The descriptors and the face of one interface, which stand or fall
/// together for the reason [`generate_pipeline`] records.
fn faced_interface(
    ctx: &Ctx,
    package: &v2::Package,
    iface_name: &str,
    interface: &v2::Interface,
) -> Result<Vec<TokenStream>, GenerateError> {
    let mut items = descriptors::one_interface_items(ctx, &package.name, iface_name, interface)?;
    if let Some(module) = face::one_interface(iface_name, interface)? {
        items.push(module);
    }
    Ok(items)
}

/// The note left where an interface's face was skipped (decision 2).
///
/// It is a `const` carrying doc attributes rather than a bare comment, for the
/// reason the codec's withheld note gives: `quote!` emits tokens, and a doc
/// attribute is the only comment that survives `prettyplease`. The name cannot
/// collide with a typl constant — typl §15.1 gives one a SCREAMING_SNAKE name,
/// and no typl name begins with an underscore.
fn skipped_interface_note(
    iface_name: &str,
    interface: &v2::Interface,
    err: &GenerateError,
) -> TokenStream {
    let name = format_ident!("__RIDL_NO_FACE_{}", snake_case(iface_name).to_uppercase());
    let headline = format!(" Interface `{iface_name}` carries no generated interaction face.");
    let reason = format!(" The emitter refused it: {}", err.message);
    let owner = match face_gap(interface, err) {
        FaceGap::CallShape => {
            " A call the face cannot carry — an interaction that does not declare \
             exactly one named parameter, or a query whose reply is not a named \
             type. The induced argument struct that removes the first is lane M's \
             parked multi-parameter follow-up."
        }
        FaceGap::Clause => {
            " A contract clause outside the form the narrow translator accepts — \
             `<subject> <comparison> <numeric literal>`, conjoined with `&&`. \
             Story E5.1 replaces the translator and removes this."
        }
        FaceGap::Other => " No story below owns this one: the reason above is the whole of it.",
    };
    quote! {
        #[doc = #headline]
        ///
        #[doc = #reason]
        ///
        #[doc = #owner]
        ///
        /// Its descriptors are absent for the same reason: the refusal is
        /// raised by the descriptor emitter, which the face is built on. The
        /// rest of this package — its domain types, its codec, and every
        /// other interface — is unaffected, which is why the build succeeded
        /// (E11.14 decision 2).
        #[allow(dead_code)]
        const #name: () = ();
    }
}

/// Which of decision 2's two owners a skipped interface belongs to.
///
/// Decided by the refusal that was actually raised, and only then by reading
/// the interface. Reading the interface alone reports the wrong owner
/// whenever an interface has both gaps: it returns `CallShape` for the first
/// badly shaped call it finds, even when what stopped the build was a clause
/// on another interaction. The corpus's own `veh.cluster.VehicleStatus` is
/// that shape, and its note used to name a reason from the clause translator
/// under an owner line about multi-parameter calls.
///
/// The refusal is matched on [`clauses::CLAUSE_REFUSAL`] rather than on a
/// literal here, so a rewording of the message changes this match with it
/// instead of silently reclassifying.
enum FaceGap {
    /// An interaction the face has no shape for at all.
    CallShape,
    /// What was refused is a clause.
    Clause,
    /// Neither: the refusal is one no owner below claims, so the note names
    /// the reason and no story. A `fixed` whose payload is not a named type
    /// is the case this exists for — naming either owner there would blame a
    /// story that does not remove it.
    Other,
}

fn face_gap(interface: &v2::Interface, err: &GenerateError) -> FaceGap {
    if err.message.starts_with(clauses::CLAUSE_REFUSAL) {
        return FaceGap::Clause;
    }
    for decl in &interface.interactions {
        let params = match decl.kind.as_ref() {
            Some(v2::decl::Kind::CommandDef(command)) => &command.params,
            Some(v2::decl::Kind::QueryDef(query)) => {
                if descriptors::query_reply_type(query, &decl.name).is_err() {
                    return FaceGap::CallShape;
                }
                &query.params
            }
            _ => continue,
        };
        if descriptors::single_param_type(params, &decl.name).is_err() {
            return FaceGap::CallShape;
        }
    }
    // Not a clause by the message, and every call has a face shape: the
    // refusal came from somewhere neither owner claims.
    FaceGap::Other
}

/// The name [`wire_alias`] emits at package scope.
const WIRE_ALIAS: &str = "Wire";

/// Refuses a package that declares an item whose emitted name is the encoding
/// alias's (driftsys/ridl#476).
///
/// `generate_face_with` emits `pub type Wire` at package scope, and a typl
/// declaration named `Wire` emits `pub struct Wire` at the same scope; rustc
/// reports E0428 on the pair, in the consumer's build rather than here. This
/// refuses it where the cause is, naming the declaration.
///
/// **Refusing rather than renaming** is E11.14 decision 5. The alias name is
/// fixed by design note D-11 of the FlatBuffers codec and is named by every
/// record and every consumer that follows it, so renaming it — or escaping the
/// declaration — would move a name many documents state, to spare one package
/// a name it is free to change. The rejected alternative is exactly that
/// rename.
///
/// It is a **build error, not decision 2's per-interface skip**: the collision
/// is a property of the package, not of one interface, so there is no interface
/// to omit that would leave the rest of the package usable.
///
/// [`generate`] is unaffected — it emits no alias, so `Wire` is an ordinary
/// declaration there, and a package built without a face keeps compiling.
fn refuse_wire_collision(package: &v2::Package) -> Result<(), GenerateError> {
    // Both namespaces, because both land at package scope: a declaration is
    // emitted as its own item, and an interface is emitted as
    // `pub struct <Interface>;` by the descriptor emitter. Scanning only the
    // declarations let `interface Wire` through to a rustc E0428 in the
    // emitted source, which is the failure this refusal exists to replace.
    //
    // The interface half walks `shapes()` rather than `interfaces`, which is
    // the complete set of interface bodies (`xtask`'s `shape_walk` guard
    // holds every reader to it), and then skips a service's inline shape for
    // the same reason `descriptors::interface_items` does: no identity struct
    // is emitted for one, so it collides with nothing.
    //
    // No case reaches that skip today — an inline shape's name is the
    // service's own, which rsdl requires to be dotted and lowercase, so it
    // can never be `Wire`. It is here so this walk and the emitter's stay the
    // same shape, not because it changes an outcome.
    let declared = package
        .decls
        .iter()
        .map(|decl| (decl.name.as_str(), "declaration"));
    let shapes = package
        .shapes()
        .filter(|shape| shape.service.is_none())
        .map(|shape| (shape.name, "interface"));
    for (name, kind) in declared.chain(shapes) {
        if name == WIRE_ALIAS {
            return Err(GenerateError {
                message: format!(
                    "`{}.{}` collides with the `{}` encoding alias the interaction \
                     face emits at package scope; rename the {kind}",
                    package.name, name, WIRE_ALIAS
                ),
            });
        }
    }
    Ok(())
}

/// The domain types and the codec over them — what [`generate`] emits, and
/// what a face is appended to.
///
/// There is one codec emitter and one call to it, which is why the face
/// compiles over the codec `generate` emits rather than over one written for
/// it (design note D-11, stage K9b).
fn package_items(ctx: &Ctx, package: &v2::Package) -> Result<Vec<TokenStream>, GenerateError> {
    let tuples = collect_tuples(package)?;
    let mut items = domain_items(ctx);
    items.extend(codec::package_items(ctx, package, &tuples)?);
    Ok(items)
}

/// The payload encoding a generated package's face encodes and verifies over
/// (design note D-11; ADR-0020 decision 1 names the three encodings).
///
/// It is a per-package build-time choice rather than a type parameter on the
/// face: the alternative would put an `E: Encoding` on every descriptor, every
/// provider trait and every caller, and a `where` bound per payload type on
/// every impl, for a choice made once per generated package.
///
/// One variant today. `ridl-backend-rust` emits a `Payload` implementation for
/// one encoding — the FlatBuffers codec of story E11.7 — so naming another
/// here would emit a face over implementations that do not exist. `repr(C)`
/// and proto3 join when E11.12 and E11.8 emit their codecs, which is what
/// `#[non_exhaustive]` says to a caller that matches on this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum WireEncoding {
    /// The FlatBuffers codec of story E11.7, which [`generate`] emits into
    /// every package's own output.
    #[default]
    FlatBuffers,
}

/// The one `pub type Wire` alias a generated package carries.
fn wire_alias(wire: WireEncoding) -> TokenStream {
    let (path, doc) = match wire {
        WireEncoding::FlatBuffers => (
            quote! { ::ridl_rt::encoding::FlatBuffers },
            "The payload encoding this package's generated interaction face \
             encodes and verifies over, and the one the `Payload` \
             implementations below implement: FlatBuffers (ADR-0019, ADR-0020 \
             decision 2). It is named once here rather than repeated at every \
             buffer and every `Ref` the face builds, so the package's \
             encoding is one line to read and one line to change.",
        ),
    };
    quote! {
        #[doc = #doc]
        pub type Wire = #path;
    }
}

/// The domain-type items of `package` — the shared work of both entry points
/// — and the induced tuple structs the walk discovered.
///
/// The tuples travel back out because the FlatBuffers codec needs them:
/// a tuple's generated struct is a FlatBuffers table like any other
/// (ADR-0019 decision 3), and only this walk knows which tuples exist and
/// what each one is named.
///
/// This does not emit the codec. [`package_items`] appends it, for both entry
/// points: the codec is `generate`'s output (design note D-1 as amended), and
/// the face compiles over that same output (D-11, stage K9b).
/// The domain-type items of `package`, emitted from the lowered model.
///
/// Stage P4 layer 1: every declaration is read from `Ctx::model`, not from
/// the IR. `model.declarations[i]` is lowered from `package.decls[i]`, so the
/// order and the count are the IR's; the induced tuple structs come from
/// `model.tuples`, which the lowering discovers in the same worklist order
/// this walk used to and names with `InducedName.rust`.
///
/// This does not emit the codec. [`package_items`] appends it, for both entry
/// points: the codec is `generate`'s output (design note D-1 as amended), and
/// the face compiles over that same output (D-11, stage K9b).
fn domain_items(ctx: &Ctx) -> Vec<TokenStream> {
    let mut items: Vec<TokenStream> = Vec::new();
    for decl in &ctx.model.declarations {
        items.push(emit_decl(ctx, decl));
    }
    // A tuple type generates a named nested struct each (typl §11). The
    // lowering lifted every one of them out of the type graph and gave each
    // the name the path that reached it spells; an unnamed entry is one no
    // backend has a rule for — a tuple reached from an interaction position,
    // or from a declaration of another package — and this backend emits none.
    for tuple in &ctx.model.tuples {
        if tuple_name(tuple).is_empty() {
            continue;
        }
        items.push(emit_tuple_struct(ctx, tuple));
    }
    items
}

/// The induced tuple set the FlatBuffers codec needs, discovered over the IR.
///
/// Stage P4 layer 1 ports the domain-type emitters onto the model; the codec
/// is layer 2 and still reads `v2::TupleType`, so the worklist that finds
/// those tuples stays here until then. It is the walk [`domain_items`] used to
/// make, with the emitted tokens discarded: the same discovery order, the same
/// one-name-one-struct rule, and the same refusal when two different tuples
/// spell one name ([`tuple_collision`]).
fn collect_tuples(package: &v2::Package) -> Result<Vec<InducedTuple>, GenerateError> {
    let mut tuples: Vec<InducedTuple> = Vec::new();
    let mut discovered: Vec<InducedTuple> = Vec::new();

    for decl in &package.decls {
        let Some(v2::decl::Kind::StructDef(sd)) = &decl.kind else {
            // Only a struct field carries a tuple position; a union arm and an
            // enum member name a type, and a constant has none.
            continue;
        };
        for member in &sd.members {
            let Some(v2::struct_member::Member::Field(field)) = &member.member else {
                continue;
            };
            let Some(ft) = field.r#type.as_ref() else {
                continue;
            };
            let hint = format!("{}{}", camel_case(&decl.name), camel_case(&field.name));
            field_type_tokens(ft, &hint, decl.visibility, &mut tuples);
        }
    }

    let mut seen: HashMap<String, InducedTuple> = HashMap::new();
    let mut index = 0;
    while index < tuples.len() {
        let induced = tuples[index].clone();
        index += 1;
        if let Some(previous) = seen.get(&induced.name) {
            if previous.tuple != induced.tuple || previous.visibility != induced.visibility {
                return Err(tuple_collision(previous, &induced));
            }
            continue;
        }
        seen.insert(induced.name.clone(), induced.clone());
        for field in &induced.tuple.fields {
            let Some(ft) = field.r#type.as_ref() else {
                continue;
            };
            let hint = format!("{}{}", induced.name, camel_case(&field.name));
            field_type_tokens(ft, &hint, induced.visibility, &mut tuples);
        }
        discovered.push(induced);
    }

    Ok(discovered)
}

// ---------------------------------------------------------------------------
// The FlatBuffers size bound's refusal (design note D-7, stages K4 and K5).
// ---------------------------------------------------------------------------

/// Refuses one declaration with no finite FlatBuffers bound (design note
/// D-7 of `docs/archive/2026-09-20-flatbuffers-codec-design.md`; §4a of that note
/// records what stage K4 built and what stage K5 closed).
///
/// **Called per type, by the codec emitter**, at the point where it is about
/// to emit that type's `Payload<FlatBuffers>` implementation — which is what
/// D-7's own wording asks for: the emitter writes no implementation *for that
/// type*. It is never a package-wide gate; a package-wide refusal would
/// withhold every type's domain code over one type's unbounded codec, which
/// D-7 does not authorise.
///
/// `Ok(())` therefore means one of two different things, and the caller
/// already knows which: the type has a bound and its codec is emitted, or the
/// cause of its missing bound is one this backend cannot judge — a
/// cross-package reference it does not resolve, or a same-package cycle — and
/// that one type simply carries no codec.
///
/// The refusal names the member wherever [`unbounded_member`] can name one,
/// and says which of the other three causes it found otherwise.
pub(crate) fn check_flatbuffers_bound(
    ctx: &Ctx,
    package: &v2::Package,
    decl: &v2::Decl,
) -> Result<(), GenerateError> {
    if fb_projection::root_table(decl).is_none() {
        return Ok(());
    }
    let packages = fb_projection::Packages {
        package,
        others: &[],
    };
    if fb_projection::max_size(packages, decl).is_some() {
        return Ok(());
    }
    let pkg = &package.name;
    let name = &decl.name;
    match unbounded_member(ctx, package, decl) {
        Attribution::Member(member) => Err(GenerateError {
            message: format!("`{pkg}.{name}.{member}` has no finite FlatBuffers bound"),
        }),
        Attribution::Untyped(member) => Err(GenerateError {
            message: format!(
                "`{pkg}.{name}.{member}` carries no type, so `{pkg}.{name}` has no FlatBuffers \
                 bound"
            ),
        }),
        Attribution::Layout(message) => Err(GenerateError {
            message: format!("`{pkg}.{name}` has no FlatBuffers table layout: {message}"),
        }),
        Attribution::Aggregate => Err(GenerateError {
            message: format!(
                "`{pkg}.{name}` has no finite FlatBuffers bound: every member is bounded on its \
                 own and the total is not"
            ),
        }),
        Attribution::Exempt => Ok(()),
    }
}

/// What [`check_flatbuffers_bound`] found when `decl`'s own
/// [`fb_projection::max_size`] answered `None`.
///
/// Stage K5 split what stage K4 called `Declaration` into three, closing the
/// second gap §4a of the design note carried forward: the three causes that
/// one variant covered are now told apart, and each writes its own message.
enum Attribution {
    /// One member — a struct field's name, a union arm's name, or the
    /// `value` field of a box root (ADR-0019 decision 8) — is individually
    /// unbounded.
    Member(String),
    /// One member carries no type at all — a struct field with no type, or
    /// the `value` of a box root whose declaration names no backing — which
    /// is malformed IR rather than an unbounded shape, and which no probe can
    /// charge.
    Untyped(String),
    /// `fb_projection::struct_table` refused the declaration's layout — two
    /// members sharing one ordinal, or an ordinal of 0. It is a property of
    /// the whole declaration and no single-member probe reproduces it, so the
    /// projection's own message is carried through.
    Layout(String),
    /// Every member is bounded on its own and the total is not: the summed
    /// size overflows `u64`, or it exceeds
    /// [`fb_projection::MAX_ENCODABLE`]. A member this backend cannot judge
    /// does not shield the sum: it is charged as a `boolean` and the total
    /// is over the ceiling even so.
    Aggregate,
    /// Every member that could be judged is individually bounded, at least
    /// one could not be judged — a cross-package reference this backend does
    /// not resolve, or a same-package cycle — and the struct as a whole still
    /// fits with each unjudged leaf charged as a `boolean`. `decl`'s own
    /// `None` is explained by that member, not by an unbounded shape.
    Exempt,
}

/// What one type position contributes to the attribution.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Verdict {
    /// It has a finite bound of its own.
    Bounded,
    /// It has none, and this backend can say so.
    Unbounded,
    /// This backend cannot say: the position reaches a cross-package
    /// reference or a cycle.
    Unjudgeable,
}

/// Attributes `decl`'s unbounded [`fb_projection::max_size`] — see
/// [`Attribution`].
///
/// Each member is judged on its own by [`judge`], never by trusting one
/// member's unboundedness to explain another's. A struct with both a bare
/// unbounded `string` map key and an unrelated cross-package field is refused
/// over the first and exempted from nothing on account of the second.
fn unbounded_member(ctx: &Ctx, package: &v2::Package, decl: &v2::Decl) -> Attribution {
    let mut any_exempt = false;
    match &decl.kind {
        Some(v2::decl::Kind::StructDef(def)) => {
            // The layout is refused for the declaration as a whole, and it is
            // checked first: with two members on one ordinal, every member
            // probes as bounded and only the aggregate answers `None`, which
            // is exactly the confusion §4a asked K5 to end.
            if let Err(err) = fb_projection::struct_table(&decl.name, def) {
                return Attribution::Layout(err.message);
            }
            for member in &def.members {
                let Some(v2::struct_member::Member::Field(field)) = &member.member else {
                    // A reserved tombstone emits no field and charges one
                    // slack byte only (typl §7.4); it is never the cause.
                    continue;
                };
                let Some(ty) = field.r#type.as_ref() else {
                    return Attribution::Untyped(field.name.clone());
                };
                match judge(ctx, package, ty) {
                    Verdict::Unbounded => return Attribution::Member(field.name.clone()),
                    Verdict::Unjudgeable => any_exempt = true,
                    Verdict::Bounded => {}
                }
            }
        }
        Some(v2::decl::Kind::UnionDef(def)) => {
            for arm in &def.arms {
                let ty = v2::FieldType {
                    optional: false,
                    kind: Some(v2::field_type::Kind::Named(arm.type_ref.clone())),
                };
                match judge(ctx, package, &ty) {
                    Verdict::Unbounded => return Attribution::Member(arm.name.clone()),
                    Verdict::Unjudgeable => any_exempt = true,
                    Verdict::Bounded => {}
                }
            }
        }
        // A declaration rooted in a box (ADR-0019 decision 8) has one member:
        // the box's `value` field, holding the declaration itself. It is
        // resolved in the package that declares it, so it is never
        // unjudgeable — only bounded, or unbounded because the IR carries no
        // width or no length bound for it.
        Some(
            v2::decl::Kind::TypeDef(_) | v2::decl::Kind::EnumDef(_) | v2::decl::Kind::EnumSetDef(_),
        ) => {
            // A named scalar with no backing at all is malformed IR rather
            // than an unbounded shape, which is what `Untyped` is for. The
            // front end leaves one behind after a parse error such as
            // `type X:`, so this is the shape such a declaration reaches the
            // backend in — not one a length or a width would fix.
            if let Some(v2::decl::Kind::TypeDef(td)) = &decl.kind
                && td.backing.is_none()
            {
                return Attribution::Untyped("value".to_string());
            }
            let ty = v2::FieldType {
                optional: false,
                kind: Some(v2::field_type::Kind::Named(decl.name.clone())),
            };
            match judge(ctx, package, &ty) {
                Verdict::Unbounded => return Attribution::Member("value".to_string()),
                Verdict::Unjudgeable => any_exempt = true,
                Verdict::Bounded => {}
            }
        }
        _ => {}
    }
    if !any_exempt {
        return Attribution::Aggregate;
    }
    // Every judged member is bounded and at least one member could not be
    // judged, so the aggregate is still open: two members each under the
    // ceiling can sum over it, and a member this backend cannot judge must
    // not shield that sum. The whole struct is charged once more over
    // `lower_bound_stand_in`'s copy of each member, which charges the
    // unjudged leaves no more than the real ones would, so `None` here is
    // an aggregate cause whatever those leaves turn out to be. A union is
    // its largest arm rather than a sum, so it has no aggregate to charge.
    if let Some(v2::decl::Kind::StructDef(def)) = &decl.kind {
        let stand_in = v2::Decl {
            kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
                members: def
                    .members
                    .iter()
                    .map(|member| match &member.member {
                        Some(v2::struct_member::Member::Field(field)) => v2::StructMember {
                            member: Some(v2::struct_member::Member::Field(v2::Field {
                                r#type: field
                                    .r#type
                                    .as_ref()
                                    .map(|ty| lower_bound_stand_in(ctx, ty)),
                                ..field.clone()
                            })),
                        },
                        _ => member.clone(),
                    })
                    .collect(),
                fixed_layout: def.fixed_layout,
            })),
            ..decl.clone()
        };
        let packages = fb_projection::Packages {
            package,
            others: &[],
        };
        if fb_projection::max_size(packages, &stand_in).is_none() {
            return Attribution::Aggregate;
        }
    }
    Attribution::Exempt
}

/// One type position's verdict.
///
/// A position this backend can resolve in full is probed as a whole, which is
/// the cheapest and the most faithful answer: [`probe_field_type`] charges it
/// exactly what [`fb_projection::max_size`] charges it as one member.
///
/// A position it cannot resolve in full is probed as a whole too, over
/// [`lower_bound_stand_in`]'s copy of it: every leaf this backend cannot
/// judge is replaced by a `boolean`, the smallest thing the projection
/// charges anything for, and what is left is charged once by the same
/// `max_size` the real position would be charged by. That charges every
/// count, every product of nested counts, and every locally known leaf in
/// the position together, which neither a leaf-by-leaf descent nor a
/// level-by-level probe of each count does — `[[veh.other.Speed; 0..2^20];
/// 0..2^20]` is unbounded by the product of its two counts and by nothing
/// else, and `[(veh.other.Speed, [boolean; 0..2^31]); 0..4]` by an outer
/// count over a local inner one. Because each stand-in is a lower bound on
/// the leaf it replaces, a stand-in that is unbounded proves the real
/// position is, and a stand-in that fits says nothing about the real leaves,
/// which is what `Unjudgeable` means.
fn judge(ctx: &Ctx, package: &v2::Package, ty: &v2::FieldType) -> Verdict {
    if member_resolves_locally(ctx, ty) {
        return if probe_field_type(package, ty).is_some() {
            Verdict::Bounded
        } else {
            Verdict::Unbounded
        };
    }
    if probe_field_type(package, &lower_bound_stand_in(ctx, ty)).is_some() {
        Verdict::Unjudgeable
    } else {
        Verdict::Unbounded
    }
}

/// `ty` with every leaf this backend cannot judge replaced by a `boolean`,
/// so that [`fb_projection::max_size`] can charge the rest.
///
/// A `boolean` is one inline byte and nothing out of line, and the
/// projection charges every other leaf at least that: a named scalar its
/// declared width, an enum eight bytes, a struct, a union or a string an
/// offset plus its own table or body. A vector's charge and a table's bound
/// are both monotone in the charge of what they hold, so the stand-in is
/// charged no more than the real position, and `None` over the stand-in is
/// `None` over the real position whatever the unjudged leaves turn out to be.
/// The replaced leaves are the ones [`member_resolves_locally`] answers
/// `false` for: a named reference that does not resolve in this package or
/// reaches a cycle, a stream, and an unspecified primitive. A `FieldType` with
/// no `kind` is kept, so it still probes to `Unbounded` (design note §4b).
fn lower_bound_stand_in(ctx: &Ctx, ty: &v2::FieldType) -> v2::FieldType {
    fn boolean(optional: bool) -> v2::FieldType {
        v2::FieldType {
            optional,
            kind: Some(v2::field_type::Kind::Primitive(
                v2::PrimitiveType::Boolean as i32,
            )),
        }
    }
    let stand_in = |leaf: &v2::FieldType| Box::new(lower_bound_stand_in(ctx, leaf));
    let kind = match ty.kind.as_ref() {
        Some(v2::field_type::Kind::Array(array)) => {
            v2::field_type::Kind::Array(Box::new(v2::ArrayType {
                element: array.element.as_deref().map(stand_in),
                min: array.min,
                max: array.max,
            }))
        }
        Some(v2::field_type::Kind::Map(map)) => v2::field_type::Kind::Map(Box::new(v2::MapType {
            key: map.key.as_deref().map(stand_in),
            value: map.value.as_deref().map(stand_in),
            min: map.min,
            max: map.max,
        })),
        Some(v2::field_type::Kind::Tuple(tuple)) => v2::field_type::Kind::Tuple(v2::TupleType {
            fields: tuple
                .fields
                .iter()
                .map(|field| v2::TupleField {
                    r#type: field.r#type.as_ref().map(|leaf| *stand_in(leaf)),
                    ..field.clone()
                })
                .collect(),
        }),
        _ if member_resolves_locally(ctx, ty) => return ty.clone(),
        // A named reference this backend does not resolve, a cycle, a
        // stream, or an unspecified primitive: the leaf itself is what
        // cannot be judged, and it is replaced.
        _ => return boolean(ty.optional),
    };
    v2::FieldType {
        optional: ty.optional,
        kind: Some(kind),
    }
}

/// The members of `decl` this backend cannot judge — a cross-package
/// reference it does not resolve, a same-package cycle, a stream, or an
/// unspecified primitive.
///
/// [`check_flatbuffers_bound`] answers `Ok(())` for a declaration whose only
/// obstacle is one of these, and the codec emitter then withholds that type's
/// implementation. This is what lets the emitted source say which member is
/// the reason, rather than leaving a consumer with a missing trait
/// implementation and nothing to read.
pub(crate) fn unjudgeable_members(ctx: &Ctx, decl: &v2::Decl) -> Vec<String> {
    let mut names = Vec::new();
    match &decl.kind {
        Some(v2::decl::Kind::StructDef(def)) => {
            for member in &def.members {
                if let Some(v2::struct_member::Member::Field(field)) = &member.member
                    && let Some(ty) = field.r#type.as_ref()
                    && !member_resolves_locally(ctx, ty)
                {
                    names.push(field.name.clone());
                }
            }
        }
        Some(v2::decl::Kind::UnionDef(def)) => {
            for arm in &def.arms {
                if !decl_resolves_locally(ctx, &arm.type_ref, &mut HashSet::new()) {
                    names.push(arm.name.clone());
                }
            }
        }
        _ => {}
    }
    names
}

/// The bound of one type position alone, charged the way
/// [`fb_projection::max_size`] charges it as one member of a struct:
/// `struct_table_bound` sums each member's own `field_charge` independently
/// and propagates the first `None`, so a struct of exactly this one field
/// reproduces the charge the position contributes, with nothing else able to
/// answer `None` in its place.
fn probe_field_type(package: &v2::Package, ty: &v2::FieldType) -> Option<u64> {
    let probe = v2::Decl {
        kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
            members: vec![v2::StructMember {
                member: Some(v2::struct_member::Member::Field(v2::Field {
                    // Ordinal 1 is the first FlatBuffers id. It is written
                    // here rather than copied from the real member so that a
                    // declaration whose ordinals are themselves malformed is
                    // attributed by `Attribution::Layout` and not mistaken
                    // for an unbounded member.
                    ordinal: 1,
                    r#type: Some(ty.clone()),
                    ..Default::default()
                })),
            }],
            fixed_layout: false,
        })),
        ..Default::default()
    };
    let packages = fb_projection::Packages {
        package,
        others: &[],
    };
    fb_projection::max_size(packages, &probe)
}

/// Whether every named type reachable from `ty` resolves inside the package
/// `ctx` indexes — the same shape [`fb_projection::max_size`] would have to
/// resolve to size it.
///
/// Two things answer `false`, "this backend cannot judge this member,
/// exempt it", rather than letting a probe answer for them — both scoped to
/// the one member being checked, never to the whole declaration, so a
/// sibling member with a genuine bound problem is still caught:
///
/// - **a cross-package (dotted) or unknown reference.** `ridl-backend-rust`
///   generates one package at a time and resolves no cross-package reference
///   itself — [`Ctx::lookup`] answers `None` for one by design, the same as
///   every other same-package-only pass in this backend
///   (`derives::type_ref_eligibility`, `defaults`). Handed `others: &[]`,
///   `fb_projection::max_size` cannot tell "this reference does not resolve
///   here" from "this member has no finite bound" — both answer `None` (its
///   own doc, "a reference that does not resolve in `packages`"). This
///   backend already generates a struct across such a reference — the
///   corpus's `ClimateReport`
///   (`crates/ridlc/tests/corpus/veh-cluster/cluster/services.ridl`,
///   `cabin`/`setpoint` typed by the imported `veh.common.Temperature`) is
///   one.
/// - **a same-package composite that reaches itself.** typl rejects one
///   (TYPL-206), so it is IR handed in directly the same as the case above.
///   This backend already has a considered answer for a cyclic struct that
///   is not refusal: `recursive_struct_default_terminates` and
///   `a_cyclic_struct_takes_no_conditional_derives` both pin that a cyclic
///   struct's *domain type* still generates, because Default derivation and
///   the conditional-derive walk both guard the same cycle and degrade to
///   the conservative answer rather than erroring.
///
/// A third case is exempted the same way: **a `Stream` field position.**
/// ridl §12.3 keeps a stream at an interaction position; it never reaches a
/// struct or tuple field in checked IR, `fb_projection::max_size` charges
/// nothing for it, and `flatbuffers_bound_leaves_a_stream_field_alone` pins
/// that this function exempts a member typed by one rather than treating the
/// `None` `field_charge` gives it as a genuine bound failure.
fn member_resolves_locally(ctx: &Ctx, ty: &v2::FieldType) -> bool {
    field_type_resolves_locally(ctx, ty, &mut HashSet::new())
}

/// Whether `reference` and everything its own shape reaches resolves inside
/// the package `ctx` indexes. See [`member_resolves_locally`] for the two
/// cases this answers `false` for, and why each is scoped to one member.
fn decl_resolves_locally(ctx: &Ctx, reference: &str, visiting: &mut HashSet<String>) -> bool {
    let Some(decl) = ctx.lookup(reference) else {
        return false;
    };
    if !visiting.insert(reference.to_string()) {
        // On the path already being walked: a cycle. Left for
        // `unbounded_member` to exempt this one member over (see the doc
        // above).
        return false;
    }
    let resolves = match &decl.kind {
        Some(v2::decl::Kind::StructDef(def)) => {
            def.members.iter().all(|member| match &member.member {
                Some(v2::struct_member::Member::Field(field)) => field
                    .r#type
                    .as_ref()
                    .is_some_and(|ty| field_type_resolves_locally(ctx, ty, visiting)),
                _ => true,
            })
        }
        Some(v2::decl::Kind::UnionDef(def)) => def
            .arms
            .iter()
            .all(|arm| decl_resolves_locally(ctx, &arm.type_ref, visiting)),
        _ => true,
    };
    visiting.remove(reference);
    resolves
}

/// One type position's contribution to [`decl_resolves_locally`], over the
/// same [`v2::FieldType`] shape `fb_projection::max_size` walks to size it.
/// A `Stream` is exempted explicitly — see [`member_resolves_locally`]'s
/// third bullet — rather than falling through to the `None` arm, which is
/// reserved for a `FieldType` this walk genuinely does not recognize.
fn field_type_resolves_locally(
    ctx: &Ctx,
    ty: &v2::FieldType,
    visiting: &mut HashSet<String>,
) -> bool {
    match &ty.kind {
        Some(v2::field_type::Kind::Named(reference)) => {
            decl_resolves_locally(ctx, reference, visiting)
        }
        // An `Unspecified` field primitive emits `()` and is charged
        // nothing, exactly as a `Stream` is, and `derives` lists the two side
        // by side among its refusing positions. It is malformed IR rather
        // than an unbounded shape, so it is exempted rather than refused.
        Some(v2::field_type::Kind::Primitive(primitive)) => v2::PrimitiveType::try_from(*primitive)
            .is_ok_and(|primitive| primitive != v2::PrimitiveType::Unspecified),
        Some(v2::field_type::Kind::InlineScalar(_)) => true,
        Some(v2::field_type::Kind::Tuple(tuple)) => tuple.fields.iter().all(|field| {
            field
                .r#type
                .as_ref()
                .is_some_and(|ty| field_type_resolves_locally(ctx, ty, visiting))
        }),
        Some(v2::field_type::Kind::Array(array)) => array
            .element
            .as_deref()
            .is_some_and(|element| field_type_resolves_locally(ctx, element, visiting)),
        Some(v2::field_type::Kind::Map(map)) => {
            map.key
                .as_deref()
                .is_some_and(|key| field_type_resolves_locally(ctx, key, visiting))
                && map
                    .value
                    .as_deref()
                    .is_some_and(|value| field_type_resolves_locally(ctx, value, visiting))
        }
        Some(v2::field_type::Kind::Stream(_)) => false,
        None => true,
    }
}

/// Parses the assembled items as a bare `syn::File` (no inner attribute, so an
/// `include!` of the output stays legal) and formats them with prettyplease.
fn render(items: Vec<TokenStream>) -> Result<Generated, GenerateError> {
    let tokens = quote! { #(#items)* };
    let file: syn::File = syn::parse2(tokens).map_err(|err| GenerateError {
        message: format!("generated Rust does not parse: {err}"),
    })?;

    Ok(Generated {
        rust_source: prettyplease::unparse(&file),
    })
}

/// One tuple type reached from a declaration, with the visibility that
/// declaration was declared at (typl §11).
///
/// A tuple has no name in source; the struct it generates is named after the
/// path that reached it and is emitted at module scope beside the declaration
/// that induced it. The visibility travels with the discovery for the same
/// reason [`v2::InterfaceShape::visibility`] carries a service's: the value is
/// authoritative at the point of discovery and nowhere else. By the time
/// [`emit_tuple_struct`] runs, the tuple is one entry in a flat worklist and
/// the declaration it came from is out of reach — which is exactly how the
/// struct came to be emitted `pub` over an `internal` declaration's payload
/// (issue #167).
///
/// One name is one struct. The same discovery repeated — the interaction
/// module pre-discovers a nested tuple's name and the drain finds it again —
/// is skipped; a repeat under one name with a different shape or visibility is
/// a collision and is refused ([`tuple_collision`]), because carrying a
/// visibility onto a name two declarations share has no sound answer. See
/// [`generate`].
#[derive(Debug, Clone)]
pub(crate) struct InducedTuple {
    /// The generated struct name — the CamelCase of the path that reached the
    /// tuple.
    pub(crate) name: String,
    pub(crate) tuple: v2::TupleType,
    /// The visibility of the declaration this tuple was reached from: an
    /// `internal struct`'s field, or an `internal interface`'s query return.
    pub(crate) visibility: i32,
}

/// Refuses a package in which two different tuples generate one struct name.
///
/// The name is the CamelCase of the path that reaches the tuple, and nothing
/// upstream keeps two paths from mangling to one string: `struct AB { c : … }`
/// and `struct A { bC : … }` both reach `ABC`, and neither draws a ridl
/// diagnostic. There is no sound way to pick between them, which is why this is
/// a refusal rather than a rule:
///
/// - **Keeping the first** — what the worklist did before — gives the second
///   declaration the *first one's shape*. `ridlc check` exits 0, the module
///   compiles, and the contract is silently wrong. It is also how carrying an
///   inducing declaration's visibility (issue #167) could narrow a struct a
///   public declaration uses, turning a silent wrong shape into a
///   `private_interfaces` build failure.
/// - **Keeping the widest visibility** would publish a package-private type's
///   shape to escape that build failure, which is the defect #167 fixed.
///
/// So neither dedup rule is sound and only rejection is. This is the same
/// answer `interact::check_name_collisions` gives every other generated-name
/// clash: codegen names the failure itself rather than handing rustc a module
/// whose meaning it cannot state.
///
/// The two shapes are named because the mangled name cannot distinguish them —
/// that is the whole defect — and the field lists are what a reader greps for.
fn tuple_collision(previous: &InducedTuple, current: &InducedTuple) -> GenerateError {
    fn shape(induced: &InducedTuple) -> String {
        let fields: Vec<String> = induced
            .tuple
            .fields
            .iter()
            .map(|field| field.name.clone())
            .collect();
        format!("({})", fields.join(", "))
    }
    GenerateError {
        message: format!(
            "the generated name {name} is claimed by two different tuple types, {a} and {b}; \
             a tuple generates a struct named for the path that reaches it, and these two paths \
             spell one name — rename a field or a declaration so they differ",
            name = current.name,
            a = shape(previous),
            b = shape(current),
        ),
    }
}

// ---------------------------------------------------------------------------
// Package context — same-package name lookups for the leaf-recursion rules.
// ---------------------------------------------------------------------------

/// Read-only view of a package indexed by declaration name, so the emitter and
/// the default-derivation pass can resolve a same-package reference to its
/// declaration (cross-package references stay unresolved by design — this
/// backend generates one package at a time).
pub(crate) struct Ctx<'a> {
    /// The lowered codegen model of this package over this scope
    /// (`ridl_ir::codegen::lower`), which the domain-type emitters, the
    /// default derivation and the derive pass read instead of the IR (lane P
    /// stage P4, design note §8.3). The remaining unported emitters read the
    /// IR beside it, paired by index: `model.declarations[i]` is lowered from
    /// `package.decls[i]`.
    pub(crate) model: &'a v1::Model,
    decls: HashMap<&'a str, &'a v2::Decl>,
    /// The other packages of the same build, in the shape
    /// `ridl-backend-flatbuffers::generate_with` already gives them
    /// (ADR-0017 decision 1). Empty for the single-package entry points.
    ///
    /// The codec reads them through the projection's `Packages` so a
    /// cross-package reference can be sized and encoded rather than withheld
    /// (driftsys/ridl#467).
    pub(crate) others: &'a [&'a v2::Package],
    /// The set of declaration names currently being expanded by the
    /// Default-derivation recursion. It guards against a cyclic IR: a
    /// same-package composite that reaches itself would otherwise recurse
    /// forever (C1b). The recursion inserts a name on entry and removes it on
    /// exit, so between top-level declarations the set is empty.
    visiting: RefCell<HashSet<String>>,
}

impl<'a> Ctx<'a> {
    pub(crate) fn new(package: &'a v2::Package, model: &'a v1::Model) -> Self {
        Ctx::with_others(package, &[], model)
    }

    /// [`Ctx::new`] with the other packages of the same build, which the
    /// codec resolves a cross-package reference through (driftsys/ridl#467).
    pub(crate) fn with_others(
        package: &'a v2::Package,
        others: &'a [&'a v2::Package],
        model: &'a v1::Model,
    ) -> Self {
        let decls = package
            .decls
            .iter()
            .map(|decl| (decl.name.as_str(), decl))
            .collect();
        Ctx {
            model,
            decls,
            others,
            visiting: RefCell::new(HashSet::new()),
        }
    }

    /// The declaration the model's `declarations` list holds at `index`.
    pub(crate) fn declaration(&self, index: u32) -> Option<&'a v1::Declaration> {
        self.model.declarations.get(index as usize)
    }

    /// The induced tuple the model's `tuples` list holds at `index`.
    pub(crate) fn tuple(&self, index: u32) -> Option<&'a v1::InducedTuple> {
        self.model.tuples.get(index as usize)
    }

    /// The Rust name of the induced tuple at `index` — the CamelCase of the
    /// path that reached it, which the lowering spells once
    /// (`InducedName.rust`).
    pub(crate) fn tuple_name(&self, index: u32) -> &'a str {
        self.tuple(index)
            .and_then(|tuple| tuple.name.as_ref())
            .map(|name| name.rust.as_str())
            .unwrap_or_default()
    }

    /// The declaration a resolved, same-package reference names, or `None`
    /// for a cross-package reference, for a name no declaration in this
    /// package answers to, and for a reference that names a constant — the
    /// three cases [`Ctx::lookup`] also answers `None` for, kept so the
    /// printers stay as conservative as they are today (design note §3.5).
    pub(crate) fn local(&self, reference: &v1::TypeRef) -> Option<&'a v1::Declaration> {
        if reference.foreign || !reference.resolved {
            return None;
        }
        self.declaration(reference.index)
    }

    /// The declaration named `name` in this package, or `None` for a
    /// cross-package (dotted) or unknown reference.
    pub(crate) fn lookup(&self, name: &str) -> Option<&'a v2::Decl> {
        self.decls.get(name).copied()
    }

    /// Marks `name` as being expanded by the Default recursion. Returns `true`
    /// when it was newly inserted, `false` when it is already on the expansion
    /// stack — a reference cycle that the caller must not recurse into (C1b).
    pub(crate) fn enter_default(&self, name: &str) -> bool {
        self.visiting.borrow_mut().insert(name.to_string())
    }

    /// Removes `name` from the Default-recursion expansion stack, balancing a
    /// prior [`enter_default`](Ctx::enter_default) that returned `true`.
    pub(crate) fn leave_default(&self, name: &str) {
        self.visiting.borrow_mut().remove(name);
    }
}

// ---------------------------------------------------------------------------
// Declaration emission.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Reading the lowered model.
// ---------------------------------------------------------------------------

/// The name the source declared, as the model carries it.
fn declared(name: Option<&v1::Spellings>) -> &str {
    name.map(|name| name.declared.as_str()).unwrap_or_default()
}

/// The pinned `snake_case` of a declared name (ADR-0016 decision 1), spelled
/// once by the lowering.
fn snake_of(name: Option<&v1::Spellings>) -> &str {
    name.map(|name| name.snake.as_str()).unwrap_or_default()
}

/// The pinned `camel_case` of a declared name (ADR-0016, 2026-09-20
/// amendment), spelled once by the lowering.
fn camel_of(name: Option<&v1::Spellings>) -> &str {
    name.map(|name| name.camel.as_str()).unwrap_or_default()
}

/// The generated struct name of one induced tuple — the CamelCase of the path
/// that reached it, which the lowering spells as `InducedName.rust`. Empty for
/// a tuple no backend has a naming rule for.
fn tuple_name(induced: &v1::InducedTuple) -> &str {
    induced
        .name
        .as_ref()
        .map(|name| name.rust.as_str())
        .unwrap_or_default()
}

/// The canonical IR text of the type one union arm names.
fn arm_reference(arm: &v1::Arm) -> &str {
    arm.r#type
        .as_ref()
        .map(|reference| reference.reference.as_str())
        .unwrap_or_default()
}

/// The model's scalar class as this backend's own classification. The two are
/// the same total function of a backing — the lowering's `scalar_class` is
/// [`backing_scalar`] moved into `ridl-ir` — so this is a rename, not a
/// second rule.
pub(crate) fn class_backing(class: i32) -> ScalarBacking {
    match v1::ScalarClass::try_from(class).unwrap_or(v1::ScalarClass::Unspecified) {
        v1::ScalarClass::Integer => ScalarBacking::Integer,
        v1::ScalarClass::Boolean => ScalarBacking::Boolean,
        v1::ScalarClass::String => ScalarBacking::String,
        v1::ScalarClass::Bytes => ScalarBacking::Bytes,
        // A unit backing, an absent backing and a float backing are all
        // float, and an unspecified class is unreachable from the lowering.
        v1::ScalarClass::Float | v1::ScalarClass::Unspecified => ScalarBacking::Float,
    }
}

/// The Rust type one scalar class is realized as (Appendix D language layer):
/// unit and float back to `f64`, integer to `i64`.
fn class_tokens(class: i32) -> TokenStream {
    match class_backing(class) {
        ScalarBacking::Float => quote! { f64 },
        ScalarBacking::Integer => quote! { i64 },
        ScalarBacking::Boolean => quote! { bool },
        ScalarBacking::String => quote! { String },
        ScalarBacking::Bytes => quote! { Vec<u8> },
    }
}

/// The Rust newtype inner type for a named scalar (Appendix D language
/// layer).
fn newtype_inner(sc: &v1::Scalar) -> TokenStream {
    class_tokens(sc.class)
}

/// The Rust type of one type position, read from the lowered model.
///
/// It is [`field_type_tokens`] over the model: the same shapes, with the
/// references already resolved and every induced tuple already named. A tuple
/// position is a `TupleRef` into `Model.tuples`, so this walk neither
/// discovers nor names one — [`domain_items`] emits the struct for each named
/// entry of that list.
pub(crate) fn model_type_tokens(ctx: &Ctx, ty: &v1::Type) -> TokenStream {
    let inner = match ty.kind.as_ref() {
        Some(v1::r#type::Kind::Named(reference)) => type_path(&reference.reference),
        Some(v1::r#type::Kind::Primitive(prim)) => model_primitive_tokens(*prim),
        Some(v1::r#type::Kind::Inline(sc)) => class_tokens(sc.class),
        Some(v1::r#type::Kind::Tuple(reference)) => {
            let id = ident(ctx.tuple_name(reference.index));
            quote! { #id }
        }
        Some(v1::r#type::Kind::Array(array)) => {
            let element = array
                .element
                .as_deref()
                .map(|element| model_type_tokens(ctx, element))
                .unwrap_or_else(|| quote! { () });
            if array.min == array.max {
                let len = usize_tokens(array.min);
                quote! { [#element; #len] }
            } else {
                quote! { Vec<#element> }
            }
        }
        Some(v1::r#type::Kind::Map(map)) => {
            let key = map
                .key
                .as_deref()
                .map(|key| model_type_tokens(ctx, key))
                .unwrap_or_else(|| quote! { () });
            let value = map
                .value
                .as_deref()
                .map(|value| model_type_tokens(ctx, value))
                .unwrap_or_else(|| quote! { () });
            quote! { Vec<(#key, #value)> }
        }
        // A stream is an interaction-position type (ridl §12.3); it never
        // reaches a struct or tuple field in checked IR. Kept total.
        Some(v1::r#type::Kind::Stream(_)) | None => quote! { () },
    };

    if ty.optional {
        quote! { Option<#inner> }
    } else {
        inner
    }
}

/// [`primitive_tokens`] over the model's own `PrimitiveType`, which restates
/// the IR's values (design note §3.8).
fn model_primitive_tokens(prim: i32) -> TokenStream {
    match v1::PrimitiveType::try_from(prim).unwrap_or(v1::PrimitiveType::Unspecified) {
        v1::PrimitiveType::Boolean => quote! { bool },
        v1::PrimitiveType::Integer => quote! { i64 },
        v1::PrimitiveType::Float => quote! { f64 },
        v1::PrimitiveType::String => quote! { String },
        v1::PrimitiveType::Bytes => quote! { Vec<u8> },
        v1::PrimitiveType::Unspecified => quote! { () },
    }
}

fn emit_decl(ctx: &Ctx, decl: &v1::Declaration) -> TokenStream {
    // The derive attribute is computed once and handed to the emitter, which
    // places it under the declaration's doc comment rather than above it.
    // Prepending it to the finished item would render it above the doc, which
    // is backwards from how Rust is written everywhere else — and `Default` is
    // never among the traits (`derives`, design decision 8).
    let derived = derives::derive_attr(ctx, decl);
    let item = match decl.kind.as_ref() {
        Some(v1::declaration::Kind::Scalar(sc)) => emit_type_def(decl, sc, &derived),
        Some(v1::declaration::Kind::Constant(cd)) => return emit_const(ctx, decl, cd),
        Some(v1::declaration::Kind::Struct(sd)) => emit_struct(ctx, decl, sd, &derived),
        Some(v1::declaration::Kind::Enum(ed)) => emit_enum(decl, ed, &derived),
        Some(v1::declaration::Kind::EnumSet(esd)) => emit_enum_set(decl, esd, &derived),
        Some(v1::declaration::Kind::Union(ud)) => emit_union(decl, ud, &derived),
        // An interaction rides `Interface.interactions`, never a package
        // declaration, so the lowering leaves the kind unset for one and
        // nothing emits them today.
        None => return quote! {},
    };

    let default_impl = defaults::decl_default_expr(ctx, decl)
        .map(|expr| {
            let name = ident(declared(decl.name.as_ref()));
            quote! { impl Default for #name { fn default() -> Self { #expr } } }
        })
        .unwrap_or_default();

    quote! { #item #default_impl }
}

/// A named scalar becomes a `#[repr(transparent)]` newtype with a private
/// inner value (typl §5.7). Construction goes through `new`, which enforces
/// the typl constraints, or `new_unchecked`, which does not.
///
/// `Violation` and `Rule` are named by absolute path and nothing is imported:
/// a typl package may declare a type named `Violation` or `Rule`, and a `use`
/// of either would collide with that declaration. The leading `::` covers a
/// package that declares a type named `ridl_rt`. The prelude names the
/// constructors use — `Result`, `Ok`, `Err`, `TryFrom`, `From` — are absolute
/// for the same reason: a type name is CamelCase (typl §15.1) and `ridl-sem`
/// reserves no identifier, so a package may declare `type Result`, and that
/// struct would shadow the prelude's in the module the constructors share
/// with it.
///
/// A deprecated declaration's impl blocks carry `#[allow(deprecated)]`, with
/// one exception: the `Default` impl `defaults::decl_default_expr` emits
/// carries no allow, because `emit_decl` attaches it outside this function.
/// Each covered impl block uses the deprecated type, and without the allow
/// the consumer's build draws the `deprecated` lint on code the consumer did
/// not write.
fn emit_type_def(decl: &v1::Declaration, sc: &v1::Scalar, derived: &TokenStream) -> TokenStream {
    let name = ident(declared(decl.name.as_ref()));
    let inner = newtype_inner(sc);
    let doc = doc_attrs(&decl.doc);
    let unchecked = unchecked_doc(sc);
    // A blank doc line keeps the unchecked note out of the declaration's own
    // doc paragraph.
    let separator = if decl.doc.is_empty() || unchecked.is_empty() {
        quote! {}
    } else {
        quote! { #[doc = ""] }
    };
    let deprecated = deprecated_attr(decl.deprecated.as_deref());
    let allow_deprecated = if decl.deprecated.is_some() {
        quote! { #[allow(deprecated)] }
    } else {
        quote! {}
    };
    let vis = vis_tokens(decl.visibility);
    let type_name = declared(decl.name.as_ref());

    if sc.vacuous {
        return emit_vacuous_type_def(decl, sc, derived);
    }

    let check_param_ty = check_param_type(sc);
    let check_shadow = check_deref_shadow(sc);
    let check_body = constraint_checks(sc, type_name, quote! { value });
    let getter = scalar_getter(sc, vis.clone(), inner.clone());

    quote! {
        #doc
        #separator
        #unchecked
        #derived
        #deprecated
        #[repr(transparent)]
        #vis struct #name(#inner);

        #allow_deprecated
        impl #name {
            /// Constructs the value, enforcing its typl constraints.
            #vis fn new(
                value: #inner,
            ) -> ::core::result::Result<Self, ::ridl_rt::payload::Violation> {
                Self::check(&value)?;
                ::core::result::Result::Ok(Self::new_unchecked(value))
            }

            /// Checks `value` against this type's typl constraints, without
            /// constructing it. `pub(crate)` rather than `pub`: a caller
            /// outside `new` is a function generated into this crate — since
            /// driftsys/ridl#467 that includes the codec of *another* package
            /// of the same build, which reaches this type through the module
            /// tree and so cannot see a private item here. The emitted crate
            /// is one crate per build, so `pub(crate)` reaches every such
            /// caller while adding nothing to the crate's public surface.
            /// Whether this becomes `pub` is Epic 10's call, still open.
            /// `new` is the composition of this and `new_unchecked`.
            pub(crate) fn check(
                value: #check_param_ty,
            ) -> ::core::result::Result<(), ::ridl_rt::payload::Violation> {
                #check_shadow
                #check_body
                ::core::result::Result::Ok(())
            }

            /// Constructs the value without checking its constraints.
            ///
            /// Safe: nothing here relies on the invariant for memory
            /// soundness. Use it only for a value already known to satisfy
            /// the contract.
            #vis const fn new_unchecked(value: #inner) -> Self {
                Self(value)
            }

            #getter
        }

        #allow_deprecated
        impl ::core::convert::TryFrom<#inner> for #name {
            type Error = ::ridl_rt::payload::Violation;
            fn try_from(value: #inner) -> ::core::result::Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        #allow_deprecated
        impl ::core::convert::From<#name> for #inner {
            fn from(value: #name) -> Self {
                value.0
            }
        }
    }
}

/// A named scalar whose constraint checks nothing: `boolean`, and `integer`
/// or `float` with no declared range. A `String` or `Vec<u8>` backing reaches
/// this only from hand-built IR: the checker always materializes the typl §4.4
/// default `[0..256]` length bound, so both are constrained on the source
/// route.
///
/// Construction is infallible, so `From<Inner>` is correct here. That is not
/// because the type carries no invariant at all — a `step`-only constraint
/// reaches this function too (`constraint_is_vacuous` excludes `step`), and
/// its quantization is a real invariant, which `unchecked_doc` names on the
/// type a few lines below. It is because `new` checks nothing `From` would
/// then bypass: `constraint_checks` emits a range branch only for a min or a
/// max, a length branch only for `len_min`/`len_max`, and a pattern branch
/// only for `pattern`/`pattern_const` — none of which a vacuous constraint
/// carries — and `step` is never checked by `new` on any type, constrained or
/// not. `From<Inner>` therefore introduces no failure the checked path would
/// have caught. Core's blanket `impl<T, U: Into<T>> TryFrom<U> for T` then
/// supplies `TryFrom<Inner>` with `Error = Infallible`, so generic consumer
/// code calling `try_from` compiles against both kinds of scalar. A manual
/// `TryFrom` would collide with that blanket impl (`rustc` reports `E0119`),
/// which is the second reason it is absent.
///
/// `new_unchecked` is deliberately absent: `new` already is the unchecked
/// path, and on this type it is `const`, so [`scalar_ctor`] routes a constant
/// and a derived default through `new` instead.
///
/// The prelude names are absolute for the reason [`emit_type_def`] records: a
/// typl package may declare `type From`, and that declaration shadows the
/// prelude in the module the generated impl shares with it.
fn emit_vacuous_type_def(
    decl: &v1::Declaration,
    sc: &v1::Scalar,
    derived: &TokenStream,
) -> TokenStream {
    let name = ident(declared(decl.name.as_ref()));
    let inner = newtype_inner(sc);
    let doc = doc_attrs(&decl.doc);
    // A `step`-only constraint is vacuous (`constraint_is_vacuous` excludes
    // `step`), and that is exactly the case `unchecked_doc` still speaks for,
    // so the note and its separator are computed here too.
    let unchecked = unchecked_doc(sc);
    let separator = if decl.doc.is_empty() || unchecked.is_empty() {
        quote! {}
    } else {
        quote! { #[doc = ""] }
    };
    let deprecated = deprecated_attr(decl.deprecated.as_deref());
    let allow_deprecated = if decl.deprecated.is_some() {
        quote! { #[allow(deprecated)] }
    } else {
        quote! {}
    };
    let vis = vis_tokens(decl.visibility);
    let getter = scalar_getter(sc, vis.clone(), inner.clone());

    quote! {
        #doc
        #separator
        #unchecked
        #derived
        #deprecated
        #[repr(transparent)]
        #vis struct #name(#inner);

        #allow_deprecated
        impl #name {
            /// Constructs the value. This type declares no constraint, so
            /// construction cannot fail.
            #vis const fn new(value: #inner) -> Self {
                Self(value)
            }

            #getter
        }

        #allow_deprecated
        impl ::core::convert::From<#inner> for #name {
            fn from(value: #inner) -> Self {
                Self(value)
            }
        }

        #allow_deprecated
        impl ::core::convert::From<#name> for #inner {
            fn from(value: #name) -> Self {
                value.0
            }
        }
    }
}

/// The associated function a constant or a derived default constructs a named
/// scalar through. A constrained type keeps `new_unchecked`, whose value is
/// checked by `ridlc` rather than at run time; a vacuous type has no
/// `new_unchecked` ([`emit_vacuous_type_def`]) and its `new` is `const`, so
/// both positions — a `const` item and the body of `fn default()` — accept it.
pub(crate) fn scalar_ctor(sc: &v1::Scalar) -> TokenStream {
    if sc.vacuous {
        quote! { new }
    } else {
        quote! { new_unchecked }
    }
}

/// The range, length and pattern checks for one constraint, as statements
/// that return early with a `Violation`. Only the branches the constraint
/// carries are emitted, so a string with a length bound and no range gets
/// only the length check. The pattern check is emitted last and is the only
/// one behind a feature gate.
///
/// A `min` or `max` is a numeric bound (typl §5.5), so a range check is
/// emitted only for a float or integer backing; on any other backing the two
/// are ignored rather than rendered as a literal of the wrong type.
fn constraint_checks(sc: &v1::Scalar, type_name: &str, value: TokenStream) -> TokenStream {
    let Some(c) = sc.constraint.as_ref() else {
        return quote! {};
    };
    let mut checks = Vec::new();

    let is_float = match class_backing(sc.class) {
        ScalarBacking::Float => Some(true),
        ScalarBacking::Integer => Some(false),
        ScalarBacking::Boolean | ScalarBacking::String | ScalarBacking::Bytes => None,
    };
    if let Some(is_float) = is_float {
        if let Some(min) = c.min.as_deref() {
            let lit = numeric_tokens(min, is_float);
            checks.push(quote! {
                if #value < #lit {
                    return ::core::result::Result::Err(::ridl_rt::payload::Violation {
                        type_name: #type_name,
                        rule: ::ridl_rt::payload::Rule::Range,
                    });
                }
            });
        }
        // The newtype backing an integer is always `i64` (`newtype_inner`), so
        // a declared maximum at `i64::MAX` (9223372036854775807) makes
        // `value > 9223372036854775807` never true: rustc draws its
        // `unused_comparisons` warning on it in the consumer's build. The
        // branch is emitted only when the maximum is below the inner type's
        // maximum.
        let checked_max = c
            .max
            .as_deref()
            .filter(|max| is_float || max.parse::<i64>() != Ok(i64::MAX));
        if let Some(max) = checked_max {
            let lit = numeric_tokens(max, is_float);
            checks.push(quote! {
                if #value > #lit {
                    return ::core::result::Result::Err(::ridl_rt::payload::Violation {
                        type_name: #type_name,
                        rule: ::ridl_rt::payload::Rule::Range,
                    });
                }
            });
        }
    }
    // Length is in characters for string (typl §5.3) and bytes for bytes
    // (§5.4), which is why the two use different expressions. The cast is
    // parenthesized because `as u64 < 8` does not parse: after a cast type,
    // `<` opens a generic-argument list.
    if c.len_min.is_some() || c.len_max.is_some() {
        let len = match class_backing(sc.class) {
            ScalarBacking::String => quote! { (#value.chars().count() as u64) },
            _ => quote! { (#value.len() as u64) },
        };
        // A minimum of 0 is the default length bound of string and bytes
        // (typl §4.4, §4.5), and `(… as u64) < 0` is never true: rustc draws
        // its `unused_comparisons` warning on it in the consumer's build. The
        // branch is emitted only for a positive minimum.
        if let Some(min) = c.len_min.filter(|min| *min > 0) {
            let lit = proc_macro2::Literal::u64_unsuffixed(min);
            checks.push(quote! {
                if #len < #lit {
                    return ::core::result::Result::Err(::ridl_rt::payload::Violation {
                        type_name: #type_name,
                        rule: ::ridl_rt::payload::Rule::Length,
                    });
                }
            });
        }
        if let Some(max) = c.len_max {
            let lit = proc_macro2::Literal::u64_unsuffixed(max);
            checks.push(quote! {
                if #len > #lit {
                    return ::core::result::Result::Err(::ridl_rt::payload::Violation {
                        type_name: #type_name,
                        rule: ::ridl_rt::payload::Rule::Length,
                    });
                }
            });
        }
    }
    if class_backing(sc.class) == ScalarBacking::String
        && let Some(pattern) = c.pattern.as_deref()
    {
        // A `match` pattern is checked against text, and `regex::Regex`
        // matches `&str`. Only a `String` backing has a value that coerces
        // to `&str` (`newtype_inner`); a bytes backing carries `Vec<u8>`,
        // against which `Regex::is_match` does not type-check.
        //
        // No typl source reaches this: the reference gives bytes no `match`
        // (§4.5, §5.4) and `lower_scalar` passes `allow_pattern: false` for
        // that backing, so the pattern never enters the IR. The guard is
        // totality over the IR rather than over the surface, on the same
        // footing as the `is_float` guard above — `lower_len_scalar` always
        // leaves `min` and `max` absent, so that one is unreachable from a
        // typl source too, and is pinned by its own test. A backend reads
        // the IR, which need not have come from this checker.
        //
        // The pattern needs a regex engine, which `core` has none of. The
        // range and length checks above are not gated; only this one is, so
        // a `--no-default-features` build still validates the bounds it
        // emits.
        //
        // `::std` and `::regex` are absolute for the reason the prelude
        // names are: the face module of an interface named `Std` or `Regex`
        // is a module of that name in this same module, and it would shadow
        // the extern crate.
        // The lowering already stripped the typl `/…/` delimiters, which are
        // syntax rather than pattern content (`Constraint.pattern`).
        let source = pattern;
        checks.push(quote! {
            #[cfg(feature = "validate-pattern")]
            {
                static PATTERN: ::std::sync::LazyLock<::regex::Regex> =
                    ::std::sync::LazyLock::new(|| {
                        ::regex::Regex::new(#source).expect("ridlc emitted an invalid pattern")
                    });
                if !PATTERN.is_match(&#value) {
                    return ::core::result::Result::Err(::ridl_rt::payload::Violation {
                        type_name: #type_name,
                        rule: ::ridl_rt::payload::Rule::Pattern,
                    });
                }
            }
        });
    }
    quote! { #(#checks)* }
}

/// The parameter type `check` (`emit_type_def`) borrows its value as. A
/// `String`/`Vec<u8>` backing borrows the slice form directly — `&str` and
/// `&[u8]` — which is what a zero-copy caller (the FlatBuffers codec's
/// `verify`) already holds and needs no allocation to produce; a `Copy`
/// backing borrows the newtype's own inner type, which [`check_deref_shadow`]
/// then reads back to a plain value so [`constraint_checks`]'s emitted
/// expressions need no change between `new`'s former inline form and `check`.
///
/// This matches on `backing_scalar` the same way [`newtype_inner`] does, by
/// the same backing's borrowed form rather than its owned one; the two
/// matches must stay in lockstep for every backing this function names, and
/// each names the other for that reason.
fn check_param_type(sc: &v1::Scalar) -> TokenStream {
    match class_backing(sc.class) {
        ScalarBacking::Float => quote! { &f64 },
        ScalarBacking::Integer => quote! { &i64 },
        ScalarBacking::Boolean => quote! { &bool },
        ScalarBacking::String => quote! { &str },
        ScalarBacking::Bytes => quote! { &[u8] },
    }
}

/// A `Copy` backing's `check` parameter is a reference (`check_param_type`),
/// while [`constraint_checks`]'s emitted comparisons are written against a
/// plain value (`value < 0.0`, not `*value < 0.0`). This reborrows the
/// parameter into a local of the same name and the owned type, so those
/// expressions type-check unchanged. `String` and `&[u8]` need no shadow:
/// their methods (`.chars()`, `.len()`) and the `&value` the pattern check
/// takes both work directly on the borrowed slice form.
fn check_deref_shadow(sc: &v1::Scalar) -> TokenStream {
    match class_backing(sc.class) {
        ScalarBacking::Float | ScalarBacking::Integer | ScalarBacking::Boolean => {
            quote! { let value = *value; }
        }
        ScalarBacking::String | ScalarBacking::Bytes => quote! {},
    }
}

/// The accessor. A `Copy` backing returns by value from a `const fn`; `String`
/// and `Vec<u8>` borrow, and gain `into_inner` for the owned form.
///
/// `backing_scalar` is total: it maps a unit backing and an absent backing to
/// `Float`, so every named scalar gets exactly one of the three forms.
fn scalar_getter(sc: &v1::Scalar, vis: TokenStream, inner: TokenStream) -> TokenStream {
    match class_backing(sc.class) {
        ScalarBacking::String => quote! {
            #vis fn get(&self) -> &str { &self.0 }
            #vis fn into_inner(self) -> String { self.0 }
        },
        ScalarBacking::Bytes => quote! {
            #vis fn get(&self) -> &[u8] { &self.0 }
            #vis fn into_inner(self) -> Vec<u8> { self.0 }
        },
        _ => quote! {
            #vis const fn get(self) -> #inner { self.0 }
        },
    }
}

/// The gaps a generated constructor does not close, named on the type itself
/// rather than left silent: a `step` is not checked by `new`. A literal
/// `match` pattern on a `String` backing is checked by `new`, but only under
/// the `validate-pattern` feature, so the type names that condition rather
/// than leaving the guarantee silently variable. On any other backing
/// `constraint_checks` emits no pattern branch at all (a `regex::Regex`
/// matches `&str`, and only a `String` backing's value coerces to one), so
/// the plain "not checked" line applies there instead. `pattern_const` is
/// read as well as `pattern`, because a pattern constant that did not resolve
/// leaves `pattern` absent while the type still carries a match constraint,
/// and no check is emitted for that case either, so it keeps the plain "not
/// checked" line.
fn unchecked_doc(sc: &v1::Scalar) -> TokenStream {
    let Some(c) = sc.constraint.as_ref() else {
        return quote! {};
    };
    let mut lines = Vec::new();
    if c.step.is_some() {
        lines.push(" Quantization (`step`) is not checked by `new`.".to_string());
    }
    if c.pattern.is_some() && class_backing(sc.class) == ScalarBacking::String {
        lines.push(
            " The `match` pattern is checked by `new` only when the crate is built with \
              the `validate-pattern` feature."
                .to_string(),
        );
    } else if c.pattern.is_some() || c.pattern_const.is_some() {
        lines.push(" The `match` pattern is not checked by `new`.".to_string());
    }
    quote! { #(#[doc = #lines])* }
}

/// A constant becomes a `pub const`. A constant of a `String`-backed named type
/// (or of the `string` primitive, or a regex constant) is realized as a
/// `&'static str` rather than a value of the newtype: `String` cannot be
/// constructed in a `const` context. This asymmetry is documented in the C
/// header and here.
fn emit_const(ctx: &Ctx, decl: &v1::Declaration, cd: &v1::Constant) -> TokenStream {
    // A constant is a value, not a type: there is nothing to derive on it.
    let attrs = decl_attrs(decl, &quote! {});
    let vis = vis_tokens(decl.visibility);
    let name = ident(declared(decl.name.as_ref()));

    match cd.typed.as_ref() {
        // A regex constant declares no type; it holds the pattern source
        // text. The IR stores that text with its typl `/…/` delimiters, which
        // are syntax, not pattern content, and the lowering strips them, so
        // the const holds the pattern a consumer can feed to a regex engine
        // (M1).
        Some(v1::constant::Typed::RegexBody(pattern)) => {
            quote! { #attrs #vis const #name: &str = #pattern; }
        }
        // A named-type constant resolves through the type's backing. Only a
        // same-package named scalar is emitted: a reference into another
        // package is skipped rather than mis-typed, which is the conservative
        // rule the model's resolution leaves to the printer (design note
        // §3.5).
        Some(v1::constant::Typed::Named(reference)) => {
            let Some(target) = ctx.local(reference) else {
                return quote! {};
            };
            let Some(v1::declaration::Kind::Scalar(sc)) = target.kind.as_ref() else {
                return quote! {};
            };
            // The constructor is read from the same declaration as the class
            // above: `new` on a vacuous type, whose `new` is `const` and
            // infallible, and `new_unchecked` on a constrained one, because
            // `new` is fallible there and does not type-check in a `const`
            // position (`scalar_ctor`).
            let ctor = scalar_ctor(sc);
            match class_backing(sc.class) {
                ScalarBacking::Float => {
                    let value = numeric_tokens(&cd.value, true);
                    let type_name = type_path(&reference.reference);
                    quote! { #attrs #vis const #name: #type_name = #type_name::#ctor(#value); }
                }
                ScalarBacking::Integer => {
                    let value = numeric_tokens(&cd.value, false);
                    let type_name = type_path(&reference.reference);
                    quote! { #attrs #vis const #name: #type_name = #type_name::#ctor(#value); }
                }
                ScalarBacking::Boolean => {
                    let value = bool_tokens(&cd.value);
                    let type_name = type_path(&reference.reference);
                    quote! { #attrs #vis const #name: #type_name = #type_name::#ctor(#value); }
                }
                ScalarBacking::String => {
                    let value = cd.value.as_str();
                    quote! { #attrs #vis const #name: &str = #value; }
                }
                ScalarBacking::Bytes => quote! {},
            }
        }
        Some(v1::constant::Typed::Primitive(prim)) => {
            match v1::PrimitiveType::try_from(*prim).unwrap_or(v1::PrimitiveType::Unspecified) {
                v1::PrimitiveType::Integer => {
                    let value = numeric_tokens(&cd.value, false);
                    quote! { #attrs #vis const #name: i64 = #value; }
                }
                v1::PrimitiveType::Float => {
                    let value = numeric_tokens(&cd.value, true);
                    quote! { #attrs #vis const #name: f64 = #value; }
                }
                v1::PrimitiveType::Boolean => {
                    let value = bool_tokens(&cd.value);
                    quote! { #attrs #vis const #name: bool = #value; }
                }
                v1::PrimitiveType::String => {
                    let value = cd.value.as_str();
                    quote! { #attrs #vis const #name: &str = #value; }
                }
                v1::PrimitiveType::Bytes | v1::PrimitiveType::Unspecified => quote! {},
            }
        }
        // A type no package in the scope declares, and a constant that
        // declares no type at all: the backing is unknown here, so the
        // constant is skipped rather than mis-typed.
        Some(v1::constant::Typed::Unresolved(_)) | None => quote! {},
    }
}

fn emit_struct(
    ctx: &Ctx,
    decl: &v1::Declaration,
    sd: &v1::Struct,
    derived: &TokenStream,
) -> TokenStream {
    let name = ident(declared(decl.name.as_ref()));
    let attrs = decl_attrs(decl, derived);
    let vis = vis_tokens(decl.visibility);
    let repr = if sd.fixed_layout {
        quote! { #[repr(C)] }
    } else {
        quote! {}
    };

    let fields = sd
        .slots
        .iter()
        .filter_map(|slot| match slot.occupant.as_ref() {
            Some(v1::slot::Occupant::Field(field)) => Some(emit_field(ctx, field)),
            // A reserved tombstone occupies an ordinal but emits no field
            // (typl §7.4).
            Some(v1::slot::Occupant::Retired(_)) | None => None,
        });

    quote! {
        #attrs
        #repr
        #vis struct #name {
            #(#fields),*
        }
    }
}

/// One struct field. The name is projected through the pinned transform
/// (ADR-0016 decisions 1 and 2): a typl field name is camelCase (typl §15.1)
/// and reaching generated Rust verbatim draws `non_snake_case` at every
/// consumer. The `hint` below keeps `camel_case`, because it builds the type
/// name of an induced tuple struct rather than a field name. That second
/// projection reaches a namespace RIDL-149 does not check — two field names
/// distinct under `snake_case` can induce one tuple type name — which is
/// driftsys/ridl#453, recorded in ADR-0016's consequences.
fn emit_field(ctx: &Ctx, field: &v1::Field) -> TokenStream {
    let field_name = ident(snake_of(field.name.as_ref()));
    let attrs = field_attrs(field);
    let ty = field
        .r#type
        .as_ref()
        .map(|ft| model_type_tokens(ctx, ft))
        .unwrap_or_else(|| quote! { () });
    quote! { #attrs pub #field_name: #ty }
}

/// An enum becomes `#[repr(i64)]` with the declared discriminants (typl §8).
/// Variant names keep their typl `SCREAMING_SNAKE` spelling.
fn emit_enum(decl: &v1::Declaration, ed: &v1::Enum, derived: &TokenStream) -> TokenStream {
    let name = ident(declared(decl.name.as_ref()));
    let attrs = decl_attrs(decl, derived);
    let vis = vis_tokens(decl.visibility);

    let variants = ed.values.iter().map(|value| {
        let vname = ident(declared(value.name.as_ref()));
        let disc = int_tokens(value.value);
        let vdoc = doc_attrs(&value.doc);
        quote! { #vdoc #vname = #disc }
    });

    // A raw discriminant off the wire is where an out-of-contract value
    // actually enters a program: a wire backend emits no constructor
    // (ADR-0013 decision 2), so this is the validating seam.
    let arms = ed.values.iter().map(|value| {
        let vname = ident(declared(value.name.as_ref()));
        let disc = int_tokens(value.value);
        quote! { #disc => ::core::result::Result::Ok(Self::#vname) }
    });
    let type_name = declared(decl.name.as_ref());
    let allow_deprecated = if decl.deprecated.is_some() {
        quote! { #[allow(deprecated)] }
    } else {
        quote! {}
    };

    quote! {
        #attrs
        #[repr(i64)]
        #vis enum #name {
            #(#variants),*
        }

        #allow_deprecated
        impl ::core::convert::TryFrom<i64> for #name {
            type Error = ::ridl_rt::payload::Violation;
            fn try_from(value: i64) -> ::core::result::Result<Self, Self::Error> {
                match value {
                    #(#arms,)*
                    _ => ::core::result::Result::Err(::ridl_rt::payload::Violation {
                        type_name: #type_name,
                        rule: ::ridl_rt::payload::Rule::Variant,
                    }),
                }
            }
        }

        #allow_deprecated
        impl ::core::convert::From<#name> for i64 {
            fn from(value: #name) -> Self { value as i64 }
        }
    }
}

/// An enum set becomes a `#[repr(transparent)]` newtype over `i64` (the
/// language layer width, Appendix D) with one associated bit constant per bit
/// position (typl §9).
///
/// The inner value is private, as a named scalar's is and for the same reason
/// ([`emit_type_def`]): a raw bit pattern enters through `TryFrom<i64>`, which
/// refuses a value carrying an undeclared bit. `get` reads it back.
///
/// The bit constants, `DECLARED_MASK` and `get` share one inherent impl block
/// so that a deprecated declaration carries `#[allow(deprecated)]` over all
/// three. The `impl` header itself names the deprecated type, as do the bit
/// constants and `get`; `DECLARED_MASK` names only `i64`, and is covered
/// because it shares the block. Without the allow the consumer's build draws
/// the `deprecated` lint on code the consumer did not write — which is what
/// driftsys/ridl#420 settled for a named scalar's impl blocks.
fn emit_enum_set(decl: &v1::Declaration, esd: &v1::EnumSet, derived: &TokenStream) -> TokenStream {
    let name = ident(declared(decl.name.as_ref()));
    let attrs = decl_attrs(decl, derived);
    let vis = vis_tokens(decl.visibility);

    let bits = esd.bits.iter().map(|bit| {
        let bname = ident(declared(bit.name.as_ref()));
        let shift = int_tokens(bit.value);
        quote! { #vis const #bname: #name = #name(1 << #shift); }
    });

    // Refusing a value that carries an undeclared bit is this backend's
    // reading, not a rule the reference states. typl §9 fixes a bit's
    // identity as its declared position and infers the width from the highest
    // one; it says nothing about what an undeclared bit means. The reading is
    // in tension with `ridl-diff`, whose `EnumSetDef` arm classifies an
    // appended bit as compatible outright — it calls `appended_slot` with an
    // empty retired set, so an enum set has no retired half the way an enum's
    // values do (`crates/ridl-diff/src/classify.rs`). A producer that appends
    // a bit on that advice sends a value an older consumer's `TryFrom` then
    // refuses whole, rather than ignoring the bit it does not know. Whether
    // an enum set is closed or open on the wire is recorded as an open
    // question rather than settled here, because settling it changes that arm
    // of `ridl-diff` as well as this backend.
    //
    // A bit outside the int64 domain contributes nothing to the mask rather
    // than shifting by it. `ridl-sem` reports TYPL-111 for a position outside
    // 0..=63 and still carries the bit into the IR — its range guard covers
    // the width it derives, not the value it stores — so this fold can be
    // handed one. `1i64 << 64` panics in a debug build, and codegen is total:
    // every failure is a `GenerateError` value, never a panic.
    //
    // The filter covers the fold alone, and that is all it is for. The bit
    // constants still emit `#name(1 << 64)` as source text, which rustc
    // rejects under its deny-by-default `arithmetic_overflow`, and the mask
    // then omits a bit the type publishes as a constant. Both are reachable
    // only on a package the checker has already failed with TYPL-111, so no
    // build that produces usable output reaches either. The filter keeps the
    // compiler from panicking; it does not make such a package emit sound
    // code.
    let mask_lit = int_tokens(esd.declared_mask);
    let type_name = declared(decl.name.as_ref());
    let allow_deprecated = if decl.deprecated.is_some() {
        quote! { #[allow(deprecated)] }
    } else {
        quote! {}
    };

    quote! {
        #attrs
        #[repr(transparent)]
        #vis struct #name(i64);
        #allow_deprecated
        impl #name {
            #(#bits)*

            /// The union of every declared bit. `TryFrom` refuses a value
            /// that carries any other bit.
            #vis const DECLARED_MASK: i64 = #mask_lit;

            #vis const fn get(self) -> i64 { self.0 }
        }

        #allow_deprecated
        impl ::core::convert::TryFrom<i64> for #name {
            type Error = ::ridl_rt::payload::Violation;
            fn try_from(value: i64) -> ::core::result::Result<Self, Self::Error> {
                if value & !Self::DECLARED_MASK != 0 {
                    return ::core::result::Result::Err(::ridl_rt::payload::Violation {
                        type_name: #type_name,
                        rule: ::ridl_rt::payload::Rule::Variant,
                    });
                }
                ::core::result::Result::Ok(Self(value))
            }
        }

        #allow_deprecated
        impl ::core::convert::From<#name> for i64 {
            fn from(value: #name) -> Self { value.0 }
        }
    }
}

/// A union becomes a `pub enum` with one variant per arm; arm names are
/// CamelCased (typl §10). Reserved arms are skipped.
fn emit_union(decl: &v1::Declaration, ud: &v1::Union, derived: &TokenStream) -> TokenStream {
    let name = ident(declared(decl.name.as_ref()));
    let attrs = decl_attrs(decl, derived);
    let vis = vis_tokens(decl.visibility);

    let variants = ud.arms.iter().map(|arm| {
        let vname = ident(camel_of(arm.name.as_ref()));
        let ty = type_path(arm_reference(arm));
        let vdoc = doc_attrs(&arm.doc);
        quote! { #vdoc #vname(#ty) }
    });

    quote! {
        #attrs
        #vis enum #name {
            #(#variants),*
        }
    }
}

/// Emits the generated struct for one tuple type (typl §11), plus its `Default`
/// impl when every tuple field is derivable.
///
/// The struct carries the visibility of the declaration that induced it, and a
/// tuple nested inside it inherits the same one — a tuple has no visibility of
/// its own to declare, so the only visibility it can have is the one it was
/// reached at (see [`InducedTuple`] and [`vis_tokens`]). The fields stay `pub`,
/// as they are on a declared `struct`: a field's effective visibility is capped
/// by the item's, so `pub(crate) struct T { pub f: Private }` exposes nothing.
///
/// A tuple field name is projected through the pinned transform, the same one
/// [`emit_field`] applies to a declared struct's field (ADR-0016 decisions 1
/// and 2). The `hint` below keeps `camel_case`, because it builds a nested
/// tuple's type name rather than a field name. Neither namespace is checked:
/// two tuple field names distinct in typl can spell one Rust field name, which
/// rustc then rejects with E0124 — driftsys/ridl#449.
fn emit_tuple_struct(ctx: &Ctx, induced: &v1::InducedTuple) -> TokenStream {
    let name = tuple_name(induced);
    let name_id = ident(name);
    let vis = vis_tokens(induced.visibility);
    let derived = derives::tuple_derive_attr(ctx, induced);
    let fields = induced.fields.iter().map(|field| {
        let fname = ident(snake_of(field.name.as_ref()));
        let ty = field
            .r#type
            .as_ref()
            .map(|ft| model_type_tokens(ctx, ft))
            .unwrap_or_else(|| quote! { () });
        quote! { pub #fname: #ty }
    });

    let struct_item = quote! {
        #derived
        #vis struct #name_id {
            #(#fields),*
        }
    };

    let default_impl = defaults::tuple_default_expr(ctx, induced)
        .map(|expr| quote! { impl Default for #name_id { fn default() -> Self { #expr } } })
        .unwrap_or_default();

    quote! { #struct_item #default_impl }
}

// ---------------------------------------------------------------------------
// Type mapping.
// ---------------------------------------------------------------------------

/// The Rust type of a field. Tuple field types generate a named nested struct
/// (recorded in `tuples`); the struct name is `hint` (CamelCase of the path).
///
/// `visibility` is the visibility of the declaration this position belongs to.
/// It is carried rather than derived because a tuple is anonymous in source and
/// declares none of its own, and because it reaches [`emit_tuple_struct`]
/// through a flat worklist that has forgotten where it came from
/// ([`InducedTuple`]).
pub(crate) fn field_type_tokens(
    ft: &v2::FieldType,
    hint: &str,
    visibility: i32,
    tuples: &mut Vec<InducedTuple>,
) -> TokenStream {
    let inner = match &ft.kind {
        Some(v2::field_type::Kind::Named(name)) => type_path(name),
        Some(v2::field_type::Kind::Primitive(prim)) => primitive_tokens(*prim),
        Some(v2::field_type::Kind::InlineScalar(td)) => inline_scalar_tokens(td),
        Some(v2::field_type::Kind::Tuple(tuple)) => {
            let tuple_name = hint.to_string();
            tuples.push(InducedTuple {
                name: tuple_name.clone(),
                tuple: tuple.clone(),
                visibility,
            });
            let id = ident(&tuple_name);
            quote! { #id }
        }
        Some(v2::field_type::Kind::Array(array)) => {
            let element = array
                .element
                .as_ref()
                .map(|el| field_type_tokens(el, &format!("{hint}Element"), visibility, tuples))
                .unwrap_or_else(|| quote! { () });
            if array.min == array.max {
                let len = usize_tokens(array.min);
                quote! { [#element; #len] }
            } else {
                quote! { Vec<#element> }
            }
        }
        Some(v2::field_type::Kind::Map(map)) => {
            let key = map
                .key
                .as_ref()
                .map(|k| field_type_tokens(k, &format!("{hint}Key"), visibility, tuples))
                .unwrap_or_else(|| quote! { () });
            let value = map
                .value
                .as_ref()
                .map(|v| field_type_tokens(v, &format!("{hint}Value"), visibility, tuples))
                .unwrap_or_else(|| quote! { () });
            quote! { Vec<(#key, #value)> }
        }
        // A stream is an interaction-position type (ridl §12.3); it never
        // reaches a struct or tuple field in checked IR. Kept total.
        Some(v2::field_type::Kind::Stream(_)) | None => quote! { () },
    };

    if ft.optional {
        quote! { Option<#inner> }
    } else {
        inner
    }
}

fn inline_scalar_tokens(td: &v2::TypeDef) -> TokenStream {
    match backing_scalar(td) {
        ScalarBacking::Float => quote! { f64 },
        ScalarBacking::Integer => quote! { i64 },
        ScalarBacking::Boolean => quote! { bool },
        ScalarBacking::String => quote! { String },
        ScalarBacking::Bytes => quote! { Vec<u8> },
    }
}

pub(crate) fn primitive_tokens(prim: i32) -> TokenStream {
    match v2::PrimitiveType::try_from(prim).unwrap_or(v2::PrimitiveType::Unspecified) {
        v2::PrimitiveType::Boolean => quote! { bool },
        v2::PrimitiveType::Integer => quote! { i64 },
        v2::PrimitiveType::Float => quote! { f64 },
        v2::PrimitiveType::String => quote! { String },
        v2::PrimitiveType::Bytes => quote! { Vec<u8> },
        v2::PrimitiveType::Unspecified => quote! { () },
    }
}

/// A resolved type reference: a bare `Ident` for a same-package name, a
/// `crate::`-anchored path for a cross-package `pkg.Name` reference (typl §3.2).
/// The dotted package path maps directly to Rust module path segments. The
/// `crate::` anchor lets a consumer compose several generated packages as
/// sibling modules rooted at the crate — `crate::veh::common::Speed` resolves
/// from any module, whereas a bare `veh::common::Speed` only resolves from the
/// crate root (I4).
pub(crate) fn type_path(reference: &str) -> TokenStream {
    if reference.contains('.') {
        let segments = reference.split('.').map(ident);
        quote! { crate #(:: #segments)* }
    } else {
        let id = ident(reference);
        quote! { #id }
    }
}

/// The Rust module-segment spelling of one typl package name segment: `mod`
/// becomes `r#mod`, `crate` becomes `crate_`, and an ordinary segment is
/// returned unchanged.
///
/// This exists so that the module tree `ridlc` writes for `--emit rust` and
/// the paths [`type_path`] emits cannot drift apart. Both spell a package
/// segment through [`ident`], which is the only definition of that spelling.
/// A tree that writes a segment raw emits `pub mod mod;`, which does not
/// parse, and a tree that escapes a keyword differently from the reference
/// emits a module the reference cannot name.
pub fn module_segment(segment: &str) -> String {
    ident(segment).to_string()
}

// ---------------------------------------------------------------------------
// Scalar backing classification (shared by emission and default derivation).
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScalarBacking {
    Float,
    Integer,
    Boolean,
    String,
    Bytes,
}

/// The Rust-layer scalar class of a type definition's backing. A unit backing
/// implies float (typl §5.1).
pub(crate) fn backing_scalar(td: &v2::TypeDef) -> ScalarBacking {
    match td.backing.as_ref().and_then(|b| b.kind.as_ref()) {
        Some(v2::backing::Kind::Unit(_)) => ScalarBacking::Float,
        Some(v2::backing::Kind::Primitive(prim)) => {
            match v2::PrimitiveType::try_from(*prim).unwrap_or(v2::PrimitiveType::Unspecified) {
                v2::PrimitiveType::Boolean => ScalarBacking::Boolean,
                v2::PrimitiveType::Integer => ScalarBacking::Integer,
                v2::PrimitiveType::Float => ScalarBacking::Float,
                v2::PrimitiveType::String => ScalarBacking::String,
                v2::PrimitiveType::Bytes | v2::PrimitiveType::Unspecified => ScalarBacking::Bytes,
            }
        }
        None => ScalarBacking::Float,
    }
}

/// The backing class of a same-package named scalar type, or `None` when the
/// reference does not name a scalar `TypeDef` in this package.
pub(crate) fn same_package_scalar_backing(ctx: &Ctx, reference: &str) -> Option<ScalarBacking> {
    match &ctx.lookup(reference)?.kind {
        Some(v2::decl::Kind::TypeDef(td)) => Some(backing_scalar(td)),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Attributes: docs, deprecation, visibility.
// ---------------------------------------------------------------------------

/// The attributes that precede a generated item: its doc comment first, then
/// its `#[derive(...)]`, then `#[deprecated]`. The derive sits under the doc
/// comment because that is where Rust is conventionally written; it sits above
/// `#[deprecated]` and the `#[repr(...)]` each emitter adds because a reader
/// looks for the trait list first.
fn decl_attrs(decl: &v1::Declaration, derived: &TokenStream) -> TokenStream {
    let doc = doc_attrs(&decl.doc);
    let deprecated = deprecated_attr(decl.deprecated.as_deref());
    quote! { #doc #derived #deprecated }
}

fn field_attrs(field: &v1::Field) -> TokenStream {
    let doc = doc_attrs(&field.doc);
    let deprecated = deprecated_attr(field.deprecated.as_deref());
    quote! { #doc #deprecated }
}

/// One `#[doc]` attribute per line; prettyplease renders these as `///`
/// comments. A leading space makes the rendered comment read `/// text`.
pub(crate) fn doc_attrs(doc: &str) -> TokenStream {
    if doc.is_empty() {
        return quote! {};
    }
    let lines = doc.split('\n').map(|line| {
        let text = format!(" {line}");
        quote! { #[doc = #text] }
    });
    quote! { #(#lines)* }
}

/// `@deprecated` maps to `#[deprecated]`; a present-but-empty reason (the IR's
/// `Some("")`) still emits the bare attribute (typl §14.2).
pub(crate) fn deprecated_attr(reason: Option<&str>) -> TokenStream {
    match reason {
        Some("") => quote! { #[deprecated] },
        Some(reason) => quote! { #[deprecated(note = #reason)] },
        None => quote! {},
    }
}

/// `internal` maps to `pub(crate)` — Rust's package-private mechanism
/// (ADR-0002 §8, ADR-0008 decision 7, typl §3.3). The rule is per declaration,
/// not per module: a package holding one `internal` and one public declaration
/// generates one `pub(crate)` item and one `pub` item.
///
/// It governs the item a declaration is realized as **and the auxiliary types
/// that item's shape induces**. A tuple in a field or an interaction position
/// generates a named struct of its own ([`emit_tuple_struct`]), and that struct
/// carries the visibility of the declaration that induced it, the way #160
/// derived one visibility per interface and applied it to all four of that
/// interface's names.
///
/// Until issue #167 the induced struct was fixed at `pub`, which is wrong in
/// both directions. It publishes the shape of a declaration the keyword hides —
/// the argument #160 made for the interface's own four names applies unchanged
/// to a fifth name the same declaration generates. And it does not compile: an
/// `internal` declaration may name `internal` declarations freely (typl §3.3),
/// so `internal struct Holder { t : (a : Hidden) }` puts a `pub(crate)` type in
/// a `pub` struct's field and rustc reports `private_interfaces`. The corpus
/// denies that lint by name, and `ridlc check` accepts the source, so the two
/// halves disagreed until the visibility was carried.
///
/// The reverse direction is closed by TYPL-005 on the **source** route: a
/// public declaration naming an `internal` one is rejected, so a `pub` induced
/// struct never holds a `pub(crate)` type. That is not the same as an invariant,
/// and the difference is load-bearing. Two declarations whose paths mangle to
/// one struct name reach the same state by a route TYPL-005 cannot see — one
/// declaration `internal`, the other public, no `internal` payload type
/// anywhere — and carrying a visibility onto a name two declarations share
/// would make a program that compiled today fail `private_interfaces`. That is
/// why [`tuple_collision`] refuses the collision instead: the invariant holds
/// because the state that breaks it is not generated, not because it cannot be
/// described.
pub(crate) fn vis_tokens(visibility: i32) -> TokenStream {
    match v2::Visibility::try_from(visibility).unwrap_or(v2::Visibility::Unspecified) {
        v2::Visibility::Internal => quote! { pub(crate) },
        _ => quote! { pub },
    }
}

// ---------------------------------------------------------------------------
// Literals and identifiers.
// ---------------------------------------------------------------------------

/// A Rust identifier for a typl name. typl names are always character-valid
/// identifiers (typl §2.3); the only conflict is a name that is a Rust keyword,
/// escaped here as a raw identifier (`r#override`). The four keywords that
/// cannot be raw identifiers (`crate`, `self`, `Self`, `super`) and the bare
/// underscore are mangled with a trailing underscore.
///
/// The call is total, per the codegen contract (ADR-0004 §5, and the
/// never-panics guarantee `ridlc::compile` documents). A valid typl name is
/// never empty, so an empty `name` only arrives from malformed IR — but the
/// backend is also reachable from the language server over half-written
/// source, so it must not panic. An empty name lowers to Rust's wildcard `_`.
/// It cannot collide with a real name: a typl name of `_` is mangled to `__`
/// on the branch above.
///
/// How far `_` is caught depends on the position, and the split is not
/// uniform:
///
/// - **Declaration-name positions** — a struct, enum, trait or type-alias
///   name, an enum variant, a `fn`, `static` or `mod` name, a trait or impl
///   method. `_` is rejected by the `syn::parse2` gate in [`generate`], which
///   returns a [`GenerateError`], so the malformed name is reported rather
///   than emitted.
/// - **Field and binding positions** — a struct field, a tuple-struct field, a
///   `fn` parameter, a `const` name. syn *accepts* `_` here
///   (`syn::Field::parse_named` calls `Ident::parse_any` once it peeks
///   `Token![_]`), so [`emit_field`] would emit `pub _: T` and the gate would
///   not catch it. `rustc` still rejects the field, so the output is never
///   silently valid, but no [`GenerateError`] is raised. A derived `Default`
///   usually catches it anyway, because the struct *expression* it builds has
///   no valid `Member` — but `defaults::struct_default` returns `None` for a
///   non-constructible field (a cross-package reference carrying a declared
///   init, for one), and then nothing is left to catch it.
///
/// A field-position empty name is unreachable today, and for a structural
/// reason rather than a lucky one: `ridl_syntax`'s `Parser::block_body` and
/// `Parser::param_list` announce a member only on `SyntaxKind::Ident`, so
/// `field_def`, `param`, `enum_value` and `union_arm` are never entered
/// without a name. `Parser::interface_body` is the exception — it announces
/// members by the *interaction keyword*, so the name can be missing — which is
/// precisely and only why interactions were the vulnerable site.
/// `generate_emits_an_empty_field_name_without_a_derivable_default` pins that
/// gap, so a regression that makes a nameless field reachable is visible
/// rather than silent.
pub(crate) fn ident(name: &str) -> Ident {
    if let Ok(parsed) = syn::parse_str::<Ident>(name) {
        return parsed;
    }
    if matches!(name, "crate" | "self" | "Self" | "super" | "_") {
        return Ident::new(&format!("{name}_"), Span::call_site());
    }
    if name.is_empty() {
        // `Ident::new_raw("")` and `Ident::new("")` both panic; `Ident::new`
        // accepts `_` (`Ident::new_raw` does not — `r#_` is not a raw
        // identifier).
        return Ident::new("_", Span::call_site());
    }
    Ident::new_raw(name, Span::call_site())
}

/// Numeric literal tokens from a canonical decimal string. The int/float kind
/// comes from the caller (derived from the backing width), never from the
/// string form: the IR drops the float form, so a float value can read `"0"`.
/// A float literal is given a decimal point so it stays a float in Rust.
pub(crate) fn numeric_tokens(value: &str, is_float: bool) -> TokenStream {
    let text = if is_float && !value.contains('.') && !value.contains('e') && !value.contains('E') {
        format!("{value}.0")
    } else {
        value.to_string()
    };
    text.parse().unwrap_or_else(|_| quote! { 0 })
}

fn int_tokens(value: i64) -> TokenStream {
    value.to_string().parse().unwrap_or_else(|_| quote! { 0 })
}

fn usize_tokens(value: u64) -> TokenStream {
    value.to_string().parse().unwrap_or_else(|_| quote! { 0 })
}

pub(crate) fn bool_tokens(value: &str) -> TokenStream {
    if value == "true" {
        quote! { true }
    } else {
        quote! { false }
    }
}

#[cfg(test)]
mod tests;
