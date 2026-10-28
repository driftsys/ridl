//! The `transcode_<s>_to_<d>` functions: one inherent function on a generated
//! type for each ordered pair of distinct listed encodings under which the
//! type implements `Payload`.
//!
//! The rule for "implements `Payload` under an encoding" is the codec
//! emitters' own: each codec reports the types it writes an implementation
//! for (`codec::package_items`, `codec_proto3::package_items`), and this
//! module reads that list instead of restating the rule.

use proc_macro2::TokenStream;
use quote::quote;

use crate::{WireEncoding, allow_dead_code_if_internal, ident};

/// A generated type that carries a `Payload` implementation for one encoding.
pub(crate) struct PayloadOwner {
    /// The generated Rust item's name.
    pub(crate) name: String,
    /// The declaration's visibility, as the IR numbers it.
    pub(crate) visibility: i32,
}

/// The transcode functions of the package, one `impl` block per type that has
/// at least one pair.
///
/// `owners` holds one list per distinct listed encoding, in the order the
/// encodings are listed. The result is empty when it holds fewer than two.
pub(crate) fn package_items(owners: &[(WireEncoding, Vec<PayloadOwner>)]) -> Vec<TokenStream> {
    if owners.len() < 2 {
        return Vec::new();
    }
    // Types in the order of the first encoding's list, then those only a later
    // encoding has, so the output order does not depend on a map.
    let mut names: Vec<(&str, i32)> = Vec::new();
    for (_, list) in owners {
        for owner in list {
            if !names.iter().any(|(name, _)| *name == owner.name) {
                names.push((&owner.name, owner.visibility));
            }
        }
    }
    let has = |encoding: WireEncoding, name: &str| {
        owners
            .iter()
            .any(|(e, list)| *e == encoding && list.iter().any(|owner| owner.name == name))
    };
    let mut items = Vec::new();
    for (name, visibility) in names {
        let mut functions = Vec::new();
        for &(source, _) in owners {
            for &(target, _) in owners {
                if source != target && has(source, name) && has(target, name) {
                    functions.push(function(source, target));
                }
            }
        }
        if functions.is_empty() {
            continue;
        }
        let ty = ident(name);
        items.push(allow_dead_code_if_internal(
            visibility,
            quote! {
                #[allow(deprecated)]
                impl #ty { #(#functions)* }
            },
        ));
    }
    items
}

fn function(source: WireEncoding, target: WireEncoding) -> TokenStream {
    let name = ident(&format!(
        "transcode_{}_to_{}",
        source.name().replace('-', "_"),
        target.name().replace('-', "_")
    ));
    let source_path = encoding_path(source);
    let target_path = encoding_path(target);
    let doc_lines = [
        format!(
            " Re-encodes a {} payload as a {} payload.",
            source.name(),
            target.name()
        ),
        String::new(),
        format!(
            " The bytes are verified as {}, decoded, and encoded as {} into `out`.",
            source.name(),
            target.name()
        ),
        " The body is [`::ridl_rt::payload::transcode`].".to_string(),
    ];
    quote! {
        #( #[doc = #doc_lines] )*
        pub fn #name<'o>(
            bytes: &[::core::primitive::u8],
            out: &'o mut [::core::primitive::u8],
        ) -> ::core::result::Result<
            ::ridl_rt::payload::Ref<'o, Self, #target_path>,
            ::ridl_rt::payload::TranscodeError,
        > {
            ::ridl_rt::payload::transcode::<Self, #source_path, #target_path>(bytes, out)
        }
    }
}

fn encoding_path(encoding: WireEncoding) -> TokenStream {
    match encoding {
        WireEncoding::FlatBuffers => quote! { ::ridl_rt::encoding::FlatBuffers },
        WireEncoding::Proto3 => quote! { ::ridl_rt::encoding::Proto3 },
        WireEncoding::Podbuf => quote! { ::ridl_rt::encoding::Podbuf },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owners(names: &[&str]) -> Vec<PayloadOwner> {
        names
            .iter()
            .map(|name| PayloadOwner {
                name: (*name).to_string(),
                visibility: 0,
            })
            .collect()
    }

    /// The type names of the emitted `impl` blocks, in output order.
    fn impl_order(items: &[TokenStream]) -> Vec<String> {
        items
            .iter()
            .map(|item| {
                let text = item.to_string();
                let rest = text.split("impl ").nth(1).expect("an impl block");
                rest.split_whitespace().next().expect("a type").to_string()
            })
            .collect()
    }

    /// The names of the transcode functions of one item, in output order.
    fn function_order(item: &TokenStream) -> Vec<String> {
        item.to_string()
            .split("fn ")
            .skip(1)
            .map(|rest| rest.split_whitespace().next().expect("a name").to_string())
            .collect()
    }

    /// The owner lists of the two encodings: FlatBuffers lists the types in
    /// alphabetical order and proto3 in the reverse order.
    fn both_lists() -> (
        (WireEncoding, Vec<PayloadOwner>),
        (WireEncoding, Vec<PayloadOwner>),
    ) {
        (
            (
                WireEncoding::FlatBuffers,
                owners(&["Alpha", "Beta", "Gamma"]),
            ),
            (WireEncoding::Proto3, owners(&["Gamma", "Beta", "Alpha"])),
        )
    }

    fn assert_function_order(items: &[TokenStream], expected: [&str; 2]) {
        assert!(!items.is_empty());
        for item in items {
            assert_eq!(function_order(item), expected);
        }
    }

    #[test]
    fn the_output_follows_the_order_of_the_first_listed_encoding() {
        let (flatbuffers, proto3) = both_lists();
        let items = package_items(&[flatbuffers, proto3]);
        assert_eq!(impl_order(&items), ["Alpha", "Beta", "Gamma"]);
        assert_function_order(
            &items,
            [
                "transcode_flatbuffers_to_proto3",
                "transcode_proto3_to_flatbuffers",
            ],
        );

        let (flatbuffers, proto3) = both_lists();
        let items = package_items(&[proto3, flatbuffers]);
        assert_eq!(impl_order(&items), ["Gamma", "Beta", "Alpha"]);
        assert_function_order(
            &items,
            [
                "transcode_proto3_to_flatbuffers",
                "transcode_flatbuffers_to_proto3",
            ],
        );
    }

    #[test]
    fn a_type_owned_by_one_encoding_gets_no_transcode_function() {
        let flatbuffers = (WireEncoding::FlatBuffers, owners(&["Alpha", "Beta"]));
        let proto3 = (WireEncoding::Proto3, owners(&["Beta", "Gamma"]));
        let items = package_items(&[flatbuffers, proto3]);
        assert_eq!(impl_order(&items), ["Beta"]);
    }
}
