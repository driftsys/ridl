//! The doc comment of a generated item, from the doc fields the model carries
//! (ADR-0026, ADR-0028): the doc text with each resolved link rewritten as a
//! rustdoc intra-doc link, then a `# Rules` section, a `# See also` section,
//! and one `Available since` line per `@since` version.
//!
//! Everything here reads the [`v1::Model`] and nothing reads the IR: the
//! backend also runs as a plugin, which is handed the model alone.
//!
//! A link whose target is not an item of this output, or is an `internal`
//! item, renders as a code span of the target's qualified name, so that no
//! generated doc carries a link rustdoc cannot resolve.

use crate::{camel_of, declared, face, ident, package_prefix, pascal_of, snake_of, type_path};
use proc_macro2::TokenStream;
use quote::quote;
use ridl_ir::codegen::v1;
use std::collections::BTreeSet;

/// The doc fields of one carrier in the model.
pub(crate) struct DocParts<'a> {
    pub doc: &'a str,
    pub links: &'a [v1::DocLink],
    pub see: &'a [v1::DocLink],
    pub since: &'a [String],
    pub rules: &'a [v1::Rule],
}

impl<'a> DocParts<'a> {
    pub(crate) fn declaration(decl: &'a v1::Declaration) -> Self {
        DocParts {
            doc: &decl.doc,
            links: &decl.links,
            see: &decl.see,
            since: &decl.since,
            rules: &decl.rules,
        }
    }

    pub(crate) fn field(field: &'a v1::Field) -> Self {
        DocParts {
            doc: &field.doc,
            links: &field.links,
            see: &field.see,
            since: &field.since,
            rules: &field.rules,
        }
    }

    pub(crate) fn enum_value(value: &'a v1::EnumValue) -> Self {
        DocParts {
            doc: &value.doc,
            links: &value.links,
            see: &value.see,
            since: &value.since,
            rules: &value.rules,
        }
    }

    pub(crate) fn arm(arm: &'a v1::Arm) -> Self {
        DocParts {
            doc: &arm.doc,
            links: &arm.links,
            see: &arm.see,
            since: &arm.since,
            rules: &arm.rules,
        }
    }

    #[expect(
        dead_code,
        reason = "no emitter renders an interface doc through `doc_block` yet"
    )]
    pub(crate) fn interface(interface: &'a v1::Interface) -> Self {
        DocParts {
            doc: &interface.doc,
            links: &interface.links,
            see: &interface.see,
            since: &interface.since,
            rules: &interface.rules,
        }
    }

    #[expect(
        dead_code,
        reason = "no emitter renders an interaction doc through `doc_block` yet"
    )]
    pub(crate) fn interaction(interaction: &'a v1::Interaction) -> Self {
        DocParts {
            doc: &interaction.doc,
            links: &interaction.links,
            see: &interaction.see,
            since: &interaction.since,
            rules: &interaction.rules,
        }
    }

    #[expect(
        dead_code,
        reason = "no emitter renders a parameter doc through `doc_block` yet"
    )]
    pub(crate) fn param(param: &'a v1::Param) -> Self {
        DocParts {
            doc: &param.doc,
            links: &param.links,
            see: &param.see,
            since: &param.since,
            rules: &param.rules,
        }
    }
}

/// Resolves a doc link's target to the rustdoc path of an item of this
/// output.
///
/// `faced` holds the indices into `model.interfaces` of the interfaces whose
/// face module this output holds; a link to any other interface has no item
/// to point at.
pub(crate) struct Links<'a> {
    model: &'a v1::Model,
    faced: &'a BTreeSet<u32>,
}

impl<'a> Links<'a> {
    pub(crate) fn new(model: &'a v1::Model, faced: &'a BTreeSet<u32>) -> Self {
        Links { model, faced }
    }

    /// The rustdoc path of `target`, a canonical qualified name `pkg.Name`
    /// with an optional `.member`, or `None` when the target is not an item
    /// of this output or is `internal`.
    ///
    /// A declaration of the package or a foreign declaration it reaches is
    /// its [`type_path`]; a member of one is that path followed by the
    /// member's Rust name: a field's `snake_case`, an enum value's
    /// `pascal_case`, a union arm's `camel_case` (the spelling `emit_union`
    /// gives the variant), an enum set bit's declared name. An interface
    /// with a face module in this output is that module, and one of its
    /// interactions is the async client's method, `<module>::Client::<snake>`.
    ///
    /// The path is read while a `Ctx` is live, because [`type_path`] and
    /// [`package_prefix`] read the package names of the build from it.
    pub(crate) fn path(&self, target: &str) -> Option<String> {
        let package = self
            .model
            .scope
            .as_ref()
            .map(|scope| scope.package.as_str())
            .unwrap_or_default();
        let local = self.model.declarations.iter().map(|decl| (package, decl));
        let foreign = self.model.foreign.iter().filter_map(|foreign| {
            foreign
                .declaration
                .as_ref()
                .map(|decl| (foreign.package.as_str(), decl))
        });
        for (owner, decl) in local.chain(foreign) {
            let qualified = format!("{owner}.{}", declared(decl.name.as_ref()));
            let Some(rest) = target.strip_prefix(qualified.as_str()) else {
                continue;
            };
            if !rest.is_empty() && !rest.starts_with('.') {
                continue;
            }
            if decl.visibility == v1::Visibility::Internal as i32 {
                return None;
            }
            let Some(member) = rest.strip_prefix('.') else {
                return Some(spelled(type_path(&qualified)));
            };
            let member = member_name(decl, member)?;
            return Some(format!("{}::{member}", spelled(type_path(&qualified))));
        }

        for (index, interface) in self.model.interfaces.iter().enumerate() {
            let Some(v1::interface::Identity::Declared(name)) = interface.identity.as_ref() else {
                continue;
            };
            let qualified = format!("{package}.{}", name.declared);
            let Some(rest) = target.strip_prefix(qualified.as_str()) else {
                continue;
            };
            if !rest.is_empty() && !rest.starts_with('.') {
                continue;
            }
            let faced = u32::try_from(index).is_ok_and(|index| self.faced.contains(&index));
            if !faced || face::is_internal(interface) {
                return None;
            }
            let module = face::module_ident(interface);
            let prefix = package_prefix(package, &module);
            let module_path = spelled(quote! { #prefix #module });
            let Some(member) = rest.strip_prefix('.') else {
                return Some(module_path);
            };
            let interaction = interface
                .slots
                .iter()
                .filter_map(|slot| match slot.occupant.as_ref() {
                    Some(v1::interaction_slot::Occupant::Interaction(interaction)) => {
                        Some(interaction)
                    }
                    _ => None,
                })
                .find(|interaction| declared(interaction.name.as_ref()) == member)?;
            // A fixed member has no client method.
            if matches!(
                interaction.shape,
                None | Some(v1::interaction::Shape::Fixed(_))
            ) {
                return None;
            }
            let method = ident(snake_of(interaction.name.as_ref()));
            return Some(format!("{module_path}::Client::{method}"));
        }
        None
    }
}

/// The Rust name of the member `member` (its declared name) of `decl`, or
/// `None` when `decl` has no such member.
fn member_name(decl: &v1::Declaration, member: &str) -> Option<String> {
    use v1::declaration::Kind;
    let is = |name: Option<&v1::Spellings>| declared(name) == member;
    let name = match decl.kind.as_ref()? {
        Kind::Struct(st) => st
            .slots
            .iter()
            .find_map(|slot| match slot.occupant.as_ref() {
                Some(v1::slot::Occupant::Field(field)) if is(field.name.as_ref()) => {
                    Some(ident(snake_of(field.name.as_ref())))
                }
                _ => None,
            })?,
        Kind::Enum(en) => en
            .values
            .iter()
            .find(|value| is(value.name.as_ref()))
            .map(|value| ident(&pascal_of(value.name.as_ref())))?,
        Kind::EnumSet(set) => set
            .bits
            .iter()
            .find(|bit| is(bit.name.as_ref()))
            .map(|bit| ident(declared(bit.name.as_ref())))?,
        Kind::Union(union) => union
            .arms
            .iter()
            .find(|arm| is(arm.name.as_ref()))
            .map(|arm| ident(camel_of(arm.name.as_ref())))?,
        Kind::Scalar(_) | Kind::Constant(_) => return None,
    };
    Some(name.to_string())
}

/// A path as rustdoc reads it: the tokens with the spaces `TokenStream`'s
/// `Display` puts between them removed.
fn spelled(path: TokenStream) -> String {
    path.to_string().replace(' ', "")
}

/// The doc comment of one carrier, as `#[doc]` attributes, which prettyplease
/// renders as `///` lines. Each part is omitted when it is empty, and the
/// blank line before a section is omitted when nothing precedes it:
///
/// 1. `doc`, with each link rewritten (see [`linked`]);
/// 2. `# Rules`, then one `- <text>` bullet per rule, the text verbatim;
/// 3. `# See also`, then one bullet per `see` target, a link or a code span;
/// 4. one `Available since <v>.` line per `since` version.
pub(crate) fn doc_block(parts: &DocParts<'_>, links: &Links<'_>) -> TokenStream {
    let mut lines: Vec<String> = Vec::new();
    if !parts.doc.is_empty() {
        let text = linked(parts.doc, parts.links, links);
        lines.extend(text.split('\n').map(|line| format!(" {line}")));
    }
    if !parts.rules.is_empty() {
        section(&mut lines, "# Rules");
        lines.extend(parts.rules.iter().map(|rule| format!(" - {}", rule.text)));
    }
    if !parts.see.is_empty() {
        section(&mut lines, "# See also");
        lines.extend(
            parts
                .see
                .iter()
                .map(|see| format!(" - {}", rendered_link(see, links))),
        );
    }
    if !parts.since.is_empty() {
        if !lines.is_empty() {
            lines.push(String::new());
        }
        lines.extend(
            parts
                .since
                .iter()
                .map(|version| format!(" Available since {version}.")),
        );
    }
    quote! { #(#[doc = #lines])* }
}

/// A blank line when something precedes, the heading, then a blank line.
fn section(lines: &mut Vec<String>, heading: &str) {
    if !lines.is_empty() {
        lines.push(String::new());
    }
    lines.push(format!(" {heading}"));
    lines.push(String::new());
}

/// `doc` with the `len` bytes at each link's `offset` replaced by the link's
/// rendering, from the last link to the first, so that each earlier offset
/// stays valid. A link whose range is out of bounds, does not fall on a
/// character boundary, or overlaps a link already replaced leaves the text
/// unchanged rather than panicking.
fn linked(doc: &str, doc_links: &[v1::DocLink], links: &Links<'_>) -> String {
    let mut text = doc.to_string();
    let mut ordered: Vec<&v1::DocLink> = doc_links.iter().collect();
    ordered.sort_by_key(|link| std::cmp::Reverse(link.offset));
    let mut limit = doc.len();
    for link in ordered {
        let start = link.offset as usize;
        let Some(end) = start.checked_add(link.len as usize) else {
            continue;
        };
        if end > limit || text.get(start..end).is_none() {
            continue;
        }
        text.replace_range(start..end, &rendered_link(link, links));
        limit = start;
    }
    text
}

/// `[<text>](<path>)` when the target is an item of this output, and a code
/// span of the qualified target name otherwise.
fn rendered_link(link: &v1::DocLink, links: &Links<'_>) -> String {
    match links.path(&link.target) {
        Some(path) => format!("[{}]({path})", link.text),
        None => format!("`{}`", link.target),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link(offset: u32, len: u32, target: &str) -> v1::DocLink {
        v1::DocLink {
            text: "X".to_string(),
            offset,
            len,
            target: target.to_string(),
        }
    }

    /// An offset inside a multi-byte character, a range past the end, and
    /// a length that overflows leave the text as written.
    #[test]
    fn a_link_off_a_character_boundary_or_out_of_range_leaves_the_text() {
        let model = v1::Model::default();
        let faced = BTreeSet::new();
        let links = Links::new(&model, &faced);
        let doc = "in °C [X]";
        for bad in [
            link(4, 2, "a.X"),
            link(8, 9, "a.X"),
            link(1, u32::MAX, "a.X"),
        ] {
            assert_eq!(linked(doc, &[bad], &links), doc);
        }
        assert_eq!(linked(doc, &[link(7, 3, "a.X")], &links), "in °C `a.X`");
    }
}
