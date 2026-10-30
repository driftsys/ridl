//! IR v2 package to the podbuf C header (ADR-0031, the projection record of
//! this crate).
//!
//! The header lets a C consumer read a buffer that the gate has already
//! verified, with no library. It holds, per package:
//!
//! - the preamble, descriptor and helper definitions, each behind its own
//!   guard so that the headers of two packages can be included together;
//! - one `struct` per head, with `_Alignas(8)` on its first member and every
//!   gap written out as a `_padN` member, so that a C compiler lays the head
//!   out as the layout law does on every little-endian target;
//! - `_Static_assert` checks on `sizeof` and `offsetof` for every head and
//!   field, so a toolchain that disagrees fails the build;
//! - the constants of the enums, enum sets and union tags;
//! - inline accessor functions, `<owner>_<field>_at`, that read a field from a
//!   head of a size the caller gives and answer 0 (or `NULL`) for a field that
//!   lies past it, so a reader built from a newer type reads the data of an
//!   older producer; they exist for the head of a declaration and for the
//!   heads in the tail (array elements, map entries, union arms), and the
//!   head of a declaration also has `<owner>_<field>`, which reads a whole
//!   buffer through the preamble's `head_size`.
//!
//! The layout is not computed here. It is read from the model's podbuf
//! projection ([`v1::PodbufProjection`]), which `ridl_ir::projection::podbuf`
//! computes once for every emitter. A head that another package declares and
//! this package lays out inline is defined again in this header behind a
//! per-type guard macro, `<PKG>_<TYPE>_HEAD_DEFINED`, that the header of the
//! declaring package defines too.

use std::collections::{BTreeMap, HashSet};
use std::fmt::Write as _;

use ridl_ir::codegen::{self, v1};
use ridl_ir::name::snake_case;
use ridl_ir::projection::flatbuffers::Packages;
use ridl_ir::projection::podbuf as layout;
use ridl_ir::v2;

mod contract;

pub use contract::Backend;

#[cfg(test)]
mod tests;

/// The generated C header for one package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Generated {
    pub header_source: String,
}

/// A failure to generate a header from a package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerateError {
    pub message: String,
}

fn fail<T>(message: String) -> Result<T, GenerateError> {
    Err(GenerateError { message })
}

/// Generates the C header for `package` with no other package in scope.
pub fn generate(package: &v2::Package) -> Result<Generated, GenerateError> {
    generate_with(package, &[])
}

/// Generates the C header for `package`, resolving foreign references against
/// `others` (ADR-0017 decision 1). A foreign struct, tuple or union that a
/// field of `package` lays out inline gets its head defined in this header,
/// behind the same guard macro the header of its own package uses.
pub fn generate_with(
    package: &v2::Package,
    others: &[&v2::Package],
) -> Result<Generated, GenerateError> {
    generate_model(package, others, &codegen::lower(package, others))
}

pub(crate) fn generate_model(
    package: &v2::Package,
    others: &[&v2::Package],
    model: &v1::Model,
) -> Result<Generated, GenerateError> {
    check_limits(package, others)?;
    let Some(projection) = model.podbuf.as_ref() else {
        return fail("the model carries no podbuf projection".to_string());
    };
    if let Some(refused) = projection.refused.first() {
        let what = match refused.source {
            Some(v1::podbuf_refused::Source::Declaration(index)) => model
                .declarations
                .get(index as usize)
                .and_then(|decl| decl.name.as_ref())
                .map(|name| format!("`{}.{}`", package.name, name.declared)),
            Some(v1::podbuf_refused::Source::Tuple(index)) => model
                .tuples
                .get(index as usize)
                .and_then(|tuple| tuple.name.as_ref())
                .map(|name| format!("the tuple `{}`", name.wire)),
            None => None,
        }
        .unwrap_or_else(|| "a type".to_string());
        return fail(format!(
            "{what} cannot be laid out as a podbuf head: {}",
            refused.cause
        ));
    }
    Header::new(model, &projection.heads).render()
}

/// Refuses a struct or union whose largest buffer no `u32` offset can address,
/// or that holds an array or map past the descriptor limits. A diagnostic of
/// the checker names the same defect in source; this keeps the backend from
/// writing a header for a type the codec cannot encode.
fn check_limits(package: &v2::Package, others: &[&v2::Package]) -> Result<(), GenerateError> {
    let packages = Packages { package, others };
    for decl in &package.decls {
        let head = match &decl.kind {
            Some(v2::decl::Kind::StructDef(def)) => layout::struct_head(packages, &decl.name, def),
            Some(v2::decl::Kind::UnionDef(def)) => layout::union_head(packages, &decl.name, def),
            _ => continue,
        };
        // A head that cannot be laid out is reported from the model's
        // `refused` list, with its own cause.
        let Ok(head) = head else { continue };
        if layout::max_size(packages, decl).is_some() {
            continue;
        }
        let cause = if layout::exceeds_descriptor_limits(&head) {
            format!(
                "an array or map past the descriptor limits of {} elements and {} bytes per element",
                layout::MAX_COUNT,
                layout::MAX_ELEM_SIZE
            )
        } else {
            format!("a largest buffer above {} bytes", u32::MAX)
        };
        return fail(format!(
            "`{}.{}` has no podbuf size bound: {cause}",
            package.name, decl.name
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------
// Names
// ---------------------------------------------------------------------

/// Words that C reserves, which a field name must not take: the C11 keywords,
/// the C23 keywords (ISO/IEC 9899:2024 §6.4.1), which are spelled without the
/// underscore prefix of their C11 forms, so a typl name can reach them, and
/// `asm`, a keyword of the GNU dialects that GCC and Clang compile by default.
const C_KEYWORDS: &[&str] = &[
    "auto",
    "break",
    "case",
    "char",
    "const",
    "continue",
    "default",
    "do",
    "double",
    "else",
    "enum",
    "extern",
    "float",
    "for",
    "goto",
    "if",
    "inline",
    "int",
    "long",
    "register",
    "restrict",
    "return",
    "short",
    "signed",
    "sizeof",
    "static",
    "struct",
    "switch",
    "typedef",
    "union",
    "unsigned",
    "void",
    "volatile",
    "while",
    "bool",
    "true",
    "false",
    "NULL",
    "alignas",
    "alignof",
    "constexpr",
    "nullptr",
    "static_assert",
    "thread_local",
    "typeof",
    "typeof_unqual",
    "asm",
];

/// A field name as a C member name: the pinned snake case of ADR-0016, with a
/// trailing underscore when C reserves the word.
fn member_name(declared: &str) -> String {
    let snake = snake_case(declared);
    if C_KEYWORDS.contains(&snake.as_str()) {
        format!("{snake}_")
    } else {
        snake
    }
}

/// Whether the typl names of the tuple members of `head`, spelled as C
/// members, would clash in a way the header refuses: two members equal under
/// [`member_name`] (`minSpeed` and `min_speed`), or a member equal to the
/// presence test `has_<member>` of an optional member (`hasX` next to `x?`).
/// Such a head names its members by position instead. The decision reads the
/// head's own fields only, so the header that declares the tuple and a header
/// that lays it out inline make the same choice. The names are compared
/// without the trailing underscore of a reserved word, as the accessor names
/// are built.
fn spelled_tuple_names_clash(head: &v1::PodbufHead) -> bool {
    use v1::podbuf_source::Kind;
    let members: Vec<(String, bool)> = head
        .fields
        .iter()
        .filter_map(|field| {
            let source = field.source.as_ref()?;
            if !matches!(source.kind, Some(Kind::Position(_))) || source.position_name.is_empty() {
                return None;
            }
            let spelled = member_name(&source.position_name);
            Some((
                spelled.trim_end_matches('_').to_string(),
                field.optional.is_some(),
            ))
        })
        .collect();
    let mut seen = HashSet::new();
    let repeated = members.iter().any(|(name, _)| !seen.insert(name.as_str()));
    let presence = members.iter().any(|(name, _)| {
        members
            .iter()
            .any(|(other, optional)| *optional && *name == format!("has_{other}"))
    });
    repeated || presence
}

/// `veh.cluster` as `veh_cluster`: each segment in snake case.
fn package_prefix(dotted: &str) -> String {
    dotted
        .split('.')
        .map(snake_case)
        .collect::<Vec<_>>()
        .join("_")
}

/// The segments of a path into snake case, a dotted segment split first.
fn path_words(path: &[String]) -> String {
    path.iter()
        .flat_map(|segment| segment.split(['.', ' ']))
        .filter(|word| !word.is_empty())
        .map(snake_case)
        .collect::<Vec<_>>()
        .join("_")
}

/// The names the header claims at file scope, so that two claims of one name
/// are refused instead of written.
#[derive(Default)]
struct Claims {
    by_name: BTreeMap<String, String>,
}

impl Claims {
    fn claim(&mut self, name: &str, what: String) -> Result<(), GenerateError> {
        match self.by_name.get(name) {
            Some(first) => fail(format!(
                "the C name `{name}` is claimed by {first} and by {what}"
            )),
            None => {
                self.by_name.insert(name.to_string(), what);
                Ok(())
            }
        }
    }
}

// ---------------------------------------------------------------------
// The header
// ---------------------------------------------------------------------

struct Header<'a> {
    model: &'a v1::Model,
    heads: &'a [v1::PodbufHead],
    /// `veh_cluster`.
    prefix: String,
    /// The C type name of each head.
    type_names: Vec<String>,
    /// Whether each head names its tuple members by position: see
    /// [`spelled_tuple_names_clash`].
    positional: Vec<bool>,
}

impl<'a> Header<'a> {
    fn new(model: &'a v1::Model, heads: &'a [v1::PodbufHead]) -> Self {
        let prefix = model
            .name
            .as_ref()
            .map(|name| package_prefix(&name.dotted))
            .unwrap_or_default();
        let mut header = Self {
            model,
            heads,
            prefix,
            type_names: Vec::new(),
            positional: heads.iter().map(spelled_tuple_names_clash).collect(),
        };
        header.type_names = (0..heads.len()).map(|at| header.type_name(at)).collect();
        header
    }

    fn declaration_snake(&self, index: u32) -> String {
        self.model
            .declarations
            .get(index as usize)
            .and_then(|decl| decl.name.as_ref())
            .map(|name| name.snake.clone())
            .unwrap_or_default()
    }

    /// The C type name of head `at`: `<package>_<type>_head` for a
    /// declaration, and a name built from the path to the head for the others.
    fn type_name(&self, at: usize) -> String {
        use v1::podbuf_head::Source;
        let prefix = &self.prefix;
        match self.heads[at].source.as_ref() {
            Some(Source::Declaration(index)) => {
                format!("{prefix}_{}_head", self.declaration_snake(*index))
            }
            Some(Source::Tuple(index)) => {
                let wire = self
                    .model
                    .tuples
                    .get(*index as usize)
                    .and_then(|tuple| tuple.name.as_ref())
                    .map(|name| name.wire.as_str())
                    .filter(|wire| !wire.is_empty());
                match wire {
                    Some(wire) => format!("{prefix}_{}_head", snake_case(wire)),
                    None => format!("{prefix}_tuple{index}_head"),
                }
            }
            Some(Source::MapEntry(entry)) => {
                let words = path_words(&entry.path);
                if is_foreign_path(&entry.path) {
                    format!("{words}_entry_head")
                } else {
                    format!("{prefix}_{words}_entry_head")
                }
            }
            Some(Source::Arm(arm)) => match arm.foreign {
                Some(index) => {
                    let (package, name) = self.foreign_names(index);
                    format!("{package}_{name}_{}_arm_head", self.arm_snake(arm, true))
                }
                None => format!(
                    "{prefix}_{}_{}_arm_head",
                    self.declaration_snake(arm.declaration),
                    self.arm_snake(arm, false)
                ),
            },
            Some(Source::Foreign(index)) => {
                let (package, name) = self.foreign_names(*index);
                format!("{package}_{name}_head")
            }
            Some(Source::NestedTuple(nested)) => format!("{}_head", path_words(&nested.path)),
            None => format!("{prefix}_head{at}"),
        }
    }

    /// The package prefix and the snake case name of a foreign declaration.
    fn foreign_names(&self, index: u32) -> (String, String) {
        self.model
            .foreign
            .get(index as usize)
            .map(|foreign| {
                (
                    package_prefix(&foreign.package),
                    foreign
                        .declaration
                        .as_ref()
                        .and_then(|decl| decl.name.as_ref())
                        .map(|name| name.snake.clone())
                        .unwrap_or_default(),
                )
            })
            .unwrap_or_default()
    }

    fn arm_snake(&self, arm: &v1::ArmHead, foreign: bool) -> String {
        let declaration = if foreign {
            arm.foreign
                .and_then(|index| self.model.foreign.get(index as usize))
                .and_then(|foreign| foreign.declaration.as_ref())
        } else {
            self.model.declarations.get(arm.declaration as usize)
        };
        let arms = match declaration.and_then(|decl| decl.kind.as_ref()) {
            Some(v1::declaration::Kind::Union(union)) => union.arms.as_slice(),
            _ => &[],
        };
        arms.iter()
            .find(|candidate| candidate.ordinal == arm.arm)
            .and_then(|candidate| candidate.name.as_ref())
            .map(|name| name.snake.clone())
            .unwrap_or_else(|| format!("arm{}", arm.arm))
    }

    /// The guard macro of head `at`: `<PKG>_<TYPE>_HEAD_DEFINED`.
    fn guard(&self, at: usize) -> String {
        let name = self.type_names[at].to_uppercase();
        let stem = name.strip_suffix("_HEAD").unwrap_or(&name);
        format!("{stem}_HEAD_DEFINED")
    }

    // -----------------------------------------------------------------
    // Which heads, in which order
    // ---------------------------------------------------------------

    /// Whether head `at` belongs to this package. A head of another package is
    /// emitted only when a local head lays it out inline.
    fn is_local(&self, at: usize) -> bool {
        use v1::podbuf_head::Source;
        match self.heads[at].source.as_ref() {
            Some(Source::Declaration(_)) | Some(Source::Tuple(_)) => true,
            Some(Source::MapEntry(entry)) => !is_foreign_path(&entry.path),
            Some(Source::Arm(arm)) => arm.foreign.is_none(),
            Some(Source::Foreign(_)) | Some(Source::NestedTuple(_)) | None => false,
        }
    }

    /// The heads a head lays out inline, which the C compiler needs defined
    /// before it.
    fn inline_dependencies(&self, at: usize) -> Vec<usize> {
        let mut out = Vec::new();
        for field in &self.heads[at].fields {
            if let Some(form) = field.form.as_ref() {
                inline_heads(form, &mut out);
            }
        }
        out
    }

    /// The heads to define, dependencies first.
    fn emission_order(&self) -> Vec<usize> {
        fn visit(header: &Header<'_>, at: usize, seen: &mut HashSet<usize>, out: &mut Vec<usize>) {
            if !seen.insert(at) {
                return;
            }
            for dependency in header.inline_dependencies(at) {
                if dependency < header.heads.len() {
                    visit(header, dependency, seen, out);
                }
            }
            out.push(at);
        }
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for at in 0..self.heads.len() {
            if self.is_local(at) {
                visit(self, at, &mut seen, &mut out);
            }
        }
        out
    }

    // -----------------------------------------------------------------
    // Members
    // ---------------------------------------------------------------

    /// The member name of a field in head `at`.
    fn field_name(&self, at: usize, field: &v1::PodbufField) -> String {
        use v1::podbuf_head::Source as Owner;
        use v1::podbuf_source::Kind;
        let head = &self.heads[at];
        let Some(source) = field.source.as_ref() else {
            return format!("_field{}", field.offset);
        };
        match source.kind.as_ref() {
            Some(Kind::Field(named)) => member_name(&named.name),
            Some(Kind::Flags(index)) => format!("_flags{index}"),
            Some(Kind::Tag(_)) => "tag".to_string(),
            Some(Kind::Arm(_)) => match head.source {
                Some(Owner::Arm(_)) => "value".to_string(),
                _ => "arm".to_string(),
            },
            // A tuple member is named by its typl name, which the model
            // carries next to the position for the tuple of another package
            // too. The member falls back to its position when the head's
            // spelled names clash ([`spelled_tuple_names_clash`]), or when a
            // model written before `position_name` existed leaves it empty.
            Some(Kind::Position(position)) => {
                if self.positional[at] || source.position_name.is_empty() {
                    format!("f{position}")
                } else {
                    member_name(&source.position_name)
                }
            }
            Some(Kind::Key(_)) => "key".to_string(),
            Some(Kind::Value(_)) => "value".to_string(),
            None => format!("_field{}", field.offset),
        }
    }

    /// The C type of a form that is not an inline array: `None` for an inline
    /// head with no field, which takes no byte.
    fn base_type(&self, form: &v1::PodbufForm) -> Option<String> {
        use v1::podbuf_descriptor::Kind as Descriptor;
        use v1::podbuf_form::Kind;
        Some(match form.kind.as_ref()? {
            Kind::Boolean(_) => "uint8_t".to_string(),
            Kind::Int(int) => int_type(int.width, int.signed),
            Kind::Float(width) => float_type(*width),
            Kind::Enum(value) => int_type(value.width, value.signed),
            Kind::EnumSet(value) => int_type(value.width, false),
            Kind::InlineHead(index) => {
                let head = self.heads.get(*index as usize)?;
                if head.size == 0 {
                    return None;
                }
                self.type_names[*index as usize].clone()
            }
            Kind::InlineArray(array) => return self.base_type(array.element.as_ref()?),
            Kind::Descriptor(descriptor) => match descriptor.kind.as_ref()? {
                Descriptor::Array(_) | Descriptor::Map(_) => "ridl_podbuf_array_desc".to_string(),
                Descriptor::Text(_) | Descriptor::Bytes(_) | Descriptor::Arms(_) => {
                    "ridl_podbuf_desc".to_string()
                }
            },
        })
    }

    /// The array dimensions of a form, outermost first, and the form of the
    /// element.
    fn dimensions<'f>(&self, form: &'f v1::PodbufForm) -> (Vec<u32>, &'f v1::PodbufForm) {
        let mut dims = Vec::new();
        let mut current = form;
        while let Some(v1::podbuf_form::Kind::InlineArray(array)) = current.kind.as_ref() {
            dims.push(array.count);
            match array.element.as_deref() {
                Some(element) => current = element,
                None => break,
            }
        }
        (dims, current)
    }

    /// The member declaration of a field, or `None` when it takes no byte.
    fn member(&self, at: usize, field: &v1::PodbufField) -> Option<(String, String)> {
        let form = field.form.as_ref()?;
        let (dims, element) = self.dimensions(form);
        let base = self.base_type(element)?;
        let name = self.field_name(at, field);
        let mut declaration = format!("{base} {name}");
        for count in dims {
            let _ = write!(declaration, "[{count}]");
        }
        Some((name, declaration))
    }

    // -----------------------------------------------------------------
    // Rendering
    // ---------------------------------------------------------------

    fn render(&self) -> Result<Generated, GenerateError> {
        let mut claims = Claims::default();
        let guard = format!("RIDL_PODBUF_{}_H", self.prefix.to_uppercase());
        let mut out = String::new();
        let _ = writeln!(out, "#ifndef {guard}");
        let _ = writeln!(out, "#define {guard}\n");
        out.push_str(PREAMBLE);
        self.render_constants(&mut out, &mut claims)?;

        let order = self.emission_order();
        for &at in &order {
            self.render_head(&mut out, at, &mut claims)?;
        }
        for &at in &order {
            if self.is_local(at) && self.heads[at].size > 0 {
                self.render_accessors(&mut out, at, &mut claims)?;
            }
        }
        let _ = writeln!(out, "#endif /* {guard} */");
        Ok(Generated { header_source: out })
    }

    /// The constants of the enums, enum sets and union tags.
    fn render_constants(&self, out: &mut String, claims: &mut Claims) -> Result<(), GenerateError> {
        let package = self.prefix.to_uppercase();
        for decl in &self.model.declarations {
            let Some(name) = decl.name.as_ref() else {
                continue;
            };
            let owner = format!("{package}_{}", name.screaming);
            match decl.kind.as_ref() {
                Some(v1::declaration::Kind::Enum(value)) => {
                    for entry in &value.values {
                        let Some(spelled) = entry.name.as_ref() else {
                            continue;
                        };
                        let constant = format!("{owner}_{}", spelled.screaming);
                        claims
                            .claim(&constant, format!("the enum value `{}`", spelled.declared))?;
                        let _ = writeln!(out, "#define {constant} {}", int_literal(entry.value));
                    }
                }
                Some(v1::declaration::Kind::EnumSet(value)) => {
                    for entry in &value.bits {
                        let Some(spelled) = entry.name.as_ref() else {
                            continue;
                        };
                        let constant = format!("{owner}_{}", spelled.screaming);
                        claims.claim(&constant, format!("the flag `{}`", spelled.declared))?;
                        let _ =
                            writeln!(out, "#define {constant} (UINT64_C(1) << {})", entry.value);
                    }
                }
                Some(v1::declaration::Kind::Union(value)) => {
                    for arm in &value.arms {
                        let Some(spelled) = arm.name.as_ref() else {
                            continue;
                        };
                        let constant = format!("{owner}_TAG_{}", spelled.screaming);
                        claims.claim(&constant, format!("the union arm `{}`", spelled.declared))?;
                        let _ = writeln!(out, "#define {constant} {}", arm.ordinal);
                    }
                }
                _ => {}
            }
        }
        out.push('\n');
        Ok(())
    }

    /// One head: its guarded `struct` and its `_Static_assert` checks.
    fn render_head(
        &self,
        out: &mut String,
        at: usize,
        claims: &mut Claims,
    ) -> Result<(), GenerateError> {
        let head = &self.heads[at];
        if head.size == 0 {
            // A head with no field takes no byte, and C has no empty struct.
            return Ok(());
        }
        let name = &self.type_names[at];
        claims.claim(name, "a head".to_string())?;
        let guard = self.guard(at);

        // The members by offset, the pads among them.
        let mut members: Vec<Member> = Vec::new();
        for field in &head.fields {
            if let Some((name, declaration)) = self.member(at, field) {
                members.push(Member {
                    offset: field.offset,
                    declaration,
                    field: Some((name, field.size)),
                });
            }
        }
        for pad in &head.pads {
            members.push(Member {
                offset: pad.offset,
                declaration: format!("uint8_t _pad{}[{}]", pad.offset, pad.size),
                field: None,
            });
        }
        members.sort_by_key(|member| member.offset);
        let mut seen = HashSet::new();
        for member in &members {
            if let Some((field, _)) = &member.field
                && !seen.insert(field.clone())
            {
                return fail(format!(
                    "the head `{name}` has two members named `{field}` ({})",
                    member.declaration
                ));
            }
        }

        let _ = writeln!(out, "#ifndef {guard}");
        let _ = writeln!(out, "#define {guard}");
        let _ = writeln!(out, "typedef struct {name} {{");
        for (index, member) in members.iter().enumerate() {
            let align = if index == 0 { "_Alignas(8) " } else { "" };
            let _ = writeln!(out, "  {align}{};", member.declaration);
        }
        let _ = writeln!(out, "}} {name};");
        let _ = writeln!(out, "#endif");
        let _ = writeln!(
            out,
            "_Static_assert(sizeof({name}) == {}, \"{name} size\");",
            head.size
        );
        let _ = writeln!(
            out,
            "_Static_assert(_Alignof({name}) == 8, \"{name} alignment\");"
        );
        for member in &members {
            let Some((field, size)) = &member.field else {
                continue;
            };
            let offset = member.offset;
            let _ = writeln!(
                out,
                "_Static_assert(offsetof({name}, {field}) == {offset}, \"{name}.{field} offset\");"
            );
            let _ = writeln!(
                out,
                "_Static_assert(sizeof((({name} *)0)->{field}) == {size}, \"{name}.{field} size\");"
            );
        }
        out.push('\n');
        Ok(())
    }

    /// The accessors of a head: for each field that is not a flags byte, a
    /// function `<owner>_<field>_at` that reads the field from a head of a
    /// given size, and a presence test per optional field. The size is the
    /// producer's: the preamble's `head_size` for the root head, the
    /// descriptor's `elem_size` for an element or map entry, the arm
    /// descriptor's `len` for an arm. A field past it reads as 0, so a reader
    /// built from a newer type reads an older producer's data. The head of a
    /// declaration also gets `<owner>_<field>`, which reads a whole buffer.
    ///
    /// A function that follows a descriptor takes the buffer start too,
    /// because `off` counts from it.
    fn render_accessors(
        &self,
        out: &mut String,
        at: usize,
        claims: &mut Claims,
    ) -> Result<(), GenerateError> {
        use v1::podbuf_descriptor::Kind as Descriptor;
        use v1::podbuf_form::Kind;
        let head = &self.heads[at];
        let root = matches!(head.source, Some(v1::podbuf_head::Source::Declaration(_)));
        let owner = match head.source {
            Some(v1::podbuf_head::Source::Declaration(declaration)) => {
                format!("{}_{}", self.prefix, self.declaration_snake(declaration))
            }
            _ => {
                let name = &self.type_names[at];
                name.strip_suffix("_head").unwrap_or(name).to_string()
            }
        };
        let flags_offset = |index: u32| -> Option<u32> {
            head.fields.iter().find_map(|field| {
                let kind = field.source.as_ref()?.kind.as_ref()?;
                matches!(kind, v1::podbuf_source::Kind::Flags(found) if *found == index)
                    .then_some(field.offset)
            })
        };

        for field in &head.fields {
            let Some(form) = field.form.as_ref() else {
                continue;
            };
            let Some(kind) = form.kind.as_ref() else {
                continue;
            };
            let source_kind = field.source.as_ref().and_then(|s| s.kind.as_ref());
            if matches!(source_kind, Some(v1::podbuf_source::Kind::Flags(_))) {
                continue;
            }
            let member = self.field_name(at, field);
            let function = format!("{owner}_{}", member.trim_end_matches('_'));
            let offset = field.offset;
            let size = field.size;
            let inline_bytes =
                format!("  return ridl_podbuf_fits(sz, {offset}, {size}) ? h + {offset} : NULL;\n");

            // The return type, the extra parameters (with the names to pass
            // on) and the body of `_at`, which sees `buf`, `h` and `sz`.
            let (ret, params, names, uses_buf, body): (String, &str, &str, bool, String) =
                match kind {
                    Kind::Boolean(_) | Kind::Int(_) | Kind::Enum(_) | Kind::EnumSet(_) => {
                        let c_type = self.base_type(form).unwrap_or_default();
                        let body = format!(
                            "  return ({c_type})ridl_podbuf_load(h, sz, {offset}, {size});\n"
                        );
                        (c_type, "", "", false, body)
                    }
                    Kind::Float(width) => {
                        let (c_type, helper) = if *width == v1::FloatWidth::F32 as i32 {
                            ("float", "ridl_podbuf_load_f32")
                        } else {
                            ("double", "ridl_podbuf_load_f64")
                        };
                        let body = format!("  return {helper}(h, sz, {offset});\n");
                        (c_type.to_string(), "", "", false, body)
                    }
                    Kind::InlineHead(_) => {
                        if self.base_type(form).is_none() {
                            continue;
                        }
                        ("const uint8_t *".to_string(), "", "", false, inline_bytes)
                    }
                    Kind::InlineArray(_) => {
                        let (_, element) = self.dimensions(form);
                        if self.base_type(element).is_none() {
                            continue;
                        }
                        ("const uint8_t *".to_string(), "", "", false, inline_bytes)
                    }
                    Kind::Descriptor(descriptor) => match descriptor.kind.as_ref() {
                        Some(Descriptor::Text(_)) => (
                            "const char *".to_string(),
                            ", uint32_t *len",
                            ", len",
                            true,
                            format!(
                                "  ridl_podbuf_desc d = ridl_podbuf_desc_at(h, sz, {offset});\n  \
                             *len = d.len;\n  \
                             return d.len ? (const char *)(buf + d.off) : NULL;\n"
                            ),
                        ),
                        Some(Descriptor::Bytes(_)) | Some(Descriptor::Arms(_)) => (
                            "const uint8_t *".to_string(),
                            ", uint32_t *len",
                            ", len",
                            true,
                            format!(
                                "  ridl_podbuf_desc d = ridl_podbuf_desc_at(h, sz, {offset});\n  \
                             *len = d.len;\n  \
                             return d.len ? buf + d.off : NULL;\n"
                            ),
                        ),
                        Some(Descriptor::Array(_)) | Some(Descriptor::Map(_)) => (
                            "const uint8_t *".to_string(),
                            ", uint16_t *count, uint16_t *elem_size",
                            ", count, elem_size",
                            true,
                            format!(
                                "  ridl_podbuf_array_desc d = ridl_podbuf_array_desc_at(h, sz, {offset});\n  \
                             *count = d.len;\n  \
                             *elem_size = d.elem_size;\n  \
                             return d.len ? buf + d.off : NULL;\n"
                            ),
                        ),
                        None => continue,
                    },
                };
            let at_name = format!("{function}_at");
            claims.claim(&at_name, format!("the accessor of `{member}`"))?;
            let lead = if uses_buf {
                "const uint8_t *buf, const uint8_t *h, uint32_t sz"
            } else {
                "const uint8_t *h, uint32_t sz"
            };
            let _ = writeln!(
                out,
                "static inline {ret}{}{at_name}({lead}{params}) {{\n{body}}}\n",
                if ret.ends_with('*') { "" } else { " " }
            );
            if root {
                claims.claim(&function, format!("the accessor of `{member}`"))?;
                let pass = if uses_buf { "buf, " } else { "" };
                let _ = writeln!(
                    out,
                    "static inline {ret}{}{function}(const uint8_t *buf{params}) {{\n  \
                     return {at_name}({pass}buf + RIDL_PODBUF_PREAMBLE_SIZE, ridl_podbuf_head_size(buf){names});\n}}\n",
                    if ret.ends_with('*') { "" } else { " " }
                );
            }

            if let Some(bit) = field.optional.as_ref() {
                let Some(flags) = flags_offset(bit.flags) else {
                    continue;
                };
                let has = format!("{owner}_has_{}", member.trim_end_matches('_'));
                claims.claim(
                    &format!("{has}_at"),
                    format!("the presence test of `{member}`"),
                )?;
                let _ = writeln!(
                    out,
                    "static inline int {has}_at(const uint8_t *h, uint32_t sz) {{\n  \
                     return (int)((ridl_podbuf_load(h, sz, {flags}, 1) >> {}) & 1u);\n}}\n",
                    bit.bit
                );
                if root {
                    claims.claim(&has, format!("the presence test of `{member}`"))?;
                    let _ = writeln!(
                        out,
                        "static inline int {has}(const uint8_t *buf) {{\n  \
                         return {has}_at(buf + RIDL_PODBUF_PREAMBLE_SIZE, ridl_podbuf_head_size(buf));\n}}\n"
                    );
                }
            }
        }
        Ok(())
    }
}

/// One member of a head `struct`: a field or a pad.
struct Member {
    offset: u32,
    /// The C declaration, without its trailing semicolon.
    declaration: String,
    /// The member name and size of a field; `None` for a pad.
    field: Option<(String, u32)>,
}

/// Whether a path begins with the qualified name of another package's
/// declaration.
fn is_foreign_path(path: &[String]) -> bool {
    path.first().is_some_and(|first| first.contains('.'))
}

/// Every head index a form lays out inline, through inline arrays but not
/// through descriptors, whose data is in the tail.
fn inline_heads(form: &v1::PodbufForm, out: &mut Vec<usize>) {
    match form.kind.as_ref() {
        Some(v1::podbuf_form::Kind::InlineHead(index)) => out.push(*index as usize),
        Some(v1::podbuf_form::Kind::InlineArray(array)) => {
            if let Some(element) = array.element.as_deref() {
                inline_heads(element, out);
            }
        }
        _ => {}
    }
}

/// The C integer type of a width and signedness.
fn int_type(width: i32, signed: bool) -> String {
    let bits = match v1::IntWidth::try_from(width) {
        Ok(v1::IntWidth::U8 | v1::IntWidth::I8) => 8,
        Ok(v1::IntWidth::U16 | v1::IntWidth::I16) => 16,
        Ok(v1::IntWidth::U32 | v1::IntWidth::I32) => 32,
        _ => 64,
    };
    if signed {
        format!("int{bits}_t")
    } else {
        format!("uint{bits}_t")
    }
}

fn float_type(width: i32) -> String {
    if width == v1::FloatWidth::F32 as i32 {
        "float".to_string()
    } else {
        "double".to_string()
    }
}

/// A signed 64-bit constant as a C expression of the right type.
fn int_literal(value: i64) -> String {
    if value == i64::MIN {
        "(-INT64_C(9223372036854775807) - 1)".to_string()
    } else if i32::try_from(value).is_ok() {
        if value < 0 {
            format!("({value})")
        } else {
            value.to_string()
        }
    } else if value < 0 {
        format!("(-INT64_C({}))", -value)
    } else {
        format!("INT64_C({value})")
    }
}

/// The part of every header that does not depend on the package.
const PREAMBLE: &str = "\
#if __BYTE_ORDER__ != __ORDER_LITTLE_ENDIAN__
#error \"podbuf requires a little-endian target\"
#endif

#include <stddef.h>
#include <stdint.h>

#ifndef RIDL_PODBUF_PREAMBLE_DEFINED
#define RIDL_PODBUF_PREAMBLE_DEFINED

#define RIDL_PODBUF_PREAMBLE_SIZE 8

/* The first 8 bytes of every buffer. */
typedef struct ridl_podbuf_preamble {
  uint32_t head_size;
  uint32_t reserved;
} ridl_podbuf_preamble;
_Static_assert(sizeof(ridl_podbuf_preamble) == 8, \"ridl_podbuf_preamble size\");

/* A string, bytes or union arm: `off` counts from the buffer start. */
typedef struct ridl_podbuf_desc {
  uint32_t off;
  uint32_t len;
} ridl_podbuf_desc;
_Static_assert(sizeof(ridl_podbuf_desc) == 8, \"ridl_podbuf_desc size\");
_Static_assert(_Alignof(ridl_podbuf_desc) == 4, \"ridl_podbuf_desc alignment\");

/* An array or map: `len` counts elements, `elem_size` is the stride. */
typedef struct ridl_podbuf_array_desc {
  uint32_t off;
  uint16_t len;
  uint16_t elem_size;
} ridl_podbuf_array_desc;
_Static_assert(sizeof(ridl_podbuf_array_desc) == 8, \"ridl_podbuf_array_desc size\");
_Static_assert(_Alignof(ridl_podbuf_array_desc) == 4, \"ridl_podbuf_array_desc alignment\");

/* The head size the producer wrote. */
static inline uint32_t ridl_podbuf_head_size(const uint8_t *buf) {
  return (uint32_t)buf[0] | ((uint32_t)buf[1] << 8) | ((uint32_t)buf[2] << 16) |
         ((uint32_t)buf[3] << 24);
}

/* Whether `n` bytes at head offset `off` lie inside a head of `sz` bytes. */
static inline int ridl_podbuf_fits(uint32_t sz, uint32_t off, uint32_t n) {
  return (uint64_t)off + n <= sz;
}

/* A little-endian unsigned load from a head of `sz` bytes; 0 past it. */
static inline uint64_t ridl_podbuf_load(const uint8_t *h, uint32_t sz, uint32_t off, uint32_t n) {
  uint64_t value = 0;
  uint32_t i;
  if (!ridl_podbuf_fits(sz, off, n)) {
    return 0;
  }
  for (i = n; i > 0; i--) {
    value = (value << 8) | h[off + i - 1];
  }
  return value;
}

static inline float ridl_podbuf_load_f32(const uint8_t *h, uint32_t sz, uint32_t off) {
  union {
    uint32_t bits;
    float value;
  } pun;
  pun.bits = (uint32_t)ridl_podbuf_load(h, sz, off, 4);
  return pun.value;
}

static inline double ridl_podbuf_load_f64(const uint8_t *h, uint32_t sz, uint32_t off) {
  union {
    uint64_t bits;
    double value;
  } pun;
  pun.bits = ridl_podbuf_load(h, sz, off, 8);
  return pun.value;
}

static inline ridl_podbuf_desc ridl_podbuf_desc_at(const uint8_t *h, uint32_t sz, uint32_t off) {
  ridl_podbuf_desc d;
  d.off = (uint32_t)ridl_podbuf_load(h, sz, off, 4);
  d.len = (uint32_t)ridl_podbuf_load(h, sz, off + 4, 4);
  return d;
}

static inline ridl_podbuf_array_desc ridl_podbuf_array_desc_at(const uint8_t *h, uint32_t sz,
                                                               uint32_t off) {
  ridl_podbuf_array_desc d;
  d.off = (uint32_t)ridl_podbuf_load(h, sz, off, 4);
  d.len = (uint16_t)ridl_podbuf_load(h, sz, off + 4, 2);
  d.elem_size = (uint16_t)ridl_podbuf_load(h, sz, off + 6, 2);
  return d;
}

#endif

";
