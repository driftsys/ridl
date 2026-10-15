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

    pub(crate) fn interface(interface: &'a v1::Interface) -> Self {
        DocParts {
            doc: &interface.doc,
            links: &interface.links,
            see: &interface.see,
            since: &interface.since,
            rules: &interface.rules,
        }
    }

    pub(crate) fn interaction(interaction: &'a v1::Interaction) -> Self {
        DocParts {
            doc: &interaction.doc,
            links: &interaction.links,
            see: &interaction.see,
            since: &interaction.since,
            rules: &interaction.rules,
        }
    }

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
    module: String,
}

impl<'a> Links<'a> {
    pub(crate) fn new(model: &'a v1::Model, faced: &'a BTreeSet<u32>) -> Self {
        let package = model
            .scope
            .as_ref()
            .map(|scope| scope.package.as_str())
            .unwrap_or_default();
        let segments = package
            .split('.')
            .filter(|segment| !segment.is_empty())
            .map(ident);
        Links {
            model,
            faced,
            module: spelled(quote! { crate #(:: #segments)* }),
        }
    }

    /// The same targets, rendered from the module that holds this doc.
    pub(crate) fn in_module(&self, module: TokenStream) -> Self {
        Links {
            model: self.model,
            faced: self.faced,
            module: spelled(module),
        }
    }

    /// The rustdoc path of `target`, a canonical qualified name `pkg.Name`
    /// with an optional `.member`, or `None` when the target is not an item
    /// of this output or is `internal`. A constant the backend skips
    /// ([`crate::declaration_emitted`]) is not an item of this output.
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
    ///
    /// A path with a raw identifier segment (`r#move`) is `None` as well:
    /// rustdoc cuts an intra-doc path at `#` and cannot resolve it. So is a
    /// path through `__ridl_package`, which [`package_prefix`] writes for an
    /// item whose name a child package's module takes: that module is
    /// `#[doc(hidden)]`, rustdoc writes no page for an item reached only
    /// through it, and the item has no other public path.
    pub(crate) fn path(&self, target: &str) -> Option<String> {
        self.resolve(target)
            .filter(|path| !path.contains("r#") && !path.contains("__ridl_package"))
    }

    fn resolve(&self, target: &str) -> Option<String> {
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
            if decl.visibility == v1::Visibility::Internal as i32
                || !crate::declaration_emitted(self.model, owner, decl)
            {
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
            // The async `Client` has a method for a signal, a command and a
            // query only: an event is received through `Subscribe`, and a
            // fixed member has no method.
            if !matches!(
                interaction.shape,
                Some(
                    v1::interaction::Shape::Signal(_)
                        | v1::interaction::Shape::Command(_)
                        | v1::interaction::Shape::Query(_)
                )
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

/// The rendered lines of one carrier's doc comment, kept apart so that a
/// caller can put a generated sentence between the doc text and the
/// sections, and a `# Parameters` section among them ([`DocLines::render`]).
pub(crate) struct DocLines {
    /// The doc text, each link rewritten (see [`linked`]).
    body: Vec<String>,
    /// One `- <text>` bullet per rule, the text verbatim.
    rules: Vec<String>,
    /// One bullet per `see` target, a link or a code span.
    see: Vec<String>,
    /// One `Available since <v>.` line per `since` version.
    since: Vec<String>,
    /// The Rust module used to resolve shortcuts in the complete comment.
    module: String,
}

impl DocLines {
    pub(crate) fn new(parts: &DocParts<'_>, links: &Links<'_>) -> Self {
        let body = if parts.doc.is_empty() {
            Vec::new()
        } else {
            linked(parts.doc, parts.links, links)
                .split('\n')
                .map(|line| format!(" {line}"))
                .collect()
        };
        DocLines {
            body,
            module: links.module.clone(),
            rules: parts
                .rules
                .iter()
                .map(|rule| format!(" - {}", rule.text))
                .collect(),
            see: parts
                .see
                .iter()
                .map(|see| format!(" - {}", rendered_link(see, links)))
                .collect(),
            since: parts
                .since
                .iter()
                .map(|version| format!(" Available since {version}."))
                .collect(),
        }
    }

    /// The doc comment, as `#[doc]` attributes, which prettyplease renders
    /// as `///` lines. Each part is omitted when it is empty, and the blank
    /// line before a part is omitted when nothing precedes it:
    ///
    /// 1. the doc text;
    /// 2. `sentence`, the generator's own text for the item, as its own
    ///    paragraph;
    /// 3. `# Rules`, then the rule bullets;
    /// 4. `# Parameters`, then `params`, the bullets [`parameter_lines`]
    ///    gives;
    /// 5. `# See also`, then the `see` bullets;
    /// 6. the `Available since` lines.
    pub(crate) fn render(&self, sentence: Option<&str>, params: &[String]) -> TokenStream {
        let lines = self.lines(sentence, params);
        quote! { #(#[doc = #lines])* }
    }

    /// The lines [`DocLines::render`] emits, one per `#[doc]` attribute.
    fn lines(&self, sentence: Option<&str>, params: &[String]) -> Vec<String> {
        let mut lines: Vec<String> = self.body.clone();
        if let Some(sentence) = sentence {
            if !lines.is_empty() {
                lines.push(String::new());
            }
            lines.push(sentence.to_string());
        }
        section(&mut lines, "# Rules", &self.rules);
        section(&mut lines, "# Parameters", params);
        section(&mut lines, "# See also", &self.see);
        if !self.since.is_empty() {
            if !lines.is_empty() {
                lines.push(String::new());
            }
            lines.extend(self.since.iter().cloned());
        }
        shorten_links(lines, &self.module)
    }
}

/// Shortens after all sections are assembled: a reference definition in any
/// section can override a shortcut in another. Code-span labels distinguish
/// a resolved intra-doc target from a user reference definition without an
/// explicit path that rustdoc would diagnose as redundant.
fn shorten_links(lines: Vec<String>, module: &str) -> Vec<String> {
    use pulldown_cmark::{Event, LinkType, Parser, Tag};

    let mut doc = lines.join("\n");
    let mut parser = Parser::new(&doc).into_offset_iter();
    let mut replacements = Vec::new();
    while let Some((event, range)) = parser.next() {
        let definitions = parser.reference_definitions();
        let Event::Start(Tag::Link {
            link_type: LinkType::Inline,
            dest_url,
            ..
        }) = event
        else {
            continue;
        };
        let Some((owner, name)) = dest_url.rsplit_once("::") else {
            continue;
        };
        if owner != module || doc[range.clone()] != format!("[{name}]({dest_url})") {
            continue;
        }
        let mut label = name.to_string();
        if definitions.get(&label).is_some() {
            let mut ticks = String::from("`");
            loop {
                label = format!("{ticks}{name}{ticks}");
                if definitions.get(&label).is_none() {
                    break;
                }
                ticks.push('`');
            }
        }
        replacements.push((range, format!("[{label}]")));
    }
    if replacements.is_empty() {
        return lines;
    }
    for (range, replacement) in replacements.into_iter().rev() {
        doc.replace_range(range, &replacement);
    }
    doc.split('\n').map(str::to_string).collect()
}

/// The doc comment of one carrier with no generated sentence: the doc text,
/// `# Rules`, `# See also` and the `Available since` lines, as
/// [`DocLines::render`] orders them.
pub(crate) fn doc_block(parts: &DocParts<'_>, links: &Links<'_>) -> TokenStream {
    DocLines::new(parts, links).render(None, &[])
}

/// The bullets of a call's `# Parameters` section, which
/// [`DocLines::render`] puts under its heading, or none for a call with no
/// parameter. Each parameter is one ``- `<snake name>`: <doc>`` bullet, its
/// doc's links rewritten as the doc text's are and a further line of the doc
/// indented under the bullet; a parameter with no doc is
/// ``- `<snake name>` `` alone. Each of the parameter's rules is a nested
/// `- <text>` bullet under it, the text verbatim. A parameter's `see` and
/// `since` are not rendered here.
pub(crate) fn parameter_lines(params: &[v1::Param], links: &Links<'_>) -> Vec<String> {
    let mut lines = Vec::new();
    for param in params {
        let name = snake_of(param.name.as_ref());
        let parts = DocParts::param(param);
        if parts.doc.is_empty() {
            lines.push(format!(" - `{name}`"));
        } else {
            let text = linked(parts.doc, parts.links, links);
            let mut doc_lines = text.split('\n');
            let first = doc_lines.next().unwrap_or_default();
            lines.push(format!(" - `{name}`: {first}"));
            lines.extend(doc_lines.map(|line| format!("   {line}")));
        }
        lines.extend(parts.rules.iter().map(|rule| format!("   - {}", rule.text)));
    }
    lines
}

/// `heading` and `items` under it, or nothing when `items` is empty: a blank
/// line when something precedes, the heading, a blank line, then the items.
fn section(lines: &mut Vec<String>, heading: &str, items: &[String]) {
    if items.is_empty() {
        return;
    }
    if !lines.is_empty() {
        lines.push(String::new());
    }
    lines.push(format!(" {heading}"));
    lines.push(String::new());
    lines.extend(items.iter().cloned());
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
        // A zero-length link has no text to replace, and at the offset of a
        // link already replaced it would insert a second rendering.
        if link.len == 0 {
            continue;
        }
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

/// An explicit path for an emitted target, or a qualified code span when the
/// target has no linkable item. Shortening waits for the complete comment.
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

    /// A zero-length link has no text to replace: alone it leaves the text,
    /// and at the offset of a link already replaced it inserts nothing.
    #[test]
    fn a_zero_length_link_inserts_nothing() {
        let model = v1::Model::default();
        let faced = BTreeSet::new();
        let links = Links::new(&model, &faced);
        let doc = "in °C [X]";
        assert_eq!(linked(doc, &[link(7, 0, "a.X")], &links), doc);
        assert_eq!(
            linked(doc, &[link(7, 3, "a.X"), link(7, 0, "a.Y")], &links),
            "in °C `a.X`"
        );
    }

    /// A link that overlaps a link already replaced leaves the text that
    /// replacement wrote.
    #[test]
    fn a_link_overlapping_a_replaced_link_is_skipped() {
        let model = v1::Model::default();
        let faced = BTreeSet::new();
        let links = Links::new(&model, &faced);
        let doc = "see [A.B] now";
        assert_eq!(
            linked(doc, &[link(4, 5, "a.Whole"), link(6, 3, "a.Tail")], &links),
            "see [A`a.Tail` now"
        );
    }

    fn param(name: &str, doc: &str) -> v1::Param {
        v1::Param {
            name: Some(v1::Spellings {
                declared: name.to_string(),
                snake: name.to_string(),
                ..Default::default()
            }),
            doc: doc.to_string(),
            ..Default::default()
        }
    }

    /// A further line of a parameter's doc is indented under its bullet.
    #[test]
    fn a_multi_line_parameter_doc_is_indented_under_its_bullet() {
        let model = v1::Model::default();
        let faced = BTreeSet::new();
        let links = Links::new(&model, &faced);
        assert_eq!(
            parameter_lines(&[param("level", "The level.\nIn percent.")], &links),
            vec![
                " - `level`: The level.".to_string(),
                "   In percent.".to_string()
            ]
        );
    }

    /// An `Available since` line with nothing before it opens the doc: no
    /// blank line precedes it. With a doc text before it, one does.
    #[test]
    fn a_lone_since_line_has_no_blank_line_before_it() {
        let model = v1::Model::default();
        let faced = BTreeSet::new();
        let links = Links::new(&model, &faced);
        let since = vec!["0.2".to_string()];
        let parts = |doc| DocParts {
            doc,
            links: &[],
            see: &[],
            since: &since,
            rules: &[],
        };
        assert_eq!(
            DocLines::new(&parts(""), &links).lines(None, &[]),
            vec![" Available since 0.2.".to_string()]
        );
        assert_eq!(
            DocLines::new(&parts("A doc."), &links).lines(None, &[]),
            vec![
                " A doc.".to_string(),
                String::new(),
                " Available since 0.2.".to_string()
            ]
        );
    }

    /// With a doc text, a generated sentence, a rule, a parameter, a `see`
    /// target and a `since` version, the parts come in the documented
    /// order: text, sentence, `# Rules`, `# Parameters`, `# See also`, then
    /// the `Available since` line, one blank line between parts.
    #[test]
    fn the_parts_of_a_doc_come_in_the_documented_order() {
        let model = v1::Model::default();
        let faced = BTreeSet::new();
        let links = Links::new(&model, &faced);
        let rules = [v1::Rule {
            text: "range 0 ..= 9".to_string(),
            ..Default::default()
        }];
        let see = [link(0, 0, "a.X")];
        let since = ["0.2".to_string()];
        let parts = DocParts {
            doc: "A doc.",
            links: &[],
            see: &see,
            since: &since,
            rules: &rules,
        };
        let params = vec![" - `level`".to_string()];
        assert_eq!(
            DocLines::new(&parts, &links).lines(Some("Generated."), &params),
            [
                " A doc.",
                "",
                "Generated.",
                "",
                " # Rules",
                "",
                " - range 0 ..= 9",
                "",
                " # Parameters",
                "",
                " - `level`",
                "",
                " # See also",
                "",
                " - `a.X`",
                "",
                " Available since 0.2."
            ]
        );
    }

    /// With no doc text, the generated sentence opens the doc: no blank line
    /// precedes it. With a doc text, one does.
    #[test]
    fn a_generated_sentence_with_no_doc_text_has_no_blank_line_before_it() {
        let model = v1::Model::default();
        let faced = BTreeSet::new();
        let links = Links::new(&model, &faced);
        let parts = |doc| DocParts {
            doc,
            links: &[],
            see: &[],
            since: &[],
            rules: &[],
        };
        assert_eq!(
            DocLines::new(&parts(""), &links).lines(Some("Generated."), &[]),
            ["Generated."]
        );
        assert_eq!(
            DocLines::new(&parts("A doc."), &links).lines(Some("Generated."), &[]),
            [" A doc.", "", "Generated."]
        );
    }
}
