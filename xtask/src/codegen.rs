//! Generates the typed AST (`crates/ridl-syntax/src/ast/generated.rs`) from
//! the grammar (`crates/ridl-syntax/family.ungram`) — ADR-0007 decision 1.
//!
//! The generator is deliberately small. It emits one struct per grammar
//! rule, cast from the `SyntaxKind` variant of the same name — referencing
//! `SyntaxKind::<Rule>` in every `cast` is one half of the node-list
//! assertion: a rule without a matching variant fails to compile. The
//! `syntax_kind_node_variants_match_the_grammar` test is the other half: a
//! `SyntaxKind` node variant without a matching grammar rule (other than the
//! rule-less `ErrorNode`) fails the test. Accessors follow rust-analyzer
//! conventions — children by cast, tokens by kind:
//!
//! - A rule that is a pure alternation of nodes (`Definition`, `Backing`,
//!   `StructMember`, `FieldType`) generates no struct; `ast.rs` defines it
//!   as a hand-written enum, and accessors reference that enum as their
//!   type.
//! - A node reference generates an `Option<N>` accessor named after its
//!   label (or the snake_case rule name); under `*` it becomes an
//!   `AstChildren<N>` accessor.
//! - When one rule references the same node type more than once under
//!   distinct labels, the accessors index by position — sound only while
//!   every occurrence except the last is mandatory. When an earlier
//!   occurrence is optional (the `Constraint` scalars), positional
//!   indexing gives the wrong child, so the generator emits nothing and
//!   `ast.rs` hand-writes token-anchored accessors instead.
//! - A token reference generates a first-token-of-kind accessor
//!   (`fn <name>_token`), deduplicated per rule.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use ungrammar::{Grammar, Rule};

/// The `crates/ridl-syntax` directory, resolved from this crate's location.
fn syntax_crate_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask sits in the workspace root")
        .join("crates/ridl-syntax")
}

fn grammar_path() -> PathBuf {
    syntax_crate_dir().join("family.ungram")
}

pub(crate) fn generated_path() -> PathBuf {
    syntax_crate_dir().join("src/ast/generated.rs")
}

/// Regenerates the committed file in place; returns its path.
pub(crate) fn write_generated() -> PathBuf {
    let path = generated_path();
    fs::write(&path, generate()).expect("write generated.rs");
    path
}

/// The token names `family.ungram` may use: the `SyntaxKind` variant each one
/// maps to, and the base of the generated accessor name.
fn token_info(name: &str) -> (&'static str, &'static str) {
    match name {
        "package" => ("PackageKw", "package"),
        "import" => ("ImportKw", "import"),
        "as" => ("AsKw", "as"),
        "internal" => ("InternalKw", "internal"),
        "type" => ("TypeKw", "type"),
        "const" => ("ConstKw", "const"),
        "struct" => ("StructKw", "struct"),
        "enum" => ("EnumKw", "enum"),
        "enumset" => ("EnumsetKw", "enumset"),
        "union" => ("UnionKw", "union"),
        "boolean" => ("BooleanKw", "boolean"),
        "integer" => ("IntegerKw", "integer"),
        "float" => ("FloatKw", "float"),
        "string" => ("StringKw", "string"),
        "bytes" => ("BytesKw", "bytes"),
        "true" => ("TrueKw", "true"),
        "false" => ("FalseKw", "false"),
        "step" => ("StepKw", "step"),
        "match" => ("MatchKw", "match"),
        "reserved" => ("ReservedKw", "reserved"),
        "error" => ("ErrorKw", "error"),
        "interface" => ("InterfaceKw", "interface"),
        "signal" => ("SignalKw", "signal"),
        "event" => ("EventKw", "event"),
        "command" => ("CommandKw", "command"),
        "query" => ("QueryKw", "query"),
        "final" => ("FinalKw", "final"),
        "require" => ("RequireKw", "require"),
        "ensure" => ("EnsureKw", "ensure"),
        "ident" => ("Ident", "ident"),
        "int_number" => ("IntNumber", "int_number"),
        "float_number" => ("FloatNumber", "float_number"),
        "string_lit" => ("String", "string_lit"),
        "regex" => ("Regex", "regex"),
        "duration" => ("Duration", "duration"),
        ":" => ("Colon", "colon"),
        "=" => ("Eq", "eq"),
        "[" => ("LBracket", "l_bracket"),
        "]" => ("RBracket", "r_bracket"),
        "{" => ("LBrace", "l_brace"),
        "}" => ("RBrace", "r_brace"),
        "(" => ("LParen", "l_paren"),
        ")" => ("RParen", "r_paren"),
        ".." => ("DotDot", "dotdot"),
        "." => ("Dot", "dot"),
        "/" => ("Slash", "slash"),
        "," => ("Comma", "comma"),
        "?" => ("Question", "question"),
        ";" => ("Semicolon", "semicolon"),
        "@" => ("At", "at"),
        "|" => ("Pipe", "pipe"),
        "<" => ("Lt", "lt"),
        ">" => ("Gt", "gt"),
        "%" => ("Percent", "percent"),
        "-" => ("Minus", "minus"),
        "||" => ("PipePipe", "pipepipe"),
        "&&" => ("AmpAmp", "ampamp"),
        "==" => ("EqEq", "eqeq"),
        "!=" => ("Neq", "neq"),
        "<=" => ("Le", "le"),
        ">=" => ("Ge", "ge"),
        "!" => ("Bang", "bang"),
        "+" => ("Plus", "plus"),
        "*" => ("Star", "star"),
        _ => panic!("family.ungram token {name:?} is missing from the codegen token table"),
    }
}

/// One node or token reference found while walking a rule body.
struct Entry {
    label: Option<String>,
    /// A node (rule) name or a token name.
    target: Target,
    repeated: bool,
    optional: bool,
}

enum Target {
    Node(String),
    Token(String),
}

/// Flattens a rule body into its references, tracking repetition (`*`) and
/// optionality (`?` or an alternation branch).
fn collect(
    grammar: &Grammar,
    rule: &Rule,
    label: Option<&str>,
    repeated: bool,
    optional: bool,
    out: &mut Vec<Entry>,
) {
    match rule {
        Rule::Labeled { label, rule } => {
            collect(grammar, rule, Some(label), repeated, optional, out);
        }
        Rule::Node(node) => out.push(Entry {
            label: label.map(str::to_owned),
            target: Target::Node(grammar[*node].name.clone()),
            repeated,
            optional,
        }),
        Rule::Token(token) => out.push(Entry {
            label: label.map(str::to_owned),
            target: Target::Token(grammar[*token].name.clone()),
            repeated,
            optional,
        }),
        Rule::Seq(rules) => {
            for rule in rules {
                collect(grammar, rule, None, repeated, optional, out);
            }
        }
        Rule::Alt(rules) => {
            for rule in rules {
                collect(grammar, rule, None, repeated, true, out);
            }
        }
        // `label:Rule?` and `label:Rule*` keep their label through the
        // wrapper; a label on a sequence or alternation does not name a
        // single reference and is dropped.
        Rule::Opt(rule) => collect(grammar, rule, label, repeated, true, out),
        Rule::Rep(rule) => collect(grammar, rule, label, true, optional, out),
    }
}

/// `true` for a rule that is a pure alternation of nodes — no struct is
/// generated; `ast.rs` provides the enum.
fn is_enum_rule(rule: &Rule) -> bool {
    match rule {
        Rule::Alt(alternatives) => alternatives
            .iter()
            .all(|alternative| matches!(alternative, Rule::Node(_))),
        _ => false,
    }
}

fn snake_case(name: &str) -> String {
    let mut out = String::new();
    for (i, ch) in name.chars().enumerate() {
        if ch.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

/// Generates the full text of `src/ast/generated.rs` from `family.ungram`.
pub(crate) fn generate() -> String {
    let grammar_text = fs::read_to_string(grammar_path()).expect("read family.ungram");
    let grammar: Grammar = grammar_text.parse().expect("family.ungram parses");

    let mut enum_rules: Vec<String> = Vec::new();
    let mut structs = TokenStream::new();
    let mut uses_children = false;

    for node in grammar.iter() {
        if is_enum_rule(&grammar[node].rule) {
            enum_rules.push(grammar[node].name.clone());
        }
    }

    for node in grammar.iter() {
        let data = &grammar[node];
        if is_enum_rule(&data.rule) {
            continue;
        }

        let mut entries = Vec::new();
        collect(&grammar, &data.rule, None, false, false, &mut entries);

        let mut accessors = TokenStream::new();

        // Node accessors. Merge same (label, type) occurrences, then decide
        // per type: single accessor, positional accessors, or hand-written.
        let mut merged: Vec<Entry> = Vec::new();
        for entry in entries
            .iter()
            .filter(|e| matches!(e.target, Target::Node(_)))
        {
            let Target::Node(node_name) = &entry.target else {
                unreachable!()
            };
            if let Some(existing) = merged.iter_mut().find(|m| {
                matches!(&m.target, Target::Node(n) if n == node_name) && m.label == entry.label
            }) {
                existing.repeated |= entry.repeated;
                existing.optional &= entry.optional;
            } else {
                merged.push(Entry {
                    label: entry.label.clone(),
                    target: Target::Node(node_name.clone()),
                    repeated: entry.repeated,
                    optional: entry.optional,
                });
            }
        }
        let node_types: Vec<String> = {
            let mut seen = Vec::new();
            for entry in &merged {
                let Target::Node(node_name) = &entry.target else {
                    unreachable!()
                };
                if !seen.contains(node_name) {
                    seen.push(node_name.clone());
                }
            }
            seen
        };
        for node_type in &node_types {
            let group: Vec<&Entry> = merged
                .iter()
                .filter(|m| matches!(&m.target, Target::Node(n) if n == node_type))
                .collect();
            let ty = format_ident!("{node_type}");
            if group.len() == 1 {
                let entry = group[0];
                let method_name = entry.label.clone().unwrap_or_else(|| snake_case(node_type));
                let method = format_ident!("{method_name}");
                if entry.repeated {
                    uses_children = true;
                    accessors.extend(quote! {
                        pub fn #method(&self) -> AstChildren<#ty> {
                            support::children(&self.syntax)
                        }
                    });
                } else {
                    accessors.extend(quote! {
                        pub fn #method(&self) -> Option<#ty> {
                            support::child(&self.syntax)
                        }
                    });
                }
            } else {
                // Positional accessors need every occurrence except the
                // last to be mandatory; otherwise positional indexing
                // gives the wrong child and ast.rs hand-writes
                // token-anchored accessors instead.
                let positional_is_sound = group.iter().all(|e| !e.repeated)
                    && group[..group.len() - 1].iter().all(|e| !e.optional);
                if !positional_is_sound {
                    continue;
                }
                for (index, entry) in group.iter().enumerate() {
                    let method_name = entry
                        .label
                        .clone()
                        .unwrap_or_else(|| format!("{}{index}", snake_case(node_type)));
                    let method = format_ident!("{method_name}");
                    accessors.extend(quote! {
                        pub fn #method(&self) -> Option<#ty> {
                            support::nth_child(&self.syntax, #index)
                        }
                    });
                }
            }
        }

        // Token accessors: first token of each kind, deduplicated per rule.
        let mut seen_tokens: Vec<String> = Vec::new();
        for entry in &entries {
            let Target::Token(token_name) = &entry.target else {
                continue;
            };
            if seen_tokens.contains(token_name) {
                continue;
            }
            seen_tokens.push(token_name.clone());
            let (kind_variant, method_base) = token_info(token_name);
            let kind = format_ident!("{kind_variant}");
            let method = format_ident!("{method_base}_token");
            accessors.extend(quote! {
                pub fn #method(&self) -> Option<SyntaxToken> {
                    support::token(&self.syntax, SyntaxKind::#kind)
                }
            });
        }

        let struct_name = format_ident!("{}", data.name);
        let doc = format!(
            " A `{}` node (`family.ungram` rule `{}`).",
            data.name, data.name
        );
        structs.extend(quote! {
            #[doc = #doc]
            #[derive(Debug, Clone, PartialEq, Eq, Hash)]
            pub struct #struct_name {
                syntax: SyntaxNode,
            }
            impl AstNode for #struct_name {
                fn cast(syntax: SyntaxNode) -> Option<Self> {
                    (syntax.kind() == SyntaxKind::#struct_name).then_some(Self { syntax })
                }
                fn syntax(&self) -> &SyntaxNode {
                    &self.syntax
                }
            }
        });
        if !accessors.is_empty() {
            structs.extend(quote! {
                impl #struct_name {
                    #accessors
                }
            });
        }
    }

    // ErrorNode is the one node kind with no grammar rule: error recovery
    // (task E1.2c) wraps arbitrary skipped tokens in it, so it has no
    // describable shape and is appended here unconditionally.
    structs.extend(quote! {
        /// An error-recovery node wrapping the tokens the parser skipped
        /// (task E1.2c). The one node kind with no rule in `family.ungram`.
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct ErrorNode {
            syntax: SyntaxNode,
        }
        impl AstNode for ErrorNode {
            fn cast(syntax: SyntaxNode) -> Option<Self> {
                (syntax.kind() == SyntaxKind::ErrorNode).then_some(Self { syntax })
            }
            fn syntax(&self) -> &SyntaxNode {
                &self.syntax
            }
        }
    });

    enum_rules.sort();
    let enum_idents: Vec<_> = enum_rules
        .iter()
        .map(|name| format_ident!("{name}"))
        .collect();
    // One `use` per item: single-item imports are stable under rustfmt, so
    // the committed file satisfies `cargo fmt --check` and the drift test at
    // the same time regardless of how long the enum list grows.
    let children_import = if uses_children {
        quote! { use super::AstChildren; }
    } else {
        quote! {}
    };
    let file: syn::File = syn::parse2(quote! {
        #children_import
        use super::AstNode;
        #(use super::#enum_idents;)*
        use super::support;
        use crate::syntax_kind::{SyntaxKind, SyntaxNode, SyntaxToken};

        #structs
    })
    .expect("generated code parses as a Rust file");

    let mut out = String::new();
    writeln!(
        out,
        "//! The typed AST node structs, generated from `family.ungram`.\n\
         //!\n\
         //! @generated by `cargo xtask codegen` — do not edit by hand. The\n\
         //! xtask drift test regenerates this file into a buffer and fails\n\
         //! when the committed text differs.\n",
    )
    .expect("write header");
    out.push_str(&prettyplease::unparse(&file));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The committed generated file must match what the grammar produces —
    /// the drift gate of ADR-0007 decision 1.
    #[test]
    fn committed_generated_ast_matches_the_grammar() {
        let fresh = generate();
        let committed = fs::read_to_string(generated_path()).unwrap_or_default();
        assert!(
            fresh == committed,
            "crates/ridl-syntax/src/ast/generated.rs is stale — run `cargo xtask codegen`",
        );
    }

    /// The `SyntaxKind` node variant names, in declaration order: every
    /// variant after the lexer's `Error` catch-all token, which is the last
    /// token before the node inventory in `syntax_kind.rs`.
    fn syntax_kind_node_variants() -> Vec<String> {
        let path = syntax_crate_dir().join("src/syntax_kind.rs");
        let text = fs::read_to_string(path).expect("read syntax_kind.rs");
        let file: syn::File = syn::parse_str(&text).expect("syntax_kind.rs parses");
        let variants = file
            .items
            .into_iter()
            .find_map(|item| match item {
                syn::Item::Enum(item) if item.ident == "SyntaxKind" => Some(item.variants),
                _ => None,
            })
            .expect("the SyntaxKind enum is present");
        let names: Vec<String> = variants.into_iter().map(|v| v.ident.to_string()).collect();
        let boundary = names
            .iter()
            .position(|name| name == "Error")
            .expect("the `Error` token variant marks the token/node boundary");
        names[boundary + 1..].to_vec()
    }

    /// The reverse of the cast-site guard: every `SyntaxKind` node variant
    /// must name a grammar rule, or be the rule-less `ErrorNode`. Together
    /// with the compile-time cast reference, this keeps the node inventory and
    /// `family.ungram` in exact correspondence.
    #[test]
    fn syntax_kind_node_variants_match_the_grammar() {
        let grammar_text = fs::read_to_string(grammar_path()).expect("read family.ungram");
        let grammar: Grammar = grammar_text.parse().expect("family.ungram parses");

        let mut expected: Vec<String> = grammar
            .iter()
            .filter(|node| !is_enum_rule(&grammar[*node].rule))
            .map(|node| grammar[node].name.clone())
            .collect();
        expected.push("ErrorNode".to_string());
        expected.sort();

        let mut actual = syntax_kind_node_variants();
        actual.sort();

        assert_eq!(
            actual, expected,
            "SyntaxKind node variants drifted from the grammar rules",
        );
    }
}
