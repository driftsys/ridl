//! IR → catalog descriptor (spec D-4, D-6, D-9). One lowering writes the
//! whole file; the toolchain version stamped in it is this crate's. The
//! numbers and the retired list are the IR's (driver §4 answer 3).

use std::fmt;

use ridl_ir::v2::{
    Constraint, Decl, FieldType, Package, Param, PrimitiveType, ReturnType, StreamType, decl,
    field_type, return_type, stream_type,
};

use crate::hash::catalog_hash;
use crate::number::{ZeroNumber, numbered_shapes};
use crate::size::{Ctx, PayloadShape, SizeState, named_payload, size_state};
use crate::{
    Catalog, Encoding, Interface, Kind, MaxSize, Member, Payload, RetiredInterface, SCHEMA_VERSION,
    SizeStateTag, Timing, TimingMode, UnboundedCause,
};

/// Every encoding the size table has a column for, in `Encoding` order.
const COLUMNS: [Encoding; 3] = [Encoding::Proto3, Encoding::FlatBuffers, Encoding::ReprC];

/// Why a checked package could not be lowered: a defect upstream, not data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LowerError {
    /// An interface shape whose IR number is 0 (`ridl-sem` numbers every
    /// shape of a checked package, so this is an internal error).
    ZeroNumber(String),
}

impl fmt::Display for LowerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroNumber(name) => write!(f, "internal error: interface `{name}` has no number"),
        }
    }
}

impl std::error::Error for LowerError {}

impl From<ZeroNumber> for LowerError {
    fn from(err: ZeroNumber) -> Self {
        Self::ZeroNumber(err.0)
    }
}

/// Lowers `package` to the finished descriptor bytes; `others` are the
/// packages it imports, for name resolution and the hash closure.
pub fn lower(package: &Package, others: &[&Package]) -> Result<Vec<u8>, LowerError> {
    let numbered = numbered_shapes(package)?;
    let hash = catalog_hash(package, others);
    let ctx = Ctx::new(package, others);

    let interfaces = package
        .shapes()
        .zip(&numbered)
        .map(|(shape, numbered)| Interface {
            name: shape.name.to_owned(),
            number: numbered.number,
            provisional: numbered.provisional,
            members: shape
                .interface
                .interactions
                .iter()
                .filter_map(|decl| member_of(decl, &ctx))
                .collect(),
            reserved_ordinals: shape
                .interface
                .interactions
                .iter()
                .filter(|decl| matches!(decl.kind, Some(decl::Kind::ReservedSlot(_))))
                .map(|decl| decl.ordinal)
                .collect(),
        })
        .collect();

    let catalog = Catalog {
        version: SCHEMA_VERSION,
        name: package.name.clone(),
        hash: hash.to_vec(),
        toolchain: env!("CARGO_PKG_VERSION").to_owned(),
        interfaces,
        retired: package
            .retired
            .iter()
            .map(|entry| RetiredInterface {
                name: entry.name.clone(),
                number: entry.number,
            })
            .collect(),
    };
    Ok(crate::finish(&catalog))
}

fn member_of(decl: &Decl, ctx: &Ctx<'_>) -> Option<Member> {
    let (kind, payloads, timing) = match decl.kind.as_ref()? {
        decl::Kind::SignalDef(def) => (
            Kind::Signal,
            vec![payload(
                "value",
                def.payload.clone(),
                &PayloadShape::Named(&def.payload),
                ctx,
            )],
            def.timing.as_ref(),
        ),
        decl::Kind::EventDef(def) => (
            Kind::Event,
            vec![payload(
                "occurrence",
                def.payload.clone(),
                &PayloadShape::Named(&def.payload),
                ctx,
            )],
            def.timing.as_ref(),
        ),
        decl::Kind::CommandDef(def) => (
            Kind::Command,
            vec![request(&def.params, ctx)],
            def.timing.as_ref(),
        ),
        decl::Kind::QueryDef(def) => {
            let mut payloads = vec![request(&def.params, ctx)];
            if let Some(ret) = &def.return_type {
                payloads.push(response(ret, ctx));
            }
            (Kind::Query, payloads, def.timing.as_ref())
        }
        decl::Kind::FixedDef(def) => {
            let ty = def.payload.as_ref()?;
            (
                Kind::Fixed,
                vec![payload("value", spell(ty), &PayloadShape::Field(ty), ctx)],
                None,
            )
        }
        // A reserved slot is listed under `reserved_ordinals`; a type
        // declaration never sits in an interface body.
        decl::Kind::ReservedSlot(_)
        | decl::Kind::TypeDef(_)
        | decl::Kind::ConstDef(_)
        | decl::Kind::StructDef(_)
        | decl::Kind::EnumDef(_)
        | decl::Kind::EnumSetDef(_)
        | decl::Kind::UnionDef(_) => return None,
    };
    Some(Member {
        name: decl.name.clone(),
        ordinal: decl.ordinal,
        kind,
        payloads,
        timing: timing.map(|t| {
            Box::new(Timing {
                mode: match ridl_ir::v2::TimingMode::try_from(t.mode) {
                    Ok(ridl_ir::v2::TimingMode::StrictPeriodic) => TimingMode::StrictPeriodic,
                    Ok(ridl_ir::v2::TimingMode::Range) => TimingMode::Range,
                    _ => TimingMode::Unspecified,
                },
                min_us: t.min_us.clone(),
                max_us: t.max_us.clone(),
            })
        }),
    })
}

/// The request payload: one parameter is spelled as its type, several as
/// `(a: T, b: U)`; only one named parameter is sized (§4 answer 6).
fn request(params: &[Param], ctx: &Ctx<'_>) -> Payload {
    let name = match params {
        [single] => single.r#type.as_ref().map(spell).unwrap_or_default(),
        _ => format!(
            "({})",
            params
                .iter()
                .map(|p| format!(
                    "{}: {}",
                    p.name,
                    p.r#type.as_ref().map(spell).unwrap_or_default()
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    };
    payload("request", name, &PayloadShape::Params(params), ctx)
}

fn response(ret: &ReturnType, ctx: &Ctx<'_>) -> Payload {
    let name = match &ret.kind {
        Some(return_type::Kind::Value(ty)) => spell(ty),
        Some(return_type::Kind::Fallible(f)) => format!("{} | {}", f.ok, f.err),
        None => String::new(),
    };
    payload("response", name, &PayloadShape::Return(ret), ctx)
}

/// One row per encoding that has a state; none when the payload is not one
/// named type (§4 answers 6 and 10).
fn payload(role: &str, type_name: String, shape: &PayloadShape<'_>, ctx: &Ctx<'_>) -> Payload {
    let max_sizes = match named_payload(shape) {
        Some(name) => COLUMNS
            .iter()
            .filter_map(|&encoding| {
                size_state(name, ctx, encoding).map(|state| row(encoding, state))
            })
            .collect(),
        None => Vec::new(),
    };
    Payload {
        role: role.to_owned(),
        type_name,
        max_sizes,
    }
}

fn row(encoding: Encoding, state: SizeState) -> MaxSize {
    match state {
        SizeState::Bounded(bytes) => MaxSize {
            encoding,
            bytes,
            state: SizeStateTag::Bounded,
            cause: UnboundedCause::Unspecified,
        },
        SizeState::Unbounded(cause) => MaxSize {
            encoding,
            bytes: 0,
            state: SizeStateTag::Unbounded,
            cause,
        },
    }
}

/// The typl source form of a type for the descriptor's `type_name`: the
/// canonical name when there is one, the structural form otherwise — a tuple
/// `(a: T, b: U)` (typl §11), an exact-length array `[T; N]`, a bounded array
/// `[T; min..max]`, a map `[K: V; min..max]` (typl §12, in `ridl-fmt`'s
/// spacing), an inline scalar as its backing and constraint, such as
/// `km/h [0.0..250.0 step 0.5]` (typl §5.1-§5.4), and a stream `<T>`
/// (ridl §12). An optional type carries the `?` suffix (typl §7.1).
fn spell(ty: &FieldType) -> String {
    let base = match &ty.kind {
        Some(field_type::Kind::Named(name)) => name.clone(),
        Some(field_type::Kind::Primitive(p)) => spell_primitive(*p),
        Some(field_type::Kind::InlineScalar(def)) => {
            let backing = match def.backing.as_ref().and_then(|b| b.kind.as_ref()) {
                Some(ridl_ir::v2::backing::Kind::Primitive(p)) => spell_primitive(*p),
                Some(ridl_ir::v2::backing::Kind::Unit(unit)) => unit.clone(),
                None => String::new(),
            };
            match def.constraint.as_ref().map(spell_constraint) {
                Some(constraint) if !constraint.is_empty() => format!("{backing} {constraint}"),
                _ => backing,
            }
        }
        Some(field_type::Kind::Tuple(tuple)) => format!(
            "({})",
            tuple
                .fields
                .iter()
                .map(|f| format!(
                    "{}: {}",
                    f.name,
                    f.r#type.as_ref().map(spell).unwrap_or_default()
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Some(field_type::Kind::Array(array)) => format!(
            "[{}; {}]",
            array.element.as_deref().map(spell).unwrap_or_default(),
            spell_bound(array.min, array.max)
        ),
        Some(field_type::Kind::Map(map)) => format!(
            "[{}: {}; {}..{}]",
            map.key.as_deref().map(spell).unwrap_or_default(),
            map.value.as_deref().map(spell).unwrap_or_default(),
            map.min,
            map.max
        ),
        Some(field_type::Kind::Stream(stream)) => format!("<{}>", spell_stream(stream)),
        None => String::new(),
    };
    if ty.optional {
        format!("{base}?")
    } else {
        base
    }
}

/// An array's bound: `N` when the length is exact (`min == max`, as the IR
/// lowers `[T; N]`), `min..max` otherwise (typl §12.3).
fn spell_bound(min: u64, max: u64) -> String {
    if min == max {
        min.to_string()
    } else {
        format!("{min}..{max}")
    }
}

/// An inline scalar's constraint as the source writes it: a range
/// `[min..max step s]` with an absent bound left empty (typl §5.5), or a
/// length `[N]` or `[min..max]` (typl §5.3, §5.4), followed inside the
/// brackets by `match` and the regex constant's name or the regex literal,
/// which the IR keeps with its `/` delimiters. Empty when the constraint
/// states nothing.
fn spell_constraint(constraint: &Constraint) -> String {
    let mut parts = Vec::new();
    if constraint.min.is_some() || constraint.max.is_some() {
        let mut range = format!(
            "{}..{}",
            constraint.min.as_deref().unwrap_or_default(),
            constraint.max.as_deref().unwrap_or_default()
        );
        if let Some(step) = &constraint.step {
            range.push_str(&format!(" step {step}"));
        }
        parts.push(range);
    } else if constraint.len_min.is_some() || constraint.len_max.is_some() {
        parts.push(spell_bound(
            constraint.len_min.unwrap_or(0),
            constraint.len_max.unwrap_or(0),
        ));
    }
    if let Some(name) = &constraint.pattern_const {
        parts.push(format!("match {name}"));
    } else if let Some(pattern) = &constraint.pattern {
        parts.push(format!("match {pattern}"));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!("[{}]", parts.join(" "))
    }
}

fn spell_stream(stream: &StreamType) -> String {
    match &stream.element {
        Some(stream_type::Element::Named(name)) => name.clone(),
        Some(stream_type::Element::Primitive(p)) => spell_primitive(*p),
        None => String::new(),
    }
}

fn spell_primitive(primitive: i32) -> String {
    match PrimitiveType::try_from(primitive) {
        Ok(PrimitiveType::Boolean) => "boolean",
        Ok(PrimitiveType::Integer) => "integer",
        Ok(PrimitiveType::Float) => "float",
        Ok(PrimitiveType::String) => "string",
        Ok(PrimitiveType::Bytes) => "bytes",
        _ => "",
    }
    .to_owned()
}
